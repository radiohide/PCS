# 日志关联整改的采集项覆盖清单

2026-10-21，基线 0.2.26 / `2870d18`。配套 [实施方案](event-object-association-plan.md)。本表是完整静态范围与后续验收账目，不是全项实测通过报告。

目录来自采集代码盘点、采集定义和类型映射。所有项目执行 A 包通用规则，再执行表内工作包；工作包的专有匹配与负例见主方案。新增数据不改变本次固定审查范围。

2026-10-22 增量验收：更新历史、Defender 检测见 [0.2.30](acceptance/retained-history-0.2.30.md)，Security／Sysmon 进程历史和组成员变更见 [0.2.31](acceptance/process-history-0.2.31.md)。其余表格继续保留原审查口径；入口存在不等于所有事件模板已经验证。待环境、较早启动范围、账户／服务别名、跨来源进程关系、原始材料处置账目及 E–L 专项仍未全部完成。

## 一、目录口径

| 口径 | 数量 |
|---|---:|
| execution_units | 13 |
| execution_unit_domains | 9 |
| implemented_sources | 133 |
| implemented_collector_sources | 131 |
| catalog_rows | 224 |
| catalog_sources | 130 |
| collection_definitions | 225 |
| item_controls | 222 |
| event_channels | 23 |
| event_item_rules | 109 |
| catalog_event_rows | 109 |

224 行界面目录共 130 个来源；225 个采集定义是配置口径，不能与目录行简单相加。133 个来源包含动态/虚拟入口和采集状态。`pcscope.usn.watch-list` 是空名单状态，不应新增独立数据开关。

## 二、执行单元目录覆盖

| 执行单元 | 界面行数 | 去重来源数 |
|---|---:|---:|
| pcscope-software.exe | 1 | 1 |
| pcscope-powershell-adapter.exe | 20 | 14 |
| system | 15 | 15 |
| security | 14 | 14 |
| network | 15 | 15 |
| storage | 9 | 9 |
| startup | 8 | 8 |
| desktop | 8 | 8 |
| virtualization | 5 | 5 |
| applications | 7 | 7 |
| scoped | 9 | 9 |
| usn | 4 | 2 |
| eventlog | 109 | 23 |

## 三、全部 225 个采集定义

每一项都需要正常样本、缺字段、冲突和历史回放结果。事件编号为空表示目录未限定单个编号，不表示该提供程序的全部编号已经有解析模板。来源状态/其他事件规则也在表中。

| # | 采集项 | 配置 ID | 来源 | 规则范围 | 工作包 |
|---:|---|---|---|---|---|
| 1 | 本机软件安装登记 | `pcscope.software` | `pcscope.software` | 按来源及对象类型 | A、H |
| 2 | 操作系统版本与安装时间 | `pcscope.system.version` | `pcscope.system.version` | 按来源及对象类型 | A、L |
| 3 | 最近启动时间与时区 | `pcscope.system.boot-state` | `pcscope.system.boot-state` | 按来源及对象类型 | A、L |
| 4 | 本机用户账户 | `pcscope.identity.accounts` | `pcscope.identity.accounts` | 按来源及对象类型 | A、C |
| 5 | 本机用户组与成员 | `pcscope.identity.groups` | `pcscope.identity.groups` | 按来源及对象类型 | A、C |
| 6 | 服务运行状态 | `pcscope.startup.services` | `pcscope.startup.services` | 按来源及对象类型 | A、B、K |
| 7 | 计划任务基础配置 | `pcscope.startup.tasks` | `pcscope.startup.tasks` | 按来源及对象类型 | A、B、K |
| 8 | 卷与文件系统 | `pcscope.storage.volumes` | `pcscope.storage.volumes` | 按来源及对象类型 | A、G |
| 9 | 网络适配器 | `pcscope.network.adapters` | `pcscope.network.adapters` | 按来源及对象类型 | A、F |
| 10 | IP 地址 | `pcscope.network.addresses` | `pcscope.network.addresses` | 按来源及对象类型 | A、F |
| 11 | 虚拟化能力与平台 | `pcscope.virtualization.platform` | `pcscope.virtualization.platform` | 按来源及对象类型 | A、K |
| 12 | 即插即用设备 | `pcscope.hardware.devices` | `pcscope.hardware.devices` | 按来源及对象类型 | A、G |
| 13 | QFE 更新登记 | `pcscope.system.hotfixes` | `pcscope.system.hotfixes` | 按来源及对象类型 | A、H、L |
| 14 | 第三方预览处理程序 | `pcscope.shell.registrations.preview` | `pcscope.shell.registrations` | 按来源及对象类型 | A、J |
| 15 | 图标覆盖处理程序 | `pcscope.shell.registrations.overlay` | `pcscope.shell.registrations` | 按来源及对象类型 | A、J |
| 16 | Shell 扩展允许与阻止列表 | `pcscope.shell.registrations.approval` | `pcscope.shell.registrations` | 按来源及对象类型 | A、J |
| 17 | 桌面与此电脑命名空间 | `pcscope.shell.registrations.namespace` | `pcscope.shell.registrations` | 按来源及对象类型 | A、J |
| 18 | 右键菜单、文件关联与默认图标 | `pcscope.shell.registrations.file-association` | `pcscope.shell.registrations` | 按来源及对象类型 | A、J |
| 19 | COM 实现与扩展文件元数据 | `pcscope.shell.registrations.com` | `pcscope.shell.registrations` | 按来源及对象类型 | A、J |
| 20 | 登录启动登记 | `pcscope.startup.logon.registry` | `pcscope.startup.logon` | 按来源及对象类型 | A、B、K |
| 21 | 用户与公共启动文件夹 | `pcscope.startup.logon.folders` | `pcscope.startup.logon` | 按来源及对象类型 | A、B、K |
| 22 | 计算机身份与域信息 | `pcscope.identity.machine` | `pcscope.identity.machine` | 按来源及对象类型 | A、C |
| 23 | 时间同步配置与时区 | `pcscope.system.clock` | `pcscope.system.clock` | 按来源及对象类型 | A、L |
| 24 | Windows Update 历史 | `pcscope.system.update-history` | `pcscope.system.update-history` | 按来源及对象类型 | A、H、L |
| 25 | 待重启标志 | `pcscope.system.reboot-pending` | `pcscope.system.reboot-pending` | 按来源及对象类型 | A、L |
| 26 | 系统还原点 | `pcscope.system.restore-points` | `pcscope.system.restore-points` | 按来源及对象类型 | A、L |
| 27 | 当前用户 AppX 包 | `pcscope.software.appx-user` | `pcscope.software.appx-user` | 按来源及对象类型 | A、H |
| 28 | 预配 AppX 包 | `pcscope.software.provisioned` | `pcscope.software.provisioned` | 按来源及对象类型 | A、H |
| 29 | Windows 可选组件 | `pcscope.software.optional-features` | `pcscope.software.optional-features` | 按来源及对象类型 | A、H |
| 30 | 当前用户软件登记 | `pcscope.software.user-inventory` | `pcscope.software.user-inventory` | 按来源及对象类型 | A、H |
| 31 | 用户配置文件 | `pcscope.identity.profiles` | `pcscope.identity.profiles` | 按来源及对象类型 | A、C、E |
| 32 | 登录会话 | `pcscope.identity.logon-sessions` | `pcscope.identity.logon-sessions` | 按来源及对象类型 | A、C |
| 33 | 交互式终端会话 | `pcscope.identity.interactive-sessions` | `pcscope.identity.interactive-sessions` | 按来源及对象类型 | A、C |
| 34 | 账户权限分配 | `pcscope.identity.assigned-rights` | `pcscope.identity.assigned-rights` | 按来源及对象类型 | A、C |
| 35 | 进程清单 | `pcscope.execution.processes` | `pcscope.execution.processes` | 按来源及对象类型 | A、D |
| 36 | 内存与 CPU 采样 | `pcscope.execution.resources` | `pcscope.execution.resources` | 按来源及对象类型 | A、R |
| 37 | Defender 防护状态 | `pcscope.security.protection` | `pcscope.security.protection` | 按来源及对象类型 | A、I |
| 38 | Defender 策略与排除项 | `pcscope.security.protection-policy` | `pcscope.security.protection-policy` | 按来源及对象类型 | A、I |
| 39 | 安全产品登记 | `pcscope.security.products` | `pcscope.security.products` | 按来源及对象类型 | A、I |
| 40 | Defender 检测记录 | `pcscope.security.detections` | `pcscope.security.detections` | 按来源及对象类型 | A、I |
| 41 | Device Guard / VBS | `pcscope.security.deviceguard` | `pcscope.security.deviceguard` | 按来源及对象类型 | A、I |
| 42 | TPM 状态 | `pcscope.security.tpm` | `pcscope.security.tpm` | 按来源及对象类型 | A、G、I |
| 43 | 安全启动状态 | `pcscope.security.secureboot` | `pcscope.security.secureboot` | 按来源及对象类型 | A、I |
| 44 | CNG 密钥元数据 | `pcscope.security.key-metadata` | `pcscope.security.key-metadata` | 按来源及对象类型 | A、I |
| 45 | 计算机证书 | `pcscope.security.certificates.localmachine` | `pcscope.security.certificates.localmachine` | 按来源及对象类型 | A、I |
| 46 | 当前用户证书 | `pcscope.security.certificates.currentuser` | `pcscope.security.certificates.currentuser` | 按来源及对象类型 | A、I |
| 47 | BitLocker 卷状态 | `pcscope.security.encryption` | `pcscope.security.encryption` | 按来源及对象类型 | A、I |
| 48 | UAC、资源管理器与软件限制策略 | `pcscope.policy.configuration` | `pcscope.policy.configuration` | 按来源及对象类型 | A、I |
| 49 | SSH 公钥指纹 | `pcscope.security.ssh-public-keys` | `pcscope.security.ssh-public-keys` | 按来源及对象类型 | A、I |
| 50 | 事件通道可读性 | `pcscope.collector.event-channel-coverage` | `pcscope.collector.event-channel-coverage` | 按来源及对象类型 | A、R |
| 51 | 路由表 | `pcscope.network.routes` | `pcscope.network.routes` | 按来源及对象类型 | A、F |
| 52 | DNS 配置 | `pcscope.network.dns-configuration` | `pcscope.network.dns-configuration` | 按来源及对象类型 | A、F |
| 53 | 网络连接配置文件 | `pcscope.network.profiles` | `pcscope.network.profiles` | 按来源及对象类型 | A、C、E、F |
| 54 | TCP 连接与监听 | `pcscope.network.tcp` | `pcscope.network.tcp` | 按来源及对象类型 | A、F |
| 55 | UDP 端点 | `pcscope.network.udp` | `pcscope.network.udp` | 按来源及对象类型 | A、F |
| 56 | 防火墙配置文件 | `pcscope.network.firewall-profiles` | `pcscope.network.firewall-profiles` | 按来源及对象类型 | A、C、E、F |
| 57 | 防火墙规则及过滤条件 | `pcscope.network.firewall-rules` | `pcscope.network.firewall-rules` | 按来源及对象类型 | A、F |
| 58 | IPsec 规则 | `pcscope.network.ipsec` | `pcscope.network.ipsec` | 按来源及对象类型 | A、F |
| 59 | 代理、远程桌面与 VPN | `pcscope.network.access` | `pcscope.network.access` | 按来源及对象类型 | A、F |
| 60 | SMB 共享与访问授权 | `pcscope.network.shares` | `pcscope.network.shares` | 按来源及对象类型 | A、E、F |
| 61 | SMB 驱动器映射 | `pcscope.network.mapped-drives` | `pcscope.network.mapped-drives` | 按来源及对象类型 | A、F |
| 62 | DNS 缓存 | `pcscope.network.dns-cache` | `pcscope.network.dns-cache` | 按来源及对象类型 | A、F |
| 63 | hosts 静态映射 | `pcscope.network.hosts` | `pcscope.network.hosts` | 按来源及对象类型 | A、F |
| 64 | SMB 客户端会话 | `pcscope.network.share-sessions` | `pcscope.network.share-sessions` | 按来源及对象类型 | A、E、F |
| 65 | WinRM 配置 | `pcscope.network.winrm-configuration` | `pcscope.network.winrm-configuration` | 按来源及对象类型 | A、F |
| 66 | 磁盘清单 | `pcscope.storage.disks` | `pcscope.storage.disks` | 按来源及对象类型 | A、G |
| 67 | 分区清单 | `pcscope.storage.partitions` | `pcscope.storage.partitions` | 按来源及对象类型 | A、G |
| 68 | 驱动仓库包 | `pcscope.hardware.driver-packages` | `pcscope.hardware.driver-packages` | 按来源及对象类型 | A、G、H |
| 69 | 设备驱动绑定 | `pcscope.hardware.driver-bindings` | `pcscope.hardware.driver-bindings` | 按来源及对象类型 | A、G |
| 70 | 驱动服务 | `pcscope.hardware.driver-services` | `pcscope.hardware.driver-services` | 按来源及对象类型 | A、B、G |
| 71 | BIOS 固件 | `pcscope.hardware.firmware` | `pcscope.hardware.firmware` | 按来源及对象类型 | A、G |
| 72 | 物理磁盘健康 | `pcscope.hardware.disk-health` | `pcscope.hardware.disk-health` | 按来源及对象类型 | A、G |
| 73 | 电池状态 | `pcscope.hardware.battery` | `pcscope.hardware.battery` | 按来源及对象类型 | A、G |
| 74 | 电源计划与休眠配置 | `pcscope.power.configuration` | `pcscope.power.configuration` | 按来源及对象类型 | A、L |
| 75 | 服务详细配置 | `pcscope.startup.service-configuration` | `pcscope.startup.service-configuration` | 按来源及对象类型 | A、B、K |
| 76 | 会话与登录初始化入口 | `pcscope.startup.session-hooks` | `pcscope.startup.session-hooks` | 按来源及对象类型 | A、B、K |
| 77 | IFEO、AppInit 与 AppCert 入口 | `pcscope.startup.advanced-extensions` | `pcscope.startup.advanced-extensions` | 按来源及对象类型 | A、B、K |
| 78 | WMI 永久订阅过滤器 | `pcscope.startup.wmi-filters` | `pcscope.startup.wmi-filters` | 按来源及对象类型 | A、B、K |
| 79 | WMI 永久订阅消费者 | `pcscope.startup.wmi-consumers` | `pcscope.startup.wmi-consumers` | 按来源及对象类型 | A、B、K |
| 80 | WMI 过滤器与消费者绑定 | `pcscope.startup.wmi-bindings` | `pcscope.startup.wmi-bindings` | 按来源及对象类型 | A、B、K |
| 81 | 计划任务执行记录与条件 | `pcscope.startup.task-details` | `pcscope.startup.task-details` | 按来源及对象类型 | A、B、K |
| 82 | 环境变量与默认文件关联 | `pcscope.environment.configuration` | `pcscope.environment.configuration` | 按来源及对象类型 | A、L |
| 83 | 打印机清单 | `pcscope.desktop.printing` | `pcscope.desktop.printing` | 按来源及对象类型 | A、J |
| 84 | 打印驱动 | `pcscope.desktop.printer-drivers` | `pcscope.desktop.printer-drivers` | 按来源及对象类型 | A、G、J |
| 85 | 打印端口 | `pcscope.desktop.printer-ports` | `pcscope.desktop.printer-ports` | 按来源及对象类型 | A、J |
| 86 | 打印作业队列 | `pcscope.desktop.print-jobs` | `pcscope.desktop.print-jobs` | 按来源及对象类型 | A、J |
| 87 | 键盘、输入法与过滤驱动配置 | `pcscope.desktop.input` | `pcscope.desktop.input` | 按来源及对象类型 | A、J |
| 88 | 剪贴板设置 | `pcscope.desktop.clipboard` | `pcscope.desktop.clipboard` | 按来源及对象类型 | A、J |
| 89 | 桌面与资源管理器设置 | `pcscope.desktop.shell-settings` | `pcscope.desktop.shell-settings` | 按来源及对象类型 | A、J |
| 90 | 桌面与开始菜单快捷方式 | `pcscope.desktop.shortcuts` | `pcscope.desktop.shortcuts` | 按来源及对象类型 | A、J |
| 91 | Hyper-V 虚拟机 | `pcscope.virtualization.hyperv-vms` | `pcscope.virtualization.hyperv-vms` | 按来源及对象类型 | A、K |
| 92 | Hyper-V 虚拟交换机 | `pcscope.virtualization.hyperv-switches` | `pcscope.virtualization.hyperv-switches` | 按来源及对象类型 | A、K |
| 93 | 虚拟机磁盘连接 | `pcscope.virtualization.hyperv-disks` | `pcscope.virtualization.hyperv-disks` | 按来源及对象类型 | A、K |
| 94 | 虚拟机检查点 | `pcscope.virtualization.hyperv-checkpoints` | `pcscope.virtualization.hyperv-checkpoints` | 按来源及对象类型 | A、K |
| 95 | WSL 发行版登记 | `pcscope.virtualization.wsl-registration` | `pcscope.virtualization.wsl-registration` | 按来源及对象类型 | A、K |
| 96 | Chrome、Edge 与 Firefox 策略 | `pcscope.apps.browser-policies` | `pcscope.apps.browser-policies` | 按来源及对象类型 | A、H、I |
| 97 | VS Code 扩展清单 | `pcscope.apps.ide-extensions` | `pcscope.apps.ide-extensions` | 按来源及对象类型 | A、H |
| 98 | .NET 运行时登记 | `pcscope.apps.dotnet-runtimes` | `pcscope.apps.dotnet-runtimes` | 按来源及对象类型 | A、H |
| 99 | 全局 npm 包清单 | `pcscope.apps.npm-global-packages` | `pcscope.apps.npm-global-packages` | 按来源及对象类型 | A、H |
| 100 | OneDrive 账户与同步登记 | `pcscope.apps.onedrive-registration` | `pcscope.apps.onedrive-registration` | 按来源及对象类型 | A、H |
| 101 | Chrome / Edge 扩展 | `pcscope.apps.chromium-extensions` | `pcscope.apps.chromium-extensions` | 按来源及对象类型 | A、H |
| 102 | Firefox 扩展 | `pcscope.apps.firefox-extensions` | `pcscope.apps.firefox-extensions` | 按来源及对象类型 | A、H |
| 103 | 指定范围配置 | `pcscope.collector.scoped-configuration` | `pcscope.collector.scoped-configuration` | 按来源及对象类型 | A、R |
| 104 | 指定目录与直接子项 | `pcscope.files.directory-security` | `pcscope.files.directory-security` | 按来源及对象类型 | A、E |
| 105 | 指定文件、ACL 与 ADS | `pcscope.files.metadata-security` | `pcscope.files.metadata-security` | 按来源及对象类型 | A、E |
| 106 | 便携软件文件登记 | `pcscope.software.portable` | `pcscope.software.portable` | 按来源及对象类型 | A、H |
| 107 | 指定进程加载模块 | `pcscope.execution.modules` | `pcscope.execution.modules` | 按来源及对象类型 | A、D |
| 108 | 模块发现与读取范围 | `pcscope.execution.module-scan-status` | `pcscope.execution.module-scan-status` | 按来源及对象类型 | A、R |
| 109 | 已观察模块的磁盘文件变化 | `pcscope.execution.module-file-state` | `pcscope.execution.module-file-state` | 按来源及对象类型 | A、R |
| 110 | Mihomo 本地运行配置 | `pcscope.apps.mihomo` | `pcscope.apps.mihomo` | 按来源及对象类型 | A、H |
| 111 | Docker 容器、镜像、卷与网络 | `pcscope.virtualization.docker` | `pcscope.virtualization.docker` | 按来源及对象类型 | A、K |
| 112 | 固定 NTFS 卷发现 | `pcscope.usn.volumes` | `pcscope.usn.volumes` | 按来源及对象类型 | A、E |
| 113 | 系统与服务事件 / 服务运行事件 | `pcscope.eventlog.system.services` | `pcscope.eventlog.system` | Service Control Manager；未限定单个 ID | A、L；按事件主体执行 B–K |
| 114 | 系统与服务事件 / 系统启动与关闭 | `pcscope.eventlog.system.lifecycle` | `pcscope.eventlog.system` | Microsoft-Windows-Kernel-General；12, 13 | A、L；按事件主体执行 B–K |
| 115 | 系统与服务事件 / 电源与异常重启 | `pcscope.eventlog.system.power` | `pcscope.eventlog.system` | Microsoft-Windows-Kernel-Power；未限定单个 ID | A、L；按事件主体执行 B–K |
| 116 | 系统与服务事件 / 系统注册表维护 | `pcscope.eventlog.system.registry-maintenance` | `pcscope.eventlog.system` | Microsoft-Windows-Kernel-General；11, 15 | A、L；按事件主体执行 B–K |
| 117 | 系统与服务事件 / 系统时间同步 | `pcscope.eventlog.system.time-source` | `pcscope.eventlog.system` | Microsoft-Windows-Time-Service；37 | A、L；按事件主体执行 B–K |
| 118 | 系统与服务事件 / 来源采集状态 | `pcscope.eventlog.system.status` | `pcscope.eventlog.system` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 119 | 系统与服务事件 / 其他事件 | `pcscope.eventlog.system.other` | `pcscope.eventlog.system` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 120 | 应用程序事件 / 软件安装事件 | `pcscope.eventlog.application.installer` | `pcscope.eventlog.application` | MsiInstaller；未限定单个 ID | A、L；按事件主体执行 B–K |
| 121 | 应用程序事件 / 应用程序错误 | `pcscope.eventlog.application.errors` | `pcscope.eventlog.application` | Application Error；未限定单个 ID | A、L；按事件主体执行 B–K |
| 122 | 应用程序事件 / 应用程序无响应 | `pcscope.eventlog.application.hangs` | `pcscope.eventlog.application` | Application Hang；未限定单个 ID | A、L；按事件主体执行 B–K |
| 123 | 应用程序事件 / NVIDIA WMI 服务状态 | `pcscope.eventlog.application.nvwmi` | `pcscope.eventlog.application` | NVWMI；3 | A、L；按事件主体执行 B–K |
| 124 | 应用程序事件 / 卷影复制服务状态 | `pcscope.eventlog.application.vss` | `pcscope.eventlog.application` | VSS；8224 | A、L；按事件主体执行 B–K |
| 125 | 应用程序事件 / Windows 错误报告 | `pcscope.eventlog.application.wer` | `pcscope.eventlog.application` | Windows Error Reporting；1001 | A、L；按事件主体执行 B–K |
| 126 | 应用程序事件 / 软件保护与授权维护 | `pcscope.eventlog.application.licensing` | `pcscope.eventlog.application` | Microsoft-Windows-Security-SPP；16384, 16394 | A、L；按事件主体执行 B–K |
| 127 | 应用程序事件 / 存储优化作业 | `pcscope.eventlog.application.optimization` | `pcscope.eventlog.application` | Microsoft-Windows-Defrag；258, 264 | A、L；按事件主体执行 B–K |
| 128 | 应用程序事件 / 来源采集状态 | `pcscope.eventlog.application.status` | `pcscope.eventlog.application` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 129 | 应用程序事件 / 其他事件 | `pcscope.eventlog.application.other` | `pcscope.eventlog.application` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 130 | Windows 安全审计 / 登录、登出与会话 | `pcscope.eventlog.security.sessions` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4624, 4625, 4634, 4647, 4800, 4801, 4778, 4779 | A、L；按事件主体执行 B–K |
| 131 | Windows 安全审计 / 账户管理 | `pcscope.eventlog.security.accounts` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4720, 4722, 4725, 4726, 4738, 4740, 4767, 4781 | A、L；按事件主体执行 B–K |
| 132 | Windows 安全审计 / 用户组成员变更 | `pcscope.eventlog.security.groups` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4732, 4733 | A、L；按事件主体执行 B–K |
| 133 | Windows 安全审计 / 特殊权限分配 | `pcscope.eventlog.security.privileges` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4672 | A、L；按事件主体执行 B–K |
| 134 | Windows 安全审计 / 进程生命周期 | `pcscope.eventlog.security.processes` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4688, 4689 | A、L；按事件主体执行 B–K |
| 135 | Windows 安全审计 / 文件访问与权限 | `pcscope.eventlog.security.files` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4663, 4670 | A、L；按事件主体执行 B–K |
| 136 | Windows 安全审计 / 注册表操作 | `pcscope.eventlog.security.registry` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4657 | A、L；按事件主体执行 B–K |
| 137 | Windows 安全审计 / 服务安装 | `pcscope.eventlog.security.services` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4697 | A、L；按事件主体执行 B–K |
| 138 | Windows 安全审计 / 计划任务变更 | `pcscope.eventlog.security.tasks` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4698, 4699, 4702 | A、L；按事件主体执行 B–K |
| 139 | Windows 安全审计 / 审计策略变更 | `pcscope.eventlog.security.audit-policy` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4719 | A、L；按事件主体执行 B–K |
| 140 | Windows 安全审计 / 网络连接与共享访问 | `pcscope.eventlog.security.network` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；5140, 5156, 5157 | A、L；按事件主体执行 B–K |
| 141 | Windows 安全审计 / 审计日志清除 | `pcscope.eventlog.security.log-clear` | `pcscope.eventlog.security` | Microsoft-Windows-Eventlog；1102 | A、L；按事件主体执行 B–K |
| 142 | Windows 安全审计 / 加密密钥操作 | `pcscope.eventlog.security.crypto-keys` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；5058, 5059, 5061 | A、L；按事件主体执行 B–K |
| 143 | Windows 安全审计 / 密码组件自检与验证 | `pcscope.eventlog.security.crypto-integrity` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；5056, 5057, 5060, 5062, 6417 | A、L；按事件主体执行 B–K |
| 144 | Windows 安全审计 / 密码提供程序与上下文配置 | `pcscope.eventlog.security.crypto-configuration` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；5063, 5064, 5065, 5066, 5067, 5068, 5069, 5070 | A、L；按事件主体执行 B–K |
| 145 | Windows 安全审计 / DPAPI 数据保护与主密钥 | `pcscope.eventlog.security.data-protection` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4692, 4693, 4694, 4695 | A、L；按事件主体执行 B–K |
| 146 | Windows 安全审计 / 凭据读取与账户检查 | `pcscope.eventlog.security.credential-access` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4648, 4797, 5379, 5382 | A、L；按事件主体执行 B–K |
| 147 | Windows 安全审计 / 用户组成员查询 | `pcscope.eventlog.security.membership-queries` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4798, 4799 | A、L；按事件主体执行 B–K |
| 148 | Windows 安全审计 / WMI 与目录对象访问审计 | `pcscope.eventlog.security.object-access` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4662 | A、L；按事件主体执行 B–K |
| 149 | Windows 安全审计 / 对象访问记录规则 | `pcscope.eventlog.security.files-audit-settings` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4907 | A、L；按事件主体执行 B–K |
| 150 | Windows 安全审计 / 进程访问令牌分配 | `pcscope.eventlog.security.process-tokens` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4696 | A、L；按事件主体执行 B–K |
| 151 | Windows 安全审计 / 系统启动、时间与审计配置 | `pcscope.eventlog.security.system-audit` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；4608, 4616, 4826, 4902 | A、L；按事件主体执行 B–K |
| 152 | Windows 安全审计 / 防火墙组件生命周期 | `pcscope.eventlog.security.firewall-lifecycle` | `pcscope.eventlog.security` | Microsoft-Windows-Security-Auditing；5024, 5033 | A、L；按事件主体执行 B–K |
| 153 | Windows 安全审计 / 审计日志服务 | `pcscope.eventlog.security.audit-service` | `pcscope.eventlog.security` | Microsoft-Windows-Eventlog；1100 | A、L；按事件主体执行 B–K |
| 154 | Windows 安全审计 / 来源采集状态 | `pcscope.eventlog.security.status` | `pcscope.eventlog.security` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 155 | Windows 安全审计 / 其他事件 | `pcscope.eventlog.security.other` | `pcscope.eventlog.security` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 156 | 计划任务执行事件 / 任务触发与执行过程 | `pcscope.eventlog.microsoft-windows-taskscheduler/operational.execution` | `pcscope.eventlog.microsoft-windows-taskscheduler/operational` | Microsoft-Windows-TaskScheduler；100, 102, 107, 110, 114, 129, 200, 201, 322, 325, 328 | A、L；按事件主体执行 B–K |
| 157 | 计划任务执行事件 / 任务登记与配置变更 | `pcscope.eventlog.microsoft-windows-taskscheduler/operational.configuration` | `pcscope.eventlog.microsoft-windows-taskscheduler/operational` | Microsoft-Windows-TaskScheduler；140 | A、L；按事件主体执行 B–K |
| 158 | 计划任务执行事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-taskscheduler/operational.status` | `pcscope.eventlog.microsoft-windows-taskscheduler/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 159 | 计划任务执行事件 / 其他事件 | `pcscope.eventlog.microsoft-windows-taskscheduler/operational.other` | `pcscope.eventlog.microsoft-windows-taskscheduler/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 160 | Windows 更新事件 / 更新检查 | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.discovery` | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational` | Microsoft-Windows-WindowsUpdateClient；26 | A、L；按事件主体执行 B–K |
| 161 | Windows 更新事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.status` | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 162 | Windows 更新事件 / 其他事件 | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.other` | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 163 | Defender 防护事件 / Defender 健康与情报 | `pcscope.eventlog.microsoft-windows-windows defender/operational.health` | `pcscope.eventlog.microsoft-windows-windows defender/operational` | Microsoft-Windows-Windows Defender；1150, 1151, 2010 | A、L；按事件主体执行 B–K |
| 164 | Defender 防护事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-windows defender/operational.status` | `pcscope.eventlog.microsoft-windows-windows defender/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 165 | Defender 防护事件 / 其他事件 | `pcscope.eventlog.microsoft-windows-windows defender/operational.other` | `pcscope.eventlog.microsoft-windows-windows defender/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 166 | PowerShell 运行事件 / PowerShell 命令与脚本 | `pcscope.eventlog.microsoft-windows-powershell/operational.commands` | `pcscope.eventlog.microsoft-windows-powershell/operational` | Microsoft-Windows-PowerShell；4100, 4103, 4104 | A、L；按事件主体执行 B–K |
| 167 | PowerShell 运行事件 / PowerShell 宿主生命周期 | `pcscope.eventlog.microsoft-windows-powershell/operational.host` | `pcscope.eventlog.microsoft-windows-powershell/operational` | Microsoft-Windows-PowerShell；40961, 40962, 53504 | A、L；按事件主体执行 B–K |
| 168 | PowerShell 运行事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-powershell/operational.status` | `pcscope.eventlog.microsoft-windows-powershell/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 169 | PowerShell 运行事件 / 其他事件 | `pcscope.eventlog.microsoft-windows-powershell/operational.other` | `pcscope.eventlog.microsoft-windows-powershell/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 170 | WMI 活动事件 / WMI 提供程序与调用 | `pcscope.eventlog.microsoft-windows-wmi-activity/operational.operations` | `pcscope.eventlog.microsoft-windows-wmi-activity/operational` | Microsoft-Windows-WMI-Activity；5857, 5858 | A、L；按事件主体执行 B–K |
| 171 | WMI 活动事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-wmi-activity/operational.status` | `pcscope.eventlog.microsoft-windows-wmi-activity/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 172 | WMI 活动事件 / 其他事件 | `pcscope.eventlog.microsoft-windows-wmi-activity/operational.other` | `pcscope.eventlog.microsoft-windows-wmi-activity/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 173 | 组策略处理事件 / 组策略服务生命周期 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational.service` | `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | Microsoft-Windows-GroupPolicy；4115, 4116, 5115, 5116 | A、L；按事件主体执行 B–K |
| 174 | 组策略处理事件 / 策略处理开始与完成 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational.processing` | `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | Microsoft-Windows-GroupPolicy；4000, 4001, 4004, 4005, 8000, 8001, 8004, 8005 | A、L；按事件主体执行 B–K |
| 175 | 组策略处理事件 / 策略会话与登录通知 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational.sessions` | `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | Microsoft-Windows-GroupPolicy；4117, 5117, 5324, 5325, 5351, 6339 | A、L；按事件主体执行 B–K |
| 176 | 组策略处理事件 / 策略获取、适用与排除 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational.applicability` | `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | Microsoft-Windows-GroupPolicy；4126, 5126, 5257, 5312, 5313 | A、L；按事件主体执行 B–K |
| 177 | 组策略处理事件 / 处理模式与内部诊断 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational.diagnostics` | `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | Microsoft-Windows-GroupPolicy；5311, 5320, 5321, 5340 | A、L；按事件主体执行 B–K |
| 178 | 组策略处理事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational.status` | `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 179 | 组策略处理事件 / 其他事件 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational.other` | `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 180 | AppX 部署事件 / 应用包部署 | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.deployment` | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational` | Microsoft-Windows-AppXDeployment-Server；404, 603, 613 | A、L；按事件主体执行 B–K |
| 181 | AppX 部署事件 / 应用包部署服务状态 | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.service` | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational` | Microsoft-Windows-AppXDeployment-Server；939, 940, 941, 942 | A、L；按事件主体执行 B–K |
| 182 | AppX 部署事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.status` | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 183 | AppX 部署事件 / 其他事件 | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.other` | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 184 | 终端会话登录、断开与注销 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational.status` | `pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 185 | 终端会话登录、断开与注销 / 运行事件 | `pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational.other` | `pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 186 | 远程连接管理事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational.status` | `pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 187 | 远程连接管理事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational.other` | `pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 188 | 无线网络连接事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational.status` | `pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 189 | 无线网络连接事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational.other` | `pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 190 | 网络配置文件事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-networkprofile/operational.status` | `pcscope.eventlog.microsoft-windows-networkprofile/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 191 | 网络配置文件事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-networkprofile/operational.other` | `pcscope.eventlog.microsoft-windows-networkprofile/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 192 | 用户模式驱动事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational.status` | `pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 193 | 用户模式驱动事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational.other` | `pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 194 | 打印服务事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-printservice/operational.status` | `pcscope.eventlog.microsoft-windows-printservice/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 195 | 打印服务事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-printservice/operational.other` | `pcscope.eventlog.microsoft-windows-printservice/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 196 | BitLocker 管理事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management.status` | `pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 197 | BitLocker 管理事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management.other` | `pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 198 | 代码完整性事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-codeintegrity/operational.status` | `pcscope.eventlog.microsoft-windows-codeintegrity/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 199 | 代码完整性事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-codeintegrity/operational.other` | `pcscope.eventlog.microsoft-windows-codeintegrity/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 200 | AppLocker 程序与 DLL 事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-applocker/exe and dll.status` | `pcscope.eventlog.microsoft-windows-applocker/exe and dll` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 201 | AppLocker 程序与 DLL 事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-applocker/exe and dll.other` | `pcscope.eventlog.microsoft-windows-applocker/exe and dll` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 202 | AppLocker 安装包与脚本事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-applocker/msi and script.status` | `pcscope.eventlog.microsoft-windows-applocker/msi and script` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 203 | AppLocker 安装包与脚本事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-applocker/msi and script.other` | `pcscope.eventlog.microsoft-windows-applocker/msi and script` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 204 | Sysmon / 进程生命周期 | `pcscope.eventlog.microsoft-windows-sysmon/operational.processes` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；1, 5 | A、L；按事件主体执行 B–K |
| 205 | Sysmon / 进程访问与异常活动 | `pcscope.eventlog.microsoft-windows-sysmon/operational.process-access` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；8, 10, 25 | A、L；按事件主体执行 B–K |
| 206 | Sysmon / 网络连接 | `pcscope.eventlog.microsoft-windows-sysmon/operational.network` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；3 | A、L；按事件主体执行 B–K |
| 207 | Sysmon / DNS 查询 | `pcscope.eventlog.microsoft-windows-sysmon/operational.dns` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；22 | A、L；按事件主体执行 B–K |
| 208 | Sysmon / 文件操作 | `pcscope.eventlog.microsoft-windows-sysmon/operational.files` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；2, 11, 15, 23, 26, 27, 28, 29 | A、L；按事件主体执行 B–K |
| 209 | Sysmon / 注册表操作 | `pcscope.eventlog.microsoft-windows-sysmon/operational.registry` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；12, 13, 14 | A、L；按事件主体执行 B–K |
| 210 | Sysmon / 模块与驱动加载 | `pcscope.eventlog.microsoft-windows-sysmon/operational.modules` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；6, 7 | A、L；按事件主体执行 B–K |
| 211 | Sysmon / WMI 持久订阅 | `pcscope.eventlog.microsoft-windows-sysmon/operational.wmi` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；19, 20, 21 | A、L；按事件主体执行 B–K |
| 212 | Sysmon / 命名管道 | `pcscope.eventlog.microsoft-windows-sysmon/operational.pipes` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；17, 18 | A、L；按事件主体执行 B–K |
| 213 | Sysmon / 原始磁盘读取 | `pcscope.eventlog.microsoft-windows-sysmon/operational.raw-disk` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；9 | A、L；按事件主体执行 B–K |
| 214 | Sysmon / 剪贴板活动 | `pcscope.eventlog.microsoft-windows-sysmon/operational.clipboard` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；24 | A、L；按事件主体执行 B–K |
| 215 | Sysmon / Sysmon 服务与配置 | `pcscope.eventlog.microsoft-windows-sysmon/operational.monitor` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | Microsoft-Windows-Sysmon；4, 16, 255 | A、L；按事件主体执行 B–K |
| 216 | Sysmon / 来源采集状态 | `pcscope.eventlog.microsoft-windows-sysmon/operational.status` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 217 | Sysmon / 其他事件 | `pcscope.eventlog.microsoft-windows-sysmon/operational.other` | `pcscope.eventlog.microsoft-windows-sysmon/operational` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 218 | Hyper-V 管理事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin.status` | `pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 219 | Hyper-V 管理事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin.other` | `pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 220 | Windows 备份事件 / 来源采集状态 | `pcscope.eventlog.microsoft-windows-backup.status` | `pcscope.eventlog.microsoft-windows-backup` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 221 | Windows 备份事件 / 运行事件 | `pcscope.eventlog.microsoft-windows-backup.other` | `pcscope.eventlog.microsoft-windows-backup` | 按通道/规则；未限定单个 ID | A、L；按事件主体执行 B–K |
| 222 | 文件变更记录 | `pcscope.usn.changes` | `pcscope.usn.` | 按来源及对象类型 | A、E |
| 223 | 手动导入的事件日志 | `pcscope.eventlog.import` | `pcscope.eventlog.import.` | 按来源及对象类型 | A、L；按事件主体执行 B–K |
| 224 | 资源管理器扩展（未细分／来源状态） | `pcscope.shell.registrations` | `pcscope.shell.registrations` | 按来源及对象类型 | A、J |
| 225 | 登录启动项（未细分／来源状态） | `pcscope.startup.logon` | `pcscope.startup.logon` | 按来源及对象类型 | A、B、K |

## 四、全部 133 个来源与身份要求

身份键来自当前模型声明，是逐项验证和修正的起点，不代表当前字段映射已经完整。scope 还须按主方案补齐机器、安装、用户或启动范围；临时主体不能用缺失键相互合并。

| # | 来源 | 路线 | 类型及当前身份键 | 工作包 | 本次样本边界 |
|---:|---|---|---|---|---|
| 1 | `pcscope.apps.browser-policies` | 对象快照 | 浏览器策略 (policy.browser_policy)：scope + path + value_name | A、H、I | records 表无记录；快照另存，不据此判缺失 |
| 2 | `pcscope.apps.chromium-extensions` | 对象快照 | 浏览器扩展 (shell.browser_extension)：browser + profile + extension_id + version | A、H | 来源 records 表 105；不等于业务对象数或验收通过 |
| 3 | `pcscope.apps.dotnet-runtimes` | 对象快照 | 运行时 (software.runtime)：runtime_family + runtime_version | A、H | 来源 records 表 24；不等于业务对象数或验收通过 |
| 4 | `pcscope.apps.firefox-extensions` | 对象快照 | 浏览器扩展 (shell.browser_extension)：browser + profile + extension_id + version | A、H | records 表无记录；快照另存，不据此判缺失 |
| 5 | `pcscope.apps.ide-extensions` | 对象快照 | 编辑器扩展 (desktop.editor_extension)：editor + extension_id + version | A、H | records 表无记录；快照另存，不据此判缺失 |
| 6 | `pcscope.apps.mihomo` | 对象快照 | 代理配置 (network.proxy_configuration)：scope + path + value_name | A、H | records 表无记录；快照另存，不据此判缺失 |
| 7 | `pcscope.apps.npm-global-packages` | 对象快照 | 语言包登记 (software.language_package)：package_manager + package_name + package_version | A、H | 来源 records 表 2；不等于业务对象数或验收通过 |
| 8 | `pcscope.apps.onedrive-registration` | 对象快照 | 同步账户登记 (desktop.sync_account)：scope + native_id | A、H | records 表无记录；快照另存，不据此判缺失 |
| 9 | `pcscope.collector.event-channel-coverage` | 状态/读数 | 按事件动作决定类型或不产生对象 | A、R | 来源 records 表 2199；不等于业务对象数或验收通过 |
| 10 | `pcscope.collector.scoped-configuration` | 状态/读数 | 按事件动作决定类型或不产生对象 | A、R | 来源 records 表 2；不等于业务对象数或验收通过 |
| 11 | `pcscope.desktop.clipboard` | 对象快照 | 桌面与资源管理器配置 (desktop.shell_configuration)：scope + path | A、J | 来源 records 表 4；不等于业务对象数或验收通过 |
| 12 | `pcscope.desktop.input` | 对象快照 | 输入法配置 (desktop.input_configuration)：scope + path | A、J | 来源 records 表 1；不等于业务对象数或验收通过 |
| 13 | `pcscope.desktop.print-jobs` | 对象快照 | 打印作业 (desktop.print_job)：printer + job_id | A、J | records 表无记录；快照另存，不据此判缺失 |
| 14 | `pcscope.desktop.printer-drivers` | 对象快照 | 打印驱动 (desktop.printer_driver)：name + environment | A、G、J | 来源 records 表 6；不等于业务对象数或验收通过 |
| 15 | `pcscope.desktop.printer-ports` | 对象快照 | 打印端口 (desktop.printer_port)：scope + name | A、J | 来源 records 表 11；不等于业务对象数或验收通过 |
| 16 | `pcscope.desktop.printing` | 对象快照 | 打印机 (desktop.printer)：scope + name | A、J | 来源 records 表 4；不等于业务对象数或验收通过 |
| 17 | `pcscope.desktop.shell-settings` | 对象快照 | 桌面与资源管理器配置 (desktop.shell_configuration)：scope + path | A、J | 来源 records 表 8；不等于业务对象数或验收通过 |
| 18 | `pcscope.desktop.shortcuts` | 对象快照 | 快捷方式 (desktop.shortcut)：volume_identity + file_id | A、J | 来源 records 表 88；不等于业务对象数或验收通过 |
| 19 | `pcscope.environment.configuration` | 对象快照 | 环境变量 (policy.environment_variable)：scope + path + value_name；文件关联 (shell.file_association)：scope + hive + view + registry_path | A、L | 来源 records 表 1112；不等于业务对象数或验收通过 |
| 20 | `pcscope.eventlog.application` | 事件解析 | 按事件动作决定类型或不产生对象 | A、L；按事件主体执行 B–K | 通道有业务材料 6507；子规则未逐项回放 |
| 21 | `pcscope.eventlog.import` | 事件解析 | 按事件动作决定类型或不产生对象 | A、L；按事件主体执行 B–K | 模板/虚拟入口，需专用回放 |
| 22 | `pcscope.eventlog.microsoft-windows-applocker/exe and dll` | 事件解析 | 按事件动作决定类型或不产生对象 | A、L；按事件主体执行 B–K | 只有状态材料 2 |
| 23 | `pcscope.eventlog.microsoft-windows-applocker/msi and script` | 事件解析 | 按事件动作决定类型或不产生对象 | A、L；按事件主体执行 B–K | 只有状态材料 2 |
| 24 | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational` | 事件解析 | AppX 包 (software.appx_package)：scope + name | A、L；按事件主体执行 B–K | 通道有业务材料 6190；子规则未逐项回放 |
| 25 | `pcscope.eventlog.microsoft-windows-backup` | 事件解析 | 按事件动作决定类型或不产生对象 | A、L；按事件主体执行 B–K | 无本次记录 |
| 26 | `pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management` | 事件解析 | BitLocker 配置 (security.bitlocker_configuration)：scope + native_id | A、L；按事件主体执行 B–K | 通道有业务材料 6；子规则未逐项回放 |
| 27 | `pcscope.eventlog.microsoft-windows-codeintegrity/operational` | 事件解析 | 监控文件 (file.watched_file)：volume_identity + file_id | A、L；按事件主体执行 B–K | 通道有业务材料 1182；子规则未逐项回放 |
| 28 | `pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational` | 事件解析 | 设备 (hardware.device)：scope + native_id | A、L；按事件主体执行 B–K | 只有状态材料 2 |
| 29 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | 事件解析 | 操作系统配置与状态 (policy.os_configuration)：scope + path | A、L；按事件主体执行 B–K | 通道有业务材料 18537；子规则未逐项回放 |
| 30 | `pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin` | 事件解析 | 虚拟机 (virtualization.virtual_machine)：scope + native_id | A、L；按事件主体执行 B–K | 只有状态材料 1 |
| 31 | `pcscope.eventlog.microsoft-windows-networkprofile/operational` | 事件解析 | 网络连接配置文件 (network.connection_profile)：scope + native_id | A、L；按事件主体执行 B–K | 通道有业务材料 11109；子规则未逐项回放 |
| 32 | `pcscope.eventlog.microsoft-windows-powershell/operational` | 事件解析 | 按事件动作决定类型或不产生对象 | A、L；按事件主体执行 B–K | 通道有业务材料 5094；子规则未逐项回放 |
| 33 | `pcscope.eventlog.microsoft-windows-printservice/operational` | 事件解析 | 打印作业 (desktop.print_job)：printer + job_id | A、L；按事件主体执行 B–K | 只有状态材料 2 |
| 34 | `pcscope.eventlog.microsoft-windows-sysmon/operational` | 事件解析 | 进程 (runtime.process)：scope + pid + created_at；模块加载实例 (runtime.module_instance)：process_identity + module_path；驱动服务 (hardware.driver_service)：scope + name；网络连接与监听端点 (network.endpoint)：protocol + local_address + local_port + remote_address + remote_port + owning_pid + created_at；监控文件 (file.watched_file)：volume_identity + file_id；备用数据流 (file.alternate_data_stream)：file_identity + stream_name；操作系统配置与状态 (policy.os_configuration)：scope + path；WMI 订阅筛选器 (startup.wmi_filter)：scope + path；WMI 订阅消费者 (startup.wmi_consumer)：scope + path；WMI 订阅绑定 (startup.wmi_binding)：namespace + filter_name + consumer_name；磁盘 (hardware.disk)：scope + native_id；桌面与资源管理器配置 (desktop.shell_configuration)：scope + path | A、L；按事件主体执行 B–K | 只有状态材料 2 |
| 35 | `pcscope.eventlog.microsoft-windows-taskscheduler/operational` | 事件解析 | 计划任务 (startup.scheduled_task)：scope + path | A、L；按事件主体执行 B–K | 通道有业务材料 15703；子规则未逐项回放 |
| 36 | `pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational` | 事件解析 | 终端会话 (runtime.terminal_session)：scope + native_id | A、L；按事件主体执行 B–K | 通道有业务材料 175；子规则未逐项回放 |
| 37 | `pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational` | 事件解析 | 终端会话 (runtime.terminal_session)：scope + native_id | A、L；按事件主体执行 B–K | 只有状态材料 2 |
| 38 | `pcscope.eventlog.microsoft-windows-windows defender/operational` | 事件解析 | 防护配置与状态 (security.protection)：scope + name；监控文件 (file.watched_file)：volume_identity + file_id | A、L；按事件主体执行 B–K | 通道有业务材料 2427；子规则未逐项回放 |
| 39 | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational` | 事件解析 | 系统更新组件 (software.system_update)：scope + native_id | A、L；按事件主体执行 B–K | 通道有业务材料 546；子规则未逐项回放 |
| 40 | `pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational` | 事件解析 | 网络连接配置文件 (network.connection_profile)：scope + native_id | A、L；按事件主体执行 B–K | 通道有业务材料 816；子规则未逐项回放 |
| 41 | `pcscope.eventlog.microsoft-windows-wmi-activity/operational` | 事件解析 | WMI 订阅筛选器 (startup.wmi_filter)：scope + path；WMI 订阅消费者 (startup.wmi_consumer)：scope + path；WMI 订阅绑定 (startup.wmi_binding)：namespace + filter_name + consumer_name | A、L；按事件主体执行 B–K | 通道有业务材料 33014；子规则未逐项回放 |
| 42 | `pcscope.eventlog.security` | 事件解析 | 用户 (account.user)：scope + sid；组成员登记 (account.group_membership)：group_sid + member_sid；登录会话 (runtime.logon_session)：scope + native_id；终端会话 (runtime.terminal_session)：scope + native_id；进程 (runtime.process)：scope + pid + created_at；监控文件 (file.watched_file)：volume_identity + file_id；文件系统权限登记 (file.acl_entry)：file_identity + trustee + access_mask；操作系统配置与状态 (policy.os_configuration)：scope + path；服务 (startup.service)：scope + name；计划任务 (startup.scheduled_task)：scope + path；网络连接与监听端点 (network.endpoint)：protocol + local_address + local_port + remote_address + remote_port + owning_pid + created_at | A、L；按事件主体执行 B–K | 通道有业务材料 123591；子规则未逐项回放 |
| 43 | `pcscope.eventlog.system` | 事件解析 | 服务 (startup.service)：scope + name；操作系统配置与状态 (policy.os_configuration)：scope + path | A、L；按事件主体执行 B–K | 通道有业务材料 6988；子规则未逐项回放 |
| 44 | `pcscope.execution.module-file-state` | 状态/读数 | 监控文件 (file.watched_file)：volume_identity + file_id | A、R | 来源 records 表 43574；不等于业务对象数或验收通过 |
| 45 | `pcscope.execution.module-scan-status` | 状态/读数 | 按事件动作决定类型或不产生对象 | A、R | 来源 records 表 3254；不等于业务对象数或验收通过 |
| 46 | `pcscope.execution.modules` | 对象快照 | 模块加载实例 (runtime.module_instance)：process_identity + module_path | A、D | 来源 records 表 173977；不等于业务对象数或验收通过 |
| 47 | `pcscope.execution.processes` | 对象快照 | 进程 (runtime.process)：scope + pid + created_at | A、D | 来源 records 表 29773；不等于业务对象数或验收通过 |
| 48 | `pcscope.execution.resources` | 状态/读数 | 按事件动作决定类型或不产生对象 | A、R | 来源 records 表 21169；不等于业务对象数或验收通过 |
| 49 | `pcscope.files.directory-security` | 对象快照 | 监控目录 (file.watched_directory)：volume_identity + file_id | A、E | records 表无记录；快照另存，不据此判缺失 |
| 50 | `pcscope.files.metadata-security` | 对象快照 | 监控文件 (file.watched_file)：volume_identity + file_id；文件系统权限登记 (file.acl_entry)：file_identity + trustee + access_mask；备用数据流 (file.alternate_data_stream)：file_identity + stream_name | A、E | records 表无记录；快照另存，不据此判缺失 |
| 51 | `pcscope.hardware.battery` | 对象快照 | 电池 (hardware.battery)：scope + native_id | A、G | 来源 records 表 256；不等于业务对象数或验收通过 |
| 52 | `pcscope.hardware.devices` | 对象快照 | 设备 (hardware.device)：scope + native_id | A、G | 来源 records 表 335；不等于业务对象数或验收通过 |
| 53 | `pcscope.hardware.disk-health` | 对象快照 | 磁盘 (hardware.disk)：scope + native_id | A、G | 来源 records 表 128；不等于业务对象数或验收通过 |
| 54 | `pcscope.hardware.driver-bindings` | 对象快照 | 设备驱动绑定 (hardware.driver_binding)：device_id + driver_version | A、G | 来源 records 表 392；不等于业务对象数或验收通过 |
| 55 | `pcscope.hardware.driver-packages` | 对象快照 | 驱动包 (hardware.driver_package)：scope + native_id | A、G、H | 来源 records 表 861；不等于业务对象数或验收通过 |
| 56 | `pcscope.hardware.driver-services` | 对象快照 | 驱动服务 (hardware.driver_service)：scope + name | A、B、G | 来源 records 表 522；不等于业务对象数或验收通过 |
| 57 | `pcscope.hardware.firmware` | 对象快照 | 固件 (hardware.firmware)：scope + native_id | A、G | 来源 records 表 128；不等于业务对象数或验收通过 |
| 58 | `pcscope.identity.accounts` | 对象快照 | 用户 (account.user)：scope + sid | A、C | 来源 records 表 13；不等于业务对象数或验收通过 |
| 59 | `pcscope.identity.assigned-rights` | 对象快照 | 账户权限分配 (account.right_assignment)：account_sid + privilege | A、C | 来源 records 表 13；不等于业务对象数或验收通过 |
| 60 | `pcscope.identity.groups` | 对象快照 | 用户组 (account.group)：scope + sid；组成员登记 (account.group_membership)：group_sid + member_sid | A、C | 来源 records 表 20；不等于业务对象数或验收通过 |
| 61 | `pcscope.identity.interactive-sessions` | 对象快照 | 终端会话 (runtime.terminal_session)：scope + native_id | A、C | 来源 records 表 2；不等于业务对象数或验收通过 |
| 62 | `pcscope.identity.logon-sessions` | 对象快照 | 登录会话 (runtime.logon_session)：scope + native_id | A、C | 来源 records 表 9；不等于业务对象数或验收通过 |
| 63 | `pcscope.identity.machine` | 对象快照 | 计算机身份与域信息 (account.machine_identity)：scope + native_id | A、C | 来源 records 表 2；不等于业务对象数或验收通过 |
| 64 | `pcscope.identity.profiles` | 对象快照 | 用户配置文件 (account.user_profile)：scope + sid | A、C、E | 来源 records 表 3892；不等于业务对象数或验收通过 |
| 65 | `pcscope.network.access` | 对象快照 | 代理配置 (network.proxy_configuration)：scope + path + value_name；VPN 配置 (network.vpn_configuration)：scope + native_id；远程访问配置 (network.remote_access_configuration)：scope + path | A、F | 来源 records 表 4；不等于业务对象数或验收通过 |
| 66 | `pcscope.network.adapters` | 对象快照 | 网络适配器 (network.adapter)：scope + native_id | A、F | 来源 records 表 226；不等于业务对象数或验收通过 |
| 67 | `pcscope.network.addresses` | 对象快照 | IP 地址配置 (network.address_configuration)：interface_guid + address + prefix_length | A、F | 来源 records 表 6746；不等于业务对象数或验收通过 |
| 68 | `pcscope.network.dns-cache` | 对象快照 | DNS 缓存项 (network.dns_cache_entry)：name + record_type | A、F | 来源 records 表 2824；不等于业务对象数或验收通过 |
| 69 | `pcscope.network.dns-configuration` | 对象快照 | DNS 配置 (network.dns_configuration)：interface_guid + address_family | A、F | 来源 records 表 14；不等于业务对象数或验收通过 |
| 70 | `pcscope.network.firewall-profiles` | 对象快照 | 防火墙配置集 (network.firewall_profile)：scope + native_id | A、C、E、F | 来源 records 表 3；不等于业务对象数或验收通过 |
| 71 | `pcscope.network.firewall-rules` | 对象快照 | 防火墙规则 (network.firewall_rule)：policy_store + name | A、F | 来源 records 表 551；不等于业务对象数或验收通过 |
| 72 | `pcscope.network.hosts` | 对象快照 | hosts 静态映射 (network.hosts_entry)：address + names | A、F | records 表无记录；快照另存，不据此判缺失 |
| 73 | `pcscope.network.ipsec` | 对象快照 | IPsec 规则 (network.ipsec_rule)：scope + native_id | A、F | records 表无记录；快照另存，不据此判缺失 |
| 74 | `pcscope.network.mapped-drives` | 对象快照 | SMB 驱动器映射 (network.smb_mapping)：local_name + remote_path | A、F | records 表无记录；快照另存，不据此判缺失 |
| 75 | `pcscope.network.profiles` | 对象快照 | 网络连接配置文件 (network.connection_profile)：scope + native_id | A、C、E、F | 来源 records 表 2；不等于业务对象数或验收通过 |
| 76 | `pcscope.network.routes` | 对象快照 | 路由 (network.route)：interface_guid + destination_prefix + next_hop | A、F | 来源 records 表 5120；不等于业务对象数或验收通过 |
| 77 | `pcscope.network.share-sessions` | 对象快照 | SMB 客户端会话 (network.smb_session)：client + user + server | A、E、F | records 表无记录；快照另存，不据此判缺失 |
| 78 | `pcscope.network.shares` | 对象快照 | SMB 共享 (network.smb_share)：scope + name | A、E、F | 来源 records 表 3；不等于业务对象数或验收通过 |
| 79 | `pcscope.network.tcp` | 对象快照 | 网络连接与监听端点 (network.endpoint)：protocol + local_address + local_port + remote_address + remote_port + owning_pid + created_at | A、F | 来源 records 表 204399；不等于业务对象数或验收通过 |
| 80 | `pcscope.network.udp` | 对象快照 | 网络连接与监听端点 (network.endpoint)：protocol + local_address + local_port + remote_address + remote_port + owning_pid + created_at | A、F | 来源 records 表 74813；不等于业务对象数或验收通过 |
| 81 | `pcscope.network.winrm-configuration` | 对象快照 | 远程访问配置 (network.remote_access_configuration)：scope + path | A、F | 来源 records 表 24；不等于业务对象数或验收通过 |
| 82 | `pcscope.policy.configuration` | 对象快照 | UAC 策略 (policy.uac_policy)：scope + path + value_name；软件限制策略 (policy.software_restriction_policy)：scope + path + value_name | A、I | 来源 records 表 34；不等于业务对象数或验收通过 |
| 83 | `pcscope.power.configuration` | 对象快照 | 电源计划 (policy.power_plan)：scope + native_id | A、L | 来源 records 表 130；不等于业务对象数或验收通过 |
| 84 | `pcscope.security.certificates.currentuser` | 对象快照 | 证书登记 (security.certificate_registration)：store_scope + store_name + thumbprint | A、I | 来源 records 表 44；不等于业务对象数或验收通过 |
| 85 | `pcscope.security.certificates.localmachine` | 对象快照 | 证书登记 (security.certificate_registration)：store_scope + store_name + thumbprint | A、I | 来源 records 表 42；不等于业务对象数或验收通过 |
| 86 | `pcscope.security.detections` | 动作材料 | 防护配置与状态 (security.protection)：scope + name | A、I | records 表无记录；快照另存，不据此判缺失 |
| 87 | `pcscope.security.deviceguard` | 对象快照 | Device Guard 配置 (security.device_guard_configuration)：scope + native_id | A、I | 来源 records 表 1；不等于业务对象数或验收通过 |
| 88 | `pcscope.security.encryption` | 对象快照 | BitLocker 配置 (security.bitlocker_configuration)：scope + native_id | A、I | 来源 records 表 1；不等于业务对象数或验收通过 |
| 89 | `pcscope.security.key-metadata` | 对象快照 | 密钥元数据对象 (security.key_metadata)：scope + native_id | A、I | 来源 records 表 5；不等于业务对象数或验收通过 |
| 90 | `pcscope.security.products` | 对象快照 | 安全产品登记 (security.security_product)：scope + native_id | A、I | 来源 records 表 1；不等于业务对象数或验收通过 |
| 91 | `pcscope.security.protection` | 对象快照 | 防护配置与状态 (security.protection)：scope + name | A、I | 来源 records 表 21；不等于业务对象数或验收通过 |
| 92 | `pcscope.security.protection-policy` | 对象快照 | 防护配置与状态 (security.protection)：scope + name；防护排除项 (security.exclusion)：product + exclusion_type + value | A、I | 来源 records 表 1；不等于业务对象数或验收通过 |
| 93 | `pcscope.security.secureboot` | 对象快照 | 安全启动配置 (security.secure_boot_configuration)：scope + name | A、I | 来源 records 表 1；不等于业务对象数或验收通过 |
| 94 | `pcscope.security.ssh-public-keys` | 对象快照 | SSH 公钥登记 (security.ssh_public_key)：file_path + algorithm | A、I | records 表无记录；快照另存，不据此判缺失 |
| 95 | `pcscope.security.tpm` | 对象快照 | TPM (hardware.tpm)：scope + name | A、G、I | 来源 records 表 1；不等于业务对象数或验收通过 |
| 96 | `pcscope.shell.registrations` | 对象快照 | 预览处理程序 (shell.preview_handler)：scope + hive + view + registry_path；图标覆盖处理程序 (shell.icon_overlay_handler)：scope + hive + view + registry_path；Shell 扩展登记 (shell.shell_extension)：scope + hive + view + registry_path；命名空间扩展 (shell.namespace_extension)：scope + hive + view + registry_path；右键菜单登记 (shell.context_menu_registration)：scope + hive + view + registry_path；文件关联 (shell.file_association)：scope + hive + view + registry_path；COM 注册 (shell.com_registration)：hive + view + clsid | A、J | 来源 records 表 8181；不等于业务对象数或验收通过 |
| 97 | `pcscope.software` | 对象快照 | 软件安装登记 (software.installation_registration)：scope + hive + view + registry_path | A、H | 来源 records 表 62；不等于业务对象数或验收通过 |
| 98 | `pcscope.software.appx-user` | 对象快照 | AppX 包 (software.appx_package)：scope + name | A、H | 来源 records 表 64；不等于业务对象数或验收通过 |
| 99 | `pcscope.software.optional-features` | 对象快照 | Windows 可选组件 (software.optional_feature)：scope + name | A、H | 来源 records 表 134；不等于业务对象数或验收通过 |
| 100 | `pcscope.software.portable` | 对象快照 | 便携程序登记 (software.portable_registration)：volume_identity + file_id | A、H | records 表无记录；快照另存，不据此判缺失 |
| 101 | `pcscope.software.provisioned` | 对象快照 | 预配包 (software.provisioned_package)：scope + name | A、H | 来源 records 表 8；不等于业务对象数或验收通过 |
| 102 | `pcscope.software.user-inventory` | 对象快照 | 软件安装登记 (software.installation_registration)：scope + hive + view + registry_path | A、H | 来源 records 表 10；不等于业务对象数或验收通过 |
| 103 | `pcscope.startup.advanced-extensions` | 对象快照 | IFEO 与注入入口 (startup.ifeo_configuration)：scope + hive + view + registry_path | A、B、K | 来源 records 表 64；不等于业务对象数或验收通过 |
| 104 | `pcscope.startup.logon` | 对象快照 | 登录启动项 (startup.logon_startup_entry)：scope + path + value_name | A、B、K | 来源 records 表 29；不等于业务对象数或验收通过 |
| 105 | `pcscope.startup.service-configuration` | 对象快照 | 服务 (startup.service)：scope + name | A、B、K | 来源 records 表 1；不等于业务对象数或验收通过 |
| 106 | `pcscope.startup.services` | 对象快照 | 服务 (startup.service)：scope + name | A、B、K | 来源 records 表 6223；不等于业务对象数或验收通过 |
| 107 | `pcscope.startup.session-hooks` | 对象快照 | 登录初始化配置 (startup.logon_initialization)：scope + hive + view + registry_path | A、B、K | 来源 records 表 77；不等于业务对象数或验收通过 |
| 108 | `pcscope.startup.task-details` | 对象快照 | 计划任务 (startup.scheduled_task)：scope + path | A、B、K | 来源 records 表 6600；不等于业务对象数或验收通过 |
| 109 | `pcscope.startup.tasks` | 对象快照 | 计划任务 (startup.scheduled_task)：scope + path | A、B、K | 来源 records 表 393；不等于业务对象数或验收通过 |
| 110 | `pcscope.startup.wmi-bindings` | 对象快照 | WMI 订阅绑定 (startup.wmi_binding)：namespace + filter_name + consumer_name | A、B、K | 来源 records 表 1；不等于业务对象数或验收通过 |
| 111 | `pcscope.startup.wmi-consumers` | 对象快照 | WMI 订阅消费者 (startup.wmi_consumer)：scope + path | A、B、K | 来源 records 表 1；不等于业务对象数或验收通过 |
| 112 | `pcscope.startup.wmi-filters` | 对象快照 | WMI 订阅筛选器 (startup.wmi_filter)：scope + path | A、B、K | 来源 records 表 1；不等于业务对象数或验收通过 |
| 113 | `pcscope.storage.disks` | 对象快照 | 磁盘 (hardware.disk)：scope + native_id | A、G | 来源 records 表 128；不等于业务对象数或验收通过 |
| 114 | `pcscope.storage.partitions` | 对象快照 | 分区 (hardware.partition)：scope + native_id | A、G | 来源 records 表 131；不等于业务对象数或验收通过 |
| 115 | `pcscope.storage.volumes` | 对象快照 | 卷 (hardware.volume)：scope + native_id | A、G | 来源 records 表 3；不等于业务对象数或验收通过 |
| 116 | `pcscope.system.boot-state` | 对象快照 | 操作系统配置与状态 (policy.os_configuration)：scope + path | A、L | 来源 records 表 7；不等于业务对象数或验收通过 |
| 117 | `pcscope.system.clock` | 对象快照 | 时间同步配置 (policy.time_sync_configuration)：scope + path | A、L | 来源 records 表 4019；不等于业务对象数或验收通过 |
| 118 | `pcscope.system.hotfixes` | 对象快照 | 系统更新组件 (software.system_update)：scope + native_id | A、H、L | 来源 records 表 9；不等于业务对象数或验收通过 |
| 119 | `pcscope.system.reboot-pending` | 对象快照 | 操作系统配置与状态 (policy.os_configuration)：scope + path | A、L | 来源 records 表 10；不等于业务对象数或验收通过 |
| 120 | `pcscope.system.restore-points` | 对象快照 | 系统还原点 (policy.system_restore_point)：scope + native_id | A、L | records 表无记录；快照另存，不据此判缺失 |
| 121 | `pcscope.system.update-history` | 动作材料 | 系统更新组件 (software.system_update)：scope + native_id | A、H、L | 来源 records 表 292；不等于业务对象数或验收通过 |
| 122 | `pcscope.system.version` | 对象快照 | 操作系统配置与状态 (policy.os_configuration)：scope + path | A、L | 来源 records 表 126；不等于业务对象数或验收通过 |
| 123 | `pcscope.usn.{drive}` | 事件解析 | 监控文件 (file.watched_file)：volume_identity + file_id | A、E | 模板/虚拟入口，需专用回放 |
| 124 | `pcscope.usn.changes` | 虚拟分组 | 监控文件 (file.watched_file)：volume_identity + file_id | A、R | 模板/虚拟入口，需专用回放 |
| 125 | `pcscope.usn.volumes` | 对象快照 | 卷 (hardware.volume)：scope + native_id | A、E | records 表无记录；快照另存，不据此判缺失 |
| 126 | `pcscope.usn.watch-list` | 状态/读数 | 按事件动作决定类型或不产生对象 | A、R | 来源 records 表 1；不等于业务对象数或验收通过 |
| 127 | `pcscope.virtualization.docker` | 对象快照 | 容器 (virtualization.container)：scope + native_id；容器镜像 (virtualization.image)：image_id + repository + tag；容器卷 (virtualization.container_volume)：scope + native_id；容器网络 (virtualization.container_network)：scope + native_id | A、K | records 表无记录；快照另存，不据此判缺失 |
| 128 | `pcscope.virtualization.hyperv-checkpoints` | 对象快照 | 虚拟机检查点 (virtualization.checkpoint)：scope + native_id | A、K | 来源 records 表 2；不等于业务对象数或验收通过 |
| 129 | `pcscope.virtualization.hyperv-disks` | 对象快照 | 虚拟磁盘连接 (virtualization.disk_attachment)：vm_id + controller_type + controller_number + controller_location | A、K | 来源 records 表 2；不等于业务对象数或验收通过 |
| 130 | `pcscope.virtualization.hyperv-switches` | 对象快照 | 虚拟交换机 (virtualization.virtual_switch)：scope + native_id | A、K | 来源 records 表 2；不等于业务对象数或验收通过 |
| 131 | `pcscope.virtualization.hyperv-vms` | 对象快照 | 虚拟机 (virtualization.virtual_machine)：scope + native_id | A、K | 来源 records 表 2；不等于业务对象数或验收通过 |
| 132 | `pcscope.virtualization.platform` | 对象快照 | 虚拟化能力与平台 (virtualization.platform_capability)：scope + name | A、K | 来源 records 表 2；不等于业务对象数或验收通过 |
| 133 | `pcscope.virtualization.wsl-registration` | 对象快照 | WSL 发行版 (virtualization.wsl_distribution)：scope + native_id | A、K | records 表无记录；快照另存，不据此判缺失 |

## 五、已定位问题与环境缺口

| 来源/路径 | 本次结论 | 后续必须完成 |
|---|---|---|
| TaskScheduler 102 | 1,691 条、110 个路径共用一个临时对象；全部有 TaskName | B：字段修复、逐事件历史拆分、任务运行/定义分离 |
| Security 4688 | 42 条进程开始关联在展示被否定 | D：进程生命区间与历史主体 |
| Security 会话/账户 | 10,257 条特殊权限、207 条失败登录、9 条断开未关联 | C：启动范围与有效时间名称映射 |
| system.update-history | 149 条历史更新操作统一未关联 | H：历史主体与当前安装分离 |
| security.detections | 标记动作材料，实际函数 Skipped；本次无记录 | I：生成动作并用脱敏样本验证 |
| eventlog.import / usn.{drive} | 动态入口不能用静态来源精确记录数证明覆盖 | E/L：独立环境范围、USN 代际和导入回放 |
| 全部 99 个 object_adapter | 已核对类型及身份声明；未穷举全字段真实输出 | 按本表逐类型执行缺键、重建、名称回退和差分测试 |
| 全部事件规则 | 通道中有材料不等于规则已转换动作 | 补 provider/ID/version 处置表；待支持与跳过可追踪 |

## 六、验收登记格式

实施时为每个采集定义填入：样本出处与版本、预期动作/主体/当前状态、实际结果、证据地址、正负例、未通过原因、待环境说明。不得把“无报错”“没有事件”“入口注册成功”当作通过。

按固定水位导出的本机证据保留在 target，不提交机器专属原始日志。该附录仅提交范围、聚合数量与规则要求。
