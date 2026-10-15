'use strict';
// Authored migration decisions; current implementation status remains owned by sources.json.
const assert=require('node:assert/strict');
const plans=new Map();
function add(ids,group,priority,route,boundary,mode='state'){
  for(const id of ids.split(/\s+/).filter(Boolean)){const source='pcscope.'+id;assert(!plans.has(source),source);plans.set(source,{group,priority,mode,target:'compiled_backend',route,boundary,validation:'接口路线已识别；字段、权限和环境一致性需逐项验收'});}
}
add('network.adapters','网卡与设备','P0','直接查询 MSFT_NetAdapter；GetIfTable2 补充接口状态；SetupAPI/CM 关联 PnP 标识','保留隐藏/虚拟/断开网卡；GetIfTable2 单独不能替代原全部字段和设备范围。GUID 与 PnP ID 分开。');
add('hardware.devices','网卡与设备','P0','SetupDiGetClassDevs / SetupDiEnumDeviceInfo / CM_Get_DevNode_Status；直接 WMI 补充字段','覆盖设备登记与当前存在状态；移除、禁用、断开分别记录，失败不提交空快照。');
add('network.addresses','网络现状','P0','GetUnicastIpAddressTable；MSFT_NetIPAddress 补充原字段','IPv4/IPv6、接口关联、状态和前缀来源均需保留。');
add('network.routes','网络现状','P0','GetIpForwardTable2；MSFT_NetRoute 补充策略来源','Active/Persistent 等策略范围不因迁移丢失。');
add('network.tcp network.udp','网络现状','P0','GetExtendedTcpTable / GetExtendedUdpTable；必要时直接查询 MSFT_NetTCPConnection / MSFT_NetUDPEndpoint','保留创建时间与所属进程等身份字段；PID 模式不足时使用更完整结构/提供程序，不能以读取时间代替创建时间。');
add('execution.processes','进程与后台','P0','Toolhelp / EnumProcesses / GetProcessTimes / QueryFullProcessImageName；Win32_Process 补充','PID+建立时间区分实例；受保护进程拒绝访问不能当作退出。命令行沿用保护规则。');
add('startup.services hardware.driver-services','进程与后台','P0','OpenSCManager / EnumServicesStatusEx / QueryServiceConfig；直接 WMI 补齐派生字段','同时覆盖 Win32 服务与驱动服务，按原来源分开；不以服务运行证明驱动文件完整。');
add('startup.tasks startup.task-details','进程与后台','P0','编译宿主直接 MI 读取 MSFT_ScheduledTask，固定只读 PS_ScheduledTask.GetInfoByObject；系统任务提供程序使用 Task Scheduler 接口','完整任务路径、嵌入操作/触发器/条件/主体及最近执行信息；不加载 PowerShell；当前配置和最近运行结果不替代每次运行历史。');
add('startup.wmi-bindings startup.wmi-consumers startup.wmi-filters','WMI 订阅配置','P1','WMI COM 直接读取 root/subscription 的 __FilterToConsumerBinding / __EventConsumer / __EventFilter','只读订阅定义；正确保留对象路径、绑定及保护的脚本内容，不创建或触发订阅。');

add('apps.browser-policies apps.onedrive-registration desktop.clipboard desktop.input desktop.shell-settings environment.configuration network.winrm-configuration policy.configuration shell.registrations software.user-inventory startup.advanced-extensions startup.logon startup.service-configuration startup.session-hooks system.clock system.reboot-pending virtualization.wsl-registration','注册表与配置','P1','RegOpenKeyEx / RegEnumKeyEx / RegQueryValueEx 或编译宿主 Registry API；必要时原生文件读取','保留 HKLM/HKCU、32/64 位视图、原递归边界与值保护；启动目录文件、时区和 COM 文件元数据由后端补齐。WSL 登记不当作发行版运行状态。');
add('apps.chromium-extensions apps.firefox-extensions apps.ide-extensions apps.npm-global-packages network.hosts software.portable security.ssh-public-keys','文件与应用登记','P1','编译后端直接枚举文件、解析 JSON/文本、读取版本和计算摘要','仅原允许目录、文件大小与层级；避免重解析点和自身数据目录。扩展安装清单不自动证明已启用；不读私钥。');
add('desktop.shortcuts','文件与应用登记','P1','编译宿主直接 COM 读取 WScript.Shell/CreateShortcut 现有元数据及 Known Folder；不启动脚本宿主','固定读取属性，不调用 Run/Exec/Save 或目标程序；原描述读取 E_FAIL 保持空值，保持原接口语言。后续可换等价 IShellLinkW/IPersistFile。');
add('hardware.battery hardware.firmware hardware.driver-bindings identity.machine identity.profiles identity.logon-sessions system.boot-state system.hotfixes system.restore-points system.version virtualization.platform','系统提供程序','P1','编译宿主通过 WMI COM / System.Management 直接查询既有 Win32_* / SystemRestore 类','保持各行现有类、命名空间、身份与字段；QFE 不是完整更新清单，登录会话是当前枚举，系统还原点有版本/权限限制。');
add('storage.disks storage.partitions hardware.disk-health','磁盘与分区','P1','直接查询 root/Microsoft/Windows/Storage：MSFT_Disk / MSFT_Partition / MSFT_PhysicalDisk','保留 ObjectId、UniqueId、访问路径、磁盘健康和关联；存储提供程序不存在时报告能力不可用。');
add('storage.volumes','磁盘与分区','P1','FindFirstVolume / GetVolumeInformation / GetDiskFreeSpaceEx；Win32_Volume 补充','包含无盘符卷，卷身份不得退化为盘符；保留原驱动器类型与容量字段。');
add('identity.accounts identity.groups','账户','P1','NetUserEnum / NetUserGetInfo / NetLocalGroupEnum / NetLocalGroupGetMembers / LookupAccountSid','SID 做身份；域不可达或成员解析失败不能提交成员全空。');
add('network.dns-configuration network.dns-cache network.profiles','网络提供程序','P1','直接查询 root/StandardCimv2：MSFT_DNSClientServerAddress / MSFT_DNSClientCache / MSFT_NetConnectionProfile；Registry 补配置','DNS 缓存是当前缓存，不是访问历史；不调用未文档化的 DNS 缓存枚举导出。');
add('network.firewall-profiles network.firewall-rules network.ipsec','防火墙与 IPsec','P1','直接调用 StandardCimv2 防火墙/IPsec 提供程序，读取规则及关联过滤器','保留 ActiveStore、PolicyStoreSource 和七类过滤器；不能只用简化 INetFwPolicy2 视图宣称完全等价。');
add('network.access','远程访问','P1','RasEnumEntries / RasGetEntryProperties / RasEnumConnections + Registry；必要时 VPN CIM 提供程序','当前用户和全用户 VPN 分开；不得收集保存的密码，保持代理/RDP 配置字段。');
add('network.shares network.mapped-drives network.share-sessions','SMB','P1','直接查询 SMB CIM 提供程序；NetShareEnum / NetSessionEnum / WNetEnumResource 作为字段可验证的补充','Get-SmbSession 实际为本机 SMB 服务端上的客户端会话，现有名称需校正；保留加密、方言、访问 ACL，不能用少字段接口缩减。');
add('power.configuration','电源','P1','PowerEnumerate / PowerReadFriendlyName / PowerGetActiveScheme + Registry；Win32_PowerPlan 保持兼容身份','保留计划 GUID 与当前使用状态，读取不切换电源计划。');
add('desktop.printing desktop.print-jobs desktop.printer-drivers desktop.printer-ports','打印','P1','Winspool：EnumPrinters / EnumJobs / EnumPrinterDrivers / EnumPorts / GetPrinter','保留用户连接打印机与队列；短暂完成作业的完整历史需要打印事件记录。');
add('security.certificates.currentuser security.certificates.localmachine','证书','P1','CertOpenStore / CertEnumCertificatesInStore 或编译宿主 X509Store','区分当前用户与机器证书库；仅元数据，不导出私钥。');
add('software.appx-user','Windows 应用','P1','WinRT PackageManager.FindPackagesForUser','按原用户 SID 查询包和依赖，保留注册状态、体系结构和安装位置；不能改为只看当前提升账户。');
add('software.optional-features','Windows 应用','P1','DISM API DismGetFeatures / DismGetFeatureInfo','只读在线映像；包/功能状态保留，权限失败不触发启用功能。');
add('software.provisioned','Windows 应用','P2','编译宿主直接 WinRT FindProvisionedPackages；API 不存在的旧系统使用 DISM _DismGetProvisionedAppxPackages','完整保留五字段、包身份与机器范围；公开接口按运行时可用性选择，不按安装包标题猜名称，不修改系统或回退 PowerShell。旧 LTSC 私有 DISM 布局和版本环境继续验证。');
add('security.protection security.protection-policy security.products security.deviceguard','安全状态','P2','直接查询 Defender WMIv2、SecurityCenter2、DeviceGuard 提供程序','Defender/第三方产品、Windows Server 和系统版本适用性分开；IsTamperProtected 等版本字段必须实测。');
add('security.detections','产品历史','P2','直接查询 MSFT_MpThreatDetection，关联 Defender Operational 事件','保留 DetectionID 和检测时间；这是产品保留的历史，不是全机器当前威胁扫描。','retained_history');
add('security.encryption','安全状态','P2','Win32_EncryptableVolume 只读属性/方法，编译宿主直接调用','不得解锁、启动加密或读取恢复密钥；保持卷和保护器元数据的范围。');
add('security.tpm','安全状态','P2','TpmCoreProvisioning 只读状态与厂商元数据，TBS 固定无认证能力查询读取锁定状态','保留九字段；待处理物理确认操作非零对应 RestartPending。避免原命令构造器的授权材料读取、自检和可能的配置改变。权限错误保留旧状态，不伪造默认值；硬件代际与版本环境继续验证。');
add('security.secureboot','安全状态','P2','GetFirmwareType + GetFirmwareEnvironmentVariableExW 读取 UEFI SecureBoot','UEFI/传统 BIOS/权限不足分别处理；不修改固件变量。');
add('system.update-history','产品历史','P2','WUA COM：IUpdateSession / IUpdateSearcher.QueryHistory','普通应用完整性级别实机 136 条历史身份/字段比对通过；低完整性拒绝。仅查询产品保留历史，不搜索、下载或安装更新；其他环境继续验收。','retained_history');
add('virtualization.hyperv-vms virtualization.hyperv-switches virtualization.hyperv-disks virtualization.hyperv-checkpoints','虚拟化','P2','Hyper-V WMI V2：root/virtualization/v2，Msvm_* 类与关联','Hyper-V 未安装/家庭版无提供程序为不适用；逐字段关联 VM、设置、交换机、磁盘、检查点，不因命令替换丢 Generation 等派生值。');
add('virtualization.docker','第三方产品','P2','编译后端通过本机命名管道调用 Docker Engine HTTP API','保持当前 docker_engine 范围，协商 API 版本；不自动启动 Docker，不使用远程端点；无 PowerShell 或 shell。');
add('apps.mihomo','第三方产品','P2','编译后端 HTTP 客户端调用现有本机 /version、/configs API','显式开启、固定回环地址、超时和大小限制；凭据保护，不修改代理配置。');

add('apps.dotnet-runtimes software','已原生登记','保留','编译宿主直接读取 Registry','保留各来源现有视图和用户范围。');
add('collector.event-channel-coverage','采集能力','保留','EventLogConfiguration / EventLogSession / EventLogReader','读取通道配置与首尾记录；不推断全部历史完整。','capability');
add('collector.scoped-configuration usn.watch-list','采集配置','保留','编译后端读取已配置名单与本地配置','名单与日志元数据分开，不产生额外系统采集。','configuration');
add('execution.resources','资源采样','保留','编译宿主直接 WMI 查询 Win32_OperatingSystem / CPU 性能类','当前已编译运行；CPU 采样区间、开销预算另验。');
add('identity.assigned-rights','账户','保留','LSA：LsaEnumerateAccountsWithUserRight / LsaEnumerateAccountRights','按 SID，区分机器权限和用户会话。');
add('identity.interactive-sessions','会话','保留','WTS：WTSEnumerateSessions / WTSQuerySessionInformation','当前会话范围，过去登录动作依赖事件。');
add('hardware.driver-packages','驱动仓库','保留','DISM API DismGetDrivers','INF+仓库路径区分包；732 包只读验收已通过。');
add('security.key-metadata','安全状态','保留','NCryptEnumStorageProviders / NCryptEnumKeys（SILENT）','仅密钥元数据，不触发 PIN；3 个提供程序/范围问题仍单列未完成。');
add('files.metadata-security files.directory-security','文件状态','保留','编译宿主文件/ACL/ADS API、SHA256、GetFileInformationByHandle','保留指定名单、深度、大小与重解析点限制。');
add('execution.modules execution.module-scan-status','进程模块','保留','PSAPI / OpenProcess / GetProcessTimes + 用户/会话过滤','只跟踪实际观察模块；进程退出后基线保留，不宣称能发现首次启动即失败的未知模块。');
add('execution.module-file-state','模块文件变化','保留','ReadDirectoryChangesW + 原生文件身份/摘要/签名检查','共享目录监听、已知文件集合、去抖、稳定复核；不是文件操作的完整时间历史。','change_tracking');
add('usn.volumes','文件系统','保留','GetLogicalDrives / GetDriveType / GetVolumeInformation / GetVolumeNameForVolumeMountPoint','当前固定 NTFS 卷范围。');
add('usn.{drive}','文件日志','保留','DeviceIoControl：FSCTL_QUERY_USN_JOURNAL / FSCTL_READ_USN_JOURNAL','需要现有 NTFS journal 和读取权限；USN 不给操作者或完整内容，不自动创建 journal。','change_journal');
add('eventlog.import usn.changes','虚拟分组','不采集','复用既有日志/USN 导入与分组','这两项不单独启动采集器。','virtual');

function forSource(source){
  if(source.startsWith('pcscope.eventlog.')&&source!=='pcscope.eventlog.import')return {group:'Windows 事件日志',priority:'保留',mode:'event_history',target:'compiled_backend',route:'现有编译 EventLogReader（Windows Event Log API）；后续 EvtSubscribe 与书签补读',boundary:'事件提供程序/通道/ID/版本联合解释；只读已启用日志，按权限/安装组件区分，不通过快照猜造动作。',validation:'已原生路由；各日志的系统配置与事件解释分别验收'};
  assert(plans.has(source),'Missing collection decision: '+source);const row=plans.get(source);return row.mode==='virtual'?{...row,target:'none',validation:'分组标识，不单独执行采集'}:row;
}
module.exports={forSource,explicitSources:[...plans.keys()]};
