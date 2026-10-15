# PCScope 自身活动识别

> 2026-10-12 防风暴补充：日志及能力检查默认原生；固定时钟调度、每通道退避、手动冷却、有限工作量和后台数据目录防自采集共同约束负载。自身活动筛选不控制采集触发，不能单靠隐藏记录防止风暴。详见 [补充验收](refactor/acceptance/N04-native-safety.md)。

时间线的“自身活动”筛选提供“全部显示”（默认）、“隐藏已确认自身活动”和“仅自身活动”。筛选在数据库分页前执行，不删除记录、不改变采集周期，也不控制后台采集。详情里的“自身活动归属”提供所属组件、运行编号、已验证进程 ID、建立时间、完整路径和判定依据。原始数据仍保留原始事件，归属信息单独放在顶层 `activity`。

## 进程身份从哪里来

- 后台通过 `OpenProcess`、`GetProcessTimes`、`QueryFullProcessImageNameW` 验证 PID、建立时间、退出时间和完整程序路径。
- 采集器由后台实际启动，使用 Windows Job 归属识别；运行期间每 200 毫秒枚举 Job 内进程，区分采集器根进程与子进程。程序结束时补记根进程退出时间。
- 后台自身每 10 秒补记存活状态。桌面每 10 秒注册自身 PID，后台读取真实进程身份，并验证路径为同一安装目录的 `pcscope-desktop.exe`。注册独立于前端自动刷新开关。
- 归属元数据持久化到 `own_processes`；事件身份摘要放入 `activity_evidence`，确认的标记放入 `self_activity`。晚到的日志和晚到的身份观测都可重新匹配。

## 匹配规则和实例

| 日志证据 | 判定方式 | 界面例子 |
|---|---|---|
| Security 4798/4799 的 `CallerProcessId`、`CallerProcessName` | PID、完整路径、事件时间与已验证的进程存活区间匹配 | PCScope 采集器：枚举用户组；详情显示所属采集组件 |
| Security 5058 等的 `ClientProcessId`、`ClientCreationTime` | PID、建立时间（毫秒换算容差 1 毫秒）、存活区间匹配 | PCScope 采集子进程：删除密钥文件；保留密钥名称、路径、操作结果 |
| Security 4688 的 `NewProcessId/NewProcessName`，或 Sysmon 的 `ProcessId/Image` | 新进程自身身份匹配已验证进程 | PCScope 采集器：进程启动 |
| Security 4688 的创建者 `ProcessId/ParentProcessName`，或 Sysmon 1 的 `ParentProcessId/ParentImage` | 创建者完整身份匹配 | PCScope 启动的进程；仅确认本次启动动作，不据此推断子进程全部后续行为 |
| PowerShell 4103/4104 的日志写入 PID | 仅限指定 PowerShell 提供程序和事件 ID，PID 落在已验证 PowerShell 宿主的存活区间内 | PCScope PowerShell 宿主日志；依据为 `verified_powershell_host` |
| Security 4662 只有账户、对象，没有调用者 PID | 不确认归属 | 无自身活动标签，隐藏自身活动时仍显示 |
| WMI 5858 只有客户端 PID、没有可匹配的完整身份 | 当前不确认归属 | 仍显示原事件及客户端字段 |

示例：PCScope 启动 PID 42，Windows 返回建立时间 10:00:00、完整路径 `C:\PCScope\collector.exe`，10:00:10 再次确认存活。10:00:05 的 4798 记录包含相同 PID 和完整路径，可确认自身活动。相同 PID 出现在 09:59:00，或者路径为 `C:\Other\collector.exe`，都不会被该身份匹配。Security 日志的写入进程即使恰好是 PID 42，也不能代替调用者字段。

## 保留和边界

- 不按账户名、文件名包含 PCScope、程序简称或采集时间相近来隐藏事件。
- 启用前的历史没有可信的进程观测，不回填猜测标记。未匹配不表示与 PCScope 无关。
- 极短命子进程可能落在枚举间隔内；未记录真实退出时间的最后一小段活动、WebView 辅助进程、间接触发的系统服务行为，以及只包含不充分身份的事件可能无法确认。它们仍保留在默认和隐藏视图中。
- PowerShell 的宿主归属与 Security 的调用者归属分别解释，不能把所有提供程序的写入 PID 当作操作进程。
- 原有滚动保留策略删除记录时，外键级联删除对应事件摘要和标记，并清理早于剩余保留单元的旧进程查找元数据；标记自身保存的依据不依赖该查找表。无限扩张模式不执行这项历史清理。
- 修改筛选不改变原始采集数据和已配置的 5 GB/无限保留策略。元数据仍占用同一数据库空间。

## 验证

Rust 测试覆盖不同路径、错误提供程序、PID 重用时间区间、建立时间不匹配、Security 写入 PID 排除、PowerShell 宿主专门规则、延迟身份匹配、分页前过滤和删除级联。Windows 原生测试验证真实 Job 中根进程和子进程身份。前端测试覆盖三种筛选值、清空恢复默认和详情原始字段保留。
