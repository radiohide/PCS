# S31 验收记录：完整构建与用户流程回归

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S31 · 待验收（**构建、检查与 IPC 全链路均为本机实测；未做 WebView2 界面实机运行**） |
| 实现范围 | Rust 全工作区测试；桌面构建；原生事件读取器构建；PowerShell 解析与来源检查；前端行为检查；「插件配置 → 采集 → 对象 → 动作 → 证据」闭环回归 |
| 代码定位 | 新增 `crates/storage/src/flow.rs`、`docs/refactor/shared-changes/0008-event-classification-and-scope.md`、本记录；修改 `crates/contracts/src/adapters/event.rs`、`crates/contracts/src/reference.rs`、`crates/storage/src/{evidence,projection,lib}.rs`、`crates/contracts/examples/adapter-coverage.rs`、`scripts/{check-adapter-coverage,build-source-type-map}.cjs`、`model/{adapter-coverage,ownership}.json`、`apps/desktop/src-tauri/Cargo.lock` |
| 验证环境 | Windows 11 26200；cargo / rustc stable-x86_64-pc-windows-msvc（**未安装 GNU target**，见第 5 节第 1 项）；Node v24.14.0；PowerShell 7；**本机真实采集**（34k 对象规模） |
| 验证过程 | `cargo test --workspace`；`cargo build`（`apps/desktop/src-tauri`）；`plugins/eventlog-native/build.ps1`；21 个 `check-*.cjs`；4 个 `test-*.ps1`；`probe.exe` 及其 6 个子命令对真实守护进程 |
| 结果 | Rust 347 项测试通过；21/21 检查通过；桌面与原生读取器构建通过；闭环回归走通并在修复后通过；两份生成产物逐字节可重生成。缺陷与未覆盖项见第 3、5 节 |
| 遗留问题 | 见第 5 节 |
| 下一步 | S32 整体切换与恢复演练 |

## 1. 构建与检查

| 项目 | 命令 | 结果 |
| --- | --- | --- |
| Rust 全工作区测试 | `cargo test --workspace` | **347 通过 / 0 失败**（contracts 239、storage 97、usn 4、service 3、ipc 2、supervisor 1、software 1；17 个测试目标） |
| 桌面构建 | `cargo build`（`apps/desktop/src-tauri`，独立工作区） | 通过，产出 `pcscope-desktop.exe` |
| 原生事件读取器 | `plugins/eventlog-native/build.ps1`（csc） | 通过，产出 `target/debug/pcscope-eventlog-native.exe` |
| 契约与目录检查 | `scripts/check-*.cjs` | **21/21 通过** |
| 生成产物一致性 | 重跑两个生成器并逐字节比对 | `model/adapter-coverage.json`、`apps/desktop/ui/source-type-map.json` **均无差异** |

S30 记录 344 项，本次 347 项：**+3 恰好是本步新增的回归测试**（`flow.rs` 两项、事件分类一项），没有其他测试被删除或跳过。

**「347 通过」是去掉了管道之后读到的真实值。** 本步第一次跑用了 `cargo test --workspace | tail`，管道的退出码是 `tail` 的，于是失败的与成功的两次运行都报 `exited with code 0`——**退出码在这里不是证据**。重跑改为直接落盘再统计 `test result:` 行，上述数字来自后者。

## 2. 闭环回归走通的链路

`crates/storage/src/flow.rs` 在同一处按顺序断言每一段，而不是只在末尾断言：

```
插件配置 → 采集材料 → 证据 → 身份 → 事实与动作 → 查询 → 证据 → 覆盖
```

用的材料是**采集端真实发出的形状**（`machine:M/user:S-1-5-18/schema:1/context:/shard:0`，动作分类在信封的 `kind` 上、字段在 Windows 名下），不是夹具形状。走完的结果：两条配置来源与一份运行日志**合成一个实例**；`startup.task_started` 与 `startup.task_ended` 是**两个动作**而不是一次成功；字段的证据引用指回它来自的来源；采集项关掉再打开的区间在覆盖里是 `gap` 而不是静默补齐；最后 `reconcile` 判定 `reconciled`。第二项回归断言**同一份流程重放不新增任何东西**。

**这一步走通之前是不通的**，见第 3 节。

## 3. 本步修掉的缺陷

**声明的分类到不了适配器，于是同一个计划任务被记成两个实例，其中一个没有名字。**

契约把分类放在信封的 `Event.kind` 上，`record_event` 把它写进证据的 `item_id`；而投影交给适配器的素材是记录里的 `data`。`adapters/event.rs` 的「事件自陈其动作」路径读的是 `payload["kind"]`——**没有任何生产者往 `data` 里放这个字段**（`collect-eventlog.ps1` 把分类放在事件上，`data` 由 `fields`/`system`/`field_schema` 组成）。于是该路径在真实素材上从未生效，每条已分类事件都落到事件 id 表回退；回退表不认识 `TaskName`，只能给出 `?path` 的临时身份，运行日志便在任务旁边另立一个实例。

适配器自己的单元测试用的是把 `kind` 放进 `data` 的形状——**一个没有生产者会发出的形状**，所以这条路径的失效一直没有被任何测试碰到。这是「按真实采集材料回归」与「按夹具回归」的差别，也是本步要求走完整闭环而不是只跑单元测试的原因。

已修：分类先读 `payload["kind"]`（自陈动作的素材行为逐字节不变），取不到目录里登记的动作时读证据的 `item_id`——契约把它定义为「该来源内的分类身份」。回退路径保留，未分类事件行为不变。

同批修掉的第二处：`pcscope.core`（应用自身的活动）不被认作事件源，投影时找不到适配器，被报成解析失败——健康页会出现一屏「解析失败」，而内容其实是应用在描述自己。现按 `EVENT_SOURCES` 明确认作事件源，并计入 `skipped` 而不是 `duplicate`。

两处都记在 `docs/refactor/shared-changes/0008-event-classification-and-scope.md`。**第二处之外还有一处行为变化**：参考适配器的身份作用域改为按机器级收窄（`installation_scope`），这是 0005 留的未决问题，此处按与 S09–S20 类别适配器一致的规则落定；存量实例不自动重算，边界写在 0008 的「数据迁移」一节。

## 4. 真实采集与后台连接实测

在本机启动真实守护进程（隔离数据目录 `data/daemon-s31`），用**桌面端同一个 `pcscope-ipc` 客户端**驱动完整采集与查询：

| 探测 | 结果 |
| --- | --- |
| 完整流程（默认） | **IPC PASS**：status、采集、时间线、对象、快照、同一快照对比无差异；`objects=34307 records=34466 snapshots=2028` |
| 采集来源覆盖 | **11 个采集作业，7 个显式报告来源缺口** |
| `plugin-control` | PASS：插件配置、间隔校验、停用后的单次运行、重复运行拒绝，**并恢复原始配置** |
| `item-controls` | 216 个采集项，停用 0，版本 0；配额 unlimited / 5 GB，物理 474 MB |
| `objects` | PASS：34298 个当前对象上的搜索、计数与组合过滤 |
| `collection-view` | 时间线 117 ms；对象过滤 1608 ms；75 个来源分面 |
| `monitoring` | `elevated=false`、`checking=false`、数据目录与历史通道就位 |

**权限不足路径可解释**：7 个作业给出的是具名原因（`Access is denied` / `requires elevation` / `spooler service is not reachable` / 找不到 `Get-VM`），并且格式区分「来源不可用，保留先前状态」与「静默的空」——与 `test-collector-audit.ps1` 在 9 个域上的实测一致。

**用户配置全部保留**：`plugin-control` 在改动插件配置、验证校验与拒绝路径之后**恢复原值**并断言通过；`item-controls` 报告停用 0、版本 0，即本步没有改动任何用户配置。

**这条证据的边界**：连接是通过桌面端所用的同一 IPC 客户端走的，**不是**通过 WebView2 界面点出来的（第 5 节第 2 项）。

## 5. 遗留问题

### 1. 桌面构建用的是 MSVC，不是清单写的 GNU target

`model/regression-checklist.json` 的 R01 写「apps/desktop/src-tauri 独立构建（GNU target）」，但本机 `rustup target list --installed` 只有 `x86_64-pc-windows-msvc`，仓库内也没有 `rust-toolchain.toml` 钉住工具链。**本步构建通过，但目标三元组与清单不一致**，需要在有 GNU target 的环境复跑一次，或确认清单该行应改为 MSVC。

### 2. 未做 WebView2 界面实机运行

桌面二进制构建通过，但**没有真正打开界面点过**。原因是这会在这台机器上弹出窗口并打断使用者，不适合在无人确认时进行。「真实桌面可连接现有后台」一项用 IPC 客户端取得（第 4 节），**不能据此宣称界面已验证**。

### 3. `test-daemon.ps1` 在本机跑不通，三处原因，均非本次重构引起

`crates/service`、`scripts/test-*.ps1` 属 `existing-runtime`，不在四条工作线内，故**只记录不改动**：

1. **找不到 `.tools`**：`build.ps1` 打印搜索路径后返回非零，而 `test-daemon.ps1` 用 `& .\build.ps1` 调用、不检查 `$LASTEXITCODE`，于是**构建失败被静默跳过**，测试继续用旧二进制。本机无 `.tools`（工具链走 PATH），所以这一路必然发生。
2. **不建 `data/`**：脚本直接 `Start-Process ... -RedirectStandardError data\daemon-smoke.stderr.log`，而 `data/` 被 `.gitignore` 忽略、新克隆不存在，于是启动即失败。手工建目录后可继续。
3. **与守护进程启动采集抢道**：守护进程每秒一次的调度在启动时就发起采集，本机一轮约 2–3 分钟；脚本只 `Start-Sleep -Milliseconds 800` 就调 `probe.exe`，探测里的 `Collect` 撞上正在跑的采集，被守卫以「同类采集正在执行，请完成后重试」拒绝。

**第 3 项里守卫的行为是对的**（重复采集本就该被拒），错的是测试的等待假设。手工等采集空闲后重跑，`probe.exe` 全部通过（第 4 节）。要让 R11 开箱可跑，需要的是「先等 `collecting=false` 再探测」，而不是放宽守卫——**这是一处未修的测试健壮性问题，不是产品缺陷**。

### 4. `test-collector-audit.ps1` 依赖控制台输出编码为 UTF-8

本机 `[Console]::OutputEncoding` 是 **gb2312**，而采集端输出 UTF-8；PowerShell 按 GBK 解码后，第一条含非 ASCII 的行（第 33 行）就不是合法 JSON，脚本报 `Conversion from JSON failed ... items[26].data.title`。

**不是采集端缺陷**：9 个域的输出用 node 逐行解析**全部合法**，设置 `[Console]::OutputEncoding=UTF8` 后 PowerShell 也 0 失败行。脚本头部只写了「Run from PowerShell 7」，没有约束控制台编码。同样**只记录不改动**（`existing-runtime`）。

### 5. 未做界面级与长跑项（S29/S30 顺延）

- 5 GB 实测与真实盘配额回收（S30 第 4 节第 1 项）；
- 72 小时长跑下的增长曲线（S30 第 4 节第 2 项）；
- 事件批（`event_batch`）入库与投影的独立开销测量（S30 第 4 节第 3 项）；
- 清理与投影并发的容量行为（S29 顺延）。

### 6. 自身活动（self-activity）只证否、未证有

`probe.exe self-activity` 三种模式都运行且未报错，但 `only` 返回 0 行、`all` 中标记为自身的行数为 0——本次运行里**没有产生自身活动记录**。证否路径（`hide` 不返回自身行、`only` 不返回未标记行）成立，**肯定路径未被正向覆盖**，需要一次会改动采集配置的会话才能产生。

## 6. 验收条件逐条对照

| 验收条件 | 结论 | 依据 |
| --- | --- | --- |
| 所需构建和检查通过 | 通过 | 347/0；21/21；桌面与原生读取器构建；两份生成产物可重生成（第 1 节） |
| 插件配置→采集→对象→动作→证据完整闭环 | **通过（本步修好才成立）** | `flow.rs` 逐段断言；真实守护进程 IPC PASS（第 2、3、4 节） |
| 真实桌面可连接现有后台 | 通过（**经 IPC 客户端实测，非界面**） | 第 4 节；边界见第 5 节第 2 项 |
| 权限不足路径可解释 | 通过 | 7/11 作业给出具名原因；9 个域的来源缺口（第 4 节） |
| 用户配置全部保留 | 通过 | `plugin-control` 恢复原值并断言；`item-controls` 停用 0、版本 0（第 4 节） |
| 没有以「以后修」挂起的数据正确性、隐私或关键操作缺陷 | 通过 | 第 3 节两处缺陷**已修**。第 5 节第 3、4 项是**验证脚手架的健壮性问题**，不属于数据正确性、隐私或关键操作缺陷，且已给出成因与修法 |

**状态定为「待验收」而不是「通过」**，因为第 5 节第 1、2 项未覆盖：目标三元组与清单不符，界面未实机运行。这两项不改变第 2–4 节的结论，但按第 18 节的判断原则，**没有做到的不写成做到了**。
