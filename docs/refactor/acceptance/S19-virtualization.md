# S19 验收记录：虚拟环境

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S19 · 待验收 |
| 实现范围 | 虚拟化类 7 个观察来源的适配器：Hyper-V 的虚拟机/交换机/磁盘挂载/检查点、WSL 注册、Docker、平台能力 |
| 代码定位 | 新增 `crates/contracts/src/adapters/virtualization.rs`；修改 `crates/contracts/src/adapters/mod.rs`（注册 `virtualization::SOURCES`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；素材键名取自 `domains/virtualization.ps1`、`collect-system.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；21 项新增测试；17 个 `scripts/check-*.cjs`；构建零警告 |
| 结果 | 248 项 Rust 测试通过；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 3 节 |
| 下一步 | S20 桌面与应用集成 |

## 1. 一个来源覆盖四个类型

`pcscope.virtualization.docker` 的 item 形如 `Item "$($spec.type)/$identity" @{kind=$spec.type;data=$data;basis=…}`：**载荷里的 `kind` 决定类型**，item id 里的身份串按 kind 而不同（镜像的是 `id/repository/tag` 三段）。适配器按 `kind` 分派：容器、卷、网络用 `native_identifier`（`native_id` 取 `data.id`），**镜像**用 `composite_native`（三个键都来自载荷）。

采集端自己的依据随材料一起来——本地 API 元数据，不含日志、参数、环境变量与容器内容——代码中没有一句话与它相左。

## 2. 三处「宁缺勿猜」

- **虚拟机的 `memory` 记为缺失。** 类型声明一个内存字段，采集端发四个（`MemoryStartup`、`DynamicMemoryEnabled`、`MemoryMinimum`、`MemoryMaximum`）且没有 `memory` 键。**四个里一个都不挑，也不做聚合**——挑一个或算一个都是在替来源下结论。同一来源的 `cpu` ← `ProcessorCount` 是唯一键的重命名，所以填了；这个不对称是刻意的。
- **Lxss 根键记为跳过并说明原因。** 采集端 `Registry … Lxss … 1` 会产出根键与其直接子键；根键是注册容器，四个声明字段（名称、版本、位置、默认 UID）没有一个承载子系统键的值。与 `policy.rs` 的电源设置键处理一致。
- **WSL 的 `native_id` 取 Lxss 子键名（注册 GUID），不取 `DistributionName`。** 类型是 `native_identifier` 而非 `registry_registration`，且其 `insufficient` 说明明确禁止以名称为键——同名的两个发行版必须保持两个对象。**注册表视图也没有折进身份**：它是读取过程的产物，折进去会让一次注册变成两个对象。

## 3. 遗留问题

1. **检查点作用域取安装作用域，与类型的作用域描述不完全一致。** 类型的描述文字是「虚拟机作用域」，但它的键是 `[scope, native_id]`，没有承载虚拟机的成分（`disk_attachment` 有 `vm_id` 键，所以不同）。**这是全批唯一一处以「声明的键」压过「作用域描述」的地方**——若原意是按虚拟机分作用域，需要类型补一个键或采集端把它放进作用域，列入 S21。
2. **样本里有一条形状已过期的 WSL item**（`id` 是发行版名、`data` 是 `{name,version,location,default_uid}`），没有任何 emitter 产生这种形状。适配器按 emitter 实现（与 `startup.rs` 的先例一致），该 item 因此返回 `NotAnObject` 并带原因。没有示例读它，所以没有回归；样本的更正属 S02/S21。
3. **未映射的载荷键留在证据里**：虚拟机的 `Version`/`Automatic*Action`/`ConfigurationLocation`、交换机的 `AllowManagementOS`、磁盘的 `VMName`/`DiskNumber`、平台能力的 `manufacturer`/`model`、WSL 的 `Flags`/`State`（source-map 自己写着运行状态尚未接入）。**没有为它们发明字段。**
4. **本机未做真实采集实测**：Hyper-V 在本机不存在，Docker 与本机 WSL 未接；全部验证在 `:memory:` 上用手工构造的素材完成，键名来自 emit 声明的静态核对。
