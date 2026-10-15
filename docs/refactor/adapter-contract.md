# 标准适配器契约（R02 冻结）

日期：2026-09-12。状态：**已冻结**（契约版本 1）。代码：`crates/contracts/src/adapter.rs`；示例：`crates/contracts/examples/adapter-contract.rs`。

依据：[并行开发前置条件与准入方案](parallel-readiness.md) R02；[对象监控模型设计](../object-model.md) 第 5–7 节；类型与字段定义的唯一来源是 `model/catalog.json`（模型目录版本 1.0.0）。

R02 是 S05、S07 的**提前实现**：本步冻结接口与样例，**原始事务、断点推进与真实投影仍由 S05/S07 完成**。

## 1. 适配器的边界

适配器接收**已持久化**的来源材料，输出**候选**。它有三条硬约束：

1. **不分配最终对象 ID。** 只提出身份候选；同一个可确认实例由多个来源提出时，归一是身份引擎（S06）的职责。
2. **不写数据库。** 不直接写 `objects`、`object_instances` 或任何表；持久化由投影层完成。
3. **不补采。** 需要的信息若不在已持久化的材料里，就记为缺失，不回头调用 Windows。

这三条不可通过约定豁免：接口签名只给适配器一个只读的 `EvidenceInput` 引用，它拿不到数据库句柄，也没有补采入口。

## 2. 输入

### EvidenceInput — 一份已持久化的来源材料

| 字段 | 说明 |
| --- | --- |
| `source` | 来源标识，如 `pcscope.startup.tasks` |
| `item_id` | 来源内的采集项或分类标识 |
| `structure_version` | **来源结构的版本**，不是模型目录版本 |
| `source_scope` | 采集器投递的作用域。业务作用域由类型决定，不由它决定 |
| `evidence` | 指向已持久化材料的稳定指针，见下 |
| `native_record_id` | 来源自身提供的记录标识，没有则为空 |
| `source_time` / `observed_at` / `ingested_at` | 源时间、观察时间、入库时间，三者分别保留 |
| `decoding` | 解码可用性：是否可读、读取问题、不可读字段、已遮罩字段 |
| `payload` | 材料**原文**。未知字段留在这里，适配器不改写它 |

`EvidenceRef` 是候选引用证据的唯一方式，取值 `record`（`records.seq`）、`snapshot`（`source\|scope\|run_id`）或 `event`（`source\|epoch\|event_id`）。候选**不复制**原始材料，因此大字段不会随每个字段重复一份。

### AdapterContext — 适配器可依赖的上下文

目录版本、规则版本、机器与安装作用域、启动标识（**仅当来源给出时**）、实际采集范围、完整性标志。

它**不提供**最终对象身份。`boot_id` 缺失时不允许推断——模型第 4 节要求「缺少启动标识时明确降级，不伪造历史上下文」。

## 3. 候选

| 候选 | 关键约束 |
| --- | --- |
| `IdentityCandidate` | `type_id` 必须是目录中已登记的类型；`scope` 必须写明；每个身份字段带 `from` 说明来源；类型要求而输入没有的键列入 `missing`，**不得用编造的值顶替** |
| `ObservationCandidate` | 至少引用一份证据；每个 `FieldCandidate` 的 `value` 只在状态允许时存在 |
| `ActionCandidate` | 目标要么是身份候选，要么是 `Unassigned { reason }`，不能悄悄挂到猜出来的对象上 |

字段状态取值：`known` / `not_provided` / `unreadable` / `not_applicable` / `conflicting` / `masked` / `evidence_pruned`。推导方式：`source_provided` / `decoded` / `snapshot_comparison` / `rule`。

时间精度：`exact` / `interval` / `observation_only`。结果语义：`succeeded` / `failed` / `started` / `ended` / `blocked` / `observed` / `unknown`——**`started` 永远不等于 `succeeded`**。

## 4. 结果

`AdapterResult` 的 `outcome` 四选一，**没有「空结果即失败」这种含糊状态**：

| outcome | 含义 |
| --- | --- |
| `adapted` | 产生了候选 |
| `not_an_object` | 材料合法但不描述对象（采样值、来源状态），必须给出原因 |
| `failed` | 无法解释，必须给出原因与是否可重试 |
| `skipped` | 有意不处理，必须给出原因 |

`complete()` 仅在 `adapted` 且没有高于 `note` 的问题时为真。

## 5. 校验规则

`validate(&AdapterResult, &AdapterContext)` 在投影之前执行。违反即**适配器有错，不是目录有错**——violations 非空时候选不得进入投影。

| 规则 | 理由 |
| --- | --- |
| `type_id` 必须已登记 | 候选不得包含未登记类型 |
| `field_id` 必须由该类型声明 | 候选不得包含未登记字段 |
| `action_id` 必须已登记，且 `target_type` 与候选身份的类型一致 | 动作不得挂到错误的类型上 |
| `known` / `masked` / `conflicting` 必须带值；其余状态**必须不带值** | 缺字段不填猜测值 |
| `conflicting` 至少保留两个候选值 | 冲突要留证据，不是覆盖 |
| 每条观察与动作至少引用一份证据 | 无证据支持的结论不予受理 |
| `interval` 必须有上下界且**不得**声明精确时间 | 快照差异不能伪装成精确时刻 |
| `observation_only` 不得声明发生时间 | 不制造虚构精度 |
| `before`/`after` 必须同时指明字段 | 变更要能定位到字段 |
| `failed` / `not_an_object` / `skipped` 必须给出非空原因 | 失败必须有明确结果 |
| `adapted` 且无候选且无问题 | 不接受含糊的空结果 |
| `context.catalog_version` 必须等于内嵌目录版本 | 消费者不得与目录漂移 |

## 6. 序列化与版本

- 契约版本 `1`，由 `contract_schema()` 给出可校验的枚举集合，供非 Rust 消费者（界面、映射报告）比对。
- 目录版本与规则版本随结果一并报告：`catalog_version` 取 `model/catalog.json` 的 `version`，`rule_version` 取其 SHA-256。
- 序列化是契约的一部分：同一结果必须能往返且**逐字节可复现**（已由测试覆盖）。

## 7. 与既有投影的关系

现有 `records` 分类走的是 `collection::item` / `search_index`。适配器契约**不替换**它，而是并行存在：S05 接入证据入口后，同一份来源材料同时供既有检索使用，并为本契约产出候选。切换在 F1 内收口。

## 8. 不在本步范围

| 事项 | 归属 |
| --- | --- |
| 身份归一、生命周期、代际判定 | S06 |
| 字段事实与动作的实际写入、当前值选择、规则版本重建 | S07 |
| 覆盖状态汇总与预算清理 | S08 |
| 逐来源的具体适配器实现 | S09–S20 |
| 原始事务、断点推进、幂等重放 | S05 |
