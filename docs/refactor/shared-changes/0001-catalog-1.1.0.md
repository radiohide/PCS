# 0001 模型目录新增两个任务执行动作，版本 1.0.0 → 1.1.0

| 字段 | 内容 |
| --- | --- |
| 共享入口 | 中立模型目录 |
| 整合责任人 | 基础数据线 |
| 提出工作线 | 类型适配线（R06 契约用例编写中发现） |
| 变更类型 | **兼容增量** |
| 影响的工作线 | 全部：目录版本号变化会重建派生表、重新生成固定响应集与类型边界表 |

## 问题

R06 的用例 1 要求「一个计划任务，两个配置来源和**一份运行日志**」，而计划第 6 节的动作用途表明确要求「区分开始、结束和结果，不把开始事件当作成功」。

但 `model/catalog.json` 的 1.0.0 版本里，`startup` 类别只有三个动作：`service_installed`、`task_created`、`task_deleted`、`task_updated`——**没有任何任务执行类动作**。运行日志（TaskScheduler 通道的事件 100 与 102）无处落脚：要么被丢弃，要么被塞进语义不符的动作里。

这是编写样例时发现的，而不是实现时发现的：用例的输入根本表达不出计划要求的场景。

## 旧结构 / 新结构

新增两个动作 ID，不改动任何既有 ID 的含义：

| 动作 ID | 名称 | 性质 | 结果 | 目标类型 |
| --- | --- | --- | --- | --- |
| `startup.task_started` | 任务开始执行 | `source_recorded` | `started` | `startup.scheduled_task` |
| `startup.task_ended` | 任务执行结束 | `source_recorded` | `ended` | `startup.scheduled_task` |

同时把两者加入 `startup.scheduled_task` 的 `actions` 列表。

**结果语义分开登记**（`started` 与 `ended`），而不是合成一条结果类动作：计划要求开始不得冒充成功，两个阶段必须是两条动作。

来源报告的结果码（事件 102 的 `result_code`）属于结束记录的数据，尚未拆出独立的结果类动作——留待 S12 实现真实适配器时决定。

## 兼容性

- 既有动作 ID 的含义与目标类型均未改动。
- 引用既有动作的消费者不受影响。
- **新增 ID 的消费者**：任何按目录校验动作 ID 的实现会自动接受新 ID（校验是「必须已登记」，不是白名单枚举）。当前消费者只有仓库内的 `validate`，不存在旧版消费者。
- 目录版本号在 `model/vocabulary.cjs` 与 `crates/contracts/src/model.rs` 两处，已同时更新为 `1.1.0`；`scripts/check-model-catalog.cjs` 强制两者一致。

## 样例变化

| 生成物 | 变化 |
| --- | --- |
| `model/catalog.json` | 动作数 71 → 73；摘要变化 |
| `model/query-samples.json` | `catalog_version` 与 `rule_version` 随之更新 |
| `docs/refactor/parallel-boundaries.md` | 输入摘要变化 |
| `settings.model_catalog_v1`（既有数据库） | 指纹变化，下次打开时重建目录查找表（既有行为，已测试） |

`model/contract-observed.json` 由 `contract-cases.exe` 重新生成。

## 数据迁移

**不需要数据迁移。** 新增的是动作定义，不改变既有 `records`、`snapshots`、`objects` 或任何用户设置。既有数据库下次打开时按指纹重建 `type_definitions`、`action_definitions`、`type_actions`、`source_map` 四张查找表——这是 S04 已实现并测试过的行为。

## 检查结果

```text
node scripts/check-model-catalog.cjs     → 目录版本 1.1.0；101 类型 / 73 动作 / 129 映射行；消费者版本一致
cargo test --workspace --offline --locked → 86 项通过
.\target\debug\examples\contract-cases.exe → 7 个用例
node scripts/check-contract-cases.cjs    → 41 条断言全部成立
15 个检查脚本                             → 全部通过
```

发现过程与修正记录见 [R06 验收记录](../acceptance/R06-contract-cases.md) 第 3 节。
