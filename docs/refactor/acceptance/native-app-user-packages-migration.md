# 当前用户应用包原生采集

2026-10-14。新增应用包原生路由，当前为 122 原生 / 9 待迁移 / 2 分组，累计迁移 82 项。实际后台尚未更新。

编译后端直接使用 Windows Runtime PackageManager.FindPackagesForUserWithPackageTypes，明确传入当前账户 SID 和 Main | Framework。保持原默认范围，不混入资源包、Bundle 或其他用户登记。默认过滤规则见 [Get-AppxPackage](https://learn.microsoft.com/en-us/powershell/module/appx/get-appxpackage?view=windowsserver2025-ps)，原生接口见 [FindPackagesForUserWithPackageTypes](https://learn.microsoft.com/en-us/uwp/api/windows.management.deployment.packagemanager.findpackagesforuserwithpackagetypes)。

保留 PackageFullName 身份及原 12 个字段：Name、PackageFullName、PackageFamilyName、Version、Architecture、Publisher、InstallLocation、IsFramework、IsBundle、NonRemovable、SignatureKind、Status。版本保持四段字符串，架构/签名类型保持原枚举数值。状态组合保留原 12 个标记，不能只用 VerifyIsOK 把异常状态压成一个布尔值，字段定义见 [PackageStatus](https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.packagestatus)。

NonRemovable 按本机原命令实现核对：系统签名直接为 true，其他包通过 appxalluserstore.dll 的 IsPackageFamilyInUninstallBlocklist 只读查询。它是系统已有接口，未将 PowerShell 程序集带入后端；接口读取失败则整批保留旧状态，不猜测 false。此接口的跨版本成功读取仍需验收，不凭本机测试声称全部 Windows 版本已支持。

读取不安装、更新、修复或移除包，不访问用户应用数据。数量和快照大小受限，进程总时限继续生效；任何字段/枚举失败拒绝整个快照，没有脚本回退。构建使用系统 Windows 元数据和 .NET Framework Windows Runtime 支持，独立运行不依赖 Codex 或 PowerShell 环境。

正式无脚本隔离路由材料：`target/native-registry-tests/1791980116762/summary.json`。62 个真实应用包身份、全部字段与原结果一致，单轮原生 711 ms、原参考 3143 ms；不将单轮测量当作长期性能承诺。离线契约验证 12 个独立状态标记、组合标记、正常状态及缺字段拒绝；共享宿主 25 项契约回归通过，确认未加载 PowerShell 引擎。Rust 归属、来源、迁移清单和文件归属检查通过。

复验命令：`pcscope-platform-native.exe app-package-contract-test` 与 `node scripts/test-native-registry.cjs --appx-user --wrapper`。其他 Windows 版本、异常/离线包、增强权限下相同账户范围、长期性能和最终部署继续开放。预配应用是另外一个来源，仍待迁移。
