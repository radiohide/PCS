//! S17 security adapters.
//!
//! Checked against the emitters in `plugins/powershell-adapter/domains/security.ps1`, whose `Emit`
//! declarations name every source here, and whose payload keys — `Item`'s hand-written table, the
//! CIM property names `Fields` copied off the instance, the certificate stores the emitter walks —
//! are the ones mapped below. Five things about these sources decide the shape of the module.
//!
//! **One type is reached by several sources, and they do not state the same fields.**
//! `security.protection` is fed by `pcscope.security.protection` (the engine's own status) and by
//! `pcscope.security.protection-policy` (the product's configured policy). A field one of them has
//! no key for is absent *for that source*: the policy source's record states no signature version,
//! and the status source's record states no exclusion list. Neither is completed from the other,
//! and neither is defaulted. The two sources also name their item differently (`defender` and
//! `defender-policy`), and that item id is the only name either material carries, so each record
//! keeps the name its own source gave it rather than this adapter merging two records into one
//! object on a name neither source wrote.
//!
//! **One source covers two types, expanded item by item.** `pcscope.security.protection-policy`
//! emits a single item whose `ExclusionPath`, `ExclusionProcess` and `ExclusionExtension` keys are
//! lists. The item is one `security.protection` record — the product whose policy it is — plus one
//! `security.exclusion` object per entry in those lists, which is the expansion the source map
//! records as this step's work. The policy switches that have no declared field (`PUAProtection`,
//! `DisableScriptScanning`, `DisableBehaviorMonitoring`, `DisableIOAVProtection`) stay in the
//! material.
//!
//! **Two sources are one type, and one of them is per-user.** Both certificate sources emit the
//! same item shape and target `security.certificate_registration`; what separates them is the store
//! location, which the payload states and which also decides the scope. The local machine's store
//! belongs to the installation, so the collecting account is dropped; the current user's store is
//! that user's own, so the scope it was handed — user segment included — is what it keeps. That is
//! the per-type narrowing shared change 0005 left open, and it is the reason the two stores of the
//! same certificate are two registrations rather than one object.
//!
//! Detection material produces timestamped actions and a historical detection subject.
//! Detection-list retention never changes the threat response or a protection product's state.
//!
//! **Two fields are assembled or decoded, and both say so.** `scan_time` is the later of the two
//! scan end times the status source states, and the policy source's `real_time` is read from its
//! `DisableRealtimeMonitoring` switch. Both are marked `Derivation::Decoded` so a reader can tell
//! them from a string the source wrote verbatim.
//!
//! **A field the material says could not be read is `unreadable`, not absent.** The security
//! sources are the category where reads fail most: BitLocker, Secure Boot and Device Guard are
//! permission-limited, and `Get-BitLockerVolume` in particular is denied on most machines. The
//! collector's own response to a denied read is to fail the whole source and send a gap rather than
//! a half-filled item (the `Emit` wrapper catches, and a gap is collection status, not this
//! adapter's input) — so a declared field whose value is missing and whose read the material states
//! as failed is kept as a failure here, and only genuine silence becomes absence. Nothing in this
//! module turns a denied read into an unencrypted volume, a TPM-less machine or a switched-off
//! setting.
//!
//! **No action is invented.** No type in this category declares one in the catalog, and nothing
//! here reports a snapshot as one.
use super::*;
use crate::semantics::{canonical_time, compare_canonical};

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.security.protection", type_id: Some("security.protection"), build: protection },
    // Two declared types behind one item: the product's policy record and one object per exclusion.
    SourceSpec { source: "pcscope.security.protection-policy", type_id: None, build: protection_policy },
    SourceSpec { source: "pcscope.security.products", type_id: Some("security.security_product"), build: security_products },
    // Action evidence: the material describes detections, and no declared object reads them.
    SourceSpec { source: "pcscope.security.detections", type_id: None, build: detections },
    SourceSpec { source: "pcscope.security.deviceguard", type_id: Some("security.device_guard_configuration"), build: device_guard },
    SourceSpec { source: "pcscope.security.secureboot", type_id: Some("security.secure_boot_configuration"), build: secure_boot },
    SourceSpec { source: "pcscope.security.certificates.currentuser", type_id: Some("security.certificate_registration"), build: certificates },
    SourceSpec { source: "pcscope.security.certificates.localmachine", type_id: Some("security.certificate_registration"), build: certificates },
    SourceSpec { source: "pcscope.security.encryption", type_id: Some("security.bitlocker_configuration"), build: bitlocker_volumes },
    SourceSpec { source: "pcscope.security.ssh-public-keys", type_id: Some("security.ssh_public_key"), build: ssh_public_keys },
];

// ---------- shared shapes ----------

/// Every source in this category describes its object with a named-property table — a hand-written
/// `[ordered]@{}` or one `Fields` built off a CIM instance. An item whose `data` is not one of those
/// describes no object any of these types has, so the outcome states what the item carried instead
/// of mining it for something that looks like a field.
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

/// A field this source states in no form this mapping can read.
///
/// The distinction it keeps from `unreadable`: nothing was read and nothing failed — the material
/// simply holds no value for the field. A field whose value is absent must never be filled with the
/// shape of something else the payload happens to carry.
fn absent(field_id: &str, kind: FieldKind) -> FieldCandidate {
    field(&Value::Null, field_id, field_id, kind)
}

/// A field whose value this adapter states because the source's own text is where it read it.
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

/// A field computed from values the source stated — assembled, selected or inverted here, never
/// written by the source in that form. Marked so the derivation is visible in the record.
fn decoded(field_id: &str, kind: FieldKind, value: Value) -> FieldCandidate {
    FieldCandidate { derivation: Derivation::Decoded, ..known(field_id, kind, value) }
}

/// Whether the persisted material states that this field could not be read.
///
/// `decoding.unreadable_fields` is the contract's place for exactly that statement. The v1 protocol
/// has no envelope field for a per-field read failure, so in today's material this list is always
/// empty and a denied read arrives as a source-level gap (S05 records both facts) — but the
/// distinction it carries is the one this category most needs, so where the statement does exist it
/// is honoured rather than being flattened into "the source did not say".
fn stated_unreadable(input: &EvidenceInput, field_id: &str) -> bool {
    input.decoding.unreadable_fields.iter().any(|stated| stated == field_id)
}

/// One declared field, read from the payload, with a stated read failure kept as one.
///
/// A value the material does carry wins: a field with a value is not a field that could not be read,
/// and the failure statement is only consulted where the material is otherwise silent. Absent and
/// unreadable are then two different answers, and only the second one is what a denied read
/// produces.
fn stated_field(input: &EvidenceInput, field_id: &str, key: &str, kind: FieldKind) -> FieldCandidate {
    let mapped = field(&input.payload, field_id, key, kind);
    if mapped.status != ValueStatus::NotProvided || !stated_unreadable(input, field_id) {
        return mapped;
    }
    unreadable(field_id, kind)
}

/// The scope a machine-level object belongs to: the collecting account is *how* the material was
/// obtained, not part of *what* the object belongs to. Every source here is machine-level except
/// the ones that say otherwise below.
fn machine_scope(context: &AdapterContext) -> String {
    installation_scope(&context.collected_scope)
}

/// The scope a genuinely per-user object belongs to: the collecting account is part of what the
/// object belongs to, so the scope is taken exactly as handed.
fn user_scope(context: &AdapterContext) -> String {
    context.collected_scope.trim().to_owned()
}

/// The name of an object the material names only through the item id its collector assigned.
///
/// Three named-entity records here are named this way — the product behind a Defender query, the
/// product whose policy was read, and the Secure Boot component. The payloads carry no product or
/// component name at all (`AMProductVersion` is a version, not a name), and the collector keyed each
/// item by the only name it has. Reading the name off the item id is reading what the source said;
/// naming a product after one of its version strings, or after this adapter's idea of what the
/// product is called, would not be.
fn item_name(input: &EvidenceInput) -> String {
    input.item_id.clone()
}

// ---------- pcscope.security.protection ----------

/// `Get-MpComputerStatus`, one item — the protection engine's own status.
///
/// The item is one object of `security.protection`, named by the item id the collector assigned it
/// (`defender`), which is also what the type's `name` field carries: the type declares that field
/// and the identity is built on it, so the name is stated once and used twice rather than being an
/// identity component the record cannot show.
///
/// `AMEngineVersion` is the engine version and `AntivirusSignatureVersion` the signature version,
/// which is what the type's two version fields name. The rest of the status the source reports — the
/// antispyware, antivirus, behaviour-monitoring, IOAV, NIS, on-access and service switches, plus
/// `AMProductVersion` and `AntivirusSignatureLastUpdated` — has no declared field and stays in the
/// material; none of them is folded into `real_time` or stretched to fit `scan_time`.
fn protection(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = machine_scope(context);
    let name = item_name(input);
    let identity = identity("security.protection", &scope, &input.evidence.id, &[("name", Some(name.clone()))]);
    let fields = vec![
        known("name", FieldKind::Identity, Value::String(name)),
        stated_field(input, "engine_version", "AMEngineVersion", FieldKind::State),
        stated_field(input, "signature_version", "AntivirusSignatureVersion", FieldKind::State),
        stated_field(input, "real_time", "RealTimeProtectionEnabled", FieldKind::State),
        stated_field(input, "tamper_protected", "IsTamperProtected", FieldKind::State),
        scan_time(input),
    ];
    let mut result = object_candidate(input, context, identity, fields);
    if let Some(note) = scan_time_note(input) {
        result.notes.push(note);
    }
    result
}

/// The two scan end times the status source states, in the order it states them.
fn scan_end_times(input: &EvidenceInput) -> Vec<String> {
    ["QuickScanEndTime", "FullScanEndTime"]
        .iter()
        .filter_map(|key| text(&input.payload, key))
        .filter(|value| !value.trim().is_empty())
        .collect()
}

/// The most recent scan, out of however many the source stated.
///
/// `Get-MpComputerStatus` reports the end time of the most recent quick scan and of the most recent
/// full scan, while the type declares one `scan_time` whose declared meaning is the most recent scan
/// time. That is the later of the two instants the source gave, so with both stated the later one is
/// carried, assembled from the source's own values and marked `decoded` so a reader can tell it was
/// selected here. With one stated, that one is the most recent scan and is carried as it stands.
///
/// Two instants that cannot be ordered put nothing in the field: this adapter will not pick one of
/// two stated scans by their order in a document, and the note on the result says both are still in
/// the material. With neither stated the field is absent, and a read failure the material states is
/// kept as one.
fn scan_time(input: &EvidenceInput) -> FieldCandidate {
    let stated = scan_end_times(input);
    if stated.is_empty() {
        return if stated_unreadable(input, "scan_time") {
            unreadable("scan_time", FieldKind::State)
        } else {
            field(&input.payload, "scan_time", "QuickScanEndTime", FieldKind::State)
        };
    }
    // The source states these two keys and no others, so the later one is decided by comparing them
    // as the times they are; text comparison would put a scan that ended at noon after one that
    // ended at 23:00 of the same day.
    if let [first, second] = stated.as_slice() {
        if let (Ok(left), Ok(right)) = (canonical_time(first), canonical_time(second)) {
            // The later scan, kept in the form the source wrote it rather than in canonical form.
            let latest = if compare_canonical(&left, &right) == std::cmp::Ordering::Less { second } else { first };
            return decoded("scan_time", FieldKind::State, Value::String(latest.clone()));
        }
        // Two stated scans this adapter cannot order: the field stays absent rather than one of them
        // being chosen by the order it happened to appear in, or by its key being the first one
        // looked at.
        return absent("scan_time", FieldKind::State);
    }
    decoded("scan_time", FieldKind::State, Value::String(stated[0].clone()))
}

/// A note when the source's two scans were both needed to state one field, so the assembly is
/// visible in the result and not only in the field's derivation.
fn scan_time_note(input: &EvidenceInput) -> Option<String> {
    (scan_end_times(input).len() > 1).then(|| {
        "the source states both a quick-scan and a full-scan end time; scan_time carries the later of the two, \
         and both remain in the material"
            .to_owned()
    })
}

// ---------- pcscope.security.protection-policy ----------

/// `Get-MpPreference`, one item — the product's configured policy, one object per exclusion in it.
///
/// The item describes two things and is adapted into both, because the source map declares both
/// types behind it. It is one `security.protection` record for the product, and one
/// `security.exclusion` object per entry of the three exclusion keys.
///
/// The product record states no version and no scan time: those belong to the status source, which
/// arrives as its own item, and they are left absent here rather than carried over from it.
fn protection_policy(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    // A Defender preference is machine-wide, so this is one of the machine-level objects.
    let scope = machine_scope(context);
    let product = item_name(input);
    let identity = identity("security.protection", &scope, &input.evidence.id, &[("name", Some(product.clone()))]);
    let fields = vec![
        known("name", FieldKind::Identity, Value::String(product.clone())),
        // The engine's and the signature library's versions, the tamper state and the scan times are
        // stated by the three other sources of this type, not by this one.
        stated_field(input, "engine_version", "AMEngineVersion", FieldKind::State),
        stated_field(input, "signature_version", "AntivirusSignatureVersion", FieldKind::State),
        policy_real_time(input),
        stated_field(input, "tamper_protected", "IsTamperProtected", FieldKind::State),
        stated_field(input, "scan_time", "QuickScanEndTime", FieldKind::State),
    ];
    let mut result = AdapterResult::adapted();
    result.observations.push(ObservationCandidate {
        identity,
        fields,
        observed_at: input.observed_at.clone(),
        evidence: vec![input.evidence.clone()],
        complete: context.complete,
    });
    for (key, exclusion_type) in EXCLUSION_KEYS {
        for value in entries(&input.payload, key) {
            result.observations.push(exclusion(input, context, &scope, &product, exclusion_type, &value));
        }
    }
    if result.observations.len() == 1 {
        // The item legitimately states no exclusion at all, which is a statement about the product
        // and not a failure to read it: nothing is reported for the exclusions, and nothing is
        // invented. The record itself is still a candidate, so the result is not empty.
        result.notes.push("the source states no exclusion entry; no exclusion object is created".to_owned());
    }
    result
}

/// The three exclusion lists and the exclusion type each one states.
const EXCLUSION_KEYS: [(&str, &str); 3] = [("ExclusionPath", "path"), ("ExclusionProcess", "process"), ("ExclusionExtension", "extension")];

/// The requested setting is distinct from the engine's actual `real_time` state.
fn policy_real_time(input: &EvidenceInput) -> FieldCandidate {
    if stated_unreadable(input, "real_time_configured") || stated_unreadable(input, "real_time") {
        return unreadable("real_time_configured", FieldKind::Configuration);
    }
    match input.payload.get("DisableRealtimeMonitoring") {
        Some(Value::Bool(disabled)) => decoded("real_time_configured", FieldKind::Configuration, Value::Bool(!disabled)),
        // The switch is there but not in a shape this field can hold, so the field stays absent
        // rather than carrying the switch's own text as if it were an on/off state.
        Some(value) if !value.is_null() => absent("real_time_configured", FieldKind::Configuration),
        _ => field(&input.payload, "real_time_configured", "DisableRealtimeMonitoring", FieldKind::Configuration),
    }
}

/// The entries of one exclusion key.
///
/// `Get-MpPreference` states each of these as a string array, and PowerShell's JSON encoder writes a
/// one-element array as the element itself — so both shapes are what this collector's own rule
/// produces for the same fact, and both are read as the entries they are. A key the item does not
/// carry, or carries as null, states no exclusion entry at all; an empty table then yields no
/// object, which is the honest reading of a product with nothing excluded.
fn entries(payload: &Value, key: &str) -> Vec<String> {
    match payload.get(key) {
        Some(Value::Array(items)) => items.iter().filter_map(entry_text).collect(),
        Some(value) if !value.is_null() => entry_text(value).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// The text of one entry of an exclusion list, which is not inside a named table of its own.
fn entry_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// One excluded path, process or extension.
///
/// The type's identity is the product, the exclusion type and the value, and the source states all
/// three: the list an entry came out of is its type, the entry is the value, and the product is the
/// one this item is about — the same name the product record carries, so a product's exclusions and
/// the product itself are named alike rather than each guessing a different spelling.
///
/// The entry's position in its list is deliberately not part of what is built here. The collector
/// sorts nothing into these lists and a list's order is not a fact about the exclusion, so the same
/// path stated twice is the same one exclusion and a re-ordered list is the same set of objects.
/// Only entries whose text the material states become objects; an entry of another shape cannot be a
/// `value`, and none of the three lists these sources emit holds one.
fn exclusion(input: &EvidenceInput, context: &AdapterContext, scope: &str, product: &str, exclusion_type: &str, value: &str) -> ObservationCandidate {
    let identity = identity(
        "security.exclusion",
        scope,
        &input.evidence.id,
        &[("product", Some(product.to_owned())), ("exclusion_type", Some(exclusion_type.to_owned())), ("value", Some(value.to_owned()))],
    );
    let fields = vec![
        known("product", FieldKind::Identity, Value::String(product.to_owned())),
        known("exclusion_type", FieldKind::Identity, Value::String(exclusion_type.to_owned())),
        known("value", FieldKind::Identity, Value::String(value.to_owned())),
    ];
    ObservationCandidate {
        identity,
        fields,
        observed_at: input.observed_at.clone(),
        evidence: vec![input.evidence.clone()],
        complete: context.complete,
    }
}

// ---------- pcscope.security.detections ----------

/// `Get-MpThreatDetection` — the detection records the source still retains.
///
/// One retained item yields the original detection and, when timestamped, its latest response.
/// No current-presence observation is produced for this historical subject.
fn detections(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    super::retained_history::detections(input,context)
}

// ---------- pcscope.security.products ----------

/// `AntiVirusProduct` in `root/SecurityCenter2`, one item per registered security product.
///
/// `Cim-Items` keys each item by the class's `instanceGuid` and `Fields` copies the properties the
/// emitter names, so the native identifier is the instance GUID, which is also what the type's
/// `instance_id` field holds. The name is the product's `displayName`; `productState` is the
/// bit-packed state code the Security Center publishes and is carried exactly as the source reports
/// it, because expanding a packed code is an interpretation of the value and not the value.
/// `timestamp` is collected and no field of this type reads it, so it stays in the material.
fn security_products(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = machine_scope(context);
    let instance = text(&input.payload, "instanceGuid").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("security.security_product", &scope, &input.evidence.id, &[("native_id", Some(instance.clone()))]);
    let fields = vec![
        known("instance_id", FieldKind::Identity, Value::String(instance)),
        stated_field(input, "name", "displayName", FieldKind::Identity),
        stated_field(input, "product_state", "productState", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.security.deviceguard ----------

/// `Win32_DeviceGuard` in the Device Guard namespace, one item — the machine's Device Guard
/// configuration.
///
/// The class publishes an `InstanceIdentifier`, which is the item id and the type's native
/// identifier. The four declared fields are the VBS status, the services configured, the services
/// running and the available security properties — all lists or codes the class publishes, carried
/// as reported. `RequiredSecurityProperties` and `CodeIntegrityPolicyEnforcementStatus` are
/// collected and no field of this type reads them, so they stay in the material.
///
/// The source is permission-limited on a normal machine, and the collector's response to a denied
/// read is a gap for the whole source rather than an item with empty fields, so a denied read never
/// reaches this function as an object with nothing in it.
fn device_guard(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = machine_scope(context);
    let native = text(&input.payload, "InstanceIdentifier").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("security.device_guard_configuration", &scope, &input.evidence.id, &[("native_id", Some(native))]);
    let fields = vec![
        stated_field(input, "vbs_status", "VirtualizationBasedSecurityStatus", FieldKind::State),
        stated_field(input, "services_configured", "SecurityServicesConfigured", FieldKind::Configuration),
        stated_field(input, "services_running", "SecurityServicesRunning", FieldKind::State),
        stated_field(input, "available_properties", "AvailableSecurityProperties", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.security.secureboot ----------

/// `Confirm-SecureBootUEFI`, one item — whether Secure Boot is on.
///
/// The emitter's whole item is `Item 'secureboot' @{enabled=(Confirm-SecureBootUEFI)}`, so the name
/// the type's identity is built on is the item id the collector assigned and the value is the
/// boolean the firmware reported. `false` here is a statement the source made, not a default: where
/// the component is unsupported or the read is denied, `Confirm-SecureBootUEFI` throws and the
/// collector sends a gap for the source, so nothing false is invented for such a machine — which is
/// the type's own note that an unsupported or unauthorised component is reported as a gap rather
/// than as a switched-off setting.
///
/// The identity key `name` is not one of this type's fields — it declares `enabled` and nothing
/// else — so the name exists only as the key the object is identified by, and no field is invented
/// to carry it. (The proximity of `security.protection`, which does declare its `name`, is exactly
/// why this is stated rather than assumed.)
fn secure_boot(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = machine_scope(context);
    let name = item_name(input);
    let identity = identity("security.secure_boot_configuration", &scope, &input.evidence.id, &[("name", Some(name))]);
    let fields = vec![stated_field(input, "enabled", "enabled", FieldKind::Configuration)];
    object_candidate(input, context, identity, fields)
}

// ---------- certificates, from each store location ----------

/// The X.509 stores the emitter walks, one item per certificate in each.
///
/// Both certificate sources emit this same shape and target `security.certificate_registration`, and
/// the store's location is what separates them. The type's composite identity is the store scope,
/// the store name and the thumbprint — the location the payload states, the store it was found in,
/// and the certificate's own thumbprint — so the same certificate in `Root` and in `CA`, or in the
/// machine's store and the current user's, is the several registrations the type says it is.
///
/// The scope follows the same fact. A machine store belongs to the installation, so the collecting
/// account is dropped from it; the current user's store is that user's own and keeps the scope it
/// was handed, user segment included, so two users' stores can never merge into one object. Which
/// of the two a store is comes from the payload's `location`, and where the material states none
/// the source itself says which — the current-user source reads the user's own store and the
/// local-machine source reads the machine's.
///
/// The collector states that trust is not evaluated, and this adapter neither evaluates it nor
/// contradicts it: `trust` has no declared field and stays in the material, with the source's own
/// statement noted on the result so a reader of the record sees it. `signature_algorithm` is
/// collected and undeclared as well.
fn certificates(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let payload = &input.payload;
    let location = text(payload, "location");
    let per_user = match location.as_deref() {
        Some(store) => store.eq_ignore_ascii_case("CurrentUser"),
        None => input.source.ends_with(".currentuser"),
    };
    let scope = if per_user { user_scope(context) } else { machine_scope(context) };
    // The item id is the collector's composite `store/thumbprint`; it is keyed that way, and neither
    // half of it stands in for a component the payload did not state.
    let identity = identity(
        "security.certificate_registration",
        &scope,
        &input.evidence.id,
        &[("store_scope", location.clone()), ("store_name", text(payload, "store")), ("thumbprint", text(payload, "thumbprint"))],
    );
    let fields = vec![
        stated_field(input, "store", "store", FieldKind::Identity),
        stated_field(input, "location", "location", FieldKind::Identity),
        stated_field(input, "thumbprint", "thumbprint", FieldKind::Identity),
        stated_field(input, "subject", "subject", FieldKind::Configuration),
        stated_field(input, "issuer", "issuer", FieldKind::Configuration),
        stated_field(input, "not_before", "not_before", FieldKind::Configuration),
        stated_field(input, "not_after", "not_after", FieldKind::Configuration),
        stated_field(input, "has_private_key", "has_private_key", FieldKind::Configuration),
    ];
    let mut result = object_candidate(input, context, identity, fields);
    if let Some(trust) = text(payload, "trust") {
        result.notes.push(format!("the collector states this certificate's trust as \"{trust}\"; trust is not evaluated here"));
    }
    result
}

// ---------- pcscope.security.encryption ----------

/// `Get-BitLockerVolume`, one item per volume.
///
/// The item id is the volume's mount point, which is also what the payload states, and the type's
/// native identifier is that mount point: it is the identifier this source gives its volumes, and
/// nothing else in the material identifies one. The four declared fields are the volume's status,
/// its protection status, the encryption method and the protector types, each carried exactly as
/// `Get-BitLockerVolume` reports it — the protector types as the list the collector sent.
///
/// `volume_type`, `encryption_percentage` and `lock_status` are collected and no field of this type
/// reads them, so they stay in the material. The collector also states, with every volume, that it
/// did not collect recovery material; that statement is carried on the result rather than being
/// turned into a field the type does not declare.
///
/// This source is the one in the category that is denied most often, and the distinction the fields
/// keep is the whole point: a volume whose statuses could not be read has fields that are
/// `unreadable`, not fields that are absent and not a volume reported as unprotected.
fn bitlocker_volumes(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = machine_scope(context);
    let mount_point = text(&input.payload, "mount_point").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("security.bitlocker_configuration", &scope, &input.evidence.id, &[("native_id", Some(mount_point))]);
    let fields = vec![
        stated_field(input, "mount_point", "mount_point", FieldKind::Identity),
        stated_field(input, "volume_status", "volume_status", FieldKind::State),
        stated_field(input, "protection_status", "protection_status", FieldKind::State),
        stated_field(input, "encryption_method", "encryption_method", FieldKind::Configuration),
        stated_field(input, "protector_types", "protector_types", FieldKind::Configuration),
    ];
    let mut result = object_candidate(input, context, identity, fields);
    if input.payload.get("recovery_material_collected").and_then(Value::as_bool) == Some(false) {
        result.notes.push("the collector states that no recovery material was collected for this volume".to_owned());
    }
    result
}

// ---------- pcscope.security.ssh-public-keys ----------

/// The `*.pub` files in the collecting user's `.ssh` directory, one item per key file.
///
/// This is a per-user object: the emitter reads `$env:USERPROFILE\.ssh`, so the file belongs to the
/// account that owns it and the scope it was handed — user segment included — is what it keeps. The
/// type's identity is the file path and the algorithm, both of which the material states, and the
/// item id is the file's full path, which is the same path the payload carries.
///
/// The collector states that it keeps neither the key material nor the comment, and this adapter
/// keeps that statement visible rather than mapping them: no field carries a key, and the note on
/// the result records what the source said. Only the algorithm and the fingerprint of the key are
/// here.
///
/// A file whose content the collector refused (too large, a reparse point, an unsupported encoding)
/// throws and fails the whole source rather than arriving as an item; a directory with no `.pub`
/// files yields no item at all. So a key that is not in this material is outside what this source
/// reported, never a key that was found and dropped.
fn ssh_public_keys(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if let Some(not_object) = not_a_table(input) {
        return not_object;
    }
    let scope = user_scope(context);
    let path = text(&input.payload, "path").unwrap_or_else(|| input.item_id.clone());
    let identity = identity(
        "security.ssh_public_key",
        &scope,
        &input.evidence.id,
        &[("file_path", Some(path.clone())), ("algorithm", text(&input.payload, "algorithm"))],
    );
    let fields = vec![
        known("path", FieldKind::Identity, Value::String(path)),
        stated_field(input, "algorithm", "algorithm", FieldKind::Identity),
        stated_field(input, "fingerprint", "fingerprint", FieldKind::Identity),
    ];
    let mut result = object_candidate(input, context, identity, fields);
    if input.payload.get("key_material_collected").and_then(Value::as_bool) == Some(false) {
        result.notes.push("the collector states that no key material and no comment were collected for this key".to_owned());
    }
    result
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

    /// The same collection run as a different account: the material is identical, and which account
    /// obtained it is the only difference.
    fn other_user_context() -> AdapterContext {
        context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0")
    }

    fn key_of(candidate: &IdentityCandidate) -> String {
        let mut parts: Vec<String> = candidate.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect();
        parts.sort();
        parts.join("|")
    }

    fn field_of<'a>(candidate: &'a ObservationCandidate, field_id: &str) -> &'a FieldCandidate {
        candidate.fields.iter().find(|field| field.field_id == field_id).expect("field is mapped")
    }

    fn first<'a>(result: &'a AdapterResult) -> &'a ObservationCandidate {
        &result.observations[0]
    }

    /// The observation of one type, out of a result that may carry several.
    fn of_type<'a>(result: &'a AdapterResult, type_id: &str) -> Vec<&'a ObservationCandidate> {
        result.observations.iter().filter(|candidate| candidate.identity.type_id == type_id).collect()
    }

    fn defender_status() -> Value {
        json!({
            "AMServiceEnabled": true,
            "AntispywareEnabled": true,
            "AntivirusEnabled": true,
            "BehaviorMonitorEnabled": true,
            "IoavProtectionEnabled": true,
            "NISEnabled": false,
            "OnAccessProtectionEnabled": false,
            "RealTimeProtectionEnabled": true,
            "IsTamperProtected": true,
            "AMEngineVersion": "1.1.24090.11",
            "AMProductVersion": "4.18.24090.11",
            "AntivirusSignatureVersion": "1.415.44.0",
            "AntivirusSignatureLastUpdated": "2026-09-11T09:00:00.0000000Z",
            "QuickScanEndTime": "2026-09-11T10:00:00.0000000Z",
            "FullScanEndTime": "2026-09-10T10:00:00.0000000Z",
            "source_properties": {}
        })
    }

    fn defender_policy() -> Value {
        json!({
            "DisableRealtimeMonitoring": false,
            "DisableBehaviorMonitoring": false,
            "DisableIOAVProtection": false,
            "DisableScriptScanning": false,
            "PUAProtection": 1,
            "ExclusionPath": ["C:\\示例\\排除", "D:\\工具"],
            "ExclusionProcess": ["示例.exe"],
            "ExclusionExtension": [".tmp"],
            "source_properties": {}
        })
    }

    fn certificate(store: &str, location: &str, thumbprint: &str) -> Value {
        json!({
            "store": store,
            "location": location,
            "thumbprint": thumbprint,
            "subject": "CN=示例主题",
            "issuer": "CN=示例颁发者",
            "not_before": "2026-01-01T00:00:00.0000000Z",
            "not_after": "2030-01-01T00:00:00.0000000Z",
            "has_private_key": false,
            "signature_algorithm": "sha256RSA",
            "trust": "not evaluated"
        })
    }

    fn bitlocker_volume() -> Value {
        json!({
            "mount_point": "C:",
            "volume_type": "OperatingSystem",
            "volume_status": "FullyEncrypted",
            "protection_status": "On",
            "encryption_method": "XtsAes256",
            "encryption_percentage": 100,
            "lock_status": "Unlocked",
            "protector_types": ["Tpm", "RecoveryPassword"],
            "recovery_material_collected": false
        })
    }

    #[test]
    fn a_defender_status_item_is_keyed_by_the_name_its_collector_recorded() {
        let result = protection(&input("pcscope.security.protection", "defender", defender_status()), &context());
        let candidate = first(&result);
        assert_eq!(candidate.identity.type_id, "security.protection");
        assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
        assert!(candidate.identity.fields.iter().any(|field| field.key == "name" && field.value == "defender"));
        assert_eq!(field_of(candidate, "name").value, Some(json!("defender")));
        assert_eq!(field_of(candidate, "engine_version").value, Some(json!("1.1.24090.11")));
        assert_eq!(field_of(candidate, "signature_version").value, Some(json!("1.415.44.0")));
        assert_eq!(field_of(candidate, "real_time").value, Some(json!(true)));
        assert_eq!(field_of(candidate, "tamper_protected").value, Some(json!(true)));
        // The switches the type declares no field for are not bent into one: only the five declared
        // fields exist, and NISEnabled's `false` is not reported as the engine's real-time state.
        assert_eq!(candidate.fields.len(), 6);
        assert!(!candidate.fields.iter().any(|field| field.field_id == "nis_enabled"));
        assert!(result.actions.is_empty(), "a status snapshot is not an action");
    }

    #[test]
    fn the_scan_time_is_the_later_scan_the_source_states() {
        let result = protection(&input("pcscope.security.protection", "defender", defender_status()), &context());
        let scan = field_of(first(&result), "scan_time");
        // The full scan ended a day before the quick scan, so the quick scan is the most recent one.
        assert_eq!(scan.value, Some(json!("2026-09-11T10:00:00.0000000Z")));
        // The value was selected out of two, so the record says it was.
        assert_eq!(scan.derivation, Derivation::Decoded);
        assert!(result.notes.iter().any(|note| note.contains("the later of the two")), "{:?}", result.notes);

        // One stated scan is the most recent scan; it is carried as it stands.
        let one = json!({"QuickScanEndTime": "2026-09-11T10:00:00.0000000Z"});
        let result = protection(&input("pcscope.security.protection", "defender", one), &context());
        assert_eq!(field_of(first(&result), "scan_time").value, Some(json!("2026-09-11T10:00:00.0000000Z")));

        // Neither stated: the field is absent rather than filled with the observation time.
        let none = json!({"QuickScanEndTime": null, "AMEngineVersion": "1.1"});
        let result = protection(&input("pcscope.security.protection", "defender", none), &context());
        assert_eq!(field_of(first(&result), "scan_time").status, ValueStatus::NotProvided);
        assert!(field_of(first(&result), "scan_time").value.is_none());
        assert!(result.notes.is_empty(), "{:?}", result.notes);
    }

    #[test]
    fn the_policy_item_yields_the_product_and_one_object_per_exclusion() {
        let result = protection_policy(&input("pcscope.security.protection-policy", "defender-policy", defender_policy()), &context());
        // Four entries across the three lists, and the product itself.
        assert_eq!(of_type(&result, "security.exclusion").len(), 4, "{:?}", result.observations.len());
        assert_eq!(of_type(&result, "security.protection").len(), 1);

        let product = of_type(&result, "security.protection")[0];
        assert!(product.identity.missing.is_empty(), "{:?}", product.identity.missing);
        assert!(product.identity.fields.iter().any(|field| field.key == "name" && field.value == "defender-policy"));
        // The policy switch is the only field this source can state, and it states it as a decode of
        // the disabling switch rather than as a value the source wrote.
        assert_eq!(field_of(product, "real_time_configured").value, Some(json!(true)));
        assert_eq!(field_of(product, "real_time_configured").derivation, Derivation::Decoded);
        assert_eq!(field_of(product, "real_time_configured").field_kind, FieldKind::Configuration);
        assert!(!product.fields.iter().any(|field| field.field_id == "real_time"));
        // The versions and the scan time belong to the status source, which arrives as its own item.
        assert_eq!(field_of(product, "engine_version").status, ValueStatus::NotProvided);
        assert_eq!(field_of(product, "signature_version").status, ValueStatus::NotProvided);
        assert_eq!(field_of(product, "scan_time").status, ValueStatus::NotProvided);

        let paths: Vec<&ObservationCandidate> = of_type(&result, "security.exclusion")
            .into_iter()
            .filter(|candidate| field_of(candidate, "exclusion_type").value == Some(json!("path")))
            .collect();
        assert_eq!(paths.len(), 2);
        for candidate in paths {
            // All three identity components are stated by the source, so none is missing and none is
            // guessed: the product is the item the entries came out of.
            assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
            assert!(candidate.identity.fields.iter().any(|field| field.key == "product" && field.value == "defender-policy"));
            assert!(candidate.identity.fields.iter().any(|field| field.key == "exclusion_type" && field.value == "path"));
            assert_eq!(field_of(candidate, "product").field_kind, FieldKind::Identity);
            assert_eq!(field_of(candidate, "value").field_kind, FieldKind::Identity);
        }
        let values: Vec<Value> = of_type(&result, "security.exclusion").iter().map(|candidate| field_of(candidate, "value").value.clone().unwrap()).collect();
        assert!(values.contains(&json!("C:\\示例\\排除")));
        assert!(values.contains(&json!("示例.exe")));
        assert!(values.contains(&json!(".tmp")));
    }

    #[test]
    fn an_exclusion_list_the_collector_wrote_as_one_value_is_still_one_entry() {
        // PowerShell serialises a one-element array as the element itself, which is this collector's
        // own output for a product with exactly one excluded path.
        let payload = json!({"DisableRealtimeMonitoring": true, "ExclusionPath": "C:\\唯一"});
        let result = protection_policy(&input("pcscope.security.protection-policy", "defender-policy", payload), &context());
        let exclusions = of_type(&result, "security.exclusion");
        assert_eq!(exclusions.len(), 1);
        assert_eq!(field_of(exclusions[0], "value").value, Some(json!("C:\\唯一")));

        // A product with nothing excluded states no exclusion, and none is invented for it.
        let quiet = json!({"DisableRealtimeMonitoring": false, "ExclusionPath": [], "ExclusionProcess": null});
        let result = protection_policy(&input("pcscope.security.protection-policy", "defender-policy", quiet), &context());
        assert!(of_type(&result, "security.exclusion").is_empty());
        assert_eq!(of_type(&result, "security.protection").len(), 1, "the product record is still there");
        assert!(result.notes.iter().any(|note| note.contains("no exclusion entry")), "{:?}", result.notes);
    }

    #[test]
    fn a_policy_switch_of_another_shape_leaves_the_state_field_absent() {
        // The source's switch was not a boolean, so this adapter does not decide what it means.
        let payload = json!({"DisableRealtimeMonitoring": "Disabled"});
        let result = protection_policy(&input("pcscope.security.protection-policy", "defender-policy", payload), &context());
        assert_eq!(field_of(of_type(&result, "security.protection")[0], "real_time_configured").status, ValueStatus::NotProvided);

        // And the policy switches with no declared field stay in the material: they are not turned
        // into fields of the product record.
        let result = protection_policy(&input("pcscope.security.protection-policy", "defender-policy", defender_policy()), &context());
        let product = of_type(&result, "security.protection")[0];
        for absent in ["pua_protection", "disable_script_scanning", "disable_behavior_monitoring"] {
            assert!(!product.fields.iter().any(|field| field.field_id == absent), "{absent}");
        }
    }

    #[test]
    fn a_security_product_is_keyed_by_the_instance_guid_of_the_security_center() {
        let payload = json!({"instanceGuid": "{00000000-0000-0000-0000-000000000001}", "displayName": "示例防护", "productState": 266240, "timestamp": "2026-09-11T09:00:00.0000000Z", "source_properties": {}});
        let result = security_products(&input("pcscope.security.products", "{00000000-0000-0000-0000-000000000001}", payload), &context());
        let candidate = first(&result);
        assert_eq!(candidate.identity.type_id, "security.security_product");
        assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
        assert!(candidate.identity.fields.iter().any(|field| field.key == "native_id" && field.value.starts_with("{")));
        assert_eq!(field_of(candidate, "instance_id").value, Some(json!("{00000000-0000-0000-0000-000000000001}")));
        assert_eq!(field_of(candidate, "name").value, Some(json!("示例防护")));
        // The packed state code is carried as the source reported it, not expanded.
        assert_eq!(field_of(candidate, "product_state").value, Some(json!(266240)));
        // `timestamp` is collected and no field of this type reads it.
        assert_eq!(candidate.fields.len(), 3);
    }

    #[test]
    fn a_device_guard_snapshot_is_keyed_by_its_instance_identifier() {
        let payload = json!({
            "InstanceIdentifier": "00000000-0000-0000-0000-000000000002",
            "VirtualizationBasedSecurityStatus": 2,
            "SecurityServicesConfigured": [1, 2],
            "SecurityServicesRunning": [1],
            "AvailableSecurityProperties": [1, 2, 3, 4],
            "RequiredSecurityProperties": [1, 2],
            "CodeIntegrityPolicyEnforcementStatus": 2,
            "source_properties": {}
        });
        let result = device_guard(&input("pcscope.security.deviceguard", "00000000-0000-0000-0000-000000000002", payload), &context());
        let candidate = first(&result);
        assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
        assert!(candidate.identity.fields.iter().any(|field| field.key == "native_id" && field.value.ends_with("000000000002")));
        assert_eq!(field_of(candidate, "vbs_status").value, Some(json!(2)));
        assert_eq!(field_of(candidate, "services_running").value, Some(json!([1])));
        assert_eq!(field_of(candidate, "services_configured").value, Some(json!([1, 2])));
        assert_eq!(field_of(candidate, "available_properties").value, Some(json!([1, 2, 3, 4])));
        // RequiredSecurityProperties and the code-integrity status have no declared field and are not
        // bent into one.
        assert_eq!(candidate.fields.len(), 4);
    }

    #[test]
    fn secure_boot_states_enabled_as_the_boolean_the_source_returned() {
        let on = secure_boot(&input("pcscope.security.secureboot", "secureboot", json!({"enabled": true})), &context());
        let candidate = first(&on);
        assert_eq!(candidate.identity.type_id, "security.secure_boot_configuration");
        assert!(candidate.identity.fields.iter().any(|field| field.key == "name" && field.value == "secureboot"));
        assert_eq!(field_of(candidate, "enabled").value, Some(json!(true)));
        // The name is the identity key and not a field: the type declares none, so none is made up
        // for it, even though the neighbour type of this category does declare its own name.
        assert!(candidate.fields.iter().all(|field| field.field_id != "name"));
        assert_eq!(candidate.fields.len(), 1);

        // `false` is a value the firmware reported, not a default for an unreadable component: where
        // the component is unsupported the collector fails the source rather than sending false.
        let off = secure_boot(&input("pcscope.security.secureboot", "secureboot", json!({"enabled": false})), &context());
        assert_eq!(field_of(first(&off), "enabled").value, Some(json!(false)));
        assert_eq!(field_of(first(&off), "enabled").status, ValueStatus::Known);

        // A component the material says nothing about is absent, not off.
        let unknown = secure_boot(&input("pcscope.security.secureboot", "secureboot", json!({"enabled": null})), &context());
        assert_eq!(field_of(first(&unknown), "enabled").status, ValueStatus::NotProvided);
        assert!(field_of(first(&unknown), "enabled").value.is_none());
    }

    #[test]
    fn the_machine_certificate_store_is_keyed_by_its_store_scope_and_thumbprint() {
        let thumbprint = "0000000000000000000000000000000000000001";
        let payload = certificate("Root", "LocalMachine", thumbprint);
        let result = certificates(&input("pcscope.security.certificates.localmachine", &format!("Root/{thumbprint}"), payload), &context());
        let candidate = first(&result);
        assert_eq!(candidate.identity.type_id, "security.certificate_registration");
        assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
        assert!(candidate.identity.fields.iter().any(|field| field.key == "store_scope" && field.value == "LocalMachine"));
        assert!(candidate.identity.fields.iter().any(|field| field.key == "store_name" && field.value == "Root"));
        assert!(candidate.identity.fields.iter().any(|field| field.key == "thumbprint" && field.value == thumbprint));
        // A machine store belongs to the installation, so the account that ran the collection is not
        // part of what the certificate belongs to.
        assert_eq!(candidate.identity.scope, "machine:M");
        assert_eq!(field_of(candidate, "not_before").value, Some(json!("2026-01-01T00:00:00.0000000Z")));
        assert_eq!(field_of(candidate, "has_private_key").value, Some(json!(false)));

        // The same certificate in another store is another registration, which is what its
        // composite key says.
        let in_ca = certificates(&input("pcscope.security.certificates.localmachine", &format!("CA/{thumbprint}"), certificate("CA", "LocalMachine", thumbprint)), &context());
        assert_ne!(key_of(&candidate.identity), key_of(&first(&in_ca).identity));
        // And the trust the collector did not evaluate stays a note and not a field.
        assert!(result.notes.iter().any(|note| note.contains("not evaluated")), "{:?}", result.notes);
        assert!(!candidate.fields.iter().any(|field| field.field_id == "trust"));
        assert!(!candidate.fields.iter().any(|field| field.field_id == "signature_algorithm"));
    }

    #[test]
    fn the_current_user_certificate_store_is_keyed_under_the_account_that_owns_it() {
        let thumbprint = "0000000000000000000000000000000000000002";
        let payload = certificate("My", "CurrentUser", thumbprint);
        let by_first = certificates(&input("pcscope.security.certificates.currentuser", &format!("My/{thumbprint}"), payload.clone()), &context());
        let by_second = certificates(&input("pcscope.security.certificates.currentuser", &format!("My/{thumbprint}"), payload), &other_user_context());
        // A user's own store belongs to that user: the collecting account is part of the object, so
        // the two accounts' stores are two registrations rather than one.
        assert_eq!(first(&by_first).identity.scope, "machine:M/user:S-1-5-18");
        assert_ne!(key_of(&first(&by_first).identity), key_of(&first(&by_second).identity));

        // The machine's store of the same certificate is a different registration again, by scope
        // and by store scope alike.
        let machine = certificates(&input("pcscope.security.certificates.localmachine", &format!("My/{thumbprint}"), certificate("My", "LocalMachine", thumbprint)), &context());
        assert_ne!(key_of(&first(&by_first).identity), key_of(&first(&machine).identity));
    }

    #[test]
    fn a_certificate_key_the_source_did_not_state_is_absent_rather_than_filled_in() {
        let payload = json!({"store": "TrustedPublisher", "location": "LocalMachine", "thumbprint": "0000000000000000000000000000000000000003", "subject": "CN=示例", "issuer": "CN=示例", "not_after": "2030-01-01T00:00:00.0000000Z", "trust": "not evaluated"});
        let result = certificates(&input("pcscope.security.certificates.localmachine", "TrustedPublisher/0000000000000000000000000000000000000003", payload), &context());
        let candidate = first(&result);
        // The thumbprint the item is keyed by is in the payload, so the identity is complete...
        assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
        // ...while the fields the material did not state are absent, not defaulted to false or empty.
        for absent in ["not_before", "has_private_key"] {
            assert_eq!(field_of(candidate, absent).status, ValueStatus::NotProvided, "{absent}");
            assert!(field_of(candidate, absent).value.is_none(), "{absent}");
        }
        assert_eq!(field_of(candidate, "not_after").value, Some(json!("2030-01-01T00:00:00.0000000Z")));

        // A store whose location and thumbprint the material never stated lists the two identity
        // components as missing instead of taking either out of the item id, which is the collector's
        // composite `store/thumbprint` and is not a thumbprint on its own.
        let bare = certificates(&input("pcscope.security.certificates.localmachine", "Root/0000000000000000000000000000000000000004", json!({"subject": "CN=示例"})), &context());
        let mut missing = first(&bare).identity.missing.clone();
        missing.sort();
        assert_eq!(missing, vec!["store_name".to_owned(), "store_scope".to_owned(), "thumbprint".to_owned()]);
    }

    #[test]
    fn a_bitlocker_volume_is_keyed_by_its_mount_point_and_carries_its_status() {
        let result = bitlocker_volumes(&input("pcscope.security.encryption", "C:", bitlocker_volume()), &context());
        let candidate = first(&result);
        assert_eq!(candidate.identity.type_id, "security.bitlocker_configuration");
        assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
        assert!(candidate.identity.fields.iter().any(|field| field.key == "native_id" && field.value == "C:"));
        assert_eq!(field_of(candidate, "mount_point").value, Some(json!("C:")));
        assert_eq!(field_of(candidate, "volume_status").value, Some(json!("FullyEncrypted")));
        assert_eq!(field_of(candidate, "protection_status").value, Some(json!("On")));
        assert_eq!(field_of(candidate, "encryption_method").value, Some(json!("XtsAes256")));
        // The protector types are carried as the list the collector sent.
        assert_eq!(field_of(candidate, "protector_types").value, Some(json!(["Tpm", "RecoveryPassword"])));
        // The collector's own statement about recovery material is kept on the result, and no field
        // is invented for it.
        assert!(result.notes.iter().any(|note| note.contains("no recovery material")), "{:?}", result.notes);
        assert_eq!(candidate.fields.len(), 5);
    }

    #[test]
    fn a_bitlocker_field_the_material_states_it_could_not_read_is_unreadable() {
        // The read was denied: the material says which fields could not be read, and the two that
        // could (the mount point and one status) are still there.
        let mut evidence = input("pcscope.security.encryption", "C:", json!({"mount_point": "C:", "protection_status": "On"}));
        evidence.decoding = Decoding { available: true, problems: vec!["access denied".into()], unreadable_fields: vec!["volume_status".into(), "encryption_method".into(), "protector_types".into()], masked_fields: Vec::new() };
        let result = bitlocker_volumes(&evidence, &context());
        let candidate = first(&result);
        for denied in ["volume_status", "encryption_method", "protector_types"] {
            assert_eq!(field_of(candidate, denied).status, ValueStatus::Unreadable, "{denied}");
            assert!(field_of(candidate, denied).value.is_none(), "{denied}");
        }
        // The fields that were read are not turned into failures along with them.
        assert_eq!(field_of(candidate, "protection_status").value, Some(json!("On")));
        // A denied read is not an absent value: it is a stated failure and stays one.
        assert_ne!(field_of(candidate, "volume_status").status, ValueStatus::NotProvided);
        assert_eq!(field_of(candidate, "mount_point").value, Some(json!("C:")));

        // A volume with no stated read failure reports plain absence, which never claims the read
        // failed.
        let quiet = bitlocker_volumes(&input("pcscope.security.encryption", "C:", json!({"mount_point": "C:", "protection_status": "Off"})), &context());
        assert_eq!(field_of(first(&quiet), "volume_status").status, ValueStatus::NotProvided);
    }

    #[test]
    fn an_ssh_public_key_is_keyed_by_its_path_and_algorithm_in_the_user_scope() {
        let payload = json!({"path": "C:\\示例\\用户\\.ssh\\id_ed25519.pub", "algorithm": "ssh-ed25519", "fingerprint": "SHA256:示例指纹占位", "comment_collected": false, "key_material_collected": false});
        let result = ssh_public_keys(&input("pcscope.security.ssh-public-keys", "C:\\示例\\用户\\.ssh\\id_ed25519.pub", payload), &context());
        let candidate = first(&result);
        assert_eq!(candidate.identity.type_id, "security.ssh_public_key");
        assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
        assert!(candidate.identity.fields.iter().any(|field| field.key == "file_path" && field.value.ends_with("id_ed25519.pub")));
        assert!(candidate.identity.fields.iter().any(|field| field.key == "algorithm" && field.value == "ssh-ed25519"));
        assert_eq!(field_of(candidate, "fingerprint").value, Some(json!("SHA256:示例指纹占位")));
        // The source reads the collecting user's own `.ssh`, so the account is part of the object.
        assert_eq!(candidate.identity.scope, "machine:M/user:S-1-5-18");
        // The key material and the comment the collector did not keep are stated, not mapped.
        assert!(result.notes.iter().any(|note| note.contains("no key material")), "{:?}", result.notes);
        assert_eq!(candidate.fields.len(), 3);

        // An unsupported encoding or an unreadable key file fails the whole source at the collector,
        // so it never arrives as an item with a missing algorithm; one that does arrive with no
        // algorithm states that component as missing rather than filling it in.
        let bare = ssh_public_keys(&input("pcscope.security.ssh-public-keys", "C:\\示例\\用户\\.ssh\\id_x.pub", json!({"path": "C:\\示例\\用户\\.ssh\\id_x.pub"})), &context());
        assert_eq!(first(&bare).identity.missing, vec!["algorithm".to_owned()]);
        assert_eq!(field_of(first(&bare), "fingerprint").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_defender_detection_record_keeps_detection_and_response_separate() {
        let payload = json!({
            "DetectionID": "{00000000-0000-0000-0000-000000000005}",
            "ThreatID": 2147720000i64,
            "InitialDetectionTime": "2026-09-10T10:00:00.0000000Z",
            "LastThreatStatusChangeTime": "2026-09-10T10:05:00.0000000Z",
            "ActionSuccess": true,
            "CleaningActionID": 3,
            "ThreatStatusID": 3,
            "CurrentThreatExecutionStatusID": 0,
            "source_properties": {}
        });
        let result = detections(&input("pcscope.security.detections", "{00000000-0000-0000-0000-000000000005}", payload), &context());
        assert_eq!(result.outcome, AdapterOutcome::Adapted);
        assert!(result.observations.is_empty(), "no protection product is invented for a detection");
        assert_eq!(result.actions.len(),2);
        assert_eq!(result.actions[0].action_id,"security.threat_detected");
        assert_eq!(result.actions[1].result,ActionOutcome::Succeeded);
        assert_ne!(result.actions[0].occurrence_key,result.actions[1].occurrence_key);
    }

    /// A sample of every source in the category, so the mapping can be checked against the catalog
    /// for each of them rather than for the one or two a test happens to open.
    fn samples() -> Vec<(&'static str, &'static str, Value)> {
        let thumbprint = "0000000000000000000000000000000000000001";
        let ssh_path = "C:\\示例\\用户\\.ssh\\id_ed25519.pub";
        vec![
            ("pcscope.security.protection", "defender", defender_status()),
            ("pcscope.security.protection-policy", "defender-policy", defender_policy()),
            ("pcscope.security.products", "{00000000-0000-0000-0000-000000000001}", json!({"instanceGuid": "{00000000-0000-0000-0000-000000000001}", "displayName": "示例防护", "productState": 266240, "timestamp": "2026-09-11T09:00:00.0000000Z", "source_properties": {}})),
            ("pcscope.security.detections", "{00000000-0000-0000-0000-000000000005}", json!({"DetectionID": "{00000000-0000-0000-0000-000000000005}", "ThreatID": 1, "ActionSuccess": false, "ThreatStatusID": 2, "source_properties": {}})),
            ("pcscope.security.deviceguard", "00000000-0000-0000-0000-000000000002", json!({"InstanceIdentifier": "00000000-0000-0000-0000-000000000002", "VirtualizationBasedSecurityStatus": 2, "SecurityServicesConfigured": [1, 2], "SecurityServicesRunning": [1], "AvailableSecurityProperties": [1, 2, 3], "source_properties": {}})),
            ("pcscope.security.secureboot", "secureboot", json!({"enabled": true})),
            ("pcscope.security.certificates.currentuser", "My/", certificate("My", "CurrentUser", thumbprint)),
            ("pcscope.security.certificates.localmachine", "Root/", certificate("Root", "LocalMachine", thumbprint)),
            ("pcscope.security.encryption", "C:", bitlocker_volume()),
            ("pcscope.security.ssh-public-keys", ssh_path, json!({"path": ssh_path, "algorithm": "ssh-ed25519", "fingerprint": "SHA256:示例指纹占位", "comment_collected": false, "key_material_collected": false})),
        ]
    }

    #[test]
    fn every_field_every_source_maps_is_declared_by_the_type_it_names() {
        let context = context();
        for (source, item_id, payload) in samples() {
            let spec = spec_for(source).expect("the source is declared in the category registry");
            let result = (spec.build)(&input(source, item_id, payload), &context);
            assert!(!matches!(result.outcome, AdapterOutcome::Failed { .. }), "{source}: {:?}", result.outcome);
            // The catalog is the authority on both counts: a field a type does not declare, and an
            // identity key neither supplied nor listed as missing, are rejected here.
            let violations = validate(&result, &context);
            assert!(violations.is_empty(), "{source}: {violations:?}");
            if let Some(type_id) = spec.type_id {
                assert!(!result.observations.is_empty(), "{source} adapted nothing");
                for candidate in &result.observations {
                    assert_eq!(candidate.identity.type_id, type_id, "{source}");
                }
            }
            if source == "pcscope.security.protection-policy" {
                // The one source that feeds two types states both, and one exclusion per entry.
                assert_eq!(of_type(&result, "security.protection").len(), 1, "{source}");
                assert_eq!(of_type(&result, "security.exclusion").len(), 4, "{source}");
            }
            {
                assert_eq!(result.outcome, AdapterOutcome::Adapted, "{source}");
                assert!(result.complete(), "{source} produced an incomplete result");
            }
        }
    }

    #[test]
    fn the_same_material_produces_the_same_identity_twice() {
        let context = context();
        let once = key_of(&first(&protection(&input("pcscope.security.protection", "defender", defender_status()), &context)).identity);
        let twice = key_of(&first(&protection(&input("pcscope.security.protection", "defender", defender_status()), &context)).identity);
        assert_eq!(once, twice, "replaying the same material must decide the same thing");

        let first_key = key_of(&first(&certificates(&input("pcscope.security.certificates.localmachine", "Root/1", certificate("Root", "LocalMachine", "1")), &context)).identity);
        let second_key = key_of(&first(&certificates(&input("pcscope.security.certificates.localmachine", "Root/1", certificate("Root", "LocalMachine", "1")), &context)).identity);
        assert_eq!(first_key, second_key);

        let exclusions: Vec<String> = protection_policy(&input("pcscope.security.protection-policy", "defender-policy", defender_policy()), &context)
            .observations
            .iter()
            .filter(|candidate| candidate.identity.type_id == "security.exclusion")
            .map(|candidate| key_of(&candidate.identity))
            .collect();
        let again: Vec<String> = protection_policy(&input("pcscope.security.protection-policy", "defender-policy", defender_policy()), &context)
            .observations
            .iter()
            .filter(|candidate| candidate.identity.type_id == "security.exclusion")
            .map(|candidate| key_of(&candidate.identity))
            .collect();
        assert_eq!(exclusions, again);
        assert_eq!(exclusions.len(), 4);
    }

    #[test]
    fn material_that_is_not_an_object_keeps_a_visible_reason() {
        // A source whose item is a bare list describes no object these types have. The outcome says
        // so, with the shape the item carried, instead of mining it for a field.
        let listed = protection(&input("pcscope.security.protection", "defender", json!(["TpmPresent", true])), &context());
        assert!(matches!(listed.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", listed.outcome);
        assert!(listed.observations.is_empty());
        match listed.outcome {
            AdapterOutcome::NotAnObject { reason } => {
                assert!(reason.contains("pcscope.security.protection"), "{reason}");
                assert!(reason.contains("list"), "{reason}");
            }
            other => panic!("expected a non-object outcome, got {other:?}"),
        }

        let scalar = bitlocker_volumes(&input("pcscope.security.encryption", "C:", json!(42)), &context());
        assert!(matches!(scalar.outcome, AdapterOutcome::NotAnObject { .. }));
    }
}
