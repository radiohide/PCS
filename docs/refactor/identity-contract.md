# 身份解析契约（R03 冻结）

日期：2026-09-12。状态：**已冻结**（契约版本 1）。代码：`crates/contracts/src/identity.rs`；样例：`crates/contracts/examples/identity-contract.rs`。

依据：[并行开发前置条件与准入方案](parallel-readiness.md) R03；[对象监控模型设计](../object-model.md) 第 4 节。上游输入是 [标准适配器契约](adapter-contract.md) 的身份候选。

R03 是 S06 的**提前实现**：本步冻结判定接口与规范化规则，**持久身份解析与生命周期集成仍由 S06 完成**。

## 1. 本模块是什么

`resolve()` 是一个**纯函数**：判定所需的一切都在 `IdentityRequest` 与 `InstanceLookup` 里，没有数据库、没有时钟、没有全局状态。因此同一输入可在任何时间重放出同一结论，这正是「规则版本可回放」的前提。

实例 ID 的分配与持久化**不在本模块**。`InstanceLookup` 只提供两个查询：

| 查询 | 用途 |
| --- | --- |
| `by_key(type, machine, installation, scope, native_key)` | 按键查找已存实例，返回 `(实例 ID, 代际)` 列表 |
| `by_source_binding(source, item_id, native_record_id)` | 查找该来源记录已绑定的实例 |

## 2. 输入

`IdentityRequest`：

| 字段 | 说明 |
| --- | --- |
| `candidate` | 上游适配器提出的身份候选 |
| `strategy` | 类型的身份策略（策略 ID、身份键、作用域、规范化、复用规则、身份不足策略），取自目录 |
| `binding` | 该候选来自哪条来源记录。**永不进入身份键** |
| `machine_id` / `installation_id` | 身份作用域 |
| `evidence_time` | 源时间与观察时间 |
| `hints` | 生命周期线索：完整观察、实际采集范围、是否配置、历史保留起点、来源是否声明对象结束、启动标识 |
| `rule_version` | 判定所依据的规则版本 |

## 3. 业务范围与采集范围的边界

**采集分片、请求 ID、插件名不参与跨来源身份。** 这不是约定：

- `InstanceIdentity` 没有能存放它们的字段——键只由 `type_id`、机器与安装作用域、业务 `scope`、规范化后的 `native_key`、`incarnation` 组成；
- 目录校验器在声明层就拒绝含这些词的**身份键**（见 S03 记录）；
- `binding` 是独立字段，只用于查绑定，不参与建键。

**用户 SID、注册表视图是否参与由类型决定**，由目录中该类型的 `identity.keys` 声明，本模块不自行增删。

## 4. 规范化规则

规范化只作用于建键，**不改写展示值**。大小写规则按 Windows 各自定义，不是全局小写：

| 策略 | 大小写 | 说明 |
| --- | --- | --- |
| `registry_registration` | 折叠 | 注册表路径、值名不区分大小写 |
| `named_entity` | 折叠 | 服务名、打印机名等命名空间不区分大小写 |
| `value_key` | 折叠 | 值名不区分大小写 |
| `account_sid` | 折叠 | SID 按定义不区分大小写 |
| `native_path` | 折叠 | 任务路径、文件路径不区分大小写；尾部反斜杠不属于身份 |
| `native_identifier` | 保留 | 来源怎么给就怎么比 |
| `file_identity` | 保留（文件 ID 转小写十六进制） | 卷身份 + 文件 ID |
| `process_instance` | 保留 | 进程 ID 与创建时间 |
| `endpoint_tuple` | 保留 | 协议与地址端口元组 |
| `composite_native` | 保留 | 各组成部分分别规范化后按稳定顺序拼接 |

**缺键不填值。** 未提供的身份键在键串中占位为 `?键名`，同时列入 `missing`，身份被报为暂定——绝不默认一个值。

## 5. 四类结果

| 结果 | 含义 |
| --- | --- |
| `Confirmed` | 身份完整。`basis` 说明是命中既有实例（按键或按来源绑定）还是需要新建。`instance_id` 为空表示需要分配新实例 |
| `Provisional` | 身份键不完整。带 `alias`、身份不足原因、以及**可补充哪些键**；若该来源记录已绑定实例，同时带出 `instance_id` |
| `Ambiguous` | 多个既有实例匹配且无其他依据区分。**禁止任选一个**，候选列表原样返回 |
| `Unresolved` | 连候选都无法提出（例如该采集项已停用），附原因 |

**暂定不会因为重复观察而变成已确认。** 来源绑定匹配告诉我们是*哪一个*实例，不代表缺失的键已经到达。只有缺失的键真正出现，身份才升级为已确认。

## 6. 实例 ID 生命周期与引用迁移

`InstanceIdentity::alias()` 由键派生，是确定性的：

```text
<type_id>@<machine>|<installation>|<scope>|<native_key>#<incarnation>
```

引用**不依赖数字 ID**。因此在暂定转确认时：

- 引用方始终可用 alias 解析到同一实例；
- 数字 ID 只在持久层内部使用，变更不会让旧详情链接静默失效；
- 暂定阶段若尚未分配 ID，`Provisional.instance_id` 为空而 `alias` 已可用。

`incarnation` 是同一原生键删除后重建的代际计数。**同键重生是新一代，绝不续接旧实例的生命周期。**

## 7. 生命周期转换

`lifecycle()` 是另一个纯函数。输入是先前状态、本次是否出现、生命周期线索。

| 先前 | 本次 | 结果 |
| --- | --- | --- |
| 无 | 出现 | `FirstObserved`——**首次观察，不是创建**，依据里写明来源未提供创建时间 |
| 活动 / 未发现 | 出现 | `StillObserved`；来源声明对象结束则 `EndedStated`；代际大于 1 则 `Reincarnated` |
| 已结束 / 已删除 | 出现 | `Reincarnated`（代际 +1） |
| 活动 | 完整观察中缺失 | `NotObserved`——**未发现不是删除** |
| 未发现 | 仍缺失 | `Unchanged`——继续缺失仍不是删除 |
| 任意 | 来源声明删除 | `RemovedStated`——**只有来源明确声明才进入已删除** |
| 任意 | 观察不完整 | `Unchanged`——不完整的观察证明不了任何事 |
| 任意 | 采集项停用 | `Unchanged`——停用不是消失 |
| 无 | 缺失 | `Unchanged`——从未观察过，无所谓转换 |

## 8. 不在本步范围

| 事项 | 归属 |
| --- | --- |
| 实例的持久化分配、来源绑定写入、代际推进 | S06 |
| 字段事实与动作的实际投影 | S07 |
| 覆盖状态与预算 | S08 |
| 按类型的具体补充规则（如 WMI 订阅的复合键细节） | S12 等 |
