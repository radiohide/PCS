# 0003 契约公开「是否陈述了值」与「生效时间」，供投影复用

| 字段 | 内容 |
| --- | --- |
| 共享入口 | 公共契约（`crates/contracts/src/semantics.rs`） |
| 整合责任人 | 基础数据线 |
| 提出工作线 | 基础数据线（R09 收尾复验中由演练产物发现） |
| 变更类型 | **兼容性新增**（新增两个公开函数；`select_current` 行为不变） |
| 影响的工作线 | 类型适配线、查询与界面线、迁移验证线：任何需要判断「这条观察算不算一个值」或「两条观察谁更新」的代码 |

## 问题

R09 的替身投影要为「字段变了」生成 `observed.configuration_changed`。它当时自己写了一套比较：把同一字段的观察按时间排序，取首尾比较 `value`。

两处据此出错，产出**没有任何来源观察到的变化**：

1. **把缺失当成值的一侧。** 用例 01 的两个来源各自描述同一任务的不同字段，对对方携带的字段保持沉默。来源沉默被读成一个值（`null`），于是「有值 → 没值」被判成变化。用例 01 因此多出 4 条幻影动作。
2. **两个来源的生效时间相同也判成变化。** §3 已经规定生效时间相同只有两种结果——值相同则取该值并引用全部来源，值不同则判为冲突——**都不是随时间发生的变化**。投影却给出 `09:00:00Z → 09:00:00Z` 这样的零长度区间，等于宣称一次没有发生的变化。

用例 04 的对比说明了差别：那里是同一来源在 10:00 与 10:05 的两次真实观察，是**应该**产生区间动作的情形——修正后它的 `before`/`after` 也才第一次成为真实的前后值（此前 `after` 是缺失）。

根因不是投影写错了一个条件，而是**契约没有把这两个判定公开出去**：`absence_rank` 是私有函数，`生效时间 = source_time 否则 observed_at` 只是 `select_current` 内部的一个闭包。于是第二个消费者只能自己再写一套——而 R04 第 9 行明确要求「任何工作线不得自行定义枚举取值或缺失值的含义」。S07 的真实投影会遇到同一个问题。

## 旧结构 / 新结构

| | 旧 | 新 |
| --- | --- | --- |
| 「这条观察陈述了一个值吗」 | `absence_rank`（私有），仅供 `select_current` 使用 | 新增公开 `states_a_value(&FactObservation) -> bool`，`select_current` 改为调用它 |
| 「这条观察的生效时间」 | `select_current` 内的局部闭包 `effective` | 新增公开 `effective_time(&FactObservation) -> String`，`select_current` 改为调用它 |

两个函数都是既有判定的**提取**，不引入新语义：`states_a_value` 返回 `absence_rank(status).is_none()`（即 `Known` / `Masked` 为真，其余缺失状态为假），`effective_time` 返回 `canonical_time(source_time 否则 observed_at)`，失败时退回 `observed_at`。

## 兼容性

纯新增：两个函数此前不存在，没有调用方需要改动；`select_current` 的输入输出与选择结果逐字不变（全部 42 项 `pcscope-contracts` 测试未改动即通过）。目录版本不变，无需变更登记号。

## 样例变化

| 位置 | 变化 |
| --- | --- |
| `model/drill-output.json` | 用例 01 的动作由 6 条减为 2 条（运行日志的开始与结束，与其契约用例 `actions.length == 2` 一致）；用例 04 的变化动作补齐真实的 `before`/`after` |
| `scripts/check-drill.cjs` | 新增：每用例计数对照 R09 记录断言；变化动作不得有零长度区间，`before`/`after` 不得缺失 |
| `docs/refactor/drill.md` 第 5 节 | 补登两个新检查 |

## 数据迁移

不需要。两个函数是纯判定，不接触存储；S07 尚未接线，没有已生成的动作需要重算。

## 检查结果

```text
cargo test --workspace                               → 86 项通过（未改动任何既有测试）
target/debug/examples/drill.exe                      → 用例 01 为 1 实例 2 动作；用例 04 的变化动作带真实前后值
node scripts/check-drill.cjs                         → 通过
17 个检查脚本                                        → 全部通过
注入测试                                             → 把旧数字写回产物、以及造一条零长度区间动作，检查均失败并点名
```
