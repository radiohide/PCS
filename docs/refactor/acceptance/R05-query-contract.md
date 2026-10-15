# R05 验收记录：冻结查询契约与模拟服务

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | R05 · 待验收 |
| 实现范围 | 契约版本 1：七类查询请求、统一响应信封、七种状态、筛选与排序、稳定游标与水位、页大小钳制、计数分离、实例/动作/证据/覆盖/投影的展示结构；固定响应集与开发模拟层。未实现真实查询（属 S22） |
| 代码定位 | 新增 `crates/contracts/src/query.rs`、`crates/contracts/examples/query-contract.rs`、`model/query-samples.json`、`apps/desktop/ui/query-mock.js`、`scripts/check-query-contract.cjs`、`docs/refactor/query-contract.md`；修改 `crates/contracts/src/lib.rs` |
| 验证环境 | Cargo / rustc 1.98.1（`x86_64-pc-windows-gnu`）；Node v24.20.0 |
| 验证过程 | `cargo test --workspace --offline --locked`；`target/debug/examples/query-contract.exe`；`node scripts/check-query-contract.cjs` |
| 结果 | 85 项 Rust 测试通过（较 R04 +5）；固定响应集 12 个样本覆盖全部 7 种状态，退出码 0；校验脚本通过；14 个既有检查全部通过；四个契约示例均退出码 0 |
| 遗留问题 | 见第 5 节 |
| 下一步 | R06：建立共享样例与契约检查 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 查询契约 | `crates/contracts/src/query.rs`（请求、响应、展示结构、`response_shape()`） |
| 固定响应集 | `model/query-samples.json`（12 个样本，46 KB） |
| 模拟服务/适配层 | `apps/desktop/ui/query-mock.js` |
| 契约说明 | `docs/refactor/query-contract.md` |
| 校验 | `scripts/check-query-contract.cjs`（新增，第 14 个检查） |

## 2. 固定响应集覆盖

```text
query contract: 12 samples written to model/query-samples.json covering 7 distinct statuses
  type_catalog / instance_list / instance_detail / instance_actions
  evidence_available / evidence_pruned / source_coverage / projection_status
  instance_list_empty / unsupported / permission_denied / version_mismatch
```

七种状态全部出现。生成器在覆盖不全时**拒绝写出**并以非零码失败——这不是文档承诺，是可执行的出口条件。

计划要求的三类情形与对应覆盖：

| 计划要求 | 覆盖 |
| --- | --- |
| 明确筛选字段、默认顺序、稳定游标、查询水位和页大小上限 | `InstanceFilter`（8 个筛选字段 + 5 种排序）、`Page.watermark`、默认 50 / 上限 200 钳制 |
| 区分总对象数、来源记录数和动作数 | `Counts` 四个字段分离，**不提供合计** |
| 定义暂定及历史对象的默认可见性 | 默认返回 `active` + `not_observed`；`ended`/`removed` 需显式筛选；`provisional` 默认包含并标注 |
| 定义七种响应状态 | 见第 3 节 |
| 同一 schema 验证模拟响应与实际响应 | `response_shape()` 随固定集一同产出，校验脚本对样本逐条比对 |
| 模拟模式显式标记 | 每个样本带 `mock: true`；真实响应不带该字段 |
| 仅用于开发，不写用户数据库、不触发采集 | 模拟层是纯函数适配层，无 IPC、无数据库访问 |
| 正式构建不会误连模拟服务 | 第 4 节 |

## 3. 校验脚本检查的内容

`node scripts/check-query-contract.cjs`：

1. 每个样本带名称、`mock: true`、契约版本与目录版本一致；
2. 状态取值在契约声明之内；
3. **除 `loaded` 与 `empty` 外，每个状态都带警告说明**；
4. `empty` 仍带空数据集而不是缺失数据；
5. 七种状态全部出现；
6. 九项必须可展示的情形（暂定身份、动作区间、多证据、遮罩、冲突、证据已清理字段、未归属动作、分页水位、计数分离、覆盖结果）逐个存在；
7. 除 `version_mismatch` 样本外，所有样本声明的目录版本等于 `model/catalog.json` 的版本；
8. 模拟层隔离（见第 4 节）。

## 4. 模拟层的隔离与行为验证

| 检查 | 方式 | 结果 |
| --- | --- | --- |
| 正式界面文件不得引用模拟层 | 扫描 `apps/desktop/ui/` 下全部 js/html，排除 `query-mock.js` 本身 | 14 个文件，0 处引用 |
| `index.html` 不得引入 | 单独断言 | 通过 |
| 未显式开启时拒绝运行 | **在 vm 上下文中实际执行**并断言抛错，而非读代码推断 | 通过 |
| 空样本集被拒绝 | 同上，实际执行 | 通过 |
| 未标记 mock 的样本被拒绝 | 同上，实际执行 | 通过 |
| 显式开启后返回对应样本 | 同上，实际执行 | 通过 |
| 未覆盖的请求报错而非空结果 | 同上，实际执行 | 通过 |

**注入测试**：往 `apps/desktop/ui/settings.js` 追加一行对模拟层的引用，检查以退出码 1 失败并点名 `settings.js`；还原后通过。`git diff --numstat` 确认 `settings.js` 与提交内容逐字节相同。

## 5. 遗留问题

1. **未与真实响应做过一致性比对。** 计划要求「用同一 schema 验证模拟响应与实际响应」，本步只完成了 schema 与模拟侧；真实侧要等 S22 实现后才能真正比对。这是 R05 最实质的未完成项。
2. **模拟层未接入任何页面。** 它是可供开发使用的适配层，但 S23–S25 尚未开始，因此没有页面在用它。
3. **游标格式未定义。** 契约只规定它是「不透明字符串」且绑定水位；具体编码（排序键 + 二级排序键）属 S22。
4. **筛选字段的取值域未全部固定。** `coverage`、`scope` 的合法取值尚未声明，等 S22 接入真实数据后确定。
5. **`type_catalog` 样本的 `supplied_by_source` 恒为真。** 生成器目前用「是否身份对象」判断，没有读取目录中「尚未接入来源」的标记（`model/catalog.json` 未导出该字段，S03 的 `typesWithoutSource` 只在编写源里）。S22 接入时应对齐。
6. **未验证大页与并发分页。** 页大小钳制只做了取值层面的固定，没有在真实数据上验证「同一水位下无重复/遗漏」。

## 6. 下一步

R06 的前置（R05）已满足。R06 需要基于既有 `fixtures/` 补齐输入到标准候选、身份结果、查询响应的对应样例，并建立独立的契约检查与集成测试命令。R06 的最小样例集包含七项，其中「新快照先入库、旧日志后补采」与「一项来源停用、一个字段不可读、一份证据已清理」已分别由 R04 的场景覆盖，需要补做的是把样例串成端到端链路。
