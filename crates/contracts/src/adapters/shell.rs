//! S13 shell extension and association adapters.
//!
//! Checked against `collect-system.ps1` (`Read-RegistryTree` and the shell walk) and
//! `domains/applications.ps1` (the two browser extension sources).
//!
//! One source here carries seven object types. `pcscope.shell.registrations` walks a registry tree
//! and emits one item per key; which type a key is depends on where in the tree it sits, so the
//! classification below is by the key's own path shape. The collector already has a path-pattern
//! map, but it maps paths to *collection switches*, not to object types — a key under
//! `\Classes\<progid>\shell` belongs to the same switch as one under `\Classes\<progid>`, while the
//! model calls them a context-menu registration and a file association. Where the two disagree the
//! model decides, and the difference is stated at the rule that makes it.
//!
//! The item carries no hive and no view: those are in the message scope, because the collector
//! walks each hive and view and says which by the scope it emits under. Reading them from there is
//! reading what the collector said, and the scope shape is `…/schema:1/<hive>/<view>/shard:<n>`.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.shell.registrations", type_id: None, build: registrations },
    SourceSpec { source: "pcscope.apps.chromium-extensions", type_id: Some("shell.browser_extension"), build: chromium_extensions },
    SourceSpec { source: "pcscope.apps.firefox-extensions", type_id: Some("shell.browser_extension"), build: firefox_extensions },
];

const HIVES: [&str; 5] = ["LocalMachine", "CurrentUser", "ClassesRoot", "Users", "CurrentConfig"];
const VIEWS: [&str; 2] = ["Registry64", "Registry32"];


/// The hive and the view a shell item was read under.
///
/// Both are in the message scope rather than the item, and both are named segments there, so they
/// are found by name rather than by position. A scope that names neither leaves them absent — the
/// identity then says which key is missing instead of this adapter picking a hive.
fn hive_and_view(scope: &str) -> (Option<String>, Option<String>) {
    let mut hive = None;
    let mut view = None;
    for segment in scope.split('/') {
        let segment = segment.trim();
        if HIVES.iter().any(|known| known.eq_ignore_ascii_case(segment)) {
            hive = Some(segment.to_owned());
        } else if VIEWS.iter().any(|known| known.eq_ignore_ascii_case(segment)) {
            view = Some(segment.to_owned());
        }
    }
    (hive, view)
}

/// The scope a shell registration is keyed in.
///
/// A machine hive belongs to the installation: which account ran the collection is how the key was
/// read, not what it registers. A `CurrentUser` key belongs to the user whose hive it is, so it
/// keeps the scope it was handed — the same rule the startup and policy categories arrived at
/// (see `docs/refactor/adapter-source-survey.md` §3.5).
fn registration_scope(collected: &str, hive: Option<&str>) -> String {
    match hive {
        Some(hive) if hive.eq_ignore_ascii_case("CurrentUser") => collected.trim().to_owned(),
        _ => installation_scope(collected),
    }
}

/// The last segment of a registry path: the key's own name.
fn key_name(path: &str) -> Option<String> {
    path.trim_end_matches('\\').rsplit('\\').next().filter(|name| !name.is_empty()).map(str::to_owned)
}

/// The segment a path puts directly below a named parent.
///
/// A component's class id is the segment after `CLSID`, not the last one: the walk descends past it
/// into `InprocServer32`, and the last segment there names the server key rather than the component.
fn segment_after(path: &str, parent: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('\\').collect();
    segments
        .iter()
        .position(|segment| segment.eq_ignore_ascii_case(parent))
        .and_then(|index| segments.get(index + 1))
        .map(|segment| (*segment).to_owned())
}

/// A GUID the shell uses to name a component.
fn is_clsid(text: &str) -> bool {
    let text = text.trim();
    text.len() == 38 && text.starts_with('{') && text.ends_with('}') && text[1..37].chars().all(|c| c == '-' || c.is_ascii_hexdigit())
}

/// The registry key's default value, which is where these keys conventionally keep the name of the
/// thing they register.
fn default_value(payload: &Value) -> Option<Value> {
    payload.get("values").and_then(|values| values.get("")).and_then(|entry| entry.get("value")).filter(|value| !value.is_null()).cloned()
}

fn shell_name(payload: &Value, path: &str) -> Option<String> {
    default_text(payload).or_else(|| key_name(path))
}

/// The registry key's default value as text, when it is one.
fn default_text(payload: &Value) -> Option<String> {
    default_value(payload).and_then(|value| value.as_str().map(str::to_owned))
}

/// Which type a registry key in the shell tree is.
///
/// The order is the collector's own for the cases it names — the CLSID check comes first there too
/// — and the two cases the model needs and the switch map does not distinguish are added after it:
/// a key below `\shell` or `\shellex` is a context-menu registration, and only what is left under
/// `\Classes\` is a file association.
fn shell_type(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.contains("\\classes\\clsid\\") {
        return "shell.com_registration";
    }
    if lower.contains("\\previewhandlers") {
        return "shell.preview_handler";
    }
    if lower.contains("\\shelliconoverlayidentifiers") {
        return "shell.icon_overlay_handler";
    }
    if lower.contains("\\shell extensions\\") {
        return "shell.shell_extension";
    }
    if lower.contains("\\desktop\\namespace") || lower.contains("\\mycomputer\\namespace") {
        return "shell.namespace_extension";
    }
    // A verb or a handler below a ProgID's `shell`/`shellex` key is a menu entry; the ProgID itself
    // is the association. The collector's switch map calls both "file-association" because the
    // switch covers the same tree — the model separates them, and the model decides here.
    if lower.contains("\\shell\\") || lower.contains("\\shellex\\") || lower.ends_with("\\shell") || lower.ends_with("\\shellex") || lower.contains("\\contextmenuhandlers") {
        return "shell.context_menu_registration";
    }
    "shell.file_association"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComKeyRole { Description, InprocServer, LocalServer, Other }

/// A CLSID's root default names the class; only its direct server children contain
/// implementation settings. Never infer that role from the value looking like a path.
fn com_key_role(path: &str) -> ComKeyRole {
    let parts: Vec<&str> = path.trim_end_matches('\\').split('\\').collect();
    let Some(at) = parts.windows(2).position(|pair| pair[0].eq_ignore_ascii_case("Classes") && pair[1].eq_ignore_ascii_case("CLSID")) else { return ComKeyRole::Other; };
    if !parts.get(at + 2).is_some_and(|value| is_clsid(value)) { return ComKeyRole::Other; }
    match &parts[at + 3..] {
        [] => ComKeyRole::Description,
        [child] if child.eq_ignore_ascii_case("InprocServer32") => ComKeyRole::InprocServer,
        [child] if child.eq_ignore_ascii_case("LocalServer32") => ComKeyRole::LocalServer,
        _ => ComKeyRole::Other,
    }
}

/// `pcscope.shell.registrations`: one key of the shell tree, classified by its own path.
///
/// The collector's basis travels with the material — a registration is not proof that the extension
/// is loaded, let alone invoked — and nothing here upgrades it. The `implementation_file` block is
/// only present under `InprocServer32`, and its own `status` says whether the file was readable at
/// all, so a version or a hash is recorded as unreadable rather than absent when the collector says
/// it could not read them.
fn registrations(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let Some(path) = text(&input.payload, "registry_path") else {
        return AdapterResult::not_an_object("shell item carries no registry_path; it is not a registration");
    };
    // The hive and the view are in the scope the collector emitted under, not in the item.
    let (hive, view) = hive_and_view(&input.source_scope);
    let scope = registration_scope(&context.collected_scope, hive.as_deref());
    let type_id = shell_type(&path);

    let identity = if type_id == "shell.com_registration" {
        identity(
            "shell.com_registration",
            &scope,
            &input.evidence.id,
            &[("hive", hive.clone()), ("view", view.clone()), ("clsid", segment_after(&path, "CLSID").filter(|name| is_clsid(name)))],
        )
    } else {
        identity(
            type_id,
            &scope,
            &input.evidence.id,
            &[("hive", hive.clone()), ("view", view.clone()), ("registry_path", Some(path.clone()))],
        )
    };

    let com_role = com_key_role(&path);
    // In particular, a root or LocalServer record must not erase the Inproc file facts
    // merely because its own payload has no file metadata block.
    let implementation = (com_role == ComKeyRole::InprocServer).then(|| input.payload.get("implementation_file")).flatten();
    // `status` is the collector's own statement about the file: `observed` means it was read;
    // `not_found` means it is genuinely not there; the rest mean the read did not happen.
    let implementation_unreadable = implementation.and_then(|file| file.get("status")).and_then(Value::as_str).is_some_and(|status| matches!(status, "not_read" | "unavailable" | "changed_during_read"));
    let implementation_field = |field_id: &str, key: &str, kind: FieldKind| match (implementation, implementation_unreadable) {
        (Some(_), true) => unreadable(field_id, kind),
        (Some(file), false) => field(file, field_id, key, kind),
        (None, _) => unreadable(field_id, kind),
    };
    let unsupported = |field_id: &str, kind: FieldKind| unreadable(field_id, kind);

    let fields = match type_id {
        "shell.preview_handler" | "shell.icon_overlay_handler" | "shell.namespace_extension" => vec![
            field(&input.payload, "path", "registry_path", FieldKind::Configuration),
            known_opt("name", FieldKind::Identity, shell_name(&input.payload, &path)),
            known_opt("hive", FieldKind::Identity, hive.clone()),
            known_opt("view", FieldKind::Identity, view.clone()),
            // These keys are named by the component's CLSID, which is the key name itself.
            known_opt("clsid", FieldKind::Configuration, key_name(&path).filter(|name| is_clsid(name))),
        ],
        "shell.shell_extension" => vec![
            known_opt("name", FieldKind::Identity, shell_name(&input.payload, &path)),
            known_opt("hive", FieldKind::Identity, hive.clone()),
            known_opt("view", FieldKind::Identity, view.clone()),
            // Which of the two sibling keys the entry sits under is what states its approval; the
            // value itself is only the CLSID.
            FieldCandidate {
                field_id: "approved".into(),
                field_kind: FieldKind::Configuration,
                value: Some(Value::Bool(path.to_lowercase().contains("\\approved"))),
                value_type: "boolean".into(),
                status: ValueStatus::Known,
                source_time: None,
                derivation: Derivation::SourceProvided,
                conflicts: Vec::new(),
            },
        ],
        "shell.context_menu_registration" => vec![
            field(&input.payload, "path", "registry_path", FieldKind::Identity),
            known_opt("command", FieldKind::Configuration, default_text(&input.payload)),
            registry_field(&input.payload, "default_icon", "Icon", FieldKind::Configuration),
        ],
        "shell.file_association" => vec![
            field(&input.payload, "path", "registry_path", FieldKind::Configuration),
            known_opt("extension", FieldKind::Identity, extension_of(&path)),
            known_opt("prog_id", FieldKind::Configuration, default_text(&input.payload)),
            // This tree registers the association; the program it names is registered elsewhere.
            unsupported("command", FieldKind::Configuration),
        ],
        _ => {
            let mut fields = vec![
                known_opt("clsid", FieldKind::Identity, segment_after(&path, "CLSID").filter(|name| is_clsid(name))),
                known_opt("hive", FieldKind::Identity, hive.clone()),
                known_opt("view", FieldKind::Identity, view.clone()),
            ];
            // These records share one COM identity but own disjoint facts. Absence of
            // another role's field is not evidence that the other setting disappeared.
            match com_role {
                ComKeyRole::Description => fields.push(registry_string(&input.payload, "name", FieldKind::Identity)),
                ComKeyRole::InprocServer => fields.extend([
                    server_path(&input.payload, implementation),
                    implementation_field("file_version", "file_version", FieldKind::Configuration),
                    implementation_field("sha256", "sha256", FieldKind::Configuration),
                ]),
                ComKeyRole::LocalServer => fields.push(registry_string(&input.payload, "local_server_command", FieldKind::Configuration)),
                ComKeyRole::Other => {},
            }
            fields
        },
    };
    object_candidate(input, context, identity, fields)
}

/// The extension a class key describes, when the key itself is an extension.
fn extension_of(path: &str) -> Option<String> {
    key_name(path).filter(|name| name.starts_with('.'))
}

/// A textual COM setting, preserving explicit protection and invalid input instead
/// of stringifying it. Used only after the registry key's exact role was established.
fn registry_string(payload: &Value, field_id: &str, kind: FieldKind) -> FieldCandidate {
    let mut result = registry_field(payload, field_id, "", kind);
    if result.value.as_ref().is_some_and(protected_com_value) {
        result.status = ValueStatus::Masked;
        result.value = Some(Value::String(crate::sensitive::MASK.into()));
        result.value_type = "string".into();
    } else if result.status == ValueStatus::Known && !result.value.as_ref().is_some_and(Value::is_string) {
        result.status = ValueStatus::Unreadable;
        result.value = None;
        result.value_type = "unknown".into();
    }
    result
}

fn protected_com_value(value: &Value) -> bool {
    crate::semantics::is_mask_sentinel(value) || value.get("redacted").and_then(Value::as_bool)==Some(true)
        || value.get("sensitive").and_then(Value::as_bool)==Some(true)
        || value.get("display").and_then(Value::as_str)==Some(crate::sensitive::MASK)
}

/// An InprocServer32 implementation, never the CLSID root description or the
/// LocalServer32 launch command. Explicitly protected input cannot be bypassed by
/// its expanded file-metadata counterpart.
fn server_path(payload: &Value, implementation: Option<&Value>) -> FieldCandidate {
    let registered = registry_string(payload, "server_path", FieldKind::Configuration);
    if matches!(registered.status, ValueStatus::Masked | ValueStatus::Unreadable) { return registered; }
    match implementation.and_then(|file| file.get("path")).filter(|value| !value.is_null()) {
        Some(value) if protected_com_value(value) => {
            let mut result = field(implementation.unwrap(), "server_path", "path", FieldKind::Configuration);
            result.status = ValueStatus::Masked;
            result.value = Some(Value::String(crate::sensitive::MASK.into()));
            result.value_type = "string".into();
            result
        },
        Some(value) if value.is_string() => field(implementation.unwrap(), "server_path", "path", FieldKind::Configuration),
        Some(_) => unreadable("server_path", FieldKind::Configuration),
        None => registered,
    }
}

/// A field whose value the adapter already holds, or an explicit absence when it holds none.
fn known_opt(field_id: &str, kind: FieldKind, value: Option<String>) -> FieldCandidate {
    match value {
        Some(value) => FieldCandidate {
            field_id: field_id.into(),
            field_kind: kind,
            value_type: "string".into(),
            value: Some(Value::String(value)),
            status: ValueStatus::Known,
            source_time: None,
            derivation: Derivation::SourceProvided,
            conflicts: Vec::new(),
        },
        None => FieldCandidate {
            field_id: field_id.into(),
            field_kind: kind,
            value: None,
            value_type: "unknown".into(),
            status: ValueStatus::NotProvided,
            source_time: None,
            derivation: Derivation::SourceProvided,
            conflicts: Vec::new(),
        },
    }
}

/// Chromium-family extensions, keyed by browser, profile, extension and version.
///
/// The collector flattens one object per installed extension per profile, and its basis says the
/// manifest does not establish that the extension is active — a recorded state and a set of disable
/// reasons are separate facts, and `state` carries what the material supports.
fn chromium_extensions(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let identity = identity(
        "shell.browser_extension",
        scope,
        &input.evidence.id,
        &[
            ("browser", text(&input.payload, "browser")),
            ("profile", text(&input.payload, "profile")),
            ("extension_id", text(&input.payload, "extension_id")),
            ("version", text(&input.payload, "version")),
        ],
    );
    let fields = vec![
        field(&input.payload, "browser", "browser", FieldKind::Identity),
        field(&input.payload, "profile", "profile", FieldKind::Identity),
        field(&input.payload, "extension_id", "extension_id", FieldKind::Identity),
        field(&input.payload, "version", "version", FieldKind::Identity),
        field(&input.payload, "name", "name", FieldKind::Configuration),
        field(&input.payload, "permissions", "permissions", FieldKind::Configuration),
        field(&input.payload, "state", "recorded_state", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// Firefox extensions, keyed the same way.
///
/// This collector's material names neither the browser nor the extension id: it emits `id`, and
/// that the source is Firefox is what this adapter knows about its own source — the same move as
/// `software.rs` stating `npm`.
fn firefox_extensions(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let identity = identity(
        "shell.browser_extension",
        scope,
        &input.evidence.id,
        &[
            ("browser", Some("firefox".into())),
            ("profile", text(&input.payload, "profile")),
            ("extension_id", text(&input.payload, "id")),
            ("version", text(&input.payload, "version")),
        ],
    );
    let fields = vec![
        known_opt("browser", FieldKind::Identity, Some("firefox".into())),
        field(&input.payload, "profile", "profile", FieldKind::Identity),
        field(&input.payload, "extension_id", "id", FieldKind::Identity),
        field(&input.payload, "version", "version", FieldKind::Identity),
        // The collector's own reading of whether Firefox has it enabled; `active` is a state, and
        // the manifest having been found is not the same statement.
        field(&input.payload, "state", "active", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::EvidenceKind;
    use serde_json::json;

    /// The scope a shell source emits under: the hive and the view are segments of it, because the
    /// item itself carries neither.
    const SCOPE: &str = "machine:M/user:S-1-5-18/schema:1/LocalMachine/Registry64/shard:3";

    fn input(item_id: &str, payload: Value) -> EvidenceInput {
        EvidenceInput {
            source: "pcscope.shell.registrations".into(),
            item_id: item_id.into(),
            structure_version: "v1".into(),
            source_scope: SCOPE.into(),
            evidence: EvidenceRef { kind: EvidenceKind::Snapshot, id: "pcscope.shell.registrations|scope|run-1".into() },
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
            collected_scope: crate::semantics::business_scope(SCOPE),
            complete: true,
        }
    }

    fn registration(path: &str, values: Value) -> Value {
        json!({"registry_path": path, "values": values, "basis": "registration only; not proof of loading or invocation"})
    }

    fn key_of(candidate: &IdentityCandidate) -> String {
        let mut parts: Vec<String> = candidate.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect();
        parts.sort();
        parts.join("|")
    }

    fn classify(path: &str) -> AdapterResult {
        registrations(&input(&path.to_lowercase(), registration(path, json!({}))), &context())
    }

    #[test]
    fn a_key_is_classified_by_where_it_sits_in_the_tree() {
        assert_eq!(classify(r"Software\Microsoft\Windows\CurrentVersion\PreviewHandlers\{11111111-2222-3333-4444-555555555555}").observations[0].identity.type_id, "shell.preview_handler");
        assert_eq!(classify(r"Software\Microsoft\Windows\CurrentVersion\Explorer\ShellIconOverlayIdentifiers\Example").observations[0].identity.type_id, "shell.icon_overlay_handler");
        assert_eq!(classify(r"Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved").observations[0].identity.type_id, "shell.shell_extension");
        assert_eq!(classify(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace\{11111111-2222-3333-4444-555555555555}").observations[0].identity.type_id, "shell.namespace_extension");
        assert_eq!(classify(r"Software\Classes\Example.ProgId").observations[0].identity.type_id, "shell.file_association");
        assert_eq!(classify(r"Software\Classes\Directory\Background\shell\Example").observations[0].identity.type_id, "shell.context_menu_registration");
        assert_eq!(classify(r"Software\Classes\CLSID\{11111111-2222-3333-4444-555555555555}\InprocServer32").observations[0].identity.type_id, "shell.com_registration");
    }

    #[test]
    fn the_hive_and_the_view_come_from_the_scope_the_collector_emitted_under() {
        let result = classify(r"Software\Microsoft\Windows\CurrentVersion\PreviewHandlers\Example");
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "hive" && field.value == "LocalMachine"));
        assert!(identity.fields.iter().any(|field| field.key == "view" && field.value == "Registry64"));
    }

    #[test]
    fn a_machine_level_registration_does_not_depend_on_the_collecting_account() {
        let result = classify(r"Software\Microsoft\Windows\CurrentVersion\PreviewHandlers\Example");
        // The collecting account is how the hive was read, not what the registration belongs to.
        assert!(!result.observations[0].identity.scope.contains("S-1-5-18"), "{}", result.observations[0].identity.scope);
    }

    #[test]
    fn a_component_is_keyed_by_its_class_id() {
        let path = r"Software\Classes\CLSID\{11111111-2222-3333-4444-555555555555}\InprocServer32";
        let result = classify(path);
        let identity = &result.observations[0].identity;
        assert!(identity.fields.iter().any(|field| field.key == "clsid" && field.value == "{11111111-2222-3333-4444-555555555555}"));
        assert!(identity.fields.iter().all(|field| field.key != "registry_path"), "this type's keys are hive, view and clsid");
    }

    #[test]
    fn a_component_the_collector_could_not_read_says_so() {
        let path = r"Software\Classes\CLSID\{11111111-2222-3333-4444-555555555555}\InprocServer32";
        let payload = json!({
            "registry_path": path,
            "values": {"": {"kind": "String", "value": "C:\\Example\\example.dll"}},
            "implementation_file": {"status": "unavailable", "error_type": "UnauthorizedAccessException"},
            "basis": "registration only; not proof of loading or invocation"
        });
        let result = registrations(&input(&path.to_lowercase(), payload), &context());
        let file_version = result.observations[0].fields.iter().find(|field| field.field_id == "file_version").expect("mapped");
        // The collector states it could not read the file; that is not the same as the file having
        // no version.
        assert_eq!(file_version.status, ValueStatus::Unreadable);
    }

    #[test]
    fn a_component_file_that_was_read_carries_its_version_and_hash() {
        let path = r"Software\Classes\CLSID\{11111111-2222-3333-4444-555555555555}\InprocServer32";
        let payload = json!({
            "registry_path": path,
            "values": {},
            "implementation_file": {"status": "observed", "path": "C:\\Example\\example.dll", "file_version": "1.2.3.4", "sha256": "abc123"},
            "basis": "registration only; not proof of loading or invocation"
        });
        let result = registrations(&input(&path.to_lowercase(), payload), &context());
        let field_of = |id: &str| result.observations[0].fields.iter().find(|field| field.field_id == id).expect("mapped").clone();
        assert_eq!(field_of("server_path").value, Some(json!("C:\\Example\\example.dll")));
        assert_eq!(field_of("file_version").value, Some(json!("1.2.3.4")));
        assert_eq!(field_of("sha256").value, Some(json!("abc123")));
    }

    #[test]
    fn com_root_description_and_inproc_file_are_disjoint_even_when_the_description_looks_like_a_path() {
        let root = r"Software\Classes\CLSID\{0af96ede-aebf-41ed-a1c8-cf7a685505b6}";
        let inproc = format!(r"{root}\InprocServer32");
        let server = registrations(&input(&inproc.to_lowercase(), json!({
            "registry_path": inproc,
            "values": {"": {"kind": "ExpandString", "value": r"%SystemRoot%\system32\shell32.dll"}},
            "implementation_file": {"status":"observed","path":r"C:\Windows\system32\shell32.dll","file_version":"10.0.19041.5678","sha256":"example-hash"}
        })), &context());
        for description in ["Library Folder Context Menu", r"C:\A name that resembles a path.dll"] {
            let root_item = registrations(&input(&root.to_lowercase(), registration(root, json!({"":{"kind":"String","value":description}}))), &context());
            assert_eq!(key_of(&root_item.observations[0].identity), key_of(&server.observations[0].identity));
            let fields = &root_item.observations[0].fields;
            assert_eq!(fields.iter().find(|field|field.field_id=="name").unwrap().value,Some(json!(description)));
            for id in ["server_path","local_server_command","file_version","sha256"] {
                assert!(!fields.iter().any(|field|field.field_id==id), "a different key role must not clear or overwrite {id}");
            }
            assert!(crate::adapter::validate(&root_item,&context()).is_empty());
        }
        assert!(!server.observations[0].fields.iter().any(|field|field.field_id=="name"));
        assert_eq!(server.observations[0].fields.iter().find(|field|field.field_id=="server_path").unwrap().value,Some(json!(r"C:\Windows\system32\shell32.dll")));
        assert!(crate::adapter::validate(&server,&context()).is_empty());
    }

    #[test]
    fn com_local_server_command_cannot_overwrite_the_inproc_file_or_its_metadata() {
        let path = r"Software\Classes\CLSID\{0af96ede-aebf-41ed-a1c8-cf7a685505b6}\LocalServer32";
        let command = r#""C:\Program Files\Example\server.exe" /Embedding"#;
        let result = registrations(&input(&path.to_lowercase(), json!({
            "registry_path":path,"values":{"":{"kind":"String","value":command}},
            "implementation_file":{"status":"observed","path":"must-not-be-used.dll","file_version":"wrong","sha256":"wrong"}
        })),&context());
        let fields = &result.observations[0].fields;
        assert_eq!(fields.iter().find(|field|field.field_id=="local_server_command").unwrap().value,Some(json!(command)));
        for id in ["name","server_path","file_version","sha256"] {assert!(!fields.iter().any(|field|field.field_id==id));}
        assert!(crate::adapter::validate(&result,&context()).is_empty());
    }

    #[test]
    fn com_repeated_interleaved_key_streams_leave_all_three_settings_stable() {
        let root = r"Software\Classes\CLSID\{0af96ede-aebf-41ed-a1c8-cf7a685505b6}";
        let fixtures = [
            (format!(r"{root}\InprocServer32"),r"C:\Windows\system32\shell32.dll"),
            (root.to_owned(),"Library Folder Context Menu"),
            (format!(r"{root}\LocalServer32"),r#""C:\Example\server.exe" /Embedding"#),
        ];
        let mut current = std::collections::BTreeMap::new();
        let mut unexpected_changes = Vec::new();
        for _ in 0..3 {
            for (path,value) in &fixtures {
                let result=registrations(&input(&path.to_lowercase(),registration(path,json!({"":{"kind":"String","value":value}}))),&context());
                for field in &result.observations[0].fields {
                    if let Some(previous)=current.insert(field.field_id.clone(),(field.status,field.value.clone())) {
                        if previous!=(field.status,field.value.clone()) {unexpected_changes.push(field.field_id.clone());}
                    }
                }
            }
        }
        assert!(unexpected_changes.is_empty(),"repeating unchanged keys manufactured changes: {unexpected_changes:?}");
        assert_eq!(current["name"].1,Some(json!("Library Folder Context Menu")));
        assert_eq!(current["server_path"].1,Some(json!(r"C:\Windows\system32\shell32.dll")));
        assert_eq!(current["local_server_command"].1,Some(json!(fixtures[2].1)));
    }

    #[test]
    fn com_nested_or_unrelated_keys_cannot_supply_implementation_facts() {
        let root = r"Software\Classes\CLSID\{0af96ede-aebf-41ed-a1c8-cf7a685505b6}";
        for suffix in [r"\InprocServer32\Nested",r"\LocalServer32\Nested",r"\TreatAs",r"\DefaultIcon"] {
            let path=format!("{root}{suffix}");
            let result=registrations(&input(&path.to_lowercase(),json!({"registry_path":path,"values":{"":{"value":"misleading value"}},"implementation_file":{"path":"misleading.dll"}})),&context());
            assert!(result.observations[0].fields.iter().all(|field|["clsid","hive","view"].contains(&field.field_id.as_str())));
        }
        assert_eq!(com_key_role(&format!(r"{root}\inprocserver32\")),ComKeyRole::InprocServer);
    }

    #[test]
    fn com_protected_or_invalid_default_cannot_fall_back_to_expanded_file_metadata() {
        let root=r"Software\Classes\CLSID\{0af96ede-aebf-41ed-a1c8-cf7a685505b6}";
        for (suffix,id) in [("","name"),(r"\InprocServer32","server_path"),(r"\LocalServer32","local_server_command")] {
            for value in [json!("******"),json!({"redacted":true})] {
                let path=format!("{root}{suffix}");
                let result=registrations(&input(&path.to_lowercase(),json!({"registry_path":path,"values":{"":{"value":value}},"implementation_file":{"path":"must-not-bypass-mask.dll"}})),&context());
                assert_eq!(result.observations[0].fields.iter().find(|field|field.field_id==id).unwrap().status,ValueStatus::Masked);
            }
            let path=format!("{root}{suffix}");
            let result=registrations(&input(&path.to_lowercase(),json!({"registry_path":path,"values":{"":{"value":42}},"implementation_file":{"path":"must-not-bypass-invalid-value.dll"}})),&context());
            let field=result.observations[0].fields.iter().find(|field|field.field_id==id).unwrap();
            assert_eq!(field.status,ValueStatus::Unreadable);assert!(field.value.is_none());
        }
    }

    #[test]
    fn com_missing_server_value_is_reported_only_by_its_own_server_key() {
        let path=r"Software\Classes\CLSID\{0af96ede-aebf-41ed-a1c8-cf7a685505b6}\InprocServer32";
        let result=registrations(&input(&path.to_lowercase(),registration(path,json!({}))),&context());
        assert_eq!(result.observations[0].fields.iter().find(|field|field.field_id=="server_path").unwrap().status,ValueStatus::NotProvided);
        assert!(!result.observations[0].fields.iter().any(|field|field.field_id=="name"||field.field_id=="local_server_command"));
    }

    #[test]
    fn an_approved_extension_and_a_blocked_one_are_distinguishable() {
        let approved = classify(r"Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved");
        let blocked = classify(r"Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked");
        let of = |result: &AdapterResult| result.observations[0].fields.iter().find(|field| field.field_id == "approved").expect("mapped").value.clone();
        assert_eq!(of(&approved), Some(json!(true)));
        assert_eq!(of(&blocked), Some(json!(false)));
    }

    #[test]
    fn an_association_reports_the_extension_and_never_invents_a_command() {
        let payload = json!({"registry_path": r"Software\Classes\.txt", "values": {"": {"kind": "String", "value": "txtfile"}}, "basis": "registration only"});
        let result = registrations(&input(r"software\classes\.txt", payload), &context());
        let field_of = |id: &str| result.observations[0].fields.iter().find(|field| field.field_id == id).expect("mapped").clone();
        assert_eq!(field_of("extension").value, Some(json!(".txt")));
        assert_eq!(field_of("prog_id").value, Some(json!("txtfile")));
        // This tree registers the association; the program it names is registered elsewhere, so the
        // command is stated as unread rather than filled in.
        assert_eq!(field_of("command").status, ValueStatus::Unreadable);
    }

    #[test]
    fn a_firefox_extension_says_which_browser_it_is() {
        let payload = json!({"profile": "default", "id": "example@addon", "version": "1.0", "type": "extension", "active": true, "user_disabled": false});
        let result = firefox_extensions(
            &EvidenceInput { source: "pcscope.apps.firefox-extensions".into(), ..input("example@addon", payload) },
            &context(),
        );
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        // The material names neither: the source is Firefox, and its `id` is the extension id.
        assert!(identity.fields.iter().any(|field| field.key == "browser" && field.value == "firefox"));
        assert!(identity.fields.iter().any(|field| field.key == "extension_id" && field.value == "example@addon"));
    }

    #[test]
    fn a_chromium_extension_carries_all_four_identity_parts() {
        let payload = json!({"browser": "chrome", "profile": "Default", "extension_id": "abcdef", "version": "2.0", "name": "Example", "recorded_state": 1});
        let result = chromium_extensions(
            &EvidenceInput { source: "pcscope.apps.chromium-extensions".into(), ..input("chrome/Default/abcdef/2.0", payload) },
            &context(),
        );
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity.fields.len(), 5, "scope plus four declared keys");
    }

    #[test]
    fn the_same_key_classifies_and_keys_the_same_way_twice() {
        let path = r"Software\Classes\Example.ProgId\shell\open";
        let once = classify(path);
        let twice = classify(path);
        assert_eq!(once.observations[0].identity.type_id, twice.observations[0].identity.type_id);
        assert_eq!(key_of(&once.observations[0].identity), key_of(&twice.observations[0].identity));
    }
}
