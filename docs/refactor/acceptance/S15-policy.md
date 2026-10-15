# S15 验收记录：策略与配置

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S15 · 待验收 |
| 实现范围 | 策略与配置类 9 个观察来源的适配器：策略配置、浏览器策略、环境配置、时钟、重启待定、还原点、电源、启动状态、系统版本 |
| 代码定位 | 新增 `crates/contracts/src/adapters/policy.rs`；修改 `crates/contracts/src/adapters/mod.rs`（注册 `policy::SOURCES`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；素材键名取自 `domains/security.ps1`、`domains/applications.ps1`、`domains/startup.ps1`、`domains/system.ps1`、`domains/storage.ps1`、`collect-system.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；12 项新增测试；17 个 `scripts/check-*.cjs`；6 个契约示例退出码；构建零警告 |
| 结果 | 174 项 Rust 测试通过；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 3 节 |
| 下一步 | S16 网络与远程访问 |

## 1. 三种身份形态并存

本类一个类别里同时出现三种注册表身份形态，逐类型取目录声明的键，没有一律套用：

| 来源 | 形态 | 说明 |
| --- | --- | --- |
| `policy.configuration` | `value_key` | 按更安全的路径判到软件限制策略，否则判到 UAC 策略 |
| `apps.browser-policies` | `value_key` | `browser` 字段取自路径中厂商之后的那一段 |
| `environment.configuration` | `value_key` + `registry_registration` | 环境键 → 环境变量；`FileExts`/`UrlAssociations` → 文件关联 |
| `system.clock` | `native_path`（两种形态都是） | 注册表键取载荷的 `path`；字面量 `timezone` 项取其 item id |
| `system.reboot-pending` | `native_path` | 每个注册表键一条记录 |
| `system.restore-points` | `native_identifier` | `native_id` 取 item id（序号） |
| `power.configuration` | `native_identifier` + 无对应类型 | 电源计划 → `policy.power_plan`；注册表键无类型，见第 3 节 |
| `system.boot-state` / `system.version` | `native_path` | `path` 取 item id |

值级类型的作用域约定与 S12 一致（见 `adapter-source-survey.md` 第 3.5 节），两条线独立推导出同一结论。

## 2. 两处如实处理而非强行归位

- **`power.configuration` 的注册表键没有对应类型。** 它是合法素材（休眠与 Modern Standby 设置），但 `policy.power_plan` 声明的字段（GUID、名称、是否活动、电源来源）描述的不是一张注册表值表。该流返回 **Skipped 并带可见原因**，而不是硬塞进一个并不描述它的类型。这是本类唯一未映射的素材流，列入 S21。
- **重启待定对 `Session Manager` 键是推导出来的。** 该键是无条件读取的，所以它的存在本身不说明任何事；事实是 `PendingFileRenameOperations` 这个值在不在。另外两个专门的 `RebootPending`/`RebootRequired` 键则以「被产出」表达状态——`Registry-Key` 打不开的键不会产出。两处推导都在代码里写明，这是本类唯一不是直接读键的值。

另外，`shell.file_association.command` 恒为 `NotProvided`：该来源读的是关联键，不读它们指向的程序，载荷里根本没有命令行。

## 3. 遗留问题

1. **`policy.os_configuration` 目前是三条记录，不是一个组装出来的对象。** `system.version`（`operating-system`）、`system.boot-state`（`boot-state`）与 `system.reboot-pending`（注册表路径）各自声明不同的 `path`，素材里没有任何两处共有一个标识——与 `account.machine_identity` 不同，后者的身份明确取自采集作用域。因此 `path` 就是记录在自身来源内的身份。**若原意是组装成同一个操作系统对象，需要采集端给出共享标识**，这不是适配器能决定的。列入 S21。
2. **值级作用域的实现有两份**（本类与 S12），已记录在勘察文档第 3.5 节，待差别定清后收拢进引擎。
3. **目录中 `apps.browser-policies` 的旧说明已过期**（写着「其他内容摘要」）。采集端代码与 S10 勘察都确证 `$allowed` 不做过滤，因此该键下的每个值名都成为一个对象。实现按代码与勘察走，未沿用旧说明；目录文案的更正属 S03/S21 范围。
4. **本机未做真实采集实测**，键名来自 emit 声明的静态核对。
