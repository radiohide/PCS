'use strict';
// Exhaustive source/item/type coverage. This is a source audit, not a machine acceptance report.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..');
const read=p=>JSON.parse(fs.readFileSync(path.join(root,p),'utf8').replace(/^\uFEFF/,''));
const inventory=read('model/sources.json'),catalog=read('model/catalog.json');
const definitions=read('apps/desktop/ui/collection-catalog.json'),controls=read('apps/desktop/ui/collection-controls.json');
const migration=read('model/native-migration.json');
const typeRows=`
account.user|创建 ↔ 删除；启用 ↔ 停用；锁定 ↔ 解锁；改名|Security 有对应分类；原生账户表提供当前状态|账户 SID；名称变化保持身份|G01,G02,G07
account.group|建立 ↔ 删除；改名|组清单可差分；当前 Security 专项仅成员加入/移除，未分类组创建/删除|组 SID；组本身与成员登记分开|G01,G02,G07
account.group_membership|加入组 ↔ 移出组|Security 4732/4733 两端；原生组成员子表更新当前状态|组 SID＋成员 SID；子表移除更新对象但未生成对应快照行动|G03,G07
account.user_profile|登记加入 ↔ 移除；加载 ↔ 卸载|原生配置文件清单和 loaded 字段|账户 SID；清单移除和配置目录删除分开|G01,G02
account.right_assignment|授予 ↔ 撤销|原生 LSA 权限子表可更新当前分配；4672 是登录令牌权限|账户 SID＋权限；尚无长期分配事件配对|G03,G07
account.machine_identity|名称、域/工作组、角色变化|原生单机配置字段对比|同一安装作用域；无新增/删除过程要求|G06
software.installation_registration|安装登记加入 ↔ 移除；版本变化|机器/用户原生卸载登记；MsiInstaller 原始日志可补充过程|hive＋视图＋路径；MSI 成功/失败/卸载未建统一过程|G01,G02,G07,G09
software.appx_package|应用包登记加入 ↔ 移除；版本替换|当前用户原生包清单；部署日志已有开始/失败/摘要说明|用户＋包全名；部署服务启停与包安装分开|G01,G02,G07,G09
software.provisioned_package|预装登记加入 ↔ 移除；版本替换|原生映像预配清单|本机＋包全名；当前用户应用安装是另一层|G01,G02,G09
software.optional_feature|启用 ↔ 禁用；等待重启 ↔ 完成|原生组件状态字段对比|组件名；已有对象切换状态，不必创建新对象|G06,G09
software.runtime|版本登记加入 ↔ 移除|原生 .NET 注册表展开为多个运行时对象|家族＋版本＋注册表视图；同父键内增减未完成行动对|G03,G09
software.portable_registration|文件登记加入 ↔ 移除；版本变化|仅配置的便携程序根目录，原生文件元数据|卷/文件身份；无需解释为安装/卸载|G01,G02,G05
software.system_update|安装 ↔ 卸载；开始 → 成功/失败/中止|原生 WUA 历史保留 operation/result；QFE 是当前清单子集|更新 ID＋修订；历史操作不并入当前安装清单|G07,G09
software.language_package|包加入 ↔ 移除；版本替换|标准全局 npm 目录原生读取清单|管理器＋包名＋版本；自定义 prefix 不覆盖|G01,G02,G09
runtime.process|启动 ↔ 退出|原生进程清单；Security 4688/4689 和 Sysmon 1/5 两端|PID＋创建时间/可靠 ProcessGuid；短进程需日志|G01,G04,G07
runtime.module_instance|加载 ↔ 卸载；所属进程退出|指定进程原生模块清单；Sysmon 7 只有加载分类|进程实例＋模块路径；同一进程卸载后重载缺再次出现行动|G01,G02,G04,G07
runtime.logon_session|登录 ↔ 注销；注销请求 → 完成|原生 LSA 会话；Security 4624/4634 两端，4647 是请求|登录 ID＋启动周期；缺启动周期不能跨重启配对|G01,G04,G07
runtime.terminal_session|登录 ↔ 注销；连接 ↔ 断开；锁定 ↔ 解锁|原生 WTS 状态；Security 与 LSM 的明确状态规则|会话 ID＋范围/代际；断开后会话可能继续存在|G01,G02,G07
startup.service|服务登记加入 ↔ 移除；运行 ↔ 停止；启动失败|原生服务清单/配置；Security 4697 安装；SCM 7036 可解码状态|稳定服务名；显示名仅唯一匹配；未建服务卸载事件行动|G01,G02,G07
startup.scheduled_task|任务创建 ↔ 删除；启用 ↔ 禁用；每次运行开始 → 结束/失败|原生基础/详细配置；Security 4698/4699；TaskScheduler 100/102|完整任务路径＋每次运行实例；同任务并行运行尚未组装过程|G01,G02,G07
startup.logon_startup_entry|启动登记加入 ↔ 移除；命令变更|原生 Run/RunOnce/启动目录等配置|范围＋登记位置＋值名；父键内增减另查|G01,G02,G03
startup.logon_initialization|登录初始化配置设置 ↔ 清除；值变化|原生 Winlogon/会话入口注册表|注册表视图＋路径；配置读取不表示本次执行|G01,G02,G03
startup.wmi_filter|订阅筛选器登记加入 ↔ 移除|原生 WMI 配置清单；Sysmon 19 是活动记录|命名空间＋原生路径；需解码 Operation 才能区分事件动作|G01,G02,G07
startup.wmi_consumer|消费者登记加入 ↔ 移除|原生 WMI 配置清单；Sysmon 20 是活动记录|命名空间＋原生路径；脚本/命令正文保护|G01,G02,G07
startup.wmi_binding|订阅绑定加入 ↔ 解除|原生 WMI 关联清单；Sysmon 21 是活动记录|筛选器＋消费者路径；不是订阅实际触发|G01,G02,G07
startup.ifeo_configuration|调试/注入入口设置 ↔ 清除；启用 ↔ 停用|原生指定 IFEO/AppInit/AppCert 配置|视图＋路径＋配置项；设置存在不表示已注入|G01,G02,G03
shell.preview_handler|预览组件登记加入 ↔ 移除；目标变更|原生注册表登记|CLSID＋范围/视图；加载执行由模块采集另证|G01,G02
shell.icon_overlay_handler|图标标记登记加入 ↔ 移除|原生注册表登记|登记路径/组件标识；没有运行时调用对|G01,G02
shell.shell_extension|桌面扩展登记加入 ↔ 移除；目标变更|原生注册表登记|范围＋视图＋登记路径；注册与加载分层|G01,G02
shell.namespace_extension|命名空间登记加入 ↔ 移除|原生注册表登记|范围＋视图＋登记路径|G01,G02
shell.context_menu_registration|右键菜单登记加入 ↔ 移除；命令变化|原生注册表登记|登记位置＋值名/组件标识|G01,G02
shell.file_association|文件关联设置 ↔ 清除；默认程序替换|原生关联登记，多个来源可描述同一对象|扩展名/ProgID＋范围＋视图；新增来源不是新关联|G01,G02,G03
shell.com_registration|组件登记加入 ↔ 移除；组件服务器路径变化|原生 COM 根/进程内/进程外服务器登记|同 CLSID 多角色；单角色移除不删除整个组件|G01,G02,G10
shell.browser_extension|扩展目录登记加入 ↔ 移除；启用 ↔ 停用（有字段时）|原生 Chromium/Firefox 清单；Chromium 目录不提供可靠启用状态|浏览器配置文件＋扩展 ID＋版本；版本目录替换需归并|G01,G02,G09
file.watched_file|路径出现 ↔ 不存在；创建 ↔ 删除；改名；内容/权限变化|指定文件原生读取；模块文件自动名单；USN 和 Sysmon 补事件|卷＋文件 ID，路径另跟踪；无法读取与明确不存在分开|G01,G02,G03,G05,G07
file.acl_entry|权限项加入 ↔ 移除；允许/拒绝/继承变化|原生文件元数据内展开 DACL；Security 4670 权限审计|文件身份＋ACE；父文件内权限增减无完整快照行动对|G03,G05,G07
file.alternate_data_stream|文件流出现 ↔ 移除；大小/内容摘要变化|指定文件内 ADS 清单；Sysmon 15 只报创建|文件身份＋流名；父文件内增减须对子项差分|G03,G05,G07
file.watched_directory|目录路径出现 ↔ 不存在；直接子项加入 ↔ 移除|原生配置目录及直接子项快照，非递归|目录身份＋配置路径；子项目前聚合在父记录|G01,G02,G03,G05
policy.environment_variable|变量设置 ↔ 清除；值变化|原生机器/当前用户环境表|范围＋变量名；父键值增减需要子项行动|G03,G06
policy.uac_policy|策略设置 ↔ 清除；值切换|原生策略键/值|范围＋视图＋键值；未配置、读取失败、关闭分开|G03,G06
policy.software_restriction_policy|规则加入 ↔ 移除；策略启用 ↔ 停用|原生策略快照|作用范围＋规则登记身份；有效访问结果不推测|G01,G02,G03,G06
policy.browser_policy|策略设置 ↔ 清除；值变化|原生浏览器策略键逐值展开|浏览器＋作用范围＋视图＋键值；父键内增减有缺口|G03,G06
policy.time_sync_configuration|同步配置/时区变化；同步状态恢复|原生配置；Time-Service 37 补时间源状态|本机配置；配置不等于每次同步过程|G06,G07
policy.power_plan|计划登记加入 ↔ 移除；当前计划 A → B|原生电源计划/激活配置|计划 GUID；计划名称是对象，是否启用是状态|G01,G02,G06
policy.os_configuration|系统版本/待重启/启动状态变化；开机 ↔ 关机|原生配置；System 生命周期/电源事件|本机单例；异常重启不能补造正常关机|G06,G07
policy.system_restore_point|还原点出现 ↔ 不再保留|原生还原点元数据|序列号/原生标识；过期清理与用户删除不强行归因|G01,G02
network.adapter|检测到 ↔ 不再检测到；启用 ↔ 停用；链路通 ↔ 断|原生适配器含隐藏项；状态字段对比|接口 GUID；移除设备与关闭网卡分开|G01,G02,G06
network.address_configuration|地址加入 ↔ 移除；可用状态变化|原生接口地址表|接口＋地址；DHCP 更新是配置状态变化|G01,G02,G06
network.route|路由加入 ↔ 移除；下一跳/度量变化|原生路由表|接口＋前缀＋策略范围；路由轮换可重复出现|G01,G02
network.dns_configuration|DNS 服务器/搜索配置设置 ↔ 清除；值替换|原生接口 DNS 配置|接口范围；不当作 DNS 查询成功/失败|G03,G06
network.dns_cache_entry|缓存加入 ↔ 到期/清除后不再列出|原生 DNS 缓存；Sysmon 22 是查询事件|名称＋类型＋数据等缓存键；查询和缓存不是一对|G01,G02,G04,G07
network.hosts_entry|静态映射加入 ↔ 移除|原生解析有效 hosts 映射|地址＋名称；注释不进入映射对象|G01,G02
network.proxy_configuration|代理启用 ↔ 停用；配置替换|当前用户/机器指定原生配置，Mihomo 显式范围|配置范围；Mihomo 运行配置与系统代理分层|G03,G06
network.vpn_configuration|VPN 登记加入 ↔ 移除；连接 ↔ 断开（需运行来源）|原生电话簿/配置登记为主|用户/配置名称；当前未有完整实时 VPN 会话过程|G01,G02,G03,G07
network.remote_access_configuration|远程访问配置启用 ↔ 停用；值变化|原生 RDP/WinRM 指定配置|配置范围；服务允许连接和实际连接分开|G03,G06
network.firewall_profile|防火墙开 ↔ 关；默认行为变化|原生配置集字段对比|配置集标识；单例状态对|G06
network.firewall_rule|规则加入 ↔ 移除；启用 ↔ 停用；条件变化|原生规则与七类过滤条件关联|InstanceID＋策略存储范围；部分关联失败保留缺口|G01,G02,G06
network.ipsec_rule|规则加入 ↔ 移除；启用 ↔ 停用|原生 IPsec 策略表|策略存储范围＋规则标识|G01,G02,G06
network.endpoint|TCP 连接/监听出现 ↔ 离开列表；UDP 端口出现 ↔ 离开列表|原生 TCP/UDP 表；Sysmon 3 有连接、Security 5156/5157 是允许/阻止|协议＋端点＋进程/创建时间；UDP 键不全导致缺席端可被跳过|G01,G02,G04,G07
network.smb_share|共享登记加入 ↔ 移除；授权变化|原生 SMB 共享与 ACL；Security 5140 是访问|共享名＋范围；访问不是创建|G01,G02,G03,G07
network.smb_mapping|映射建立 ↔ 解除；状态变化|原生本机映射表|本地盘符＋远端路径；可反复映射|G01,G02
network.smb_session|会话出现 ↔ 结束/离开列表|原生 SMB 客户端会话表|会话 ID＋客户端＋用户；轮询可漏短会话|G01,G02,G04
network.connection_profile|网络连接 ↔ 断开；连通性/类别变化|原生配置文件状态；WLAN/NetworkProfile 原始运行事件|网络标识＋接口；有配置文件不等于保持联网|G01,G02,G06,G07
security.certificate_registration|证书登记加入 ↔ 移除；有效 ↔ 到期|原生当前用户/机器证书存储|存储区＋指纹；到期是按时间计算而非删除|G01,G02,G06
security.key_metadata|密钥登记出现 ↔ 不再列出|原生 CNG 提供程序用户/机器范围元数据；Security 密码操作日志需解码|提供程序＋范围＋名称；SILENT 枚举错误不能作为移除|G01,G02,G07
security.protection|防护开 ↔ 关；健康恢复；检测 → 处置结果|原生防护状态、策略及保留检测记录；Defender 通道|配置、健康、单次威胁处置分开；检测历史不是当前感染|G03,G06,G07
security.exclusion|排除项加入 ↔ 移除|原生 Defender 策略聚合列表展开|排除类型＋值摘要/身份；父列表内增减行动未齐|G03
security.security_product|产品登记加入 ↔ 移除；防护状态变化|原生安全中心登记|产品标识；系统版本/安全中心可用性影响范围|G01,G02,G06
security.bitlocker_configuration|加密 ↔ 解密；保护开启 ↔ 暂停/关闭；锁定 ↔ 解锁|原生卷配置和状态；管理通道原始日志|卷身份；保护状态、转换进度、锁定状态分开|G06,G07
security.ssh_public_key|公钥登记加入 ↔ 移除|配置范围内原生公钥指纹读取|文件范围＋算法＋指纹；不采集私钥|G01,G02,G03
security.device_guard_configuration|配置开 ↔ 关；实际运行 ↔ 未运行|原生 Device Guard/VBS 能力与运行字段|能力、配置、实际运行分别展示|G06
security.secure_boot_configuration|安全启动启用 ↔ 关闭|原生固件配置读取|不支持/拒绝读取是环境状态，不能写成已关闭|G06
hardware.device|检测到 ↔ 不再检测到；正常 ↔ 故障；启用 ↔ 停用|原生 PnP 清单和配置错误码；UMDF 原始日志补故障|PnP 实例 ID；拔出/卸载/隐藏设备状态分开|G01,G02,G06,G07
hardware.driver_binding|设备绑定驱动 A ↔ B；绑定加入 ↔ 解除|原生 Win32_PnPSignedDriver|设备 ID＋驱动版本；同一源行版本变化未保证旧/新绑定行动|G01,G02,G08,G09
hardware.driver_service|驱动服务登记加入 ↔ 移除；运行 ↔ 停止；加载 ↔ 卸载|原生 Win32_SystemDriver 状态；Sysmon 6 仅加载事件|服务名；缺卸载日志分类，服务运行状态另做字段对比|G01,G02,G06,G07
hardware.disk|检测到 ↔ 不再检测到；健康/在线状态变化|原生磁盘与健康视图|原生 ObjectId；多来源补充同一对象不生成新到达|G01,G02,G06,G10
hardware.partition|分区出现 ↔ 不再列出；属性变化|原生分区表|分区原生标识；不直接声称是谁创建/删除|G01,G02
hardware.volume|卷出现 ↔ 不再列出；盘符/文件系统变化|原生卷清单；USN 卷发现仅监控涉及的固定 NTFS 卷|卷标识；无盘符卷与卷日志不可读分开|G01,G02,G10
hardware.driver_package|仓库包加入 ↔ 移除；安装尝试 → 成功/失败|原生 DISM 仓库全包清单提供登记对比|发布 INF＋仓库路径；无 SetupAPI 安装过程解析|G01,G02,G07,G09
hardware.firmware|版本/发布日期变化|原生 BIOS/SMBIOS 元数据|固件身份；不建立虚假的固件新增/删除对|G06,G09
hardware.tpm|存在/就绪/锁定等状态变化|原生 TPM 能力/状态及待处理请求|TPM 环境能力；未读取与不存在分开|G06
hardware.battery|检测到 ↔ 不再检测到；充放电/健康变化|原生电池状态|原生设备标识；没有电池的桌面机属正常环境|G01,G02,G06
virtualization.virtual_machine|登记加入 ↔ 移除；启动 ↔ 关闭；暂停 ↔ 恢复|Hyper-V 四项仍兼容采集；VMMS 日志保留|虚拟机 GUID；无当前可用实机环境验收|G01,G02,G06,G07,G11
virtualization.virtual_switch|交换机加入 ↔ 移除；配置变化|Hyper-V 兼容清单|交换机 GUID；缺 Hyper-V 实机验收|G01,G02,G11
virtualization.disk_attachment|虚拟磁盘连接 ↔ 解除|Hyper-V 兼容连接清单|虚拟机＋控制器位置；镜像文件删除是另一事件|G01,G02,G11
virtualization.checkpoint|检查点出现 ↔ 移除|Hyper-V 兼容检查点清单|虚拟机＋检查点 ID；不把清单加入写成创建成功|G01,G02,G11
virtualization.wsl_distribution|发行版登记加入 ↔ 移除；启动 ↔ 停止（需运行来源）|原生当前用户 WSL 注册表|用户＋发行版 GUID；当前只有登记，缺运行状态|G01,G02,G07
virtualization.container|容器加入 ↔ 移除；运行 ↔ 停止/退出|原生后端访问本机 Docker 命名管道|引擎范围＋容器 ID；短容器需事件流，当前无专门事件订阅|G01,G02,G04,G06
virtualization.image|镜像加入 ↔ 移除；标签变化|原生本机 Docker 镜像元数据|镜像 ID；同镜像多个标签不等于多个镜像|G01,G02,G09
virtualization.container_volume|容器卷加入 ↔ 移除|原生本机 Docker 卷元数据|引擎范围＋卷名；不检查卷内数据|G01,G02
virtualization.container_network|容器网络加入 ↔ 移除|原生本机 Docker 网络元数据|网络 ID；连接端点增减不是网络创建/删除|G01,G02,G03
virtualization.platform_capability|虚拟化能力/功能开关变化|原生系统能力读取|能力不是虚拟机实例，无创建/删除过程要求|G06
desktop.printer|打印机登记加入 ↔ 移除；就绪 ↔ 故障|原生打印机清单和状态|打印机范围＋标识；打印服务不可用不批量删除|G01,G02,G06
desktop.printer_port|打印端口登记加入 ↔ 移除|原生打印端口清单|端口名；端口登记不是网络监听端口|G01,G02
desktop.printer_driver|打印驱动登记加入 ↔ 移除；版本/路径变化|原生打印驱动清单|名称＋打印环境；与系统驱动仓库包分层|G01,G02,G09
desktop.print_job|进入队列 → 完成/取消/失败；离开队列|原生打印队列；PrintService 原始日志|打印机＋作业 ID＋提交时间；离队尚无确定完成/取消分类|G01,G04,G07
desktop.input_configuration|输入法/键盘配置加入 ↔ 移除；默认项切换|原生指定输入配置|范围＋配置身份；过滤驱动与实际加载分层|G01,G02,G03,G06
desktop.shell_configuration|桌面/资源管理器设置变化；开关切换|原生指定设置；剪贴板日志只元数据|配置范围；不采集剪贴板正文|G03,G06,G07
desktop.shortcut|快捷方式出现 ↔ 移除；目标/参数变化|原生桌面/开始菜单快捷方式元数据|路径/文件身份；不执行目标|G01,G02,G05
desktop.editor_extension|编辑器扩展目录登记加入 ↔ 移除；版本变化|原生 VS Code 扩展目录清单|编辑器＋扩展 ID/版本；缺实时启用状态|G01,G02,G09
desktop.sync_account|同步账户登记加入 ↔ 移除；配置变化|原生 OneDrive 账户登记|用户＋账户登记路径；缺同步运行、错误、完成过程|G01,G02,G07
`.trim().split('\n').map(line=>{const [id,pairs,current,identity,gaps]=line.split('|');return {id,pairs,current,identity,gaps:gaps.split(',')};});
const issues=[
 ['G01','P0','首次出现端曾缺失','0.2.4 已补原始 object.first_seen → 新增时间线，首轮基线不报新增；不能据此宣布所有对象的加入/移除完整。','独立源行新增 → 消失各一条；重发不重复；首轮基线零新增。'],
 ['G02','P0','同身份再次出现','当前新增行动要求 first_observed_at 等于本条时间；旧对象离开后回来不会补“重新出现”。','同键 A→空→A→空，四阶段齐全；确认删除后新建须按代际规则。'],
 ['G03','P0','父记录中的子项增减','只有组成员/账户权限有专门子项缺席状态处理，仍未生成对应快照行动；其他展开来源须逐一补子集合差分。','父项 ID 不变，子项 [A,B]→[B,C]→[]：A/C/B 各自有增减，无父对象串联。'],
 ['G04','P1','短生命周期或不完整身份','快照可漏掉两轮之间完成的活动；UDP 等暂定身份新增可唯一绑定，缺席分支却要求完整键，存在两端不对称。','TCP/监听/UDP/短进程/打印作业分别验证；无可靠身份只保留来源记录，不套当前对象。'],
 ['G05','P1','路径、文件身份与监控名单变化','文件不存在、无法读取、路径改名、文件被替换、从监控名单移除须分别处理；USN 只覆盖名单与可读 NTFS 卷。','原文件改名、同路径换文件、拒绝读取、移除监控名单四类互不误报。'],
 ['G06','P1','状态对与清单对混用','已有字段变化并不等于存在可读的启停/恢复过程；单例配置不需要虚构新增/删除。','同对象字段 A→B→A 完整；错误/不支持/未知不转换成关闭或移除。'],
 ['G07','P1','原始日志不等于过程已建模','通道读入、前端事件解释、对象行动、同一次操作的开始/结果关联是不同层。多数扩展通道仅原始日志，不能记作成对过程已完成。','按 provider/channel/version/ID/操作字段解码；按 TaskInstanceId/ActivityId 等可靠键关联；无终点不补成功。'],
 ['G08','P0','稳定源行中的对象身份更换','设备 ID 不变而 DriverVersion 改变，原始记录是 object.changed；只处理 object.first_seen/removed 无法保证旧绑定退出与新绑定加入。','驱动 A→B→A：两次旧绑定离开、两次新绑定加入，设备仍是同一个。'],
 ['G09','P1','版本替换与操作结果','版本对象身份可改变；安装/升级/卸载应按产品/包/设备锚点归并，不能把独立行硬配成一次成功操作。','并存版本、升级失败回滚、只移除登记、下载但未安装分别验收。'],
 ['G10','P1','多来源、多登记角色及作用范围','单个来源/COM 角色离开不意味着整个对象消失；同来源多项绑定和机器/用户、32/64 位范围须保留。','A 来源离开但 B 仍在；COM 一个角色移除；禁用采集不产生移除。'],
 ['G11','P2','Hyper-V 环境缺口','四项 Hyper-V 来源仍为 PowerShell 兼容，用户暂未提供测试环境；不列为原生迁移或实机配对验收通过。','有环境后验证虚拟机、检查点、交换机、磁盘连接全流程；现阶段不启用系统功能。'],
 ['G12','P0','Sysmon 生产分类未接到对象行动','原生 Kind() 对全部 Sysmon 事件输出 execution.sysmon_event；它不是已注册对象动作，EventAdapter 回退表又只支持 Security/TaskScheduler。collection-catalog 的 actions 用于分类/解释，未被该链路用于改写事件 kind。因此 1/5、6/7、3、11/23/26 等当前只能留原始事件，不能按已有分类宣称对象行动已接通。','将真实生产信封 1/5、6/7、3、11/23/26 贯穿存储与同用查询；校验 provider/channel、时间、身份和文件名单；不能仅传预分类的 fixture kind 验收。'],
].map(([id,priority,name,current,acceptance])=>({id,priority,status:id==='G01'?'implemented_with_stated_limits':'open',name,current,acceptance}));
const types=new Map(typeRows.map(t=>[t.id,t]));
assert.equal(types.size,typeRows.length,'duplicate type review');
assert.deepEqual([...types.keys()].sort(),catalog.types.map(t=>t.id).sort(),'type review drift');
const knownIssues=new Set(issues.map(x=>x.id));
for(const t of typeRows)for(const gap of t.gaps)assert(knownIssues.has(gap),`${t.id}: ${gap}`);
const sourceKey=d=>d.prefix&&d.id==='pcscope.usn.changes'?'pcscope.usn.changes':d.prefix&&d.id==='pcscope.eventlog.import'?'pcscope.eventlog.import':d.source;
const unique=a=>[...new Set(a)];
const reading={
 'pcscope.collector.event-channel-coverage':'逐通道权限/可用性与保留信息；无安装/删除过程。',
 'pcscope.collector.scoped-configuration':'监控范围配置；名单关闭/清空不产生被监控对象删除。',
 'pcscope.execution.module-scan-status':'模块发现/读取结果；读取失败与进程未发现分开，无模块卸载推断。',
 'pcscope.execution.module-file-state':'从成功模块观察建立文件名单，跟踪磁盘稳定复核；变化/恢复留在 source_readings，稳定存在时也可形成文件对象；通用新增/缺席证据主动排除此来源。',
 'pcscope.execution.resources':'CPU/内存采样是数值序列，无新增/删除成对要求。',
 'pcscope.usn.watch-list':'文件名单与采集范围状态；自身不产生文件对象。',
 'pcscope.eventlog.import':'手动历史事件导入分组；保留当时事件，不冒充当前运行状态。',
 'pcscope.usn.changes':'逐卷 USN 事件汇总开关，不是另一个独立采集器。',
 'pcscope.usn.{drive}':'原生已有 USN 日志增量；名单内文件 reason 标记有创建/删除/重命名依据，但尚无逐类结构化对象行动适配。',
 'pcscope.security.detections':'原生读取 Defender 已保留的检测记录，适配为检测证据；不是当前感染状态的清单，不生成历史记录的新增/消失来推断感染/治愈。',
 'pcscope.system.update-history':'原生 WUA 历史有安装/卸载 operation 与 result 结果；记录为独立操作，不把保留历史条目消失当作卸载。',
};
const eventNotes={
 'pcscope.eventlog.security':'账户/成员/任务创建删除、进程启退、会话登录注销等有显式动作；4697 仅服务安装，5156/5157 允许/阻止不组成连接建立/关闭；密码操作须解码操作字段。',
 'pcscope.eventlog.system':'SCM 7036 明确运行状态可唯一显示名匹配服务；其余服务/系统事件可读原始记录。缺服务卸载专门动作和统一启动失败关联。',
 'pcscope.eventlog.application':'MSI、错误、无响应、WER 等原始日志与可读解释；未实现通用安装/卸载开始与结果的过程关联。',
 'pcscope.eventlog.microsoft-windows-taskscheduler/operational':'100/102 开始/结束可分类，100 携带实例；未按同一次运行组织开始、动作返回、结束及失败。',
 'pcscope.eventlog.microsoft-windows-sysmon/operational':'前端/目录有进程 1/5、文件 11/23/26 等分类，但生产原生 Kind() 统一给 execution.sysmon_event，未接入对象行动（G12）。6/7 本身也只有加载，3 只有连接观察，没有卸载/关闭另一端。',
 'pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational':'603 开始、404 失败、613 性能摘要已有文案；939–942 是部署服务状态；未构成应用包安装/卸载的统一结果过程。',
 'pcscope.eventlog.microsoft-windows-grouppolicy/operational':'已有服务启停、400x/800x 处理开始/完成等事件说明；未按处理活动关联到一次过程。',
 'pcscope.eventlog.microsoft-windows-windowsupdateclient/operational':'日志可读安装/检查阶段；原生 WUA 历史另有安装/卸载和结果，尚未跨来源关联一次更新操作。',
 'pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational':'LSM 24/25 有明确断开/重连状态规则；21–23 等其余原始记录尚未完整适配，会话连接/断开和登录/注销须分开。',
 'pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational':'保留远程连接管理记录；认证/连接请求不能直接充当完整会话开始/结束。',
 'pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational':'UMDF 运行/故障原始记录；未有驱动包安装/卸载过程解析；通道存在不代表另一端已实现。',
 'pcscope.eventlog.microsoft-windows-printservice/operational':'原始打印事件保留；尚缺打印作业完成/取消/失败的统一结构化结果关联。',
 'pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational':'原始连接事件保留；缺连接/断开/失败的统一对象过程适配。',
 'pcscope.eventlog.microsoft-windows-networkprofile/operational':'原始网络配置文件事件保留；缺可靠接口/配置文件身份下的连接过程关联。',
 'pcscope.eventlog.microsoft-windows-windows defender/operational':'检测、处置、健康事件保留；与防护开关及当前威胁状态分开，缺通用检测→处置/恢复过程。',
 'pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management':'原始管理日志保留；加密进度、保护暂停和卷锁定须分过程，当前主要依赖原生状态字段。',
 'pcscope.eventlog.microsoft-windows-codeintegrity/operational':'校验失败/阻止等异常证据，通常没有天然成功配对；需按文件身份和一次加载关联，不推定启动根因。',
 'pcscope.eventlog.microsoft-windows-applocker/exe and dll':'程序/DLL 允许、审计、阻止记录；是判定结果，不能把允许与阻止当作启停一对。',
 'pcscope.eventlog.microsoft-windows-applocker/msi and script':'安装包/脚本允许、审计、阻止记录；不代表安装结果。',
 'pcscope.eventlog.microsoft-windows-powershell/operational':'宿主开始/就绪、命令/脚本元数据；不执行脚本或读取正文来补过程，不是完整开始/退出配对。',
 'pcscope.eventlog.microsoft-windows-wmi-activity/operational':'提供程序启动/调用失败等原始记录；不是永久订阅新增/删除的一对。',
 'pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin':'原始虚拟化管理事件保留；未验收虚拟机启停与任务结果过程，环境缺口仍开放。',
 'pcscope.eventlog.microsoft-windows-backup':'原始备份记录；尚未实现一次备份开始/完成/失败的过程关联。',
};
const expandedSources=new Set(['pcscope.identity.groups','pcscope.identity.assigned-rights','pcscope.files.metadata-security','pcscope.network.access','pcscope.policy.configuration','pcscope.apps.browser-policies','pcscope.environment.configuration','pcscope.security.protection-policy','pcscope.startup.logon','pcscope.apps.dotnet-runtimes']);
// A catalog action and a fixture carrying that action are not proof the native emitter uses it.
for(const id of catalog.sources.filter(s=>s.source==='pcscope.eventlog.microsoft-windows-sysmon/operational').flatMap(s=>s.types))types.get(id).gaps.push('G12');
const replaceCurrent=(id,text)=>{types.get(id).current=text;};
replaceCurrent('runtime.process','原生进程清单；Security 4688/4689 有两端；Sysmon 1/5 仅目录分类，生产对象行动链路未接通（G12）');
replaceCurrent('runtime.module_instance','指定进程原生模块清单；Sysmon 7 有加载日志，但生产对象行动链路未接通（G12），且无卸载对应分类');
replaceCurrent('hardware.driver_service','原生 Win32_SystemDriver 状态；Sysmon 6 有加载日志，生产对象行动链路未接通（G12）');
replaceCurrent('file.watched_file','指定文件原生读取、模块文件自动名单；USN 有通用文件变更行动；Sysmon 创建/删除分类尚未接到生产对象行动（G12）');
replaceCurrent('file.alternate_data_stream','指定文件内 ADS 清单；Sysmon 15 有创建日志，未接生产对象行动，且无删除对应分类');
replaceCurrent('network.endpoint','原生 TCP/UDP 表；Security 5156/5157 是允许/阻止；Sysmon 3 原始连接日志尚未接到对象行动（G12）');
reading['pcscope.usn.{drive}']='原生已有 USN 日志增量，已有 files.usn_changed 通用行动；名单内 reason_mask 保留创建/删除/重命名依据，尚未按 reason 拆为逐类行动/关联重命名前后。';
const sourceRows=inventory.sources.map(s=>{
 const maps=catalog.sources.filter(x=>x.source===s.source),typeIds=unique(maps.flatMap(x=>x.types));
 const defs=definitions.filter(d=>sourceKey(d)===s.source),ms=migration.sources.find(m=>m.source===s.source);
 const backend=s.virtual?'virtual':s.collection_backend;
 assert(backend,`${s.source}: unknown backend`);assert(ms,`${s.source}: missing migration entry`);
 const note=reading[s.source]||eventNotes[s.source]||`完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。${expandedSources.has(s.source)?'此来源包含父记录展开，必须补子集合增减行动。':''}`;
 if(s.source.startsWith('pcscope.eventlog.')&&!s.virtual)assert(eventNotes[s.source],`${s.source}: unreviewed channel`);
 const environments=unique([
  s.virtual?'手动导入/汇总，无独立轮询':backend==='powershell_compatibility'?'Hyper-V 组件、PowerShell 模块、相应权限；暂无实机环境':s.source.startsWith('pcscope.eventlog.')?'Windows 通道存在且可读；权限、启用状态及保留范围决定可用性':'Windows 本机、当前账户与已授权机器范围',
  /execution.modules|execution.module-|files\.|software.portable|apps.mihomo/.test(s.source)?'仅显式配置/模块观察建立的名单；不扩大到全盘/全部进程':null,
  /security.key-metadata/.test(s.source)?'CNG 提供程序/用户与机器范围；SILENT 枚举，可部分失败':null,
  /driver-packages|software.optional-features|software.provisioned|security.secureboot|security.encryption|security.tpm/.test(s.source)?'部分接口需同账户管理员权限；拒绝读取不提交空清单':null,
  /virtualization.docker/.test(s.source)?'本机 Docker 引擎可用且元数据采集开关开启':null,
  /pcscope.usn\./.test(s.source)?'监控文件涉及的固定 NTFS 卷、已有可读 USN 日志；不创建日志':null,
 ].filter(Boolean));
 return {source:s.source,name:s.catalog_name||reading[s.source],backend,virtual:s.virtual,material:unique(maps.map(m=>m.destination)),type_ids:typeIds,definition_ids:defs.map(d=>d.id),control_ids:controls.filter(c=>sourceKey(c)===s.source).map(c=>c.id),current:note,environment:environments,emitter_refs:s.emitters,gaps:unique([...typeIds.flatMap(id=>types.get(id).gaps),...(s.source.startsWith('pcscope.eventlog.')?['G07']:[])]),snapshot_child_expansion:expandedSources.has(s.source)};
});
const sources=new Map(sourceRows.map(s=>[s.source,s]));
const registeredActions=new Set(catalog.actions.map(a=>a.id));
const ruleReviews=Object.fromEntries(`
system.services|登记加入/移除、运行/停止、启动失败分别核对；7036 状态有专门适配，安装/删除日志过程尚未统一。
system.lifecycle|12/13 说明系统启动/关闭；非正常重启可无正常关闭端，按本次启动周期记录，不伪补结束。
system.power|电源/异常重启不是每次均有开始结束；保留来源类型和结果，需结合启动周期，不当作服务启停。
system.registry-maintenance|11/15 是注册表维护诊断，不是同一对象创建/删除一对。
system.time-source|37 是收到有效时间源，只有状态证据；未实现同步失败→恢复的完整过程。
application.installer|需要区分安装/修复/卸载、尝试/成功/失败；当前通用 MSI kind 未接统一对象行动或一次操作关联。
application.errors|应用崩溃是单次故障；没有自然反向事件。后续启动/恢复另作运行证据，不凭缺少新错误宣布恢复。
application.hangs|无响应是故障证据；进程退出、恢复响应、被终止是不同结果，当前未组织过程。
application.nvwmi|3 是服务进度/检查点，需解码状态字段；不能当作固定启动或停止端。
application.vss|8224 是空闲停止；只有停止端，配对需同服务运行状态/日志，不能作为备份失败。
application.wer|1001 是错误报告登记，可能重复引用同一故障；按报告/进程身份关联，不能作为另一场独立崩溃。
application.licensing|安排重启与离线迁移完成是不同操作，不能按这两个事件组成开始/结束。
application.optimization|258/264 是完成/未完成两个结果分支，不能把它们作为开始/结束一对。
security.sessions|4624/4634 登录注销、4800/4801 锁定解锁、4778/4779 重连断开分三对；4625 是失败，4647 是注销请求。
security.accounts|4720/4726 创建删除，4722/4725 启用停用，4740/4767 锁定解锁；4738/4781 更新/改名不是创建删除。
security.groups|4732/4733 成员加入移除；须完整组 SID＋成员 SID；不是用户组本身创建删除。
security.privileges|4672 是登录令牌获得特殊权限，无长期权限撤销另一端；不得充当 LSA 账户权限分配。
security.processes|4688/4689 是启动退出两端，PID 复用须分代；日志身份不全不能硬拼到当前进程。
security.files|4663 访问、4670 权限审计是不同性质；不是文件创建删除一对，仍受配置文件名单范围限制。
security.registry|4657 须解码值操作/前后内容；单一事件编号并不固定表示创建或删除。
security.services|4697 只有服务安装；没有对应删除事件规则，移除由完整服务清单确认。
security.tasks|4698/4699 创建删除，4702 更新；任务配置过程与每次运行开始结束分开。
security.audit-policy|4719 是审计设置变化，以具体类别和前后值解释，无开始结束要求。
security.network|5156/5157 是允许/阻止的判定结果，5140 是共享访问；三者均不是连接建立关闭一对。
security.log-clear|1102 是审计日志清除记录；无自然反向端，不在用户界面推测不可修复的历史完整性。
security.crypto-keys|5058/5059/5061 可含多种密钥操作，必须读取操作/结果字段后才称创建删除/导入导出；当前仅有事件解释。
security.crypto-integrity|组件自检、验证、加载等诊断；按返回码说明结果，无通用启停/增删配对。
security.crypto-configuration|5063–5070 涉及提供程序/上下文/函数配置，应解码对象和操作，尚未形成对应配置子项过程。
security.data-protection|DPAPI 备份/恢复与数据保护/解保护是不同操作，不能仅按相邻编号配对；不采集密钥材料。
security.credential-access|凭据读取、显式身份使用、账户检查为访问行为，不存在自然创建删除对；敏感内容保持保护。
security.membership-queries|4798/4799 是成员查询，不能充当成员加入/移除。
security.object-access|4662 是对象访问审计；按对象/权限字段解释，不自动写为 WMI 订阅增删。
security.files-audit-settings|4907 是对象审计规则变化；SACL 与 DACL 权限不同，不能当作允许/拒绝访问结果。
security.process-tokens|4696 是令牌分配记录；不当作进程启动、永久权限授予或其反向端。
security.system-audit|4608 启动、4616 时间更改、4826 启动配置、4902 审计表初始化分别保留；不是同一个生命周期的四阶段。
security.firewall-lifecycle|5024 服务启动成功、5033 驱动启动成功属于两个组件的同一侧；目前规则缺停止另一端。
security.audit-service|1100 是事件日志服务停止；缺该分类中的启动另一端。
microsoft-windows-taskscheduler/operational.execution|100/102 任务运行开始结束，200/201 动作开始/返回；触发、进程创建、忽略/排队/中止分别保留，同运行实例关联尚缺。
microsoft-windows-taskscheduler/operational.configuration|140 是更新任务登记；创建删除由 Security/任务快照提供，需保持同任务路径。
microsoft-windows-windowsupdateclient/operational.discovery|26 是检查结果，只有检查阶段；更新安装/卸载及结果从 WUA 历史补，尚未跨来源配对。
microsoft-windows-windows defender/operational.health|1150/1151/2010 健康/情报不是检测处置的一对；防护状态字段另核对开关与恢复。
microsoft-windows-powershell/operational.commands|4100 错误、4103 命令调用、4104 脚本块元数据不是三阶段同一次执行；不读取受保护正文补过程。
microsoft-windows-powershell/operational.host|40961 正在启动、40962 就绪、53504 IPC 监听启动均不是退出；目前没有完整宿主启退对。
microsoft-windows-wmi-activity/operational.operations|5857 提供程序启动、5858 客户端失败属于不同活动；不能配成开始/失败或永久订阅新增删除。
microsoft-windows-grouppolicy/operational.service|4115/5115 服务启停，4116/5116 初始化开始完成是两对；目前保留解释，缺一次过程关联。
microsoft-windows-grouppolicy/operational.processing|4000/8000、4001/8001、4004/8004、4005/8005 各是对应处理开始完成；须同 ActivityId/对象范围关联。
microsoft-windows-grouppolicy/operational.sessions|4117/5117 是策略会话开始完成，其余是通知/返回/桌面处理；不能只按会话词语归成一对。
microsoft-windows-grouppolicy/operational.applicability|4126/5126 获取开始完成；5257 下载完成，5312/5313 适用/排除列表不同于启用/禁用结果。
microsoft-windows-grouppolicy/operational.diagnostics|环回模式、内部状态、实例诊断是诊断字段，无通用开始结束要求。
microsoft-windows-appxdeploymentserver/operational.deployment|603 开始、404 失败，613 是性能摘要不能直接标成功；缺包级安装/卸载完整结果关联。
microsoft-windows-appxdeploymentserver/operational.service|939/940 正在启动/运行，941/942 正在停止/停止；描述部署服务，不表示应用包安装卸载成功。
microsoft-windows-sysmon/operational.processes|1/5 是启退对应分类；原生统一 kind 未接到对象动作（G12），PID/ProcessGuid 仍须可靠关联。
microsoft-windows-sysmon/operational.process-access|8/10/25 是远程线程、访问和映像异常行为；无天然反向端；生产动作链路仍缺 G12。
microsoft-windows-sysmon/operational.network|3 只有连接观察，无关闭事件分类；生产对象行动链路还缺 G12。
microsoft-windows-sysmon/operational.dns|22 是查询行为和结果，不是缓存加入/删除的一对；生产动作链路仍缺 G12。
microsoft-windows-sysmon/operational.files|11 创建/覆盖、23/26 删除等已有分类，须名单内文件身份；15 流创建缺移除，生产行动仍缺 G12。
microsoft-windows-sysmon/operational.registry|12 须读 EventType 才区分创建/删除，13 设置、14 改名；当前只有事件分类，生产行动仍缺 G12。
microsoft-windows-sysmon/operational.modules|6 驱动加载、7 模块加载是两类对象的同一侧，不是加载卸载一对；生产行动仍缺 G12。
microsoft-windows-sysmon/operational.wmi|19/20/21 是筛选器、消费者、绑定三类记录，须读 Operation 区分增删；生产行动仍缺 G12。
microsoft-windows-sysmon/operational.pipes|17 创建和18连接不是创建删除一对；无关闭另一端，生产行动仍缺 G12。
microsoft-windows-sysmon/operational.raw-disk|9 是读取行为，无自然反向端；不能当作磁盘出现或消失，生产行动仍缺 G12。
microsoft-windows-sysmon/operational.clipboard|24 是剪贴板变化元数据，无开始结束要求；正文不采集，生产行动仍缺 G12。
microsoft-windows-sysmon/operational.monitor|4 需读服务状态区分启停，16 配置变化，255 内部错误不同过程；生产行动仍缺 G12。
`.trim().split('\n').map(line=>{const [key,review]=line.split('|');return[key,review];}));
const itemRows=definitions.map(d=>{
 const key=sourceKey(d),s=sources.get(key);assert(s,`${d.id}: source omitted`);
 const rule=d.event_rule;
 let kind=rule?.status?'采集健康':rule?'事件':d.prefix?'汇总/导入':s.material.includes('observation')?'状态清单':s.material.includes('action_evidence')?'历史动作':s.material.includes('sampling')?'资源采样':'采集健康';
 const actions=rule?Object.entries(rule.actions||{}).map(([eventId,a])=>({event_id:Number(eventId),action_id:a.id,name:a.name,catalog_action:registeredActions.has(a.id)})):[];
 const ruleKey=d.id.slice('pcscope.eventlog.'.length);
 if(rule&&!['status','other'].includes(rule.suffix))assert(ruleReviews[ruleKey],`${d.id}: missing specific pair review`);
 let review=rule?.status?'来源错误/恢复及断点信息；不当作对象生命周期。':rule?.suffix==='other'?`${s.current} 兜底原始事件，不计为已实现的成对过程。`:rule?ruleReviews[ruleKey]:s.current;
 return {id:d.id,name:d.name,source_key:key,kind,configurable:controls.some(c=>c.id===d.id),type_ids:s.type_ids,review,rule:rule||null,declared_event_actions:actions,gaps:rule?.status?[]:s.gaps};
});
assert.equal(new Set(itemRows.map(x=>x.id)).size,definitions.length,'duplicate items');
for(const c of controls)assert(itemRows.some(i=>i.id===c.id),`uncovered control ${c.id}`);
assert.deepEqual(sourceRows.flatMap(s=>s.control_ids).sort(),controls.map(c=>c.id).sort(),'source/control association drift');
assert.deepEqual(sourceRows.flatMap(s=>s.definition_ids).sort(),definitions.map(d=>d.id).sort(),'source/definition association drift');
for(const t of catalog.types){const r=types.get(t.id);r.name=t.name;r.category=t.category;r.source_ids=unique(catalog.sources.filter(s=>s.types.includes(t.id)).map(s=>s.source));assert(r.source_ids.length,`uncovered type ${t.id}`);for(const s of r.source_ids)assert(sources.has(s),`unknown type source ${s}`);}
const inputPaths=['model/sources.json','model/catalog.json','model/native-migration.json','apps/desktop/ui/collection-catalog.json','apps/desktop/ui/collection-controls.json','crates/storage/src/projection.rs','crates/storage/src/identity.rs','crates/storage/src/evidence.rs','crates/storage/src/item_controls.rs','crates/contracts/src/adapters/event.rs','crates/contracts/src/adapters/event_state_adapter.rs','crates/contracts/src/adapters/source_gaps.rs','crates/contracts/src/reference.rs','plugins/eventlog-native/Program.cs','plugins/eventlog/src/main.rs','plugins/usn/src/main.rs','plugins/eventlog/channels.json'];
const hashes=Object.fromEntries(inputPaths.map(p=>[p,crypto.createHash('sha256').update(fs.readFileSync(path.join(root,p))).digest('hex')]));
const report={date:'2026-10-15',product_version:'0.2.4',basis:'current_source_review',verification:'逐项源码覆盖审查；没有执行真实驱动安装/卸载等破坏性操作；未声称全部端到端实机通过。',counts:{definitions:definitions.length,controls:controls.length,sources:sourceRows.length,types:typeRows.length,event_channels:Object.keys(eventNotes).length,event_rules:definitions.filter(d=>d.event_rule).length,native_sources:sourceRows.filter(s=>s.backend==='native').length,compatibility_sources:sourceRows.filter(s=>s.backend==='powershell_compatibility').length,virtual_sources:sourceRows.filter(s=>s.virtual).length,uncovered_definitions:0,uncovered_controls:0,uncovered_sources:0,uncovered_types:0},issues,sources:sourceRows,items:itemRows,types:typeRows,input_sha256:hashes};
assert.equal(report.counts.event_channels,read('plugins/eventlog/channels.json').length,'event channel coverage drift');
const esc=x=>String(x??'').replace(/\|/g,'\\|').replace(/\r?\n/g,'<br>');
const list=a=>a.map(esc).join('<br>')||'—';
const nav='[总评与整改顺序](collection-lifecycle-audit.md) · [101 类对象](collection-lifecycle-types.md) · [133 个来源](collection-lifecycle-sources.md) · [225 个采集定义](collection-lifecycle-items.md) · [可核对数据](collection-lifecycle-audit.json)';
const banner='本表逐项审查当前代码与来源，**覆盖不等于功能已完整或实机通过**。新增、离开清单、状态切换、操作开始/结果分别核对；明确缺口见总评。\n\n';
let typeDoc='# 全部对象的成对变化审查\n\n'+nav+'\n\n'+banner+'| 对象类型 | 应检查的过程 | 当前来源能提供什么 | 身份与判定边界 | 缺口 |\n|---|---|---|---|---|\n';
for(const t of typeRows)typeDoc+=`| ${esc(t.name)}<br>\`${t.id}\` | ${esc(t.pairs)} | ${esc(t.current)} | ${esc(t.identity)} | ${t.gaps.join(', ')} |\n`;
let sourceDoc='# 全部来源的成对变化审查\n\n'+nav+'\n\n'+banner+'后端“原生”按当前归属清单核对；旧迁移清单中的 PowerShell API 名称不作为当前生产执行路径。兼容来源只剩四项 Hyper-V。\n\n| 来源/项目 | 后端与材料 | 类型与定义数 | 当前配对能力/缺口 | 适用环境 |\n|---|---|---|---|---|\n';
for(const s of sourceRows)sourceDoc+=`| ${esc(s.name)}<br>\`${s.source}\` | ${esc(s.backend)}<br>${list(s.material)} | ${list(s.type_ids)}<br>${s.definition_ids.length} 个定义 / ${s.control_ids.length} 个开关 | ${esc(s.current)}<br>${s.gaps.join(', ')} | ${list(s.environment)} |\n`;
let itemDoc='# 全部采集定义与开关的审查\n\n'+nav+'\n\n'+banner+'225 个定义逐行列出；其中 222 个有单独开关。状态/其他事件分类也计入，避免只检查有明确事件编号的规则。事件标题列出编号与来源名称；动作在模型中注册仍不代表已完成跨记录关联。\n\n| 采集定义 | 性质/开关 | 事件范围或对象 | 审查结果 |\n|---|---|---|---|\n';
for(const i of itemRows){const rule=i.rule;const range=rule?`提供程序：${rule.provider||'通道范围'}；编号：${(rule.event_ids||[]).join(', ')||'按分类/兜底读取'}${i.declared_event_actions.length?'<br>'+i.declared_event_actions.map(a=>`${a.event_id} ${a.name}${a.catalog_action?'〔模型动作〕':'〔事件解释〕'}`).join('<br>'):''}`:list(i.type_ids);itemDoc+=`| ${esc(i.name)}<br>\`${i.id}\` | ${i.kind}<br>${i.configurable?'有开关':'无独立开关（汇总/诊断范围）'} | ${range} | ${esc(i.review)}<br>${i.gaps.join(', ')} |\n`;}
let main='# 采集全过程与成对变化审查（0.2.4）\n\n'+nav+'\n\n';
main+='结论：原生采集已覆盖大部分当前状态，但“读取了清单”和“用户能看到完整的来去过程”之间仍有共性缺口。不能因为两种原始日志都被读入，就宣布安装/卸载、启停或开始/结果已经闭环。\n\n';
main+=`本次逐项覆盖 **${definitions.length} 个定义、${controls.length} 个开关、${sourceRows.length} 个来源、${typeRows.length} 类对象、23 个日志通道、109 个日志分类**，未覆盖项为 **0**。当前来源归属为 127 原生、4 Hyper-V 兼容、2 汇总/导入。验证口径为源码审查与覆盖校验，不是 225 项真实操作验收。\n\n`;
main+='## 当前必须先补的四处\n\n';
main+='1. **首次出现**：此前只记入原始差异，没进入对象时间线；0.2.4 已补，首轮基线保持安静。\n2. **再次出现**：同一个对象离开后回来，不能继续只认第一次看到。\n3. **子项增减**：组成员、权限、浏览器策略、排除项、环境变量、ACL、文件流等，需要对子集合逐项差分。\n4. **身份替换**：驱动版本等改变身份时，同一源行的 `object.changed` 要拆出旧绑定离开和新绑定加入，不能只等源行新增/删除。\n\n';
main+='这四项解决后，再做运行启停、操作结果和多来源关联。单例配置、采样、能力诊断没有自然的安装/删除对，应保留状态变化，不制造事件。\n\n';
main+='**另有已确认的 P0 生产断点 G12**：Sysmon 所有事件在原生采集器中统一为 `execution.sysmon_event`，后端回退映射不支持该提供程序，目录里的事件动作没有参与改写。于是进程启退、驱动/模块加载、连接、文件创建/删除虽有界面分类，当前不能据此认定已进入对象行动。验收必须用原生实际信封，不能只用手工填写精确 action kind 的测试材料。\n\n';
main+='## 驱动应拆成五层\n\n| 层次 | 加入端 | 移除/结束端 | 当前状态 |\n|---|---|---|---|\n';
main+='| 驱动仓库包 | 仓库中新增包 | 仓库不再列出该包 | 原生 DISM 已有全清单；0.2.4 接通首次出现；重复加入、安装过程仍缺。身份为 INF＋仓库路径。 |\n';
main+='| 驱动服务登记 | 服务登记加入 | 服务登记移除 | 原生 Win32_SystemDriver；清单对不等于驱动包安装/卸载。 |\n';
main+='| 设备驱动绑定 | 设备绑定到版本 B | 旧版本 A 绑定解除 | 设备 ID 不变时原始行是 changed；A→B→A 必须专项补齐，列为 P0。 |\n';
main+='| 运行状态/加载 | 驱动开始运行；Sysmon 6 加载日志 | 驱动停止运行；真正卸载事件 | 原生 State 可作字段对比；Sysmon 加载事件还存在 G12 生产断点，也未接对应卸载事件。 |\n';
main+='| 安装操作 | 开始安装/更新/卸载 | 成功、失败、回滚、需要重启 | 当前没有 SetupAPI 安装过程解析；UMDF 通道已读原始日志，尚未实现包级结果关联。 |\n\n';
main+='用户时间线建议分别显示“新增驱动包”“仓库中已无此驱动包”“设备驱动：A → B”“驱动已开始运行／已停止”“驱动安装失败（原因）”。只有明确操作结果才写安装/卸载成功。多个包共享 INF、包存在但未绑定设备、设备已移除但包仍保留、安装失败回滚都要单独验证。\n\n';
main+='## 成对过程的统一判定\n\n';
main+='- **清单增减**：以前完整清单无该对象、这次有，显示新发现；同范围完整清单从有到无，按类型显示离开/移除。首次基线不当新增操作。\n- **状态变化**：运行/停止、启用/停用、健康/故障、连接/断开，记录同对象的前后值；未知和读取失败不参与开关判定。\n- **操作过程**：开始、成功、失败、取消、回滚按来源分别记录。同次操作用明确的 ActivityId/TaskInstanceId/包或设备身份关联，不按“时间差不多”硬配。\n- **重现与版本**：再次出现要记录重现；确认终结后按代际建新实例。A→B 是替换还是并存由实际身份/产品锚点判定。\n- **来源关系**：同一对象多个来源可以互补。一个来源读失败/禁用、一个 COM 角色移除，不使整个对象消失。\n- **时间**：快照仅给出两次完整检查之间的区间；有明确日志时间才用精确时间。界面的 30 秒时间分组只影响排版，不合并成同一次操作。\n- **模块文件**：sihost 等目标仍融入进程/模块/文件体系；成功观察模块后跟踪磁盘文件变化，校验失败/缺失/恢复是文件状态，不能直接认定进程启动失败原因。\n\n';
main+='## 整改与验收队列\n\n| 编号 | 优先级 | 问题与当前结论 | 验收门槛 |\n|---|---|---|---|\n';
for(const i of issues)main+=`| ${i.id} | ${i.priority} | **${i.name}**：${esc(i.current)} | ${esc(i.acceptance)} |\n`;
main+='\n顺序：G02/G03/G08 共性投影与 G12 生产事件分类 → G04 暂定身份/短生命周期 → G06 各类型状态文案 → G07/G09 日志过程与版本归并 → G10 多来源专项 → G11 环境验收。G01 是本次已实现项，其余不标完成。\n\n';
main+='## 环境与性能约束\n\n';
main+='继续采用原生后端采集；127 个原生来源失败后不回退 PowerShell。四项 Hyper-V 等有环境再迁移。普通/增强权限、当前用户/机器范围、已启用日志与可读保留历史分别记录。CNG 某提供程序失败只阻断该提供程序/范围，不能移除其旧密钥；无 Docker、无电池、无 Hyper-V 等按环境能力处理。\n\n';
main+='状态差分只比较同范围、完整、成功且开关启用的相邻集合；绑定与水位持久保存，重发/重启可去重。优先使用变化通知唤醒有界复核，单个来源上轮完成再安排下轮；文件抖动合并、稳定后复核，排除历史数据库/WAL/SHM 和程序自身产物。不得为配对逐对象启动 PowerShell，不通过高频全盘/全注册表轮询补另一端。所有新事件流需有每轮预算、队列上限、背压与断点。\n\n';
main+='## 核对依据与复核命令\n\n';
main+='采集定义/开关来自 `collection-catalog.json` / `collection-controls.json`；来源和对象来自 `model/sources.json` / `model/catalog.json`；原生归属来自 `native-migration.json` 和实际 emitters。通用新增/缺席、子对象、日志动作分别检查 `crates/storage/src/projection.rs`、`identity.rs`、`evidence.rs`、`crates/contracts/src/adapters/event*.rs`、`source_gaps.rs`、`reference.rs`。前端事件标题和当前状态文案分别检查 `event-semantics.js`、`type-presentation.js`，不将已有标题计作后端配对完成。\n\n';
main+='运行 `node scripts/build-lifecycle-audit.cjs --check` 核对输入摘要、133/225/222/101 的完整覆盖和全部生成表；输入改变时需重新审查并运行不带 `--check` 的生成命令。该命令仅检查清单一致性，不替代驱动安装、会话、文件或虚拟化实际流程验收。\n';
const outputs={
 'docs/refactor/collection-lifecycle-audit.json':JSON.stringify(report,null,2)+'\n',
 'docs/refactor/collection-lifecycle-audit.md':main,
 'docs/refactor/collection-lifecycle-types.md':typeDoc,
 'docs/refactor/collection-lifecycle-sources.md':sourceDoc,
 'docs/refactor/collection-lifecycle-items.md':itemDoc,
};
for(const [p,text]of Object.entries(outputs)){if(process.argv.includes('--check'))assert.equal(fs.readFileSync(path.join(root,p),'utf8'),text,`audit needs review/regeneration: ${p}`);else fs.writeFileSync(path.join(root,p),text);}
console.log(JSON.stringify({mode:process.argv.includes('--check')?'check':'generate',...report.counts,issues:issues.length}));
