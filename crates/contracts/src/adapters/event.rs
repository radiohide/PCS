//! The one entry point for `pcscope.eventlog.*` sources.
//!
//! Event material is not object material. An event states that something happened, names what it
//! happened to, and says nothing about the rest of that object — so it becomes an *action*, and the
//! type it targets comes from the catalog's action definition rather than from the source. That is
//! why this is one adapter for every event channel instead of a per-source mapping: `pcscope.
//! eventlog.security` alone reaches a dozen types, and the collector has already decided which
//! action each event is.
//!
//! Two paths, in order:
//!
//!   1. **The event states its action.** The collector classifies each record and writes the action
//!      id into `kind` (`plugins/eventlog/event-schema.ps1`). When that id is registered in the
//!      catalog, the action and its target type are read from there — one place decides, and an
//!      action added later needs no change here.
//!   2. **Otherwise the record's own id is mapped.** Some material reaches this adapter without a
//!      classified `kind` (the contract fixtures do). Those fall back to the same event-id table the
//!      reference adapter has always used, so nothing that worked before changes behaviour.
//!
//! What this adapter will not do: invent an action for an event whose kind is unknown, or a target
//! for one whose action declares none. An event that cannot be attributed stays attributed to
//! nothing, and says so.
use super::*;
use crate::adapter::{ActionCandidate, ActionOutcome, TimePrecision};
use crate::reference::SecurityEventAdapter;
use serde_json::Value;

#[path = "event_state_adapter.rs"]
mod event_state_adapter;
#[path = "sysmon.rs"]
mod sysmon;

pub struct EventAdapter;

/// The event-schema field names the collectors project, per declared identity key.
///
/// The `fields` map an event carries uses the Windows event's own field names — `NewProcessId`,
/// `TaskName`, `ServiceName` — not the catalog's field ids, so the two have to be related
/// somewhere. This is that place, and it is deliberately a table rather than a rule: which field
/// names an event type publishes is knowledge about the event, not something derivable.
///
/// A key with no entry, or an entry the event did not carry, is left **missing**. That is the
/// honest outcome and not a shortcoming: an event that names a process id but not when the process
/// started cannot identify it, which is exactly the incomplete-event case the identity contract
/// already has a provisional outcome for.
fn identity_fields(type_id: &str) -> &'static [(&'static str, &'static [&'static str])] {
    match type_id {
        "runtime.process" => &[("pid", &["NewProcessId", "ProcessId", "ProcessID"])],
        "runtime.logon_session" => &[("native_id", &["TargetLogonId", "LogonId", "SubjectLogonId"])],
        "runtime.terminal_session" => &[("native_id", &["SessionId", "SessionID"])],
        "runtime.module_instance" => &[("module_path", &["ImageLoaded"])],
        // Account events use the target principal, never the actor's or a group member's SID.
        "account.user" => &[("sid", &["TargetUserSid", "TargetSid"])],
        "account.group_membership" => &[("group_sid", &["TargetSid"]),("member_sid", &["MemberSid"])],
        "startup.scheduled_task" => &[("path", &["TaskName"])],
        "startup.service" => &[("name", &["ServiceName"])],
        "hardware.driver_service" => &[("name", &["DriverName", "ServiceName"])],
        "hardware.disk" => &[("native_id", &["DeviceInstanceId"])],
        // The USN plugin names the same two things the file collectors now send, under its own
        // keys. They are not spelled the same way, which is recorded as an open item: the two
        // sources describe one file and their identities do not yet meet.
        "file.watched_file" => &[("volume_identity", &["volume"]), ("file_id", &["file_id"])],
        "network.endpoint" => &[
            ("local_address", &["SourceIp"]),
            ("local_port", &["SourcePort"]),
            ("remote_address", &["DestinationIp"]),
            ("remote_port", &["DestinationPort"]),
            ("protocol", &["Protocol"]),
            ("owning_pid", &["ProcessId", "ProcessID"]),
        ],
        // Everything else declares keys an event does not carry — a file's volume identity, a
        // registry path, a share name. Those actions still exist and are still reported; their
        // targets are provisional until some other source states the missing part.
        _ => &[],
    }
}

fn event_field(payload: &Value, names: &[&str]) -> Option<String> {
    let fields=payload.get("fields")?;
    let value=names.iter().find_map(|name|fields.get(*name))?;
    if crate::semantics::is_mask_sentinel(value){return None}
    match value {
        Value::String(text)=>Some(text.trim().to_owned()).filter(|v|!v.is_empty()),
        Value::Number(number) if number.is_i64()||number.is_u64()=>Some(number.to_string()),
        _=>None,
    }
}

/// A null SID means the account was not identified; it is not a real user account. Keep this
/// validation local to account-event attribution so unrelated identity namespaces are unchanged.
pub fn is_account_sid(value: &str) -> bool {
    let value = value.trim();
    if value.eq_ignore_ascii_case("S-1-0-0") { return false; }
    let parts: Vec<&str> = value.split('-').collect();
    if !(4..=18).contains(&parts.len()) || !parts[0].eq_ignore_ascii_case("S") || parts[1] != "1" { return false; }
    let authority = parts[2].strip_prefix("0x").or_else(||parts[2].strip_prefix("0X"))
        .map(|hex|u64::from_str_radix(hex,16)).unwrap_or_else(||parts[2].parse::<u64>());
    authority.is_ok_and(|authority|authority<(1_u64<<48)) && parts[3..].iter().all(|part|!part.is_empty() && part.bytes().all(|byte|byte.is_ascii_digit()) && part.parse::<u32>().is_ok())
}

fn explicit_text(payload: &Value, names: &[&str], legacy_names: &[&str]) -> Option<String> {
    // An explicit protected/invalid value cannot be bypassed by falling back to a different name.
    let value = names.iter().find_map(|name|payload.get("fields").and_then(|fields|fields.get(*name)))
        .or_else(||legacy_names.iter().find_map(|name|payload.get(*name)))?;
    value.as_str().map(str::trim).filter(|text|!text.is_empty()&&!crate::semantics::is_mask_sentinel(value)).map(str::to_owned)
}

/// TaskScheduler writes TaskName inside fields; older fixtures used task_name at the root.
/// A protected or malformed explicit value must not be bypassed through a legacy alias.
pub(crate) fn task_path(payload: &Value) -> Option<String> {
    explicit_text(payload, &["TaskName"], &["task_name"])
}

pub(crate) fn task_event_target(input: &EvidenceInput, context: &AdapterContext) -> ActionTarget {
    match task_path(&input.payload) {
        Some(path) => ActionTarget::Assigned { identity: identity("startup.scheduled_task",
            &installation_scope(&context.collected_scope), &input.evidence.id, &[("path",Some(path))]) },
        None => ActionTarget::Unassigned { reason: "事件未提供任务路径。".into() },
    }
}

pub(crate) fn account_event_sid(payload: &Value) -> Option<String> {
    explicit_text(payload,&["TargetUserSid","TargetSid"],&["target_user_sid","target_sid"])
        .filter(|value|is_account_sid(value))
}

pub(crate) fn account_event_target(input: &EvidenceInput, context: &AdapterContext) -> ActionTarget {
    match account_event_sid(&input.payload) {
        Some(sid) => ActionTarget::Assigned { identity: identity("account.user",&installation_scope(&context.collected_scope),&input.evidence.id,&[("sid",Some(sid))]) },
        None => ActionTarget::Unassigned { reason:"事件没有提供可确认的目标账户标识，暂不关联到账户。".into() },
    }
}

fn membership_target(input:&EvidenceInput,context:&AdapterContext,action:&str)->ActionTarget {
    let p=&input.payload;
    let id=p["event_id"].as_u64();
    let number=|v:&Value|v.as_u64().or_else(||v.as_str()?.parse().ok());
    let nested=p.get("system").and_then(|s|s.get("Version")).and_then(|v|v.get("value"));
    let version=nested.or_else(||p.get("version")).and_then(number);
    let header_ok=input.decoding.available&&input.decoding.problems.is_empty()&&version==Some(0)
        &&p.get("version").is_none_or(|v|number(v)==version)
        &&p.get("system").is_none_or(|s|s.get("Version").is_none_or(|_|nested.is_some())
            &&s.get("EventID").is_none_or(|v|v.get("value").and_then(number)==id)
            &&s.get("Provider").is_none_or(|v|v["Name"]=="Microsoft-Windows-Security-Auditing")
            &&s.get("Channel").is_none_or(|v|v["value"]=="Security"));
    let matches_id=if action=="identity.group_member_added"{matches!(id,Some(4728|4732|4756))}else{matches!(id,Some(4729|4733|4757))};
    let values=crate::event_state::field(input,&["TargetSid"]).zip(crate::event_state::field(input,&["MemberSid"]));
    if header_ok&&input.source=="pcscope.eventlog.security"&&p["channel"]=="Security"&&p["provider"]=="Microsoft-Windows-Security-Auditing"&&matches_id {
        if let Some((group,member))=values.filter(|(group,member)|is_account_sid(group)&&is_account_sid(member)) {
            return ActionTarget::Assigned{identity:identity("account.group_membership",&installation_scope(&context.collected_scope),&input.evidence.id,&[("group_sid",Some(group.to_ascii_uppercase())),("member_sid",Some(member.to_ascii_uppercase()))])}
        }
    }
    ActionTarget::Unassigned{reason:"成员变更记录缺少有效的组与成员标识。".into()}
}

pub(crate) fn logon_scope(context: &AdapterContext) -> String {
    let scope=installation_scope(&context.collected_scope);
    match context.boot_id.as_deref().map(str::trim).filter(|boot|!boot.is_empty()) {
        Some(boot) => format!("{scope}/boot:{boot}"),
        None => scope,
    }
}

pub(crate) fn normalized_logon_id(value: &str) -> Option<String> {
    value.strip_prefix("0x").or_else(||value.strip_prefix("0X"))
        .map(|hex|u64::from_str_radix(hex,16).ok()).unwrap_or_else(||value.parse::<u64>().ok())
        .filter(|number|*number!=0).map(|number|format!("0x{number:x}"))
}

pub(crate) fn logon_event_id(payload: &Value, boot_known: bool) -> Option<String> {
    let value=explicit_text(payload,&["TargetLogonId","LogonId","SubjectLogonId"],&["target_logon_id","logon_id","subject_logon_id"])?;
    if boot_known {normalized_logon_id(&value)} else {Some(value)}
}

/// 4672 describes privileges on a new logon token, not a persistent LSA right assignment.
/// A logon identifier is reusable after restart. Only an event's proven boot scope can make
/// this a reliable session reference; the current machine boot must not be assumed for old logs.
pub(crate) fn special_logon_target(input: &EvidenceInput, context: &AdapterContext) -> ActionTarget {
    let boot_known=context.boot_id.as_deref().is_some_and(|boot|!boot.trim().is_empty());
    let logon=explicit_text(&input.payload,&["SubjectLogonId"],&["subject_logon_id"])
        .and_then(|value|normalized_logon_id(&value));
    match (boot_known,logon) {
        (true,Some(logon)) => ActionTarget::Assigned { identity:identity("runtime.logon_session",&logon_scope(context),&input.evidence.id,&[("native_id",Some(logon))]) },
        (false,_) => ActionTarget::Unassigned { reason:"已记录登录获得特殊权限；缺少该事件所属启动周期，暂不关联登录会话。".into() },
        (_,None) => ActionTarget::Unassigned { reason:"已记录登录获得特殊权限；事件没有提供有效登录会话标识。".into() },
    }
}

/// Whether a source's material is events rather than objects.
///
/// The event-log channels above all, and the USN plugin's per-drive sources, which emit the same
/// `event_batch` shape with a `kind` of their own. `pcscope.usn.volumes` is deliberately not here:
/// it emits a snapshot of volumes, and a snapshot is an object.
pub fn is_event_source(source: &str) -> bool {
    if EVENT_EXCLUSIONS.contains(&source) {
        return false;
    }
    EVENT_SOURCES.contains(&source) || source.starts_with("pcscope.eventlog.") || source.starts_with("pcscope.usn.")
}

/// Sources whose material is events without belonging to a collector's channels.
///
/// `pcscope.core` is this application's own activity — a collection item switched on or off, the
/// background starting and stopping. It arrives as an event batch like any other, and without being
/// recognised as one it would queue for projection, find no adapter, and be reported as a parse
/// failure. The health page would then show a screen of errors that are really the application
/// describing itself, which is the opposite of what that page is for.
pub const EVENT_SOURCES: &[&str] = &["pcscope.core"];

/// Sources that share an event prefix without being event channels.
///
/// `pcscope.usn.volumes` emits a snapshot of volumes, and a snapshot is an object. `pcscope.usn.
/// changes` is a virtual group over the per-drive sources with no collector behind it. `pcscope.
/// usn.watch-list` reports the watch list's own state, which is coverage rather than an object.
/// The list is exported with the coverage artifact so the reconciliation applies this same rule
/// instead of keeping a second copy of it.
pub const EVENT_EXCLUSIONS: &[&str] = &["pcscope.usn.volumes", "pcscope.usn.changes", "pcscope.usn.watch-list"];

/// The action id an event states, when it states one that the catalog knows.
///
/// The classification reaches this adapter as the evidence's `item_id`, because that is where the
/// contract puts it: `Event.kind` is a field of the envelope, `record_event` stores it as the
/// item's identity, and `EvidenceInput.item_id` is documented as the "classification identity
/// within the source". It is *not* in the payload — a collector's `data` is the material it
/// projected (`fields`, `system`, `field_schema`), and no producer duplicates the classification
/// into it. Reading only the payload therefore never fired on real material: every classified
/// event fell through to the id table below, which cannot name a task by its path, so the run log
/// landed in a provisional object of its own beside the task it describes.
///
/// The payload is still consulted first so that material which does state its action there — the
/// contract fixtures — keeps deciding by exactly what it states.
fn stated_action(input: &EvidenceInput) -> Option<(&'static str, &'static str)> {
    let payload_kind = input.payload.get("kind").and_then(Value::as_str);
    let kind = payload_kind
        .filter(|kind| crate::model::action_definition(kind).is_some())
        .unwrap_or(input.item_id.as_str());
    let definition = crate::model::action_definition(kind)?;
    let target = definition["target_type"].as_str()?;
    // The id has to be a `'static` registered one, so it is looked up rather than borrowed.
    Some((crate::model::registered_action_id(kind)?, target))
}

/// The catalog's own statement of what the action's result is, so an adapter never hardcodes one.
fn declared_outcome(action_id: &str) -> ActionOutcome {
    match crate::model::action_definition(action_id).and_then(|definition| definition["result"].as_str()) {
        Some("succeeded") => ActionOutcome::Succeeded,
        Some("failed") => ActionOutcome::Failed,
        Some("started") => ActionOutcome::Started,
        Some("ended") => ActionOutcome::Ended,
        Some("blocked") => ActionOutcome::Blocked,
        Some("observed") => ActionOutcome::Observed,
        _ => ActionOutcome::Unknown,
    }
}

impl SourceAdapter for EventAdapter {
    fn adapter_id(&self) -> &'static str {
        "adapter.eventlog"
    }
    fn adapt(&self, input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
        if !is_event_source(&input.source) {
            return AdapterResult::not_an_object(format!("{} is not event material", input.source));
        }
        if input.source=="pcscope.eventlog.microsoft-windows-sysmon/operational"||input.payload["provider"]=="Microsoft-Windows-Sysmon"||input.item_id=="execution.sysmon_event" {
            if let Some(action)=sysmon::action(input,context){let mut result=AdapterResult::adapted();result.actions.push(action);return result;}
            return AdapterResult::not_an_object("Sysmon event schema is not supported; original event retained");
        }
        if let Some(action)=event_state_adapter::state_action(input,context) {
            let mut result=AdapterResult::adapted();
            result.actions.push(action);
            return result;
        }
        let Some((action_id, target_type)) = stated_action(input) else {
            // The event did not state a registered action, so the record's own id decides —
            // unchanged behaviour for material that predates the classification.
            return SecurityEventAdapter.adapt(input, context);
        };
        let scope = if target_type.starts_with("account.") { installation_scope(&context.collected_scope) }
            else if matches!(target_type,"runtime.logon_session"|"runtime.terminal_session") { logon_scope(context) }
            else { context.collected_scope.clone() };
        // The keys come from the type, not from this table: a key the event has no field for must
        // still be listed, so that the identity says it is missing instead of quietly dropping it.
        // The table only says which event field names to read for a key that has any.
        let declared = crate::model::type_definition(target_type)
            .and_then(|definition| definition["identity"]["keys"].as_array().cloned())
            .map(|keys| keys.iter().filter_map(|key| key.as_str().map(str::to_owned)).collect::<Vec<String>>())
            .unwrap_or_default();
        let table = identity_fields(target_type);
        let keys: Vec<(&str, Option<String>)> = declared
            .iter()
            .filter(|key| key.as_str() != "scope")
            .map(|key| {
                let names = table.iter().find(|(declared_key, _)| declared_key == &key.as_str()).map(|(_, names)| *names).unwrap_or(&[]);
                let value=if target_type=="runtime.logon_session" && key=="native_id" {
                    logon_event_id(&input.payload,context.boot_id.as_deref().is_some_and(|boot|!boot.trim().is_empty()))
                } else { event_field(&input.payload, names) };
                (key.as_str(), value)
            })
            .collect();
        let identity = identity(target_type, &scope, &input.evidence.id, &keys);
        let target = if action_id=="identity.special_privileges" { special_logon_target(input,context) }
            else if target_type=="account.user" { account_event_target(input,context) }
            else if target_type=="account.group_membership" { membership_target(input,context,action_id) }
            else if target_type=="startup.scheduled_task" { task_event_target(input,context) }
            else { ActionTarget::Assigned { identity } };
        let mut result = AdapterResult::adapted();
        result.actions.push(ActionCandidate {
            occurrence_key: None, source_record: None,
            target,
            action_id: action_id.into(),
            // The catalog declares no stages; the action's own id says which stage it is.
            phase: None,
            result: declared_outcome(action_id),
            precision: TimePrecision::Exact,
            occurred_at: Some(input.source_time.clone().unwrap_or_else(|| input.observed_at.clone())),
            interval_from: None,
            interval_until: None,
            observed_at: input.observed_at.clone(),
            field_id: None,
            before: None,
            after: None,
            evidence: vec![input.evidence.clone()],
            actor: event_field(&input.payload, &["SubjectUserName"]),
            program: event_field(&input.payload, &["NewProcessName", "Image", "ProcessName"]),
        });
        // The event's own time is what it is; whether the source stated one is a fact about the
        // material, and an event without it is stamped derived rather than pretending to be exact.
        if input.payload.get("source_time_available").and_then(Value::as_bool) == Some(false) {
            result.actions[0].precision = TimePrecision::ObservationOnly;
            result.actions[0].occurred_at = None;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{AdapterOutcome, EvidenceKind};
    use serde_json::json;

    #[test]
    fn malformed_or_protected_identity_does_not_fall_back_to_an_alias() {
        for value in [serde_json::Value::Null,json!({"sensitive":true,"ciphertext":"secret"}),json!(["id"]),json!(true),json!(crate::sensitive::MASK)] {
            assert!(event_field(&json!({"fields":{"ServiceName":value,"DriverName":"fallback"}}),&["ServiceName","DriverName"]).is_none());
        }
        assert_eq!(event_field(&json!({"fields":{"SessionID":12}}),&["SessionID"]),Some("12".into()));
    }

    const CHANNEL: &str = "pcscope.eventlog.security";

    fn input(event_id: i64, kind: &str, fields: Value) -> EvidenceInput {
        EvidenceInput {
            source: CHANNEL.into(),
            item_id: "100".into(),
            structure_version: "v1".into(),
            source_scope: "machine:M/user:S-1-5-18/schema:1/context:/shard:0".into(),
            evidence: EvidenceRef { kind: EvidenceKind::Event, id: format!("{CHANNEL}|epoch|100") },
            native_record_id: "100".into(),
            source_time: Some("2026-09-12T00:00:00Z".into()),
            observed_at: "2026-09-12T00:00:01Z".into(),
            ingested_at: "2026-09-12T00:00:02Z".into(),
            decoding: Decoding { available: true, ..Default::default() },
            payload: json!({
                "channel": "Security", "record_id": 100, "provider": "Microsoft-Windows-Security-Auditing",
                "event_id": event_id, "kind": kind, "fields": fields, "basis": "persisted Windows event evidence",
                "source_time_available": true
            }),
        }
    }

    /// The shape a collector actually sends, which `input` above does not: `collect-eventlog.ps1`
    /// puts the classified action id on the **event** (`kind=$projection.kind`) and builds `data`
    /// from the projected fields alone, so `data` holds no copy of the classification. The event's
    /// `kind` reaches the adapter as the evidence's `item_id`, which is where the contract puts a
    /// source's classification identity.
    ///
    /// Every other test here passes the classification inside the payload, which is a shape no
    /// producer emits — which is how the payload-only lookup below went unnoticed.
    fn collected(event_id: i64, kind: &str, fields: Value) -> EvidenceInput {
        let mut event = input(event_id, kind, fields);
        event.item_id = kind.into();
        event.payload.as_object_mut().expect("an event's data is an object").remove("kind");
        event
    }

    fn context() -> AdapterContext {
        AdapterContext {
            catalog_version: crate::model::catalog_version().into(),
            rule_version: crate::model::catalog_fingerprint(),
            machine_id: "M".into(),
            installation_id: "I".into(),
            boot_id: None,
            collected_scope: "machine:M/install:I".into(),
            complete: true,
        }
    }

    #[test]
    fn task_paths_accept_native_and_legacy_shapes_without_bypassing_protection() {
        for payload in [json!({"fields":{"TaskName":"\\Folder\\Task"}}),json!({"task_name":"\\Folder\\Task"})] {
            assert_eq!(task_path(&payload).as_deref(),Some("\\Folder\\Task"));
        }
        for value in [Value::Null,json!(" "),json!({"protected":true}),json!(["\\Task"])] {
            assert!(task_path(&json!({"fields":{"TaskName":value},"task_name":"\\DoNotUse"})).is_none());
        }
    }

    #[test]
    fn an_event_that_states_its_action_gets_it_from_the_catalog() {
        let payload = input(4688, "execution.process_started", json!({"NewProcessId": "1234", "NewProcessName": "C:\\Example\\example.exe"}));
        let result = EventAdapter.adapt(&payload, &context());
        assert_eq!(result.actions.len(), 1);
        let action = &result.actions[0];
        assert_eq!(action.action_id, "execution.process_started");
        // The result is the catalog's statement, not this adapter's.
        assert_eq!(action.result, ActionOutcome::Started);
        assert!(matches!(action.target, ActionTarget::Assigned { .. }));
    }

    #[test]
    fn the_target_identity_is_read_from_the_events_own_field_names() {
        let payload = input(4688, "execution.process_started", json!({"NewProcessId": "1234"}));
        let result = EventAdapter.adapt(&payload, &context());
        let ActionTarget::Assigned { identity } = &result.actions[0].target else { panic!("assigned target") };
        assert_eq!(identity.type_id, "runtime.process");
        assert!(identity.fields.iter().any(|field| field.key == "pid" && field.value == "1234"));
    }

    #[test]
    fn an_event_that_cannot_name_the_whole_identity_says_which_part_is_missing() {
        // A 4688 names the process id but never when it started, and the type asks for both.
        let payload = input(4688, "execution.process_started", json!({"NewProcessId": "1234"}));
        let result = EventAdapter.adapt(&payload, &context());
        let ActionTarget::Assigned { identity } = &result.actions[0].target else { panic!("assigned target") };
        assert_eq!(identity.missing, vec!["created_at".to_owned()]);
        // The action is still reported; only the target is provisional.
        assert_eq!(result.actions[0].action_id, "execution.process_started");
    }

    #[test]
    fn the_classification_is_read_from_the_event_when_the_payload_does_not_state_one() {
        // A task run as the run log reports it: the event says which action it is, and `data`
        // carries `TaskName` and nothing else about the classification.
        let payload = collected(100, "startup.task_started", json!({"TaskName": "\\A\\task"}));
        let result = EventAdapter.adapt(&payload, &context());
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].action_id, "startup.task_started");
        // The classified path carries no phase: the action's own id already says which stage it is.
        // The id table below would have set one, so this is what says which path answered.
        assert!(result.actions[0].phase.is_none(), "the event's own action id decided, not the id table");
        let ActionTarget::Assigned { identity } = &result.actions[0].target else { panic!("assigned target") };
        assert_eq!(identity.type_id, "startup.scheduled_task");
        // Named by its path, so it lands on the task a configuration source already describes
        // rather than in a provisional object of its own beside it.
        assert!(identity.fields.iter().any(|field| field.key == "path" && field.value == "\\A\\task"), "{:?}", identity.fields);
        assert!(identity.missing.is_empty(), "the task's path is known: {:?}", identity.missing);
    }

    #[test]
    fn an_unclassified_event_still_reaches_its_action_through_the_record_id() {
        // Material that predates the collector's classification states no registered action, so the
        // record's own id decides — the behaviour that was there before must not change.
        let mut payload = input(100, "windows.event", json!({"TaskName": "\\Example"}));
        payload.source = "pcscope.eventlog.microsoft-windows-taskscheduler/operational".into();
        payload.payload["channel"] = json!("Microsoft-Windows-TaskScheduler/Operational");
        payload.payload["provider"] = json!("Microsoft-Windows-TaskScheduler");
        let result = EventAdapter.adapt(&payload, &context());
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].action_id, "startup.task_started");
        assert_eq!(result.actions[0].phase.as_deref(), Some("task run"));
    }

    #[test]
    fn application_event_numbers_never_create_task_or_security_actions() {
        for event_id in [100, 102, 4624, 4625, 4672, 4688, 4720] {
            let mut payload = input(event_id,"windows.event",json!({"TaskName":"\\Misleading"}));
            payload.source="pcscope.eventlog.application".into();
            payload.payload["provider"]=json!("Example application");
            payload.payload["channel"]=json!("Application");
            assert!(EventAdapter.adapt(&payload,&context()).actions.is_empty(),"an event number alone cannot classify {event_id}");
        }
        let mut wrong_provider=input(4624,"windows.event",json!({}));
        wrong_provider.payload["provider"]=json!("Example application");
        assert!(EventAdapter.adapt(&wrong_provider,&context()).actions.is_empty());
    }

    #[test]
    fn special_logon_privileges_without_a_proven_boot_remain_unassigned_events() {
        for kind in ["identity.special_privileges","windows.event"] {
            let input=input(4672,kind,json!({"SubjectUserSid":"S-1-5-18","SubjectUserName":"SYSTEM","SubjectLogonId":"0x3e7","PrivilegeList":"SeBackupPrivilege SeDebugPrivilege"}));
            let original=input.clone();
            let result=EventAdapter.adapt(&input,&context());
            assert_eq!(result.actions.len(),1);
            assert!(result.observations.is_empty());
            assert_eq!(result.actions[0].action_id,"identity.special_privileges");
            assert_eq!(result.actions[0].result,ActionOutcome::Observed);
            assert!(matches!(&result.actions[0].target,ActionTarget::Unassigned{reason} if reason.contains("启动周期")));
            assert_eq!(result.actions[0].evidence,vec![input.evidence.clone()]);
            assert_eq!(input,original,"the raw event and its privilege list are unchanged");
            assert!(crate::adapter::validate(&result,&context()).is_empty());
        }
        assert_eq!(crate::model::action_definition("identity.special_privileges").unwrap()["target_type"],"runtime.logon_session");
        assert!(!crate::model::type_definition("account.right_assignment").unwrap()["actions"].as_array().unwrap().iter().any(|id|id=="identity.special_privileges"));
    }

    #[test]
    fn a_proven_boot_and_logon_identifier_link_privileges_to_the_same_logon() {
        let mut ctx=context();
        ctx.collected_scope="machine:M/install:I/user:collector".into();
        ctx.boot_id=Some("boot-A".into());
        for kinds in [["identity.logon","identity.special_privileges"],["windows.event","windows.event"]] {
            let logon=input(4624,kinds[0],json!({"TargetLogonId":"0x000003E7","TargetUserSid":"S-1-5-18"}));
            let privileges=input(4672,kinds[1],json!({"SubjectLogonId":"0X3e7","SubjectUserSid":"S-1-5-18"}));
            let session=EventAdapter.adapt(&logon,&ctx);
            let rights=EventAdapter.adapt(&privileges,&ctx);
            let ActionTarget::Assigned{identity:left}=&session.actions[0].target else {panic!("logon identity")};
            let ActionTarget::Assigned{identity:right}=&rights.actions[0].target else {panic!("privilege session identity")};
            let strategy=crate::identity::TypeStrategy::for_type("runtime.logon_session").unwrap();
            assert_eq!(crate::identity::normalise(&strategy,left),crate::identity::normalise(&strategy,right));
            assert_eq!(right.type_id,"runtime.logon_session");
            assert_eq!(right.scope,"machine:M/install:I/boot:boot-A");
            assert!(right.missing.is_empty());
            let mut next_boot=ctx.clone();next_boot.boot_id=Some("boot-B".into());
            let other=EventAdapter.adapt(&privileges,&next_boot);
            let ActionTarget::Assigned{identity:other}=&other.actions[0].target else {panic!("next boot")};
            assert_ne!(crate::identity::normalise(&strategy,right),crate::identity::normalise(&strategy,other));
        }
    }

    #[test]
    fn special_privileges_do_not_guess_a_missing_or_protected_logon_identifier() {
        let mut ctx=context();ctx.boot_id=Some("boot-A".into());
        for value in [Value::Null,json!("0x0"),json!("-"),json!("not-an-id"),json!({"sensitive":true,"display":"******"})] {
            for kind in ["identity.special_privileges","windows.event"] {
                let input=input(4672,kind,json!({"SubjectLogonId":value,"TargetLogonId":"0x3e7","SubjectUserSid":"S-1-5-18"}));
                assert!(matches!(EventAdapter.adapt(&input,&ctx).actions[0].target,ActionTarget::Unassigned{..}));
            }
        }
        // Historical flat payloads use the same safety checks as the collected fields map.
        let mut input=input(4672,"windows.event",json!({}));
        input.payload["subject_logon_id"]=json!("0x3e7");
        assert!(matches!(EventAdapter.adapt(&input,&ctx).actions[0].target,ActionTarget::Assigned{..}));
    }

    #[test]
    fn failed_logons_never_confirm_null_sids_or_substitute_the_reporting_account() {
        for value in [Value::Null,json!("S-1-0-0"),json!(""),json!("-"),json!("S-1-5-not-a-sid"),json!({"sensitive":true,"display":"******"})] {
            for kind in ["identity.logon_failed","windows.event"] {
                let input=input(4625,kind,json!({"TargetUserSid":value,"TargetUserName":"attempted-name","SubjectUserSid":"S-1-5-18"}));
                let result=EventAdapter.adapt(&input,&context());
                assert_eq!(result.actions.len(),1);
                assert!(matches!(result.actions[0].target,ActionTarget::Unassigned{..}),"{kind}: {value}");
                assert!(result.observations.is_empty());
                assert!(crate::adapter::validate(&result,&context()).is_empty());
            }
        }
        let mut flat=input(4625,"windows.event",json!({}));
        flat.payload["target_user_sid"]=json!("S-1-0-0");
        flat.payload["subject_user_sid"]=json!("S-1-5-18");
        assert!(matches!(EventAdapter.adapt(&flat,&context()).actions[0].target,ActionTarget::Unassigned{..}));
    }

    #[test]
    fn account_events_use_the_target_sid_and_the_same_installation_scope_as_snapshots() {
        let sid="S-1-5-21-1-2-3-1001";
        let mut ctx=context();ctx.collected_scope="machine:M/install:I/user:collector".into();
        for (event_id,kind,key) in [(4720,"identity.account_created","TargetSid"),(4625,"identity.logon_failed","TargetUserSid")] {
            for classified in [kind,"windows.event"] {
                let mut input=input(event_id,classified,json!({"SubjectUserSid":"S-1-5-18"}));
                input.payload["fields"][key]=json!(sid);
                let result=EventAdapter.adapt(&input,&ctx);
                let ActionTarget::Assigned{identity:event_identity}=&result.actions[0].target else {panic!("known target SID")};
                let mut snapshot=input.clone();snapshot.source="pcscope.identity.accounts".into();snapshot.item_id=sid.into();snapshot.payload=json!({"name":"Account"});
                let object=(super::super::account::SOURCES[0].build)(&snapshot,&ctx);
                let strategy=crate::identity::TypeStrategy::for_type("account.user").unwrap();
                assert_eq!(crate::identity::normalise(&strategy,event_identity),crate::identity::normalise(&strategy,&object.observations[0].identity));
                assert!(event_identity.fields.iter().any(|field|field.key=="sid"&&field.value==sid));
            }
        }
    }

    #[test]
    fn an_event_nothing_maps_to_is_kept_without_an_invented_action() {
        let payload = input(9999, "windows.event", json!({"provider": "Example"}));
        let result = EventAdapter.adapt(&payload, &context());
        assert!(result.actions.is_empty(), "no action is invented for an unmapped event");
        assert!(result.problems.iter().any(|problem| problem.message.contains("not mapped to a target type")));
    }

    #[test]
    fn an_event_without_a_stated_time_is_not_stamped_exact() {
        let mut payload = input(4688, "execution.process_started", json!({"NewProcessId": "1234"}));
        payload.payload["source_time_available"] = Value::Bool(false);
        let result = EventAdapter.adapt(&payload, &context());
        // The time it carries is the observation time, and the action says so rather than claiming
        // the source stated when it happened.
        assert_eq!(result.actions[0].precision, TimePrecision::ObservationOnly);
        assert!(result.actions[0].occurred_at.is_none());
    }

    #[test]
    fn material_from_another_source_is_not_event_material() {
        let mut payload = input(4688, "execution.process_started", json!({}));
        payload.source = "pcscope.startup.services".into();
        let result = EventAdapter.adapt(&payload, &context());
        assert!(result.actions.is_empty());
        assert!(matches!(&result.outcome, AdapterOutcome::NotAnObject { reason } if reason.contains("not event material")), "{:?}", result.outcome);
    }

    #[test]
    fn the_same_event_twice_produces_the_same_action() {
        let payload = input(4688, "execution.process_started", json!({"NewProcessId": "1234", "NewProcessName": "C:\\Example\\example.exe"}));
        let once = EventAdapter.adapt(&payload, &context());
        let twice = EventAdapter.adapt(&payload, &context());
        assert_eq!(once.actions[0].action_id, twice.actions[0].action_id);
        assert_eq!(once.actions[0].evidence, twice.actions[0].evidence);
        assert_eq!(once.actions[0].occurred_at, twice.actions[0].occurred_at);
    }
}
