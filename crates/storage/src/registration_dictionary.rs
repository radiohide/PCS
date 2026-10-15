//! Descriptions for precisely identified Windows uninstall registrations. This is presentation
//! metadata, never an installation inventory, health verdict, or replacement for source facts.
use pcscope_contracts::{adapters::identity, identity::{normalise, TypeStrategy}};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, sync::LazyLock};

const TYPE_ID: &str = "software.installation_registration";
const ROOT: &str = "hklm\\software\\microsoft\\windows\\currentversion\\uninstall\\";

#[derive(Debug, Deserialize)]
struct Spec {
    schema_version: u32,
    id: String,
    revision: String,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    id: String,
    hive: String,
    registry_path: String,
    views: Vec<String>,
    name: String,
    description: String,
    #[serde(default)]
    publishers: Vec<String>,
    #[serde(default)]
    references: Vec<String>,
}

struct Dictionary {
    spec: Spec,
    by_suffix: BTreeMap<String, usize>,
}

impl Dictionary {
    fn parse(raw: &str) -> Result<Self, String> {
        let spec: Spec=serde_json::from_str(raw).map_err(|error|error.to_string())?;
        if spec.schema_version!=1 || spec.id.trim().is_empty() || spec.revision.trim().is_empty() {return Err("dictionary metadata is incomplete".into())}
        let strategy=TypeStrategy::for_type(TYPE_ID).ok_or("registration identity strategy is absent")?;
        let mut ids=BTreeSet::new();let mut by_suffix=BTreeMap::new();
        for (index,entry) in spec.entries.iter().enumerate() {
            let path=entry.registry_path.trim().trim_end_matches('\\').to_lowercase();
            let leaf=path.strip_prefix(ROOT).filter(|leaf|!leaf.is_empty()&&!leaf.contains(['\\','|']));
            if leaf.is_none() || !entry.hive.eq_ignore_ascii_case("LocalMachine") || entry.views.is_empty()
                || entry.name.trim().is_empty() || entry.description.trim().is_empty() || entry.id.trim().is_empty() || !ids.insert(entry.id.clone()) {
                return Err(format!("invalid or duplicate registration dictionary entry {}",entry.id))
            }
            for view in &entry.views {
                if !matches!(view.to_lowercase().as_str(),"32"|"64"|"registry32"|"registry64") {return Err(format!("unsupported registry view {view}"))}
                // Scope remains a query-row input; every other identity component is an exact
                // dictionary value normalised by the same contract as the stored identity.
                let candidate=identity(TYPE_ID,"","dictionary",&[("hive",Some(entry.hive.clone())),("view",Some(view.clone())),("registry_path",Some(entry.registry_path.clone()))]);
                let (suffix,missing)=normalise(&strategy,&candidate);
                if !missing.is_empty() || by_suffix.insert(suffix,index).is_some() {return Err(format!("overlapping registration match in {}",entry.id))}
            }
        }
        Ok(Self{spec,by_suffix})
    }

    fn entry(&self, scope:&str, native_key:&str)->Option<&Entry> {
        let scope=scope.trim().to_lowercase();
        let suffix=native_key.strip_prefix(&scope)?;
        self.by_suffix.get(suffix).map(|index|&self.spec.entries[*index])
    }

    fn name_sql(&self)->String {
        if self.by_suffix.is_empty(){return "NULL".into()}
        let mut sql=format!("CASE WHEN i.type_id={} AND i.identity_status='confirmed' AND {} THEN CASE ",quote(TYPE_ID),MISSING_NAME_SQL);
        for (suffix,index) in &self.by_suffix {
            let entry=&self.spec.entries[*index];
            sql.push_str(&format!("WHEN i.native_key=lower(trim(i.scope))||{} AND {} THEN {} ",quote(suffix),publisher_sql(entry),quote(&entry.name)));
        }
        sql.push_str("ELSE NULL END ELSE NULL END");sql
    }
}

fn quote(text:&str)->String {format!("'{}'",text.replace('\'',"''"))}

fn publisher_sql(entry:&Entry)->String {
    let allowed=entry.publishers.iter().map(|publisher|quote(&publisher.trim().to_lowercase())).collect::<Vec<_>>();
    let explicit=if allowed.is_empty(){"0".to_owned()}else{format!("lower(trim(json_extract(dp.value,'$'))) IN ({})",allowed.join(","))};
    format!("EXISTS(SELECT 1 FROM object_facts dp WHERE dp.instance_id=i.instance_id AND dp.field_id='publisher' AND ((dp.value_status='not_provided' AND (dp.value IS NULL OR json_type(dp.value)='null')) OR (dp.value_status='known' AND json_type(dp.value)='text' AND (trim(json_extract(dp.value,'$'))='' OR {explicit}))))")
}

// Absence is explicit. Masked, protected, conflicting, unreadable or missing facts cannot be
// replaced by a dictionary name. Known non-string values are not assumed to mean an empty name.
const MISSING_NAME_SQL:&str="EXISTS(SELECT 1 FROM object_facts dn WHERE dn.instance_id=i.instance_id AND dn.field_id='display_name' AND ((dn.value_status='not_provided' AND (dn.value IS NULL OR json_type(dn.value)='null')) OR (dn.value_status='known' AND json_type(dn.value)='text' AND trim(json_extract(dn.value,'$'))='')))";

static DICTIONARY:LazyLock<Dictionary>=LazyLock::new(||Dictionary::parse(include_str!("registration_dictionary.json")).expect("the bundled registration dictionary is valid"));

pub(crate) fn name_sql()->String {DICTIONARY.name_sql()}
pub(crate) fn revision()->&'static str {&DICTIONARY.spec.revision}

pub(crate) fn lookup(type_id:&str, scope:&str, native_key:&str, identity_status:&str, facts:&[Value])->Option<Value> {
    if type_id!=TYPE_ID || identity_status!="confirmed" {return None}
    let fact=facts.iter().find(|fact|fact["field_id"]=="display_name")?;
    let absent=(fact["status"]=="not_provided"&&fact["value"].is_null())
        ||(fact["status"]=="known"&&fact["value"].as_str().is_some_and(|value|value.trim().is_empty()));
    if !absent {return None}
    let entry=DICTIONARY.entry(scope,native_key)?;
    let publisher=facts.iter().find(|fact|fact["field_id"]=="publisher")?;
    let publisher_allowed=(publisher["status"]=="not_provided"&&publisher["value"].is_null())
        ||(publisher["status"]=="known"&&publisher["value"].as_str().is_some_and(|value|value.trim().is_empty()||entry.publishers.iter().any(|allowed|allowed.trim().eq_ignore_ascii_case(value.trim()))));
    if !publisher_allowed{return None}
    Some(json!({
        "name":entry.name,"description":entry.description,"name_status":"dictionary_matched","basis":"registration_dictionary",
        "dictionary_id":DICTIONARY.spec.id,"dictionary_revision":DICTIONARY.spec.revision,"dictionary_entry_id":entry.id,
        "match_basis":"按精确登记标识匹配的名称解释；组件资料用于说明用途，不证明登记真实性、软件已安装或运行正常。",
        "technical_key":native_key,"references":entry.references,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec(path:&str,name:&str)->String {json!({"schema_version":1,"id":"test","revision":"1","entries":[{"id":"example","hive":"LocalMachine","registry_path":path,"views":["32","64"],"name":name,"description":"仅说明登记用途。"}]}).to_string()}

    #[test]
    fn registry_paths_and_views_are_exact_and_overlap_is_rejected() {
        let path=format!("{ROOT}Example");let dictionary=Dictionary::parse(&spec(&path,"示例登记")).unwrap();
        let key="machine:m|localmachine|64|hklm\\software\\microsoft\\windows\\currentversion\\uninstall\\example";
        assert!(dictionary.entry("machine:M",key).is_some());
        assert!(dictionary.entry("machine:M",&format!("{key}-suffix")).is_none());
        assert!(dictionary.entry("machine:M",&key.replace("localmachine","currentuser")).is_none());
        assert!(dictionary.entry("machine:M",&key.replace("|64|","|128|")).is_none());
        let mut duplicated:Value=serde_json::from_str(&spec(&path,"name")).unwrap();
        duplicated["entries"][0]["views"]=json!(["64","64"]);
        assert!(Dictionary::parse(&duplicated.to_string()).is_err());
        assert!(Dictionary::parse(&spec("HKLM\\Other\\Example","name")).is_err());
    }

    #[test]
    fn dictionary_sql_quotes_registration_names_as_data() {
        let path=format!("{ROOT}Example's component");let dictionary=Dictionary::parse(&spec(&path,"工具 'one'")).unwrap();
        let conn=rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE object_instances(instance_id INTEGER,type_id TEXT,identity_status TEXT,scope TEXT,native_key TEXT);CREATE TABLE object_facts(instance_id INTEGER,field_id TEXT,value_status TEXT,value TEXT);").unwrap();
        let suffix=dictionary.by_suffix.keys().next().unwrap();
        conn.execute("INSERT INTO object_instances VALUES(1,?,'confirmed','machine:M',?)",rusqlite::params![TYPE_ID,format!("machine:m{suffix}")]).unwrap();
        conn.execute("INSERT INTO object_facts VALUES(1,'display_name','not_provided',NULL)",[]).unwrap();
        conn.execute("INSERT INTO object_facts VALUES(1,'publisher','not_provided',NULL)",[]).unwrap();
        let query=format!("SELECT {} FROM object_instances i",dictionary.name_sql());
        assert_eq!(conn.query_row(&query,[],|row|row.get::<_,String>(0)).unwrap(),"工具 'one'");
        for state in ["masked","conflicting","unreadable","evidence_pruned"] {
            conn.execute("UPDATE object_facts SET value_status=? WHERE field_id='display_name'",[state]).unwrap();
            assert!(conn.query_row(&query,[],|row|row.get::<_,Option<String>>(0)).unwrap().is_none());
        }
    }

    #[test]
    fn bundled_dictionary_is_valid() {let _=&*DICTIONARY;}
}
