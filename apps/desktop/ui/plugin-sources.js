'use strict';
// Each row documents an implemented source, not a claim that the source is readable on this PC.
const pluginSources=(()=>{
  const result={};
  function group(id,text){result[id]=text.trim().split('\n').filter(Boolean).map(line=>{const [source,name,api,method]=line.split('|');if(!method)throw new Error('Invalid source catalog row: '+line);return {source:'pcscope.'+source,name,api,method};});}
  group('pcscope-software.exe',String.raw`
software|本机软件安装登记|注册表 · Rust winreg::RegKey；HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall|分别枚举 32/64 位视图，读取键内全部登记值与类型（敏感值脱敏）；完整快照对比
`);
  group('pcscope-powershell-adapter.exe',String.raw`
system.version|操作系统版本与安装时间|CIM · root/cimv2: Win32_OperatingSystem|Get-CimInstance 查询 Caption、Version、BuildNumber、OSArchitecture、InstallDate 等；状态快照
system.boot-state|最近启动时间与时区|CIM · Win32_OperatingSystem.LastBootUpTime；PowerShell Get-TimeZone|读取系统启动时间与时区 ID；状态快照
identity.accounts|本机用户账户|PowerShell · Get-LocalUser|枚举账户、SID、启用状态、最近登录及密码策略；来源不提供建立时间
identity.groups|本机用户组与成员|PowerShell · Get-LocalGroup / Get-LocalGroupMember -SID|按组 SID 枚举成员 SID；快照对比，不计算全部有效权限
startup.services|服务运行状态|CIM · root/cimv2: Win32_Service|查询名称、状态、启动模式、运行账户和服务说明；非服务事件订阅
startup.tasks|计划任务基础配置|PowerShell · Get-ScheduledTask|枚举任务、Principal、Settings 与 Actions；保存动作全部 CIM 属性及脱敏参数
storage.volumes|卷与文件系统|CIM · root/cimv2: Win32_Volume|枚举卷 ID、盘符、文件系统、容量和卷类型；状态快照
network.adapters|网络适配器|PowerShell · Get-NetAdapter -IncludeHidden|包括隐藏网卡，读取 GUID、状态、MAC、速度与驱动版本
network.addresses|IP 地址|PowerShell · Get-NetIPAddress|枚举接口地址、前缀长度、地址族和地址状态；状态快照
virtualization.platform|虚拟化能力与平台|CIM · Win32_ComputerSystem / Win32_Processor|查询 HypervisorPresent、固件虚拟化、监控扩展与 SLAT 能力；不枚举运行虚拟机
hardware.devices|即插即用设备|CIM · root/cimv2: Win32_PnPEntity|枚举设备 ID、类别、制造商、服务、状态与配置错误码
system.hotfixes|QFE 更新登记|CIM · root/cimv2: Win32_QuickFixEngineering|枚举 HotFixID、InstalledOn 等；范围为 QFE 更新登记
shell.registrations|第三方预览处理程序|注册表 · Microsoft.Win32.RegistryKey；HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\PreviewHandlers|只读扫描 32/64 位视图；提取 CLSID 并关联 COM 注册
shell.registrations|图标覆盖处理程序|注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\ShellIconOverlayIdentifiers|只读递归读取扩展登记
shell.registrations|Shell 扩展允许与阻止列表|注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved、Blocked|读取 32/64 位视图中的登记值
shell.registrations|桌面与此电脑命名空间|注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace、MyComputer\NameSpace|递归读取命名空间及关联 CLSID
shell.registrations|右键菜单、文件关联与默认图标|注册表 · HKLM/HKCU\Software\Classes\{类型}\shell、shellex、DefaultIcon；Directory\Background\shell、shellex|逐类型枚举，递归最多 5 层；扩展名根读取默认关联；命令字段保存脱敏原值
shell.registrations|COM 实现与扩展文件元数据|注册表 · Software\Classes\CLSID\{已发现CLSID}\InprocServer32、LocalServer32；.NET FileInfo / FileVersionInfo / SHA256|关联已发现的 CLSID；本地 DLL 读取版本、大小、写入时间与哈希；不执行 DLL、不验证签名
startup.logon|登录启动登记|注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Run、RunOnce、Explorer\StartupApproved|枚举 32/64 位视图；Run 命令保存脱敏原值，StartupApproved 保存登记信息
startup.logon|用户与公共启动文件夹|文件系统 · Environment.GetFolderPath(Startup / CommonStartup)；Get-ChildItem / SHA256|枚举文件并读取大小、时间与摘要；不解析快捷方式目标
`);
  group('system',String.raw`
identity.machine|计算机身份与域信息|CIM · root/cimv2: Win32_ComputerSystem / Win32_ComputerSystemProduct|查询名称、域、角色、制造商、型号及 UUID；状态快照
system.clock|时间同步配置与时区|注册表 · HKLM\SYSTEM\CurrentControlSet\Services\W32Time；Get-TimeZone|递归配置 3 层；读取键内值并脱敏，标注限定深度外的子键数量
system.update-history|Windows Update 历史|COM · Microsoft.Update.Session → CreateUpdateSearcher → GetTotalHistoryCount / QueryHistory|每批 128 条读取本机保留的更新历史；非在线检查更新，不安装更新
system.reboot-pending|待重启标志|注册表 · HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending；WindowsUpdate\Auto Update\RebootRequired；SYSTEM\CurrentControlSet\Control\Session Manager|读取标志键与 PendingFileRenameOperations；状态快照
system.restore-points|系统还原点|CIM · root/default: SystemRestore|枚举序号、说明、建立时间与类型；不创建还原点
software.appx-user|当前用户 AppX 包|PowerShell · Get-AppxPackage|枚举当前用户包登记与版本、发布者、位置、状态
software.provisioned|预配 AppX 包|PowerShell · Get-AppxProvisionedPackage -Online|读取本机映像预配包清单；需要相应权限，不执行安装
software.optional-features|Windows 可选组件|PowerShell · Get-WindowsOptionalFeature -Online|读取组件名称与状态；不启用或禁用组件
software.user-inventory|当前用户软件登记|注册表 · HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall|只读枚举 32/64 位视图下一层子键，键内软件登记值保留类型与脱敏原值
identity.profiles|用户配置文件|CIM · root/cimv2: Win32_UserProfile|读取 SID、路径、加载状态与 LastUseTime；状态快照
identity.logon-sessions|登录会话|CIM · root/cimv2: Win32_LogonSession|读取 LogonId、类型、开始时间、认证包；不是完整登录登出历史
identity.interactive-sessions|交互式终端会话|Win32 API · wtsapi32!WTSEnumerateSessionsW / WTSQuerySessionInformationW|通过 IdentityNative.cs P/Invoke 枚举会话、站点、用户、域与状态码
identity.assigned-rights|账户权限分配|Win32 API · advapi32!LsaOpenPolicy / LsaEnumerateAccountsWithUserRight / LsaEnumerateAccountRights|通过 IdentityNative.cs 读取 SID 的 LSA 权限分配；需策略读取权限，非有效访问判定
execution.processes|进程清单|CIM · root/cimv2: Win32_Process|按 PID 与创建时间记录父 PID、程序路径、会话；保存脱敏命令行；来源未返回时标记原因
execution.resources|内存与 CPU 采样|CIM · Win32_OperatingSystem / Win32_PerfFormattedData_PerfOS_Processor|读取内存总量/可用量与 CPU 百分比；时点采样，不是连续性能曲线
`);
  group('security',String.raw`
security.protection|Defender 防护状态|PowerShell · Get-MpComputerStatus|读取引擎、签名、防护开关与最近扫描时间；状态快照
security.protection-policy|Defender 策略与排除项|PowerShell · Get-MpPreference|读取指定策略、排除路径/进程/扩展名；不修改防护配置
security.products|安全产品登记|CIM · root/SecurityCenter2: AntiVirusProduct|读取产品名称、实例 ID 与产品状态码；依赖系统提供该命名空间
security.detections|Defender 检测记录|PowerShell · Get-MpThreatDetection|读取来源保留的检测时间、威胁与处理状态；非主动扫描
security.deviceguard|Device Guard / VBS|CIM · root/Microsoft/Windows/DeviceGuard: Win32_DeviceGuard|读取安全能力、配置与运行服务编号；状态快照
security.tpm|TPM 状态|PowerShell · Get-Tpm|读取 TPM 存在、就绪、开启、锁定及制造商信息
security.secureboot|安全启动状态|PowerShell · Confirm-SecureBootUEFI|查询 UEFI Secure Boot；组件不支持或权限不足报告缺口
security.key-metadata|CNG 密钥元数据|NCryptEnumStorageProviders / NCryptEnumKeys（SILENT）|按提供程序与用户/机器范围读取名称、算法和旧式 key spec；不打开、导出或创建密钥
security.certificates.localmachine|计算机证书|.NET API · X509Store(StoreLocation.LocalMachine)，Root / CA / My / TrustedPublisher / Disallowed / TrustedPeople|ReadOnly + OpenExistingOnly 枚举证书元数据；不导出私钥，不执行信任评估
security.certificates.currentuser|当前用户证书|.NET API · X509Store(StoreLocation.CurrentUser)，Root / CA / My / TrustedPublisher / Disallowed / TrustedPeople|只读枚举指纹、主题、颁发者、有效期与私钥存在标志
security.encryption|BitLocker 卷状态|PowerShell · Get-BitLockerVolume|读取加密方法、进度、保护与锁定状态、保护器类型；不采集恢复材料
policy.configuration|UAC、资源管理器与软件限制策略|注册表 · HKLM/HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies；HKLM\SOFTWARE\Policies\Microsoft\Windows\Safer|递归最多 3 层；白名单字段保留原值，其他值保存摘要
security.ssh-public-keys|SSH 公钥指纹|文件系统 · %USERPROFILE%\.ssh\*.pub；File.ReadAllText / SHA256|读取支持的公钥编码并生成算法与指纹；不保存密钥材料和注释
collector.event-channel-coverage|事件通道可读性|PowerShell · Get-WinEvent -ListLog；随包 channels.json|逐通道查询启用、容量、保留策略、记录数与错误；查询通道状态
`);
  group('network',String.raw`
network.routes|路由表|PowerShell · Get-NetRoute|枚举接口、目的前缀、下一跳、度量与策略来源；状态快照
network.dns-configuration|DNS 配置|Get-DnsClientServerAddress；注册表 HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters|查询接口 DNS 服务器及域/搜索列表配置
network.profiles|网络连接配置文件|PowerShell · Get-NetConnectionProfile|读取网络名称、类别、IPv4/IPv6 连通性
network.tcp|TCP 连接与监听|PowerShell · Get-NetTCPConnection|时点枚举本地/远端地址端口、状态、所属 PID 与创建时间；不抓包
network.udp|UDP 端点|PowerShell · Get-NetUDPEndpoint|时点枚举本地地址端口、PID 与创建时间；不读取报文
network.firewall-profiles|防火墙配置文件|PowerShell · Get-NetFirewallProfile|查询默认入站/出站动作、防火墙开关与日志配置；不读取流量日志正文
network.firewall-rules|防火墙规则及过滤条件|Get-NetFirewallRule；Get-NetFirewall{Port,Address,Application,Service,Interface,InterfaceType,Security}Filter -PolicyStore ActiveStore|分别枚举 7 类过滤器，按 InstanceID 关联规则；缺失关联报告缺口
network.ipsec|IPsec 规则|PowerShell · Get-NetIPsecRule -PolicyStore ActiveStore|枚举规则、安全模式与策略来源；状态快照
network.access|代理、远程桌面与 VPN|Get-VpnConnection；HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings；HKLM\SYSTEM\CurrentControlSet\Control\Terminal Server|读取当前用户 VPN 和代理、RDP 配置；注册表非白名单值保存摘要
network.shares|SMB 共享与访问授权|PowerShell · Get-SmbShare / Get-SmbShareAccess -Name|枚举共享路径与共享层 ACL；非文件系统全部有效权限
network.mapped-drives|SMB 驱动器映射|PowerShell · Get-SmbMapping|读取本地盘符、远端路径与状态
network.dns-cache|DNS 缓存|PowerShell · Get-DnsClientCache|时点缓存条目与解析值；非完整查询历史或查询发生时间
network.hosts|hosts 静态映射|文件系统 · %SystemRoot%\System32\drivers\etc\hosts；Get-Content|解析有效地址和名称、排除注释；拒绝超大文件与重解析点
network.share-sessions|SMB 客户端会话|PowerShell · Get-SmbSession|读取会话、客户端、用户、打开数、协议与加密状态
network.winrm-configuration|WinRM 配置|注册表 · HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\WSMAN|递归最多 3 层，保存指定认证、监听与传输配置；不连接远端机器
`);
  group('storage',String.raw`
storage.disks|磁盘清单|PowerShell · Get-Disk|读取磁盘 ID、总线、分区样式、容量、启动和读写状态
storage.partitions|分区清单|PowerShell · Get-Partition|读取磁盘/分区编号、GUID、偏移、大小与访问路径
hardware.driver-packages|驱动仓库包|PowerShell · Get-WindowsDriver -Online -All（DISM）|枚举本机仓库中的全部包，保留发布 INF、原 INF、提供方、版本与签名描述；不安装驱动
hardware.driver-bindings|设备驱动绑定|CIM · root/cimv2: Win32_PnPSignedDriver|读取设备与驱动版本、日期、提供方、INF 及来源签名标志
hardware.driver-services|驱动服务|CIM · root/cimv2: Win32_SystemDriver|读取驱动路径、服务类型、启动模式与运行状态
hardware.firmware|BIOS 固件|CIM · root/cimv2: Win32_BIOS|读取制造商、BIOS/SMBIOS 版本与发布日期
hardware.disk-health|物理磁盘健康|PowerShell · Get-PhysicalDisk|读取固件版本、介质类型、健康与操作状态；不是完整 SMART 原始属性
hardware.battery|电池状态|CIM · root/cimv2: Win32_Battery|读取来源提供的容量、化学类型、余量与估算时间
power.configuration|电源计划与休眠配置|CIM root/cimv2/power: Win32_PowerPlan；HKLM\SYSTEM\CurrentControlSet\Control\Power|查询激活计划和指定休眠/待机配置；不切换计划
`);
  group('startup',String.raw`
startup.service-configuration|服务详细配置|注册表 · HKLM\SYSTEM\CurrentControlSet\Services|递归最多 2 层，读取启动类型、账户、依赖与 ServiceDll 等指定值
startup.session-hooks|会话与登录初始化入口|HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon；HKLM\SYSTEM\CurrentControlSet\Control\Session Manager；HKLM/HKCU\Software\Microsoft\Active Setup\Installed Components、Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run|注册表快照，读取 Shell/Userinit/BootExecute 等；非白名单内容仅摘要
startup.advanced-extensions|IFEO、AppInit 与 AppCert 入口|注册表 · HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options、SilentProcessExit、Windows；HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\AppCertDlls|递归最多 2 层；读取指定开关及其他值摘要，不加载扩展
startup.wmi-filters|WMI 永久订阅过滤器|CIM · root/subscription: __EventFilter|读取名称、命名空间、查询语言与查询摘要；这是订阅配置清单，不是主动订阅事件
startup.wmi-consumers|WMI 永久订阅消费者|CIM · root/subscription: __EventConsumer|读取类、脚本引擎、程序路径；命令/脚本只保存摘要
startup.wmi-bindings|WMI 过滤器与消费者绑定|CIM · root/subscription: __FilterToConsumerBinding|枚举 Filter/Consumer 关联与安全上下文配置
startup.task-details|计划任务执行记录与条件|PowerShell · Get-ScheduledTask / Get-ScheduledTaskInfo|读取最近/下次运行、返回码、漏跑次数、触发器、条件、执行目标；参数仅摘要
environment.configuration|环境变量与默认文件关联|HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment；HKCU\Environment、Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts、Software\Microsoft\Windows\Shell\Associations\UrlAssociations|注册表快照，读取指定环境变量及 ProgId/Application；其他内容摘要
`);
  group('desktop',String.raw`
desktop.printing|打印机清单|PowerShell · Get-Printer|枚举打印机、共享、驱动、端口与状态；依赖打印服务
desktop.printer-drivers|打印驱动|PowerShell · Get-PrinterDriver|读取名称、环境、制造商、驱动/INF/配置文件路径
desktop.printer-ports|打印端口|PowerShell · Get-PrinterPort|读取端口类型、主机地址、协议、端口号与 SNMP 配置
desktop.print-jobs|打印作业队列|PowerShell · Get-Printer / Get-PrintJob -PrinterName|逐打印机读取队列、提交时间、大小、页数和状态；不保存文档名/正文
desktop.input|键盘、输入法与过滤驱动配置|HKCU\Keyboard Layout；HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts；HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e96b-e325-11ce-bfc1-08002be10318}|注册表快照，读取布局文件、IME 文件和 UpperFilters/LowerFilters 等
desktop.clipboard|剪贴板设置|HKCU\Software\Microsoft\Clipboard；HKLM\Software\Policies\Microsoft\Windows\System|读取历史/同步开关与策略；不读取剪贴板正文
desktop.shell-settings|桌面与资源管理器设置|HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced、User Shell Folders；HKCU\Control Panel\Desktop|注册表快照，读取指定显示、壁纸与系统文件夹配置
desktop.shortcuts|桌面与开始菜单快捷方式|COM · WScript.Shell.CreateShortcut；Environment.GetFolderPath(DesktopDirectory / CommonDesktopDirectory / StartMenu / CommonStartMenu)|只读取现有 .lnk 目标、工作目录、图标与热键；不 Save、不执行目标；参数摘要
`);
  group('virtualization',String.raw`
virtualization.hyperv-vms|Hyper-V 虚拟机|PowerShell · Get-VM|读取虚拟机 ID、状态、代数、CPU、内存与启动行为；需 Hyper-V 模块和访问权限
virtualization.hyperv-switches|Hyper-V 虚拟交换机|PowerShell · Get-VMSwitch|枚举交换机类型、网卡描述与管理系统访问配置
virtualization.hyperv-disks|虚拟机磁盘连接|PowerShell · Get-VM / Get-VMHardDiskDrive|逐虚拟机读取虚拟磁盘路径与控制器位置；不读取磁盘内容
virtualization.hyperv-checkpoints|虚拟机检查点|PowerShell · Get-VM / Get-VMSnapshot|枚举检查点 ID、建立时间、类型与父检查点；不创建检查点
virtualization.wsl-registration|WSL 发行版登记|注册表 · HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss|读取下一层发行版名称、版本、位置与默认 UID；不启动 WSL
`);
  group('applications',String.raw`
apps.browser-policies|Chrome、Edge 与 Firefox 策略|注册表 · HKLM/HKCU\Software\Policies\Google\Chrome、Microsoft\Edge、Mozilla\Firefox|递归最多 2 层；指定策略原值与其他内容摘要
apps.ide-extensions|VS Code 扩展清单|文件系统 · %USERPROFILE%\.vscode\extensions、.vscode-insiders\extensions 下 package.json|枚举扩展目录并解析包清单；读取名称、发布者、版本和引擎约束
apps.dotnet-runtimes|.NET 运行时登记|注册表 · HKLM\SOFTWARE\dotnet\Setup\InstalledVersions；HKLM\SOFTWARE\Microsoft\NET Framework Setup\NDP|读取 32/64 位视图，分别递归最多 4/3 层
apps.npm-global-packages|全局 npm 包清单|文件系统 · %APPDATA%\npm\node_modules；%ProgramFiles%\nodejs\node_modules 下 package.json|解析普通包与 scope 包清单；不运行 npm，自定义 prefix 不覆盖
apps.onedrive-registration|OneDrive 账户与同步登记|HKCU\Software\Microsoft\OneDrive\Accounts、Software\SyncEngines\Providers\OneDrive；HKLM\Software\Policies\Microsoft\OneDrive|注册表快照，读取指定账户、挂载与策略字段；不访问云端
apps.chromium-extensions|Chrome / Edge 扩展|文件系统 · %LOCALAPPDATA%\Google\Chrome\User Data、Microsoft\Edge\User Data 下 Default / Profile N\Extensions\{ID}\{版本}\manifest.json|解析名称、版本与权限清单；不判定是否启用，拒绝重解析目录
apps.firefox-extensions|Firefox 扩展|文件系统 · %APPDATA%\Mozilla\Firefox\Profiles\{配置文件}\extensions.json|解析插件版本、启用/禁用、签名状态与安装/更新时间字段
`);
  group('scoped',String.raw`
collector.scoped-configuration|指定范围配置|插件配置文件列表 + 本地 JSON · collector-options.json|文件列表由后台持久保存并传给采集器；默认为空；其他可选运行时读取 JSON
files.directory-security|指定目录与直接子项|Get-Item / Get-ChildItem / Get-Acl / GetFileInformationByHandle|只读用户配置目录的属性、Owner/DACL、直接子项和文件系统身份；不递归、不保存正文
files.metadata-security|指定文件、ACL 与 ADS|Get-Item -LiteralPath；FileSystemInfo.GetAccessControl；Get-Item -Stream *；SHA256|仅逐项读取插件文件列表，忽略旧 file_roots，不扫描相邻文件；读取时间、属性、Owner/DACL、流名称/大小；hash_files 开启才计算限额内哈希，不保存正文
software.portable|便携软件文件登记|文件系统 · portable_roots；.NET FileVersionInfo.GetVersionInfo|扫描 EXE/DLL/MSI 的版本、公司及文件元数据；共享 ACL/ADS/可选哈希流程，不验证签名
execution.modules|指定进程加载模块|原生 PSAPI / OpenProcess / GetProcessTimes|仅观察当前账户及会话的精确名称；按进程实例独立保存路径、基址、映像大小；不执行模块，验签独立处理
execution.module-scan-status|模块发现与读取范围|原生进程发现 / 账户与会话范围 / PSAPI 结果|保留每个目标及实例的观察、未观察、读取失败、部分覆盖和原始错误码；记录进程发现及读取结果
execution.module-file-state|已观察模块的磁盘文件变化|原生文件身份 / ReadDirectoryChangesW / SHA256 / WinVerifyTrust|由成功模块观察建立自动名单；稳定复核磁盘元数据、内容、签名和 ACL
apps.mihomo|Mihomo 本地运行配置|HTTP API · GET http://127.0.0.1:{mihomo_port}/version、/configs；Invoke-RestMethod|mihomo_metadata 开启才查询；默认端口 9090，支持 PCSCOPE_MIHOMO_TOKEN；不跟随重定向、不读取节点凭据
virtualization.docker|Docker 容器、镜像、卷与网络|Docker Desktop docker.exe --host npipe:////./pipe/docker_engine；container ls --all / image ls / volume ls / network ls|docker_metadata 开启才经本机命名管道读取格式化清单；不读取日志、环境变量和容器内容
`);
  group('usn',String.raw`
usn.volumes|固定 NTFS 卷发现|Win32 API · GetLogicalDrives / GetDriveTypeW / GetVolumeInformationW / GetVolumeNameForVolumeMountPointW|只检查文件列表涉及的固定 NTFS 卷与卷身份；空列表不枚举或读取卷日志；不扫描无盘符卷
usn.{drive}|USN 日志身份与断点|Win32 API · CreateFileW 打开卷；DeviceIoControl（0x900F4）|查询已有日志元数据，验证日志 ID、断点有效范围；权限不足报告缺口
usn.{drive}|文件变更记录|Win32 API · DeviceIoControl（0x900BB）；USN V2/V3 记录解析|按父目录 ID + 精确文件名匹配白名单；从列表首次读取建立的断点增量读取，每卷每轮最多 8 × 16 KiB；仅保存命中记录；多次写入可能合并为一条记录；不保存文件内容
usn.{drive}|采集库文件自身排除|Win32 API · CreateFileW / GetFileInformationByHandle|按卷序列号和文件 ID 排除 history.db、WAL、SHM 的变化，避免自采集循环
`);
  const channels=[
    ['System','系统与服务事件'],
    ['Application','应用程序事件'],
    ['Security','安全审计、登录登出'],
    ['Microsoft-Windows-TaskScheduler/Operational','计划任务执行事件'],
    ['Microsoft-Windows-WindowsUpdateClient/Operational','Windows 更新事件'],
    ['Microsoft-Windows-Windows Defender/Operational','Defender 防护事件'],
    ['Microsoft-Windows-PowerShell/Operational','PowerShell 运行事件'],
    ['Microsoft-Windows-WMI-Activity/Operational','WMI 活动事件'],
    ['Microsoft-Windows-GroupPolicy/Operational','组策略处理事件'],
    ['Microsoft-Windows-AppXDeploymentServer/Operational','AppX 部署事件'],
    ['Microsoft-Windows-TerminalServices-LocalSessionManager/Operational','终端会话登录、断开与注销'],
    ['Microsoft-Windows-TerminalServices-RemoteConnectionManager/Operational','远程连接管理事件'],
    ['Microsoft-Windows-WLAN-AutoConfig/Operational','无线网络连接事件'],
    ['Microsoft-Windows-NetworkProfile/Operational','网络配置文件事件'],
    ['Microsoft-Windows-DriverFrameworks-UserMode/Operational','用户模式驱动事件'],
    ['Microsoft-Windows-PrintService/Operational','打印服务事件'],
    ['Microsoft-Windows-BitLocker/BitLocker Management','BitLocker 管理事件'],
    ['Microsoft-Windows-CodeIntegrity/Operational','代码完整性事件'],
    ['Microsoft-Windows-AppLocker/EXE and DLL','AppLocker 程序与 DLL 事件'],
    ['Microsoft-Windows-AppLocker/MSI and Script','AppLocker 安装包与脚本事件'],
    ['Microsoft-Windows-Sysmon/Operational','Sysmon 系统活动事件'],
    ['Microsoft-Windows-Hyper-V-VMMS/Admin','Hyper-V 管理事件'],
    ['Microsoft-Windows-Backup','Windows 备份事件']
  ];
  result.eventlog=channels.map(([channel,name])=>({
    source:'pcscope.eventlog.'+channel.toLowerCase(),name,
    api:'Windows 事件日志 · '+channel+'；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml',
    method:'按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；'+(['System','Application'].includes(channel)?'每轮查询。':'扩展通道轮换查询。')+' 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；'+(channel==='Security'?'需相应审计策略和读取权限。':channel.includes('Sysmon')?'需已安装 Sysmon 并启用该通道。':'通道需存在且可读取；禁用通道也尝试读取保留历史。')
  }));
  const eventItems=typeof module!=='undefined'?require('./event-items.js'):PCScopeEventItems;
  result.eventlog=result.eventlog.flatMap((row,i)=>eventItems.expand(row,channels[i][0]));
  return result;
})();
if(typeof module!=='undefined')module.exports=pluginSources;
