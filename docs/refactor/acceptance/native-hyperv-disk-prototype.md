# Hyper-V 磁盘连接原生原型

2026-10-14。原型已编译，连接字段、身份和共享引用契约通过，未切换生产归属。仍为 122 原生 / 9 待迁移 / 2 分组。四个 Hyper-V 来源现在均有原生原型，成功读取与字段兼容仍未全部验收。

读取本机 VM 的当前 Realized 设置及资源图，区分 Synthetic Disk Drive 与 Physical Disk Drive，按 Parent 关联控制器和虚拟磁盘介质。保留 VMId、VMName、Path、ControllerType、ControllerNumber、ControllerLocation、DiskNumber；身份仍使用 VM GUID/IDE 或 SCSI/编号/插槽。资源定义见 [ResourceAllocationSettingData](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/msvm-resourceallocationsettingdata) 和 [StorageAllocationSettingData](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/msvm-storageallocationsettingdata)。

本机原 getter 元数据审计确认：插槽来自 AddressOnParent；虚拟磁盘路径来自附加介质 HostResource，物理 DiskNumber 为空。直通磁盘的 Path 来自所引用设备 ElementName，DiskNumber 来自主机物理设备 DriveNumber，不将显示名当作文件路径打开。字段定义见 [Msvm_DiskDrive](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/msvm-diskdrive)。审计材料仍为 `target/hyperv-field-reference-audit.json`。

已进一步核对原 getter：IDE 和 SCSI 控制器按返回顺序分别从 0 编号，不使用 Address。原型已经修正为单独枚举同一资源类并保持各类型的返回顺序；交错类型、非字典顺序和重复身份拒绝契约通过。MI 与原 WMI 客户端在真实环境中的返回顺序一致性、直通磁盘本机引用键及设备关联仍需成功环境比对。

2026-10-14 新版 Objects/Common.Types 已在工作区解压用于纯反射审计，当前 HardDiskDrive.get_DiskNumber 调用物理设备 PhysicalDiskNumber，管理层 VMHardDiskDriveView.get_PhysicalDiskNumber 直接读 DriveNumber，确认原型该字段读取来源。新版 Path 对物理设备查询失败保留 null；原型当前拒绝失败关联并保留整个来源旧状态，成功路径与该失败行为差异仍待完整验收，不能据此声明新版完整兼容。

所有引用限定本机 namespace/固定类和键，不连接远程 WMI；父控制器必须属于当前 VM 资源集。未知磁盘/控制器类型、丢失编号、父关系无法确认、重复资源身份或多介质返回均拒绝整批快照并保留原数据。无介质的已配置插槽保留 null 路径，不编造磁盘编号。数量、操作和宿主时限有界，不循环重试。

只读取配置元数据，不打开 VHD/物理磁盘，不挂载、不运行磁盘检查或调整 VM。`hyperv-disk-contract-test` 通过 IDE/SCSI 身份、虚拟路径/空编号、物理显示名/编号、引用转义、缺编号和多介质拒绝测试；共享交换机引用契约回归通过。普通权限材料 `target/native-hyperv-disk-ordinary.jsonl` 表明本机命名空间不存在，一个 gap、零快照。来源、迁移和文件归属检查通过。

仍需实测 MI/WMI 控制器返回顺序一致性、实际资源继承/关联、未附介质插槽、直通设备键布局、共享 VHD/AVHDX、不同代际与目标版本以及完整字段比对。其他 Hyper-V 原型、增强安全/系统四项验收、长期性能与实际部署继续开放。原四项 UAC 准备程序和请求没有扩张或替换。
