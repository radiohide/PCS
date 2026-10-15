# 工作区与集成策略（R08）

日期：2026-09-12。脚本：`scripts/verify-workstream.ps1`。所有权依据：[代码与目录所有权](ownership.md) 与 `model/ownership.json`。

依据：[并行开发前置条件与准入方案](parallel-readiness.md) R08。

## 1. 每条工作线一个工作树

方案要求「每条独立开发线使用独立分支和工作目录」。本机具备该条件（R01 实测 git 2.55.0，仓库根 `SYS/SSS`），因此采用工作树隔离，**不退化为共享目录串行整合**。

| 工作线 | 分支 | 工作树 |
| --- | --- | --- |
| 基础数据线 | `ws/base-data` | `SYS/pcscope-worktrees/base-data` |
| 类型适配线 | `ws/type-adapters` | `SYS/pcscope-worktrees/type-adapters` |
| 查询与界面线 | `ws/query-ui` | `SYS/pcscope-worktrees/query-ui` |
| 迁移验证线 | `ws/migration` | `SYS/pcscope-worktrees/migration` |

分支从 R01 基线创建；工作树**放在两个仓库之外**（`SYS/pcscope-worktrees/`），不落在任何仓库目录内——否则 `check-ownership.cjs` 会把工作树里的文件也当成需要归属的文件。

创建：

```powershell
git -C C:\Users\radiohide\Desktop\SYS\SSS worktree add -b ws/<id> C:\Users\radiohide\Desktop\SYS\pcscope-worktrees\<id> <基线提交>
```

**工作树是整个仓库的检出，而仓库根是 `SYS/SSS`。** 因此工作树里还有一个 `pcscope/` 子目录，工程根是 `<工作树>/pcscope`，不是 `<工作树>`。`verify-workstream.ps1` 由脚本自身位置推导工程根，不需要调用方指定。

### 1.1 共同基线的历史不得改写

工作树一旦从某提交创建，**该提交不得再被修订或改写**——工作线分支会与之分叉。需要更正时向前追加提交。

本步实际踩到过这一点：修订了基线提交之后，两个工作线分支立刻分叉。分支上当时没有真实工作，因此重置是安全的；若有真实工作，就必须改用合并。

## 2. 独立测试目录

每条工作线在自己的工作树里运行，因此以下全部天然隔离：

| 资源 | 隔离方式 |
| --- | --- |
| 编译产物 | 每个工作树有自己的 `target/` |
| 测试数据库 | `:memory:` 或系统临时目录，文件名含进程号与时间戳；`schema-info` 等示例由调用方显式指定路径 |
| 夹具与生成物 | 工作树内各自生成，互不可见 |
| 端口 | 不使用 TCP 端口 |

**不得并行写正式库。** 测试与示例只允许写：内存库、系统临时目录、工作树内的 `target/` 与 `data/`。

### 2.1 唯一的真实约束：每用户命名管道

守护进程使用**每用户唯一**的命名管道 `\\.\pipe\PCScope.dev.<用户SID>`，并以 `first_pipe_instance` 创建，第二个实例会被拒绝（既有测试 `test-daemon.ps1` 覆盖了这一行为）。

因此：

- **任何启动真实守护进程的测试不能并发运行**，无论是另一条工作线还是本机已有一个开发中的守护进程；
- 这是**有意的**设计，不是缺陷：同一用户下两个守护进程同时写同一份历史会破坏一致性保证；
- 因此 `test-daemon.ps1` 与 `stage-dev.ps1` 不在 `verify-workstream.ps1` 的常规验证集内，它们是集成门禁，需要串行执行。

其余全部（脚本检查、Rust 单元测试、契约示例）可并发。

### 2.2 不得指向正在运行的库

本机当前有一个开发中的守护进程在使用**另一个检出**的库（`SYS/pcscope/data/daemon-smoke/history.db`）。本仓库的脚本一律以 `$PSScriptRoot` 推导工程根，已核查没有任何脚本引用该路径；测试也不会写到那里。

**约定：任何测试与示例都不得把路径指向已存在的运行库。** 需要真实规模的验证时，用一致性备份或拷贝，不在原库上操作。

## 3. 每线验证集与结果追溯

```powershell
.\scripts\verify-workstream.ps1 -Workstream <id> [ -SkipCargo ]
```

脚本按 `model/ownership.json` 的归属，只跑该工作线范围内的检查，而不是整套：

| 工作线 | 验证集 |
| --- | --- |
| `base-data` | 目录校验、归属校验、`cargo test -p pcscope-contracts -p pcscope-storage` |
| `type-adapters` | 来源盘点、契约用例、`cargo test -p pcscope-contracts` |
| `query-ui` | 查询契约、界面与展示相关检查（6 项） |
| `migration` | 目录校验、`cargo test -p pcscope-cli` |

**每次运行都打印**工作树路径、分支、提交、目录版本与规则版本：

```text
workstream      query-ui (查询与界面线)
work tree       C:\Users\radiohide\Desktop\SYS\SSS\pcscope
branch          main
commit          34e63b9
catalog version 1.1.0
rule version    c5f29bb3…
```

这就是「能定位每条结果所用契约版本」的落点：结果与它依据的契约版本始终一起出现，不需要事后推断。

### 3.1 版本标识按内容而非按检出形式

`core.autocrlf=true` 下，**同一个提交在不同工作树里检出的字节可能不同**：同一份 `catalog.json` 在一个工作树里是 LF、在另一个里是 CRLF，哈希因此不同。本步实测到两个工作树对同一提交报出**不同的规则版本**——而那正是这条验收要防的事。

修法是所有按内容算版本标识的地方先统一行尾：

| 位置 | 处理 |
| --- | --- |
| `crates/contracts/src/model.rs` | `catalog_fingerprint()` 先做行尾规范化 |
| `scripts/build-parallel-boundaries.cjs` | 同上 |
| `scripts/check-source-inventory.cjs` | **采集器代码摘要**同样处理（实测 `.ps1` 在工作树里是 CRLF） |
| `scripts/verify-workstream.ps1` | 按文本读取、规范化后再哈希，不再用 `Get-FileHash` |

规范化后，三个工作树对同一提交给出一致的 `c5f29bb3…`，且与既有指纹相同，因此不需要重新生成既有产物。

**未做**：仓库没有 `.gitattributes`，工作树之间的行尾差异仍然存在。它已不影响版本标识与检查结果，但会让 `git status` 在不同工作树里表现不一致。补齐 `.gitattributes`（`* text=auto eol=lf`）会重写大量文件的检出，属于独立改动，未在本步夹带。

## 4. 集成流程

1. **从共同基线开分支。** 工作线自己的分支从当时的共同基线创建。
2. **在本线范围内自由提交。** 共享入口的改动走 [共享改动登记](ownership.md)（见其第 4 节）。
3. **集成前先同步基线。** 把共同基线合入本线，在本线内解决冲突并重跑本线验证集。
4. **整合到 `main` 串联进行**，一次一条工作线。共享 Git 索引的目录不并发 commit、切分支或 reset——不同工作树各有自己的索引，这一点天然满足，但 `main` 只有一个。
5. **整合后跑整套门禁**：16 个检查脚本 + `cargo test --workspace`。单个工作线的验证集不是门禁。
6. **契约变更先整合、消费者后更新**，不用各线私有兼容分支长期维持差异。

## 5. 不进源码提交的东西

`.gitignore` 已覆盖：`target/`、`dist/`、`data/`、`.tools/`、`*.db`、`*.log`、`*.evtx`、`fixtures/scaled/`（可重建的生成语料）、`apps/desktop/src-tauri/gen/`。

**约定**：基准数据库、运行库副本、原始日志、临时产物一律不提交。生成物（`model/*.json`、`collection-*.json`、`parallel-boundaries.md`）**要提交**，它们有登记的生成器（见 `model/ownership.json` 的 `generated`）。

## 6. 已知边界

- **工作树隔离只解决文件与数据库冲突，不解决命名管道冲突**（见 2.1）。
- **`main` 的整合是串行的。** 这是刻意的：并行只在各线内部，公共基线的变动由整合责任人串行合并。
- **本文件的四张分支表是约定，不是强制。** 没有任何检查验证分支名称或工作树位置；工作树是否真的建立取决于执行者。可强制的是「每条工作线能独立跑完自己的验证集并给出契约版本」。
