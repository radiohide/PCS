# WMI 日志的易读展示

本次针对 `Microsoft-Windows-WMI-Activity/Operational` 中、提供程序为 `Microsoft-Windows-WMI-Activity` 的 5857 和 5858。历史记录立即使用新展示，无须重新采集。

## 5857：启动系统管理组件

实际来源字段示例：

```json
{"Operation_StartedOperational/ProviderName":"ScheduledTaskProv","Operation_StartedOperational/HostProcess":"wmiprvse.exe","Operation_StartedOperational/ProcessID":"25","Operation_StartedOperational/Code":"0x0","Operation_StartedOperational/ProviderPath":"%SYSTEMROOT%\\System32\\wbem\\schedprov.dll"}
```

显示为“启动系统管理组件 · 计划任务管理组件”，结果“成功”，显示宿主程序和宿主 PID。这里的启动对象是管理组件，不是某个计划任务。组件名称、DLL 路径放在默认折叠的“WMI 接口与原始操作”中。

## 5858：客户端调用

兼容平铺字段和 `Operation_ClientFailure/` 前缀字段。`ClientProcessId` 是发起进程，`User` 是操作账户，`ClientMachine` 是发起计算机；日志写入 PID 不用作调用者。

| 原始 Operation 示例（前缀省略） | 标题 | 行为性质 |
|---|---|---|
| `IWbemServices::CreateInstanceEnum - ROOT\StandardCimv2 : MSFT_Printer` | 查询对象列表 · 打印机 | 读取信息 |
| `IWbemServices::CreateInstanceEnum - ROOT\StandardCimv2 : MSFT_PrinterPort` | 查询对象列表 · 打印端口 | 读取信息 |
| `IWbemServices::ExecQuery - ROOT\CIMV2 : SELECT * FROM Win32_Service` | 查询对象列表 · Windows 服务 | 读取信息 |
| `IWbemServices::GetObject - ROOT\CIMV2 : Win32_Process.Handle="42"` | 读取对象信息 · 进程 | 读取信息 |
| `IWbemServices::ExecMethod - ROOT\CIMV2 : Win32_Process.Handle="42"::Terminate` | 调用管理方法 · 进程 | 执行管理方法 |
| `IWbemServices::ExecNotificationQuery - ROOT\CIMV2 : SELECT * FROM __InstanceCreationEvent` | 订阅对象变化 · __InstanceCreationEvent | 监听变化 |

未知类保留原类名；不能解析的操作显示“WMI 管理调用”，对象缺失和隐私隐藏各自明示。仅解析已识别接口的直接对象或简单 SELECT 查询主体，不从正文中搜到一个类名就认定操作目标。异步接口沿用对应同步接口的行为解释。

例如查询打印机返回 `0x80041001`：显示“失败；调用失败，来源未给出具体原因”。查询服务返回 `0x80041032`：显示“已取消；调用已取消”。不把读取操作写成配置变化，不从一般失败码推导设备故障。

结果码支持十六进制、无符号十进制和有符号 32 位表示。缺失、隐藏、无效或未知码保留明确状态和原码。已解释调用失败、对象找不到、权限不足、命名空间找不到、对象类无效、查询无效、调用取消。完整接口、命名空间和查询语句放入折叠详情；原始数据页保留完整事件。

## 来源和验证

- [微软 IWbemServices 接口定义](https://learn.microsoft.com/en-us/windows/win32/api/wbemcli/nn-wbemcli-iwbemservices)
- [微软 WMI 返回码定义](https://learn.microsoft.com/en-us/windows/win32/wmisdk/wmi-error-constants)
- [微软对 5858 / 0x80041032 的解释](https://learn.microsoft.com/en-us/troubleshoot/windows-client/system-management-components/wmi-activity-event-5858-logged-with-resultcode-0x80041032)

本次只读历史检查包含 1253 条组件启动和 212 条调用失败记录，抽取每类最近两条验证打印机组件、计划任务组件、打印机查询和打印端口查询。自动测试覆盖来源隔离、字段前缀、接口解析、错误码、隐私状态、未知对象、宿主与发起者区分和原始记录不变。这不是对所有 Windows WMI 事件的穷尽解释。
