//! S15 policy and configuration adapters.
//!
//! Checked against the collector's own emit declarations: `domains/security.ps1`
//! (`policy.configuration`), `domains/applications.ps1` (`apps.browser-policies`),
//! `domains/startup.ps1` (`environment.configuration`), `domains/system.ps1` (`system.clock`,
//! `system.reboot-pending`, `system.restore-points`), `domains/storage.ps1`
//! (`power.configuration`) and `collect-system.ps1` (`system.boot-state`, `system.version`). The
//! payload keys below are those emitters' own key names.
//!
//! Four of these sources carry more than one type or more than one item shape, so the shape of the
//! material decides the shape of the result rather than the other way round:
//!
//!   * `policy.configuration` and `environment.configuration` read registry keys under several
//!     paths, and the path decides which of their declared types a key belongs to;
//!   * `system.clock` emits a registry key and a literal item, and both are one type;
//!   * `power.configuration` emits a `Win32_PowerPlan` stream *and* a registry key no declared type
//!     reads, so one of its two streams has no object to become.
//!
//! Three identity shapes occur here and they are not interchangeable. `registry_registration`
//! (`shell.file_association`) takes the hive, the view and the key path as keys, so it has no room
//! for a value name and one key is one candidate. `native_path` (`policy.time_sync_configuration`,
//! `policy.os_configuration`) takes the key path, or the item's own classification id where the item
//! carries no path. `value_key` (`policy.uac_policy`, `policy.software_restriction_policy`,
//! `policy.browser_policy`, `policy.environment_variable`) makes one object out of one *value*: a
//! key expands into one candidate per entry in its value table, and each carries that entry's name.
//! Applying the wrong one merges values that are separate objects, or splits one key into many.
//!
//! **No type in this category declares an action.** `system.restore-points` states that it does not
//! create restore points; nothing here reports a snapshot as an action.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.policy.configuration", type_id: None, build: policy_configuration },
    SourceSpec { source: "pcscope.apps.browser-policies", type_id: Some("policy.browser_policy"), build: browser_policies },
    SourceSpec { source: "pcscope.environment.configuration", type_id: None, build: environment_configuration },
    SourceSpec { source: "pcscope.system.clock", type_id: None, build: system_clock },
    SourceSpec { source: "pcscope.system.reboot-pending", type_id: Some("policy.os_configuration"), build: reboot_pending },
    SourceSpec { source: "pcscope.system.restore-points", type_id: Some("policy.system_restore_point"), build: restore_points },
    // Two streams, and only one of them is a declared object, so the target is decided per item.
    SourceSpec { source: "pcscope.power.configuration", type_id: None, build: power_configuration },
    SourceSpec { source: "pcscope.system.boot-state", type_id: Some("policy.os_configuration"), build: boot_state },
    SourceSpec { source: "pcscope.system.version", type_id: Some("policy.os_configuration"), build: system_version },
];

// ---------- registry shapes ----------

/// The collector's value table of a registry item: `name -> {kind,value}`. `Registry-Key` builds
/// every registry item with one, and no other item shape in these sources has it.
fn values_of(payload: &Value) -> Option<&serde_json::Map<String, Value>> {
    payload.get("values").and_then(Value::as_object)
}

/// Whether the item is one of the registry-shaped payloads:
/// `{path;hive;view;values;basis;collection}`. Recognised by its value table rather than by its
/// `path` key, because `system.clock` also emits a literal item that carries no path at all.
fn registry_shaped(payload: &Value) -> bool {
    values_of(payload).is_some()
}

/// The scope a hive's objects belong to.
///
/// A machine hive belongs to the installation: which account ran the collection is *how* the key was
/// read, not *what* it configures, so the collecting user segment is dropped — the machine-level
/// narrowing `installation_scope` performs. The current user's hive is the one case here that is
/// genuinely per-user; it keeps the scope it was handed, user segment included, so two users'
/// settings can never be merged into one object.
fn hive_scope(context: &AdapterContext, payload: &Value) -> String {
    let hive = text(payload, "hive").unwrap_or_default();
    if hive.eq_ignore_ascii_case("CurrentUser") {
        context.collected_scope.trim().to_owned()
    } else {
        installation_scope(&context.collected_scope)
    }
}

/// The scope a value-level registry identity is formed in.
///
/// The value-level types declare `[scope, path, value_name]` and have no key for the hive or for the
/// registry view, yet the collector reads the same path under both views and, for policies and
/// browser policies, under both the machine and the current-user hive. Their declared scope names
/// exactly those two — a user or machine scope plus the registry view — and their `insufficient`
/// clause forbids merging same-named values across users or across views. Both are therefore folded
/// into the scope, the one component these keys leave room for. The `registry_registration` type
/// below does not do this: its keys have a `hive` and a `view` of their own.
fn value_scope(context: &AdapterContext, payload: &Value) -> String {
    let base = hive_scope(context, payload);
    match text(payload, "view") {
        Some(view) if !view.trim().is_empty() => format!("{base}/view:{view}"),
        _ => base,
    }
}

/// A field whose value is known because the adapter just read it out of the source's own text.
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

/// One observation for one value name, shared by every value-level expansion here: the identity of
/// a value-level type is always the key's path paired with the name of a value inside it.
fn value_observation(
    input: &EvidenceInput,
    context: &AdapterContext,
    type_id: &str,
    scope: &str,
    path: Option<String>,
    name: &str,
    fields: Vec<FieldCandidate>,
) -> ObservationCandidate {
    ObservationCandidate {
        identity: identity(type_id, scope, &input.evidence.id, &[("path", path), ("value_name", Some(name.to_owned()))]),
        fields,
        observed_at: input.observed_at.clone(),
        evidence: vec![input.evidence.clone()],
        complete: context.complete,
    }
}

/// One candidate per entry in a registry key's value table, for the types whose identity is a value.
///
/// `values` holds every value under the key: `Registry-Key`'s `$allowed` parameter is not a filter —
/// the collector keeps configuration values rather than a historical display allowlist — so a value
/// name outside the list the `Emit` call names is still collected and still becomes an object.
/// Expanding over the names actually present is therefore both the faithful reading and the one that
/// keeps the policy sources' own distinction between *unset* and *explicitly off*: a name the key
/// does not hold produces no object at all, while a name it holds with a zero produces one whose
/// value is that zero.
///
/// Values are read through `registry_field`, so a value the source masked stays `Masked` rather than
/// becoming a value that participates in comparison. A value whose data the source sent as null
/// falls through `registry_value` to the key's own top level; only the six reserved keys of the
/// registry shape (`path`, `hive`, `view`, `values`, `basis`, `collection`) could be mistaken for
/// data, and none of the value names these three sources carry is one of them.
fn value_candidates(
    input: &EvidenceInput,
    context: &AdapterContext,
    type_id: &str,
    scope: &str,
    payload: &Value,
    leading: Vec<FieldCandidate>,
) -> AdapterResult {
    let Some(values) = values_of(payload) else {
        return AdapterResult::failed("registry item carries no value table; a value-level identity cannot be formed", false);
    };
    let path = text(payload, "path");
    let candidates: Vec<ObservationCandidate> = values
        .keys()
        .map(|name| {
            let mut fields = leading.clone();
            fields.push(field(payload, "path", "path", FieldKind::Identity));
            fields.push(known("name", FieldKind::Identity, Value::String(name.clone())));
            fields.push(registry_field(payload, "value", name, FieldKind::Configuration));
            value_observation(input, context, type_id, scope, path.clone(), name, fields)
        })
        .collect();
    observations(candidates)
}

// ---------- policy.configuration ----------

/// `SOFTWARE\Microsoft\Windows\CurrentVersion\Policies` under both hives, and
/// `SOFTWARE\Policies\Microsoft\Windows\Safer`, each swept three levels deep.
///
/// The sweep means the items are not one key per base path but every subkey too, and both declared
/// types are value-level, so each of those keys expands into one object per value name it holds.
///
/// The routing between the two types is the path's: Safer is Windows' software restriction policy,
/// while the `...\CurrentVersion\Policies` subtree is where the UAC settings live (`...\Policies\
/// System`). The catalog declares no third type for that subtree, so a value the sweep found
/// elsewhere under it is still reported under the type that fits the path — dropping material the
/// source sent is the collector's decision, not this adapter's. Neither type declares an action.
fn policy_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if !registry_shaped(payload) {
        return AdapterResult::not_an_object("policy configuration item is not a registry key; no declared type reads another shape");
    }
    let type_id = if is_restriction_path(payload) { "policy.software_restriction_policy" } else { "policy.uac_policy" };
    value_candidates(input, context, type_id, &value_scope(context, payload), payload, Vec::new())
}

/// Whether the key is the software restriction policy's: the one base path the collector reads
/// outside `...\CurrentVersion\Policies`. Compared case-insensitively because registry path
/// case is not significant, which is a routing decision and not an identity value.
fn is_restriction_path(payload: &Value) -> bool {
    text(payload, "path")
        .map(|path| path.to_lowercase().contains("\\policies\\microsoft\\windows\\safer"))
        .unwrap_or(false)
}

// ---------- apps.browser-policies ----------

/// `Software\Policies\<vendor>\<browser>` for Chrome, Edge and Firefox, under both hives and two
/// levels deep.
///
/// Value-level like the policy sources above: a browser policy is a value name under the browser's
/// base key, and a policy that carries a list is a subkey whose numbered values are the entries.
/// Both are collected, and both expand into one object per value name.
fn browser_policies(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if !registry_shaped(payload) {
        return AdapterResult::not_an_object("browser policy item is not a registry key; this source emits none");
    }
    let leading = vec![match browser_of(payload) {
        Some(browser) => known("browser", FieldKind::Identity, Value::String(browser)),
        // The path does not name a browser; the field is left absent rather than filled with one of
        // the three this source reads.
        None => field(payload, "browser", "browser", FieldKind::Identity),
    }];
    value_candidates(input, context, "policy.browser_policy", &value_scope(context, payload), payload, leading)
}

/// The browser a policy key belongs to, read out of the path the collector wrote: every base path
/// this source reads is `Software\Policies\<vendor>\<browser>`, and a key beneath one keeps the same
/// vendor and browser, so the segment after the vendor names it. A path without that shape yields
/// nothing, which leaves the field absent.
fn browser_of(payload: &Value) -> Option<String> {
    let path = text(payload, "path")?;
    let segments: Vec<&str> = path.split('\\').filter(|segment| !segment.trim().is_empty()).collect();
    let policies = segments.iter().position(|segment| segment.eq_ignore_ascii_case("Policies"))?;
    segments.get(policies + 2).map(|browser| (*browser).to_owned())
}

// ---------- environment.configuration ----------

/// The environment keys — `SYSTEM\CurrentControlSet\Control\Session Manager\Environment` for the
/// machine, `Environment` for the collecting user — and the user's two association keys,
/// `...\Explorer\FileExts` and `...\Shell\Associations\UrlAssociations`, two levels deep.
///
/// Two declared types share this source and the path decides between them. The environment keys hold
/// one variable per value name, so they expand value-level; the association keys are registrations
/// in their own right, so each key is one object. That the association type lives in the shell
/// category changes nothing here: the adapter belongs to the source that reads it.
fn environment_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if !registry_shaped(payload) {
        return AdapterResult::not_an_object("environment configuration item is not a registry key; this source emits none");
    }
    if is_association_path(payload) {
        file_association(input, context)
    } else {
        environment_variable(input, context)
    }
}

/// Whether the key is one of the two association keys rather than an environment key.
fn is_association_path(payload: &Value) -> bool {
    text(payload, "path")
        .map(|path| {
            path.split('\\')
                .any(|segment| segment.eq_ignore_ascii_case("FileExts") || segment.eq_ignore_ascii_case("UrlAssociations"))
        })
        .unwrap_or(false)
}

/// One candidate per value of an environment key.
///
/// The type's three fields are the variable's name, the scope it is set in and its value. The scope
/// is the hive the source actually read, stated as the source states it; that is also what keeps the
/// machine's variables and the collecting user's apart, which the type requires to be separate
/// instances rather than one object matched by name. The value is read through `registry_field`, so
/// a masked variable stays masked — the collector keeps the masked original rather than dropping it.
fn environment_variable(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let Some(values) = values_of(payload) else {
        return AdapterResult::failed("environment key carries no value table", false);
    };
    let scope = value_scope(context, payload);
    let path = text(payload, "path");
    let hive = text(payload, "hive");
    let candidates: Vec<ObservationCandidate> = values
        .keys()
        .map(|name| {
            let fields = vec![
                known("name", FieldKind::Identity, Value::String(name.clone())),
                match hive.clone() {
                    Some(hive) => known("scope", FieldKind::Identity, Value::String(hive)),
                    None => field(payload, "scope", "hive", FieldKind::Identity),
                },
                registry_field(payload, "value", name, FieldKind::Configuration),
            ];
            value_observation(input, context, "policy.environment_variable", &scope, path.clone(), name, fields)
        })
        .collect();
    observations(candidates)
}

/// One candidate per association key.
///
/// `shell.file_association` is a `registry_registration`, so the hive, the view and the key path are
/// the identity and each key is its own object — including the subkeys the two-level sweep reaches,
/// such as `.txt\UserChoice`. The hive is a key of this type, so it does not go into the scope the
/// way the value-level types above must fold it; the scope is the machine's or the user's, per
/// `hive_scope`.
///
/// `command` is absent: this source reads the association keys, not the programs they name, and the
/// declared field is left `NotProvided` rather than filled from a value that is not a command.
fn file_association(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let path = text(payload, "path");
    let scope = hive_scope(context, payload);
    let identity = identity(
        "shell.file_association",
        &scope,
        &input.evidence.id,
        &[("hive", text(payload, "hive")), ("view", text(payload, "view")), ("registry_path", path)],
    );
    let fields = vec![
        match association_of(payload) {
            Some(extension) => known("extension", FieldKind::Identity, Value::String(extension)),
            None => field(payload, "extension", "Extension", FieldKind::Identity),
        },
        field(payload, "path", "path", FieldKind::Configuration),
        registry_field(payload, "prog_id", "ProgId", FieldKind::Configuration),
        field(payload, "command", "command", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// The association's own name: the key segment right after `FileExts` or `UrlAssociations`.
///
/// Deliberately not the path's last segment. The collector sweeps two levels below each base path, so
/// `...\FileExts\.txt`, `...\FileExts\.txt\UserChoice` and the keys below that are all emitted; the
/// segment after the marker is the `.txt` or `http` every one of them belongs to, while the last
/// segment would read `UserChoice` as an extension.
fn association_of(payload: &Value) -> Option<String> {
    let path = text(payload, "path")?;
    let segments: Vec<&str> = path.split('\\').filter(|segment| !segment.trim().is_empty()).collect();
    for marker in ["FileExts", "UrlAssociations"] {
        if let Some(index) = segments.iter().position(|segment| segment.eq_ignore_ascii_case(marker)) {
            return segments.get(index + 1).map(|name| (*name).to_owned());
        }
    }
    None
}

// ---------- system.clock ----------

/// The time service's configuration key, and the machine's time zone.
///
/// The source emits two item shapes. `Registry-Key`'s item is a registry key and carries a value
/// table; the second is a literal `Item 'timezone'` whose data is `{id;basis}` and nothing else.
/// Both belong to `policy.time_sync_configuration` — the type declares `path`, `time_zone` and
/// `values`, one field for each thing the two shapes state — so the payload decides which branch
/// forms the object, and each branch leaves the other's field absent.
///
/// The key's whole value table becomes the `values` field: the type declares one field for the time
/// service's settings rather than one per setting, and the masked values inside the table stay as the
/// collector left them. The source states that it does not modify the system time, and nothing here
/// claims it did.
fn system_clock(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if registry_shaped(payload) {
        let scope = value_scope(context, payload);
        let path = text(payload, "path");
        let identity = identity("policy.time_sync_configuration", &scope, &input.evidence.id, &[("path", path)]);
        let fields = vec![
            field(payload, "path", "path", FieldKind::Identity),
            // This shape states no time zone; the other item is where the source states it.
            field(payload, "time_zone", "time_zone", FieldKind::Configuration),
            field(payload, "values", "values", FieldKind::Configuration),
        ];
        return object_candidate(input, context, identity, fields);
    }
    // A literal item: its classification id is the natural path it is keyed by, and the id is what
    // the source states — the item carries no `path` of its own.
    let scope = installation_scope(&context.collected_scope);
    let path = input.item_id.clone();
    let identity = identity("policy.time_sync_configuration", &scope, &input.evidence.id, &[("path", Some(path.clone()))]);
    let fields = vec![
        known("path", FieldKind::Identity, Value::String(path)),
        field(payload, "time_zone", "id", FieldKind::Configuration),
        // This shape states no value table.
        field(payload, "values", "values", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- system.reboot-pending ----------

/// The keys Windows sets when a reboot is pending.
///
/// Each is a registry key and `policy.os_configuration`'s natural path is the key path, so each key
/// is one record. The type carries the whole operating system's fields while this source states only
/// the pending-reboot one: the rest are left `NotProvided` rather than filled from the other two
/// sources feeding the type, which arrive as their own items and are never this adapter's to read.
fn reboot_pending(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let Some(path) = text(payload, "path").filter(|_| registry_shaped(payload)) else {
        return AdapterResult::not_an_object("reboot-pending item is not a registry key; this source emits none");
    };
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("policy.os_configuration", &scope, &input.evidence.id, &[("path", Some(path))]);
    let fields = vec![
        field(payload, "path", "path", FieldKind::Identity),
        field(payload, "caption", "caption", FieldKind::Configuration),
        field(payload, "version", "version", FieldKind::Configuration),
        field(payload, "build", "build", FieldKind::Configuration),
        field(payload, "install_time", "install_time", FieldKind::Configuration),
        field(payload, "boot_time", "boot_time", FieldKind::State),
        pending_field(payload),
    ];
    object_candidate(input, context, identity, fields)
}

/// Whether this key states that a reboot is pending, read from the source's own emission.
///
/// `Registry-Key` returns nothing for a key it cannot open, so an item under a `RebootPending` or
/// `RebootRequired` path *is* the statement that the key is there, and those two keys exist only
/// while a reboot is pending — which is why the collector reads them. `Session Manager` is read
/// unconditionally, so its mere presence states nothing: there the fact is the value
/// `PendingFileRenameOperations`, and a key the collector did read without that value is the
/// statement that it is not set. The flag is a runtime state and is mapped as one. A key whose
/// meaning this adapter does not know states no pending reboot, and none is invented for it.
fn pending_field(payload: &Value) -> FieldCandidate {
    let path = text(payload, "path").unwrap_or_default().to_lowercase();
    if path.ends_with("rebootpending") || path.ends_with("rebootrequired") {
        return known("reboot_pending", FieldKind::State, Value::Bool(true));
    }
    if path.ends_with("session manager") {
        return known("reboot_pending", FieldKind::State, Value::Bool(registry_value(payload, "PendingFileRenameOperations").is_some()));
    }
    field(payload, "reboot_pending", "reboot_pending", FieldKind::State)
}

// ---------- system.restore-points ----------

/// `SystemRestore` in the `root/default` namespace: one item per restore point, keyed by the
/// `SequenceNumber` the collector keyed it by.
///
/// The type's native identifier is the source's own, so this adapter supplies it and nothing else.
/// `EventType` is collected as well, but the type declares no field for it and no field is added.
/// The source states that it does not create restore points, and no action is invented from it.
fn restore_points(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("policy.system_restore_point", &scope, &input.evidence.id, &[("native_id", Some(input.item_id.clone()))]);
    let fields = vec![
        field(payload, "sequence", "SequenceNumber", FieldKind::Identity),
        field(payload, "description", "Description", FieldKind::Configuration),
        field(payload, "creation_time", "CreationTime", FieldKind::Configuration),
        field(payload, "type", "RestorePointType", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- power.configuration ----------

/// `Win32_PowerPlan` from the `root/cimv2/power` namespace, and the power settings key.
///
/// The plan stream is this source's object material: one item per plan, keyed by the `InstanceID`
/// the class publishes, with the plan's GUID read out of that identifier. `Description` is collected
/// and the type declares no field for it, so it is not mapped. The source states that it does not
/// switch plans, and nothing here claims one was switched.
///
/// The settings key is a registry key, and no declared type reads it: `policy.power_plan` is about
/// plans, and none of its fields — GUID, name, active, power source — is a registry value table. The
/// key's hibernation and Modern Standby settings therefore stay unadapted rather than being forced
/// into a type that does not describe them; the outcome says so and the reason stays visible.
fn power_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if registry_shaped(payload) {
        return AdapterResult {
            outcome: AdapterOutcome::Skipped { reason: "power settings key describes no declared object; policy.power_plan covers plans, not a registry value table".into() },
            ..AdapterResult::adapted()
        };
    }
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("policy.power_plan", &scope, &input.evidence.id, &[("native_id", Some(input.item_id.clone()))]);
    let fields = vec![
        match plan_guid(payload) {
            Some(guid) => known("guid", FieldKind::Identity, Value::String(guid)),
            // An identifier of another shape is not rewritten: the field falls back to the
            // identifier itself, and to absent when the source stated none.
            None => field(payload, "guid", "InstanceID", FieldKind::Identity),
        },
        field(payload, "name", "ElementName", FieldKind::Configuration),
        field(payload, "active", "IsActive", FieldKind::State),
        // Win32_PowerPlan states no power source, and one is not assumed.
        field(payload, "power_source", "power_source", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// The plan's GUID, out of the instance identifier the class publishes
/// (`Microsoft:PowerPlan\{...}`). Taking the identifier's own GUID is reading what the source wrote;
/// an identifier that does not carry that prefix yields nothing rather than a fabricated GUID.
fn plan_guid(payload: &Value) -> Option<String> {
    let instance = text(payload, "InstanceID")?;
    let guid = instance.rsplit_once('\\')?.1.trim().trim_matches(|character| character == '{' || character == '}');
    (!guid.is_empty()).then(|| guid.to_owned())
}

// ---------- system.version and system.boot-state ----------

/// `Item 'operating-system'`: the machine's version, build and install time.
///
/// `policy.os_configuration`'s natural path is the record's identity within the source, and for this
/// item that is its classification id — the item carries no `path` key. The three sources feeding
/// this type state three different paths (the two literal ids and the pending-reboot registry keys)
/// and nothing in the material gives any two of them one in common, so they are three records rather
/// than one object assembled from three streams; that assembly is what `account.machine_identity`
/// does, where the identity comes from the collection scope instead.
///
/// The collector also states where the install time came from. The type declares no basis field, so
/// it stays in the payload and is not mapped; nothing here states a boot time or a pending reboot,
/// and neither is assumed.
fn system_version(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let path = input.item_id.clone();
    let identity = identity("policy.os_configuration", &scope, &input.evidence.id, &[("path", Some(path.clone()))]);
    let fields = vec![
        known("path", FieldKind::Identity, Value::String(path)),
        field(&input.payload, "caption", "caption", FieldKind::Configuration),
        field(&input.payload, "version", "version", FieldKind::Configuration),
        field(&input.payload, "build", "build", FieldKind::Configuration),
        field(&input.payload, "install_time", "install_time", FieldKind::Configuration),
        // This stream states no boot time.
        field(&input.payload, "boot_time", "boot_time", FieldKind::State),
        field(&input.payload, "reboot_pending", "reboot_pending", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Item 'boot-state'`: the machine's last boot time.
///
/// The same type and the same natural-path rule as `system_version`, so the record is keyed by this
/// item's own classification id. The collector sends the machine's time zone here too; the type
/// declares no time-zone field, so it is not mapped, and the time the collector sends is the runtime
/// state it says it is rather than an install time.
fn boot_state(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let path = input.item_id.clone();
    let identity = identity("policy.os_configuration", &scope, &input.evidence.id, &[("path", Some(path.clone()))]);
    let fields = vec![
        known("path", FieldKind::Identity, Value::String(path)),
        field(&input.payload, "caption", "caption", FieldKind::Configuration),
        field(&input.payload, "version", "version", FieldKind::Configuration),
        field(&input.payload, "build", "build", FieldKind::Configuration),
        // This stream states no install time.
        field(&input.payload, "install_time", "install_time", FieldKind::Configuration),
        field(&input.payload, "boot_time", "last_boot_time", FieldKind::State),
        field(&input.payload, "reboot_pending", "reboot_pending", FieldKind::State),
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

    fn context_for(scope: &str) -> AdapterContext {
        AdapterContext {
            catalog_version: crate::model::catalog_version().into(),
            rule_version: crate::model::catalog_fingerprint(),
            machine_id: "M".into(),
            installation_id: "I".into(),
            boot_id: None,
            collected_scope: crate::semantics::business_scope(scope),
            complete: true,
        }
    }

    fn context() -> AdapterContext {
        context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0")
    }

    /// A registry item exactly as `Registry-Key` builds it.
    fn registry_item(hive: &str, view: &str, path: &str, values: Value) -> Value {
        json!({"path": path, "hive": hive, "view": view, "values": values, "basis": "configuration snapshot", "collection": {"policy": "evidence-v2"}})
    }

    /// One entry of `Registry-Key`'s value table.
    fn value(kind: &str, value: impl Into<Value>) -> Value {
        let value: Value = value.into();
        json!({"kind": kind, "value": value})
    }

    fn key_of(candidate: &IdentityCandidate) -> String {
        let mut parts: Vec<String> = candidate.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect();
        parts.sort();
        parts.join("|")
    }

    fn field_of<'a>(result: &'a AdapterResult, field_id: &str) -> &'a FieldCandidate {
        result.observations[0].fields.iter().find(|field| field.field_id == field_id).expect("field is mapped")
    }

    #[test]
    fn a_policy_key_expands_into_one_candidate_per_value_name() {
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
            json!({"EnableLUA": value("DWord", 1), "ConsentPromptBehaviorAdmin": value("DWord", 2)}),
        );
        let result = policy_configuration(&input("pcscope.policy.configuration", "LocalMachine/Registry64/policies", payload), &context());
        assert_eq!(result.observations.len(), 2, "one value name is one object, not one key");
        for candidate in &result.observations {
            assert_eq!(candidate.identity.type_id, "policy.uac_policy");
            assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
            assert!(candidate.identity.fields.iter().any(|field| field.key == "value_name"));
            let name = candidate.fields.iter().find(|field| field.field_id == "name").expect("name is mapped");
            assert_eq!(name.status, ValueStatus::Known);
        }

        // The Safer key is the category's other declared type, and it says so by its path.
        let safer = registry_item(
            "LocalMachine",
            "Registry64",
            "SOFTWARE\\Policies\\Microsoft\\Windows\\Safer\\CodeIdentifiers",
            json!({"DefaultLevel": value("DWord", 0)}),
        );
        let result = policy_configuration(&input("pcscope.policy.configuration", "LocalMachine/Registry64/safer", safer), &context());
        assert_eq!(result.observations[0].identity.type_id, "policy.software_restriction_policy");
        assert_eq!(field_of(&result, "value").value, Some(json!(0)), "an explicit zero is a value, not an absence");
    }

    #[test]
    fn the_scope_carries_the_hive_and_the_view_so_neither_merges() {
        let source = "pcscope.environment.configuration";
        // The same key path under both hives: the hive is the only difference, and it is what the
        // value-level identity has no key of its own for.
        let machine = registry_item("LocalMachine", "Registry64", "Environment", json!({"Path": value("ExpandString", "C:\\Windows")}));
        let user = registry_item("CurrentUser", "Registry64", "Environment", json!({"Path": value("ExpandString", "C:\\Users\\u")}));
        let machine = environment_configuration(&input(source, "machine", machine), &context());
        let user = environment_configuration(&input(source, "user", user), &context());
        assert_ne!(key_of(&machine.observations[0].identity), key_of(&user.observations[0].identity));

        // And the same key read under the two views is two objects as well.
        let view32 = registry_item("CurrentUser", "Registry32", "Environment", json!({"Path": value("ExpandString", "C:\\Users\\u")}));
        let view32 = environment_configuration(&input(source, "view32", view32), &context());
        assert_ne!(key_of(&user.observations[0].identity), key_of(&view32.observations[0].identity));
    }

    #[test]
    fn a_masked_policy_value_stays_masked() {
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
            json!({"EnableLUA": {"kind": "String", "value": crate::sensitive::MASK}}),
        );
        let result = policy_configuration(&input("pcscope.policy.configuration", "item", payload), &context());
        let field = field_of(&result, "value");
        assert_eq!(field.status, ValueStatus::Masked, "a display mask is not a value to compare");
        assert_eq!(field.value, Some(json!(crate::sensitive::MASK)));
    }

    #[test]
    fn browser_policies_expand_per_value_and_name_the_browser_the_path_names() {
        let source = "pcscope.apps.browser-policies";
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "Software\\Policies\\Microsoft\\Edge",
            json!({"PasswordManagerEnabled": value("DWord", 0), "BrowserSignin": value("DWord", 2)}),
        );
        let result = browser_policies(&input(source, "edge", payload), &context());
        assert_eq!(result.observations.len(), 2);
        for candidate in &result.observations {
            assert_eq!(candidate.identity.type_id, "policy.browser_policy");
            assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
            assert_eq!(candidate.fields.iter().find(|field| field.field_id == "browser").expect("browser is mapped").value, Some(json!("Edge")));
        }

        // A list policy is a subkey with numbered values; the browser is still the key's own.
        let sub = registry_item(
            "LocalMachine",
            "Registry64",
            "Software\\Policies\\Google\\Chrome\\ExtensionInstallBlocklist",
            json!({"1": value("String", "*")}),
        );
        let result = browser_policies(&input(source, "chrome", sub), &context());
        assert_eq!(result.observations[0].fields.iter().find(|field| field.field_id == "browser").expect("browser is mapped").value, Some(json!("Chrome")));
    }

    #[test]
    fn environment_configuration_separates_environment_variables_from_file_associations() {
        let source = "pcscope.environment.configuration";
        let environment = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            json!({"windir": value("ExpandString", "C:\\Windows"), "NUMBER_OF_PROCESSORS": value("String", "8")}),
        );
        let result = environment_configuration(&input(source, "environment", environment), &context());
        assert_eq!(result.observations.len(), 2, "one variable per value name");
        for candidate in &result.observations {
            assert_eq!(candidate.identity.type_id, "policy.environment_variable");
            assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
            let scope = candidate.fields.iter().find(|field| field.field_id == "scope").expect("scope is mapped");
            assert_eq!(scope.value, Some(json!("LocalMachine")), "the scope states the hive the source read");
            assert_eq!(scope.field_kind, FieldKind::Identity);
        }

        // The association keys are the other declared type: registrations, not values.
        let association = registry_item(
            "CurrentUser",
            "Registry64",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.txt\\UserChoice",
            json!({"ProgId": value("String", "txtfile")}),
        );
        let result = environment_configuration(&input(source, "association", association), &context());
        let candidate = &result.observations[0];
        assert_eq!(candidate.identity.type_id, "shell.file_association");
        assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
        assert!(candidate.identity.fields.iter().any(|field| field.key == "hive" && field.value == "CurrentUser"));
        assert!(candidate.identity.fields.iter().any(|field| field.key == "view" && field.value == "Registry64"));
        assert!(candidate.identity.fields.iter().any(|field| field.key == "registry_path" && field.value.ends_with("UserChoice")));
        assert_eq!(candidate.fields.iter().find(|field| field.field_id == "extension").expect("extension is mapped").value, Some(json!(".txt")));
        assert_eq!(candidate.fields.iter().find(|field| field.field_id == "prog_id").expect("prog_id is mapped").value, Some(json!("txtfile")));
        assert_eq!(field_of(&result, "command").status, ValueStatus::NotProvided, "this source reads no command");
    }

    #[test]
    fn the_clock_source_maps_both_of_its_item_shapes_to_one_type() {
        let source = "pcscope.system.clock";
        let key = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Services\\W32Time",
            json!({"Type": value("String", "NTP"), "Enabled": value("DWord", 1)}),
        );
        let key = system_clock(&input(source, "LocalMachine/Registry64/W32Time", key), &context());
        assert_eq!(key.observations[0].identity.type_id, "policy.time_sync_configuration");
        assert!(key.observations[0].identity.missing.is_empty(), "{:?}", key.observations[0].identity.missing);
        assert!(key.observations[0].identity.fields.iter().any(|field| field.key == "path" && field.value == "SYSTEM\\CurrentControlSet\\Services\\W32Time"));
        assert_eq!(field_of(&key, "values").status, ValueStatus::Known);
        assert_eq!(field_of(&key, "time_zone").status, ValueStatus::NotProvided);

        // The literal item is the other shape: no path key, no value table, and the time zone is
        // where the source states it.
        let timezone = system_clock(&input(source, "timezone", json!({"id": "China Standard Time", "basis": "observed configuration"})), &context());
        assert_eq!(timezone.observations[0].identity.type_id, "policy.time_sync_configuration");
        assert!(timezone.observations[0].identity.fields.iter().any(|field| field.key == "path" && field.value == "timezone"));
        assert_eq!(field_of(&timezone, "time_zone").value, Some(json!("China Standard Time")));
        assert_eq!(field_of(&timezone, "values").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_pending_reboot_is_stated_by_the_key_and_read_from_the_session_manager_value() {
        let source = "pcscope.system.reboot-pending";
        let pending = registry_item(
            "LocalMachine",
            "Registry64",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Component Based Servicing\\RebootPending",
            json!({}),
        );
        let pending = reboot_pending(&input(source, "cbs", pending), &context());
        assert_eq!(field_of(&pending, "reboot_pending").value, Some(json!(true)), "the key is there only while a reboot is pending");

        let session = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager",
            json!({"PendingFileRenameOperations": value("MultiString", json!(["\\??\\C:\\tmp\\x"]))}),
        );
        let session = reboot_pending(&input(source, "session", session), &context());
        assert_eq!(field_of(&session, "reboot_pending").value, Some(json!(true)));

        // This key is read unconditionally, so the value is what states the fact.
        let quiet = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager",
            json!({"SafeDllSearchMode": value("DWord", 1)}),
        );
        let quiet = reboot_pending(&input(source, "quiet", quiet), &context());
        assert_eq!(field_of(&quiet, "reboot_pending").value, Some(json!(false)));
        assert_eq!(field_of(&quiet, "reboot_pending").field_kind, FieldKind::State);
        // The type's other fields belong to the other two sources, and are not filled in here.
        assert_eq!(field_of(&quiet, "caption").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&quiet, "boot_time").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_restore_point_is_keyed_by_the_sequence_number_and_invents_no_action() {
        let payload = json!({"SequenceNumber": 12, "Description": "Windows Update", "CreationTime": "2026-09-01T00:00:00.0000000Z", "RestorePointType": 0, "EventType": 1, "source_properties": {}});
        let result = restore_points(&input("pcscope.system.restore-points", "12", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "policy.system_restore_point");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "native_id" && field.value == "12"));
        assert_eq!(field_of(&result, "sequence").value, Some(json!(12)));
        assert_eq!(field_of(&result, "description").value, Some(json!("Windows Update")));
        assert_eq!(field_of(&result, "creation_time").value, Some(json!("2026-09-01T00:00:00.0000000Z")));
        // The source does not create restore points, and a snapshot is not an action.
        assert!(result.actions.is_empty());
    }

    #[test]
    fn a_power_plan_is_keyed_by_its_instance_id_and_the_settings_key_is_not_a_declared_object() {
        let source = "pcscope.power.configuration";
        let plan = json!({
            "InstanceID": "Microsoft:PowerPlan\\{381b4222-f694-41f0-9685-ff5bb260df2e}",
            "ElementName": "Balanced",
            "IsActive": true,
            "Description": "Balanced plan",
            "source_properties": {}
        });
        let result = power_configuration(&input(source, "Microsoft:PowerPlan\\{381b4222-f694-41f0-9685-ff5bb260df2e}", plan), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "policy.power_plan");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "native_id" && field.value.starts_with("Microsoft:PowerPlan")));
        assert_eq!(field_of(&result, "guid").value, Some(json!("381b4222-f694-41f0-9685-ff5bb260df2e")));
        assert_eq!(field_of(&result, "active").value, Some(json!(true)));
        assert_eq!(field_of(&result, "power_source").status, ValueStatus::NotProvided);

        // The settings key is legitimate material, but no declared type describes it.
        let settings = registry_item("LocalMachine", "Registry64", "SYSTEM\\CurrentControlSet\\Control\\Power", json!({"HibernateEnabled": value("DWord", 1)}));
        let result = power_configuration(&input(source, "settings", settings), &context());
        assert!(matches!(result.outcome, AdapterOutcome::Skipped { .. }), "{:?}", result.outcome);
        assert!(result.observations.is_empty(), "no object is invented for it");
    }

    #[test]
    fn the_operating_system_streams_are_separate_records_and_absent_fields_stay_absent() {
        let version = system_version(
            &input(
                "pcscope.system.version",
                "operating-system",
                json!({"caption": "Windows 11 Pro", "version": "10.0.26200", "build": "26200", "architecture": "64-bit", "install_time": "2026-01-01T00:00:00.0000000Z", "install_time_basis": "Win32_OperatingSystem.InstallDate", "computer_name": "DESKTOP-1"}),
            ),
            &context(),
        );
        assert_eq!(version.observations[0].identity.type_id, "policy.os_configuration");
        assert!(version.observations[0].identity.fields.iter().any(|field| field.key == "path" && field.value == "operating-system"));
        assert_eq!(field_of(&version, "build").value, Some(json!("26200")));
        assert_eq!(field_of(&version, "boot_time").status, ValueStatus::NotProvided, "this stream states no boot time");

        let boot = boot_state(
            &input("pcscope.system.boot-state", "boot-state", json!({"last_boot_time": "2026-09-11T00:00:00.0000000Z", "basis": "snapshot", "timezone": "China Standard Time"})),
            &context(),
        );
        assert_eq!(field_of(&boot, "boot_time").value, Some(json!("2026-09-11T00:00:00.0000000Z")));
        assert_eq!(field_of(&boot, "install_time").status, ValueStatus::NotProvided);

        // Two sources, two classification ids: nothing gives them a path in common, so they stay
        // two records rather than being forced into one.
        assert_ne!(key_of(&version.observations[0].identity), key_of(&boot.observations[0].identity));

        let pending = reboot_pending(
            &input("pcscope.system.reboot-pending", "session", registry_item("LocalMachine", "Registry64", "SYSTEM\\CurrentControlSet\\Control\\Session Manager", json!({}))),
            &context(),
        );
        assert!(pending.observations[0].identity.fields.iter().any(|field| field.key == "path" && field.value == "SYSTEM\\CurrentControlSet\\Control\\Session Manager"));
    }

    #[test]
    fn the_same_material_produces_the_same_candidates_twice() {
        let source = "pcscope.policy.configuration";
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
            json!({"EnableLUA": value("DWord", 1), "FilterAdministratorToken": value("DWord", 0)}),
        );
        let once: Vec<String> = policy_configuration(&input(source, "item", payload.clone()), &context()).observations.iter().map(|candidate| key_of(&candidate.identity)).collect();
        let twice: Vec<String> = policy_configuration(&input(source, "item", payload), &context()).observations.iter().map(|candidate| key_of(&candidate.identity)).collect();
        assert_eq!(once, twice, "replaying the same material must decide the same thing");
        assert_eq!(once.len(), 2);
    }

    #[test]
    fn a_source_read_by_a_different_account_keeps_machine_level_objects_apart_from_per_user_ones() {
        let source = "pcscope.apps.browser-policies";
        let payload = registry_item("LocalMachine", "Registry64", "Software\\Policies\\Google\\Chrome", json!({"SyncDisabled": value("DWord", 1)}));
        // The same machine-collected policy under two collecting accounts is one object...
        let by_system = browser_policies(&input(source, "chrome", payload.clone()), &context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let by_user = browser_policies(&input(source, "chrome", payload), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_eq!(key_of(&by_system.observations[0].identity), key_of(&by_user.observations[0].identity));

        // ...while a policy the user's own hive holds keeps the account that owns it.
        let per_user = registry_item("CurrentUser", "Registry64", "Software\\Policies\\Google\\Chrome", json!({"SyncDisabled": value("DWord", 1)}));
        let first_user = browser_policies(&input(source, "chrome", per_user.clone()), &context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let second_user = browser_policies(&input(source, "chrome", per_user), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_ne!(key_of(&first_user.observations[0].identity), key_of(&second_user.observations[0].identity));
    }
}
