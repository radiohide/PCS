//! S11 running-instance adapters.
//!
//! Checked against the emitters in `plugins/powershell-adapter/domains/system.ps1` and
//! `domains/scoped.ps1`. The payload keys below are those emitters' own key names.
//!
//! These four sources describe things that exist for a while and then stop existing. Two of them
//! the collector itself warns are incomplete — a process snapshot can miss a short-lived process,
//! and a session snapshot carries no logon or logoff time. Those statements travel with the
//! material; nothing here upgrades them into a complete record, and nothing here concludes that a
//! process ended because a later snapshot does not contain it. That conclusion needs the absence
//! rules, and it is not this adapter's to draw.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.execution.processes", type_id: Some("runtime.process"), build: processes },
    SourceSpec { source: "pcscope.execution.modules", type_id: Some("runtime.module_instance"), build: modules },
    SourceSpec { source: "pcscope.identity.logon-sessions", type_id: Some("runtime.logon_session"), build: logon_sessions },
    SourceSpec { source: "pcscope.identity.interactive-sessions", type_id: Some("runtime.terminal_session"), build: terminal_sessions },
];

/// Running processes, keyed by process id together with the time it started.
///
/// The pair is the point: a process id on its own is reused, so a snapshot taken later can show the
/// same number for a different process. The collector's item id is that same pair, and the type's
/// identity asks for both, which is what keeps a reused id from looking like one long-lived process.
///
/// The collector states its own limit — a snapshot misses short-lived processes — and the command
/// line is protected before it is sent, with `command_line_status` saying whether it was returned
/// at all. Both are recorded rather than smoothed over.
pub(crate) fn processes(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    // Production and legacy contract material use different names for these same fields. An
    // explicitly unavailable/protected production value must not fall through to a legacy copy.
    let selected = |primary: &str, legacy: &str| {
        if input.payload.get(primary).is_some() { primary.to_owned() } else { legacy.to_owned() }
    };
    let process_field = |field_id: &str, key: &str, kind: FieldKind| {
        let mut value=field(&input.payload,field_id,key,kind);
        if input.decoding.masked_fields.iter().any(|name|name==key)
            || value.value.as_ref().is_some_and(|value|crate::semantics::is_mask_sentinel(value)
                || value.get("sensitive").and_then(Value::as_bool)==Some(true)
                || value.get("redacted").and_then(Value::as_bool)==Some(true)) {
            value.status=ValueStatus::Masked;value.value=None;
        } else if !input.decoding.available || input.decoding.unreadable_fields.iter().any(|name|name==key) {
            value.status=ValueStatus::Unreadable;value.value=None;
        }
        value
    };
    let pid=process_field("pid","pid",FieldKind::Identity);
    let created_at=process_field("created_at",&selected("start_time","created_at"),FieldKind::Identity);
    let pid_identity=pid.value.as_ref().filter(|_|pid.status==ValueStatus::Known).and_then(|value| {
        value.as_u64().and_then(|number|u32::try_from(number).ok()).map(|number|number.to_string())
            .or_else(||value.as_str().and_then(|text|text.parse::<u32>().ok()).map(|number|number.to_string()))
    });
    let time_identity=created_at.value.as_ref().filter(|_|created_at.status==ValueStatus::Known)
        // Validate the collector's UTC shape, but retain its full precision in the identity.
        // canonical_time is a millisecond sort representation, not a process creation key.
        .and_then(Value::as_str).and_then(|value|crate::semantics::canonical_time(value).ok().map(|_|value.trim().to_owned()));
    let identity = identity(
        "runtime.process",
        scope,
        &input.evidence.id,
        &[("pid", pid_identity), ("created_at", time_identity)],
    );
    let fields = vec![
        pid,
        process_field("name", "name", FieldKind::Configuration),
        // The catalog's `image` is the program path, which is what the collector calls `path`.
        process_field("image", &selected("path","image"), FieldKind::Configuration),
        process_field("parent_pid", "parent_pid", FieldKind::Configuration),
        process_field("session", &selected("session_id","session"), FieldKind::Configuration),
        created_at,
        process_field("command_line", "command_line", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// Loaded modules of the processes the scope selected, keyed by owning process and module path.
///
/// `process_identity` is the collector's own identifier for the process — the process id and its
/// start time. It is deliberately *not* rewritten into the form `runtime.process` uses for its
/// native key: that key is assembled by the identity layer from a scope this adapter would have to
/// reproduce by hand, and reproducing it here would look like a link the model never asked for.
/// Whether modules should join to processes on that key is an open question, recorded in the
/// step's acceptance note rather than settled by inventing a convention.
fn modules(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let process_identity = match (text(&input.payload, "pid"), text(&input.payload, "process_start")) {
        (Some(pid), Some(started)) => Some(format!("{pid}/{started}")),
        _ => None,
    };
    let identity = identity(
        "runtime.module_instance",
        scope,
        &input.evidence.id,
        &[("process_identity", process_identity), ("module_path", text(&input.payload, "module_path"))],
    );
    let fields = vec![
        field(&input.payload, "process", "pid", FieldKind::Identity),
        field(&input.payload, "process_start", "process_start", FieldKind::Identity),
        field(&input.payload, "process_name", "process_name", FieldKind::Configuration),
        field(&input.payload, "session", "session_id", FieldKind::Configuration),
        field(&input.payload, "user_sid", "user_sid", FieldKind::Configuration),
        field(&input.payload, "module_path", "module_path", FieldKind::Identity),
        field(&input.payload, "module_base", "module_base", FieldKind::State),
        field(&input.payload, "size", "size", FieldKind::State),
        field(&input.payload, "version", "version", FieldKind::State),
        field(&input.payload, "signature_status", "signature_status", FieldKind::State),
        field(&input.payload, "module_origin", "module_origin", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// Logon sessions, keyed by the logon id the operating system assigned.
///
/// A logon id is an identifier the system states, not a composite this adapter assembles, so the
/// type's `native_identifier` strategy applies directly. The session's start time is a
/// configuration fact here; the end of the session is not in this material at all — the collector
/// says logon and logoff times come from event evidence — so nothing here reports one.
fn logon_sessions(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let logon_id = text(&input.payload, "LogonId").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("runtime.logon_session", scope, &input.evidence.id, &[("native_id", Some(logon_id.clone()))]);
    let fields = vec![
        field(&input.payload, "logon_id", "LogonId", FieldKind::Identity),
        field(&input.payload, "logon_type", "LogonType", FieldKind::Configuration),
        field(&input.payload, "start_time", "StartTime", FieldKind::Configuration),
        field(&input.payload, "authentication_package", "AuthenticationPackage", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// Interactive terminal sessions, keyed by the session id the station assigned.
///
/// The collector carries its own query result: when it could not resolve the session's user it
/// sends `identity_query_error` rather than leaving the fields blank, so a session with no user is
/// shown as one whose user could not be read. Nothing here turns that into "no user".
fn terminal_sessions(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let session_id = text(&input.payload, "session_id").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("runtime.terminal_session", scope, &input.evidence.id, &[("native_id", Some(session_id))]);
    let mut fields = vec![
        field(&input.payload, "session_id", "session_id", FieldKind::Identity),
        field(&input.payload, "station", "station", FieldKind::Configuration),
    ];
    // The collector says outright when it could not resolve these, so a session whose user could
    // not be read is not the same material as one that never mentioned a user.
    if text(&input.payload, "identity_query_error").is_some() {
        fields.push(unreadable("user", FieldKind::Configuration));
        fields.push(unreadable("domain", FieldKind::Configuration));
    } else {
        fields.push(field(&input.payload, "user", "user", FieldKind::Configuration));
        fields.push(field(&input.payload, "domain", "domain", FieldKind::Configuration));
    }
    fields.push(field(&input.payload, "state", "state_code", FieldKind::State));
    object_candidate(input, context, identity, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::EvidenceKind;
    use serde_json::json;

    fn input(source: &str, item_id: &str, payload: Value) -> EvidenceInput {
        EvidenceInput {
            source: source.into(),
            item_id: item_id.into(),
            structure_version: "v1".into(),
            source_scope: "machine:M/user:S-1-5-18/schema:1/context:/shard:0".into(),
            evidence: EvidenceRef { kind: EvidenceKind::Snapshot, id: format!("{source}|scope|run-1") },
            native_record_id: item_id.into(),
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

    fn key_of(candidate: &IdentityCandidate) -> String {
        let mut parts: Vec<String> = candidate.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect();
        parts.sort();
        parts.join("|")
    }

    fn field_of<'a>(result: &'a AdapterResult, field_id: &str) -> &'a FieldCandidate {
        result.observations[0].fields.iter().find(|field| field.field_id == field_id).expect("field is mapped")
    }

    fn process_payload(pid: i64, started: &str) -> Value {
        json!({"pid": pid, "parent_pid": 4, "name": "example.exe", "path": "C:\\Example\\example.exe", "start_time": started, "session_id": 1, "command_line": null, "command_line_collected": false, "command_line_status": "not_returned_system_process_access_or_exit", "basis": "snapshot; short-lived processes may be missed"})
    }

    #[test]
    fn the_production_process_entry_uses_collector_names_and_keeps_pid_generations_separate() {
        let adapter=crate::reference::adapter_for("pcscope.execution.processes").unwrap();
        let first=adapter.adapt(&input("pcscope.execution.processes","100/first",process_payload(100,"2026-09-12T00:00:00Z")),&context());
        let second=adapter.adapt(&input("pcscope.execution.processes","100/second",process_payload(100,"2026-09-12T01:00:00Z")),&context());
        assert!(first.observations[0].identity.missing.is_empty());
        assert_ne!(key_of(&first.observations[0].identity),key_of(&second.observations[0].identity));
        assert_eq!(field_of(&first,"image").value,Some(json!("C:\\Example\\example.exe")));
        assert_eq!(field_of(&first,"name").value,Some(json!("example.exe")));
        assert_eq!(field_of(&first,"session").value,Some(json!(1)));
        assert_eq!(field_of(&first,"created_at").value,Some(json!("2026-09-12T00:00:00Z")));
        let legacy=adapter.adapt(&input("pcscope.execution.processes","100/legacy",json!({
            "pid":"100","created_at":"2026-09-12T00:00:00Z","image":"C:\\Example\\example.exe","session":1
        })),&context());
        assert_eq!(key_of(&first.observations[0].identity),key_of(&legacy.observations[0].identity),"legacy aliases describe the same process");
        assert_eq!(field_of(&legacy,"image").value,field_of(&first,"image").value);
        assert_eq!(field_of(&legacy,"session").value,field_of(&first,"session").value);
        let precise=adapter.adapt(&input("pcscope.execution.processes","100/precise",process_payload(100,"2026-09-12T00:00:00.1234567Z")),&context());
        assert!(precise.observations[0].identity.fields.iter().any(|field|field.key=="created_at"&&field.value=="2026-09-12T00:00:00.1234567Z"));
    }

    #[test]
    fn protected_or_unavailable_process_values_do_not_fall_through_to_legacy_aliases() {
        let adapter=crate::reference::adapter_for("pcscope.execution.processes").unwrap();
        for unavailable in [Value::Null,json!(crate::sensitive::MASK),json!({"sensitive":true,"ciphertext":"protected"}),json!("not a time")] {
            let result=adapter.adapt(&input("pcscope.execution.processes","100",json!({
                "pid":100,"start_time":unavailable,"created_at":"2026-09-12T00:00:00Z",
                "path":null,"image":"must not escape.exe","session_id":null,"session":1
            })),&context());
            assert!(result.observations[0].identity.missing.iter().any(|key|key=="created_at"));
            assert_eq!(field_of(&result,"image").status,ValueStatus::NotProvided);
            assert!(field_of(&result,"image").value.is_none());
            assert_eq!(field_of(&result,"session").status,ValueStatus::NotProvided);
        }
        let mut material=input("pcscope.execution.processes","100",process_payload(100,"2026-09-12T00:00:00Z"));
        material.decoding.masked_fields=vec!["start_time".into(),"path".into()];
        let result=adapter.adapt(&material,&context());
        assert!(result.observations[0].identity.missing.iter().any(|key|key=="created_at"));
        assert_eq!(field_of(&result,"created_at").status,ValueStatus::Masked);
        assert_eq!(field_of(&result,"image").status,ValueStatus::Masked);
    }

    #[test]
    fn a_reused_process_id_is_not_one_long_lived_process() {
        let source = "pcscope.execution.processes";
        let context = context();
        let first = processes(&input(source, "100/2026-09-12T00:00:00Z", process_payload(100, "2026-09-12T00:00:00Z")), &context);
        let second = processes(&input(source, "100/2026-09-12T01:00:00Z", process_payload(100, "2026-09-12T01:00:00Z")), &context);
        // The same number at a different start time is a different process, which is why the type's
        // identity asks for both.
        assert_ne!(key_of(&first.observations[0].identity), key_of(&second.observations[0].identity));
        assert!(first.observations[0].identity.missing.is_empty());
    }

    #[test]
    fn a_process_carries_the_program_path_as_its_image() {
        let result = processes(&input("pcscope.execution.processes", "100/2026-09-12T00:00:00Z", process_payload(100, "2026-09-12T00:00:00Z")), &context());
        assert_eq!(field_of(&result, "image").value, Some(json!("C:\\Example\\example.exe")));
        assert_eq!(field_of(&result, "created_at").value, Some(json!("2026-09-12T00:00:00Z")));
    }

    #[test]
    fn a_command_line_the_collector_could_not_read_is_absent_not_empty() {
        let result = processes(&input("pcscope.execution.processes", "100/2026-09-12T00:00:00Z", process_payload(100, "2026-09-12T00:00:00Z")), &context());
        let command_line = field_of(&result, "command_line");
        assert_eq!(command_line.status, ValueStatus::NotProvided);
        assert!(command_line.value.is_none(), "a refused command line is not an empty one");
    }

    #[test]
    fn a_module_names_the_process_it_was_loaded_into() {
        let payload = json!({"pid": 100, "process_start": "2026-09-12T00:00:00Z", "process_name": "example.exe", "module_path": "C:\\Windows\\System32\\example.dll", "size": 4096, "version": "1.0.0.0"});
        let result = modules(&input("pcscope.execution.modules", "100/2026-09-12T00:00:00Z", payload), &context());
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "process_identity" && field.value == "100/2026-09-12T00:00:00Z"));
        assert!(identity.fields.iter().any(|field| field.key == "module_path"));
        assert_eq!(field_of(&result, "size").field_kind, FieldKind::State);
    }

    #[test]
    fn a_module_without_its_process_timestamp_has_an_incomplete_identity() {
        let payload = json!({"pid": 100, "module_path": "C:\\Example\\example.dll"});
        let result = modules(&input("pcscope.execution.modules", "100", payload), &context());
        assert_eq!(result.observations[0].identity.missing, vec!["process_identity".to_owned()]);
    }

    #[test]
    fn a_logon_session_is_keyed_by_the_identifier_the_system_assigned() {
        let payload = json!({"LogonId": "99999", "LogonType": 2, "StartTime": "2026-09-12T00:00:00Z", "AuthenticationPackage": "Negotiate", "Caption": "EXAMPLE\\user"});
        let result = logon_sessions(&input("pcscope.identity.logon-sessions", "99999", payload), &context());
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "native_id" && field.value == "99999"));
        assert_eq!(field_of(&result, "logon_type").value, Some(json!(2)));
    }

    #[test]
    fn a_session_whose_user_could_not_be_read_says_so() {
        let unreadable_payload = json!({"session_id": 2, "station": "Console", "state_code": 0, "user": null, "domain": null, "identity_query_error": "access denied", "basis": "WTS snapshot"});
        let result = terminal_sessions(&input("pcscope.identity.interactive-sessions", "2", unreadable_payload), &context());
        // Denied and absent are different facts about the same field.
        assert_eq!(field_of(&result, "user").status, ValueStatus::Unreadable);
        assert_eq!(field_of(&result, "domain").status, ValueStatus::Unreadable);

        let resolved = json!({"session_id": 2, "station": "Console", "state_code": 0, "user": "example", "domain": "EXAMPLE"});
        let result = terminal_sessions(&input("pcscope.identity.interactive-sessions", "2", resolved), &context());
        assert_eq!(field_of(&result, "user").status, ValueStatus::Known);
        assert_eq!(field_of(&result, "user").value, Some(json!("example")));
    }

    #[test]
    fn the_same_material_produces_the_same_identity_twice() {
        let context = context();
        let payload = process_payload(100, "2026-09-12T00:00:00Z");
        let once = key_of(&processes(&input("pcscope.execution.processes", "100/2026-09-12T00:00:00Z", payload.clone()), &context).observations[0].identity);
        let twice = key_of(&processes(&input("pcscope.execution.processes", "100/2026-09-12T00:00:00Z", payload), &context).observations[0].identity);
        assert_eq!(once, twice);
    }
}
