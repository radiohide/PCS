//! S10 software and component adapters.
//!
//! Checked against the emitters: `plugins/software/src/main.rs` for the machine registration
//! inventory, and `plugins/powershell-adapter/domains/system.ps1`, `domains/applications.ps1` and
//! `collect-system.ps1` for the rest. The payload keys below are those emitters' own key names.
//!
//! **No type in this category declares an action.** Software appearing, changing or going away is
//! expressed by the observation it is, not by an action: an uninstall registration disappearing is
//! `object.no_longer_observed`, and a version changing is a field change. Nothing here invents an
//! action the catalog does not have.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.software", type_id: Some("software.installation_registration"), build: machine_registrations },
    SourceSpec { source: "pcscope.software.user-inventory", type_id: Some("software.installation_registration"), build: user_registrations },
    SourceSpec { source: "pcscope.system.hotfixes", type_id: Some("software.system_update"), build: hotfixes },
    SourceSpec { source: "pcscope.software.appx-user", type_id: Some("software.appx_package"), build: appx_packages },
    SourceSpec { source: "pcscope.software.provisioned", type_id: Some("software.provisioned_package"), build: provisioned_packages },
    SourceSpec { source: "pcscope.software.optional-features", type_id: Some("software.optional_feature"), build: optional_features },
    SourceSpec { source: "pcscope.apps.npm-global-packages", type_id: Some("software.language_package"), build: npm_packages },
    SourceSpec { source: "pcscope.software.portable", type_id: Some("software.portable_registration"), build: portable_files },
];

/// The identity every uninstall registration shares: which hive, which registry view, which key.
///
/// The two emitters name these differently — the software plugin writes `registry_path` and
/// `registry_view`, the PowerShell collector writes `path` and `view` — because one predates the
/// other. Both mean the same three things, and the type's identity asks for all three, so a
/// registration found by either collector resolves to the same object.
fn registration_identity(input: &EvidenceInput, scope: &str, hive: &str, view: String, path: String) -> IdentityCandidate {
    identity(
        "software.installation_registration",
        scope,
        &input.evidence.id,
        &[("hive", Some(hive.to_owned())), ("view", Some(view)), ("registry_path", Some(path))],
    )
}

/// The hive is not a payload key in either emitter; it is the prefix of the registry path they both
/// write. Reading it from there is reading what the source said, not inferring it.
fn hive_of(path: &str) -> Option<String> {
    let prefix = path.split('\\').next()?;
    Some(match prefix {
        "HKLM" => "LocalMachine".to_owned(),
        "HKCU" => "CurrentUser".to_owned(),
        "HKCR" => "ClassesRoot".to_owned(),
        "HKU" => "Users".to_owned(),
        "HKCC" => "CurrentConfig".to_owned(),
        other => other.to_owned(),
    })
}

fn registration_fields(payload: &Value) -> Vec<FieldCandidate> {
    vec![
        field(payload, "path", if payload.get("registry_path").is_some() {"registry_path"} else {"path"}, FieldKind::Configuration),
        registry_field(payload, "display_name", "DisplayName", FieldKind::Configuration),
        registry_field(payload, "display_version", "DisplayVersion", FieldKind::Configuration),
        registry_field(payload, "publisher", "Publisher", FieldKind::Configuration),
        registry_field(payload, "install_location", "InstallLocation", FieldKind::Configuration),
        registry_field(payload, "uninstall_command", "UninstallString", FieldKind::Configuration),
    ]
}

/// The machine's uninstall registrations, from the software plugin.
///
/// An entry with no `DisplayName` is still an object: the plugin keeps unnamed registrations on
/// purpose, because absence of a display name is not absence of the software.
fn machine_registrations(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let Some(path) = text(&input.payload, "registry_path") else {
        return AdapterResult::failed("registration item carries no registry_path; its identity cannot be formed", false);
    };
    let Some(hive) = hive_of(&path) else {
        return AdapterResult::failed("registration path has no hive prefix", false);
    };
    let view = text(&input.payload, "registry_view").unwrap_or_default();
    let identity = registration_identity(input, scope, &hive, view, path);
    // A registration missing its view is a registration whose identity is incomplete, and the
    // identity layer will say so rather than this adapter guessing one of the two views.
    object_candidate(input, context, identity, registration_fields(&input.payload))
}

/// The same registrations as seen under the current user's hive.
fn user_registrations(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let path = text(&input.payload, "path").or_else(|| text(&input.payload, "registry_path"));
    let Some(path) = path else {
        return AdapterResult::failed("registration item carries no registry path", false);
    };
    let hive = text(&input.payload, "hive").or_else(|| hive_of(&path)).unwrap_or_default();
    let view = text(&input.payload, "view").or_else(|| text(&input.payload, "registry_view")).unwrap_or_default();
    let identity = registration_identity(input, scope, &hive, view, path);
    object_candidate(input, context, identity, registration_fields(&input.payload))
}

/// `Win32_QuickFixEngineering`, keyed by hotfix id and service-pack level.
///
/// The collector states its own coverage: QFE only, not the complete Windows Update inventory. That
/// statement travels with the material and nothing here contradicts it.
fn hotfixes(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let identity = identity("software.system_update", scope, &input.evidence.id, &[("native_id", Some(input.item_id.clone()))]);
    let fields = vec![
        field(&input.payload, "hotfix_id", "hotfix_id", FieldKind::Configuration),
        field(&input.payload, "installed_on", "installed_on", FieldKind::State),
        field(&input.payload, "description", "description", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Get-AppxPackage`, keyed by package full name.
fn appx_packages(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let name = text(&input.payload, "Name").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("software.appx_package", scope, &input.evidence.id, &[("name", Some(name))]);
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Configuration),
        field(&input.payload, "version", "Version", FieldKind::Configuration),
        field(&input.payload, "publisher", "Publisher", FieldKind::Configuration),
        field(&input.payload, "install_location", "InstallLocation", FieldKind::Configuration),
        field(&input.payload, "status", "Status", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Get-AppxProvisionedPackage`, keyed by package name.
fn provisioned_packages(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let name = text(&input.payload, "DisplayName").or_else(|| text(&input.payload, "PackageName")).unwrap_or_else(|| input.item_id.clone());
    let identity = identity("software.provisioned_package", scope, &input.evidence.id, &[("name", Some(name))]);
    let fields = vec![
        field(&input.payload, "name", "DisplayName", FieldKind::Configuration),
        field(&input.payload, "version", "Version", FieldKind::Configuration),
        // This collector reports no publisher at all, so the field is absent rather than empty.
        field(&input.payload, "publisher", "Publisher", FieldKind::Configuration),
        field(&input.payload, "install_location", "InstallLocation", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Get-WindowsOptionalFeature`, keyed by feature name.
fn optional_features(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let name = text(&input.payload, "name").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("software.optional_feature", scope, &input.evidence.id, &[("name", Some(name))]);
    let fields = vec![
        field(&input.payload, "name", "name", FieldKind::Configuration),
        field(&input.payload, "state", "state", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// Globally installed npm packages, keyed by manager, name and version.
///
/// The manager name is the adapter's own knowledge of which source this is — the material says
/// `name` and `version` and nothing about npm, because npm is what the source is. The collector
/// also states its own limit: standard global directories only, custom prefixes are not scanned.
fn npm_packages(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let name = text(&input.payload, "name");
    let version = text(&input.payload, "version");
    let identity = identity(
        "software.language_package",
        scope,
        &input.evidence.id,
        &[("package_manager", Some("npm".into())), ("package_name", name), ("package_version", version)],
    );
    let fields = vec![
        known("manager", FieldKind::Configuration, Value::String("npm".into())),
        field(&input.payload, "name", "name", FieldKind::Configuration),
        field(&input.payload, "version", "version", FieldKind::Configuration),
        field(&input.payload, "path", "path", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// Portable executables found under the roots the user configured, keyed like any other file.
///
/// The collector scans explicit roots and nothing else, so an empty watch list means no objects at
/// all — not an empty inventory of installed software. The item is the scanned file, and its
/// identity is the same volume-and-file-id pair the file category uses, which is what makes one
/// executable appearing under two roots one object rather than two.
///
/// The collector states its own limits in the material: the version it reports is the file's
/// version resource and its signature is not verified. Neither is upgraded here.
fn portable_files(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let (Some(volume), Some(file_id)) = (text(&input.payload, "volume_identity"), text(&input.payload, "file_id")) else {
        // A file that could not be opened has no identity, and keying it on the absent pair would
        // merge every unreadable file into one object.
        let error = text(&input.payload, "identity_error").unwrap_or_default();
        return AdapterResult::not_an_object(format!("the file identity could not be read (win32 error {error}); a file that was never opened has nothing to key on"));
    };
    let identity = identity("software.portable_registration", scope, &input.evidence.id, &[("volume_identity", Some(volume)), ("file_id", Some(file_id))]);
    let fields = vec![
        field(&input.payload, "path", "path", FieldKind::Identity),
        field(&input.payload, "version", "version", FieldKind::Configuration),
        field(&input.payload, "company", "company", FieldKind::Configuration),
        field(&input.payload, "size", "size", FieldKind::State),
        field(&input.payload, "sha256", "sha256", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
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

    /// The software plugin's item: path and view at the top level, values in a map.
    fn plugin_registration() -> Value {
        json!({
            "registry_path": "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Example",
            "registry_view": "Registry64",
            "DisplayName": "示例软件",
            "values": {"DisplayName": {"kind": "String", "value": "示例软件"}, "DisplayVersion": {"kind": "String", "value": "1.2.3"}},
            "collection": {"policy": "software-metadata-v2"}
        })
    }

    /// The PowerShell collector's item: hive and view as keys, path under `path`.
    fn powershell_registration() -> Value {
        json!({
            "path": "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Example",
            "hive": "CurrentUser",
            "view": "Registry32",
            "values": {"DisplayName": {"kind": "String", "value": "示例软件"}},
            "basis": "configuration snapshot"
        })
    }

    #[test]
    fn two_collectors_describing_one_registration_agree_on_its_identity() {
        let context = context();
        let plugin = machine_registrations(&input("pcscope.software", "example", plugin_registration()), &context);
        // The plugin says HKLM in the path prefix; the type's identity wants the hive named.
        let identity = &plugin.observations[0].identity;
        assert_eq!(identity.missing, Vec::<String>::new(), "every declared identity key is supplied");
        assert!(identity.fields.iter().any(|field| field.key == "hive" && field.value == "LocalMachine"));
        assert!(identity.fields.iter().any(|field| field.key == "view" && field.value == "Registry64"));

        let powershell = user_registrations(&input("pcscope.software.user-inventory", "example", powershell_registration()), &context);
        assert_eq!(powershell.observations[0].identity.missing, Vec::<String>::new());
        assert!(powershell.observations[0].identity.fields.iter().any(|field| field.key == "hive" && field.value == "CurrentUser"));
    }

    #[test]
    fn a_registration_without_a_view_has_an_incomplete_identity_rather_than_a_guessed_one() {
        let mut payload = plugin_registration();
        payload.as_object_mut().unwrap().remove("registry_view");
        let result = machine_registrations(&input("pcscope.software", "example", payload), &context());
        // Neither view is picked for it: the identity says which key is missing and the identity
        // layer decides what that means.
        assert_eq!(result.observations[0].identity.missing, vec!["view".to_owned()]);
    }

    #[test]
    fn registration_fields_are_read_from_the_value_map() {
        let result = machine_registrations(&input("pcscope.software", "example", plugin_registration()), &context());
        assert_eq!(field_of(&result, "display_name").value, Some(json!("示例软件")));
        assert_eq!(field_of(&result, "display_version").value, Some(json!("1.2.3")));
        // The source has no publisher for this registration, and none is invented.
        assert_eq!(field_of(&result, "publisher").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_masked_registry_value_stays_masked() {
        let mut payload = plugin_registration();
        payload["values"]["InstallLocation"] = json!({"kind": "String", "value": crate::sensitive::MASK});
        let result = machine_registrations(&input("pcscope.software", "example", payload), &context());
        let field = field_of(&result, "install_location");
        assert_eq!(field.status, ValueStatus::Masked, "a display mask is not a value to compare");
    }

    #[test]
    fn an_appx_package_is_keyed_by_its_name_and_keeps_what_the_collector_read() {
        let payload = json!({"Name": "Example.App", "Version": "1.0.0", "Publisher": "CN=Example", "InstallLocation": "C:\\Program Files\\WindowsApps\\Example", "Status": "Ok"});
        let result = appx_packages(&input("pcscope.software.appx-user", "Example.App_1.0.0_x64__abc", payload), &context());
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "name" && field.value == "Example.App"));
        assert_eq!(field_of(&result, "status").value, Some(json!("Ok")));
    }

    #[test]
    fn a_provisioned_package_reports_the_publisher_it_does_not_have_as_absent() {
        let payload = json!({"DisplayName": "Example.App", "Version": "1.0.0", "PackageName": "Example.App_1.0.0.0_x64__abc", "InstallLocation": "C:\\Program Files\\WindowsApps\\Example"});
        let result = provisioned_packages(&input("pcscope.software.provisioned", "Example.App_1.0.0.0_x64__abc", payload), &context());
        assert_eq!(field_of(&result, "publisher").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&result, "install_location").status, ValueStatus::Known);
    }

    #[test]
    fn a_hotfix_is_keyed_by_the_collector_and_carries_its_own_coverage_note() {
        let payload = json!({"hotfix_id": "KB123456", "description": "Example update", "installed_on": "2026-01-02", "coverage": "QFE only; not the complete Windows Update inventory"});
        let result = hotfixes(&input("pcscope.system.hotfixes", "KB123456/", payload), &context());
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "native_id" && field.value == "KB123456/"));
        assert_eq!(field_of(&result, "hotfix_id").value, Some(json!("KB123456")));
    }

    #[test]
    fn an_npm_package_carries_the_manager_the_source_is() {
        let payload = json!({"name": "@example/tool", "version": "2.0.0", "path": "C:\\Users\\u\\AppData\\Roaming\\npm\\node_modules\\@example\\tool", "basis": "standard global npm directory"});
        let result = npm_packages(&input("pcscope.apps.npm-global-packages", "tool", payload), &context());
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "package_manager" && field.value == "npm"));
        assert!(identity.fields.iter().any(|field| field.key == "package_name" && field.value == "@example/tool"));
        assert_eq!(field_of(&result, "manager").value, Some(json!("npm")));
    }

    #[test]
    fn an_optional_feature_carries_its_state_as_a_state_field() {
        let payload = json!({"name": "Microsoft-Windows-Example", "state": "Enabled"});
        let result = optional_features(&input("pcscope.software.optional-features", "Microsoft-Windows-Example", payload), &context());
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "name" && field.value == "Microsoft-Windows-Example"));
        assert_eq!(field_of(&result, "state").field_kind, FieldKind::State);
    }

    #[test]
    fn the_same_material_produces_the_same_candidates_twice() {
        let context = context();
        let once = key_of(&machine_registrations(&input("pcscope.software", "example", plugin_registration()), &context).observations[0].identity);
        let twice = key_of(&machine_registrations(&input("pcscope.software", "example", plugin_registration()), &context).observations[0].identity);
        assert_eq!(once, twice, "replaying the same material must decide the same thing");
    }

    #[test]
    fn a_portable_executable_is_keyed_like_any_other_file() {
        let payload = json!({
            "path": "C:\\Tools\\example.exe", "directory": false, "size": 2048,
            "version": "1.2.3.4", "company": "Example", "signature_status": "not_verified",
            "volume_identity": "volume:64770b15", "file_id": "file:000500000017455b", "identity_status": "observed"
        });
        let result = portable_files(&input("pcscope.software.portable", "c:\\tools\\example.exe", payload), &context());
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "volume_identity" && field.value == "volume:64770b15"));
        // The path is a field: one executable found under two configured roots is one object.
        assert!(identity.fields.iter().all(|field| field.key != "path"));
        assert_eq!(field_of(&result, "version").value, Some(json!("1.2.3.4")));
    }

    #[test]
    fn a_portable_file_that_could_not_be_opened_proposes_nothing() {
        let payload = json!({"path": "C:\\Tools\\locked.exe", "identity_status": "unreadable", "identity_error": 5});
        let result = portable_files(&input("pcscope.software.portable", "c:\\tools\\locked.exe", payload), &context());
        assert!(result.observations.is_empty());
        assert!(matches!(&result.outcome, crate::adapter::AdapterOutcome::NotAnObject { reason } if reason.contains("win32 error 5")), "{:?}", result.outcome);
    }
}
