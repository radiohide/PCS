# 0005 契约公开「采集作用域 → 业务作用域」的换算

| 字段 | 内容 |
| --- | --- |
| 共享入口 | 公共契约（`crates/contracts/src/semantics.rs`） |
| 整合责任人 | 基础数据线 |
| 提出工作线 | 基础数据线（S07 实现真实投影时发现，S06 记录已把它列为阻塞项） |
| 变更类型 | **兼容性新增**（新增一个公开函数；既有调用方行为不变） |
| 影响的工作线 | 类型适配线（适配器从 `AdapterContext.collected_scope` 取身份作用域）、查询与界面线（实例作用域字段的含义）、迁移验证线 |

## 问题

R02 在 `EvidenceInput.source_scope` 上写明：**「采集作用域由采集端给出；业务作用域由类型决定，不是由它决定。」** 但真实链路里没有人做这个换算——S07 的投影最初把采集作用域原样填进 `AdapterContext.collected_scope`，适配器再把它当作身份作用域。

采集作用域的形状是**身份段 + 采集切分段**：

```text
machine:m/install:i/schema:1/context:/shard:0          ← 基础任务清单
machine:m/install:i/schema:1/context:task-detail       ← 任务详细配置
```

两段材料描述的是同一个计划任务，只有 `context:` 不同。把切分段留在身份作用域里，**同一个任务会在第二次来源看向它时裂成两个实例**。

R09 的演练当时之所以归一，是因为用例文件直接给定了 `collected_scope`（`install:example-install`）——真实链路没有这个输入。S06 记录第 5 节第 1 项、S07 记录第 5 节第 1 项都把这列为阻塞级。

## 旧结构 / 新结构

| | 旧 | 新 |
| --- | --- | --- |
| 身份作用域的来源 | 无定义；投影原样使用采集作用域 | `semantics::business_scope(采集作用域)`：去掉 `schema:` / `context:` / `shard:` 三类采集切分段，保留身份段 |
| 换算位置 | 无 | 契约内，投影与任何未来消费者共用 |

保留原样的情形：作用域里没有身份段（例如 `fixed drives with drive letters only`）或为空时原样返回。**理由是不发明**：一个凭空造出来的作用域会让从未被证明相同的对象静默合并，比留一个不透明的原值更糟。

## 兼容性

纯新增：函数此前不存在，没有调用方需要改动。已有事实与动作的作用域由 S07 首次写入，尚未有需要重算的存量数据。

**注意它与目录的关系**：类型目录里每个身份策略都有一句作用域说明（`安装作用域`、`用户作用域`、`启动作用域`、`卷身份`……）。本函数**不**按类型收窄，只做「去掉采集切分段」这一步。是否要按类型进一步收窄（例如只保留 `install:` 段而不保留 `machine:` 段）是一个尚未决定的问题，列入 S21 全来源对账。在没有决定之前，作用域比类型描述略宽——宽不会把两个对象错误合并，窄才会。

## 样例变化

| 位置 | 变化 |
| --- | --- |
| `crates/contracts/src/semantics.rs` 测试 | 新增 `a_business_scope_drops_how_the_collection_was_cut_up` |
| `crates/storage/src/projection.rs` 测试 | 新增 `two_sources_that_cut_the_same_task_up_differently_land_on_one_instance`——两个来源、不同 `context:`、一个实例、两条开放绑定、字段引用两份证据 |
| `model/` 与 `fixtures/` | 无变化 |

## 数据迁移

不需要。`object_instances` 与 `source_bindings` 目前只在开发数据库中由 S07 写入，没有需要重算的生产数据；S27 迁移历史时按新规则生成作用域即可。

## 检查结果

```text
cargo test --workspace                               → 114 项通过（较 S07 的 112 增加 2 项）
17 个检查脚本                                        → 全部通过
6 个契约示例                                         → 全部退出码 0
```
