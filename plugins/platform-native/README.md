# 原生平台采集器

使用系统 .NET Framework x64 编译宿主直接调用 Registry、WMI、LSA、WTS、CNG、DISM、ACL 与文件 API；不启动或加载 PowerShell。`build.ps1` 输出 `target/debug/pcscope-platform-native.exe`，开发打包脚本自动构建并复制。

正式路由由 `ownership.json` 控制。每个来源使用既有协议和范围：基础来源保持原单快照，其余来源保持 16 个完整分片；一个来源读取失败输出 gap，旧状态保留。包装器会禁用兼容脚本中的原生归属来源，不因原生失败自动回退。WUA 历史已在普通应用完整性级别通过 136 条实机比对并进入正式原生归属，低完整性拒绝，其他环境仍分别验收。

`State.cs` 直接使用 Windows 系统 GAC 中的 Microsoft.Management.Infrastructure，保留隐藏网卡等提供程序选项和旧采集语言；不加载 System.Management.Automation，也不启动脚本或命令行。新增系统状态采集在低完整性进程中拒绝提交结果，防止系统接口静默缩减范围被当作对象消失。`state-test` 执行离线范围、原子性、日期及敏感字段测试；`scripts/test-native-state.ps1` 在无兼容脚本的隔离目录验证正式包装器、低完整性拒绝和缺少原生程序的失败路径。

`RegistryState.cs` 按原路径、深度与 32/64 位视图直接读取配置。网络、任务、注册表的比对与权限边界见[新批次验收](../../docs/refactor/acceptance/native-network-registry-migration.md)。系统任务提供程序 PS_ScheduledTask 的 GetInfoByObject 是固定只读接口，类名不意味着加载 PowerShell。原生归属与权限成功分别记录，不通过跳过受保护键来提交缩减的完整快照。

`capabilities` 只读检测。`capabilities --repair` 是显式修改入口，只有已提权且产品设置明确启用时使用。`assess <已有隔离目录> <原用户 SID>` 是固定只读验收入口，缓存驱动 API 材料，检查同一账户，不启动正式后台、不修复系统配置。`layout-test` 离线验证 DISM x64 SDK 结构和数组布局。

范围、权限、性能、防风暴与尚未完成的工作见 [N04 补充验收](../../docs/refactor/acceptance/N04-native-safety.md)。开发包处于用户可写工作区，不能当作受保护安装目录中的系统服务发行版。
