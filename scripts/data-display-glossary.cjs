'use strict';
// Documentation-only glossary. It does not add translations to the running UI.
const entries=String.raw`
exit_code|进程或服务退出码，保留来源数值
service_specific_exit_code|服务自行定义的退出码
command_collected|是否取得运行命令
shortcut_target_collected|是否解析快捷方式目标
engines|扩展或包声明的运行引擎及版本约束
browser|浏览器类型
profile|用户配置文件或配置文件名称；具体类型由来源决定
disable_reasons|浏览器记录的扩展禁用原因代码
extension_id|扩展唯一标识
host_permissions|扩展声明的主机访问权限
manifest_version|扩展清单格式版本
optional_permissions|可选权限声明
permissions|权限声明列表
preferences_status|扩展偏好配置的读取状态
recorded_state|浏览器登记的扩展状态代码
registered_path|登记的扩展目录
active|来源报告是否处于活动状态
app_disabled|是否由应用程序禁用
signed_state|扩展签名状态代码
type|来源定义的类型；具体含义取决于本项对象
update_date|来源登记的更新时间，原始时间格式按来源保留
user_disabled|是否被用户禁用
DriverName|驱动名称
PortName|端口名称
PrinterStatus|打印机状态代码
Published|是否发布到目录服务
RenderingMode|打印渲染模式
Shared|是否共享
ShareName|共享名称
ConfigFile|配置文件路径
DataFile|驱动数据文件路径
DriverPath|驱动文件路径
InfPath|驱动安装信息文件路径
MajorVersion|主版本号
PrinterEnvironment|打印驱动适用的系统环境
PortMonitor|打印端口监视器
PortNumber|端口号
PrinterHostAddress|打印设备主机地址
Protocol|协议名称或协议编号，保持来源格式
SNMPEnabled|是否启用 SNMP
content_exclusion_reason|文档内容的采集范围说明
 document_content_collected|是否采集文档正文
pages_printed|已打印页数
printer|打印机名称
submitted_at|作业提交时间
total_pages|作业总页数
attributes|文件或对象属性标志
hotkey|快捷方式热键
icon|图标位置
reparse_target_followed|是否进入重解析点指向的目标
target|快捷方式或操作目标
window_style|快捷方式窗口显示方式
DestinationPrefix|目标网络前缀
NextHop|下一跳地址
PolicyStore|配置所属的策略存储区
Publish|路由发布设置
RouteMetric|路由度量值
ServerAddresses|服务器地址列表；DNS 来源中为 DNS 服务器
IPv4Connectivity|IPv4 连通性状态
IPv6Connectivity|IPv6 连通性状态
NetworkCategory|网络配置文件类别
AppliedSetting|应用到连接的设置类别
CreationTime|来源提供的创建时间
LocalAddress|本地地址
LocalPort|本地端口
OwningProcess|拥有该连接或端点的进程编号
RemoteAddress|远端地址
RemotePort|远端端口
AllowInboundRules|是否允许入站规则生效
AllowLocalFirewallRules|是否合并本地防火墙规则
AllowLocalIPsecRules|是否合并本地 IPsec 规则
DefaultInboundAction|默认入站动作
DefaultOutboundAction|默认出站动作
LogAllowed|是否记录允许的连接
LogBlocked|是否记录被阻止的连接
LogFileName|日志文件路径
LogMaxSizeKilobytes|日志最大大小，单位 KiB
Action|来源定义的执行、允许或阻止动作
Direction|方向，来源可能使用枚举或资源代码
EdgeTraversalPolicy|防火墙边缘遍历策略
PolicyStoreSource|策略存储来源
PolicyStoreSourceType|策略存储来源类型
PrimaryStatus|来源报告的主要状态
InboundSecurity|入站安全要求
Mode|来源运行或连接模式
OutboundSecurity|出站安全要求
AllUserConnection|VPN 是否为所有用户连接
AuthenticationMethod|认证方法
ConnectionStatus|连接状态
SplitTunneling|是否启用分流隧道
TunnelType|隧道类型
access|访问授权列表
AccessControlType|访问控制项类型
AccessRight|访问权限
AccountName|账户名称
encrypt_data|共享是否要求数据加密
special|是否为系统特殊对象
LocalPath|本地路径
RemotePath|远端路径
data|嵌套业务数据；具体结构随对象类型变化
entry|条目名称
section|来源分组或配置节
line|来源文件行信息
ClientComputerName|客户端计算机名称
ClientUserName|客户端用户名称
ContinuouslyAvailable|共享是否支持持续可用
Dialect|SMB 协议版本
Encrypted|会话是否加密
NumOpens|打开的对象数量
content_versions|是否保存文件内容版本的能力配置
 docker_metadata|是否启用 Docker 元数据采集
file_roots|配置的文件根目录
hash_files|是否启用文件哈希采集
max_files_per_root|每个根目录允许扫描的条目上限
max_hash_bytes|允许计算哈希的文件大小上限
mihomo_metadata|是否启用 Mihomo 元数据采集
module_process_names|指定采集模块列表的进程名称
portable_roots|便携软件扫描目录
watch_files|显式文件监测名单
allow_lan|是否允许局域网连接
ipv6|IPv6 设置
log_level|日志级别
meta|来源返回的附加标志
mixed_port|混合协议代理端口
port|端口
socks_port|SOCKS 代理端口
tun_enabled|TUN 是否启用
exists|指定路径在采集时是否存在
module_path|模块文件路径
process_name|进程名称
process_start|进程建立时间
AMEngineVersion|Defender 引擎版本
AMProductVersion|Defender 产品版本
AMServiceEnabled|反恶意软件服务是否启用
AntispywareEnabled|反间谍软件功能是否启用
AntivirusEnabled|防病毒功能是否启用
AntivirusSignatureLastUpdated|病毒情报最近更新时间
AntivirusSignatureVersion|病毒情报版本
BehaviorMonitorEnabled|行为监控是否启用
FullScanEndTime|最近完整扫描结束时间
IoavProtectionEnabled|下载文件与附件防护是否启用
IsTamperProtected|篡改防护是否启用
NISEnabled|网络检查系统是否启用
OnAccessProtectionEnabled|访问时防护是否启用
QuickScanEndTime|最近快速扫描结束时间
RealTimeProtectionEnabled|实时防护是否启用
DisableBehaviorMonitoring|是否禁用行为监控
DisableIOAVProtection|是否禁用下载文件与附件防护
DisableRealtimeMonitoring|是否禁用实时监控
DisableScriptScanning|是否禁用脚本扫描
ExclusionExtension|排除的文件扩展名
ExclusionPath|排除路径
ExclusionProcess|排除进程
PUAProtection|潜在不受欢迎应用防护设置
instanceGuid|实例 GUID
productState|安全产品登记状态代码
 timestamp|来源时间戳
ActionSuccess|处置动作是否成功
CleaningActionID|清理动作编号
CurrentThreatExecutionStatusID|威胁执行状态编号
DetectionID|检测记录标识
InitialDetectionTime|首次检测时间
LastThreatStatusChangeTime|威胁状态最近变化时间
ThreatID|威胁标识
ThreatStatusID|威胁状态编号
AvailableSecurityProperties|设备可用安全能力编号列表
CodeIntegrityPolicyEnforcementStatus|代码完整性策略执行状态代码
InstanceIdentifier|实例标识
RequiredSecurityProperties|所需安全能力编号列表
SecurityServicesConfigured|已配置的安全服务编号列表
SecurityServicesRunning|正在运行的安全服务编号列表
VirtualizationBasedSecurityStatus|基于虚拟化的安全状态代码
LockedOut|是否处于锁定状态
ManufacturerIdTxt|制造商文本标识
ManufacturerVersion|制造商版本
RestartPending|是否等待重启
TpmActivated|TPM 是否激活
TpmEnabled|TPM 是否启用
TpmOwned|TPM 是否已取得所有权
TpmPresent|TPM 是否存在
TpmReady|TPM 是否就绪
has_private_key|证书是否关联私钥，仅记录存在标志
issuer|证书颁发者
location|证书存储位置或来源指定位置
not_after|证书有效期截止时间
not_before|证书有效期开始时间
signature_algorithm|签名算法标识
store|证书存储区名称
subject|证书主题
thumbprint|证书指纹
trust|本次是否执行信任评估的标记
encryption_method|加密方法
encryption_percentage|加密完成百分比
lock_status|卷锁定状态
mount_point|挂载点
protection_status|保护状态
protector_types|密钥保护器类型列表
recovery_material_collected|是否采集恢复材料
volume_status|卷状态
volume_type|卷类型
algorithm|算法名称
comment_collected|是否采集注释
fingerprint|指纹
key_material_collected|是否保存密钥材料
log_mode|日志保留模式
maximum_bytes|日志最大容量，字节
oldest_record|最早可用记录编号
readable|通道是否可读
record_count|日志记录数量
event_namespace|WMI 事件命名空间
query_language|WMI 查询语言
executable_path|可执行程序路径
script|脚本内容或受保护的脚本字段
script_engine|脚本引擎
Consumer|WMI 消费者引用
DeliverSynchronously|是否同步交付事件
Filter|WMI 过滤器引用
MaintainSecurityContext|是否保持安全上下文
SlowDownProviders|是否要求提供程序降低事件产生速度
Author|作者登记值
conditions|计划任务运行条件
last_result|最近执行结果代码
last_run|最近执行时间
missed_runs|错过执行次数
next_run|下次计划执行时间
principal|任务运行身份和权限配置
registration|任务登记元数据
Source|登记来源
triggers|任务触发器列表
URI|任务资源标识
BusType|存储总线类型
FriendlyName|友好名称
HealthStatus|来源报告的健康状态
IsBoot|是否为启动相关磁盘
IsOffline|是否脱机
IsReadOnly|是否只读
IsSystem|是否为系统磁盘
Number|来源对象编号
ObjectId|对象标识
OperationalStatus|操作状态列表
PartitionStyle|分区表样式
UniqueId|唯一标识
AccessPaths|访问路径列表
DiskId|磁盘标识
DiskNumber|磁盘编号
GptType|GPT 分区类型标识
Guid|GUID 标识
IsActive|是否为活动分区
IsHidden|是否隐藏
MbrType|MBR 分区类型代码
Offset|分区起始偏移
PartitionNumber|分区编号
DeviceName|设备名称
DriverDate|驱动日期
DriverProviderName|驱动提供方
InfName|驱动 INF 名称
IsSigned|来源报告是否签名
Signer|签名者
PathName|程序或驱动路径
Started|是否已启动
ReleaseDate|发布日期
SMBIOSBIOSVersion|SMBIOS 中的 BIOS 版本
SMBIOSMajorVersion|SMBIOS 主版本
SMBIOSMinorVersion|SMBIOS 次版本
FirmwareVersion|固件版本
MediaType|介质类型
BatteryStatus|电池状态代码
Chemistry|电池化学类型代码
DesignCapacity|设计容量，单位依来源接口
EstimatedChargeRemaining|估算剩余电量百分比
EstimatedRunTime|估算剩余运行时间，单位依来源接口
FullChargeCapacity|充满时容量，单位依来源接口
ElementName|元素显示名称
InstanceID|实例标识
Domain|计算机所属域、工作组或账户域；按来源区分
DomainRole|计算机域角色代码
PartOfDomain|是否加入域
SystemType|系统类型
UUID|系统产品 UUID
Vendor|厂商
date|来源登记日期或时间
hresult|HRESULT 结果代码
result|来源结果代码
revision|修订号
title|标题
update_id|更新标识
EventType|事件类型代码
RestorePointType|还原点类型
SequenceNumber|序列编号
IsBundle|是否为应用包捆绑包
IsFramework|是否为框架包
NonRemovable|是否标记不可移除
PackageFamilyName|包系列名称
PackageFullName|包完整名称
SignatureKind|包签名类型
PackageName|包名称
LastUseTime|最近使用时间
Loaded|配置文件是否已加载
AuthenticationPackage|认证包
LogonId|登录会话标识
LogonType|登录类型代码
identity_query_error|身份查询错误码
state_code|来源状态代码
station|终端站点名称
user|用户名称
command_line_status|命令行字段读取状态
free_kib|可用内存，KiB
percent_processor_time|CPU 使用百分比
total_kib|总内存，KiB
AutomaticStartAction|虚拟机自动启动动作
AutomaticStopAction|虚拟机自动停止动作
CheckpointType|检查点类型配置
ConfigurationLocation|虚拟机配置位置
DynamicMemoryEnabled|是否启用动态内存
Generation|虚拟机代数
MemoryMaximum|最大内存
MemoryMinimum|最小内存
MemoryStartup|启动内存
ProcessorCount|虚拟处理器数量
AllowManagementOS|是否允许管理操作系统使用交换机
NetAdapterInterfaceDescription|物理网卡接口说明
SwitchType|虚拟交换机类型
ControllerLocation|虚拟磁盘在控制器上的位置
ControllerNumber|控制器编号
ControllerType|控制器类型
VMId|虚拟机标识
VMName|虚拟机名称
ParentSnapshotId|父检查点标识
SnapshotType|检查点类型
volume|卷标识
filesystem|文件系统名称
eligible|是否符合本采集器条件
journal_id|USN 日志身份标识
usn|USN 记录位置
file_id|文件标识
parent_id|父目录标识
watch_path|匹配的监测文件路径
watch_revision|监测名单修订号
reason_mask|USN 变更原因位掩码
record_version|USN 记录格式版本
source_time_available|是否取得来源时间
full_path_resolved|是否还原完整历史路径
remaining_depth|剩余注册表扫描深度
hash_status|哈希计算状态
security_status|安全属性读取状态
implementation_file|COM 实现文件元数据
hash_files|是否启用文件哈希
embedded_signer|文件嵌入签名的主题
error_type|读取失败的异常类型
creation_utc|文件系统创建时间 UTC
last_write_utc|文件系统最近写入时间 UTC
dacl_sddl|所有者和 DACL 的 SDDL 表示
streams|文件数据流元数据列表
repository|容器镜像仓库
tag|容器镜像标签
names|容器名称
image|镜像标识或程序路径，按来源区分
driver|驱动名称
`;const normalize=s=>String(s).replace(/[_\s-]/g,'').toLowerCase();
module.exports=Object.fromEntries(entries.trim().split('\n').map(line=>{const [k,v]=line.trim().split('|');return [normalize(k),v];}));
Object.assign(module.exports,{contentstored:'是否保存文件正文',directory:'该条目是否为目录',reparsepoint:'是否为文件系统重解析点',drive:'盘符'});
