# 全部对象的成对变化审查

[总评与整改顺序](collection-lifecycle-audit.md) · [101 类对象](collection-lifecycle-types.md) · [133 个来源](collection-lifecycle-sources.md) · [225 个采集定义](collection-lifecycle-items.md) · [可核对数据](collection-lifecycle-audit.json)

本表逐项审查当前代码与来源，**覆盖不等于功能已完整或实机通过**。新增、离开清单、状态切换、操作开始/结果分别核对；明确缺口见总评。

| 对象类型 | 应检查的过程 | 当前来源能提供什么 | 身份与判定边界 | 缺口 |
|---|---|---|---|---|
| 用户<br>`account.user` | 创建 ↔ 删除；启用 ↔ 停用；锁定 ↔ 解锁；改名 | Security 有对应分类；原生账户表提供当前状态 | 账户 SID；名称变化保持身份 | G01, G02, G07 |
| 用户组<br>`account.group` | 建立 ↔ 删除；改名 | 组清单可差分；当前 Security 专项仅成员加入/移除，未分类组创建/删除 | 组 SID；组本身与成员登记分开 | G01, G02, G07 |
| 组成员登记<br>`account.group_membership` | 加入组 ↔ 移出组 | Security 4732/4733 两端；原生组成员子表更新当前状态 | 组 SID＋成员 SID；完整子表增减已补行动，旧材料重放另行验收 | G03, G07 |
| 用户配置文件<br>`account.user_profile` | 登记加入 ↔ 移除；加载 ↔ 卸载 | 原生配置文件清单和 loaded 字段 | 账户 SID；清单移除和配置目录删除分开 | G01, G02 |
| 账户权限分配<br>`account.right_assignment` | 授予 ↔ 撤销 | 原生 LSA 权限子表可更新当前分配；4672 是登录令牌权限 | 账户 SID＋权限；尚无长期分配事件配对 | G03, G07 |
| 计算机身份与域信息<br>`account.machine_identity` | 名称、域/工作组、角色变化 | 原生单机配置字段对比 | 同一安装作用域；无新增/删除过程要求 | G06 |
| 软件安装登记<br>`software.installation_registration` | 安装登记加入 ↔ 移除；版本变化 | 机器/用户原生卸载登记；MsiInstaller 原始日志可补充过程 | hive＋视图＋路径；MSI 成功/失败/卸载未建统一过程 | G01, G02, G07, G09 |
| AppX 包<br>`software.appx_package` | 应用包登记加入 ↔ 移除；版本替换 | 当前用户原生包清单；部署日志已有开始/失败/摘要说明 | 用户＋包全名；部署服务启停与包安装分开 | G01, G02, G07, G09 |
| 预配包<br>`software.provisioned_package` | 预装登记加入 ↔ 移除；版本替换 | 原生映像预配清单 | 本机＋包全名；当前用户应用安装是另一层 | G01, G02, G09 |
| Windows 可选组件<br>`software.optional_feature` | 启用 ↔ 禁用；等待重启 ↔ 完成 | 原生组件状态字段对比 | 组件名；已有对象切换状态，不必创建新对象 | G06, G09 |
| 运行时<br>`software.runtime` | 版本登记加入 ↔ 移除 | 原生 .NET 注册表展开为多个运行时对象 | 家族＋版本＋注册表视图；同父键内增减未完成行动对 | G03, G09 |
| 便携程序登记<br>`software.portable_registration` | 文件登记加入 ↔ 移除；版本变化 | 仅配置的便携程序根目录，原生文件元数据 | 卷/文件身份；无需解释为安装/卸载 | G01, G02, G05 |
| 系统更新组件<br>`software.system_update` | 安装 ↔ 卸载；开始 → 成功/失败/中止 | 原生 WUA 历史保留 operation/result；QFE 是当前清单子集 | 更新 ID＋修订；历史操作不并入当前安装清单 | G07, G09 |
| 语言包登记<br>`software.language_package` | 包加入 ↔ 移除；版本替换 | 标准全局 npm 目录原生读取清单 | 管理器＋包名＋版本；自定义 prefix 不覆盖 | G01, G02, G09 |
| 进程<br>`runtime.process` | 启动 ↔ 退出 | 原生进程清单；Security 4688/4689 有两端；0.2.14 已接 Sysmon 1/5 指定版本，原生实例关联仍待补（G12） | PID＋创建时间/可靠 ProcessGuid；短进程需日志 | G01, G04, G07, G12 |
| 模块加载实例<br>`runtime.module_instance` | 加载 ↔ 卸载；所属进程退出 | 指定进程原生模块清单已有再现行动；0.2.14 已接 Sysmon 7 指定版本，无卸载对应分类，跨来源实例关联待补（G12） | 进程实例＋模块路径；同一进程卸载后重载已有再现行动，Sysmon 跨来源关联待补 | G01, G02, G04, G07, G12 |
| 登录会话<br>`runtime.logon_session` | 登录 ↔ 注销；注销请求 → 完成 | 原生 LSA 会话；Security 4624/4634 两端，4647 是请求 | 登录 ID＋启动周期；缺启动周期不能跨重启配对 | G01, G04, G07 |
| 终端会话<br>`runtime.terminal_session` | 登录 ↔ 注销；连接 ↔ 断开；锁定 ↔ 解锁 | 原生 WTS 状态；Security 与 LSM 的明确状态规则 | 会话 ID＋范围/代际；断开后会话可能继续存在 | G01, G02, G07 |
| 服务<br>`startup.service` | 服务登记加入 ↔ 移除；运行 ↔ 停止；启动失败 | 原生服务清单/配置；Security 4697 安装；SCM 7036 可解码状态 | 稳定服务名；显示名仅唯一匹配；未建服务卸载事件行动 | G01, G02, G07 |
| 计划任务<br>`startup.scheduled_task` | 任务创建 ↔ 删除；启用 ↔ 禁用；每次运行开始 → 结束/失败 | 原生基础/详细配置；Security 4698/4699；TaskScheduler 100/102 | 完整任务路径＋每次运行实例；同任务并行运行尚未组装过程 | G01, G02, G07 |
| 登录启动项<br>`startup.logon_startup_entry` | 启动登记加入 ↔ 移除；命令变更 | 原生 Run/RunOnce/启动目录等配置 | 范围＋登记位置＋值名；父键内增减另查 | G01, G02, G03 |
| 登录初始化配置<br>`startup.logon_initialization` | 登录初始化配置设置 ↔ 清除；值变化 | 原生 Winlogon/会话入口注册表 | 注册表视图＋路径；配置读取不表示本次执行 | G01, G02, G03 |
| WMI 订阅筛选器<br>`startup.wmi_filter` | 订阅筛选器登记加入 ↔ 移除 | 原生 WMI 配置清单；Sysmon 19 是活动记录 | 命名空间＋原生路径；需解码 Operation 才能区分事件动作 | G01, G02, G07, G12 |
| WMI 订阅消费者<br>`startup.wmi_consumer` | 消费者登记加入 ↔ 移除 | 原生 WMI 配置清单；Sysmon 20 是活动记录 | 命名空间＋原生路径；脚本/命令正文保护 | G01, G02, G07, G12 |
| WMI 订阅绑定<br>`startup.wmi_binding` | 订阅绑定加入 ↔ 解除 | 原生 WMI 关联清单；Sysmon 21 是活动记录 | 筛选器＋消费者路径；不是订阅实际触发 | G01, G02, G07, G12 |
| IFEO 与注入入口<br>`startup.ifeo_configuration` | 调试/注入入口设置 ↔ 清除；启用 ↔ 停用 | 原生指定 IFEO/AppInit/AppCert 配置 | 视图＋路径＋配置项；设置存在不表示已注入 | G01, G02, G03 |
| 预览处理程序<br>`shell.preview_handler` | 预览组件登记加入 ↔ 移除；目标变更 | 原生注册表登记 | CLSID＋范围/视图；加载执行由模块采集另证 | G01, G02 |
| 图标覆盖处理程序<br>`shell.icon_overlay_handler` | 图标标记登记加入 ↔ 移除 | 原生注册表登记 | 登记路径/组件标识；没有运行时调用对 | G01, G02 |
| Shell 扩展登记<br>`shell.shell_extension` | 桌面扩展登记加入 ↔ 移除；目标变更 | 原生注册表登记 | 范围＋视图＋登记路径；注册与加载分层 | G01, G02 |
| 命名空间扩展<br>`shell.namespace_extension` | 命名空间登记加入 ↔ 移除 | 原生注册表登记 | 范围＋视图＋登记路径 | G01, G02 |
| 右键菜单登记<br>`shell.context_menu_registration` | 右键菜单登记加入 ↔ 移除；命令变化 | 原生注册表登记 | 登记位置＋值名/组件标识 | G01, G02 |
| 文件关联<br>`shell.file_association` | 文件关联设置 ↔ 清除；默认程序替换 | 原生关联登记，多个来源可描述同一对象 | 扩展名/ProgID＋范围＋视图；新增来源不是新关联 | G01, G02, G03 |
| COM 注册<br>`shell.com_registration` | 组件登记加入 ↔ 移除；组件服务器路径变化 | 原生 COM 根/进程内/进程外服务器登记 | 同 CLSID 多角色；单角色移除不删除整个组件 | G01, G02, G10 |
| 浏览器扩展<br>`shell.browser_extension` | 扩展目录登记加入 ↔ 移除；启用 ↔ 停用（有字段时） | 原生 Chromium/Firefox 清单；Chromium 目录不提供可靠启用状态 | 浏览器配置文件＋扩展 ID＋版本；版本目录替换需归并 | G01, G02, G09 |
| 监控文件<br>`file.watched_file` | 路径出现 ↔ 不存在；创建 ↔ 删除；改名；内容/权限变化 | 指定文件原生读取、模块文件自动名单；USN 有通用文件变更行动；0.2.14 已接 Sysmon 11/23/26 指定版本的操作历史，缺 FileId 时不关联当前文件（G12） | 卷＋文件 ID，路径另跟踪；无法读取与明确不存在分开 | G01, G02, G03, G05, G07, G12 |
| 文件系统权限登记<br>`file.acl_entry` | 权限项加入 ↔ 移除；允许/拒绝/继承变化 | 原生文件元数据内展开 DACL；Security 4670 权限审计 | 文件身份＋ACE；父文件内权限增减无完整快照行动对 | G03, G05, G07 |
| 备用数据流<br>`file.alternate_data_stream` | 文件流出现 ↔ 移除；大小/内容摘要变化 | 指定文件内 ADS 清单；Sysmon 15 有创建日志，未接生产对象行动，且无删除对应分类 | 文件身份＋流名；父文件内增减须对子项差分 | G03, G05, G07, G12 |
| 监控目录<br>`file.watched_directory` | 目录路径出现 ↔ 不存在；直接子项加入 ↔ 移除 | 原生配置目录及直接子项快照，非递归 | 目录身份＋配置路径；子项目前聚合在父记录 | G01, G02, G03, G05 |
| 环境变量<br>`policy.environment_variable` | 变量设置 ↔ 清除；值变化 | 原生机器/当前用户环境表 | 范围＋变量名；父键值增减需要子项行动 | G03, G06 |
| UAC 策略<br>`policy.uac_policy` | 策略设置 ↔ 清除；值切换 | 原生策略键/值 | 范围＋视图＋键值；未配置、读取失败、关闭分开 | G03, G06 |
| 软件限制策略<br>`policy.software_restriction_policy` | 规则加入 ↔ 移除；策略启用 ↔ 停用 | 原生策略快照 | 作用范围＋规则登记身份；有效访问结果不推测 | G01, G02, G03, G06 |
| 浏览器策略<br>`policy.browser_policy` | 策略设置 ↔ 清除；值变化 | 原生浏览器策略键逐值展开 | 浏览器＋作用范围＋视图＋键值；父键内增减有缺口 | G03, G06 |
| 时间同步配置<br>`policy.time_sync_configuration` | 同步配置/时区变化；同步状态恢复 | 原生配置；Time-Service 37 补时间源状态 | 本机配置；配置不等于每次同步过程 | G06, G07 |
| 电源计划<br>`policy.power_plan` | 计划登记加入 ↔ 移除；当前计划 A → B | 原生电源计划/激活配置 | 计划 GUID；计划名称是对象，是否启用是状态 | G01, G02, G06 |
| 操作系统配置与状态<br>`policy.os_configuration` | 系统版本/待重启/启动状态变化；开机 ↔ 关机 | 原生配置；System 生命周期/电源事件 | 本机单例；异常重启不能补造正常关机 | G06, G07, G12 |
| 系统还原点<br>`policy.system_restore_point` | 还原点出现 ↔ 不再保留 | 原生还原点元数据 | 序列号/原生标识；过期清理与用户删除不强行归因 | G01, G02 |
| 网络适配器<br>`network.adapter` | 检测到 ↔ 不再检测到；启用 ↔ 停用；链路通 ↔ 断 | 原生适配器含隐藏项；状态字段对比 | 接口 GUID；移除设备与关闭网卡分开 | G01, G02, G06 |
| IP 地址配置<br>`network.address_configuration` | 地址加入 ↔ 移除；可用状态变化 | 原生接口地址表 | 接口＋地址；DHCP 更新是配置状态变化 | G01, G02, G06 |
| 路由<br>`network.route` | 路由加入 ↔ 移除；下一跳/度量变化 | 原生路由表 | 接口＋前缀＋策略范围；路由轮换可重复出现 | G01, G02 |
| DNS 配置<br>`network.dns_configuration` | DNS 服务器/搜索配置设置 ↔ 清除；值替换 | 原生接口 DNS 配置 | 接口范围；不当作 DNS 查询成功/失败 | G03, G06 |
| DNS 缓存项<br>`network.dns_cache_entry` | 缓存加入 ↔ 到期/清除后不再列出 | 原生 DNS 缓存；Sysmon 22 是查询事件 | 名称＋类型＋数据等缓存键；查询和缓存不是一对 | G01, G02, G04, G07 |
| hosts 静态映射<br>`network.hosts_entry` | 静态映射加入 ↔ 移除 | 原生解析有效 hosts 映射 | 地址＋名称；注释不进入映射对象 | G01, G02 |
| 代理配置<br>`network.proxy_configuration` | 代理启用 ↔ 停用；配置替换 | 当前用户/机器指定原生配置，Mihomo 显式范围 | 配置范围；Mihomo 运行配置与系统代理分层 | G03, G06 |
| VPN 配置<br>`network.vpn_configuration` | VPN 登记加入 ↔ 移除；连接 ↔ 断开（需运行来源） | 原生电话簿/配置登记为主 | 用户/配置名称；当前未有完整实时 VPN 会话过程 | G01, G02, G03, G07 |
| 远程访问配置<br>`network.remote_access_configuration` | 远程访问配置启用 ↔ 停用；值变化 | 原生 RDP/WinRM 指定配置 | 配置范围；服务允许连接和实际连接分开 | G03, G06 |
| 防火墙配置集<br>`network.firewall_profile` | 防火墙开 ↔ 关；默认行为变化 | 原生配置集字段对比 | 配置集标识；单例状态对 | G06 |
| 防火墙规则<br>`network.firewall_rule` | 规则加入 ↔ 移除；启用 ↔ 停用；条件变化 | 原生规则与七类过滤条件关联 | InstanceID＋策略存储范围；部分关联失败保留缺口 | G01, G02, G06 |
| IPsec 规则<br>`network.ipsec_rule` | 规则加入 ↔ 移除；启用 ↔ 停用 | 原生 IPsec 策略表 | 策略存储范围＋规则标识 | G01, G02, G06 |
| 网络连接与监听端点<br>`network.endpoint` | TCP 连接/监听出现 ↔ 离开列表；UDP 端口出现 ↔ 离开列表 | 原生 TCP/UDP 表；Security 5156/5157 是允许/阻止；0.2.14 已接 Sysmon 3 指定版本，完整连接身份和关闭端继续开放（G12） | 协议＋端点＋进程/创建时间；UDP 键不全导致缺席端可被跳过 | G01, G02, G04, G07, G12 |
| SMB 共享<br>`network.smb_share` | 共享登记加入 ↔ 移除；授权变化 | 原生 SMB 共享与 ACL；Security 5140 是访问 | 共享名＋范围；访问不是创建 | G01, G02, G03, G07 |
| SMB 驱动器映射<br>`network.smb_mapping` | 映射建立 ↔ 解除；状态变化 | 原生本机映射表 | 本地盘符＋远端路径；可反复映射 | G01, G02 |
| SMB 客户端会话<br>`network.smb_session` | 会话出现 ↔ 结束/离开列表 | 原生 SMB 客户端会话表 | 会话 ID＋客户端＋用户；轮询可漏短会话 | G01, G02, G04 |
| 网络连接配置文件<br>`network.connection_profile` | 网络连接 ↔ 断开；连通性/类别变化 | 原生配置文件状态；WLAN/NetworkProfile 原始运行事件 | 网络标识＋接口；有配置文件不等于保持联网 | G01, G02, G06, G07 |
| 证书登记<br>`security.certificate_registration` | 证书登记加入 ↔ 移除；有效 ↔ 到期 | 原生当前用户/机器证书存储 | 存储区＋指纹；到期是按时间计算而非删除 | G01, G02, G06 |
| 密钥元数据对象<br>`security.key_metadata` | 密钥登记出现 ↔ 不再列出 | 原生 CNG 提供程序用户/机器范围元数据；Security 密码操作日志需解码 | 提供程序＋范围＋名称；SILENT 枚举错误不能作为移除 | G01, G02, G07 |
| 防护配置与状态<br>`security.protection` | 防护开 ↔ 关；健康恢复；检测 → 处置结果 | 原生防护状态、策略及保留检测记录；Defender 通道 | 配置、健康、单次威胁处置分开；检测历史不是当前感染 | G03, G06, G07 |
| 防护排除项<br>`security.exclusion` | 排除项加入 ↔ 移除 | 原生 Defender 策略聚合列表展开 | 排除类型＋值摘要/身份；父列表内增减行动未齐 | G03 |
| 安全产品登记<br>`security.security_product` | 产品登记加入 ↔ 移除；防护状态变化 | 原生安全中心登记 | 产品标识；系统版本/安全中心可用性影响范围 | G01, G02, G06 |
| BitLocker 配置<br>`security.bitlocker_configuration` | 加密 ↔ 解密；保护开启 ↔ 暂停/关闭；锁定 ↔ 解锁 | 原生卷配置和状态；管理通道原始日志 | 卷身份；保护状态、转换进度、锁定状态分开 | G06, G07 |
| SSH 公钥登记<br>`security.ssh_public_key` | 公钥登记加入 ↔ 移除 | 配置范围内原生公钥指纹读取 | 文件范围＋算法＋指纹；不采集私钥 | G01, G02, G03 |
| Device Guard 配置<br>`security.device_guard_configuration` | 配置开 ↔ 关；实际运行 ↔ 未运行 | 原生 Device Guard/VBS 能力与运行字段 | 能力、配置、实际运行分别展示 | G06 |
| 安全启动配置<br>`security.secure_boot_configuration` | 安全启动启用 ↔ 关闭 | 原生固件配置读取 | 不支持/拒绝读取是环境状态，不能写成已关闭 | G06 |
| 设备<br>`hardware.device` | 检测到 ↔ 不再检测到；正常 ↔ 故障；启用 ↔ 停用 | 原生 PnP 清单和配置错误码；UMDF 原始日志补故障 | PnP 实例 ID；拔出/卸载/隐藏设备状态分开 | G01, G02, G06, G07 |
| 设备驱动绑定<br>`hardware.driver_binding` | 设备绑定驱动 A ↔ B；绑定加入 ↔ 解除 | 原生 Win32_PnPSignedDriver | 设备 ID＋驱动版本；同一源行版本变化未保证旧/新绑定行动 | G01, G02, G08, G09 |
| 驱动服务<br>`hardware.driver_service` | 驱动服务登记加入 ↔ 移除；运行 ↔ 停止；加载 ↔ 卸载 | 原生 Win32_SystemDriver 状态；0.2.14 已接 Sysmon 6 驱动文件加载，不能由文件名猜服务 Name（G12） | 服务名；缺卸载日志分类，服务运行状态另做字段对比 | G01, G02, G06, G07, G12 |
| 磁盘<br>`hardware.disk` | 检测到 ↔ 不再检测到；健康/在线状态变化 | 原生磁盘与健康视图 | 原生 ObjectId；多来源补充同一对象不生成新到达 | G01, G02, G06, G10, G12 |
| 分区<br>`hardware.partition` | 分区出现 ↔ 不再列出；属性变化 | 原生分区表 | 分区原生标识；不直接声称是谁创建/删除 | G01, G02 |
| 卷<br>`hardware.volume` | 卷出现 ↔ 不再列出；盘符/文件系统变化 | 原生卷清单；USN 卷发现仅监控涉及的固定 NTFS 卷 | 卷标识；无盘符卷与卷日志不可读分开 | G01, G02, G10 |
| 驱动包<br>`hardware.driver_package` | 仓库包加入 ↔ 移除；安装尝试 → 成功/失败 | 原生 DISM 仓库全包清单提供登记对比 | 发布 INF＋仓库路径；无 SetupAPI 安装过程解析 | G01, G02, G07, G09 |
| 固件<br>`hardware.firmware` | 版本/发布日期变化 | 原生 BIOS/SMBIOS 元数据 | 固件身份；不建立虚假的固件新增/删除对 | G06, G09 |
| TPM<br>`hardware.tpm` | 存在/就绪/锁定等状态变化 | 原生 TPM 能力/状态及待处理请求 | TPM 环境能力；未读取与不存在分开 | G06 |
| 电池<br>`hardware.battery` | 检测到 ↔ 不再检测到；充放电/健康变化 | 原生电池状态 | 原生设备标识；没有电池的桌面机属正常环境 | G01, G02, G06 |
| 虚拟机<br>`virtualization.virtual_machine` | 登记加入 ↔ 移除；启动 ↔ 关闭；暂停 ↔ 恢复 | Hyper-V 四项仍兼容采集；VMMS 日志保留 | 虚拟机 GUID；无当前可用实机环境验收 | G01, G02, G06, G07, G11 |
| 虚拟交换机<br>`virtualization.virtual_switch` | 交换机加入 ↔ 移除；配置变化 | Hyper-V 兼容清单 | 交换机 GUID；缺 Hyper-V 实机验收 | G01, G02, G11 |
| 虚拟磁盘连接<br>`virtualization.disk_attachment` | 虚拟磁盘连接 ↔ 解除 | Hyper-V 兼容连接清单 | 虚拟机＋控制器位置；镜像文件删除是另一事件 | G01, G02, G11 |
| 虚拟机检查点<br>`virtualization.checkpoint` | 检查点出现 ↔ 移除 | Hyper-V 兼容检查点清单 | 虚拟机＋检查点 ID；不把清单加入写成创建成功 | G01, G02, G11 |
| WSL 发行版<br>`virtualization.wsl_distribution` | 发行版登记加入 ↔ 移除；启动 ↔ 停止（需运行来源） | 原生当前用户 WSL 注册表 | 用户＋发行版 GUID；当前只有登记，缺运行状态 | G01, G02, G07 |
| 容器<br>`virtualization.container` | 容器加入 ↔ 移除；运行 ↔ 停止/退出 | 原生后端访问本机 Docker 命名管道 | 引擎范围＋容器 ID；短容器需事件流，当前无专门事件订阅 | G01, G02, G04, G06 |
| 容器镜像<br>`virtualization.image` | 镜像加入 ↔ 移除；标签变化 | 原生本机 Docker 镜像元数据 | 镜像 ID；同镜像多个标签不等于多个镜像 | G01, G02, G09 |
| 容器卷<br>`virtualization.container_volume` | 容器卷加入 ↔ 移除 | 原生本机 Docker 卷元数据 | 引擎范围＋卷名；不检查卷内数据 | G01, G02 |
| 容器网络<br>`virtualization.container_network` | 容器网络加入 ↔ 移除 | 原生本机 Docker 网络元数据 | 网络 ID；连接端点增减不是网络创建/删除 | G01, G02, G03 |
| 虚拟化能力与平台<br>`virtualization.platform_capability` | 虚拟化能力/功能开关变化 | 原生系统能力读取 | 能力不是虚拟机实例，无创建/删除过程要求 | G06 |
| 打印机<br>`desktop.printer` | 打印机登记加入 ↔ 移除；就绪 ↔ 故障 | 原生打印机清单和状态 | 打印机范围＋标识；打印服务不可用不批量删除 | G01, G02, G06 |
| 打印端口<br>`desktop.printer_port` | 打印端口登记加入 ↔ 移除 | 原生打印端口清单 | 端口名；端口登记不是网络监听端口 | G01, G02 |
| 打印驱动<br>`desktop.printer_driver` | 打印驱动登记加入 ↔ 移除；版本/路径变化 | 原生打印驱动清单 | 名称＋打印环境；与系统驱动仓库包分层 | G01, G02, G09 |
| 打印作业<br>`desktop.print_job` | 进入队列 → 完成/取消/失败；离开队列 | 原生打印队列；PrintService 原始日志 | 打印机＋作业 ID＋提交时间；离队尚无确定完成/取消分类 | G01, G04, G07 |
| 输入法配置<br>`desktop.input_configuration` | 输入法/键盘配置加入 ↔ 移除；默认项切换 | 原生指定输入配置 | 范围＋配置身份；过滤驱动与实际加载分层 | G01, G02, G03, G06 |
| 桌面与资源管理器配置<br>`desktop.shell_configuration` | 桌面/资源管理器设置变化；开关切换 | 原生指定设置；剪贴板日志只元数据 | 配置范围；不采集剪贴板正文 | G03, G06, G07, G12 |
| 快捷方式<br>`desktop.shortcut` | 快捷方式出现 ↔ 移除；目标/参数变化 | 原生桌面/开始菜单快捷方式元数据 | 路径/文件身份；不执行目标 | G01, G02, G05 |
| 编辑器扩展<br>`desktop.editor_extension` | 编辑器扩展目录登记加入 ↔ 移除；版本变化 | 原生 VS Code 扩展目录清单 | 编辑器＋扩展 ID/版本；缺实时启用状态 | G01, G02, G09 |
| 同步账户登记<br>`desktop.sync_account` | 同步账户登记加入 ↔ 移除；配置变化 | 原生 OneDrive 账户登记 | 用户＋账户登记路径；缺同步运行、错误、完成过程 | G01, G02, G07 |
