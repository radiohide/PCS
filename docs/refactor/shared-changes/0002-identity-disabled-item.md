# 0002 身份解析不再因采集项停用而拒绝，停用门移到缺失去资格

| 字段 | 内容 |
| --- | --- |
| 共享入口 | 公共契约（`crates/contracts/src/identity.rs`） |
| 整合责任人 | 基础数据线 |
| 提出工作线 | 基础数据线（R09 贯通演练中由集成行为发现） |
| 变更类型 | **破坏性变更**（`resolve` 在 `hints.configured = false` 时的返回类型改变） |
| 影响的工作线 | 类型适配线、查询与界面线、迁移验证线：任何以停用状态调用 `resolve` 的代码 |

## 问题

R09 的贯通演练跑用例 05（采集项已停用）时产出了 **0 个对象实例**，而该用例的证据材料是齐备的。

原因：`resolve` 在 `request.hints.configured == false` 时直接返回 `Unresolved`。但**候选之所以存在，正是因为该采集项在那一轮采集到了东西**。停用发生在采集之后，材料仍在。

后果比「少显示一行」严重：模型要求「停用采集后保留最后状态并标注已暂停监控，不产生对象删除动作」。若解析阶段就拒绝，最后观察到的状态会被丢弃——这正是该要求要防的事。

门设错了层：停用影响的是**能否把缺失判定为未发现**，不是**能否识别一个已经拿到的对象**。

## 旧结构 / 新结构

| | 旧 | 新 |
| --- | --- | --- |
| `resolve` 对 `configured=false` | 返回 `IdentityResolution::Unresolved` | 正常解析；`configured` 不参与 |
| 停用的门 | 在 `resolve` 内 | 只在 `semantics::absence_eligible`（本就在那里） |

`LifecycleHints.configured` 字段保留：`absence_eligible` 与生命周期转换仍需要它。

## 兼容性

破坏性：若某处依赖「停用即 Unresolved」，其行为会改变。

本仓库内**没有任何调用方依赖该行为**——`resolve` 目前只在测试与示例中调用，S06 才接线。因此停用与升级顺序是：先更新本契约与其测试，再让消费者接入。

反面证据也在仓库里：R04 的 `absence_eligible` 已经独立实现了停用判定，并有测试覆盖（`absence_needs_complete_configured_and_successful_coverage`）。两处重复且行为相反，是这次修正的直接依据。

## 样例变化

| 位置 | 变化 |
| --- | --- |
| `crates/contracts/src/identity.rs` 的测试 | `a_disabled_item_resolves_to_unresolved_rather_than_absent` 改为 `a_disabled_item_still_resolves_so_retained_facts_survive`，断言相反结果 |
| `docs/refactor/identity-contract.md` | 第 5 节补注：停用不在身份解析层拒绝 |
| `model/drill-output.json` | 用例 05 由 0 个实例变为 1 个 |
| `docs/refactor/acceptance/R03-identity-contract.md` | 不改写既有结论；本文件与其后续的 R09 记录共同说明该处已修正 |

## 数据迁移

不需要。`resolve` 不接触存储；S06 尚未接线，没有已产生的事实需要重算。

## 检查结果

```text
cargo test --offline --locked -p pcscope-contracts   → 42 项通过
cargo build --example drill                          → 用例 05 由 0 个实例变为 1 个
node scripts/check-drill.cjs                         → 通过
17 个检查脚本                                        → 全部通过
```
