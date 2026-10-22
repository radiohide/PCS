'use strict';
// S03 object types. One row per type, referencing a named identity strategy from vocabulary.cjs so
// that types state only what differs. Field kind suffixes: i=identity, c=configuration, s=state.
//
// row = id | name | identity strategy | extra identity fields | key fields | actions | overrides
//   extra identity fields: appended to the strategy defaults (comma list, '-' for none)
//   key fields:            id:label:kind (comma list) — every cell needs all three parts
//   actions:               action IDs from vocabulary.cjs ('-' for none)
//   overrides:             scope=…;normalization=…;reuse=…;insufficient=… ('-' to inherit)
//
// A type listed here without an implementation source is listed in typesWithoutSource below;
// listing a type is not a claim that it is collected.
const types=[
// ---------- 账户与身份 ----------
'account.user|用户|account_sid|-|name:名称:c,enabled:是否启用:c,last_logon:最近登录:s,password_required:是否要求密码:c|identity.account_created,identity.account_deleted,identity.account_changed,identity.account_renamed,identity.account_enabled,identity.account_disabled,identity.account_locked,identity.account_unlocked,identity.logon_failed|-',
'account.group|用户组|account_sid|-|name:名称:c,members:成员:c|-|-',
'account.group_membership|组成员登记|composite_native|group_sid,member_sid|group:组:i,member:成员:i|identity.group_member_added,identity.group_member_removed|scope=安装作用域与组 SID',
'account.user_profile|用户配置文件|account_sid|-|path:配置文件路径:c,loaded:是否已加载:s,last_use:最近使用:s|-|-',
'account.right_assignment|账户权限分配|composite_native|account_sid,privilege|privilege:权限:i|-|scope=安装作用域与账户 SID',
'account.machine_identity|计算机身份与域信息|native_identifier|-|name:计算机名称:c,domain:域或工作组:c,role:角色:c,manufacturer:制造商:c,model:型号:c,uuid:机器 UUID:i|-|-',
// ---------- 软件与组件 ----------
'software.installation_registration|软件安装登记|registry_registration|-|path:登记路径:c,display_name:显示名称:c,display_version:版本:c,publisher:发布者:c,install_location:安装位置:c,uninstall_command:卸载命令:c|-|scope=安装作用域 + 注册表视图；同名软件不一定是同一登记',
'software.appx_package|AppX 包|named_entity|-|name:包名称:c,version:版本:c,publisher:发布者:c,install_location:安装位置:c,status:状态:s|-|scope=用户作用域 + 包全名',
'software.provisioned_package|预配包|named_entity|-|name:包名称:c,version:版本:c,publisher:发布者:c,install_location:安装位置:c|-|scope=本机映像作用域 + 包全名',
'software.optional_feature|Windows 可选组件|named_entity|-|name:组件名称:c,state:状态:s|-|scope=本机作用域 + 组件名称',
'software.runtime|运行时|composite_native|runtime_family,runtime_version|family:运行时家族:i,version:版本:i,install_path:安装路径:c|-|scope=安装作用域（含 32/64 位视图）',
'software.portable_registration|便携程序登记|file_identity|-|path:文件路径:i,version:版本:c,company:公司:c,size:大小:s,sha256:内容摘要:s|-|scope=配置的便携程序根目录；路径文件登记不证明程序已安装',
'software.system_update|系统更新组件|native_identifier|-|hotfix_id:更新编号:i,installed_on:安装日期:c,description:说明:c|-|scope=安装作用域；仅 QFE 范围，不等于完整更新历史',
'software.update_history_subject|更新历史|composite_native|update_id,revision|name:更新名称:c,update_id:更新标识:i,revision:修订号:i|software.update_history|scope=安装作用域；历史操作主体，安装状态由组件清单另行维护',
'software.language_package|语言包登记|composite_native|package_manager,package_name,package_version|manager:包管理器:i,name:包名称:i,version:版本:i,path:安装路径:c|-|scope=安装作用域 + 包管理器；自定义前缀目录不覆盖',
// ---------- 运行实例 ----------
'runtime.process|进程|process_instance|-|pid:进程 ID:i,name:程序名称:c,image:程序路径:c,parent_pid:父进程 ID:c,session:会话:c,created_at:创建时间:i,command_line:命令行:c|execution.process_started,execution.process_stopped,execution.remote_thread,execution.process_access,execution.process_tampering,execution.pipe_created,execution.pipe_connected|-',
'runtime.module_instance|模块加载实例|composite_native|process_identity,module_path|process:所属进程:i,process_start:进程创建时间:i,process_name:进程名称:c,session:会话:c,user_sid:账户 SID:c,module_path:模块路径:i,module_base:观察基址:s,size:映像大小:s,version:磁盘版本:s,signature_status:磁盘签名检查状态:s,module_origin:发布者类别:s|execution.module_loaded|scope=安装作用域；模块身份含进程 PID 与建立时间，磁盘签名不证明内存字节',
'runtime.logon_session|登录会话|native_identifier|-|logon_id:登录 ID:i,logon_type:登录类型:c,start_time:开始时间:c,authentication_package:认证包:c|identity.logon,identity.logoff,identity.special_privileges|scope=安装作用域 + 登录 ID；已确认启动周期时隔离会话；特殊权限事件缺启动周期时不关联会话',
'runtime.terminal_session|终端会话|native_identifier|-|session_id:会话 ID:i,station:站点:c,user:用户:c,domain:域:c,state:状态:s|identity.session_locked,identity.session_unlocked,identity.session_reconnected,identity.session_disconnected|-',
// ---------- 自动执行入口 ----------
'startup.service|服务|named_entity|-|name:服务名称:i,display_name:显示名称:c,state:状态:s,start_mode:启动模式:c,account:运行账户:c,binary_path:程序路径:c|startup.service_installed,startup.service_state_changed|scope=安装作用域 + 服务名称；显示名称可能变化或重复，不作为键',
'startup.scheduled_task|计划任务|native_path|-|path:任务路径:i,name:名称:c,enabled:是否启用:c,principal:运行账户:c,actions:执行动作:c,triggers:触发条件:c,next_run:下次运行时间:s,last_run:最近运行:s,last_result:最近结果:s|startup.task_created,startup.task_deleted,startup.task_updated,startup.task_started,startup.task_ended|scope=安装作用域 + 完整任务路径；不同文件夹的同名任务不合并',
'startup.logon_startup_entry|登录启动项|value_key|-|location:登记位置:i,value_name:值名:i,command:命令:c|-|scope=安装或用户作用域 + 注册表视图 + 登记位置',
'startup.logon_initialization|登录初始化配置|registry_registration|-|path:注册表路径:i,values:配置值:c|-|scope=安装作用域 + 注册表视图',
'startup.wmi_filter|WMI 订阅筛选器|native_path|-|name:名称:i,namespace:命名空间:i,query_language:查询语言:c,query:查询:c|execution.wmi_filter|scope=WMI 命名空间作用域；这是订阅配置，不是主动订阅事件',
'startup.wmi_consumer|WMI 订阅消费者|native_path|-|name:名称:i,namespace:命名空间:i,consumer_type:消费者类型:c,script_engine:脚本引擎:c,program:程序路径:c|execution.wmi_consumer|scope=WMI 命名空间作用域',
'startup.wmi_binding|WMI 订阅绑定|composite_native|namespace,filter_name,consumer_name|namespace:命名空间:i,filter:筛选器:i,consumer:消费者:i,security_context:安全上下文:c|execution.wmi_binding|scope=WMI 命名空间作用域；过滤器、消费者与绑定各自保留真实身份',
'startup.ifeo_configuration|IFEO 与注入入口|registry_registration|-|path:注册表路径:i,values:配置值:c|-|scope=安装作用域 + 注册表视图',
// ---------- 系统扩展与关联 ----------
'shell.preview_handler|预览处理程序|registry_registration|-|path:登记路径:c,name:名称:i,hive:注册表配置单元:i,view:注册表视图:i,clsid:CLSID:c|-|scope=用户或机器作用域 + 注册表视图',
'shell.icon_overlay_handler|图标覆盖处理程序|registry_registration|-|path:登记路径:c,name:名称:i,hive:注册表配置单元:i,view:注册表视图:i,clsid:CLSID:c|-|scope=用户或机器作用域 + 注册表视图；登记不证明已加载',
'shell.shell_extension|Shell 扩展登记|registry_registration|-|name:名称:i,hive:注册表配置单元:i,view:注册表视图:i,approved:允许状态:c|-|scope=用户或机器作用域 + 注册表视图 + 允许或阻止列表',
'shell.namespace_extension|命名空间扩展|registry_registration|-|path:登记路径:c,name:名称:i,hive:注册表配置单元:i,view:注册表视图:i,clsid:CLSID:c|-|scope=用户或机器作用域 + 注册表视图',
'shell.context_menu_registration|右键菜单登记|registry_registration|-|path:注册表路径:i,command:命令:c,default_icon:默认图标:c|-|scope=用户或机器作用域 + 注册表视图 + 关联类型',
'shell.file_association|文件关联|registry_registration|-|path:登记路径:c,extension:扩展名:i,prog_id:程序标识:c,command:命令:c|-|scope=用户或机器作用域 + 注册表视图；主归系统扩展与关联',
'shell.com_registration|COM 注册|composite_native|hive,view,clsid|clsid:CLSID:i,hive:注册表配置单元:i,view:注册表视图:i,name:登记名称:i,server_path:进程内实现文件:c,local_server_command:独立程序启动命令:c,file_version:文件版本:c,sha256:内容摘要:c|-|scope=用户或机器作用域 + 32/64 位视图；同一 CLSID 的不同注册位置不无条件合并',
'shell.browser_extension|浏览器扩展|composite_native|browser,profile,extension_id,version|browser:浏览器:i,profile:配置文件:i,extension_id:扩展 ID:i,version:版本:i,name:名称:c,permissions:权限:c,state:状态:s|-|scope=浏览器 + 用户配置文件 + 扩展 ID；不判定是否实时激活',
// ---------- 文件与访问控制 ----------
'file.watched_file|监控文件|file_identity|-|path:路径:i,exists:是否存在:s,size:大小:s,creation_time:创建时间:c,last_write:最后写入:c,attributes:属性:c,owner:所有者:c,dacl:访问控制:c,sha256:内容摘要:s|files.created,files.deleted,files.deleted_archived,files.creation_time_changed,files.executable_detected,files.executable_blocked,files.shredding_blocked,files.usn_changed,files.access_audit|scope=用户配置的文件名单；空名单不枚举；名单移除不产生删除动作',
'file.acl_entry|文件系统权限登记|composite_native|file_identity,trustee,access_mask|trustee:被授权者:i,access:访问权限:i,inherited:是否继承:c|files.permissions_audit|scope=文件身份作用域；只读 DACL，不含 SACL',
'file.alternate_data_stream|备用数据流|composite_native|file_identity,stream_name|stream_name:流名称:i,size:大小:s|files.stream_created|scope=文件身份作用域；只记录流名称与大小，不保存内容',
'file.watched_directory|监控目录|file_identity|-|path:路径:i,exists:是否存在:s,attributes:属性:c,owner:所有者:c,dacl:访问控制:c,entries:直接子项:s,last_write:最近写入:s|-|scope=用户显式配置的目录名单；只读直接子项，不递归',
// ---------- 策略与配置 ----------
'policy.environment_variable|环境变量|value_key|-|name:变量名:i,scope:作用域:i,value:变量值:c|-|scope=用户或机器作用域分别建实例；未设置不等于关闭',
'policy.uac_policy|UAC 策略|value_key|-|path:注册表路径:i,name:值名:i,value:值:c|-|scope=安装作用域 + 注册表视图',
'policy.software_restriction_policy|软件限制策略|value_key|-|path:注册表路径:i,name:值名:i,value:值:c|-|scope=安装作用域 + 注册表视图',
'policy.browser_policy|浏览器策略|value_key|-|browser:浏览器:i,path:策略路径:i,name:策略名:i,value:值:c|-|scope=浏览器 + 用户或机器作用域；浏览器策略主归策略与配置',
'policy.time_sync_configuration|时间同步配置|native_path|-|path:注册表路径:i,time_zone:时区:c,values:配置值:c|-|scope=安装作用域 + 注册表视图',
'policy.power_plan|电源计划|native_identifier|-|guid:计划 GUID:i,name:计划名称:c,active:是否激活:s,power_source:电源来源:s|-|scope=安装作用域',
'policy.os_configuration|操作系统配置与状态|native_path|-|path:配置路径:i,caption:系统名称:c,version:版本:c,build:生成号:c,install_time:安装时间:c,boot_time:最近启动时间:s,reboot_pending:待重启标志:s|-|scope=安装作用域；安装时间与首次观察时间不同',
'policy.system_restore_point|系统还原点|native_identifier|-|sequence:序号:i,description:说明:c,creation_time:建立时间:c,type:类型:c|-|scope=安装作用域',
// ---------- 网络与远程访问 ----------
'network.adapter|网络适配器|native_identifier|-|guid:适配器 GUID:i,name:名称:c,status:状态:s,mac:物理地址:c,speed:速率:s,driver_version:驱动版本:c,hidden:是否隐藏:c|-|scope=安装作用域 + 适配器 GUID；改名不生成新身份',
'network.address_configuration|IP 地址配置|composite_native|interface_guid,address,prefix_length|interface:接口:i,address:地址:i,prefix_length:前缀长度:i,family:地址族:c,state:状态:s|-|scope=接口 GUID 作用域',
'network.route|路由|composite_native|interface_guid,destination_prefix,next_hop|interface:接口:i,destination:目的前缀:i,next_hop:下一跳:i,metric:度量:c,policy_source:策略来源:c|-|scope=接口 GUID 作用域',
'network.dns_configuration|DNS 配置|composite_native|interface_guid,address_family|path:登记路径:c,interface:接口:i,servers:DNS 服务器:c,search_list:搜索列表:c|-|scope=接口 GUID 作用域',
'network.dns_cache_entry|DNS 缓存项|composite_native|name,record_type|name:名称:i,record_type:记录类型:i,data:解析值:s|network.dns_query|scope=安装作用域；缓存过期不解释为用户删除',
'network.hosts_entry|hosts 静态映射|composite_native|address,names|address:地址:i,names:名称:i|-|scope=安装作用域；解析有效条目，排除注释',
'network.proxy_configuration|代理配置|value_key|-|scope:作用域:i,path:配置路径:i,enabled:是否启用:c,server:代理服务器:c,bypass:绕过列表:c|-|scope=当前用户或机器作用域',
'network.vpn_configuration|VPN 配置|native_identifier|-|name:连接名称:i,server:服务器:c,tunnel_type:隧道类型:c,status:状态:s|-|scope=当前用户作用域；启用不证明发生过连接',
'network.remote_access_configuration|远程访问配置|native_path|-|service:服务:i,path:配置路径:i,values:配置值:c|-|scope=安装作用域 + 服务名',
'network.firewall_profile|防火墙配置集|native_identifier|-|name:配置集名称:i,inbound:入站动作:c,outbound:出站动作:c,enabled:是否启用:c,log_path:日志路径:c|-|scope=安装作用域',
'network.firewall_rule|防火墙规则|composite_native|policy_store,name|display_name:显示名称:c,name:名称:i,direction:方向:c,action:动作:c,enabled:是否启用:c,profile:适用配置集:c,instance_id:实例 ID:i|-|scope=策略存储作用域（ActiveStore 等）；完整过滤条件可追溯',
'network.ipsec_rule|IPsec 规则|native_identifier|-|name:名称:i,security_mode:安全模式:c,policy_source:策略来源:c|-|scope=策略存储作用域',
'network.endpoint|网络连接与监听端点|endpoint_tuple|-|protocol:协议:c,local_address:本地地址:i,local_port:本地端口:i,remote_address:远端地址:i,remote_port:远端端口:i,state:状态:s,owning_pid:所属进程:c,creation_time:创建时间:i|network.connection_observed,network.connection_allowed,network.connection_blocked|scope=安装作用域；端点复用不拼接不同连接；未采到进程时不猜测归属',
'network.smb_share|SMB 共享|named_entity|-|name:共享名称:i,path:路径:c,share_acl:共享权限:c,special:是否特殊共享:c|network.share_access|scope=安装作用域 + 共享名称',
'network.smb_mapping|SMB 驱动器映射|composite_native|local_name,remote_path|local_name:本地盘符:i,remote_path:远端路径:i,status:状态:s|-|scope=用户作用域',
'network.smb_session|SMB 客户端会话|composite_native|client,user,server|client:客户端:i,user:用户:i,server:服务器:c,open_count:打开数:s,protocol:协议:c,encrypted:是否加密:s|-|scope=安装作用域',
'network.connection_profile|网络连接配置文件|native_identifier|-|name:网络名称:i,category:类别:c,ipv4_connectivity:IPv4 连通性:s,ipv6_connectivity:IPv6 连通性:s|-|scope=安装作用域',
// ---------- 安全与信任 ----------
'security.certificate_registration|证书登记|composite_native|store_scope,store_name,thumbprint|store:存储区:i,location:存储位置:i,thumbprint:指纹:i,subject:主题:c,issuer:颁发者:c,not_before:生效时间:c,not_after:失效时间:c,has_private_key:是否有私钥:c|-|scope=存储区作用域 + 证书指纹；同一证书在不同存储区的登记分别存在',
'security.key_metadata|密钥元数据对象|native_identifier|-|provider:提供程序:i,name:密钥名称:i,algorithm:算法:c,legacy_key_spec:旧式密钥类别:c|-|scope=提供程序及用户或机器作用域；仅 CNG 身份与元数据，不读取私钥内容',
'security.protection|防护配置与状态|named_entity|-|name:产品名称:i,engine_version:引擎版本:s,signature_version:特征库版本:s,real_time:实时防护:s,real_time_configured:实时防护设置:c,tamper_protected:防篡改:s,scan_time:最近扫描时间:s|-|scope=安装作用域 + 产品；不修改防护配置',
'security.detection_history|安全检测记录|composite_native|detection_id,threat_id|name:检测名称:c,detection_id:检测标识:i,threat_id:威胁标识:i|security.threat_detected,security.threat_status_recorded|scope=安装作用域；保留检测与处理历史，清单消失不改变处理结果',
'security.exclusion|防护排除项|composite_native|product,exclusion_type,value|product:产品:i,exclusion_type:排除类型:i,value:排除值:i|-|scope=产品作用域；排除项变化保留前后证据',
'security.security_product|安全产品登记|native_identifier|-|name:产品名称:i,instance_id:实例 ID:i,product_state:产品状态码:s|-|scope=安装作用域；依赖系统提供安全中心命名空间',
'security.bitlocker_configuration|BitLocker 配置|native_identifier|-|mount_point:装载点:i,volume_status:卷状态:s,protection_status:保护状态:s,encryption_method:加密方法:c,protector_types:保护器类型:c|-|scope=卷作用域；不采集恢复材料',
'security.ssh_public_key|SSH 公钥登记|composite_native|file_path,algorithm|path:文件路径:i,algorithm:算法:i,fingerprint:指纹:i|-|scope=用户作用域；不保存密钥材料与注释',
'security.device_guard_configuration|Device Guard 配置|native_identifier|-|vbs_status:VBS 状态:s,services_configured:已配置服务:c,services_running:运行中服务:s,available_properties:可用属性:c|-|scope=安装作用域',
'security.secure_boot_configuration|安全启动配置|named_entity|-|enabled:是否启用:c|-|scope=安装作用域；组件不支持或权限不足报告缺口',
// ---------- 硬件与驱动 ----------
'hardware.device|设备|native_identifier|-|device_id:设备 ID:i,name:设备名称:c,class:类别:c,manufacturer:制造商:c,service:关联服务:c,status:状态:s,config_error:配置错误码:s|-|scope=安装作用域 + 设备 ID；名称是可变展示属性',
'hardware.driver_binding|设备驱动绑定|composite_native|device_id,driver_version|name:名称:c,device:设备:i,driver_version:驱动版本:i,driver_date:驱动日期:c,provider:提供方:c,inf:驱动 INF:c,signed:签名状态:c|-|scope=设备作用域',
'hardware.driver_service|驱动服务|named_entity|-|name:服务名称:i,driver_path:驱动路径:c,service_type:服务类型:c,start_mode:启动模式:c,state:状态:s|execution.driver_loaded|scope=安装作用域 + 驱动服务名称；驱动服务主归硬件与驱动',
'hardware.disk|磁盘|native_identifier|-|name:名称:c,number:磁盘号:i,bus_type:总线类型:c,partition_style:分区样式:c,size:容量:c,is_boot:是否启动盘:s,is_system:是否系统盘:s,health:健康状态:s,operational_status:运行状态:s|storage.raw_read|scope=安装作用域 + 磁盘号与总线；盘符变化不影响身份',
'hardware.partition|分区|native_identifier|-|disk_number:磁盘号:i,partition_number:分区号:i,guid:分区 GUID:i,offset:偏移:c,size:大小:c,access_paths:访问路径:c|-|scope=磁盘作用域 + 分区号',
'hardware.volume|卷|native_identifier|-|volume_id:卷标识:i,drive_letter:盘符:c,filesystem:文件系统:c,size:容量:c,volume_type:卷类型:c|-|scope=安装作用域 + 卷标识；盘符是可变的展示属性',
'hardware.driver_package|驱动包|native_identifier|-|name:包名称:i,provider:提供方:c,version:版本:c,inf:驱动 INF:c,signed:签名状态:c|-|scope=本机驱动仓库作用域；由 DISM 发布 INF 标识',
'hardware.firmware|固件|native_identifier|-|manufacturer:制造商:i,bios_version:BIOS 版本:c,smbios_version:SMBIOS 版本:c,release_date:发布日期:c|-|scope=安装作用域',
'hardware.tpm|TPM|named_entity|-|present:是否存在:s,ready:是否就绪:s,enabled:是否启用:s,activated:是否已激活:s,manufacturer:制造商:c,version:版本:c,locked_out:是否锁定:s|-|scope=安装作用域',
'hardware.battery|电池|native_identifier|-|name:名称:i,chemistry:化学类型:c,charge:余量:s,estimated_time:估算时间:s|-|scope=安装作用域 + 电池标识',
// ---------- 虚拟环境 ----------
'virtualization.virtual_machine|虚拟机|native_identifier|-|vm_id:虚拟机 ID:i,name:名称:c,state:状态:s,generation:代数:c,cpu:处理器配置:c,memory:内存配置:c|-|scope=虚拟化引擎作用域；引擎未运行不表示虚拟机已删除',
'virtualization.virtual_switch|虚拟交换机|native_identifier|-|switch_id:交换机 ID:i,name:名称:c,type:类型:c,adapter:关联网卡:c|-|scope=虚拟化引擎作用域',
'virtualization.disk_attachment|虚拟磁盘连接|composite_native|vm_id,controller_type,controller_number,controller_location|vm:虚拟机:i,path:磁盘路径:i,controller_type:控制器类型:i,controller_number:控制器号:i,controller_location:控制器位置:i|-|scope=虚拟机作用域；不读取磁盘内容',
'virtualization.checkpoint|虚拟机检查点|native_identifier|-|checkpoint_id:检查点 ID:i,name:名称:c,creation_time:建立时间:c,type:类型:c,parent:父检查点:c|-|scope=虚拟机作用域',
'virtualization.wsl_distribution|WSL 发行版|native_identifier|-|name:发行版名称:i,version:版本:c,location:位置:c,default_uid:默认用户:c|-|scope=用户作用域 + 发行版标识；不启动 WSL',
'virtualization.container|容器|native_identifier|-|container_id:容器 ID:i,names:名称:c,image:镜像:c,state:状态:s,status:运行状态:s|-|scope=容器引擎作用域；不读取日志、环境变量与容器内容',
'virtualization.image|容器镜像|composite_native|image_id,repository,tag|repository:仓库:i,tag:标签:i,image_id:镜像 ID:i|-|scope=容器引擎作用域',
'virtualization.container_volume|容器卷|native_identifier|-|name:卷名称:i,driver:驱动:c|-|scope=容器引擎作用域',
'virtualization.container_network|容器网络|native_identifier|-|network_id:网络 ID:i,name:名称:i,driver:驱动:c,scope:作用域:c|-|scope=容器引擎作用域',
'virtualization.platform_capability|虚拟化能力与平台|named_entity|-|hypervisor_present:虚拟化监控程序存在:c,firmware_virtualization:固件虚拟化:c,monitor_extensions:监控扩展:c,slat:二级地址转换:c|-|scope=安装作用域；能力标志不是虚拟机实例',
// ---------- 桌面与应用集成 ----------
'desktop.printer|打印机|named_entity|-|name:名称:i,driver:驱动:c,port:端口:c,shared:是否共享:c,status:状态:s|-|scope=安装作用域 + 打印机名称',
'desktop.printer_port|打印端口|named_entity|-|name:端口名称:i,port_type:端口类型:c,host:主机地址:c,protocol:协议:c,port_number:端口号:c|-|scope=安装作用域 + 端口名称',
'desktop.printer_driver|打印驱动|composite_native|name,environment|name:名称:i,environment:环境:i,manufacturer:制造商:c,driver_path:驱动路径:c,inf:驱动 INF:c|-|scope=打印驱动环境作用域',
'desktop.print_job|打印作业|composite_native|printer,job_id|printer:打印机:i,job_id:作业 ID:i,submitted_at:提交时间:c,size:大小:c,pages:页数:c,status:状态:s|-|scope=打印机作用域；作业与打印机分开；不保存文档名与正文',
'desktop.input_configuration|输入法配置|native_path|-|path:注册表路径:i,layout_file:布局文件:c,ime_file:输入法文件:c,upper_filters:上层过滤驱动:c,lower_filters:下层过滤驱动:c|-|scope=用户或机器作用域；不枚举全部钩子',
'desktop.shell_configuration|桌面与资源管理器配置|native_path|-|path:配置路径:i,name:项名:c,value:值:c|execution.clipboard_changed|scope=用户作用域；不读取剪贴板正文',
'desktop.shortcut|快捷方式|file_identity|-|path:路径:i,target:目标:c,working_directory:工作目录:c,arguments:参数:c,icon:图标:c,hotkey:热键:c|-|scope=用户作用域；只读解析，不执行目标；快捷方式存在不代表目标执行过',
'desktop.editor_extension|编辑器扩展|composite_native|editor,extension_id,version|editor:编辑器:i,extension_id:扩展 ID:i,version:版本:i,name:名称:c,publisher:发布者:c,engines:引擎约束:c|-|scope=编辑器 + 用户配置文件',
'desktop.sync_account|同步账户登记|native_identifier|-|provider:同步提供程序:i,account:账户:i,mount_point:挂载点:c,policy:策略:c|-|scope=用户作用域；登记不表示已发生文件同步',
];

// Types that the model defines but no current source feeds. Marked explicitly so an empty list is
// never presented as a completed collection.
const typesWithoutSource=[];

module.exports={types,typesWithoutSource};
