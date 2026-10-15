use serde_json::Value;
use std::sync::LazyLock;

static CATALOG: LazyLock<Vec<Value>> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../apps/desktop/ui/collection-catalog.json"
    ))
    .expect("valid bundled collection catalog")
});
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

// SQL fragments use only bundled metadata and internal column names, never user input.
pub fn item(source: &str, id: &str) -> String {
    classify(source,id,None)
}
pub fn event_item(source:&str,id:&str,kind:&str,data:&str)->String {
    classify(source,id,Some((kind,data)))
}
fn classify(source:&str,id:&str,event:Option<(&str,&str)>)->String {
    let mut rows: Vec<_> = CATALOG.iter().collect();
    rows.sort_by_key(|r| {
        (
            r["prefix"].as_bool().unwrap_or(false),
            r["contains"].is_null(),
            r["priority"].as_u64().unwrap_or(0),
        )
    });
    let mut sql = String::from("CASE ");
    for r in rows {
        let value = r["source"].as_str().unwrap();
        let condition = if r["prefix"] == true {
            format!("substr({source},1,{})={}", value.len(), quoted(value))
        } else {
            format!("{source}={}", quoted(value))
        };
        let mut extra = r["contains"]
            .as_array()
            .map(|parts| {
                format!(
                    " AND ({})",
                    parts
                        .iter()
                        .map(|p| format!("instr(lower({id}),{})>0", quoted(p.as_str().unwrap())))
                        .collect::<Vec<_>>()
                        .join(" OR ")
                )
            })
            .unwrap_or_default();
        if let Some(rule)=r.get("event_rule") {
            if let Some((kind,data))=event {
                if rule["status"]==true {
                    extra.push_str(&format!(" AND substr({kind},1,9)='coverage.' AND {kind}<>'coverage.security_log_cleared'"));
                }
                if let Some(provider)=rule["provider"].as_str(){extra.push_str(&format!(" AND json_extract({data},'$.provider')={}",quoted(provider)));}
                if let Some(ids)=rule["event_ids"].as_array(){extra.push_str(&format!(" AND json_extract({data},'$.event_id') IN ({})",ids.iter().map(|v|v.as_u64().expect("event ID").to_string()).collect::<Vec<_>>().join(",")));}
            }else if rule["suffix"]!="other" {continue;}
        }
        sql.push_str(&format!(
            "WHEN {condition}{extra} THEN {} ",
            quoted(r["id"].as_str().unwrap())
        ));
    }
    sql.push_str(&format!("ELSE {source} END"));
    sql
}
pub fn field(item: &str, field: &str, fallback: &str) -> String {
    assert!(["name","category"].contains(&field));
    format!("COALESCE((SELECT {field} FROM collection_definitions WHERE key=({item})),{fallback})")
}
pub fn definitions() -> &'static [Value] { &CATALOG }
