# R04 验收记录：冻结事实、动作、时间和清理语义

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | R04 · 待验收 |
| 实现范围 | 时间编码与排序锚点、六种缺失状态与遮罩边界、当前值选择与冲突、字段变化到动作的映射、动作归并规则、缺失去资格判定、证据清理后的引用行为。均为**判定规则**，选择/清理的实现留给 S07/S08 |
| 代码定位 | 新增 `crates/contracts/src/semantics.rs`、`crates/contracts/examples/semantics-contract.rs`；修改 `crates/contracts/src/lib.rs`；新增 `docs/refactor/semantics-decisions.md` |
| 验证环境 | Cargo / rustc 1.98.1（`x86_64-pc-windows-gnu`） |
| 验证过程 | `cargo test --workspace --offline --locked`；`target/debug/examples/semantics-contract.exe` |
| 结果 | 80 项 Rust 测试通过（较 R03 +11）；示例 5 个验收场景全过，退出码 0；三个契约示例均退出码 0；13 个既有检查通过 |
| 遗留问题 | 见第 5 节 |
| 下一步 | R05：冻结查询契约与模拟服务 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 语义决策表 | `docs/refactor/semantics-decisions.md` |
| 判定规则实现 | `crates/contracts/src/semantics.rs` |
| 预期输出样例 | `crates/contracts/examples/semantics-contract.rs`（5 个场景）+ 11 项单元测试 |

## 2. 五个验收场景的实测结果

```text
PASS  latest API snapshot plus an earlier log   当前值取较晚的读取；较早记录不回退；变化记为区间
PASS  same field, disagreeing sources           保留为冲突，两个候选都在，不静默偏好某来源
PASS  collection item disabled                  缺失去资格；最后观察到的值保留，不清空
PASS  evidence pruned by retention              引用返回已清理并带时间；结论与观察时间仍在
PASS  task next-run time changes                只产生 observed.state_changed；目录中性质为 state_change
```

计划验收条件的对应关系：

| 计划要求 | 覆盖 |
| --- | --- |
| API 最新快照 + 较早日志有唯一处理方式 | 场景 1 + 单元测试 `a_late_arriving_older_observation_never_becomes_current` |
| 同字段冲突有唯一处理方式 | 场景 2 + 单元测试 `disagreeing_sources_at_the_same_time_are_kept_as_a_conflict` |
| 停用采集有唯一处理方式 | 场景 3 + 单元测试 `absence_needs_complete_configured_and_successful_coverage` |
| 证据被清理有唯一处理方式 | 场景 4 + 单元测试 `a_pruned_reference_states_that_it_was_pruned` |
| 任务下次运行时间变化有唯一处理方式 | 场景 5 + 单元测试 `a_field_change_can_only_produce_an_observed_action` |
| 各工作线不自行定义枚举或缺失值含义 | 枚举来自 `model/catalog.json` 与 `adapter.rs`；语义只在 `semantics.rs` 定义一处 |

## 3. 单元测试覆盖的规则

11 项：

- 规范时间宽度固定，文本序等于时间序；带偏移与裸日期被拒绝；
- 采集器实际产出的两种时间格式（7 位与 3 位小数秒）都能规范化，且截断不四舍五入；
- 三种排序各读自己的锚点，回退到观察时间时 `derived` 被置位；
- 迟到的旧观察不会成为当前值；
- 同一时间上的分歧保留为冲突，一致时引用全部来源；
- 遮罩哨兵作为值被拒绝；
- 缺缺失状态按信息量取最具体者，且不变成值；
- 字段变化只能产生 `observed.*` 动作，且该动作在目录中**不是** `source_recorded`；
- 动作归并需目标、类型、阶段、时间四项同时一致；相差一秒不归并；未归属不参与；
- 缺缺失资格的四项条件分别报告；
- 已清理引用返回带时间的 `Pruned`。

其中一项是**对目录的校验**，不是对代码的校验：断言 `observed.configuration_changed` 与 `observed.state_changed` 在 `model/catalog.json` 中的 `nature` 不是 `source_recorded`。若有人把这两个动作改成来源记录类，「下次运行时间变化不会被当成人工修改」这条保证会立刻失效并被测试拦下。

## 4. 过程中确定的两处细节

1. **区间动作的排序锚点取上界而非下界。** 上界是变化确定已生效的最早时刻；用下界会声称变化在被观察到之前就已发生。
2. **缺少发生时间时不伪造时间。** 退回观察时间并置 `derived`，要求界面可见。这一条与身份契约「首次观察不是创建」是同一条原则在时间维度上的体现。

## 5. 遗留问题

1. **来源优先级尚未声明**（决策表第 7 节第 1 项）。当前所有同生效时间的分歧都是冲突。若实际使用中冲突过多，必须先在目录中显式声明优先级——**不得由实现方隐式决定**。这是 S07 落地前需要确认的首要未决项。
2. **`not_applicable` 的产生方式未定**（第 7 节第 2 项），需 S09–S20 观察真实实例形态后确定。
3. **历史版本的清理粒度未定**（第 7 节第 3 项），属 S08。
4. **这些规则尚未接入任何调用方。** `select_current` / `merge_action` / `absence_eligible` 目前只在测试与样例中调用；S07/S08 才是真实调用点。
5. **未验证真实数据下的冲突率。** 本步只固定「分歧即冲突」的规则，没有在真实规模数据上统计会产生多少冲突。R06 的共享样例应包含冲突样本，S21 的全量对账应统计冲突数量。
6. **时间规范化尚未应用于既有存储。** 现有 `records.occurred_at` 中的历史值仍是原始格式（多为 7 位小数秒），未做统一。是否需要一次性回填属于数据迁移（S27）的范畴；在此之前，跨格式的文本比较需要在查询侧先规范化。
7. **遮罩边界只覆盖了「遮罩被当作值提交」一种误用。** 「遮罩值参与身份匹配」的防护依赖身份层不接收遮罩（`InstanceIdentity` 的键来自类型声明的字段，遮罩字段不会被声明为身份键），未做独立测试。

## 6. 下一步

R05 的前置（R04）已满足。R05 需要先固定类型目录、实例列表、实例详情、实例动作、证据详情、来源覆盖、投影进度的请求/响应结构，并给出与真实响应同结构的模拟服务，不要求真实后端全部实现。
