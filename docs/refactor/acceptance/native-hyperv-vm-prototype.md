# Hyper-V 虚拟机原生原型

2026-10-14。虚拟机只读原型已编译，状态/内存契约及缺组件失败路径通过，未切换生产归属。仍为 122 原生 / 9 待迁移 / 2 分组。

直接查询本机 Msvm_ComputerSystem 及当前 Realized 设置，关联内存和处理器配置；只通过固定 GetSummaryInformation 请求 HealthState(109)，不调用启动、停止、创建或配置方法。字段依据见 [VirtualSystemSettingData](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/msvm-virtualsystemsettingdata)、[MemorySettingData](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/msvm-memorysettingdata) 和 [GetSummaryInformation](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/getsummaryinformation-msvm-virtualsystemmanagementservice)。

原型包含原 14 个字段：ID、名称、状态、代际、版本、处理器数量、启动/最小/最大内存、动态内存、启动/关机动作、配置位置及检查点类型。内存明确按提供程序单位换算成字节，单位未知拒绝快照，不猜乘数。运行状态结合实际摘要 HealthState 的严重错误状态；未知数值保留，缺少必要关联和摘要拒绝整批结果。

已扩展本机程序集的只读元数据审计：启动/关机动作枚举保持 2/3/4，版本为字符串，内存/处理器数量为 Int64；状态会依据 OtherEnabledState 与健康状态转换。材料仍在 `target/hyperv-field-reference-audit.json`。新版 Objects 文件已按单块 DCS 的有界长度检查，使用系统 Cabinet LZMS 原始块解压至工作区，未安装或修改系统组件；见 `target/expand-hyperv-audit.ps1`、`target/hyperv-objects-audit.dll` 和 [CreateDecompressor 的原始块模式](https://learn.microsoft.com/en-us/windows/win32/api/compressapi/nf-compressapi-createdecompressor)。读取程序集仅用 ReflectionOnly，未实例化管理对象或调用 getter。

新版 `VirtualMachine.get_CheckpointType` 直接返回 UserSnapshotType，枚举值为 Disabled=2、Production=3、ProductionOnly=4、Standard=5。原证据函数保留 ValueType，Windows PowerShell JSON 将枚举写为数值；`target/test-hyperv-enum-wire.ps1` 通过实际原证据函数验证 `{"CheckpointType":4}`，原型已由名称改为数值以保持字段兼容。

新版 `VirtualMachine.get_State` 在摘要 HealthState 不为 5 时调用 CriticalStateConverter.GetCriticalState。读取该转换器 IL 及枚举后，发现原型的异常状态编号偏低四位：RunningCritical 应为 32785，OffCritical 为 32786，至 FastSavingCritical 为 32796；已修正全部十二种映射，并包含 HealthState=10 的非正常摘要处理。构造契约同时检查健康状态 5 下保持原值、未知状态保留。这里只证明转换与当前本机程序集一致，代际、Realized 选择、其他状态转换和摘要关联仍需真实成功环境比对。

进一步读取 `VMComputerSystemBaseView.get_State` 和 `VMComputerSystemStateUtilities.ConvertVMComputerSystemOtherState`：EnabledState=1 时仅识别 Quiescing、Resuming、Saving、FastSaving、ComponentServicing 五个提供程序文本，忽略大小写；Quiescing 对应 Pausing(32776)，维护对应 32784。原型已改为这些确切文本，删除猜测 Running/Paused 等普通状态名称的转换；未知原始数值保持，未知 Other 文本仍为原始 Other=1，不推断健康状态。强制关机/重启、休眠及维护 32781–32784 数值保持契约通过。当前记录证明本机元数据和离线转换，真实提供程序及旧版本行为仍待验证。

离线 `hyperv-vm-contract-test` 验证严重状态、Other 状态、未知状态保留、4 GiB 字节换算、未知单位拒绝和检查点含义。2026-10-14 补齐完整 14 字段的构造与序列化检查，通过名称、状态、版本、三种内存值、动作和位置的精确输出比对；这是构造材料验证，不是实际提供程序字段等价验收。新增 VM 与 ConfigurationID 不一致、负内存、换算溢出拒绝，避免错误关联或无效数值进入状态快照。测试在 `target/native-hyperv-vm-fields/pcscope-platform-native.exe` 普通应用完整性级别运行并通过，不调用 Hyper-V 接口。

此次契约输出保存于 `target/native-hyperv-vm-fields/contract.json`，原枚举序列化验证保存于同目录 `legacy-enum-wire.json`。普通权限材料为 `target/native-hyperv-vm-ordinary.jsonl`，因本机命名空间不存在，只有 gap、没有空快照。来源和迁移清单检查通过。没有创建、启动或调整虚拟机。

尚待完整字段/类型兼容、实际 VM 状态与代际矩阵、真实资源和摘要关联、配置异常、目标版本与性能验收。磁盘连接和检查点已有原型，真实成功路径仍待验收；正式部署和已请求的四项 UAC 授权仍开放，授权范围没有扩张。
