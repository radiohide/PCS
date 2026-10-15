# debug 直接启动、显式路径与配置读取

日期：2026-10-14。最新包 `dist/debug-direct-ready`；直接运行入口 `apps/desktop/src-tauri/target/debug/pcscope-desktop.exe`。

## 配置与运行

“系统配置 → 数据与程序位置”提供数据目录及后台/采集程序目录。数据目录必须已存在，依赖目录需要完整采集程序和脚本。保存仅原子更新 exe 旁的 `pcscope-launch.json`；无效目录不会覆盖原配置，不迁移历史、不发起采集、不申请权限。重新打开界面后使用新配置；已经运行的后台不会自动替换。

启动配置示例（绝对路径按本机修改）：

```json
{"version":1,"data_dir":"D:\\PCScopeData","runtime_dir":"D:\\PCScopeRuntime","webview_dir":"D:\\PCScopeWebView"}
```

优先级：启动参数 > 对应环境变量 > 配置文件 > 应用默认值。支持 `--data-dir`、`--runtime-dir`、`--webview-dir`，对应 `PCSCOPE_DATA_DIR`、`PCSCOPE_RUNTIME_DIR`、`WEBVIEW2_USER_DATA_FOLDER`。配置文件要求 data_dir；其余目录可省略。显式参数或环境变量覆盖的目录在界面禁用编辑。错误配置会报告启动失败，不能偷偷切换到另一份数据库。runtime_dir 指后台与采集依赖目录；WebView2Loader.dll 仍随桌面 exe 放置。

`build-debug-desktop.ps1` 编译桌面并调用 `prepare-debug-runtime.ps1` 补齐依赖，保留用户配置。仅 11 个固定的 EXE/DLL 文件使用普通运行所需的 Medium 完整性标签，不递归改目录、数据库或 IPC 权限。桌面编译输出先与 Cargo 硬链接分离，防止编译缓存被一同标记而导致后续链接失败。构建时先关闭此 debug 界面，后台可继续运行。

普通目录的预览快捷方式同样读取本地配置，不再用快捷方式参数锁死数据目录。旧包保留。

## 配置页超时修复

实机复现同一后台 PID 25060：retention 10,017 ms 后超时，privacy_settings 14 ms、monitoring_settings 18 ms 成功。此前前端用 Promise.all 等待历史存储和隐私，任何一个失败都会挡住整个配置页，错误文案又笼统称为监控设置失败。

现各分类独立、按需读取。常规和路径配置不依赖后台，读取中切换标签不重复发起请求，错误只在对应分类显示并提供手动重试；不伪造默认配置，不因超时触发提权或后台重复启动。retention 后端改走独立 WAL 只读连接，避免等待采集写入的全局锁。写入设置的流程仍使用原来的写锁。

## 验证

- 桌面原生启动/配置 12 项测试通过：严格路径解析、旧配置兼容、保存原子替换、拒绝无效依赖及保留原文件。
- 服务 read_api_tests 6 项通过，包括持有数据库写锁时读取真实已提交设置、历史存储，以及隐私显示、优雅退出与替换。
- `check-settings-isolation.cjs`：后台慢请求期间常规、权限、隐私可用，无重复请求，超时局部提示与手动重试通过。
- 路径离线浏览器验收、真实 debug WebView 原生调用验收通过，证据在 `target/debug-path-settings-ui-qa.json`、`target/debug-direct-live-qa.json`。界面保存相同路径并验证错误路径不改变配置；测试阶段原增强后台保持运行。
- query-consumers、product-status-copy、front-refresh 检查通过。

现阶段仍为开发构建。正式安装、发布签名等门槛未由此次修复替代。后台只读修复需要新后台进程运行才生效；仅更新磁盘文件不会更换正在运行的旧版本。
## 最终实机结果

10:08 发起同账户 UAC 正常替换；旧后台 PID 25060 完成退出，新增强后台 PID 25556 接管原 `target/desktop-preview-data`，没有强制结束进程。10:11 在实际 debug WebView 中验证全部设置分类能打开：历史存储 23 ms、隐私设置 16 ms、采集权限 21 ms。先前历史存储 10 秒超时已不再复现。本次是单机现场读数，不代表长期性能上限。

最终界面再以无参数、无路径环境变量、无调试端口的方式直接打开，见 `target/debug-direct-final-launch.json`。普通预览目录更新为 `%LOCALAPPDATA%/Programs/PCScopePreview/915CDFF7AEFB412B`，旧版本保留。
