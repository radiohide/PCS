//! Sources completed after the N01 baseline. Non-object readings retain their own query path.
use super::*;
use crate::adapter::{ActionCandidate, ActionOutcome, ActionTarget, TimePrecision};
use serde_json::json;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.execution.module-scan-status", type_id: None, build: source_reading },
    SourceSpec { source: "pcscope.execution.module-file-state", type_id: None, build: module_file_state },
    SourceSpec { source: "pcscope.apps.dotnet-runtimes", type_id: Some("software.runtime"), build: runtimes },
    SourceSpec { source: "pcscope.system.update-history", type_id: None, build: update_history },
    SourceSpec { source: "pcscope.hardware.driver-packages", type_id: Some("hardware.driver_package"), build: driver_packages },
    SourceSpec { source: "pcscope.security.key-metadata", type_id: Some("security.key_metadata"), build: key_metadata },
    SourceSpec { source: "pcscope.collector.scoped-configuration", type_id: None, build: source_reading },
    SourceSpec { source: "pcscope.collector.event-channel-coverage", type_id: None, build: source_reading },
    SourceSpec { source: "pcscope.usn.watch-list", type_id: None, build: source_reading },
    SourceSpec { source: "pcscope.execution.resources", type_id: None, build: source_reading },
];

pub fn reading_destination(source: &str) -> Option<&'static str> {
    match source {
        "pcscope.execution.resources" => Some("sampling"),
        "pcscope.execution.module-file-state" | "pcscope.execution.module-scan-status" | "pcscope.collector.scoped-configuration" | "pcscope.collector.event-channel-coverage" | "pcscope.usn.watch-list" => Some("coverage"),
        _ => None,
    }
}

fn source_reading(input: &EvidenceInput, _: &AdapterContext) -> AdapterResult {
    if !input.payload.is_object() {
        return AdapterResult::failed("source reading must carry a structured payload", false);
    }
    AdapterResult::not_an_object(format!("{} reading; retained and queryable by source_readings", reading_destination(&input.source).unwrap_or("source")))
}
fn module_file_state(input:&EvidenceInput,context:&AdapterContext)->AdapterResult {
    let check=&input.payload["check"];
    if check["status"]!="stable"||check["exists"]!=true {return source_reading(input,context);}
    // Reuse the canonical volume/file identity and ACL adapters. Raw module links/checks remain
    // available through source_readings; a missing path never invents deletion of that file ID.
    let mut nested=input.clone();nested.payload=check.clone();
    let mut result=super::file::metadata_and_security(&nested,context);
    // One stable primary file per check. Owner/DACL stay on that file and in the raw record;
    // do not bind several ACL identities to the same source item.
    result.observations.retain(|o|o.identity.type_id=="file.watched_file");
    result
}

fn runtimes(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let Some(path) = text(&input.payload, "path") else { return AdapterResult::failed("runtime registration has no path", false) };
    let parts: Vec<&str> = path.split('\\').collect();
    let mut versions: Vec<(String, String)> = Vec::new();
    let lower = path.to_ascii_lowercase();
    if lower.contains("\\dotnet\\setup\\installedversions\\") {
        if let Some(index) = parts.iter().position(|p| p.eq_ignore_ascii_case("InstalledVersions")) {
            if parts.len() == index + 4 && (parts[index + 2].eq_ignore_ascii_case("sharedfx") || parts[index + 2].eq_ignore_ascii_case("sdk")) {
                let family = format!("{}/{}/{}", parts[index + 1], parts[index + 2], parts[index + 3]);
                if let Some(values) = input.payload["values"].as_object() {
                    for (version, entry) in values {
                        if version.chars().next().is_some_and(|c| c.is_ascii_digit()) && entry["value"].as_i64() == Some(1) {
                            versions.push((family.clone(), version.clone()));
                        }
                    }
                }
            } else if parts.len() == index + 3 && parts[index + 2].eq_ignore_ascii_case("sdk") {
                if let Some(values) = input.payload["values"].as_object() {
                    for (version, entry) in values {
                        if version.chars().next().is_some_and(|c| c.is_ascii_digit()) && entry["value"].as_i64() == Some(1) {
                            versions.push((format!("{}/sdk", parts[index + 1]), version.clone()));
                        }
                    }
                }
            } else if parts.len() == index + 3 && (parts[index + 2].eq_ignore_ascii_case("sharedhost") || parts[index + 2].eq_ignore_ascii_case("hostfxr")) {
                if let Some(version)=registry_value(&input.payload,"Version").and_then(|v|v.as_str().map(str::to_owned)) {
                    versions.push((format!("{}/{}",parts[index+1],parts[index+2]),version));
                }
            }
        }
    } else if let Some(index) = parts.iter().position(|p| p.eq_ignore_ascii_case("NDP")) {
        if registry_value(&input.payload, "Install").or_else(||registry_value(&input.payload,"InstallSuccess")).and_then(|v| v.as_i64()) == Some(1) {
            if let Some(version) = registry_value(&input.payload, "Version").and_then(|v| v.as_str().map(str::to_owned)) {
                versions.push((format!(".NET Framework/{}", parts[index + 1..].join("/")), version));
            }
        }
    }
    let view = text(&input.payload, "view").unwrap_or_default();
    if view.is_empty() { return AdapterResult::failed("runtime registry view is missing", false) }
    let scope = format!("{}/view:{view}", installation_scope(&context.collected_scope));
    let candidates = versions.into_iter().map(|(family, version)| {
        let fields = vec![
            field(&json!({"family":family}), "family", "family", FieldKind::Identity),
            field(&json!({"version":version}), "version", "version", FieldKind::Identity),
            registry_field(&input.payload, "install_path", if lower.contains("\\ndp\\") { "InstallPath" } else { "InstallLocation" }, FieldKind::Configuration),
        ];
        ObservationCandidate {
            identity: identity("software.runtime", &scope, &input.evidence.id, &[("runtime_family", Some(family)), ("runtime_version", Some(version))]),
            fields, observed_at: input.observed_at.clone(), evidence: vec![input.evidence.clone()], complete: context.complete,
        }
    }).collect();
    observations(candidates)
}

fn update_history(input: &EvidenceInput, _: &AdapterContext) -> AdapterResult {
    // A retained operation is not evidence that an update component is currently installed.
    let occurred_at = text(&input.payload, "date").and_then(|date| crate::semantics::canonical_time(&date).ok().map(|_| date));
    let result = match input.payload["result"].as_i64() { Some(1) => ActionOutcome::Started, Some(2) => ActionOutcome::Succeeded, Some(3) => ActionOutcome::Observed, Some(4) => ActionOutcome::Failed, Some(5) => ActionOutcome::Ended, _ => ActionOutcome::Unknown };
    let phase = match input.payload["operation"].as_i64() { Some(1) => "installation", Some(2) => "uninstallation", _ => "unknown" };
    let mut adapted = AdapterResult::adapted();
    adapted.actions.push(ActionCandidate {
        occurrence_key: Some(format!("{}|{}|{}", input.source, crate::semantics::business_scope(&input.source_scope), input.item_id)),
        source_record: Some(input.payload.clone()),
        target: ActionTarget::Unassigned { reason: "保留的更新操作记录不证明组件当前仍安装；原始证据保留更新 ID、修订号、标题与错误码".into() },
        action_id: "software.update_history".into(), phase: Some(phase.into()), result,
        precision: if occurred_at.is_some() { TimePrecision::Exact } else { TimePrecision::ObservationOnly }, occurred_at,
        interval_from: None, interval_until: None, observed_at: input.observed_at.clone(), field_id: None,
        before: None, after: None, evidence: vec![input.evidence.clone()], actor: None, program: None,
    });
    adapted
}

fn driver_packages(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let native = text(&input.payload, "native_package_id").or_else(||text(&input.payload, "Driver"));
    if native.as_deref().is_none_or(str::is_empty) { return AdapterResult::failed("driver package has no published INF identity", false) }
    object_candidate(input, context, identity("hardware.driver_package", &installation_scope(&context.collected_scope), &input.evidence.id, &[("native_id", native)]), vec![
        field(&input.payload, "name", "Driver", FieldKind::Identity), field(&input.payload, "provider", "ProviderName", FieldKind::Configuration),
        field(&input.payload, "version", "Version", FieldKind::Configuration), field(&input.payload, "inf", "OriginalFileName", FieldKind::Configuration),
        field(&input.payload, "signed", "DriverSignature", FieldKind::Configuration),
    ])
}

fn key_metadata(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if input.payload.get("providers").is_some() { return AdapterResult::not_an_object("registered provider list; enumeration capability evidence") }
    let (Some(provider), Some(_name), Some(location)) = (text(&input.payload, "provider"), text(&input.payload, "name"), text(&input.payload, "location")) else { return AdapterResult::failed("key metadata lacks provider, name or user/machine location", false) };
    let base = match location.as_str() { "machine" => installation_scope(&context.collected_scope), "user" => context.collected_scope.clone(), _ => return AdapterResult::failed("unknown key location", false) };
    let scope = format!("{base}/key-provider:{provider}/key-location:{location}");
    object_candidate(input, context, identity("security.key_metadata", &scope, &input.evidence.id, &[("native_id", Some(input.item_id.clone()))]), vec![
        field(&input.payload, "provider", "provider", FieldKind::Identity), field(&input.payload, "name", "name", FieldKind::Identity),
        field(&input.payload, "algorithm", "algorithm", FieldKind::Configuration), field(&input.payload, "legacy_key_spec", "legacy_key_spec", FieldKind::Configuration),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> AdapterContext { AdapterContext { catalog_version:crate::model::catalog_version().into(),rule_version:crate::model::catalog_fingerprint(),machine_id:"M".into(),installation_id:"I".into(),boot_id:None,collected_scope:"machine:M/user:U".into(),complete:true } }
    fn input(source:&str,payload:Value)->EvidenceInput { EvidenceInput {source:source.into(),item_id:"record-1".into(),structure_version:"v1".into(),source_scope:"machine:M/user:U".into(),evidence:EvidenceRef{kind:crate::adapter::EvidenceKind::Snapshot,id:"evidence-1".into()},native_record_id:"record-1".into(),source_time:None,observed_at:"2026-10-12T00:00:00Z".into(),ingested_at:"2026-10-12T00:00:01Z".into(),decoding:crate::adapter::Decoding{available:true,..Default::default()},payload} }
    #[test]
    fn runtime_value_names_expand_versions_and_views_do_not_merge() {
        let payload=json!({"path":"SOFTWARE\\dotnet\\Setup\\InstalledVersions\\x64\\sharedfx\\Microsoft.NETCore.App","view":"Registry64","values":{"8.0.1":{"value":1},"9.0.2":{"value":1},"10.0.0":{"value":0},"InstallLocation":{"value":"C:\\dotnet"}}});
        let a=runtimes(&input("pcscope.apps.dotnet-runtimes",payload.clone()),&context());
        assert_eq!(a.observations.len(),2);assert!(crate::adapter::validate(&a,&context()).is_empty());
        assert_ne!(a.observations[0].identity,a.observations[1].identity);
        let mut p=payload;p["view"]=json!("Registry32");
        let b=runtimes(&input("pcscope.apps.dotnet-runtimes",p),&context());
        assert_ne!(a.observations[0].identity.scope,b.observations[0].identity.scope);
    }
    #[test]
    fn framework_install_flag_and_family_are_preserved() {
        let mut payload=json!({"path":"SOFTWARE\\Microsoft\\NET Framework Setup\\NDP\\v4\\Full","view":"Registry64","values":{"Version":{"value":"4.8.1"},"Install":{"value":1}}});
        let a=runtimes(&input("pcscope.apps.dotnet-runtimes",payload.clone()),&context());
        assert_eq!(a.observations.len(),1);
        payload["values"]["Install"]["value"]=json!(0);
        assert!(runtimes(&input("pcscope.apps.dotnet-runtimes",payload),&context()).observations.is_empty());
    }
    #[test]
    fn different_inbox_store_packages_do_not_collapse_on_published_inf() {
        let a=driver_packages(&input("pcscope.hardware.driver-packages",json!({"Driver":"inbox.inf","native_package_id":"inbox.inf/store-a"})),&context());
        let b=driver_packages(&input("pcscope.hardware.driver-packages",json!({"Driver":"inbox.inf","native_package_id":"inbox.inf/store-b"})),&context());
        assert_eq!(a.observations.len(),1);assert_ne!(a.observations[0].identity,b.observations[0].identity);assert!(crate::adapter::validate(&a,&context()).is_empty());
    }
    #[test]
    fn shared_host_and_legacy_framework_success_shape_are_supported() {
        for path in ["SOFTWARE\\dotnet\\Setup\\InstalledVersions\\x64\\hostfxr","SOFTWARE\\Microsoft\\NET Framework Setup\\NDP\\v3.0\\Setup"] {
            let result=runtimes(&input("pcscope.apps.dotnet-runtimes",json!({"path":path,"view":"Registry32","values":{"Version":{"value":"3.0"},"InstallSuccess":{"value":1}}})),&context());
            assert_eq!(result.observations.len(),1);assert!(crate::adapter::validate(&result,&context()).is_empty());
        }
    }
    #[test]
    fn updates_keep_failure_partial_and_uninstall_without_installed_objects() {
        for (code,outcome) in [(1,ActionOutcome::Started),(2,ActionOutcome::Succeeded),(3,ActionOutcome::Observed),(4,ActionOutcome::Failed),(5,ActionOutcome::Ended),(0,ActionOutcome::Unknown)] {
            let result=update_history(&input("pcscope.system.update-history",json!({"date":"2026-10-11T00:00:00Z","operation":2,"result":code,"hresult":-1,"update_id":"U","title":"Example"})),&context());
            assert!(result.observations.is_empty());assert_eq!(result.actions[0].result,outcome);assert_eq!(result.actions[0].phase.as_deref(),Some("uninstallation"));assert!(crate::adapter::validate(&result,&context()).is_empty());
        }
    }
    #[test]
    fn key_provider_and_location_are_identity_components() {
        let payload=json!({"provider":"Provider A","name":"same name","location":"user","algorithm":"RSA"});
        let a=key_metadata(&input("pcscope.security.key-metadata",payload.clone()),&context());
        assert!(crate::adapter::validate(&a,&context()).is_empty());
        for (key,value) in [("provider","Provider B"),("location","machine")] { let mut other=payload.clone();other[key]=json!(value);let b=key_metadata(&input("pcscope.security.key-metadata",other),&context());assert_ne!(a.observations[0].identity.scope,b.observations[0].identity.scope); }
        let missing=key_metadata(&input("pcscope.security.key-metadata",json!({"name":"key"})),&context());
        assert!(matches!(missing.outcome,crate::adapter::AdapterOutcome::Failed{..}));
    }
    #[test]
    fn non_object_readings_have_declared_paths_and_no_fake_objects() {
        for source in ["pcscope.execution.resources","pcscope.usn.watch-list","pcscope.collector.scoped-configuration","pcscope.collector.event-channel-coverage"] {
            let result=source_reading(&input(source,json!({"active":false,"unknown_nested":{"a":[1,null,"中文"]}})),&context());
            assert!(result.observations.is_empty());assert!(crate::adapter::validate(&result,&context()).is_empty());
            assert!(reading_destination(source).is_some());
        }
    }
}
