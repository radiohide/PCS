# S21 验收记录：全来源覆盖对账与兜底

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S21 · 待验收 |
| 实现范围 | 机器可核对的来源与类型对账：适配器声明、来源映射、目录三方比对，差异必须逐条给出原因；补齐对账中发现的、当时可实现的来源（`software.portable`）；修正 USN 事件来源未接入事件适配器的路由缺口 |
| 代码定位 | 新增 `crates/contracts/examples/adapter-coverage.rs`、`scripts/check-adapter-coverage.cjs`、`model/adapter-exceptions.cjs`、生成物 `model/adapter-coverage.json`；修改 `crates/contracts/src/adapters/event.rs`（`is_event_source`、`EVENT_EXCLUSIONS`）、`crates/contracts/src/reference.rs`（声明优先于事件回退）、`crates/contracts/src/adapters/software.rs`（`software.portable`）、`model/ownership.json` |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；Node v24.14.0 |
| 验证过程 | `cargo test --workspace`；`adapter-coverage.exe` 生成清单；`node scripts/check-adapter-coverage.cjs`；18 个检查脚本；契约用例与演练输出重新生成后逐字节未变 |
| 结果 | 311 项 Rust 测试通过；18/18 检查通过；**128 个来源全部有明确处置**（96 个对象适配器 + 25 个事件规则 + 7 条例外）；**101 个类型中 98 个可从已处理来源到达，3 个显式标记未接入来源** |
| 遗留问题 | 见第 4 节 |
| 下一步 | S22 实现新查询与稳定分页 |

## 1. 对账是一个检查，不是一份文档

三方机器可读输入：

| 输入 | 来源 |
| --- | --- |
| 适配器声明了哪些来源 | `model/adapter-coverage.json`，由 Rust 示例写出（`adapters::all_specs()`） |
| 有哪些来源、各自去向 | `model/source-map.cjs` |
| 目录声明了哪些类型 | `model/catalog.json` |

**差异没有第三条路**：来源映射里的每个来源、目录里的每个类型，要么被覆盖，要么在 `model/adapter-exceptions.cjs` 里逐条给出原因。检查同时断言**例外表自身诚实**——条目过期（指向不存在的来源/类型）或**冗余（其实已被覆盖）**都会失败。冗余比缺失更糟：它读起来像一个已知缺口，实际却是好的，还会掩盖某个来源日后失去覆盖。

规则参数由 Rust 侧导出（事件前缀与排除清单），检查脚本读同一份，因此**两边不可能各自维护一套"哪些来源是事件"的规则**——这正是本轮踩过的坑（见第 3 节）。

## 2. 兜底：对账时发现并补上的两处

- **`software.portable` 现在可以实现了。** 它的身份是文件身份，此前被采集端不发该值阻塞；S14 的决定落地后，它只是同一对值的另一个使用者。已补齐，含两条测试（正常身份、以及**读不到身份时不产出对象**）。
- **USN 的事件来源此前没有接到事件适配器。** `pcscope.usn.{drive}` 发的是 `event_batch`，`kind` 是 `files.usn_changed`（目录中已登记的动作），但路由只覆盖 `pcscope.eventlog.*`，因此这些事件既没动作也没对象。已把 USN 的按盘来源接入统一事件适配器，并给事件适配器补上 `file.watched_file` 的身份字段映射。

  路由顺序同时调整为**声明优先**：`pcscope.usn.volumes` 发的是快照（是对象），不能因为共享前缀被当成事件。契约用例与演练输出重新生成后逐字节未变。

## 3. 过程中修正的问题：规则写了三份，各自漂移

事件来源的判定一开始在三个地方各有一份：Rust 的 `is_event_source`、检查脚本里的同名前缀判断、以及例外表里的人肉登记。结果是检查连续报出两条自相矛盾的结论——`usn.changes`（无采集端的虚拟组）被前缀规则"覆盖"、`usn.watch-list`（覆盖类来源）被误判为事件。

改成**规则参数由 Rust 导出**（`event_prefix`、`usn_prefix`、`event_exclusions` 一并写进生成物），检查脚本读该清单应用同一规则。改完之后三方计数第一次加得起来：96 + 25 + 7 = 128。

## 4. 遗留问题

1. **`pcscope.system.update-history` 需要一次目录决定。** 来源映射把它标为 `software.system_update` 的动作证据，但该类型没有声明任何动作，而它实际发出的是快照项。目前既不产出对象也不产出动作，素材留存。要么改映射（当对象来源），要么为它登记动作（改目录）——属目录范围。
2. **`pcscope.apps.dotnet-runtimes` 未实现**（唯一的未实现来源，其类型 `software.runtime` 已显式标记）。它需要先定「家族与版本各自从子键路径的哪一段取」。
3. **3 个类型没有任何来源**：`file.watched_directory`、`security.key_metadata`、`hardware.driver_package`，均已显式标记为未接入，未以空适配器充数。
4. **7 条例外的具体内容见 `model/adapter-exceptions.cjs`**（每条例外都带原因，且由检查强制不能被悄悄删除）。
5. **该对账核对的是"有明确处置路径"，不是"键名与真实输出一致"。** 键名核对属真实机器实测，本机未做；勘察文档 §5 已记录样本与采集端键名不一致的两处。
6. **F2 阶段出口条件**（「全部现有来源有明确、验证过的处理路径」）在**结构层面**达成；「验证过」的实机部分仍以本机未测为前提，遵循计划中「本机没有对应组件时使用样例验证并标注未做本机实测」的要求。
