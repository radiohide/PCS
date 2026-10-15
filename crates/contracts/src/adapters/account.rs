//! S09 account and identity adapters.
//!
//! Checked against the collector's own emit declarations in
//! `plugins/powershell-adapter/collect-system.ps1` and `domains/system.ps1`, which is where the
//! payload keys below come from. Two of these sources carry more than one object per item, so the
//! shape of the material decides the shape of the result rather than the other way round.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.identity.accounts", type_id: Some("account.user"), build: accounts },
    SourceSpec { source: "pcscope.identity.groups", type_id: Some("account.group"), build: groups },
    SourceSpec { source: "pcscope.identity.profiles", type_id: Some("account.user_profile"), build: profiles },
    SourceSpec { source: "pcscope.identity.assigned-rights", type_id: Some("account.right_assignment"), build: assigned_rights },
    SourceSpec { source: "pcscope.identity.machine", type_id: Some("account.machine_identity"), build: machine },
];

/// `Get-LocalUser`, keyed by SID.
///
/// The account's name is a field, never part of the identity: a rename must not read as a different
/// account, and two accounts that happen to share a name must not merge. That is decided by the
/// type's strategy, and this adapter's part in it is to supply the SID and not the name.
///
/// The collector sends `created_at: null` with a basis saying the source cannot report it. The type
/// declares no creation-time field, so nothing is written — and nothing is invented to fill it.
fn accounts(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let identity = identity("account.user", scope, &input.evidence.id, &[("sid", Some(input.item_id.clone()))]);
    let fields = vec![
        field(&input.payload, "name", "name", FieldKind::Configuration),
        field(&input.payload, "enabled", "enabled", FieldKind::Configuration),
        field(&input.payload, "last_logon", "last_logon", FieldKind::State),
        field(&input.payload, "password_required", "password_required", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Get-LocalGroup`, plus one candidate per member.
///
/// A group and its membership are two different objects with two different identities: the group is
/// keyed by its own SID, and each membership by the pair. Expansion happens here because the source
/// reports membership as a list inside the group, which is a shape of the material, not a claim
/// that membership has no identity of its own.
///
/// When the member list cannot be read the collector does not emit the source at all — it raises,
/// and the run sends a gap instead. So an unreadable list never reaches this function, and an empty
/// list here means an empty group rather than a denied read.
fn groups(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let group_sid = input.item_id.clone();
    let group_identity = identity("account.group", scope, &input.evidence.id, &[("sid", Some(group_sid.clone()))]);
    let fields = vec![
        field(&input.payload, "name", "name", FieldKind::Configuration),
        match input.payload.get("members") {
            Some(Value::Array(members)) => {
                let mut candidate = field(&input.payload, "members", "members", FieldKind::Configuration);
                candidate.value = Some(Value::Array(members.clone()));
                candidate.value_type = "list".into();
                candidate.status = ValueStatus::Known;
                candidate
            }
            // The key is there but is not a list: the material does not say what the membership is,
            // and saying "no members" would be inventing one.
            Some(_) => unreadable("members", FieldKind::Configuration),
            None => field(&input.payload, "members", "members", FieldKind::Configuration),
        },
    ];
    let mut candidates = vec![ObservationCandidate {
        identity: group_identity,
        fields,
        observed_at: input.observed_at.clone(),
        evidence: vec![input.evidence.clone()],
        complete: context.complete,
    }];
    if let Some(Value::Array(members)) = input.payload.get("members") {
        for member in members.iter().filter_map(Value::as_str) {
            let membership_identity = identity("account.group_membership", scope, &input.evidence.id, &[("group_sid", Some(group_sid.clone())), ("member_sid", Some(member.to_owned()))]);
            let membership_fields = vec![
                field(&input.payload, "group", "name", FieldKind::Configuration).with_id("group"),
                known("member", FieldKind::Configuration, Value::String(member.to_owned())),
            ];
            candidates.push(ObservationCandidate {
                identity: membership_identity,
                fields: membership_fields,
                observed_at: input.observed_at.clone(),
                evidence: vec![input.evidence.clone()],
                complete: context.complete,
            });
        }
    }
    observations(candidates)
}

/// `Win32_UserProfile`, keyed by SID.
fn profiles(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let identity = identity("account.user_profile", scope, &input.evidence.id, &[("sid", Some(input.item_id.clone()))]);
    let fields = vec![
        field(&input.payload, "path", "LocalPath", FieldKind::Configuration),
        field(&input.payload, "loaded", "Loaded", FieldKind::State),
        field(&input.payload, "last_use", "LastUseTime", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

/// `PCScopeIdentity::Rights()`, one candidate per privilege.
///
/// The source reports one account's whole right set in a single item, so one item is several
/// objects: the identity is the pair of account and privilege, and the privilege is what makes the
/// second half. Emitting one object per account instead would lose which right was assigned.
///
/// The collector's own note travels with the material: an assigned right is not effective access on
/// every resource. Nothing here upgrades it into one.
fn assigned_rights(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let Some(rights) = input.payload.get("rights").and_then(Value::as_array) else {
        return AdapterResult::failed("right assignment item carries no rights list; nothing can be attributed", false);
    };
    let account = input.payload.get("sid").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| input.item_id.clone());
    let mut candidates = Vec::new();
    for right in rights.iter().filter_map(Value::as_str) {
        let identity = identity(
            "account.right_assignment",
            scope,
            &input.evidence.id,
            &[("account_sid", Some(account.clone())), ("privilege", Some(right.to_owned()))],
        );
        let fields = vec![known("privilege", FieldKind::Configuration, Value::String(right.to_owned()))];
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

/// The machine itself, arriving in two item streams.
///
/// `Win32_ComputerSystem` and `Win32_ComputerSystemProduct` are emitted by one declaration as two
/// kinds of item: one keyed by the computer name, one by its UUID. They describe one machine, so
/// the identity cannot be the item id — that would make two machines out of one. The machine is
/// what the collection scope already names, so it is the identity, and each stream contributes the
/// fields it carries.
fn machine(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = &installation_scope(&context.collected_scope);
    let native_id = scope
        .split('/')
        .find_map(|segment| segment.strip_prefix("machine:"))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| input.item_id.clone());
    let identity = identity("account.machine_identity", scope, &input.evidence.id, &[("native_id", Some(native_id))]);
    // Win32_ComputerSystemProduct.Name is the product model, not the computer's name.
    // The two streams own disjoint facts; a product must not overwrite the host name.
    if input.payload.get("UUID").is_some() {
        return object_candidate(input, context, identity, vec![
            field(&input.payload, "uuid", "UUID", FieldKind::Configuration),
        ]);
    }
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Configuration),
        field(&input.payload, "domain", "Domain", FieldKind::Configuration),
        field(&input.payload, "role", "DomainRole", FieldKind::Configuration),
        field(&input.payload, "manufacturer", "Manufacturer", FieldKind::Configuration),
        field(&input.payload, "model", "Model", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// A field whose value is known because the adapter just stated it, not because a key happened to
/// be present.
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

trait WithId {
    fn with_id(self, field_id: &str) -> Self;
}
impl WithId for FieldCandidate {
    fn with_id(mut self, field_id: &str) -> Self {
        self.field_id = field_id.into();
        self
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

    fn context(scope: &str) -> AdapterContext {
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

    fn account_payload(name: &str) -> Value {
        json!({"name": name, "enabled": true, "last_logon": "2026-09-11T00:00:00Z", "password_required": true, "created_at": null, "created_at_basis": "not available from this source"})
    }

    fn key_of(candidate: &IdentityCandidate) -> String {
        candidate.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect::<Vec<_>>().join("|")
    }

    #[test]
    fn a_rename_does_not_change_the_identity() {
        let source = "pcscope.identity.accounts";
        let context = context("machine:M/user:S-1-5-18/schema:1/context:/shard:0");
        let before = accounts(&input(source, "S-1-5-21-1", account_payload("old-name")), &context);
        let after = accounts(&input(source, "S-1-5-21-1", account_payload("new-name")), &context);
        // The account is the SID. Renaming it is a change of one field, not a different account.
        assert_eq!(key_of(&before.observations[0].identity), key_of(&after.observations[0].identity));
    }

    #[test]
    fn two_accounts_with_one_name_stay_apart() {
        let source = "pcscope.identity.accounts";
        let context = context("machine:M/user:S-1-5-18/schema:1/context:/shard:0");
        let left = accounts(&input(source, "S-1-5-21-1", account_payload("same-name")), &context);
        let right = accounts(&input(source, "S-1-5-21-2", account_payload("same-name")), &context);
        assert_ne!(key_of(&left.observations[0].identity), key_of(&right.observations[0].identity));
    }

    #[test]
    fn a_machine_level_group_does_not_depend_on_the_collecting_account() {
        let source = "pcscope.identity.groups";
        let payload = json!({"name": "Administrators", "members": ["S-1-5-21-1"]});
        // The same machine, collected by two different accounts. A local group belongs to the
        // machine, so which account ran the collection must not make it a different group.
        let by_system = groups(&input(source, "S-1-5-32-544", payload.clone()), &context("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let by_user = groups(&input(source, "S-1-5-32-544", payload), &context("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_eq!(key_of(&by_system.observations[0].identity), key_of(&by_user.observations[0].identity));
    }

    #[test]
    fn a_group_expands_into_one_membership_per_member() {
        let source = "pcscope.identity.groups";
        let payload = json!({"name": "Administrators", "members": ["S-1-5-21-1", "S-1-5-21-2"]});
        let result = groups(&input(source, "S-1-5-32-544", payload), &context("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        assert_eq!(result.observations.len(), 3, "one group and one membership per member");
        let memberships: Vec<&ObservationCandidate> = result.observations.iter().filter(|candidate| candidate.identity.type_id == "account.group_membership").collect();
        assert_eq!(memberships.len(), 2);
        for membership in memberships {
            assert!(membership.identity.fields.iter().any(|field| field.key == "group_sid" && field.value == "S-1-5-32-544"));
            assert!(membership.identity.fields.iter().any(|field| field.key == "member_sid"));
            assert!(membership.identity.missing.is_empty());
        }
    }

    #[test]
    fn a_field_the_source_did_not_send_is_absent_rather_than_defaulted() {
        let source = "pcscope.identity.accounts";
        // A source that reports no password policy state at all.
        let payload = json!({"name": "n", "enabled": false});
        let result = accounts(&input(source, "S-1-5-21-1", payload), &context("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let fields = &result.observations[0].fields;
        let password = fields.iter().find(|field| field.field_id == "password_required").expect("field is mapped");
        assert_eq!(password.status, ValueStatus::NotProvided);
        assert!(password.value.is_none(), "an absent value must stay absent");
        let enabled = fields.iter().find(|field| field.field_id == "enabled").expect("field is mapped");
        assert_eq!(enabled.status, ValueStatus::Known);
    }

    #[test]
    fn a_right_assignment_is_one_object_per_privilege() {
        let source = "pcscope.identity.assigned-rights";
        let payload = json!({"sid": "S-1-5-21-1", "rights": ["SeBackupPrivilege", "SeRestorePrivilege"], "basis": "LSA"});
        let result = assigned_rights(&input(source, "S-1-5-21-1", payload), &context("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        assert_eq!(result.observations.len(), 2);
        for candidate in &result.observations {
            assert!(candidate.identity.missing.is_empty());
            assert!(candidate.identity.fields.iter().any(|field| field.key == "privilege"));
        }
    }

    #[test]
    fn the_machine_stays_one_object_across_both_item_streams() {
        let source = "pcscope.identity.machine";
        let context = context("machine:M/user:S-1-5-18/schema:1/context:/shard:0");
        let system = machine(&input(source, "DESKTOP-1", json!({"Name": "DESKTOP-1", "Domain": "WORKGROUP", "DomainRole": 0, "Manufacturer": "ACME", "Model": "X"})), &context);
        let product = machine(&input(source, "11111111-2222-3333-4444-555555555555", json!({"UUID": "11111111-2222-3333-4444-555555555555", "Vendor": "ACME"})), &context);
        // Two item streams, one machine: the identity is the machine the collection scope names.
        assert_eq!(key_of(&system.observations[0].identity), key_of(&product.observations[0].identity));
        assert!(system.observations[0].identity.missing.is_empty());
        let uuid = product.observations[0].fields.iter().find(|field| field.field_id == "uuid").expect("uuid mapped");
        assert_eq!(uuid.status, ValueStatus::Known);
        assert!(!system.observations[0].fields.iter().any(|field|field.field_id=="uuid"));
        let product_named=machine(&input(source,"product",json!({"UUID":"u","Name":"model number"})),&context);
        assert!(!product_named.observations[0].fields.iter().any(|field|field.field_id=="name"));
    }
}
