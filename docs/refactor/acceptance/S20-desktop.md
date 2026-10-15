# S20 验收记录：桌面与应用集成

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S20 · 待验收 |
| 实现范围 | 桌面类 10 个来源 → 9 个声明类型的适配器：打印机四源、输入配置、剪贴板、Shell 设置、快捷方式、IDE 扩展、OneDrive 账户 |
| 代码定位 | 新增 `crates/contracts/src/adapters/desktop.rs`；修改 `crates/contracts/src/adapters/mod.rs`（注册 `desktop::SOURCES`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；素材键名取自 `domains/desktop.ps1`、`domains/applications.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；28 项新增测试；17 个 `scripts/check-*.cjs`；契约用例与演练输出重新生成后逐字节未变；构建零警告 |
| 结果 | 284 项 Rust 测试通过；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 3 节 |
| 下一步 | S21 全来源对账 |

## 1. 一组「两个事实、一个字段」的处理

本类有三处类型声明一个字段、素材给出两个或更多事实，处理方式统一且一致：

| 类型 | 声明字段 | 素材给出 | 处理 |
| --- | --- | --- | --- |
| `desktop.printer_port` | `port_type` | `Description` 与 `PortMonitor` | 都保留在材料里，字段记缺失——挑一个会丢掉另一个 |
| `desktop.print_job` | `pages` | `total_pages` 与 `pages_printed` | 同上 |
| `desktop.shell_configuration` | 若干 | 注册表值表 | 按值名各自成键（值级约定，§3.5） |

这与 `virtualization.rs` 处理「一个内存字段 vs 四个内存数字」的做法相同：**不挑一个，也不聚合**。

## 2. 未映射与跳过的两处

- **打印任务从不映射 `document_name`**（类型没有声明该字段），它因此不会到达候选。采集端自己的声明——只采集打印元数据、绝不采集文档内容——原样转述为一条结果说明，有测试断言文档名不出现在序列化结果里。
- **OneDrive 的两个容器键记为跳过并说明原因**（`…\Accounts` 与 `…\Providers\OneDrive` 是列表本身，不是列表中的一项），其内容留在材料里。机器策略键则正常映射（来源自己的描述包含策略字段，类型也声明了 `policy`），整张值表作为 `policy`。

## 3. 遗留问题

1. **`desktop.shortcut` 的 `file_identity` 无法满足**（勘察文档 §3.1）。目录树遍历发的是路径与快捷方式元数据，**从不发** `volume_identity`/`file_id`；两者在每个快捷方式上都记为缺失，**路径没有被拿来顶替**（类型自己的说明写着路径只用于展示）。与 S14 同一问题，一并待决。
2. **`desktop.editor_extension` 缺 `editor` 与 `extension_id`**，是**勘察文档原先未列出**的同类缺陷：emitter 只读 `name`/`publisher`/`version`/`engines`。适配器拒绝了两条看起来可行的替代——从 item id 取扩展目录名（那命名的其实是配置文件夹而不是编辑器）、以及把 `publisher.name` 拼成 id。已补进勘察文档 §3.2。
3. **两个 `native_path` 注册表类型未把视图折进作用域**，因此在两个视图下读到同一个键会得到同一身份。本适配器选择与 `network.rs` 一致（目录没有声明视图就不折），而不是 `policy.rs` 的 `value_scope`；依据是目录自己的作用域文字——`policy.time_sync_configuration` 明确写了「+ 注册表视图」，而这两个桌面类型只写「用户作用域」。**这是一处需要统一的分歧**，列入 S21。
4. **快捷方式的作用域保留交来的用户作用域**，即使遍历也覆盖所有用户的桌面与开始菜单：材料里没有任何东西说明某个路径来自哪个文件夹，收窄就是 R02 禁止的推断。
5. **样本与采集端键名不一致**（`desktop.printing`/`desktop.shortcuts` 的样本用目录字段名，emitter 用 `Name`/`DriverName`）。按勘察文档 §5 的约定以采集端为准；没有示例读这些 item，生成物未变。
6. **本机未做真实采集实测**；打印机、剪贴板等在本机未接真实输出，键名来自 emit 声明的静态核对。
