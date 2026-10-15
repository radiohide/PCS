# Hyper-V 检查点原生原型

2026-10-14。检查点原型已编译，身份/时间/父 ID 和七种类型契约通过，未切换生产归属。仍为 122 原生 / 9 待迁移 / 2 分组。

通过本机 MI 读取 Msvm_ComputerSystem，并通过 Msvm_SnapshotOfVirtualSystem 的 Antecedent→Dependent 关联获取检查点。父关系通过 Msvm_ParentChildSettingData 的 Dependent→Antecedent 查询，不读取子节点作为父节点、不把当前配置当作检查点。接口见 [SnapshotOfVirtualSystem](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/msvm-snapshotofvirtualsystem) 和 [ParentChildSettingData](https://learn.microsoft.com/en-us/windows/win32/hyperv_v2/msvm-parentchildsettingdata)。只枚举和读取，没有创建、删除、应用或导出调用。

本机原 getter 的只读元数据审计确认：检查点 ID 来自 ConfigurationID，所属 VM 来自 VirtualSystemIdentifier，名称来自 ElementName，时间来自 CreationTime，父 ID 为可空 GUID。七种 VirtualSystemType 对应原 SnapshotType 0/1/2/5/6/7/8，包含复制和应用一致性检查点，不缩减为标准检查点。材料在 `target/hyperv-field-reference-audit.json`，同时核对了 [Get-VMSnapshot 的默认范围](https://learn.microsoft.com/en-us/powershell/module/hyper-v/get-vmsnapshot?view=windowsserver2025-ps)。

保留原六个字段 Id、VMId、Name、CreationTime、SnapshotType、ParentSnapshotId，GUID 规范化、时间按原隐私策略转 UTC。错误所属 VM、空/无效 ID、自指父关系、歧义父关联、存在 Parent 引用但无法确认的关系、未知类型和缺失时间都拒绝整批快照，不填假根节点或删除旧记录。只有成功读取后才能确认空列表。操作超时、VM/检查点数量及宿主总时限有界，不递归查询父链，不产生循环采集。

`hyperv-snapshot-contract-test` 验证非空样本 ID、所属 VM、单个父 ID、根 null 和精确 UTC 时间，七种类型和拒绝当前配置、错误 VM、自指父 ID。这里的父映射检查不是实际多层检查点链路的成功验收。

普通权限材料为 `target/native-hyperv-snapshot-ordinary.jsonl`：本机缺少命名空间，输出一个 gap、零快照，没有假空清单。来源和迁移清单检查通过。正式后台和用户历史库未改变。

仍待实际非空读取、嵌套/分支父关系与孤立记录、复制与缺失检查点、目标版本及字段完整比对。磁盘连接来源仍未实现，虚拟机与交换机原型的兼容/成功路径继续开放；不能把当前缺组件结果当作全部 Hyper-V 迁移完成。原四项 UAC 准备程序与授权请求未改变。
