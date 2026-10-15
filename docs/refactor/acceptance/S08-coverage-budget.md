# S08 验收记录：实现覆盖状态与预算清理

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S08 · 待验收 |
| 实现范围 | 来源覆盖：每次采集尝试写入 `coverage_scopes`（是否启用、最近结果、最近成功、最近尝试、历史完整度）；开关动作在**改变的那一刻**写入覆盖与缺口；`coverage_report` 把采集失败与解析失败分开汇总。预算：证据索引与引用随原始材料一起标记清理；投影队列积压达上限时拒绝入库 |
| 代码定位 | 修改 `crates/storage/src/coverage.rs`（`record_attempt`、`set_configured`、`recovered` 不再回收停用区间）、`crates/storage/src/evidence.rs`（`mark_pruned_for_unit`）、`crates/storage/src/item_controls.rs`（开关变更同步覆盖）、`crates/storage/src/lib.rs`（入库记录覆盖、队列上限、清理接线、`coverage_report`）、`crates/storage/src/projection.rs`（启用状态取自采集项开关） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0（`x86_64-pc-windows-msvc`，取自 PATH）；`:memory:` 数据库 |
| 验证过程 | `cargo test --workspace`；5 项新增测试（第 3 节）；17 个 `scripts/check-*.cjs`；6 个契约示例退出码；构建零警告 |
| 结果 | 119 项 Rust 测试通过（较 S07/0005 的 114 增加 5 项，均为本步新增）；17/17 检查通过；6 个示例退出码 0；7 条验收条件逐条见第 3 节 |
| 遗留问题 | 见第 5 节 |
| 下一步 | F1 阶段（S04–S08）出口条件「原始入库、对象投影、证据及清理闭环成立」达成；下一步 S09–S20 逐类型适配 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 覆盖查询 | `Store::coverage_report`——每个来源与范围一行，含开关、最近结果、最近成功、历史完整度，以及投影的待办/失败/已完成计数 |
| 容量维护 | `maintain_capacity` 新增 `evidence::mark_pruned_for_unit` 接线；入库新增投影积压上限 |
| 清理一致性检查 | `pruning_marks_the_evidence_and_leaves_no_unexplained_reference`（第 3 节） |

## 2. 设计要点

- **采集失败与解析失败分开。** 采集失败是材料从未到手（权限、来源不可用、来源声明的缺口），记在 `coverage_scopes.last_result`；解析失败是材料到手了但没变成对象（没有适配器、候选被契约拒绝），记在投影队列项的 `last_error`。报告把两者并列，因为一个来源完全可能采集正常而解析全废。
- **开关的动作记在开关那一刻。** 停用采集项后**不会有任何采集发生**（`select_envelope` 直接丢弃停用项的材料），所以不可能在采集路径上发现「这个区间是空的」。缺口只能由改变开关这个动作写入，这也是健康页能区分「关掉了」和「没消息」的唯一依据——两者在数据里长得一样，含义相反。
- **恢复不等于补齐。** 停用区间产生的缺口**不被成功采集回收**（`recovered` 显式排除该原因）。采集恢复正常是一回事，那个区间有没有被观察到是另一回事。
- **积压时拒绝，而不是自行清理。** 队列超过上限时入库整批回滚并明确报错。另一种做法——接受下来再丢掉积压——丢掉的正是对象模型赖以建立的原始证据。

## 3. 验收条件对照

| 计划验收 | 结果 |
| --- | --- |
| 单项停用保留旧字段值和真实观察时间 | 通过：`a_disabled_item_keeps_its_facts_and_the_time_they_were_observed`——停用后对完整观察调用缺席判定不产生任何转换，字段值与观察时间逐字不变 |
| 恢复不声称补齐缺失历史 | 通过：`re_enabling_does_not_claim_the_missing_interval_was_covered`——停用写入缺口，重新启用后成功采集仍为 `gap`，缺口记录保持 active |
| 相同来源异常不逐对象重复写入 | 通过：`one_failing_source_writes_one_row_not_one_per_object`——同一来源两次缺口在 `coverage_state` 与 `coverage_scopes` 各只有一行 |
| 清理后无无法解释的悬空引用 | 通过：`pruning_marks_the_evidence_and_leaves_no_unexplained_reference`——按保留单元标记后，证据行与引用同时置 `pruned_at`，动作本身保留自己的值与观察时间 |
| 无限模式不自动删除 | 通过（既有）：`maintain_capacity` 在 `retention_mode=unlimited` 时直接返回，仅放开页面上限；现有保留测试覆盖 |
| 必要当前状态超额时明确报错 | 通过（既有）：保留单元耗尽仍超预算时报错「required current state and dedup metadata exceed quota; increase quota or select unlimited」，不静默删除必要状态 |
| 投影队列积压不无限占盘 | 通过：`a_deep_projection_backlog_stops_ingest_with_a_clear_error`——超限入库被拒且整批回滚（来源可重发），投影后即可继续 |

容量预算按数据库整体计量（`live_bytes` / `physical_bytes` 读页数与文件大小），因此新增的 `evidence`、`projection_queue` 与 `object_*` 各表**自动计入**既有配额，不需要单独登记。

## 4. 过程中修正的问题

**覆盖逻辑一开始挂在了不会发生的路径上。** 最初的实现是在入库时按 `source_configured(...)` 写入启用状态。测试立刻暴露出问题：停用采集项后再入库，覆盖行纹丝不动——因为 `select_envelope` 会把停用项的整条消息直接丢弃，入库根本不会发生。也就是说那段代码在真实链路上永远算不出 `false`。

改成由开关变更驱动：`set_item_enabled` 在写设置之后，找出该控制项覆盖的来源，同步更新覆盖行并在停用时写入缺口。这才是唯一能观察到这件事的时机。

顺带修正 `recovered`：它原本会回收该来源的全部 active 缺口，包括停用区间。那等于用一次成功采集宣称「关掉的那段时间也算看过」。现在显式排除该原因。

## 5. 遗留问题

1. **停用项的材料被整条丢弃，不留痕迹。** `select_envelope` 对停用项直接不选中，材料不入库、不进队列，也不出现在证据里。这是本步之前就有的行为，S08 只是绕开了它；但「关掉期间来源到底有没有产出」这个问题在库中无迹可寻，只有开关动作这一条记录。是否要在停用期间保留一份**计数**（不存材料）供健康页显示，需要产品判断，列入 S21。
2. **`configured` 列只在开关变更时更新。** 若某个来源从未有过覆盖行（例如从未采集成功过），它的停用不会留下覆盖行，健康页也就看不到它。需要一个「来源从未成功过」的显式状态，属 S22/S25 的展示范围。
3. **规则版本重建仍未实现**（S07 遗留顺延）。
4. **本机未做真实采集实测**，全部验证在 `:memory:` 上完成。

## 6. 下一步

- F1 阶段出口条件达成：原始入库（S05）、对象投影（S06/S07）、证据与清理闭环（S08）均已有实现与验证。
- 下一步 S09–S20 逐类型适配，每个类型都要接上真实来源的候选与动作；S21 收口全来源对账，届时处理第 5 节第 1、2 项。
