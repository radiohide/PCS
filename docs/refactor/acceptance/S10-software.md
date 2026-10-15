# S10 验收记录：软件与组件

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S10 · 进行中（**12 个来源中 7 个完成**） |
| 实现范围 | 机器与用户卸载登记、QFE 修补、AppX、预配包、可选组件、npm 全局包共 7 个来源的适配器；为注册表型素材新增值读取与掩码处理 |
| 代码定位 | 新增 `crates/contracts/src/adapters/software.rs`；修改 `crates/contracts/src/adapters/mod.rs`（`registry_value` / `registry_field`、注册表 `software::SOURCES`）、`model/ownership.json` |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；`:memory:` 数据库；素材键名取自 `plugins/software/src/main.rs`、`plugins/powershell-adapter/collect-system.ps1`、`domains/system.ps1`、`domains/applications.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；10 项新增测试；17 个 `scripts/check-*.cjs`；6 个契约示例退出码 |
| 结果 | 136 项 Rust 测试通过（较 S09 的 126 增加 10 项）；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 3 节——**5 个来源尚未实现**，本步不得标记通过 |
| 下一步 | 补齐其余 5 个来源后转 S11 |

## 1. 本步完成的部分

| 来源 | 类型 | 说明 |
| --- | --- | --- |
| `pcscope.software` | `software.installation_registration` | Rust 软件插件；`registry_path` + `registry_view` + `values` 映射 |
| `pcscope.software.user-inventory` | 同上 | PowerShell 采集端；同样的三件事但键名不同（`path` / `view` / `hive`） |
| `pcscope.system.hotfixes` | `software.system_update` | 身份即采集端的 item id |
| `pcscope.software.appx-user` | `software.appx_package` | 按包名归一 |
| `pcscope.software.provisioned` | `software.provisioned_package` | 该采集端不提供 publisher，如实记为缺失 |
| `pcscope.software.optional-features` | `software.optional_feature` | `state` 是状态字段 |
| `pcscope.apps.npm-global-packages` | `software.language_package` | 复合身份，manager 取 `npm` |

**两个采集端描述同一登记时身份一致。** Rust 插件写 `registry_path`/`registry_view`，PowerShell 采集端写 `path`/`view`/`hive`；两者指的是同样三件事，类型身份要的也正是这三件，所以任何一个采集端发现的登记都归一到同一对象。hive 不在任一采集端的载荷里，是它们共同写出的注册表路径前缀（`HKLM\`），从那里读是读来源说过的话，不是推断。

**没有任何 spec 填来源没给的字段。** 注册表值的显示掩码保持 `Masked` 而不是变成一个参与比较的值；采集端不提供的字段记为 `NotProvided`；缺少视图的登记身份记为不完整（`missing`），由身份层决定这意味着什么，而不是适配器替它挑一个视图。

**本类 8 个类型全部没有声明动作**，因此 S10 是纯观察类：软件的出现、变更与消失由观察本身表达。没有为它们发明目录里不存在的动作。

## 2. 结构：多了一种素材形状

注册表型素材（`Registry-Key` 生成）的载荷是 `Item "$hive/$view/$path" @{path;hive;view;values;basis;collection}`，其中 `values` 是 `名称 -> {kind,value}` 的映射。这是 S09 之外的新形状，引擎新增 `registry_value` / `registry_field` 两个入口。

顺带记录一条影响多个类别的事实：`Registry-Key` 的 `$allowed` 参数**不用于过滤**——源码注释写明「保留配置值，不只是历史上的显示字段白名单」，所以每个键下的全部值都被采集。多类别共用这一点，已写入勘察文档。

## 3. 尚未实现的来源（本步不通过的原因）

| 来源 | 为什么还没做 |
| --- | --- |
| `pcscope.apps.dotnet-runtimes` | 一个注册表来源覆盖两个路径族，且运行时身份是 `[runtime_family, runtime_version]`，需要按子键展开——形态与值级展开不同，要单独定 |
| `pcscope.software.portable` | 身份策略是 `file_identity`（`[volume_identity, file_id]`），而采集端只发 `path` 与元数据，**两个键都不在素材里**。见勘察文档第 3 节：这不是适配器能自己解决的，需要先决定是采集端补发还是改身份策略 |
| `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational` | 事件来源，已由 S17 的统一事件适配器接管（`docs/refactor/event-adapter.md`）。**但该通道的事件在 `event-schema.ps1` 里没有分类映射**，`kind` 保持 `windows.event`，回退路径的事件 id 表里也没有它们——因此素材留存、不产出候选。要产出需要扩采集端的投影表 |
| `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational` | 同上 |

**其余 4 项按同一引擎可在下一步补完**；`software.portable` 需要先解决 `file_identity` 的结构问题。

## 4. 下一步

- 补齐 `apps.dotnet-runtimes`。两个事件通道已由统一事件适配器接管，但需要扩采集端的投影表才能产出候选（属采集端工作）。
- `software.portable` 待 `file_identity` 问题决定后处理，该问题同时影响 S14（文件与访问控制）与 S20（快捷方式），已在勘察文档中登记。
