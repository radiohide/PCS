# Windows Event Log EXE 插件

`pcscope-eventlog-native.exe` 是独立 C#/.NET Framework 程序，通过
`System.Diagnostics.Eventing.Reader` 调用 Windows Event Log API。运行时不启动
PowerShell、不加载 PowerShell 引擎。Windows 自带 .NET Framework 4.x 即可运行。
原有 `pcscope-eventlog.exe` 包装器、`collect-eventlog.ps1` 和分类脚本保留。

## 构建及切换

运行 `plugins/eventlog-native/build.ps1`，产物位于 `target/debug`。
`scripts/stage-dev.ps1` 会构建并复制新插件，同时保留原脚本。
重新构建的 `pcscope-eventlog.exe` 根据同目录的 `eventlog-backend.json` 选择后端：

```json
{"backend":"native"}
```

2026-10-12 更新：默认已改为原生后端；移除配置文件仍使用原生。只有显式配置
`{"backend":"powershell"}` 才使用兼容插件。原生失败不自动回退。
配置在每次调用时读取，不需要重启后台；但是首次安装新版包装器应在停止采集后进行，
避免覆盖正在使用的 EXE。配置错误或原生 EXE 缺失会明确失败，不静默回退。
新插件依赖同目录 `channels.json`；PS 后端仍需原有三个 PS1 文件。

两种后端使用相同协议、来源名称和 record ID + XML SHA256 断点格式，因此正式切换
可以续用断点。A/B 对比工具不会连接后台或写入数据库，分别管理两种后端断点。
不能把两个并行采集器的输出同时写入同一来源。

## 固定日志对比

先使用 `wevtutil epl` 将目标通道导出为工作目录中的 `.evtx`，不清除来源日志。
然后以运行 PCScope 的同一 Windows 用户执行：

```text
python plugins/eventlog-native/compare.py target/debug target/debug/native-compare.evtx
```

比较目录必须没有启用 native 的 `eventlog-backend.json`，以确保原包装器走 PS 后端。
默认通道为 `Microsoft-Windows-PowerShell/Operational`；第三个参数可指定其他通道。
比较事件 ID、分类、原 XML 哈希、System 字段和载荷字段；还验证连续断点读取、
故意破坏断点后的重建以及原 XML/脚本字段加密。只打印计数和耗时，不输出日志正文。
两端各有 128 条/约 950 KB 的批次上限，加密的随机性和序列化大小可能影响批次边界。

## 隐私与限制

原 XML 使用相同 DPAPI CurrentUser 格式和熵加密，可由现有界面解密。
脚本内容和敏感命名字段同样加密；文本识别采取更保守的敏感关键词检测，可能比 PS
后端多加密少量普通文本，但不丢弃原值。文件观察范围继续按完整路径匹配。
无法解密的沙箱用户不能用于正常对比，插件会返回 gap 并保留原断点。

此修改减少了 PowerShell 自身审计噪声和进程启动成本，但没有调整后台轮换频率，
没有扩大 Windows 日志容量，也不会恢复已被覆盖的日志。仍需另行观察高吞吐通道是否追平。

2026-09-11 固定 PowerShell EVTX 的一次验证：首批同为 10 条，原生 0.336 秒、
PS 1.000 秒；续批同为 13 条，原生 0.361 秒、PS 1.162 秒。属于局部样本，非全量吞吐保证。
