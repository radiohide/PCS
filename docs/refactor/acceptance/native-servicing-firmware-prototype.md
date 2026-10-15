# 可选功能与安全启动原生采集原型

2026-10-15 更新：134 个真实可选功能和安全启动读取成功、字段与原参考一致，均已迁入原生，实际无脚本路由与对象查询通过。其它固件/版本矩阵仍开放。详见[五项验收](native-five-enhanced-migration.md)。以下为此前原型记录。

2026-10-14。两项实现已加入编译后端，但尚未切换生产 ownership；总进度仍为 117 项原生、14 项待迁移、2 项分组。正在运行的软件尚未更新。

可选功能使用 `DismGetFeatures` 读取在线系统，保留原 FeatureName 身份和 name/state 字段；DISM 结构按 SDK 的 1 字节对齐。状态映射覆盖 0–7，未知值拒绝整个快照。仅调用查询接口，不启用或禁用功能。接口和状态定义见 [DismGetFeatures](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismgetfeatures-function?view=windows-11) 与 [DismPackageFeatureState](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismpackagefeaturestate-enumeration?view=windows-11)。

本机 DISM 程序集的独立枚举元数据已保存为 `target/native-dism-feature-state-metadata.json`。PackageFeatureState 数值与显示用 FeatureState 数值不同，例如 Installed=4 与 Enabled=2；原脚本明确输出 State 字符串，原型因此保留名称而不是把原生数字直接展示。Staged=2、Removed/Resolved=3 及其他已映射状态已核对；此审计仅读取元数据，未构造查询对象、初始化 DISM 或查询在线功能。

安全启动先读取固件类型，再临时启用采集进程自身已有的 SeSystemEnvironmentPrivilege，读取 EFI 全局命名空间的 SecureBoot 变量。读取后恢复权限并关闭令牌；没有固件写入或系统设置修改。只接受长度为 1、值为 0/1 的结果。旧式启动、不足权限、缺失变量和格式错误都作为读取失败保留先前状态，不生成 enabled=false。权限要求见 [GetFirmwareEnvironmentVariableExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfirmwareenvironmentvariableexw)。

隔离测试结果：`target/native-registry-tests/1791977806067/summary.json`。普通权限下，两项与原采集都无法读取；原生输出各一个 gap、零快照，没有把读取失败提交为正常或不存在。离线契约检查验证结构布局、八种状态、未知状态拒绝及固件值检查。来源清单与迁移清单检查通过。

尚未完成：增强权限下读取实际可选功能并逐字段比对；安全启动开启/关闭平台结果比对；真实旧式固件与变量缺失环境验收。当前失败路径验证不能代替成功路径验收，因此两项继续列入待迁移，不计为完成。

复验命令：构建 `plugins/platform-native/build.ps1` 后，使用工作区隔离副本运行 `servicing-firmware-test` 和 `node scripts/test-native-registry.cjs --servicing-firmware`。测试只读、不启动正式后台、不修改用户数据库。无需为每轮采集弹出 UAC；增强权限由后台启动边界处理。
