# N01 验收记录：在新检出上锁定工作树并打通构建

日期：2026-09-21。依据：`docs/new-program-development-plan.md` §4 N01。

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | N01 · **已实施**（任务清单全部完成；两条验收条目实测满足）。冒烟不在本步通过条件内，按计划推迟到 N02，见第 8 节 |
| 实现范围 | 在一个**新的检出**上恢复缺失的 `plugins/`；修 `build.ps1` 的三处工具链定位缺陷并新增 `-Project`、扩充 `-Check`；打通工作区、examples、桌面工程、原生事件采集器与 staging；实测失败传播；产出产物与哈希 |
| 代码定位 | 修改 `build.ps1`（本步唯一改动的源码文件，6 处，见第 3.2 节与第 10 节的差异复现方法）；恢复 `plugins/**` 33 个文件（内容取自 `SSS-main` 里的 2026-09-12 01:37 GitHub 快照，**非本检出原有内容**）；更正 README 中一条不可复现的声明；追加 `docs/refactor/dev-log.md` 一节；新增本记录与 `N01-baseline.sha256`。**未改任何 Rust 源码、未改任何检查脚本的断言** |
| 验证环境 | Windows 11 26200；工作树 `C:\Users\radiohide\Documents\dev\pcscope`（由 `pcscope.7z`，2026-09-20 06:49，312,125,417 字节解出）；工具链取自 `<工作区>\SSS-main\SSS-main\pcscope\.tools`（见第 4 节） |
| 验证过程 | `build.ps1 -Check`；`cargo check --workspace --all-targets`；`build --workspace --examples` 后 `build --workspace`；桌面工作区 `-Project` 构建；`csc` 构建原生采集器；`stage-dev.ps1` 打包；`cargo test --workspace`；22 个 `check-*.cjs`；`clippy -D warnings`；三组失败传播负测试 |
| 结果 | 构建与测试全部通过：**348 项 Rust 测试通过 / 0 失败**，`dist/dev` 产出 17 个文件 + `domains/` 10 个文件且哈希已记录；**20/22 检查脚本直接通过**，其余 2 项受沙箱限制、其底层命令或受控替代均通过；失败传播三组负测试全部按预期失败且不产出假成功；**Clippy 未通过（14 条 lint，全在 `crates/contracts`，非本轮引入）** |
| 遗留问题 | 第 8 节：Clippy 14 条未修；冒烟推迟到 N02；本检出无 `.git`，所有权检查只能以受控替代运行；9/12 的演练副本不在本机，无法复核 |
| 下一步 | N02：新库与旧环境隔离（独立数据目录、单实例、IPC 端点、自身产物采集排除），之后才跑冒烟 |

## 1. 本步的起点：这份检出缺什么

工作从 2026-09-12 18:48 的工作树快照换到一个由 `pcscope.7z` 解出的新检出。它不是旧工作树的延续，因此 N01 的三件前提都要现场重建：

1. **缺 `plugins/`。** `Cargo.toml` 的 workspace 成员含 `plugins/{simulator,software,powershell-adapter,eventlog,usn}`，而 7z 里没有它们。`cargo metadata` 直接失败：

   ```text
   error: failed to load manifest for workspace member `...\pcscope\plugins/simulator`
     failed to read `...\pcscope\plugins\simulator\Cargo.toml`
     系统找不到指定的路径。 (os error 3)
   ```

2. **没有 `.git`。** 目录树里没有仓库，上一层也没有。9/12 记录里依据的「领先 origin/main 45 个提交」「74 笔提交」在本检出**无法核对**，也拿不到任何提交号作基线。
3. **树内没有工具链。** `build.ps1` 按设计拒绝回退 PATH，因此在新检出上第一次调用就报「Rust toolchain not found」。

## 2. 基线盘点与变更范围

| 项 | 值 |
| --- | --- |
| 未忽略文件（`git ls-files -co --exclude-standard pcscope`） | 299（清单：`N01-baseline.sha256`，301 行 = 2 行说明 + 299 条，该清单自身 sha256 `0ab11cec6a882bbb71029a6d05fbb6b681eb2ab4e4a7773c6646e6a64d7e753b`；清单按设计不含自身与本记录） |
| Rust 源码 | 69 个 `.rs`，24,096 行 |
| 前端脚本 | 15 个 `.js`，1,759 行 |
| 检查脚本 | 33 个 `.cjs`（其中 `check-*.cjs` 22 个），3,388 行 |
| PowerShell | 40 个 `.ps1`，1,883 行 |
| 验收记录 | `docs/refactor/acceptance/` 44 份（含本记录与基线清单） |

**本步的变更范围**（均可逐行核对）：

| 变更 | 文件 | 性质 |
| --- | --- | --- |
| 恢复 workspace 成员 | `plugins/**`（33 个文件） | 新增；内容来自 `SSS-main` 的 2026-09-12 01:37 快照（`SSS-main.zip`，commit a104c97），**不是**本检出 18:48 状态的对应版本——插件侧没有 18:48 的版本可用。是否编得过必须实跑，本轮实测为可编译、348 项测试全过 |
| 工具链定位与入口 | `build.ps1` | 修改，共 6 处，见 3.2 |
| 一条不可复现的声明 | `README.md`（「Strict Clippy checks all workspace targets」） | 更正，见 3.3 |
| 本轮经过与状态 | `docs/refactor/dev-log.md` | 追加一节「N01 在一个新的检出上把构建打通」 |
| 基线与证据 | `docs/refactor/acceptance/N01-baseline.sha256`、`docs/refactor/acceptance/N01-source-baseline.md` | 新增 |

`plugins/**` 文件从下载的 zip 复制而来，带 `Zone.Identifier`（Mark-of-the-Web），`plugins/eventlog-native/build.ps1` 因此报「未进行数字签名，无法运行」。已用 `Unblock-File` 清除全部 33 个文件的标记。**这一步不是可选的收尾**：不清标记，staging 会在调用原生采集器构建脚本时中断。

**没有做的事**：未提交任何内容（本检出无仓库，且计划明确不授权自动提交、推送或替换正在运行的程序）；未删除或改写任何既有记录；未改 Rust 源码与检查脚本断言。

## 3. 实施内容

### 3.1 恢复 `plugins/`

来源、内容与风险见第 2 节。恢复后 `cargo check --workspace --all-targets` 无错误无警告，`cargo test --workspace` 348 项通过——即 01:37 的插件源码与本检出 18:48 的 `crates/contracts` 接口**在本轮实测中兼容**。这是实测结论，不是保证：两者修订不同，后续改动插件时仍应以本检出契约为准。

### 3.2 `build.ps1` 的工具链定位（三处缺陷 + 两处扩充）

| # | 缺陷 | 证据 | 修法 |
| --- | --- | --- | --- |
| 1 | `Find-BinDirectory` 对**不含通配符**的模式恒不匹配：`Get-ChildItem -Path <dir> -Directory` 列的是该目录的**子项**，因此 `'mingw64\bin'` 这一项从未生效，是死代码 | 第一次 `cargo check` 时 linker bin 报的是 llvm-mingw；把 `'mingw64\bin'` 提到 llvm-mingw 之前仍报 llvm-mingw | 字面路径按目录本身判定（`Test-Path -PathType Container`），通配符路径仍走 `Get-ChildItem` |
| 2 | 优先选 LLVM-MinGW，而 Rust 的 `x86_64-pc-windows-gnu` std 链接 `libgcc`/`libgcc_eh`，LLVM-MinGW 用 compiler-rt、不带 libgcc | 一次 `cargo check` 在链接期失败 13 个 crate：`lld: error: unable to find library -lgcc_eh` / `-lgcc` | 把 GNU `mingw64\bin` 排在 llvm-mingw 之前 |
| 3 | 只把 bin 目录塞进 PATH，不指定 linker：rustc 按名字 `x86_64-w64-mingw32-gcc` 在 PATH 上解析，选到谁取决于顺序，静默且不可见 | 与 1、2 同源；缺陷 1 修好后才第一次选对 | 显式设置 `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER`；调用方已设置时不覆盖（与脚本既有的「显式请求不被悄悄替换」一致） |

两处扩充：

- **`-Project <dir>`**：桌面端是独立 cargo 工作区（`apps/desktop/src-tauri`），此前只能手工搭一份同样的环境去构建。现在两个工作区共用同一次工具链解析与 linker 选择；未传时行为不变（本脚本所在目录）。
- **`-Check` 增加 `target linker` 与 `libgcc.a` 两行**：正是因为缺这两条，缺陷 2 只能等到链接失败、并以 13 个 crate 同时报错的形式出现。现在预检会直接说明所选发行版是否带 libgcc。

### 3.3 README 更正

原文「Strict Clippy checks all workspace targets」在本检出**不成立**（第 5 节实测 14 条 lint），已按实测更正，并交叉引用 `R09-drill.md` §8——那份记录在 rustc 1.96.0 上就已把它标为不可复现，本次在 rustc 1.98.1 上复现为 14 条，说明这不是工具链版本差异。同时把「桌面工程要手工搭环境」改为 `-Project` 用法，并把工具链为外部依赖、`-Check` 会报告 libgcc 写进构建小节。

## 4. 验证环境

工具链不在树内（`.tools/` 被 `.gitignore` 忽略）；本机可用的那一套位于 `SSS-main\SSS-main\pcscope\.tools`，与旧记录（R08/S02/S04）所称的固定工具链一致。**R09 §8 的「该固定工具链不存在」是在当时那台机器/会话上的结论；本机存在，本轮就是用它跑的。**

```text
project                C:\Users\radiohide\Documents\dev\pcscope
tools root             C:\Users\radiohide\Documents\dev\SSS-main\SSS-main\pcscope\.tools
cargo                  cargo 1.98.1 (797e8a9bc 2026-08-05)
rustc                  rustc 1.98.1 (48a229cea 2026-09-01)
host target            x86_64-pc-windows-gnu
linker bin             ...\.tools\mingw64\bin
target linker          ...\.tools\mingw64\bin\x86_64-w64-mingw32-gcc.exe
libgcc.a               ...\.tools\mingw64\lib\gcc\x86_64-w64-mingw32\16.2.0\libgcc.a
csc (native reader)    C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe
node / powershell      v24.21.0 / 7.6.6
git                    C:\Program Files\Git\cmd\git.exe
```

命令（`$T` = 上面的 tools root）：

```powershell
.\build.ps1 -Check -ToolsRoot $T
.\build.ps1 -ToolsRoot $T -CargoArgs @('check','--workspace','--all-targets','--offline')
.\build.ps1 -ToolsRoot $T -CargoArgs @('build','--workspace','--examples')
.\build.ps1 -ToolsRoot $T -CargoArgs @('build','--workspace')
.\build.ps1 -ToolsRoot $T -Project apps/desktop/src-tauri -CargoArgs @('build')
powershell -NoProfile -File plugins/eventlog-native/build.ps1
powershell -NoProfile -File scripts/stage-dev.ps1
.\build.ps1 -ToolsRoot $T -CargoArgs @('test','--workspace','--offline')
node scripts/check-*.cjs
```

**一条实测到的顺序要求**：只跑 `build --workspace --examples` 之后，`target/debug` 下**没有**工作区二进制（实测：只有 `pcscope-eventlog-native.exe`，示例在 `target/debug/examples/`）；再跑 `build --workspace` 才把 `pcscope-cli/service/software/powershell-adapter/eventlog/usn/simulator.exe` 落到位。README 本来就把两条都写了，本条记录说明第二条不是可选的。

## 5. 实测结果

| 项 | 命令 | 结果 |
| --- | --- | --- |
| 工作区编译 | `cargo check --workspace --all-targets` | 通过，0 错误 0 警告 |
| 工作区构建 | `build --workspace --examples` → `build --workspace` | 通过；7 个工作区二进制 + 17 个示例落在 `target/debug` 与 `target/debug/examples` |
| 工作区测试 | `cargo test --workspace`（经 `build.ps1`） | **348 通过 / 0 失败**（17 个测试二进制），与 9/12 记录的数字一致 |
| 桌面工作区 | `-Project apps/desktop/src-tauri -CargoArgs @('build')` | 通过，4 分 18 秒；产出 `pcscope-desktop.exe` 220.76 MB 与 `WebView2Loader.dll`。一条非致命链接警告：`multiple non-default manifests`（MinGW 资源合并） |
| 原生事件采集器 | `plugins/eventlog-native/build.ps1`（`csc`） | 通过；`pcscope-eventlog-native.exe` 22,528 字节 |
| 打包 | `scripts/stage-dev.ps1` | 通过；`dist/dev` 产出 17 个文件 + `domains/` 10 个文件，与 9/18 参考包逐项一致 |
| 检查脚本 | 22 × `node scripts/check-*.cjs` | **20 直接通过**；`check-source-type-map.cjs` 与 `check-ownership.cjs` 受沙箱限制，见第 7 节，其验证内容均通过 |
| 静态检查 | `cargo clippy --workspace --all-targets -- -D warnings` | **未通过：14 条 lint**，全部在 `crates/contracts`（`adapters/{network,virtualization,security,event,desktop}.rs`、`semantics.rs`、`sensitive.rs`、`query.rs`），无一条来自本轮恢复的插件代码；本轮不修，理由见第 8 节 |

关键细节：

- `WebView2Loader.dll` **只有在桌面构建真正跑通后**才会出现在 `apps/desktop/src-tauri/target/debug/`。本检出 9/12 的旧 `target/` 里没有它，`stage-dev.ps1` 第 10 行会因此失败——所以「桌面工程没构建」在这里不是靠时间戳判断的，是会直接中断打包的硬依赖。
- 22 个检查脚本里 `check-model-catalog.cjs` 与 `check-source-inventory.cjs` 会**重写生成物**。运行前后对 `model/ownership.json` 声明的 9 个生成物取 SHA-256 对比：**零漂移**，即本检出的生成物与生成器一致。

## 6. 产物与哈希

`dist/dev`（17 个文件 + `domains/` 10 个文件，与 9/18 参考包逐项一致）。哈希为 SHA-256（完整值）：

| 文件 | 字节 | SHA-256 |
| --- | --- | --- |
| `pcscope-desktop.exe` | 231,485,863 | `14db27bf7ea5f2a8c434ae1227b1300c0f289e99e657ff97eb70c78d237c755a` |
| `pcscope-service.exe` | 65,173,990 | `172226c3acedc7691ddf0afee8246625c6bcd71e6f10a64e837037653b40f488` |
| `pcscope-cli.exe` | 39,693,642 | `9060cbe826e2b7be934bf11059cf950976dd3a57eed1494f9de81176797d068c` |
| `pcscope-probe.exe` | 31,253,603 | `4e5764fc71952716d00173e5908f1b3a631fb9f1dcd4773359be5f4a7afb1b0c` |
| `pcscope-software.exe` | 20,586,129 | `82dd46103b7e74a921114e7364c4b75634c4d5f62446e1432579bfa960b02316` |
| `pcscope-powershell-adapter.exe` | 20,403,040 | `07600b2b3e41998f40e71a2d724c2996dc746632d970f55d00e175ae895f3a4d` |
| `pcscope-usn.exe` | 20,374,371 | `4c3ca5676c28068e2ea71823adf800a0dbafb42ba8784ac899f055624700aafd` |
| `pcscope-eventlog.exe` | 20,356,547 | `40241a422d92b76324ff86037e45dc65825c410b01c2e4d5ed3dfd47c4851ac0` |
| `pcscope-eventlog-native.exe` | 22,528 | `21bb7dd056c12665253400f4a18f4a809944b1a5ff79e833cbdafae82769e02c` |
| `WebView2Loader.dll` | 160,320 | `8427b1fc58ec707813e5c0a51eb5d69397bb333250a7b891be4d3b123f1e0f1c` |

脚本与配置：`collect-system.ps1` `57e4e5c3…`、`collect-domain.ps1` `f7c68336…`、`evidence.ps1` `41b9a94a…`、`collect-eventlog.ps1` `dac25dc8…`、`event-schema.ps1` `e986f7d4…`、`channels.json` `f7ea3ac8…`、`collector-options.json` `50ee551f…`、`domains/` 10 个文件（`applications/desktop/network/scoped/security/startup/storage/system/virtualization.ps1`、`IdentityNative.cs`）。

这些哈希对应的是**本轮源码状态的产物**，不是「文件时间戳新」——计划明确要求不得用时间戳或输出字符串作为产物溯源证明。产物与源码状态的对应关系由第 2 节的变更范围与 `N01-baseline.sha256` 一起固定。

## 7. 失败传播与两条受沙箱限制的检查

**负测试（三组，全部实测，不是读代码得出的）：**

| 场景 | 操作 | 结果 |
| --- | --- | --- |
| 打包缺产物 | 移走 `target/debug/examples/probe.exe` 后跑 `stage-dev.ps1` | 退出码 1；**不打印** `Development bundle staged`；报「找不到路径 …probe.exe」；不会拿上一次的旧文件充当成功。移回后重跑成功 |
| 构建失败 | `build.ps1 -CargoArgs @('build','-p','no-such-crate')` | 抛出 `Cargo failed: 101`，退出码 1 |
| 工具链请求被拒 | `-ToolsRoot`／`PCSCOPE_TOOLS` 指向不含 `cargo\bin\cargo.exe` 的目录 | 打印「显式指定的工具链不会被悄悄替换」并退出 2；完全不指定时打印全部搜索过的路径——**都不回退 PATH** |

**两条检查受沙箱限制。** 本沙箱不允许 node 捕获子进程的管道输出（`spawnSync ... EPERM`，实测一个最小复现同样被拒），而这两个脚本正是这样调用子进程的：

- `check-source-type-map.cjs` 只是包了一层 `execFileSync(process.execPath, ['build-source-type-map.cjs','--check'])`。把它底层那条命令**直接用 PowerShell 调用**即通过：`Source type map verified: 128 sources, 7 with no handling path`。
- `check-ownership.cjs` 用 `git ls-files -co --exclude-standard pcscope` 枚举文件，且仓库根在工程目录的**上一层**（原布局 `SYS/SSS/pcscope`）。本检出两层都没有仓库，因此它**无法按原样运行**。采用受控替代：临时在上一层 `git init`（**不提交任何东西**）、用 PowerShell 跑同一条 `git ls-files` 拿到清单、用一个垫片（`ownership-shim.cjs`，只替换「文件枚举」这一步）调用仓库自己的检查，跑完立即删除 `.git`（已确认删除）。断言、消息、退出码均为仓库原文，结果为：**299 个文件全部有归属、0 个无归属**；加入本记录与基线清单后在最终树上重跑（**301 个文件**）同样全部有归属、0 个无归属。`pcscope/plugins/**` 在所有权模型中有声明，因此本轮恢复的插件文件没有引入未归属文件。

这两条都标注为**环境限制下的替代验证**，不当作「原样通过」记入。

## 8. 未做与边界

1. **冒烟（`test-daemon.ps1`）未跑。** 计划 N01 的验收提到「冒烟与打包必须停止」（该条以负测试满足），同时计划第 5 节写着「在 N02 完成前不可盲跑会使用固定数据目录/IPC 的冒烟脚本」。本机守护进程用每用户唯一命名管道、数据目录尚未隔离，因此留到 N02，不作为本步通过条件。
2. **Clippy 14 条未修。** 不在 N01 范围；都在 `crates/contracts` 的既有代码里，改动它们要连带重跑测试与相关验收记录，属独立改动。本记录只如实报告并更正 README 的相反声明。
3. **本检出没有仓库。** 因此拿不到提交号作基线，N01 要求记录的「基线版本」以 `N01-baseline.sha256`（299 条 SHA-256）代替，变更范围以第 2 节的表固定。是否 `git init` 并建立正式仓库，需要用户决定——计划不授权自动提交。
4. **9/12 的演练副本不在这台机器上**（`data/s32-drill`、`data/s32-rehearsal`、`data/daemon-s31` 均不存在），本记录不对它们的现状作任何声明。
5. **插件修订混用。** `plugins/**` 是 01:37 快照，本检出其余是 18:48 状态。本轮实测兼容，但这不是保证；后续触碰插件时应重新核对契约。

## 9. 与计划 N01 条目的对照

| 计划要求 | 本步结果 |
| --- | --- |
| 盘点暂存/未暂存/新增文件，记录基线版本与变更范围，不得 reset 或覆盖既有修改 | 第 2 节：299 条哈希清单 + 三处变更范围；未 reset、未覆盖、未提交 |
| 为当前平台记录 Rust/MSVC、Node、PowerShell、WebView2 和原生事件采集器依赖 | 第 4 节：cargo/rustc 1.98.1 gnu、MinGW GCC 16.2.0（含 libgcc）、node v24.21.0、PowerShell 7.6.6、`csc` 路径；WebView2Loader.dll 由桌面构建产出（第 5 节） |
| 解决 `build.ps1` 工具链定位：明确支持的安装布局，不得偷偷回退 | 第 3.2 节：三处缺陷修复；布局为「含 `cargo\bin\cargo.exe` 的目录」，GNU `mingw64\bin`/`mingw\*\bin` 优先、需带 libgcc；被拒或找不到时明确失败，不回退 PATH |
| 审查 PowerShell 构建与 staging 的失败传播：每次原生命令失败都须终止 | 第 7 节三组负测试；`plugins/eventlog-native/build.ps1` 自身在 `csc` 非零时 `throw` |
| 打通工作区、examples、桌面工程和原生事件采集器的构建与打包；记录产物路径、哈希、版本与源码状态 | 第 5、6 节：全部打通；产物路径 + SHA-256 + 工具链版本 + 源码基线清单 |

验收条目：**「从明确输入构建所有必需产物」**——第 4 节的命令列表与第 6 节的全部产物；**「故意使一次构建失败时，冒烟与打包必须停止」**——第 7 节的打包负测试（冒烟见第 8 节第 1 项）。

## 10. 证据位置与可复跑性

- 本记录、`N01-baseline.sha256`：仓库内，可提交。
- `build.ps1` 与归档原文的逐行差异：从 `pcscope.7z` 取出原始 `build.ps1` 后 `Compare-Object` 即可复现，改动共 6 处，全部在本记录第 3.2、3.3 节列出。
- 运行日志（`build1.log`、`test-final.log`、`clippy.log`、22 份检查日志、`checks-final-result.json`、`ls-files.txt`、`generated-before.json`、`ownership-shim.cjs`）：在工作区**之外**的临时目录 `.n01-tmp\` 下，**不属于仓库**，也不作为交付物。本记录把结论所需的数字与命令都写在正文里，日志只用于复核。
- 可复跑的核心命令见第 4 节；`-Check` 的完整输出即第 4 节的表格。
