# N04 采集完整性、性能与适用环境评估

> 后续证据见 [N04 原生、权限与循环防护补充验收](N04-native-safety.md)：管理员只读驱动验收已通过，新开发包默认使用原生日志。本文件以下数字和“默认 PS / 尚未管理员测试”等表述对应此前旧包的历史测试，不作为新版状态。

评估日期：2026-10-12。基线：根目录 `pcscope`，N04 来源补齐后的源码、schema 9，以及 `dist/n04-source-gaps` 开发包。此记录追加新证据，不替代此前 N01/N04 或 S30 的历史验收结论。

## 1. 结论

当前功能适合 **Windows 本机的周期状态盘点、已有事件日志的增量读取，以及明确指定文件/目录的元数据观察**。本次采到的原始材料在隔离数据库中全部保存且可回读；不能据此宣称覆盖 Windows 全部对象、所有用户、所有活动或完整历史。

当前机器实际产生 125 个具体来源名称：83 个取得非空数据、17 个成功但范围内为空、24 个不可用、1 个部分可用。共 28 条明确 gap，其中 CNG 单个来源有 4 个提供程序/位置缺口。不可用主要来自当前普通用户的读取权限、未安装 Hyper-V/Sysmon 等组件。成功且为空也不代表该类活动从未发生。

**部署可运行性尚未通过。** 当前包的 PowerShell 包装器按 `RemoteSigned` 启动时，实测返回未数字签名错误，退出码 1；直接 PS5 按同样策略启动也失败。以下成功采集数字来自仅对测试进程设置 `Bypass` 的诊断运行，未修改系统执行策略。正式部署需要解决脚本信任/签名和运行策略兼容，再使用真实包装器验收，不能把诊断结果当成当前包默认可运行的证明。

性能方面，本机单轮状态采集总耗时约 43 秒；实际材料入库 30.87 秒，投影 3.03 秒。常用对象查询在本次数据规模下低于 50 ms 的热查询 p95。固定 Application 历史的原生后端比 PS 后端快约 3.57 倍，但默认后端仍是 PS。采集轮换有明显吞吐上限，历史保存也会在对象不变时继续增长，因此不适合宣称高吞吐全量审计或无限期低容量保存。

## 2. 实测环境及方法

| 项目 | 当前证据 |
| --- | --- |
| 系统 | Windows 10 企业版 LTSC，10.0.19044，x64 |
| CPU / 内存 | i9-13900H，20 个逻辑处理器，约 31.68 GiB RAM |
| 文件系统 | 本地固定盘 NTFS，约 472 GiB 可用空间；未记录磁盘型号，不将磁盘性能外推 |
| 读取身份 | 当前用户，管理员 token=false；未自动提权 |
| 采集运行时 | Windows PowerShell 5.1.19041.7725；测试调度器 PowerShell 7.6.6 |
| Rust 测量产物 | rustc/cargo 1.98.1 GNU；storage 示例使用 release；两个原生 Rust 采集器来自 debug 开发包 |
| 配置 | 默认 collector-options；仅在隔离夹具中指定 1 个普通文件、1 个目录。Docker/Mihomo/模块/便携目录配置未开启 |
| 数据隔离 | 独立进程读取源，隔离新建 SQLite 文件；未启动后台、未打开正式数据库、未修改日志/审计策略或 USN journal |

先按实际执行策略尝试，再诊断采集能力。执行 11 个状态执行单元、USN，以及 23 个日志通道的首批读取；Application 额外验证续读。基线、指定范围和 Application 首批另复测 3 次。计时包含直接采集进程的启动、读取、序列化、加密和输出，不包含 Rust 包装器、服务排队、IPC 和 UI。进程内存按 50 ms 轮询；两个短于采样周期的原生 Rust 进程没有有效内存样本，不能解释成零内存。

测试调度器移除给 PS5 子进程继承的 PS7 `PSModulePath`/`PSExecutionPolicyPreference`，让 PS5 发现自己的模块。早期未清理环境的测试中出现目录 `Get-Acl` 模块加载失败，清理后目录采集成功；该早期缺口不计入最终产品缺口。SQLite 测量使用工作区内可写 TEMP/TMP；早期临时目录访问错误的结果不计入有效性能数据。

本次没有执行同一系统的管理员正向测试，也没有 Windows 11、Server、ARM64、低配置机器或其他文件系统的实机证据。

## 3. 完整性验证结果

| 验证层 | 本次结果 | 能证明的范围 |
| --- | --- | --- |
| 输出协议 | 1,185 个完整快照、20 个事件批次、28 个 gap；解析/超时/超长行/重复快照 ID/缺失分片组均为 0 | 已输出的报文符合协议；未采到的范围仍然是缺口 |
| 实际原始材料 | 12,230 条快照 item + 1,419 条 event = 13,649 条证据 | 包括空分片在内的报文统计与对象数量不同 |
| 入库回读 | 13,649/13,649 原始载荷逐项 JSON 值相等；入库拒绝、载荷差异、额外载荷均为 0 | 证明这份真实样本从输出到存储不丢字段，不证明所有 Windows API 属性已被采集 |
| 投影 | 13,649 项处理完；unadapted=0、rejected=0，失败队列为空；形成 7,341 个对象、29,131 个事实、537 个动作 | 合并身份、元数据与未注册业务事件不会一项一对象/一项一动作；原文仍保留 |
| 实际 XML | 1,419 条全部可用同一用户 DPAPI 解密；解密后 SHA256 与采集时原 XML 哈希相符 | 未用裁剪后的字段替代原 XML |
| 更新 / .NET / 采样 | 更新历史 128 条，属性 getter 错误 0；.NET 原始登记 24 条；资源采样 22 条 | 更新是历史操作，.NET 是规定注册表范围，采样不是持久对象 |
| 进程字段 | 247 个进程中 100 个取得路径和命令行，147 个未返回 | 原始值缺失是实测覆盖限制；不能宣传普通用户取得所有进程命令行 |
| Application 续读 | 同一 epoch，batch_seq 1→2；ID 1–128→129–256，无交叠 | 验证本次正常连续读取，不代替重置、重启与覆盖压力验收 |
| 同报文重放 | 无新增投影任务；历史首批 Application 在已有第二批后重放，1 次被顺序校验拒绝 | 这是旧 batch_seq 的预期拒绝；不能宣称任意旧批次都可重新接受 |
| 数据库一致性 | 两个有效隔离数据库 `PRAGMA integrity_check=ok` | 该次正常结束；未证明断电/崩溃恢复 |

日志的 41 项 field_issues 是加密、预览等投影限制说明，不能当成 41 条事件丢失。完整 XML 单独验证了可恢复性。API 枚举返回值也并非统一全属性：注册表在声明路径/深度内保留原始值，部分 CIM 来源有 source_properties；非 CIM 对象与基础盘点仍包含选定字段，不能统一标记“所有 API 字段齐全”。

### 固定历史全量验证

只读导出 Application 为 2,166,784 字节 EVTX，在不变化的文件上分别连续读取到 `has_more=false`：两端都是 29 批、3,638 条，无重复或缺失 ID；分类、System、原 XML 哈希及解密后完整字段值均一致。共 7,276 份 XML 解密与哈希验证全部通过。

25 条事件存在字段表示差异：原生后端更保守的加密，以及长字段的预览包装不同。比较时解密并取 full_value，不能直接比较随机密文或仅比较 preview；完整源值没有差异。此次全量结论只覆盖这个固定 EVTX，不覆盖所有通道和所有事件类型。

### 日志范围与当前缺口

23 个通道中 19 个可读取、4 个不可读：Security 和 GroupPolicy 当前无读取权限，Sysmon 和 Hyper-V VMMS 通道未安装。TaskScheduler、PrintService、DriverFrameworks-UserMode 三个通道当前禁用，读取为空不能证明任务/打印/设备活动已记录。AppLocker 等启用但为空的通道，也没有审计策略正向触发验收。

19 个可读通道都是 Circular 模式。AppX 最旧保留 ID=1086、CodeIntegrity=2185、PowerShell=31678、WMI=1376：源目前只保留一个窗口，窗口之前的历史并未在本次取得。PowerShell 在观测时只保留约 94 秒的窗口；本次测试本身也会产生 PowerShell/WMI 活动，不能据此推算正常业务日流量。

## 4. 功能适用范围及环境要求

| 功能 | 适用范围与必要条件 | 明确不涵盖 / 本次未验收 |
| --- | --- | --- |
| 系统、账号、启动、硬件、网络、桌面盘点 | Windows 本机、64 位 PS5、相关 CIM/系统模块可用；普通用户取得其有权读取的状态 | 所有机器/其他未加载用户配置；短时活动；全部 API 属性 |
| 机器软件 / 当前用户应用 | HKLM 两个注册表视图的 Uninstall、当前用户登记/AppX、标准应用目录 | 全部安装机制、所有用户 AppX、未登记程序；portable 自定义目录须单独配置 |
| .NET / npm / IDE / 浏览器扩展 | 声明的注册表深度、标准 npm/VS Code 目录、当前用户的受支持浏览器 profile | 自定义前缀、ZIP/便携运行时、任意 profile 目录；磁盘安装清单不是当前激活证明 |
| 更新历史 / QFE | 本机 Windows Update COM 保留历史，以及 QFE inventory | 完整更新生命周期、已经被系统清理的历史；不能把一次成功更新操作当成当前安装状态 |
| 驱动仓库、预配应用、可选组件 | DISM Online、本机管理读取权限；当前普通用户三者均返回需提权 | 管理员实机正向结果未测；仓库存在不证明驱动已加载或签名可信 |
| 安全状态、证书、CNG 元数据 | Defender/TPM/BitLocker 等组件存在；每个来源有独立读取授权；证书仅指定 6 个 store，CNG 按提供程序/user/machine 范围只读枚举 | 私钥、凭据、恢复密钥、证书信任判定、全部证书 store、传统 CAPI 全清单；CNG 有 4 个范围不可用 |
| 事件日志 | 23 个清单通道；读取权限、对应组件、日志启用及审计策略须同时满足；断点增量读取已有记录 | 安装前/被清理/已覆盖的历史；任意其他通道；管理员权限本身不能补回未写入的事件 |
| 固定 EVTX 导入 | 普通本地 `.evtx`、读取期间不变、受支持通道类型；本次验证 Application | UNC、重解析文件、变化中的归档；其他事件类型完整性未全量验收 |
| 指定文件 | 明确的本地文件名单，元数据/Owner/DACL/ADS 描述，原生卷与文件身份；hash 默认关闭、大小受限 | 正文/内容版本、全盘观察、网络路径、重解析目标内容、SACL；文件不存在不是已证实删除 |
| 指定目录 | 明确本地普通目录名单，目录元数据与直接子项；名单上限 100，默认条目限制 20,000 | 递归整棵目录、文件正文或重解析目标；名单为空时不监控 |
| USN 文件变化 | 有盘符的固定 NTFS 卷、已有 journal、明确文件名单，并具备 journal 读取授权 | 本机当前拒绝访问；非 NTFS、网络盘/移动盘、无盘符卷、逐次写入和内容版本；历史父目录重命名可能影响归属 |
| Hyper-V / WSL / Docker / Mihomo | 组件存在且本地 API 可读；Docker/Mihomo 为显式开关，Docker 使用固定本地 Desktop CLI/管道 | 当前无 Hyper-V 模块/Sysmon；Docker/Mihomo/portable/module 真实运行范围本次未开启验收；非本地 Docker engine 或远程代理 API |
| 资源采样 / 进程模块 | CPU/内存为采样时值；模块仅枚举显式进程名，受进程访问权限限制 | 连续性能追踪、每个短进程、任意进程的全部模块 |

适用配置应分为“普通用户盘点”和“具备所需读取授权的增强盘点”。后者还需逐项确认日志启用、审计策略、组件和文件名单，不能使用一个“管理员模式”标签替代范围验证。已验证 OS 只有当前 Win10 LTSC x64；Win11/Server/ARM64 应列入兼容性验证，暂不写成已支持环境。

## 5. 性能数据与边界

### 采集进程

单轮状态执行单元合计 42.62 秒；日志首批与 Application 续批合计 16.82 秒。两者不是实际服务的一轮耗时，服务有两个调度通道并且日志按清单轮换。本次直接输出共 15,256,234 字节。

| 执行单元 | 单次墙钟秒 | CPU 秒 | 采样工作集峰值 MiB |
| --- | ---: | ---: | ---: |
| 基础盘点 | 9.06 | 7.70 | 221.5 |
| system | 7.78 | 2.55 | 151.5 |
| security | 7.18 | 1.94 | 152.4 |
| network | 3.26 | 3.09 | 169.5 |
| storage | 6.14 | 4.53 | 200.4 |
| startup | 5.94 | 6.69 | 145.9 |
| desktop | 1.58 | 1.92 | 151.9 |
| virtualization | 0.34 | 0.48 | 80.7 |
| applications | 0.66 | 0.91 | 145.2 |
| 指定范围 | 0.62 | 0.55 | 93.0 |
| 原生 software | 0.037 | CPU 计时粒度内 | 未取到样本 |
| 原生 USN（本次拒绝访问） | 0.025 | 0.016 | 未取到样本 |

另 3 次重复：基础盘点 8.47–10.14 秒，指定范围 0.56–0.64 秒，Application 首批 1.01–1.25 秒。单轮其他来源是观测点，不是 p95；USN 拒绝访问耗时不能用来估计成功读取性能。PS 的约 80–222 MiB 工作集是单个采集进程，尚无服务与 UI 合计内存测量。

固定 Application 3,638 条全量：PS **30.57 秒 / 119 条每秒**，原生 **8.55 秒 / 425 条每秒**。两端都包含每批重新启动进程、格式化、加密和 JSON 输出。原生方案降低启动/处理开销；没有改变日志轮换频率，不能直接承诺服务获得 425 条每秒的持续吞吐。

### 隔离存储与查询

| 数据集 | 入库 | 投影 | 对象首屏热查询 p50 / p95 | 未命中对象搜索 p95 |
| --- | ---: | ---: | ---: | ---: |
| 真实 13,649 条证据，7,341 对象 | 30.87 秒 | 3.03 秒 | 40.85 / 47.19 ms | 27.21 ms |
| 合成 2,000 个任务对象 | 1.33 秒 | 0.51 秒 | 5.18 / 5.36 ms | 2.95 ms |
| 合成累计 10,000 个任务对象（新增 8,000） | 新增部分 5.69 秒 | 新增部分 3.00 秒 | 25.05 / 29.46 ms | 22.91 ms |

真实集动作首屏 p95=0.915 ms（537 动作），资源采样 p95=0.611 ms，来源覆盖 p95=20.23 ms。每种热查询测 30 次，使用最近秩 p95；另测新连接，但 OS 缓存没有清空，不能称磁盘冷启动。全部是 Store 进程内查询，不是前端响应时延。合成集动作查询为空，不作为大动作时间线性能证据。

真实集逻辑分配约 123.30 MiB；采样时 DB 122.83 MiB、WAL 9.10 MiB、SHM 32 KiB，总物理约 131.96 MiB，已计 WAL。该结果包含初次盘点的事实、索引与基线，不能用一轮的倍率直接估算长期增量。

合成 10,000 对象后，选同一 200 个不变对象重复 20 轮：2.18 秒；事实保持 60,000，动作保持 0；数据库分配 **53,374,976→60,882,944 字节**，增加 **7,507,968 字节**，约 **375,398 字节/轮**。这是新轮次保留历史的增长，同一 request 重放则不新增证据。无限保存会持续增长，滚动配额/清理必须做单独压力验收。

### 调度上限与历史追赶

源码默认状态周期 300 秒、事件周期 10 秒；同类任务串行，状态和日志分别占一个执行通道。事件每轮服务前 4 个主通道，加 19 个次通道中的 1 个；每通道最多 3 批，每批最多扫描 128 条，且有约 950 KB 事件预算。

忽略执行时间和字节预算的**理论上限**：主通道 384/10=38.4 条每秒，单个次通道 384/(19×10)≈2.02 条每秒；次通道最少 190 秒才再次轮到。实际更低，因为下一周期从执行完成后计算，且本次 PowerShell 首批大事件在字节预算下只输出 12 条。这个机制适合低频留存日志，会在高频脚本/网络审计下积压；日志窗口短于追赶间隔时可能被覆盖。不能把后台的 caught_up 当成完整历史证明。

协议单行 1 MiB、单进程输出 64 MiB；PS 状态内层超时 110 秒、服务外层 120 秒，PS 日志内层 30 秒。超预算或超时会保留旧状态/断点，但一个巨大事件或分片可以持续阻塞该来源。目录/便携扫描的条目限制、CIM 嵌套深度、清单和投影积压限制也属于性能适用边界。

目录当前先完整枚举并排序直接子项，再检查条目数量；上限限制的是提交规模，不能当成枚举阶段的内存上限。本次仅小目录夹具，超大平面目录的耗时/内存仍需专门验证。

## 6. 开发与验收优先级

| 优先级 | 事项 | 完成条件 |
| --- | --- | --- |
| P0 | 包装器脚本信任/执行策略 | 在正常策略下用当前包完成真实采集，无需测试 ByPass；覆盖签名/信任部署和模块环境 |
| P0 | 完整度与环境缺口显示（N03） | 区分未配置、组件缺失、无权限、禁用日志、已追平但历史未知；失败不能成为空清单或对象删除 |
| P1 | 日志吞吐及自身审计噪声 | 评估原生后端；按 backlog/保留窗口调度；实测高吞吐追赶、清除与重置，明确缺口与 SLA |
| P1 | 长期容量与重启恢复（N06） | 5 GB / 72 小时 / 配额清理、并发查询/投影、崩溃恢复；核算历史增长与保留天数 |
| P1 | 增强权限和组件正向验证 | driver/DISM、LSA、Security、BitLocker、USN、Hyper-V/Sysmon 用授权环境逐项验收 |
| P2 | 字段与跨环境兼容矩阵 | 非 CIM API 属性、额外应用路径/用户范围，以及 Win11/Server/ARM64、低配置环境 |

本次评估完成；N02 实例隔离、N03 全局完整度、N04 所有 API 字段审查、N05 UI、N06 持续运行与恢复仍未全部验收。没有修改采集运行代码或默认后端以隐藏评估发现。

## 7. 复现与证据

入口：`scripts/test-collection-assessment.ps1`、`scripts/test-eventlog-performance.ps1`、`crates/storage/examples/collection-assessment.rs`。在工程根目录用 PS7 执行，始终使用新建的 `target` 子目录：

```powershell
# 验证默认执行策略；结果单独保存，失败不作为性能样本。
.\scripts\test-collection-assessment.ps1
# 仅诊断进程采用 Bypass；不修改系统策略。
.\scripts\test-collection-assessment.ps1 -CollectorPolicy Bypass
.\scripts\test-collection-assessment.ps1 -CollectorPolicy Bypass -Repeat 3 -OnlyJobs base,scoped,event-13
# 将 <run> 换成上述输出的 GUID 目录。
$env:TEMP = Join-Path $PWD 'target/n04-temp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Path $env:TEMP -Force | Out-Null
.\build.ps1 -ToolsRoot '..\SSS-main\SSS-main\pcscope\.tools' -CargoArgs @('build','--release','-p','pcscope-storage','--example','collection-assessment')
.\target\release\examples\collection-assessment.exe target/collection-assessment/<run>/summary.json
.\scripts\test-eventlog-performance.ps1 -OutputDirectory target/collection-assessment/<run>
.\target\release\examples\collection-assessment.exe compare-eventlog target/collection-assessment/<run>/eventlog-<guid>/result.json
```

正式证据位于 `target/collection-assessment/c5aa0c94807b4f14bf4f95bc1e46d9ac`；3 次复测位于 `b080dfb24a854e5e9b1b0d98be2ea05c`；固定归档位于正式目录的 `eventlog-c0a1f7e8fda5458aa07fa69d5cea756a`。原始材料含本机信息且被 `.gitignore` 排除，不纳入公开报告。统计摘要保存为同目录文档 `N04-collection-metrics.json`，来源逐项结果见下表。

<!-- assessment-source-table -->

## 8. 本机逐来源结果

来源目录声明 131 个标签，包含动态来源模板与虚拟前缀；本机实际输出 125 个具体名称，不以二者相除计算“全系统完整率”。下表只记录诊断配置下的结果。

| 来源（省略 pcscope.） | 结果 | 快照 item | event | gap |
| --- | --- | ---: | ---: | ---: |
| apps.browser-policies | 范围内为空 | 0 | 0 | 0 |
| apps.chromium-extensions | 有数据 | 12 | 0 | 0 |
| apps.dotnet-runtimes | 有数据 | 24 | 0 | 0 |
| apps.firefox-extensions | 范围内为空 | 0 | 0 | 0 |
| apps.ide-extensions | 范围内为空 | 0 | 0 | 0 |
| apps.npm-global-packages | 有数据 | 2 | 0 | 0 |
| apps.onedrive-registration | 范围内为空 | 0 | 0 | 0 |
| collector.event-channel-coverage | 有数据 | 23 | 0 | 0 |
| collector.scoped-configuration | 有数据 | 1 | 0 | 0 |
| desktop.clipboard | 有数据 | 4 | 0 | 0 |
| desktop.input | 不可用 | 0 | 0 | 1 |
| desktop.print-jobs | 范围内为空 | 0 | 0 | 0 |
| desktop.printer-drivers | 有数据 | 5 | 0 | 0 |
| desktop.printer-ports | 有数据 | 10 | 0 | 0 |
| desktop.printing | 有数据 | 3 | 0 | 0 |
| desktop.shell-settings | 有数据 | 6 | 0 | 0 |
| desktop.shortcuts | 有数据 | 94 | 0 | 0 |
| environment.configuration | 有数据 | 1106 | 0 | 0 |
| eventlog.application | 有数据 | 0 | 256 | 0 |
| eventlog.microsoft-windows-applocker/exe and dll | 范围内为空 | 0 | 0 | 0 |
| eventlog.microsoft-windows-applocker/msi and script | 范围内为空 | 0 | 0 | 0 |
| eventlog.microsoft-windows-appxdeploymentserver/operational | 有数据 | 0 | 128 | 0 |
| eventlog.microsoft-windows-backup | 范围内为空 | 0 | 0 | 0 |
| eventlog.microsoft-windows-bitlocker/bitlocker management | 有数据 | 0 | 5 | 0 |
| eventlog.microsoft-windows-codeintegrity/operational | 有数据 | 0 | 122 | 0 |
| eventlog.microsoft-windows-driverframeworks-usermode/operational | 范围内为空 | 0 | 0 | 0 |
| eventlog.microsoft-windows-grouppolicy/operational | 不可用 | 0 | 0 | 1 |
| eventlog.microsoft-windows-hyper-v-vmms/admin | 不可用 | 0 | 0 | 1 |
| eventlog.microsoft-windows-networkprofile/operational | 有数据 | 0 | 128 | 0 |
| eventlog.microsoft-windows-powershell/operational | 有数据 | 0 | 12 | 0 |
| eventlog.microsoft-windows-printservice/operational | 范围内为空 | 0 | 0 | 0 |
| eventlog.microsoft-windows-sysmon/operational | 不可用 | 0 | 0 | 1 |
| eventlog.microsoft-windows-taskscheduler/operational | 范围内为空 | 0 | 0 | 0 |
| eventlog.microsoft-windows-terminalservices-localsessionmanager/operational | 有数据 | 0 | 128 | 0 |
| eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational | 范围内为空 | 0 | 0 | 0 |
| eventlog.microsoft-windows-windows defender/operational | 有数据 | 0 | 128 | 0 |
| eventlog.microsoft-windows-windowsupdateclient/operational | 有数据 | 0 | 128 | 0 |
| eventlog.microsoft-windows-wlan-autoconfig/operational | 有数据 | 0 | 128 | 0 |
| eventlog.microsoft-windows-wmi-activity/operational | 有数据 | 0 | 128 | 0 |
| eventlog.security | 不可用 | 0 | 0 | 1 |
| eventlog.system | 有数据 | 0 | 128 | 0 |
| execution.processes | 有数据 | 247 | 0 | 0 |
| execution.resources | 有数据 | 22 | 0 | 0 |
| files.directory-security | 有数据 | 1 | 0 | 0 |
| files.metadata-security | 有数据 | 1 | 0 | 0 |
| hardware.battery | 有数据 | 1 | 0 | 0 |
| hardware.devices | 有数据 | 212 | 0 | 0 |
| hardware.disk-health | 有数据 | 1 | 0 | 0 |
| hardware.driver-bindings | 有数据 | 215 | 0 | 0 |
| hardware.driver-packages | 不可用 | 0 | 0 | 1 |
| hardware.driver-services | 有数据 | 395 | 0 | 0 |
| hardware.firmware | 有数据 | 1 | 0 | 0 |
| identity.accounts | 有数据 | 7 | 0 | 0 |
| identity.assigned-rights | 不可用 | 0 | 0 | 1 |
| identity.groups | 有数据 | 20 | 0 | 0 |
| identity.interactive-sessions | 有数据 | 2 | 0 | 0 |
| identity.logon-sessions | 有数据 | 4 | 0 | 0 |
| identity.machine | 有数据 | 2 | 0 | 0 |
| identity.profiles | 有数据 | 4 | 0 | 0 |
| network.access | 有数据 | 4 | 0 | 0 |
| network.adapters | 有数据 | 17 | 0 | 0 |
| network.addresses | 有数据 | 12 | 0 | 0 |
| network.dns-cache | 有数据 | 8 | 0 | 0 |
| network.dns-configuration | 有数据 | 14 | 0 | 0 |
| network.firewall-profiles | 有数据 | 3 | 0 | 0 |
| network.firewall-rules | 不可用 | 0 | 0 | 1 |
| network.hosts | 范围内为空 | 0 | 0 | 0 |
| network.ipsec | 不可用 | 0 | 0 | 1 |
| network.mapped-drives | 范围内为空 | 0 | 0 | 0 |
| network.profiles | 有数据 | 2 | 0 | 0 |
| network.routes | 有数据 | 37 | 0 | 0 |
| network.share-sessions | 不可用 | 0 | 0 | 1 |
| network.shares | 有数据 | 3 | 0 | 0 |
| network.tcp | 有数据 | 322 | 0 | 0 |
| network.udp | 有数据 | 50 | 0 | 0 |
| network.winrm-configuration | 不可用 | 0 | 0 | 1 |
| policy.configuration | 有数据 | 34 | 0 | 0 |
| power.configuration | 不可用 | 0 | 0 | 1 |
| security.certificates.currentuser | 有数据 | 42 | 0 | 0 |
| security.certificates.localmachine | 有数据 | 40 | 0 | 0 |
| security.detections | 范围内为空 | 0 | 0 | 0 |
| security.deviceguard | 有数据 | 1 | 0 | 0 |
| security.encryption | 不可用 | 0 | 0 | 1 |
| security.key-metadata | 部分可用 | 2 | 0 | 4 |
| security.products | 有数据 | 1 | 0 | 0 |
| security.protection | 有数据 | 1 | 0 | 0 |
| security.protection-policy | 有数据 | 1 | 0 | 0 |
| security.secureboot | 不可用 | 0 | 0 | 1 |
| security.ssh-public-keys | 范围内为空 | 0 | 0 | 0 |
| security.tpm | 有数据 | 1 | 0 | 0 |
| shell.registrations | 有数据 | 8217 | 0 | 0 |
| software | 有数据 | 52 | 0 | 0 |
| software.appx-user | 有数据 | 62 | 0 | 0 |
| software.optional-features | 不可用 | 0 | 0 | 1 |
| software.provisioned | 不可用 | 0 | 0 | 1 |
| software.user-inventory | 有数据 | 10 | 0 | 0 |
| startup.advanced-extensions | 不可用 | 0 | 0 | 1 |
| startup.logon | 有数据 | 26 | 0 | 0 |
| startup.service-configuration | 不可用 | 0 | 0 | 1 |
| startup.services | 有数据 | 284 | 0 | 0 |
| startup.session-hooks | 有数据 | 65 | 0 | 0 |
| startup.task-details | 有数据 | 155 | 0 | 0 |
| startup.tasks | 有数据 | 155 | 0 | 0 |
| startup.wmi-bindings | 有数据 | 1 | 0 | 0 |
| startup.wmi-consumers | 有数据 | 1 | 0 | 0 |
| startup.wmi-filters | 有数据 | 1 | 0 | 0 |
| storage.disks | 有数据 | 1 | 0 | 0 |
| storage.partitions | 有数据 | 4 | 0 | 0 |
| storage.volumes | 有数据 | 3 | 0 | 0 |
| system.boot-state | 有数据 | 1 | 0 | 0 |
| system.clock | 有数据 | 29 | 0 | 0 |
| system.hotfixes | 有数据 | 8 | 0 | 0 |
| system.reboot-pending | 有数据 | 2 | 0 | 0 |
| system.restore-points | 不可用 | 0 | 0 | 1 |
| system.update-history | 有数据 | 128 | 0 | 0 |
| system.version | 有数据 | 1 | 0 | 0 |
| usn.c | 不可用 | 0 | 0 | 1 |
| usn.volumes | 有数据 | 1 | 0 | 0 |
| usn.watch-list | 有数据 | 1 | 0 | 0 |
| virtualization.hyperv-checkpoints | 不可用 | 0 | 0 | 1 |
| virtualization.hyperv-disks | 不可用 | 0 | 0 | 1 |
| virtualization.hyperv-switches | 不可用 | 0 | 0 | 1 |
| virtualization.hyperv-vms | 不可用 | 0 | 0 | 1 |
| virtualization.platform | 有数据 | 2 | 0 | 0 |
| virtualization.wsl-registration | 范围内为空 | 0 | 0 | 0 |

主轮没有字面输出的声明：Mihomo、Docker、modules、portable 因配置未开启；eventlog.import 在独立固定归档测试中产生了实际散列后缀来源；usn.{drive} 的实际来源为 usn.c；usn.changes 是虚拟分组。它们不能一律计为采集失败。17 个空结果的具体来源已在表中列出，需结合应用/组件和日志启用状态解释。
