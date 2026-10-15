//! R05 fixed response set for the query contract.
//!
//! usage: query-contract [output-path]   (default: model/query-samples.json)
//!
//! Builds one response per state the frontend must render, checks that every required state and
//! display case is present, and writes the set. Exits non-zero if anything is missing, so the set
//! cannot silently lose a case.
use pcscope_contracts::adapter::EvidenceRef;
use pcscope_contracts::model;
use pcscope_contracts::query::*;
use serde_json::{json, Value};

fn response(status: ResponseStatus, data: Value) -> QueryResponse {
    QueryResponse { contract_version: 1, catalog_version: model::catalog_version().into(), status, data: Some(data), warnings: Vec::new(), page: None, counts: None, mock: true }
}
fn field(field_id: &str, label: &str, kind: &str, value: Option<Value>, status: &str, observed_at: &str, evidence: Vec<EvidenceRef>) -> FieldView {
    // Compute the type before the value is moved into the struct.
    let value_type = match value.as_ref() {
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::Array(_)) => "list",
        Some(Value::Object(_)) => "object",
        Some(Value::Null) | None => "unknown",
        Some(Value::String(_)) => "string",
    };
    FieldView {
        field_id: field_id.into(), label: label.into(), kind: kind.into(), value,
        value_type: value_type.into(),
        status: status.into(), source_time: None, observed_at: observed_at.into(),
        derivation: "source_provided".into(), evidence, conflicts: Vec::new(),
    }
}
fn coverage(source: &str, result: &str, history: &str, configured: bool) -> CoverageView {
    CoverageView {
        source: source.into(), scope: "machine:示例机器/install:示例安装".into(), configured,
        last_result: result.into(),
        last_success_at: if result == "success" { Some("2026-09-12T09:00:00.000Z".into()) } else { None },
        last_attempt_at: Some("2026-09-12T09:00:00.000Z".into()),
        history_state: history.into(), detail: None,
        // A denied source still has outstanding work, and a source whose material no adapter
        // understands fails here rather than at collection. The sample carries both so the health
        // page's two failure kinds are both exercised.
        projection: ProjectionCounts {
            pending: if result == "denied" { 128 } else { 0 },
            failed: if result == "partial" { 2 } else { 0 },
            done: 4096,
        },
    }
}
fn task_summary() -> InstanceSummary {
    InstanceSummary {
        instance_id: Some(1041),
        alias: "startup.scheduled_task@示例机器|示例安装|install|\\示例文件夹\\示例任务#1".into(),
        type_id: "startup.scheduled_task".into(),
        category: "startup".into(),
        type_name: "计划任务".into(),
        record_basis: None,
        presentation: None,
        display_name: "示例任务".into(),
        scope: "install".into(),
        identity_status: "confirmed".into(),
        lifecycle: "active".into(),
        key_facts: vec![
            field("path", "任务路径", "identity", Some(json!("\\示例文件夹\\示例任务")), "known", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11)]),
            field("enabled", "是否启用", "configuration", Some(json!(true)), "known", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11)]),
            field("next_run", "下次运行时间", "state", Some(json!("2026-09-12T18:00:00.000Z")), "known", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11)]),
        ],
        changed_at: Some("2026-09-12T08:00:00.000Z".into()),
        seen_at: Some("2026-09-12T09:00:00.000Z".into()),
        coverage: vec![coverage("pcscope.startup.tasks", "success", "caught_up", true)],
    }
}

fn samples() -> Vec<(String, QueryResponse)> {
    let mut set = Vec::new();

    // 1. Type catalog, built from the embedded catalog so it cannot go stale.
    let entries: Vec<TypeCatalogEntry> = model::types().iter().filter_map(|entry| Some(TypeCatalogEntry {
        type_id: entry["id"].as_str()?.into(),
        category: entry["category"].as_str()?.into(),
        name: entry["name"].as_str()?.into(),
        supplied_by_source: entry["identity"].is_object(),
    })).collect();
    set.push(("type_catalog".into(), response(ResponseStatus::Loaded, serde_json::to_value(&entries).unwrap())));

    // 2. Instance list: a confirmed instance, a provisional one, and one no longer observed.
    let provisional_process = InstanceSummary {
        instance_id: None,
        alias: "runtime.process@示例机器|示例安装|install|4100|?created_at#1".into(),
        type_id: "runtime.process".into(), category: "runtime".into(), type_name: "进程".into(),
        record_basis: None,
        presentation: None,
        display_name: "示例程序.exe".into(), scope: "install".into(),
        identity_status: "provisional".into(), lifecycle: "active".into(),
        key_facts: vec![field("pid", "进程 ID", "identity", Some(json!("4100")), "known", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(12)])],
        changed_at: None, seen_at: Some("2026-09-12T09:00:00.000Z".into()),
        coverage: vec![coverage("pcscope.execution.processes", "partial", "catching_up", true)],
    };
    let not_observed_service = InstanceSummary {
        instance_id: Some(77),
        alias: "startup.service@示例机器|示例安装|install|示例服务#1".into(),
        type_id: "startup.service".into(), category: "startup".into(), type_name: "服务".into(),
        record_basis: None,
        presentation: None,
        display_name: "示例服务".into(), scope: "install".into(),
        identity_status: "confirmed".into(), lifecycle: "not_observed".into(),
        key_facts: vec![field("state", "状态", "state", Some(json!("Stopped")), "known", "2026-09-11T09:00:00.000Z", vec![EvidenceRef::record(9)])],
        changed_at: Some("2026-09-11T09:00:00.000Z".into()), seen_at: Some("2026-09-11T09:00:00.000Z".into()),
        coverage: vec![coverage("pcscope.startup.services", "success", "caught_up", true)],
    };
    let list = json!([task_summary(), provisional_process, not_observed_service]);
    let mut list_response = response(ResponseStatus::Loaded, list);
    list_response.page = Some(Page { next_cursor: None, limit: DEFAULT_PAGE, watermark: "wm-2026-09-12T09:00:00.000Z-1".into(), has_more: false });
    list_response.counts = Some(Counts { instances: 3, source_records: 41, actions: 7, evidence: 9 });
    set.push(("instance_list".into(), list_response));

    // 3. Instance detail: a masked field, a conflict, and a field whose evidence was pruned.
    let masked = field("principal", "运行账户", "configuration", Some(json!(pcscope_contracts::sensitive::MASK)), "masked", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11)]);
    let mut conflicting = field("actions", "执行动作", "configuration", Some(json!("--示例程序 B")), "conflicting", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11), EvidenceRef::record(13)]);
    conflicting.conflicts = vec![
        ConflictView { value: json!("--示例程序 B"), source: "pcscope.startup.tasks".into(), source_time: Some("2026-09-12T09:00:00.000Z".into()) },
        ConflictView { value: json!("--示例程序 A"), source: "pcscope.eventlog.system".into(), source_time: Some("2026-09-12T09:00:00.000Z".into()) },
    ];
    let pruned_support = field("triggers", "触发条件", "configuration", None, "evidence_pruned", "2026-09-12T09:00:00.000Z", vec![]);
    let detail = InstanceDetail {
        summary: task_summary(),
        fields: vec![
            field("path", "任务路径", "identity", Some(json!("\\示例文件夹\\示例任务")), "known", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11)]),
            field("name", "名称", "configuration", Some(json!("示例任务")), "known", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11)]),
            field("enabled", "是否启用", "configuration", Some(json!(true)), "known", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11)]),
            masked, conflicting, pruned_support,
            field("next_run", "下次运行时间", "state", Some(json!("2026-09-12T18:00:00.000Z")), "known", "2026-09-12T09:00:00.000Z", vec![EvidenceRef::record(11)]),
            field("last_run", "最近运行", "state", None, "not_provided", "2026-09-12T09:00:00.000Z", vec![]),
        ],
        actions_preview: vec![action_exact(), action_interval(), action_unassigned()],
        bindings: vec![
            BindingView { source: "pcscope.startup.tasks".into(), item_id: "pcscope.startup.tasks".into(), scope: "install".into(), native_record_id: Some("\\示例文件夹\\示例任务".into()), valid_from: "2026-09-10T00:00:00.000Z".into(), valid_to: None },
            BindingView { source: "pcscope.eventlog.microsoft-windows-taskscheduler/operational".into(), item_id: "pcscope.eventlog.microsoft-windows-taskscheduler/operational".into(), scope: "install".into(), native_record_id: Some("\\示例文件夹\\示例任务".into()), valid_from: "2026-09-12T08:00:00.000Z".into(), valid_to: None },
        ],
    };
    let mut detail_response = response(ResponseStatus::Loaded, serde_json::to_value(&detail).unwrap());
    detail_response.counts = Some(Counts { instances: 1, source_records: 2, actions: 7, evidence: 4 });
    set.push(("instance_detail".into(), detail_response));

    // 4. Actions: exact, interval (time derived), and one whose target could not be determined.
    let mut actions_response = response(ResponseStatus::Loaded, json!([action_exact(), action_interval(), action_unassigned()]));
    actions_response.page = Some(Page { next_cursor: None, limit: DEFAULT_PAGE, watermark: "wm-2026-09-12T09:00:00.000Z-2".into(), has_more: false });
    actions_response.counts = Some(Counts { instances: 1, source_records: 3, actions: 3, evidence: 5 });
    set.push(("instance_actions".into(), actions_response));

    // 5. Evidence: one readable, one whose material retention removed.
    let available = EvidenceView {
        reference: EvidenceRef::record(11), state: "available".into(), pruned_at: None, retained_from: None,
        source: "pcscope.startup.tasks".into(), source_time: None,
        observed_at: "2026-09-12T09:00:00.000Z".into(), ingested_at: "2026-09-12T09:00:01.000Z".into(),
        decoding: "available".into(), problems: vec![], raw_available: true,
    };
    set.push(("evidence_available".into(), response(ResponseStatus::Loaded, serde_json::to_value(&available).unwrap())));
    let pruned = EvidenceView {
        reference: EvidenceRef::record(7), state: "pruned".into(),
        pruned_at: Some("2026-09-12T00:00:00.000Z".into()), retained_from: Some("2026-09-01T00:00:00.000Z".into()),
        source: "pcscope.startup.services".into(), source_time: None,
        observed_at: "2026-08-30T09:00:00.000Z".into(), ingested_at: "2026-08-30T09:00:01.000Z".into(),
        decoding: "available".into(), problems: vec!["原始材料已按保留策略清理".into()], raw_available: false,
    };
    // The status is separate from "empty": the conclusion and its observation time survive, and the
    // banner says so at the top level rather than making the page dig it out of the payload.
    let mut pruned_response = response(ResponseStatus::EvidencePruned, serde_json::to_value(&pruned).unwrap());
    pruned_response.warnings = vec![Warning {
        code: "evidence_pruned".into(),
        message: "支撑证据已按保留策略清理；当前结论及其观察时间仍然保留，完整历史材料不可用".into(),
        at: Some("record".into()),
    }];
    set.push(("evidence_pruned".into(), pruned_response));

    // 6. Coverage: success, partial, denied, missing, and a source that is not configured at all.
    let coverages = json!([
        coverage("pcscope.startup.tasks", "success", "caught_up", true),
        coverage("pcscope.eventlog.security", "denied", "catching_up", true),
        coverage("pcscope.security.encryption", "denied", "catching_up", true),
        coverage("pcscope.virtualization.hyperv-vms", "missing", "source_missing", true),
        coverage("pcscope.apps.mihomo", "never", "caught_up", false),
    ]);
    let mut coverage_response = response(ResponseStatus::Partial, coverages);
    coverage_response.warnings = vec![
        Warning { code: "source_denied".into(), message: "来源读取被拒绝；本对象的这部分属性暂不可用".into(), at: Some("pcscope.eventlog.security".into()) },
        Warning { code: "source_missing".into(), message: "来源不存在或组件未安装".into(), at: Some("pcscope.virtualization.hyperv-vms".into()) },
    ];
    set.push(("source_coverage".into(), coverage_response));

    // 7. Projection status.
    let projections = json!([
        ProjectionStatusView { projection: "objects".into(), rule_version: model::catalog_fingerprint(), schema_version: 6, cursor: Some("4100".into()), watermark: Some("wm-1".into()), pending: 0, failed: 0, last_error: None, updated_at: "2026-09-12T09:00:05.000Z".into() },
        ProjectionStatusView { projection: "facts".into(), rule_version: model::catalog_fingerprint(), schema_version: 6, cursor: Some("2080".into()), watermark: Some("wm-1".into()), pending: 128, failed: 2, last_error: Some("示例解析问题：来源结构版本不受支持".into()), updated_at: "2026-09-12T09:00:05.000Z".into() },
    ]);
    set.push(("projection_status".into(), response(ResponseStatus::Loaded, projections)));

    // 7b. The action timeline: what happened, newest first, across objects.
    //
    // Two things this sample exists to show and no other sample does: an action that belongs to no
    // object, and a page with a next cursor. Both are states the timeline page has to render right,
    // and a fixed set without them would leave those paths unexercised.
    let timeline_rows = json!([
        ActionView {
            context: None,
            source_record: None,
            action_id: "startup.task_started".into(), title: "计划任务开始运行".into(), result: "started".into(),
            phase: Some("task run".into()), precision: "exact".into(),
            occurred_at: Some("2026-09-12T08:00:00.000Z".into()), interval_from: None, interval_until: None,
            observed_at: "2026-09-12T08:00:01.000Z".into(), field_id: None, before: None, after: None,
            time_derived: false, evidence: vec![EvidenceRef::event("pcscope.eventlog.microsoft-windows-taskscheduler/operational", "1", "100")],
            unassigned_reason: None,
        },
        ActionView {
            context: None,
            source_record: None,
            action_id: "observed.configuration_changed".into(), title: "观察到配置变化".into(), result: "observed".into(),
            phase: None, precision: "interval".into(),
            occurred_at: None,
            interval_from: Some("2026-09-12T07:00:00.000Z".into()), interval_until: Some("2026-09-12T07:30:00.000Z".into()),
            observed_at: "2026-09-12T07:30:00.000Z".into(), field_id: Some("enabled".into()),
            before: Some(json!(true)), after: Some(json!(false)),
            // No source recorded the edit, so the interval is what is known and the end is a bound.
            time_derived: true,
            evidence: vec![EvidenceRef::snapshot("pcscope.startup.tasks", "machine:m/install:i/schema:1/context:/shard:0", "run-7")],
            unassigned_reason: None,
        },
        ActionView {
            context: None,
            source_record: None,
            action_id: "identity.logon_failed".into(), title: "登录失败".into(), result: "failed".into(),
            phase: Some("session".into()), precision: "exact".into(),
            occurred_at: Some("2026-09-12T06:00:00.000Z".into()), interval_from: None, interval_until: None,
            observed_at: "2026-09-12T06:00:02.000Z".into(), field_id: None, before: None, after: None,
            time_derived: false, evidence: vec![EvidenceRef::event("pcscope.eventlog.security", "1", "4625")],
            // Kept with the reason it could not be attributed, rather than dropped or filed under
            // whichever account happened to be nearest.
            unassigned_reason: Some("事件只提供了用户名，没有可确认的账户标识；不猜测是哪一个账户".into()),
        },
    ]);
    let mut timeline_response = response(ResponseStatus::Loaded, timeline_rows);
    timeline_response.page = Some(Page {
        next_cursor: Some("e4102i88a210~2026-09-12T06:00:00.000Z~210".into()),
        limit: DEFAULT_PAGE,
        watermark: "e4102i88a210".into(),
        has_more: true,
    });
    timeline_response.counts = Some(Counts { instances: 2, source_records: 3, actions: 210, evidence: 4 });
    set.push(("action_timeline".into(), timeline_response));

    set.push(("source_readings".into(),response(ResponseStatus::Loaded,json!([{
        "source":"pcscope.execution.resources","scope":"machine:M","item_id":"memory","destination":"sampling",
        "observed_at":"2026-09-12T09:00:00Z","source_time":null,
        "evidence":{"kind":"snapshot","id":"sample-reading"},"payload":{"total_kib":1024,"free_kib":512},"progress":null
    }]))));

    set.push(("update_operation".into(),response(ResponseStatus::Loaded,json!([{
        "action_id":"software.update_history","title":"Windows 更新操作记录","result":"failed","phase":"installation","precision":"exact",
        "occurred_at":"2026-10-12T00:00:00Z","observed_at":"2026-10-12T00:01:00Z","instance_id":null,"unassigned_reason":"更新操作不证明当前安装状态",
        "source_record":{"title":"示例更新","update_id":"U","revision":1,"result":4,"hresult":-1,"operation":1}
    }]))));
    // 8. The remaining states: empty, unsupported, permission denied, version mismatch.
    let mut empty = response(ResponseStatus::Empty, json!([]));
    empty.page = Some(Page { next_cursor: None, limit: DEFAULT_PAGE, watermark: "wm-2026-09-12T09:00:00.000Z-3".into(), has_more: false });
    empty.counts = Some(Counts::default());
    set.push(("instance_list_empty".into(), empty));

    set.push(("unsupported".into(), QueryResponse::rejected(ResponseStatus::Unsupported, "unsupported_type", "该类型尚未接入来源；显示为未支持而不是空列表", model::catalog_version())));
    set.push(("permission_denied".into(), QueryResponse::rejected(ResponseStatus::PermissionDenied, "permission_denied", "读取该来源需要更高权限；请以管理员身份运行后台", model::catalog_version())));
    let mut mismatch = QueryResponse::rejected(ResponseStatus::VersionMismatch, "catalog_version_mismatch", "前端与后台的模型目录版本不一致；请重新构建", "9.9.9");
    mismatch.mock = true;
    set.push(("version_mismatch".into(), mismatch));

    // Presentation revision 1 adds optional labels without replacing the original 15 cases.
    // The old list/action samples deliberately keep these fields absent for compatibility checks.
    let mut named = task_summary();
    named.presentation = Some(json!({"name":"示例任务","name_status":"source_provided","basis":"object_facts"}));
    let unnamed = InstanceSummary {
        instance_id: Some(2041), alias: "software.installation_registration@示例机器|示例安装|install|unnamed-registration#1".into(),
        type_id: "software.installation_registration".into(), category: "software".into(), type_name: "软件安装登记".into(),
        display_name: String::new(), scope: "install".into(), identity_status: "confirmed".into(), lifecycle: "active".into(),
        key_facts: vec![], changed_at: None, seen_at: Some("2026-09-12T09:00:00.000Z".into()),
        coverage: vec![coverage("pcscope.software.inventory", "success", "caught_up", true)],
        record_basis: None,
        presentation: Some(json!({"name":"未命名的软件登记","name_status":"not_provided","basis":"registration_without_display_name","description":"系统保留了这条登记，尚不能据此确认对应软件是否已安装。"})),
    };
    let mut dictionary = unnamed.clone();
    dictionary.instance_id = Some(2042);
    dictionary.alias = "software.installation_registration@示例机器|示例安装|install|fixture-component-registration#1".into();
    dictionary.presentation = Some(json!({
        "name":"示例系统组件登记","description":"用于示例系统组件的安装维护。","name_status":"dictionary_matched","basis":"registration_dictionary",
        "dictionary_id":"fixture-registration-dictionary","dictionary_revision":1,"dictionary_entry_id":"fixture-component",
        "match_basis":"合成样本：精确匹配登记位置与视图","technical_key":"HKLM|Registry64|Software\\Example\\Uninstall\\Fixture","references":[]
    }));
    set.push(("instance_list_presentation".into(), response(ResponseStatus::Loaded, json!([named, unnamed, dictionary]))));
    let mut historical = action_exact();
    historical.source_record = Some(json!({"provider":"Microsoft-Windows-Security-Auditing","event_id":4702,"fields":{"TaskName":"\\历史文件夹\\历史任务名称"}}));
    historical.context = Some(json!({
        "type_id":"startup.scheduled_task","type_name":"计划任务","display_name":"示例任务","key_facts":[],
        "presentation":{"name":"示例任务","name_status":"source_provided","basis":"object_facts"},
        "subject_label":"历史任务名称","subject_detail":"\\历史文件夹\\历史任务名称","subject_basis":"event_evidence",
        "field_label":null,"stored_instance_id":1041,"attribution_status":"mismatch","attribution_reason":"这条记录的对象信息与历史关联不一致，尚不能确认归属。"
    }));
    historical.unassigned_reason = Some("这条记录的对象信息与历史关联不一致，尚不能确认归属。".into());
    let mut protected = action_exact();
    protected.action_id = "identity.logon".into(); protected.title = "登录记录".into();
    protected.source_record = Some(json!({"provider":"Microsoft-Windows-Security-Auditing","event_id":4624,"fields":{"TargetUserName":pcscope_contracts::sensitive::MASK,"LogonType":"5"}}));
    protected.context = Some(json!({
        "type_id":"account.user","type_name":"用户","display_name":null,"key_facts":[],
        "presentation":{"name":null,"name_status":"not_provided","basis":"object_facts"},
        "subject_label":null,"subject_detail":null,"subject_basis":"unavailable","field_label":null,
        "logon_type":"5","stored_instance_id":null,"attribution_status":"unassigned","attribution_reason":"原始账户名称已隐藏，尚不能确认涉及的账户。"
    }));
    protected.unassigned_reason = Some("原始账户名称已隐藏，尚不能确认涉及的账户。".into());
    set.push(("action_timeline_context".into(), response(ResponseStatus::Loaded, json!([historical, protected]))));

    for (_, sample) in set.iter_mut() {
        sample.mock = true;
    }
    set
}

fn action_exact() -> ActionView {
    ActionView {
        context: None,
        source_record: None,
        action_id: "startup.task_updated".into(), title: "任务更新".into(), result: "succeeded".into(),
        phase: Some("task lifecycle".into()), precision: "exact".into(),
        occurred_at: Some("2026-09-12T08:00:00.000Z".into()), interval_from: None, interval_until: None,
        observed_at: "2026-09-12T09:00:00.000Z".into(), field_id: None, before: None, after: None,
        time_derived: false, evidence: vec![EvidenceRef::record(11)], unassigned_reason: None,
    }
}
fn action_interval() -> ActionView {
    ActionView {
        context: None,
        source_record: None,
        action_id: "observed.configuration_changed".into(), title: "观察到配置变化".into(), result: "observed".into(),
        phase: None, precision: "interval".into(),
        occurred_at: None,
        interval_from: Some("2026-09-12T07:00:00.000Z".into()),
        interval_until: Some("2026-09-12T08:00:00.000Z".into()),
        observed_at: "2026-09-12T08:00:00.000Z".into(),
        field_id: Some("actions".into()), before: Some(json!("--示例程序 A")), after: Some(json!("--示例程序 B")),
        time_derived: true, evidence: vec![EvidenceRef::record(11), EvidenceRef::record(13)], unassigned_reason: None,
    }
}
fn action_unassigned() -> ActionView {
    ActionView {
        context: None,
        source_record: None,
        action_id: "files.usn_changed".into(), title: "文件变更记录".into(), result: "observed".into(),
        phase: None, precision: "observation_only".into(),
        occurred_at: None, interval_from: None, interval_until: None,
        observed_at: "2026-09-12T09:00:00.000Z".into(), field_id: None, before: None, after: None,
        time_derived: true, evidence: vec![EvidenceRef::event("pcscope.usn.c", "usn-epoch-1", "9001")],
        unassigned_reason: Some("来源未提供可解析的完整路径，无法确定目标对象".into()),
    }
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "model/query-samples.json".into());
    let set = samples();
    let document = json!({
        "generated_by": "crates/contracts/examples/query-contract.rs",
        "note": "开发用固定响应集。每条响应都带 mock:true；正式后台不产生该标记。",
        "shape": response_shape(),
        "samples": set.iter().map(|(name, response)| json!({"name": name, "response": response})).collect::<Vec<_>>(),
    });

    // Coverage checks: every state and every display case the plan requires must be present.
    let statuses: Vec<ResponseStatus> = set.iter().map(|(_, r)| r.status).collect();
    let distinct: std::collections::BTreeSet<String> = statuses.iter().map(|s| format!("{s:?}")).collect();
    let mut problems = Vec::new();
    for required in [
        ResponseStatus::Loaded, ResponseStatus::Empty, ResponseStatus::Unsupported, ResponseStatus::PermissionDenied,
        ResponseStatus::Partial, ResponseStatus::EvidencePruned, ResponseStatus::VersionMismatch,
    ] {
        if !statuses.contains(&required) {
            problems.push(format!("缺少状态：{required:?}"));
        }
    }
    let text = serde_json::to_string(&document).unwrap();
    for (name, needle) in [
        ("身份暂定", "\"provisional\""),
        ("动作区间", "\"interval\""),
        ("多证据", "\"kind\":\"record\""),
        ("遮罩", "\"masked\""),
        ("冲突", "\"conflicting\""),
        ("证据已清理字段", "\"evidence_pruned\""),
        ("未归属动作", "\"unassigned_reason\""),
        ("分页水位", "\"watermark\""),
        ("计数分离", "\"source_records\""),
    ] {
        if !text.contains(needle) {
            problems.push(format!("固定响应集缺少可展示项：{name}"));
        }
    }
    if set.iter().any(|(_, r)| !r.mock) {
        problems.push("存在未标记 mock 的响应".into());
    }

    // Decode both generations using the public structs; an absent extension must stay absent,
    // while extension fields (including future optional keys) must survive a round trip exactly.
    for name in ["instance_list", "instance_list_presentation"] {
        let data = set.iter().find(|(n, _)| n == name).unwrap().1.data.clone().unwrap();
        let rows: Vec<InstanceSummary> = serde_json::from_value(data.clone()).expect("decode instance sample");
        if serde_json::to_value(&rows).unwrap() != data { problems.push(format!("对象展示字段往返改变：{name}")); }
        if rows.iter().any(|row| row.presentation.is_some()) != name.ends_with("_presentation") { problems.push(format!("对象展示扩展兼容样本缺失：{name}")); }
    }
    for name in ["action_timeline", "action_timeline_context"] {
        let data = set.iter().find(|(n, _)| n == name).unwrap().1.data.clone().unwrap();
        let rows: Vec<ActionView> = serde_json::from_value(data.clone()).expect("decode action sample");
        if serde_json::to_value(&rows).unwrap() != data { problems.push(format!("动作上下文字段往返改变：{name}")); }
        if rows.iter().any(|row| row.context.is_some()) != name.ends_with("_context") { problems.push(format!("动作展示扩展兼容样本缺失：{name}")); }
    }

    if problems.is_empty() {
        std::fs::write(&path, serde_json::to_string_pretty(&document).unwrap() + "\n").expect("write samples");
        println!("query contract: {} samples written to {} covering {} distinct statuses", set.len(), path, distinct.len());
        for (name, _) in &set { println!("  {name}"); }
        println!("query contract: coverage complete");
        return;
    }
    eprintln!("query contract FAILED:");
    for problem in &problems {
        eprintln!("  ! {problem}");
    }
    std::process::exit(1);
}
