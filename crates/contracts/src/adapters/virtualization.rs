//! S19 virtualization adapters.
//!
//! Checked against the emitters: `plugins/powershell-adapter/domains/virtualization.ps1` (the four
//! Hyper-V sources and the WSL registration), `domains/scoped.ps1` (the Docker source) and
//! `collect-system.ps1` (the platform capability source). The payload keys below are those
//! emitters' own key names.
//!
//! Three of these sources state more than one kind of object, and the material decides which kind
//! each item is rather than the other way round:
//!
//!   * `virtualization.docker` is **one source feeding four types**. Each item it emits carries a
//!     `kind` naming the Docker object it came from, and the item id is composed differently per
//!     kind — `"$($data.id)/$($data.repository)/$($data.tag)"` for an image and the bare id for the
//!     rest — so the `kind` chooses the type and the payload's own fields supply the identity,
//!     which is what keeps an image from being keyed as though its repository were part of its id.
//!   * `virtualization.platform` emits **two shapes from one declaration**: a literal item named
//!     `hypervisor` and one item per processor keyed by `DeviceID`. Both are the machine's platform
//!     capability, and each shape supplies only the fields its own payload carries.
//!   * `virtualization.wsl-registration` emits one item per registry key it could read, which
//!     includes the `Lxss` key itself. That key is the registration container, not a distribution.
//!
//! **Scope.** `installation_scope` is used wherever the object belongs to an engine the machine
//! owns: the four Hyper-V sources (the Hyper-V engine), the Docker source — the collector hard-codes
//! the Docker Desktop CLI under `$env:ProgramFiles` and reads it over the machine-wide
//! `npipe:////./pipe/docker_engine`, so the engine is one per installation, and which account ran
//! the collection is *how* the list was obtained — and the platform source, whose flags describe the
//! machine. The WSL registration is the one genuinely per-user source here: it is read out of
//! `HKCU`, so it keeps the scope it was handed, user segment included, and two users' registrations
//! can never merge into one object.
//!
//! **Nothing here concludes that an object was deleted.** An engine that is not running, or a
//! collector without access to it, produces a source-level gap instead of a short item list, and no
//! adapter draws a conclusion from a snapshot that never mentioned an object.
//!
//! **No type in this category declares an action**, so none is reported: the collector states that
//! it does not start WSL, that it does not create checkpoints, and that the Docker source reads
//! local API metadata only — no logs, arguments, environment or container contents. Nothing below
//! contradicts any of those statements.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.virtualization.hyperv-vms", type_id: Some("virtualization.virtual_machine"), build: hyperv_vms },
    SourceSpec { source: "pcscope.virtualization.hyperv-switches", type_id: Some("virtualization.virtual_switch"), build: hyperv_switches },
    SourceSpec { source: "pcscope.virtualization.hyperv-disks", type_id: Some("virtualization.disk_attachment"), build: hyperv_disks },
    SourceSpec { source: "pcscope.virtualization.hyperv-checkpoints", type_id: Some("virtualization.checkpoint"), build: hyperv_checkpoints },
    SourceSpec { source: "pcscope.virtualization.wsl-registration", type_id: Some("virtualization.wsl_distribution"), build: wsl_registration },
    // One source, four types: the target is decided per item by the `kind` the item carries.
    SourceSpec { source: "pcscope.virtualization.docker", type_id: None, build: docker },
    SourceSpec { source: "pcscope.virtualization.platform", type_id: Some("virtualization.platform_capability"), build: platform },
];

// ---------- hyperv-vms ----------

/// `Get-VM`, one item per virtual machine keyed by the engine's own machine id.
///
/// The type's `native_identifier` key is that id, so the item's identity is the identifier
/// `Get-VM` published — never the name, which the type's `insufficient` clause explicitly forbids
/// substituting for an identifier. The payload's `Id` is the same value and is mapped to the
/// type's `vm_id` field; the item id is the fallback when the payload does not carry it.
///
/// `memory` is left absent. The type declares one memory field and the collector sends four
/// separate memory facts (`MemoryStartup`, `DynamicMemoryEnabled`, `MemoryMinimum`,
/// `MemoryMaximum`), so there is no payload key this field reads: naming one of the four would
/// discard three, and assembling the four into a value the collector never sent would be this
/// adapter inventing an object. They stay in the material, which is where the source put them.
/// `Version`, `AutomaticStartAction`, `AutomaticStopAction`, `ConfigurationLocation` and
/// `CheckpointType` are collected and the type declares no field for them, so they are not mapped
/// either. The source states its own precondition — the Hyper-V module and access to it are
/// required — and that limit travels with the material.
fn hyperv_vms(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let vm_id = text(&input.payload, "Id").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("virtualization.virtual_machine", scope, &input.evidence.id, &[("native_id", Some(vm_id))]);
    let fields = vec![
        field(&input.payload, "vm_id", "Id", FieldKind::Identity),
        field(&input.payload, "name", "Name", FieldKind::Configuration),
        field(&input.payload, "state", "State", FieldKind::State),
        field(&input.payload, "generation", "Generation", FieldKind::Configuration),
        // The catalog's `cpu` is the virtual processor configuration; `ProcessorCount` is the one
        // processor fact this source sends.
        field(&input.payload, "cpu", "ProcessorCount", FieldKind::Configuration),
        field(&input.payload, "memory", "memory", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- hyperv-switches ----------

/// `Get-VMSwitch`, keyed by the switch id `Get-VMSwitch` published.
///
/// `AllowManagementOS` is collected and the type declares no field for it, so it is not mapped.
/// The source states its own limit — the engine not running is reported as a gap, not as a machine
/// with no switches — and nothing here reads a gap as an empty inventory.
fn hyperv_switches(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let switch_id = text(&input.payload, "Id").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("virtualization.virtual_switch", scope, &input.evidence.id, &[("native_id", Some(switch_id))]);
    let fields = vec![
        field(&input.payload, "switch_id", "Id", FieldKind::Identity),
        field(&input.payload, "name", "Name", FieldKind::Configuration),
        field(&input.payload, "type", "SwitchType", FieldKind::Configuration),
        field(&input.payload, "adapter", "NetAdapterInterfaceDescription", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- hyperv-disks ----------

/// `Get-VM | Get-VMHardDiskDrive`: one item per attachment.
///
/// The type is a `composite_native` over the machine and the controller position, and every one of
/// those four components is a payload key the collector sends separately (`VMId`,
/// `ControllerType`, `ControllerNumber`, `ControllerLocation`). They are read from there rather
/// than split back out of the item id, which the emitter builds by joining the same four — the two
/// numbers would have to be turned into text and back, and a component read from a joined string is
/// this adapter decoding a format rather than reading a field. A missing component is listed as
/// missing, which makes the identity provisional instead of guessed.
///
/// The type's `vm` field is the machine the attachment belongs to: it parallels the `vm_id` key, so
/// it takes the identifier, not the mutable display name the payload also carries as `VMName` and
/// for which the type declares no field. `DiskNumber` is collected and not declared, so it is not
/// mapped; the source states that the disk's *contents* are never read.
fn hyperv_disks(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let identity = identity(
        "virtualization.disk_attachment",
        scope,
        &input.evidence.id,
        &[
            ("vm_id", text(&input.payload, "VMId")),
            ("controller_type", text(&input.payload, "ControllerType")),
            ("controller_number", text(&input.payload, "ControllerNumber")),
            ("controller_location", text(&input.payload, "ControllerLocation")),
        ],
    );
    let fields = vec![
        field(&input.payload, "vm", "VMId", FieldKind::Identity),
        field(&input.payload, "path", "Path", FieldKind::Identity),
        field(&input.payload, "controller_type", "ControllerType", FieldKind::Identity),
        field(&input.payload, "controller_number", "ControllerNumber", FieldKind::Identity),
        field(&input.payload, "controller_location", "ControllerLocation", FieldKind::Identity),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- hyperv-checkpoints ----------

/// `Get-VM | Get-VMSnapshot`, keyed by the checkpoint id the engine assigned.
///
/// The type's scope prose names the virtual machine, but its declared keys are `[scope,
/// native_id]` with no component for the machine — unlike `virtualization.disk_attachment`, whose
/// keys carry `vm_id` — and a checkpoint's own id is the source's native identifier. The scope is
/// therefore the installation's, the engine scope the other Hyper-V sources here use, and the
/// payload's `VMId` stays in the material rather than being folded into a scope no declared key
/// asks for.
///
/// A root checkpoint has no `ParentSnapshotId`, and the collector sends that as null: the type's
/// `parent` field is then absent rather than empty, because a checkpoint with no parent is not a
/// checkpoint whose parent could not be read. The source states that it does not create
/// checkpoints, and none is reported.
fn hyperv_checkpoints(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let checkpoint_id = text(&input.payload, "Id").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("virtualization.checkpoint", scope, &input.evidence.id, &[("native_id", Some(checkpoint_id))]);
    let fields = vec![
        field(&input.payload, "checkpoint_id", "Id", FieldKind::Identity),
        field(&input.payload, "name", "Name", FieldKind::Configuration),
        field(&input.payload, "creation_time", "CreationTime", FieldKind::Configuration),
        field(&input.payload, "type", "SnapshotType", FieldKind::Configuration),
        field(&input.payload, "parent", "ParentSnapshotId", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- virtualization.wsl-registration ----------

/// The registration key the collector reads; its immediate subkeys are the distributions.
const WSL_ROOT: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Lxss";

/// The collector's value table of a registry item: `name -> {kind,value}`.
fn values_of(payload: &Value) -> Option<&serde_json::Map<String, Value>> {
    payload.get("values").and_then(Value::as_object)
}

/// The distribution a registration key belongs to, out of the key's path.
///
/// `Registry-Key` emits one item per key it read, and the collector reads the `Lxss` key one level
/// deep, so the items are the root and each of its immediate subkeys. A subkey directly below the
/// root is a distribution's own registration and is named by its subkey name — the registration
/// identifier WSL assigned it. Anything else, the root included, is not a distribution.
fn wsl_registration_name(path: &str) -> Option<&str> {
    let head = path.get(..WSL_ROOT.len())?;
    if !head.eq_ignore_ascii_case(WSL_ROOT) {
        return None;
    }
    let name = path.get(WSL_ROOT.len()..)?.strip_prefix('\\')?;
    (!name.is_empty() && !name.contains('\\')).then_some(name)
}

/// The WSL registration keys, one per distribution the current user has registered.
///
/// The type is a `native_identifier`, and the identifier it asks for is the registration's own:
/// the `Lxss` subkey the distribution lives under, not its `DistributionName` — the type's
/// `insufficient` clause forbids putting a display name where a native identifier belongs, and two
/// distributions can carry the same name while remaining two registrations. The hive is always
/// `CurrentUser` for this source and the registry view is *how* the key was read (the collector
/// reads both views, and folding the view in would turn one registration into two objects), so
/// neither takes part in the identity; the scope keeps the collecting user, which is what the
/// per-user scope is for.
///
/// The root key is **skipped with its reason**: it registers the WSL subsystem and names the
/// default distribution, and no declared field of `virtualization.wsl_distribution` — name,
/// version, location, default uid — carries a subsystem key's values. Forcing it into the type
/// would produce a distribution with no name. The other values the collector reads there (`Flags`,
/// `State`, `DefaultDistribution`) are not declared either and stay in the material; the source-map
/// says the running state is still to be filled in, and nothing here reports one.
///
/// A `values` key the item presents as something other than a table is `unreadable`: the collector
/// was looking at the key, and reporting four absent fields instead would read as a distribution
/// registered with nothing in it.
fn wsl_registration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if payload.get("values").is_none() {
        return AdapterResult::not_an_object("wsl registration item is not a registry key; this source emits none");
    }
    let Some(path) = text(payload, "path") else {
        return AdapterResult::failed("wsl registration item carries no path; its identity cannot be formed", false);
    };
    let path = path.trim().to_owned();
    let Some(registration) = wsl_registration_name(&path) else {
        if path.eq_ignore_ascii_case(WSL_ROOT) {
            return AdapterResult {
                outcome: AdapterOutcome::Skipped {
                    reason: "the Lxss key registers the WSL subsystem and names the default distribution; the declared type describes a distribution, and no declared field carries a subsystem key's values".into(),
                },
                ..AdapterResult::adapted()
            };
        }
        return AdapterResult::not_an_object("wsl registration item is not a distribution registration below the Lxss key; this source emits none");
    };
    // Read out of the current user's hive, so the collecting user stays in the scope.
    let scope = context.collected_scope.trim().to_owned();
    let identity = identity("virtualization.wsl_distribution", &scope, &input.evidence.id, &[("native_id", Some(registration.to_owned()))]);
    let fields = match values_of(payload) {
        Some(_) => vec![
            registry_field(payload, "name", "DistributionName", FieldKind::Identity),
            registry_field(payload, "version", "Version", FieldKind::Configuration),
            registry_field(payload, "location", "BasePath", FieldKind::Configuration),
            registry_field(payload, "default_uid", "DefaultUid", FieldKind::Configuration),
        ],
        None => vec![
            unreadable("name", FieldKind::Identity),
            unreadable("version", FieldKind::Configuration),
            unreadable("location", FieldKind::Configuration),
            unreadable("default_uid", FieldKind::Configuration),
        ],
    };
    object_candidate(input, context, identity, fields)
}

// ---------- virtualization.docker ----------

/// The Docker source: one item per container, image, volume and network, each carrying the `kind`
/// it came from.
///
/// The `kind` decides which of the four the item is, and the item id is composed differently per
/// kind, so the identity components are read out of the item's own `data` object rather than out of
/// the id: an image's id is `"$($data.id)/$($data.repository)/$($data.tag)"` while every other kind
/// carries its bare id, and a repository or tag containing a separator would make splitting that
/// string back apart a guess. Each kind then forms the identity its own type declares:
///
///   * a container, a volume and a network are `native_identifier`s over the id Docker published
///     (for a volume the container lists its name under the template's `id`, and that name is the
///     volume's native identifier; for a network the id and the name are separate facts, and the
///     type declares a field for each);
///   * an image is a `composite_native` over the repository, the tag and the image id, and all
///     three are supplied, with an untagged image's null left absent so its identity is provisional
///     rather than invented.
///
/// The collector states its own basis — local API metadata only, no logs, arguments, environment or
/// container contents — and the type states the same limit. Nothing here reads or reports any of
/// those, and a container whose id the source did not send has no invented identifier.
///
/// A `kind` no declared type describes is **skipped with its reason** rather than forced into one
/// of the four, and an item carrying no metadata object at all has no identity to form and says so.
fn docker(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let Some(kind) = text(payload, "kind") else {
        return AdapterResult::not_an_object("docker item states no kind; which of the declared types it belongs to cannot be decided");
    };
    let Some(data) = payload.get("data").filter(|value| value.is_object()) else {
        return AdapterResult::failed("docker item carries no metadata object; its identity cannot be formed", false);
    };
    let scope = &installation_scope(&context.collected_scope);
    let (identity, fields) = match kind.as_str() {
        "container" => (
            identity("virtualization.container", scope, &input.evidence.id, &[("native_id", text(data, "id"))]),
            vec![
                field(data, "container_id", "id", FieldKind::Identity),
                field(data, "names", "names", FieldKind::Configuration),
                field(data, "image", "image", FieldKind::Configuration),
                field(data, "state", "state", FieldKind::State),
                field(data, "status", "status", FieldKind::State),
            ],
        ),
        "image" => (
            identity(
                "virtualization.image",
                scope,
                &input.evidence.id,
                &[("image_id", text(data, "id")), ("repository", text(data, "repository")), ("tag", text(data, "tag"))],
            ),
            vec![
                field(data, "repository", "repository", FieldKind::Identity),
                field(data, "tag", "tag", FieldKind::Identity),
                field(data, "image_id", "id", FieldKind::Identity),
            ],
        ),
        "volume" => (
            // `docker volume ls` writes the volume's name into the template's `id` slot; the name
            // is the volume's native identifier, so it is both the identity and the `name` field.
            identity("virtualization.container_volume", scope, &input.evidence.id, &[("native_id", text(data, "id"))]),
            vec![field(data, "name", "id", FieldKind::Identity), field(data, "driver", "driver", FieldKind::Configuration)],
        ),
        "network" => (
            identity("virtualization.container_network", scope, &input.evidence.id, &[("native_id", text(data, "id"))]),
            vec![
                field(data, "network_id", "id", FieldKind::Identity),
                field(data, "name", "name", FieldKind::Identity),
                field(data, "driver", "driver", FieldKind::Configuration),
                field(data, "scope", "scope", FieldKind::Configuration),
            ],
        ),
        other => {
            return AdapterResult {
                outcome: AdapterOutcome::Skipped {
                    reason: format!("docker {other} object has no declared type; this source feeds the container, image, volume and network types"),
                },
                ..AdapterResult::adapted()
            }
        }
    };
    object_candidate(input, context, identity, fields)
}

// ---------- virtualization.platform ----------

/// `Win32_ComputerSystem` and `Win32_Processor`: the machine's virtualization capability, as two
/// item shapes in one declaration.
///
/// The literal `Item 'hypervisor'` carries `hypervisor_present` along with the manufacturer and
/// model; each `Win32_Processor` item is keyed by `DeviceID` and carries `name`,
/// `firmware_virtualization`, `vm_monitor_extensions` and `slat`. Both are the one declared type —
/// a capability of the machine, which is why the type is a `platform_capability` and not a virtual
/// machine — so each shape maps the fields whose keys it carries and leaves the other shape's
/// absent, exactly as the payload decides.
///
/// The type is a `named_entity` over `[scope, name]`, and the name is the item's own classification
/// id: `hypervisor` for the literal item and the processor's `DeviceID` for a processor. The
/// payload's `name` is deliberately not used for it — `Win32_Processor.Name` is the model string,
/// which every core of one model reports identically, and the type's `insufficient` clause says a
/// duplicated name is an ambiguity rather than grounds for merging. Two processors are two objects.
///
/// `manufacturer` and `model` are collected and no declared field carries them, so they stay in the
/// material. The source states no failure per item; a machine without Hyper-V simply reports what
/// its firmware and processors say, and nothing here upgrades a flag into a virtual machine.
fn platform(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let name = input.item_id.clone();
    let identity = identity("virtualization.platform_capability", scope, &input.evidence.id, &[("name", Some(name))]);
    let fields = vec![
        field(&input.payload, "hypervisor_present", "hypervisor_present", FieldKind::Configuration),
        field(&input.payload, "firmware_virtualization", "firmware_virtualization", FieldKind::Configuration),
        // The catalog calls the fact `monitor_extensions`; the collector calls the property it read
        // `vm_monitor_extensions`.
        field(&input.payload, "monitor_extensions", "vm_monitor_extensions", FieldKind::Configuration),
        field(&input.payload, "slat", "slat", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::EvidenceKind;
    use serde_json::json;

    const VM_ID: &str = "11111111-2222-3333-4444-555555555555";
    const SWITCH_ID: &str = "22222222-3333-4444-5555-666666666666";
    const CHECKPOINT_ID: &str = "33333333-4444-5555-6666-777777777777";
    const DISTRO_KEY: &str = "{c6eaf6e5-0000-1111-2222-333344445555}";

    fn input(source: &str, item_id: &str, payload: Value) -> EvidenceInput {
        EvidenceInput {
            source: source.into(),
            item_id: item_id.into(),
            structure_version: "v1".into(),
            source_scope: "machine:M/user:S-1-5-21-9/schema:1/context:/shard:0".into(),
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
        context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0")
    }

    fn key_of(candidate: &IdentityCandidate) -> String {
        let mut parts: Vec<String> = candidate.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect();
        parts.sort();
        parts.join("|")
    }

    fn field_of<'a>(result: &'a AdapterResult, field_id: &str) -> &'a FieldCandidate {
        result.observations[0].fields.iter().find(|field| field.field_id == field_id).expect("field is mapped")
    }

    fn vm_payload() -> Value {
        json!({
            "Id": VM_ID,
            "Name": "示例虚拟机",
            "State": "Running",
            "Generation": 2,
            "Version": "10.0",
            "ProcessorCount": 4,
            "MemoryStartup": 2147483648i64,
            "DynamicMemoryEnabled": true,
            "MemoryMinimum": 536870912i64,
            "MemoryMaximum": 4294967296i64,
            "AutomaticStartAction": "StartIfRunning",
            "AutomaticStopAction": "Save",
            "ConfigurationLocation": "C:\\ProgramData\\Microsoft\\Windows\\Hyper-V",
            "CheckpointType": "Production"
        })
    }

    fn switch_payload() -> Value {
        json!({"Id": SWITCH_ID, "Name": "示例交换机", "SwitchType": "External", "NetAdapterInterfaceDescription": "Intel(R) Ethernet", "AllowManagementOS": true})
    }

    fn disk_payload() -> Value {
        json!({"VMId": VM_ID, "VMName": "示例虚拟机", "Path": "C:\\示例\\disk.vhdx", "ControllerType": "SCSI", "ControllerNumber": 0, "ControllerLocation": 1, "DiskNumber": 3})
    }

    fn checkpoint_payload() -> Value {
        json!({"Id": CHECKPOINT_ID, "VMId": VM_ID, "Name": "示例检查点", "CreationTime": "2026-09-11T10:00:00.0000000Z", "SnapshotType": "Standard", "ParentSnapshotId": null})
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

    fn wsl_payload() -> Value {
        registry_item(
            "CurrentUser",
            "Registry64",
            &format!("{WSL_ROOT}\\{DISTRO_KEY}"),
            json!({
                "DistributionName": value("String", "示例发行版"),
                "Version": value("DWord", 2),
                "BasePath": value("String", "C:\\示例\\wsl"),
                "DefaultUid": value("DWord", 1000),
                "Flags": value("DWord", 15),
                "State": value("DWord", 1)
            }),
        )
    }

    /// A Docker item exactly as the emitter builds it.
    fn docker_item(kind: &str, data: Value) -> Value {
        json!({"kind": kind, "data": data, "basis": "local Docker API metadata; no logs, arguments, environment or container contents"})
    }

    #[test]
    fn a_virtual_machine_is_keyed_by_the_identifier_the_engine_assigned() {
        let result = hyperv_vms(&input("pcscope.virtualization.hyperv-vms", VM_ID, vm_payload()), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "virtualization.virtual_machine");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "native_id" && field.value == VM_ID));
        assert_eq!(field_of(&result, "vm_id").value, Some(json!(VM_ID)));
        assert_eq!(field_of(&result, "name").value, Some(json!("示例虚拟机")));
        assert_eq!(field_of(&result, "state").value, Some(json!("Running")));
        assert_eq!(field_of(&result, "state").field_kind, FieldKind::State);
        assert_eq!(field_of(&result, "generation").value, Some(json!(2)));
        // The one processor fact the source sends is the count the type's processor field reads.
        assert_eq!(field_of(&result, "cpu").value, Some(json!(4)));
    }

    #[test]
    fn a_field_the_type_has_no_single_payload_key_for_stays_absent() {
        let result = hyperv_vms(&input("pcscope.virtualization.hyperv-vms", VM_ID, vm_payload()), &context());
        // Four memory facts are in the material and the type declares one memory field. None of
        // them is picked, and no aggregate is assembled from them.
        let memory = field_of(&result, "memory");
        assert_eq!(memory.status, ValueStatus::NotProvided);
        assert!(memory.value.is_none(), "an absence is never filled in");
    }

    #[test]
    fn a_switch_is_keyed_by_its_own_identifier() {
        let result = hyperv_switches(&input("pcscope.virtualization.hyperv-switches", SWITCH_ID, switch_payload()), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "virtualization.virtual_switch");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "native_id" && field.value == SWITCH_ID));
        assert_eq!(field_of(&result, "type").value, Some(json!("External")));
        assert_eq!(field_of(&result, "adapter").value, Some(json!("Intel(R) Ethernet")));
        // The type declares no field for the management-OS setting, so it is not invented here.
        assert!(result.observations[0].fields.iter().all(|field| field.field_id != "allow_management_os"));
    }

    #[test]
    fn a_disk_attachment_is_keyed_by_the_machine_and_the_controller_position() {
        let result = hyperv_disks(&input("pcscope.virtualization.hyperv-disks", &format!("{VM_ID}/SCSI/0/1"), disk_payload()), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "virtualization.disk_attachment");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        for (key, expected) in [("vm_id", VM_ID), ("controller_type", "SCSI"), ("controller_number", "0"), ("controller_location", "1")] {
            assert!(identity.fields.iter().any(|field| field.key == key && field.value == expected), "{key} is a declared identity key");
        }
        // The `vm` field parallels the `vm_id` key, so it takes the identifier rather than the
        // mutable display name the same payload carries.
        assert_eq!(field_of(&result, "vm").value, Some(json!(VM_ID)));
        assert_eq!(field_of(&result, "path").value, Some(json!("C:\\示例\\disk.vhdx")));
        assert_eq!(field_of(&result, "controller_number").value, Some(json!(0)));
    }

    #[test]
    fn an_attachment_missing_a_controller_component_has_a_provisional_identity() {
        let mut payload = disk_payload();
        payload.as_object_mut().unwrap().remove("ControllerLocation");
        let result = hyperv_disks(&input("pcscope.virtualization.hyperv-disks", VM_ID, payload), &context());
        // No component is invented from the item id, which carries the same four values joined.
        assert_eq!(result.observations[0].identity.missing, vec!["controller_location".to_owned()]);
    }

    #[test]
    fn a_checkpoint_is_keyed_by_its_identifier_and_a_root_checkpoint_has_no_parent() {
        let result = hyperv_checkpoints(&input("pcscope.virtualization.hyperv-checkpoints", CHECKPOINT_ID, checkpoint_payload()), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "virtualization.checkpoint");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "native_id" && field.value == CHECKPOINT_ID));
        assert_eq!(field_of(&result, "checkpoint_id").value, Some(json!(CHECKPOINT_ID)));
        assert_eq!(field_of(&result, "creation_time").value, Some(json!("2026-09-11T10:00:00.0000000Z")));
        assert_eq!(field_of(&result, "type").value, Some(json!("Standard")));
        // A root checkpoint states a null parent; a null is an absence, not an empty parent.
        assert_eq!(field_of(&result, "parent").status, ValueStatus::NotProvided);

        // A checkpoint that does state one keeps it.
        let child = hyperv_checkpoints(
            &input(
                "pcscope.virtualization.hyperv-checkpoints",
                "44444444-5555-6666-7777-888888888888",
                json!({"Id": "44444444-5555-6666-7777-888888888888", "Name": "child", "ParentSnapshotId": CHECKPOINT_ID}),
            ),
            &context(),
        );
        assert_eq!(field_of(&child, "parent").value, Some(json!(CHECKPOINT_ID)));
    }

    #[test]
    fn a_wsl_distribution_is_keyed_by_its_registration_and_not_by_its_name() {
        let source = "pcscope.virtualization.wsl-registration";
        let result = wsl_registration(&input(source, &format!("CurrentUser/Registry64/{WSL_ROOT}\\{DISTRO_KEY}"), wsl_payload()), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "virtualization.wsl_distribution");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        // The registration's own subkey, not the DistributionName the type forbids substituting.
        assert!(identity.fields.iter().any(|field| field.key == "native_id" && field.value == DISTRO_KEY));
        assert_eq!(field_of(&result, "name").value, Some(json!("示例发行版")));
        assert_eq!(field_of(&result, "version").value, Some(json!(2)));
        assert_eq!(field_of(&result, "location").value, Some(json!("C:\\示例\\wsl")));
        assert_eq!(field_of(&result, "default_uid").value, Some(json!(1000)));

        // Two registrations with one name stay two objects.
        let other = wsl_payload();
        let mut other = other;
        other["path"] = json!(format!("{WSL_ROOT}\\{{99999999-0000-1111-2222-333344445555}}"));
        let other = wsl_registration(&input(source, "other", other), &context());
        assert_eq!(field_of(&other, "name").value, Some(json!("示例发行版")));
        assert_ne!(key_of(&identity), key_of(&other.observations[0].identity), "an identical name is not an identical registration");
    }

    #[test]
    fn a_wsl_registration_keeps_the_user_it_belongs_to() {
        let source = "pcscope.virtualization.wsl-registration";
        let payload = wsl_payload();
        let first = wsl_registration(&input(source, "distro", payload.clone()), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        let second = wsl_registration(&input(source, "distro", payload), &context_for("machine:M/user:S-1-5-21-77/schema:1/context:/shard:0"));
        assert_ne!(key_of(&first.observations[0].identity), key_of(&second.observations[0].identity), "registrations are per-user");
        assert!(first.observations[0].identity.scope.contains("user:S-1-5-21-9"));

        // The engine-owned sources, by contrast, keep machine-level objects as one.
        let vm = vm_payload();
        let by_system = hyperv_vms(&input("pcscope.virtualization.hyperv-vms", VM_ID, vm.clone()), &context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let by_user = hyperv_vms(&input("pcscope.virtualization.hyperv-vms", VM_ID, vm), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_eq!(key_of(&by_system.observations[0].identity), key_of(&by_user.observations[0].identity));
    }

    #[test]
    fn the_lxss_key_itself_is_not_a_distribution() {
        let source = "pcscope.virtualization.wsl-registration";
        let root = registry_item(
            "CurrentUser",
            "Registry64",
            WSL_ROOT,
            json!({"DefaultDistribution": value("String", DISTRO_KEY), "Version": value("DWord", 2)}),
        );
        let result = wsl_registration(&input(source, &format!("CurrentUser/Registry64/{WSL_ROOT}"), root), &context());
        // The key is legitimate material and describes no declared object, so it is skipped with a
        // visible reason rather than becoming a nameless distribution.
        match &result.outcome {
            AdapterOutcome::Skipped { reason } => assert!(reason.contains("Lxss key"), "{reason}"),
            other => panic!("expected a skipped outcome, got {other:?}"),
        }
        assert!(result.observations.is_empty(), "no object is invented for it");
    }

    #[test]
    fn a_wsl_item_that_is_not_a_registry_key_is_not_an_object() {
        let source = "pcscope.virtualization.wsl-registration";
        let result = wsl_registration(&input(source, "示例发行版", json!({"name": "示例发行版", "version": 2, "location": "C:\\示例\\wsl", "default_uid": 1000})), &context());
        assert!(matches!(result.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", result.outcome);
        // And a registry item with no path cannot form an identity at all.
        let pathless = wsl_registration(&input(source, "item", json!({"values": {}})), &context());
        assert!(matches!(pathless.outcome, AdapterOutcome::Failed { .. }), "{:?}", pathless.outcome);
    }

    #[test]
    fn a_wsl_value_table_the_item_did_not_present_as_a_table_is_unreadable() {
        let source = "pcscope.virtualization.wsl-registration";
        let payload = json!({"path": format!("{WSL_ROOT}\\{DISTRO_KEY}"), "hive": "CurrentUser", "view": "Registry64", "values": "not a table", "basis": "configuration snapshot"});
        let result = wsl_registration(&input(source, "distro", payload), &context());
        // The collector was reading the key; reporting four absent fields would read as a
        // distribution registered with nothing in it.
        for field_id in ["name", "version", "location", "default_uid"] {
            assert_eq!(field_of(&result, field_id).status, ValueStatus::Unreadable, "{field_id}");
        }
        assert_eq!(field_of(&result, "name").value, None);
    }

    #[test]
    fn the_docker_source_produces_the_type_the_kind_names() {
        let source = "pcscope.virtualization.docker";
        let cases = [
            ("container", "container", "virtualization.container", json!({"id": "abc", "names": "示例", "image": "nginx:latest", "state": "running", "status": "Up 2 hours"})),
            ("image", "image", "virtualization.image", json!({"id": "sha256:deadbeef", "repository": "library/nginx", "tag": "latest"})),
            ("volume", "volume", "virtualization.container_volume", json!({"id": "示例卷", "driver": "local"})),
            ("network", "network", "virtualization.container_network", json!({"id": "net1", "name": "示例网络", "driver": "nat", "scope": "local"})),
        ];
        for (kind, item_id, type_id, data) in cases {
            let result = docker(&input(source, &format!("{kind}/{item_id}"), docker_item(kind, data)), &context());
            assert_eq!(result.observations[0].identity.type_id, type_id, "{kind}");
            assert!(result.observations[0].identity.missing.is_empty(), "{kind}: {:?}", result.observations[0].identity.missing);
        }
    }

    #[test]
    fn a_container_and_a_volume_that_share_an_id_are_not_one_object() {
        let source = "pcscope.virtualization.docker";
        let container = docker(&input(source, "container/shared", docker_item("container", json!({"id": "shared"}))), &context());
        let volume = docker(&input(source, "volume/shared", docker_item("volume", json!({"id": "shared"}))), &context());
        // The kind decides the type, and the identity engine looks a native key up within a type,
        // so one id under two kinds is two objects rather than one object with two shapes.
        let type_of = |result: &AdapterResult| result.observations[0].identity.type_id.clone();
        assert_eq!(type_of(&container), "virtualization.container");
        assert_eq!(type_of(&volume), "virtualization.container_volume");
        assert_ne!(
            format!("{}|{}", type_of(&container), key_of(&container.observations[0].identity)),
            format!("{}|{}", type_of(&volume), key_of(&volume.observations[0].identity))
        );
    }

    #[test]
    fn an_image_is_keyed_by_its_repository_and_tag_as_well_as_its_id() {
        let source = "pcscope.virtualization.docker";
        // The repository is read out of the payload rather than split out of the item id, which the
        // emitter builds by joining the three with the same separator the repository itself uses.
        let result = docker(
            &input(source, "image/sha256:deadbeef/library/nginx/latest", docker_item("image", json!({"id": "sha256:deadbeef", "repository": "library/nginx", "tag": "latest"}))),
            &context(),
        );
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        for (key, expected) in [("image_id", "sha256:deadbeef"), ("repository", "library/nginx"), ("tag", "latest")] {
            assert!(identity.fields.iter().any(|field| field.key == key && field.value == expected), "{key}");
        }
        assert_eq!(field_of(&result, "repository").value, Some(json!("library/nginx")));

        // An untagged image states nulls, and none of them is filled with a placeholder.
        let untagged = docker(&input(source, "image/untagged", docker_item("image", json!({"id": "sha256:untagged", "repository": null, "tag": null}))), &context());
        assert_eq!(untagged.observations[0].identity.missing, vec!["repository".to_owned(), "tag".to_owned()]);
        assert_eq!(field_of(&untagged, "tag").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_volume_is_keyed_by_the_name_the_template_wrote_as_its_id() {
        let source = "pcscope.virtualization.docker";
        let result = docker(&input(source, "volume/示例卷", docker_item("volume", json!({"id": "示例卷", "driver": "local"}))), &context());
        assert_eq!(field_of(&result, "name").value, Some(json!("示例卷")));
        assert_eq!(field_of(&result, "driver").value, Some(json!("local")));
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "native_id" && field.value == "示例卷"));
    }

    #[test]
    fn a_docker_item_of_an_undeclared_kind_or_with_no_metadata_is_reported() {
        let source = "pcscope.virtualization.docker";
        let other = docker(&input(source, "service/web", docker_item("service", json!({"id": "web"}))), &context());
        match &other.outcome {
            AdapterOutcome::Skipped { reason } => assert!(reason.contains("service"), "{reason}"),
            outcome => panic!("expected a skipped outcome, got {outcome:?}"),
        }
        assert!(other.observations.is_empty(), "no declared type is forced onto it");

        let empty = docker(&input(source, "container/abc", json!({"kind": "container", "basis": "local Docker API metadata"})), &context());
        assert!(matches!(empty.outcome, AdapterOutcome::Failed { .. }), "{:?}", empty.outcome);

        let kindless = docker(&input(source, "abc", json!({"data": {"id": "abc"}})), &context());
        assert!(matches!(kindless.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", kindless.outcome);
    }

    #[test]
    fn the_platform_source_maps_both_of_its_item_shapes_to_one_type() {
        let source = "pcscope.virtualization.platform";
        let hypervisor = platform(
            &input(source, "hypervisor", json!({"hypervisor_present": true, "manufacturer": "示例厂商", "model": "示例型号"})),
            &context(),
        );
        let identity = &hypervisor.observations[0].identity;
        assert_eq!(identity.type_id, "virtualization.platform_capability");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "name" && field.value == "hypervisor"));
        assert_eq!(field_of(&hypervisor, "hypervisor_present").value, Some(json!(true)));
        // The other three flags are the processor items' facts, not this item's.
        for field_id in ["firmware_virtualization", "monitor_extensions", "slat"] {
            assert_eq!(field_of(&hypervisor, field_id).status, ValueStatus::NotProvided, "{field_id}");
        }

        // A processor item is keyed by the DeviceID the collector keyed the item by, and carries the
        // three capability flags; the monitor-extensions fact is under the collector's own key name.
        let processor = platform(
            &input(
                source,
                "CPU0",
                json!({"name": "示例处理器 3.60GHz", "firmware_virtualization": true, "vm_monitor_extensions": true, "slat": true}),
            ),
            &context(),
        );
        assert!(processor.observations[0].identity.fields.iter().any(|field| field.key == "name" && field.value == "CPU0"));
        assert_eq!(field_of(&processor, "hypervisor_present").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&processor, "firmware_virtualization").value, Some(json!(true)));
        assert_eq!(field_of(&processor, "monitor_extensions").value, Some(json!(true)));
        assert_eq!(field_of(&processor, "slat").value, Some(json!(true)));
    }

    #[test]
    fn two_processors_of_one_model_are_two_objects() {
        let source = "pcscope.virtualization.platform";
        // Hardware puts the model string in `name`, and every core of one model reports it the same.
        let payload = json!({"name": "示例处理器 3.60GHz", "firmware_virtualization": true, "vm_monitor_extensions": true, "slat": true});
        let first = platform(&input(source, "CPU0", payload.clone()), &context());
        let second = platform(&input(source, "CPU1", payload), &context());
        assert_ne!(key_of(&first.observations[0].identity), key_of(&second.observations[0].identity), "a duplicated name is not a merge");
    }

    #[test]
    fn the_same_material_produces_the_same_candidates_twice() {
        let source = "pcscope.virtualization.docker";
        let payload = docker_item("container", json!({"id": "abc", "names": "示例", "image": "nginx:latest", "state": "running"}));
        let once = key_of(&docker(&input(source, "container/abc", payload.clone()), &context()).observations[0].identity);
        let twice = key_of(&docker(&input(source, "container/abc", payload), &context()).observations[0].identity);
        assert_eq!(once, twice, "replaying the same material must decide the same thing");

        let wsl = wsl_payload();
        let once = key_of(&wsl_registration(&input("pcscope.virtualization.wsl-registration", "distro", wsl.clone()), &context()).observations[0].identity);
        let twice = key_of(&wsl_registration(&input("pcscope.virtualization.wsl-registration", "distro", wsl), &context()).observations[0].identity);
        assert_eq!(once, twice);
    }

    #[test]
    fn every_source_that_produces_an_object_validates_against_the_catalog() {
        let source = "pcscope.virtualization.docker";
        let cases: Vec<(&str, AdapterResult)> = vec![
            ("hyperv-vms", hyperv_vms(&input("pcscope.virtualization.hyperv-vms", VM_ID, vm_payload()), &context())),
            ("hyperv-switches", hyperv_switches(&input("pcscope.virtualization.hyperv-switches", SWITCH_ID, switch_payload()), &context())),
            ("hyperv-disks", hyperv_disks(&input("pcscope.virtualization.hyperv-disks", VM_ID, disk_payload()), &context())),
            ("hyperv-checkpoints", hyperv_checkpoints(&input("pcscope.virtualization.hyperv-checkpoints", CHECKPOINT_ID, checkpoint_payload()), &context())),
            ("wsl-registration", wsl_registration(&input("pcscope.virtualization.wsl-registration", "distro", wsl_payload()), &context())),
            ("docker", docker(&input(source, "container/abc", docker_item("container", json!({"id": "abc"}))), &context())),
            ("platform", platform(&input("pcscope.virtualization.platform", "hypervisor", json!({"hypervisor_present": true})), &context())),
        ];
        for (name, result) in cases {
            let violations = validate(&result, &context());
            assert!(violations.is_empty(), "{name}: {violations:?}");
            assert!(result.complete(), "{name} is a complete observation");
        }
    }

    #[test]
    fn every_declared_source_is_registered_under_this_category() {
        let declared: Vec<&str> = SOURCES.iter().map(|spec| spec.source).collect();
        for source in [
            "pcscope.virtualization.hyperv-vms",
            "pcscope.virtualization.hyperv-switches",
            "pcscope.virtualization.hyperv-disks",
            "pcscope.virtualization.hyperv-checkpoints",
            "pcscope.virtualization.wsl-registration",
            "pcscope.virtualization.docker",
            "pcscope.virtualization.platform",
        ] {
            assert!(declared.contains(&source), "{source} is declared");
            assert!(adapter_for(source).is_some(), "{source} has an adapter");
        }
        assert_eq!(declared.len(), 7);
    }
}
