# 事件日志展示与筛选

事件日志插件（ID `eventlog`）包含 23 个来源分组、76 个采集项。插件详情、时间线标题和采集项筛选均使用同一份目录。所有自动事件日志采集项统一属于“事件日志”分类，包括用户模式驱动日志。

| 层级 | 示例 |
| --- | --- |
| 插件／分类 | 事件日志 |
| 来源分组 | Windows 安全审计 |
| 采集项 | Windows 安全审计 / 进程生命周期 |
| 事件动作 | 进程启动 |
| 原始证据 | Security；Microsoft-Windows-Security-Auditing；事件 ID 4688 |

时间线可按分类、来源分组、采集项、记录类型和事件动作组合过滤。事件动作按所选来源和采集项下的保留历史提供。采集项列表包含尚无记录的已定义事件项，选择后明确显示无匹配记录。采集周期、启停和手动触发仍由事件日志插件统一控制；细分目录不额外执行重复采集。

## 细分范围

- Windows 安全审计：登录登出与会话、账户管理、用户组成员变更、特殊权限分配、进程生命周期、文件访问与权限、注册表操作、服务安装、计划任务变更、审计策略变更、网络连接与共享访问、审计日志清除、来源采集状态、其他事件。
- Sysmon：进程生命周期、进程访问与异常活动、网络连接、DNS 查询、文件操作、注册表操作、模块与驱动加载、WMI 持久订阅、命名管道、原始磁盘读取、剪贴板活动、Sysmon 服务与配置、来源采集状态、其他事件。
- System：服务运行、系统启动关闭、电源与异常重启、来源采集状态、其他事件。
- Application：软件安装、应用错误、无响应、来源采集状态、其他事件。
- 其余通道：各自的运行事件和来源采集状态。

具体匹配须同时符合来源、提供程序及已定义事件编号；不同提供程序的同号事件不会误归类。Sysmon 规则依据 [Microsoft Sysmon 事件说明](https://learn.microsoft.com/en-us/sysinternals/downloads/sysmon)。没有命中明确规则的事件进入该来源的兜底项，不丢弃原始数据，不猜测动作。目录中的采集能力不表示本机已安装 Sysmon、启用全部审计或实际产生了相应事件。

## 实现

`event-items.js` 定义分组、采集项、匹配规则和动作名称。`plugin-sources.js` 展示采集接口与规则，`build-collection-catalog.cjs` 生成前后端共享目录。Rust 根据目录重建 `record_search` 分类与动作索引，旧历史也适用；原始 `records.kind` 和证据内容不改写。新增的 `action` 是展示与过滤使用的归一化动作。

目录更新后第一次启动在事务中重建索引。默认查询及采集项、动作过滤继续使用索引。已有缺口去重逻辑保持，来源缺口和恢复进入“来源采集状态”；真实审计日志清除事件进入“审计日志清除”。手动导入仍保留独立入口。

## 2026-09-11 历史兜底事件审查

对审查时保留的历史按来源、提供程序、事件编号分组，共 94 种组合，其中 87 种、25,284 条记录此前进入兜底项。新增 103 条有独立解释的提供程序限定规则，覆盖上述全部兜底组合，并扩展尚未出现的密码组件、DPAPI 操作。此统计是本次保留历史的覆盖情况，不代表覆盖 Windows 所有可能事件。

新增细分包括加密密钥操作、密码组件自检、密码提供程序配置、DPAPI、凭据读取、用户组查询、WMI 对象审计、进程令牌、组策略生命周期及处理阶段、任务执行、PowerShell、AppX、Defender、更新、VSS 和授权维护。完整目录由 `event-semantics.js` 与 `event-items.js` 共同生成。

5058、5059、5061 的易读说明显示具体操作、密钥名称、返回结果、账户、客户端进程及建立时间、算法、存储提供程序和文件路径。资源代码只映射经来源正文核实的值；未知值明确标记未解析并保留原码。不能从密钥名推断程序身份。客户端 PID 与日志写入 PID 分别说明；缺失或遮罩返回码不推断成功。5379、5382 的非零返回码显示失败，避免把事件模板中的“读取”误认作读取成功。任务事件 114 表示错过计划后补启动。

验证脚本 `scripts/check-event-semantics.cjs` 检查提供程序隔离、规则唯一性、目录与界面一致、密钥操作、失败返回码、隐私遮罩。可选参数接受 `event-audit` 只读工具生成的审查 JSON，验证保留历史的兜底分类覆盖。实际审查样本含本机元数据，仅保存在忽略的 target 目录，不提交。

依据：Microsoft [5058](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/auditing/event-5058)、[5059](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/auditing/event-5059)、[5061](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/auditing/event-5061)、[审计事件目录](https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/plan/appendix-l--events-to-monitor)，以及本机提供程序消息资源和已保留的来源正文。
