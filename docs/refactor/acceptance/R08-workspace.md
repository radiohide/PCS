# R08 验收记录：固定工作区与集成策略

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | R08 · 待验收 |
| 实现范围 | 每条工作线独立分支与工作树；独立测试目录约定；每线验证集与结果追溯；集成流程；版本标识改为按内容计算；构建入口对输出重定向的健壮性修复 |
| 代码定位 | 新增 `scripts/verify-workstream.ps1`、`docs/refactor/workspace-conventions.md`；修改 `build.ps1`、`crates/contracts/src/model.rs`、`scripts/build-parallel-boundaries.cjs`、`scripts/check-source-inventory.cjs`、`model/ownership.json`，并重新生成 `model/sources.json`、`docs/refactor/parallel-boundaries.md` |
| 验证环境 | Git 2.55.0（PortableGit）；Node v24.20.0；Cargo / rustc 1.98.1；工作树 `SYS/pcscope-worktrees/{base-data,type-adapters}` |
| 验证过程 | 两条工作线**并发**运行各自验证集；三次注入测试（R07 沿用）；运行库写入核查 |
| 结果 | 两条工作线同时完成，各自通过，报出**相同的提交与规则版本**；16/16 检查通过 |
| 遗留问题 | 见第 6 节 |
| 下一步 | R09：最小贯通演练 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 基线/分支对应表 | `docs/refactor/workspace-conventions.md` 第 1 节 |
| 独立测试目录约定 | 同文件第 2 节 |
| 集成分支流程 | 同文件第 4 节 |
| 每线验证入口 | `scripts/verify-workstream.ps1` |

## 2. 并发验证（验收硬性条件）

两条工作线各自在自己的工作树里同时运行完整验证集（含编译）：

```text
base-data      C:\Users\radiohide\Desktop\SYS\pcscope-worktrees\base-data\pcscope  ws/base-data
               commit 649b322   catalog 1.1.0   rule c5f29bb3…
  ok   0.2s  scripts/check-model-catalog.cjs
  ok   0.2s  scripts/check-ownership.cjs
  ok  58.8s  test --offline --locked -p pcscope-contracts -p pcscope-storage

type-adapters  C:\Users\radiohide\Desktop\SYS\pcscope-worktrees\type-adapters\pcscope  ws/type-adapters
               commit 649b322   catalog 1.1.0   rule c5f29bb3…
  ok   0.1s  scripts/check-source-inventory.cjs
  ok   0.1s  scripts/check-contract-cases.cjs
  ok  11.6s  test --offline --locked -p pcscope-contracts

墙钟 60.2 s（串行需 70.4 s），两者都 Completed
```

| 验收条件 | 结果 |
| --- | --- |
| 无端口冲突 | 通过：不使用 TCP 端口；见第 3 节的命名管道约束 |
| 无文件冲突 | 通过：各自独立的 `target/` 与工作树，互不可见 |
| 无数据库冲突 | 通过：`:memory:` 或临时目录，文件名含进程号与时间戳 |
| 能定位每条结果所用契约版本 | 通过：每次运行打印提交、目录版本与规则版本，两条线的值一致 |
| 没有对当前运行库的写入 | 通过：见第 4 节 |

## 3. 唯一真实约束：每用户命名管道

守护进程使用每用户唯一的命名管道并以 `first_pipe_instance` 创建，第二个实例会被拒绝。因此：

- **启动真实守护进程的测试不能并发运行**，`test-daemon.ps1` 与 `stage-dev.ps1` 不在常规验证集内，它们是串行门禁；
- 这是**有意的设计**，不是缺陷：同一用户下两个守护进程同时写同一份历史会破坏一致性。

本机当前有一个**兄弟检出**的开发守护进程在运行（管道已占用，其库为 `SYS/pcscope/data/daemon-smoke/history.db`）。这不影响本仓库的并发验证，因为它与本仓库不共享任何库文件。

## 4. 未写入运行库的核查

| 检查 | 结果 |
| --- | --- |
| 是否有脚本引用兄弟检出的路径 | 无：全部脚本以 `$PSScriptRoot` 推导工程根 |
| 本仓库 `data/` 内容 | 仅 `scoped-fixture/` 两个测试样例文件，未变 |
| 测试写库位置 | 内存库或系统临时目录 |
| 运行中的库 | 由该守护进程自身持续写入；本仓库测试不引用该路径 |

## 5. 过程中修正的四个问题

**1. 同一提交在两个工作树报出不同的规则版本（最重要）。** `core.autocrlf=true` 下，同一份 `catalog.json` 在一个工作树里检出为 LF、在另一个里为 CRLF，哈希因此不同。实测两个工作树对提交 `d4efabb` 分别报出 `c5f29bb3…` 与 `60d81589…`。

这直接破坏本步「能定位每条结果所用契约版本」的验收：两个工作线会声称使用了不同的规则，而实际规则相同。更糟的是相反方向同样可能——不同规则被报成同一版本。

修法：所有按内容计算版本标识的位置先统一行尾（Rust 的 `catalog_fingerprint()`、类型边界生成器、采集器摘要、验证脚本）。规范化后三个工作树一致给出 `c5f29bb3…`，且与既有指纹相同，因此无需重新生成既有产物。

实测采集器脚本 `.ps1` 在工作树里**确实是 CRLF**，而 `catalog.json` 与 `.rs` 是 LF——所以 `collector_digest` 的值随之变化，这是修正后应有的结果。

**2. `build.ps1` 在调用方重定向输出时中断。** 它自身设置 `$ErrorActionPreference='Stop'`，而 cargo 把进度写到 stderr；PowerShell 5.1 会把每一行包装成终止性错误。结果是：直接运行正常，一旦在作业或日志捕获里运行就在第一行进度处中断。已在构建入口修复，而不是在每个调用方绕开。

**3. 工作树的工程根多一层。** 仓库根是 `SYS/SSS`，工作树是整个仓库的检出，因此工程根是 `<工作树>/pcscope`。我最初按 `<工作树>` 查找，报「脚本不存在」。已在约定文档中写明。

**4. 修订基线提交导致工作线分支分叉。** 我在工作树已创建之后修订了 `main`，两个 `ws/*` 分支随即分叉。分支上当时没有真实工作，重置是安全的；但这条已写进约定：**共同基线一旦被工作树取用，不得再改写其历史**。

## 6. 遗留问题

1. **没有 `.gitattributes`。** 工作树之间的行尾差异仍然存在。已不影响版本标识与检查结果（第 3.1 节），但会让 `git status` 在不同工作树里表现不一致。补齐会重写大量文件的检出，属独立改动。
2. **分支与工作树是约定，不是强制。** 没有检查验证分支名称或工作树位置；可强制的只是「每条工作线能独立跑完自己的验证集并给出契约版本」。
3. **只验证了两条工作线的并发。** `query-ui` 与 `migration` 的验证集已定义，但未做四线并发验证；四线并发的主要成本是编译资源的争用，不是正确性风险。
4. **`main` 的整合仍是串行且手工的。** 这是刻意的（第 4 节），但没有自动化检查阻止两人同时整合。
5. **命名管道冲突只有文档约定，没有检查。** 一个在并发环境里误跑 `test-daemon.ps1` 的人只会看到「同类采集正在执行」或守护进程启动失败，而不是一条指向本约定的提示。

## 7. 下一步

R09 的前置（R08）已满足。R09 需要一次最小贯通演练：用已有样例走通「证据输入 → 适配候选 → 身份结果 → 模拟或最小投影 → 查询响应 → 前端详情」，底层可有明确标记的替身，且不宣称已完成 S05–S08。
