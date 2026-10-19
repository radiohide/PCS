# 历史变化与当前状态显示核查（0.2.13）

日期：2026-10-19。范围：全部 101 种系统对象，以及旧采集记录的公共显示入口。

## 显示规则

- 时间线和对象中的历史变化描述当时采集到的变化；首次采集记录按类型写明运行、加载、登记或清单项，不改写成安装、创建或启用操作。
- 时间线按钮写“查看当前状态”，打开时仍查询该对象的最新详情，不把历史事件的属性用作当前状态。
- 对象详情将“当前状态”单列。已不在清单、已结束或已移除对象的旧属性放在“最后记录的信息”下；模块用“最后加载时的信息”，进程用“最后运行时的信息”。
- 只有历史日志的对象显示“未取得当前状态”。原生快照确认的活动进程、模块分别显示“运行中”“已加载”。
- 历史对比用“变化前 / 变化后”；仅有一侧值写“当时记录”。任务执行时间、执行计划和网卡速率变化不再写成当前状态。
- 模块来源与签名的 unknown、pending 等状态转成中文。pending 显示“尚未取得签名结果”，不暗示后台一定还有签名任务排队。
- 不改变采集范围、身份规则、生命周期判断、原始记录、展示游标或数据目录。模块加载地址变化继续保留。

## pdh.dll 核查

同一 sihost 进程的完整原生快照在 13:16:21 至 13:25:19 含 pdh.dll，13:26:20 起后续完整快照不再包含它。直接读取现有进程模块列表也未见 pdh.dll。界面应同时保留“首次记录到进程加载此模块”的历史事件，并显示当前“未加载”。此前“新发现”用词使这两个时点混淆，采集记录本身并未关联错对象。

## 全类型核查表

下表逐类型覆盖首次记录、清单缺失后的当前状态及旧属性标题。当前状态来自最近采集结果；详细运行状态仍由各类型属性解释。

| 类型 | 首次记录的历史描述 | 清单缺失后的当前状态 | 旧属性标题 |
| --- | --- | --- | --- |
| 用户账户（account.user） | 首次记录到此用户账户 | 账户清单未再列出 | 最后记录的信息 |
| 用户组（account.group） | 首次记录到此用户组 | 用户组未再列出 | 最后记录的信息 |
| 用户组成员（account.group_membership） | 首次记录到此成员关系 | 成员关系不再列出 | 最后记录的信息 |
| 用户资料（account.user_profile） | 首次记录到此用户资料 | 用户配置登记不再列出 | 最后记录的信息 |
| 账户权限（account.right_assignment） | 首次记录到此权限分配 | 权限分配不再列出 | 最后记录的信息 |
| 计算机身份（account.machine_identity） | 首次记录到此计算机身份 | 计算机身份未再列出 | 最后记录的信息 |
| 软件安装信息（software.installation_registration） | 首次记录到此软件安装登记 | 安装登记不再列出 | 最后记录的信息 |
| 已安装应用（software.appx_package） | 首次记录到此应用包 | 应用包不再列出 | 最后记录的信息 |
| 预装应用（software.provisioned_package） | 首次记录到此预装应用登记 | 预装应用登记不再列出 | 最后记录的信息 |
| Windows 可选功能（software.optional_feature） | 首次记录到此Windows 可选功能 | Windows 可选功能未再列出 | 最后记录的信息 |
| 程序运行环境（software.runtime） | 首次记录到此程序运行环境 | 程序运行环境未再列出 | 最后记录的信息 |
| 便携程序（software.portable_registration） | 首次记录到此便携程序登记 | 安装登记不再列出 | 最后记录的信息 |
| 系统更新（software.system_update） | 首次记录到此系统更新 | 系统更新未再列出 | 最后记录的信息 |
| 开发工具包（software.language_package） | 首次记录到此开发工具包 | 开发工具包未再列出 | 最后记录的信息 |
| 程序进程（runtime.process） | 首次记录到此进程运行 | 已不在运行列表 | 最后运行时的信息 |
| 程序模块（runtime.module_instance） | 首次记录到进程加载此模块 | 未加载 | 最后加载时的信息 |
| 登录会话（runtime.logon_session） | 首次记录到此登录会话 | 已不在登录会话列表 | 最后记录的信息 |
| 桌面会话（runtime.terminal_session） | 首次记录到此登录会话 | 已不在登录会话列表 | 最后记录的信息 |
| 系统服务（startup.service） | 首次记录到此服务登记 | 服务登记不再列出 | 最后记录的信息 |
| 计划任务（startup.scheduled_task） | 首次记录到此计划任务登记 | 计划任务登记不再列出 | 最后记录的信息 |
| 登录启动项（startup.logon_startup_entry） | 首次记录到此登录启动项 | 登录启动项未再列出 | 最后记录的信息 |
| 登录初始化设置（startup.logon_initialization） | 首次记录到此登录初始化设置 | 登录初始化设置未再列出 | 最后记录的信息 |
| 系统事件触发条件（startup.wmi_filter） | 首次记录到此系统事件触发条件 | 系统事件触发条件未再列出 | 最后记录的信息 |
| 系统事件触发动作（startup.wmi_consumer） | 首次记录到此系统事件触发动作 | 系统事件触发动作未再列出 | 最后记录的信息 |
| 系统事件触发关联（startup.wmi_binding） | 首次记录到此系统事件触发关联 | 系统事件触发关联未再列出 | 最后记录的信息 |
| 程序启动与调试设置（startup.ifeo_configuration） | 首次记录到此程序启动与调试设置 | 程序启动与调试设置未再列出 | 最后记录的信息 |
| 文件预览组件（shell.preview_handler） | 首次记录到此文件预览组件 | 文件预览组件未再列出 | 最后记录的信息 |
| 文件图标标记组件（shell.icon_overlay_handler） | 首次记录到此文件图标标记组件 | 文件图标标记组件未再列出 | 最后记录的信息 |
| 桌面扩展（shell.shell_extension） | 首次记录到此桌面扩展 | 桌面扩展未再列出 | 最后记录的信息 |
| 资源管理器位置扩展（shell.namespace_extension） | 首次记录到此资源管理器位置扩展 | 资源管理器位置扩展未再列出 | 最后记录的信息 |
| 右键菜单（shell.context_menu_registration） | 首次记录到此右键菜单 | 右键菜单未再列出 | 最后记录的信息 |
| 文件打开方式（shell.file_association） | 首次记录到此文件打开方式 | 文件打开方式未再列出 | 最后记录的信息 |
| 应用组件（shell.com_registration） | 首次记录到此应用组件 | 应用组件未再列出 | 最后记录的信息 |
| 浏览器扩展（shell.browser_extension） | 首次记录到此浏览器扩展 | 浏览器扩展未再列出 | 最后记录的信息 |
| 监控文件（file.watched_file） | 首次记录到此文件 | 监控清单未再列出 | 最后记录的信息 |
| 文件访问权限（file.acl_entry） | 首次记录到此文件访问权限 | 文件访问权限未再列出 | 最后记录的信息 |
| 文件附加数据（file.alternate_data_stream） | 首次记录到此文件附加数据 | 文件附加数据未再列出 | 最后记录的信息 |
| 监控文件夹（file.watched_directory） | 首次记录到此文件夹 | 监控清单未再列出 | 最后记录的信息 |
| 环境变量（policy.environment_variable） | 首次记录到此环境变量 | 环境变量未再列出 | 最后记录的信息 |
| 管理员授权设置（policy.uac_policy） | 首次记录到此管理员授权设置 | 管理员授权设置未再列出 | 最后记录的信息 |
| 程序运行限制（policy.software_restriction_policy） | 首次记录到此程序运行限制 | 程序运行限制未再列出 | 最后记录的信息 |
| 浏览器策略（policy.browser_policy） | 首次记录到此浏览器策略 | 浏览器策略未再列出 | 最后记录的信息 |
| 时间同步设置（policy.time_sync_configuration） | 首次记录到此时间同步设置 | 时间同步设置未再列出 | 最后记录的信息 |
| 电源计划（policy.power_plan） | 首次记录到此电源计划 | 电源计划未再列出 | 最后记录的信息 |
| 操作系统（policy.os_configuration） | 首次记录到此操作系统 | 操作系统未再列出 | 最后记录的信息 |
| 系统还原点（policy.system_restore_point） | 首次记录到此系统还原点 | 系统还原点未再列出 | 最后记录的信息 |
| 网卡（network.adapter） | 首次检测到此设备 | 本次未检测到 | 最后记录的信息 |
| IP 地址（network.address_configuration） | 首次记录到此IP 地址 | IP 地址未再列出 | 最后记录的信息 |
| 网络路由（network.route） | 首次记录到此网络路由 | 网络路由未再列出 | 最后记录的信息 |
| 域名解析设置（network.dns_configuration） | 首次记录到此域名解析设置 | 域名解析设置未再列出 | 最后记录的信息 |
| 域名解析缓存（network.dns_cache_entry） | 首次记录到此域名解析缓存 | DNS 缓存中已无此项 | 最后记录的信息 |
| 本机域名映射（network.hosts_entry） | 首次记录到此本机域名映射 | 本机域名映射未再列出 | 最后记录的信息 |
| 代理设置（network.proxy_configuration） | 首次记录到此代理设置 | 代理设置未再列出 | 最后记录的信息 |
| VPN 连接（network.vpn_configuration） | 首次记录到此VPN 连接 | VPN 连接未再列出 | 最后记录的信息 |
| 远程访问设置（network.remote_access_configuration） | 首次记录到此远程访问设置 | 远程访问设置未再列出 | 最后记录的信息 |
| 防火墙设置（network.firewall_profile） | 首次记录到此防火墙设置 | 防火墙设置未再列出 | 最后记录的信息 |
| 防火墙规则（network.firewall_rule） | 首次记录到此防火墙规则 | 防火墙规则未再列出 | 最后记录的信息 |
| 安全连接规则（network.ipsec_rule） | 首次记录到此安全连接规则 | 安全连接规则未再列出 | 最后记录的信息 |
| 网络连接（network.endpoint） | 首次记录到此网络项 | 已不在连接与监听列表 | 最后记录的信息 |
| 共享文件夹（network.smb_share） | 首次记录到此共享文件夹 | 共享文件夹未再列出 | 最后记录的信息 |
| 网络驱动器（network.smb_mapping） | 首次记录到此网络驱动器 | 网络驱动器未再列出 | 最后记录的信息 |
| 文件共享会话（network.smb_session） | 首次记录到此文件共享会话 | 已不在文件共享连接列表 | 最后记录的信息 |
| 连接网络（network.connection_profile） | 首次记录到此连接网络 | 连接网络未再列出 | 最后记录的信息 |
| 证书（security.certificate_registration） | 首次记录到此证书 | 证书登记不再列出 | 最后记录的信息 |
| 密钥信息（security.key_metadata） | 首次记录到此密钥信息 | 密钥清单未再列出 | 最后记录的信息 |
| Windows 防护（security.protection） | 首次记录到此Windows 防护 | Windows 防护未再列出 | 最后记录的信息 |
| 防护排除项（security.exclusion） | 首次记录到此防护排除项 | 防护排除项未再列出 | 最后记录的信息 |
| 安全软件（security.security_product） | 首次记录到此安全软件 | 安全软件未再列出 | 最后记录的信息 |
| 磁盘加密（security.bitlocker_configuration） | 首次记录到此磁盘加密 | 磁盘加密未再列出 | 最后记录的信息 |
| SSH 公钥（security.ssh_public_key） | 首次记录到此SSH 公钥 | SSH 公钥未再列出 | 最后记录的信息 |
| 系统安全隔离（security.device_guard_configuration） | 首次记录到此系统安全隔离 | 系统安全隔离未再列出 | 最后记录的信息 |
| 安全启动（security.secure_boot_configuration） | 首次记录到此安全启动 | 安全启动未再列出 | 最后记录的信息 |
| 硬件设备（hardware.device） | 首次检测到此设备 | 本次未检测到 | 最后记录的信息 |
| 设备驱动（hardware.driver_binding） | 首次记录到此设备驱动 | 设备驱动关联不再列出 | 最后记录的信息 |
| 驱动服务（hardware.driver_service） | 首次记录到此驱动服务登记 | 服务登记不再列出 | 最后记录的信息 |
| 磁盘（hardware.disk） | 首次检测到此设备 | 本次未检测到 | 最后记录的信息 |
| 磁盘分区（hardware.partition） | 首次记录到此磁盘分区 | 本次未检测到 | 最后记录的信息 |
| 存储卷（hardware.volume） | 首次记录到此存储卷 | 本次未检测到 | 最后记录的信息 |
| 驱动包（hardware.driver_package） | 首次记录到此驱动包 | 驱动包未再列出 | 最后记录的信息 |
| 固件（hardware.firmware） | 首次记录到此固件 | 本次未检测到 | 最后记录的信息 |
| 安全芯片（hardware.tpm） | 首次检测到此设备 | 本次未检测到 | 最后记录的信息 |
| 电池（hardware.battery） | 首次检测到此设备 | 本次未检测到 | 最后记录的信息 |
| 虚拟机（virtualization.virtual_machine） | 首次记录到此虚拟机 | 虚拟机登记不再列出 | 最后记录的信息 |
| 虚拟交换机（virtualization.virtual_switch） | 首次记录到此虚拟交换机 | 虚拟交换机未再列出 | 最后记录的信息 |
| 虚拟机磁盘（virtualization.disk_attachment） | 首次记录到此虚拟机磁盘 | 虚拟磁盘连接不再列出 | 最后记录的信息 |
| 虚拟机检查点（virtualization.checkpoint） | 首次记录到此虚拟机检查点 | 虚拟机检查点未再列出 | 最后记录的信息 |
| Linux 子系统（virtualization.wsl_distribution） | 首次记录到此Linux 子系统 | Linux 子系统未再列出 | 最后记录的信息 |
| 容器（virtualization.container） | 首次记录到此容器 | 容器清单未再列出 | 最后记录的信息 |
| 容器镜像（virtualization.image） | 首次记录到此容器镜像 | 容器镜像未再列出 | 最后记录的信息 |
| 容器数据卷（virtualization.container_volume） | 首次记录到此容器数据卷 | 容器数据卷未再列出 | 最后记录的信息 |
| 容器网络（virtualization.container_network） | 首次记录到此容器网络 | 容器网络未再列出 | 最后记录的信息 |
| 虚拟化支持（virtualization.platform_capability） | 首次记录到此虚拟化支持 | 虚拟化支持未再列出 | 最后记录的信息 |
| 打印机（desktop.printer） | 首次检测到此设备 | 本次未检测到 | 最后记录的信息 |
| 打印连接（desktop.printer_port） | 首次记录到此打印连接 | 打印连接未再列出 | 最后记录的信息 |
| 打印驱动（desktop.printer_driver） | 首次记录到此打印驱动 | 打印驱动未再列出 | 最后记录的信息 |
| 打印任务（desktop.print_job） | 首次记录到此打印作业进入队列 | 已离开打印队列 | 最后在队列中的信息 |
| 输入法设置（desktop.input_configuration） | 首次记录到此输入法设置 | 输入法设置未再列出 | 最后记录的信息 |
| 桌面设置（desktop.shell_configuration） | 首次记录到此桌面设置 | 桌面设置未再列出 | 最后记录的信息 |
| 快捷方式（desktop.shortcut） | 首次记录到此快捷方式 | 快捷方式未再列出 | 最后记录的信息 |
| 编辑器扩展（desktop.editor_extension） | 首次记录到此编辑器扩展 | 编辑器扩展未再列出 | 最后记录的信息 |
| 同步账户（desktop.sync_account） | 首次记录到此同步账户 | 同步账户未再列出 | 最后记录的信息 |

网络连接另按事件自身快照区分“首次记录到此连接 / 此监听端口 / 此 UDP 端口”；缺少该次状态时使用“首次记录到此网络项”，不借用对象后来的状态。

## 验证

- 101 种类型覆盖首次记录和缺失状态；验证历史显示不会声称安装、创建或启用，且保留原始数据。
- 39 个实际页面交互场景：从历史加载记录打开未加载对象、重新打开读取变化后的当前状态、仅有日志时保持当前状态未知，以及现有分类、折叠、分页、自动刷新和日常更新。
- 复用现有状态格式、易读详情、查询消费者、查询路径和采集视图检查。截图与本机原始快照核查材料只放在被忽略的 target 目录。

## 部署验证

0.2.13 已构建并更新到固定的 `dist/dev` 和 `apps/desktop/src-tauri/target/debug`，程序依赖与原有路径配置保留，正常界面已重新创建 PCScope 窗口。更新前后正式后台仍为同一增强采集进程；数据目录不变，展示代次保留，最新记录标记继续推进。13:59 的正式对象详情仍为“未加载”，并保留首次加载历史。

此次显示修正随界面生效，无需替换增强后台或再次 UAC。只清理本次已替换且可删除的程序备份，释放约 379 MiB；正在使用的后台程序备份保留。
