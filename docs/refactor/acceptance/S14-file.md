# S14 验收记录：文件与访问控制

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S14 · 待验收 |
| 实现范围 | 两部分：①采集端补发文件身份（`volume_identity` 与 `file_id`），覆盖三处扫描位置；②`pcscope.files.metadata-security` 的适配器，一个来源产出三个类型 |
| 代码定位 | 新增 `crates/contracts/src/adapters/file.rs`（适配器）；采集端：`PCScopeIdentity::FileIdentity`（`domains/IdentityNative.cs`）、helper `Add-FileIdentity` 与脚本根变量（`collect-domain.ps1`）、接线于 `domains/scoped.ps1` 与 `domains/desktop.ps1`；修改 `crates/contracts/src/adapters/mod.rs` |
| 验证环境 | Windows 11；PowerShell 7（`Add-Type` 编译 C# 并调用）；**对真实文件与真实目录实测**，非静态核对 |
| 验证过程 | 采集端：`Add-Type -Path IdentityNative.cs`；对真实文件、真实目录、不存在路径各调用一次；同一文件两次比对；把 `Add-FileIdentity` 的**函数原文抽出执行**再测一遍；三个改动脚本用 PowerShell AST 解析器做语法检查。适配器：`cargo test --workspace`；7 项新增测试，其中 ACL 用例使用**本机真实文件产出的 SDDL 字符串**（`%SystemRoot%\System32\kernel32.dll`）而非手工构造 |
| 结果 | 采集端：真实文件与目录返回 `observed` 及身份；不存在的路径返回 `unreadable` + Win32 错误 3；同一文件两次一致；三个脚本解析通过。适配器：真实 SDDL 解析出 6 条 ACE，受托者、符号掩码（`FA`）与十六进制掩码（`0x1200a9`）均如实承载。整体 309 项测试、17/17 检查、生成物逐字节未变 |
| 遗留问题 | 见第 3 节 |
| 下一步 | S21 全来源对账 |

## 1. 为什么动采集端

按 [文件身份从哪来](../file-identity-decision.md) 的决定（方案 A）：六个类型需要卷标识与文件 ID，而快照采集端从不发它们；R02 禁止适配器自行调用 Windows 补采。因此这个值只能由采集端补发。

## 2. 实测中发现并修掉的两个真实缺陷

**结构体对齐错误（会返回数字，但不是这个文件的）。** 第一版用 `long` 表示 `BY_HANDLE_FILE_INFORMATION` 里的三个 `FILETIME`。该原生结构是**纯 DWORD 序列**，而托管侧 `long` 会按 8 字节对齐插入填充，导致其后每个字段都从错误偏移读取。第一次实测返回的就是**看似合理的假值**：卷序列号恒为 `0`、文件索引低 32 位恒为 `0`。改为全部 `uint`（13 个 DWORD，与原生布局逐一对应）后，卷序列号 `64770b15`、文件 ID `000500000017455b`——其中低半部与错位时读到的 `0017455b` 吻合，正好印证了偏移问题。

**`$PSScriptRoot` 在函数体内不可靠。** helper 原本在函数里现取 `$PSScriptRoot` 去定位 `IdentityNative.cs`。抽取执行时它为空，`Add-Type` 失败，于是**每一个文件都返回 `unreadable`**——一个不会报错、只会让功能静默失效的失败方式。改为在 `collect-domain.ps1` 加载时把根目录存入 `$script:AdapterRoot` 再由 helper 使用。

## 3. 适配器的三处判断

- **路径不是身份。** `file.watched_file` 的键是卷标识与文件 ID；路径是字段。文件改名后路径变了而身份不变，这正是类型选择 `file_identity` 的理由。
- **一个 item 是一棵树。** 同一个 item 里的 `dacl_sddl` 展开为若干 `file.acl_entry`，`streams` 展开为若干 `file.alternate_data_stream`——每个 ACE、每个数据流都是独立对象。默认数据流（`:$DATA`）不算「alternate」，不产出对象。
- **读不到身份的文件不产出对象。** 若照常产出但把两个键记为缺失，**所有读不到的文件会共用同一对缺失键而并成一个对象**；有测试专门钉住这一点。

## 4. 遗留问题

1. **便携软件的扫描只覆盖用户显式配置的根**（`Scan-Root` 由 `portable_roots` 驱动，默认空），因此 `software.portable_registration` 的身份只在用户配置了根之后才有对象。这是既有行为，不是本步引入。
2. **`file.watched_directory` 没有任何来源**（`source-map.cjs` 无对应行），按计划应标记为未接入，不补空适配器。
3. **本步是唯一改了 `existing-runtime` 工作线所属文件的步骤**，改动的依据是 [文件身份从哪来](../file-identity-decision.md) 中选定的方案 A，并已取得项目所有者确认。
4. **未做真实采集实测**：三处扫描位置都在真实采集流程里，本次只对 helper 与函数原文做了实测，未跑完整采集流程（需要配置监控名单与便携根目录）。
