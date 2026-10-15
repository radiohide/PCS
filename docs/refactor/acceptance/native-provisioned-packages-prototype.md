# 预配应用原生采集原型

2026-10-15 更新：已接入公开 WinRT 路径并迁入原生。八个真实包、完整五字段、无脚本编译路由及对象查询通过；旧 DISM 和旧参考在本机均拒绝，未当作成功或空清单。旧系统保留 DISM，方法存在时用公开接口。详见[五项验收](native-five-enhanced-migration.md)。以下为此前原型记录。

2026-10-14。原型已编译，布局/字段契约与普通权限失败路径通过，增强成功路径仍未验证，不切换生产归属。当前为 122 原生 / 9 待迁移 / 2 分组。

预配应用决定新账户登录时会获得哪些应用，不等同于当前用户安装清单，见 [Get-AppxProvisionedPackage](https://learn.microsoft.com/en-us/powershell/module/dism/get-appxprovisionedpackage?view=windowsserver2025-ps)。本机 DISM 命令实际调用 dismapi.dll 的 `_DismGetProvisionedAppxPackages`。原型由编译后端直接调用相同只读接口，不运行或加载 PowerShell，不调用安装或移除功能。

本机原接口布局已通过反射核对：1 字节对齐、x64 大小 68、Architecture 偏移 40、InstallLocation 偏移 52。包含包名/显示名/发布者指针、四个 uint32 版本段、架构及资源/位置/地区指针。后端只输出原五个字段 DisplayName、Version、Architecture、PackageName、InstallLocation，以 PackageName 为身份；版本格式与本机 DISM 的扩展 Version 属性一致，空位置保持 null，不填为猜测路径。

在线 DISM 会话使用独立诊断日志，读取后释放原生数组、关闭会话并关闭 DISM；数量和快照大小有界，宿主有总时限。不足权限、接口不存在、结构错误或读取失败均保留旧状态，没有脚本回退。该入口属于系统私有接口，版本可用性不能仅靠名称作承诺；读取成功验收及目标 LTSC/Windows 11 接口存在/布局核对继续开放。WinRT FindProvisionedPackages 的最低版本 19041 差异仍见[全量方案](../../backend-native-collection-plan.md)。

离线契约验证双记录数组步长、中文显示名、四段版本、数值架构、null 位置和缺身份拒绝。普通权限材料：`target/native-registry-tests/1791980404505/summary.json`，原生与原参考均因需要提升权限而拒绝，零快照。复验命令为 `pcscope-platform-native.exe provisioned-contract-test` 和 `node scripts/test-native-registry.cjs --provisioned`。

补强完整字段契约：此前数组材料复用了同一包名且第二条版本不匹配名称，现改为两个独立、版本匹配的包身份，覆盖 x64/neutral 数值架构、空显示名、空位置与中文位置；精确核对五字段完整 JSON，避免部分字段断言遗漏格式差异。结果为 `target/native-hyperv-vm-fields/provisioned-contract.json`。这是构造材料检查，仍不能代替真实在线成功读取，未改变生产归属或待授权的冻结程序。

已准备新的四项只读增强验收入口 `migration-assess`，固定查询可选功能、预配应用、安全启动和 BitLocker 元数据；要求当前账户 SID 一致，拒绝输出目录经过重解析点，输出文件使用 CreateNew。临时目录仅在此进程内改为工作区隔离目录，诊断日志不会使用默认系统日志路径。完成标记只表示四项查询执行完毕，来源结果还必须逐项验证，不把执行结束当作全部采集成功。

准备信息在 `target/native-admin-migration-assessment-request.json`；隔离程序摘要为 `0C2E4A00D16BE853A3FD7563CC60E58AB09034854537BF3EC70029B60BECA4BD`。三组离线契约通过，程序尚未获得本轮 UAC 授权或执行增强读取。不启动正式后台、不修改系统配置、不读取恢复密码/密钥。授权结果必须来自用户回复，文件中的准备状态不是授权。
