# 启动检测、权限与历史追溯

> 2026-10-12 更新：能力检测和 UAC 启动均改为固定原生程序/API，重复检测有 30 秒冷却；默认只读检测不修改审计。用户后台与未来机器服务的边界、管理员只读实测及循环防护见 [补充验收](refactor/acceptance/N04-native-safety.md)。下文此前桌面实测不能替代新版原生启动入口的 GUI 验收。

后台每次启动检查实际令牌权限、三项审计策略和事件采集器清单中的 23 个日志通道。首次连接提示用户选择增强采集或低权限运行。选择保存在当前历史数据库，系统配置页可以随时修改。保存模式和实际后台权限分别显示，取消 UAC 不会伪装成提权成功。

增强模式只提升后台，桌面保持普通权限。Windows UAC 需要同一 Windows 账户确认；不允许通过另一个管理员账户访问原账户的 DPAPI 历史。切换先等待原后台结束当前采集并正常退出，再占用同一用户管道、打开原数据库。当前版本仍是用户后台，未安装 Windows 系统服务。

可选“自动启用必要审计”会启用以下成功审计和通道，并重新读取确认：

| 设置 | 用途 | 查询 / 配置接口 |
| --- | --- | --- |
| Other Object Access Events | 4698–4702，计划任务操作及账户 | AuditQuerySystemPolicy / auditpol |
| Process Creation | 4688，进程路径及父进程证据 | AuditQuerySystemPolicy / auditpol |
| Logon | 4624，登录会话关联 | AuditQuerySystemPolicy / auditpol |
| TaskScheduler/Operational | 计划任务调度、运行记录 | EventLogConfiguration.SaveChanges |

原有失败审计、日志容量和覆盖策略不变。切回低权限不会关闭系统已经开启的审计。策略可能受组策略控制；配置失败或检测未知都单独展示。低权限不能执行自动审计配置。默认的 5 GB PCScope 存储配额与 Windows 日志容量互相独立。

配置页展示通道是否启用、是否实际可读、容量、覆盖策略、最早和最新现存记录、记录数量以及补采状态。检测使用 EventLogConfiguration、GetLogInformation 和正反向 EventLogReader，不能只凭管理员身份判断数据完整。

System、Application、Security 和 TaskScheduler/Operational 每轮优先读取，其他通道轮换。每通道每轮至多三批，从已提交游标继续，避免一个日志长期独占采集。无游标时从现存最早记录开始。仅在提交批次后前移游标；缺口、覆盖和清空继续由事件采集器检测。禁用插件后不会自动重新启用，可通过“重新检测 / 补采”手动读取一轮。

“已追到日志末尾”表示该次查询追上现存记录，不表示历史完整。无法恢复未记录、清空、覆盖、旧版本丢弃的证据；未自动扫描磁盘上的离线 EVTX 归档，归档仍使用既有 import-evtx 导入能力。

计划任务快照新增前次观察时间。详情关联同一完整任务路径、两次观察之间的 Security 和 TaskScheduler 事件，展示账户、SID、登录会话、客户端/父进程 ID、原始字段及来源说明。旧记录仅在有保留快照时推导区间；缺少区间明确返回未知。结果为关联证据，不将时间接近或账户字段直接当成某个字段变化的确定原因；最多显示 100 条并标明截断。进程 ID 不能单独证明具体进程身份或人类操作者。

验证：工作区 Rust 回归测试、桌面参数引号测试、monitoring UI 渲染测试、刷新与快照差异回归、PowerShell 语法与普通权限实机检查。实机已验证用户确认 UAC 后后台切换、审计策略启用并复查、Security 历史补采和 TaskScheduler 日志追到末尾。UAC 取消及从管理员切回低权限尚未进行实机验收。

参考：[4702](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/auditing/event-4702)、[AuditQuerySystemPolicy](https://learn.microsoft.com/en-us/windows/win32/api/ntsecapi/nf-ntsecapi-auditquerysystempolicy)、[auditpol set](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/auditpol-set)。
