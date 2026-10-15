//! S18 hardware, storage and TPM adapters.
//!
//! Checked against the emitters: `plugins/powershell-adapter/domains/storage.ps1` for the disks,
//! partitions, driver bindings, driver services, firmware, physical-disk health and battery;
//! `collect-system.ps1` for the device list and the `Win32_Volume` inventory;
//! `domains/security.ps1` for the TPM; and `plugins/usn/src/main.rs` for the volume discovery the
//! Rust USN plugin emits rather than PowerShell. The payload keys below are those emitters' own key
//! names, and they are not uniform: the hand-built items carry snake-case keys (`device_id`,
//! `file_system`, `capacity`) because the collector wrote them by hand with `[ordered]@{}`, while
//! the CIM items carry the class's own property names (`ObjectId`, `HealthStatus`, `PathName`)
//! because `Fields` copied them off the instance. Each source is read the way it writes.
//!
//! **Two of these types are reached by more than one source, and the sources do not carry the same
//! keys.**
//!
//!   * `hardware.disk` — `pcscope.storage.disks` (`Get-Disk`) and `pcscope.hardware.disk-health`
//!     (`Get-PhysicalDisk`). Both emitters record the storage subsystem's `ObjectId` as the item id,
//!     which is the native identifier this type's identity asks for. The disk view states the disk
//!     number, the partition style and the boot flags; the physical-disk view states none of those
//!     and states the media type and firmware version instead, for which the type declares no field.
//!     A field one source has no key for is absent *for that source*: this adapter never fills it in
//!     from the other's meaning and never defaults it to zero or false.
//!   * `hardware.volume` — `pcscope.storage.volumes` (`Win32_Volume`) and `pcscope.usn.volumes`.
//!     The first is a capacity record; the second is a diagnostic view of whether the volume's USN
//!     journal can be read, and it carries no capacity or volume type at all. Neither is completed
//!     from the other.
//!
//! Every type in this category belongs to the machine, so every identity here is built on
//! `installation_scope`: the account a collection ran as is *how* the material was obtained, not
//! part of *what* a disk, a device or the TPM belongs to. (`pcscope.usn.volumes` is the exception
//! worth naming: the USN plugin writes a drive-letter scope rather than a machine scope, and
//! `installation_scope` passes it through unchanged. Reconciling that with the PowerShell scope is
//! S21's, not this adapter's — inventing a machine scope here would be.)
//!
//! **No action is invented.** `hardware.disk` and `hardware.driver_service` declare actions, and
//! the snapshot sources adapted here carry no action evidence: a disk being read and a driver being
//! loaded are reported by event sources, which the event adapter handles. Nothing here turns a
//! snapshot into an action.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.hardware.devices", type_id: Some("hardware.device"), build: devices },
    SourceSpec { source: "pcscope.hardware.driver-bindings", type_id: Some("hardware.driver_binding"), build: driver_bindings },
    SourceSpec { source: "pcscope.hardware.driver-services", type_id: Some("hardware.driver_service"), build: driver_services },
    SourceSpec { source: "pcscope.hardware.firmware", type_id: Some("hardware.firmware"), build: firmware },
    SourceSpec { source: "pcscope.hardware.disk-health", type_id: Some("hardware.disk"), build: disk_health },
    SourceSpec { source: "pcscope.hardware.battery", type_id: Some("hardware.battery"), build: batteries },
    SourceSpec { source: "pcscope.storage.disks", type_id: Some("hardware.disk"), build: disks },
    SourceSpec { source: "pcscope.storage.partitions", type_id: Some("hardware.partition"), build: partitions },
    SourceSpec { source: "pcscope.storage.volumes", type_id: Some("hardware.volume"), build: volumes },
    SourceSpec { source: "pcscope.security.tpm", type_id: Some("hardware.tpm"), build: tpm },
    SourceSpec { source: "pcscope.usn.volumes", type_id: Some("hardware.volume"), build: usn_volumes },
];

// ---------- shared shapes ----------

/// Every source in this category describes its objects with a named-property table that `Fields` or
/// a hand-written `[ordered]@{}` built. An item whose `data` is not one of those — a list, a
/// scalar, a bare string — describes no object any of these types has, so the outcome says what the
/// item carried rather than mining it for something that looks like a field.
fn not_a_table(input: &EvidenceInput) -> Option<AdapterResult> {
    if input.payload.is_object() {
        return None;
    }
    Some(AdapterResult::not_an_object(format!(
        "item \"{}\" of {} carries a {} where this source's objects are named-property tables",
        input.item_id,
        input.source,
        value_type(&input.payload)
    )))
}

/// The storage subsystem's own identifier for a disk or a partition.
///
/// Both disk emitters and the partition emitter record `Item ([string]$_.ObjectId)`, so the
/// payload's `ObjectId` and the item id are the same string the storage stack published for the
/// device. Reading the payload key and falling back to the item id reads what the source stated;
/// nothing here rebuilds the identifier, and a disk with neither is a disk whose native id is
/// missing rather than one keyed on its display name.
fn object_id(input: &EvidenceInput) -> String {
    text(&input.payload, "ObjectId").unwrap_or_else(|| input.item_id.clone())
}

/// A field whose value the adapter states because the source it adapts is the thing that makes it
/// true, not because a payload key happened to be present.
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

/// A field this source has no key for at all.
///
/// The distinction it keeps: `size` is absent from the USN volume view because that source's shape
/// has no capacity key — there was nothing it could have read — whereas a key it has and could not
/// fill is `unreadable`. Absent is the honest answer for the first, and it is not a zero.
fn absent(field_id: &str, kind: FieldKind) -> FieldCandidate {
    field(&Value::Null, field_id, field_id, kind)
}

// ---------- hardware.disk, from two sources ----------

/// The fields `hardware.disk` declares, read from this source's own keys.
///
/// Both disk collectors write the CIM property names, so one mapping reads either. It is not a
/// merge of the two views: a key the payload does not carry resolves to `NotProvided`, which is
/// what makes the physical-disk view report no disk number and no boot flags rather than reporting
/// them as false.
fn disk_fields(payload: &Value) -> Vec<FieldCandidate> {
    vec![
        field(payload, "name", "FriendlyName", FieldKind::Configuration),
        field(payload, "number", "Number", FieldKind::Identity),
        field(payload, "bus_type", "BusType", FieldKind::Configuration),
        field(payload, "partition_style", "PartitionStyle", FieldKind::Configuration),
        field(payload, "size", "Size", FieldKind::Configuration),
        field(payload, "is_boot", "IsBoot", FieldKind::State),
        field(payload, "is_system", "IsSystem", FieldKind::State),
        field(payload, "health", "HealthStatus", FieldKind::State),
        field(payload, "operational_status", "OperationalStatus", FieldKind::State),
    ]
}

/// `Get-Disk`, one item per disk.
///
/// The item id is the disk's `ObjectId`, which is the type's native identifier and the same string
/// `hardware.disk-health` records for the physical disk behind it. `UniqueId`, `FriendlyName`,
/// `IsOffline` and `IsReadOnly` are collected and the type declares no field for them, so they stay
/// in the material.
fn disks(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("hardware.disk", &scope, &input.evidence.id, &[("native_id", Some(object_id(input)))]);
    object_candidate(input, context, identity, disk_fields(&input.payload))
}

/// `Get-PhysicalDisk`, one item per physical disk.
///
/// The same type as `storage.disks` and a different view of it: this one states the health and the
/// operational status of the physical device, and states neither the disk number, the partition
/// style nor the boot flags. Those four fields come out `NotProvided` here for that reason, and
/// nothing infers them. `MediaType` and `FirmwareVersion` are collected and undeclared, so they
/// stay in the material. The source states its own limit — health and operational status, not the
/// raw SMART attributes — and that statement travels with the material.
fn disk_health(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("hardware.disk", &scope, &input.evidence.id, &[("native_id", Some(object_id(input)))]);
    object_candidate(input, context, identity, disk_fields(&input.payload))
}

// ---------- storage ----------

/// `Get-Partition`, one item per partition.
///
/// The native identifier is the partition's `ObjectId`, the identifier the storage stack published
/// for it. `DiskId`, `Type`, `GptType`, `MbrType`, `IsActive`, `IsBoot`, `IsHidden` and `IsReadOnly`
/// are collected and the type declares no field for them; `AccessPaths` is declared, and it is
/// carried as the list the collector sent even when it holds one path.
fn partitions(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("hardware.partition", &scope, &input.evidence.id, &[("native_id", Some(object_id(input)))]);
    let fields = vec![
        field(&input.payload, "disk_number", "DiskNumber", FieldKind::Identity),
        field(&input.payload, "partition_number", "PartitionNumber", FieldKind::Identity),
        field(&input.payload, "guid", "Guid", FieldKind::Identity),
        field(&input.payload, "offset", "Offset", FieldKind::Configuration),
        field(&input.payload, "size", "Size", FieldKind::Configuration),
        field(&input.payload, "access_paths", "AccessPaths", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Win32_Volume`, one item per volume, from the system collection.
///
/// The native identifier is the volume's `DeviceID` — the `\\?\Volume{…}\` path the volume is
/// addressed by — and the drive letter is *not* it: the type's scope says in as many words that the
/// letter is a variable display attribute, so a volume that loses or gains a letter stays the same
/// volume, and a volume that has no letter at all (a mount-point-only volume) is not given one.
/// `label` is collected and undeclared. `capacity` is this source's word for the type's `size`;
/// carrying it across is reading the file system's own capacity, not estimating it.
fn volumes(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("hardware.volume", &scope, &input.evidence.id, &[("native_id", text(&input.payload, "device_id"))]);
    let fields = vec![
        field(&input.payload, "volume_id", "device_id", FieldKind::Identity),
        field(&input.payload, "drive_letter", "drive_letter", FieldKind::Configuration),
        field(&input.payload, "filesystem", "file_system", FieldKind::Configuration),
        field(&input.payload, "size", "capacity", FieldKind::Configuration),
        field(&input.payload, "volume_type", "drive_type", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `pcscope.usn.volumes`, the Rust plugin's discovery of the fixed drives that carry a journal.
///
/// This is a different fact about the same volume, not a second capacity reading. The plugin
/// enumerates the fixed drives that have a letter, and for each one it publishes the volume's GUID
/// path, its file system, and whether that file system is one a USN journal can live on. It
/// publishes no capacity and no volume type under any circumstance, so those two fields are absent
/// here — for the readable volume and for the unreadable one alike — rather than zero.
///
/// The drive letter is the item id the plugin assigned each item, and the plugin states its own
/// limit with the material: fixed drives with letters only, so a volume it does not list is a
/// volume outside that enumeration, never a volume that went away.
///
/// When the plugin could not read the volume it emits `{"readable": false, "error": …}` instead of
/// the volume's identity. That is a stated read failure and it is kept as one: the values the
/// source would have supplied — the volume id and the file system — are `unreadable`, the native id
/// the object would be keyed by is listed as *missing* (the drive letter is a display attribute and
/// does not stand in for it), and the plugin's own error text stays visible in the result. Turning
/// this into a volume with no capacity would make a permission failure look like an empty disk.
fn usn_volumes(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    // The plugin keys each volume item by the drive letter itself (`Item { id: drive, .. }`), so the
    // item id is where this source puts the letter — the payload of either shape carries no key for
    // it.
    let drive = input.item_id.clone();
    if input.payload.get("readable").and_then(Value::as_bool) == Some(false) {
        let identity = identity("hardware.volume", &scope, &input.evidence.id, &[("native_id", None)]);
        let fields = vec![
            unreadable("volume_id", FieldKind::Identity),
            known("drive_letter", FieldKind::Configuration, Value::String(drive)),
            unreadable("filesystem", FieldKind::Configuration),
            absent("size", FieldKind::Configuration),
            absent("volume_type", FieldKind::Configuration),
        ];
        let mut result = object_candidate(input, context, identity, fields);
        result.notes.push(match text(&input.payload, "error") {
            Some(error) => format!("the USN plugin states this volume could not be read: {error}"),
            None => "the USN plugin states this volume could not be read and sent no reason".to_owned(),
        });
        return result;
    }
    let identity = identity("hardware.volume", &scope, &input.evidence.id, &[("native_id", text(&input.payload, "volume"))]);
    let fields = vec![
        field(&input.payload, "volume_id", "volume", FieldKind::Identity),
        known("drive_letter", FieldKind::Configuration, Value::String(drive)),
        field(&input.payload, "filesystem", "filesystem", FieldKind::Configuration),
        // This source has no capacity key and no volume-type key at all: nothing to read, and
        // nothing to report as unreadable either.
        absent("size", FieldKind::Configuration),
        absent("volume_type", FieldKind::Configuration),
    ];
    // `eligible` — whether the file system is one a USN journal can live on — is collected and no
    // catalog type declares a field for it, so it stays in the material.
    object_candidate(input, context, identity, fields)
}

// ---------- devices, drivers, firmware, power ----------

/// `Win32_PnPEntity`, one item per device.
///
/// The collector keyed each item by `$_.DeviceID` — the PnP device instance id — and the payload
/// carries the display name, class, manufacturer, service and status. The device id is therefore
/// taken from the item id, which is where this source states it; taking it from the display name
/// instead is exactly what the type's `insufficient` rule forbids. `class_guid` is collected and
/// undeclared.
///
/// `status` and `error_code` are running state at the moment of the snapshot, which is what the
/// type declares them as. The source's own note — the config error code is a state field — travels
/// with the material.
fn devices(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let device_id = input.item_id.clone();
    let identity = identity("hardware.device", &scope, &input.evidence.id, &[("native_id", Some(device_id.clone()))]);
    let fields = vec![
        known("device_id", FieldKind::Identity, Value::String(device_id)),
        field(&input.payload, "name", "name", FieldKind::Configuration),
        field(&input.payload, "class", "class", FieldKind::Configuration),
        field(&input.payload, "manufacturer", "manufacturer", FieldKind::Configuration),
        field(&input.payload, "service", "service", FieldKind::Configuration),
        field(&input.payload, "status", "status", FieldKind::State),
        field(&input.payload, "config_error", "error_code", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Win32_PnPSignedDriver`, one item per device the driver is bound to.
///
/// The type's identity is a composite of the device and the driver version, so both are supplied
/// and neither is derived from the other: a device whose driver is updated to a new version is a
/// new binding, which is what the composite key says. The scope is passed for the record; the two
/// components the type names are what its key is built from. `Signer` and `DriverName` are
/// collected and undeclared.
fn driver_bindings(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let device = text(&input.payload, "DeviceID").unwrap_or_else(|| input.item_id.clone());
    let version = text(&input.payload, "DriverVersion");
    let identity = identity(
        "hardware.driver_binding",
        &scope,
        &input.evidence.id,
        &[("device_id", Some(device)), ("driver_version", version)],
    );
    let fields = vec![
        field(&input.payload, "name", "DeviceName", FieldKind::Configuration),
        field(&input.payload, "device", "DeviceID", FieldKind::Identity),
        field(&input.payload, "driver_version", "DriverVersion", FieldKind::Identity),
        field(&input.payload, "driver_date", "DriverDate", FieldKind::Configuration),
        field(&input.payload, "provider", "DriverProviderName", FieldKind::Configuration),
        field(&input.payload, "inf", "InfName", FieldKind::Configuration),
        field(&input.payload, "signed", "IsSigned", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Win32_SystemDriver`, one item per driver service, keyed by the name the collector keyed it by.
///
/// These are the same kernel services `startup.services` sees through `Win32_Service`, and the
/// catalog settles which type they belong to: they are driver services here and are not counted
/// twice as ordinary services. `DisplayName`, `Status` and `Started` are collected and undeclared.
///
/// The type declares an action — a driver being loaded — and a snapshot of a service's state is not
/// one: the action evidence comes from the driver-framework event channel, which the event adapter
/// handles. Nothing here reports a load from a `State` of `Running`.
fn driver_services(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let name = text(&input.payload, "Name").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("hardware.driver_service", &scope, &input.evidence.id, &[("name", Some(name))]);
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Identity),
        field(&input.payload, "driver_path", "PathName", FieldKind::Configuration),
        field(&input.payload, "service_type", "ServiceType", FieldKind::Configuration),
        field(&input.payload, "start_mode", "StartMode", FieldKind::Configuration),
        field(&input.payload, "state", "State", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Win32_BIOS`, one item per BIOS, keyed by the name the collector recorded.
///
/// `Version` is collected and undeclared: the type's two version fields are the BIOS version string
/// and the SMBIOS version, and `Version` is neither.
fn firmware(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    // The item id is `[string]$_.Name`, the BIOS name string the class publishes.
    let native = text(&input.payload, "Name").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("hardware.firmware", &scope, &input.evidence.id, &[("native_id", Some(native))]);
    let fields = vec![
        field(&input.payload, "manufacturer", "Manufacturer", FieldKind::Identity),
        field(&input.payload, "bios_version", "SMBIOSBIOSVersion", FieldKind::Configuration),
        smbios_version(&input.payload),
        field(&input.payload, "release_date", "ReleaseDate", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// The SMBIOS version, which `Win32_BIOS` states as the two numbers the specification is numbered
/// by.
///
/// The type declares one `smbios_version` where the class publishes a major and a minor, so the two
/// values the source gave are joined with the dot the version is written with. The field is marked
/// `decoded` rather than `source_provided` precisely because the string is assembled here: a reader
/// of the record can tell it apart from a string the source wrote. Nothing is looked up or
/// interpreted — both numbers are the source's own.
///
/// Half of a version is not a version. With either number missing or blank the field stays absent
/// instead of being completed with a zero.
fn smbios_version(payload: &Value) -> FieldCandidate {
    match (text(payload, "SMBIOSMajorVersion"), text(payload, "SMBIOSMinorVersion")) {
        (Some(major), Some(minor)) if !major.trim().is_empty() && !minor.trim().is_empty() => FieldCandidate {
            field_id: "smbios_version".into(),
            field_kind: FieldKind::Configuration,
            value: Some(Value::String(format!("{}.{}", major.trim(), minor.trim()))),
            value_type: "string".into(),
            status: ValueStatus::Known,
            source_time: None,
            derivation: Derivation::Decoded,
            conflicts: Vec::new(),
        },
        _ => absent("smbios_version", FieldKind::Configuration),
    }
}

/// `Win32_Battery`, one item per battery, keyed by the `DeviceID` the collector keyed it by.
///
/// `DesignCapacity`, `FullChargeCapacity`, `BatteryStatus` and `Status` are collected and
/// undeclared. The two fields that are declared are carried exactly as the class reports them:
/// `Chemistry` is a numeric code and `EstimatedRunTime` is minutes, including the sentinel value
/// Windows uses to mean "no estimate". Expanding a code or rewriting the sentinel would replace the
/// source's own statement with this adapter's reading of it, and the source states its own limit
/// with the material: capacities and an estimate, not a battery health history.
fn batteries(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let native = text(&input.payload, "DeviceID").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("hardware.battery", &scope, &input.evidence.id, &[("native_id", Some(native))]);
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Identity),
        field(&input.payload, "chemistry", "Chemistry", FieldKind::Configuration),
        field(&input.payload, "charge", "EstimatedChargeRemaining", FieldKind::State),
        field(&input.payload, "estimated_time", "EstimatedRunTime", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- security.tpm ----------

/// `Get-Tpm`, one item — the platform's TPM module.
///
/// The source is `pcscope.security.tpm` and the type it produces is `hardware.tpm`: a TPM is a
/// hardware device, and the security category reaches it through a security-oriented query. The
/// item id is the collector's own name for the object — the literal `tpm` — and the type's identity
/// is a name in the installation scope, so that is the name supplied: the material carries no
/// other, and the model name of the chip is not a substitute for it.
///
/// The identity key `name` is not one of the type's fields: the type declares the TPM's presence,
/// readiness and state and no field holding the name, so the name exists only as the key the object
/// is identified by, and no field is invented to carry it.
///
/// `TpmOwned` and `RestartPending` are collected and undeclared. `TpmPresent` is carried as the
/// boolean the source stated: a machine whose TPM is absent says so, and that is a value, not a
/// missing field — the remaining fields of such a record are the ones the source could not fill.
fn tpm(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = installation_scope(&context.collected_scope);
    let name = input.item_id.clone();
    let identity = identity("hardware.tpm", &scope, &input.evidence.id, &[("name", Some(name))]);
    let fields = vec![
        field(&input.payload, "present", "TpmPresent", FieldKind::State),
        field(&input.payload, "ready", "TpmReady", FieldKind::State),
        field(&input.payload, "enabled", "TpmEnabled", FieldKind::State),
        field(&input.payload, "activated", "TpmActivated", FieldKind::State),
        field(&input.payload, "manufacturer", "ManufacturerIdTxt", FieldKind::Configuration),
        field(&input.payload, "version", "ManufacturerVersion", FieldKind::Configuration),
        field(&input.payload, "locked_out", "LockedOut", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{validate, AdapterOutcome, EvidenceKind};
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

    /// One disk, as each of the two disk collectors describes it.
    fn disk_object_id() -> &'static str {
        "\\\\?\\scsi#disk&ven_example&prod_example#5&1b1&0&000000#{53f56307-b6bf-11d0-94f2-00a0c91efb8b}"
    }

    fn disk_view_payload() -> Value {
        json!({
            "ObjectId": disk_object_id(),
            "UniqueId": "EXAMPLE-DISK-0001",
            "Number": 0,
            "FriendlyName": "示例磁盘",
            "BusType": "NVMe",
            "PartitionStyle": "GPT",
            "Size": 512110190592i64,
            "IsBoot": true,
            "IsSystem": true,
            "IsOffline": false,
            "IsReadOnly": false,
            "OperationalStatus": [2],
            "HealthStatus": "Healthy"
        })
    }

    fn physical_disk_payload() -> Value {
        json!({
            "ObjectId": disk_object_id(),
            "FriendlyName": "示例磁盘",
            "MediaType": "SSD",
            "BusType": "NVMe",
            "Size": 512110190592i64,
            "HealthStatus": "Healthy",
            "OperationalStatus": [2],
            "FirmwareVersion": "EXAM001"
        })
    }

    fn usn_volume_payload() -> Value {
        json!({
            "volume": "\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\",
            "filesystem": "NTFS",
            "eligible": true,
            "scope": "fixed drives with drive letters only"
        })
    }

    /// A sample of each source in the category, so the mapping can be checked against the catalog
    /// for every one of them rather than for the one or two a test happens to open.
    fn samples() -> Vec<(&'static str, &'static str, Value)> {
        vec![
            (
                "pcscope.hardware.devices",
                "PCI\\VEN_8086&DEV_1234&SUBSYS_0000",
                json!({"name": "示例显示设备", "class": "Display", "class_guid": "{00000000-0000-0000-0000-000000000000}", "manufacturer": "示例制造商", "service": "igfx", "status": "OK", "error_code": 0}),
            ),
            (
                "pcscope.hardware.driver-bindings",
                "PCI\\VEN_8086&DEV_1234&SUBSYS_0000",
                json!({"DeviceID": "PCI\\VEN_8086&DEV_1234&SUBSYS_0000", "DeviceName": "示例显示设备", "DriverVersion": "31.0.101.4502", "DriverDate": "2026-01-01T00:00:00.0000000Z", "DriverProviderName": "示例制造商", "InfName": "oem12.inf", "IsSigned": true, "Signer": "CN=示例制造商", "DriverName": "示例驱动.sys"}),
            ),
            (
                "pcscope.hardware.driver-services",
                "示例驱动",
                json!({"Name": "示例驱动", "DisplayName": "示例驱动服务", "PathName": "C:\\Windows\\System32\\drivers\\示例.sys", "ServiceType": "Kernel Driver", "StartMode": "Manual", "State": "Stopped", "Status": "OK", "Started": false}),
            ),
            (
                "pcscope.hardware.firmware",
                "BIOS Date: 01/01/26 00:00:00 Ver: 1.15.0",
                json!({"Manufacturer": "示例制造商", "Name": "BIOS Date: 01/01/26 00:00:00 Ver: 1.15.0", "SMBIOSBIOSVersion": "1.15.0", "SMBIOSMajorVersion": 3, "SMBIOSMinorVersion": 4, "ReleaseDate": "2026-01-01T00:00:00.0000000Z", "Version": "EXAMPLE - 1"}),
            ),
            ("pcscope.hardware.disk-health", disk_object_id(), physical_disk_payload()),
            (
                "pcscope.hardware.battery",
                "BAT0",
                json!({"DeviceID": "BAT0", "Name": "示例电池", "Status": "OK", "BatteryStatus": 2, "Chemistry": 6, "DesignCapacity": 50000, "FullChargeCapacity": 48000, "EstimatedChargeRemaining": 87, "EstimatedRunTime": 71582788}),
            ),
            ("pcscope.storage.disks", disk_object_id(), disk_view_payload()),
            (
                "pcscope.storage.partitions",
                "\\\\?\\scsi#disk&ven_example#5&1b1&0&000000#{53f56307-b6bf-11d0-94f2-00a0c91efb8b}#1",
                json!({"ObjectId": "\\\\?\\scsi#disk&ven_example#5&1b1&0&000000#{53f56307-b6bf-11d0-94f2-00a0c91efb8b}#1", "DiskId": "\\\\?\\scsi#disk&ven_example#5&1b1&0&000000", "DiskNumber": 0, "PartitionNumber": 1, "Guid": "{00000000-0000-0000-0000-000000000001}", "Offset": 1048576i64, "Size": 53687091200i64, "Type": "Basic", "GptType": "{ebd0a0a2-b9e5-4433-87c0-68b6b72699c7}", "MbrType": 0, "AccessPaths": ["C:\\"], "IsActive": true, "IsBoot": false, "IsHidden": false, "IsReadOnly": false}),
            ),
            (
                "pcscope.storage.volumes",
                "\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\",
                json!({"device_id": "\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\", "drive_letter": "C:", "label": "示例卷", "file_system": "NTFS", "capacity": 53687091200i64, "drive_type": 3}),
            ),
            (
                "pcscope.security.tpm",
                "tpm",
                json!({"TpmPresent": true, "TpmReady": true, "TpmEnabled": true, "TpmActivated": true, "TpmOwned": true, "RestartPending": false, "ManufacturerIdTxt": "EXAMPLE", "ManufacturerVersion": "1.2", "LockedOut": false}),
            ),
            ("pcscope.usn.volumes", "C:", usn_volume_payload()),
            // The same source on a volume it could not read: still an object, and one whose fields
            // say the read failed rather than that the volume is empty.
            ("pcscope.usn.volumes", "D:", json!({"readable": false, "error": "Access is denied. (os error 5)"})),
        ]
    }

    #[test]
    fn the_two_disk_sources_key_the_same_disk_the_same_way() {
        let context = context();
        let disk = disks(&input("pcscope.storage.disks", disk_object_id(), disk_view_payload()), &context);
        let health = disk_health(&input("pcscope.hardware.disk-health", disk_object_id(), physical_disk_payload()), &context);
        // One disk, two views: the storage ObjectId is the native identifier both sources state, so
        // the identity engine sees one object.
        assert_eq!(key_of(&disk.observations[0].identity), key_of(&health.observations[0].identity));
        assert!(disk.observations[0].identity.missing.is_empty(), "{:?}", disk.observations[0].identity.missing);
        assert_eq!(disk.observations[0].identity.type_id, "hardware.disk");
        assert_eq!(health.observations[0].identity.type_id, "hardware.disk");
        // The fields both views carry agree for the same disk.
        for field_id in ["bus_type", "size", "health", "operational_status"] {
            assert_eq!(field_of(&disk, field_id).value, field_of(&health, field_id).value, "{field_id}");
        }
    }

    #[test]
    fn the_physical_disk_view_states_no_number_partition_style_or_boot_flag() {
        let result = disk_health(&input("pcscope.hardware.disk-health", disk_object_id(), physical_disk_payload()), &context());
        // Get-PhysicalDisk has no key for these four: they are absent from this source, not false
        // and not zero, and the disk view's values for them are not borrowed.
        for field_id in ["number", "partition_style", "is_boot", "is_system"] {
            let field = field_of(&result, field_id);
            assert_eq!(field.status, ValueStatus::NotProvided, "{field_id}");
            assert!(field.value.is_none(), "{field_id}");
        }
        // What it does state stays stated.
        assert_eq!(field_of(&result, "health").value, Some(json!("Healthy")));
        assert_eq!(field_of(&result, "size").value, Some(json!(512110190592i64)));

        let disk = disks(&input("pcscope.storage.disks", disk_object_id(), disk_view_payload()), &context());
        assert_eq!(field_of(&disk, "number").value, Some(json!(0)));
        assert_eq!(field_of(&disk, "is_boot").value, Some(json!(true)));
        assert_eq!(field_of(&disk, "partition_style").value, Some(json!("GPT")));
    }

    #[test]
    fn a_partition_is_keyed_by_the_storage_object_id_and_carries_its_numbers_and_paths() {
        let payload = json!({"ObjectId": "\\\\?\\scsi#disk#0#1", "DiskNumber": 0, "PartitionNumber": 1, "Guid": "{00000000-0000-0000-0000-000000000001}", "Offset": 1048576i64, "Size": 53687091200i64, "AccessPaths": ["C:\\"], "IsBoot": false});
        let result = partitions(&input("pcscope.storage.partitions", "\\\\?\\scsi#disk#0#1", payload), &context());
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "native_id" && field.value == "\\\\?\\scsi#disk#0#1"));
        assert!(result.observations[0].identity.missing.is_empty());
        assert_eq!(field_of(&result, "disk_number").value, Some(json!(0)));
        assert_eq!(field_of(&result, "partition_number").value, Some(json!(1)));
        // The paths are carried as the list the collector sent.
        assert_eq!(field_of(&result, "access_paths").value, Some(json!(["C:\\"])));
    }

    #[test]
    fn a_volume_carries_its_capacity_under_the_key_this_collector_wrote() {
        let payload = json!({"device_id": "\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\", "drive_letter": "C:", "label": "示例卷", "file_system": "NTFS", "capacity": 53687091200i64, "drive_type": 3});
        let result = volumes(&input("pcscope.storage.volumes", "\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\", payload), &context());
        assert_eq!(field_of(&result, "size").value, Some(json!(53687091200i64)));
        assert_eq!(field_of(&result, "filesystem").value, Some(json!("NTFS")));
        assert_eq!(field_of(&result, "volume_type").value, Some(json!(3)));
        assert_eq!(field_of(&result, "volume_id").value, Some(json!("\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\")));
    }

    #[test]
    fn a_volume_without_a_drive_letter_is_not_given_one() {
        // A mount-point-only volume: Win32_Volume states the letter as null, and a letter is not
        // invented from the volume path.
        let payload = json!({"device_id": "\\\\?\\Volume{00000000-0000-0000-0000-0000000000d0}\\", "drive_letter": null, "label": null, "file_system": "NTFS", "capacity": 1048576i64, "drive_type": 3});
        let result = volumes(&input("pcscope.storage.volumes", "\\\\?\\Volume{00000000-0000-0000-0000-0000000000d0}\\", payload), &context());
        assert_eq!(field_of(&result, "drive_letter").status, ValueStatus::NotProvided);
        assert!(field_of(&result, "drive_letter").value.is_none());
        // The volume is still identified by its own id.
        assert!(result.observations[0].identity.missing.is_empty());
    }

    #[test]
    fn the_usn_view_carries_no_capacity_and_the_two_volume_sources_agree_on_the_identifier() {
        let context = context();
        let usn = usn_volumes(&input("pcscope.usn.volumes", "C:", usn_volume_payload()), &context);
        // This source has no capacity key and no volume-type key at all: nothing was read and
        // nothing failed, so the fields are absent rather than zero.
        for field_id in ["size", "volume_type"] {
            assert_eq!(field_of(&usn, field_id).status, ValueStatus::NotProvided, "{field_id}");
        }
        assert_eq!(field_of(&usn, "filesystem").value, Some(json!("NTFS")));
        // The drive letter is the item id the plugin assigned, and it is a display attribute.
        assert_eq!(field_of(&usn, "drive_letter").value, Some(json!("C:")));

        let volumes_result = volumes(
            &input(
                "pcscope.storage.volumes",
                "\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\",
                json!({"device_id": "\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\", "drive_letter": "C:", "file_system": "NTFS", "capacity": 53687091200i64, "drive_type": 3}),
            ),
            &context,
        );
        // The volume GUID path is the same volume both sources name, so the two views resolve to
        // one object rather than two.
        assert_eq!(key_of(&usn.observations[0].identity), key_of(&volumes_result.observations[0].identity));
    }

    #[test]
    fn a_volume_the_usn_plugin_could_not_read_is_unreadable_rather_than_capacity_free() {
        let payload = json!({"readable": false, "error": "Access is denied. (os error 5)"});
        let result = usn_volumes(&input("pcscope.usn.volumes", "D:", payload), &context());
        // The read failure is stated, not smoothed away.
        assert_eq!(field_of(&result, "volume_id").status, ValueStatus::Unreadable);
        assert_eq!(field_of(&result, "filesystem").status, ValueStatus::Unreadable);
        assert!(field_of(&result, "volume_id").value.is_none());
        // The capacity this source never carries is still absent, not unreadable and not zero.
        assert_eq!(field_of(&result, "size").status, ValueStatus::NotProvided);
        // The native id is the one thing that cannot be supplied, and it is listed as missing
        // rather than replaced by the drive letter, which is a display attribute.
        assert_eq!(result.observations[0].identity.missing, vec!["native_id".to_owned()]);
        assert!(!result.observations[0].identity.fields.iter().any(|field| field.key == "native_id"));
        assert_eq!(field_of(&result, "drive_letter").value, Some(json!("D:")));
        // And the plugin's own error stays readable.
        assert!(result.notes.iter().any(|note| note.contains("Access is denied")), "{:?}", result.notes);
    }

    #[test]
    fn a_device_is_keyed_by_the_device_id_the_collector_recorded_it_under() {
        let payload = json!({"name": "示例显示设备", "class": "Display", "class_guid": "{00000000-0000-0000-0000-000000000000}", "manufacturer": "示例制造商", "service": "igfx", "status": "OK", "error_code": 28});
        let item_id = "PCI\\VEN_8086&DEV_1234&SUBSYS_0000";
        let result = devices(&input("pcscope.hardware.devices", item_id, payload), &context());
        // The device id is the identifier the collector keyed the item by, not its display name.
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "native_id" && field.value == item_id));
        assert_eq!(field_of(&result, "device_id").value, Some(json!(item_id)));
        assert_eq!(field_of(&result, "name").value, Some(json!("示例显示设备")));
        assert_eq!(field_of(&result, "class").value, Some(json!("Display")));
        assert_eq!(field_of(&result, "config_error").value, Some(json!(28)));
    }

    #[test]
    fn a_driver_binding_is_keyed_by_the_device_and_the_driver_version() {
        let payload = json!({"DeviceID": "PCI\\VEN_8086&DEV_1234&SUBSYS_0000", "DeviceName": "示例显示设备", "DriverVersion": "31.0.101.4502", "DriverDate": "2026-01-01T00:00:00.0000000Z", "DriverProviderName": "示例制造商", "InfName": "oem12.inf", "IsSigned": true, "Signer": "CN=示例制造商"});
        let context = context();
        let result = driver_bindings(&input("pcscope.hardware.driver-bindings", "PCI\\VEN_8086&DEV_1234&SUBSYS_0000", payload), &context);
        let identity = &result.observations[0].identity;
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "device_id" && field.value == "PCI\\VEN_8086&DEV_1234&SUBSYS_0000"));
        assert!(identity.fields.iter().any(|field| field.key == "driver_version" && field.value == "31.0.101.4502"));
        assert_eq!(field_of(&result, "signed").value, Some(json!(true)));
        assert_eq!(field_of(&result, "inf").value, Some(json!("oem12.inf")));
        assert_eq!(field_of(&result, "provider").value, Some(json!("示例制造商")));

        // A binding whose version the source did not state is one whose composite identity is
        // incomplete: the missing component is listed, and the device is not used in its place.
        let without_version = driver_bindings(&input("pcscope.hardware.driver-bindings", "PCI\\VEN_8086&DEV_1234&SUBSYS_0000", json!({"DeviceID": "PCI\\VEN_8086&DEV_1234&SUBSYS_0000"})), &context);
        assert_eq!(without_version.observations[0].identity.missing, vec!["driver_version".to_owned()]);
    }

    #[test]
    fn a_driver_service_is_keyed_by_its_name() {
        let payload = json!({"Name": "示例驱动", "DisplayName": "示例驱动服务", "PathName": "C:\\Windows\\System32\\drivers\\示例.sys", "ServiceType": "Kernel Driver", "StartMode": "Manual", "State": "Stopped", "Status": "OK", "Started": false});
        let result = driver_services(&input("pcscope.hardware.driver-services", "示例驱动", payload), &context());
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "name" && field.value == "示例驱动"));
        assert_eq!(field_of(&result, "driver_path").value, Some(json!("C:\\Windows\\System32\\drivers\\示例.sys")));
        assert_eq!(field_of(&result, "start_mode").value, Some(json!("Manual")));
        assert_eq!(field_of(&result, "state").value, Some(json!("Stopped")));
        // A snapshot of a service's state is not the load action the type declares.
        assert!(result.actions.is_empty());
    }

    #[test]
    fn firmware_states_the_smbios_version_as_the_two_numbers_the_source_gave() {
        let payload = json!({"Manufacturer": "示例制造商", "Name": "示例 BIOS", "SMBIOSBIOSVersion": "1.15.0", "SMBIOSMajorVersion": 3, "SMBIOSMinorVersion": 4, "ReleaseDate": "2026-01-01T00:00:00.0000000Z"});
        let result = firmware(&input("pcscope.hardware.firmware", "示例 BIOS", payload), &context());
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "native_id" && field.value == "示例 BIOS"));
        assert_eq!(field_of(&result, "bios_version").value, Some(json!("1.15.0")));
        assert_eq!(field_of(&result, "smbios_version").value, Some(json!("3.4")));
        // The string is assembled from the source's two numbers, and says so.
        assert_eq!(field_of(&result, "smbios_version").derivation, Derivation::Decoded);
    }

    #[test]
    fn a_half_stated_smbios_version_is_not_completed() {
        let payload = json!({"Manufacturer": "示例制造商", "Name": "示例 BIOS", "SMBIOSBIOSVersion": "1.15.0", "SMBIOSMajorVersion": 3});
        let result = firmware(&input("pcscope.hardware.firmware", "示例 BIOS", payload), &context());
        let smbios = field_of(&result, "smbios_version");
        // Half a version is not a version, and the missing half is not filled with a zero.
        assert_eq!(smbios.status, ValueStatus::NotProvided);
        assert!(smbios.value.is_none());
    }

    #[test]
    fn a_tpm_is_keyed_by_the_name_its_collector_recorded_it_under() {
        let payload = json!({"TpmPresent": true, "TpmReady": false, "TpmEnabled": true, "TpmActivated": false, "TpmOwned": true, "RestartPending": true, "ManufacturerIdTxt": "EXAMPLE", "ManufacturerVersion": "1.2", "LockedOut": true});
        let result = tpm(&input("pcscope.security.tpm", "tpm", payload), &context());
        // The security source produces the hardware type, and the name in the material is the item
        // id the collector assigned.
        assert_eq!(result.observations[0].identity.type_id, "hardware.tpm");
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "name" && field.value == "tpm"));
        assert_eq!(field_of(&result, "present").value, Some(json!(true)));
        assert_eq!(field_of(&result, "ready").value, Some(json!(false)));
        assert_eq!(field_of(&result, "locked_out").value, Some(json!(true)));
        assert_eq!(field_of(&result, "version").value, Some(json!("1.2")));
        // The name is the identity key and not a field: the type declares none, so none is made up.
        assert!(result.observations[0].fields.iter().all(|field| field.field_id != "name"));
    }

    #[test]
    fn a_battery_carries_the_values_the_source_stated_verbatim() {
        let payload = json!({"DeviceID": "BAT0", "Name": "示例电池", "Status": "OK", "BatteryStatus": 2, "Chemistry": 6, "DesignCapacity": 50000, "FullChargeCapacity": 48000, "EstimatedChargeRemaining": 87, "EstimatedRunTime": 71582788});
        let result = batteries(&input("pcscope.hardware.battery", "BAT0", payload), &context());
        assert!(result.observations[0].identity.fields.iter().any(|field| field.key == "native_id" && field.value == "BAT0"));
        assert_eq!(field_of(&result, "charge").value, Some(json!(87)));
        // The numeric chemistry code and the runtime sentinel are the class's own values; neither
        // is expanded into this adapter's reading of it.
        assert_eq!(field_of(&result, "chemistry").value, Some(json!(6)));
        assert_eq!(field_of(&result, "estimated_time").value, Some(json!(71582788)));
    }

    #[test]
    fn material_that_is_not_an_object_keeps_a_visible_reason() {
        // A source whose item is a bare list describes no object these types have. The outcome says
        // so, with the shape the item carried, instead of mining it for a field.
        let listed = usn_volumes(&input("pcscope.usn.volumes", "C:", json!(["C:", "NTFS"])), &context());
        assert!(matches!(listed.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", listed.outcome);
        assert!(listed.observations.is_empty());
        match listed.outcome {
            AdapterOutcome::NotAnObject { reason } => {
                assert!(reason.contains("pcscope.usn.volumes"), "{reason}");
                assert!(reason.contains("list"), "{reason}");
            }
            other => panic!("expected a non-object outcome, got {other:?}"),
        }

        let scalar = disk_health(&input("pcscope.hardware.disk-health", "0", json!(42)), &context());
        assert!(matches!(scalar.outcome, AdapterOutcome::NotAnObject { .. }));
    }

    #[test]
    fn every_field_every_source_maps_is_declared_by_the_type_it_names() {
        let context = context();
        for (source, item_id, payload) in samples() {
            let spec = spec_for(source).expect("the source is declared in the category registry");
            let type_id = spec.type_id.expect("every source in this category produces one type");
            let result = (spec.build)(&input(source, item_id, payload), &context);
            assert_eq!(result.outcome, AdapterOutcome::Adapted, "{source} did not adapt its material");
            assert_eq!(result.observations.len(), 1, "{source}");
            assert_eq!(result.observations[0].identity.type_id, type_id, "{source}");
            assert!(result.complete(), "{source} produced an incomplete result");
            // The catalog is the authority on both counts: a field a type does not declare, or an
            // identity key neither supplied nor listed as missing, is rejected here.
            let violations = validate(&result, &context);
            assert!(violations.is_empty(), "{source}: {violations:?}");
        }
    }

    #[test]
    fn the_same_material_produces_the_same_identity_twice() {
        let context = context();
        let once = key_of(&disks(&input("pcscope.storage.disks", disk_object_id(), disk_view_payload()), &context).observations[0].identity);
        let twice = key_of(&disks(&input("pcscope.storage.disks", disk_object_id(), disk_view_payload()), &context).observations[0].identity);
        assert_eq!(once, twice, "replaying the same material must decide the same thing");

        let unreadable_once = key_of(&usn_volumes(&input("pcscope.usn.volumes", "D:", json!({"readable": false, "error": "denied"})), &context).observations[0].identity);
        let unreadable_twice = key_of(&usn_volumes(&input("pcscope.usn.volumes", "D:", json!({"readable": false, "error": "denied"})), &context).observations[0].identity);
        assert_eq!(unreadable_once, unreadable_twice);
    }
}
