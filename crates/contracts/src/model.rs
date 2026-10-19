//! The neutral object-model catalog, embedded so the backend and the UI cannot drift apart.
//!
//! `model/catalog.json` is generated from `model/*.cjs` by `scripts/check-model-catalog.cjs`.
//! This module is the only place the catalog is read from disk; `pcscope-storage` uses it to fill
//! the catalog lookup tables, and adapters use it to validate their candidates.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use std::collections::HashMap;

/// Version of the catalog this build understands. `scripts/check-model-catalog.cjs` fails when this
/// and the catalog disagree, so a consumer cannot silently run against a different model.
pub const MODEL_CATALOG_VERSION: &str = "1.2.7";
const CATALOG_JSON: &str = include_str!("../../../model/catalog.json");
const PRESENTATION_JSON: &str = include_str!("../../../model/presentation.json");
static PRESENTATION: LazyLock<Value> = LazyLock::new(||
    serde_json::from_str(PRESENTATION_JSON).expect("model/presentation.json must be valid JSON"));

/// Shared with every UI surface; the query stays bounded to one object's indexed facts.
pub fn summary_field_ids(type_id: &str) -> Vec<&'static str> {
    PRESENTATION["types"][type_id]["summary_fields"].as_array()
        .map(|fields| fields.iter().filter_map(Value::as_str).filter(|id| has_field(type_id, id)).collect())
        .unwrap_or_default()
}

pub static CATALOG: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(CATALOG_JSON).expect("model/catalog.json must be valid JSON"));

pub fn catalog() -> &'static Value {
    &CATALOG
}
pub fn catalog_version() -> &'static str {
    CATALOG["version"].as_str().unwrap_or("unknown")
}
/// Content digest of the catalog. Used both as the derived-table fingerprint and as the rule
/// version recorded against projected facts and actions.
///
/// Line endings are normalised before hashing. With `core.autocrlf` enabled the same commit can be
/// checked out with different byte sequences in different work trees, and a version identifier that
/// changes with the checkout form would defeat its own purpose: two work streams reporting "the same
/// rule version" must actually mean the same rules.
pub fn catalog_fingerprint() -> String {
    static FINGERPRINT:LazyLock<String>=LazyLock::new(||format!("{:x}",Sha256::digest(normalise_line_endings(CATALOG_JSON).as_bytes())));
    FINGERPRINT.clone()
}

/// CRLF and LF describe the same content; only the content belongs in a version identifier.
pub fn normalise_line_endings(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains("\r\n") {
        std::borrow::Cow::Owned(text.replace("\r\n", "\n"))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}
pub fn types() -> &'static [Value] {
    CATALOG["types"].as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn actions() -> &'static [Value] {
    CATALOG["actions"].as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn sources() -> &'static [Value] {
    CATALOG["sources"].as_array().map(Vec::as_slice).unwrap_or(&[])
}
/// The type definition for an ID, or `None` when the ID is not registered. A candidate naming an
/// unregistered type is a contract violation, never a new type discovered at runtime.
pub fn type_definition(type_id: &str) -> Option<&'static Value> {
    static INDEX:LazyLock<HashMap<&'static str,&'static Value>>=LazyLock::new(||types().iter().filter_map(|entry|Some((entry["id"].as_str()?,entry))).collect());
    INDEX.get(type_id).copied()
}
/// The registered spelling of an action id, when the catalog declares it.
///
/// The catalog is static, so this turns a runtime string into the `'static` id a candidate has to
/// carry — without it, a caller would have to own the string it looked up.
pub fn registered_action_id(action_id: &str) -> Option<&'static str> {
    action_definition(action_id)?["id"].as_str()
}

pub fn action_definition(action_id: &str) -> Option<&'static Value> {
    static INDEX:LazyLock<HashMap<&'static str,&'static Value>>=LazyLock::new(||actions().iter().filter_map(|entry|Some((entry["id"].as_str()?,entry))).collect());
    INDEX.get(action_id).copied()
}
/// Field IDs declared by a type. Empty when the type is unknown.
pub fn field_ids(type_id: &str) -> Vec<&'static str> {
    type_definition(type_id)
        .and_then(|entry| entry["fields"].as_array())
        .map(|fields| fields.iter().filter_map(|field| field["id"].as_str()).collect())
        .unwrap_or_default()
}
pub fn has_field(type_id: &str, field_id: &str) -> bool {
    field_value(type_id,field_id).is_some()
}
fn field_value(type_id:&str,field_id:&str)->Option<&'static Value>{
    static INDEX:LazyLock<HashMap<&'static str,HashMap<&'static str,&'static Value>>>=LazyLock::new(||types().iter().filter_map(|typ|{
        let id=typ["id"].as_str()?;let fields=typ["fields"].as_array()?.iter().filter_map(|field|Some((field["id"].as_str()?,field))).collect();Some((id,fields))
    }).collect());
    INDEX.get(type_id)?.get(field_id).copied()
}
/// Label and kind a type declares for a field. A projection reads these instead of inventing a
/// display name, so the frontend never has to map a field ID to something readable itself.
pub fn field_definition(type_id: &str, field_id: &str) -> Option<(&'static str, &'static str)> {
    let field = field_value(type_id,field_id)?;
    Some((field["value"]["label"].as_str()?, field["value"]["kind"].as_str()?))
}
/// Category and display name of a type, for building a summary without a second lookup.
pub fn type_summary(type_id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let definition = type_definition(type_id)?;
    Some((definition["category"].as_str()?, definition["name"].as_str()?, definition["identity"]["strategy"].as_str()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_matches_its_declared_version() {
        assert_eq!(catalog_version(), MODEL_CATALOG_VERSION);
        assert!(types().len() > 100, "catalog types look truncated");
        assert!(actions().len() > 60, "catalog actions look truncated");
        assert!(!catalog_fingerprint().is_empty());
    }

    #[test]
    fn unknown_identifiers_resolve_to_nothing() {
        assert!(type_definition("startup.scheduled_task").is_some());
        assert!(type_definition("startup.no_such_type").is_none());
        assert!(action_definition("startup.task_created").is_some());
        assert!(action_definition("startup.task_vanished").is_none());
        assert!(has_field("startup.scheduled_task", "path"));
        assert!(!has_field("startup.scheduled_task", "not_a_field"));
        assert!(field_ids("no.such.type").is_empty());
    }

    #[test]
    fn indexed_catalog_preserves_every_declared_definition_and_digest() {
        assert_eq!(catalog_fingerprint(),format!("{:x}",Sha256::digest(normalise_line_endings(CATALOG_JSON).as_bytes())));
        for typ in types(){let id=typ["id"].as_str().unwrap();assert_eq!(type_definition(id),Some(typ));
            for field in typ["fields"].as_array().unwrap(){let field_id=field["id"].as_str().unwrap();assert!(has_field(id,field_id));
                assert_eq!(field_definition(id,field_id),Some((field["value"]["label"].as_str().unwrap(),field["value"]["kind"].as_str().unwrap())));}}
        for action in actions(){let id=action["id"].as_str().unwrap();assert_eq!(action_definition(id),Some(action));assert_eq!(registered_action_id(id),Some(id));}
    }
}
