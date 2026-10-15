# 后端采集范围与原生迁移方案

日期：2026-10-15。范围是当前 `pcscope` 的全部 133 个来源，不以界面选项数量代替来源数量。全量清点之后已累计接入 87 项迁移；最新隔离开发包为 `dist/native-five-migrated`，尚未替换正式后台。目标保持全部可直接读取的采集迁入后端。

## 结论与实际进度

131 个实际来源中，127 个已进入编译后端路由，仅用户明确暂缓的四个 Hyper-V 来源仍保留兼容路由；另有 2 个仅用于分组。初始梳理基线为 40 / 91 / 2。23 个 Windows 日志通道已强制原生读取。逐项决策覆盖 133/133，未分类为 0。完整来源、当前实现、目标接口和边界见[逐项清单](refactor/native-collection-inventory.md)，最近五项的授权、真实数据与字段核对见[五项增强验收](refactor/acceptance/native-five-enhanced-migration.md)，此前各批次证据保留在 acceptance 目录。

统一要求：能由后端直接读取的全部迁入后端。这里的原生采集指 Rust 或编译后的 C# 宿主直接调用 Windows、系统提供程序或产品接口，不启动或加载 PowerShell。WMI、CIM、COM 本身不依赖 PowerShell；[WMI 官方接口](https://learn.microsoft.com/en-us/windows/win32/wmisdk/creating-a-wmi-application-using-c-)支持应用直接访问。

“原生还是日志”不是二选一：原生描述执行方式，日志描述数据来源。状态用接口读，历史动作读事件，两者都在编译后端运行。目前没有发现必须使用 PowerShell 的来源；这不等于所有替代接口已经完成字段与环境验收。

### 尚未切换的全部 4 项

| 采集项 | 原生实现 | 切换前尚缺什么 |
|---|---|---|
| Hyper-V 虚拟机 | 直接 Hyper-V WMI V2 | 实际虚拟机环境、字段与关联验收 |
| Hyper-V 虚拟交换机 | 直接 Hyper-V WMI V2 | 实际交换机、组合网卡及版本验收 |
| Hyper-V 磁盘连接 | 直接 Hyper-V WMI V2 | 实际虚拟/直通磁盘关联和枚举顺序验收 |
| Hyper-V 检查点 | 直接 Hyper-V WMI V2 | 实际检查点及父子关联验收 |

这四项已有编译原型，仍明确保留兼容归属。用户已确认没有 Hyper-V 测试环境并要求暂缓。本轮授权五项只读 UAC 验收已执行：可选组件、安全启动、BitLocker、TPM 直接成功；预配应用修正为运行时选择公开 WinRT 后成功读取八个包。五项均通过无兼容脚本的实际编译路由和隔离对象查询，不再列为待迁移。

## 哪些直接原生读取

| 内容 | 后端读取方法 | 对用户提供什么 |
|---|---|---|
| 网卡、设备、地址、路由、连接 | SetupAPI、配置管理器、IP Helper、直接 CIM | 哪些设备仍在、连接状态及前后变化 |
| 进程、服务、驱动服务、计划任务 | 进程/服务 API、Task Scheduler COM | 当前运行情况、配置和最近结果 |
| 软件、扩展、启动项、系统配置 | 注册表、文件解析、Shell/WinRT/DISM | 安装和配置现状及变化 |
| 账户、组、权限、会话 | NetAPI、LSA、WTS、直接 WMI | 当前账户、成员关系、权限和会话 |
| 磁盘、卷、固件、电池、电源、打印 | 存储提供程序、卷 API、WMI、PowrProf、Winspool | 硬件状态、当前电源计划和打印队列 |
| DNS、防火墙、VPN、SMB、代理 | 直接 CIM、RAS、注册表及网络 API | 当前网络配置、规则和会话 |
| 证书、密钥、防护、加密、TPM、安全启动 | 证书/CNG、Defender、BitLocker、TPM/UEFI 接口 | 当前保护配置和运行状态 |
| SIHost 及其他选定程序的模块 | 进程模块枚举、文件监听、摘要和签名 | 已加载模块及其文件后续变化 |
| Hyper-V、WSL、Docker、Mihomo | Hyper-V WMI、WSL 登记、Docker 本机管道、Mihomo 本机 HTTP | 已安装能力、对象及允许读取的配置 |

例如[网卡接口表](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getiftable2)需要结合设备登记及 CIM 字段，不能仅更换成字段较少的 API。任务通过[Task Scheduler COM](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-interfaces)读取。Docker 使用[Engine API](https://docs.docker.com/reference/api/engine/)，不依赖命令行启动 Docker。

## 哪些历史信息必须有事件记录

以下内容无法由一次当前状态查询还原。需要 Windows 日志、产品保留历史，或者事先运行的事件跟踪；并非都只能来自 EVTX 文件。

| 要回答的问题 | 所需来源 | 当前状态能提供的补充 |
|---|---|---|
| 谁在何时登录、失败或修改了受审计设置 | Security 等审计事件，取决于系统配置 | 当前账户与会话 |
| 任务每次何时启动、结束、返回什么结果 | Task Scheduler 事件 | 当前任务配置、最近一次结果 |
| 短命进程、服务曾经怎样启动或停止 | 对应日志或事先启用的 ETW/通知 | 当前进程和服务状态 |
| 程序为何崩溃、卡死或加载被阻止 | Application、WER、Code Integrity 等相应事件 | 进程是否存在、已知文件是否变化 |
| 更新、安装、防护处置过去是否成功 | 安装/更新/防护事件及 WUA、Defender 保留历史 | 当前版本、已安装项目、防护状态 |
| 文件曾经发生哪些文件系统变更 | 可用的 NTFS USN 日志或事先运行的监听 | 当前文件属性、摘要、签名 |

Windows 日志继续原生读取并使用书签控制补读；[EvtSubscribe](https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtsubscribe)可用于后续订阅。[ETW](https://learn.microsoft.com/en-us/windows/win32/etw/about-event-tracing)只能收集运行期间相应提供程序产生的事件。[USN](https://learn.microsoft.com/en-us/windows/win32/fileio/change-journals)不提供完整文件内容和操作者。

网卡删除不应以存在某条日志为前提：保留成功基线，原生通知触发重新枚举，以范围完整的后续结果识别变化；准确区分设备不在场、禁用和登记移除。只有成功的完整枚举才能确认相应范围的缺席。仅靠前后快照时记录观察时间区间，不编造精确删除时刻或操作者。

服务已接入原生 PnP 和网络通知至现有状态调度器，注册先于首次枚举，回调只标记复查；合并、冷却、退避及执行期间保留请求验证通过，真实注册/注销与服务 27 项回归通过。保存的设备基线和实际原生枚举在隔离包中已验证缺席变化能进入对象和时间线；真实设备操作触发链路与正式部署仍待验证，见[通知验收范围](refactor/acceptance/native-device-notifications.md)和[无日志缺席链路验收](refactor/acceptance/native-snapshot-absence.md)。

## 迁移顺序与验收

| 顺序 | 待迁移来源数 | 内容 |
|---|---:|---|
| P0 | 0（初始 11） | 11 项已进入原生代码；设备通知、快速变化及完整应用链路验收继续开放 |
| P1 | 0（初始 64） | 全部已接入原生 |
| P2 | 4（初始 16） | 仅剩用户暂缓的四个 Hyper-V 来源 |

每项先保留原身份、字段、范围、分片和隐私处理，再用隔离数据比对新旧输出及失败语义，验证超时和资源消耗，通过后切换 ownership；切换后失败不得自动回退 PowerShell。旧脚本在最后一个使用者迁走后再删除。不得以默认空值、缩减范围或缺字段来完成迁移。

本次 Wintun 漏记列为 P0 回归：移除日志记录 2026-10-14 15:13:51 操作成功，但设备最后成功观察为 15:00:55。隔离诊断显示兼容脚本因未签名被执行策略拒绝，后台没有得到后续快照。保存的证据位于 `target/wintun-deletion-evidence.json` 和 `target/wintun-collector-diagnostic.json`。

验收先回放已保存材料，检查设备 ID、最后状态和缺席变化；再在隔离测试环境验证虚拟设备创建、禁用、断开、移除及快速重复变化。不操作用户现有 Meta Tunnel。通过完整原生链路产生变化后，确认时间线、对象列表与对象详情一致。此次历史删除时间不会仅凭恢复采集自动补造为精确事实。

Hyper-V 磁盘连接原型已编译，虚拟/直通字段及引用契约通过；已修正 IDE/SCSI 分别按返回顺序编号；真实枚举顺序和资源关联仍待验收，不计为完成，见[磁盘原型记录](refactor/acceptance/native-hyperv-disk-prototype.md)。

Hyper-V 检查点原型已编译，六字段身份/UTC 时间/父 ID 与七种类型契约通过，成功关联和版本矩阵仍开放，见[检查点原型记录](refactor/acceptance/native-hyperv-snapshot-prototype.md)。

Hyper-V 虚拟机原型已编译并验证状态与内存转换，完整字段序列化/成功关联尚待验收，见[VM 原型记录](refactor/acceptance/native-hyperv-vm-prototype.md)。

Hyper-V 字段审计与交换机只读原型已经完成初步契约，实际提供程序在本机不可用，成功路径及其它三项实现仍开放，不计为完成，见[Hyper-V 原型记录](refactor/acceptance/native-hyperv-switch-prototype.md)。

TPM 九字段已增强成功读取并进入原生归属，只调用元数据和固定无认证能力查询，避免原构造器的授权读取、自检和配置改变。TPM 1.2 真实厂商能力和其它硬件矩阵继续开放，见[五项验收](refactor/acceptance/native-five-enhanced-migration.md)及[TPM 历史记录](refactor/acceptance/native-tpm-prototype.md)。

预配应用在有公开 WinRT 接口时读取完整五字段，旧系统保留私有 DISM 路径。本机 DISM 和旧参考均被提供程序拒绝，公开接口成功读取八个包并完成身份、字段与版本一致性校验，已进入原生归属；旧 LTSC 实际环境仍需验证。

BitLocker 已增强成功读取一个卷的九项元数据并进入原生归属，与不读取恢复材料的安全参考一致；其它卷状态和保护器矩阵仍需验证。

可选功能 134 项及安全启动一项的增强读取与原参考字段一致，普通权限失败不提交空快照，已进入原生归属。

## 运行环境与尚待解决的接口边界

适用矩阵至少覆盖 Windows 10 LTSC 的实际版本和 Windows 11、普通/增强权限、当前用户/机器范围、相应组件存在/不存在。Codex 验收与独立运行使用同一编译程序和明确路径配置；不依赖 Codex 的环境变量或放宽脚本执行策略。

- 普通权限先采可读范围；用户选择增强模式后在启动/切换边界申请权限，不在每轮或每项反复弹 UAC。管理员权限不能修复缺少组件、版本不支持、文件不存在或提供程序错误。
- 预配应用按 WinRT 方法实际可用性选路；公开方法不存在时使用私有 DISM，不缩减旧系统范围。当前成功不等于旧 LTSC 的私有接口版本验证完成，详见[五项验收](refactor/acceptance/native-five-enhanced-migration.md)。
- WUA 历史在普通应用完整性级别已读取 136 条并完成身份/字段比对，已进入正式原生归属；其他环境和最终部署继续验收。
- TPM 复合字段、防护提供程序的版本字段、Hyper-V 派生字段必须逐项验证，不能凭基础接口可调用就宣布等价。
- TCP/UDP 连接创建时间、隐藏网卡、持久路由、[防火墙策略来源及过滤器](https://learn.microsoft.com/en-us/windows/win32/fwp/wmi/wfascimprov/msft-netfirewallrule)不得在迁移时丢失。
- `Get-SmbSession` 读取本机 SMB 服务端的客户端会话，当前“SMB 客户端会话”命名容易误导；迁移时同步校正展示。
- CNG 已原生，但 3 个提供程序/范围问题仍待分别处理；“原生”不等于所有提供程序都可用。
- SIHost 延续“实际加载模块 → 已知模块文件后续变化”；未知模块第一次加载就失败，不能仅靠已加载列表提前发现。相关错误事件作为补充进入现有体系。

## 性能与防循环约束

使用唯一后台调度器；每来源最多一个执行实例，采集器内部不再另起循环。迁移完成前的兼容脚本也仅执行单次有界采集。事件回调只入队，按来源/对象合并和去抖；耗时枚举在工作队列执行，队列设上限并周期性对账。[设备通知 API](https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_register_notification)要求及时返回；先注册通知再枚举，并对重叠结果去重。

按来源设置超时、输出上限、并发预算、失败退避与冷却；不可取消的提供程序查询放入可终止的隔离宿主。记录采集耗时、CPU、内存、句柄、输出量和事件积压，常态与事件风暴分别验收。不可用来源不持续高速重试。

十个已归属原生的状态单元已完成一次普通权限隔离检查，全部正常退出；本次耗时含测试输出解析，观察到的内存峰值为 23.6–55.8 MiB。权限失败、产品非空环境、独立事件/USN 单元及事件风暴仍开放，不能据此宣布全量性能验收完成。见[结果与计量限制](refactor/acceptance/native-owned-performance.md)。

排除自身数据、缓存、日志和诊断输出目录；文件事件只触发已监控对象复核，不能因写入自身历史再次触发采集。模块共享目录监听、文件稳定后复查，按文件身份去重；预算溢出触发有界对账，不能无限启动子进程。

界面继续优先显示当前状态、系统影响和可执行处理；原始接口名及错误码放详情。不恢复泛化的“过去可能遗漏”提示。真实当前失败仍显示，恢复后依据成功采集结果清除。

## 本次校验范围

`check-source-inventory.cjs` 核对源码来源；`build-native-migration.cjs --check` 同时核对机器清单和逐项文档，新增来源未决策、旧决策失效或生成文件落后均报错。已构建并通过五项实际编译包装器和隔离对象查询；权限拒绝保留状态，不当作成功采集。正式后台与历史未替换。四项 Hyper-V 迁移及上述环境、性能矩阵继续开放。
