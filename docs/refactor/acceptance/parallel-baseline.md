# R01 准入基线

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | R01 · 已完成执行，待复核 |
| 依据 | [并行开发前置条件与准入方案](../parallel-readiness.md) 第 3 节 R01 |
| 实测时间 | 2026-09-11 |
| 交付 | 本文件（已通过项、缺口、可供各工作线使用的基线） |

本文记录**实际执行结果**，不引用未经验证的环境标签。

## 1. 执行环境探测（现场实测）

```text
工作树根       C:\Users\radiohide\Desktop\SYS\SSS          （pcscope 的父目录，不是 pcscope 本身）
分支           main
探测时 HEAD    a2311a4  "Organize system settings into tabs with compact diagnostics"
提交后 HEAD    2f69dc8  （本轮 7 笔提交，见第 6 节）
Git            git 2.55.0.windows.1
               C:\Users\radiohide\AppData\Local\Programs\PortableGit\cmd\git.exe
               （不在 PATH 上，需要显式加入或使用完整路径）
Git 身份       仓库级配置 user.name/user.email，与历史 6 笔提交一致
探测时未提交   20 项 = 6 个已修改 + 14 个未跟踪目录/文件；现已在第 6 节提交完毕，工作区干净
```

**必须先记录一个此前的错误结论。** 上一轮评审曾报告「这个目录不是 git 仓库，机器上没有任何 git 可执行文件」。该结论**是错的**，原因有二：仓库根在上一级目录 `SYS/SSS`，而检查只看了 `SYS/SSS/pcscope`、`SYS`、`Desktop`、`~`，**漏了中间的 `SSS`**；Git 只查了三个硬编码路径，漏了 `%LOCALAPPDATA%\Programs\PortableGit`，且 `where.exe git` 因不在 PATH 而失败。会话元数据当时报告的「是 git 仓库、main 分支、干净」反而更接近事实。

这正是 `parallel-readiness.md` 第 1.1 节要求「每个实际执行环境都必须现场检查路径和命令」的实例。教训记录在此：**不采信未经验证的标签，也包括不采信自己未穷尽的探测。**

## 2. 恢复路径选择

Git 可用，选**有 Git 路径**。但未提交的 4 个步骤产出不在版本控制之内，因此按方案要求补做独立快照。

| 项 | 值 |
| --- | --- |
独立副本位于 `C:\Users\radiohide\Desktop\SYS\pcscope-baseline-snapshots\r01-baseline\pcscope-all.bundle`（916 KB）。

**形式选择过程**：最初采用「逐文件复制 + SHA-256 清单」，恢复演练通过（47/47 一致）。提交完成后改为归档整个提交，第一版用了 `git archive` 生成的 tar——**这违反了本文件第 3 节自己的结论**（tar 无法正确处理非 ASCII 文件名）。已改为 `git bundle`，它是 git 原生格式，且实测可还原中文名文件。

副本形式与验证：

| 项 | 值 |
| --- | --- |
| 形式 | `git bundle --all`（含全部引用） |
| 大小 | 916121 字节 |
| 内容 | `refs/heads/main` 及 `refs/remotes/origin/*` |
| 还原方式 | `git clone <bundle> <dir>` |
| 验证 | 克隆后 HEAD = `bd6ce23`，181 个文件，`PCScope-开发计划.md`（中文名）存在，S04 模块与还原文档均在 |

副本**不会自动更新**：每次提交后需重做（`git bundle create <path> --all`）。恢复基线优先用第 3 节选定的 `git worktree` 方式；副本用于 git 不可用或需要异地副本时。

已排除 `.gitignore` 覆盖的运行库、数据库、构建产物与生成的真实规模语料（`fixtures/scaled/`，11 MB，可由 `node scripts/build-fixtures.cjs --scale` 重建）。首次快照误将 `fixtures/scaled/` 一并复制（12 MB），已重建。

## 3. 恢复演练（三种方式，实测结果）

| 方式 | 结果 | 结论 |
| --- | --- | --- |
| 逐文件快照还原（提交前） | 47 个文件逐一还原到独立目录并比对 SHA-256，**0 处不匹配** | 可用 |
| `git archive a2311a4 \| tar -x` | **不可靠**：非 ASCII 文件名被写成转义形式，2 个文件报 `Invalid empty pathname` 完全未提取；提取到 140 个文件，提交中为 142 个 | **不采用** |
| `git worktree add --detach a2311a4` | 提取 142 个文件，与提交一致；中文名文档正常；主工作树未受影响 | **采用** |
| `git bundle --all` + `git clone` | 克隆得到 181 个文件，HEAD 正确，中文名文档正常 | **采用**（作为独立副本） |

恢复基线提交请用 `git worktree add --detach <dir> <commit>`，不要用 `git archive` 管道到 `tar`。

## 4. S01–S04 实际验收状态

| 步骤 | 交付物 | 状态 |
| --- | --- | --- |
| S01 盘点来源与既有功能 | `model/sources.json`、`model/regression-checklist.json`、`scripts/check-source-inventory.cjs`、验收记录 | 已交付 · **待验收** |
| S02 固定样例与性能基线 | `fixtures/`、`crates/storage/examples/refactor-baseline.rs`、`model/baseline-*.json`、验收记录 | 已交付 · **待验收** |
| S03 冻结类型身份字段契约 | `model/vocabulary.cjs`、`types.cjs`、`source-map.cjs`、`catalog.json`、`scripts/check-model-catalog.cjs`、验收记录 | 已交付 · **待验收** |
| S04 增量数据库结构 | `crates/storage/src/model.rs`、`schema-info.rs`、结构说明、验收记录 | 已交付 · **待验收** |

**没有任何步骤被标记为「通过」。** 方案第 1 节指出 S03 记录存在「状态表写待验收、末尾写前置已满足」的表述差异——这不是矛盾：状态表的「待验收」描述该步骤本身的验收状态，末尾的「前置已满足」描述**下一步的前置条件已具备**。两者含义不同。此点在此明确，避免后续再被读作自相矛盾。

各步骤的验收证据均在 `docs/refactor/acceptance/S0*.md` 中，含可复现命令与实测结果。

## 5. 基线内容

| 项 | 值 |
| --- | --- |
| 基线提交 | `a2311a4`（= 当前 HEAD，工作区改动未提交） |
| 目录版本 | `1.0.0` |
| 目录摘要（SHA-256） | `348dbde1040d53eab0cfff1008a0f31234b166bea3c8cd572a35fd32f8ccde04` |
| 数据库结构版本 | 6（`pcscope_storage::model::SCHEMA_VERSION`） |
| 样例版本 | 基础语料 41 条协议消息 + 6 个非法输入；真实规模语料由脚本按需生成 |
| 目录规模 | 12 类、101 类型、71 动作、128 来源（129 条映射行，`pcscope.usn.{drive}` 有两个去向） |
| 工具链 | 见第 7 节 |
| 未提交改动快照 | 见第 2 节 |

目录摘要在三处一致：`model/catalog.json` 文件本身、Rust 内嵌计算并写入的 `settings.model_catalog_v1`、`docs/refactor/parallel-boundaries.md` 声明的输入摘要。

## 6. 提交记录

探测到的 20 项未提交改动已按步骤提交为 7 笔，工作区现为干净。

| 提交 | 内容 | 归属 |
| --- | --- | --- |
| `9359658` | 对象模型设计与重构计划文档 | 既有输入文档 |
| `42c9cc4` | 来源盘点与目录对账（S01） | S01 |
| `b2b15bd` | 去标识样例语料与查询/容量基线（S02） | S02 |
| `4d97247` | 类型、身份与来源映射目录（S03） | S03 |
| `9e0a43d` | 对象模型增量数据库结构（S04） | S04 |
| `7d2d8e5` | 外部工具链解析与并行基线（R01） | R01 |
| `2f69dc8` | 并行准入方案与生成的类型边界表 | 另一条工作线 |

按步骤分笔的目的：每步的验收证据与其实现同处一个提交，可单独回溯与回退。另一条工作线的三个文件单独成笔，未混入本工作线的提交。

相对原基线 `a2311a4`，累计 46 个文件、+17255 / −13 行。

**远端状态**：仓库配置了远端 `origin = https://github.com/radiohide/SSS.git`。本地 `main` 相对 `origin/main` **领先 8 笔、落后 0 笔，尚未推送**。推送是对外发布动作，本步只做本地提交，未推送。

**提交时发现的一处工作区异常**：会话开始时，`pcscope/docs/` 下 14 个**已跟踪**文档在工作区缺失（README 与计划中的文档链接因此是死链）。这些文件已从 `docs/sensitive-display.zip` 还原，且与提交内容**逐字节相同**（`git diff HEAD` 为空），因此不产生改动、无需提交。该异常说明探测时的检出并不完整，值得在后续环境预检中留意。

另：`pcscope/apps/desktop/ui/plugin-sources.js` 在探测时被列为已修改，但内容与 HEAD 的 blob 哈希完全相同，属于 stat 缓存差异，`git add` 刷新后即消失，未产生提交。

## 7. 工具链：外部依赖登记与构建修复

工具链**不在仓库内**且被 `.gitignore` 覆盖，属于外部依赖。

| 组件 | 路径 | 版本 |
| --- | --- | --- |
| 工具链根 | `C:\Users\radiohide\Desktop\SYS\pcscope\.tools` | — |
| Cargo / rustc | `<根>\cargo\bin` | 1.98.1 |
| Rustup home | `<根>\rustup` | stable-x86_64-pc-windows-gnu |
| 目标平台 | `x86_64-pc-windows-gnu` | — |
| 链接器 bin | `<根>\mingw\mingw64\bin` | MinGW-w64 |
| Node | PATH | v24.20.0 |
| PowerShell | — | 5.1.26100.9278 |
| C# 编译器（原生读取器） | `%SystemRoot%\Microsoft.NET\Framework64\v4.0.30319\csc.exe` | .NET Framework 4.0 |
| Git | `%LOCALAPPDATA%\Programs\PortableGit\cmd` | 2.55.0 |

**已完成构建修复。** 原 `build.ps1` 无条件把 `CARGO_HOME` 指向仓库内 `.tools`，该目录不存在，任何新检出直接失败。现改为按序解析并在失败时列出搜索过的路径：

1. `-ToolsRoot <path>`
2. `$env:PCSCOPE_TOOLS`
3. `<仓库>\.tools`
4. `<祖先>\.tools` 与 `<祖先>\*\.tools`，最多四级祖先

显式请求是权威的：`-ToolsRoot` 或 `PCSCOPE_TOOLS` 指向无效目录时**直接失败并报出该路径**，不会静默换用另一套工具链——用错工具链比失败更糟。

新增 `-Check` 预检，在无历史 PATH 补丁的新终端中报告全部外部依赖的解析结果与版本。实测：

```text
tools root   C:\Users\radiohide\Desktop\SYS\pcscope\.tools
cargo 1.98.1 / rustc 1.98.1 / host x86_64-pc-windows-gnu / linker <根>\mingw\mingw64\bin
csc %SystemRoot%\Microsoft.NET\Framework64\v4.0.30319\csc.exe   node v24.20.0
```

三条失败路径均实测：错误的 `PCSCOPE_TOOLS`、错误的 `-ToolsRoot`、以及远离任何 `.tools` 的副本——三者都以退出码 2 结束并指出准确路径。

## 8. 可复现性验证

从基线重新生成目录并运行既有检查：

```text
node scripts/build-collection-catalog.cjs      → 219 条采集定义
node scripts/check-model-catalog.cjs           → 101 类型 / 128 来源 / 35 多来源类型 / 13 多类型来源
node scripts/build-parallel-boundaries.cjs     → 已生成
node scripts/build-fixtures.cjs                → 基础语料
```

四份生成物（`model/catalog.json`、`collection-catalog.json`、`collection-controls.json`、`parallel-boundaries.md`）重新生成后**逐字节一致**；`build.ps1` 解析零错误；13 个既有检查全部通过；`.\build.ps1` 默认的命令（`cargo test --workspace`）通过 47 项测试。

快照中的 46 个文件与当前工作区逐一比对，**0 处漂移**。

## 9. 准入判定

| 准入等级 | 判定 | 依据 |
| --- | --- | --- |
| 准备（盘点、样例、设计、边界澄清） | **可进行** | 无前置要求 |
| 独立开发 | **不可进行** | 需要 R01–R08 对该工作线所需契约全部通过，且 S04 已验收；当前 S04 为待验收，R02–R08 未开始 |
| 全工作线并行 | **不可进行** | 需要 R01–R09 全部通过 |
| 实际步骤验收 | **不可进行** | 需要主计划对应步骤的真实依赖满足 |

按方案第 3 节，实际顺序为：**S04 通过 → R01（本文件）→ R02–R05 提前接口工作 → 其余准入任务通过 → 并行实施**。本文件完成的是 R01 的**执行**，但 R01 自身的验收条件之一是「S04 的新库/旧库升级、幂等启动及事务失败恢复通过」——该证据已在 `S04-schema.md` 第 3–6 节给出，因此 **R01 的验收具备证据**，判定留给复核。

## 10. 缺口与阻塞

1. ~~**未提交改动无版本控制保护。**~~ **已解决**：第 6 节的 7 笔提交已把全部产出纳入版本控制，工作区干净。第 2 节的独立快照保留作为同一内容的第二份副本，其 `manifest.json` 仍可逐文件校验；后续推进不再依赖它。
2. **R02–R05 未开始。** 这四项是提前实现并冻结 S05/S06/S07/S22 的接口部分，成本计入原步骤，不是无成本的拆解。
3. **`build.ps1` 的绝对路径不作为跨环境默认值。** 第 7 节的路径全部是本机实测结果，其他环境必须用 `-Check` 重新探测。方案第 1.1 节对此有明确要求。
4. **只读方在迁移窗口内仍会报 schema 错误**（S02 已记录，S04 未改变）。
5. **新表未纳入容量预算**（S04 已记录，属于 S08）。
6. **`coverage_scopes` 与既有 `coverage_state` 的汇总关系未定义**（S04 已记录，属于 S08）。
7. **两套采集目录仍并存**：`apps/desktop/ui/collection-catalog.json` 与 `model/catalog.json`。方案第 1 节将此列为 R07 必须解决的问题，当前未解决。
8. **`git archive | tar` 恢复路径不可用**（第 3 节），任何自动化脚本不要使用该方式还原基线。

## 11. 建议的下一步

产出已按步骤提交（第 6 节），缺口 1 已消除。按方案顺序，下一步应先取得 S01–S04 与 R01 的验收判定，再开始 R02–R05 的接口冻结工作。
