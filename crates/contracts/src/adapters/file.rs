//! S14 file and access-control adapters.
//!
//! One source, three types. `pcscope.files.metadata-security` reads the paths the user put on the
//! watch list — one item per path, and nothing else on the machine is touched — and one item can
//! describe a file, several access-control entries and several data streams at once. That is the
//! tree shape the survey describes, and each part of it becomes the object it names.
//!
//! The file's identity is a pair the collector now sends: the volume it lives on and the id the
//! filesystem gave it. A path is deliberately **not** used as the identity — it changes when the
//! file is renamed, which is the case a file monitor exists to survive. Until this step the
//! collectors sent no such pair and every file object would have been provisional; the pair comes
//! from `GetFileInformationByHandle` in the collector's native helper.
//!
//! The two composite types take the same pair joined as their `file_identity` key, so an access
//! entry and a data stream resolve against the file they belong to rather than against a path that
//! may since have moved.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.files.metadata-security", type_id: None, build: metadata_and_security },
    SourceSpec { source: "pcscope.files.directory-security", type_id: Some("file.watched_directory"), build: directory_metadata },
];

fn directory_metadata(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if input.payload["exists"].as_bool() == Some(false) { return AdapterResult::not_an_object("configured directory absent; not proof of deletion") }
    let (Some(volume), Some(file_id)) = (text(&input.payload, "volume_identity"), text(&input.payload, "file_id")) else { return AdapterResult::failed("directory filesystem identity unavailable", false) };
    object_candidate(input, context, identity("file.watched_directory", &installation_scope(&context.collected_scope), &input.evidence.id, &[("volume_identity", Some(volume)), ("file_id", Some(file_id))]), vec![
        field(&input.payload, "path", "path", FieldKind::Identity), field(&input.payload, "exists", "exists", FieldKind::State),
        field(&input.payload, "attributes", "attributes", FieldKind::Configuration), field(&input.payload, "owner", "owner", FieldKind::Configuration),
        field(&input.payload, "dacl", "dacl_sddl", FieldKind::Configuration), field(&input.payload, "entries", "entries", FieldKind::State),
        field(&input.payload, "last_write", "last_write_utc", FieldKind::State),
    ])
}

/// The pair the file types key on, joined so it can be one component of a composite key.
///
/// Both parts are the collector's own spellings; joining them keeps the file's identity recognisable
/// in an access entry or a stream without inventing a third form.
fn file_identity_value(payload: &Value) -> Option<String> {
    let volume = text(payload, "volume_identity")?;
    let id = text(payload, "file_id")?;
    Some(format!("{volume}|{id}"))
}

/// One ACE out of an SDDL discretionary ACL.
///
/// The collector sends the DACL as the SDDL text Windows itself produced, and an entry's fields are
/// the ACE's own: the trustee is the trailing SID, the access mask is the rights token, and an
/// inherited entry says so in its flags. Parsing the string is reading what the source stated; the
/// alternative — one opaque `dacl` string on the file — would leave every individual permission
/// unaddressable, which is the thing `file.acl_entry` exists to avoid.
struct Ace {
    trustee: String,
    access: String,
    inherited: bool,
}

fn parse_dacl(sddl: &str) -> Vec<Ace> {
    // The DACL is the `D:` section; sections are single letters, and the owner `O:` and group `G:`
    // that precede it are not permissions.
    let Some(start) = sddl.find("D:") else { return Vec::new() };
    let mut aces = Vec::new();
    let mut rest = &sddl[start + 2..];
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')') else { break };
        let body = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        let parts: Vec<&str> = body.split(';').collect();
        // (type;flags;rights;object guid;inherit guid;trustee)
        if parts.len() < 6 {
            continue;
        }
        let trustee = parts[5].trim();
        if trustee.is_empty() {
            continue;
        }
        aces.push(Ace {
            trustee: trustee.to_owned(),
            access: parts[2].trim().to_owned(),
            // `ID` is an inherited ACE; `IU`/`IO` are inherit-only, which is still inheritance.
            inherited: parts[1].contains("ID") || parts[1].contains("IU") || parts[1].contains("IO"),
        });
    }
    aces
}

/// Whether a stream is the file's own data, as opposed to an alternate one.
fn is_default_stream(name: &str) -> bool {
    name.is_empty() || name.eq_ignore_ascii_case(":$DATA")
}

pub(super) fn metadata_and_security(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    // A path that was not there states its own limit: absent at observation is not proof of
    // deletion. It also has no identity to key on, so no object is proposed for it.
    if input.payload.get("exists").and_then(Value::as_bool) == Some(false) {
        return AdapterResult::not_an_object("the path was absent at observation, which the collector states is not proof of deletion; no identity can be formed for a file that was not opened");
    }
    let Some(identity_value) = file_identity_value(&input.payload) else {
        // The collector could not open the path, so it has no identity. Proposing an object anyway
        // would key every unreadable path on the same missing pair and merge them into one.
        let error = text(&input.payload, "identity_error").unwrap_or_default();
        return AdapterResult::not_an_object(format!("the file identity could not be read (win32 error {error}); a file that was never opened has nothing to key on"));
    };
    let volume = text(&input.payload, "volume_identity").unwrap_or_default();
    let file_id = text(&input.payload, "file_id").unwrap_or_default();

    let mut candidates = vec![ObservationCandidate {
        identity: identity("file.watched_file", scope, &input.evidence.id, &[("volume_identity", Some(volume.clone())), ("file_id", Some(file_id.clone()))]),
        fields: vec![
            field(&input.payload, "path", "path", FieldKind::Identity),
            field(&input.payload, "exists", "exists", FieldKind::State),
            field(&input.payload, "size", "size", FieldKind::State),
            field(&input.payload, "creation_time", "creation_utc", FieldKind::Configuration),
            field(&input.payload, "last_write", "last_write_utc", FieldKind::Configuration),
            field(&input.payload, "attributes", "attributes", FieldKind::Configuration),
            field(&input.payload, "owner", "owner", FieldKind::Configuration),
            field(&input.payload, "dacl", "dacl_sddl", FieldKind::Configuration),
            field(&input.payload, "sha256", "sha256", FieldKind::State),
        ],
        observed_at: input.observed_at.clone(),
        evidence: vec![input.evidence.clone()],
        complete: context.complete,
    }];

    if let Some(dacl) = text(&input.payload, "dacl_sddl") {
        for ace in parse_dacl(&dacl) {
            candidates.push(ObservationCandidate {
                identity: identity(
                    "file.acl_entry",
                    scope,
                    &input.evidence.id,
                    &[("file_identity", Some(identity_value.clone())), ("trustee", Some(ace.trustee.clone())), ("access_mask", Some(ace.access.clone()))],
                ),
                fields: vec![
                    known("trustee", FieldKind::Identity, Value::String(ace.trustee)),
                    known("access", FieldKind::Identity, Value::String(ace.access)),
                    known("inherited", FieldKind::Configuration, Value::Bool(ace.inherited)),
                ],
                observed_at: input.observed_at.clone(),
                evidence: vec![input.evidence.clone()],
                complete: context.complete,
            });
        }
    }

    if let Some(Value::Array(streams)) = input.payload.get("streams") {
        for stream in streams {
            let Some(name) = stream.get("name").and_then(Value::as_str) else { continue };
            // The unnamed stream is the file's own data, not an alternate one; the type is about
            // the alternates.
            if is_default_stream(name) {
                continue;
            }
            candidates.push(ObservationCandidate {
                identity: identity(
                    "file.alternate_data_stream",
                    scope,
                    &input.evidence.id,
                    &[("file_identity", Some(identity_value.clone())), ("stream_name", Some(name.to_owned()))],
                ),
                fields: vec![known("stream_name", FieldKind::Identity, Value::String(name.to_owned())), field(stream, "size", "size", FieldKind::State)],
                observed_at: input.observed_at.clone(),
                evidence: vec![input.evidence.clone()],
                complete: context.complete,
            });
        }
    }
    observations(candidates)
}

/// A field whose value is known because the adapter just stated it.
fn known(field_id: &str, kind: FieldKind, value: Value) -> FieldCandidate {
    FieldCandidate {
        field_id: field_id.into(),
        field_kind: kind,
        value_type: value_type(&value),
        value: Some(value),
        status: ValueStatus::Known,
        source_time: None,
        derivation: Derivation::SourceProvided,
        conflicts: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::EvidenceKind;
    use serde_json::json;

    /// A discretionary ACL taken from a real file on the machine this was written on
    /// (`%SystemRoot%\System32\kernel32.dll`), not composed for the test. It carries the DACL
    /// control flag `P` between `D:` and the first ACE, and mixes a symbolic mask with a hex one.
    const REAL_SDDL: &str = "O:S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464G:S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464D:P(A;;0x1200a9;;;SY)(A;;0x1200a9;;;BA)(A;;0x1200a9;;;BU)(A;;FA;;;S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464)(A;;0x1200a9;;;AC)(A;;0x1200a9;;;S-1-15-2-2)";

    fn input(payload: Value) -> EvidenceInput {
        EvidenceInput {
            source: "pcscope.files.metadata-security".into(),
            item_id: "c:\\example\\watched.exe".into(),
            structure_version: "v1".into(),
            source_scope: "machine:M/user:S-1-5-18/schema:1/context:/shard:0".into(),
            evidence: EvidenceRef { kind: EvidenceKind::Snapshot, id: "pcscope.files.metadata-security|scope|run-1".into() },
            native_record_id: "c:\\example\\watched.exe".into(),
            source_time: None,
            observed_at: "2026-09-12T00:00:00Z".into(),
            ingested_at: "2026-09-12T00:00:01Z".into(),
            decoding: Decoding { available: true, ..Default::default() },
            payload,
        }
    }

    fn context() -> AdapterContext {
        AdapterContext {
            catalog_version: crate::model::catalog_version().into(),
            rule_version: crate::model::catalog_fingerprint(),
            machine_id: "M".into(),
            installation_id: "I".into(),
            boot_id: None,
            collected_scope: crate::semantics::business_scope("machine:M/user:S-1-5-18/schema:1/context:/shard:0"),
            complete: true,
        }
    }

    fn found_file() -> Value {
        json!({
            "path": "C:\\Example\\watched.exe", "exists": true, "directory": false, "size": 4096,
            "creation_utc": "2026-01-02T00:00:00Z", "last_write_utc": "2026-01-03T00:00:00Z",
            "attributes": "Archive", "reparse_point": false, "content_stored": false,
            "volume_identity": "volume:64770b15", "file_id": "file:000500000017455b", "identity_status": "observed",
            "basis": "explicit file allowlist snapshot; no recursive directory scan"
        })
    }

    fn of<'a>(result: &'a AdapterResult, type_id: &str) -> Vec<&'a ObservationCandidate> {
        result.observations.iter().filter(|candidate| candidate.identity.type_id == type_id).collect()
    }

    #[test]
    fn a_watched_file_is_keyed_by_the_volume_and_the_file_id() {
        let result = metadata_and_security(&input(found_file()), &context());
        let watched = of(&result, "file.watched_file");
        assert_eq!(watched.len(), 1);
        let identity = &watched[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "volume_identity" && field.value == "volume:64770b15"));
        assert!(identity.fields.iter().any(|field| field.key == "file_id" && field.value == "file:000500000017455b"));
        // The path is a field, never the key: it changes when the file is renamed.
        assert!(identity.fields.iter().all(|field| field.key != "path"));
    }

    #[test]
    fn a_real_acl_becomes_one_entry_per_ace() {
        let mut payload = found_file();
        payload["dacl_sddl"] = json!(REAL_SDDL);
        let result = metadata_and_security(&input(payload), &context());
        let entries = of(&result, "file.acl_entry");
        assert_eq!(entries.len(), 6, "the real ACL carries six entries");
        let trustees: Vec<String> = entries
            .iter()
            .filter_map(|entry| entry.fields.iter().find(|field| field.field_id == "trustee").and_then(|field| field.value.as_ref()).and_then(Value::as_str).map(str::to_owned))
            .collect();
        assert!(trustees.contains(&"SY".to_string()) && trustees.contains(&"BA".to_string()) && trustees.contains(&"BU".to_string()), "{trustees:?}");
        // The symbolic mask and the hex mask are both carried as the source stated them.
        let masks: Vec<String> = entries
            .iter()
            .filter_map(|entry| entry.fields.iter().find(|field| field.field_id == "access").and_then(|field| field.value.as_ref()).and_then(Value::as_str).map(str::to_owned))
            .collect();
        assert!(masks.contains(&"FA".to_string()) && masks.contains(&"0x1200a9".to_string()), "{masks:?}");
        for entry in &entries {
            assert!(entry.identity.missing.is_empty(), "{:?}", entry.identity.missing);
            assert!(entry.identity.fields.iter().any(|field| field.key == "file_identity" && field.value == "volume:64770b15|file:000500000017455b"));
        }
    }

    #[test]
    fn an_inherited_entry_says_so() {
        // Synthetic, to reach the inherited flag the real file above does not happen to exercise.
        let sddl = "O:SYG:SYD:(A;ID;0x1301bf;;;BU)(A;;FA;;;SY)";
        let mut payload = found_file();
        payload["dacl_sddl"] = json!(sddl);
        let result = metadata_and_security(&input(payload), &context());
        let entries = of(&result, "file.acl_entry");
        assert_eq!(entries.len(), 2);
        let inherited = |entry: &&ObservationCandidate| entry.fields.iter().find(|field| field.field_id == "inherited").and_then(|field| field.value.as_ref()).and_then(Value::as_bool).unwrap_or(false);
        assert!(entries.iter().any(inherited));
        assert!(entries.iter().any(|entry| !inherited(entry)));
    }

    #[test]
    fn an_alternate_data_stream_becomes_its_own_object_and_the_default_one_does_not() {
        let mut payload = found_file();
        payload["streams"] = json!([{"name": ":$DATA", "size": 4096}, {"name": ":Zone.Identifier", "size": 26}]);
        let result = metadata_and_security(&input(payload), &context());
        let streams = of(&result, "file.alternate_data_stream");
        // The unnamed stream is the file's own data, not an alternate one.
        assert_eq!(streams.len(), 1, "{:?}", streams.iter().map(|s| &s.identity.fields).collect::<Vec<_>>());
        assert!(streams[0].identity.fields.iter().any(|field| field.key == "stream_name" && field.value == ":Zone.Identifier"));
    }

    #[test]
    fn a_path_that_was_not_there_proposes_no_object() {
        let payload = json!({"path": "C:\\Example\\gone.exe", "exists": false, "basis": "explicit file path absent at observation; not proof of deletion"});
        let result = metadata_and_security(&input(payload), &context());
        // Absence is not deletion, and a file that was never opened has nothing to key on.
        assert!(result.observations.is_empty());
        assert!(matches!(&result.outcome, AdapterOutcome::NotAnObject { reason } if reason.contains("not proof of deletion")), "{:?}", result.outcome);
    }

    #[test]
    fn a_file_whose_identity_could_not_be_read_does_not_merge_with_others() {
        // The dangerous alternative: propose an object with the keys missing, and every unreadable
        // path would key on the same absent pair and collapse into one object.
        let payload = json!({"path": "C:\\Example\\locked.exe", "exists": true, "identity_status": "unreadable", "identity_error": 5});
        let result = metadata_and_security(&input(payload), &context());
        assert!(result.observations.is_empty());
        assert!(matches!(&result.outcome, AdapterOutcome::NotAnObject { reason } if reason.contains("win32 error 5")), "{:?}", result.outcome);
    }

    #[test]
    fn the_same_material_produces_the_same_objects_twice() {
        let mut payload = found_file();
        payload["dacl_sddl"] = json!(REAL_SDDL);
        payload["streams"] = json!([{"name": ":Zone.Identifier", "size": 26}]);
        let once = metadata_and_security(&input(payload.clone()), &context());
        let twice = metadata_and_security(&input(payload), &context());
        assert_eq!(once.observations.len(), twice.observations.len());
        let keys = |result: &AdapterResult| {
            result
                .observations
                .iter()
                .map(|candidate| {
                    let mut parts: Vec<String> = candidate.identity.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect();
                    parts.sort();
                    format!("{}|{}", candidate.identity.type_id, parts.join(","))
                })
                .collect::<Vec<String>>()
        };
        assert_eq!(keys(&once), keys(&twice));
    }
}
