# S03 验收记录：冻结类型、身份和字段契约

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S03 · 待验收 |
| 实现范围 | 十二个主类别、101 个对象类型、10 个身份策略、71 个动作、字段与值/时间契约、128/128 来源映射；未修改采集行为、运行配置或既有数据库 |
| 代码定位 | 新增 `model/vocabulary.cjs`、`model/types.cjs`、`model/source-map.cjs`、`scripts/check-model-catalog.cjs`；生成 `model/catalog.json` |
| 验证环境 | Node v24.20.0；对 S01 的 `model/sources.json` 核对；未启动后台服务、未读取用户数据库 |
| 验证过程 | `node scripts/check-model-catalog.cjs`；5 次人为注入测试（见第 8 节） |
| 结果 | 检查通过；13 个既有检查脚本全部通过；5 次注入全部被拦下 |
| 遗留问题 | 见第 10 节 |
| 下一步 | S03 通过后进入 S04（实现增量数据库结构） |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 中立模型目录 | `model/catalog.json`（展开结果，197 KB） |
| 目录编写来源 | `model/vocabulary.cjs`（类别、身份策略、字段/值/时间契约、动作登记）、`model/types.cjs`（类型）、`model/source-map.cjs`（来源映射草案） |
| 类型契约 | `model/catalog.json` 的 `types[]`：身份策略、作用域、规范化、复用判断、身份不足策略；关键字段；支持动作 |
| 来源映射草案 | `model/catalog.json` 的 `sources[]`，覆盖 S01 全部来源 |
| 目录校验器 | `scripts/check-model-catalog.cjs` |

ID 的唯一来源是 `model/` 下的三个编写文件；`model/catalog.json` 是展开结果，由校验器每次重新生成，避免出现陈旧副本。校验器同时扫描仓库，任何声明 `MODEL_CATALOG_VERSION` 的消费者版本不一致都会失败。

## 2. 目录规模（全部由脚本计算）

```text
Catalog version 1.0.0
Categories 12; types 101; actions 71; sources 129
Identity strategies 10; types without a source 3
Types per category: account=6 software=8 runtime=4 startup=8 shell=8 file=4 policy=8
                    network=17 security=9 hardware=10 virtualization=10 desktop=9
Sources per destination: observation=96 coverage=4 action_evidence=28 sampling=1
S01 sources covered: 128/128
```

## 3. 身份策略

身份规则抽成 10 个具名策略，类型只引用策略并写出差异，避免 101 个类型各写一遍相似规则。策略携带**可机器校验的身份键**，这是「不能全部以名称为键」能被检查的前提。

| 策略 | 身份键 | 适用类型数 |
| --- | --- | --- |
| `native_identifier` | 作用域 + 来源原生标识 | 30 |
| `composite_native` | 类型显式列出的组合 | 27 |
| `named_entity` | 作用域 + 名称 | 12 |
| `registry_registration` | 作用域 + 配置单元 + 视图 + 注册表路径 | 9 |
| `native_path` | 作用域 + 完整路径 | 8 |
| `value_key` | 作用域 + 配置位置 + 值名 | 6 |
| `file_identity` | 卷身份 + 文件 ID | 4 |
| `account_sid` | 作用域 + SID | 3 |
| `process_instance` | 作用域 + 进程 ID + 创建时间 | 1 |
| `endpoint_tuple` | 协议 + 本地/远端地址端口 + 所属进程 + 创建时间 | 1 |
| | 合计 | 101 |

要点：

- 只有 12 个类型使用 `named_entity`，且身份键是「作用域 + 名称」两个部分，不是名称单独作键。
- 进程用创建时间区分 ID 复用；文件用卷身份与文件 ID，只有路径时标记身份能力受限。
- 每个类型都必须有作用域、复用/重建判断与身份不足策略，缺一即检查失败。

## 4. 字段、值与时间契约

| 契约 | 取值 |
| --- | --- |
| 字段性质 | `identity` / `configuration` / `state` / `evidence` —— 配置与状态分组显示但共用历史 |
| 值状态 | `known` / `not_provided` / `unreadable` / `not_applicable` / `conflicting` / `masked` / `evidence_pruned` |
| 推导方式 | `source_provided` / `decoded` / `snapshot_comparison` / `rule` |
| 时间精度 | `exact`（精确发生）/ `interval`（变化区间）/ `observation_only`（仅观察时间） |
| 结果语义 | `succeeded` / `failed` / `started` / `ended` / `blocked` / `observed` / `unknown` |

`not_provided` 与 `not_applicable`、`unreadable`、`evidence_pruned` 分开，取代「null 就是缺失」。时间精度三档保证快照差异只能记为区间，不制造虚构的精确时间。结果语义把「开始」与「成功」分开，来源只记录开始时不得显示为成功。

## 5. 动作登记

71 个动作。**性质**与**结果**是两个独立轴，不混为一谈——见第 8 节的自我纠错。

性质（PCScope 如何得知该动作）：

| 性质 | 数量 | 展示约束 |
| --- | --- | --- |
| `source_recorded` 来源明确记录 | 62 | 使用来源事件语义，引用证据 |
| `internal` PCScope 自身活动 | 5 | 作为采集活动展示，不当作真实系统动作 |
| `snapshot_change` 快照观察到的变化 | 3 | 写「观察到变化」，时间为观察区间 |
| `state_change` 运行状态变化 | 1 | 写具体状态变化，**不推断为人工修改** |

结果语义（动作的结果）：

| 结果 | 数量 |
| --- | --- |
| `observed` 已观察 | 42 |
| `succeeded` 成功 | 20 |
| `failed` 失败 | 4 |
| `blocked` 被阻止 | 3 |
| `started` 已开始 | 1 |
| `ended` 已结束 | 1 |

计划第 6 节的第四种性质「结果记录」在本目录中不单列，而是「`source_recorded` 动作 + 结果语义」的组合：来源记录了任务完成事件、与任务执行成功，是同一条记录的两个描述，不是两种动作。`started` 与 `ended` 分开登记，保证「开始」不会被显示成「成功」。

`observed.*` 系列（配置变化、状态变化、首次观察、本次未发现）单独归类为快照比较产生，从类型上就不允许被描述成人工操作。

71 个动作中 21 个类型声明了动作，80 个类型没有动作声明（其变化由快照比较的 `observed.*` 表达）。每个动作的目标类型与类型的动作列表双向校验：动作指向不存在的类型会失败，类型声明了登记表中目标不同的动作也会失败。

## 6. 来源映射草案

128/128 个 S01 来源都有明确去向，无遗漏项。

| 去向 | 条数 | 说明 |
| --- | --- | --- |
| `observation` 对象观察 | 96 | 映射到 1 个或多个对象类型 |
| `action_evidence` 动作证据 | 28 | 23 个日志通道 + USN 变更 + 软件检测 + 更新历史 + 导入通道 |
| `coverage` 采集状态 | 4 | 通道可读性、指定范围配置、USN 断点与名单状态 |
| `sampling` 运行采样 | 1 | 内存与处理器时点采样，不是对象 |

非 `observation` 的去向必须写明理由，否则检查失败——避免把「不是对象」的条目含糊地塞进对象列表。

一个来源可以映射到多个去向：例如 `pcscope.usn.{drive}` 既产生文件变更动作证据，也在断点校验失败时产生采集状态。一个来源也可以映射到多个类型：`pcscope.shell.registrations` 覆盖 7 个类型，`pcscope.security.protection-policy` 覆盖防护配置与排除项。

## 7. 尚未接入来源的类型

模型目录里存在、但当前没有任何来源的类型，显式标记，不以空列表冒充已采集：

| 类型 | 类别 | 原因 |
| --- | --- | --- |
| `security.key_metadata` | 安全与信任 | 现有来源没有密钥元数据对象；密钥操作日志只证明发生过操作 |
| `file.watched_directory` | 文件与访问控制 | 当前文件名单只接受文件，`Scan-WatchedFile` 明确拒绝目录 |
| `hardware.driver_package` | 硬件与驱动 | 驱动仓库包枚举未实现（`collector-coverage.md` 记为待补） |

检查要求「有来源」与「被标记为无来源」互斥且完备：把有来源的类型标成无来源会失败，把无来源的类型不标记也会失败。

## 8. 注入测试（验收硬性条件）

五次人为注入全部被拦下并按名报出，注入后均已精确还原（最终检查通过、`model/catalog.json` 重新生成一致）。

| 注入 | 结果 |
| --- | --- |
| 把 `shard` 加入计划任务的身份字段 | 退出码 1：`identity key "shard" contains collection delivery detail "shard"` |
| 引用不存在的动作 `startup.task_vanished` | 退出码 1：`references unknown action "startup.task_vanished"` |
| 把有来源的类型标为「无来源」 | 退出码 1：`is fed by a source but is listed as having none` |
| 把某来源名改错，使其失去去向 | 退出码 1：同时报出「无 S01 来源」「S01 来源无去向」「覆盖了 S01 不认识的来源」 |
| 把命名实体策略的身份键改成只有显示名称 | 退出码 1：12 个类型同时报 `identity must not be a display name alone` |
| 声明不一致的 `MODEL_CATALOG_VERSION` | 退出码 1；声明相同版本则通过（证明检测生效，不是恒失败） |

**两次自我纠错，都记在此处：**

1. **退出码为 1 不等于检查起了作用。** 最初验证版本约束时，脚本因 `rel` 未定义抛 ReferenceError，退出码同为 1，看起来像「测试通过」。核实失败原因后发现是**假阳性**——干净状态下消费者列表为空，循环根本没执行。修正后重测：错误版本报错、匹配版本通过，这才证明检测生效。

2. **动作的两个轴被写混，且校验漏检。** 动作登记的行格式是「性质 | 结果」，但我给 71 行中的 68 行在结果槽位填了 `result` 这个值——它既不是合法结果语义，也把「结果记录」这一性质混进了结果轴。校验器当时只验证性质、没验证结果，所以这个无效值一路通过。修正为：性质轴取 `source_recorded` / `snapshot_change` / `state_change` / `internal`，结果轴取 `result_semantics` 中的合法值，并在校验器中补上结果语义校验与目标类型校验。

修正后新增的**目标类型双向校验立刻抓出一处真实矛盾**：`identity.logon` 在登记表中指向 `account.user`，而 `runtime.logon_session` 也声明了该动作。一个动作只有一个目标对象，按模型登录事件创建的是登录会话、账户的参与由会话的用户字段体现，因此把 `identity.logon` / `identity.logoff` / `identity.logoff_requested` 的目标改为 `runtime.logon_session`，并从 `account.user` 的动作列表移除。这条是新增校验带来的实际修正，不是理论检查。

## 9. 需复核的建模判断

以下是本步做出的、依据不够充分的归类，留给后续复核，不视为已定论：

1. **`policy.system_restore_point` 归入策略与配置**。该类别的定义是「系统和应用按照什么规则运行」，而还原点是系统状态而非规则。十二类已冻结，暂不新增类别；若复核认为不妥，应在 S04 冻结 schema 前提出。
2. **`account.machine_identity` 归入账户与身份**。计算机身份与域信息既涉及身份（域、角色）也涉及硬件（制造商、型号）。另一种选择是归入硬件与驱动。
3. **`policy.os_configuration` 的边界较宽**，吸收了操作系统版本、安装时间、最近启动时间、待重启标志，以及被 Sysmon/组策略/注册表审计事件指向的目标。这是为了让无独立身份的事件有归属类型；若 S09–S20 发现需要拆分，应在此目录中调整。
4. **`security.exclusion` 需要适配器展开**：现有来源把排除项作为聚合字段投递，逐项展开为独立对象属于 S17 的工作，本步只定义类型与身份策略。

## 10. 遗留问题

1. **前后端尚未真正使用同一目录**。当前只做到：单一编写来源、每次检查重新生成、版本戳一致、任何声明版本的消费者不一致即失败。消费者数量为 0——Rust 内嵌（S04）与界面读取（S22）尚未接入。验收条件「前后端使用同一目录版本」在本步只完成了可检查的基础，未完成接线。
2. **字段级契约未逐类型核实**。类型的关键字段是按现有采集项与模型设计编写的，尚未逐项对照真实输出。按计划属于 S09–S20 的「核查真实输出结构」，S21 做全来源覆盖对账。
3. **`model/catalog.json` 未纳入既有前端构建**。`collection-catalog.json` 仍由 `plugin-sources.js` 独立生成，两套目录并存；并行期需要明确谁是谁的唯一来源，这属于 F1（S04–S08）的切换工作。
4. **动作登记的 71 个 ID 未与 `event-items.js` 的现有动作 ID 逐一对账**。计划要求「保留已有可读解释」，对账属于 S21。
5. **`policy.browser_policy` 只覆盖浏览器策略**，「应用策略」（AppLocker 等）目前没有独立类型，仅在事件映射中作为未归属证据保留。

## 11. 下一步

S04 的前置（S03）已满足：类别、类型、身份与字段契约已冻结并可由脚本校验，来源映射覆盖 100%。S04 需要实现增量数据库结构，并让 Rust 与界面真正共用同一份目录与版本。
