# 后端原生采集逐项清单

由 `scripts/build-native-migration.cjs` 生成；决策维护于 `model/native-collection-plan.cjs`。当前实现依据 `model/sources.json`，目标接口不表示已完成。

返回[范围与实施方案](../backend-native-collection-plan.md)。

共 133 项：127 项已原生、4 项待迁移、2 项分组。未分类来源为 0；待迁移 P0 0 / P1 0 / P2 4。

采集器摘要：`aef4ed8a4b702f9f873c0c72699f19716d626f381678e790ca2849561b1b6754`。

## 注册表与配置

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.apps.browser-policies — Chrome、Edge 与 Firefox 策略 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.apps.onedrive-registration — OneDrive 账户与同步登记 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.desktop.clipboard — 剪贴板设置 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.desktop.input — 键盘、输入法与过滤驱动配置 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.desktop.shell-settings — 桌面与资源管理器设置 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.environment.configuration — 环境变量与默认文件关联 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.network.winrm-configuration — WinRM 配置 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.policy.configuration — UAC、资源管理器与软件限制策略 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.shell.registrations — 第三方预览处理程序 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.software.user-inventory — 当前用户软件登记 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.startup.advanced-extensions — IFEO、AppInit 与 AppCert 入口 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.startup.logon — 登录启动登记 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.startup.service-configuration — 服务详细配置 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.startup.session-hooks — 会话与登录初始化入口 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.system.clock — 时间同步配置与时区 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.system.reboot-pending — 待重启标志 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |
| pcscope.virtualization.wsl-registration — WSL 发行版登记 | 已原生 / P1 | 当前状态 | RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取 | 保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。 |

## 文件与应用登记

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.apps.chromium-extensions — Chrome / Edge 扩展 | 已原生 / P1 | 当前状态 | 编译后端直接枚举文件、解析 JSON/文本、读取版本和计算摘要 | 仅原允许目录、文件大小与层级；避免重解析点和自身数据目录。扩展安装清单不自动证明已启用；不读私钥。 |
| pcscope.apps.firefox-extensions — Firefox 扩展 | 已原生 / P1 | 当前状态 | 编译后端直接枚举文件、解析 JSON/文本、读取版本和计算摘要 | 仅原允许目录、文件大小与层级；避免重解析点和自身数据目录。扩展安装清单不自动证明已启用；不读私钥。 |
| pcscope.apps.ide-extensions — VS Code 扩展清单 | 已原生 / P1 | 当前状态 | 编译后端直接枚举文件、解析 JSON/文本、读取版本和计算摘要 | 仅原允许目录、文件大小与层级；避免重解析点和自身数据目录。扩展安装清单不自动证明已启用；不读私钥。 |
| pcscope.apps.npm-global-packages — 全局 npm 包清单 | 已原生 / P1 | 当前状态 | 编译后端直接枚举文件、解析 JSON/文本、读取版本和计算摘要 | 仅原允许目录、文件大小与层级；避免重解析点和自身数据目录。扩展安装清单不自动证明已启用；不读私钥。 |
| pcscope.desktop.shortcuts — 桌面与开始菜单快捷方式 | 已原生 / P1 | 当前状态 | 编译宿主直接 COM 读取 WScript.Shell/CreateShortcut 现有元数据及 Known Folder；不启动脚本宿主 | 固定读取属性，不调用 Run/Exec/Save 或目标程序；原描述读取 E_FAIL 保持空值，保持原接口语言。后续可换等价 IShellLinkW/IPersistFile。 |
| pcscope.network.hosts — hosts 静态映射 | 已原生 / P1 | 当前状态 | 编译后端直接枚举文件、解析 JSON/文本、读取版本和计算摘要 | 仅原允许目录、文件大小与层级；避免重解析点和自身数据目录。扩展安装清单不自动证明已启用；不读私钥。 |
| pcscope.security.ssh-public-keys — SSH 公钥指纹 | 已原生 / P1 | 当前状态 | 编译后端直接枚举文件、解析 JSON/文本、读取版本和计算摘要 | 仅原允许目录、文件大小与层级；避免重解析点和自身数据目录。扩展安装清单不自动证明已启用；不读私钥。 |
| pcscope.software.portable — 便携软件文件登记 | 已原生 / P1 | 当前状态 | 编译后端直接枚举文件、解析 JSON/文本、读取版本和计算摘要 | 仅原允许目录、文件大小与层级；避免重解析点和自身数据目录。扩展安装清单不自动证明已启用；不读私钥。 |

## 已原生登记

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.apps.dotnet-runtimes — .NET 运行时登记 | 已原生 / 保留 | 当前状态 | 编译宿主直接读取 Registry | 保留各来源现有视图和用户范围。 |
| pcscope.software — 本机软件安装登记 | 已原生 / 保留 | 当前状态 | 编译宿主直接读取 Registry | 保留各来源现有视图和用户范围。 |

## 第三方产品

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.apps.mihomo — Mihomo 本地运行配置 | 已原生 / P2 | 当前状态 | 编译后端 HTTP 客户端调用现有本机 /version、/configs API | 显式开启、固定回环地址、超时和大小限制；凭据保护，不修改代理配置。 |
| pcscope.virtualization.docker — Docker 容器、镜像、卷与网络 | 已原生 / P2 | 当前状态 | 编译后端通过本机命名管道调用 Docker Engine HTTP API | 保持当前 docker_engine 范围，协商 API 版本；不自动启动 Docker，不使用远程端点；无 PowerShell 或 shell。 |

## 采集能力

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.collector.event-channel-coverage — 事件通道可读性 | 已原生 / 保留 | 采集能力 | EventLogConfiguration / EventLogSession / EventLogReader | 读取通道配置与首尾记录；不推断全部历史完整。 |

## 采集配置

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.collector.scoped-configuration — 指定范围配置 | 已原生 / 保留 | 采集配置 | 编译后端读取已配置名单与本地配置 | 名单与日志元数据分开，不产生额外系统采集。 |
| pcscope.usn.watch-list — null | 已原生 / 保留 | 采集配置 | 编译后端读取已配置名单与本地配置 | 名单与日志元数据分开，不产生额外系统采集。 |

## 打印

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.desktop.print-jobs — 打印作业队列 | 已原生 / P1 | 当前状态 | Winspool：EnumPrinters / EnumJobs / EnumPrinterDrivers / EnumPorts / GetPrinter | 保留用户连接打印机与队列；短暂完成作业的完整历史需要打印事件记录。 |
| pcscope.desktop.printer-drivers — 打印驱动 | 已原生 / P1 | 当前状态 | Winspool：EnumPrinters / EnumJobs / EnumPrinterDrivers / EnumPorts / GetPrinter | 保留用户连接打印机与队列；短暂完成作业的完整历史需要打印事件记录。 |
| pcscope.desktop.printer-ports — 打印端口 | 已原生 / P1 | 当前状态 | Winspool：EnumPrinters / EnumJobs / EnumPrinterDrivers / EnumPorts / GetPrinter | 保留用户连接打印机与队列；短暂完成作业的完整历史需要打印事件记录。 |
| pcscope.desktop.printing — 打印机清单 | 已原生 / P1 | 当前状态 | Winspool：EnumPrinters / EnumJobs / EnumPrinterDrivers / EnumPorts / GetPrinter | 保留用户连接打印机与队列；短暂完成作业的完整历史需要打印事件记录。 |

## Windows 事件日志

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.eventlog.application — 应用程序事件 / 软件安装事件 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-applocker/exe and dll — AppLocker 程序与 DLL 事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-applocker/msi and script — AppLocker 安装包与脚本事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational — AppX 部署事件 / 应用包部署 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-backup — Windows 备份事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management — BitLocker 管理事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-codeintegrity/operational — 代码完整性事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational — 用户模式驱动事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-grouppolicy/operational — 组策略处理事件 / 组策略服务生命周期 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin — Hyper-V 管理事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-networkprofile/operational — 网络配置文件事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-powershell/operational — PowerShell 运行事件 / PowerShell 命令与脚本 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-printservice/operational — 打印服务事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-sysmon/operational — Sysmon / 进程生命周期 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-taskscheduler/operational — 计划任务执行事件 / 任务触发与执行过程 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational — 终端会话登录、断开与注销 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational — 远程连接管理事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-windows defender/operational — Defender 防护事件 / Defender 健康与情报 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-windowsupdateclient/operational — Windows 更新事件 / 更新检查 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational — 无线网络连接事件 / 来源采集状态 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.microsoft-windows-wmi-activity/operational — WMI 活动事件 / WMI 提供程序与调用 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.security — Windows 安全审计 / 登录、登出与会话 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |
| pcscope.eventlog.system — 系统与服务事件 / 服务运行事件 | 已原生 / 保留 | 事件历史 | 现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读 | 事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。 |

## 虚拟分组

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.eventlog.import — null | 不采集 / 不采集 | 分组 | 复用既有日志/USN 导入与分组 | 这两项不单独启动采集器。 |
| pcscope.usn.changes — null | 不采集 / 不采集 | 分组 | 复用既有日志/USN 导入与分组 | 这两项不单独启动采集器。 |

## 模块文件变化

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.execution.module-file-state — 已观察模块的磁盘文件变化 | 已原生 / 保留 | 变化跟踪 | ReadDirectoryChangesW + 原生文件身份/摘要/签名检查 | 共享目录监听、已知文件集合、去抖、稳定复核；不是文件操作的完整时间历史。 |

## 进程模块

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.execution.module-scan-status — 模块发现与读取范围 | 已原生 / 保留 | 当前状态 | PSAPI / OpenProcess / GetProcessTimes + 用户/会话过滤 | 只跟踪实际观察模块；进程退出后基线保留，不宣称能发现首次启动即失败的未知模块。 |
| pcscope.execution.modules — 指定进程加载模块 | 已原生 / 保留 | 当前状态 | PSAPI / OpenProcess / GetProcessTimes + 用户/会话过滤 | 只跟踪实际观察模块；进程退出后基线保留，不宣称能发现首次启动即失败的未知模块。 |

## 进程与后台

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.execution.processes — 进程清单 | 已原生 / P0 | 当前状态 | Toolhelp / EnumProcesses / GetProcessTimes / QueryFullProcessImageName；Win32_Process 补充 | PID+建立时间区分实例；受保护进程拒绝访问不能当作退出。命令行沿用保护规则。 |
| pcscope.hardware.driver-services — 驱动服务 | 已原生 / P0 | 当前状态 | OpenSCManager / EnumServicesStatusEx / QueryServiceConfig；直接 WMI 补齐派生字段 | 同时覆盖 Win32 服务与驱动服务，按原来源分开；不以服务运行证明驱动文件完整。 |
| pcscope.startup.services — 服务运行状态 | 已原生 / P0 | 当前状态 | OpenSCManager / EnumServicesStatusEx / QueryServiceConfig；直接 WMI 补齐派生字段 | 同时覆盖 Win32 服务与驱动服务，按原来源分开；不以服务运行证明驱动文件完整。 |
| pcscope.startup.task-details — 计划任务执行记录与条件 | 已原生 / P0 | 当前状态 | 编译宿主直接 MI 读取 MSFT_ScheduledTask，固定只读 PS_ScheduledTask.GetInfoByObject；系统任务提供程序使用 Task Scheduler 接口 | 完整任务路径、嵌入操作/触发器/条件/主体及最近执行信息；不加载 PowerShell；当前配置和最近运行结果不替代每次运行历史。 |
| pcscope.startup.tasks — 计划任务基础配置 | 已原生 / P0 | 当前状态 | 编译宿主直接 MI 读取 MSFT_ScheduledTask，固定只读 PS_ScheduledTask.GetInfoByObject；系统任务提供程序使用 Task Scheduler 接口 | 完整任务路径、嵌入操作/触发器/条件/主体及最近执行信息；不加载 PowerShell；当前配置和最近运行结果不替代每次运行历史。 |

## 资源采样

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.execution.resources — 内存与 CPU 采样 | 已原生 / 保留 | 当前状态 | 编译宿主直接 WMI 查询 Win32_OperatingSystem / CPU 性能类 | 当前已编译运行；CPU 采样区间、开销预算另验。 |

## 文件状态

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.files.directory-security — 指定目录与直接子项 | 已原生 / 保留 | 当前状态 | 编译宿主文件/ACL/ADS API、SHA256、GetFileInformationByHandle | 保留指定名单、深度、大小与重解析点限制。 |
| pcscope.files.metadata-security — 指定文件、ACL 与 ADS | 已原生 / 保留 | 当前状态 | 编译宿主文件/ACL/ADS API、SHA256、GetFileInformationByHandle | 保留指定名单、深度、大小与重解析点限制。 |

## 系统提供程序

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.hardware.battery — 电池状态 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.hardware.driver-bindings — 设备驱动绑定 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.hardware.firmware — BIOS 固件 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.identity.logon-sessions — 登录会话 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.identity.machine — 计算机身份与域信息 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.identity.profiles — 用户配置文件 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.system.boot-state — 最近启动时间与时区 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.system.hotfixes — QFE 更新登记 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.system.restore-points — 系统还原点 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.system.version — 操作系统版本与安装时间 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |
| pcscope.virtualization.platform — 虚拟化能力与平台 | 已原生 / P1 | 当前状态 | 编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类 | 保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。 |

## 网卡与设备

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.hardware.devices — 即插即用设备 | 已原生 / P0 | 当前状态 | SetupDiGetClassDevs / SetupDiEnumDeviceInfo / CM_Get_DevNode_Status；直接 WMI 补充字段 | 覆盖设备登记与当前存在状态；移除、禁用、断开分别记录，失败不提交空快照。 |
| pcscope.network.adapters — 网络适配器 | 已原生 / P0 | 当前状态 | 直接查询 MSFT_NetAdapter；GetIfTable2 补充接口状态；SetupAPI/CM 关联 PnP 标识 | 保留隐藏/虚拟/断开网卡；GetIfTable2 单独不能替代原全部字段和设备范围。GUID 与 PnP ID 分开。 |

## 磁盘与分区

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.hardware.disk-health — 物理磁盘健康 | 已原生 / P1 | 当前状态 | 直接查询 root/Microsoft/Windows/Storage：MSFT_Disk / MSFT_Partition / MSFT_PhysicalDisk | 保留 ObjectId、UniqueId、访问路径、磁盘健康和关联；存储提供程序不存在时报告能力不可用。 |
| pcscope.storage.disks — 磁盘清单 | 已原生 / P1 | 当前状态 | 直接查询 root/Microsoft/Windows/Storage：MSFT_Disk / MSFT_Partition / MSFT_PhysicalDisk | 保留 ObjectId、UniqueId、访问路径、磁盘健康和关联；存储提供程序不存在时报告能力不可用。 |
| pcscope.storage.partitions — 分区清单 | 已原生 / P1 | 当前状态 | 直接查询 root/Microsoft/Windows/Storage：MSFT_Disk / MSFT_Partition / MSFT_PhysicalDisk | 保留 ObjectId、UniqueId、访问路径、磁盘健康和关联；存储提供程序不存在时报告能力不可用。 |
| pcscope.storage.volumes — 卷与文件系统 | 已原生 / P1 | 当前状态 | FindFirstVolume / GetVolumeInformation / GetDiskFreeSpaceEx；Win32_Volume 补充 | 包含无盘符卷，卷身份不得退化为盘符；保留原驱动器类型与容量字段。 |

## 驱动仓库

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.hardware.driver-packages — 驱动仓库包 | 已原生 / 保留 | 当前状态 | DISM API DismGetDrivers | INF+仓库路径区分包；732 包只读验收已通过。 |

## 账户

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.identity.accounts — 本机用户账户 | 已原生 / P1 | 当前状态 | NetUserEnum / NetUserGetInfo / NetLocalGroupEnum / NetLocalGroupGetMembers / LookupAccountSid | SID 做身份；域不可达或成员解析失败不能提交成员全空。 |
| pcscope.identity.assigned-rights — 账户权限分配 | 已原生 / 保留 | 当前状态 | LSA：LsaEnumerateAccountsWithUserRight / LsaEnumerateAccountRights | 按 SID，区分机器权限和用户会话。 |
| pcscope.identity.groups — 本机用户组与成员 | 已原生 / P1 | 当前状态 | NetUserEnum / NetUserGetInfo / NetLocalGroupEnum / NetLocalGroupGetMembers / LookupAccountSid | SID 做身份；域不可达或成员解析失败不能提交成员全空。 |

## 会话

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.identity.interactive-sessions — 交互式终端会话 | 已原生 / 保留 | 当前状态 | WTS：WTSEnumerateSessions / WTSQuerySessionInformation | 当前会话范围，过去登录动作依赖事件。 |

## 远程访问

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.network.access — 代理、远程桌面与 VPN | 已原生 / P1 | 当前状态 | RasEnumEntries / RasGetEntryProperties / RasEnumConnections + Registry；必要时 VPN CIM 提供程序 | 当前用户和全用户 VPN 分开；不得收集保存的密码，保持代理/RDP 配置字段。 |

## 网络现状

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.network.addresses — IP 地址 | 已原生 / P0 | 当前状态 | GetUnicastIpAddressTable；MSFT_NetIPAddress 补充原字段 | IPv4/IPv6、接口关联、状态和前缀来源均需保留。 |
| pcscope.network.routes — 路由表 | 已原生 / P0 | 当前状态 | GetIpForwardTable2；MSFT_NetRoute 补充策略来源 | Active/Persistent 等策略范围不因迁移丢失。 |
| pcscope.network.tcp — TCP 连接与监听 | 已原生 / P0 | 当前状态 | GetExtendedTcpTable / GetExtendedUdpTable；必要时直接查询 MSFT_NetTCPConnection / MSFT_NetUDPEndpoint | 保留创建时间与所属进程等身份字段；PID 模式不足时使用更完整结构/提供程序，不能以读取时间代替创建时间。 |
| pcscope.network.udp — UDP 端点 | 已原生 / P0 | 当前状态 | GetExtendedTcpTable / GetExtendedUdpTable；必要时直接查询 MSFT_NetTCPConnection / MSFT_NetUDPEndpoint | 保留创建时间与所属进程等身份字段；PID 模式不足时使用更完整结构/提供程序，不能以读取时间代替创建时间。 |

## 网络提供程序

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.network.dns-cache — DNS 缓存 | 已原生 / P1 | 当前状态 | 直接查询 root/StandardCimv2：MSFT_DNSClientServerAddress / MSFT_DNSClientCache / MSFT_NetConnectionProfile；Registry 补配置 | DNS 缓存是当前缓存，不是访问历史；不调用未文档化的 DNS 缓存枚举导出。 |
| pcscope.network.dns-configuration — DNS 配置 | 已原生 / P1 | 当前状态 | 直接查询 root/StandardCimv2：MSFT_DNSClientServerAddress / MSFT_DNSClientCache / MSFT_NetConnectionProfile；Registry 补配置 | DNS 缓存是当前缓存，不是访问历史；不调用未文档化的 DNS 缓存枚举导出。 |
| pcscope.network.profiles — 网络连接配置文件 | 已原生 / P1 | 当前状态 | 直接查询 root/StandardCimv2：MSFT_DNSClientServerAddress / MSFT_DNSClientCache / MSFT_NetConnectionProfile；Registry 补配置 | DNS 缓存是当前缓存，不是访问历史；不调用未文档化的 DNS 缓存枚举导出。 |

## 防火墙与 IPsec

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.network.firewall-profiles — 防火墙配置文件 | 已原生 / P1 | 当前状态 | 直接调用 StandardCimv2 防火墙/IPsec 提供程序，读取规则及关联过滤器 | 保留 ActiveStore、PolicyStoreSource 和七类过滤器；不能只用简化 INetFwPolicy2 视图宣称完全等价。 |
| pcscope.network.firewall-rules — 防火墙规则及过滤条件 | 已原生 / P1 | 当前状态 | 直接调用 StandardCimv2 防火墙/IPsec 提供程序，读取规则及关联过滤器 | 保留 ActiveStore、PolicyStoreSource 和七类过滤器；不能只用简化 INetFwPolicy2 视图宣称完全等价。 |
| pcscope.network.ipsec — IPsec 规则 | 已原生 / P1 | 当前状态 | 直接调用 StandardCimv2 防火墙/IPsec 提供程序，读取规则及关联过滤器 | 保留 ActiveStore、PolicyStoreSource 和七类过滤器；不能只用简化 INetFwPolicy2 视图宣称完全等价。 |

## SMB

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.network.mapped-drives — SMB 驱动器映射 | 已原生 / P1 | 当前状态 | 直接查询 SMB CIM 提供程序；NetShareEnum / NetSessionEnum / WNetEnumResource 作为字段可验证的补充 | Get-SmbSession 实际为本机 SMB 服务端上的客户端会话，现有名称需校正；保留加密、方言、访问 ACL，不能用少字段接口缩减。 |
| pcscope.network.share-sessions — SMB 客户端会话 | 已原生 / P1 | 当前状态 | 直接查询 SMB CIM 提供程序；NetShareEnum / NetSessionEnum / WNetEnumResource 作为字段可验证的补充 | Get-SmbSession 实际为本机 SMB 服务端上的客户端会话，现有名称需校正；保留加密、方言、访问 ACL，不能用少字段接口缩减。 |
| pcscope.network.shares — SMB 共享与访问授权 | 已原生 / P1 | 当前状态 | 直接查询 SMB CIM 提供程序；NetShareEnum / NetSessionEnum / WNetEnumResource 作为字段可验证的补充 | Get-SmbSession 实际为本机 SMB 服务端上的客户端会话，现有名称需校正；保留加密、方言、访问 ACL，不能用少字段接口缩减。 |

## 电源

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.power.configuration — 电源计划与休眠配置 | 已原生 / P1 | 当前状态 | PowerEnumerate / PowerReadFriendlyName / PowerGetActiveScheme + Registry；Win32_PowerPlan 保持兼容身份 | 保留计划 GUID 与当前使用状态，读取不切换电源计划。 |

## 证书

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.security.certificates.currentuser — 当前用户证书 | 已原生 / P1 | 当前状态 | CertOpenStore / CertEnumCertificatesInStore 或编译宿主 X509Store | 区分当前用户与机器证书库；仅元数据，不导出私钥。 |
| pcscope.security.certificates.localmachine — 计算机证书 | 已原生 / P1 | 当前状态 | CertOpenStore / CertEnumCertificatesInStore 或编译宿主 X509Store | 区分当前用户与机器证书库；仅元数据，不导出私钥。 |

## 产品历史

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.security.detections — Defender 检测记录 | 已原生 / P2 | 产品保留历史 | 直接查询 MSFT_MpThreatDetection，关联 Defender Operational 事件 | 保留 DetectionID 和检测时间；这是产品保留的历史，不是全机器当前威胁扫描。 |
| pcscope.system.update-history — Windows Update 历史 | 已原生 / P2 | 产品保留历史 | WUA COM：IUpdateSession / IUpdateSearcher.QueryHistory | 普通应用完整性级别实机 136 条历史身份/字段比对通过；低完整性拒绝。仅查询产品保留历史，不搜索、下载或安装更新；其他环境继续验收。 |

## 安全状态

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.security.deviceguard — Device Guard / VBS | 已原生 / P2 | 当前状态 | 直接查询 Defender WMIv2、SecurityCenter2、DeviceGuard 提供程序 | Defender/第三方产品、Windows Server 和系统版本适用性分开；IsTamperProtected 等版本字段必须实测。 |
| pcscope.security.encryption — BitLocker 卷状态 | 已原生 / P2 | 当前状态 | Win32_EncryptableVolume 只读属性/方法，编译宿主直接调用 | 不得解锁、启动加密或读取恢复密钥；保持卷和保护器元数据的范围。 |
| pcscope.security.key-metadata — CNG 密钥元数据 | 已原生 / 保留 | 当前状态 | NCryptEnumStorageProviders / NCryptEnumKeys（SILENT） | 仅密钥元数据，不触发 PIN；3 个提供程序/范围问题仍单列未完成。 |
| pcscope.security.products — 安全产品登记 | 已原生 / P2 | 当前状态 | 直接查询 Defender WMIv2、SecurityCenter2、DeviceGuard 提供程序 | Defender/第三方产品、Windows Server 和系统版本适用性分开；IsTamperProtected 等版本字段必须实测。 |
| pcscope.security.protection — Defender 防护状态 | 已原生 / P2 | 当前状态 | 直接查询 Defender WMIv2、SecurityCenter2、DeviceGuard 提供程序 | Defender/第三方产品、Windows Server 和系统版本适用性分开；IsTamperProtected 等版本字段必须实测。 |
| pcscope.security.protection-policy — Defender 策略与排除项 | 已原生 / P2 | 当前状态 | 直接查询 Defender WMIv2、SecurityCenter2、DeviceGuard 提供程序 | Defender/第三方产品、Windows Server 和系统版本适用性分开；IsTamperProtected 等版本字段必须实测。 |
| pcscope.security.secureboot — 安全启动状态 | 已原生 / P2 | 当前状态 | GetFirmwareType + GetFirmwareEnvironmentVariableExW 读取 UEFI SecureBoot | UEFI/传统 BIOS/权限不足分别处理；不修改固件变量。 |
| pcscope.security.tpm — TPM 状态 | 已原生 / P2 | 当前状态 | TpmCoreProvisioning 只读状态与厂商元数据，TBS 固定无认证能力查询读取锁定状态 | 保留九字段；待处理物理确认操作非零对应 RestartPending。避免原命令构造器的授权材料读取、自检和可能的配置改变。权限错误保留旧状态，不伪造默认值；硬件代际与版本环境继续验证。 |

## Windows 应用

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.software.appx-user — 当前用户 AppX 包 | 已原生 / P1 | 当前状态 | WinRT PackageManager.FindPackagesForUser | 按原用户 SID 查询包和依赖，保留注册状态、体系结构和安装位置；不能改为只看当前提升账户。 |
| pcscope.software.optional-features — Windows 可选组件 | 已原生 / P1 | 当前状态 | DISM API DismGetFeatures / DismGetFeatureInfo | 只读在线映像；包/功能状态保留，权限失败不触发启用功能。 |
| pcscope.software.provisioned — 预配 AppX 包 | 已原生 / P2 | 当前状态 | 编译宿主直接 WinRT FindProvisionedPackages；API 不存在的旧系统使用 DISM _DismGetProvisionedAppxPackages | 完整保留五字段、包身份与机器范围；公开接口按运行时可用性选择，不按安装包标题猜名称，不修改系统或回退 PowerShell。旧 LTSC 私有 DISM 布局和版本环境继续验证。 |

## WMI 订阅配置

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.startup.wmi-bindings — WMI 过滤器与消费者绑定 | 已原生 / P1 | 当前状态 | WMI COM 直接读取 root/subscription 的 __FilterToConsumerBinding / __EventConsumer / __EventFilter | 只读订阅定义；正确保留对象路径、绑定及保护的脚本内容，不创建或触发订阅。 |
| pcscope.startup.wmi-consumers — WMI 永久订阅消费者 | 已原生 / P1 | 当前状态 | WMI COM 直接读取 root/subscription 的 __FilterToConsumerBinding / __EventConsumer / __EventFilter | 只读订阅定义；正确保留对象路径、绑定及保护的脚本内容，不创建或触发订阅。 |
| pcscope.startup.wmi-filters — WMI 永久订阅过滤器 | 已原生 / P1 | 当前状态 | WMI COM 直接读取 root/subscription 的 __FilterToConsumerBinding / __EventConsumer / __EventFilter | 只读订阅定义；正确保留对象路径、绑定及保护的脚本内容，不创建或触发订阅。 |

## 文件日志

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.usn.{drive} — USN 日志身份与断点 | 已原生 / 保留 | 文件日志 | DeviceIoControl：FSCTL_QUERY_USN_JOURNAL / FSCTL_READ_USN_JOURNAL | 需要现有 NTFS journal 和读取权限；USN 不给操作者或完整内容，不自动创建 journal。 |

## 文件系统

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.usn.volumes — 固定 NTFS 卷发现 | 已原生 / 保留 | 当前状态 | GetLogicalDrives / GetDriveType / GetVolumeInformation / GetVolumeNameForVolumeMountPoint | 当前固定 NTFS 卷范围。 |

## 虚拟化

| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |
|---|---|---|---|---|
| pcscope.virtualization.hyperv-checkpoints — 虚拟机检查点 | 待迁移 / P2 | 当前状态 | Hyper-V WMI V2：root/virtualization/v2，Msvm_* 类与关联 | Hyper-V 未安装/家庭版无提供程序为不适用；逐字段关联 VM、设置、交换机、磁盘、检查点，不因命令替换丢 Generation 等派生值。 |
| pcscope.virtualization.hyperv-disks — 虚拟机磁盘连接 | 待迁移 / P2 | 当前状态 | Hyper-V WMI V2：root/virtualization/v2，Msvm_* 类与关联 | Hyper-V 未安装/家庭版无提供程序为不适用；逐字段关联 VM、设置、交换机、磁盘、检查点，不因命令替换丢 Generation 等派生值。 |
| pcscope.virtualization.hyperv-switches — Hyper-V 虚拟交换机 | 待迁移 / P2 | 当前状态 | Hyper-V WMI V2：root/virtualization/v2，Msvm_* 类与关联 | Hyper-V 未安装/家庭版无提供程序为不适用；逐字段关联 VM、设置、交换机、磁盘、检查点，不因命令替换丢 Generation 等派生值。 |
| pcscope.virtualization.hyperv-vms — Hyper-V 虚拟机 | 待迁移 / P2 | 当前状态 | Hyper-V WMI V2：root/virtualization/v2，Msvm_* 类与关联 | Hyper-V 未安装/家庭版无提供程序为不适用；逐字段关联 VM、设置、交换机、磁盘、检查点，不因命令替换丢 Generation 等派生值。 |
