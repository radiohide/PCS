//! S12 autostart-entry adapters.
//!
//! Checked against the collectors' own emit declarations, which is where the payload keys below
//! come from: `plugins/powershell-adapter/collect-system.ps1` for the service inventory, the task
//! inventory and the logon startup entries, and `plugins/powershell-adapter/domains/startup.ps1`
//! for the service configuration, the session hooks, the IFEO entries and the three WMI
//! subscription classes. The shared helpers in `collect-domain.ps1` are what give the registry- and
//! CIM-shaped items their shape, and a key that a helper writes but no type declares stays in the
//! material rather than becoming a field here.
//!
//! Three things this category deliberately does not do:
//!
//!   * **invent a value a collector states it did not obtain.** `pcscope.startup.logon` marks every
//!     startup-folder file with `shortcut_target_collected: false`; that statement is carried as
//!     `unreadable`, not flattened into an absent command that would read as "this entry runs
//!     nothing".
//!   * **read a registration as an execution.** Every source here is a configuration snapshot, and
//!     several say so themselves — `basis: "configuration snapshot"` from `Registry-Key`,
//!     `basis: "registration only; not proof of loading or invocation"` from `Read-RegistryTree`.
//!     Nothing here turns the existence of a registration into evidence that it ever ran.
//!   * **split what the sources say is one object.** `pcscope.startup.services` and
//!     `pcscope.startup.service-configuration` are one service, as are `pcscope.startup.tasks` and
//!     `pcscope.startup.task-details` one task. They converge because the identity is the type's
//!     and the scope is the installation's, not because either collector decided so.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.startup.services", type_id: Some("startup.service"), build: services },
    SourceSpec { source: "pcscope.startup.service-configuration", type_id: Some("startup.service"), build: service_configuration },
    SourceSpec { source: "pcscope.startup.tasks", type_id: Some("startup.scheduled_task"), build: scheduled_task },
    SourceSpec { source: "pcscope.startup.task-details", type_id: Some("startup.scheduled_task"), build: scheduled_task },
    SourceSpec { source: "pcscope.startup.logon", type_id: Some("startup.logon_startup_entry"), build: logon },
    SourceSpec { source: "pcscope.startup.session-hooks", type_id: Some("startup.logon_initialization"), build: session_hooks },
    SourceSpec { source: "pcscope.startup.advanced-extensions", type_id: Some("startup.ifeo_configuration"), build: advanced_extensions },
    SourceSpec { source: "pcscope.startup.wmi-filters", type_id: Some("startup.wmi_filter"), build: wmi_filters },
    SourceSpec { source: "pcscope.startup.wmi-consumers", type_id: Some("startup.wmi_consumer"), build: wmi_consumers },
    SourceSpec { source: "pcscope.startup.wmi-bindings", type_id: Some("startup.wmi_binding"), build: wmi_bindings },
];

/// A field whose value the adapter states because the source it adapts is the thing that makes it
/// true, not because a key happened to be present.
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

/// A field the collector states whether it obtained, keyed by the collector's own flag.
///
/// An explicit `false` is the source saying the value was not obtained — `unreadable`, which is not
/// the same fact as "this item never mentions the field" — and is carried as such. A flag that is
/// absent or true leaves the ordinary mapping in place, so a key that is simply missing stays
/// `NotProvided`.
fn reported_field(payload: &Value, field_id: &str, key: &str, kind: FieldKind, flag: &str) -> FieldCandidate {
    if payload.get(key).is_some_and(|value| !value.is_null()) {
        return field(payload, field_id, key, kind);
    }
    match payload.get(flag).and_then(Value::as_bool) {
        Some(false) => unreadable(field_id, kind),
        _ => field(payload, field_id, key, kind),
    }
}

// ---------- services: the running state and the registry configuration of one object ----------

/// `Get-CimInstance Win32_Service`, keyed by the service name.
///
/// The type's identity is the name, and the collector lower-cases the item id while keeping the
/// source's spelling in `name`. The payload value therefore wins and the item id is only a
/// fallback: the type's normalisation, not the adapter, is what decides that service names compare
/// case-insensitively.
///
/// The item carries both running state (`state` from `Win32_Service`, which is what the CIM class
/// observed) and configuration (`start_mode`, `start_account`). Those are separate fields, so a
/// service that is stopped is not also a service whose start mode is unknown.
fn services(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let name = text(&input.payload, "name").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("startup.service", &scope, &input.evidence.id, &[("name", Some(name.clone()))]);
    let fields = vec![
        known("name", FieldKind::Identity, Value::String(name)),
        field(&input.payload, "display_name", "display_name", FieldKind::Configuration),
        field(&input.payload, "state", "state", FieldKind::State),
        field(&input.payload, "start_mode", "start_mode", FieldKind::Configuration),
        field(&input.payload, "account", "start_account", FieldKind::Configuration),
        // The command line the service runs. `Protect-Text` may have replaced it with its protected
        // form; that is the value the source kept, and it is carried rather than unwrapped here.
        field(&input.payload, "binary_path", "command", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// The key whose subkeys are the services: `SYSTEM\CurrentControlSet\Services`.
const SERVICE_ROOT: [&str; 3] = ["SYSTEM", "CurrentControlSet", "Services"];

/// The service a key below `SYSTEM\CurrentControlSet\Services` belongs to.
///
/// `domains/startup.ps1` walks that key two levels deep, so the material holds the service keys
/// *and* their own subkeys: `<root>\<name>` is the service's key and `<root>\<name>\<subkey>` is
/// part of the same service's configuration tree. The service is named by the segment directly
/// below the root, so both describe the one object the type keys by name. The root itself names no
/// service, and is not one.
///
/// The comparison against the root is case-insensitive because Windows compares registry paths that
/// way; the returned name keeps whatever case the source used, because the type says it does.
fn service_name_of(path: &str) -> Option<String> {
    let mut segments = path.split('\\');
    for expected in SERVICE_ROOT {
        if !segments.next()?.eq_ignore_ascii_case(expected) {
            return None;
        }
    }
    segments.next().map(str::to_owned).filter(|name| !name.trim().is_empty())
}

/// The service configuration read out of the registry by `domains/startup.ps1`.
///
/// This source states configuration, not running state: `Start`, `Type`, `ObjectName` and
/// `DisplayName` are what the service *is configured* to do, and `state` — what it is doing now —
/// belongs to `pcscope.startup.services`. The field is mapped from a key this material does not
/// carry, so it is reported as absent rather than filled from the configuration and presented as
/// current state.
fn service_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let Some(path) = text(&input.payload, "path") else {
        return AdapterResult::failed("service configuration item carries no registry path; its identity cannot be formed", false);
    };
    let Some(name) = service_name_of(&path) else {
        return AdapterResult::not_an_object(format!("{path} is not a service key below {}", SERVICE_ROOT.join("\\")));
    };
    let identity = identity("startup.service", &scope, &input.evidence.id, &[("name", Some(name.clone()))]);
    let fields = vec![
        known("name", FieldKind::Identity, Value::String(name)),
        registry_field(&input.payload, "display_name", "DisplayName", FieldKind::Configuration),
        registry_field(&input.payload, "start_mode", "Start", FieldKind::Configuration),
        registry_field(&input.payload, "account", "ObjectName", FieldKind::Configuration),
        registry_field(&input.payload, "binary_path", "ImagePath", FieldKind::Configuration),
        registry_field(&input.payload, "state", "State", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- scheduled tasks: two item streams, one object ----------

/// Both task sources describe one task, so they share one mapping and one identity: the type keys a
/// task by its full path, and both emitters write `path` and `name` from the same
/// `Get-ScheduledTask` object. The base inventory (`pcscope.startup.tasks`) sends `user_id` and
/// `run_level` where the detail source sends a whole `principal` table, and neither sends everything
/// the type declares — a key a source does not send is `NotProvided`, which is a different fact from
/// a key it sent as empty and must never be enough to clear a value the other source observed.
const TASK_FIELDS: [(&str, &str, FieldKind); 9] = [
    ("path", "path", FieldKind::Identity),
    ("name", "name", FieldKind::Configuration),
    ("enabled", "enabled", FieldKind::Configuration),
    ("principal", "principal", FieldKind::Configuration),
    ("actions", "actions", FieldKind::Configuration),
    ("triggers", "triggers", FieldKind::Configuration),
    ("next_run", "next_run", FieldKind::State),
    ("last_run", "last_run", FieldKind::State),
    ("last_result", "last_result", FieldKind::State),
];

/// `Get-ScheduledTask`, from either the inventory or the detail emitter.
///
/// A task with no complete path cannot be told apart from a same-named task in another folder, and
/// the type's own rule is that those must not merge. So the item fails rather than being registered
/// under the task name alone.
pub(crate) fn scheduled_task(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let path = match scheduled_task_path(input) {
        Ok(path) => path,
        Err(reason) => return AdapterResult::failed(reason, false),
    };
    let identity = identity("startup.scheduled_task", &scope, &input.evidence.id, &[("path", Some(path.clone()))]);
    let fields = TASK_FIELDS
        .into_iter()
        .map(|(field_id, key, kind)| if field_id == "path" {
            known(field_id, kind, Value::String(path.clone()))
        } else { field(&input.payload, field_id, key, kind) })
        .collect();
    object_candidate(input, context, identity, fields)
}

/// The Windows emitters write TaskPath (a folder ending in `\`) and TaskName separately, with
/// their concatenation in item_id. Older fixture/record shapes already put the complete path in
/// payload.path. Resolve both shapes here, so the production/reference and category entry points
/// cannot disagree. A bare name or a folder is never sufficient, and conflicting complete paths
/// are rejected rather than choosing which task the source meant.
fn scheduled_task_path(input: &EvidenceInput) -> Result<String, &'static str> {
    fn component(value: &str) -> bool {
        !value.trim().is_empty() && value != "." && value != ".."
            && value != crate::sensitive::MASK
            && !value.chars().any(|c| c == '\\' || c == '/' || c.is_control())
    }
    fn full_path(value: &str) -> bool {
        value.strip_prefix('\\').is_some_and(|rest| rest.split('\\').all(component))
    }
    fn same(left: &str, right: &str) -> bool { left.to_lowercase() == right.to_lowercase() }
    fn supplied<'a>(payload: &'a Value, key: &str) -> Result<Option<&'a str>, &'static str> {
        match payload.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(value)) if value != crate::sensitive::MASK => {
                Ok((!value.trim().is_empty()).then_some(value.as_str()))
            }
            _ => Err("scheduled-task path/name is protected or not readable; identity cannot be formed"),
        }
    }
    let path = supplied(&input.payload, "path")?;
    let name = supplied(&input.payload, "name")?;
    if name.is_some_and(|name| !component(name)) {
        return Err("scheduled-task item carries an invalid task name");
    }
    let item_path = full_path(&input.item_id).then_some(input.item_id.as_str());
    let resolved = match path {
        Some(folder) if folder.ends_with('\\') => {
            if let Some(name) = name.filter(|name| component(name)) {
                format!("{folder}{name}")
            } else if let Some(item) = item_path.filter(|item| {
                item.rsplit_once('\\').is_some_and(|(parent, _)| same(&format!("{parent}\\"), folder))
            }) {
                item.to_owned()
            } else {
                return Err("scheduled-task item only identifies a folder, not a complete task path");
            }
        }
        Some(path) if full_path(path) => {
            // Accept a non-terminated folder only when the collector's complete item_id confirms
            // the concatenation. Otherwise a non-terminated payload path is the legacy full path.
            let joined = name.filter(|name| component(name)).map(|name| format!("{path}\\{name}"));
            if let Some(joined) = joined.filter(|joined| item_path.is_some_and(|item| same(item, joined))) {
                joined
            } else { path.to_owned() }
        }
        Some(_) => return Err("scheduled-task item carries an invalid task path"),
        None => item_path.ok_or("scheduled-task item carries no complete task path")?.to_owned(),
    };
    if !full_path(&resolved) {
        return Err("scheduled-task item carries an invalid complete task path");
    }
    if item_path.is_some_and(|item| !same(item, &resolved)) {
        return Err("scheduled-task payload and record id identify different task paths");
    }
    Ok(resolved)
}

// ---------- logon startup entries: one source, three item shapes ----------

/// The registrations `pcscope.startup.logon` produces, which its emitter writes in three shapes.
///
/// The shapes are told apart by what the item actually carries, never by assuming one:
///
///   * a `values` table — a `StartupApproved` key from `Read-RegistryTree`, which holds one entry
///     per value in the key and therefore expands;
///   * a flat `registry_path` plus `name` — one `Run`/`RunOnce` value, already one entry per item;
///   * a file `path` with no registry material at all — one file in a startup folder.
///
/// All three become `startup.logon_startup_entry`, because that is the type the mapping names for
/// this source; the shape decides the candidate, not the destination.
fn logon(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if input.payload.get("values").is_some() {
        return logon_approved_entries(input, context);
    }
    if let (Some(registry_path), Some(value_name)) = (text(&input.payload, "registry_path"), text(&input.payload, "name")) {
        return logon_run_entry(input, context, &registry_path, &value_name);
    }
    if let Some(path) = text(&input.payload, "path") {
        return logon_folder_file(input, context, &path);
    }
    AdapterResult::failed("logon item carries neither a registry value, a StartupApproved key nor a startup-folder file", false)
}

/// The hive a logon item came from, out of the payload or the item id.
///
/// The `Run`/`RunOnce` shape states it as a key. The `StartupApproved` shape does not: its payload
/// keeps only the path and the value table, and `collect-system.ps1` wraps it as
/// `Item "$hive/$view/$($item.id)"`, so the hive is read from the source's own record id rather
/// than guessed from the path.
fn logon_hive(input: &EvidenceInput) -> Option<String> {
    text(&input.payload, "hive").or_else(|| item_id_segment(input, is_hive))
}

/// The registry view a logon item came from, with the same two sources as the hive.
fn logon_view(input: &EvidenceInput) -> Option<String> {
    text(&input.payload, "view").or_else(|| item_id_segment(input, is_view))
}

/// Looks for a hive or a view in the item id the collector built.
///
/// Only the two leading segments are considered: the collector writes `Item "$hive/$view/..."` for
/// this shape, and a registry path further along can contain a segment that happens to look like a
/// hive or a view without being the shape's statement of where the item came from.
fn item_id_segment(input: &EvidenceInput, accept: fn(&str) -> bool) -> Option<String> {
    input.item_id.split('/').take(2).find(|segment| accept(segment)).map(str::to_owned)
}

fn is_hive(segment: &str) -> bool {
    ["LocalMachine", "CurrentUser", "ClassesRoot", "Users", "CurrentConfig"].iter().any(|hive| segment.eq_ignore_ascii_case(hive))
}

fn is_view(segment: &str) -> bool {
    ["Registry64", "Registry32"].iter().any(|view| segment.eq_ignore_ascii_case(view))
}

/// The scope a logon startup entry belongs to.
///
/// The type's own scope is "install or user scope + registry view + registration location", and
/// `model/source-map.cjs` states that the user/machine scope and the registry view each build
/// separate instances. The declared identity keys are `scope`, `path` and `value_name`, so the
/// view has nowhere else to live and goes into the scope.
///
/// A `CurrentUser` registration belongs to the user whose hive was read, so it keeps the scope it
/// was handed; a `LocalMachine` one belongs to the installation, and the account the collection ran
/// as is how the data was obtained rather than what the entry belongs to. The startup-folder shape
/// states no hive at all, so it also keeps the scope it was handed: the collector does not say which
/// of the two startup folders it enumerated, and picking one would file a machine-wide entry under
/// whichever user happened to run the collection.
fn logon_scope(collected: &str, hive: Option<&str>, view: Option<&str>, hive_unstated: bool) -> String {
    let base = if hive_unstated || hive.is_some_and(|hive| hive.eq_ignore_ascii_case("CurrentUser")) {
        collected.trim().trim_end_matches('/').to_owned()
    } else {
        installation_scope(collected)
    };
    match view.filter(|view| !view.trim().is_empty()) {
        Some(view) => format!("{base}/view:{view}"),
        None => base,
    }
}

/// A `Run`/`RunOnce` value: the collector has already flattened one entry per value.
fn logon_run_entry(input: &EvidenceInput, context: &AdapterContext, registry_path: &str, value_name: &str) -> AdapterResult {
    let scope = logon_scope(&context.collected_scope, logon_hive(input).as_deref(), logon_view(input).as_deref(), false);
    let identity = identity(
        "startup.logon_startup_entry",
        &scope,
        &input.evidence.id,
        &[("path", Some(registry_path.to_owned())), ("value_name", Some(value_name.to_owned()))],
    );
    let fields = vec![
        known("location", FieldKind::Identity, Value::String(registry_path.to_owned())),
        known("value_name", FieldKind::Identity, Value::String(value_name.to_owned())),
        reported_field(&input.payload, "command", "command", FieldKind::Configuration, "command_collected"),
    ];
    object_candidate(input, context, identity, fields)
}

/// A `StartupApproved` key: one registration per value in the key.
///
/// `Read-RegistryTree` emits the key with its whole value table rather than one item per value, so
/// this is where one item becomes several objects. The type keys an entry by path *and value name*,
/// which is exactly this table's shape.
///
/// The value itself is the approval state (and a timestamp), not a command: what the entry runs is
/// in the `Run` key, and putting the state blob into `command` would be a wrong value rather than a
/// missing one. So the command is left as the material left it — absent — and the key the value
/// lives in stays in the evidence.
fn logon_approved_entries(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let Some(values) = input.payload.get("values").and_then(Value::as_object) else {
        return AdapterResult::failed("StartupApproved item carries no value table; no entry can be attributed", false);
    };
    let Some(registry_path) = text(&input.payload, "registry_path") else {
        return AdapterResult::failed("StartupApproved item carries no registry path; identity cannot be formed", false);
    };
    let scope = logon_scope(&context.collected_scope, logon_hive(input).as_deref(), logon_view(input).as_deref(), false);
    let mut candidates = Vec::new();
    for value_name in values.keys() {
        let identity = identity(
            "startup.logon_startup_entry",
            &scope,
            &input.evidence.id,
            &[("path", Some(registry_path.clone())), ("value_name", Some(value_name.clone()))],
        );
        let fields = vec![
            known("location", FieldKind::Identity, Value::String(registry_path.clone())),
            known("value_name", FieldKind::Identity, Value::String(value_name.clone())),
            field(&input.payload, "command", "command", FieldKind::Configuration),
        ];
        candidates.push(ObservationCandidate {
            identity,
            fields,
            observed_at: input.observed_at.clone(),
            evidence: vec![input.evidence.clone()],
            complete: context.complete,
        });
    }
    observations(candidates)
}

/// A file in a startup folder.
///
/// The item states the file and that the shortcut target was not collected. A file has no registry
/// value name, and the type's identity asks for one: the key is left unstated so the identity is
/// provisional rather than being filled with the file name, which would make the file look like a
/// registration it is not.
fn logon_folder_file(input: &EvidenceInput, context: &AdapterContext, path: &str) -> AdapterResult {
    let scope = logon_scope(&context.collected_scope, None, None, true);
    let identity = identity(
        "startup.logon_startup_entry",
        &scope,
        &input.evidence.id,
        &[("path", Some(path.to_owned())), ("value_name", None)],
    );
    let fields = vec![
        // The shape names no registration location: the collector does not say which of the two
        // startup folders this file came from, so none is claimed.
        field(&input.payload, "location", "location", FieldKind::Identity),
        field(&input.payload, "value_name", "name", FieldKind::Identity),
        reported_field(&input.payload, "command", "command", FieldKind::Configuration, "shortcut_target_collected"),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- registry-backed registrations ----------

/// A registry key registration: the type keys it by hive, view and path, and `Registry-Key` writes
/// all three into the item itself.
///
/// The values table is carried whole. A key the material presents as something other than a table
/// is `unreadable` — the source was looking at the key but what it read is not a configuration
/// table, and reporting "no values" instead would read as an empty key.
fn registry_registration(input: &EvidenceInput, context: &AdapterContext, type_id: &str) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let Some(path) = text(&input.payload, "path") else {
        return AdapterResult::failed("registry item carries no path; its identity cannot be formed", false);
    };
    let identity = identity(
        type_id,
        &scope,
        &input.evidence.id,
        &[("hive", text(&input.payload, "hive")), ("view", text(&input.payload, "view")), ("registry_path", Some(path.clone()))],
    );
    let fields = vec![
        known("path", FieldKind::Identity, Value::String(path)),
        match input.payload.get("values") {
            Some(Value::Object(values)) => known("values", FieldKind::Configuration, Value::Object(values.clone())),
            Some(_) => unreadable("values", FieldKind::Configuration),
            None => field(&input.payload, "values", "values", FieldKind::Configuration),
        },
    ];
    object_candidate(input, context, identity, fields)
}

/// Session and logon initialisation entries: `Shell`, `Userinit`, `BootExecute`, Active Setup and
/// the Explorer `Run` policy key.
fn session_hooks(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    registry_registration(input, context, "startup.logon_initialization")
}

/// The IFEO, SilentProcessExit, AppInit and AppCert keys.
///
/// The source is explicit about what it read: the named switches and value summaries only, and it
/// does not load the extensions. Nothing here reads a registration as a loaded extension.
fn advanced_extensions(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    registry_registration(input, context, "startup.ifeo_configuration")
}

// ---------- WMI permanent subscriptions ----------

/// The namespace the WMI subscription collector queries.
///
/// `domains/startup.ps1` asks `root/subscription` for all three classes. The bindings' identity
/// keys ask for a namespace that their item does not carry, and the consumers' item carries none
/// either, so this is the adapter's knowledge of which source it adapts — the same kind of
/// statement as `software.rs` naming the npm source's manager — rather than a value read out of the
/// material.
const WMI_SUBSCRIPTION_NAMESPACE: &str = "root/subscription";

/// The instance's own WMI path, out of the property tree the emitter saves for CIM instances.
///
/// The named keys of a filter or consumer hold no path, but `Fields`/`Public-CimFields` keep every
/// `CimInstanceProperties` entry under `source_properties`, and `__RELPATH` is the instance's
/// native path. It is deliberately not mapped to any declared field — `source_properties` is
/// material, not a field table — and is read only to give the identity the key its type declares.
/// When it is absent the identity states that the key is missing instead of inventing one.
fn wmi_path(payload: &Value) -> Option<String> {
    let properties = payload.get("source_properties")?;
    ["__RELPATH", "__PATH"].iter().find_map(|key| properties.get(*key).and_then(Value::as_str)).map(str::to_owned)
}

/// `__EventFilter`, keyed by its native path.
///
/// The item states an event namespace, which is what the collector's own key calls it; that is
/// mapped to the type's `namespace` field. The namespace the filter is *registered in* is the one
/// the collector queried, and no field of this type asks for it.
fn wmi_filters(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("startup.wmi_filter", &scope, &input.evidence.id, &[("path", wmi_path(&input.payload))]);
    let fields = vec![
        field(&input.payload, "name", "name", FieldKind::Identity),
        field(&input.payload, "namespace", "event_namespace", FieldKind::Identity),
        field(&input.payload, "query_language", "query_language", FieldKind::Configuration),
        // `Protect-Text` may have replaced the query with its protected form; the value the source
        // kept is carried as it stands.
        field(&input.payload, "query", "query", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `__EventConsumer`, keyed by its native path.
///
/// The item's `class` is the consumer class the object was instantiated from — the type's
/// `consumer_type` — and `executable_path` is the program it names. The command line and script it
/// may also carry are summaries the type declares no field for, so they stay in the material.
fn wmi_consumers(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("startup.wmi_consumer", &scope, &input.evidence.id, &[("path", wmi_path(&input.payload))]);
    let fields = vec![
        field(&input.payload, "name", "name", FieldKind::Identity),
        // The consumer item states no namespace; the one the collector queried is the source's own.
        known("namespace", FieldKind::Identity, Value::String(WMI_SUBSCRIPTION_NAMESPACE.into())),
        field(&input.payload, "consumer_type", "class", FieldKind::Configuration),
        field(&input.payload, "script_engine", "script_engine", FieldKind::Configuration),
        field(&input.payload, "program", "executable_path", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `__FilterToConsumerBinding`, keyed by namespace, filter and consumer together.
///
/// A binding is the registration of one filter against one consumer, so its identity is the pair
/// plus the namespace they live in — not a new object standing for the relationship. The item
/// states the two references and no namespace, and both are carried as the source wrote them.
fn wmi_bindings(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let filter = text(&input.payload, "Filter");
    let consumer = text(&input.payload, "Consumer");
    let identity = identity(
        "startup.wmi_binding",
        &scope,
        &input.evidence.id,
        &[
            ("namespace", Some(WMI_SUBSCRIPTION_NAMESPACE.to_owned())),
            ("filter_name", filter.clone()),
            ("consumer_name", consumer.clone()),
        ],
    );
    let fields = vec![
        known("namespace", FieldKind::Identity, Value::String(WMI_SUBSCRIPTION_NAMESPACE.into())),
        match filter {
            Some(filter) => known("filter", FieldKind::Identity, Value::String(filter)),
            None => field(&input.payload, "filter", "Filter", FieldKind::Identity),
        },
        match consumer {
            Some(consumer) => known("consumer", FieldKind::Identity, Value::String(consumer)),
            None => field(&input.payload, "consumer", "Consumer", FieldKind::Identity),
        },
        field(&input.payload, "security_context", "MaintainSecurityContext", FieldKind::Configuration),
    ];
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

    /// `Get-CimInstance Win32_Service`.
    fn service_item() -> Value {
        json!({
            "name": "ExampleSvc",
            "display_name": "Example Service",
            "state": "Running",
            "start_mode": "Auto",
            "start_account": "LocalSystem",
            "service_type": "Own Process",
            "description": "example",
            "command": "C:\\Example\\svc.exe -k",
            "pid": 42,
            "exit_code": 0,
            "service_specific_exit_code": 0
        })
    }

    #[test]
    fn a_service_is_keyed_by_its_name_and_keeps_its_spelling() {
        let result = services(&input("pcscope.startup.services", "examplesvc", service_item()), &context());
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        // The item id was lower-cased by the collector; the payload's own spelling is the identity
        // value, and the type's normalisation is what compares them case-insensitively.
        assert!(identity.fields.iter().any(|field| field.key == "name" && field.value == "ExampleSvc"));
        assert_eq!(field_of(&result, "name").value, Some(json!("ExampleSvc")));
        assert_eq!(field_of(&result, "state").field_kind, FieldKind::State);
        assert_eq!(field_of(&result, "account").value, Some(json!("LocalSystem")));
        // A machine-level service keeps the installation scope: which account collected it is how
        // the data was obtained, not what the service belongs to.
        assert_eq!(identity.scope, "machine:M");
    }

    /// The service key `Registry-Key` emits, plus the values table it writes.
    fn service_key_item(path: &str, values: Value) -> Value {
        json!({
            "path": path,
            "hive": "LocalMachine",
            "view": "Registry64",
            "values": values,
            "basis": "configuration snapshot",
            "collection": {"policy": "evidence-v2", "scope": "configured_registry_paths_and_depth_only", "remaining_depth": 2, "unvisited_subkeys": 7}
        })
    }

    #[test]
    fn the_running_service_and_its_registry_configuration_are_one_object() {
        let context = context();
        let cim = services(&input("pcscope.startup.services", "examplesvc", service_item()), &context);
        let registry = service_configuration(
            &input("pcscope.startup.service-configuration", "LocalMachine/Registry64/SYSTEM\\CurrentControlSet\\Services\\ExampleSvc", service_key_item(
                "SYSTEM\\CurrentControlSet\\Services\\ExampleSvc",
                json!({"DisplayName": {"kind": "String", "value": "Example Service"}, "Start": {"kind": "DWord", "value": 2}, "ObjectName": {"kind": "String", "value": "LocalSystem"}}),
            )),
            &context,
        );
        // One service, seen twice: same type, same scope, same name — so the identity engine places
        // both observations on one instance instead of two.
        assert_eq!(key_of(&cim.observations[0].identity), key_of(&registry.observations[0].identity));
        assert_eq!(field_of(&registry, "display_name").value, Some(json!("Example Service")));
        assert_eq!(field_of(&registry, "start_mode").value, Some(json!(2)));
    }

    #[test]
    fn a_service_subkey_still_names_its_service_and_the_root_names_none() {
        let context = context();
        let nested = service_configuration(
            &input("pcscope.startup.service-configuration", "LocalMachine/Registry64/SYSTEM\\CurrentControlSet\\Services\\ExampleSvc\\Parameters", service_key_item("SYSTEM\\CurrentControlSet\\Services\\ExampleSvc\\Parameters", json!({}))),
            &context,
        );
        assert_eq!(nested.observations[0].identity.fields.iter().find(|f| f.key == "name").unwrap().value, "ExampleSvc");

        // The emitter also returns the `Services` key itself, which names no service; it is not one.
        let root = service_configuration(
            &input("pcscope.startup.service-configuration", "LocalMachine/Registry64/SYSTEM\\CurrentControlSet\\Services", service_key_item("SYSTEM\\CurrentControlSet\\Services", json!({}))),
            &context,
        );
        assert!(matches!(root.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", root.outcome);
        assert!(root.observations.is_empty());
    }

    #[test]
    fn registry_configuration_does_not_claim_the_running_state() {
        let result = service_configuration(
            &input("pcscope.startup.service-configuration", "LocalMachine/Registry64/SYSTEM\\CurrentControlSet\\Services\\ExampleSvc", service_key_item(
                "SYSTEM\\CurrentControlSet\\Services\\ExampleSvc",
                json!({"Start": {"kind": "DWord", "value": 3}}),
            )),
            &context(),
        );
        // The key configures the service; it does not say what it is doing. `state` waits for the
        // source that observes it rather than being read out of `Start`.
        assert_eq!(field_of(&result, "state").status, ValueStatus::NotProvided);
        assert!(field_of(&result, "state").value.is_none());
        // And a key the source has no value for is absent, not zero.
        assert_eq!(field_of(&result, "binary_path").status, ValueStatus::NotProvided);
    }

    #[test]
    fn both_task_sources_place_one_task_on_one_instance() {
        let context = context();
        let inventory = scheduled_task(
            &input("pcscope.startup.tasks", "\\microsoft\\windows\\example", json!({"name": "Example", "path": "\\Microsoft\\Windows\\", "state": "Ready", "enabled": true, "user_id": "SYSTEM", "run_level": "Highest", "actions": [{"Execute": "C:\\Example\\app.exe"}]})),
            &context,
        );
        let details = scheduled_task(
            &input("pcscope.startup.task-details", "\\Microsoft\\Windows\\Example", json!({"path": "\\Microsoft\\Windows\\", "name": "Example", "last_run": "2026-09-11T00:00:00.0000000Z", "last_result": 0, "next_run": "2026-09-13T00:00:00.0000000Z", "triggers": [{"Enabled": true}], "principal": {"UserId": "SYSTEM"}})),
            &context,
        );
        // Different item streams and differently-cased item ids, one task path: one object.
        assert_eq!(key_of(&inventory.observations[0].identity), key_of(&details.observations[0].identity));
        assert!(details.observations[0].identity.missing.is_empty(), "{:?}", details.observations[0].identity.missing);
        assert_eq!(field_of(&inventory, "path").value, Some(json!("\\Microsoft\\Windows\\Example")));
        assert_eq!(field_of(&details, "path").value, Some(json!("\\Microsoft\\Windows\\Example")));
    }

    #[test]
    fn production_task_dispatch_separates_siblings_and_same_names_in_other_folders() {
        let mut keys = std::collections::BTreeSet::new();
        for (folder, name) in [("\\Folder\\", "A"), ("\\Folder\\", "B"), ("\\Other\\", "A")] {
            let path = format!("{folder}{name}");
            let mut source_keys = Vec::new();
            for source in ["pcscope.startup.tasks", "pcscope.startup.task-details"] {
                let material = input(source, &path.to_lowercase(), json!({"path":folder,"name":name,"last_result":0}));
                let result = crate::reference::adapter_for(source).unwrap().adapt(&material, &context());
                assert!(crate::adapter::validate(&result, &context()).is_empty());
                assert_eq!(result.observations.len(), 1);
                assert_eq!(field_of(&result, "path").value, Some(json!(path)));
                assert_eq!(field_of(&result, "last_result").value, Some(json!(0)));
                source_keys.push(key_of(&result.observations[0].identity));
            }
            assert_eq!(source_keys[0], source_keys[1], "inventory and detail must converge");
            assert!(keys.insert(source_keys.remove(0)), "each full task path is distinct");
        }
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn complete_task_paths_and_reliable_record_ids_remain_compatible() {
        for (item_id, payload, expected) in [
            ("\\Folder\\A", json!({"path":"\\Folder\\A","name":"Display name"}), "\\Folder\\A"),
            ("\\Folder\\A", json!({"path":"\\Folder","name":"A"}), "\\Folder\\A"),
            ("\\Folder\\A", json!({"path":"\\Folder\\"}), "\\Folder\\A"),
            ("\\Folder\\A", json!({"name":"A"}), "\\Folder\\A"),
            ("opaque-record", json!({"path":"\\Folder\\A","name":"A"}), "\\Folder\\A"),
            ("\\A", json!({"path":"\\","name":"A"}), "\\A"),
        ] {
            let result = scheduled_task(&input("pcscope.startup.tasks", item_id, payload), &context());
            assert_eq!(result.observations.len(), 1, "{item_id}: {:?}", result.outcome);
            assert_eq!(field_of(&result, "path").value, Some(json!(expected)));
        }
    }

    #[test]
    fn incomplete_conflicting_or_protected_task_paths_never_become_folder_objects() {
        for (item_id, payload) in [
            ("folder", json!({"path":"\\Folder\\"})),
            ("\\Folder\\", json!({"path":"\\Folder\\"})),
            ("A", json!({"name":"A"})),
            ("\\Other\\A", json!({"path":"\\Folder\\","name":"A"})),
            ("\\Folder\\A", json!({"path":"\\Folder\\B","name":"B"})),
            ("\\Folder\\A", json!({"path":"relative","name":"A"})),
            ("\\Folder\\A", json!({"path":"\\Folder\\","name":".."})),
            ("\\Folder\\A", json!({"path":"\\Folder\\","name":"B\\C"})),
            ("\\Folder\\A", json!({"path":crate::sensitive::MASK,"name":"A"})),
            ("\\Folder\\A", json!({"path":"\\Folder\\","name":crate::sensitive::MASK})),
            ("\\Folder\\A", json!({"path":{"sensitive":true,"ciphertext":"opaque"},"name":"A"})),
            ("\\Folder\\A", json!({"path":"\\Folder\\","name":{"redacted":true}})),
        ] {
            let result = crate::reference::adapter_for("pcscope.startup.tasks").unwrap()
                .adapt(&input("pcscope.startup.tasks", item_id, payload), &context());
            assert!(matches!(result.outcome, AdapterOutcome::Failed { .. }), "{item_id}: {:?}", result.outcome);
            assert!(result.observations.is_empty());
        }
    }

    #[test]
    fn a_task_key_a_source_did_not_send_stays_absent() {
        // The inventory emitter sends no `principal` or `triggers`; those are the detail source's.
        let result = scheduled_task(
            &input("pcscope.startup.tasks", "\\example", json!({"name": "Example", "path": "\\", "enabled": true})),
            &context(),
        );
        assert_eq!(field_of(&result, "principal").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&result, "triggers").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&result, "enabled").value, Some(json!(true)));
        // An absent key is not a cleared one.
        assert!(field_of(&result, "principal").value.is_none());
    }

    #[test]
    fn a_task_without_a_full_path_is_not_registered_under_its_name_alone() {
        // `path` is what tells two same-named tasks in different folders apart.
        let result = scheduled_task(&input("pcscope.startup.tasks", "example", json!({"name": "Example"})), &context());
        assert!(matches!(result.outcome, AdapterOutcome::Failed { .. }), "{:?}", result.outcome);
    }

    /// A `Run` value, flattened by the collector into one item per value.
    fn run_item(name: &str) -> Value {
        json!({
            "hive": "LocalMachine",
            "view": "Registry64",
            "registry_path": "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            "name": name,
            "command": "C:\\Example\\app.exe",
            "command_collected": true
        })
    }

    #[test]
    fn a_run_value_is_one_entry_keyed_by_its_location_and_value_name() {
        let result = logon(&input("pcscope.startup.logon", "LocalMachine/Registry64/Run/Example", run_item("Example")), &context());
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "path" && field.value == "Software\\Microsoft\\Windows\\CurrentVersion\\Run"));
        assert!(identity.fields.iter().any(|field| field.key == "value_name" && field.value == "Example"));
        assert_eq!(field_of(&result, "command").value, Some(json!("C:\\Example\\app.exe")));
        assert_eq!(field_of(&result, "location").value, Some(json!("Software\\Microsoft\\Windows\\CurrentVersion\\Run")));
        // A machine-hive registration belongs to the installation, and the view is part of the scope
        // because the declared identity keys have nowhere else to put it.
        assert_eq!(identity.scope, "machine:M/view:Registry64");
    }

    #[test]
    fn the_registry_view_and_the_hive_each_build_their_own_instance() {
        let context = context();
        let mut current_user = run_item("Example");
        current_user["hive"] = json!("CurrentUser");
        let machine = logon(&input("pcscope.startup.logon", "LocalMachine/Registry64/Run/Example", run_item("Example")), &context);
        let user = logon(&input("pcscope.startup.logon", "CurrentUser/Registry64/Run/Example", current_user.clone()), &context);
        // Same path and value name under two hives: two registrations, and the user one keeps the
        // user whose hive it was read from.
        assert_ne!(key_of(&machine.observations[0].identity), key_of(&user.observations[0].identity));
        assert_eq!(user.observations[0].identity.scope, "machine:M/user:S-1-5-18/view:Registry64");

        let mut view32 = current_user;
        view32["view"] = json!("Registry32");
        let other_view = logon(&input("pcscope.startup.logon", "CurrentUser/Registry32/Run/Example", view32), &context);
        assert_ne!(key_of(&user.observations[0].identity), key_of(&other_view.observations[0].identity));
        assert_eq!(other_view.observations[0].identity.scope, "machine:M/user:S-1-5-18/view:Registry32");
    }

    /// A `StartupApproved` key, as `Read-RegistryTree` emits it: one item holding the whole table.
    fn startup_approved_item() -> Value {
        json!({
            "registry_path": "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run",
            "values": {
                "Example": {"kind": "Binary", "value": {"encoding": "base64", "bytes": 12, "value": "020000000000000000000000"}},
                "Other": {"kind": "Binary", "value": {"encoding": "base64", "bytes": 12, "value": "060000000000000000000000"}}
            },
            "basis": "registration only; not proof of loading or invocation",
            "collection": {"policy": "evidence-v2", "remaining_depth": 1, "unvisited_subkeys": 0}
        })
    }

    #[test]
    fn a_startup_approved_key_expands_into_one_registration_per_value() {
        let result = logon(&input("pcscope.startup.logon", "LocalMachine/Registry64/software\\microsoft\\windows\\currentversion\\explorer\\startupapproved\\run", startup_approved_item()), &context());
        assert_eq!(result.observations.len(), 2, "one entry per value in the key");
        for candidate in &result.observations {
            assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
            assert!(candidate.identity.fields.iter().any(|field| field.key == "value_name"));
            assert!(candidate.identity.fields.iter().any(|field| field.key == "path"));
        }
        // The hive is not a payload key in this shape; it is read from the item id the collector
        // built, so the machine scope still applies.
        assert_eq!(result.observations[0].identity.scope, "machine:M/view:Registry64");
        // The value is the approval state, not a command; filling `command` with that blob would be
        // a wrong value rather than a missing one.
        assert_eq!(field_of(&result, "command").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_startup_folder_file_states_no_value_name_and_no_command() {
        let payload = json!({
            "path": "C:\\Example\\Startup\\tool.lnk",
            "length": 1024,
            "last_write_utc": "2026-09-11T00:00:00Z",
            "sha256": "ab",
            "shortcut_target_collected": false
        });
        let result = logon(&input("pcscope.startup.logon", "c:\\example\\startup\\tool.lnk", payload), &context());
        let identity = &result.observations[0].identity;
        // The type's identity asks for a value name and a file has none: the key is listed as
        // missing so the identity is provisional, rather than being filled with the file name.
        assert_eq!(identity.missing, vec!["value_name".to_owned()]);
        assert!(identity.fields.iter().any(|field| field.key == "path" && field.value == "C:\\Example\\Startup\\tool.lnk"));
        // The collector states it did not collect the shortcut target. That statement is the answer,
        // so the field is unreadable rather than absent-and-therefore-empty.
        assert_eq!(field_of(&result, "command").status, ValueStatus::Unreadable);
        assert!(field_of(&result, "command").value.is_none());
        // The shape names no registration location, and none is invented.
        assert_eq!(field_of(&result, "location").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_session_hook_is_keyed_by_hive_view_and_path() {
        let payload = json!({
            "path": "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon",
            "hive": "LocalMachine",
            "view": "Registry64",
            "values": {"Shell": {"kind": "String", "value": "explorer.exe"}, "Userinit": {"kind": "String", "value": "C:\\Windows\\system32\\userinit.exe,"}},
            "basis": "configuration snapshot"
        });
        let result = session_hooks(&input("pcscope.startup.session-hooks", "LocalMachine/Registry64/SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "startup.logon_initialization");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "hive" && field.value == "LocalMachine"));
        assert!(identity.fields.iter().any(|field| field.key == "view" && field.value == "Registry64"));
        assert!(identity.fields.iter().any(|field| field.key == "registry_path"));
        assert_eq!(field_of(&result, "values").value.as_ref().unwrap()["Shell"]["value"], json!("explorer.exe"));
        assert_eq!(identity.scope, "machine:M");
    }

    #[test]
    fn a_registry_item_whose_values_are_not_a_table_is_unreadable() {
        let payload = json!({"path": "SYSTEM\\CurrentControlSet\\Control\\Session Manager", "hive": "LocalMachine", "view": "Registry64", "values": "not a table"});
        let result = session_hooks(&input("pcscope.startup.session-hooks", "LocalMachine/Registry64/SYSTEM\\CurrentControlSet\\Control\\Session Manager", payload), &context());
        // "the values could not be read as a table" must not read as "this key has no values".
        assert_eq!(field_of(&result, "values").status, ValueStatus::Unreadable);
    }

    #[test]
    fn a_wmi_filter_and_consumer_are_keyed_by_the_path_the_source_saved() {
        let context = context();
        let filter = wmi_filters(
            &input("pcscope.startup.wmi-filters", "ExampleFilter", json!({
                "name": "ExampleFilter",
                "event_namespace": "root\\cimv2",
                "query_language": "WQL",
                "query": "SELECT * FROM __InstanceCreationEvent",
                "source_properties": {"__RELPATH": "__EventFilter.Name=\"ExampleFilter\"", "__NAMESPACE": "root/subscription"}
            })),
            &context,
        );
        let identity = &filter.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "path" && field.value == "__EventFilter.Name=\"ExampleFilter\""));
        assert_eq!(field_of(&filter, "namespace").value, Some(json!("root\\cimv2")));

        let consumer = wmi_consumers(
            &input("pcscope.startup.wmi-consumers", "CommandLineEventConsumer/Example", json!({
                "name": "Example",
                "class": "CommandLineEventConsumer",
                "command": "C:\\Example\\app.exe",
                "script_engine": null,
                "executable_path": "C:\\Example\\app.exe",
                "source_properties": {"__RELPATH": "CommandLineEventConsumer.Name=\"Example\""}
            })),
            &context,
        );
        assert!(consumer.observations[0].identity.missing.is_empty(), "{:?}", consumer.observations[0].identity.missing);
        assert_eq!(field_of(&consumer, "consumer_type").value, Some(json!("CommandLineEventConsumer")));
        assert_eq!(field_of(&consumer, "program").value, Some(json!("C:\\Example\\app.exe")));
        // The item sent `script_engine` as an explicit null; the type has no engine it can claim.
        assert_eq!(field_of(&consumer, "script_engine").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_wmi_item_with_no_path_lists_the_key_as_missing_rather_than_inventing_one() {
        let result = wmi_filters(&input("pcscope.startup.wmi-filters", "ExampleFilter", json!({"name": "ExampleFilter", "query": "SELECT *"})), &context());
        // The type declares a native path and the source named none it can be read from; the
        // identity says so instead of substituting the name.
        assert_eq!(result.observations[0].identity.missing, vec!["path".to_owned()]);
        assert!(!result.observations[0].identity.fields.iter().any(|field| field.key == "path"));
    }

    #[test]
    fn a_binding_is_keyed_by_its_namespace_filter_and_consumer() {
        let result = wmi_bindings(
            &input("pcscope.startup.wmi-bindings", "__EventFilter.Name=\"ExampleFilter\"/CommandLineEventConsumer.Name=\"Example\"", json!({
                "Filter": "__EventFilter.Name=\"ExampleFilter\"",
                "Consumer": "CommandLineEventConsumer.Name=\"Example\"",
                "MaintainSecurityContext": true,
                "SlowDownProviders": false,
                "DeliverSynchronously": false,
                "source_properties": {"__NAMESPACE": "root/subscription"}
            })),
            &context(),
        );
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        for key in ["namespace", "filter_name", "consumer_name"] {
            assert!(identity.fields.iter().any(|field| field.key == key), "{key} is supplied");
        }
        // The binding keeps the two references it registers instead of building a relationship
        // object of its own.
        assert_eq!(field_of(&result, "filter").value, Some(json!("__EventFilter.Name=\"ExampleFilter\"")));
        assert_eq!(field_of(&result, "consumer").value, Some(json!("CommandLineEventConsumer.Name=\"Example\"")));
        assert_eq!(field_of(&result, "security_context").value, Some(json!(true)));
        assert_eq!(identity.scope, "machine:M");
    }

    #[test]
    fn every_source_declares_the_type_it_produces() {
        // The category's sources are the ten the plan assigns to it, and each names a type: a source
        // whose type the adapter decided per item would leave this `None`.
        assert_eq!(SOURCES.len(), 10);
        for spec in SOURCES {
            assert!(spec.type_id.is_some(), "{} names the type it produces", spec.source);
        }
    }

    #[test]
    fn the_same_material_produces_the_same_candidates_twice() {
        let context = context();
        // The expanding shape is the one worth replaying: an unstable iteration over the value table
        // would reorder or reselect the entries between two runs of the same material.
        let payload = startup_approved_item();
        let once: Vec<String> = logon(&input("pcscope.startup.logon", "LocalMachine/Registry64/software\\startupapproved\\run", payload.clone()), &context)
            .observations
            .iter()
            .map(|candidate| key_of(&candidate.identity))
            .collect();
        let twice: Vec<String> = logon(&input("pcscope.startup.logon", "LocalMachine/Registry64/software\\startupapproved\\run", payload), &context)
            .observations
            .iter()
            .map(|candidate| key_of(&candidate.identity))
            .collect();
        assert_eq!(once, twice, "replaying the same material must decide the same thing");
    }
}
