//! Object and action presentation builders, separated from paging and count queries.
//! This source participates in the disposable display fingerprint.
use super::*;

pub(super) const ACTION_COLUMNS:&str="a.action_row,a.action_id,a.title,a.result,a.phase,a.precision,a.occurred_at,
    a.interval_from,a.interval_until,a.observed_at,a.field_id,a.before_value,a.after_value,
    a.unassigned_reason,a.instance_id,a.source_record,a.target_type";

/// One representation for prepared views, fallback timelines and object history.
pub(super) fn action_row(row:&rusqlite::Row<'_>)->rusqlite::Result<Value> {
    let precision:String=row.get(5)?;let occurred:Option<String>=row.get(6)?;
    Ok(json!({
        "action_id":row.get::<_,String>(1)?,"title":row.get::<_,Option<String>>(2)?,
        "result":row.get::<_,String>(3)?,"phase":row.get::<_,Option<String>>(4)?,
        "precision":precision,"occurred_at":occurred,
        "interval_from":row.get::<_,Option<String>>(7)?,"interval_until":row.get::<_,Option<String>>(8)?,
        "observed_at":row.get::<_,String>(9)?,"field_id":row.get::<_,Option<String>>(10)?,
        "before":row.get::<_,Option<String>>(11)?.and_then(|text|serde_json::from_str::<Value>(&text).ok()),
        "after":row.get::<_,Option<String>>(12)?.and_then(|text|serde_json::from_str::<Value>(&text).ok()),
        "unassigned_reason":row.get::<_,Option<String>>(13)?,"instance_id":row.get::<_,Option<i64>>(14)?,
        "source_record":row.get::<_,Option<String>>(15)?.and_then(|text|serde_json::from_str::<Value>(&text).ok()),
        "_target_type":row.get::<_,Option<String>>(16)?,"_action_row":row.get::<_,i64>(0)?,
        "time_derived":precision!="exact"||occurred.is_none()
    }))
}

pub(crate) fn is_legacy_com_rule(rule: &str) -> bool {
    history_labels::LEGACY_COM_RULES.contains(&rule)
}

/// The alias a reference carries before an id exists, exactly as `InstanceIdentity::alias` spells
/// it, so a stored row can be found by either.
pub(super) const ALIAS: &str = "type_id || '@' || machine_id || '|' || installation_id || '|' || scope || '|' || native_key || '#' || incarnation";
pub(super) const CHANGED_AT_SQL: &str = "(SELECT max(observed_until) FROM fact_history h WHERE h.instance_id=i.instance_id)";

// Names such as a software DisplayName are configuration facts, not identity fields. Read the
// same known value for display, search, sort and cursor boundaries, including pre-existing data.
pub(super) const PROVIDED_NAME: &str = "(SELECT trim(json_extract(f.value,'$')) FROM object_facts f
    WHERE f.instance_id=i.instance_id AND f.field_id IN ('display_name','name','caption','subject')
      AND f.value_status='known' AND json_type(f.value)='text'
      AND trim(json_extract(f.value,'$')) NOT IN ('','-')
    ORDER BY CASE f.field_id WHEN 'display_name' THEN 0 WHEN 'name' THEN 1 WHEN 'caption' THEN 2 ELSE 3 END LIMIT 1)";

pub(super) fn display_name_sql()->&'static str {
    // The legacy column can hold an identity path. A software registration's source name lives
    // in its facts, so that cache must not override an explicitly absent or protected name.
    static SQL:std::sync::LazyLock<String>=std::sync::LazyLock::new(||format!("coalesce({PROVIDED_NAME},{},CASE WHEN i.type_id='software.installation_registration' THEN '' ELSE i.display_name END,'')",crate::registration_dictionary::name_sql()));
    SQL.as_str()
}


/// Identity, readable names and the type's declared state summary, using the shared presentation model.
pub(super) fn key_facts(conn: &Connection, instance_id: i64, type_id: &str) -> Result<Vec<Value>> {
    let summary_fields = pcscope_contracts::model::summary_field_ids(type_id);
    let mut statement = conn.prepare_cached(
        "SELECT field_id,field_kind,value,value_type,value_status,source_time,observed_at,derivation
         FROM object_facts WHERE instance_id=? ORDER BY field_id",
    )?;
    let rows = statement.query_map(params![instance_id], |row| {
        let value: Option<String> = row.get(2)?;
        Ok(json!({
            "field_id": row.get::<_, String>(0)?,
            "label": pcscope_contracts::model::field_definition(type_id, &row.get::<_, String>(0)?)
                .map(|(label, _)| label)
                .unwrap_or_default(),
            "kind": row.get::<_, String>(1)?,
            "value": value.and_then(|text| serde_json::from_str::<Value>(&text).ok()),
            "value_type": row.get::<_, String>(3)?,
            "status": row.get::<_, String>(4)?,
            "source_time": row.get::<_, Option<String>>(5)?,
            "observed_at": row.get::<_, String>(6)?,
            "derivation": row.get::<_, String>(7)?,
        }))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?.into_iter().filter(|fact| {
        let id = fact["field_id"].as_str().unwrap_or_default();
        fact["kind"] == "identity" || summary_fields.contains(&id)
            || matches!(id, "display_name" | "name" | "description" | "caption" | "subject" | "publisher")
    }).collect())
}

/// Presentation is derived from supplied facts, never from an internal identity or record number.
/// An uninstall registration without DisplayName is not proof that a product is installed.
pub(super) fn presentation(type_id: &str, facts: &[Value], identity:Option<(&str,&str,&str)>) -> Value {
    let name = ["display_name", "name", "caption", "subject"]
        .iter()
        .find_map(|key| facts.iter().find(|fact| fact["field_id"] == *key && fact["status"] == "known")
            .and_then(|fact| readable_text(&fact["value"])));
    if type_id == "software.installation_registration" {
        if name.is_none() && identity.is_some_and(|(scope,key,status)|{
            let prefix=format!("{}|",scope.trim().to_lowercase());
            let parts:Vec<_>=key.strip_prefix(&prefix).unwrap_or_default().split('|').collect();
            status=="confirmed" && parts.len()==3 && ["currentuser","localmachine"].contains(&parts[0])
                && ["registry32","registry64","32","64"].contains(&parts[1])
                && ["software\\microsoft\\windows\\currentversion\\uninstall","hklm\\software\\microsoft\\windows\\currentversion\\uninstall","hkcu\\software\\microsoft\\windows\\currentversion\\uninstall"].contains(&parts[2])
        }) {return json!({"name":"软件安装信息目录","name_status":"container","basis":"registration_container"});}
        match name {
            Some(name) => json!({"name":name,"description":"系统中保留了该软件的安装登记。","name_status":"source_provided","basis":"display_name"}),
            None => identity.and_then(|(scope,key,status)|crate::registration_dictionary::lookup(type_id,scope,key,status,facts))
                .unwrap_or_else(||json!({"name":"未命名的软件登记","description":"系统保留了这条登记，尚不能据此确认对应软件是否已安装。","name_status":"not_provided","basis":"registration_without_display_name"})),
        }
    } else {
        json!({"name":name,"name_status":if name.is_some(){"source_provided"}else{"not_provided"},"basis":"object_facts"})
    }
}

/// Only clear, non-placeholder text is eligible for a user-facing label. Protected values remain
/// protected: a JSON object containing ciphertext must never become a name through to_string().
pub(super) fn readable_text(value: &Value) -> Option<String> {
    value.as_str().map(str::trim)
        .filter(|text| !text.is_empty() && *text != "-" && !pcscope_contracts::semantics::is_mask_sentinel(value))
        .map(str::to_owned)
}

pub(super) fn short_path_name(path: &str) -> String {
    path.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or(path).to_owned()
}

/// Reconstruct changed fields at the action's observation. In particular, a subsequent rename
/// must never rename an old snapshot change. Event subjects still come from their own evidence.
pub(super) fn historical_facts(conn:&Connection,instance:i64,type_id:&str,at:&str)->Result<Vec<Value>> {
    let mut facts=key_facts(conn,instance,type_id)?;
    let mut statement=conn.prepare_cached("SELECT before_value,before_status,before_source_time,observed_from
        FROM fact_history WHERE instance_id=? AND field_id=? AND observed_until>?
        ORDER BY observed_until LIMIT 1")?;
    for fact in &mut facts {
        if matches!(fact["status"].as_str(),Some("masked"|"unreadable"|"conflicting"|"evidence_pruned"))
            || pcscope_contracts::semantics::is_mask_sentinel(&fact["value"]) {continue;}
        let prior:Option<(Option<String>,Option<String>,Option<String>,String)>=statement.query_row(
            params![instance,fact["field_id"].as_str(),at],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        if let Some((value,status,time,observed))=prior {
            fact["value"]=value.and_then(|v|serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null);
            fact["status"]=json!(status);fact["source_time"]=json!(time);fact["observed_at"]=json!(observed);
        }
    }
    Ok(facts)
}

/// Keep historical event labels tied to that event's own evidence. Current object facts are a
/// separately marked fallback, so a renamed task does not silently rename its old executions.
pub(super) fn action_context(conn: &Connection, action_row: i64, instance: Option<i64>, target_type: Option<&str>, action_id: &str, field_id: Option<&str>, labels: &mut HistoryLabelCache) -> Result<Value> {
    let display_name=display_name_sql();
    let current: Option<Value> = match instance {
        Some(id) => conn.prepare_cached(&format!("SELECT i.type_id,{display_name},i.scope,i.native_key,i.machine_id,i.installation_id,i.identity_status FROM object_instances i WHERE i.instance_id=?"))?.query_row([id], |row| Ok(json!({"type_id":row.get::<_,String>(0)?,"display_name":row.get::<_,Option<String>>(1)?,"scope":row.get::<_,String>(2)?,"native_key":row.get::<_,String>(3)?,"machine_id":row.get::<_,String>(4)?,"installation_id":row.get::<_,String>(5)?,"identity_status":row.get::<_,String>(6)?}))).optional()?,
        None => None,
    };
    let type_id = current.as_ref().and_then(|object|object["type_id"].as_str()).or(target_type).unwrap_or_default();
    let at:String=conn.query_row("SELECT observed_at FROM object_actions WHERE action_row=?",[action_row],|r|r.get(0))?;
    let facts = instance.map(|id| historical_facts(conn,id,type_id,&at)).transpose()?.unwrap_or_default();
    let registry_identity=current.as_ref().and_then(|object|Some((object["scope"].as_str()?,object["native_key"].as_str()?,object["identity_status"].as_str()?)));
    let present = presentation(type_id, &facts,registry_identity);
    let mut context = json!({
        "type_id":type_id,
        "type_name":pcscope_contracts::model::type_summary(type_id).map(|(_,name,_)|name),
        "display_name":present["name"],
        "identity_status":current.as_ref().and_then(|object|object["identity_status"].as_str()),
        "key_facts":facts,
        "presentation":present,
        "subject_label":Value::Null,
        "subject_detail":Value::Null,
        "subject_basis":"unavailable",
        "stored_instance_id":instance,
        "attribution_status":if instance.is_some(){"stored"}else{"unassigned"},
        "attribution_reason":Value::Null,
        "field_label":field_id.and_then(|id|pcscope_contracts::model::field_definition(type_id,id)).map(|(label,_)|label),
    });
    // Start at this action's primary-key prefix. Without the explicit join order SQLite may
    // scan every retained event for each timeline row, despite both address indexes existing.
    let mut statement = conn.prepare_cached(
        "SELECT r.data,e.source,e.item_id,e.native_record_id,e.source_scope,e.evidence_id,e.source_time,e.observed_at,e.ingested_at,e.structure_version FROM evidence_refs er
         CROSS JOIN evidence e ON e.evidence_id=er.evidence_id AND e.kind=er.evidence_kind
         CROSS JOIN records r ON r.seq=CAST(substr(e.payload_ref,8) AS INTEGER)
         WHERE er.subject_kind='action' AND er.subject_id=? AND er.pruned_at IS NULL
           AND e.pruned_at IS NULL AND e.kind='event' AND e.payload_ref LIKE 'record:%'
         ORDER BY e.seq LIMIT 8"
    )?;
    let has_event:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM evidence_refs WHERE subject_kind='action' AND subject_id=? AND evidence_kind='event')",[action_row.to_string()],|row|row.get(0))?;
    let persisted=crate::event_association::stored(conn,action_row,instance)?;
    if let Some(proof)=&persisted {
        context["attribution_status"]=proof["status"].clone();context["attribution_reason"]=proof["reason"].clone();
    } else if has_event {context["attribution_status"]=json!("unverified");context["attribution_reason"]=json!("尚不能确认这条记录属于哪个对象。");}
    // Snapshot changes cannot have event labels or event-attribution checks. Avoid stepping the
    // event join for them: one snapshot address can refer to thousands of independent items.
    if has_event {
    let rows = statement.query_map([action_row.to_string()], |row| {
        Ok(pcscope_contracts::adapter::EvidenceInput {
            payload:serde_json::from_str::<Value>(&row.get::<_,String>(0)?).unwrap_or(Value::Null),
            source:row.get(1)?,item_id:row.get(2)?,native_record_id:row.get(3)?,source_scope:row.get(4)?,
            evidence:EvidenceRef{kind:pcscope_contracts::adapter::EvidenceKind::Event,id:row.get(5)?},
            source_time:row.get(6)?,observed_at:row.get(7)?,ingested_at:row.get(8)?,structure_version:row.get(9)?,
            decoding:pcscope_contracts::adapter::Decoding{available:true,..Default::default()},
        })
    })?;
    for input in rows {
        let input=input?;
        if persisted.is_none() {
        let scope=if input.source_scope.trim().is_empty(){format!("machine:{}",current.as_ref().and_then(|object|object["machine_id"].as_str()).unwrap_or_default())}else{pcscope_contracts::semantics::business_scope(&input.source_scope)};
        let adapter_context=pcscope_contracts::adapter::AdapterContext{
            catalog_version:pcscope_contracts::model::catalog_version().into(),rule_version:String::new(),
            machine_id:current.as_ref().and_then(|object|object["machine_id"].as_str()).unwrap_or_default().into(),
            installation_id:current.as_ref().and_then(|object|object["installation_id"].as_str()).unwrap_or_default().into(),
            boot_id:None,collected_scope:scope,complete:true,
        };
        let mut adapted=pcscope_contracts::adapters::event::EventAdapter.adapt(&input,&adapter_context);
        if let Some((identity,_))=crate::projection::service_state_target(conn,&input,&adapter_context)? {
            for action in &mut adapted.actions {
                if action.action_id=="startup.service_state_changed" {
                    action.target=pcscope_contracts::adapter::ActionTarget::Assigned {identity:identity.clone()};
                }
            }
        }
        let matching=adapted.actions.iter().find(|action|action.action_id==action_id);
        if matching.is_none() {
            context["classification_status"]=json!("unverified");
            context["stored_action_id"]=json!(action_id);
            context["attribution_status"]=json!("mismatch");
            context["attribution_reason"]=json!("原记录的事件类型与日志来源不符，已按普通系统事件展示。");
            continue;
        }
        if let Some(action)=matching {
            if let pcscope_contracts::adapter::ActionTarget::Unassigned{reason,..}=&action.target {
                context["attribution_status"]=json!(if instance.is_some(){"mismatch"}else{"unassigned"});
                context["attribution_reason"]=json!(reason);
                if let Some(target)=pcscope_contracts::model::action_definition(action_id).and_then(|entry|entry["target_type"].as_str()) {
                    context["type_id"]=json!(target);
                    context["type_name"]=json!(pcscope_contracts::model::type_summary(target).map(|(_,name,_)|name));
                }
            }
        }
        if let (Some(current),Some(action),Some(instance))=(&current,matching,instance) {
            if let pcscope_contracts::adapter::ActionTarget::Assigned{identity}=&action.target {
                let normalised=pcscope_contracts::identity::TypeStrategy::for_type(&identity.type_id).map(|strategy|pcscope_contracts::identity::normalise(&strategy,identity));
                let binding:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM source_bindings WHERE instance_id=? AND source=? AND item_id=? AND scope=? AND native_record_id IN (?,?))",params![instance,input.source,input.item_id,identity.scope,input.native_record_id,input.evidence.id],|row|row.get(0))?;
                let verified=normalised.is_some_and(|(key,missing)|missing.is_empty()&&current["native_key"]==key)
                    &&current["type_id"]==identity.type_id&&current["scope"]==identity.scope&&current["identity_status"]=="confirmed"&&binding;
                if !verified {
                    context["attribution_status"]=json!("mismatch");
                    context["attribution_reason"]=json!("这条记录的对象信息与历史关联不一致，尚不能确认归属。");
                } else if context["attribution_status"]!="mismatch" {
                    context["attribution_status"]=json!("verified");context["attribution_reason"]=Value::Null;
                }
            }
        }
        }
        let fields=&input.payload["fields"];
        let first = |keys: &[&str]| keys.iter().find_map(|key|readable_text(&fields[*key]));
        let scm=pcscope_contracts::event_state::service_display_name(&input);
        let subject = if let Some(display_name)=scm {
            context["type_id"]=json!("startup.service");context["type_name"]=json!("系统服务");
            Some((display_name.clone(),display_name))
        } else if action_id.starts_with("startup.task_") {
            first(&["TaskName"]).map(|full|(short_path_name(&full),full))
        } else if action_id.starts_with("identity.") {
            let keys: &[&str] = if action_id == "identity.special_privileges" { &["SubjectUserName"] } else { &["TargetUserName", "AccountName", "UserName"] };
            first(keys).map(|name| {
                let domain = if action_id == "identity.special_privileges" {first(&["SubjectDomainName"])} else {first(&["TargetDomainName", "DomainName"])};
                let full = domain.map(|domain|format!("{domain}\\{name}")).unwrap_or_else(||name.clone());
                let sid = if action_id == "identity.special_privileges" {first(&["SubjectUserSid"])} else {first(&["TargetUserSid"])};
                let label = match sid.as_deref() {Some("S-1-5-18")=>"系统账户".to_owned(),Some("S-1-5-19")=>"本地服务账户".to_owned(),Some("S-1-5-20")=>"网络服务账户".to_owned(),_=>name};
                (label,full)
            })
        } else if type_id == "startup.service" || type_id == "hardware.driver_service" {
            first(&["ServiceName", "DriverName"]).map(|name|(name.clone(),name))
                .or_else(||first(&["ImageLoaded"]).map(|full|(short_path_name(&full),full)))
        } else if type_id == "runtime.module_instance" {
            first(&["ImageLoaded"]).map(|full|(short_path_name(&full),full))
        } else if type_id.starts_with("file.") {
            first(&["TargetFilename", "ObjectName"]).map(|full|(short_path_name(&full),full))
        } else if type_id == "runtime.process" {
            first(&["NewProcessName", "Image", "ProcessName"]).map(|full|(short_path_name(&full),full))
        } else {None};
        if let Some((label,full)) = subject {
            if !label.is_empty() {
                context["subject_label"] = json!(label);
                context["subject_detail"] = json!(full);
                context["subject_basis"] = json!("event_evidence");
            }
        }
        if let Some(program)=first(&["NewProcessName", "Image", "ProcessName"]) {context["program_label"]=json!(short_path_name(&program));}
        if let Some(logon_type)=first(&["LogonType"]) {context["logon_type"]=json!(logon_type);}
    }
    }
    if context["subject_basis"] == "unavailable" && present["name_status"] == "source_provided" && (!has_event || context["attribution_status"]=="verified") {
        context["subject_label"] = present["name"].clone();
        context["subject_basis"] = json!("current_object_facts");
    }
    if !has_event && type_id=="shell.com_registration" {
        if let (Some(instance),Some(current))=(instance,&current) {
            if !labels.contains_key(&instance) {labels.insert(instance,history_labels::com_metadata(conn,instance,current)?);}
            let metadata=&labels[&instance];
            let protected_name=context["key_facts"].as_array().is_some_and(|facts|facts.iter().any(|fact|
                matches!(fact["field_id"].as_str(),Some("name"|"display_name"))
                    && (matches!(fact["status"].as_str(),Some("masked"|"unreadable"|"conflicting"|"evidence_pruned"))
                        || fact["value"]==pcscope_contracts::sensitive::MASK
                        || fact["value"]["sensitive"]==true || fact["value"]["redacted"]==true
                        || fact["value"]["display"]==pcscope_contracts::sensitive::MASK
                        || (fact["status"]=="known"&&!fact["value"].is_null()&&!fact["value"].is_string()))));
            if context["subject_basis"]=="unavailable" && !protected_name {
                if let Some(label)=&metadata.label {
                    context["subject_label"]=json!(label);
                    context["subject_detail"]=json!(label);
                    context["subject_basis"]=json!("collected_registry_label");
                }
            }
            if field_id==Some("server_path") && metadata.mixed_roles {
                let rule:String=conn.query_row("SELECT rule_version FROM object_actions WHERE action_row=?",[action_row],|row|row.get(0))?;
                if is_legacy_com_rule(&rule) {
                    context["history_quality"]=json!({"status":"legacy_mapping_ambiguous","reason":"旧版混用了组件说明和文件路径，这条比较暂不能用于判断系统变化。"});
                }
            }
        }
    }
    if has_event && context["attribution_status"]!="verified" {
        // The historical object remains auditable, but cannot provide a default name for an event
        // whose link was not verified. Otherwise a frontend fallback would reintroduce the error.
        context["display_name"]=Value::Null;
        context["identity_status"]=Value::Null;
        context["key_facts"]=json!([]);
        context["presentation"]=json!({"name":Value::Null,"name_status":"not_provided","basis":"unverified_attribution"});
    }
    if context["classification_status"]=="unverified" {
        context["type_id"]=json!("");context["type_name"]=json!("系统事件");
        context["subject_label"]=Value::Null;context["subject_detail"]=Value::Null;context["subject_basis"]=json!("unavailable");
    }
    Ok(context)
}

/// The collection this instance is bound to, as coverage views.
pub(super) fn coverage_of(conn: &Connection, instance_id: i64) -> Result<Vec<Value>> {
    if !crate::display::available(conn)? {return Ok(Vec::new());}
    let mut statement = conn.prepare_cached(
        "SELECT DISTINCT c.source,c.scope,c.configured,c.last_result,c.last_success_at,c.last_attempt_at,c.history_state,c.detail
         FROM object_collection_scopes b JOIN coverage_scopes c ON c.source=b.source AND c.scope=b.scope
         WHERE b.instance_id=? ORDER BY c.source,c.scope",
    )?;
    let rows = statement.query_map(params![instance_id], |row| {
        Ok(json!({
            "source": row.get::<_, String>(0)?,
            "scope": row.get::<_, String>(1)?,
            "configured": row.get::<_, i64>(2)? != 0,
            "last_result": row.get::<_, String>(3)?,
            "last_success_at": row.get::<_, Option<String>>(4)?,
            "last_attempt_at": row.get::<_, Option<String>>(5)?,
            "history_state": row.get::<_, String>(6)?,
            "detail": row.get::<_, Option<String>>(7)?,
        }))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn summary_row(conn: &Connection, row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let instance_id: i64 = row.get(0)?;
    if let Some(mut summary)=crate::display::cached(conn,"object",instance_id).map_err(sql_display_error)? {
        // These cheap live attributes change every poll, independently of semantic display facts.
        summary["seen_at"]=json!(row.get::<_,Option<String>>(10)?);
        summary["lifecycle"]=json!(row.get::<_,String>(8)?);
        summary["coverage"]=json!(coverage_of(conn,instance_id).map_err(sql_display_error)?);
        return Ok(summary);
    }
    uncached_summary_row(conn,row)
}

pub(super) fn sql_display_error(error:anyhow::Error)->rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(error.into())
}

pub(super) fn uncached_summary_row(conn: &Connection, row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    uncached_summary_row_cached(conn,row,&mut ProcessLabelCache::default())
}
pub(super) fn uncached_summary_row_cached(conn:&Connection,row:&rusqlite::Row<'_>,labels:&mut ProcessLabelCache)->rusqlite::Result<Value>{
    let instance_id: i64 = row.get(0)?;
    let type_id: String = row.get(1)?;
    let fact = key_facts(conn, instance_id, &type_id).unwrap_or_default();
    let coverage = coverage_of(conn, instance_id).unwrap_or_default();
    let scope:String=row.get(6)?;let identity_status:String=row.get(7)?;let native_key:String=row.get(11)?;
    let mut presentation = presentation(&type_id, &fact,Some((&scope,&native_key,&identity_status)));
    let field=|id:&str|fact.iter().find(|f|f["field_id"]==id);
    let legacy_process=identity_status=="provisional" && field("created_at").is_some_and(|f|f["status"]=="not_provided");
    let process_started=field("created_at").filter(|f|f["status"]=="known").and_then(|f|f["value"].as_str());
    if type_id=="runtime.process" && (legacy_process || identity_status=="confirmed"&&process_started.is_some())
        && field("image").is_some_and(|f|f["status"]=="not_provided")
        && field("name").is_none_or(|f|f["status"]=="not_provided") {
        if let Some(pid)=field("pid").filter(|f|f["status"]=="known").and_then(|f|f["value"].as_u64()) {
            if legacy_process {presentation["record_status"]=json!("legacy_snapshot");}
            if let Ok(Some(label))=history_labels::legacy_process_label_cached(conn,instance_id,&scope,pid,process_started,labels) {
                presentation["record_label"]=json!(label);
                if !legacy_process {presentation["record_status"]=json!("snapshot_named");}
            } else if legacy_process {
                let multiple:bool=conn.query_row("SELECT count(*)>1 FROM (SELECT DISTINCT item_id FROM source_bindings WHERE instance_id=? AND source='pcscope.execution.processes' AND scope=? LIMIT 2)",params![instance_id,scope],|r|r.get(0))?;
                if multiple {presentation["record_status"]=json!("legacy_mixed_process");}
            }
        }
    }
    if type_id=="account.group_membership" && identity_status=="confirmed" {
        if let Some(sid)=field("member").filter(|f|f["status"]=="known").and_then(|f|f["value"].as_str()) {
            if native_key.rsplit('|').next()==Some(sid) {
                if let Ok(Some(label))=history_labels::member_label(conn,instance_id,&scope,sid) {presentation["member_label"]=json!(label);}
            }
        }
    }
    // An event-only identity says that a log mentioned something; it is not a current inventory
    // observation. This also keeps legacy event placeholders from looking like live accounts.
    let record_basis = if type_id.starts_with("account.") || fact.is_empty() {
        let observed:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM evidence_refs WHERE subject_kind='observation' AND subject_id=?)",[instance_id.to_string()],|r|r.get(0))?;
        if observed {"observation"} else {
            let logged:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM object_actions WHERE instance_id=?)",[instance_id],|r|r.get(0))?;
            if logged {"event_only"} else {"unknown"}
        }
    } else {"unknown"};
    // Old event placeholders often have no inventory facts. A log label can describe that
    // record, but must never become a verified current process/session name or identity.
    if record_basis == "event_only" && fact.is_empty() {
        let mut statement=conn.prepare_cached("SELECT action_row,action_id FROM object_actions WHERE instance_id=? ORDER BY action_row DESC LIMIT 9")?;
        let actions=statement.query_map([instance_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut labels=std::collections::BTreeSet::new();let mut invalid=false;
        if actions.len()<=8 {
            for (action,id) in &actions {
                if let Ok(context)=action_context(conn,*action,Some(instance_id),Some(&type_id),id,None,&mut HistoryLabelCache::new()) {
                    if context["classification_status"]=="unverified" {invalid=true;continue;}
                    if context["subject_basis"]=="event_evidence" {
                        if let Some(name)=readable_text(&context["subject_label"]) {labels.insert(name);}
                    }
                }
            }
        }
        presentation["record_status"]=json!(if invalid {"legacy_misclassified"} else if labels.len()==1 {"event_named"} else {"event_incomplete"});
        if !invalid && labels.len()==1 {presentation["record_label"]=json!(labels.into_iter().next().unwrap());}
    }
    Ok(json!({
        "instance_id": instance_id,
        "alias": row.get::<_, String>(2)?,
        "type_id": type_id,
        "category": row.get::<_, String>(3)?,
        "type_name": row.get::<_, String>(4)?,
        "display_name": row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        "scope": row.get::<_, String>(6)?,
        "identity_status": row.get::<_, String>(7)?,
        "lifecycle": row.get::<_, String>(8)?,
        "key_facts": fact,
        "presentation": presentation,
        "record_basis": record_basis,
        "changed_at": row.get::<_, Option<String>>(9)?,
        "seen_at": row.get::<_, Option<String>>(10)?,
        "coverage": coverage,
    }))
}

pub(crate) fn build_summary(conn:&Connection,id:i64,labels:&mut ProcessLabelCache)->Result<Option<Value>> {
    static SQL:std::sync::LazyLock<String>=std::sync::LazyLock::new(||format!("SELECT i.instance_id,i.type_id,{ALIAS},i.category,
        coalesce((SELECT name FROM type_definitions t WHERE t.type_id=i.type_id),i.type_id),
        {},i.scope,i.identity_status,i.lifecycle,{CHANGED_AT_SQL},i.last_observed_at,i.native_key
        FROM object_instances i WHERE i.instance_id=?",display_name_sql()));
    Ok(conn.prepare_cached(&SQL)?.query_row([id],|row|uncached_summary_row_cached(conn,row,labels)).optional()?)
}


pub(super) fn enrich_action_context(conn: &Connection, action: &mut Value, labels: &mut HistoryLabelCache) -> Result<()> {
    if let Some(view)=crate::display::cached(conn,"action",action["_action_row"].as_i64().unwrap_or_default())? {
        *action=view;
        return Ok(());
    }
    uncached_enrich_action_context(conn,action,labels)
}

pub(super) fn uncached_enrich_action_context(conn: &Connection, action: &mut Value, labels: &mut HistoryLabelCache) -> Result<()> {
    let context = action_context(conn, action["_action_row"].as_i64().unwrap_or_default(), action["instance_id"].as_i64(), action["_target_type"].as_str(), action["action_id"].as_str().unwrap_or_default(), action["field_id"].as_str(),labels)?;
    if matches!(context["attribution_status"].as_str(),Some("unverified"|"mismatch")) {
        action["instance_id"]=Value::Null;
        action["unassigned_reason"]=context["attribution_reason"].clone();
    }
    if action["action_id"]=="identity.special_privileges" && context["classification_status"]!="unverified" {
        action["title"]=json!("登录获得特殊权限");
    }
    if context["classification_status"]=="unverified" {
        action["action_id"]=json!("windows.event");action["title"]=json!("系统事件");action["result"]=json!("observed");action["phase"]=Value::Null;
    }
    action["context"] = context;
    if let Some(object) = action.as_object_mut() {
        object.remove("_action_row");
        object.remove("_target_type");
    }
    Ok(())
}

pub(crate) fn build_action(conn:&Connection,id:i64)->Result<Option<(Value,bool,bool)>> {
    let raw:Option<Value>=conn.query_row(&format!("SELECT {ACTION_COLUMNS} FROM object_actions a WHERE a.action_row=?"),[id],action_row).optional()?;
    let Some(mut view)=raw else{return Ok(None)};
    let type_id:Option<String>=view["instance_id"].as_i64().map(|id|conn.query_row("SELECT type_id FROM object_instances WHERE instance_id=?",[id],|r|r.get(0)).optional()).transpose()?.flatten();
    let show=|routine|crate::timeline_policy::visible(view["action_id"].as_str().unwrap_or_default(),
        type_id.as_deref().or(view["_target_type"].as_str()).unwrap_or_default(),
        view["field_id"].as_str().unwrap_or_default(),Some(&view["before"]),Some(&view["after"]),routine);
    let (main,all)=(show(false),show(true));
    uncached_enrich_action_context(conn,&mut view,&mut HistoryLabelCache::new())?;
    Ok(Some((view,main,all)))
}

