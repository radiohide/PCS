# 0008 事件的分类由事件本身给出，参考适配器按机器收窄作用域

| 字段 | 内容 |
| --- | --- |
| 共享入口 | 公共契约（`crates/contracts/examples/adapter-coverage.rs`）、契约检查（`scripts/check-adapter-coverage.cjs`）、中立模型目录（`model/adapter-coverage.json`） |
| 整合责任人 | 基础数据线 |
| 提出工作线 | 基础数据线（S31 用户流程回归走通全链路时发现） |
| 变更类型 | **兼容性新增**（覆盖率产物多一个字段；事件源判定多一份名单；分类读取多一条来源） |
| 影响的工作线 | 类型适配线（事件源判定规则）、查询与界面线（来源类型映射沿用同一规则）、迁移验证线（覆盖率产物重新生成） |

## 问题

S31 的闭环回归（`crates/storage/src/flow.rs`）按「插件配置 → 采集 → 对象 → 动作 → 证据」走一遍，走到投影就断了：**两条配置来源与一份运行日志描述同一个计划任务，得到两个实例，其中一个没有名字。**

查下来是两处各自成立、合起来不成立的地方：

1. **声明的分类到不了适配器。** 契约把分类放在信封的 `Event.kind` 上，`record_event` 把它写进证据的 `item_id`；而投影交给适配器的素材是记录里的 `data`。`adapters/event.rs` 的「事件自陈其动作」路径读的是 `payload["kind"]`——**没有任何生产者往 `data` 里放这个字段**（`collect-eventlog.ps1` 把分类放在事件上，`data` 由 `fields`/`system`/`field_schema` 组成）。于是该路径在真实素材上从未生效，每条已分类事件都落到事件 id 表回退；回退表不认识 `TaskName`，只能给出 `?path` 的临时身份，运行日志便在任务旁边另立一个实例。适配器自己的单元测试用的是把 `kind` 放进 `data` 的形状——**一个没有生产者会发出的形状**，所以这条路径的失效一直没被任何测试碰到。

2. **`pcscope.core` 不是事件源。** 应用自身的活动（采集项被开关、后台启动停止）以 `event_batch` 到达，但不在 `pcscope.eventlog.` 或 `pcscope.usn.` 前缀下。判定为「非事件源」后，它排队投影、找不到适配器，被报成解析失败——健康页上出现一屏「解析失败」，而内容其实是应用在描述自己。

## 旧结构 / 新结构

| | 旧 | 新 |
| --- | --- | --- |
| 分类的读取 | 只读 `payload["kind"]`（无生产者写入） | 先读 `payload["kind"]`（夹具等自陈动作的素材不变），取不到目录里登记的动作时读证据的 `item_id`——契约把它定义为「该来源内的分类身份」，`record_event` 写入的正是 `Event.kind` |
| 事件源判定 | `eventlog.` / `usn.` 两个前缀减排除项 | 同上，再加 `EVENT_SOURCES = ["pcscope.core"]` |
| 规则的导出 | `event_prefix` / `usn_prefix` / `event_exclusions` | 同上 + `event_sources`，两个 JS 检查与 Rust 用同一份参数 |
| 参考适配器的身份作用域 | `context.collected_scope` 原样 | `installation_scope(...)` 收窄，机器级对象不再随「采集时用的是哪个账户」分裂 |

**`installation_scope` 正是 0005 记录里留的那个未决问题。** 0005 写明：契约只做「去掉采集切分段」，**是否按类型进一步收窄（例如只保留 `install:` 段）是尚未决定的问题**，并指明在决定前作用域宁可偏宽。此处按机器级类型收窄为「去掉 `user:` 段」，与 S09–S20 的类别适配器一致——参考适配器此前没有跟上，于是它与类别适配器对同一个对象给出两个作用域。

## 兼容性

- **分类读取是纯扩宽。** 自陈动作的素材（契约夹具）仍由 `payload["kind"]` 决定，行为逐字节不变；只有此前落到回退路径的素材改由事件自身的分类决定。既有回退路径保留，未分类事件的行为不变（`an_unclassified_event_still_reaches_its_action_through_the_record_id` 仍通过）。
- **`event_sources` 是纯新增字段。** 两个 JS 检查都以 `(coverage.event_sources || [])` 读取，产物里没有该字段时退化为旧规则。
- **`pcscope.core` 的判定变化只在 Rust 侧生效。** 它不在 `model/source-map.cjs` 的来源清单里，因此覆盖率对账本身不受影响——变化是「自身活动不再被报成解析失败」。
- **作用域收窄是行为变化，不是纯新增。** 机器级对象在旧的参考适配器下会带上 `user:` 段；同一对象在收窄前后是两个不同的 `native_key`。见「数据迁移」。

## 样例变化

| 位置 | 变化 |
| --- | --- |
| `model/adapter-coverage.json` | 重新生成，多出 `event_sources: ["pcscope.core"]` |
| `apps/desktop/ui/source-type-map.json` | **未变**：`pcscope.core` 不在来源清单内，映射结果相同 |
| `crates/contracts/src/adapters/event.rs` 测试 | 新增 `collected()` 帮手（采集端真实形状）与 `the_classification_is_read_from_the_event_when_the_payload_does_not_state_one` |

## 数据迁移

**尚未需要，但有一个必须知道的边界。** 参考适配器的作用域收窄只影响此后写入的实例：已经落在开发库里的机器级对象仍带着旧的、含 `user:` 段的作用域，不会自动重算。目前参考适配器只服务契约夹具与三份参考来源（`pcscope.startup.tasks`、`pcscope.startup.task-details`、`pcscope.execution.processes`），开发库里的相关行数是可数的；**S32 整体切换时若发现有存量差异，按实例逐条处理，不在这里夹带迁移**。

事件侧的改动不需要迁移：分类本来就存在于 `records.kind` 与证据的 `item_id` 中，改动只是把它读出来。

## 检查结果

见 `docs/refactor/acceptance/S31-flow.md`。
