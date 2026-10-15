# 查询契约与模拟服务（R05 冻结）

日期：2026-09-12；兼容展示扩展更新于 2026-10-12。状态：**基础契约版本 1 冻结，展示扩展修订 2**。代码：`crates/contracts/src/query.rs`；固定响应集：`model/query-samples.json`；模拟适配层：`apps/desktop/ui/query-mock.js`；校验：`scripts/check-query-contract.cjs`。

依据：[并行开发前置条件与准入方案](parallel-readiness.md) R05；[全量对象模型重构计划](../full-object-refactor-plan.md) S22、S23–S25。

R05 是 S22 的**提前实现**：本步冻结请求/响应结构与固定响应集，**实际查询、索引与真实页面集成仍由 S22–S25 完成**。

## 1. 请求

七类查询，请求体是 `{"query": "<名称>", …}`：

| 请求 | 关键参数 |
| --- | --- |
| `type_catalog` | 无 |
| `instance_list` | 筛选、排序、游标、页大小 |
| `instance_detail` | 实例 alias 或 ID |
| `instance_actions` | 实例、游标、页大小 |
| `action_timeline` | 动作、类别、类型、来源、实例、是否含未归属、时间下界、游标、页大小 |
| `evidence_detail` | 证据引用 |
| `source_coverage` | 可选来源 |
| `projection_status` | 无 |

### 1.1 筛选与排序

筛选字段：类别、类型、搜索、作用域、身份状态、采集状态、来源、生命周期。

排序取值：`name_asc`（默认）、`name_desc`、`changed_desc`、`seen_desc`、`type_asc`。

### 1.2 默认可见性

- **默认返回 `active` 与 `not_observed` 的实例**；
- `ended` 与 `removed` 属历史，**必须显式指定生命周期筛选**才会返回；
- **暂定身份（`provisional`）默认包含**，由 `identity_status` 标注，不隐藏——隐藏会让用户以为对象不存在。

### 1.3 分页

| 项 | 值 |
| --- | --- |
| 默认页大小 | 50 |
| 页大小上限 | 200（超出则**钳制**，不报错） |
| 游标 | 不透明字符串；缺省表示第一页 |
| 水位 | `page.watermark` 标识该游标属于哪次查询 |
| 已应用页大小 | `page.limit` 回传实际值，调用方据此知道真正取到多少 |

**跨水位复用游标必须被拒绝。** 这是「同一稳定查询水位下无重复/遗漏」的实现方式：水位变化后旧游标失效，而不是静默给出错位的页。

## 2. 响应

统一信封：契约版本、目录版本、状态、数据、警告、分页、计数、`mock` 标记。

### 2.1 七种状态

| 状态 | 含义 | 前端表现 |
| --- | --- | --- |
| `loaded` | 有数据 | 正常渲染 |
| `empty` | 查询成功但没有匹配行 | 空态，带筛选说明 |
| `unsupported` | 该类型/视图尚未实现 | 能力说明，**不是空列表** |
| `permission_denied` | 读取需更高权限 | 权限说明与建议 |
| `partial` | 部分来源成功、部分没有 | 渲染已有数据 + 警告列出缺失来源 |
| `evidence_pruned` | 结论在，但支撑证据已被保留策略清理 | 渲染结论 + 顶部横幅说明完整材料不可用 |
| `version_mismatch` | 前后端目录版本不一致 | 明确提示重新构建，不猜测字段 |

**除 `loaded` 与 `empty` 外，每个状态都必须带警告说明。** 校验脚本强制这一点：一个只说「没数据」而不说为什么不行的响应，会让用户以为对象不存在。

### 2.2 计数分离

`counts` 分别报告 `instances`、`source_records`、`actions`、`evidence`，**不提供合计字段**——避免把对象数和证据数相加。

### 2.3 字段由后台解释

`FieldView` 携带 `label`、`kind`、`status`、`value_type`、`derivation`，全部来自后台。**前端无需猜测字段含义**，也不需要自带一份中文名映射。

### 2.4 可选易读展示扩展（修订 1 与 2）

按[共享变更 0010](shared-changes/0010-readable-presentation.md)增加以下可选字段。基础响应的 `contract_version` 仍为 `1`；`response_shape()` 用 `presentation_revision: 2` 与 `presentation_extensions` 声明展示能力，不触发全局数据库或契约版本迁移。

| 位置 | 展示字段 | 约束 |
| --- | --- | --- |
| `InstanceSummary.presentation` | `name`、`description`（可选）、`name_status`、`basis` | 名称状态为 `source_provided`、`not_provided` 或 `dictionary_matched`；安装登记及字典匹配都不能据此解释为已安装或仍在使用。 |
| `ActionView.context` | `type_id`、`type_name`、`display_name`、`key_facts`、`presentation`、`subject_label`、`subject_detail`、`subject_basis`、`field_label`，以及可选的 `program_label`、`logon_type` | 优先使用该条事件自己的名称，完整名称供详情使用；当前对象名称只能作为明确标注的后备。 |
| `ActionView.context` 的关联核验 | `attribution_status`、`attribution_reason`、`stored_instance_id`；可选 `classification_status`、`stored_action_id` | 归属状态为 `verified`、`unverified`、`mismatch`、`unassigned` 或 `stored`。未核实或不一致的关联不能当作确定对象链接，历史保存标识和分类只供追溯。 |

`subject_basis` 为 `event_evidence`、`current_object_facts` 或 `unavailable`。受保护、不可读或已清理的原名不能转入展示字段。`classification_status: "unverified"` 表示历史分类未通过来源核验，保留旧分类供详情检查。

修订 2 依照[共享变更 0011](shared-changes/0011-registration-dictionary.md)增加字典名称，仍兼容省略展示字段的旧响应；字典内容自身版本独立计算。`dictionary_matched` 仅解释已匹配登记的性质，`basis` 为 `registration_dictionary`。对应的 `dictionary_id`、`dictionary_revision`、`dictionary_entry_id`、`match_basis`、`technical_key`、`references` 作为可选追溯信息进入折叠详情。真实来源名称始终优先；受保护、冲突或不可读取的名称不能通过字典补出。默认界面显示简短用途和“按登记识别”，不把匹配结果当作软件已安装或正在使用的证据。同名登记仍保留独立对象身份、选择值和详情入口。

两个扩展在 Rust 契约中均为 `Option<Value>`，使用 `serde(default, skip_serializing_if = "Option::is_none")`：旧后台省略它们仍可读取；无扩展时序列化继续省略；旧消费者可忽略新字段。扩展内部未来增加字段也应按可选字段处理，不因缺少扩展而把已有可读记录判为版本不匹配。

## 3. 必须可展示的情形

固定响应集覆盖以下情形，并由校验脚本强制其存在：

| 情形 | 承载字段 |
| --- | --- |
| 身份暂定 | `identity_status: "provisional"`，且 `instance_id` 可缺省而 `alias` 必有 |
| 动作区间 | `precision: "interval"` + `interval_from`/`interval_until`，且 `time_derived: true` |
| 多证据 | 一条动作引用多个证据 |
| 遮罩 | `status: "masked"`，值与真实值同存于后台 |
| 冲突 | `status: "conflicting"` + `conflicts[]` 保留各来源候选 |
| 证据已清理字段 | `status: "evidence_pruned"` |
| 未归属动作 | `unassigned_reason` 非空 |
| 覆盖状态 | `configured` 与 `last_result` 分开，含 `denied`、`missing`、`never` |

`alias` 始终存在、`instance_id` 可缺省——这让暂定实例在尚未分配 ID 时也能被引用（与身份契约第 6 节一致）。

## 4. 模拟服务

`model/query-samples.json` 是固定响应集，由 `crates/contracts/examples/query-contract.rs` 生成。生成器**在覆盖不全时拒绝写出**：缺少任一状态或任一可展示情形都以非零退出码失败。

原 15 个样本名称与路径保留；`instance_list`、`action_timeline` 刻意省略展示扩展。另增加 `instance_list_presentation` 与 `action_timeline_context`，覆盖来源命名/未命名登记、合成的字典匹配登记、事件当时名称与当前名称不同、受保护账户名称与未归属动作。生成器通过公共 Rust 结构反序列化并再次序列化这四组新旧响应，要求完全一致；契约单元测试另验证可选未知字段往返和旧消费者忽略新字段。所有覆盖与往返检查通过后才写出固定集。

`apps/desktop/ui/query-mock.js` 是无数据的开发适配层：

- **不内置任何样本**，样本由调用方显式传入；
- 未显式开启时**拒绝运行**（需 `globalThis.PCSCOPE_QUERY_MOCK`）；
- 样本必须全部带 `mock:true`，空集被拒绝；
- 未覆盖的请求**报错**，不静默返回空结果；
- 返回深拷贝，页面无法污染固定集。

### 4.1 正式构建不会误连

两条约束由 `scripts/check-query-contract.cjs` 强制：

1. **除 `query-mock.js` 外，`apps/desktop/ui/` 下任何文件都不得引用模拟层或样本集**；`index.html` 尤其不得引入。
2. 模拟层在未显式开启时必须抛错，开启后才返回样本——**由实际运行 vm 上下文验证**，不是读代码推断。

另有 `mock: true` 标记：每条模拟响应都带，真实响应不带。校验脚本对两者都检查。

## 5. 不在本步范围

| 事项 | 归属 |
| --- | --- |
| 实际查询实现、窄索引、先过滤后计数分页 | S22 |
| 真实页面的接入与交互 | S23–S25 |
| 与真实响应的一致性验证 | S22 完成后 |
| 全目录覆盖验证 | S21 |
