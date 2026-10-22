//! S22: the query layer over the object model.
//!
//! The shapes are R05's, not invented here: `QueryRequest` and `QueryResponse` come from
//! `pcscope_contracts::query`, and this module's job is to fill them from the stored model rather
//! than to decide what they should be. A response that R05 names is always produced — `Empty` for a
//! query that matched nothing, `Unsupported` for one that is not implemented — so the frontend can
//! tell "no rows" apart from "this does not exist" without guessing.
//!
//! **Paging is stable while data arrives.** A cursor carries the watermark it was issued under, and
//! the page window is bounded by that watermark rather than by the store's present size. Rows that
//! arrive afterwards are not spliced into a page already being walked; they appear on a fresh query,
//! which is a new watermark. Re-issuing a cursor from a different watermark is refused rather than
//! silently re-scoped.
use anyhow::Result;
use pcscope_contracts::adapter::{EvidenceRef, SourceAdapter};
use pcscope_contracts::query::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

#[path = "history_labels.rs"]
mod history_labels;
#[path="query_views.rs"]
mod query_views;
#[path = "state_compare.rs"]
mod state_compare;
use query_views::*;
pub(crate) use query_views::{build_summary,build_action,is_legacy_com_rule};

type HistoryLabelCache = std::collections::HashMap<i64, history_labels::ComMetadata>;
pub(crate) use history_labels::ProcessLabelCache;
fn clamp(limit: usize) -> usize {
    limit.clamp(1, MAX_PAGE)
}

/// What the store looked like when the query started, and which query it was.
///
/// The fingerprint is what makes a cursor belong to a query: a cursor handed back with different
/// filters or a different sort is a cursor from another walk, and answering it would splice two
/// page sequences together. The state part bounds the window, so rows arriving later stay out of a
/// walk already under way instead of being spliced into a page the caller has passed.
fn watermark(conn: &Connection, fingerprint: &str) -> Result<String> {
    let evidence: i64 = conn.query_row("SELECT coalesce(max(seq),0) FROM evidence", [], |r| r.get(0))?;
    let instances: i64 = conn.query_row("SELECT coalesce(max(instance_id),0) FROM object_instances", [], |r| r.get(0))?;
    let actions: i64 = conn.query_row("SELECT coalesce(max(action_row),0) FROM object_actions", [], |r| r.get(0))?;
    Ok(format!("{fingerprint}-e{evidence}i{instances}a{actions}"))
}

/// A short digest of everything that decides which rows a walk returns. `cursor` and `limit` are
/// excluded: they describe where the walk is, not which walk it is.
fn fingerprint<T: serde::Serialize>(filter: &T) -> String {
    let text = serde_json::to_string(filter).unwrap_or_default();
    format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(text.as_bytes()))[..12].to_owned()
}

/// A cursor: the watermark it belongs to and where the last page stopped.
fn encode_cursor(watermark: &str, key: &str, id: i64) -> String {
    format!("{watermark}~{key}~{id}")
}

/// The instance bound a watermark carries.
///
/// The window is bounded by how large the store was when the walk started — that is what keeps new
/// rows out of a page sequence already under way. The cursor's own id is only where the last page
/// stopped; using it as the bound would drop every row after it.
fn instance_bound(watermark: &str) -> i64 {
    watermark.split("-e").nth(1).and_then(|rest| rest.split('i').nth(1)).and_then(|value| value.split('a').next()).and_then(|value| value.parse().ok()).unwrap_or(i64::MAX)
}

/// The action bound a watermark carries, for the same reason the instance bound exists.
fn action_bound(watermark: &str) -> i64 {
    watermark.rsplit('a').next().and_then(|value|value.split('g').next()).and_then(|value| value.parse().ok()).unwrap_or(i64::MAX)
}

fn display_bound(watermark:&str)->Option<i64> {watermark.rsplit_once('g').and_then(|(_,value)|value.parse().ok())}

/// The query a watermark belongs to.
fn watermark_fingerprint(watermark: &str) -> &str {
    watermark.split("-e").next().unwrap_or_default()
}

fn decode_cursor(cursor: &str) -> Option<(String, String, i64)> {
    let mut parts = cursor.split('~');
    let watermark = parts.next()?.to_owned();
    let key = parts.next()?.to_owned();
    let id = parts.next()?.parse().ok()?;
    Some((watermark, key, id))
}

fn response(status: ResponseStatus, data: Value, page: Option<Page>, counts: Option<Counts>) -> QueryResponse {
    QueryResponse {
        contract_version: 1,
        catalog_version: pcscope_contracts::model::catalog_version().into(),
        status,
        data: Some(data),
        warnings: Vec::new(),
        page,
        counts,
        instance_total: None,
        mock: false,
    }
}

/// One request, answered. The dispatcher exists so every entry point reports the same envelope and
/// a caller cannot accidentally reach a query that skips the status rules.
pub fn answer(conn: &Connection, request: &QueryRequest) -> Result<QueryResponse> {
    match request {
        QueryRequest::TypeCatalog => types(conn),
        QueryRequest::StateCompare { from, until, watermark, type_id, cursor, limit } => state_compare::answer_at(conn, from, until, type_id.as_deref(), cursor.as_deref(), *limit, watermark.as_deref()),
        QueryRequest::InstanceList(filter) => instances(conn, filter),
        QueryRequest::InstanceDetail { instance } => self::instance(conn, instance),
        QueryRequest::InstanceActions { instance, limit, .. } => actions(conn, instance, *limit),
        QueryRequest::ActionTimeline(filter) => timeline(conn, filter),
        QueryRequest::ActionDetail { action_row } => action_detail(conn, *action_row),
        QueryRequest::EvidenceDetail { evidence } => self::evidence(conn, evidence),
        QueryRequest::SourceCoverage { source } => coverage(conn, source.as_deref()),
        QueryRequest::SourceReadings { source, cursor, limit } => source_readings(conn, source.as_deref(), cursor.as_deref(), *limit),
        QueryRequest::ProjectionStatus => projection(conn),
    }
}

/// The types the catalog declares, and whether any source feeds them.
///
/// `supplied_by_source` is false for a type no source reaches: the UI must show it as not collected
/// rather than as an empty collection, which is what the flag exists for.
pub fn types(conn: &Connection) -> Result<QueryResponse> {
    let mut statement = conn.prepare_cached("SELECT type_id,category,name,supplied_by_source FROM type_definitions ORDER BY category,type_id")?;
    let rows = statement.query_map([], |row| {
        Ok(json!({
            "type_id": row.get::<_, String>(0)?,
            "category": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?,
            "supplied_by_source": row.get::<_, i64>(3)? != 0,
        }))
    })?;
    let entries = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let status = if entries.is_empty() { ResponseStatus::Empty } else { ResponseStatus::Loaded };
    Ok(response(status, Value::Array(entries), None, None))
}

fn sort_sql(sort: InstanceSort,display_name:&str) -> (String, bool) {
    // The key column and whether the comparison runs descending. Every sort ends with instance_id
    // so two rows that tie on the key still have one stable order.
    match sort {
        InstanceSort::NameAsc => (display_name.to_owned(), false),
        InstanceSort::NameDesc => (display_name.to_owned(), true),
        InstanceSort::ChangedDesc => {
            // This key also appears in WHERE clauses for page boundaries and counts, where a
            // SELECT alias would not exist. Keep the projected value and every comparison equal.
            static KEY: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| format!("coalesce({CHANGED_AT_SQL},'')"));
            (KEY.to_string(), true)
        }
        InstanceSort::SeenDesc => ("last_observed_at".into(), true),
        InstanceSort::TypeAsc => ("type_id".into(), false),
    }
}

fn lifecycle_clause(filter: &LifecycleFilter) -> Option<&'static str> {
    match filter {
        // The default set: what is there, or was there and has not been seen since. History has to
        // be asked for.
        LifecycleFilter::Active => Some("i.lifecycle = 'active'"),
        LifecycleFilter::NotObserved => Some("i.lifecycle = 'not_observed'"),
        LifecycleFilter::Ended => Some("i.lifecycle = 'ended'"),
        LifecycleFilter::Removed => Some("i.lifecycle = 'removed'"),
        LifecycleFilter::Any => None,
    }
}

/// The instances matching a filter, one page at a time.
pub fn instances(conn: &Connection, filter: &InstanceFilter) -> Result<QueryResponse> {
    let display_name=if crate::display::available(conn)? {
        format!("coalesce((SELECT d.sort_name FROM display_objects d WHERE d.instance_id=i.instance_id
          AND d.revision='{}' AND NOT EXISTS(SELECT 1 FROM display_work w WHERE w.kind='object' AND w.id=i.instance_id)),{})",crate::display::revision(),display_name_sql())
    }else{display_name_sql().to_owned()};
    let (key, descending) = sort_sql(filter.sort,&display_name);
    let limit = clamp(filter.limit);
    // Where the walk is, and how big a page is, are not part of which walk it is.
    let mut probe = filter.clone();
    probe.cursor = None;
    probe.limit = 0;
    probe.include_counts = true;
    probe.include_instance_total = false;
    // A dictionary update may change displayed names and their ordering without adding data.
    // Its revision therefore belongs to an instance walk, independently of the data watermark.
    let walk = fingerprint(&(&probe,crate::registration_dictionary::revision()));
    let (walk_watermark, window, cursor_key, cursor_id) = match filter.cursor.as_deref() {
        Some(cursor) => match decode_cursor(cursor) {
            // A cursor from another walk is refused rather than re-scoped: silently answering it
            // would mix two page sequences into one and skip or repeat rows.
            Some((issued, key, id)) if watermark_fingerprint(&issued) == walk => (issued.clone(), Some(instance_bound(&issued)), key, id),
            _ => {
                return Ok(QueryResponse::rejected(
                    ResponseStatus::Unsupported,
                    "stale_cursor",
                    "this cursor belongs to a different query; start the walk again",
                    pcscope_contracts::model::catalog_version(),
                ))
            }
        },
        // A fresh walk starts here, and keeps its own watermark for as long as it runs.
        None => (watermark(conn, &walk)?, None, String::new(), 0),
    };

    let mut clauses = vec!["1=1".to_owned()];
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(instance) = window {
        clauses.push("i.instance_id <= ?".into());
        args.push(Box::new(instance));
    }
    if let Some(category) = &filter.category {
        clauses.push("i.category = ?".into());
        args.push(Box::new(category.clone()));
    }
    if let Some(type_id) = &filter.type_id {
        clauses.push("i.type_id = ?".into());
        args.push(Box::new(type_id.clone()));
    }
    if let Some(scope) = &filter.scope {
        clauses.push("i.scope = ?".into());
        args.push(Box::new(scope.clone()));
    }
    if let Some(status) = &filter.identity_status {
        clauses.push("i.identity_status = ?".into());
        args.push(Box::new(status.clone()));
    }
    if let Some(source) = &filter.source {
        clauses.push("EXISTS(SELECT 1 FROM source_bindings b WHERE b.instance_id=i.instance_id AND b.source=? AND b.valid_to IS NULL)".into());
        args.push(Box::new(source.clone()));
    }
    if let Some(search) = &filter.search {
        // A literal search over what the row itself states: the name, the key and the scope.
        clauses.push(format!("({display_name} LIKE ? OR i.native_key LIKE ? OR i.scope LIKE ?)"));
        let pattern = format!("%{search}%");
        args.push(Box::new(pattern.clone()));
        args.push(Box::new(pattern.clone()));
        args.push(Box::new(pattern));
    }
    if let Some(clause) = lifecycle_clause(filter.lifecycle.as_ref().unwrap_or(&LifecycleFilter::Any)) {
        clauses.push(clause.into());
    }

    // The page boundary. A descending sort walks the same tuple the other way, so the comparison
    // flips with it.
    let comparison = if descending { "<" } else { ">" };
    let cursor_clause = if filter.cursor.is_some() {
        format!("(({key} {comparison} ?) OR ({key} = ? AND i.instance_id > ?))")
    } else {
        String::new()
    };
    if !cursor_clause.is_empty() {
        clauses.push(cursor_clause);
        args.push(Box::new(cursor_key.clone()));
        args.push(Box::new(cursor_key));
        args.push(Box::new(cursor_id));
    }

    let order = if descending { "DESC" } else { "ASC" };
    let fast_names=matches!(filter.sort,InstanceSort::NameAsc|InstanceSort::NameDesc)
        &&crate::display::available(conn)?&&conn.query_row("SELECT revision=? AND object_cursor>=object_bound FROM display_build WHERE id=1",[crate::display::revision()],|r|r.get::<_,bool>(0))?;
    let fields=format!("i.instance_id,i.type_id,{ALIAS},i.category,
        coalesce((SELECT name FROM type_definitions t WHERE t.type_id=i.type_id),i.type_id),
        {display_name},i.scope,i.identity_status,i.lifecycle,{CHANGED_AT_SQL},i.last_observed_at,i.native_key");
    let sql = if fast_names {
        // Prepared names have their own order index. Changed rows are a small, complementary
        // set: they remain visible with fresh facts instead of disappearing until prepared.
        // Limit both sorted streams before merging, so no full-catalog sort is needed.
        let ready_clauses=clauses.join(" AND ").replace(&display_name,"d.sort_name");
        format!("WITH ready AS (
            SELECT d.instance_id,d.sort_name AS sort_key FROM display_objects d INDEXED BY display_objects_name_{}
            CROSS JOIN object_instances i ON i.instance_id=d.instance_id
            WHERE d.revision='{}' AND NOT EXISTS(SELECT 1 FROM display_work w WHERE w.kind='object' AND w.id=d.instance_id)
            AND {ready_clauses} ORDER BY d.sort_name {order},d.instance_id ASC LIMIT ?),
          changed AS (
            SELECT i.instance_id,{display_name} AS sort_key FROM display_work w
            CROSS JOIN object_instances i ON i.instance_id=w.id
            WHERE w.kind='object' AND {} ORDER BY sort_key {order},i.instance_id ASC LIMIT ?),
          page_rows AS (SELECT * FROM ready UNION ALL SELECT * FROM changed)
          SELECT {fields} FROM page_rows p CROSS JOIN object_instances i ON i.instance_id=p.instance_id
          ORDER BY p.sort_key {order},p.instance_id ASC LIMIT ?",
          if descending{"desc"}else{"asc"},crate::display::revision(),clauses.join(" AND "))
    } else {format!(
        "SELECT i.instance_id, i.type_id, {ALIAS}, i.category,
                coalesce((SELECT name FROM type_definitions t WHERE t.type_id=i.type_id), i.type_id),
                {display_name}, i.scope, i.identity_status, i.lifecycle,
                {CHANGED_AT_SQL},
                i.last_observed_at,i.native_key
         FROM object_instances i
         WHERE {}
         ORDER BY {key} {order}, i.instance_id ASC
         LIMIT ?",
        clauses.join(" AND ")
    )};
    let mut statement = conn.prepare_cached(&sql)?;
    let bound: Vec<&dyn rusqlite::ToSql> = args.iter().map(|value| value.as_ref()).collect();
    let mut all: Vec<&dyn rusqlite::ToSql> = bound.clone();
    let limit_plus_one = (limit + 1) as i64;
    all.push(&limit_plus_one);
    if fast_names {all.extend(bound);all.push(&limit_plus_one);all.push(&limit_plus_one);}
    let rows = statement.query_map(all.as_slice(), |row| summary_row(conn, row))?;
    let mut data = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    // One row beyond the page tells us whether there is another page, without counting the rest.
    let has_more = data.len() > limit;
    data.truncate(limit);
    let status = if data.is_empty() { ResponseStatus::Empty } else { ResponseStatus::Loaded };

    let page_watermark = walk_watermark.clone();
    let next_cursor = if has_more {
        let last = data.last().cloned().unwrap_or(Value::Null);
        let id = last["instance_id"].as_i64().unwrap_or(0);
        let key_value = match filter.sort {
            InstanceSort::NameAsc | InstanceSort::NameDesc => last["display_name"].as_str().unwrap_or_default().to_owned(),
            InstanceSort::TypeAsc => last["type_id"].as_str().unwrap_or_default().to_owned(),
            InstanceSort::SeenDesc => last["seen_at"].as_str().unwrap_or_default().to_owned(),
            InstanceSort::ChangedDesc => last["changed_at"].as_str().unwrap_or_default().to_owned(),
        };
        Some(encode_cursor(&page_watermark, &key_value, id))
    } else {
        None
    };

    let page = Page { next_cursor, limit, watermark: walk_watermark, has_more };
    let counts = if filter.include_counts {Some(counts_for(conn, &clauses, &args)?)}else{None};
    let instance_total=if filter.include_instance_total {
        Some(if let Some(counts)=&counts {counts.instances} else {
            let mut bound:Vec<&dyn rusqlite::ToSql>=args.iter().map(|value|value.as_ref()).collect();
            let sql=if fast_names&&filter.search.is_some() {
                // The covering name index also serves search totals. Reading cached names via
                // one scalar query per wide object row was slower than the page itself.
                let ready=clauses.join(" AND ").replace(&display_name,"d.sort_name");
                bound.extend(args.iter().map(|value|value.as_ref()));
                format!("SELECT
                    (SELECT count(*) FROM display_objects d INDEXED BY display_objects_name_asc
                     CROSS JOIN object_instances i ON i.instance_id=d.instance_id
                     WHERE d.revision='{}' AND {ready}
                     AND NOT EXISTS(SELECT 1 FROM display_work w WHERE w.kind='object' AND w.id=d.instance_id)) +
                    (SELECT count(*) FROM display_work w CROSS JOIN object_instances i ON i.instance_id=w.id
                     WHERE w.kind='object' AND {})",crate::display::revision(),clauses.join(" AND "))
            }else{format!("SELECT count(*) FROM object_instances i WHERE {}",clauses.join(" AND "))};
            conn.prepare_cached(&sql)?.query_row(bound.as_slice(),|row|row.get::<_,i64>(0))?.max(0) as u64
        })
    }else{None};
    let mut answer=response(status, Value::Array(data), Some(page), counts);
    answer.instance_total=instance_total;
    Ok(answer)
}

/// Counts over the same filter, per kind. They are never summed across kinds: an instance and an
/// action are not the same thing counted twice.
fn counts_for(conn: &Connection, clauses: &[String], args: &[Box<dyn rusqlite::ToSql>]) -> Result<Counts> {
    if clauses==["1=1"] && args.is_empty() && crate::display::available(conn)? {
        if let Some(counts)=crate::display_counts::counts(conn)? {return Ok(counts);}
    }
    let where_clause = clauses.join(" AND ");
    let bound: Vec<&dyn rusqlite::ToSql> = args.iter().map(|value| value.as_ref()).collect();
    // A reader never installs the optional index. Do not even prepare an INDEXED BY clause
    // for an old store: SQLite resolves its index name before evaluating a CASE condition.
    let indexed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='index' AND name=?)",
        [crate::model::OBSERVATION_ADDRESS_INDEX], |row| row.get(0),
    )?;
    let sql = counts_sql(&where_clause, indexed);
    Ok(conn.query_row(&sql, bound.as_slice(), |row| Ok(Counts {
        instances: row.get::<_, i64>(0)?.max(0) as u64,
        actions: row.get::<_, i64>(1)?.max(0) as u64,
        source_records: row.get::<_, i64>(2)?.max(0) as u64,
        evidence: row.get::<_, i64>(3)?.max(0) as u64,
    }))?)
}

fn counts_sql(where_clause: &str, indexed: bool) -> String {
    let by_subject = "SELECT count(DISTINCT r.evidence_id) FROM evidence_refs r
        WHERE r.subject_kind='observation' AND r.subject_id IN (SELECT subject_id FROM matched)";
    // One object's detail already has an exact primary key. Counting all citations merely
    // to choose a broad-query strategy caused cold detail reads to time out on large stores.
    let (addresses, evidence) = if indexed && where_clause != "i.instance_id = ?" {
        let index = crate::model::OBSERVATION_ADDRESS_INDEX;
        // UNION ALL carries at most one next address. MIN with a strict indexed lower bound
        // skips all repeated citations without an in-memory set or a temporary DISTINCT tree.
        let addresses = format!(
            ", sparse_probe(refs) AS MATERIALIZED (
                SELECT count(*) FROM (SELECT 1 FROM evidence_refs r
                    WHERE r.subject_kind='observation' AND r.subject_id IN (SELECT subject_id FROM matched) LIMIT 1000)
             ), route_scale(probe_limit) AS MATERIALIZED (
                SELECT max(10000,min(250000,((SELECT count(*) FROM evidence_refs)+3)/4))
             ), addresses(evidence_id) AS (
                SELECT min(evidence_id) FROM evidence_refs INDEXED BY {index} WHERE subject_kind='observation'
                UNION ALL
                SELECT (SELECT min(evidence_id) FROM evidence_refs INDEXED BY {index}
                        WHERE subject_kind='observation' AND evidence_id>a.evidence_id)
                FROM addresses a WHERE a.evidence_id IS NOT NULL
             )"
        );
        // Object count alone does not predict citation volume. A near-complete object selection
        // already benefits from address skipping. Otherwise probe up to one quarter of the
        // total reference count, bounded at 250k: the total uses SQLite's unfiltered Count
        // operation, while the probe uses the per-subject index. A fixed 100k cutoff incorrectly
        // routed small categories just above that cutoff through millions of unrelated citations.
        // The 10k floor avoids the global walk on small stores. These are route estimates only;
        // both branches retain exact counts. The CAST prevents address x subject seek fan-out.
        let evidence = format!(
            "CASE WHEN (SELECT count(*) FROM matched)=0 THEN 0
             WHEN (SELECT refs FROM sparse_probe)<1000 THEN ({by_subject})
             WHEN (SELECT count(*) FROM matched)*10 < (SELECT count(*) FROM object_instances)*9
               AND (SELECT count(*) FROM (
                SELECT 1 FROM evidence_refs r WHERE r.subject_kind='observation'
                  AND r.subject_id IN (SELECT subject_id FROM matched)
                  LIMIT (SELECT probe_limit FROM route_scale)
             ))<(SELECT probe_limit FROM route_scale) THEN ({by_subject}) ELSE (
             SELECT count(*) FROM addresses a WHERE a.evidence_id IS NOT NULL AND EXISTS(
                SELECT 1 FROM evidence_refs r INDEXED BY {index}
                WHERE r.subject_kind='observation' AND r.evidence_id=a.evidence_id
                  AND CAST(r.subject_id AS TEXT) IN (SELECT subject_id FROM matched)
             )) END"
        );
        (addresses, evidence)
    } else {
        (String::new(), format!("({by_subject})"))
    };
    // Preserve the exact original filter including the cursor window, but evaluate expensive
    // effective-name/history predicates only once for all four counts. Only numeric identifiers
    // and their canonical text form are materialized, never the long evidence addresses.
    format!(
        "WITH RECURSIVE matched(instance_id,subject_id) AS MATERIALIZED (
            SELECT i.instance_id,CAST(i.instance_id AS TEXT) FROM object_instances i WHERE {where_clause}
         ){addresses}
         SELECT (SELECT count(*) FROM matched),
                (SELECT count(*) FROM object_actions a WHERE a.instance_id IN (SELECT instance_id FROM matched)),
                (SELECT count(*) FROM source_bindings b WHERE b.valid_to IS NULL AND b.instance_id IN (SELECT instance_id FROM matched)),
                {evidence}"
    )
}

/// One instance in full: its summary, every field, a first page of actions and its bindings.
pub fn instance(conn: &Connection, reference: &str) -> Result<QueryResponse> {
    let display_name=display_name_sql();
    let id: Option<i64> = match reference.parse::<i64>() {
        Ok(id) => conn.query_row("SELECT instance_id FROM object_instances WHERE instance_id=?", params![id], |r| r.get(0)).optional()?,
        // An alias is what a reference carries before an id exists, and after one does.
        Err(_) => conn.query_row(&format!("SELECT instance_id FROM object_instances WHERE {ALIAS}=?"), params![reference], |r| r.get(0)).optional()?,
    };
    let Some(id) = id else {
        return Ok(QueryResponse::rejected(ResponseStatus::Empty, "unknown_instance", "no instance matches that reference", pcscope_contracts::model::catalog_version()));
    };
    let row_sql = format!(
        "SELECT i.instance_id, i.type_id, {ALIAS}, i.category,
                coalesce((SELECT name FROM type_definitions t WHERE t.type_id=i.type_id), i.type_id),
                {display_name}, i.scope, i.identity_status, i.lifecycle,
                {CHANGED_AT_SQL},
                i.last_observed_at,i.native_key
         FROM object_instances i WHERE i.instance_id=?"
    );
    let summary = conn.query_row(&row_sql, params![id], |row| summary_row(conn, row))?;
    let fields = key_facts_all(conn, id, &type_of(conn, id)?)?;
    let actions = action_views(conn, Some(id), None, DEFAULT_PAGE)?;
    let bindings = binding_views(conn, id)?;
    let counts = counts_for(conn, &["i.instance_id = ?".to_owned()], &[Box::new(id)])?;
    Ok(response(ResponseStatus::Loaded, json!({"summary": summary, "fields": fields, "actions_preview": actions.0, "bindings": bindings}), None, Some(counts)))
}

/// The type an instance is, which its field labels are looked up under.
fn type_of(conn: &Connection, instance_id: i64) -> Result<String> {
    Ok(conn.query_row("SELECT type_id FROM object_instances WHERE instance_id=?", params![instance_id], |r| r.get(0))?)
}

/// Every declared field of the instance, not only the identity ones.
fn key_facts_all(conn: &Connection, instance_id: i64, type_id: &str) -> Result<Vec<Value>> {
    let mut statement = conn.prepare_cached(
        "SELECT field_id,field_kind,value,value_type,value_status,source_time,observed_at,derivation,conflict_sources
         FROM object_facts WHERE instance_id=? ORDER BY field_kind,field_id",
    )?;
    let rows = statement.query_map(params![instance_id], |row| {
        let field_id: String = row.get(0)?;
        let value: Option<String> = row.get(2)?;
        let conflicts: Option<String> = row.get(8)?;
        Ok(json!({
            "field_id": field_id,
            "label": pcscope_contracts::model::field_definition(type_id, &row.get::<_, String>(0)?).map(|(label, _)| label).unwrap_or_default(),
            "kind": row.get::<_, String>(1)?,
            "value": value.and_then(|text| serde_json::from_str::<Value>(&text).ok()),
            "value_type": row.get::<_, String>(3)?,
            "status": row.get::<_, String>(4)?,
            "source_time": row.get::<_, Option<String>>(5)?,
            "observed_at": row.get::<_, String>(6)?,
            "derivation": row.get::<_, String>(7)?,
            "conflicts": conflicts.and_then(|text| serde_json::from_str::<Value>(&text).ok()).and_then(|value| value["candidates"].as_array().cloned()).map(|candidates| candidates.into_iter().map(|candidate| json!({"value": candidate["value"], "source": candidate["source"], "basis": "同一生效时间上来源给出不同值"})).collect::<Vec<_>>()).unwrap_or_default(),
        }))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn action_visibility_sql(conn:&Connection,routine:bool)->Result<String> {
    let fallback=format!("pcscope_action_visible(a.action_id,coalesce((SELECT type_id FROM object_instances i WHERE i.instance_id=a.instance_id),a.target_type,''),coalesce(a.field_id,''),a.before_value,a.after_value,{})",u8::from(routine));
    if !crate::display::available(conn)? {return Ok(fallback);}
    Ok(format!("coalesce((SELECT {} FROM display_actions d WHERE d.action_row=a.action_row AND d.revision='{}'
        AND d.identity_guard={} AND NOT EXISTS(SELECT 1 FROM display_identity_work w WHERE w.instance_id=a.instance_id)
        AND NOT EXISTS(SELECT 1 FROM display_work w WHERE w.kind='action' AND w.id=a.action_row)),{fallback})",
        if routine{"visible_all"}else{"visible_main"},crate::display::revision(),crate::display::identity_guard_sql()))
}

fn action_views(conn: &Connection, instance: Option<i64>, unassigned: Option<bool>, limit: usize) -> Result<(Vec<Value>, bool)> {
    let mut clauses = vec![action_visibility_sql(conn,true)?];
    match instance {
        Some(id) => clauses.push(format!("instance_id = {id}")),
        None => {
            if let Some(unassigned) = unassigned {
                clauses.push(if unassigned { "unassigned_reason IS NOT NULL".to_owned() } else { "instance_id IS NOT NULL".to_owned() });
            }
        }
    }
    let where_clause = if clauses.is_empty() { "1=1".to_owned() } else { clauses.join(" AND ") };
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {ACTION_COLUMNS}
         FROM object_actions a WHERE {where_clause} ORDER BY coalesce(interval_until,occurred_at,observed_at) DESC, action_row DESC LIMIT ?"
    ))?;
    let limit = clamp(limit);
    let rows = statement.query_map(params![(limit + 1) as i64], action_row)?;
    let mut data = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = data.len() > limit;
    data.truncate(limit);
    let mut labels=HistoryLabelCache::new();
    for action in &mut data { enrich_action_context(conn,action,&mut labels)?; }
    Ok((data, has_more))
}

fn binding_views(conn: &Connection, instance_id: i64) -> Result<Vec<Value>> {
    let mut statement = conn.prepare_cached(
        "SELECT source,item_id,scope,native_record_id,valid_from,valid_to FROM source_bindings WHERE instance_id=? ORDER BY valid_from,source",
    )?;
    let rows = statement.query_map(params![instance_id], |row| {
        Ok(json!({
            "source": row.get::<_, String>(0)?,
            "item_id": row.get::<_, String>(1)?,
            "scope": row.get::<_, String>(2)?,
            "native_record_id": row.get::<_, String>(3)?,
            "valid_from": row.get::<_, String>(4)?,
            "valid_to": row.get::<_, Option<String>>(5)?,
        }))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// The action history of one instance.
pub fn actions(conn: &Connection, reference: &str, limit: usize) -> Result<QueryResponse> {
    let id: Option<i64> = reference.parse::<i64>().ok();
    let Some(id) = id else {
        return Ok(QueryResponse::rejected(ResponseStatus::Unsupported, "unsupported_reference", "action queries take an instance id", pcscope_contracts::model::catalog_version()));
    };
    let (data, has_more) = action_views(conn, Some(id), None, limit)?;
    let status = if data.is_empty() { ResponseStatus::Empty } else { ResponseStatus::Loaded };
    let page = Page { next_cursor: None, limit: clamp(limit), watermark: watermark(conn, "actions")?, has_more };
    Ok(response(status, Value::Array(data), Some(page), None))
}

// Sparse object scopes must not walk every unrelated display row to prove the page
// ended. Read a bounded candidate set through the existing instance/action index.
// Dense scopes retain the time index, so pagination never sorts all network history.
fn scoped_action_candidates(conn: &Connection, filter: &ActionFilter) -> Result<Option<Vec<i64>>> {
    const MAX_CANDIDATES: usize = 10_000;
    // Rare event kinds (for example failed logons) must not scan the entire display
    // time index just to prove that fewer than one transport page exist.
    if let Some(action)=&filter.action_id {
        let mut statement=conn.prepare_cached("SELECT action_row FROM object_actions WHERE action_id=? LIMIT 10001")?;
        let ids=statement.query_map([action],|r|r.get::<_,i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        if ids.len()<=MAX_CANDIDATES {return Ok(Some(ids));}
    }
    let mut clauses=Vec::new();
    let mut args:Vec<Box<dyn rusqlite::ToSql>>=Vec::new();
    if let Some(category)=&filter.category {clauses.push("category=?".to_owned());args.push(Box::new(category.clone()));}
    if let Some(typ)=&filter.type_id {clauses.push("type_id=?".to_owned());args.push(Box::new(typ.clone()));}
    if let Some(instance)=&filter.instance {
        match instance.parse::<i64>() {
            Ok(id)=>{clauses.push("instance_id=?".to_owned());args.push(Box::new(id));}
            Err(_)=>{clauses.push(format!("{ALIAS}=?"));args.push(Box::new(instance.clone()));}
        }
    }
    if clauses.is_empty(){return Ok(None);}
    let mut sql=format!("SELECT a.action_row FROM object_actions a INDEXED BY object_actions_instance
        WHERE a.instance_id IN (SELECT instance_id FROM object_instances WHERE {})",clauses.join(" AND "));
    // An event can have a known type without an authenticated object identity.
    // Keep those records in category/type searches, but never in an object's history.
    if filter.include_unassigned && filter.instance.is_none() {
        let mut orphan=vec!["a.instance_id IS NULL".to_owned()];
        if let Some(category)=&filter.category {
            orphan.push("a.target_type IN (SELECT type_id FROM type_definitions WHERE category=?)".into());
            args.push(Box::new(category.clone()));
        }
        if let Some(typ)=&filter.type_id {orphan.push("a.target_type=?".into());args.push(Box::new(typ.clone()));}
        sql.push_str(&format!(" UNION ALL SELECT a.action_row FROM object_actions a WHERE {}",orphan.join(" AND ")));
    }
    sql.push_str(&format!(" LIMIT {}",MAX_CANDIDATES+1));
    let bound:Vec<&dyn rusqlite::ToSql>=args.iter().map(|v|v.as_ref()).collect();
    let mut statement=conn.prepare_cached(&sql)?;
    let ids=statement.query_map(bound.as_slice(),|r|r.get::<_,i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((ids.len()<=MAX_CANDIDATES).then_some(ids))
}

/// Every action, newest first.
///
/// This is what the timeline reads: `InstanceActions` answers "what happened to this one", and this
/// answers "what happened". Actions that belong to no object are included by default -- an event
/// that could not be attributed is still something that happened, and dropping it would make the
/// timeline look complete when it is not.
pub fn timeline(conn: &Connection, filter: &ActionFilter) -> Result<QueryResponse> {
    timeline_with_scope_plan(conn,filter,true)
}

fn timeline_with_scope_plan(conn: &Connection, filter: &ActionFilter, use_scope: bool) -> Result<QueryResponse> {
    let limit = if filter.cards_only { filter.limit.clamp(1, MAX_TIMELINE_CARDS) } else { clamp(filter.limit) };
    let mut probe = filter.clone();
    probe.cursor = None;
    probe.limit = 0;
    let walk = fingerprint(&(&probe,crate::display::revision()));
    let (walk_watermark, window, cursor_key, cursor_id) = match filter.cursor.as_deref() {
        Some(cursor) => match decode_cursor(cursor) {
            Some((issued, key, id)) if watermark_fingerprint(&issued) == walk => (issued.clone(), Some(action_bound(&issued)), key, id),
            _ => {
                return Ok(QueryResponse::rejected(ResponseStatus::Unsupported, "stale_cursor", "this cursor belongs to a different query; start the walk again", pcscope_contracts::model::catalog_version()))
            }
        },
        None => {
            let mut mark=watermark(conn,&walk)?;
            if crate::display::published(conn)? {mark.push_str(&format!("g{}",crate::display::generation(conn)?));}
            (mark,None,String::new(),0)
        },
    };

    // The anchor is the latest time the action is known to have been in effect. It is a fallback
    // whenever the source did not state a moment, which is why the response marks it rather than
    // presenting it as when something happened.
    let prepared=display_bound(&walk_watermark);
    let anchor = if prepared.is_some(){"d.anchor"}else{"coalesce(a.interval_until,a.occurred_at,a.observed_at)"};
    let mut clauses = if let Some(generation)=prepared {
        vec![format!("d.revision='{}' AND d.prepared_generation<={generation} AND d.{} AND d.identity_guard={}
            AND NOT EXISTS(SELECT 1 FROM display_work w WHERE w.kind='action' AND w.id=a.action_row)
            AND NOT EXISTS(SELECT 1 FROM display_identity_work w WHERE w.instance_id=a.instance_id)",crate::display::revision(),if filter.include_routine{"visible_all"}else{"visible_main"},crate::display::identity_guard_sql())]
    }else{
        // While old history is being prepared, preserve the original query path. Looking up
        // millions of missing display rows before LIMIT would make migration slower to browse.
        vec![format!("pcscope_action_visible(a.action_id,coalesce((SELECT type_id FROM object_instances i WHERE i.instance_id=a.instance_id),a.target_type,''),coalesce(a.field_id,''),a.before_value,a.after_value,{})",u8::from(filter.include_routine))]
    };
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let candidates=if use_scope {scoped_action_candidates(conn,filter)?}else{None};
    if let Some(ids)=&candidates {
        // One bounded JSON parameter keeps the statement reusable for different scopes.
        clauses.push("a.action_row IN (SELECT CAST(value AS INTEGER) FROM json_each(?))".into());
        args.push(Box::new(serde_json::to_string(ids)?));
    }
    if let Some(bound) = window {
        clauses.push("a.action_row <= ?".into());
        args.push(Box::new(bound));
    }
    if !filter.include_unassigned {
        clauses.push("a.instance_id IS NOT NULL".into());
    }
    if !filter.include_routine {
        // One security update summary per observation; full object history retains both fields.
        // Bind the sibling lookup to this page walk's watermark, too.
        let sibling=if let Some(generation)=prepared {format!("EXISTS(SELECT 1 FROM display_actions ready WHERE ready.action_row=s.action_row AND ready.revision='{}' AND ready.prepared_generation<={generation} AND ready.visible_all AND NOT EXISTS(SELECT 1 FROM display_work w WHERE w.kind='action' AND w.id=s.action_row))",crate::display::revision())}else{"pcscope_action_visible(s.action_id,'security.protection',s.field_id,s.before_value,s.after_value,1)".into()};
        clauses.push(format!("NOT(a.action_id='observed.state_changed' AND coalesce(a.field_id,'')='engine_version'
            AND EXISTS(SELECT 1 FROM object_instances i WHERE i.instance_id=a.instance_id AND i.type_id='security.protection')
            AND EXISTS(SELECT 1 FROM object_actions s WHERE s.instance_id=a.instance_id AND s.observed_at=a.observed_at
                AND s.field_id='signature_version' AND s.action_id='observed.state_changed' AND s.action_row<={}
                AND {sibling}))",action_bound(&walk_watermark)));
    }
    if let Some(action_id) = &filter.action_id {
        clauses.push("a.action_id = ?".into());
        args.push(Box::new(action_id.clone()));
    }
    if let Some(type_id) = &filter.type_id {
        clauses.push("(EXISTS(SELECT 1 FROM object_instances i WHERE i.instance_id=a.instance_id AND i.type_id=?) OR (a.instance_id IS NULL AND a.target_type=?))".into());
        args.push(Box::new(type_id.clone()));
        args.push(Box::new(type_id.clone()));
    }
    if let Some(category) = &filter.category {
        clauses.push("(EXISTS(SELECT 1 FROM object_instances i WHERE i.instance_id=a.instance_id AND i.category=?) OR (a.instance_id IS NULL AND a.target_type IN (SELECT type_id FROM type_definitions WHERE category=?)))".into());
        args.push(Box::new(category.clone()));
        args.push(Box::new(category.clone()));
    }
    if let Some(instance) = &filter.instance {
        match instance.parse::<i64>() {
            Ok(id) => {
                clauses.push("a.instance_id = ?".into());
                args.push(Box::new(id));
            }
            Err(_) => {
                clauses.push(format!("EXISTS(SELECT 1 FROM object_instances i WHERE i.instance_id=a.instance_id AND {ALIAS}=?)"));
                args.push(Box::new(instance.clone()));
            }
        }
    }
    if let Some(since) = &filter.since {
        clauses.push(format!("{anchor} >= ?"));
        args.push(Box::new(since.clone()));
    }
    if let Some(source) = &filter.source {
        clauses.push("EXISTS(SELECT 1 FROM evidence_refs r JOIN evidence e ON e.evidence_id=r.evidence_id WHERE r.subject_kind = 'action' AND r.subject_id=CAST(a.action_row AS TEXT) AND e.source=?)".into());
        args.push(Box::new(source.clone()));
    }
    if filter.cursor.is_some() {
        clauses.push(format!("(({anchor} < ?) OR ({anchor} = ? AND a.action_row < ?))"));
        args.push(Box::new(cursor_key.clone()));
        args.push(Box::new(cursor_key));
        args.push(Box::new(cursor_id));
    }

    let from=if prepared.is_some(){
        if candidates.is_some(){"object_actions a CROSS JOIN display_actions d ON d.action_row=a.action_row"}
        else{"display_actions d INDEXED BY display_actions_anchor CROSS JOIN object_actions a ON a.action_row=d.action_row"}
    }else{"object_actions a"};
    let stored=if prepared.is_some(){"d.view"}else{"NULL"};
    let sql = format!(
        "SELECT {ACTION_COLUMNS}, {anchor} AS anchor,{stored}
         FROM {from} WHERE {} ORDER BY {anchor} DESC, {} DESC LIMIT ?",
        clauses.join(" AND "),if prepared.is_some(){"d.action_row"}else{"a.action_row"}
    );
    let mut statement = conn.prepare_cached(&sql)?;
    let bound: Vec<&dyn rusqlite::ToSql> = args.iter().map(|value| value.as_ref()).collect();
    let mut all: Vec<&dyn rusqlite::ToSql> = bound;
    let limit_plus_one = (limit + 1) as i64;
    all.push(&limit_plus_one);
    let rows = statement.query_map(all.as_slice(), |row| {
        let cached:Option<String>=row.get(18)?;
        let view=if let Some(text)=cached {serde_json::from_str::<Value>(&text).map_err(|e|rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?}else{action_row(row)?};
        Ok((view,
            row.get::<_, String>(17)?,
            row.get::<_, i64>(0)?,
        ))
    })?;
    let mut has_more = false;
    let mut data = Vec::with_capacity(limit);
    let mut last_key = None;
    let mut bytes = 2;
    let mut labels=HistoryLabelCache::new();
    for (index,row) in rows.enumerate() {
        if index==limit {has_more=true;break;}
        let (mut action,anchor,id)=row?;
        if prepared.is_none(){enrich_action_context(conn,&mut action,&mut labels)?;}
        if !filter.include_routine && action["field_id"]=="signature_version" && action["context"]["type_id"]=="security.protection" {
            let engine:Option<(Option<String>,Option<String>)>=conn.query_row("SELECT before_value,after_value FROM object_actions s WHERE instance_id=? AND observed_at=? AND field_id='engine_version' AND action_id='observed.state_changed' AND action_row<=? AND pcscope_action_visible(s.action_id,'security.protection',s.field_id,s.before_value,s.after_value,1) ORDER BY action_row DESC LIMIT 1",params![action["instance_id"].as_i64(),action["observed_at"].as_str(),action_bound(&walk_watermark)],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((before,after))=engine {
                let decode=|text:Option<String>|text.and_then(|text|serde_json::from_str::<Value>(&text).ok());
                action["source_record"]=json!({"protection_update":{"engine_changed":true,"signature_changed":true,"engine_before":decode(before),"engine_after":decode(after)}});
            }
        }
        if filter.cards_only {
            compact_action(&mut action, id);
            let size=serde_json::to_vec(&action)?.len()+1;
            if !data.is_empty() && bytes+size>TIMELINE_CARD_BYTES {has_more=true;break;}
            // One oversized row must not trap a cursor in an endless empty page.
            if size+2>TIMELINE_CARD_BYTES {
                return Ok(QueryResponse::rejected(ResponseStatus::Unsupported,"timeline_record_too_large","这条记录过大，请缩小筛选范围后重试。",pcscope_contracts::model::catalog_version()));
            }
            bytes+=size;
        }
        last_key=Some((anchor,id));data.push(action);
    }
    let next_cursor=if has_more {last_key.map(|(anchor,id)|encode_cursor(&walk_watermark,&anchor,id))}else{None};
    let status = if data.is_empty() { ResponseStatus::Empty } else { ResponseStatus::Loaded };
    let page = Page { next_cursor, limit, watermark: walk_watermark, has_more };
    Ok(response(status, Value::Array(data), Some(page), None))
}

/// Preserve all card inputs, including source hints which affect first-seen and update wording.
/// Do not change the prepared projection or its revision: detail pruning is a transport concern.
fn compact_action(action: &mut Value, id: i64) {
    action["detail_id"]=json!(id);
    if let Some(source)=action.get_mut("source_record").and_then(Value::as_object_mut) {
        source.retain(|key,_|matches!(key.as_str(),"snapshot_presence"|"protection_update"|"title"));
    }
    if let Some(facts)=action["context"]["key_facts"].as_array_mut() {
        for fact in facts {
            if let Some(fields)=fact.as_object_mut() {
                fields.remove("observed_at");fields.remove("source_time");fields.remove("derivation");
            }
        }
    }
}

fn action_detail(conn: &Connection, id: i64) -> Result<QueryResponse> {
    let mut action=conn.query_row(&format!("SELECT {ACTION_COLUMNS} FROM object_actions a WHERE a.action_row=?"),[id],action_row).optional()?;
    if let Some(value)=action.as_mut() {enrich_action_context(conn,value,&mut HistoryLabelCache::new())?;}
    Ok(response(if action.is_some(){ResponseStatus::Loaded}else{ResponseStatus::Empty},action.unwrap_or(Value::Null),None,None))
}

/// One piece of evidence, by its address.
pub fn evidence(conn: &Connection, reference: &EvidenceRef) -> Result<QueryResponse> {
    let row: Option<(String, Option<String>, String, Option<String>, String, String, String, Option<String>)> = conn
        .query_row(
            "SELECT evidence_id,source_time,observed_at,ingested_at,decoding,source,kind,pruned_at FROM evidence WHERE evidence_id=? LIMIT 1",
            params![reference.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
        )
        .optional()?;
    let Some((id, source_time, observed_at, ingested_at, decoding, source, _kind, pruned_at)) = row else {
        return Ok(QueryResponse::rejected(ResponseStatus::Empty, "unknown_evidence", "no evidence matches that reference", pcscope_contracts::model::catalog_version()));
    };
    let decoding: Value = serde_json::from_str(&decoding).unwrap_or(Value::Null);
    let pruned = pruned_at.is_some();
    let problems: Vec<Value> = decoding["problems"].as_array().cloned().unwrap_or_default();
    let _ = id;
    Ok(response(
        ResponseStatus::Loaded,
        json!({
            "reference": {"kind": match reference.kind {
                pcscope_contracts::adapter::EvidenceKind::Record => "record",
                pcscope_contracts::adapter::EvidenceKind::Snapshot => "snapshot",
                pcscope_contracts::adapter::EvidenceKind::Event => "event",
            }, "id": reference.id},
            "state": if pruned { "pruned" } else { "available" },
            "pruned_at": pruned_at,
            "source": source,
            "source_time": source_time,
            "observed_at": observed_at,
            "ingested_at": ingested_at,
            "decoding": if decoding["available"].as_bool().unwrap_or(false) { "available" } else { "partial" },
            "problems": problems,
            // The address survives pruning; the material behind it does not.
            "raw_available": !pruned,
        }),
        None,
        None,
    ))
}

/// What every source and scope is doing.
pub fn coverage(conn: &Connection, source: Option<&str>) -> Result<QueryResponse> {
    let rows = crate::coverage::report(conn, source)?;
    let status = if rows.is_empty() { ResponseStatus::Empty } else { ResponseStatus::Loaded };
    Ok(response(status, Value::Array(rows), None, None))
}

/// These sources describe sampling/coverage, not persistent objects. Read their original stored
/// evidence rather than duplicating payloads in another table. Retention remains authoritative.
pub fn source_readings(conn: &Connection, source: Option<&str>, cursor: Option<&str>, limit: usize) -> Result<QueryResponse> {
    let destination=pcscope_contracts::adapters::source_gaps::reading_destination;
    if source.is_some_and(|source| destination(source).is_none()) {
        return Ok(QueryResponse::rejected(ResponseStatus::Unsupported, "not_a_reading_source", "请选择资源采样或采集范围来源", pcscope_contracts::model::catalog_version()));
    }
    let fp=fingerprint(&json!({"query":"source_readings","source":source}));
    let (walk,before)=match cursor {
        Some(cursor)=>match decode_cursor(cursor) {
            Some((walk,_,before)) if watermark_fingerprint(&walk)==fp && before>=0 => (walk,before),
            _=>return Ok(QueryResponse::rejected(ResponseStatus::VersionMismatch,"invalid_cursor","分页游标与当前来源不匹配",pcscope_contracts::model::catalog_version())),
        },
        None=>(watermark(conn,&fp)?,i64::MAX),
    };
    let bound=walk.split("-e").nth(1).and_then(|value|value.split('i').next()).and_then(|value|value.parse::<i64>().ok()).unwrap_or(0);
    let where_sql="source IN ('pcscope.execution.module-file-state','pcscope.execution.module-scan-status','pcscope.execution.resources','pcscope.collector.scoped-configuration','pcscope.collector.event-channel-coverage','pcscope.usn.watch-list') AND (?1 IS NULL OR source=?1) AND seq<=?2 AND pruned_at IS NULL";
    let count:i64=conn.query_row(&format!("SELECT count(*) FROM evidence WHERE {where_sql}"),params![source,bound],|row|row.get(0))?;
    let limit=clamp(limit);
    let mut statement=conn.prepare_cached(&format!("SELECT seq,evidence_id,item_id,source,source_scope,observed_at FROM evidence WHERE {where_sql} AND seq<?3 ORDER BY seq DESC LIMIT ?4"))?;
    let mut rows=statement.query_map(params![source,bound,before,(limit+1) as i64],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,String>(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more=rows.len()>limit;rows.truncate(limit);
    let next=if has_more { rows.last().map(|row|encode_cursor(&walk,"reading",row.0)) } else { None };
    let mut cache=crate::evidence::SnapshotCache::default();
    let mut data=Vec::new();
    for (_,evidence_id,item_id,source,scope,observed_at) in rows {
        if let Some(input)=crate::evidence::input_at_cached(conn,&evidence_id,&item_id,&mut cache)? {
            let progress=if source=="pcscope.collector.event-channel-coverage" {
                input.payload["channel"].as_str().map(|channel|channel_progress(conn,channel)).transpose()?
            }else{None};
            data.push(json!({"source":source,"scope":scope,"item_id":item_id,"destination":destination(&source),"observed_at":observed_at,"source_time":input.source_time,"evidence":input.evidence,"payload":input.payload,"progress":progress}));
        }
    }
    Ok(response(if data.is_empty(){ResponseStatus::Empty}else{ResponseStatus::Loaded},Value::Array(data),Some(Page{next_cursor:next,limit,watermark:walk,has_more}),Some(Counts{source_records:count as u64,evidence:count as u64,..Counts::default()})))
}

fn channel_progress(conn:&Connection,channel:&str)->Result<Value> {
    let source=format!("pcscope.eventlog.{}",channel.to_lowercase());
    let epochs:i64=conn.query_row("SELECT count(*) FROM checkpoints WHERE source=?",[&source],|row|row.get(0))?;
    let mut statement=conn.prepare_cached("SELECT epoch,value FROM checkpoints WHERE source=? ORDER BY rowid DESC LIMIT 32")?;
    let checkpoints=statement.query_map([&source],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?.into_iter().map(|(epoch,value)|json!({"epoch":epoch,"checkpoint":serde_json::from_str::<Value>(&value).unwrap_or(Value::String(value))})).collect::<Vec<_>>();
    let mut statement=conn.prepare_cached("SELECT reason,first_seen,last_seen,active FROM coverage_state WHERE source=? ORDER BY last_seen DESC LIMIT 32")?;
    let gaps=statement.query_map([&source],|row|Ok(json!({"reason":row.get::<_,String>(0)?,"first_seen":row.get::<_,String>(1)?,"last_seen":row.get::<_,String>(2)?,"active":row.get::<_,i64>(3)?!=0})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let total_gaps:i64=conn.query_row("SELECT count(*) FROM coverage_state WHERE source=?",[&source],|row|row.get(0))?;
    Ok(json!({"source":source,"epochs":checkpoints,"total_epochs":epochs,"has_more_epochs":epochs>32,"known_gaps":gaps,"total_gaps":total_gaps,"has_more_gaps":total_gaps>32,"basis":"当前数据库已提交的读取断点；到达现存日志末尾不表示完整历史；所有代际分别保留"}))
}

/// How far the projection has got.
pub fn projection(conn: &Connection) -> Result<QueryResponse> {
    let mut statement = conn.prepare_cached(
        "SELECT projection,rule_version,schema_version,cursor,watermark,pending,failed,last_error,updated_at
         FROM projection_state ORDER BY projection",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(json!({
            "projection": row.get::<_, String>(0)?,
            "rule_version": row.get::<_, String>(1)?,
            "schema_version": row.get::<_, i64>(2)?,
            "cursor": row.get::<_, Option<String>>(3)?,
            "watermark": row.get::<_, Option<String>>(4)?,
            "pending": row.get::<_, i64>(5)?.max(0) as u64,
            "failed": row.get::<_, i64>(6)?.max(0) as u64,
            "last_error": row.get::<_, Option<String>>(7)?,
            "updated_at": row.get::<_, String>(8)?,
        }))
    })?;
    let mut entries = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    // Objects, facts and actions are produced by one transactional worker, not three queues.
    // Read its real queue even before the worker's first pass; schema seed rows are not progress.
    // The dashboard needs outstanding work, not a count of every completed item retained over
    // the lifetime of the installation. Both seeks stay in the small indexed live ranges.
    let (pending,failed):(i64,i64)=conn.query_row(
        "SELECT (SELECT count(*) FROM projection_queue WHERE projection=?1 AND state='pending'),
                (SELECT count(*) FROM projection_queue WHERE projection=?1 AND state='failed')",
        [crate::evidence::OBJECT_PROJECTION],|row|Ok((row.get(0)?,row.get(1)?)))?;
    let last_error: Option<String> = conn.query_row(
        "SELECT last_error FROM projection_queue WHERE projection=? AND state='failed' ORDER BY queue_id DESC LIMIT 1",
        [crate::evidence::OBJECT_PROJECTION], |r|r.get(0)).optional()?.flatten();
    for entry in &mut entries {
        if matches!(entry["projection"].as_str(), Some("objects" | "facts" | "actions")) {
            entry["pending"] = json!(pending);
            entry["failed"] = json!(failed);
            entry["last_error"] = json!(last_error);
        }
    }
    if crate::display::available(conn)? {
        let display=crate::display::status(conn)?;
        entries.push(json!({"projection":"display","rule_version":crate::display::revision(),
            "schema_version":crate::model::SCHEMA_VERSION,"cursor":display.to_string(),
            "watermark":Value::Null,"pending":display["pending"],"failed":display["failed"],
            "last_error":conn.query_row("SELECT last_error FROM display_work WHERE attempts>0 ORDER BY id DESC LIMIT 1",[],|r|r.get::<_,Option<String>>(0)).optional()?.flatten(),
            "updated_at":conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')",[],|r|r.get::<_,String>(0))?,"display_state":display}));
    }
    let status = if entries.is_empty() { ResponseStatus::Empty } else { ResponseStatus::Loaded };
    Ok(response(status, Value::Array(entries), None, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;

    const TASKS: &str = "pcscope.startup.tasks";
    const SCOPE: &str = "machine:M/install:I/schema:1/context:/shard:0";

    fn source_snapshot(source:&str,run:&str,items:Value)->pcscope_contracts::Envelope {
        pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":format!("{source}/{run}"),"type":"snapshot","source":source,"scope":SCOPE,"run_id":run,"observed_at":if run=="later"{"2026-10-12T01:00:00Z"}else{"2026-10-12T00:00:00Z"},"complete":true,"items":items}).to_string()).unwrap()
    }

    #[test]
    fn every_declared_summary_field_reaches_the_list_without_exposing_unselected_configuration() {
        let store=Store::open(":memory:").unwrap();
        for (index,type_def) in pcscope_contracts::model::types().iter().enumerate() {
            let id=index as i64+1;
            let type_id=type_def["id"].as_str().unwrap();
            let selected=pcscope_contracts::model::summary_field_ids(type_id);
            assert!(!selected.is_empty(),"{type_id}");
            store.conn.execute("INSERT INTO object_instances(instance_id,type_id,category,machine_id,installation_id,scope,native_key,identity_status,lifecycle,first_observed_at,last_observed_at) VALUES(?,?,'test','M','I','machine:M',?,'known','active','2026-10-13T00:00:00Z','2026-10-13T00:00:00Z')",params![id,type_id,type_id]).unwrap();
            // No full query fixture is needed: exercise the exact bounded selector used by list,
            // detail and timeline against all declared fields, including unavailable values.
            for field in type_def["fields"].as_array().unwrap() {
                let field_id=field["id"].as_str().unwrap();
                let kind=field["value"]["kind"].as_str().unwrap();
                store.conn.execute("INSERT INTO object_facts(instance_id,field_id,field_kind,value,value_type,value_status,observed_at,derivation,rule_version) VALUES(?,?,?,NULL,'unknown','not_provided','2026-10-13T00:00:00Z','source_provided','test')",params![id,field_id,kind]).unwrap();
            }
            let facts=key_facts(&store.conn,id,type_id).unwrap();
            for field in &selected {assert!(facts.iter().any(|f|f["field_id"]==*field),"{type_id}.{field}");}
            assert!(facts.iter().all(|f|f["status"]=="not_provided"));
        }
    }

    #[test]
    fn software_names_and_unnamed_registrations_remain_distinct() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&source_snapshot("pcscope.software","first",json!([
            {"id":"UnknownNameFixture","data":{"registry_path":"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\UnknownNameFixture","registry_view":"64","values":{}}},
            {"id":"Example","data":{"registry_path":"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Example","registry_view":"64","values":{"DisplayName":{"kind":"REG_SZ","value":"示例编辑器"}}}}
        ]))).unwrap();
        store.project_pending("M","I",100).unwrap();
        let result=store.query(&QueryRequest::InstanceList(InstanceFilter::default())).unwrap();
        let rows=result.data.unwrap();
        let unnamed=rows.as_array().unwrap().iter().find(|row|row["presentation"]["name_status"]=="not_provided").unwrap();
        assert_eq!(unnamed["presentation"]["name"],"未命名的软件登记");
        assert!(unnamed["presentation"]["description"].as_str().unwrap().contains("尚不能"));
        let named=rows.as_array().unwrap().iter().find(|row|row["presentation"]["name_status"]=="source_provided").unwrap();
        assert_eq!(named["presentation"]["name"],"示例编辑器");
        assert_eq!(named["key_facts"][0]["label"],"显示名称","labels use the catalog's label, never its field kind");
        assert_eq!(store.conn.query_row("SELECT count(*) FROM object_instances",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    }

    #[test]
    fn software_display_names_drive_search_sort_and_page_boundaries_together() {
        let mut store=Store::open(":memory:").unwrap();
        let items:Vec<Value>=[("z-key","Alpha"),("a-key","Charlie"),("m-key","Bravo")].into_iter().map(|(key,name)|json!({"id":key,"data":{"registry_path":format!("HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{key}"),"registry_view":"64","values":{"DisplayName":{"kind":"REG_SZ","value":name}}}})).collect();
        store.ingest(&source_snapshot("pcscope.software","first",json!(items))).unwrap();
        store.project_pending("M","I",100).unwrap();
        let filtered=store.query(&QueryRequest::InstanceList(InstanceFilter{search:Some("Bravo".into()),..Default::default()})).unwrap();
        assert_eq!(filtered.counts.unwrap().instances,1);
        assert_eq!(filtered.data.unwrap()[0]["display_name"],"Bravo");
        for (sort,expected) in [(InstanceSort::NameAsc,["Alpha","Bravo","Charlie"]),(InstanceSort::NameDesc,["Charlie","Bravo","Alpha"])] {
            let mut cursor=None;
            for name in expected {
                let page=store.query(&QueryRequest::InstanceList(InstanceFilter{sort,limit:1,cursor:cursor.clone(),..Default::default()})).unwrap();
                assert_eq!(page.data.as_ref().unwrap()[0]["display_name"],name);
                assert_eq!(page.data.as_ref().unwrap()[0]["presentation"]["name"],name);
                cursor=page.page.unwrap().next_cursor;
            }
            assert!(cursor.is_none());
        }
    }

    #[test]
    fn registration_dictionary_names_are_exact_guarded_and_used_for_search_and_paging() {
        let mut store=Store::open(":memory:").unwrap();
        let edge="Microsoft Edge Update";let nvidia="{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}_Display.NVWMI";
        let item=|id:&str,key:&str,view:&str,values:Value|json!({"id":id,"data":{"registry_path":format!("HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{key}"),"registry_view":view,"values":values}});
        store.ingest(&source_snapshot("pcscope.software","first",json!([
            item("edge",edge,"64",json!({})),
            item("source-name",edge,"32",json!({"DisplayName":{"value":"来自来源的软件名称"}})),
            item("foreign-publisher",edge,"Registry64",json!({"Publisher":{"value":"Unrelated company"}})),
            item("masked-name",edge,"Registry32",json!({"DisplayName":{"value":pcscope_contracts::sensitive::MASK}})),
            item("nvidia",nvidia,"64",json!({"Publisher":{"value":"NVIDIA Corporation"}})),
            item("wrong-vendor",nvidia,"32",json!({"Publisher":{"value":"Microsoft Corporation"}})),
            item("other-guid","{00000000-0000-0000-0000-000000000000}_Display.NVWMI","64",json!({"Publisher":{"value":"NVIDIA Corporation"}})),
        ]))).unwrap();store.project_pending("M","I",100).unwrap();
        let response=store.query(&QueryRequest::InstanceList(InstanceFilter{sort:InstanceSort::NameAsc,..Default::default()})).unwrap();
        let rows=response.data.as_ref().unwrap().as_array().unwrap();
        let dictionary:Vec<_>=rows.iter().filter(|row|row["presentation"]["name_status"]=="dictionary_matched").collect();
        assert_eq!(dictionary.len(),2,"foreign publishers, masked names and other GUIDs are not dictionary matches");
        assert!(rows.iter().any(|row|row["presentation"]["name"]=="来自来源的软件名称"&&row["presentation"]["name_status"]=="source_provided"));
        assert!(dictionary.iter().all(|row|row["display_name"]==row["presentation"]["name"]));
        for row in &dictionary {
            let name=row["presentation"]["name"].as_str().unwrap();
            let found=store.query(&QueryRequest::InstanceList(InstanceFilter{search:Some(name.into()),..Default::default()})).unwrap();
            assert_eq!(found.counts.unwrap().instances,1);
            assert_eq!(found.data.unwrap()[0]["instance_id"],row["instance_id"]);
        }
        let mut seen=Vec::new();let mut cursor=None;
        loop {
            let page=store.query(&QueryRequest::InstanceList(InstanceFilter{sort:InstanceSort::NameAsc,limit:2,cursor,..Default::default()})).unwrap();
            seen.extend(page.data.unwrap().as_array().unwrap().iter().map(|row|row["instance_id"].clone()));
            cursor=page.page.unwrap().next_cursor;if cursor.is_none(){break}
        }
        assert_eq!(seen,rows.iter().map(|row|row["instance_id"].clone()).collect::<Vec<_>>());
        let edge_id=dictionary.iter().find(|row|row["presentation"]["dictionary_entry_id"]=="microsoft-edge-update").unwrap()["instance_id"].as_i64().unwrap();
        store.conn.execute("UPDATE object_instances SET display_name='unverified legacy label' WHERE instance_id=?",[edge_id]).unwrap();
        let cached=store.query(&QueryRequest::InstanceDetail{instance:edge_id.to_string()}).unwrap().data.unwrap();
        assert_eq!(cached["summary"]["presentation"]["name"],cached["summary"]["display_name"],"an identity/cache label cannot disagree with the effective name");
        let cached_search=store.query(&QueryRequest::InstanceList(InstanceFilter{search:Some("unverified legacy label".into()),..Default::default()})).unwrap();
        assert_eq!(cached_search.counts.unwrap().instances,0);
        for state in ["masked","conflicting","unreadable"] {
            store.conn.execute("UPDATE object_facts SET value_status=? WHERE instance_id=? AND field_id='publisher'",params![state,edge_id]).unwrap();
            let detail=store.query(&QueryRequest::InstanceDetail{instance:edge_id.to_string()}).unwrap().data.unwrap();
            assert_eq!(detail["summary"]["presentation"]["name_status"],"not_provided");
            assert_ne!(detail["summary"]["display_name"],"Edge 更新组件登记");
        }
        store.conn.execute("UPDATE object_facts SET value_status='not_provided' WHERE instance_id=? AND field_id='publisher'",[edge_id]).unwrap();
        store.conn.execute("UPDATE object_instances SET identity_status='provisional' WHERE instance_id=?",[edge_id]).unwrap();
        assert_eq!(store.query(&QueryRequest::InstanceDetail{instance:edge_id.to_string()}).unwrap().data.unwrap()["summary"]["presentation"]["name_status"],"not_provided");
    }

    #[test]
    fn com_history_uses_scoped_collected_names_and_marks_only_known_legacy_field_mappings() {
        let mut store=Store::open(":memory:").unwrap();
        let root="software\\classes\\clsid\\{11111111-1111-1111-1111-111111111111}";
        let server=format!("{root}\\inprocserver32");
        let mut snapshot=source_snapshot("pcscope.shell.registrations","first",json!([
            {"id":root,"data":{"registry_path":root,"values":{"":{"value":"图片预览组件"}}}},
            {"id":server,"data":{"registry_path":server,"values":{"":{"value":"C:\\Example\\preview.dll"}}}}
        ]));
        if let pcscope_contracts::Message::Snapshot{scope,..}=&mut snapshot.message {*scope="machine:M/user:A/schema:1/LocalMachine/Registry64/shard:0".into();}
        store.ingest(&snapshot).unwrap();store.project_pending("M","I",100).unwrap();
        let instance:i64=store.conn.query_row("SELECT instance_id FROM object_instances WHERE type_id='shell.com_registration'",[],|row|row.get(0)).unwrap();
        // Legacy projection omitted the root label. The retained source object still has it.
        store.conn.execute("DELETE FROM object_facts WHERE instance_id=? AND field_id='name'",[instance]).unwrap();
        store.conn.execute("UPDATE object_instances SET display_name=NULL WHERE instance_id=?",[instance]).unwrap();
        add_action(&store,"observed.configuration_changed","2026-10-12T02:00:00Z",Some(instance),None);
        let action=store.conn.last_insert_rowid();
        store.conn.execute("UPDATE object_actions SET field_id='server_path',before_value='\"图片预览组件\"',after_value='\"preview.dll\"' WHERE action_row=?",[action]).unwrap();
        for rule in history_labels::LEGACY_COM_RULES {
            store.conn.execute("UPDATE object_actions SET rule_version=? WHERE action_row=?",params![rule,action]).unwrap();
            let row=&timeline(&store,ActionFilter::default()).data.unwrap()[0];
            assert_eq!(row["context"]["subject_label"],"图片预览组件");
            assert_eq!(row["context"]["subject_basis"],"collected_registry_label");
            assert_eq!(row["context"]["history_quality"]["status"],"legacy_mapping_ambiguous");
            assert_eq!(row["before"],"图片预览组件","stored comparison evidence is preserved");
        }
        store.conn.execute("UPDATE object_actions SET rule_version='future-rule' WHERE action_row=?",[action]).unwrap();
        assert!(timeline(&store,ActionFilter::default()).data.unwrap()[0]["context"]["history_quality"].is_null());
        // A masked current name is an explicit privacy boundary even if retained raw metadata
        // would otherwise offer a readable fallback.
        store.conn.execute("INSERT INTO object_facts(instance_id,field_id,field_kind,value,value_type,value_status,observed_at,derivation,rule_version) VALUES(?,'name','identity','\"******\"','string','masked','2026-10-12','source_provided','test')",[instance]).unwrap();
        assert!(timeline(&store,ActionFilter::default()).data.unwrap()[0]["context"]["subject_label"].is_null());
    }

    #[test]
    fn snapshot_history_preserves_the_identity_guard_for_registration_dictionary_names() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&source_snapshot("pcscope.software","first",json!([
            {"id":"edge","data":{"registry_path":"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Microsoft Edge Update","registry_view":"64","values":{}}}
        ]))).unwrap();
        store.project_pending("M","I",100).unwrap();
        let instance:i64=store.conn.query_row("SELECT instance_id FROM object_instances",[],|row|row.get(0)).unwrap();
        add_action(&store,"observed.configuration_changed","2026-10-12T02:00:00Z",Some(instance),None);
        let row=&timeline(&store,ActionFilter::default()).data.unwrap()[0];
        assert_eq!(row["context"]["identity_status"],"confirmed");
        assert_eq!(row["context"]["attribution_status"],"stored");
        assert_eq!(row["context"]["presentation"]["name_status"],"dictionary_matched");
        assert_eq!(row["context"]["presentation"]["name"],"Edge 更新组件登记");
        assert_eq!(row["context"]["display_name"],row["context"]["presentation"]["name"]);
        // A dictionary explanation is current registration metadata, not a name supplied by the
        // historical event. Do not relabel its provenance while making the guard available.
        assert_eq!(row["context"]["presentation"]["basis"],"registration_dictionary");
        assert_eq!(row["context"]["subject_basis"],"unavailable");
        store.conn.execute("UPDATE object_instances SET identity_status='provisional' WHERE instance_id=?",[instance]).unwrap();
        let row=&timeline(&store,ActionFilter::default()).data.unwrap()[0];
        assert_eq!(row["context"]["identity_status"],"provisional");
        assert_eq!(row["context"]["presentation"]["name_status"],"not_provided");
    }

    #[test]
    fn a_cursor_from_an_older_registration_dictionary_cannot_continue_a_new_name_order() {
        let store=seeded(3);
        let mut probe=InstanceFilter{sort:InstanceSort::NameAsc,limit:0,..Default::default()};probe.cursor=None;
        let obsolete=fingerprint(&(&probe,"obsolete dictionary revision"));
        let cursor=encode_cursor(&watermark(store.reader(),&obsolete).unwrap(),"task-00",1);
        let result=store.query(&QueryRequest::InstanceList(InstanceFilter{sort:InstanceSort::NameAsc,limit:1,cursor:Some(cursor),..Default::default()})).unwrap();
        assert_eq!(result.status,ResponseStatus::Unsupported);
    }

    fn presentation_events(fields: Vec<(&str, Value)>) -> pcscope_contracts::Envelope {
        let events:Vec<Value>=fields.into_iter().enumerate().map(|(index,(kind,fields))|json!({
            "id":index.to_string(),"occurred_at":"2026-10-12T02:00:00Z","kind":kind,"object_id":index.to_string(),
            "data":{"provider":"presentation regression","fields":fields}
        })).collect();
        pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":"presentation-events","type":"event_batch","source":"pcscope.eventlog.security","epoch":"1","batch_seq":1,"checkpoint":"1","events":events}).to_string()).unwrap()
    }

    #[test]
    fn account_summaries_distinguish_inventory_from_log_mentions_and_include_useful_facts() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&source_snapshot("pcscope.identity.accounts","first",json!([
            {"id":"S-1-5-21-123-1001","data":{"name":"示例用户","enabled":false,"last_logon":"2026-10-12T00:00:00Z"}}
        ]))).unwrap();
        store.ingest(&source_snapshot("pcscope.identity.assigned-rights","first",json!([
            {"id":"S-1-5-21-123-1001","data":{"sid":"S-1-5-21-123-1001","rights":["SeRemoteInteractiveLogonRight"]}}
        ]))).unwrap();
        store.ingest(&presentation_events(vec![("identity.account_created",json!({"TargetUserSid":"S-1-5-21-123-1002"}))])).unwrap();
        store.project_pending("M","I",100).unwrap();
        let rows=store.query(&QueryRequest::InstanceList(InstanceFilter::default())).unwrap().data.unwrap();
        let rows=rows.as_array().unwrap();
        let user=rows.iter().find(|row|row["display_name"]=="示例用户").unwrap();
        assert_eq!(user["record_basis"],"observation");
        assert!(user["key_facts"].as_array().unwrap().iter().any(|f|f["field_id"]=="enabled"&&f["value"]==false));
        let right=rows.iter().find(|row|row["type_id"]=="account.right_assignment").unwrap();
        assert_eq!(right["record_basis"],"observation");
        assert!(right["key_facts"].as_array().unwrap().iter().any(|f|f["field_id"]=="privilege"&&f["value"]=="SeRemoteInteractiveLogonRight"));
        let logged=rows.iter().find(|row|row["type_id"]=="account.user"&&row["display_name"]=="").unwrap();
        assert_eq!(logged["record_basis"],"event_only");
        let detail=store.query(&QueryRequest::InstanceDetail{instance:logged["instance_id"].to_string()}).unwrap().data.unwrap();
        assert_eq!(detail["summary"]["record_basis"],"event_only");
    }

    #[test]
    fn old_null_account_and_special_privilege_links_remain_history_not_confirmed_objects() {
        for (kind,fields,old_type) in [
            ("identity.logon_failed",json!({"TargetUserSid":"S-1-0-0","TargetUserName":"示例用户"}),"account.user"),
            ("identity.special_privileges",json!({"SubjectUserSid":"S-1-5-18","SubjectUserName":"SYSTEM","SubjectLogonId":"0x3e7"}),"account.right_assignment"),
        ] {
            let mut store=seeded(1);
            store.ingest(&presentation_events(vec![(kind,fields)])).unwrap();
            store.project_pending("M","I",100).unwrap();
            let instance:i64=store.conn.query_row("SELECT min(instance_id) FROM object_instances",[],|r|r.get(0)).unwrap();
            store.conn.execute("UPDATE object_instances SET type_id=?,category='account',display_name='错误的旧名称' WHERE instance_id=?",params![old_type,instance]).unwrap();
            store.conn.execute("UPDATE object_actions SET instance_id=?,target_type=?,unassigned_reason=NULL WHERE action_id=?",params![instance,old_type,kind]).unwrap();
            let response=timeline(&store,ActionFilter::default()).data.unwrap();
            let row=response.as_array().unwrap().iter().find(|row|row["action_id"]==kind).unwrap();
            assert!(row["instance_id"].is_null());
            assert_eq!(row["context"]["attribution_status"],"mismatch");
            assert!(row["context"]["display_name"].is_null());
            assert!(row["context"]["subject_label"].is_string(),"event account text remains usable without claiming a verified identity");
            if kind=="identity.special_privileges" {
                assert_eq!(row["title"],"登录获得特殊权限");
                assert_eq!(row["context"]["type_id"],"runtime.logon_session");
            }
            assert_eq!(store.conn.query_row("SELECT instance_id FROM object_actions WHERE action_id=?",[kind],|r|r.get::<_,i64>(0)).unwrap(),instance,"read verification preserves stored history");
        }
    }

    #[test]
    fn failed_logons_with_known_type_remain_in_category_search_without_claiming_identity() {
        let mut store=seeded(1);
        store.ingest(&presentation_events(vec![("identity.logon_failed",json!({
            "TargetUserSid":"S-1-0-0","TargetUserName":"radiohide",
            "SubjectUserSid":"S-1-5-18","Status":"0xc000006d","SubStatus":"0xc000006a"
        }))])).unwrap();
        store.project_pending("M","I",100).unwrap();
        for prepared in [false,true] {
            if prepared {
                for _ in 0..100 {
                    if store.prepare_display(32,std::time::Duration::from_millis(50)).unwrap()["pending"]==false {break;}
                }
                assert!(crate::display::published(&store.conn).unwrap());
            }
            for (category,typ) in [(None,None),(Some("account"),None),(None,Some("account.user")),(Some("account"),Some("account.user"))] {
                for action in [None,Some("identity.logon_failed")] {
                    let filter=ActionFilter{category:category.map(str::to_owned),type_id:typ.map(str::to_owned),action_id:action.map(str::to_owned),..Default::default()};
                    let result=timeline(&store,filter.clone());
                    let row=result.data.unwrap().as_array().unwrap().iter().find(|r|r["action_id"]=="identity.logon_failed").unwrap().clone();
                    assert!(row["instance_id"].is_null());
                    assert_eq!(row["context"]["type_id"],"account.user");
                    assert_eq!(row["context"]["subject_label"],"radiohide");
                    let hidden=timeline(&store,ActionFilter{include_unassigned:false,..filter});
                    assert!(!action_ids(&hidden).contains(&"identity.logon_failed".into()));
                }
            }
            for filter in [
                ActionFilter{category:Some("hardware".into()),..Default::default()},
                ActionFilter{category:Some("account".into()),type_id:Some("runtime.process".into()),..Default::default()},
                ActionFilter{instance:Some("1".into()),..Default::default()},
            ] {
                assert!(!action_ids(&timeline(&store,filter)).contains(&"identity.logon_failed".into()));
            }
        }
    }

    #[test]
    fn every_visible_typed_unassigned_action_remains_reachable_by_category_and_type() {
        let store=Store::open(":memory:").unwrap();
        let definitions:Vec<(String,String,String)>={
            let mut q=store.conn.prepare("SELECT a.action_id,a.target_type,t.category FROM action_definitions a JOIN type_definitions t ON t.type_id=a.target_type").unwrap();
            q.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap()
        };
        for (action,typ,_) in &definitions {
            add_action(&store,action,"2026-10-20T00:00:00Z",None,Some("fixture: event target identity absent"));
            store.conn.execute("UPDATE object_actions SET target_type=? WHERE action_id=?",params![typ,action]).unwrap();
        }
        let visible=action_ids(&timeline(&store,ActionFilter{include_routine:true,limit:200,..Default::default()}));
        assert!(visible.len()>40,"exercise the catalog, not only the four live examples");
        for (action,typ,category) in &definitions {
            if !visible.contains(action) {continue;}
            for filter in [
                ActionFilter{type_id:Some(typ.clone()),..Default::default()},
                ActionFilter{category:Some(category.clone()),..Default::default()},
            ] {
                assert!(action_ids(&timeline(&store,ActionFilter{include_routine:true,limit:200,..filter})).contains(action),"{category}/{typ}/{action}");
            }
        }
    }

    #[test]
    fn legacy_snapshot_names_require_exact_generation_and_unprotected_collected_material() {
        let mut store=Store::open(":memory:").unwrap();
        let source="pcscope.execution.processes";
        let started="2026-10-12T01:00:00Z";
        store.ingest(&source_snapshot(source,"first",json!([{"id":format!("42/{started}"),"data":{"pid":42,"start_time":started,"name":"example.exe","path":"C:\\example.exe"}}]))).unwrap();
        store.project_pending("M","I",100).unwrap();
        let id:i64=store.conn.query_row("SELECT instance_id FROM object_instances WHERE type_id='runtime.process'",[],|r|r.get(0)).unwrap();
        store.conn.execute("DELETE FROM object_facts WHERE instance_id=? AND field_id='name'",[id]).unwrap();
        store.conn.execute("UPDATE object_facts SET value='null',value_status='not_provided' WHERE instance_id=? AND field_id='image'",[id]).unwrap();
        let detail=|store:&Store|store.query(&QueryRequest::InstanceDetail{instance:id.to_string()}).unwrap().data.unwrap()["summary"].clone();
        assert_eq!(detail(&store)["presentation"]["record_status"],"snapshot_named");
        store.conn.execute("UPDATE object_facts SET value='\"2026-10-13T01:00:00Z\"' WHERE instance_id=? AND field_id='created_at'",[id]).unwrap();
        assert!(detail(&store)["presentation"]["record_label"].is_null(),"the confirmed creation time must match the bound item");
        store.conn.execute("UPDATE object_instances SET identity_status='provisional' WHERE instance_id=?",[id]).unwrap();
        store.conn.execute("UPDATE object_facts SET value='null',value_status='not_provided' WHERE instance_id=? AND field_id='created_at'",[id]).unwrap();
        assert_eq!(detail(&store)["presentation"]["record_label"],"example.exe");
        assert_eq!(detail(&store)["presentation"]["name_status"],"not_provided");
        store.conn.execute("UPDATE objects SET data=json_set(data,'$.start_time','2026-10-13T01:00:00Z') WHERE source=?",[source]).unwrap();
        assert!(detail(&store)["presentation"]["record_label"].is_null(),"PID reuse is not a match");
        store.conn.execute("UPDATE objects SET data=json_set(data,'$.start_time',?,'$.name','******') WHERE source=?",params![started,source]).unwrap();
        assert!(detail(&store)["presentation"]["record_label"].is_null(),"protected names stay protected");
        store.conn.execute("DELETE FROM objects WHERE source=?",[source]).unwrap();
        assert_eq!(detail(&store)["presentation"]["record_label"],"example.exe","an exited process can retain its exact original snapshot");
        store.conn.execute("UPDATE evidence SET pruned_at='2026-10-13' WHERE source=?",[source]).unwrap();
        assert!(detail(&store)["presentation"]["record_label"].is_null(),"pruned evidence is not reconstructed");
        store.conn.execute("UPDATE evidence SET pruned_at=NULL WHERE source=?",[source]).unwrap();
        store.conn.execute("DELETE FROM snapshots WHERE source=?",[source]).unwrap();
        assert!(detail(&store)["presentation"]["record_label"].is_null(),"missing material is not reconstructed");
        store.conn.execute("INSERT INTO source_bindings(instance_id,source,item_id,scope,native_record_id,valid_from) SELECT instance_id,source,'42/2026-10-13T01:00:00Z',scope,'42/2026-10-13T01:00:00Z','2026-10-13' FROM source_bindings WHERE instance_id=? LIMIT 1",[id]).unwrap();
        assert_eq!(detail(&store)["presentation"]["record_status"],"legacy_mixed_process");
    }

    #[test]
    fn membership_labels_use_same_installation_account_and_respect_name_status() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&source_snapshot("pcscope.identity.accounts","first",json!([{"id":"S-1-5-21-1-1001","data":{"name":"Alice","enabled":true}}]))).unwrap();
        store.ingest(&source_snapshot("pcscope.identity.groups","first",json!([{"id":"S-1-5-32-544","data":{"name":"Administrators","members":["S-1-5-21-1-1001"]}}]))).unwrap();
        store.project_pending("M","I",100).unwrap();
        let id:i64=store.conn.query_row("SELECT instance_id FROM object_instances WHERE type_id='account.group_membership'",[],|r|r.get(0)).unwrap();
        let label=|store:&Store|store.query(&QueryRequest::InstanceDetail{instance:id.to_string()}).unwrap().data.unwrap()["summary"]["presentation"]["member_label"].clone();
        assert_eq!(label(&store),"Alice");
        store.conn.execute("UPDATE object_instances SET installation_id='Other' WHERE type_id='account.user'",[]).unwrap();
        assert!(label(&store).is_null());
        store.conn.execute("UPDATE object_instances SET installation_id='I' WHERE type_id='account.user'",[]).unwrap();
        store.conn.execute("UPDATE object_facts SET value_status='masked' WHERE field_id='name'",[]).unwrap();
        assert!(label(&store).is_null());
    }

    #[test]
    fn event_only_summary_names_are_qualified_and_ambiguous_or_pruned_labels_stay_absent() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&presentation_events(vec![
            ("startup.task_started",json!({"TaskName":"\\Example\\Daily backup"})),
            ("startup.task_started",json!({"TaskName":"\\Example\\Other task"})),
        ])).unwrap();
        store.project_pending("M","I",100).unwrap();
        let rows=store.query(&QueryRequest::InstanceList(InstanceFilter::default())).unwrap().data.unwrap();
        let first=&rows[0];let id=first["instance_id"].as_i64().unwrap();
        assert_eq!(first["record_basis"],"event_only");
        assert_eq!(first["presentation"]["record_status"],"event_named");
        assert!(first["presentation"]["record_label"].is_string());
        assert_eq!(first["presentation"]["name_status"],"not_provided","a log label is not an inventory name");
        store.conn.execute("UPDATE object_actions SET instance_id=?",[id]).unwrap();
        let detail=store.query(&QueryRequest::InstanceDetail{instance:id.to_string()}).unwrap().data.unwrap();
        assert_eq!(detail["summary"]["presentation"]["record_status"],"event_incomplete");
        assert!(detail["summary"]["presentation"]["record_label"].is_null());
        store.conn.execute("UPDATE evidence SET pruned_at='2026-10-13'",[]).unwrap();
        let detail=store.query(&QueryRequest::InstanceDetail{instance:id.to_string()}).unwrap().data.unwrap();
        assert!(detail["summary"]["presentation"]["record_label"].is_null());
    }

    #[test]
    fn timeline_names_come_from_the_particular_event_and_preserve_service_login_context() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&presentation_events(vec![
            ("startup.task_started",json!({"TaskName":"\\Microsoft\\Windows\\Example\\Daily backup"})),
            ("identity.logon",json!({"TargetUserName":"SYSTEM","TargetUserSid":"S-1-5-18","TargetDomainName":"NT AUTHORITY","TargetLogonId":"0x3e7","LogonType":"5","ProcessName":"C:\\Windows\\System32\\services.exe"})),
            ("identity.special_privileges",json!({"SubjectUserName":"SYSTEM","SubjectUserSid":"S-1-5-18","SubjectDomainName":"NT AUTHORITY","SubjectLogonId":"0x3e7"})),
        ])).unwrap();
        store.project_pending("M","I",100).unwrap();
        let result=timeline(&store,ActionFilter::default());
        let rows=result.data.unwrap();
        let task=rows.as_array().unwrap().iter().find(|row|row["action_id"]=="startup.task_started").unwrap();
        assert_eq!(task["context"]["attribution_status"],"verified");
        assert_eq!(task["context"]["identity_status"],"confirmed");
        assert_eq!(task["context"]["subject_label"],"Daily backup");
        assert_eq!(task["context"]["subject_basis"],"event_evidence");
        assert_eq!(task["context"]["subject_detail"],"\\Microsoft\\Windows\\Example\\Daily backup");
        let login=rows.as_array().unwrap().iter().find(|row|row["action_id"]=="identity.logon").unwrap();
        assert_eq!(login["context"]["subject_label"],"系统账户");
        assert_eq!(login["context"]["logon_type"],"5");
        assert_eq!(login["context"]["program_label"],"services.exe");
        let task_id=task["instance_id"].as_i64().unwrap();
        store.conn.execute("UPDATE object_instances SET display_name='different current name' WHERE instance_id=?",[task_id]).unwrap();
        let history=store.query(&QueryRequest::InstanceActions{instance:task_id.to_string(),limit:10,cursor:None}).unwrap();
        assert_eq!(history.data.unwrap()[0]["context"]["subject_label"],"Daily backup");
    }

    #[test]
    fn timeline_presentation_never_turns_protected_or_pruned_event_fields_into_names() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&presentation_events(vec![("identity.logon",json!({"TargetUserName":{"sensitive":true,"ciphertext":"not-displayable"},"TargetLogonId":"0x11"}))])).unwrap();
        store.project_pending("M","I",100).unwrap();
        let result=timeline(&store,ActionFilter::default());
        assert!(result.data.as_ref().unwrap()[0]["context"]["subject_label"].is_null());
        assert!(!result.data.unwrap().to_string().contains("not-displayable"));
        store.conn.execute("UPDATE evidence SET pruned_at='2026-10-12T03:00:00Z'",[]).unwrap();
        let context=&timeline(&store,ActionFilter::default()).data.unwrap()[0]["context"];
        assert_eq!(context["subject_basis"],"unavailable");
        assert_eq!(context["attribution_status"],"unassigned");
        assert!(context["identity_status"].is_null());
    }

    #[test]
    fn timeline_context_does_not_scan_unrelated_retained_events_per_row() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&presentation_events(vec![("startup.task_started",json!({"TaskName":"\\Example\\Daily backup"}))])).unwrap();
        store.project_pending("M","I",100).unwrap();
        store.conn.execute_batch("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20000)
            INSERT INTO evidence(evidence_id,item_id,kind,source,structure_version,source_scope,native_record_id,observed_at,ingested_at,decoding,payload_ref)
            SELECT 'unrelated-'||x,'unrelated','event','other','v1','other','unrelated','2026-10-12','2026-10-12','{}','record:0' FROM n;").unwrap();
        let steps=Arc::new(AtomicUsize::new(0));let used=steps.clone();
        store.conn.progress_handler(1000,Some(move||used.fetch_add(1000,Ordering::Relaxed)>=100000));
        let result=store.query(&QueryRequest::ActionTimeline(ActionFilter::default())).expect("a small page reads its referenced events, regardless of unrelated history volume");
        assert_eq!(result.data.unwrap()[0]["context"]["subject_label"],"Daily backup");
        assert!(steps.load(Ordering::Relaxed)<100000);
    }

    #[test]
    fn legacy_task_placeholder_links_are_withheld_in_timeline_and_object_history() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&presentation_events(vec![
            ("startup.task_started",json!({"TaskName":"\\A\\First task"})),
            ("startup.task_started",json!({"TaskName":"\\B\\Second task"})),
        ])).unwrap();
        store.project_pending("M","I",100).unwrap();
        let original=timeline(&store,ActionFilter::default()).data.unwrap();
        assert!(original.as_array().unwrap().iter().all(|row|row["context"]["attribution_status"]=="verified"));
        let placeholder=original[0]["instance_id"].as_i64().unwrap();
        // Reproduce the old reader's stored mistake without rewriting source evidence.
        store.conn.execute("UPDATE object_actions SET instance_id=?",[placeholder]).unwrap();
        store.conn.execute("UPDATE object_instances SET native_key=scope||'|?path',identity_status='provisional',display_name='misleading current task' WHERE instance_id=?",[placeholder]).unwrap();
        let before=store.conn.query_row("SELECT count(*) FROM object_actions WHERE instance_id=?",[placeholder],|r|r.get::<_,i64>(0)).unwrap();
        for rows in [timeline(&store,ActionFilter::default()).data.unwrap(),store.query(&QueryRequest::InstanceDetail{instance:placeholder.to_string()}).unwrap().data.unwrap()["actions_preview"].clone()] {
            assert_eq!(rows.as_array().unwrap().len(),2);
            assert!(rows.as_array().unwrap().iter().all(|row|row["instance_id"].is_null()&&row["context"]["attribution_status"]=="mismatch"));
            let names:Vec<_>=rows.as_array().unwrap().iter().map(|row|row["context"]["subject_label"].as_str().unwrap()).collect();
            assert!(names.contains(&"First task")&&names.contains(&"Second task"));
            assert!(!rows.to_string().contains("misleading current task"));
        }
        assert_eq!(store.conn.query_row("SELECT count(*) FROM object_actions WHERE instance_id=?",[placeholder],|r|r.get::<_,i64>(0)).unwrap(),before,"reading does not migrate historical links");
    }

    #[test]
    fn an_event_needs_its_exact_source_binding_even_when_the_object_key_matches() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&presentation_events(vec![("startup.task_started",json!({"TaskName":"\\A\\Example"}))])).unwrap();
        store.project_pending("M","I",100).unwrap();
        store.conn.execute("UPDATE source_bindings SET native_record_id='a different event'",[]).unwrap();
        let row=&timeline(&store,ActionFilter::default()).data.unwrap()[0];
        assert!(row["instance_id"].is_null());
        assert_eq!(row["context"]["attribution_status"],"mismatch");
        assert!(row["context"]["identity_status"].is_null(),"a confirmed stored object does not make a mismatched event trustworthy");
        assert!(row["context"]["display_name"].is_null());
        assert_eq!(row["context"]["key_facts"],json!([]));
        assert_eq!(row["context"]["presentation"]["basis"],"unverified_attribution");
    }

    #[test]
    fn old_application_event_100_is_shown_as_a_system_event_without_rewriting_history() {
        let mut store=seeded(1);
        let mut event=presentation_events(vec![("windows.event",json!({"TaskName":"\\Not a task"}))]);
        if let pcscope_contracts::Message::EventBatch{source,events,..}=&mut event.message {
            *source="pcscope.eventlog.application".into();
            events[0].data=json!({"event_id":100,"provider":"Example application","channel":"Application","fields":{"TaskName":"\\Not a task"}});
        }
        store.ingest(&event).unwrap();store.project_pending("M","I",100).unwrap();
        let instance=store.conn.query_row("SELECT min(instance_id) FROM object_instances",[],|r|r.get::<_,i64>(0)).unwrap();
        add_action(&store,"startup.task_started","2026-10-12T02:00:00Z",Some(instance),None);
        let row=store.conn.last_insert_rowid();
        store.conn.execute("INSERT INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) SELECT 'action',?,'event',evidence_id FROM evidence WHERE source='pcscope.eventlog.application'",[row.to_string()]).unwrap();
        let response=timeline(&store,ActionFilter::default()).data.unwrap();
        assert_eq!(response[0]["action_id"],"windows.event");assert_eq!(response[0]["title"],"系统事件");
        assert!(response[0]["instance_id"].is_null());assert!(response[0]["context"]["subject_label"].is_null());
        assert_eq!(response[0]["context"]["stored_action_id"],"startup.task_started");
        assert_eq!(store.conn.query_row("SELECT action_id FROM object_actions WHERE action_row=?",[row],|r|r.get::<_,String>(0)).unwrap(),"startup.task_started");
    }

    #[test]
    fn source_gaps_project_query_and_replay_without_fake_objects_or_lost_samples() {
        let mut store=Store::open(":memory:").unwrap();
        let readings=["pcscope.execution.module-scan-status","pcscope.execution.module-file-state","pcscope.execution.resources","pcscope.collector.scoped-configuration","pcscope.collector.event-channel-coverage","pcscope.usn.watch-list"];
        for source in readings {
            store.ingest(&source_snapshot(source,"first",json!([{"id":"a","data":{"active":false,"sampled_at":"2026-10-12T00:00:00Z","unknown_nested":{"long":"中文".repeat(5000),"values":[null,true,4]}}}]))).unwrap();
        }
        store.ingest(&source_snapshot("pcscope.hardware.driver-packages","first",json!([{"id":"oem1.inf","data":{"Driver":"oem1.inf","ProviderName":"Example","Version":"1","OriginalFileName":"C:\\driver.inf","DriverSignature":"Signed"}}]))).unwrap();
        store.ingest(&source_snapshot("pcscope.security.key-metadata","first",json!([{"id":"key","data":{"provider":"Example provider","location":"user","name":"key","algorithm":"RSA"}}]))).unwrap();
        store.ingest(&source_snapshot("pcscope.files.directory-security","first",json!([{"id":"directory","data":{"path":"C:\\fixture","exists":true,"volume_identity":"V","file_id":"0000000000000001","entries":[{"name":"child","directory":false}]}}]))).unwrap();
        let updates=json!([{"id":"update/1","data":{"date":"2026-10-11T00:00:00Z","operation":1,"result":4,"hresult":-1,"update_id":"U","title":"Failed update"}},{"id":"update/2","data":{"date":"2026-10-11T00:00:00Z","operation":1,"result":4,"hresult":-1,"update_id":"U","title":"Failed update"}}]);
        store.ingest(&source_snapshot("pcscope.system.update-history","first",updates.clone())).unwrap();
        let report=store.project_pending("M","I",200).unwrap();assert_eq!(report.unadapted,0);assert_eq!(report.rejected,0);
        let watch=store.query(&QueryRequest::SourceCoverage{source:Some("pcscope.usn.watch-list".into())}).unwrap();
        assert_eq!(watch.data.as_ref().unwrap()[0]["configured"],false);assert_eq!(watch.data.as_ref().unwrap()[0]["history_state"],"unknown");
        let objects=store.query(&QueryRequest::InstanceList(InstanceFilter::default())).unwrap();
        assert_eq!(objects.counts.unwrap().instances,3,"samples, provider checks and failed updates must not create objects");
        let timeline=store.query(&QueryRequest::ActionTimeline(ActionFilter{source:Some("pcscope.system.update-history".into()),..Default::default()})).unwrap();
        assert_eq!(timeline.data.unwrap().as_array().unwrap().len(),2,"distinct identical-looking history records must survive");
        store.ingest(&source_snapshot("pcscope.system.update-history","later",updates)).unwrap();store.project_pending("M","I",200).unwrap();
        let timeline=store.query(&QueryRequest::ActionTimeline(ActionFilter{source:Some("pcscope.system.update-history".into()),..Default::default()})).unwrap();
        assert_eq!(timeline.data.unwrap().as_array().unwrap().len(),2,"a later snapshot must not duplicate historical operations");
        let first=store.query(&QueryRequest::SourceReadings{source:None,cursor:None,limit:3}).unwrap();
        assert_eq!(first.counts.unwrap().source_records,6);let next=first.page.unwrap().next_cursor.unwrap();
        store.ingest(&source_snapshot(readings[0],"later",json!([{"id":"new","data":{"percent_processor_time":20}}]))).unwrap();store.project_pending("M","I",200).unwrap();
        let second=store.query(&QueryRequest::SourceReadings{source:None,cursor:Some(next.clone()),limit:3}).unwrap();
        assert_eq!(second.counts.unwrap().source_records,6);assert!(!second.page.unwrap().has_more);
        assert_eq!(second.data.unwrap()[0]["payload"]["unknown_nested"]["long"].as_str().unwrap().len(),"中文".repeat(5000).len());
        let wrong=store.query(&QueryRequest::SourceReadings{source:Some(readings[0].into()),cursor:Some(next),limit:3}).unwrap();assert_eq!(wrong.status,ResponseStatus::VersionMismatch);
        let fresh=store.query(&QueryRequest::SourceReadings{source:Some(readings[0].into()),cursor:None,limit:20}).unwrap();assert_eq!(fresh.counts.unwrap().source_records,2);
        let detail=store.query(&QueryRequest::InstanceList(InstanceFilter{type_id:Some("file.watched_directory".into()),..Default::default()})).unwrap();assert_eq!(detail.counts.unwrap().instances,1);
        assert_eq!(store.reader().query_row("SELECT count(*) FROM projection_queue WHERE state='failed'",[],|row|row.get::<_,i64>(0)).unwrap(),0);
    }

    #[test]
    fn directory_allowlist_is_separate_versioned_and_rejects_implicit_ranges() {
        let mut store=Store::open(":memory:").unwrap();let at="2026-10-12T00:00:00Z";
        assert_eq!(store.directory_watch_settings().unwrap()["directories"],json!([]));
        for path in ["relative",r"C:\",r"C:\..\other",r"C:\*.txt",r"\\server\share",r"C:\bad.:stream"] { assert!(store.set_directory_watch_settings(vec![path.into()],at).is_err(),"accepted {path}"); }
        let dirs=store.set_directory_watch_settings(vec![r"C:\fixture".into(),r"c:\FIXTURE".into()],at).unwrap();assert_eq!(dirs["directories"].as_array().unwrap().len(),1);assert_eq!(dirs["revision"],1);
        assert_eq!(store.file_watch_settings().unwrap()["revision"],0);
        assert_eq!(store.set_directory_watch_settings(vec![r"C:\fixture".into()],at).unwrap()["revision"],1);
        assert_eq!(store.set_directory_watch_settings(vec![],at).unwrap()["revision"],2);
    }
    #[test]
    fn channel_readings_expose_committed_epochs_without_erasing_known_gaps() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&source_snapshot("pcscope.collector.event-channel-coverage","first",json!([{"id":"System","data":{"channel":"System","readable":true,"oldest_record":10,"newest_record":20}}]))).unwrap();
        for (epoch,more) in [("old",true),("new",false)] { store.reader().execute("INSERT INTO checkpoints VALUES('pcscope.eventlog.system',?,?)",params![epoch,json!({"record_id":20,"has_more":more,"oldest_available_record":10}).to_string()]).unwrap(); }
        crate::coverage::gap(store.reader(),"pcscope.eventlog.system","retained log reset","2026-10-11T00:00:00Z").unwrap();
        store.project_pending("M","I",50).unwrap();
        let response=store.query(&QueryRequest::SourceReadings{source:None,cursor:None,limit:20}).unwrap();let row=&response.data.as_ref().unwrap()[0];
        assert_eq!(row["progress"]["total_epochs"],2);assert_eq!(row["progress"]["epochs"][0]["checkpoint"]["has_more"],false);assert_eq!(row["progress"]["known_gaps"][0]["reason"],"retained log reset");
    }

    fn snapshot(run: &str, paths: &[(&str, &str)]) -> pcscope_contracts::Envelope {
        let items: Vec<serde_json::Value> = paths
            .iter()
            .map(|(path, name)| json!({"id": path, "data": {"path": path, "name": name}}))
            .collect();
        pcscope_contracts::parse(
            &json!({
                "protocol_version": 1, "request_id": run, "type": "snapshot", "source": TASKS,
                "scope": SCOPE, "run_id": run, "observed_at": "2026-09-12T00:00:00Z", "complete": true, "items": items
            })
            .to_string(),
        )
        .unwrap()
    }

    /// A store with `count` scheduled tasks projected, named so the sort order is the insertion
    /// order.
    fn seeded(count: usize) -> Store {
        let mut store = Store::open(":memory:").unwrap();
        let owned: Vec<(String, String)> = (0..count).map(|index| (format!("\\A\\task-{index:02}"), format!("task-{index:02}"))).collect();
        let paths: Vec<(&str, &str)> = owned.iter().map(|(path, name)| (path.as_str(), name.as_str())).collect();
        store.ingest(&snapshot("run-1", &paths)).unwrap();
        store.project_pending("M", "I", 500).unwrap();
        store
    }

    fn list(store: &Store, cursor: Option<String>, limit: usize) -> QueryResponse {
        let filter = InstanceFilter { limit, cursor, ..InstanceFilter::default() };
        store.query(&QueryRequest::InstanceList(filter)).unwrap()
    }

    fn names(response: &QueryResponse) -> Vec<String> {
        response.data.as_ref().and_then(Value::as_array).map(|rows| rows.iter().filter_map(|row| row["display_name"].as_str().map(str::to_owned)).collect()).unwrap_or_default()
    }

    #[test]
    fn a_list_query_reports_what_it_matched() {
        let store = seeded(3);
        let response = list(&store, None, 10);
        assert_eq!(response.status, ResponseStatus::Loaded);
        assert_eq!(names(&response), vec!["task-00", "task-01", "task-02"]);
        let counts = response.counts.expect("counts");
        // Counts are per kind and are not summed: three instances are not nine of anything.
        assert_eq!(counts.instances, 3);
        assert_eq!(counts.source_records, 3);
        let page = response.page.expect("page");
        assert!(!page.has_more);
        assert!(page.next_cursor.is_none());
    }

    fn broad_reference_store() -> Store {
        let store = seeded(2);
        store.conn.execute("DELETE FROM evidence_refs", []).unwrap();
        store.conn.execute_batch(
            "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<600)
             INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,
                 display_name,identity_status,lifecycle,first_observed_at,last_observed_at)
             SELECT 'startup.scheduled_task','startup','M','I','scope','extra-'||x,
                    'zz-extra-'||x,'confirmed','active','2026-10-01','2026-10-01' FROM n;"
        ).unwrap();
        store
    }

    #[test]
    fn indexed_evidence_counts_preserve_exact_addresses_filters_and_page_windows() {
        let store = broad_reference_store();
        // Same address across kinds/objects counts once. Metadata is intentionally absent;
        // pruning changes availability, not whether the saved observation cites this address.
        for (subject, kind, address, pruned) in [
            ("1", "snapshot", "shared", None), ("2", "event", "shared", None),
            ("1", "event", "", None), ("2", "snapshot", "旧材料", Some("2026-10-02")),
            ("2", "snapshot", "z~quoted'\"", None), ("01", "event", "not-canonical", None),
            ("1x", "event", "not-an-integer", None), ("999999", "event", "missing-object", None),
        ] {
            store.conn.execute("INSERT INTO evidence_refs VALUES('observation',?,?,?,?)", params![subject, kind, address, pruned]).unwrap();
        }
        store.conn.execute("INSERT INTO evidence_refs VALUES('action','1','event','only-action',NULL)", []).unwrap();
        let first = list(&store, None, 1);
        assert_eq!(first.counts.as_ref().unwrap().evidence, 4);
        let second = list(&store, first.page.unwrap().next_cursor, 1);
        assert_eq!(second.data.as_ref().unwrap()[0]["instance_id"], 2);
        assert_eq!(second.counts.as_ref().unwrap().evidence, 3, "the second page excludes object 1's empty address");
        let narrow = store.query(&QueryRequest::InstanceList(InstanceFilter { search: Some("task-00".into()), ..Default::default() })).unwrap();
        assert_eq!(narrow.counts.unwrap().evidence, 2);
        let indexed = counts_for(&store.conn, &["i.instance_id>=?".into()], &[Box::new(2_i64)]).unwrap().evidence;
        store.conn.execute_batch("DROP INDEX evidence_refs_observation_address").unwrap();
        let legacy = counts_for(&store.conn, &["i.instance_id>=?".into()], &[Box::new(2_i64)]).unwrap().evidence;
        assert_eq!(indexed, legacy, "old read-only stores retain the same exact counting semantics");
        assert_eq!(list(&store, None, 1).counts.unwrap().evidence, 4);
    }

    #[test]
    fn broad_evidence_counts_skip_repeated_citations_with_a_bounded_sql_budget() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let store = broad_reference_store();
        store.conn.execute("DELETE FROM object_instances WHERE instance_id>301", []).unwrap();
        for index in 0..1000 {
            let address = match index {
                0 => String::new(),
                1 => "旧材料".into(),
                2 => "z~quoted'\"".into(),
                _ => format!("snapshot/{index:03}/{}", "long-address/".repeat(3)),
            };
            store.conn.execute(
                "INSERT INTO evidence_refs SELECT 'observation',CAST(instance_id AS TEXT),'snapshot',?,NULL FROM object_instances",
                [&address],
            ).unwrap();
        }
        // Make the selection genuinely partial so it must pay for the bounded reference probe.
        store.conn.execute_batch(
            "INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,
                display_name,identity_status,lifecycle,first_observed_at,last_observed_at)
             SELECT type_id,'shell',machine_id,installation_id,scope,'unrelated-'||instance_id,
                    display_name,identity_status,lifecycle,first_observed_at,last_observed_at FROM object_instances;"
        ).unwrap();
        let steps = Arc::new(AtomicUsize::new(0));
        let used = steps.clone();
        // The decision probes one quarter of the 301k citations. This budget includes that
        // explicit work but still rejects walking/de-duplicating all citations below.
        store.conn.progress_handler(1000, Some(move || used.fetch_add(1000, Ordering::Relaxed) >= 1_000_000));
        let result = store.query(&QueryRequest::InstanceList(InstanceFilter { category: Some("startup".into()), limit: 50, ..Default::default() })).unwrap();
        let counts = result.counts.unwrap();
        assert_eq!(counts.instances, 301, "fewer than 512 objects can still have dense citations");
        assert_eq!(counts.evidence, 1000);
        assert_eq!(result.data.unwrap().as_array().unwrap().len(), 50);
        let indexed_steps = steps.swap(0, Ordering::Relaxed);
        let legacy = store.conn.query_row(&counts_sql("i.category='startup'", false), [], |row| row.get::<_, i64>(3));
        assert!(legacy.is_err(), "301k citations must exceed the same bounded budget without address skipping; indexed used {indexed_steps} steps");
    }

    #[test]
    fn single_object_counts_never_probe_global_reference_volume() {
        let store=broad_reference_store();
        let sql=counts_sql("i.instance_id = ?",true);
        assert!(!sql.contains("route_scale"));assert!(!sql.contains("addresses(evidence_id)"));
        assert!(sql.contains("subject_id IN (SELECT subject_id FROM matched)"));
        let expected=store.conn.query_row(&counts_sql("i.instance_id = ?",false),[1],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?))).unwrap();
        let actual=counts_for(&store.conn,&["i.instance_id = ?".into()],&[Box::new(1i64)]).unwrap();
        assert_eq!((actual.instances as i64,actual.actions as i64,actual.source_records as i64,actual.evidence as i64),expected);
    }

    #[test]
    fn sparse_category_with_many_objects_does_not_walk_unrelated_citations() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let store = broad_reference_store();
        store.conn.execute_batch(
            "UPDATE object_instances SET category='runtime';
             INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,
                 display_name,identity_status,lifecycle,first_observed_at,last_observed_at)
             SELECT type_id,'shell',machine_id,installation_id,scope,'unrelated-'||instance_id,
                    display_name,identity_status,lifecycle,first_observed_at,last_observed_at FROM object_instances;
             INSERT INTO evidence_refs SELECT 'observation',CAST(instance_id AS TEXT),'snapshot','individual-'||instance_id,NULL FROM object_instances WHERE category='runtime';
             WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<300)
             INSERT INTO evidence_refs SELECT 'observation',CAST(instance_id AS TEXT),'snapshot','unrelated-shared-'||x,NULL FROM object_instances,n WHERE category='shell';"
        ).unwrap();
        let steps = Arc::new(AtomicUsize::new(0));
        let used = steps.clone();
        store.conn.progress_handler(1000, Some(move || used.fetch_add(1000, Ordering::Relaxed) >= 100_000));
        let counts = counts_for(&store.conn, &["i.category=?".into()], &[Box::new("runtime".to_owned())]).unwrap();
        assert_eq!(counts.instances, 602);
        assert_eq!(counts.evidence, 602);
        assert_eq!(counts.source_records, 2);
        assert!(!counts_sql("i.category=?", false).contains("probe_limit"), "legacy readers skip the unused route probe");
        assert!(!counts_sql("i.category=?", false).contains("INDEXED BY"));
    }

    #[test]
    fn changed_seen_and_type_paging_preserve_ties_null_changes_and_exact_counts() {
        let store = seeded(6);
        for (id, at) in [(1, "2026-10-02"), (2, "2026-10-03"), (3, "2026-10-03"), (5, "2026-10-01")] {
            store.conn.execute("INSERT INTO fact_history(instance_id,field_id,observed_from,observed_until,source,rule_version) VALUES(?,'enabled','2026-09-30',?,'fixture','1')", params![id,at]).unwrap();
        }
        store.conn.execute("UPDATE object_instances SET last_observed_at=coalesce((SELECT max(observed_until) FROM fact_history WHERE instance_id=object_instances.instance_id),'2026-09-30')", []).unwrap();
        store.conn.execute("UPDATE object_instances SET type_id='account.user',category='identity' WHERE instance_id IN (2,4)", []).unwrap();
        store.conn.execute("INSERT INTO evidence_refs SELECT 'observation',CAST(instance_id AS TEXT),'snapshot','individual-'||instance_id,NULL FROM object_instances", []).unwrap();
        for (sort, expected) in [
            (InstanceSort::ChangedDesc, vec![2,3,1,5,4,6]),
            (InstanceSort::SeenDesc, vec![2,3,1,5,4,6]),
            (InstanceSort::TypeAsc, vec![2,4,1,3,5,6]),
        ] {
            let mut cursor = None;
            let mut visited = Vec::new();
            for (offset, expected_id) in expected.iter().enumerate() {
                let page = store.query(&QueryRequest::InstanceList(InstanceFilter { sort, limit:1, cursor, ..Default::default() })).unwrap();
                let row = &page.data.as_ref().unwrap()[0];
                assert_eq!(row["instance_id"], *expected_id, "{sort:?} page {offset}");
                if matches!(*expected_id, 4 | 6) { assert!(row["changed_at"].is_null(), "no history remains unknown, not an invented change time"); }
                assert_eq!(page.counts.as_ref().unwrap().instances, (6-offset) as u64);
                assert_eq!(page.counts.as_ref().unwrap().evidence, (7-offset) as u64, "counts retain the exact same page window");
                visited.push(*expected_id);
                cursor = page.page.unwrap().next_cursor;
            }
            assert!(cursor.is_none());
            assert_eq!(visited, expected);
        }
    }

    #[test]
    fn a_walk_stays_stable_while_new_rows_arrive() {
        let mut store = seeded(5);
        let first = list(&store, None, 2);
        assert_eq!(names(&first), vec!["task-00", "task-01"]);
        let page = first.page.clone().expect("page");
        assert!(page.has_more);
        let cursor = page.next_cursor.expect("a cursor for the next page");

        // Two more objects arrive between the pages. They belong to a later query, not to this walk.
        store.ingest(&snapshot("run-2", &[("\\A\\task-99", "task-99")])).unwrap();
        store.project_pending("M", "I", 500).unwrap();

        let second = list(&store, Some(cursor.clone()), 2);
        assert_eq!(names(&second), vec!["task-02", "task-03"], "the walk continues where it stopped, without the new row spliced in");
        let third = list(&store, second.page.as_ref().and_then(|page| page.next_cursor.clone()), 2);
        assert_eq!(names(&third), vec!["task-04"]);

        // Nothing repeats and nothing is skipped across the walk.
        let walked: Vec<String> = [names(&first), names(&second), names(&third)].concat();
        let mut sorted = walked.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), walked.len(), "a row appeared twice: {walked:?}");
        assert_eq!(walked, vec!["task-00", "task-01", "task-02", "task-03", "task-04"]);

        // A fresh query is a new watermark, and it does see the new row.
        let fresh = list(&store, None, 10);
        assert!(names(&fresh).contains(&"task-99".to_owned()));
    }

    #[test]
    fn a_cursor_from_another_walk_is_refused_rather_than_re_scoped() {
        let store = seeded(3);
        let foreign = "e999i999~task-00~1";
        let response = list(&store, Some(foreign.to_owned()), 2);
        assert_eq!(response.status, ResponseStatus::Unsupported);
        // The difference matters: "no rows" and "that cursor is not mine" are not the same answer.
        assert_eq!(response.warnings[0].code, "stale_cursor");
    }

    #[test]
    fn a_filter_that_matches_nothing_is_empty_and_not_an_error() {
        let store = seeded(2);
        let filter = InstanceFilter { type_id: Some("startup.service".into()), ..InstanceFilter::default() };
        let response = store.query(&QueryRequest::InstanceList(filter)).unwrap();
        assert_eq!(response.status, ResponseStatus::Empty);
        assert!(names(&response).is_empty());
    }

    #[test]
    fn a_search_matches_the_name_and_the_key() {
        let store = seeded(4);
        let by_name = store.query(&QueryRequest::InstanceList(InstanceFilter { search: Some("task-02".into()), ..InstanceFilter::default() })).unwrap();
        assert_eq!(names(&by_name), vec!["task-02"]);
        let by_key = store.query(&QueryRequest::InstanceList(InstanceFilter { search: Some("task-03".into()), ..InstanceFilter::default() })).unwrap();
        assert_eq!(names(&by_key), vec!["task-03"]);
    }

    #[test]
    fn a_detail_query_answers_by_id_and_by_alias() {
        let store = seeded(1);
        let listed = list(&store, None, 10);
        let row = &listed.data.as_ref().unwrap()[0];
        let id = row["instance_id"].as_i64().unwrap();
        let alias = row["alias"].as_str().unwrap().to_owned();
        assert!(row["key_facts"].as_array().unwrap().iter().any(|fact|fact["field_id"]=="name"&&fact["value"]=="task-00"),"supplied readable names must be available to the list");

        for reference in [id.to_string(), alias] {
            let response = store.query(&QueryRequest::InstanceDetail { instance: reference.clone() }).unwrap();
            assert_eq!(response.status, ResponseStatus::Loaded, "{reference}");
            let data = response.data.unwrap();
            assert_eq!(data["summary"]["display_name"], "task-00");
            // The identity facts are what name the object, and they carry their labels.
            let facts = data["summary"]["key_facts"].as_array().unwrap();
            assert!(facts.iter().any(|fact| fact["field_id"] == "path"), "{facts:?}");
            assert!(facts.iter().any(|fact| fact["field_id"] == "path" && fact["label"] == "任务路径"), "{facts:?}");
        }
    }

    #[test]
    fn an_unknown_reference_says_so_rather_than_returning_a_blank_object() {
        let store = seeded(1);
        let response = store.query(&QueryRequest::InstanceDetail { instance: "no-such-instance".into() }).unwrap();
        assert_eq!(response.status, ResponseStatus::Empty);
        assert_eq!(response.warnings[0].code, "unknown_instance");
    }

    #[test]
    fn the_type_catalog_says_which_types_a_source_feeds() {
        let store = seeded(1);
        let response = store.query(&QueryRequest::TypeCatalog).unwrap();
        let entries = response.data.unwrap();
        let rows = entries.as_array().unwrap();
        assert!(rows.len() > 50, "the catalog declares the whole model, not just what has data");
        let task = rows.iter().find(|row| row["type_id"] == "startup.scheduled_task").expect("the type is declared");
        assert_eq!(task["supplied_by_source"], true);
    }

    #[test]
    fn evidence_reports_its_own_address_after_the_material_is_gone() {
        let store = seeded(1);
        let address: String = store.conn.query_row("SELECT evidence_id FROM evidence LIMIT 1", [], |r| r.get(0)).unwrap();
        let reference = EvidenceRef { kind: pcscope_contracts::adapter::EvidenceKind::Snapshot, id: address.clone() };
        let response = store.query(&QueryRequest::EvidenceDetail { evidence: reference }).unwrap();
        assert_eq!(response.status, ResponseStatus::Loaded);
        assert_eq!(response.data.as_ref().unwrap()["state"], "available");

        // Mark it pruned, as the retention sweep does, and ask again.
        store.conn.execute("UPDATE evidence SET pruned_at='2026-09-12T02:00:00Z'", []).unwrap();
        let response = store.query(&QueryRequest::EvidenceDetail { evidence: EvidenceRef { kind: pcscope_contracts::adapter::EvidenceKind::Snapshot, id: address } }).unwrap();
        assert_eq!(response.data.as_ref().unwrap()["state"], "pruned");
        assert_eq!(response.data.as_ref().unwrap()["raw_available"], false);
    }

    #[test]
    fn coverage_and_projection_answer_for_the_health_page() {
        let store = seeded(1);
        let coverage = store.query(&QueryRequest::SourceCoverage { source: None }).unwrap();
        assert_eq!(coverage.status, ResponseStatus::Loaded);
        let rows = coverage.data.unwrap();
        assert!(rows.as_array().unwrap().iter().any(|row| row["source"] == TASKS));

        let projection = store.query(&QueryRequest::ProjectionStatus).unwrap();
        assert_eq!(projection.status, ResponseStatus::Loaded);
    }

    fn add_action(store: &Store, id: &str, at: &str, instance: Option<i64>, unassigned: Option<&str>) {
        store
            .conn
            .execute(
                "INSERT INTO object_actions(instance_id,unassigned_reason,action_id,title,result,precision,occurred_at,observed_at,rule_version)
                 VALUES(?,?,?,?,'observed','exact',?,?, 'test')",
                rusqlite::params![instance, unassigned, id, id, at, at],
            )
            .unwrap();
    }

    fn timeline(store: &Store, filter: ActionFilter) -> QueryResponse {
        store.query(&QueryRequest::ActionTimeline(filter)).unwrap()
    }

    fn action_ids(response: &QueryResponse) -> Vec<String> {
        response.data.as_ref().and_then(Value::as_array).map(|rows| rows.iter().filter_map(|row| row["action_id"].as_str().map(str::to_owned)).collect()).unwrap_or_default()
    }

    #[test]
    fn sparse_scope_paging_matches_time_scan_without_visiting_unrelated_actions() {
        let store=seeded(2);
        store.conn.execute_batch("UPDATE object_instances SET category='hardware',type_id='hardware.device' WHERE instance_id=1;
            UPDATE object_instances SET category='network',type_id='network.endpoint' WHERE instance_id=2;
            WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20000)
            INSERT INTO object_actions(instance_id,action_id,result,precision,observed_at,rule_version)
            SELECT 2,'fixture.network','observed','observation_only','2026-10-20T00:00:00Z','test' FROM n;").unwrap();
        for _ in 0..9 {add_action(&store,"fixture.hardware","2020-01-01T00:00:00Z",Some(1),None);}
        for _ in 0..9 {add_action(&store,"identity.logon_failed","2020-01-01T00:00:00Z",None,Some("unknown SID"));}
        store.conn.execute("UPDATE object_actions SET target_type='account.user' WHERE action_id='identity.logon_failed'",[]).unwrap();
        // Keep this scale fixture independent of raw parsing; normal production builders
        // and revision invalidation have their own coverage in display_tests.
        store.conn.execute(&format!("INSERT INTO display_actions(action_row,revision,visible_main,visible_all,view,identity_guard,anchor,prepared_generation)
            SELECT a.action_row,?,1,1,json_object('action_id',a.action_id,'instance_id',a.instance_id,
                'observed_at',a.observed_at,'precision',a.precision,'context',json_object('type_id',coalesce(i.type_id,a.target_type))),
                {},coalesce(a.interval_until,a.occurred_at,a.observed_at),1
            FROM object_actions a LEFT JOIN object_instances i ON i.instance_id=a.instance_id",crate::display::identity_guard_sql()),[crate::display::revision()]).unwrap();
        store.conn.execute_batch("DELETE FROM display_work;DELETE FROM display_identity_work;UPDATE display_build SET published=1,generation=1;").unwrap();
        let alias:String=store.conn.query_row(&format!("SELECT {ALIAS} FROM object_instances WHERE instance_id=1"),[],|r|r.get(0)).unwrap();
        let filters=[
            ActionFilter{category:Some("hardware".into()),..Default::default()},
            ActionFilter{type_id:Some("hardware.device".into()),..Default::default()},
            ActionFilter{instance:Some("1".into()),..Default::default()},
            ActionFilter{instance:Some(alias),include_routine:true,..Default::default()},
            ActionFilter{category:Some("hardware".into()),type_id:Some("network.endpoint".into()),..Default::default()},
            ActionFilter{category:Some("hardware".into()),since:Some("2026".into()),..Default::default()},
            ActionFilter{action_id:Some("identity.logon_failed".into()),..Default::default()},
            ActionFilter{category:Some("account".into()),..Default::default()},
            ActionFilter{type_id:Some("account.user".into()),..Default::default()},
            ActionFilter{category:Some("account".into()),type_id:Some("account.user".into()),include_unassigned:false,..Default::default()},
        ];
        for mut filter in filters {
            filter.limit=3;filter.cards_only=true;
            loop {
                let expected=timeline_with_scope_plan(&store.conn,&filter,false).unwrap();
                let visits=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));let counter=visits.clone();
                store.conn.progress_handler(100,Some(move ||counter.fetch_add(1,std::sync::atomic::Ordering::Relaxed)>200));
                let actual=timeline(&store,filter.clone());
                store.conn.progress_handler(0,None::<fn()->bool>);
                assert_eq!(serde_json::to_value(&actual).unwrap(),serde_json::to_value(&expected).unwrap());
                let page=actual.page.unwrap();if !page.has_more {break;}filter.cursor=page.next_cursor;
            }
        }
        assert!(scoped_action_candidates(&store.conn,&ActionFilter{category:Some("network".into()),..Default::default()}).unwrap().is_none(),"dense scopes retain time-index paging");
        assert!(scoped_action_candidates(&store.conn,&ActionFilter::default()).unwrap().is_none());
    }

    #[test]
    fn lightweight_timeline_has_independent_limits_and_exact_details() {
        let store=seeded(1);
        for _ in 0..805 {add_action(&store,"test.card","2026-10-20T00:00:00Z",None,Some("unassigned"));}
        store.conn.execute("UPDATE object_actions SET source_record=?",[json!({"raw":"x".repeat(16000),"snapshot_presence":{"state":"Listen"}}).to_string()]).unwrap();
        let full=timeline(&store,ActionFilter{limit:800,..Default::default()});
        assert_eq!(full.data.as_ref().unwrap().as_array().unwrap().len(),200);
        let first=timeline(&store,ActionFilter{cards_only:true,limit:9999,..Default::default()});
        let data=first.data.as_ref().unwrap().as_array().unwrap();
        assert_eq!(data.len(),800);
        assert!(data[0]["source_record"]["raw"].is_null());
        assert_eq!(data[0]["source_record"]["snapshot_presence"]["state"],"Listen");
        let id=data[0]["detail_id"].as_i64().unwrap();
        let detail=store.query(&QueryRequest::ActionDetail{action_row:id}).unwrap().data.unwrap();
        assert_eq!(detail,full.data.unwrap()[0]);
        let second=timeline(&store,ActionFilter{cards_only:true,limit:400,cursor:first.page.unwrap().next_cursor,..Default::default()});
        assert_eq!(second.data.unwrap().as_array().unwrap().len(),5);
        assert!(!second.page.unwrap().has_more);
        assert_eq!(store.query(&QueryRequest::ActionDetail{action_row:-1}).unwrap().status,ResponseStatus::Empty);
    }

    #[test]
    fn lightweight_byte_budget_resumes_at_last_returned_row() {
        let store=seeded(1);
        for _ in 0..8 {add_action(&store,"test.large","2026-10-20T00:00:00Z",None,Some("unassigned"));}
        store.conn.execute("UPDATE object_actions SET before_value=?",[json!("x".repeat(1024*1024)).to_string()]).unwrap();
        let mut cursor=None;let mut ids=std::collections::HashSet::new();
        loop {
            let page=timeline(&store,ActionFilter{cards_only:true,limit:800,cursor,..Default::default()});
            let rows=page.data.unwrap();assert!(rows.to_string().len()<=TIMELINE_CARD_BYTES);
            for row in rows.as_array().unwrap(){assert!(ids.insert(row["detail_id"].as_i64().unwrap()));}
            let page=page.page.unwrap();if !page.has_more{break;}cursor=page.next_cursor;
        }
        assert_eq!(ids.len(),8);
    }

    #[test]
    fn the_timeline_orders_by_when_things_took_effect() {
        let store = seeded(1);
        let instance = store.conn.query_row("SELECT min(instance_id) FROM object_instances", [], |r| r.get(0)).unwrap();
        add_action(&store, "a.early", "2026-09-12T01:00:00Z", Some(instance), None);
        add_action(&store, "a.late", "2026-09-12T03:00:00Z", Some(instance), None);
        add_action(&store, "a.middle", "2026-09-12T02:00:00Z", Some(instance), None);
        // Newest first: a timeline reads backwards from now.
        assert_eq!(action_ids(&timeline(&store, ActionFilter::default())), vec!["a.late", "a.middle", "a.early"]);
    }

    #[test]
    fn an_action_that_belongs_to_no_object_is_still_something_that_happened() {
        let store = seeded(1);
        let instance = store.conn.query_row("SELECT min(instance_id) FROM object_instances", [], |r| r.get(0)).unwrap();
        add_action(&store, "a.bound", "2026-09-12T01:00:00Z", Some(instance), None);
        add_action(&store, "a.orphan", "2026-09-12T02:00:00Z", None, Some("the target identity could not be resolved"));

        let included = timeline(&store, ActionFilter::default());
        assert_eq!(action_ids(&included), vec!["a.orphan", "a.bound"]);
        // The row says it belongs to nothing, which is how the page marks it rather than showing it
        // as somebody's action.
        let orphan = &included.data.as_ref().unwrap()[0];
        assert!(orphan["instance_id"].is_null());
        assert!(orphan["unassigned_reason"].as_str().unwrap().contains("could not be resolved"));

        // A caller that asked for only attributed actions gets only those.
        let excluded = timeline(&store, ActionFilter { include_unassigned: false, ..ActionFilter::default() });
        assert_eq!(action_ids(&excluded), vec!["a.bound"]);
    }

    #[test]
    fn the_timeline_filters_and_pages_like_the_object_list() {
        let store = seeded(1);
        let instance = store.conn.query_row("SELECT min(instance_id) FROM object_instances", [], |r| r.get(0)).unwrap();
        for index in 0..5 {
            add_action(&store, &format!("a.{index}"), &format!("2026-09-12T0{index}:00:00Z"), Some(instance), None);
        }
        let first = timeline(&store, ActionFilter { limit: 2, ..ActionFilter::default() });
        assert_eq!(action_ids(&first), vec!["a.4", "a.3"]);
        let page = first.page.clone().expect("page");
        assert!(page.has_more);
        let second = timeline(&store, ActionFilter { limit: 2, cursor: page.next_cursor.clone(), ..ActionFilter::default() });
        assert_eq!(action_ids(&second), vec!["a.2", "a.1"], "the walk continues where it stopped");

        // A filter narrows what the walk covers, and a cursor from a different filter is refused.
        let filtered = timeline(&store, ActionFilter { action_id: Some("a.2".into()), ..ActionFilter::default() });
        assert_eq!(action_ids(&filtered), vec!["a.2"]);
        let foreign = timeline(&store, ActionFilter { action_id: Some("a.2".into()), cursor: page.next_cursor.clone(), ..ActionFilter::default() });
        assert_eq!(foreign.status, ResponseStatus::Unsupported);
    }

    #[test]
    fn a_since_bound_asks_only_for_what_happened_after_it() {
        let store = seeded(1);
        let instance = store.conn.query_row("SELECT min(instance_id) FROM object_instances", [], |r| r.get(0)).unwrap();
        add_action(&store, "a.old", "2026-09-12T01:00:00Z", Some(instance), None);
        add_action(&store, "a.new", "2026-09-12T05:00:00Z", Some(instance), None);
        let response = timeline(&store, ActionFilter { since: Some("2026-09-12T03:00:00Z".into()), ..ActionFilter::default() });
        assert_eq!(action_ids(&response), vec!["a.new"]);
    }
}
