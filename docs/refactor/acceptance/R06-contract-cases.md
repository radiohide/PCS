# R06 验收记录：建立共享样例与契约检查

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | R06 · 待验收 |
| 实现范围 | 七个共享契约用例（输入 + 手写断言）、契约用例运行器、契约检查脚本、参考适配器提取为库模块；目录从 1.0.0 升到 1.1.0（新增两个任务执行动作） |
| 代码定位 | 新增 `fixtures/contract/`（7 用例 × 2 文件）、`crates/contracts/examples/contract-cases.rs`、`crates/contracts/src/reference.rs`、`scripts/check-contract-cases.cjs`、`model/contract-observed.json`、`docs/refactor/contract-cases.md`；修改 `model/vocabulary.cjs`、`model/types.cjs`、`crates/contracts/src/model.rs`、`crates/contracts/src/adapter.rs`、`crates/contracts/examples/adapter-contract.rs`、`crates/contracts/src/lib.rs`，并重新生成 `model/catalog.json`、`model/query-samples.json`、`docs/refactor/parallel-boundaries.md` |
| 验证环境 | Cargo / rustc 1.98.1（`x86_64-pc-windows-gnu`）；Node v24.20.0 |
| 验证过程 | `cargo test --workspace --offline --locked`；`target/debug/examples/contract-cases.exe`；`node scripts/check-contract-cases.cjs`；15 个检查脚本 |
| 结果 | 86 项 Rust 测试通过（较 R05 +1）；7 个用例 41 条断言全部成立；15/15 检查通过；五个契约示例均退出码 0 |
| 遗留问题 | 见第 6 节 |
| 下一步 | R07：划分代码与目录所有权 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 样例说明 | 本文件与 `docs/refactor/contract-cases.md` |
| 共享加载工具 | `crates/contracts/src/reference.rs` 的 `read_fixture` / `evidence_from` / `adapter_for` |
| 契约用例 | `fixtures/contract/`（7 个用例） |
| 独立的契约检查 | `scripts/check-contract-cases.cjs`（第 15 个检查） |
| 集成测试命令 | `contract-cases.exe` + `check-contract-cases.cjs`（见契约文档第 1 节） |

## 2. 七个用例

```text
01-task-two-sources-and-run-log               4 input, 1 instance,  2 actions
02-same-name-different-path-and-recreated     5 input, 3 instances, 1 action
03-pid-reuse-and-incomplete-event             3 input, 3 instances, 1 action
04-late-log-and-conflicting-value             3 input, 1 instance,  0 actions
05-disabled-unreadable-pruned                 1 input, 1 instance,  0 actions
06-sensitive-value-and-mask                   2 input, 2 instances, 0 actions
07-unattributable-log-and-gap                 1 input, 0 instances, 0 actions
```

共 **41 条断言**，每条带 `what` 与 `basis`。

## 3. 过程中修正的缺陷

**1. 目录缺少任务执行类动作（真实缺口）。** 用例 1 要求「一份运行日志」与任务归一，而目录里只有任务创建/删除/更新，没有任何执行类动作——计划的动作表明确要求「区分开始、结束和结果」。已新增 `startup.task_started` / `startup.task_ended`，目录升到 1.1.0。

**2. 参考适配器的结果语义与目录脱钩（真实 bug）。** 适配器原先用硬编码分支决定每个事件的结果语义，新增任务执行动作后没跟上，于是 `startup.task_started` 产出 `observed` 而不是目录声明的 `started`。**手写的断言把它抓了出来**——这正是「预期不能由被测实现生成」的价值所在。

修正方式不是补一个分支，而是**改为从目录读取动作的结果语义**（`declared_outcome`）：目录是唯一决定者，漂移从结构上不可能再发生。

**3. 两处我自己的断言写错。** 用例 4 断言「两份观察」而实测是三份——第三份材料声明来源未提供该字段，它也是一次合法的观察。已改为断言真正要验的东西：当前值只由最新那份决定（`sources` 只有一个），而三份观察都保留。

## 4. 验收条件对照

| 计划验收 | 结果 |
| --- | --- |
| 所有样例通过 schema 校验 | 通过：用例目录结构、必填字段、断言字段逐项校验 |
| 错误类型 ID 被拒绝 | `an_unregistered_type_is_rejected` |
| 错误枚举被拒绝 | 新增 `an_invalid_enum_value_is_rejected_rather_than_defaulted`，覆盖字段状态、字段性质、推导方式、时间精度、动作结果、证据种类、动作目标 |
| 缺证据引用被拒绝 | `an_evidence_free_or_unexplained_result_is_rejected` |
| 错误时间区间被拒绝 | `an_interval_action_must_not_claim_an_exact_time` |
| 日志样例不包含真实秘密 | 检查脚本逐个输入扫描凭据与个人数据特征 |
| Rust 和前端侧对同一数据含义一致 | 两侧读同一份 `model/contract-observed.json`，枚举取值同源于目录 |

## 5. 关于「预期不能由被测实现生成」

这是 R06 的核心要求，落点如下：

- 运行器**没有**写用例文件的代码路径，只写实测结果；
- 每条断言必须有非空 `basis`，否则检查失败；
- 断言路径不得引用实测输出；
- `case.json` 中不得出现 `observed`/`actual`/`generated_by` 之类键。

**须说明的限制**：以上能强制的是流程与结构，**不能机械证明断言出自人手**。第 3 节第 2 项是这条要求真正生效的实例——硬编码的结果语义漂移是被手写断言抓到的，而不是被某个检查规则抓到的。这一点在契约文档中如实写明，不把检查说成比它实际更强。

## 6. 遗留问题

1. **参考适配器只覆盖四个来源。** 计划任务（含详细配置）、进程、事件日志；其余 124 个来源没有参考适配器。这不影响 R06 的验收（七项样例未要求），但 S09–S20 每类都要产出真实适配器并补样例。
2. **运行器未做跨轮次缺失的生命周期折叠。** 只实现了「来源明确删除后重建」这一条（R03 已冻结的规则）；跨轮次的未发现/确认删除完整流程属 S06。
3. **`field_status_overrides` 是测试脚手架。** 它让「字段不可读」「证据已清理」可以在样例中表达，但真实来源如何产生这两种状态要等 S05/S09–S20。
4. **断言未覆盖查询响应侧。** 本步的用例走的是适配 → 身份 → 语义链路；查询契约（R05）的固定响应集是独立生成的，两者尚未串成一条端到端链路——那是 R09 的最小贯通演练。
5. **目录升版未同步更新 R02–R05 记录中的版本引用。** 那些记录里的「catalog 1.0.0」是各自提交时的状态，按记录不改写；当前状态以 `model/catalog.json` 为准。
6. **`model/query-samples.json` 与 `parallel-boundaries.md` 随目录版本重新生成**，其内容随版本变化，不属缺陷但需在每次目录变更后重跑。

## 7. 下一步

R07 的前置（R06）已满足。R07 需要划分代码与目录所有权：文件责任表、类型边界表、共享改动登记格式。类型边界表已由 `docs/refactor/parallel-boundaries.md` 生成，需补的是文件责任边界与共享改动流程。
