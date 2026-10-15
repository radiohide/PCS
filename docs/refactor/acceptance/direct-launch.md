# 双击启动验收

后续已支持 debug exe 本身直接启动，见 [后续验收](debug-direct-settings.md)。本文保留最初快捷方式方案的历史事实。

日期：2026-10-14。源码基准 `pcscope`，开发包 `dist/direct-launch-fixed`，模型 1.2.6。

用户报告 `apps/desktop/src-tauri/target/debug/pcscope-desktop.exe` 双击退出。按默认环境实际复现，退出码 101，Tauri setup hook 报 `拒绝访问 (os error 5)`。文件继承工作区低完整性标签，默认启动目录不可访问；该编译目录也没有配套的 `pcscope-service.exe`。未修改工作区权限或标签。

修复分为两部分：

- 原始构建程序初始化失败时，显示“PCScope 无法启动”错误窗口，将详细错误写到同目录 `pcscope-startup-error.log`，有快捷方式时直接提示使用该入口；不再静默退出。
- `scripts/prepare-desktop-preview.ps1` 校验完整包后，复制到 `%LOCALAPPDATA%/Programs/PCScopePreview/<文件集合哈希>`。项目根目录及指定 debug 目录均生成“打开 PCScope.lnk”。入口明确传入数据目录与 WebView 缓存目录，运行程序不依赖 PowerShell 或临时环境变量。

原生启动新增 `--data-dir`、`--webview-dir`，拒绝不完整、重复、未知或相对目录参数；无参数时保留原有独立应用默认目录和环境变量行为。`scripts/open-preview.ps1` 统一使用上述准备流程和入口，支持指定刚生成的开发包。不同版本分别存放，避免覆盖正在运行的程序；不删除旧版本或历史数据。

验收结果：

1. 9 项原生桌面测试通过（其中 4 项新增启动参数与可读错误测试）。后台连接和产品状态回归通过。
2. 29 个程序文件复制后 SHA-256 一致。
3. 原始受限 exe 已实测显示错误窗口并写出诊断；关闭提示后退出码为 1。
4. 移除 `PCSCOPE_DATA_DIR` 和 `WEBVIEW2_USER_DATA_FOLDER`，以 ShellExecute 打开指定 debug 目录的真实 `.lnk`。窗口标题 PCScope、正常响应，桌面 PID 23104；实际路径为普通应用目录下的完整包。原后台 PID 25060、原数据目录和增强采集权限均保持。验收时间 2026-10-14 09:34。

证据：`target/direct-debug-start.stderr.log`、`target/direct-launch-preview.json`（完整文件哈希与启动参数）、`target/direct-launch-live.json`。

边界：原始受限编译 exe 仍不能直接代替完整运行包，本轮提供可靠快捷入口和明确失败提示。没有安装正式安装器、Windows 服务或开机任务；已有链接器资源清单警告仍未消除。
