# S07 验收记录：实现事实、动作和当前投影

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S07 · 待验收 |
| 实现范围 | 投影引擎：消费 S05 的队列，逐条经适配器与校验器、S06 的身份引擎，写出字段事实、字段历史、动作与证据引用。判定规则全部来自 R04 的既有函数，不在本模块重述。**规则版本重建未实现**，见第 3 节与第 5 节 |
| 代码定位 | 新增 `crates/storage/src/projection.rs`；修改 `crates/storage/src/lib.rs`（模块声明、`Store::project_pending`、v8 迁移块）、`crates/storage/src/model.rs`（`SCHEMA_VERSION` 7 → 8、`object_actions` 增 `phase` 列）、`crates/storage/src/evidence.rs`（新增 `input_at`，`inputs` 与之共用读回逻辑）、`model/ownership.json` |
| 验证环境 | Windows 11；cargo / rustc 1.96.0（`x86_64-pc-windows-msvc`，取自 PATH）；`:memory:` 数据库 |
| 验证过程 | `cargo test --workspace`；8 项新增测试（第 3 节）；17 个 `scripts/check-*.cjs`；6 个契约示例退出码 |
| 结果 | 114 项 Rust 测试通过（较 S06 的 104 增加 10 项：本步 9 项 + 共享改动 0005 的 1 项）；17/17 检查通过；6 个示例退出码 0；7 条验收条件中 6 条通过、1 条未实现 |
| 遗留问题 | 见第 5 节；原阻塞级第 1 项已由共享改动 0005 解决 |
| 下一步 | S08 接入覆盖与预算 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 投影引擎 | `crates/storage/src/projection.rs` 的 `project_pending` 与 `apply` |
| 动作生成规则 | 字段变化走 `semantics::change_action`；动作归并走 `semantics::merge_action`；两者都不在本模块重写 |
| 字段选择策略 | 直接调用 `semantics::select_current`；「这条观察算不算一个值」用 `semantics::states_a_value`，生效时间用 `semantics::effective_time` |

## 2. 设计要点

- **规则不重述。** 当前值、变化动作、动作归并、缺失判定四件事全部调用 R04 的函数。R09 的替身在共享改动 0003 之后也调用同一批函数，所以替身与真实引擎不可能各自读出两套语义。
- **旧观察不回退新值。** 存储行的生效时间参与选择，到达顺序不参与；更晚到达的旧观察只进历史。
- **两侧都是值、且生效时间不同，才算变化。** 缺失不是值，同一时刻的两次观察是一致或冲突——这两条正是共享改动 0003/0004 的规则，在真实引擎里第一次被执行。
- **同一证据应用一次。** 每次应用把用掉的证据写进 `evidence_refs`，重复处理时命中即跳过，而不是靠调用方保证不重跑。

## 3. 验收条件对照

| 计划验收 | 结果 |
| --- | --- |
| 旧日志晚到不回退新状态 | 通过：`a_late_older_observation_does_not_roll_the_current_value_back`——先投影较新的观察，再投影较早的，当前值仍是较新的那个 |
| 快照变化显示时间区间 | 通过：`a_changed_field_produces_one_interval_action`——动作精度为 `interval`，区间是两个观察时刻，不声称精确发生时间 |
| 未变化观察不重复生成变化动作 | 通过：`an_unchanged_field_produces_no_change_action`——两次相同值，动作与历史均为 0 |
| 开始和结束不误去重 | 通过：`a_start_and_an_end_are_two_actions`——同一批里的 100 与 102 落成两条动作，未被合并成一条成功 |
| 同时间不同结果不被静默覆盖 | 通过：`values_that_disagree_at_one_time_are_kept_as_a_conflict`——字段状态为 `conflicting`，两个候选都留在 `conflict_sources` 里 |
| 规则重建不制造真实系统动作 | **未实现**。`rule_version` 已逐行记录在事实、历史与动作上，具备按版本筛选的基础，但没有重建入口。重建的难点在语义而不在代码：重建产生的动作必须与来源记录类动作区分开，否则一次规则升级看上去就像系统真的发生了变化。列为第 5 节第 2 项 |
| 同一证据处理两次结果一致 | 通过：`re_processing_the_same_evidence_writes_nothing_new`——把队列重置为待处理后重跑，事实、动作、历史三者数量逐一不变 |

其余 3 项测试为：`a_snapshot_projects_into_field_facts`（投影确实产出字段事实与证据引用）、`a_source_without_an_adapter_is_recorded_rather_than_dropped`（无适配器来源写入队列失败原因，材料本身仍在库中可读）、`values_that_disagree_at_one_time_are_kept_as_a_conflict`（见上表）。

## 4. 过程中修正的问题

**`object_actions` 没有 `phase` 列（S04 结构缺口）。** 写归并比较时才发现：R04 的归并规则要比较阶段（「阶段不同；开始与结束必须分别保留，不得去重成一条成功事件」），目录里也声明了阶段，但 S04 建的动作表没有存它的列。

已按迁移流程补上：`SCHEMA_VERSION` 升到 8，新库由 `create_schema` 直接建出该列，旧库由 v8 块补列（先查 `PRAGMA table_info` 再决定是否 `ALTER`，因为当前 `create_schema` 已含该列，无条件 `ALTER` 会在新库上报重复列）。动作写入也一并补上该列。

## 5. 遗留问题

1. ~~**【阻塞级】业务作用域在真实链路上没有定义来源。**~~ **已解决（共享改动 0005）。** 采集作用域是「身份段 + 采集切分段」的拼接，而投影最初把整串当作身份作用域，于是同一任务被两个来源采集时会因为 `context:` 不同裂成两个实例。已在契约中补 `semantics::business_scope`：去掉 `schema:` / `context:` / `shard:` 三类切分段，保留身份段。新增测试 `two_sources_that_cut_the_same_task_up_differently_land_on_one_instance` 在真实投影链路上验证：两个来源、一个实例、两条开放绑定、字段引用两份证据。

   仍未决定的是**是否要按类型进一步收窄**（例如只保留 `install:` 而不保留 `machine:`）。当前作用域比类型描述略宽；宽不会把两个对象错误合并，窄才会。列入 S21。

2. **规则版本重建未实现**（第 3 节）。需要先定「重建产生的动作如何与来源记录的动作区分」——建议在目录里为动作增加来源性质标记，而不是靠 `rule_version` 反推。
3. **无适配器来源的队列项会一直是 `failed`。** 目前 128 个来源里参考适配器只覆盖 4 个，其余会逐条记一条失败原因。这是 S09–S20 的工作面，不是错误，但健康页需要能按来源汇总，避免刷屏（属 S08/S25）。
4. **缺席判定仍未接入采集循环**（S06 遗留顺延）。`conclude_absence` 已实现，但没有人按「完整快照结束」的时机调用它。
5. **本机未做真实采集实测**，全部验证在 `:memory:` 上用样例消息完成。

## 6. 下一步

- S08 接入覆盖状态与预算，并把无适配器来源的失败按来源汇总。
- 规则版本重建（第 5 节第 2 项）在 S08 之后处理，或并入 S27 的迁移与重建。
- 作用域是否按类型收窄并入 S21 的对账清单。
