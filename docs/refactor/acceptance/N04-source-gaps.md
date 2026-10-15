# N04 来源缺口补齐记录

日期：2026-10-12。工作树：`C:\Users\radiohide\Documents\dev\pcscope`。

按用户“先把来源缺口补齐”的要求，先实施 N04 的来源缺口子任务。N01 已完成；N02 新库/实例隔离、N03 全局完整度语义及 N04 其余采集器字段审查不由本记录宣称完成。

## 1. 来源处理矩阵

| 来源/类型 | 实现与业务去向 | 验证与边界 |
| --- | --- | --- |
| `apps.dotnet-runtimes` / `software.runtime` | 展开 sharedfx/SDK 版本登记、hostfxr/sharedhost 以及 Framework 的 Install/InstallSuccess；家族、版本和注册表视图区分身份 | 实机 24 条原始登记；登记容器不制造运行时对象；未提供的安装路径不猜测 |
| `system.update-history` | 明确记录为更新操作动作，保存发生时间、安装/卸载阶段、结果与错误码；原始属性及读取错误保留 | 最终实机 128 条历史全部可分页查询；重复观察去重；相同时间和内容的不同记录分别保留；历史操作不制造“已安装组件” |
| `collector.scoped-configuration` | 原始配置进入覆盖读数查询，包括文件、目录、便携程序、模块和可选运行时范围 | 明确非对象；原始未知字段和嵌套值保留，不进入解析失败队列 |
| `collector.event-channel-coverage` | 通道元数据加实际读取的最早/最新现存端点；查询结合数据库已提交断点、各日志代际和已知缺口 | 实机 23 个通道的可读/不可读结果；断点及缺口各显示最多 32 项，并提供总数和更多标记；不将日志末尾等同完整历史 |
| `usn.watch-list` | 空和非空名单均产生范围读数；空名单明确未配置 | 不产生对象删除或虚假完整历史；实际卷日志可读性由逐盘来源报告 |
| `execution.resources` | CPU/内存采样原文保存在证据路径；新增稳定分页查询和可读显示 | 实机 22 条采样；有采样时间、单位、原始嵌套字段；不制造持久对象 |
| `usn.changes` | 保留为按盘来源的虚拟汇总前缀，实际来源沿用统一事件适配器 | 是唯一剩余来源例外，原因是不存在独立采集端；界面明确虚拟分组，不显示“尚未接入” |
| `files.directory-security` / `file.watched_directory` | 新增独立目录名单、版本与 IPC 配置；目录身份、属性、Owner/DACL、直接子项及变化进入对象事实 | 目录默认空；不递归、不读取正文；名单变化拒收旧范围在途结果；缺失、错误路径和重解析目录有明确结果 |
| `security.key-metadata` / `security.key_metadata` | CNG 提供程序按用户/机器范围只读枚举；名称、算法、LegacyKeySpec、Flags 保留 | 本机 4 个提供程序、4 个范围可读、4 个范围不可读，取得 1 个密钥元数据对象；不打开、导出、创建或删除密钥；不涵盖独立的传统 CAPI 全清单 |
| `hardware.driver-packages` / `hardware.driver_package` | DISM `Get-WindowsDriver -Online -All` 枚举驱动仓库；以发布 INF 定义身份，保留全部返回属性和类型字段 | 夹具验证入库和查询；当前用户实机返回需提权，管理员环境正向验收仍待完成；未知签名不伪装为已签名 |

此外，恢复后的 PowerShell 文件采集器缺失文件身份字段；本轮补上文件/目录的只读原生身份，验证重命名前后身份一致。日志通道清单同时支持源码目录和 staging 目录，修复开发目录下找不到 `channels.json` 的问题。

## 2. 存储与界面

- 来源适配入口：`crates/contracts/src/adapters/source_gaps.rs`，目录适配在 `adapters/file.rs`。
- 更新操作新增可选 `occurrence_key` 和 `source_record`，与字段 before/after 分离。数据库 schema 9 增加对应列和原生操作身份唯一索引；所有数据库验证使用隔离夹具或内存库。
- `source_readings` 查询复用原始证据，不另复制采样/配置载荷。按保留证据序号固定分页水位，拒绝跨来源游标。
- 采集健康页增加“资源采样与采集范围”，可按四类来源筛选、翻页、展开完整原始字段；日志范围附当前已提交断点及已知缺口。
- 指定范围插件增加独立目录名单编辑器；文件名单与 USN 规则保留原有边界。
- 更新动作时间线显示标题、结果、更新 ID、修订、错误码和完整操作记录。敏感数据继续由服务统一呈现规则处理。
- 来源读数不会计为对象，也不会因为缺对象适配器而成为永久解析失败；配置/采样读数不宣称完整日志历史。

## 3. 实测证据

| 验证 | 结果 | 日志/入口 |
| --- | --- | --- |
| 全工作区回归 | 356 通过、0 失败（补采进度专项新增前的全量运行） | `target/n04-tests.log` |
| 最终相关回归 | contracts 245、storage 查询 17、projection 9 通过；更新结果码专项 6 通过 | `n04-focused-tests.log`、`n04-query-final.log`、`n04-final-source-tests.log` |
| 全部检查 | 22 个原样通过；所有权检查使用文件枚举替代，断言未修改 | `target/n04-checks.json`；见下段 |
| 原生目录夹具 | 元数据、原生身份、重命名、子项变化、不递归、明确缺失及错误类型通过 | `scripts/test-source-gaps.ps1`；`target/n04-native-complete.log` |
| 实机来源 | .NET 24 登记、更新历史 128、资源采样 22、日志范围 23；CNG 4 个范围不可读；驱动包读取需提权 | `scripts/test-live-source-gaps.ps1`；`target/n04-live-final-inputs.log` |
| 原始材料到业务查询 | 152 个协议消息进入全新内存库：17 个对象、128 个更新动作、22 个采样；解析失败 0；重复输入新增工作 0 | `crates/storage/examples/source-gap-audit.rs`；`target/n04-source-audit-final.log` |
| 构建与开发包 | 工作区、examples、桌面及原生事件读取器成功；17 个顶层文件和 domains 辅助文件 | `dist/n04-source-gaps`；`N04-artifacts.sha256` |
| Strict Clippy | 未通过，仍为 N01 已记录的 contracts 14 条诊断；新增来源模块无诊断 | `target/n04-clippy.log` |

本检出没有 Git 仓库。所有权检查沿用 N01 的垫片，仅用 `rg --files --hidden --no-require-git` 替代 Git 文件枚举；遵守当前 `.gitignore`，运行仓库原有断言。没有创建仓库、提交或改写断言。

最后一次实机材料：`target/n04-live-sources/bed8d2863d9d425c962c52b8fb79e2f3`；目录夹具：`target/n04-source-fixtures/23ba264c01a54da9b805b20ed768664f`。这些是被忽略的本机验证材料，不作为可分享的源码文件。详细失败原因保存在原始缺口消息中，汇总不输出密钥名称。

工具链仍取 N01 指定的 `SSS-main/SSS-main/pcscope/.tools`。编译器在系统短路径临时目录曾遇到访问失败，验证构建统一设置本进程 TEMP/TMP 为项目 `target/n04-temp`。桌面链接保留原有非致命 `multiple non-default manifests` 警告。

## 4. 复跑

在项目根运行（`$taskTools` 指向上述工具链）：

```powershell
$env:TEMP=Join-Path $PWD 'target/n04-temp'
$env:TMP=$env:TEMP
.\build.ps1 -ToolsRoot $taskTools -CargoArgs @('test','--workspace')
.\build.ps1 -ToolsRoot $taskTools -CargoArgs @('build','--workspace','--examples')
.\build.ps1 -ToolsRoot $taskTools -CargoArgs @('build','--workspace')
.\build.ps1 -ToolsRoot $taskTools -Project apps/desktop/src-tauri -CargoArgs @('build')
.\scripts\test-live-source-gaps.ps1
# 目录原生夹具用 Windows PowerShell 5.1 运行。
.\scripts\stage-dev.ps1 -Destination (Join-Path $PWD 'dist/n04-source-gaps')
```

`source-gap-audit.exe` 接收以上采集脚本输出目录中的 `.jsonl` 路径；使用内存库，不发现后台、不连接正式 IPC。开发包已生成，未启动它或替换正在运行的后台。正式新程序运行验收继续依赖 N02 的数据/端点隔离。

## 5. 完成判定与剩余工作

来源清单中 131 个唯一来源均有处理说明；101/101 个目录类型有来源路径；来源例外由 7 个减为 1，类型例外由 4 个减为 0。保留的 1 个例外仅为虚拟 USN 前缀，实现性例外为 0。它们表示代码路径已接通，不表示当前机器所有来源均可读。

驱动包的管理员正向实机验收、受限 CNG 范围的适用环境验收仍待完成；未用空列表代替成功。目录采用直接子项快照，递归监控、文件正文版本、非 NTFS 实时通知不由此实现。N02/N03 及完整桌面实机操作验收继续保留，不将本轮检查结果作为它们的完成证明。

原始 N01、S21 等历史记录保持原文。本轮源码状态由 `N04-source-baseline.sha256` 固定，开发产物由 `N04-artifacts.sha256` 固定。

## 6. 来源语义依据

- [.NET Framework 安装登记](https://learn.microsoft.com/en-us/dotnet/framework/install/how-to-determine-which-versions-are-installed)。
- [Windows 更新历史属性](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/aa386472%28v%3Dvs.85%29)与[结果码](https://learn.microsoft.com/zh-cn/openspecs/windows_protocols/ms-uamg/a0eb1e12-0a6a-47c9-a70f-f272d87b5227)。
- [CNG 密钥名称枚举](https://learn.microsoft.com/en-us/windows/win32/api/ncrypt/nf-ncrypt-ncryptenumkeys)与[提供程序枚举](https://learn.microsoft.com/en-us/windows/win32/api/ncrypt/nf-ncrypt-ncryptenumstorageproviders)。
- [DISM 驱动仓库查询](https://learn.microsoft.com/en-us/powershell/module/dism/get-windowsdriver)。
