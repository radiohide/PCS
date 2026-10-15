# Hyper-V 字段审计与交换机原型

2026-10-14。本轮新增交换机原生原型和 Hyper-V 字段审计，未切换生产归属，当前仍为 122 原生 / 9 待迁移 / 2 分组。Hyper-V 的四个来源保持完整目标，不缩减成只采名称。

本机 Hyper-V PowerShell 模块路径不存在，原生命名空间查询返回“无效命名空间”。未启用组件、创建虚拟机或调整现有网络。通过只读反射检查系统组件仓库中已有程序集，记录枚举、字段类型及交换机 getter 的实际转换规则；没有调用其中的管理命令或构造会访问系统的对象。材料在 `target/hyperv-field-reference-audit.json`，审计程序为 `target/audit-hyperv-fields.ps1`。

已核对旧接口的 State、ControllerType、SnapshotType、SwitchType 数值及多数字段类型。新版 Objects 和 Common.Types 压缩程序集已在隔离目录有界解压，ReflectionOnly 审计补齐关键 getter；不加载它们执行管理代码。当前本机版本证据仍不能代替全部目标版本及成功环境验收。

管理客户端程序集 Microsoft.Virtualization.Client.Management 引用 System.Management.Automation，不能作为生产原生替代直接加载。本轮原型只使用系统 MI/System.Management 元数据与引用解析，无 Hyper-V 管理程序集或 PowerShell 引擎依赖。Hyper-V 的底层接口见 [WMI V2](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/refactor-wmi-v1-to-wmi-v2)。

交换机保持五个原字段及 GUID 身份。类型按外部连接优先，其次内部连接，否则 Private；普通外部网卡描述按原 getter 来自 ExternalEthernetPort.Name，不猜 ElementName。新版 VMSwitch.get_AllowManagementOS 直接返回内部宿主网卡集合是否非空，内部交换机也可为 true；原型已纠正旧 getter 带外部条件的转换。内部连接由关联的端口分配 HostResource 指向本机 Msvm_ComputerSystem 确认，不直接靠名字或网卡数量猜测。旧/新版本的属性行为差异需列入最终环境验收。

新版 NetAdapterInterfaceDescription 在未组队时取描述数组首项，在 SET 组队时返回专用描述。原型现已读取设置的 TeamingEnabled，并在启用组队时有界读取多个 HostResource 外部端口，不再无条件拒绝多个成员；非组队多绑定、缺组队字段或异常引用仍拒绝。管理层 getter 确认 TeamingEnabled 直接来自同名提供程序属性，成员来自端口分配的 HostResources；属性范围见 [Hyper-V 新增字段](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/what-s-new-in-hyper-v)。

`target/read-hyperv-audit-resources.ps1` 使用 ReflectionOnly 资源流及 ResourceReader，只读字符串，确认组队描述资源为英文 Teamed-Interface、简体中文“组合接口”；未调用资源 getter。原型新增可选本机语言资源查找，按当前 UI 语言及父语言读取固定系统模块/GAC 目录的资源程序集，版本候选最多 64、程序集/资源/字符串分别限 4 MiB/1 MiB/1024 字节。程序集只用于 ReflectionOnly；通过 GetResourceData 验证 ResourceTypeCode.String 并解析原始字符串，拒绝反序列化自定义资源对象，不加载管理程序集执行代码。

缺少语言资源时采用已核对的通用/简体中文描述，移除原“其他语言直接失败”的人工边界；不依赖 PowerShell 引擎，也不要求安装模块才能读取交换机状态。字符串解码、非字符串资源拒绝及原字段契约通过；真实安装语言资源、SET 关联和成功字段比对仍开放，不能据此切换生产归属。

原型查询交换机、关联设置与端口分配，读取指定本机外部端口。引用必须是本机 root/virtualization/v2 的实例，类型、键、编码和长度受限；拒绝远程主机、未知连接类别、丢失设置、丢失物理网卡或非组队多外部绑定等异常结构。空/未连接分配与未知布局分开处理，失败保留旧快照，不当作空清单。查询有操作时限、数量和宿主总时限；没有修改或创建调用。

离线 `hyperv-switch-contract-test` 通过内部/外部字段、宿主引用、远程引用拒绝和缺网卡拒绝检查；新增内部连接为 true 与私有交换机不编造宿主连接，结果在 `target/native-hyperv-vm-fields/switch-contract.json`。普通权限材料为 `target/native-hyperv-switch-ordinary.jsonl`，结果一个 gap、零快照，实际 WMI 提供程序不存在。来源清单、迁移清单和文件归属检查通过。

尚未完成：交换机实际关联和成功字段比对、Private/Internal/External、SET/多物理绑定、Default Switch 及目标版本差异。虚拟机、磁盘连接和检查点实现继续开放。未验证布局不能以当前简单原型代替原完整范围；不能因当前机器无组件而声明四项迁移完成。原四项 UAC 准备程序保持不变，未扩大其待授权范围。
