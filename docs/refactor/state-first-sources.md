# 逐来源整改清单

本清单规定整改目标和验收门槛，**不是实现完成或实机验收通过证明**。正常状态直接表达，技术范围与缺口在本工程清单中说明；不把这些限制逐条堆到产品主界面。

[整改方案](state-first-remediation-plan.md) · [覆盖与缺口总表](state-first-coverage.md) · [采集项目](state-first-items.md) · [对象类型](state-first-types.md)

<a id="source-001"></a>

## 001 Chrome、Edge 与 Firefox 策略

来源：`pcscope.apps.browser-policies`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 读取 Chrome、Edge、Firefox 的机器/当前用户策略键及 32/64 位视图；白名单策略值保留原值，其余受保护。适配为每个注册表值一条 browser_policy 候选。

**整改后输出：** 按浏览器显示“密码保存被禁用”等有明确含义的策略、作用范围和最近读取时间；登记配置与浏览器当前是否运行分开。

**判断规则：** 仅按确切策略键、值名和有效值解码配置；缺失值表示未取得该设置，受保护值不反推。机器/用户冲突保留来源，不擅自宣布最终生效。完整快照差异表示配置变化，未证实时间不编造精确修改时刻。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前默认摘要没有策略值和作用范围的可读说明；扩展名单等数组不能统一解释为开关。
- 一源键展开多个值已在 adapter 实现，但 storage 特殊展开绑定目前仅用于组/权限，需补此路径的持久化多候选验收，防止同父记录串对象。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产仍走 PowerShell；原生注册表读取及有界深度/值数量、事件合并调度待迁移。

**验收：** 同键三个策略值、HKCU/HKLM 与两个视图均独立；禁用、未配置、受保护和读取失败展示不同，刷新不启动浏览器。

采集定义：`pcscope.apps.browser-policies`。

对象类型：`policy.browser_policy`。

依据：[plugins/powershell-adapter/domains/applications.ps1:1](../../plugins/powershell-adapter/domains/applications.ps1#L1)、[crates/contracts/src/adapters/policy.rs:206](../../crates/contracts/src/adapters/policy.rs#L206)、[crates/storage/src/projection.rs:151](../../crates/storage/src/projection.rs#L151)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-002"></a>

## 002 Chrome / Edge 扩展

来源：`pcscope.apps.chromium-extensions`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 现代码已读取扩展 manifest 和 Preferences/Secure Preferences，包含 recorded_state、disable_reasons、权限、注册路径及偏好读取状态；文件大小和重解析目录有约束。adapter 保留名称、版本、permissions 与 recorded_state。

**整改后输出：** 按用户、浏览器、配置文件展示扩展名称、版本和“登记启用/停用/状态未知”，能解释已记录的停用原因；不把磁盘清单写成此刻已加载。

**判断规则：** manifest 证明目录中的扩展信息；有效偏好条目的 recorded_state 才用于登记启停，缺失或读坏保持未知。来源更新可改变登记状态，不要求额外启动浏览器证明登记事实。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- disable_reasons、preferences_status、host_permissions 等已采字段未进入对象字段/摘要，无法解释停用或状态缺口。
- name 可能是 __MSG_ 占位，尚无受限本地化解析。
- adapter 用 installation_scope，加上 profile 的目录短名，需修复不同用户相同 Default/profile 的身份冲突边界。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 PowerShell 文件解析待迁移到有界原生文件读取；应保持现有 1 MiB manifest、16 MiB preferences 和不跟随重解析点限制。

**验收：** 两个用户各有 Default 和相同扩展时分开显示；偏好启用、停用、有停用原因、损坏及仅有 manifest 五类均不误称运行中。

采集定义：`pcscope.apps.chromium-extensions`。

对象类型：`shell.browser_extension`。

依据：[plugins/powershell-adapter/domains/applications.ps1:12](../../plugins/powershell-adapter/domains/applications.ps1#L12)、[crates/contracts/src/adapters/shell.rs:268](../../crates/contracts/src/adapters/shell.rs#L268)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-003"></a>

## 003 .NET 运行时登记

来源：`pcscope.apps.dotnet-runtimes`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP09。

**现状：** 生产归属已是原生注册表采集；读取 .NET sharedfx、SDK、sharedhost/hostfxr 和 .NET Framework 安装登记，adapter 识别版本值名及 Install/InstallSuccess，保留运行库家族、版本、安装位置和视图。

**整改后输出：** 显示“.NET 运行库/SDK、架构、版本、登记位置”及登记新增/移除，不写成某个程序正在使用。

**判断规则：** 仅受支持的路径结构、版本值和安装标志生成登记对象；按家族、版本、视图区分，安装位置不承担版本身份。完整有效快照可更新登记清单。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 家族仍含 x64/sharedfx 等内部片段，需按已知家族解释名称并保留原文详情。
- 一个注册表键可展开多个版本，需补持久化层多候选绑定/消失验收；现专项 expanded 分支仅组/权限，adapter 单元测试不足以证明落库不合并。

**验收：** 同键两个版本、SDK 与运行库、32/64 位同时存在时均独立；移除一个版本不覆盖另一个，缺失安装标志不虚构已安装。

采集定义：`pcscope.apps.dotnet-runtimes`。

对象类型：`software.runtime`。

依据：[plugins/platform-native/Sources.cs:157](../../plugins/platform-native/Sources.cs#L157)、[plugins/powershell-adapter/domains/applications.ps1:3](../../plugins/powershell-adapter/domains/applications.ps1#L3)、[crates/contracts/src/adapters/source_gaps.rs:46](../../crates/contracts/src/adapters/source_gaps.rs#L46)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[crates/storage/src/projection.rs:151](../../crates/storage/src/projection.rs#L151)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)。

<a id="source-004"></a>

## 004 Firefox 扩展

来源：`pcscope.apps.firefox-extensions`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 读取当前用户 Firefox Profiles 的 extensions.json，采到 id、版本、active、user_disabled、app_disabled、signed_state 和登记日期；adapter 只保留浏览器、配置文件、ID、版本与 active。

**整改后输出：** 显示 Firefox 扩展登记启停、可取得的名称及停用原因；未知名称仍如实说明，保留配置文件和最近读取时间。

**判断规则：** active 是 Firefox 文件中记录的状态，不能升级成当前进程已加载；user_disabled/app_disabled 分别解释用户停用或应用停用。读取失败保留上次资料并标过时。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 采集未提供人类可读的扩展名称；停用原因和签名状态已采但未适配。
- installation_scope 未保留用户，需验证并修复不同用户同名 profile 的碰撞。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产仍为 PowerShell 文件解析；原生迁移保留重解析点和文件大小边界。

**验收：** 启用、用户停用、应用停用、文件损坏以及不同用户同名配置文件分别显示，未读取签名不显示签名安全。

采集定义：`pcscope.apps.firefox-extensions`。

对象类型：`shell.browser_extension`。

依据：[plugins/powershell-adapter/domains/applications.ps1:45](../../plugins/powershell-adapter/domains/applications.ps1#L45)、[crates/contracts/src/adapters/shell.rs:298](../../crates/contracts/src/adapters/shell.rs#L298)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-005"></a>

## 005 VS Code 扩展清单

来源：`pcscope.apps.ide-extensions`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 枚举当前用户 VS Code 与 Insiders 扩展 package.json，采 name、publisher、version、engines；adapter 明确将 editor、extension_id 留空，故身份暂定。

**整改后输出：** 按编辑器和用户显示扩展登记名称、发布者、版本、兼容编辑器范围；说明这是磁盘登记，不代表已激活。

**判断规则：** 采集端显式提供编辑器种类与由有效 publisher/name 构成的扩展 ID，再按用户、编辑器、ID、版本建身份；不从任意文件夹名猜发布者。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 缺 editor/extension_id 导致已有有效清单仍无法确认身份。
- name 是 manifest 技术名称，displayName/本地化名称未采；兼容版本范围未形成易读摘要。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：PowerShell 目录扫描待原生迁移，并补目录边界与重解析点回归。

**验收：** 稳定版与 Insiders 的同名扩展分开，升级、损坏 manifest、缺 publisher 不误合并；不把存在扩展目录等同启用。

采集定义：`pcscope.apps.ide-extensions`。

对象类型：`desktop.editor_extension`。

依据：[plugins/powershell-adapter/domains/applications.ps1:2](../../plugins/powershell-adapter/domains/applications.ps1#L2)、[crates/contracts/src/adapters/desktop.rs:433](../../crates/contracts/src/adapters/desktop.rs#L433)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-006"></a>

## 006 Mihomo 本地运行配置

来源：`pcscope.apps.mihomo`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 显式开启后，对固定 loopback 端口 GET /version 与 /configs，5 秒超时且不重定向；采模式、端口、TUN、IPv6、LAN 等。adapter 却映射到需要 path/value_name 的注册表代理配置，value_name 缺失且所有运行配置字段未适配。

**整改后输出：** 显示“Mihomo 本地接口可访问、规则/全局/直连模式、TUN 配置、监听端口”和读取时间；失败区分未连接、授权不足、读取失败。

**判断规则：** 成功 API 响应已足以说明该接口当时可达和它返回的配置，不要求另查进程重复证明；mode、tun_enabled 与系统代理设置是不同事实。以本机端点建立明确身份，令牌不进入资料。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前对象类型与输入不匹配，核心配置全部丢失且身份长期暂定。
- 无法由现有输入断言实际流量路径、网络可用性；此为接口范围限制，应明确展示而非补猜测。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 HTTP 查询仍由 PowerShell 发起；应迁为后端 HTTP 客户端，单飞、超时、冷却和禁重定向保持。

**验收：** 模式/TUN/端口变化直接更新相应配置；接口未启用、401、超时不写成代理已关闭，不暴露 token。

采集定义：`pcscope.apps.mihomo`。

对象类型：`network.proxy_configuration`。

依据：[plugins/powershell-adapter/domains/scoped.ps1:71](../../plugins/powershell-adapter/domains/scoped.ps1#L71)、[crates/contracts/src/adapters/network.rs:798](../../crates/contracts/src/adapters/network.rs#L798)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-007"></a>

## 007 全局 npm 包清单

来源：`pcscope.apps.npm-global-packages`。路线：原生。用途：状态或配置快照。任务：SP01、SP04、SP05、SP06、SP07、SP09。

**现状：** 扫描当前用户 APPDATA npm 与标准 Program Files node_modules（含 @scope），不跟随重解析目录，manifest 限 1 MiB；adapter 保留 npm、包名、版本、路径。

**整改后输出：** 显示“npm 全局包登记”、名称和版本及所在用户/位置；完整扫描中的新增移除形成登记变化。

**判断规则：** 有效 package.json 支持包登记事实，不支持包执行状态。只涵盖当前配置的标准根目录，不能把未扫描的自定义 prefix 判为空。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 默认名称与包管理器/范围说明不充分，可能被当作普通已安装应用。
- 自定义 npm prefix 未覆盖，属于可选范围扩展，不是标准根目录采集失败。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产文件遍历仍为 PowerShell，需原生有界扫描及多根目录同文件去重验收。

**验收：** 普通包、@scope 包、用户与机器根、重解析目录和超限 manifest 均正确，未配置自定义目录显示范围限制。

采集定义：`pcscope.apps.npm-global-packages`。

对象类型：`software.language_package`。

依据：[plugins/powershell-adapter/domains/applications.ps1:4](../../plugins/powershell-adapter/domains/applications.ps1#L4)、[crates/contracts/src/adapters/software.rs:157](../../crates/contracts/src/adapters/software.rs#L157)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-008"></a>

## 008 OneDrive 账户与同步登记

来源：`pcscope.apps.onedrive-registration`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 读取当前用户 OneDrive Accounts、SyncEngines Providers 与机器 OneDrive 策略三种形状；adapter 统一为 sync_account，以 NamespaceID 或 AccountName 识别，挂载位置只取 MountPoint。

**整改后输出：** 分别呈现同步账户登记、同步文件夹登记、OneDrive 策略；有明确证据才关联，名称下说明用途，不将登记称同步正常。

**判断规则：** AccountName/NamespaceID 作为对应登记身份；UserFolder 和 MountPoint 保留各自含义。策略按策略对象表达，不强行要求账户 ID。登记存在不推出客户端正在同步。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 三种资料强装同一账户类型；机器策略缺账户 ID 而暂定，UserFolder 未适配，账户与提供程序的关联未建模。
- 未采同步进度/最近成功时间是当前登记范围限制，不应伪装运行健康状态。
- 原生注册表迁移与用户隔离验收待完成。

**验收：** 账户、挂载点、机器策略各有准确名称与范围；两个账户不按目录短名合并，未运行客户端仍只显示登记。

采集定义：`pcscope.apps.onedrive-registration`。

对象类型：`desktop.sync_account`。

依据：[plugins/powershell-adapter/domains/applications.ps1:11](../../plugins/powershell-adapter/domains/applications.ps1#L11)、[crates/contracts/src/adapters/desktop.rs:502](../../crates/contracts/src/adapters/desktop.rs#L502)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-009"></a>

## 009 事件通道可读性

来源：`pcscope.collector.event-channel-coverage`。路线：原生。用途：采集健康。任务：SP01、SP05、SP06、SP09。

**现状：** 原生读取日志配置、可读性、保留端点，保存source_readings；不证明审计完整。

**整改后输出：** 按通道显示可读/权限不足/未开启、现存记录时间范围及本次检查时间。

**判断规则：** readable=false为无法读取，不是零条；enabled与审计策略分开，空日志不证明期间无活动。

**具体缺口与范围：** 

- 当前通用摘要缺通道可读性与审计策略/保留历史的区分
- 未统一展示过期能力检查与后续实际读取失败

**验收：** 覆盖可读空日志、ACL拒绝、关闭通道、截断、审计未配置；无一项被显示为系统功能正常。

采集定义：`pcscope.collector.event-channel-coverage`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/platform-native/Sources.cs:171](../../plugins/platform-native/Sources.cs#L171)、[crates/contracts/src/adapters/source_gaps.rs:28](../../crates/contracts/src/adapters/source_gaps.rs#L28)。

<a id="source-010"></a>

## 010 指定范围配置

来源：`pcscope.collector.scoped-configuration`。路线：原生。用途：采集健康。任务：SP01、SP05、SP06、SP07、SP09。

**现状：** 原生保存显式文件/目录名单、模块目标、采样限制与content_versions未实现提示。

**整改后输出：** 显示目前监控哪些路径/进程，以及排除范围、暂停和配置版本。

**判断规则：** 范围配置是采集设置，不是系统配置状态；空名单显示未配置，不是监控正常。

**具体缺口与范围：** 

- content_versions仅声明not implemented，未保存文件内容版本
- 范围配置与真正成功观察的范围尚未统一对账

**验收：** 空名单/改名单/禁用/路径不可读显示真实覆盖，配置变动不触发无界递归扫描。

采集定义：`pcscope.collector.scoped-configuration`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/platform-native/Sources.cs:180](../../plugins/platform-native/Sources.cs#L180)、[crates/contracts/src/adapters/source_gaps.rs:28](../../crates/contracts/src/adapters/source_gaps.rs#L28)。

<a id="source-011"></a>

## 011 剪贴板设置

来源：`pcscope.desktop.clipboard`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** 读取剪贴板历史、云剪贴板及相关机器策略的注册表配置；adapter 为 shell_configuration 整表 values，不读取剪贴板内容。

**整改后输出：** 显示“剪贴板历史：已开启/关闭/未取得设置”“跨设备同步：允许/限制”等配置和范围，解释对使用功能的影响。

**判断规则：** 只按明确值名和取值解码；用户设置与禁止策略分开，不用值缺失代替关闭，不把配置变化当成复制事件。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前整张 values 表缺少逐项名称、值语义和策略/偏好区分。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：PowerShell 注册表采集待原生迁移；内容始终不属于该来源采集范围。

**验收：** 历史开关、云同步、机器禁用、缺失和读取失败均有不同说明，任何状态不显示或采集剪贴板内容。

采集定义：`pcscope.desktop.clipboard`。

对象类型：`desktop.shell_configuration`。

依据：[plugins/powershell-adapter/domains/desktop.ps1:6](../../plugins/powershell-adapter/domains/desktop.ps1#L6)、[crates/contracts/src/adapters/desktop.rs:347](../../crates/contracts/src/adapters/desktop.rs#L347)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-012"></a>

## 012 键盘、输入法与过滤驱动配置

来源：`pcscope.desktop.input`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 读取用户键盘布局顺序、机器布局名称/文件及键盘类过滤驱动；adapter 只保留 layout_file、ime_file、upper/lower_filters 与路径。

**整改后输出：** 显示“已登记输入布局”“默认顺序”“键盘过滤组件”及变更，使用已采 LayoutText 等可信名称。

**判断规则：** 布局登记顺序不等于当前窗口正在使用的输入法；过滤驱动名单不等于实际键盘钩子。按用户布局 ID 与机器布局定义准确关联。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- LayoutText、LayoutId 与用户 1..5 顺序值未适配，现有名称和输入布局关系难读。
- 当前活动输入法、进程钩子不在此来源范围，不能将未采解释为没有。
- 注册表原生迁移与跨用户/布局引用关联待完成。

**验收：** 两种布局顺序交换可读展示；未找到机器定义时保留未知；过滤驱动名称不被写成已拦截输入。

采集定义：`pcscope.desktop.input`。

对象类型：`desktop.input_configuration`。

依据：[plugins/powershell-adapter/domains/desktop.ps1:5](../../plugins/powershell-adapter/domains/desktop.ps1#L5)、[crates/contracts/src/adapters/desktop.rs:308](../../crates/contracts/src/adapters/desktop.rs#L308)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-013"></a>

## 013 打印作业队列

来源：`pcscope.desktop.print-jobs`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 按打印机读取队列任务，源记录 ID 包含打印机、任务编号和提交时间；采状态、总页数、已打印页、大小，文档名受保护。adapter 身份却只有打印机和 job_id，pages 读取不存在的 pages 键。

**整改后输出：** 显示“某打印机的打印任务、排队/打印/暂停/错误、进度”，名称不依赖文档内容；保留任务生命周期和最后记录时间。

**判断规则：** 队列状态直接支持当时任务状态；同打印机编号复用必须加任务代际。任务从快照消失只表示不再队列中；有可靠打印完成/取消日志时可直接确立终态。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 已采 total_pages/pages_printed 未正确映射；队列状态枚举未形成易读影响。
- 模型身份缺 submitted_time，有复用任务编号误合并风险。
- 生产每次 Get-Printer 再逐机 Get-PrintJob；需原生 spooler 有界查询、并发上限和取消，防慢/远端队列拖累全部采集。

**验收：** 同打印机重复 job_id 的不同提交任务分离；页数与进度正确，队列消失不直接判打印成功，文档名遮罩保留。

采集定义：`pcscope.desktop.print-jobs`。

对象类型：`desktop.print_job`。

依据：[plugins/powershell-adapter/domains/desktop.ps1:4](../../plugins/powershell-adapter/domains/desktop.ps1#L4)、[crates/contracts/src/adapters/desktop.rs:252](../../crates/contracts/src/adapters/desktop.rs#L252)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-014"></a>

## 014 打印驱动

来源：`pcscope.desktop.printer-drivers`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 采打印驱动名称、环境架构、厂商、MajorVersion 和驱动/INF/配置/数据文件路径；adapter 保留 name、environment、manufacturer、driver_path、inf_path。

**整改后输出：** 显示打印驱动登记名称、适用架构、厂商与关联打印机；变更表示驱动登记变化。

**判断规则：** name+environment 区分登记；MajorVersion 仅驱动模型代际，不冒充完整软件版本。登记不证明当前被 spooler 加载。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- MajorVersion 和配置/数据文件未进入语义字段，需明确哪些用于概要、哪些只保留详情。
- 原生 WinSpool 采集及按精确驱动名/环境关联打印机待完成。

**验收：** 同名不同架构驱动分开；V3/V4 代际不会标成应用版本，驱动登记移除不伪造卸载执行时间。

采集定义：`pcscope.desktop.printer-drivers`。

对象类型：`desktop.printer_driver`。

依据：[plugins/powershell-adapter/domains/desktop.ps1:2](../../plugins/powershell-adapter/domains/desktop.ps1#L2)、[crates/contracts/src/adapters/desktop.rs:187](../../crates/contracts/src/adapters/desktop.rs#L187)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-015"></a>

## 015 打印端口

来源：`pcscope.desktop.printer-ports`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** 采名称、说明、PortMonitor、地址、端口号、Protocol 和 SNMPEnabled；adapter 保留 host、protocol、port_number，但 port_type 读取源未提供的字段。

**整改后输出：** 显示本地/网络打印端口、目标地址和明确协议，说明“打印机通过此端口连接”；不声称目标在线。

**判断规则：** 按端口名区分配置，确切协议值进行类型专属解释；PortMonitor 是端口监视器，不能直接冒充统一端口类型枚举。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 监视器、说明及 SNMP 开关已采未适配；port_type 长期缺失，协议数字缺中文解释。
- 网络连通性不是端口配置来源所覆盖；原生端口枚举迁移待办。

**验收：** 本地端口、RAW/LPR 网络端口和未知协议均准确；无法读取端口不写成目标打印机离线。

采集定义：`pcscope.desktop.printer-ports`。

对象类型：`desktop.printer_port`。

依据：[plugins/powershell-adapter/domains/desktop.ps1:3](../../plugins/powershell-adapter/domains/desktop.ps1#L3)、[crates/contracts/src/adapters/desktop.rs:218](../../crates/contracts/src/adapters/desktop.rs#L218)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-016"></a>

## 016 打印机清单

来源：`pcscope.desktop.printing`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-Printer 采名称、驱动、端口、共享、打印机状态、类型和渲染模式；adapter 保留 name、driver、port、shared、status。

**整改后输出：** 显示打印机名称及“就绪/暂停/脱机/需要处理”等来源明确状态，配合驱动和连接方式；对象采集状态单独说明。

**判断规则：** 按 Windows 打印状态枚举或组合位解释，未知值原样留详情；配置存在只说明队列存在。状态报告有时效，不扩展成物理设备完整健康判断。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 打印状态缺按类型解码和摘要输出，通用对象状态掩盖具体打印影响。
- Type、ShareName 等已采字段未适配，关联端口/驱动需精确名称与机器范围。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 PowerShell 待原生 spooler 迁移，保留远端慢队列超时和逐对象部分失败。

**验收：** 就绪、脱机、暂停、未知组合和读取失败分别显示；一台失败不清空其他打印机资料。

采集定义：`pcscope.desktop.printing`。

对象类型：`desktop.printer`。

依据：[plugins/powershell-adapter/domains/desktop.ps1:1](../../plugins/powershell-adapter/domains/desktop.ps1#L1)、[crates/contracts/src/adapters/desktop.rs:162](../../crates/contracts/src/adapters/desktop.rs#L162)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-017"></a>

## 017 桌面与资源管理器设置

来源：`pcscope.desktop.shell-settings`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** 读取 Explorer Advanced、Control Panel Desktop、User Shell Folders 三类设置，adapter 按注册表键保存 values。

**整改后输出：** 按“资源管理器显示”“桌面行为”“个人文件夹位置”展示可解释配置和变化；技术键名折叠。

**判断规则：** 采用具体设置白名单、类型和 Windows 适用版本解释，未知值保留。个人偏好不直接推出当前 Explorer 已重载；目录重定向只陈述登记目标。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 整表 values 未转成各设置可读状态，三类内容仍容易变成路径列表。
- 对 Windows 版本有效性的说明以及目录位置隐私展示需一致；生产注册表原生迁移待办。

**验收：** 显示隐藏文件、桌面设置和目录位置各有准确名称；未知键不猜含义，路径遮罩与详情保持一致。

采集定义：`pcscope.desktop.shell-settings`。

对象类型：`desktop.shell_configuration`。

依据：[plugins/powershell-adapter/domains/desktop.ps1:7](../../plugins/powershell-adapter/domains/desktop.ps1#L7)、[crates/contracts/src/adapters/desktop.rs:347](../../crates/contracts/src/adapters/desktop.rs#L347)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-018"></a>

## 018 桌面与开始菜单快捷方式

来源：`pcscope.desktop.shortcuts`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 有界枚举用户/公共桌面和开始菜单 .lnk，拒绝跟随重解析目录和 UNC；通过 WScript.Shell 读取目标、工作目录、受保护参数等。adapter 的 file_identity 所需 volume_identity/file_id 均为空。

**整改后输出：** 显示快捷方式的可读名称、目标和位置；目标或图标配置变化能持续归到同一快捷方式，未解析说明原因。

**判断规则：** 原生 IShellLink 读取元数据且不执行目标；显式读取文件身份以支持重命名连续性，解析失败与目标不存在分别表示。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 所有快捷方式缺文件身份，长期暂定；当前材料有路径但不应拿路径伪装稳定文件 ID。
- 未验证目标存在是当前元数据范围，不自动宣布快捷方式失效；若增加目标检查应独立、限本地且有界。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：PowerShell+COM 逐项读取待原生迁移，保留总量限制与慢链接取消。

**验收：** 重命名同一 .lnk 不造新身份；损坏链接和不可读文件各有解释，所有测试均不启动快捷方式目标。

采集定义：`pcscope.desktop.shortcuts`。

对象类型：`desktop.shortcut`。

依据：[plugins/powershell-adapter/domains/desktop.ps1:8](../../plugins/powershell-adapter/domains/desktop.ps1#L8)、[crates/contracts/src/adapters/desktop.rs:395](../../crates/contracts/src/adapters/desktop.rs#L395)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-019"></a>

## 019 环境变量与默认文件关联

来源：`pcscope.environment.configuration`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已读机器/当前用户环境变量及文件、URL 默认关联，注册表全部值经 Protect-Value 保留；adapter 将变量逐值展开，并将 FileExts/UrlAssociations 转为文件关联登记。

**整改后输出：** 显示变量所属用户或机器、是否设置及变化；文件类型/链接类型目前指向哪个程序标识、修改可能影响新启动程序或默认打开方式。

**判断规则：** 按 hive+view+path+value_name 读变量；按实际关联键读取 ProgId，未解析到程序显示名称时仍说明对应程序尚未识别。新进程通常读取新环境，不能说所有现有进程已切换。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 代码路径已证实的关联风险，尚未在本轮复现数据损坏：一个父记录展开多个对象，但 projection.apply 目前仅对账户权限/组成员启用按子身份绑定；此来源仍可能复用父绑定并跳过其他子项。
- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。
- 文件关联身份已按 hive/view/registry_path 建立；但 ProgId 到可读程序名称/可执行路径的关联尚未覆盖，不能把 ProgId 当已验证程序可用。

**验收：** 同键多变量、两个用户同名变量及 .txt/.pdf/https 三种关联分别保留；遮罩值不进入标题或明文变化。

采集定义：`pcscope.environment.configuration`。

对象类型：`policy.environment_variable`、`shell.file_association`。

依据：[plugins/powershell-adapter/domains/startup.ps1:8](../../plugins/powershell-adapter/domains/startup.ps1#L8)、[crates/contracts/src/adapters/policy.rs:241](../../crates/contracts/src/adapters/policy.rs#L241)、[crates/contracts/src/adapters/policy.rs:270](../../crates/contracts/src/adapters/policy.rs#L270)、[crates/contracts/src/adapters/policy.rs:305](../../crates/contracts/src/adapters/policy.rs#L305)、[crates/storage/src/projection.rs:151](../../crates/storage/src/projection.rs#L151)、[model/types.cjs:50](../../model/types.cjs#L50)。

<a id="source-020"></a>

## 020 应用程序事件 / 软件安装事件

来源：`pcscope.eventlog.application`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源10项定义，其中5项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**具体缺口与范围：** 

- installer：目前只按provider或少量ID分组，缺明确事件语义、返回码及字段映射；原始记录保留，不代表已更新当前对象状态。
- errors：目前只按provider或少量ID分组，缺明确事件语义、返回码及字段映射；原始记录保留，不代表已更新当前对象状态。
- hangs：目前只按provider或少量ID分组，缺明确事件语义、返回码及字段映射；原始记录保留，不代表已更新当前对象状态。
- nvwmi：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- vss：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- wer：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- licensing：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- optimization：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.application.installer`、`pcscope.eventlog.application.errors`、`pcscope.eventlog.application.hangs`、`pcscope.eventlog.application.nvwmi`、`pcscope.eventlog.application.vss`、`pcscope.eventlog.application.wer`、`pcscope.eventlog.application.licensing`、`pcscope.eventlog.application.optimization`、`pcscope.eventlog.application.status`、`pcscope.eventlog.application.other`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-021"></a>

## 021 手动导入事件日志

来源：`pcscope.eventlog.import`。路线：虚拟汇总/导入。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP09。

**现状：** 虚拟入口prefix定义id=pcscope.eventlog.import/source=pcscope.eventlog.import.；导入材料与实时日志同属event路径。

**整改后输出：** 单独显示导入来源、原始事件时间、导入时间和来源机器/通道，保留已知事件解释。

**判断规则：** 可修补历史事件/历史状态，但旧导入不得覆盖较新当前状态；无原机器/启动周期不绑定本机现有实例。

**具体缺口与范围：** 

- 尚无统一导入历史不推进当前字段/实例生命周期的强制规则
- 语义规则按精确实时source匹配，导入prefix可能无法复用provider+channel语义
- 需明确来源机器与事件epoch/recordID避免导入重复

**验收：** 同日志重复导入、乱序、外机、旧启动周期、未来时钟、导入后不刷新当前运行状态。

采集定义：`pcscope.eventlog.import`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[apps/desktop/ui/collection-catalog.json](../../apps/desktop/ui/collection-catalog.json)、[crates/contracts/src/adapters/event.rs:155](../../crates/contracts/src/adapters/event.rs#L155)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-022"></a>

## 022 AppLocker 程序与 DLL 事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-applocker/exe and dll`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：程序/DLL的允许、阻止、仅审计模式及策略应用结果。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-applocker/exe and dll.status`、`pcscope.eventlog.microsoft-windows-applocker/exe and dll.other`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-023"></a>

## 023 AppLocker 安装包与脚本事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-applocker/msi and script`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：安装包/脚本的允许、阻止、仅审计模式及策略应用结果。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-applocker/msi and script.status`、`pcscope.eventlog.microsoft-windows-applocker/msi and script.other`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-024"></a>

## 024 AppX 部署事件 / 应用包部署

来源：`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源4项定义，其中2项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- deployment：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- service：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.deployment`、`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.service`、`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.status`、`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.other`。

对象类型：`software.appx_package`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-025"></a>

## 025 Windows 备份事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-backup`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：备份作业的开始、完成、失败及目标卷身份。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-backup.status`、`pcscope.eventlog.microsoft-windows-backup.other`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-026"></a>

## 026 BitLocker 管理事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：卷加密、保护开关、锁定/解锁及恢复操作。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management.status`、`pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management.other`。

对象类型：`security.bitlocker_configuration`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-027"></a>

## 027 代码完整性事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-codeintegrity/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：代码验证、强制阻止与仅审计事件，关联实际文件身份。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-codeintegrity/operational.status`、`pcscope.eventlog.microsoft-windows-codeintegrity/operational.other`。

对象类型：`file.watched_file`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-028"></a>

## 028 用户模式驱动事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：特定设备/驱动实例的启动、退出、超时与恢复。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational.status`、`pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational.other`。

对象类型：`hardware.device`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-029"></a>

## 029 组策略处理事件 / 组策略服务生命周期

来源：`pcscope.eventlog.microsoft-windows-grouppolicy/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源7项定义，其中5项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- service：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- processing：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- sessions：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- applicability：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- diagnostics：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-grouppolicy/operational.service`、`pcscope.eventlog.microsoft-windows-grouppolicy/operational.processing`、`pcscope.eventlog.microsoft-windows-grouppolicy/operational.sessions`、`pcscope.eventlog.microsoft-windows-grouppolicy/operational.applicability`、`pcscope.eventlog.microsoft-windows-grouppolicy/operational.diagnostics`、`pcscope.eventlog.microsoft-windows-grouppolicy/operational.status`、`pcscope.eventlog.microsoft-windows-grouppolicy/operational.other`。

对象类型：`policy.os_configuration`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-030"></a>

## 030 Hyper-V 管理事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：特定虚拟机运行操作与管理服务状态。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin.status`、`pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin.other`。

对象类型：`virtualization.virtual_machine`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-031"></a>

## 031 网络配置文件事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-networkprofile/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：网络配置文件的连接、断开与类别。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-networkprofile/operational.status`、`pcscope.eventlog.microsoft-windows-networkprofile/operational.other`。

对象类型：`network.connection_profile`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-032"></a>

## 032 PowerShell 运行事件 / PowerShell 命令与脚本

来源：`pcscope.eventlog.microsoft-windows-powershell/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源4项定义，其中2项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**具体缺口与范围：** 

- commands：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- host：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-powershell/operational.commands`、`pcscope.eventlog.microsoft-windows-powershell/operational.host`、`pcscope.eventlog.microsoft-windows-powershell/operational.status`、`pcscope.eventlog.microsoft-windows-powershell/operational.other`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-033"></a>

## 033 打印服务事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-printservice/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：打印作业的提交、完成、失败，不能当打印机全局状态。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-printservice/operational.status`、`pcscope.eventlog.microsoft-windows-printservice/operational.other`。

对象类型：`desktop.print_job`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-034"></a>

## 034 Sysmon / 进程生命周期

来源：`pcscope.eventlog.microsoft-windows-sysmon/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源14项定义，其中12项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- processes：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- process-access：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- network：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- dns：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- files：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- registry：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- modules：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- wmi：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- pipes：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- raw-disk：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- clipboard：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- monitor：采集定义已有逐ID动作名称，但原生Kind仍输出execution.sysmon_event，尚未将这些映射接到统一模型动作/字段投影；不能记为已经状态化。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-sysmon/operational.processes`、`pcscope.eventlog.microsoft-windows-sysmon/operational.process-access`、`pcscope.eventlog.microsoft-windows-sysmon/operational.network`、`pcscope.eventlog.microsoft-windows-sysmon/operational.dns`、`pcscope.eventlog.microsoft-windows-sysmon/operational.files`、`pcscope.eventlog.microsoft-windows-sysmon/operational.registry`、`pcscope.eventlog.microsoft-windows-sysmon/operational.modules`、`pcscope.eventlog.microsoft-windows-sysmon/operational.wmi`、`pcscope.eventlog.microsoft-windows-sysmon/operational.pipes`、`pcscope.eventlog.microsoft-windows-sysmon/operational.raw-disk`、`pcscope.eventlog.microsoft-windows-sysmon/operational.clipboard`、`pcscope.eventlog.microsoft-windows-sysmon/operational.monitor`、`pcscope.eventlog.microsoft-windows-sysmon/operational.status`、`pcscope.eventlog.microsoft-windows-sysmon/operational.other`。

对象类型：`runtime.process`、`runtime.module_instance`、`hardware.driver_service`、`network.endpoint`、`file.watched_file`、`file.alternate_data_stream`、`policy.os_configuration`、`startup.wmi_filter`、`startup.wmi_consumer`、`startup.wmi_binding`、`hardware.disk`、`desktop.shell_configuration`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-035"></a>

## 035 计划任务执行事件 / 任务触发与执行过程

来源：`pcscope.eventlog.microsoft-windows-taskscheduler/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源4项定义，其中2项有逐ID中文动作表；status是来源健康，other是未知回退。任务100/102有旧动作fallback，其余中文语义主要用于原始事件展示。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。 已增加精确事件规则：账户启停、任务开始时间、同范围唯一服务状态；终端会话因缺少事件启动周期只形成可读事件。仅适用此来源对应的事件 ID/版本，详见实施记录。

**具体缺口与范围：** 

- execution：前端逐ID已有解释；原生仍输出startup.task_event，仅100/102走旧模型fallback且只生成动作；其余主要留在原始事件。所有列出的运行实例状态转换待实现。
- configuration：前端逐ID已有解释；原生仍输出startup.task_event，仅100/102走旧模型fallback且只生成动作；其余主要留在原始事件。所有列出的运行实例状态转换待实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-taskscheduler/operational.execution`、`pcscope.eventlog.microsoft-windows-taskscheduler/operational.configuration`、`pcscope.eventlog.microsoft-windows-taskscheduler/operational.status`、`pcscope.eventlog.microsoft-windows-taskscheduler/operational.other`。

对象类型：`startup.scheduled_task`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-036"></a>

## 036 终端会话登录、断开与注销 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。 已增加精确事件规则：账户启停、任务开始时间、同范围唯一服务状态；终端会话因缺少事件启动周期只形成可读事件。仅适用此来源对应的事件 ID/版本，详见实施记录。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：具体启动周期/会话的登录、退出、连接与断开。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational.status`、`pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational.other`。

对象类型：`runtime.terminal_session`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-037"></a>

## 037 远程连接管理事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：远程桌面身份验证和连接阶段；验证成功不等于桌面就绪。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational.status`、`pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational.other`。

对象类型：`runtime.terminal_session`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-038"></a>

## 038 Defender 防护事件 / Defender 健康与情报

来源：`pcscope.eventlog.microsoft-windows-windows defender/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源3项定义，其中1项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- health：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-windows defender/operational.health`、`pcscope.eventlog.microsoft-windows-windows defender/operational.status`、`pcscope.eventlog.microsoft-windows-windows defender/operational.other`。

对象类型：`security.protection`、`file.watched_file`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-039"></a>

## 039 Windows 更新事件 / 更新检查

来源：`pcscope.eventlog.microsoft-windows-windowsupdateclient/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源3项定义，其中1项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- discovery：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.discovery`、`pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.status`、`pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.other`。

对象类型：`software.system_update`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-040"></a>

## 040 无线网络连接事件 / 来源采集状态

来源：`pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源2项定义，其中0项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前此通道只有status/other定义，未覆盖：特定无线接口的关联、认证、断开及原因；连接Wi-Fi不等于可访问互联网。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational.status`、`pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational.other`。

对象类型：`network.connection_profile`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-041"></a>

## 041 WMI 活动事件 / WMI 提供程序与调用

来源：`pcscope.eventlog.microsoft-windows-wmi-activity/operational`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源3项定义，其中1项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- operations：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.microsoft-windows-wmi-activity/operational.operations`、`pcscope.eventlog.microsoft-windows-wmi-activity/operational.status`、`pcscope.eventlog.microsoft-windows-wmi-activity/operational.other`。

对象类型：`startup.wmi_filter`、`startup.wmi_consumer`、`startup.wmi_binding`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-042"></a>

## 042 Windows 安全审计 / 登录、登出与会话

来源：`pcscope.eventlog.security`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源26项定义，其中24项有逐ID中文动作表；status是来源健康，other是未知回退。Security核心已注册动作可入模型动作表，但没有通用事件→当前字段写入。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。 已增加精确事件规则：账户启停、任务开始时间、同范围唯一服务状态；终端会话因缺少事件启动周期只形成可读事件。仅适用此来源对应的事件 ID/版本，详见实施记录。 2026-10-13 已为既有 Security 日志增加精确提供程序+4907 的标题/影响解释及独立子项开关；8,727 条冻结原始记录由未知标题变为可读。未新增来源、轮询或模型动作投影，不凭标题建立对象关联或更新文件内容/访问权限。

**具体缺口与范围：** 

- sessions：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- accounts：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- groups：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- privileges：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- processes：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- files：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- registry：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- services：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- tasks：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- audit-policy：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- network：原生已输出这组已注册动作，能进入动作投影；尚未把事件明确值更新到object_facts。删除动作仅有少数生命周期提示，仍需统一时序和身份保护。
- log-clear：已有明确采集缺口语义名称；需验收1102实际驱动覆盖边界，而不只保留原始清除事件。
- crypto-keys：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- crypto-integrity：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- crypto-configuration：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- data-protection：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- credential-access：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- membership-queries：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- object-access：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- process-tokens：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- system-audit：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- firewall-lifecycle：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- audit-service：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。
- files-audit-settings：4907 原始标题/影响已解释；仍无模型动作投影或精确SACL对象关联，不将审计设置变化混为访问权限或文件内容变化。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.security.sessions`、`pcscope.eventlog.security.accounts`、`pcscope.eventlog.security.groups`、`pcscope.eventlog.security.privileges`、`pcscope.eventlog.security.processes`、`pcscope.eventlog.security.files`、`pcscope.eventlog.security.registry`、`pcscope.eventlog.security.services`、`pcscope.eventlog.security.tasks`、`pcscope.eventlog.security.audit-policy`、`pcscope.eventlog.security.network`、`pcscope.eventlog.security.log-clear`、`pcscope.eventlog.security.crypto-keys`、`pcscope.eventlog.security.crypto-integrity`、`pcscope.eventlog.security.crypto-configuration`、`pcscope.eventlog.security.data-protection`、`pcscope.eventlog.security.credential-access`、`pcscope.eventlog.security.membership-queries`、`pcscope.eventlog.security.object-access`、`pcscope.eventlog.security.files-audit-settings`、`pcscope.eventlog.security.process-tokens`、`pcscope.eventlog.security.system-audit`、`pcscope.eventlog.security.firewall-lifecycle`、`pcscope.eventlog.security.audit-service`、`pcscope.eventlog.security.status`、`pcscope.eventlog.security.other`。

对象类型：`account.user`、`account.group_membership`、`runtime.logon_session`、`runtime.terminal_session`、`runtime.process`、`file.watched_file`、`file.acl_entry`、`policy.os_configuration`、`startup.service`、`startup.scheduled_task`、`network.endpoint`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-043"></a>

## 043 系统与服务事件 / 服务运行事件

来源：`pcscope.eventlog.system`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生EventLogReader增量采集；本来源7项定义，其中2项有逐ID中文动作表；status是来源健康，other是未知回退。中文动作表主要用于原始日志显示，不等同后台当前对象状态投影。

**整改后输出：** 逐事件说明发生了什么及结果；明确状态事件直接更新对应字段，运行实例/阶段和对象配置分开。未识别日志继续可查。

**判断规则：** 仅由已核对的通道、提供程序、事件ID、版本和字段布局决定转换；明确完成事件可直接更新对应状态，无需另一次轮询证明。按事件发生时间和对象世代/会话归属仲裁，重复与乱序幂等，导入历史不得盖过较新当前值；诊断仅说明故障经历，开始不当作成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。 已增加精确事件规则：账户启停、任务开始时间、同范围唯一服务状态；终端会话因缺少事件启动周期只形成可读事件。仅适用此来源对应的事件 ID/版本，详见实施记录。

**具体缺口与范围：** 

- services：目前只按provider或少量ID分组，缺明确事件语义、返回码及字段映射；原始记录保留，不代表已更新当前对象状态。
- lifecycle：目前只按provider或少量ID分组，缺明确事件语义、返回码及字段映射；原始记录保留，不代表已更新当前对象状态。
- power：目前只按provider或少量ID分组，缺明确事件语义、返回码及字段映射；原始记录保留，不代表已更新当前对象状态。
- registry-maintenance：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- time-source：前端按provider+ID已有中文语义；这些event.*动作未接入模型注册/当前字段投影，且没有事件Version适配门槛。拟定字段转换尚未实现。
- 仍有未映射事件；补逐ID清单及版本验收，未知回退保留原始材料。
- 语义规则仅channel/provider/event_id匹配，事件Version/字段布局未作为适配边界；未知版本必须保留原始记录且不猜状态。

**验收：** 逐项执行event-items-review对应ID/版本/返回码正反例；同ID异provider、未知版本、字段缺失、乱序/重复/导入旧日志不得误写状态；原生读取不启动PowerShell形成自采集循环。

采集定义：`pcscope.eventlog.system.services`、`pcscope.eventlog.system.lifecycle`、`pcscope.eventlog.system.power`、`pcscope.eventlog.system.registry-maintenance`、`pcscope.eventlog.system.time-source`、`pcscope.eventlog.system.status`、`pcscope.eventlog.system.other`。

对象类型：`startup.service`、`policy.os_configuration`。

依据：[plugins/eventlog-native/Program.cs:32](../../plugins/eventlog-native/Program.cs#L32)、[apps/desktop/ui/event-semantics.js:180](../../apps/desktop/ui/event-semantics.js#L180)、[crates/contracts/src/adapters/event.rs:218](../../crates/contracts/src/adapters/event.rs#L218)、[crates/storage/src/projection.rs:535](../../crates/storage/src/projection.rs#L535)。

<a id="source-044"></a>

## 044 已观察模块的磁盘文件变化

来源：`pcscope.execution.module-file-state`。路线：原生。用途：多种材料。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP08、SP09。

**现状：** 已观察模块形成有界文件名单；原生目录通知+定期复核；stable且exists时投影file.watched_file，missing/unavailable只保留检查记录。

**整改后输出：** 展示磁盘文件缺失、内容/权限/版本/签名/基础结构变化以及依赖进程；说明下次启动/使用可能受影响。

**判断规则：** 最后有效观察与本次失败分开；磁盘变化不等于当前已加载内存变化；未签名不等于损坏；移动必须核对卷和文件ID。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- USN尚未接入模块自动名单；目前通知+定期补查
- 只覆盖曾成功加载的文件，无法提前发现从未成功加载的依赖
- 基础PE检查不验证加载器完整性/初始化；动态依赖和损坏根因未覆盖
- missing/unavailable没有当前对象字段投影，需要统一读取检查状态而非造删除事实

**验收：** 已加载第三方模块后删除/替换/修改ACL/签名失败/同路径替换/改名/通知溢出/64MiB以上文件/超时退避均有准确状态且不自触发风暴。

采集定义：`pcscope.execution.module-file-state`。

对象类型：`file.watched_file`。

依据：[crates/service/src/runtime.rs:1](../../crates/service/src/runtime.rs#L1)、[crates/service/src/runtime.rs:134](../../crates/service/src/runtime.rs#L134)、[plugins/platform-native/Modules.cs:88](../../plugins/platform-native/Modules.cs#L88)、[crates/contracts/src/adapters/source_gaps.rs:34](../../crates/contracts/src/adapters/source_gaps.rs#L34)。

<a id="source-045"></a>

## 045 模块发现与读取范围

来源：`pcscope.execution.module-scan-status`。路线：原生。用途：采集健康。任务：SP01、SP05、SP06、SP08、SP09。

**现状：** 保存observed/not_observed/partial/discovery_failed及每进程读取结果；source_readings保留。

**整改后输出：** 显示sihost是否本次被发现、是否读完模块、为何缺口以及检查时间。

**判断规则：** 没发现进程不自动等于崩溃；partial不推卸载；曾发现不证明桌面功能正常。

**具体缺口与范围：** 

- 缺通用摘要/类型状态展示与时效统一
- 跨会话或未覆盖目标应明确，不把skipped_other_scope当失败

**验收：** 目标未启动、退出、权限不足、模块不稳定、用户切会话的文案准确；健康状态不替代实际功能状态。

采集定义：`pcscope.execution.module-scan-status`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/platform-native/Modules.cs:31](../../plugins/platform-native/Modules.cs#L31)、[crates/contracts/src/adapters/source_gaps.rs:6](../../crates/contracts/src/adapters/source_gaps.rs#L6)、[apps/desktop/ui/model-view.js:326](../../apps/desktop/ui/model-view.js#L326)。

<a id="source-046"></a>

## 046 指定进程加载模块

来源：`pcscope.execution.modules`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP08、SP09。

**现状：** 原生按当前用户/当前会话及精确进程名枚举模块，PID+创建时刻+路径标识；进程/模块并发变化会放弃完整快照。

**整改后输出：** 按进程实例直接显示已加载模块和当前加载清单、模块名称及发布者类别；检查时间和用户/会话范围作辅助信息，只有采集失败或信息过期才提示清单可能不完整。

**判断规则：** 加载记录证明当时已加载；module_base变化只表示地址变化。不得等同磁盘内容、当前仍加载或未来加载必成功。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 模块到runtime.process的正式关联未完成
- 枚举可能遗漏瞬态、手动映射、数据文件加载与其他会话模块
- 普通模型缺完整实例结束/卸载与时效标记

**验收：** 同名进程/PID复用/跨会话隔离、枚举中退出与列表变化、权限拒绝、第三方模块、基址变化均不误判文件损坏。

采集定义：`pcscope.execution.modules`。

对象类型：`runtime.module_instance`。

依据：[plugins/platform-native/Modules.cs:31](../../plugins/platform-native/Modules.cs#L31)、[crates/contracts/src/adapters/runtime.rs:57](../../crates/contracts/src/adapters/runtime.rs#L57)。

<a id="source-047"></a>

## 047 进程清单

来源：`pcscope.execution.processes`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 目前system PowerShell/CIM Win32_Process快照；PID+创建时间标识，命令行受隐私保护；适配不映射来源name。

**整改后输出：** 显示程序名称、当前运行状态、所属账户及必要路径摘要；明确退出事件更新对应进程实例为已退出。最近检查时间作辅助，读取失败或信息过期时明确提示。

**判断规则：** 快照会漏短命进程；缺路径/命令行可为权限或已退出，不能推断不存在；PID复用必须分世代。

**本轮实施进展：** 2026-10-13 已将生产进程入口统一到适配器，承接 path/start_time/session_id 并兼容旧字段；PID 与完整创建时间区分实例，明确不可读或保护值不回退。旧进程历史未批量重新投影；短命进程、退出关联、单独 name 字段和原生迁移仍未完成。见 docs/refactor/acceptance/history-readability-audit.md；本轮实机确认待补。

**具体缺口与范围：** 

- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：进程快照仍由PowerShell采集，尚未全部原生化
- name已采但未映射到模型，摘要只剩PID/创建时间等技术字段
- 无事件补齐的短生命周期和快照absence处理仍不完整

**验收：** 采样与4688/4689/Sysmon交叉验证短进程/PID复用，低权限缺字段有原因，采集自身进程不触发递归。

采集定义：`pcscope.execution.processes`。

对象类型：`runtime.process`。

依据：[plugins/powershell-adapter/domains/system.ps1:25](../../plugins/powershell-adapter/domains/system.ps1#L25)、[crates/contracts/src/adapters/runtime.rs:30](../../crates/contracts/src/adapters/runtime.rs#L30)。

<a id="source-048"></a>

## 048 内存与 CPU 采样

来源：`pcscope.execution.resources`。路线：原生。用途：资源采样。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** 原生资源采样输出CPU百分比、总/可用内存和采样时间，保留source_readings，不建系统对象。

**整改后输出：** 显示采样时刻、单位、CPU/内存读数与变化趋势，采集故障和高使用率分开。

**判断规则：** 瞬时高值不是故障；过期读数不作当前值；缺采样不能当0。

**具体缺口与范围：** 

- 暂无统一趋势/时效规则和采样缺口展示
- 不应把每次sampled_at改变生成用户系统配置变更噪声

**验收：** 零值/极值/多核/采样缺失/时间倒退/长时间运行容量与CPU预算；只显示测量，不宣称诊断结论。

采集定义：`pcscope.execution.resources`。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/platform-native/Sources.cs:160](../../plugins/platform-native/Sources.cs#L160)、[crates/contracts/src/adapters/source_gaps.rs:19](../../crates/contracts/src/adapters/source_gaps.rs#L19)。

<a id="source-049"></a>

## 049 指定目录与直接子项

来源：`pcscope.files.directory-security`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生显式目录读取存在性、目录身份、安全描述符及直接子项；不是全盘递归。

**整改后输出：** 显示目录是否存在/可读取、直接子项和权限变化，明确扫描范围与限制。

**判断规则：** 直接子项不等于所有后代；无法读取不可显示空目录；目录通知只是复核提示。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 目录摘要/枚举完整性在主要信息未充分暴露
- 权限子项展开及scope隔离需要独立验收
- 内容版本/全目录递归历史不在实现范围

**验收：** 拒绝访问、超限、子项同名替换、目录重命名、重解析点、自己写库均不能产生无界循环。

采集定义：`pcscope.files.directory-security`。

对象类型：`file.watched_directory`。

依据：[plugins/platform-native/Sources.cs:182](../../plugins/platform-native/Sources.cs#L182)、[crates/contracts/src/adapters/file.rs](../../crates/contracts/src/adapters/file.rs)。

<a id="source-050"></a>

## 050 指定文件、ACL 与 ADS

来源：`pcscope.files.metadata-security`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 原生只读显式文件名单的身份、存在性、大小、时间、属性、所有者/DACL、可选摘要；适配文件与权限对象。

**整改后输出：** 用文件名、存在/无法检查、最近内容或权限变化展示，保留完整路径和标识于详情。

**判断规则：** 文件身份为卷+文件ID；缺失路径不等同旧文件已删除；哈希变化说明内容变动，不保证恶意或损坏。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 多权限子项父记录绑定需另审，不能把本轮账户修复当全覆盖
- 内容版本保存未实现
- 完整路径与重命名/替换/不可读的当前语义仍需统一

**验收：** 同路径新文件、移动、删除、拒绝访问、权限项增删、大小限制、监控自身数据目录排除和重扫预算。

采集定义：`pcscope.files.metadata-security`。

对象类型：`file.watched_file`、`file.acl_entry`、`file.alternate_data_stream`。

依据：[plugins/platform-native/Sources.cs:181](../../plugins/platform-native/Sources.cs#L181)、[crates/contracts/src/adapters/file.rs](../../crates/contracts/src/adapters/file.rs)。

<a id="source-051"></a>

## 051 电池状态

来源：`pcscope.hardware.battery`。路线：原生。用途：多种材料。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** Win32_Battery 采电池身份、名称、状态、化学类型、设计/满充容量、剩余电量和估算时间；adapter 仅保留名称、化学类型、电量、估算时间。

**整改后输出：** 显示“剩余电量、充电/放电/未知、估算续航”，有有效设计与满充容量时另示容量变化；台式机无电池为不适用。

**判断规则：** 按字段各自枚举和哨兵值处理，未知时间不换算成巨大分钟；容量均有效且同单位才计算比例，不能从单次比例诊断故障。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- BatteryStatus、Status 与设计/满充容量未适配；已有电量等测量缺概要与采样时效。
- 字段哨兵、零容量、多电池和无电池环境需明确验收。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 CIM PowerShell 待后端原生查询，测量降频、单飞，不由 UI 每次刷新触发。

**验收：** 充放电、无电池、双电池、未知续航及零容量均正确，百分比与估算值不被写成电池健康诊断。

采集定义：`pcscope.hardware.battery`。

对象类型：`hardware.battery`。

依据：[plugins/powershell-adapter/domains/storage.ps1:8](../../plugins/powershell-adapter/domains/storage.ps1#L8)、[crates/contracts/src/adapters/hardware.rs:412](../../crates/contracts/src/adapters/hardware.rs#L412)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-052"></a>

## 052 即插即用设备

来源：`pcscope.hardware.devices`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP05、SP06、SP07、SP09。

**现状：** Win32_PnPEntity 采设备名称、类、厂商、服务、Status 和 ConfigManagerErrorCode；adapter 已补齐 name，作为可变展示字段传递，不改变 DeviceID 身份。

**整改后输出：** 显示真实设备名称和明确影响，如“设备已禁用”“驱动配置出现问题”；连接记录、故障状态与采集失败分列。

**判断规则：** 按 PnP 配置管理器错误码字典解释；0 只说明该接口未报告配置错误。设备未再出现只能按完整枚举及类型生命周期规则表达，不等同硬件损坏。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 名称字段遗漏已修复；来源确实无名仍保持未知。已解释正常/禁用状态，其他配置错误码影响及完整场景验收仍待补齐。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 CIM PowerShell 待 SetupAPI/原生查询；事件通知若接入要合并突发插拔，保留一次有界重新枚举。

**验收：** 正常、禁用、错误码未知、拔出与读取失败五类可区分；设备名称不覆盖稳定 DeviceID。

采集定义：`pcscope.hardware.devices`。

对象类型：`hardware.device`。

依据：[plugins/powershell-adapter/collect-system.ps1:60](../../plugins/powershell-adapter/collect-system.ps1#L60)、[crates/contracts/src/adapters/hardware.rs:281](../../crates/contracts/src/adapters/hardware.rs#L281)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-053"></a>

## 053 物理磁盘健康

来源：`pcscope.hardware.disk-health`。路线：原生。用途：多种材料。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-PhysicalDisk 提供 ObjectId、友好名、介质/总线、大小、健康/运行状态及固件版本；共用 disk adapter 只保留部分磁盘字段，未保留友好名、介质和固件。

**整改后输出：** 显示物理磁盘名称、容量、介质，以及系统报告的“健康/警告/异常/未知”；说明报告来源和时间。

**判断规则：** HealthStatus 与 OperationalStatus 分别解码并可并列，多个运行状态不能只取首个；本来源不含完整 SMART 指标，不扩大为完整故障预测。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 缺友好名、介质/固件字段和多值状态解释；共享磁盘类型中的分区号等无关字段长期缺失。
- Get-PhysicalDisk 与 Get-Disk 的 ObjectId 不应假定跨 API 一定相等，需验证关联或保留独立身份。
- 原生 Storage CIM 查询迁移待办，慢硬件枚举需超时及较低频率。

**验收：** 健康/警告/失败/多个运行状态、虚拟磁盘和未知返回分别准确；不把缺 SMART 当成已全面检查。

采集定义：`pcscope.hardware.disk-health`。

对象类型：`hardware.disk`。

依据：[plugins/powershell-adapter/domains/storage.ps1:7](../../plugins/powershell-adapter/domains/storage.ps1#L7)、[crates/contracts/src/adapters/hardware.rs:148](../../crates/contracts/src/adapters/hardware.rs#L148)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-054"></a>

## 054 设备驱动绑定

来源：`pcscope.hardware.driver-bindings`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Win32_PnPSignedDriver 采 DeviceID、DeviceName、驱动版本/日期/厂商、INF、签名与签名者；adapter 按设备+版本建绑定，保留版本/日期/厂商/INF/签名布尔。

**整改后输出：** 显示“某设备使用的驱动登记、版本、厂商”及绑定变化，签名信息以来源报告表达。

**判断规则：** 同机器 DeviceID 关联已采设备，不能只按名称；该快照说明驱动绑定，不自动证明内核正在执行该文件。签名字段未知不改成未签名。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- DeviceName、Signer 已采未适配；用户难以知道绑定属于哪个设备。
- 设备+版本身份与同版本更换 INF 的变化语义需验收，历史绑定不可被误写成同时生效。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 CIM PowerShell 待原生设备/签名驱动枚举，避免为每次摘要再跑签名验证。

**验收：** 升级、同版本换包、两个同名设备与未知签名分别展示，不把登记签名报告升级成文件完整性结论。

采集定义：`pcscope.hardware.driver-bindings`。

对象类型：`hardware.driver_binding`。

依据：[plugins/powershell-adapter/domains/storage.ps1:3](../../plugins/powershell-adapter/domains/storage.ps1#L3)、[crates/contracts/src/adapters/hardware.rs:306](../../crates/contracts/src/adapters/hardware.rs#L306)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-055"></a>

## 055 驱动仓库包

来源：`pcscope.hardware.driver-packages`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 生产已由原生 DISM API 采驱动仓库，使用 published INF+仓库路径区分包，结构对齐及重复 INF 问题已修；adapter 保留包名、厂商、版本、INF 和签名结果，原始元数据仍存。

**整改后输出：** 显示“驱动仓库中的包”、厂商/版本、系统自带或第三方的明确依据及登记变化，不称当前已加载。

**判断规则：** 仓库身份保持完整复合键；有明确 DISM 字段才解释 Inbox、BootCritical 等含义，签名未知保留未知。授权不足显示此来源无法读取，不能清空仓库清单。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 默认包名仍偏 INF 技术名，友好标题可用已采厂商/类别/版本组合但不得合并同名不同仓库路径。
- Inbox、BootCritical、Class 等元数据未适配为可读重点，是否采用需按用途分级；仓库来源本身不覆盖加载状态。

**验收：** 同 INF 不同仓库包保留；普通权限拒绝与管理员成功不冲突；不反复触发 DISM 或 UAC，历史原材料可离线复验。

采集定义：`pcscope.hardware.driver-packages`。

对象类型：`hardware.driver_package`。

依据：[plugins/platform-native/Sources.cs:163](../../plugins/platform-native/Sources.cs#L163)、[plugins/powershell-adapter/domains/storage.ps1:4](../../plugins/powershell-adapter/domains/storage.ps1#L4)、[plugins/platform-native/Sources.cs:215](../../plugins/platform-native/Sources.cs#L215)、[crates/contracts/src/adapters/source_gaps.rs:118](../../crates/contracts/src/adapters/source_gaps.rs#L118)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-056"></a>

## 056 驱动服务

来源：`pcscope.hardware.driver-services`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP05、SP06、SP07、SP09。

**现状：** Win32_SystemDriver 采 Name、DisplayName、路径、服务类型、启动方式、State、Status、Started；adapter 保留内部名称、路径、类型、启动方式和 State。

**整改后输出：** 显示驱动服务的可读名称、“正在运行/已停止/启动方式”及相应变化，配置启动与实际运行分开。

**判断规则：** State 是当时系统报告的运行状态，StartMode 是配置；已限定来源和身份的驱动服务事件可直接更新其相应状态，失败不能用配置覆盖。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- DisplayName 已采未适配；Started/Status 与 State 的对应或冲突未显式处理，类型和启动枚举仍偏技术。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 CIM PowerShell 待原生服务查询，慢项不可阻塞其他对象。

**验收：** 自动启动但已停止、手动但运行、启动失败和未知状态均分开；名称不覆盖服务内部标识。

采集定义：`pcscope.hardware.driver-services`。

对象类型：`hardware.driver_service`。

依据：[plugins/powershell-adapter/domains/storage.ps1:5](../../plugins/powershell-adapter/domains/storage.ps1#L5)、[crates/contracts/src/adapters/hardware.rs:339](../../crates/contracts/src/adapters/hardware.rs#L339)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-057"></a>

## 057 BIOS 固件

来源：`pcscope.hardware.firmware`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** Win32_BIOS 采厂商、BIOS/SMBIOS 版本及发布日期；adapter 映射厂商、BIOS 版本、SMBIOS 版本和发布日期。

**整改后输出：** 显示“系统固件、厂商、版本、发布日期”，版本改变作为发现的配置变化，不误报固件更新执行时间。

**判断规则：** 发布日期是厂商元数据；首次发现/采集时间不是刷写时间。虚拟机返回的虚拟固件也按来源真实名称说明，空值未知。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 缺固定可读对象用途标题与版本/发布日期摘要，容易被通用名称或记录状态代替。
- 固件真实性或漏洞评估不在当前元数据范围；原生 CIM 迁移待办。

**验收：** 物理机、虚拟机、缺版本和版本变化均能读懂，发布日期从不显示为本机安装日期。

采集定义：`pcscope.hardware.firmware`。

对象类型：`hardware.firmware`。

依据：[plugins/powershell-adapter/domains/storage.ps1:6](../../plugins/powershell-adapter/domains/storage.ps1#L6)、[crates/contracts/src/adapters/hardware.rs:360](../../crates/contracts/src/adapters/hardware.rs#L360)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-058"></a>

## 058 本机用户账户

来源：`pcscope.identity.accounts`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-LocalUser 已采 SID、名称、enabled、最近登录、密码及到期信息；adapter 已映射名称、启用、最近登录、是否要求密码，摘要已补 enabled/last_logon，事件专有旧对象已明确区分。

**整改后输出：** 显示真实账户名称、已启用/已停用、最近登录和资料更新时间；当前账户清单与日志中的账户记录分别标明来源。

**判断规则：** 有效 SID 对齐当前用户；有明确目标 SID 的启用/停用/删除日志可直接更新对应状态并标来源时间；保留已修 NULL SID 不确认账户、Subject 不代替 Target 的规则。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 账户到期、密码到期相关事实已在原始材料但未映射；需要字段级解释，不暴露密码。
- 事件目前主要形成动作，尚未有完整按时间更新 enabled/锁定等状态事实的归并规则。
- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。

**验收：** 已知账户停用事件可更新状态；S-1-0-0、旧 Application 错分类和仅日志账户不变成已确认本机用户；旧纠正回归继续通过。

采集定义：`pcscope.identity.accounts`。

对象类型：`account.user`。

依据：[plugins/powershell-adapter/collect-system.ps1:34](../../plugins/powershell-adapter/collect-system.ps1#L34)、[crates/contracts/src/adapters/account.rs:31](../../crates/contracts/src/adapters/account.rs#L31)、[crates/contracts/src/adapters/event.rs:86](../../crates/contracts/src/adapters/event.rs#L86)、[crates/storage/src/query.rs:389](../../crates/storage/src/query.rs#L389)。

<a id="source-059"></a>

## 059 账户权限分配

来源：`pcscope.identity.assigned-rights`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP09。

**现状：** 原生 LSA 已逐 SID 读取权限列表；adapter 输出每个 SID×权限对象。多权限绑定、完整父列表缺失判断、乱序及空列表水位保护已修，不应再列为未完成。

**整改后输出：** 按账户显示已登记的登录/管理权限及增减，常见权限用中文说明所允许的操作和适用范围。

**判断规则：** 可靠 SID 关联账户名称，未知 SID 保留独立登记；仅可读、启用且完整的本账户列表确定缺失。4672 是登录特权事件，不再作为长期权限分配。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 权限来源只给 SID，不给账户显示名；需基于可靠 SID 关联已采账户/组，无法解析的保留可定位说明。
- 常见权限解释和账户详情的权限汇总尚需完整覆盖；全有效访问计算不在本来源能力内。
- 权限分配清单尚不计算对任意资源的最终有效访问权；资源 ACL、组令牌及其他限制仍可能影响实际访问。

**验收：** 一账户多权限、重排、缺失、禁用采集、遮罩列表、较新空列表后旧未知权限均回归；不复活旧 4672 权限占位对象。

采集定义：`pcscope.identity.assigned-rights`。

对象类型：`account.right_assignment`。

依据：[plugins/platform-native/Sources.cs:161](../../plugins/platform-native/Sources.cs#L161)、[plugins/powershell-adapter/domains/system.ps1:24](../../plugins/powershell-adapter/domains/system.ps1#L24)、[crates/contracts/src/adapters/account.rs:126](../../crates/contracts/src/adapters/account.rs#L126)、[crates/storage/src/projection.rs:151](../../crates/storage/src/projection.rs#L151)、[crates/storage/src/identity.rs:107](../../crates/storage/src/identity.rs#L107)。

<a id="source-060"></a>

## 060 本机用户组与成员

来源：`pcscope.identity.groups`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-LocalGroup/Member 已读组 SID、组名、成员 SID；adapter 分为组及组成员登记，父记录展开和完整成员列表缺失处理已修。

**整改后输出：** 显示组名、当前记录的成员及增减，解释成员可能继承该组权限；域或未知成员显示尚未解析名称。

**判断规则：** 成员身份为组 SID+成员 SID；可信成员增删日志可直接更新这段关系，较晚完整采样校验；采集失败或停用不等于空组。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 成员名称及组用途需从 SID 可靠关联，不能用同名猜测；不递归计算所有域组和资源有效权限。
- 成员增删事件与当前成员状态尚需按准确组/成员 SID 关联并更新。

**验收：** 一组两成员、同成员多组、组改名、空组及晚到旧列表保持独立；已修多成员投影回归不退回。

采集定义：`pcscope.identity.groups`。

对象类型：`account.group`、`account.group_membership`。

依据：[plugins/powershell-adapter/collect-system.ps1:37](../../plugins/powershell-adapter/collect-system.ps1#L37)、[crates/contracts/src/adapters/account.rs:57](../../crates/contracts/src/adapters/account.rs#L57)、[crates/storage/src/projection.rs:817](../../crates/storage/src/projection.rs#L817)、[crates/contracts/src/adapters/event.rs](../../crates/contracts/src/adapters/event.rs)。

<a id="source-061"></a>

## 061 交互式终端会话

来源：`pcscope.identity.interactive-sessions`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP09。

**现状：** 原生 WTS 已采 session_id、站点、state_code、用户/域和身份查询错误；adapter 保留读取失败和 state 数字。

**整改后输出：** 显示哪个用户的会话处于已连接、已断开等状态，用户读不到时明确资料不可读；与锁定/解锁事件分别呈现。

**判断规则：** 按 WTS 状态枚举解码；明确会话事件可更新锁定/连接状态。会话 ID 需结合机器和启动/会话代际，避免 ID 复用串历史。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前 identity 仅安装 scope+session_id，没有启动/会话代际。
- WTS 枚举、日志锁定/重连与当前状态尚无统一读模型；WTS 状态不自动等于锁屏状态。

**验收：** 同 ID 在新启动周期不继承旧用户或锁定状态；身份查询失败仍保留会话可读状态，不显示无人登录。

采集定义：`pcscope.identity.interactive-sessions`。

对象类型：`runtime.terminal_session`。

依据：[plugins/platform-native/Sources.cs:162](../../plugins/platform-native/Sources.cs#L162)、[plugins/powershell-adapter/domains/system.ps1:23](../../plugins/powershell-adapter/domains/system.ps1#L23)、[crates/contracts/src/adapters/runtime.rs:110](../../crates/contracts/src/adapters/runtime.rs#L110)。

<a id="source-062"></a>

## 062 登录会话

来源：`pcscope.identity.logon-sessions`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Win32_LogonSession 已采 LogonId、LogonType、StartTime、AuthenticationPackage；adapter 按原始登录 ID 建会话，当前无 boot_id。

**整改后输出：** 按交互、服务、网络等登录类型说明会话用途和最近状态，将用户交互、服务运行和网络访问分别归类。

**判断规则：** 将快照十进制 LogonId 与日志十六进制 LogonId 规范为同值并隔离启动周期；可靠登录/注销事件可直接决定该会话状态，无需额外轮询证明。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- snapshot adapter 仍保留十进制 ID，而事件 ID 为十六进制；缺启动周期，不能可靠归并或防跨启动复用。
- 4672 缺 boot 时保持未归属事件已修；本来源需提供可靠 boot/session 身份后才能参与该关联。

**验收：** 同一启动中的十进制/十六进制登录 ID 合一，跨启动分离；缺 boot 的特权事件仍可展示但不强行连会话。

采集定义：`pcscope.identity.logon-sessions`。

对象类型：`runtime.logon_session`。

依据：[plugins/powershell-adapter/domains/system.ps1:22](../../plugins/powershell-adapter/domains/system.ps1#L22)、[crates/contracts/src/adapters/runtime.rs:92](../../crates/contracts/src/adapters/runtime.rs#L92)、[crates/contracts/src/adapters/event.rs:131](../../crates/contracts/src/adapters/event.rs#L131)。

<a id="source-063"></a>

## 063 计算机身份与域信息

来源：`pcscope.identity.machine`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Win32_ComputerSystem 和 Win32_ComputerSystemProduct 已采名称、域/工作组、角色、厂商、型号和 UUID；adapter 按安装内机器身份汇合两条来源记录。

**整改后输出：** 显示计算机名称、域或工作组及变化，说明域/名称变更可能影响登录和网络识别。

**判断规则：** 两条快照按机器/安装身份合并，缺字段不清除另一条已知值；域成员身份依据来源的域/角色字段解释，不由域字符串单独猜测。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- PartOfDomain 已采但未映射，现只有 domain/role 容易混淆工作组。
- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。

**验收：** 计算机改名保持同一安装对象；同名工作组与域明确区分，重装后不沿用旧机器记录。

采集定义：`pcscope.identity.machine`。

对象类型：`account.machine_identity`。

依据：[plugins/powershell-adapter/domains/system.ps1:1](../../plugins/powershell-adapter/domains/system.ps1#L1)、[crates/contracts/src/adapters/account.rs:161](../../crates/contracts/src/adapters/account.rs#L161)。

<a id="source-064"></a>

## 064 用户配置文件

来源：`pcscope.identity.profiles`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Win32_UserProfile 已采 SID、路径、Loaded、Special、Status、LastUseTime；adapter 映射路径、已加载和最近使用。

**整改后输出：** 显示哪位账户的配置文件、目前是否加载和最近使用；路径默认简化，完整路径可展开定位。

**判断规则：** 通过 SID 对齐账户；Loaded 只说明该配置文件处于加载状态，不直接解释成有人坐在电脑前；日志可补登录关联。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- Special/Status 原始状态未映射，loaded/last_use 未统一进摘要。
- SID 对应显示名和旧配置文件/已删账户区分缺统一关联。
- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。

**验收：** 已加载系统配置文件不显示为交互登录；用户名变化仍按 SID 对齐，读取失败不冒充未加载。

采集定义：`pcscope.identity.profiles`。

对象类型：`account.user_profile`。

依据：[plugins/powershell-adapter/domains/system.ps1:21](../../plugins/powershell-adapter/domains/system.ps1#L21)、[crates/contracts/src/adapters/account.rs:104](../../crates/contracts/src/adapters/account.rs#L104)。

<a id="source-065"></a>

## 065 代理、远程桌面与 VPN

来源：`pcscope.network.access`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已读当前用户代理注册表、机器远程桌面允许开关、当前用户 VPN 名称/连接状态/类型。Registry 当前保留所有值，ProxyServer 可在原始证据中，不是被旧 allowlist 丢弃。

**整改后输出：** 分别显示用户代理设置、是否允许远程桌面、VPN 当前连接状态；解释设置变更影响联网路径或远程访问入口。

**判断规则：** ProxyEnable 按 DWORD 解码，fDenyTSConnections 按反向开关解码；VPN 直接用 ConnectionStatus。配置可确定配置状态，不等同于连接已发生；连接日志可直接补运行事实。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 代码路径已证实的关联风险，尚未在本轮复现数据损坏：一个父记录展开多个对象，但 projection.apply 目前仅对账户权限/组成员启用按子身份绑定；此来源仍可能复用父绑定并跳过其他子项。
- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。
- 代理 AutoDetect/自动配置地址、VPN ServerAddress/AllUserConnection 等字段未完整映射；VPN emitter 尚未采 ServerAddress。
- 远程桌面只有注册配置，未结合服务/监听当前状态；需分别展示，不能默认判为可从远端访问。

**验收：** 一个代理键含多个值全部保留；不同用户同名 VPN 不合并；VPN 断开、代理关闭、RDP 禁止均各自清楚且不互相推导。

采集定义：`pcscope.network.access`。

对象类型：`network.proxy_configuration`、`network.vpn_configuration`、`network.remote_access_configuration`。

依据：[plugins/powershell-adapter/domains/network.ps1:22](../../plugins/powershell-adapter/domains/network.ps1#L22)、[crates/storage/src/projection.rs:151](../../crates/storage/src/projection.rs#L151)、[crates/contracts/src/adapters/network.rs:470](../../crates/contracts/src/adapters/network.rs#L470)、[crates/contracts/src/adapters/network.rs:511](../../crates/contracts/src/adapters/network.rs#L511)、[crates/contracts/src/adapters/network.rs:589](../../crates/contracts/src/adapters/network.rs#L589)。

<a id="source-066"></a>

## 066 网络适配器

来源：`pcscope.network.adapters`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-NetAdapter -IncludeHidden 已按 InterfaceGuid 采名称、接口描述、Status、MAC、LinkSpeed、DriverVersion；adapter 已映射状态和链路速率。

**整改后输出：** 显示网卡名称、已连接/未连接/已禁用及链路协商速率，说明当前网卡连接状态对联网的影响。

**判断规则：** 按 GUID 保持改名后的身份，直接解码 Status 和 LinkSpeed；连接事件可补变化时点，展示最近检查时间。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 未采 InterfaceIndex，无法给 IP/路由/DNS 来源建立可靠 index→GUID 对照；hidden 字段未由 emitter 输出，不能因 IncludeHidden 就标每张网卡隐藏。
- 已有字段未进入按类型选择的状态摘要，列表/详情/时间线尚未统一解释其状态、采样时间及影响。
- LinkSpeed 是链路协商速率；当前来源未测量实际吞吐量或下载速度。

**验收：** 网卡改名仍同对象；连接/断开/禁用正确区分；接口索引变化不串联另一网卡。

采集定义：`pcscope.network.adapters`。

对象类型：`network.adapter`。

依据：[plugins/powershell-adapter/collect-system.ps1:49](../../plugins/powershell-adapter/collect-system.ps1#L49)、[crates/contracts/src/adapters/network.rs:188](../../crates/contracts/src/adapters/network.rs#L188)。

<a id="source-067"></a>

## 067 IP 地址

来源：`pcscope.network.addresses`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采 interface_index/alias、地址、前缀长度、地址族、来源和 address_state；adapter 已映射地址状态，但 interface_guid 明确缺失。

**整改后输出：** 在所属网卡下显示 IPv4/IPv6 地址、可用/试探/冲突等状态，以及地址变更对通信的影响。

**判断规则：** 依据 AddressState 的明确枚举解释；以同次接口索引→GUID 对照补稳定身份，不把接口别名当 GUID；地址冲突状态无需另一项检测才能展示。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 所需 interface_guid 尚未采集/关联，provisional 键可能把不同接口相同地址/前缀并在一起（代码路径风险，待隔离复现）。
- prefix_origin/suffix_origin 已采未映射，无法说明 DHCP/手工等来源。

**验收：** 两张网卡相同地址和前缀不合并；明确冲突/试探状态显示用户影响，正常空列表与采集失败区分。

采集定义：`pcscope.network.addresses`。

对象类型：`network.address_configuration`。

依据：[plugins/powershell-adapter/collect-system.ps1:52](../../plugins/powershell-adapter/collect-system.ps1#L52)、[crates/contracts/src/adapters/network.rs:213](../../crates/contracts/src/adapters/network.rs#L213)。

<a id="source-068"></a>

## 068 DNS 缓存

来源：`pcscope.network.dns-cache`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采缓存 entry/name/type/data/status/section；adapter 按 name+type 建对象并仅映射 data 状态，没有 TTL 字段采集。

**整改后输出：** 显示缓存的名称解析地址或别名及最近变化，帮助解释本机当前保存了哪些解析结果。

**判断规则：** 同一名称/类型可能同时有多结果，应表示结果集合或独立记录；DNS 查询日志可展示真实查询及结果，无需再查询外网验证。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前 key 不含 data/entry，同名同类型多答案可能合并为冲突或被同快照 claim 跳过（代码路径风险）；需集合/多记录身份。
- status/section 未映射、TTL 未采；不能推断精确过期时刻或全量查询历史。
- 缓存消失的原因未由此来源记录，不能单凭条目不再出现判定人为删除或网站不可访问。

**验收：** 同名两个 A 答案均可展示，TTL 不存在时不编造到期时间；真实查询事件与缓存观察分别标时。

采集定义：`pcscope.network.dns-cache`。

对象类型：`network.dns_cache_entry`。

依据：[plugins/powershell-adapter/domains/network.ps1:25](../../plugins/powershell-adapter/domains/network.ps1#L25)、[crates/storage/src/projection.rs:205](../../crates/storage/src/projection.rs#L205)、[crates/contracts/src/adapters/network.rs:349](../../crates/contracts/src/adapters/network.rs#L349)。

<a id="source-069"></a>

## 069 DNS 配置

来源：`pcscope.network.dns-configuration`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已读接口 DNS 服务器列表，以及机器 Tcpip Parameters 的域/搜索后缀配置；adapter 映射 servers/search_list，但接口 GUID 缺失，机器设置也被放进接口型 DNS 对象。

**整改后输出：** 按网卡显示 DNS 服务器，单独显示机器搜索后缀；解释解析路径、后缀变更对名称查找的影响。

**判断规则：** 接口用同次 index→GUID 关联；机器搜索列表使用机器配置身份，不虚造接口或地址族。直接按已读配置得出配置结论。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 接口对象缺 interface_guid，机器级配置缺 interface_guid/address_family；不同接口同地址族可能合并（代码路径风险）。
- Domain/NV Domain/UseDomainNameDevolution 已采但未映射；需要与接口服务器分层。

**验收：** 两个接口相同地址族分别保留，机器搜索后缀不归到任一网卡；服务器顺序变化可追踪。

采集定义：`pcscope.network.dns-configuration`。

对象类型：`network.dns_configuration`。

依据：[plugins/powershell-adapter/domains/network.ps1:2](../../plugins/powershell-adapter/domains/network.ps1#L2)、[crates/contracts/src/adapters/network.rs:297](../../crates/contracts/src/adapters/network.rs#L297)、[crates/contracts/src/adapters/network.rs:325](../../crates/contracts/src/adapters/network.rs#L325)。

<a id="source-070"></a>

## 070 防火墙配置文件

来源：`pcscope.network.firewall-profiles`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采域/专用/公用防火墙配置集开关、入站/出站默认动作及日志选项；adapter 映射开关、默认动作、日志路径。

**整改后输出：** 显示各配置集是否启用防火墙、默认允许/阻止方向，以及当前网络使用哪个配置集。

**判断规则：** 以 Enabled/default actions 原始枚举解释配置；网络配置文件可说明当前适用配置集。过滤事件可直接记录某次允许/阻止。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- Allow*Rules、日志开关/大小已采未映射；尚未将当前网络配置集关联到相应防火墙策略。
- 已有字段未进入按类型选择的状态摘要，列表/详情/时间线尚未统一解释其状态、采样时间及影响。

**验收：** 公用关闭不被另一个已开启配置集掩盖；未知枚举不默认开启；某次拦截事件无需二次探测即可说明。

采集定义：`pcscope.network.firewall-profiles`。

对象类型：`network.firewall_profile`。

依据：[plugins/powershell-adapter/domains/network.ps1:6](../../plugins/powershell-adapter/domains/network.ps1#L6)、[crates/contracts/src/adapters/network.rs:609](../../crates/contracts/src/adapters/network.rs#L609)。

<a id="source-071"></a>

## 071 防火墙规则及过滤条件

来源：`pcscope.network.firewall-rules`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP05、SP06、SP07、SP09。

**现状：** ActiveStore 规则已关联 Port/Address/Application/Service/Interface/InterfaceType/Security 七类过滤条件，关联缺失则来源报 gap；adapter 只映射 Name、方向、动作、启用、profile。

**整改后输出：** 显示规则可读名称、开关、允许/阻止哪个程序或端口/方向；说明适用范围，保持具体拦截事实与规则配置的区别。

**判断规则：** 规则身份用 ActiveStore+Name；直接根据 filters 形成有边界的规则摘要，DisplayName 只作显示；真实拦截日志可更新该次事件状态。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- DisplayName、完整 filters、PrimaryStatus/Status 已采但未进字段模型，默认名称可能是技术 GUID。
- InstanceID 字段未由 emitter 输出，adapter 对该字段始终 not_provided；不要将缺此附加字段误判为整个规则身份不完整。

**验收：** 一条带程序、端口、远端限制规则完整解释作用范围；少任何过滤关联显示采集缺口，不能变成允许所有端口。

采集定义：`pcscope.network.firewall-rules`。

对象类型：`network.firewall_rule`。

依据：[plugins/powershell-adapter/domains/network.ps1:7](../../plugins/powershell-adapter/domains/network.ps1#L7)、[crates/contracts/src/adapters/network.rs:641](../../crates/contracts/src/adapters/network.rs#L641)。

<a id="source-072"></a>

## 072 hosts 静态映射

来源：`pcscope.network.hosts`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已安全读取 hosts 有效条目，去注释、校验地址、逐名称输出；adapter 身份为 address+name，source item_id 使用行号/name。

**整改后输出：** 显示本机将哪个名称固定指向哪个地址、新增/修改/撤销，说明可能改变本机名称解析。

**判断规则：** 根据条目本身确定静态映射；地址不需要联网验证。显示身份与行号分离，行移动不当成映射改变。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- source binding 优先且 item_id 含行号；同一行同名改地址可能沿旧对象身份、移动行可能改变绑定，需验证并明确映射身份/变化语义。
- 未有按完整安全快照移除条目的来源级状态归并。

**验收：** 只移动行号不产生业务变化；同名地址改变保留前后值；非法或过大文件报失败不当作空 hosts。

采集定义：`pcscope.network.hosts`。

对象类型：`network.hosts_entry`。

依据：[plugins/powershell-adapter/domains/network.ps1:26](../../plugins/powershell-adapter/domains/network.ps1#L26)、[crates/storage/src/identity.rs:81](../../crates/storage/src/identity.rs#L81)、[crates/contracts/src/adapters/network.rs:377](../../crates/contracts/src/adapters/network.rs#L377)。

<a id="source-073"></a>

## 073 IPsec 规则

来源：`pcscope.network.ipsec`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP05、SP06、SP07、SP09。

**现状：** ActiveStore 已采规则名称、显示名、Enabled/Profile/Mode、双向安全要求及策略来源；adapter 只映射 Name、Mode、PolicyStoreSource。

**整改后输出：** 显示 IPsec 规则开关、适用配置集、传输/隧道模式及认证/加密要求，说明哪些流量被配置为需要安全保护。

**判断规则：** 依据已有 Enabled、InboundSecurity、OutboundSecurity 等配置给结论；另有协商成功日志时可直接作为已建立安全关联的事实。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 已采启用、显示名、配置集和双向安全要求未映射。
- 当前来源未枚举实际安全关联/协商状态；该能力需新来源或相关日志映射，而非从 Mode 推断。
- 此来源是规则配置，未采集当前安全关联；具体通信是否已加密需对应的协商或连接记录。

**验收：** 禁用 IPsec 规则不显示保护生效；入站要求与出站要求分别解释，未知模式保留原值。

采集定义：`pcscope.network.ipsec`。

对象类型：`network.ipsec_rule`。

依据：[plugins/powershell-adapter/domains/network.ps1:21](../../plugins/powershell-adapter/domains/network.ps1#L21)、[crates/contracts/src/adapters/network.rs:674](../../crates/contracts/src/adapters/network.rs#L674)。

<a id="source-074"></a>

## 074 SMB 驱动器映射

来源：`pcscope.network.mapped-drives`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-SmbMapping 已采本地盘符、远程路径及 Status，adapter 保留用户 scope 与映射二元身份。

**整改后输出：** 显示网络驱动器盘符、共享位置和连接状态；断开可能使该盘文件暂不可用。

**判断规则：** 按原始 Status 判断映射状态；同用户相同本地/远端映射关联，可信连接/断开日志可补时点；不得据无映射推断共享服务器离线。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。
- 映射状态枚举和用户归属摘要尚未统一；仅采当前运行账户可见映射，不覆盖未加载用户会话。

**验收：** 两个用户同盘符映射不串联；已断开与来源读取失败有不同说明。

采集定义：`pcscope.network.mapped-drives`。

对象类型：`network.smb_mapping`。

依据：[plugins/powershell-adapter/domains/network.ps1:24](../../plugins/powershell-adapter/domains/network.ps1#L24)、[crates/contracts/src/adapters/network.rs:751](../../crates/contracts/src/adapters/network.rs#L751)。

<a id="source-075"></a>

## 075 网络连接配置文件

来源：`pcscope.network.profiles`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采接口索引、网络名称、NetworkCategory 与 IPv4/IPv6Connectivity；adapter 完整映射这些状态码，identity 为 index/name。

**整改后输出：** 显示公用/专用/域网络及 IPv4、IPv6 连通范围，解释对共享/防火墙适用策略的影响。

**判断规则：** 按具体 connectivity 枚举解释，不将 IPv4 可用外推为 IPv6 可用；来源明确的互联网状态可以直接展示，不强制额外联网探测。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 稳定接口关联缺 GUID；网络改名/索引复用可能影响身份，需用可靠接口与配置文件标识关联。
- 状态码未按类型译为可读连通范围。

**验收：** IPv4 Internet 与 IPv6 LocalNetwork 分别呈现；网卡重命名不把旧连通状态传给新接口。

采集定义：`pcscope.network.profiles`。

对象类型：`network.connection_profile`。

依据：[plugins/powershell-adapter/domains/network.ps1:3](../../plugins/powershell-adapter/domains/network.ps1#L3)、[crates/contracts/src/adapters/network.rs:275](../../crates/contracts/src/adapters/network.rs#L275)。

<a id="source-076"></a>

## 076 路由表

来源：`pcscope.network.routes`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采接口索引/别名、目的前缀、下一跳、RouteMetric、Protocol、AddressFamily、PolicyStore、Publish；adapter 映射目的/下一跳/度量/策略来源，GUID 缺失。

**整改后输出：** 显示流量到某网段/默认路由经哪张网卡和网关，路由变化可能改变出口或使某网络不可达。

**判断规则：** 直接解读路由条目；同次接口索引→GUID 对齐，若判断优先出口还需接口度量等完整优先级，不能只用 RouteMetric 排全局优先级。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- interface_guid 未采/关联，可能合并不同接口同目的/下一跳（代码路径风险）。
- 类型身份不含策略存储而 emitter item_id 含 PolicyStore；需明确不同存储是否为同一有效路由及来源。
- InterfaceMetric 未采，其他已采字段尚未完整映射。

**验收：** 两接口相同默认网关不合并；无接口度量时只显示路由度量，不冒称唯一首选出口。

采集定义：`pcscope.network.routes`。

对象类型：`network.route`。

依据：[plugins/powershell-adapter/domains/network.ps1:1](../../plugins/powershell-adapter/domains/network.ps1#L1)、[crates/contracts/src/adapters/network.rs:246](../../crates/contracts/src/adapters/network.rs#L246)。

<a id="source-077"></a>

## 077 SMB 客户端会话

来源：`pcscope.network.share-sessions`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-SmbSession 已采 SessionId、客户端、用户、NumOpens、Dialect、Encrypted；adapter 映射客户/用户/打开数/协议/加密，但 identity 要求缺失 server 且不使用 SessionId。

**整改后输出：** 显示本机共享被哪个客户端/账户连接、是否加密和打开文件数，说明当前仍有远程访问活动。

**判断规则：** 以来源确切服务器 scope+SessionId/代际定义会话，不以客户+用户把多个会话合成一个；Encrypted 直接解释该会话。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前 key 缺 server 且丢 SessionId，同客户端同用户多会话可能合并（代码路径风险，待复现）。
- 缺会话代际、断开事实和打开数/加密状态的可读摘要。

**验收：** 同一客户端/用户两个 SessionId 分开，Encrypted=false 显示未加密，缺字段不推断未加密。

采集定义：`pcscope.network.share-sessions`。

对象类型：`network.smb_session`。

依据：[plugins/powershell-adapter/domains/network.ps1:27](../../plugins/powershell-adapter/domains/network.ps1#L27)、[crates/contracts/src/adapters/network.rs:720](../../crates/contracts/src/adapters/network.rs#L720)。

<a id="source-078"></a>

## 078 SMB 共享与访问授权

来源：`pcscope.network.shares`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采共享名称、路径、说明、EncryptData、Special 和共享 ACL；adapter 映射名称/路径/ACL/特殊共享。

**整改后输出：** 显示共享位置、哪些账户被授予/拒绝共享权限、是否要求加密，及配置变化影响。

**判断规则：** 共享 ACL 本身可确定共享层授权；明确这是共享权限，文件系统 ACL 和账户令牌可能进一步限制访问。访问日志可直接说明一次访问结果。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- description/encrypt_data 已采未映射；共享权限缺可读主体与权利解释。
- 未计算合并文件 ACL 后的每用户有效访问；需要该能力时单独设计，不能把共享 ACL 当全部访问权。

**验收：** 共享允许但文件 ACL 不明时只说明共享层允许；加密要求关闭不显示所有连接都明文。

采集定义：`pcscope.network.shares`。

对象类型：`network.smb_share`。

依据：[plugins/powershell-adapter/domains/network.ps1:23](../../plugins/powershell-adapter/domains/network.ps1#L23)、[crates/contracts/src/adapters/network.rs:694](../../crates/contracts/src/adapters/network.rs#L694)。

<a id="source-079"></a>

## 079 TCP 连接与监听

来源：`pcscope.network.tcp`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-NetTCPConnection 已采两端地址/端口、PID、CreationTime、State；adapter 从 TCP 来源确定协议，并以七元组建身份，状态数字原样保留。

**整改后输出：** 显示程序/本地端口在监听、连接中或已建立连接及对端，解释当前网络活动，技术地址可展开。

**判断规则：** 直接按 TCP State 解码；结合 PID+程序建立时间和端点创建时间做进程关联，明确连接日志可直接记录连接成功/阻止。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- TCP 状态数字尚无统一类型解码，PID 未与可靠进程代际关联。
- 采样间短连接可能未出现；没有更新的结束证据时应显示最近观察时间而非长期持续在线。

**验收：** Listen 与 Established、TimeWait 分开；复用 PID/四元组时旧程序不接管新连接；短连接日志仍可展示。

采集定义：`pcscope.network.tcp`。

对象类型：`network.endpoint`。

依据：[plugins/powershell-adapter/domains/network.ps1:4](../../plugins/powershell-adapter/domains/network.ps1#L4)、[crates/contracts/src/adapters/network.rs:410](../../crates/contracts/src/adapters/network.rs#L410)、[crates/contracts/src/adapters/network.rs:446](../../crates/contracts/src/adapters/network.rs#L446)。

<a id="source-080"></a>

## 080 UDP 端点

来源：`pcscope.network.udp`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-NetUDPEndpoint 已采本地绑定地址/端口、PID、CreationTime；adapter 当前连 UDP 协议也置缺失，并沿用要求远端地址/端口的 TCP 式端点键。

**整改后输出：** 显示哪个程序绑定了 UDP 本地地址和端口、最近检查时间，帮助定位本机可接收 UDP 数据的端点。

**判断规则：** 来源本身确定 UDP 协议，无需额外证明；应采用本地 UDP 绑定身份，以 PID/建立时间避免复用，远端在具体通信日志出现时归到该次活动。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 当前协议/远端组件缺失使所有 UDP 记录身份降为 provisional；需协议专用身份而非填虚假远端。
- 缺可靠进程代际关联和可读绑定状态。
- 本地绑定快照没有远端通信记录；远端地址和发送/接收结果需由具体通信来源提供。

**验收：** 只提供本地端的 UDP 记录得到稳定绑定身份和 UDP 标签，绝不虚造远端连接。

采集定义：`pcscope.network.udp`。

对象类型：`network.endpoint`。

依据：[plugins/powershell-adapter/domains/network.ps1:5](../../plugins/powershell-adapter/domains/network.ps1#L5)、[crates/contracts/src/adapters/network.rs:410](../../crates/contracts/src/adapters/network.rs#L410)、[crates/contracts/src/adapters/network.rs:453](../../crates/contracts/src/adapters/network.rs#L453)。

<a id="source-081"></a>

## 081 WinRM 配置

来源：`pcscope.network.winrm-configuration`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已读 WSMAN 注册表指定树深度内的认证/加密/兼容监听设置，保留全值表；adapter 映射 path/service/values，不连接远程机器。

**整改后输出：** 显示远程管理的认证与加密设置及变化；将配置允许、服务运行和实际监听分别说明。

**判断规则：** 根据已读具体设置解释配置；服务或明确监听/连接日志可更新对应运行状态，不因注册表键存在就称 WinRM 正在监听。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 远程访问身份没有 registry view 维度，需确认视图等价或显式区分。
- values 未转类型化设置；当前未完整读取实际 WSMan Listener 配置和服务/端口对应。

**验收：** Basic/AllowUnencrypted 等开关可读且相互独立；仅配置存在时不显示远程连接已建立。

采集定义：`pcscope.network.winrm-configuration`。

对象类型：`network.remote_access_configuration`。

依据：[plugins/powershell-adapter/domains/network.ps1:28](../../plugins/powershell-adapter/domains/network.ps1#L28)、[crates/contracts/src/adapters/network.rs:564](../../crates/contracts/src/adapters/network.rs#L564)、[crates/contracts/src/adapters/network.rs:775](../../crates/contracts/src/adapters/network.rs#L775)。

<a id="source-082"></a>

## 082 UAC、资源管理器与软件限制策略

来源：`pcscope.policy.configuration`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已读取 HKLM/HKCU CurrentVersion Policies 和机器 Safer 树的全部脱敏配置值；adapter 按每个值展开，Safer 归软件限制，其余整个 Policies 树都归 UAC。

**整改后输出：** 分别显示管理员授权策略、软件限制和其他系统策略的实际设置/变化；解释关闭提示、改变限制等对用户操作的影响。

**判断规则：** 以精确路径+值名决定策略语义、按有效枚举解释；缺值表示未在该位置配置，不默认关。全值证据已采，不必再扫描同一注册表。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 代码路径已证实的关联风险，尚未在本轮复现数据损坏：一个父记录展开多个对象，但 projection.apply 目前仅对账户权限/组成员启用按子身份绑定；此来源仍可能复用父绑定并跳过其他子项。
- 非 Safer 值一概归 UAC 的路由过宽，会把无关 Explorer 等策略误标管理员授权设置；需精确规则和通用未解释策略类型。
- 枚举/位标记及机器/用户策略优先级未解释；不能只取任一注册值宣称最终生效策略。

**验收：** Policies/System 的 EnableLUA 与 Explorer 其他策略分类正确；同键多值全部保留，未配置与明确0分开。

采集定义：`pcscope.policy.configuration`。

对象类型：`policy.uac_policy`、`policy.software_restriction_policy`。

依据：[plugins/powershell-adapter/domains/security.ps1:27](../../plugins/powershell-adapter/domains/security.ps1#L27)、[crates/contracts/src/adapters/policy.rs:142](../../crates/contracts/src/adapters/policy.rs#L142)、[crates/contracts/src/adapters/policy.rs:180](../../crates/contracts/src/adapters/policy.rs#L180)、[plugins/powershell-adapter/collect-domain.ps1:36](../../plugins/powershell-adapter/collect-domain.ps1#L36)。

<a id="source-083"></a>

## 083 电源计划与休眠配置

来源：`pcscope.power.configuration`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 采 Win32_PowerPlan 的 GUID、ElementName、IsActive、Description，并读取 Power 注册表的 HibernateEnabled 等；adapter 正确保留计划名称和 active，却明确跳过休眠/待机注册表对象，power_source 未提供。

**整改后输出：** 电源计划显示原名和“正在使用/未使用”；休眠和相关待机配置另列可解释设置，不与计划启用混为一谈。

**判断规则：** IsActive 直接判该计划当时是否选中，false 不是计划被禁用；名称允许用户自定义，不能见 Balanced 就认作当前方案。休眠值按确切键及 Windows 适用性解释；缺 AC/DC 来源不猜电源接入状态。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 计划 active 已知却未进入列表摘要，造成名称、类型和记录状态混淆。
- 已采 HibernateEnabled、HibernateEnabledDefault、CsEnabled、PlatformAoAcOverride 未适配，需独立配置类型或明确字段；不能把配置位当成硬件睡眠能力。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 PowerShell 待原生 Power API/注册表迁移，范围包括方案与休眠设置而非只迁方案。

**验收：** 两个计划仅一个 active、自定义 Balanced 名称、休眠开/关/未知和不同 Windows 待机配置分别正确；选中状态不显示成“最近仍在”。

采集定义：`pcscope.power.configuration`。

对象类型：`policy.power_plan`。

依据：[plugins/powershell-adapter/domains/storage.ps1:9](../../plugins/powershell-adapter/domains/storage.ps1#L9)、[crates/contracts/src/adapters/policy.rs:466](../../crates/contracts/src/adapters/policy.rs#L466)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-084"></a>

## 084 当前用户证书

来源：`pcscope.security.certificates.currentuser`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已读 当前用户 Root/CA/My/TrustedPublisher/Disallowed/TrustedPeople 六个证书库，采存储区、指纹、主题、颁发者、有效期及有无私钥；adapter 这些字段已映射。

**整改后输出：** 显示证书用途名称、所在库、尚未生效/有效期内/已过期，以及新增/移除；失效可能影响依赖它的认证或签名。

**判断规则：** 由 not_before/not_after 与当前时间直接计算尚未生效、有效期内、已过期；以用户/机器+库+指纹定位，主题用于显示用途和名称。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。
- 尚缺证书库友好名称、有效期摘要及证书到具体服务/应用用途的可靠关联。
- 链、吊销、用途匹配验证未采，不能称证书可信/应用一定正常；这些是明确未覆盖能力。
- 当前来源明确 trust=not evaluated，未执行完整证书链与撤销检查；时间有效性和证书链信任状态需要分别展示。

**验收：** 同证书在两个库或两个用户下分别定位；刚过期只改时间有效性，不擅称被吊销；私钥只显示存在标志。

采集定义：`pcscope.security.certificates.currentuser`。

对象类型：`security.certificate_registration`。

依据：[plugins/powershell-adapter/domains/security.ps1:20](../../plugins/powershell-adapter/domains/security.ps1#L20)、[crates/contracts/src/adapters/security.rs:532](../../crates/contracts/src/adapters/security.rs#L532)、[crates/storage/src/identity.rs:81](../../crates/storage/src/identity.rs#L81)。

<a id="source-085"></a>

## 085 计算机证书

来源：`pcscope.security.certificates.localmachine`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已读 机器 Root/CA/My/TrustedPublisher/Disallowed/TrustedPeople 六个证书库，采存储区、指纹、主题、颁发者、有效期及有无私钥；adapter 这些字段已映射。

**整改后输出：** 显示证书用途名称、所在库、尚未生效/有效期内/已过期，以及新增/移除；失效可能影响依赖它的认证或签名。

**判断规则：** 由 not_before/not_after 与当前时间直接计算尚未生效、有效期内、已过期；以用户/机器+库+指纹定位，主题用于显示用途和名称。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。
- 尚缺证书库友好名称、有效期摘要及证书到具体服务/应用用途的可靠关联。
- 链、吊销、用途匹配验证未采，不能称证书可信/应用一定正常；这些是明确未覆盖能力。
- 当前来源明确 trust=not evaluated，未执行完整证书链与撤销检查；时间有效性和证书链信任状态需要分别展示。

**验收：** 同证书在两个库或两个用户下分别定位；刚过期只改时间有效性，不擅称被吊销；私钥只显示存在标志。

采集定义：`pcscope.security.certificates.localmachine`。

对象类型：`security.certificate_registration`。

依据：[plugins/powershell-adapter/domains/security.ps1:20](../../plugins/powershell-adapter/domains/security.ps1#L20)、[crates/contracts/src/adapters/security.rs:532](../../crates/contracts/src/adapters/security.rs#L532)、[crates/storage/src/identity.rs:81](../../crates/storage/src/identity.rs#L81)。

<a id="source-086"></a>

## 086 Defender 检测记录

来源：`pcscope.security.detections`。路线：原生。用途：变化与事件。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-MpThreatDetection 已采 DetectionID/ThreatID、发现与状态变化时间、ActionSuccess 和处理/威胁状态码；当前 adapter 明确 Skipped，无可用对象或动作输出，原始证据仍存。

**整改后输出：** 显示已检测到的威胁、处理是否成功、仍需处理/已处理和最近变化，说明安全影响并给出记录来源。

**判断规则：** 将保留的检测记录按 DetectionID 和来源状态时间归并为事件/处置状态；明确成功/失败字段可直接作结论，不要求二次扫描；后来不再列出不能等同已解决。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- action_evidence 映射存在但 detections() 没产出动作，检测内容未进入时间线和状态读模型。
- ThreatID 到人类名称与状态枚举解释缺失；来源不含威胁路径时不能猜受影响文件。

**验收：** 一个检测从发现到处理成功生成连续状态，重复快照幂等；失败明确显示，记录过期不伪造已解决。

采集定义：`pcscope.security.detections`。

对象类型：`security.protection`。

依据：[plugins/powershell-adapter/domains/security.ps1:4](../../plugins/powershell-adapter/domains/security.ps1#L4)、[crates/contracts/src/adapters/security.rs:417](../../crates/contracts/src/adapters/security.rs#L417)、[model/source-map.cjs](../../model/source-map.cjs)。

<a id="source-087"></a>

## 087 Device Guard / VBS

来源：`pcscope.security.deviceguard`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** 已采 VBS 状态、配置/运行服务列表、可用/必需安全能力、代码完整性策略状态；adapter 映射前四项中的 VBS/配置服务/运行服务/可用属性。

**整改后输出：** 分别显示已配置和正在运行的安全隔离能力，说明内存完整性等保护是否实际运行。

**判断规则：** 按官方 VBS 与服务枚举逐项解码；配置列表与运行列表不同可直接说明已配置但未运行，不自动推断故障原因。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- RequiredSecurityProperties、CodeIntegrityPolicyEnforcementStatus 已采未映射。
- 数字/数组仍技术化；缺字段与0含义须区分，不能把不支持或无权限当关闭。

**验收：** 已配置未运行、确实未启用、能力不支持和读取失败四种情况各自显示，不用列表非空当所有保护开启。

采集定义：`pcscope.security.deviceguard`。

对象类型：`security.device_guard_configuration`。

依据：[plugins/powershell-adapter/domains/security.ps1:5](../../plugins/powershell-adapter/domains/security.ps1#L5)、[crates/contracts/src/adapters/security.rs:468](../../crates/contracts/src/adapters/security.rs#L468)。

<a id="source-088"></a>

## 088 BitLocker 卷状态

来源：`pcscope.security.encryption`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-BitLockerVolume 已采卷状态、保护状态、算法、保护器类型，以及加密百分比/锁定状态；adapter 映射前四项，identity 仍以盘符或挂载点为 native_id。

**整改后输出：** 分别显示加密进度、保护开关和是否锁定，明确说明已加密但保护关闭等组合状态及其影响。

**判断规则：** 直接使用 VolumeStatus、ProtectionStatus、LockStatus、EncryptionPercentage 的明确报告；不读取恢复材料。卷身份应按稳定卷标识关联，盘符仅作显示。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- encryption_percentage/lock_status/volume_type 已采未映射。
- 挂载点不是稳定卷身份，盘符换到另一卷可能沿用旧对象；需与卷 GUID 关联（代码路径风险）。

**验收：** FullyEncrypted+Off、加密中百分比、Locked 和无法读取分别解释；盘符重分配后旧加密状态不带给新卷。

采集定义：`pcscope.security.encryption`。

对象类型：`security.bitlocker_configuration`。

依据：[plugins/powershell-adapter/domains/security.ps1:26](../../plugins/powershell-adapter/domains/security.ps1#L26)、[crates/contracts/src/adapters/security.rs:586](../../crates/contracts/src/adapters/security.rs#L586)。

<a id="source-089"></a>

## 089 CNG 密钥元数据

来源：`pcscope.security.key-metadata`。路线：原生。用途：多种材料。任务：SP01、SP03、SP04、SP05、SP06、SP09。

**现状：** 原生已列 CNG 提供程序，并按 provider×user/machine 独立只读枚举 name/algorithm/LegacyKeySpec；静默模式、不读私钥。adapter 已加入 provider/location scope，provider 列表作为能力证据而非对象。 当前对象采集健康已保留同一 source 下所有不同非空 scope 的结果；仅时间明确较晚的非空 scope 尝试可替代旧空 scope 失败。

**整改后输出：** 显示密钥登记名称、算法、提供程序及用户/机器位置；按具体提供程序和位置说明可读取、无密钥或读取受限，便于定位覆盖缺口。

**判断规则：** 以提供程序+位置+所属用户+原生名/LegacyKeySpec 定位；已枚举元数据形成密钥登记清单，独立保留各范围成功/拒绝/不支持的结果，范围健康按当前已修的 scope 保留规则汇总。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。
- 原生 item_id 仍 name/LegacyKeySpec；必须验证同名键跨 provider/location 的普通 source binding 不串联。
- 逐提供程序和用户/机器位置的可读健康说明仍需补齐，避免只展示内部 scope 标识；不同非空 scope 的健康结果保留逻辑已修，不再列为缺口。
- 密钥用途/所属应用、实际私钥可用性和操作自检未覆盖；不是枚举失败即可断言密钥损坏。

**验收：** 两个 provider、两个位置同名键保持四个身份；一范围失败、另一范围稍后成功仍保留该失败，只有旧空 scope 失败可按已有时间规则被替代；错误能定位到具体提供程序和位置。

采集定义：`pcscope.security.key-metadata`。

对象类型：`security.key_metadata`。

依据：[plugins/platform-native/Sources.cs:166](../../plugins/platform-native/Sources.cs#L166)、[plugins/powershell-adapter/domains/security.ps1:16](../../plugins/powershell-adapter/domains/security.ps1#L16)、[plugins/powershell-adapter/domains/security.ps1:8](../../plugins/powershell-adapter/domains/security.ps1#L8)、[crates/contracts/src/adapters/source_gaps.rs:128](../../crates/contracts/src/adapters/source_gaps.rs#L128)、[crates/storage/src/identity.rs:81](../../crates/storage/src/identity.rs#L81)、[apps/desktop/ui/model-view.js:96](../../apps/desktop/ui/model-view.js#L96)。

<a id="source-090"></a>

## 090 安全产品登记

来源：`pcscope.security.products`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** SecurityCenter2 AntiVirusProduct 已采 instanceGuid、displayName、productState、timestamp；adapter 保留产品登记名和 packed 状态码，未映射 timestamp。

**整改后输出：** 显示系统安全中心登记的产品及其报告状态/更新时间，说明登记和厂商实际运行状态的来源区别。

**判断规则：** 产品 GUID 稳定定位；只有有依据且版本明确的状态码解释才给启用/过期等结论，未知编码显示系统报告了未识别状态。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 产品状态码无可靠类型解码/支持边界，timestamp 未映射。
- 不能从安全中心登记存在推出服务必然运行；与 Defender 运行来源需并列或可靠关联。

**验收：** 同名不同 GUID 安全产品分开；未知 productState 不变成正常，过旧更新时间明确展示。

采集定义：`pcscope.security.products`。

对象类型：`security.security_product`。

依据：[plugins/powershell-adapter/domains/security.ps1:3](../../plugins/powershell-adapter/domains/security.ps1#L3)、[crates/contracts/src/adapters/security.rs:439](../../crates/contracts/src/adapters/security.rs#L439)。

<a id="source-091"></a>

## 091 Defender 防护状态

来源：`pcscope.security.protection`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-MpComputerStatus 已给引擎/特征库版本、RealTimeProtectionEnabled、IsTamperProtected、扫描时间及其他防护开关；adapter 已映射实时状态、防篡改和较晚扫描结束时间。

**整改后输出：** 显示 Defender 报告的实时保护和防篡改当前状态、特征库更新时间及最近扫描；关闭可能使部分实时检测无法工作。

**判断规则：** 运行来源明确布尔直接决定该运行事实；保留观察时效，可由可信 Defender 状态变化日志及时更新，不要求再次扫描才承认事件。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 与 protection-policy 共用 real_time 字段语义但来源含义不同；需拆开运行状态/策略要求，并明确两个对象 defender/defender-policy 的产品关联。
- 服务、行为监控、IOAV/NIS 等开关及特征库更新时间已采未映射。

**验收：** 运行 false、策略未禁用时明确运行关闭而非显示保护开启；新事件更新后旧快照不回滚较晚状态。

采集定义：`pcscope.security.protection`。

对象类型：`security.protection`。

依据：[plugins/powershell-adapter/domains/security.ps1:1](../../plugins/powershell-adapter/domains/security.ps1#L1)、[crates/contracts/src/adapters/security.rs:191](../../crates/contracts/src/adapters/security.rs#L191)、[crates/contracts/src/adapters/security.rs:331](../../crates/contracts/src/adapters/security.rs#L331)。

<a id="source-092"></a>

## 092 Defender 策略与排除项

来源：`pcscope.security.protection-policy`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-MpPreference 已采实时/行为/IOAV/脚本设置、PUAProtection 及路径/进程/扩展名排除项；adapter 将 DisableRealtimeMonitoring 取反塞进 real_time，并输出多个排除对象。

**整改后输出：** 独立显示防护策略设置、排除项及变化，解释某类检测被配置停用或某项不参与扫描，帮助定位保护范围的调整。

**判断规则：** Disable* 解释为配置要求，直接由配置得结论；排除项按类型和值独立归属，保持脱敏。状态来源另行给实际运行事实。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。 实时防护设置已改写 real_time_configured；实际运行仍由 real_time 表达；旧记录在显示层按来源区分。

**具体缺口与范围：** 

- 策略取反与实际运行共用 State.real_time，应分离字段/类型语义。
- 代码路径已证实的关联风险，尚未在本轮复现数据损坏：一个父记录展开多个对象，但 projection.apply 目前仅对账户权限/组成员启用按子身份绑定；此来源仍可能复用父绑定并跳过其他子项。
- DisableBehaviorMonitoring/DisableIOAVProtection/DisableScriptScanning/PUAProtection 已采未映射。

**验收：** 同一偏好记录含两个路径及一扩展排除时全部出现且不覆盖产品；策略允许但实时运行关闭可同时表达。

采集定义：`pcscope.security.protection-policy`。

对象类型：`security.protection`、`security.exclusion`。

依据：[plugins/powershell-adapter/domains/security.ps1:2](../../plugins/powershell-adapter/domains/security.ps1#L2)、[crates/contracts/src/adapters/security.rs:280](../../crates/contracts/src/adapters/security.rs#L280)、[crates/contracts/src/adapters/security.rs:331](../../crates/contracts/src/adapters/security.rs#L331)、[crates/contracts/src/adapters/security.rs:381](../../crates/contracts/src/adapters/security.rs#L381)、[crates/storage/src/projection.rs:151](../../crates/storage/src/projection.rs#L151)。

<a id="source-093"></a>

## 093 安全启动状态

来源：`pcscope.security.secureboot`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** Confirm-SecureBootUEFI 返回布尔已映射 enabled；不支持或拒绝读取抛错，由来源健康报告，未伪造 false。

**整改后输出：** 显示安全启动开启/关闭或无法读取，说明是否启用了启动阶段的签名保护。

**判断规则：** 直接使用固件 API 的布尔结果；类型名称可作固定友好标题，无需虚构设备名。适用性和权限错误归健康状态。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 已有字段未进入按类型选择的状态摘要，列表/详情/时间线尚未统一解释其状态、采样时间及影响。
- 需要区分不支持传统启动环境与临时权限不足，并保留最近成功读数的日期。

**验收：** true/false/unsupported/access denied 分成四种可读情况；没有读数时不显示关闭。

采集定义：`pcscope.security.secureboot`。

对象类型：`security.secure_boot_configuration`。

依据：[plugins/powershell-adapter/domains/security.ps1:7](../../plugins/powershell-adapter/domains/security.ps1#L7)、[crates/contracts/src/adapters/security.rs:500](../../crates/contracts/src/adapters/security.rs#L500)。

<a id="source-094"></a>

## 094 SSH 公钥指纹

来源：`pcscope.security.ssh-public-keys`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 只扫描当前用户 .ssh 下安全大小的 .pub 文件，支持列明算法并计算 SHA256 指纹；adapter 按路径+算法映射登记，不保存密钥材料和注释。

**整改后输出：** 显示公钥文件、算法和指纹变化，帮助定位更换/丢失的公钥登记；完整路径与指纹可展开。

**判断规则：** 同路径算法与指纹变化说明该文件公钥已更换；根据已读算法和指纹解释变化，保护完整路径及原始文本。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 文件名称/指纹变化摘要未统一；当前没有 authorized_keys、远程服务器授权或私钥配对/可用性覆盖。
- 代码路径已证实的作用域风险，尚未在本轮复现数据损坏：普通 DbLookup.by_source_binding 仅比较 source/item_id/native_record_id，不比较候选 type、机器、安装或 scope；同名记录跨作用域可能沿用旧绑定。
- 当前来源未采集服务器授权关系或私钥可用性；公钥文件存在不能确定它已被某台服务器授权。

**验收：** 更换 .pub 内容明确指纹改变，不称登录权限已改变；超大/重解析/不支持内容报覆盖问题且不泄露材料。

采集定义：`pcscope.security.ssh-public-keys`。

对象类型：`security.ssh_public_key`。

依据：[plugins/powershell-adapter/domains/security.ps1:28](../../plugins/powershell-adapter/domains/security.ps1#L28)、[crates/contracts/src/adapters/security.rs:625](../../crates/contracts/src/adapters/security.rs#L625)。

<a id="source-095"></a>

## 095 TPM 状态

来源：`pcscope.security.tpm`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** Get-Tpm 已采 present/ready/enabled/activated/owned/restart pending/manufacturer/version/locked out；adapter 映射存在、就绪、启用、激活、厂商、版本和锁定。

**整改后输出：** 显示安全芯片是否存在、是否就绪/锁定和是否需要重启，说明可能影响凭据或磁盘保护操作。

**判断规则：** 每个布尔独立解释；存在不代表已就绪，就绪不代表未锁定。不支持/读取失败属于采集健康。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- TpmOwned/RestartPending 已采未映射；当前主要信息缺有优先级的状态组合。

**验收：** 存在但未就绪、已锁定、需重启等组合全部可区分，不把制造商版本当运行健康。

采集定义：`pcscope.security.tpm`。

对象类型：`hardware.tpm`。

依据：[plugins/powershell-adapter/domains/security.ps1:6](../../plugins/powershell-adapter/domains/security.ps1#L6)、[crates/contracts/src/adapters/hardware.rs:445](../../crates/contracts/src/adapters/hardware.rs#L445)。

<a id="source-096"></a>

## 096 第三方预览处理程序

来源：`pcscope.shell.registrations`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP08、SP09。

**现状：** 采 HKLM/HKCU、32/64 位的七个开关范围：预览、图标覆盖、允许/阻止、命名空间、文件关联/菜单/图标、COM 及未细分/来源状态。按选定树有界枚举；InprocServer32 可采本地实现文件存在性、版本、时间、长度和有界哈希，签名仍 not_verified；adapter 多数按键分类。

**整改后输出：** 每种登记有用途名称和登记状态；明确的实现文件缺失显示“相关扩展可能无法使用”，登记允许/阻止单独表示。运行时已加载模块的文件跟踪与登记关联，但不把所有登记宣称为 sihost 加载清单。

**判断规则：** 按 hive/view/用户/完整登记身份区分；值级列表逐条建对象并连接精确 CLSID，跨键关联保留来源。文件 not_found 是当时目标缺失，unavailable 是无法检查。登记、批准、文件可读和运行加载为四种事实；没有运行证据不猜承载进程。

**本轮实施进展：** 2026-10-13 已拆分 COM 根名称、InprocServer32 实现文件及 LocalServer32 启动命令；同快照多登记项按对象和原始项幂等投影，首次纠正旧映射建立基线而不制造系统变化。精确已采根键补名及 125 条名称解释保留身份/保护边界，旧混合角色比较标为待复核；历史未批量重放，其他 Shell 子项和文件状态缺口保持。见 docs/refactor/acceptance/history-readability-audit.md；本轮实机确认待补。

**具体缺口与范围：** 

- PreviewHandlers 与 Approved/Blocked 是值级多登记，当前键级 adapter 未逐值建可辨认对象；需同时验收多候选落库与历史兼容。
- 文件关联/菜单/图标跨键解析不完整；COM 的 implementation_file.status 未成为明确文件状态字段，not_found 容易只剩缺版本/哈希。
- 只对 InprocServer32 有有界文件材料；LocalServer32、相对路径/网络路径未检查是明确边界，不能判为坏文件。
- 生产原生注册表/文件读取迁移尚未归属；保持路径缓存、大小限额和变更合并，避免登记刷新触发全盘或循环模块扫描。

**验收：** 七子项分别有样本：同键多值、同 CLSID 不同 hive/view、缺失/不可读/变化中文件、不验证签名和运行关联都不串对象、不误称系统必然无法启动。

采集定义：`pcscope.shell.registrations.preview`、`pcscope.shell.registrations.overlay`、`pcscope.shell.registrations.approval`、`pcscope.shell.registrations.namespace`、`pcscope.shell.registrations.file-association`、`pcscope.shell.registrations.com`、`pcscope.shell.registrations`。

对象类型：`shell.preview_handler`、`shell.icon_overlay_handler`、`shell.shell_extension`、`shell.namespace_extension`、`shell.context_menu_registration`、`shell.file_association`、`shell.com_registration`。

依据：[plugins/powershell-adapter/collect-system.ps1:151](../../plugins/powershell-adapter/collect-system.ps1#L151)、[plugins/powershell-adapter/collect-system.ps1:154](../../plugins/powershell-adapter/collect-system.ps1#L154)、[crates/contracts/src/adapters/shell.rs:139](../../crates/contracts/src/adapters/shell.rs#L139)、[crates/storage/src/projection.rs:151](../../crates/storage/src/projection.rs#L151)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-097"></a>

## 097 本机软件安装登记

来源：`pcscope.software`。路线：原生。用途：状态或配置快照。任务：SP01、SP04、SP05、SP06、SP09。

**现状：** 独立 Rust 原生采集器读取 HKLM 两视图卸载登记，保留名称、版本、厂商等；登记身份为 hive/view/完整路径。缺名登记已有精确字典补充，受保护或冲突名称不会被字典绕过。

**整改后输出：** 显示软件登记原名/经解释的登记名、版本和厂商；字典识别明确说明性质和使用情况未确认，登记增加/消失不是凭空安装/卸载操作。

**判断规则：** 原始 DisplayName 优先；字典仅精确登记标识与允许的缺名状态匹配。名称和版本不作为唯一身份，两个同名登记不合并；完整登记快照更新当前清单。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 已有字典与名字保护应沿用；本轮仍缺版本/厂商/登记状态的统一列表摘要及跨页面术语。
- 该来源只覆盖机器卸载登记，不能作为所有程序存在、可运行、正在运行的完整证明；用户登记和便携扫描须注明各自范围。

**验收：** 原名、字典名、仍未知名及遮罩四类均维持保护；同名不同路径保留，快照差异不伪造具体安装/卸载时间。

采集定义：`pcscope.software`。

对象类型：`software.installation_registration`。

依据：[plugins/software/src/main.rs:125](../../plugins/software/src/main.rs#L125)、[plugins/software/src/main.rs:135](../../plugins/software/src/main.rs#L135)、[plugins/software/src/main.rs:67](../../plugins/software/src/main.rs#L67)、[crates/contracts/src/adapters/software.rs:67](../../crates/contracts/src/adapters/software.rs#L67)、[crates/storage/src/query.rs:201](../../crates/storage/src/query.rs#L201)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)。

<a id="source-098"></a>

## 098 当前用户 AppX 包

来源：`pcscope.software.appx-user`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-AppxPackage 读取当前用户包全名、家族、名称、版本、架构、发布者、位置、签名种类和 Status；adapter 保留包身份、名称/版本/发布者/位置/状态。

**整改后输出：** 显示“当前用户的 Windows 应用包”、可读名称、版本及部署状态；缺友好名称时说明包用途或保留中性名称。

**判断规则：** 包全名和用户范围决定身份，Status 按部署状态含义解码；已登记不等于正在运行。非当前用户未覆盖属于范围，不显示全机无此应用。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- Package Name 常为技术名称，未解析包显示名称资源；架构、SignatureKind 等已采字段未适配。
- Status 未进入摘要/专属解释，泛生命周期不能代表包部署状态。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 PowerShell 待原生 Deployment API 迁移，保持用户令牌范围与不可用环境说明。

**验收：** 同包不同用户、正常/异常/未知部署状态和无法解析显示名称分别正确，未扫描用户不判卸载。

采集定义：`pcscope.software.appx-user`。

对象类型：`software.appx_package`。

依据：[plugins/powershell-adapter/domains/system.ps1:17](../../plugins/powershell-adapter/domains/system.ps1#L17)、[crates/contracts/src/adapters/software.rs:111](../../crates/contracts/src/adapters/software.rs#L111)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-099"></a>

## 099 Windows 可选组件

来源：`pcscope.software.optional-features`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** Get-WindowsOptionalFeature -Online 读取 FeatureName 和 State，adapter 保留 name/state。

**整改后输出：** 显示 Windows 功能名称及“已启用/已关闭/等待重启完成启用或关闭/组件文件未保留”等准确状态。

**判断规则：** 按 DISM 状态枚举解释，待重启与已经生效分开；启用表示功能配置，不表示相应服务正在运行。未知枚举不退化为关闭。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- FeatureName 偏内部名，需可读名称字典及原文详情；State 未出现在列表语义摘要。
- 原生 DISM 迁移与不同 Windows 版本/版本类型适用性待验收，权限失败不等同功能不存在。

**验收：** Enabled、Disabled、EnablePending、DisablePending、Removed 和未知枚举均区别展示，未授权不清空功能列表。

采集定义：`pcscope.software.optional-features`。

对象类型：`software.optional_feature`。

依据：[plugins/powershell-adapter/domains/system.ps1:19](../../plugins/powershell-adapter/domains/system.ps1#L19)、[crates/contracts/src/adapters/software.rs:141](../../crates/contracts/src/adapters/software.rs#L141)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-100"></a>

## 100 便携软件文件登记

来源：`pcscope.software.portable`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 显式 portable_roots 下有界扫描 exe/dll/msi，不跟随重解析目录；采文件身份、路径、版本资源、公司、大小和按配置哈希，签名未验证。adapter 仅在取得 volume/file ID 时生成对象。

**整改后输出：** 显示“指定目录中的程序文件”、名称/版本/位置及文件变化；扫描范围和哈希状态清楚，不能称全部便携软件或已安装应用。

**判断规则：** 同文件身份跨重命名/重叠根去重，文件创建时间不作为安装时间；哈希变化是内容变化，不直接代表恶意或不可用。缺身份保留采集缺口。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 不可取得文件身份时仅不建对象，用户需能看到对应源材料与失败原因，避免以为该目录已完整覆盖。
- hash_status、signature_status 和扫描限额尚未形成易读说明。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产仍为 PowerShell，而非 scoped 中已迁移的文件监控；需原生扫描、按文件变动缓存版本/哈希，保持上限及变更中读取丢弃规则。

**验收：** 空配置、重叠根、同文件重命名、不可读、超限、重解析和读取时变化均正确；无哈希不写内容未变，无签名验证不写可信。

采集定义：`pcscope.software.portable`。

对象类型：`software.portable_registration`。

依据：[plugins/powershell-adapter/domains/scoped.ps1:96](../../plugins/powershell-adapter/domains/scoped.ps1#L96)、[plugins/powershell-adapter/domains/scoped.ps1:24](../../plugins/powershell-adapter/domains/scoped.ps1#L24)、[crates/contracts/src/adapters/software.rs:185](../../crates/contracts/src/adapters/software.rs#L185)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-101"></a>

## 101 预配 AppX 包

来源：`pcscope.software.provisioned`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-AppxProvisionedPackage -Online 读取系统为新用户预置的包名称、版本、架构、包身份和安装位置；adapter 映射 name/version/location，publisher 未由来源提供。

**整改后输出：** 显示“为新用户预置的 Windows 应用包”，名称和版本可读，和当前用户应用包分开。

**判断规则：** 预置登记表示创建用户时的部署准备，不证明当前任何用户已安装或运行；按包身份比较快照，不拿 displayName 合并。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 类型说明及预置与当前用户安装的区别未进入主摘要；架构已采未适配，publisher 缺失不可猜。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 DISM PowerShell 待原生迁移，并区分权限不足和不支持该环境。

**验收：** 相同包同时为预置和当前用户安装时可区分；预置移除不声称已经从所有用户卸载。

采集定义：`pcscope.software.provisioned`。

对象类型：`software.provisioned_package`。

依据：[plugins/powershell-adapter/domains/system.ps1:18](../../plugins/powershell-adapter/domains/system.ps1#L18)、[crates/contracts/src/adapters/software.rs:126](../../crates/contracts/src/adapters/software.rs#L126)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-102"></a>

## 102 当前用户软件登记

来源：`pcscope.software.user-inventory`。路线：原生。用途：状态或配置快照。任务：SP01、SP04、SP05、SP06、SP07、SP09。

**现状：** 读取当前用户 HKCU 卸载登记，adapter 与机器登记共用字段规则但保留用户范围；名称、版本、厂商、位置来自注册表值。

**整改后输出：** 显示“当前用户的软件登记”、原名和版本；缺名保持未知或仅在获准精确字典规则下解释。

**判断规则：** 用户 SID+hive/view/完整路径决定登记身份；机器精确字典不可无条件套到 HKCU。登记缺失只按本用户有效完整范围判断。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 用户范围和登记性质缺统一摘要，容易与机器级同名登记混淆。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产仍走 PowerShell，原生注册表迁移需保留同账户语义；提权账户改变时不得把另一用户资料当当前用户。

**验收：** 机器与两个用户的同名登记保持独立，提权不同账户不得静默替换本用户范围；受保护名称不被补名绕过。

采集定义：`pcscope.software.user-inventory`。

对象类型：`software.installation_registration`。

依据：[plugins/powershell-adapter/domains/system.ps1:20](../../plugins/powershell-adapter/domains/system.ps1#L20)、[crates/contracts/src/adapters/software.rs:83](../../crates/contracts/src/adapters/software.rs#L83)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-103"></a>

## 103 IFEO、AppInit 与 AppCert 入口

来源：`pcscope.startup.advanced-extensions`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP08、SP09。

**现状：** 已读 IFEO、SilentProcessExit、AppInit、AppCert 指定树与深度全部脱敏值；adapter 统一为 ifeo_configuration，保留注册表身份和 values。

**整改后输出：** 分别说明程序调试/退出监控/全局注入登记与相关开关变化，解释可能影响哪个程序的启动或加载行为。

**判断规则：** 按精确路径和值名识别功能，配置存在或开关开启可直接说明配置；实际模块加载/错误日志可更新发生事实，不把登记一律当已加载。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 多种机制全部显示 IFEO 类名，不利于理解；values 未按机制解码。
- 配置引用目标与监控文件状态尚未关联；本来源本身不验证模块能否加载。

**验收：** IFEO Debugger、AppInit 开关、AppCert 登记分别解释；目标缺失只在文件/加载证据支持时显示，保留登记原文。

采集定义：`pcscope.startup.advanced-extensions`。

对象类型：`startup.ifeo_configuration`。

依据：[plugins/powershell-adapter/domains/startup.ps1:3](../../plugins/powershell-adapter/domains/startup.ps1#L3)、[crates/contracts/src/adapters/startup.rs:362](../../crates/contracts/src/adapters/startup.rs#L362)、[crates/contracts/src/adapters/startup.rs:394](../../crates/contracts/src/adapters/startup.rs#L394)。

<a id="source-104"></a>

## 104 登录启动登记

来源：`pcscope.startup.logon`。路线：原生。用途：多种材料。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP08、SP09。

**现状：** 同来源含 Run/RunOnce 单值、StartupApproved 多值表、用户/公共启动文件夹文件；adapter 分三形状。采集定义有 registry、folders、未细分/来源状态共三项，均应列入。

**整改后输出：** registry 子项显示登录启动登记及允许状态；folders 子项显示启动文件/快捷方式及目标；未细分子项保留来源健康和无法分类记录，不重复计成另一批启动程序。

**判断规则：** Run/RunOnce 命令与 StartupApproved 状态按同 hive/view/用户/名称可靠关联；文件夹文件单独身份并标用户/公共范围。启动日志可说明实际执行，登记本身只说明启动配置。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 代码路径已证实的关联风险，尚未在本轮复现数据损坏：一个父记录展开多个对象，但 projection.apply 目前仅对账户权限/组成员启用按子身份绑定；此来源仍可能复用父绑定并跳过其他子项。
- StartupApproved 二进制许可状态已采但未解码到字段；不能把值表当启动命令。
- folder adapter 沿用要求 value_name 的注册表身份而缺该键，源未显式记录用户/公共 folder 类别；shortcut_target_collected=false，未解析目标。
- 启动文件哈希/大小/写入时间已采未映射；三采集定义的健康和覆盖需独立归属。

**验收：** Run/RunOnce、两条 StartupApproved、用户/公共启动文件夹均独立；关闭 registry 不影响 folders，未细分来源状态不冒充实际项目。

采集定义：`pcscope.startup.logon.registry`、`pcscope.startup.logon.folders`、`pcscope.startup.logon`。

对象类型：`startup.logon_startup_entry`。

依据：[plugins/powershell-adapter/collect-system.ps1:159](../../plugins/powershell-adapter/collect-system.ps1#L159)、[crates/contracts/src/adapters/startup.rs:201](../../crates/contracts/src/adapters/startup.rs#L201)、[crates/contracts/src/adapters/startup.rs:298](../../crates/contracts/src/adapters/startup.rs#L298)、[crates/contracts/src/adapters/startup.rs:336](../../crates/contracts/src/adapters/startup.rs#L336)、[crates/storage/src/projection.rs:151](../../crates/storage/src/projection.rs#L151)。

<a id="source-105"></a>

## 105 服务详细配置

来源：`pcscope.startup.service-configuration`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP08、SP09。

**现状：** 已读 Services 两级子树的全值，包括 Start、DisplayName、ObjectName、ImagePath、依赖与 ServiceDll 等；adapter 按路径服务名汇到 startup.service，仅映射名称/启动模式/账户/程序路径。

**整改后输出：** 显示服务的启动方式、运行账户、程序/服务 DLL 及配置变化；与服务当前运行状态并排说明启动安排和实际运行情况。

**判断规则：** 按服务名统一注册配置和 Win32_Service 运行来源；Start 数字与 StartMode 字符串归为同一语义；子键只提供它明确声明的字段，不抹除主键事实。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- Start 数字与运行来源 StartMode 字符串未规范化，可能制造无意义前后变化。
- 依赖、DelayedAutoStart、ServiceDll、失败处理等已采未映射；驱动服务也在 Services 树，需避免不分类型重复归类。

**验收：** Start=2 与 Auto 语义一致不重复报改动；主键/Parameters 子键不互相清空，DLL 路径变化可定位。

采集定义：`pcscope.startup.service-configuration`。

对象类型：`startup.service`。

依据：[plugins/powershell-adapter/domains/startup.ps1:1](../../plugins/powershell-adapter/domains/startup.ps1#L1)、[crates/contracts/src/adapters/startup.rs:130](../../crates/contracts/src/adapters/startup.rs#L130)。

<a id="source-106"></a>

## 106 服务运行状态

来源：`pcscope.startup.services`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Win32_Service 已采状态、启动方式、账户、命令、PID、退出码等；adapter 映射显示名、state/start_mode/account/binary_path，保留按服务名身份。

**整改后输出：** 显示服务正在运行/已停止/转换中及启动方式；失败退出码用易懂说明，解释对应功能可能不可用。

**判断规则：** 服务状态来源直接决定运行事实；准确服务控制事件可即时更新状态，之后较晚快照覆盖；手动服务已停止不默认判故障。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- PID、exit_code、service_specific_exit_code、description 等已采未映射。
- 状态摘要与服务事件更新规则缺失，启动方式来源枚举需统一。

**验收：** 运行→停止事件及时更新且旧快照不回滚；按需服务正常停止不显示异常，明确失败码才说明失败。

采集定义：`pcscope.startup.services`。

对象类型：`startup.service`。

依据：[plugins/powershell-adapter/collect-system.ps1:40](../../plugins/powershell-adapter/collect-system.ps1#L40)、[crates/contracts/src/adapters/startup.rs:83](../../crates/contracts/src/adapters/startup.rs#L83)、[crates/storage/src/projection.rs:530](../../crates/storage/src/projection.rs#L530)。

<a id="source-107"></a>

## 107 会话与登录初始化入口

来源：`pcscope.startup.session-hooks`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP08、SP09。

**现状：** 已采 Winlogon Shell/Userinit、BootExecute、Active Setup、Explorer Run 等精确路径全值；adapter 保留注册表身份和 values。

**整改后输出：** 按桌面启动、登录初始化、首次登录配置分组说明登记及变化，指出可能影响登录或桌面启动。

**判断规则：** 准确配置键和值直接确定配置含义；实际启动/失败日志可说明执行事实。用户设置与机器设置要保留所属用户，不能仅用 CurrentUser 字样合并。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- registry_registration 一律 installation_scope，CurrentUser Active Setup/Run 的用户 SID 丢失，存在跨用户同键合并风险。
- values 尚未类型化解释，目标程序/文件健康无可靠关联。

**验收：** 两个用户同 Active Setup 键不合并；Shell/Userinit 更改明确说明影响，缺日志不伪造实际运行。

采集定义：`pcscope.startup.session-hooks`。

对象类型：`startup.logon_initialization`。

依据：[plugins/powershell-adapter/domains/startup.ps1:2](../../plugins/powershell-adapter/domains/startup.ps1#L2)、[crates/contracts/src/adapters/startup.rs:362](../../crates/contracts/src/adapters/startup.rs#L362)、[crates/contracts/src/adapters/startup.rs:386](../../crates/contracts/src/adapters/startup.rs#L386)。

<a id="source-108"></a>

## 108 计划任务执行记录与条件

来源：`pcscope.startup.task-details`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采 LastRunTime/LastTaskResult/NextRunTime/NumberOfMissedRuns，以及 triggers、conditions、principal、registration、actions；两来源共用 scheduled_task adapter，但身份当前只取 payload.path（TaskPath目录），没有加 TaskName。

**整改后输出：** 显示任务名称、完整目录、启用状态、是否正在运行/最近结果及下次运行；失败或错过运行时说明该任务未按计划完成。

**判断规则：** 以 collector 已有 TaskPath+TaskName 或可靠完整 item_id 作为身份；任务开始/结束/结果日志可直接更新本次运行状态，与快照按时间归并，不要求重新运行验证。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- P0 代码路径风险，待隔离复现：同目录不同任务仅以目录建键，可能合并；不能凭显示名修补对象链接。
- state 与基础 principal 字段未按实际 payload 适配；TASK_FIELDS 要 enabled/principal，但 detail 只有 conditions/principal、base 只有 user_id/run_level。
- missed_runs、conditions、registration 已采但未映射；结果码和时间零值需要按任务语义解释。

**验收：** 同目录A/B、不同目录同名任务互不串联，两个来源同任务合并；开始/结束/失败日志只更新准确任务/运行实例，旧错归属只审计不硬迁移。

采集定义：`pcscope.startup.task-details`。

对象类型：`startup.scheduled_task`。

依据：[plugins/powershell-adapter/domains/startup.ps1:7](../../plugins/powershell-adapter/domains/startup.ps1#L7)、[crates/contracts/src/adapters/startup.rs:162](../../crates/contracts/src/adapters/startup.rs#L162)、[crates/contracts/src/adapters/startup.rs:175](../../crates/contracts/src/adapters/startup.rs#L175)、[plugins/powershell-adapter/collect-system.ps1:43](../../plugins/powershell-adapter/collect-system.ps1#L43)。

<a id="source-109"></a>

## 109 计划任务基础配置

来源：`pcscope.startup.tasks`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采 TaskName/TaskPath、state、enabled、user_id/run_level 与 actions；两来源共用 scheduled_task adapter，但身份当前只取 payload.path（TaskPath目录），没有加 TaskName。

**整改后输出：** 显示任务名称、完整目录、启用状态、是否正在运行/最近结果及下次运行；失败或错过运行时说明该任务未按计划完成。

**判断规则：** 以 collector 已有 TaskPath+TaskName 或可靠完整 item_id 作为身份；任务开始/结束/结果日志可直接更新本次运行状态，与快照按时间归并，不要求重新运行验证。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- P0 代码路径风险，待隔离复现：同目录不同任务仅以目录建键，可能合并；不能凭显示名修补对象链接。
- state 与基础 principal 字段未按实际 payload 适配；TASK_FIELDS 要 enabled/principal，但 detail 只有 conditions/principal、base 只有 user_id/run_level。
- missed_runs、conditions、registration 已采但未映射；结果码和时间零值需要按任务语义解释。

**验收：** 同目录A/B、不同目录同名任务互不串联，两个来源同任务合并；开始/结束/失败日志只更新准确任务/运行实例，旧错归属只审计不硬迁移。

采集定义：`pcscope.startup.tasks`。

对象类型：`startup.scheduled_task`。

依据：[plugins/powershell-adapter/collect-system.ps1:43](../../plugins/powershell-adapter/collect-system.ps1#L43)、[crates/contracts/src/adapters/startup.rs:162](../../crates/contracts/src/adapters/startup.rs#L162)、[crates/contracts/src/adapters/startup.rs:175](../../crates/contracts/src/adapters/startup.rs#L175)、[plugins/powershell-adapter/domains/startup.ps1:7](../../plugins/powershell-adapter/domains/startup.ps1#L7)。

<a id="source-110"></a>

## 110 WMI 过滤器与消费者绑定

来源：`pcscope.startup.wmi-bindings`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采 Filter/Consumer 引用、MaintainSecurityContext、SlowDownProviders、DeliverSynchronously；adapter 以 root/subscription+两引用建绑定身份并映射安全上下文。

**整改后输出：** 显示哪个系统事件条件触发哪个动作，以及绑定新增/撤销；引用详情可展开定位。

**判断规则：** 来源明确引用可直接建立配置关系；相应 WMI 事件可说明绑定建立/动作发生，不把配置存在等同于已触发。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- Filter/Consumer 完整引用尚未解析为可读对象链接；已有两个附加配置未映射。
- 父过滤器/消费者身份路径可能缺失，需要与本绑定引用按规范身份关联。

**验收：** 同名不同类/namespace 消费者不串；仅显示绑定已配置，只有触发日志才显示动作发生。

采集定义：`pcscope.startup.wmi-bindings`。

对象类型：`startup.wmi_binding`。

依据：[plugins/powershell-adapter/domains/startup.ps1:6](../../plugins/powershell-adapter/domains/startup.ps1#L6)、[crates/contracts/src/adapters/startup.rs:464](../../crates/contracts/src/adapters/startup.rs#L464)。

<a id="source-111"></a>

## 111 WMI 永久订阅消费者

来源：`pcscope.startup.wmi-consumers`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采消费者名、类、受保护命令/脚本、脚本引擎、程序路径及 source_properties；adapter 读 __RELPATH/__PATH 作身份，映射名称/类/引擎/程序。

**整改后输出：** 显示订阅动作将运行的程序或脚本类型及变化，敏感命令保留保护；明确配置登记与实际执行区别。

**判断规则：** 用 WMI 原生实例路径+注册 namespace 定位；已有程序字段给可读目标，受保护脚本不可展开成正文；实际执行日志可直接说明发生过。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- Public-CimFields 保存 CimInstanceProperties，不保证含 __RELPATH/__PATH；当前身份读取这两个键，需用真实样本验证/补采 CimSystemProperties.Path，不能用显示名猜。
- 命令/脚本保护摘要未形成可读变更字段；需保留保护而非重采明文。

**验收：** 同名不同消费者类有独立原生路径；身份缺失明确标资料不全，命令被保护不泄露或伪称未配置。

采集定义：`pcscope.startup.wmi-consumers`。

对象类型：`startup.wmi_consumer`。

依据：[plugins/powershell-adapter/domains/startup.ps1:5](../../plugins/powershell-adapter/domains/startup.ps1#L5)、[crates/contracts/src/adapters/startup.rs:416](../../crates/contracts/src/adapters/startup.rs#L416)、[crates/contracts/src/adapters/startup.rs:445](../../crates/contracts/src/adapters/startup.rs#L445)。

<a id="source-112"></a>

## 112 WMI 永久订阅过滤器

来源：`pcscope.startup.wmi-filters`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 已采名称、事件 namespace、QueryLanguage、受保护 Query 和 source_properties；adapter 映射条件字段，身份也依赖 __RELPATH/__PATH。

**整改后输出：** 显示这项订阅在等待哪类系统事件、关联动作及条件变化，帮助解释系统已配置的自动响应条件。

**判断规则：** 注册 namespace 和事件 namespace 分清；可读查询条件根据已采语句解释，受保护时只说明条件已记录/变化；触发事件可直接提供发生依据。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 需验证/补原生 WMI 实例路径；不能把 event_namespace 误当注册 namespace。
- 查询表达式与绑定的友好摘要尚未建立，保护内容不能强行解码。

**验收：** 注册于 root/subscription、监听 root/cimv2 的过滤器两种 namespace 分明；查询变化只报配置改变而非已执行。

采集定义：`pcscope.startup.wmi-filters`。

对象类型：`startup.wmi_filter`。

依据：[plugins/powershell-adapter/domains/startup.ps1:4](../../plugins/powershell-adapter/domains/startup.ps1#L4)、[crates/contracts/src/adapters/startup.rs:416](../../crates/contracts/src/adapters/startup.rs#L416)、[crates/contracts/src/adapters/startup.rs:426](../../crates/contracts/src/adapters/startup.rs#L426)。

<a id="source-113"></a>

## 113 磁盘清单

来源：`pcscope.storage.disks`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-Disk 采 ObjectId/UniqueId、编号、友好名、总线、分区样式、容量、引导/系统、离线/只读与健康/运行状态；adapter 保留部分字段，未保留 FriendlyName、IsOffline、IsReadOnly。

**整改后输出：** 显示磁盘名称、容量、用途和“在线/离线、只读、健康报告”，把影响读写的状态放在概要。

**判断规则：** 磁盘编号只是定位标签，稳定 ObjectId 为身份；离线、只读、健康是不同维度，不能统一成启用布尔。未知/多个状态保留。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 友好名、离线、只读已采未适配，健康/运行状态列表缺可读摘要。
- 与 PhysicalDisk 和分区之间缺明确关联验收；编号变化不应误作新磁盘。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 Storage PowerShell 待原生查询，明确外接/虚拟/动态磁盘及权限边界。

**验收：** 外接磁盘编号变更、离线、只读、告警和虚拟磁盘均准确，读取失败不标磁盘已拔出。

采集定义：`pcscope.storage.disks`。

对象类型：`hardware.disk`。

依据：[plugins/powershell-adapter/domains/storage.ps1:1](../../plugins/powershell-adapter/domains/storage.ps1#L1)、[crates/contracts/src/adapters/hardware.rs:133](../../crates/contracts/src/adapters/hardware.rs#L133)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-114"></a>

## 114 分区清单

来源：`pcscope.storage.partitions`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Get-Partition 采对象与磁盘 ID、编号、GUID、偏移、大小、分区类型、挂载路径及活动/引导/隐藏/只读标志；adapter 保留编号、GUID、偏移、大小、访问路径。

**整改后输出：** 显示“某磁盘的分区”、容量、挂载位置、明确用途和限制；无盘符不等于异常。

**判断规则：** 按来源稳定身份区分，DiskId 精确连接磁盘；GPT/MBR 类型按各自字典解释，恢复/系统分区只读或无盘符可以正常。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- DiskId 关系、Type/GptType/MbrType 和用途/限制标志未适配，默认对象难辨用途。
- 原生 Storage 查询迁移及分区调整/编号重排的历史身份验收待办。

**验收：** 系统、恢复、普通数据、无盘符和重排编号分区均可辨认，目录挂载点不被误当盘符。

采集定义：`pcscope.storage.partitions`。

对象类型：`hardware.partition`。

依据：[plugins/powershell-adapter/domains/storage.ps1:2](../../plugins/powershell-adapter/domains/storage.ps1#L2)、[crates/contracts/src/adapters/hardware.rs:169](../../crates/contracts/src/adapters/hardware.rs#L169)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-115"></a>

## 115 卷与文件系统

来源：`pcscope.storage.volumes`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Win32_Volume 实际只采 DeviceID、盘符、卷标、文件系统、容量、驱动器类型；adapter 保留除卷标外这些信息。该来源当前没有 free_space。

**整改后输出：** 显示“卷标（盘符/挂载位置）”、文件系统、容量和类型；明确这不是剩余空间监测。

**判断规则：** 卷 DeviceID 为身份，盘符和卷标是可变显示；没有盘符仍可能是有效卷。容量已知不推出空间充足，未采 free_space 不填写 0。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 已采 label 未适配，造成 UUID 泛名；drive_type 未中文解释。
- 剩余空间未采，若产品需要空间告警须作为新增测量列入范围，不是现有字段改个名称就可实现。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 CIM PowerShell 待原生卷枚举，并限制不可达卷/远端挂载阻塞。

**验收：** 改卷标/盘符不改身份，无盘符卷正常展示；不存在 free_space 时绝不显示“剩余 0”或空间不足。

采集定义：`pcscope.storage.volumes`。

对象类型：`hardware.volume`。

依据：[plugins/powershell-adapter/collect-system.ps1:46](../../plugins/powershell-adapter/collect-system.ps1#L46)、[crates/contracts/src/adapters/hardware.rs:194](../../crates/contracts/src/adapters/hardware.rs#L194)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-116"></a>

## 116 最近启动时间与时区

来源：`pcscope.system.boot-state`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 采 Win32_OperatingSystem.LastBootUpTime 与时区；adapter 映射为单独 operating_system_configuration 的 boot_time，其它系统配置字段不提供。

**整改后输出：** 系统摘要显示“最近启动时间”和资料时间；与版本、时区、待重启信息组成有来源的系统概况。

**判断规则：** LastBootUpTime 直接支持本次系统报告的启动时间；可用有明确启动含义的系统日志更新启动事实，不强制再采快照才承认。采样时间不冒充启动时间。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 启动、版本、时钟、待重启分散成同一通用配置类型，缺跨来源按字段组合的系统摘要。
- 本源 timezone 未适配且时钟源另有时区，需统一来源/时效规则，避免无关字段缺失互相覆盖。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 CIM PowerShell 待原生系统查询。

**验收：** 启动快照和启动日志按时间有序更新同一系统概况；较旧材料不覆盖较新启动，读取失败保留上次时间并标时效。

采集定义：`pcscope.system.boot-state`。

对象类型：`policy.os_configuration`。

依据：[plugins/powershell-adapter/collect-system.ps1:30](../../plugins/powershell-adapter/collect-system.ps1#L30)、[crates/contracts/src/adapters/policy.rs:537](../../crates/contracts/src/adapters/policy.rs#L537)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-117"></a>

## 117 时间同步配置与时区

来源：`pcscope.system.clock`。路线：原生。用途：状态或配置快照。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 采 W32Time 配置键（含 Type/NtpServer/Enabled/同步间隔）及系统时区；adapter 将注册表配置与 timezone 字面记录分别映射为系统配置。

**整改后输出：** 显示时区、配置的时间来源/服务器、同步相关开关，明确这些是设置；若没有成功同步证据，不写“时间已同步”。

**判断规则：** 时区和各配置值按确切字段解释；Enable 标志须注明属于哪个 provider/client，不能从任意 Enabled 推断整体服务已同步。可靠时间同步日志可直接记录成功/失败与时间。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 注册表 values 未适配为分项可读配置，系统时区与启动源材料缺整合。
- 当前不采实际偏差或最近同步成功；若要同步健康须新增可用事件/接口字段，不能用 NtpServer 存在替代。
- 原生注册表/时区读取迁移待办。

**验收：** 手动/域/指定服务器配置与真实同步事件分开；时区变化清晰，只有服务器配置时不声称时间准确。

采集定义：`pcscope.system.clock`。

对象类型：`policy.time_sync_configuration`。

依据：[plugins/powershell-adapter/domains/system.ps1:2](../../plugins/powershell-adapter/domains/system.ps1#L2)、[crates/contracts/src/adapters/policy.rs:357](../../crates/contracts/src/adapters/policy.rs#L357)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-118"></a>

## 118 QFE 更新登记

来源：`pcscope.system.hotfixes`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Win32_QuickFixEngineering 采 HotFixID、说明和 InstalledOn 字符串，并明确 QFE only；adapter 建 hotfix 对象并保留这些字段。

**整改后输出：** 显示“系统报告的热修复登记”、KB 编号、可读说明和可解析安装日期，明确清单范围。

**判断规则：** QFE 记录支持它列出的已登记热修复，不代表完整 Windows Update 历史。InstalledOn 有效时按其精度展示；未知/地区格式无法确定时保留原文详情。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- KB 与说明/日期缺统一列表摘要，日期为地区格式字符串需要明确解析/未知规则。
- 全部 Windows 更新不在 QFE 覆盖范围，应与更新历史按可信更新标识关联而非拼名称。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 CIM PowerShell 待原生查询。

**验收：** 有日期、缺日期、地区格式和仅历史出现的更新分开；QFE 空列表不显示“系统没有安装更新”。

采集定义：`pcscope.system.hotfixes`。

对象类型：`software.system_update`。

依据：[plugins/powershell-adapter/collect-system.ps1:63](../../plugins/powershell-adapter/collect-system.ps1#L63)、[crates/contracts/src/adapters/software.rs:99](../../crates/contracts/src/adapters/software.rs#L99)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-119"></a>

## 119 待重启标志

来源：`pcscope.system.reboot-pending`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 读取 CBS RebootPending、Windows Update RebootRequired、Session Manager PendingFileRenameOperations 三类信号；adapter 按存在键/值产生各自 reboot_pending 配置。

**整改后输出：** 系统概要显示“有待重启事项”并说明更新或待处理文件操作；只有所有受支持信号均成功检查时才显示“这些检查未发现待重启事项”。

**判断规则：** 任一明确 marker 为真即可直接报告对应原因；不是所有重启需求的完整探测。有效完整检查确认 marker 不再存在可清除此原因；权限/缺口/部分扫描不能当 false。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 多个原因分散且缺系统级汇总；不存在键通常不发 item，需显式检查结果或可靠完整快照清除旧原因，防长期显示已消失的 pending。
- “未发现”与“无需重启”语义边界、最新检查时效尚未统一。
- 原生注册表迁移需保留每种 marker 的成功/失败结果，不只给总成功。

**验收：** 单原因、多个原因、重启后全部消失、部分拒绝访问均正确；部分失败不能显示无需重启，旧原因不永久滞留。

采集定义：`pcscope.system.reboot-pending`。

对象类型：`policy.os_configuration`。

依据：[plugins/powershell-adapter/domains/system.ps1:15](../../plugins/powershell-adapter/domains/system.ps1#L15)、[crates/contracts/src/adapters/policy.rs:393](../../crates/contracts/src/adapters/policy.rs#L393)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-120"></a>

## 120 系统还原点

来源：`pcscope.system.restore-points`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** SystemRestore 查询读取序号、说明、创建时间、还原点类型与事件类型；adapter 保留序号/说明/创建时间/类型。

**整改后输出：** 显示还原点名称、创建时间和类型，以及清单变化；不把列出还原点等同已验证可恢复。

**判断规则：** 同系统序号为来源身份，创建时间是来源明确时间；实际恢复成功仅能来自相应执行结果，不从存在性推出。未开启系统保护、权限不足和没有还原点区分。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 类型/事件类型枚举与对象用途缺可读解释，EventType 已采未适配。
- 当前不做还原点可用性或恢复演练，属于只读清单范围。
- 原生 COM/WMI 迁移及系统保护不适用环境说明待办。

**验收：** 正常清单、空清单、功能关闭和访问拒绝分开；创建时间精度保留，从不声称已验证能恢复。

采集定义：`pcscope.system.restore-points`。

对象类型：`policy.system_restore_point`。

依据：[plugins/powershell-adapter/domains/system.ps1:16](../../plugins/powershell-adapter/domains/system.ps1#L16)、[crates/contracts/src/adapters/policy.rs:440](../../crates/contracts/src/adapters/policy.rs#L440)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-121"></a>

## 121 Windows Update 历史

来源：`pcscope.system.update-history`。路线：原生。用途：变化与事件。任务：SP01、SP02、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 生产仍为 PowerShell Windows Update COM 历史分页读取，保留更新 ID/修订、日期、操作、结果、错误码和受保护标题；adapter 已生成有结果/阶段/时间的 update_history action，但 target 全部 unassigned。Sources.cs 有原生实现而 ownership 未启用。

**整改后输出：** 时间线显示具体更新的“安装成功/安装失败/部分成功/卸载成功/已中止”等；系统概要可显示该更新最近已知结果、时间和是否存在后续相反记录。

**判断规则：** 有效历史操作本身足以确认当时安装/卸载结果，不要求无谓二次证明；按 update ID+修订和严格时间顺序更新最近已知状态，明确“截至此记录”，后续成功卸载、回滚或更新来源冲突可改变结论。历史保留有界，记录缺失不反证没发生。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 目前 action 始终 unassigned，无更新对象关联和最近已知操作状态；不能因此把可证实的成功结果永远当未知。
- result=3/5 在通用 outcome 下为 Observed/Ended，需要保留其部分成功/中止专属语义及错误码的简短解释。
- 原生实现尚未进入生产归属；切换前验证所有原始字段、分页、重复事件、部分字段失败和受保护标题对等，不直接计入原生覆盖。

**验收：** 安装成功→失败尝试→卸载成功及乱序重放得到正确最近已知状态；部分成功/中止不冒充成功，原生接管后无重复/丢历史。

采集定义：`pcscope.system.update-history`。

对象类型：`software.system_update`。

依据：[plugins/powershell-adapter/domains/system.ps1:11](../../plugins/powershell-adapter/domains/system.ps1#L11)、[crates/contracts/src/adapters/source_gaps.rs:100](../../crates/contracts/src/adapters/source_gaps.rs#L100)、[plugins/platform-native/Sources.cs:159](../../plugins/platform-native/Sources.cs#L159)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-122"></a>

## 122 操作系统版本与安装时间

来源：`pcscope.system.version`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 采 Windows Caption、Version、BuildNumber、架构、安装时间和计算机名；adapter 仅映射 caption/version/build/install_time 到单独系统配置对象。

**整改后输出：** 系统概况显示 Windows 名称、版本、内部版本、架构和来源明确的安装日期；系统字段与记录生命周期分开。

**判断规则：** 采用系统原始字段，安装日期按 Win32_OperatingSystem.InstallDate 说明，不把首次被软件发现当安装时间。机器名称可变但不改变本机系统身份。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- architecture、computer_name 已采未适配；版本、启动、时区、待重启分散，通用对象名称不利理解。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产 CIM PowerShell 待原生版本/系统查询；版本升级的时间只按有证据的精度报告。

**验收：** 名称/版本/架构清楚；升级、重命名计算机和首次采集不混为系统新安装，安装时间缺失保持未知。

采集定义：`pcscope.system.version`。

对象类型：`policy.os_configuration`。

依据：[plugins/powershell-adapter/collect-system.ps1:26](../../plugins/powershell-adapter/collect-system.ps1#L26)、[crates/contracts/src/adapters/policy.rs:514](../../crates/contracts/src/adapters/policy.rs#L514)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-123"></a>

## 123 USN 日志身份与断点

来源：`pcscope.usn.{drive}`。路线：原生。用途：变化与事件。任务：SP02、SP03、SP04、SP05、SP06、SP07、SP08、SP09。

**现状：** Rust原生读取现有NTFS USN，按显式文件名单过滤，保留journal_id/checkpoint/reason_mask及覆盖缺口；不创建日志。

**整改后输出：** 逐监控文件展示有变化及原因位含义，显示监控范围/断点/遗漏时段。

**判断规则：** USN不是每次写入的内容版本；原因可合并；无完整路径或父目录更名不能猜关联；读断点不等于历史全覆盖。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- USN volume/file_id与文件快照身份统一尚未完成
- reason_mask尚未全部逐位语义化/对象字段转换
- 模块自动名单未接USN
- 实际每盘来源需按prefix映射到虚拟pcscope.usn.changes，不能重复统计

**验收：** 日志换代/回绕/删日志/读途中换代、watch_revision变化、非NTFS、父目录改名、断点重放及去重均保守且有界。

采集定义：内部/动态来源，由对应汇总入口承接，不新增独立轮询或开关。

对象类型：`file.watched_file`。

依据：[plugins/usn/src/main.rs:118](../../plugins/usn/src/main.rs#L118)、[plugins/usn/src/main.rs:194](../../plugins/usn/src/main.rs#L194)、[crates/contracts/src/adapters/event.rs:55](../../crates/contracts/src/adapters/event.rs#L55)。

<a id="source-124"></a>

## 124 文件变更记录汇总

来源：`pcscope.usn.changes`。路线：虚拟汇总/导入。用途：变化与事件。任务：SP01、SP05、SP06、SP09。

**现状：** 虚拟来源，无独立collector；prefix定义id=pcscope.usn.changes/source=pcscope.usn.覆盖实际每盘事件，volumes/watch-list为专门来源。

**整改后输出：** 统一入口聚合pcscope.usn.{drive}，准确区分文件变化、卷能力和名单配置。

**判断规则：** 只作聚合和开关映射，不新造事件/当前对象；精确来源规则优先于prefix，不把健康快照当文件变更。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 来源台账需显式记录prefix归属，避免虚拟源被当未采集或重复计数
- 未知盘来源仍需安全归类

**验收：** 新盘字母来源、volumes、watch-list均分类唯一；开关过滤前后无重复计数。

采集定义：`pcscope.usn.changes`。

对象类型：`file.watched_file`。

依据：[apps/desktop/ui/collection-catalog.json](../../apps/desktop/ui/collection-catalog.json)、[crates/storage/src/collection.rs:20](../../crates/storage/src/collection.rs#L20)、[crates/contracts/src/adapters/event.rs:180](../../crates/contracts/src/adapters/event.rs#L180)。

<a id="source-125"></a>

## 125 固定 NTFS 卷发现

来源：`pcscope.usn.volumes`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP09。

**现状：** 只读枚举固定盘盘符/卷身份/文件系统及NTFS资格；映射hardware.volume，不代表日志可读。

**整改后输出：** 显示卷名称、文件系统、能否使用USN及读取检查结果分栏。

**判断规则：** eligible=true仅为文件系统资格，不证明USN已存在、权限足够或断点有效。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 盘符/卷标友好名与卷身份关联需统一
- USN资格与实际journal读取权限/历史范围易混淆

**验收：** NTFS无日志、非NTFS、不可访问、卷换盘符、无盘符卷覆盖边界均明确。

采集定义：`pcscope.usn.volumes`。

对象类型：`hardware.volume`。

依据：[plugins/usn/src/main.rs:279](../../plugins/usn/src/main.rs#L279)、[crates/contracts/src/adapters/hardware.rs](../../crates/contracts/src/adapters/hardware.rs)。

<a id="source-126"></a>

## 126 文件监控名单与配置状态

来源：`pcscope.usn.watch-list`。路线：原生。用途：采集健康。任务：SP01、SP03、SP05、SP06、SP07、SP09。

**现状：** 显示显式名单、revision、active；空名单直接返回配置快照，不扫描卷。

**整改后输出：** 显示配置了哪些文件、名单版本、未配置/暂停与实际读取覆盖。

**判断规则：** active表示名单已配置，不是文件活动中或USN监控已经成功。

**具体缺口与范围：** 

- 主要信息active通用翻译易误导
- 名单配置成功与实际卷读取成功未统一展示
- prefix台账中需保留本精确源优先级

**验收：** 空名单零事件读取、变更revision重置边界、不解析路径有缺口，不能把active=true当采集成功。

采集定义：内部/动态来源，由对应汇总入口承接，不新增独立轮询或开关。

对象类型：事件、采样或健康记录；按具体内容处理，无需强造对象类型。

依据：[plugins/usn/src/main.rs:229](../../plugins/usn/src/main.rs#L229)、[crates/contracts/src/adapters/source_gaps.rs:19](../../crates/contracts/src/adapters/source_gaps.rs#L19)。

<a id="source-127"></a>

## 127 Docker 容器、镜像、卷与网络

来源：`pcscope.virtualization.docker`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 显式开启后用固定 Docker Desktop CLI 与固定本地 named pipe 执行四次列表：容器（含停止）、镜像、卷、网络；adapter 按 kind 分四类型，镜像 ID+仓库+标签区分，容器保留 state/status。

**整改后输出：** 容器显示名称、镜像和运行/停止/重启等报告状态；镜像、卷、网络显示登记用途和配置，不统一套“运行中”。说明只覆盖指定本地 Docker 引擎。

**判断规则：** 本地 API/CLI 成功列表直接支持当时对应实体状态；容器 state 与 status 分开解释。完整列表移除仅更新该类型范围；引擎未连接不删除上次资料。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 容器 names 字段与通用可读命名未统一；四类型需要各自摘要与状态，镜像标签不是运行状态。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：生产仍是 PowerShell 启动四个 CLI 子进程，并非直接原生 API；迁移为本地管道后端客户、总预算、单飞及退避，不自行启动 Docker。
- 未安装 Docker Desktop、daemon 停止、权限不够和显式关闭需准确区分；远端/custom context 不在当前范围。

**验收：** 容器运行/停止、同镜像多标签、同名不同 kind、引擎离线和权限失败均准确；刷新不连续派生 CLI 或启动 daemon。

采集定义：`pcscope.virtualization.docker`。

对象类型：`virtualization.container`、`virtualization.image`、`virtualization.container_volume`、`virtualization.container_network`。

依据：[plugins/powershell-adapter/domains/scoped.ps1:79](../../plugins/powershell-adapter/domains/scoped.ps1#L79)、[crates/contracts/src/adapters/virtualization.rs:286](../../crates/contracts/src/adapters/virtualization.rs#L286)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-128"></a>

## 128 虚拟机检查点

来源：`pcscope.virtualization.hyperv-checkpoints`。路线：PowerShell 兼容。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 按每台 VM 枚举检查点，采检查点 ID、VMId、名称、创建时间、类型、父检查点；adapter 保留除 VMId 外这些字段。

**整改后输出：** 显示“某虚拟机的检查点”、名称、时间、类型和父子关系，登记存在不声称可恢复性已验证。

**判断规则：** 检查点稳定 ID 区分，VMId 精确关联 VM；无父 ID 是根检查点，读取不到父字段不是同一含义。只读列表不产生创建/恢复操作。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- VMId 已采未投影，跨虚拟机同名检查点难辨归属；类型数字/名称缺用户解释。
- 生产 Hyper-V cmdlet 待原生管理接口迁移；未安装角色、无管理权限、主机不可用需作为适用性/健康显示。

**验收：** 同名不同 VM 检查点分离，父子树与根节点正确，Hyper-V 不适用不写全部检查点已删除。

采集定义：`pcscope.virtualization.hyperv-checkpoints`。

对象类型：`virtualization.checkpoint`。

依据：[plugins/powershell-adapter/domains/virtualization.ps1:4](../../plugins/powershell-adapter/domains/virtualization.ps1#L4)、[crates/contracts/src/adapters/virtualization.rs:161](../../crates/contracts/src/adapters/virtualization.rs#L161)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-129"></a>

## 129 虚拟机磁盘连接

来源：`pcscope.virtualization.hyperv-disks`。路线：PowerShell 兼容。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 采 VMId/VMName、虚拟磁盘路径、控制器类型/编号/位置及 DiskNumber；adapter 以 VM+控制器位置建磁盘附件身份，保留路径与位置。

**整改后输出：** 显示“某虚拟机的磁盘连接”、位置、文件或物理盘目标；接入配置与磁盘是否正在读写分开。

**判断规则：** 按 VMId 精确关联并优先展示其可信名称；连接槽位是附件身份，同槽替换磁盘表现目标变化。无文件路径可能是物理盘直通，不能直接判损坏。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- VMName、DiskNumber 未适配，直通磁盘目标和虚拟机归属显示不足。
- 当前不检查 VHD 内部数据、I/O 或文件完整性，属于配置清单边界；原生 Hyper-V 管理接口迁移待办。

**验收：** 文件磁盘、物理盘直通、同槽换盘、同名不同 VM 和读取失败均正确，不把附件登记称正在读写。

采集定义：`pcscope.virtualization.hyperv-disks`。

对象类型：`virtualization.disk_attachment`。

依据：[plugins/powershell-adapter/domains/virtualization.ps1:3](../../plugins/powershell-adapter/domains/virtualization.ps1#L3)、[crates/contracts/src/adapters/virtualization.rs:123](../../crates/contracts/src/adapters/virtualization.rs#L123)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-130"></a>

## 130 Hyper-V 虚拟交换机

来源：`pcscope.virtualization.hyperv-switches`。路线：PowerShell 兼容。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 采虚拟交换机 ID、名称、类型、网卡说明及 AllowManagementOS；adapter 保留 id/name/type/adapter。

**整改后输出：** 显示交换机名称、外部/内部/专用用途、关联适配器及是否允许主机共用；配置不等于网络可达。

**判断规则：** 按稳定 ID 区分，SwitchType 用专属枚举；AllowManagementOS 解释主机访问配置，不冒充接口链路状态。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- AllowManagementOS 已采未适配，类型缺用途解释及状态摘要。
- 网卡目前仅说明，跨来源关联需要可靠标识，不能按重复说明文本强连；原生 Hyper-V 查询迁移待办。

**验收：** 外部/内部/专用三种用途及同名适配器准确，未知类型保留，交换机存在不声称虚拟机已联网。

采集定义：`pcscope.virtualization.hyperv-switches`。

对象类型：`virtualization.virtual_switch`。

依据：[plugins/powershell-adapter/domains/virtualization.ps1:2](../../plugins/powershell-adapter/domains/virtualization.ps1#L2)、[crates/contracts/src/adapters/virtualization.rs:94](../../crates/contracts/src/adapters/virtualization.rs#L94)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-131"></a>

## 131 Hyper-V 虚拟机

来源：`pcscope.virtualization.hyperv-vms`。路线：PowerShell 兼容。用途：状态或配置快照。任务：SP01、SP02、SP03、SP05、SP06、SP07、SP09。

**现状：** 采 VM ID、名称、State、代际、版本、CPU 数、启动/最小/最大内存、动态内存开关、自动启动/停止配置等；adapter 保留 ID/name/state/generation/cpu，单一 memory 字段故意不从四种内存事实猜值。

**整改后输出：** 显示虚拟机名称、“运行/关闭/暂停/保存/未知”及 CPU/内存配置；自动启动配置与当前状态独立。

**判断规则：** State 直接支持采样时虚拟机状态；可靠 Hyper-V 事件可按 VMId 更新对应状态并保留时效。内存分启动值、动态开关及上下限，不能挑一个硬塞“内存”。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 四类内存及自动启动/停止等已采字段没有合适模型，摘要缺具体运行状态；代际/状态需专属枚举。
- 生产 Hyper-V cmdlet 待原生管理接口迁移；无角色或无权限不产生空 VM 清单替换成功状态。

**验收：** 运行/关闭/暂停/保存、动态内存、自动启动但已关机和不适用环境均准确；旧事件不覆盖更新快照。

采集定义：`pcscope.virtualization.hyperv-vms`。

对象类型：`virtualization.virtual_machine`。

依据：[plugins/powershell-adapter/domains/virtualization.ps1:1](../../plugins/powershell-adapter/domains/virtualization.ps1#L1)、[crates/contracts/src/adapters/virtualization.rs:70](../../crates/contracts/src/adapters/virtualization.rs#L70)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-132"></a>

## 132 虚拟化能力与平台

来源：`pcscope.virtualization.platform`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** Win32_ComputerSystem 提供 hypervisor_present、厂商/机型；每个 CPU 提供固件虚拟化、VM Monitor 扩展与 SLAT。adapter 将 hypervisor 与各处理器作为独立能力对象，映射四类布尔。

**整改后输出：** 系统概况分别显示“检测到虚拟机监控程序”“处理器支持”“固件中启用”及未知；明确这些与有没有 VM 在运行不是同一件事。

**判断规则：** 每个布尔只陈述源字段本义；HypervisorPresent=true 不等于用户安装并正在使用 Hyper-V 管理角色，硬件支持不等于已启用。虚拟机里被屏蔽的 CPU 能力保持来源范围。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- 厂商、机型及 CPU 名称已采未适配，能力对象难辨；分散布尔缺按维度解释的汇总。
- 历史迁移边界记录（当前已由原生采集；其中字段、范围与环境要求继续保留）：物理主机、虚拟机、嵌套虚拟化及多 CPU 不同结果需适用性验收；生产 CIM PowerShell 待原生查询。

**验收：** 支持未启用、已检测监控程序、虚拟机受限能力和多个 CPU 分别显示，不从任一布尔推断 VM 正在运行。

采集定义：`pcscope.virtualization.platform`。

对象类型：`virtualization.platform_capability`。

依据：[plugins/powershell-adapter/collect-system.ps1:55](../../plugins/powershell-adapter/collect-system.ps1#L55)、[crates/contracts/src/adapters/virtualization.rs:367](../../crates/contracts/src/adapters/virtualization.rs#L367)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

<a id="source-133"></a>

## 133 WSL 发行版登记

来源：`pcscope.virtualization.wsl-registration`。路线：原生。用途：状态或配置快照。任务：SP01、SP03、SP04、SP05、SP06、SP07、SP09。

**现状：** 读取当前用户 Lxss 根及发行版子键，采发行版名、版本、路径、默认 UID、Flags/State 与根 DefaultDistribution；adapter 跳过根，保留各发行版名/版本/路径/UID。

**整改后输出：** 显示“当前用户的 WSL 发行版登记”、名称、WSL 版本、默认发行版标记和位置；不以注册表 State 当作实时运行状态。

**判断规则：** 用户+发行版登记 ID 建身份；根 DefaultDistribution 只通过精确登记 ID 设置默认标记。Registry State 含义需明确后适配，未采运行接口则不声称正在运行/停止。

**本轮实施进展：** 已接入按类型选择摘要和跨页面值解释；已有字段可展示，原始字段缺口、时效及专项验收仍待完成。

**具体缺口与范围：** 

- DefaultDistribution 已采但根被跳过，无法显示默认发行版；Flags/State 原材料未适配，不能按通用布尔解释。
- 当前只覆盖当前用户登记，不覆盖所有用户或发行版当前进程；原生注册表迁移与用户范围验收待办。

**验收：** 两用户同名发行版分离，默认发行版切换可读；已登记但未运行不误称运行中，缺根或权限失败不把默认标记改成否。

采集定义：`pcscope.virtualization.wsl-registration`。

对象类型：`virtualization.wsl_distribution`。

依据：[plugins/powershell-adapter/domains/virtualization.ps1:5](../../plugins/powershell-adapter/domains/virtualization.ps1#L5)、[crates/contracts/src/adapters/virtualization.rs:221](../../crates/contracts/src/adapters/virtualization.rs#L221)、[crates/storage/src/query.rs:174](../../crates/storage/src/query.rs#L174)、[plugins/platform-native/ownership.json](../../plugins/platform-native/ownership.json)、[plugins/powershell-adapter/src/main.rs:53](../../plugins/powershell-adapter/src/main.rs#L53)。

