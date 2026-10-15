# 原生设备通知与状态复查

2026-10-14。服务新增 CM_Register_Notification，按全部设备实例注册 PnP 通知；回调只设置一个原子待复查标志，不读取事件材料、查库、启动程序或执行采集。过滤器依据 [CM_NOTIFY_FILTER](https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/ns-cfgmgr32-cm_notify_filter)。注册发生在调度器第一次枚举前，避免基线期间的通知丢失。

调度器每秒取标志，静默两秒合并突发变化，持续事件十秒触发一次待复查。PnP 只请求既有基础与存储状态单元，沿用唯一状态采集通道；每个任务仅有一个待复查位，采集期间的通知保留至后续复查。成功后至少三十秒冷却，失败沿用既有退避；关闭的单元不被通知开启，日志采集不被设备通知触发。下一次采集时间显示使用同一调度逻辑。

网络另注册 NotifyIpInterfaceChange、NotifyUnicastIpAddressChange、NotifyRouteChange2，AF_UNSPEC 覆盖 IPv4/IPv6，首次确认通知关闭且回调显式忽略 MibInitialNotification。回调不借用返回行，仅标记待复查；网络单独合并后请求基础与网络状态单元，和 PnP 共用调度任务去重及冷却。接口依据 [NotifyIpInterfaceChange](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-notifyipinterfacechange)，注销依据 [CancelMibChangeNotify2](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-cancelmibchangenotify2)。三个注册任一失败时清理已注册句柄，保留周期网络枚举；不因通知错误判断网卡不存在。

退出时先注销通知，确认回调完成后释放上下文。若系统注销失败，保留回调上下文，避免悬空访问。注册不可用时一次性记录诊断，既有周期枚举继续运行，不高速重复注册。

Windows 普通应用环境实测 PnP 和三个网络通知注册、注销成功；测试只注册/注销，不创建、禁用或删除设备，不修改 IP/路由。合并测试覆盖突发与连续事件；调度测试覆盖一万次请求、执行期间保留、冷却、失败退避、关闭任务与日志隔离。网络回调构造测试覆盖一万次地址/路由通知只保留一个标志，以及初始确认不当作变化。服务 27 项测试全部通过，包括投影恢复、失败重试和读请求回归。

回归中发现旧投影测试的任务记录使用非法裸路径，已替换为真实采集的完整任务 ID、根文件夹与名称；生产身份校验未放宽。

仍需：真实设备创建/禁用/断开/删除、地址与路由变更及快速变化的完整入库、时间线与对象详情验证；故障及长期性能检查；最终部署。未操作用户的 Meta Tunnel、现有设备或历史库，也未更新正式后台。通知是触发复查的依据，确认对象移除仍必须依赖成功完整枚举；不能从通知虚构操作者或精确历史删除时间。
