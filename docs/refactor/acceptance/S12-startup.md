# S12 验收记录：启动配置

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S12 · 待验收 |
| 实现范围 | 启动配置类 10 个观察来源的适配器：服务、计划任务（两个来源）、登录启动项、服务配置、会话钩子、IFEO、WMI 过滤器/消费者/绑定 |
| 代码定位 | 新增 `crates/contracts/src/adapters/startup.rs`；修改 `crates/contracts/src/adapters/mod.rs`（注册 `startup::SOURCES`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；素材键名取自 `collect-system.ps1`、`domains/startup.ps1`、`collect-domain.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；18 项新增测试；17 个 `scripts/check-*.cjs`；6 个契约示例退出码；构建零警告 |
| 结果 | 174 项 Rust 测试通过（含 S11 的 8 项与 S15 的 12 项）；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 3 节 |
| 下一步 | S13 系统扩展与关联 |

## 1. 本类的判断要点

- **计划任务的两个来源按身份与作用域归一**，不按字段重叠。字段映射与 R02 参考适配器的键对逐一对齐（`path`/`name`/`enabled`/`principal`/`actions`/`triggers`），并补上目录声明、详细配置来源提供的三个状态字段（`next_run`/`last_run`/`last_result`）。
- **`startup.logon` 一个来源有三种 item 形态**（Run/RunOnce 值、StartupApproved 登记、启动文件夹文件），按载荷实际携带的内容分派，不假定一种。
- **服务配置来源本身不写服务名**——它是注册表树，名字要从 `SYSTEM\CurrentControlSet\Services` 之下第一段路径导出，这样子键（`…\<名称>\Parameters`）仍能归属到它的服务；树根不是对象。
- **该来源的 `state` 记为缺失**：它是配置来源，运行状态属 `pcscope.startup.services`。把配置来源的状态字段填成正数会与另一个来源打架。
- **`binary_path` 取自 `ImagePath`**，即使采集端的 `$allowed` 列表里没有它——勘察文档第 2 节第 2 条已确证 `$allowed` 不做过滤。
- **启动文件夹形态的 `command` 记为 `unreadable`**：载荷里 `shortcut_target_collected: false`，采集端明说自己没有读。这与「该字段不存在」是两回事，写清楚才不会被后续读成「这个启动项什么都不运行」。

## 2. WMI 过滤器与消费者的身份（一处已核对的不确定）

`startup.wmi_filter` 与 `startup.wmi_consumer` 都是 `native_path`（`[scope, path]`），但**两个 emitter 都没有写 path 键**。实现改为从 `source_properties` 里读实例自身的 `__RELPATH`（退回 `__PATH`）；若两者都没有，身份把 `path` 列为缺失，而不是拿过滤器/消费者的名字顶替。

实施时这条留下了一个未经执行验证的疑问：`source_properties` 里到底有没有 CIM 系统属性。**本次已静态核对**：采集端的 `Public-CimFields`（`plugins/powershell-adapter/evidence.ps1`）遍历 `$object.CimInstanceProperties`，而两个 emitter 用的都是 `Get-CimInstance`，其返回的 CIM 实例的该属性集合**包含** `__RELPATH`/`__PATH` 等系统属性。因此身份可以取到值，不是暂定身份。

**这仍是静态核对，不是实机实测**（本机未运行采集）。真实输出的核对并入 S21。

`startup.wmi_binding` 的 `namespace` 是声明的身份键而素材里没有，取自 emitter 自己声明的命名空间（`root/subscription`）——与 `software.rs` 写 `npm` 是同一类做法：适配器对其**自身来源**的知识。

## 3. 遗留问题

1. **值级身份的作用域约定在两处各有一份实现。** 本步的 `logon_scope` 与 S15 的 `value_scope`/`hive_scope` 规则相同（机器配置单元 → 安装作用域；当前用户 → 保留交来的作用域；`value_key` 类追加 `/view:`），但签名不同，对「hive 未知」的处理也有细微差别。约定已写入 `docs/refactor/adapter-source-survey.md` 第 3.5 节；**实现应在该差别定清后收拢进引擎**，避免两份各自漂移。未收拢是刻意的：仓促合并会改动一处尚未完全验证的行为。
2. **`startup.logon` 的启动文件夹形态保留了交来的作用域。** 采集端不说它枚举的是两个启动文件夹中的哪一个，挑一个会把机器级条目挂到碰巧执行采集的账户下。因此该形态的归属比机器级对象略宽——宽不会错误合并，窄才会。
3. **本机未做真实采集实测**，键名来自 emit 声明的静态核对。
4. **动作只来自事件证据。** 本类类型声明的动作（服务安装/启动/停止、任务创建/删除/运行等）在快照素材里没有，归 S17 的事件适配器。
