# S13 验收记录：系统扩展与关联

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S13 · 待验收 |
| 实现范围 | 系统扩展类 3 个观察来源的适配器；其中 `pcscope.shell.registrations` 一个来源覆盖七个类型 |
| 代码定位 | 新增 `crates/contracts/src/adapters/shell.rs`；修改 `crates/contracts/src/adapters/mod.rs`（注册 `shell::SOURCES`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；素材键名取自 `collect-system.ps1`（`Read-RegistryTree` 与 shell 遍历）、`domains/applications.ps1` |
| 验证过程 | `cargo test --workspace`；11 项新增测试；17 个 `scripts/check-*.cjs`；6 个契约示例退出码；构建零警告 |
| 结果 | 185 项 Rust 测试通过（含并行进行的 S16/S18/S19）；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 4 节 |
| 下一步 | S14 文件与访问控制 |

## 1. 一个来源、七个类型

`pcscope.shell.registrations` 遍历注册表树，每个键一条 item，**类型由键在树中的位置决定**。分类规则按路径形态判定，顺序如下（前四条与采集端自己的开关映射一致）：

| 路径形态 | 类型 |
| --- | --- |
| `\Classes\CLSID\…` | `shell.com_registration` |
| `\PreviewHandlers` | `shell.preview_handler` |
| `\ShellIconOverlayIdentifiers` | `shell.icon_overlay_handler` |
| `\Shell Extensions\` | `shell.shell_extension` |
| `\Explorer\…\NameSpace` 或 `\MyComputer\NameSpace` | `shell.namespace_extension` |
| `\…\shell\…`、`\…\shellex\…`、`\ContextMenuHandlers` | `shell.context_menu_registration` |
| 其余 `\Classes\…` | `shell.file_association` |

**采集端的映射不能直接照抄。** 它把路径映射到**采集项开关**，不是对象类型：开关映射把 `\Classes\<progid>` 与 `\Classes\<progid>\shell` 归为同一项，而模型把后者叫上下文菜单登记、前者叫文件关联。两者不一致处由模型决定，差异写在做出该决定的规则旁边。

## 2. 两处结构性判断

- **hive 与 view 不在 item 里，而在消息作用域里。** 采集端按「配置单元 × 视图」分别发出，用作用域说明是哪一个：scope 形如 `…/schema:1/<hive>/<view>/shard:<n>`。适配器按**名字**在作用域里找这两段，而不是按位置；作用域里没有就记为缺失，由身份层说明缺了哪个键，而不是替它挑一个配置单元。

  机器配置单元 → 安装作用域（去掉采集账户段）；`CurrentUser` → 保留交来的作用域。与 S12/S15 独立得出的规则一致（勘察文档 §3.5），有测试钉住「机器级登记不因采集账户变化而改变身份」。

- **`implementation_file` 的 `status` 就是采集端对文件可读性的结论。** 该块只在 `InprocServer32` 下出现，`observed` 表示读过；`not_found` 表示文件确实不在；`not_read`/`unavailable`/`changed_during_read` 都表示这次没读到。三者含义不同，适配器分别处理：前者的版本与哈希记为值，后者记为 `unreadable`，而不是「这个组件没有版本」。

## 3. 与 S15 的边界

`pcscope.environment.configuration` 同时映射到 `policy.environment_variable` 与 `shell.file_association`。**S15 已实现该来源并产出这两类候选**，因此本步不重复注册它——适配器注册表按来源查找，一个来源两条 spec 会让其中一条永远不可达。此处记录以免日后被当成遗漏。

## 4. 遗留问题

1. **分类规则是本适配器按模型判断写的，未有独立来源核对。** 采集端的开关映射只覆盖其中四类，上下文菜单与文件关联的区分（`\shell`/`\shellex` 之下 vs 其余 `\Classes\` 之下）是模型层面的划分，依据是类型名称与路径语义。**真实树上是否还有既非这两者也非其余五类的键**，需在 S21 用真实采集输出核对。
2. **`shell.file_association.command` 恒为 `unreadable`。** 该来源读的是关联键，不读它指向的程序；把它写成缺失会读成「这个关联没有命令」，写成空值更糟。
3. **`shell.shell_extension.approved` 由路径推导**（在 `\Approved` 之下为真，`\Blocked` 之下为假），值本身只是 CLSID。这是本步唯一不是直接读值的字段，已在代码里写明。
4. **本机未做真实采集实测**；键名与路径形态来自 emit 声明的静态核对，真实树上的分类结果并入 S21。
5. **动作来自事件证据**：本类类型的动作（扩展加载、关联变更等）不在快照素材里，归 S17 的事件适配器。
