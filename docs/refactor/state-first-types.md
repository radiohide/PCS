# 全部对象类型展示规则

本清单规定整改目标和验收门槛，**不是实现完成或实机验收通过证明**。正常状态直接表达，技术范围与缺口在本工程清单中说明；不把这些限制逐条堆到产品主界面。

[整改方案](state-first-remediation-plan.md) · [覆盖总表](state-first-coverage.md)

以下 101 类各有一条展示配置。字段是从当前目录选择的候选摘要，实施时按值状态、隐私和来源含义解释；不能只把白名单放宽。需要新字段或拆分语义的情况在“表达规则”与来源缺口中列出。

## 账户与身份

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 用户账户<br>`account.user` | 名称、是否启用、最近登录 | 账户启用状态与最近登录分开；停用事件可更新开关 | [pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.identity.accounts](state-first-sources.md#source-058) |
| 用户组<br>`account.group` | 名称、成员 | 成员清单和成员增减；空组可以是有效结果 | [pcscope.identity.groups](state-first-sources.md#source-060) |
| 用户组成员<br>`account.group_membership` | 组、成员 | 明确谁加入或退出哪个组 | [pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.identity.groups](state-first-sources.md#source-060) |
| 用户资料<br>`account.user_profile` | 配置文件路径、是否已加载、最近使用 | 用户资料是否已加载与最近使用 | [pcscope.identity.profiles](state-first-sources.md#source-064) |
| 账户权限<br>`account.right_assignment` | 权限 | 展示谁被分配什么权限；登录会话获权作为另一类事件 | [pcscope.identity.assigned-rights](state-first-sources.md#source-059) |
| 计算机身份<br>`account.machine_identity` | 计算机名称、域或工作组、角色、型号 | 计算机名称、域或工作组与角色 | [pcscope.identity.machine](state-first-sources.md#source-063) |

## 软件与组件

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 软件安装信息<br>`software.installation_registration` | 显示名称、版本、发布者 | 名称与版本；安装登记不需要强加运行状态 | [pcscope.software.user-inventory](state-first-sources.md#source-102)<br>[pcscope.software](state-first-sources.md#source-097) |
| 已安装应用<br>`software.appx_package` | 包名称、版本、状态 | 应用包版本与部署状态 | [pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational](state-first-sources.md#source-024)<br>[pcscope.software.appx-user](state-first-sources.md#source-098) |
| 预装应用<br>`software.provisioned_package` | 包名称、版本、发布者 | 提供给新用户的应用清单与版本 | [pcscope.software.provisioned](state-first-sources.md#source-101) |
| Windows 可选功能<br>`software.optional_feature` | 组件名称、状态 | 已启用、已停用、等待重启等具体状态 | [pcscope.software.optional-features](state-first-sources.md#source-099) |
| 程序运行环境<br>`software.runtime` | 运行时家族、版本 | 运行环境家族与版本，可并存多个版本 | [pcscope.apps.dotnet-runtimes](state-first-sources.md#source-003) |
| 便携程序<br>`software.portable_registration` | 文件路径、版本、公司 | 受监控目录内的程序文件、版本与变化 | [pcscope.software.portable](state-first-sources.md#source-100) |
| 系统更新<br>`software.system_update` | 更新编号、安装日期、说明 | 现存更新登记与安装操作历史分开 | [pcscope.eventlog.microsoft-windows-windowsupdateclient/operational](state-first-sources.md#source-039)<br>[pcscope.system.update-history](state-first-sources.md#source-121)<br>[pcscope.system.hotfixes](state-first-sources.md#source-118) |
| 开发工具包<br>`software.language_package` | 包管理器、包名称、版本 | 包管理器、包名称与版本 | [pcscope.apps.npm-global-packages](state-first-sources.md#source-007) |

## 运行实例

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 程序进程<br>`runtime.process` | 程序路径、进程 ID、创建时间 | 运行和退出按同一个进程实例判断；补状态字段或明确生命周期读模型 | [pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034)<br>[pcscope.execution.processes](state-first-sources.md#source-047) |
| 程序模块<br>`runtime.module_instance` | 进程名称、模块路径、发布者类别、磁盘签名检查状态 | 所属程序、加载状态与磁盘文件状态分开；时间和签名范围准确 | [pcscope.execution.modules](state-first-sources.md#source-046)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034) |
| 登录会话<br>`runtime.logon_session` | 登录类型、开始时间、认证包 | 已登录或已注销，按用户和启动周期关联 | [pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.identity.logon-sessions](state-first-sources.md#source-062) |
| 桌面会话<br>`runtime.terminal_session` | 用户、站点、状态 | 活动、断开、注销按终端会话状态解释 | [pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational](state-first-sources.md#source-036)<br>[pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational](state-first-sources.md#source-037)<br>[pcscope.identity.interactive-sessions](state-first-sources.md#source-061) |

## 自动执行入口

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 系统服务<br>`startup.service` | 显示名称、状态、启动模式 | 当前运行状态与启动方式分别显示 | [pcscope.startup.service-configuration](state-first-sources.md#source-105)<br>[pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.eventlog.system](state-first-sources.md#source-043)<br>[pcscope.startup.services](state-first-sources.md#source-106) |
| 计划任务<br>`startup.scheduled_task` | 名称、是否启用、最近结果、下次运行时间 | 是否允许执行、执行进展、最近结果分开；补执行状态承接 | [pcscope.startup.task-details](state-first-sources.md#source-108)<br>[pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.eventlog.microsoft-windows-taskscheduler/operational](state-first-sources.md#source-035)<br>[pcscope.startup.tasks](state-first-sources.md#source-109) |
| 登录启动项<br>`startup.logon_startup_entry` | 值名、登记位置 | 登录时将运行什么；补启用状态，命令细节折叠 | [pcscope.startup.logon](state-first-sources.md#source-104) |
| 登录初始化设置<br>`startup.logon_initialization` | 注册表路径、配置值 | 按已知设置解释登录入口与用途 | [pcscope.startup.session-hooks](state-first-sources.md#source-107) |
| 系统事件触发条件<br>`startup.wmi_filter` | 名称、命名空间 | 显示触发条件用途，查询表达式折叠 | [pcscope.startup.wmi-filters](state-first-sources.md#source-112)<br>[pcscope.eventlog.microsoft-windows-wmi-activity/operational](state-first-sources.md#source-041)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034) |
| 系统事件触发动作<br>`startup.wmi_consumer` | 名称、消费者类型 | 显示将执行的动作类别和程序 | [pcscope.startup.wmi-consumers](state-first-sources.md#source-111)<br>[pcscope.eventlog.microsoft-windows-wmi-activity/operational](state-first-sources.md#source-041)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034) |
| 系统事件触发关联<br>`startup.wmi_binding` | 筛选器、消费者 | 把触发条件与执行动作关联为可读句子 | [pcscope.startup.wmi-bindings](state-first-sources.md#source-110)<br>[pcscope.eventlog.microsoft-windows-wmi-activity/operational](state-first-sources.md#source-041)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034) |
| 程序启动与调试设置<br>`startup.ifeo_configuration` | 注册表路径、配置值 | 按程序和已知设置解释影响，不把未知注入入口认定恶意 | [pcscope.startup.advanced-extensions](state-first-sources.md#source-103) |

## 系统扩展与关联

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 文件预览组件<br>`shell.preview_handler` | 名称 | 用于预览何种内容及实现文件关联；无名称时按可验证登记字典说明 | [pcscope.shell.registrations](state-first-sources.md#source-096) |
| 文件图标标记组件<br>`shell.icon_overlay_handler` | 名称 | 名称与用途，加载运行情况另由运行时来源给出 | [pcscope.shell.registrations](state-first-sources.md#source-096) |
| 桌面扩展<br>`shell.shell_extension` | 名称、允许状态 | 扩展名称与允许或阻止设置 | [pcscope.shell.registrations](state-first-sources.md#source-096) |
| 资源管理器位置扩展<br>`shell.namespace_extension` | 名称 | 在桌面或此电脑增加的入口 | [pcscope.shell.registrations](state-first-sources.md#source-096) |
| 右键菜单<br>`shell.context_menu_registration` | 注册表路径、命令 | 可读菜单项及动作；完整命令折叠 | [pcscope.shell.registrations](state-first-sources.md#source-096) |
| 文件打开方式<br>`shell.file_association` | 扩展名、程序标识 | 扩展名对应的应用；内部程序标识解析为应用名称 | [pcscope.environment.configuration](state-first-sources.md#source-019)<br>[pcscope.shell.registrations](state-first-sources.md#source-096) |
| 应用组件<br>`shell.com_registration` | 登记名称、进程内实现文件、文件版本 | 组件名称、进程内实现文件与版本分别显示；独立服务器命令仅详情保留，旧混合角色比较待复核，登记不代表加载 | [pcscope.shell.registrations](state-first-sources.md#source-096) |
| 浏览器扩展<br>`shell.browser_extension` | 名称、浏览器、版本、状态 | 所属浏览器、版本和该来源实际提供的启用设置 | [pcscope.apps.chromium-extensions](state-first-sources.md#source-002)<br>[pcscope.apps.firefox-extensions](state-first-sources.md#source-004) |

## 文件与访问控制

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 监控文件<br>`file.watched_file` | 路径、是否存在、大小、最后写入 | 文件存在、修改和权限变化；摘要/签名关联另按来源 | [pcscope.execution.module-file-state](state-first-sources.md#source-044)<br>[pcscope.files.metadata-security](state-first-sources.md#source-050)<br>[pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.eventlog.microsoft-windows-windows defender/operational](state-first-sources.md#source-038)<br>[pcscope.eventlog.microsoft-windows-codeintegrity/operational](state-first-sources.md#source-027)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034)<br>[pcscope.usn.{drive}](state-first-sources.md#source-123)<br>[pcscope.usn.changes](state-first-sources.md#source-124) |
| 文件访问权限<br>`file.acl_entry` | 被授权者、访问权限、是否继承 | 谁能执行哪些文件操作；权限继承单独说明 | [pcscope.files.metadata-security](state-first-sources.md#source-050)<br>[pcscope.eventlog.security](state-first-sources.md#source-042) |
| 文件附加数据<br>`file.alternate_data_stream` | 流名称、大小 | 附加数据名称与大小，不展示正文 | [pcscope.files.metadata-security](state-first-sources.md#source-050)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034) |
| 监控文件夹<br>`file.watched_directory` | 路径、是否存在、最近写入、直接子项 | 文件夹存在、权限与直接子项变化 | [pcscope.files.directory-security](state-first-sources.md#source-049) |

## 策略与配置

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 环境变量<br>`policy.environment_variable` | 变量名、作用域、变量值 | 变量名、适用范围与值的变化；敏感值保持隐藏 | [pcscope.environment.configuration](state-first-sources.md#source-019) |
| 管理员授权设置<br>`policy.uac_policy` | 值名、值 | 按已知系统选项翻译当前授权行为 | [pcscope.policy.configuration](state-first-sources.md#source-082) |
| 程序运行限制<br>`policy.software_restriction_policy` | 值名、值 | 解释当前限制和作用范围；未知规则保留原值 | [pcscope.policy.configuration](state-first-sources.md#source-082) |
| 浏览器策略<br>`policy.browser_policy` | 浏览器、策略名、值 | 显示具体策略和当前设置 | [pcscope.apps.browser-policies](state-first-sources.md#source-001) |
| 时间同步设置<br>`policy.time_sync_configuration` | 时区、配置值 | 时区和同步设置；同步成功事件更新同步结果而非配置 | [pcscope.system.clock](state-first-sources.md#source-117) |
| 电源计划<br>`policy.power_plan` | 计划名称、是否激活 | 平衡等方案名；正在使用或未在使用；休眠配置另建读模型 | [pcscope.power.configuration](state-first-sources.md#source-083) |
| 操作系统<br>`policy.os_configuration` | 系统名称、版本、最近启动时间、待重启标志 | 版本、启动时间、重启需求按对应记录汇总 | [pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.eventlog.system](state-first-sources.md#source-043)<br>[pcscope.eventlog.microsoft-windows-grouppolicy/operational](state-first-sources.md#source-029)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034)<br>[pcscope.system.reboot-pending](state-first-sources.md#source-119)<br>[pcscope.system.boot-state](state-first-sources.md#source-116)<br>[pcscope.system.version](state-first-sources.md#source-122) |
| 系统还原点<br>`policy.system_restore_point` | 说明、建立时间、类型 | 还原点名称、时间与类型 | [pcscope.system.restore-points](state-first-sources.md#source-120) |

## 网络与远程访问

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 网卡<br>`network.adapter` | 名称、状态、速率 | 连接状态与链路速率，能否上网由连接性来源说明 | [pcscope.network.adapters](state-first-sources.md#source-066) |
| IP 地址<br>`network.address_configuration` | 接口、地址、前缀长度、状态 | 所属网卡、地址及地址可用状态 | [pcscope.network.addresses](state-first-sources.md#source-067) |
| 网络路由<br>`network.route` | 目的前缀、下一跳、接口、度量 | 到哪里、经哪个网关/网卡，优先级清楚 | [pcscope.network.routes](state-first-sources.md#source-076) |
| 域名解析设置<br>`network.dns_configuration` | 接口、DNS 服务器、搜索列表 | 网卡、解析服务器和搜索域 | [pcscope.network.dns-configuration](state-first-sources.md#source-069) |
| 域名解析缓存<br>`network.dns_cache_entry` | 名称、记录类型、解析值 | 名称解析到何处、记录类型，不推出网站可访问 | [pcscope.network.dns-cache](state-first-sources.md#source-068) |
| 本机域名映射<br>`network.hosts_entry` | 名称、地址 | 哪些名称固定指向哪个地址 | [pcscope.network.hosts](state-first-sources.md#source-072) |
| 代理设置<br>`network.proxy_configuration` | 是否启用、代理服务器、作用域 | 是否启用及作用范围，不能以配置存在证明代理可用 | [pcscope.apps.mihomo](state-first-sources.md#source-006)<br>[pcscope.network.access](state-first-sources.md#source-065) |
| VPN 连接<br>`network.vpn_configuration` | 连接名称、状态、服务器 | 连接名称、连接状态与目标服务器 | [pcscope.network.access](state-first-sources.md#source-065) |
| 远程访问设置<br>`network.remote_access_configuration` | 服务、配置值 | 远程访问功能的允许设置与实际会话分开 | [pcscope.network.access](state-first-sources.md#source-065)<br>[pcscope.network.winrm-configuration](state-first-sources.md#source-081) |
| 防火墙设置<br>`network.firewall_profile` | 配置集名称、是否启用、入站动作、出站动作 | 公用、专用或域配置的开关与默认处理 | [pcscope.network.firewall-profiles](state-first-sources.md#source-070) |
| 防火墙规则<br>`network.firewall_rule` | 名称、是否启用、方向、动作 | 规则是否启用、方向及允许/阻止动作 | [pcscope.network.firewall-rules](state-first-sources.md#source-071) |
| 安全连接规则<br>`network.ipsec_rule` | 名称、安全模式、策略来源 | 规则名称、安全要求及策略来源 | [pcscope.network.ipsec](state-first-sources.md#source-073) |
| 网络连接<br>`network.endpoint` | 协议、状态、本地地址、远端地址 | TCP 连接/监听与 UDP 绑定按不同语义显示；端口作为组合属性 | [pcscope.eventlog.security](state-first-sources.md#source-042)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034)<br>[pcscope.network.tcp](state-first-sources.md#source-079)<br>[pcscope.network.udp](state-first-sources.md#source-080) |
| 共享文件夹<br>`network.smb_share` | 共享名称、路径、共享权限 | 共享名称、目录与访问许可 | [pcscope.network.shares](state-first-sources.md#source-078) |
| 网络驱动器<br>`network.smb_mapping` | 本地盘符、远端路径、状态 | 盘符连接到何处及连接状态 | [pcscope.network.mapped-drives](state-first-sources.md#source-074) |
| 文件共享会话<br>`network.smb_session` | 客户端、用户、打开数、是否加密 | 用户、客户端、打开项目数及传输加密 | [pcscope.network.share-sessions](state-first-sources.md#source-077) |
| 连接网络<br>`network.connection_profile` | 网络名称、类别、IPv4 连通性、IPv6 连通性 | 网络名称、类别与联网情况分别展示 | [pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational](state-first-sources.md#source-040)<br>[pcscope.eventlog.microsoft-windows-networkprofile/operational](state-first-sources.md#source-031)<br>[pcscope.network.profiles](state-first-sources.md#source-075) |

## 安全与信任

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 证书<br>`security.certificate_registration` | 主题、颁发者、失效时间、存储区 | 证书用途线索、颁发者、有效期和存储位置；链信任检查另列能力 | [pcscope.security.certificates.currentuser](state-first-sources.md#source-084)<br>[pcscope.security.certificates.localmachine](state-first-sources.md#source-085) |
| 密钥信息<br>`security.key_metadata` | 密钥名称、提供程序、算法 | 名称、提供程序与已取得的算法，材料不导出 | [pcscope.security.key-metadata](state-first-sources.md#source-089) |
| Windows 防护<br>`security.protection` | 实时防护、防篡改、最近扫描时间 | 先拆配置允许与实际运行；实际状态、防篡改、最近扫描各有来源 | [pcscope.security.detections](state-first-sources.md#source-086)<br>[pcscope.security.protection](state-first-sources.md#source-091)<br>[pcscope.security.protection-policy](state-first-sources.md#source-092)<br>[pcscope.eventlog.microsoft-windows-windows defender/operational](state-first-sources.md#source-038) |
| 防护排除项<br>`security.exclusion` | 排除类型、排除值、产品 | 哪些文件、进程或扩展名被排除 | [pcscope.security.protection-policy](state-first-sources.md#source-092) |
| 安全软件<br>`security.security_product` | 产品名称、产品状态码 | 安全中心登记与有依据的产品状态码解释 | [pcscope.security.products](state-first-sources.md#source-090) |
| 磁盘加密<br>`security.bitlocker_configuration` | 装载点、卷状态、保护状态 | 是否加密、保护是否启用、锁定状态分别承接 | [pcscope.security.encryption](state-first-sources.md#source-088)<br>[pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management](state-first-sources.md#source-026) |
| SSH 公钥<br>`security.ssh_public_key` | 文件路径、算法、指纹 | 文件、算法与指纹；使用情况需要独立来源 | [pcscope.security.ssh-public-keys](state-first-sources.md#source-094) |
| 系统安全隔离<br>`security.device_guard_configuration` | VBS 状态、已配置服务、运行中服务 | 支持、已配置与实际运行分别解释 | [pcscope.security.deviceguard](state-first-sources.md#source-087) |
| 安全启动<br>`security.secure_boot_configuration` | 是否启用 | 已开启或已关闭；不支持和读取失败属于采集说明 | [pcscope.security.secureboot](state-first-sources.md#source-093) |

## 硬件与驱动

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 硬件设备<br>`hardware.device` | 类别、状态、配置错误码、制造商 | 设备名称与工作状态，配置错误说明具体影响 | [pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational](state-first-sources.md#source-028)<br>[pcscope.hardware.devices](state-first-sources.md#source-052) |
| 设备驱动<br>`hardware.driver_binding` | 设备、驱动版本、提供方、签名状态 | 设备正在绑定的驱动版本、提供方与签名信息 | [pcscope.hardware.driver-bindings](state-first-sources.md#source-054) |
| 驱动服务<br>`hardware.driver_service` | 服务名称、状态、启动模式 | 驱动运行状态与启动方式分别解释 | [pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034)<br>[pcscope.hardware.driver-services](state-first-sources.md#source-056) |
| 磁盘<br>`hardware.disk` | 磁盘号、容量、健康状态、运行状态 | 磁盘容量、健康与运行状态；健康来源关联要有稳定键 | [pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034)<br>[pcscope.hardware.disk-health](state-first-sources.md#source-053)<br>[pcscope.storage.disks](state-first-sources.md#source-113) |
| 磁盘分区<br>`hardware.partition` | 磁盘号、分区号、大小、访问路径 | 所属磁盘、分区、容量及盘符/挂载点 | [pcscope.storage.partitions](state-first-sources.md#source-114) |
| 存储卷<br>`hardware.volume` | 盘符、文件系统、容量 | 卷名称、盘符、文件系统和容量 | [pcscope.storage.volumes](state-first-sources.md#source-115)<br>[pcscope.usn.volumes](state-first-sources.md#source-125) |
| 驱动包<br>`hardware.driver_package` | 包名称、提供方、版本 | 驱动仓库包及版本；与绑定设备、加载状态分开 | [pcscope.hardware.driver-packages](state-first-sources.md#source-055) |
| 固件<br>`hardware.firmware` | 制造商、BIOS 版本、发布日期 | 固件版本和发布日期 | [pcscope.hardware.firmware](state-first-sources.md#source-057) |
| 安全芯片<br>`hardware.tpm` | 是否存在、是否就绪、是否启用、是否锁定 | 是否存在、是否就绪和是否锁定 | [pcscope.security.tpm](state-first-sources.md#source-095) |
| 电池<br>`hardware.battery` | 名称、余量、估算时间 | 剩余电量和可用的续航估计，特殊未知值不换算成巨量分钟 | [pcscope.hardware.battery](state-first-sources.md#source-051) |

## 虚拟环境

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 虚拟机<br>`virtualization.virtual_machine` | 名称、状态、处理器配置、内存配置 | 运行、暂停、关闭等状态与配置分别解释 | [pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin](state-first-sources.md#source-030)<br>[pcscope.virtualization.hyperv-vms](state-first-sources.md#source-131) |
| 虚拟交换机<br>`virtualization.virtual_switch` | 名称、类型、关联网卡 | 交换机类型及关联网卡 | [pcscope.virtualization.hyperv-switches](state-first-sources.md#source-130) |
| 虚拟机磁盘<br>`virtualization.disk_attachment` | 虚拟机、磁盘路径、控制器位置 | 哪台虚拟机接入哪个虚拟磁盘 | [pcscope.virtualization.hyperv-disks](state-first-sources.md#source-129) |
| 虚拟机检查点<br>`virtualization.checkpoint` | 名称、建立时间、类型 | 名称、建立时间与类型 | [pcscope.virtualization.hyperv-checkpoints](state-first-sources.md#source-128) |
| Linux 子系统<br>`virtualization.wsl_distribution` | 发行版名称、版本、位置 | 已登记发行版及版本；当前运行状态另列未采能力 | [pcscope.virtualization.wsl-registration](state-first-sources.md#source-133) |
| 容器<br>`virtualization.container` | 名称、镜像、状态、运行状态 | 容器名称、镜像、运行状态与健康说明按原字段解释 | [pcscope.virtualization.docker](state-first-sources.md#source-127) |
| 容器镜像<br>`virtualization.image` | 仓库、标签 | 仓库、标签与镜像存在情况 | [pcscope.virtualization.docker](state-first-sources.md#source-127) |
| 容器数据卷<br>`virtualization.container_volume` | 卷名称、驱动 | 数据卷名称与存储方式 | [pcscope.virtualization.docker](state-first-sources.md#source-127) |
| 容器网络<br>`virtualization.container_network` | 名称、驱动、作用域 | 网络名称、驱动和范围 | [pcscope.virtualization.docker](state-first-sources.md#source-127) |
| 虚拟化支持<br>`virtualization.platform_capability` | 虚拟化监控程序存在、固件虚拟化、二级地址转换 | 硬件支持、固件设置和当前虚拟化平台分别显示 | [pcscope.virtualization.platform](state-first-sources.md#source-132) |

## 桌面与应用集成

| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |
| --- | --- | --- | --- |
| 打印机<br>`desktop.printer` | 名称、状态、是否共享 | 打印机名称、状态和共享设置 | [pcscope.desktop.printing](state-first-sources.md#source-016) |
| 打印连接<br>`desktop.printer_port` | 端口名称、端口类型、主机地址 | 打印连接方式与目标 | [pcscope.desktop.printer-ports](state-first-sources.md#source-015) |
| 打印驱动<br>`desktop.printer_driver` | 名称、环境、制造商 | 驱动名称、环境与制造商 | [pcscope.desktop.printer-drivers](state-first-sources.md#source-014) |
| 打印任务<br>`desktop.print_job` | 打印机、状态、页数、提交时间 | 在哪台打印机、排队或打印状态及提交时间 | [pcscope.desktop.print-jobs](state-first-sources.md#source-013)<br>[pcscope.eventlog.microsoft-windows-printservice/operational](state-first-sources.md#source-033) |
| 输入法设置<br>`desktop.input_configuration` | 布局文件、输入法文件 | 输入法和键盘布局名称，过滤驱动细节折叠 | [pcscope.desktop.input](state-first-sources.md#source-012) |
| 桌面设置<br>`desktop.shell_configuration` | 项名、值 | 具体设置名称、当前值及用途 | [pcscope.desktop.clipboard](state-first-sources.md#source-011)<br>[pcscope.desktop.shell-settings](state-first-sources.md#source-017)<br>[pcscope.eventlog.microsoft-windows-sysmon/operational](state-first-sources.md#source-034) |
| 快捷方式<br>`desktop.shortcut` | 路径、目标、热键 | 快捷方式名称、指向应用与快捷键 | [pcscope.desktop.shortcuts](state-first-sources.md#source-018) |
| 编辑器扩展<br>`desktop.editor_extension` | 名称、编辑器、版本、发布者 | 所属编辑器、扩展名称和版本 | [pcscope.apps.ide-extensions](state-first-sources.md#source-005) |
| 同步账户<br>`desktop.sync_account` | 同步提供程序、账户、挂载点 | 同步账户和目录；实际同步进度单独列未采能力 | [pcscope.apps.onedrive-registration](state-first-sources.md#source-008) |

