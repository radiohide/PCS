# 对象模型数据库结构（schema 6）

日期：2026-09-11。阶段：S04 产出。本文说明增量结构本身，不说明投影引擎（S07）或容量预算（S08）。

模型依据：[对象监控模型设计](../object-model.md)。计划依据：[全量对象模型重构计划](../full-object-refactor-plan.md) 第 7、11 节。类型与身份的唯一来源是 `model/catalog.json`。

## 1. 三个版本号必须分开

| 版本 | 存放位置 | 写入方 | 变化时机 |
| --- | --- | --- | --- |
| 数据库结构版本 | `PRAGMA user_version` | **只有** `Store::open` 的迁移块 | 结构变化时 |
| 目录（规则）版本 | `pcscope_contracts::MODEL_CATALOG_VERSION` 与 `model/catalog.json` 的 `version` | 目录编写源 `model/vocabulary.cjs` | 类型、身份、字段或动作定义变化时 |
| 派生表指纹 | `settings.model_catalog_v1` | `model::ensure_catalog` | 目录内容变化时 |

三者分开的理由：结构变化需要迁移数据，规则变化只需要重建派生表，两者混在一个号里会让「改了目录」看起来像「改了结构」。

**修过的一个缺陷**：`search_index::ensure` 原先硬编码 `PRAGMA user_version=5`，而它在迁移之后运行，会把新版本号覆盖回 5。结构版本现在只有一个写入方。派生表的重建不再触碰版本号。

## 2. 增量结构

12 张新表，全部为新增，不修改 `records`、`snapshots`、`objects` 或设置内容。

### 2.1 对象实例与来源绑定

| 表 | 键 | 说明 |
| --- | --- | --- |
| `object_instances` | `instance_id`；唯一约束 `(type_id,machine_id,installation_id,scope,native_key,incarnation)` | 对象实例。`identity_status` ∈ 已确定/暂定/歧义；`lifecycle` 区分活动、本次未发现、确认删除、确认结束、不确定；`created_at` 只在来源明确提供时填写，否则为空 |
| `source_bindings` | `binding_id`；唯一约束 `(instance_id,source,item_id,scope,native_record_id,valid_from)` | 实例与来源记录的有效范围。带 `valid_to` 时间范围，使删除重建可以区分代际；`instance_id` 级联删除 |

`incarnation` 是同一原生标识删除后重建的实例区分。采集分片、请求 ID、插件名**不在**身份键中。

### 2.2 字段事实与历史

| 表 | 键 | 说明 |
| --- | --- | --- |
| `object_facts` | `(instance_id,field_id)` | 当前字段值。`field_kind` ∈ 身份/配置/状态；`value_status` 为七值枚举（已知、来源未提供、无法读取、不适用、冲突、已遮罩、证据已清理）；`derivation` 记录取值方式；`conflict_sources` 保留无法消解的多来源候选 |
| `fact_history` | `(instance_id,field_id,observed_until)` | 变化历史。仅在值变化时追加，不记录每次不变观察；`observed_from`/`observed_until` 表示观察区间 |

### 2.3 动作

| 表 | 键 | 说明 |
| --- | --- | --- |
| `object_actions` | `action_row` | 动作。`precision` ∈ 精确/区间/仅观察；区间动作填 `interval_from`/`interval_until` 而非伪造 `occurred_at`；`unassigned_reason` 与 `instance_id` 互斥（CHECK 约束），因此「未归属」必须写明原因，不能假装已归属 |

### 2.4 证据引用与清理

| 表 | 键 | 说明 |
| --- | --- | --- |
| `evidence_refs` | `(subject_kind,subject_id,evidence_kind,evidence_id)` | 事实、历史、动作、绑定指向证据的多对多引用。只存指针，不复制大字段。`pruned_at` 记录支撑证据被保留策略清理的时间 |

`evidence_kind` ∈ `record` / `snapshot`；`evidence_id` 对 record 是 `records.seq`，对 snapshot 是 `source|scope|run_id`。`subject_id` 是主体复合键的稳定字符串形式。

### 2.5 覆盖与投影状态

| 表 | 键 | 说明 |
| --- | --- | --- |
| `coverage_scopes` | `(source,scope)` | 采集覆盖：监控配置、最近读取结果、最近成功与尝试时间、历史覆盖状态。与既有 `coverage_state`（按来源与原因的缺口记录）并存，后者保留原有行为 |
| `projection_state` | `projection` | 每条投影的规则版本、结构版本、游标、水位、待处理与失败计数、最近错误 |

### 2.6 目录快照

| 表 | 说明 |
| --- | --- |
| `type_definitions` | 类型 ID、类别、名称、身份策略、身份键、是否被来源供给 |
| `action_definitions` | 动作 ID、名称、性质、结果语义、目标类型 |
| `type_actions` | 类型与动作的多对多 |
| `source_map` | 来源到去向的映射，用于核对覆盖 |

这些表由 `model/catalog.json` 生成，**不是**手工维护的第二套目录。`supplied_by_source=0` 的类型就是「尚未接入来源」的标记，避免空列表被读成已采集完成。

## 3. 引用清理行为

保留策略删除证据时，顺序是**先标记、后删除**：

1. `model::mark_pruned_records` 标记指向本批次 `records.seq` 的引用；
2. `model::mark_pruned_snapshots` 标记指向本批次快照的引用；
3. 才删除 `records`、`snapshots`，再删除保留单位。

结果：结论仍然可解释——字段事实保留自己的值与观察时间，只有指针被标记为已清理，不产生无法解释的悬空引用。

对象实例被删除时，`source_bindings`、`object_facts`、`fact_history`、`object_actions` 通过外键级联删除；`model::delete_orphan_refs` 清除随之失效的引用。

**注意**：`value_status=evidence_pruned` 与 `evidence_refs.pruned_at` 不是同一件事。前者表示字段值本身随证据消失而不可用；后者表示值仍在、只是支撑材料被清理。当前设计属于后者，因此 `value_status` 保持 `known`。

## 4. 兼容性声明

**不默认向后兼容。** 升级到 schema 6 之后：

| 情形 | 行为 |
| --- | --- |
| 本版本打开 schema 6 | 正常 |
| 本版本打开 schema 7 及以上 | 写入方与只读方都**明确拒绝**，报「database schema version N is newer than this application」 |
| 只懂 schema ≤5 的旧程序打开 schema 6 | 旧程序按其原逻辑遇到高于自身支持的版本号会拒绝。新表是纯新增，旧程序不会读到它们，但**不要假定旧程序可继续使用该库** |
| 降级回旧程序 | **不支持**。新表为新增，但版本号与投影状态不会被旧程序维护。需要回到旧版本时应使用升级前的备份，不能就地降级 |

因此升级前应做一致性备份；备份与回退演练属于 S32。

## 5. 不在本步范围

- **投影引擎**：本步只建结构，不生成实例、事实与动作。写入逻辑属于 S06/S07。
- **容量预算**：新表尚未纳入 `maintain_capacity` 的预算计算，滚动模式下它们目前不被计入配额。这属于 S08，且在此之前不能声称「新结构参与预算」。
- **字段级适配**：`object_facts` 的 `field_id` 取值来自目录，但逐类型映射属于 S09–S20。
