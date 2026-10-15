# S11 验收记录：运行实例

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S11 · 待验收 |
| 实现范围 | 运行实例类 4 个观察来源的适配器：进程、模块、登录会话、终端会话。与 S12/S15 并行推进（各自独立的类别文件） |
| 代码定位 | 新增 `crates/contracts/src/adapters/runtime.rs`；修改 `crates/contracts/src/adapters/mod.rs`（注册 `runtime::SOURCES`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；`:memory:` 数据库；素材键名取自 `domains/system.ps1`、`domains/scoped.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；8 项新增测试；17 个 `scripts/check-*.cjs`；6 个契约示例退出码；构建零警告 |
| 结果 | 144 项 Rust 测试通过（较 S10 的 136 增加 8 项）；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 3 节 |
| 下一步 | S12 启动配置（与本步并行进行中） |

## 1. 交付物

| 来源 | 类型 | 身份策略 |
| --- | --- | --- |
| `pcscope.execution.processes` | `runtime.process` | `process_instance`：`[scope, pid, created_at]`，`created_at` 取自 `start_time` |
| `pcscope.execution.modules` | `runtime.module_instance` | `composite_native`：`[process_identity, module_path]` |
| `pcscope.identity.logon-sessions` | `runtime.logon_session` | `native_identifier`：`[scope, native_id]`，取 LogonId |
| `pcscope.identity.interactive-sessions` | `runtime.terminal_session` | `native_identifier`：`[scope, native_id]`，取会话 Id |

## 2. 本类的判断要点

- **进程 ID 会复用，所以身份是 ID 加创建时间。** 同一编号在另一个启动时刻是另一个进程，这正是类型身份同时要两者的原因；有一条测试专门钉住这一点。
- **目录里的 `image` 是「程序路径」**，对应采集端的 `path`；`name` 是另一个概念，没有映射到该字段。
- **采集端自己声明的不完整性原样保留。** 进程快照会漏掉短命进程、会话快照不含登录/注销时间——这两句话随材料一起过来，适配器不把它们升格为完整记录，**也不因为下一次快照里没有就断言进程已结束**：那个结论属于缺席规则，不是适配器的。
- **命令行为空与被拒是两回事。** 采集端在被拒时发 `command_line_collected=false` 与 `command_line_status`，本适配器把值缺失记为 `NotProvided`。
- **会话用户读不到时记为 `unreadable`。** 采集端在查询失败时发 `identity_query_error`；把它当作「没有用户」会让一次权限拒绝看起来像一次真实状态。

## 3. 遗留问题

1. **模块与进程的关联键尚未确定。** `runtime.module_instance` 的键是 `[process_identity, module_path]`，而 `runtime.process` 的原生键由身份层用「作用域 + 进程 ID + 创建时间」拼成。本适配器的 `process_identity` 取**采集端自己的进程标识**（`<pid>/<启动时间>`），与后者的字符串形式不同——刻意如此：在适配器里手写那个拼接形式会凭空造出一条模型并未要求的关联。

   需决定的是**模块是否应当按进程原生键与进程对象关联**。这不是适配器能单独定的（要么身份层提供一种引用方式，要么目录说明两个类型的关系）。列入 S21。

2. **本机未做真实采集实测。** 键名来自采集端 emit 声明；真实输出的核对并入 S21。进程与模块两个来源需要显式配置或按需触发，本机权限下也未跑。
3. **动作只来自事件证据。** 本类 4 个类型都声明了动作（进程启停、模块加载、登录注销、会话锁定等），但快照素材里没有这些——采集端的登录/注销时间来自事件证据。动作解析归 S17 的事件适配器统一入口。
