# S18 验收记录：硬件与驱动

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S18 · 待验收 |
| 实现范围 | 硬件类 11 个观察来源的适配器：设备、驱动绑定、驱动服务、固件、磁盘健康、电池、磁盘、分区、卷、TPM，以及 USN 卷（Rust 插件发出，形状与 PowerShell 不同） |
| 代码定位 | 新增 `crates/contracts/src/adapters/hardware.rs`；修改 `crates/contracts/src/adapters/mod.rs`（注册 `hardware::SOURCES`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；素材键名取自 `domains/storage.ps1`、`domains/security.ps1`、`collect-system.ps1` 与 `plugins/usn/src/main.rs` |
| 验证过程 | `cargo test --workspace`；17 项新增测试；17 个 `scripts/check-*.cjs`；构建零警告。实施中另用一个临时测试把全部样本过了一遍 `validate()`，发现违规后修正再删去 |
| 结果 | 248 项 Rust 测试通过；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 3 节 |
| 下一步 | S19 虚拟环境（已并行完成） |

## 1. 类型声明的字段与采集端提供的键不总是一一对应

- **`security.tpm` 指向 `hardware.tpm`，而该类型没有 `name` 字段**——`name` 只作为 `named_entity` 的身份键存在。实施时的临时校验测试先发现了这条违规，**没有为承载名字而发明一个字段**。这是本批里校验器直接拦住错误的一例。
- **`smbios_version` 由 `SMBIOSMajorVersion` 与 `SMBIOSMinorVersion` 拼成 `"major.minor"`**：类型声明一个字段，`Win32_BIOS` 给的是两个数字。拼接后标为 `Derivation::Decoded`（本批首次使用该推导方式）使拼接可见；任一数字缺失或为空则整个字段记为缺失。**若项目惯例更严，这是唯一需要回退的一行。**
- **电池的 `chemistry` 与 `estimated_time` 原样承载**，包括 `71582788` 这个「无估算」哨兵值。展开它属于解读，不是采集——**但这也意味着界面上会显示一个看起来像真值的数字**，是否要为该哨兵单独定状态列入 S21。

## 2. 跨来源与作用域的两处不一致

- **`hardware.disk` 的原生标识是两个来源各自的 `ObjectId`。** `Get-Disk` 与 `Get-PhysicalDisk` 都把 `ObjectId` 放在身份位置，所以适配器陈述的就是来源说的。**两个 `ObjectId` 对同一块盘是否逐字节相同是实机问题**，适配器没有加兜底，已在模块文档中注明，并入 S21。
- **`usn.volumes` 的作用域是盘符作用域（`fixed-drive-letters/v1`），不是机器作用域。** `installation_scope` 对它原样透传，因此 USN 采集到的卷与 PowerShell 采集到的同一个卷**不会共用作用域，会成为两个实例**。没有靠发明一个机器作用域把它糊过去——差异已记入勘察文档第 3 节，列入 S21。

## 3. 遗留问题

1. **同一卷两个来源作用域不同**（第 2 节），需要在 S21 决定由谁统一：采集端补发机器作用域，还是身份策略允许跨来源归一。
2. **两个 `ObjectId` 是否等价未经实机核对**（第 2 节）。
3. **`71582788` 哨兵值原样承载**（第 1 节），是否单独定状态待定。
4. **`smbios_version` 的拼接**是本批唯一组装出来的字段值，标了推导方式；若惯例不允许拼接需回退。
5. **本机未做真实采集实测**；电池、TPM、磁盘健康等在虚拟机与受限权限下均无真实输出，键名来自 emit 声明的静态核对。
