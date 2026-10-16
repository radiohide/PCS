# 0013：持久化展示结果

日期：2026-10-16。整合版本：0.2.11；数据库结构 10；采集模型仍为 1.2.6，基础查询契约仍为 1。

对象摘要、历史行动上下文及主时间线可见性由后台准备，查询复用已存结果。新增 `display_objects`、`display_actions`、`display_work`、`display_identity_work`、`display_build`、`object_collection_scopes`、`display_totals` 和 `display_evidence_totals`。原始记录、快照和对象事实不重写。

原有 `source_bindings.scope` 继续表示对象身份范围。新增关联单独保存原始 `source_scope`，覆盖结果必须同时匹配来源和采集范围；不得以二者直接相等或来源相等替代。旧材料恢复要求精确来源、原始条目、原生身份和该对象实际引用的证据。

查询字段保持兼容；`projection_status` 可增加 `display` 行及可选 `display_state`。界面将其作为后台整理进度，失败进入现有全局异常入口。没有新增产品页签。

时间线游标内部增加已准备结果的代次边界。既有游标仍不透明；同一翻页过程不会被后来补算的旧记录插入。规则更新会拒绝旧规则的游标，要求重新读取。统计仍区分对象、行动、来源登记和去重证据；复杂筛选不使用全局统计冒充筛选统计。

所有结构、触发器及版本标记在同一事务提交。旧后台拒绝结构 10；正式替换须保持同一 Windows 账户，完成后不直接降回旧二进制。读者不执行迁移、准备展示结果或增加任务。

实施与验收见 [持久化展示结果](../acceptance/persisted-display-model.md)。
