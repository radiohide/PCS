'use strict';
// S03 model vocabulary: the categories, identity strategies, field/value/time contracts and the
// action registry that every object type references. Authored here as one readable source and
// expanded into model/catalog.json by scripts/build-model-catalog.cjs.
//
// This file is the single definition of these IDs. Nothing else in the repository may invent a
// type, action or field ID; scripts/check-model-catalog.cjs enforces that.

// Twelve main categories. A type belongs to exactly one, so an instance is never double counted.
const categories=[
  ['account','账户与身份','谁能登录、使用什么身份、获得什么权限'],
  ['software','软件与组件','系统具备哪些功能和执行能力'],
  ['runtime','运行实例','当前执行什么、使用什么身份、加载什么代码'],
  ['startup','自动执行入口','什么条件下自动执行什么代码'],
  ['shell','系统扩展与关联','用户或程序操作系统功能时调用什么组件'],
  ['file','文件与访问控制','实际代码与数据是什么，谁能读取、修改或执行'],
  ['policy','策略与配置','系统和应用按照什么规则运行'],
  ['network','网络与远程访问','流量走哪里、哪些连接被允许、资源如何对外开放'],
  ['security','安全与信任','信任谁、保护什么、哪些检查被豁免'],
  ['hardware','硬件与驱动','设备如何工作，哪些组件参与系统底层执行'],
  ['virtualization','虚拟环境','电脑中有哪些隔离环境及其资源配置'],
  ['desktop','桌面与应用集成','用户操作如何被处理，应用如何接入桌面'],
];

// Field kinds. A field is either part of identity, a configuration value, a runtime state value,
// or an evidence pointer. Configuration and state are shown in separate sections but share history.
const field_kinds=[
  ['identity','身份字段','参与实例识别，不参与普通属性变更历史'],
  ['configuration','配置属性','对象的结构与设定；变化即配置变化'],
  ['state','运行状态','某次观察时的运行情况；变化不等于被人为修改'],
  ['evidence','证据引用','指向支撑本对象的证据材料'],
];

// value_status replaces "null means missing": each absence has a distinct meaning.
const value_status=[
  ['known','已知','来源提供了值'],
  ['not_provided','来源未提供','来源结构中没有该字段'],
  ['unreadable','无法读取','字段存在但读取被拒绝或失败'],
  ['not_applicable','不适用','该字段对本实例没有意义'],
  ['conflicting','存在冲突','多个来源给出无法消解的不同值'],
  ['masked','已遮罩','敏感值按显示设置遮罩'],
  ['evidence_pruned','证据已清理','值曾采集到，但支撑证据已按保留策略清理'],
];

// How a value was obtained. Re-parsing never rewrites the original evidence.
const derivations=[
  ['source_provided','来源直接提供','来源原样给出，未做解释'],
  ['decoded','解码解释','由稳定编码、结构或字段含义解释得到'],
  ['snapshot_comparison','快照比较','由两次完整快照的差异得到'],
  ['rule','其他明确规则','由类型规定的其他确定性规则得到'],
];

// Time precision. No fabricated precision: a snapshot difference is an interval, not an instant.
const time_precisions=[
  ['exact','精确发生时间','来源记录了动作发生的确切时间'],
  ['interval','变化区间','只能确定变化发生在两次观察之间'],
  ['observation_only','仅观察时间','只有采集观察时间，无法定位发生时间'],
];

// Named identity strategies. A type references one and adds only what differs, so 99 types do not
// each restate the same rules.
//   keys          machine-checkable identity key names; the checker proves no type is keyed on a
//                 display name alone. `@composite` means the type supplies its own keys.
//   description   what the strategy identifies
//   normalization / reuse / insufficient are the strategy defaults a type may override.
const identity_strategies=[
  ['registry_registration','注册表登记','scope,hive,view,registry_path',
    '用户或机器作用域 + 注册表视图（32/64 位） + 注册表路径',
    '注册表路径保留来源原大小写；比较时仅对 Windows 已定义的注册表路径规则做规范化，不做统一小写。',
    '同一路径重新出现不保证是删除前的同一登记；需要生命周期证据才升级为确认删除或重建。',
    '缺少注册表视图或作用域时标记身份能力受限，不与另一视图的登记合并。'],
  ['native_identifier','来源原生标识','scope,native_id',
    '安装/用户作用域 + 来源系统提供的原生标识符',
    '原生标识按来源格式保留；只有 Windows 明确定义为不区分大小写的标识才按该规则比较。',
    '同一标识在来源中消失后重新出现时，只有在来源提供实例代际证据时才判为同一实例。',
    '来源未提供原生标识时不得用显示名称代替，标记身份能力受限。'],
  ['account_sid','账户 SID','scope,sid',
    '安装作用域 + 账户 SID',
    'SID 按 Windows 规则不区分大小写。',
    '同名不同 SID 不合并；账户改名不产生新实例；SID 复用需要来源的代际证据。',
    '无法取得 SID 时标记暂定身份，不得以名称为键合并。'],
  ['named_entity','命名实体','scope,name',
    '安装/用户作用域 + 来源系统中的稳定名称',
    '名称保留来源原大小写；仅对 Windows 定义为不区分大小写的命名空间按该规则比较。',
    '名称被删除后由不同对象复用时不自动判为同一实例。',
    '名称重复或来源未提供唯一名称时标记歧义，保留多个候选。'],
  ['native_path','原生路径','scope,path',
    '安装作用域 + 类型规定的完整路径',
    '路径分隔符与大小写按类型规则规范化；不对所有字符串统一处理。',
    '路径相同不保证删除重建前后是同一实例；需要生命周期证据。',
    '缺少完整路径时标记身份能力受限。'],
  ['process_instance','进程实例','scope,pid,created_at',
    '启动作用域 + 进程 ID + 进程创建时间',
    '进程 ID 为数值，创建时间按来源时间格式保留。',
    '进程 ID 复用（相同 ID、不同创建时间）必须是两个实例，不得拼接生命周期。',
    '缺少创建时间时标记暂定身份，不得仅凭进程 ID 归一。'],
  ['file_identity','文件身份','volume_identity,file_id',
    '卷身份 + 文件 ID',
    '文件 ID 按十六进制规范化；路径保留原大小写，仅用于展示。',
    '文件 ID 被复用时不自动判为同一文件；需要路径与时间证据。',
    '只有路径时标记身份能力受限：路径可变，改名与移动都会改变它。'],
  ['composite_native','类型自定义组合','@composite',
    '由类型显式列出的多个来源字段组合',
    '各组成部分按各自字段规则规范化。',
    '组合键相同的对象在来源中消失后重现时，需要生命周期证据。',
    '任一组成部分缺失时标记暂定身份或歧义。'],
  ['value_key','配置值键','scope,path,value_name',
    '作用域 + 配置位置 + 值名',
    '值名保留来源原大小写；仅对 Windows 定义的注册表值名规则做规范化。',
    '值被删除后重新设置不保证是同一登记。',
    '作用域缺失时不得合并不同用户或不同视图下的同名值。'],
  ['endpoint_tuple','连接端点元组','protocol,local_address,local_port,remote_address,remote_port,owning_pid,created_at',
    '协议 + 本地地址端口 + 远端地址端口 + 所属进程 + 创建时间',
    '地址按 IP 文本规范化；端口为数值；创建时间按来源时间格式保留。',
    '端点被来源复用时不自动判为同一条连接；时间相同使用稳定的第二排序键，不把不同连接拼接。',
    '缺少创建时间或所属进程时标记暂定身份；远端地址缺失（监听端点）时按本地元组识别并标注范围受限。'],
];
// A display name is never sufficient on its own. The checker fails a type whose identity keys
// reduce to just a display name, and fails any identity key containing a delivery detail.
const name_key_terms=['display_name','friendly_name','caption'];
// Identity keys that describe the identity *context* rather than an identity value. A type keyed
// only on context (no distinguishing value) would merge distinct objects, so the checker requires
// at least one value-bearing key alongside them.
const scope_only_keys=['scope','hive','view'];

// Collection sharding is a delivery detail, never part of business identity. Enforced by the
// checker so a later adapter cannot quietly fold a shard index into an identity key.
const forbidden_identity_terms=['shard','batch_seq','request_id','run_id','plugin','采集分片','分片'];

// Action registry. Two independent axes, never conflated:
//
//   nature  how PCScope came to know the action
//     source_recorded  来源明确记录的动作
//     snapshot_change  快照观察到的配置变化（时间是区间）
//     state_change     运行状态变化（不得推断为人工修改）
//     internal         PCScope 自身活动，不作为真实系统动作展示
//
//   result  what the action's outcome is (result_semantics below)
//
// The plan's fourth kind, 结果记录, is the combination: a source_recorded action carrying an
// outcome semantic such as succeeded or failed. It is not a separate nature, because "the source
// logged a task-completed event" and "the task succeeded" are the same record described twice.
//
// row = id | name | nature | result | target type
const actions=[
  'software.update_history|Windows 更新操作记录|source_recorded|unknown|software.system_update',
  // 账户与身份
  // 登录事件创建的是登录会话，账户的参与由会话的用户字段体现；一个动作只有一个目标对象。
  'identity.logon|登录|source_recorded|succeeded|runtime.logon_session',
  'identity.logon_failed|登录失败|source_recorded|failed|account.user',
  'identity.logoff|注销|source_recorded|succeeded|runtime.logon_session',
  'identity.logoff_requested|请求注销|source_recorded|observed|runtime.logon_session',
  'identity.session_locked|会话锁定|source_recorded|observed|runtime.terminal_session',
  'identity.session_unlocked|会话解锁|source_recorded|observed|runtime.terminal_session',
  'identity.session_reconnected|会话重连|source_recorded|observed|runtime.terminal_session',
  'identity.session_disconnected|会话断开|source_recorded|observed|runtime.terminal_session',
  'identity.account_created|账户建立|source_recorded|succeeded|account.user',
  'identity.account_enabled|账户启用|source_recorded|succeeded|account.user',
  'identity.account_disabled|账户禁用|source_recorded|succeeded|account.user',
  'identity.account_deleted|账户删除|source_recorded|succeeded|account.user',
  'identity.account_changed|账户变更|source_recorded|succeeded|account.user',
  'identity.account_locked|账户锁定|source_recorded|succeeded|account.user',
  'identity.account_unlocked|账户解锁|source_recorded|succeeded|account.user',
  'identity.account_renamed|账户重命名|source_recorded|succeeded|account.user',
  'identity.group_member_added|组成员加入|source_recorded|succeeded|account.group_membership',
  'identity.group_member_removed|组成员移除|source_recorded|succeeded|account.group_membership',
'identity.special_privileges|登录获得特殊权限|source_recorded|observed|runtime.logon_session',
  // 运行实例
  'execution.process_started|进程启动|source_recorded|started|runtime.process',
  'execution.process_stopped|进程退出|source_recorded|ended|runtime.process',
  'execution.remote_thread|远程线程创建|source_recorded|observed|runtime.process',
  'execution.process_access|进程访问|source_recorded|observed|runtime.process',
  'execution.process_tampering|进程映像异常|source_recorded|observed|runtime.process',
  'execution.driver_loaded|驱动加载|source_recorded|observed|hardware.driver_service',
  'execution.module_loaded|模块加载|source_recorded|observed|runtime.module_instance',
  'execution.wmi_filter|WMI 筛选器活动|source_recorded|observed|startup.wmi_filter',
  'execution.wmi_consumer|WMI 消费者活动|source_recorded|observed|startup.wmi_consumer',
  'execution.wmi_binding|WMI 绑定活动|source_recorded|observed|startup.wmi_binding',
  'execution.pipe_created|管道创建|source_recorded|observed|runtime.process',
  'execution.pipe_connected|管道连接|source_recorded|observed|runtime.process',
  'execution.clipboard_changed|剪贴板变化|source_recorded|observed|desktop.shell_configuration',
  // 文件与访问控制
  'files.access_audit|对象访问审计|source_recorded|observed|file.watched_file',
  'files.permissions_audit|对象权限变更审计|source_recorded|observed|file.acl_entry',
  'files.creation_time_changed|文件创建时间修改|source_recorded|observed|file.watched_file',
  'files.created|文件创建或覆盖|source_recorded|observed|file.watched_file',
  'files.stream_created|文件流创建|source_recorded|observed|file.alternate_data_stream',
  'files.deleted_archived|文件删除并归档|source_recorded|observed|file.watched_file',
  'files.deleted|文件删除|source_recorded|observed|file.watched_file',
  'files.executable_blocked|可执行文件创建被阻止|source_recorded|blocked|file.watched_file',
  'files.shredding_blocked|文件粉碎被阻止|source_recorded|blocked|file.watched_file',
  'files.executable_detected|可执行文件创建|source_recorded|observed|file.watched_file',
  'files.usn_changed|文件变更记录|source_recorded|observed|file.watched_file',
  // 注册表与策略
  'registry.value_operation|注册表值操作|source_recorded|observed|policy.os_configuration',
  'registry.object_changed|注册表对象创建或删除|source_recorded|observed|policy.os_configuration',
  'registry.value_set|注册表值设置|source_recorded|observed|policy.os_configuration',
  'registry.renamed|注册表重命名|source_recorded|observed|policy.os_configuration',
  'policy.audit_changed|审计策略变更|source_recorded|succeeded|policy.os_configuration',
  // 自动执行入口
  'startup.service_installed|服务安装|source_recorded|succeeded|startup.service',
  'startup.service_state_changed|服务状态变化|source_recorded|observed|startup.service',
  'startup.task_created|任务创建|source_recorded|succeeded|startup.scheduled_task',
  'startup.task_deleted|任务删除|source_recorded|succeeded|startup.scheduled_task',
  'startup.task_updated|任务更新|source_recorded|succeeded|startup.scheduled_task',
  // 任务执行过程分阶段登记：开始与结束是两条动作，不得去重成一条成功事件。
  // 来源报告的结果码属于结束记录的数据，逐类型适配时再决定是否拆出结果类动作。
  'startup.task_started|任务开始执行|source_recorded|started|startup.scheduled_task',
  'startup.task_ended|任务执行结束|source_recorded|ended|startup.scheduled_task',
  // 网络
  'network.share_access|共享访问|source_recorded|observed|network.smb_share',
  'network.connection_allowed|连接允许|source_recorded|succeeded|network.endpoint',
  'network.connection_blocked|连接阻止|source_recorded|blocked|network.endpoint',
  'network.connection_observed|网络连接|source_recorded|observed|network.endpoint',
  'network.dns_query|DNS 查询|source_recorded|observed|network.dns_cache_entry',
  // 硬件与存储
  'storage.raw_read|原始磁盘读取|source_recorded|observed|hardware.disk',
  // Sysmon 自身
  'sysmon.service_changed|Sysmon 服务状态变化|source_recorded|observed|policy.os_configuration',
  'sysmon.config_changed|Sysmon 配置变化|source_recorded|observed|policy.os_configuration',
  'sysmon.error|Sysmon 内部错误|source_recorded|failed|policy.os_configuration',
  // 快照比较产生的动作：绝不用来描述人工操作
  'observed.configuration_changed|观察到配置变化|snapshot_change|observed|-',
  'observed.state_changed|观察到状态变化|state_change|observed|-',
  'observed.first_seen|首次观察到|snapshot_change|observed|-',
  'observed.reappeared|再次记录到此对象|snapshot_change|observed|-',
  'observed.no_longer_observed|本次未发现|snapshot_change|observed|-',
  // 采集状态：不是被监控对象的动作
  'coverage.gap|采集缺口|internal|failed|-',
  'coverage.recovered|采集恢复|internal|succeeded|-',
  'coverage.usn_gap|USN 断点缺口|internal|failed|-',
  'coverage.security_log_cleared|安全审计日志清除|source_recorded|observed|policy.os_configuration',
  'coverage.unclean_shutdown|非正常退出|internal|observed|-',
  'collector.item_configured|采集项配置变化|internal|succeeded|-',
];

// Result semantics. Kept explicit because "开始" must never be shown as "成功".
const result_semantics=[
  ['succeeded','成功','来源明确报告成功'],
  ['failed','失败','来源明确报告失败'],
  ['started','已开始','来源记录了开始，未记录结果'],
  ['ended','已结束','来源记录了结束，未报告结果'],
  ['blocked','被阻止','来源明确报告被阻止或拒绝'],
  ['observed','已观察','只有观察事实，无结果判定'],
  ['unknown','未知','来源未提供结果'],
];

module.exports={version:'1.2.7',categories,field_kinds,value_status,derivations,time_precisions,identity_strategies,name_key_terms,scope_only_keys,forbidden_identity_terms,actions,result_semantics};
