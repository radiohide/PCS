# 共享样例与契约检查（R06）

日期：2026-09-12。目录：`fixtures/contract/`；运行器：`crates/contracts/examples/contract-cases.rs`；检查：`scripts/check-contract-cases.cjs`。

依据：[并行开发前置条件与准入方案](parallel-readiness.md) R06。

## 1. 一条命令

```powershell
cargo build --offline --locked -p pcscope-contracts --examples
.\target\debug\examples\contract-cases.exe      # 跑用例，写 model/contract-observed.json
node scripts/check-contract-cases.cjs           # 比对断言
```

运行器只输出**实测结果**，从不写断言。断言在 `fixtures/contract/*/case.json` 中手写，每条带 `basis` 说明它依据哪些证据。

## 2. 为什么这样分

R06 明确要求「预期结果必须写明证据依据，**不能直接由待测实现生成后当成正确答案**」。分工照此设计：

| 角色 | 做什么 | 不做什么 |
| --- | --- | --- |
| 运行器（Rust） | 把输入跑过适配器 → 身份 → 事实选择 → 缺失去资格，输出实测 | 不生成、不修改断言 |
| 检查脚本（JS） | 拿实测比对断言；校验用例文件本身 | 不写用例文件 |

**机械上能强制的**：检查脚本没有写用例文件的代码路径；每条断言必须有非空 `basis`；断言路径不得引用实测输出；`case.json` 中不得出现 `observed`/`actual`/`generated_by` 之类疑似由实现生成的键。

**机械上不能证明的**：断言确实由人写。这是评审事项，本文件如实说明，不把检查说成比它实际更强。

## 3. 用例集

七个用例对应计划的七项最小样例，每个目录含 `input.jsonl`（协议消息）与 `case.json`（说明、运行参数、断言）。

| 用例 | 覆盖 |
| --- | --- |
| `01-task-two-sources-and-run-log` | 一个计划任务，两个配置来源和一份运行日志 |
| `02-same-name-different-path-and-recreated` | 同名不同路径；来源明确删除后重建 |
| `03-pid-reuse-and-incomplete-event` | 同 PID 不同创建时间；只有 PID 之外信息缺失的事件 |
| `04-late-log-and-conflicting-value` | 新快照先入库、旧记录后补采；同字段无法消解的分歧 |
| `05-disabled-unreadable-pruned` | 采集项停用、字段不可读、证据已清理 |
| `06-sensitive-value-and-mask` | 敏感值保留；遮罩被当作值提交时拒绝 |
| `07-unattributable-log-and-gap` | 无法归属的日志；纯采集缺口记录 |

### 3.1 运行参数

`case.json` 可声明：

| 键 | 含义 |
| --- | --- |
| `machine_id` / `installation_id` / `collected_scope` | 身份作用域 |
| `configured` | 采集项是否启用（停用则缺失去资格） |
| `complete` | 快照是否完整 |
| `states_object_ended` | 来源是否声明对象结束 |
| `field_status_overrides` | 声明某字段不可读或证据已清理 |

`field_status_overrides` 是**测试工具的脚手架**，放在运行器而不是参考适配器里——适配器要模拟真实适配器，不该带测试专用开关。

### 3.2 断言操作

| 操作 | 含义 |
| --- | --- |
| `equals` | 路径处的值部分匹配给定值（对象按键、数组按元素） |
| `contains` | 路径处是数组且至少一个元素部分匹配 |
| `min` | 路径处是数字且不小于给定值 |
| `none` | 路径处是数组且没有任何元素匹配 |
| `absent` | 路径处为空 |

每条断言都有 `what`（断言什么）与 `basis`（依据什么）。

## 4. 运行器模拟了哪些规则

运行器把 R02–R04 的契约串成一条链路，并**只**实现判定所需的部分：

- 按来源选择参考适配器（计划任务、任务详细配置、进程、事件日志）；
- 身份解析（R03）与来源绑定；
- **重生规则**：来源明确删除后同一身份键再次出现 → 新一代实例，而不是续接旧实例。这是 R03 已冻结的规则，运行器把它接上；跨轮次的缺失与完整生命周期集成仍属 S06；
- 字段当前值选择（R04）；
- 缺失去资格（R04）；
- 缺口作为采集状态单独报告，不混入对象。

## 5. 目录版本

**本步把目录从 1.0.0 升到 1.1.0**，因为用例 1 暴露了一个真实缺口：计划要求「任务运行开始/结束保留阶段」，但目录里没有任何任务执行类动作，只有创建/删除/更新。已新增两个动作：

| 动作 | 名称 | 性质 | 结果 | 目标 |
| --- | --- | --- | --- | --- |
| `startup.task_started` | 任务开始执行 | `source_recorded` | `started` | `startup.scheduled_task` |
| `startup.task_ended` | 任务执行结束 | `source_recorded` | `ended` | `startup.scheduled_task` |

这是**兼容的增量变更**：新增 ID，不改动既有 ID 的含义。按方案第 5 节的版本纪律，版本号随之更新，且所有消费方一起重建——目录版本在 `model/catalog.json`、`crates/contracts/src/model.rs` 与检查脚本之间逐一对齐，任一处不一致即失败。

R02–R05 的验收记录中出现的「catalog 1.0.0」是它们各自提交时的状态，不改写；当前状态以本文件与 `model/catalog.json` 为准。

## 6. 验收条件的落点

| 计划验收 | 落点 |
| --- | --- |
| 所有样例通过 schema 校验 | 检查脚本校验用例文件结构与断言字段 |
| 错误类型 ID、错误枚举、缺证据引用、错误时间区间等故意破坏能够被拒绝 | `crates/contracts/src/adapter.rs` 的校验测试，含枚举反序列化拒绝 |
| 日志样例不包含真实秘密 | 检查脚本对每个输入扫描凭据与个人数据特征 |
| Rust 和前端侧对同一数据含义一致 | 两侧读同一份 `model/contract-observed.json`；枚举取值来自同一份目录 |
