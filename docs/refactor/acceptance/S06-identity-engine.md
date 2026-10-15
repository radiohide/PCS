# S06 验收记录：实现实例身份与生命周期引擎

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S06 · 待验收 |
| 实现范围 | R03 身份契约的持久化一半：从 `object_instances` / `source_bindings` 读回匹配依据、把四种身份结果写进库、按来源绑定归属记录、按 R03 生命周期规则推进状态。未做字段事实与动作（S07）、未接覆盖与预算（S08） |
| 代码定位 | 新增 `crates/storage/src/identity.rs`、`docs/refactor/acceptance/S06-identity-engine.md`；修改 `crates/storage/src/lib.rs`（模块声明、`apply_identity` / `conclude_absence` / `instance_summary` 三个接口）、`model/ownership.json`。数据库结构版本不变（S05 的 v7 已含所需表） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0（`x86_64-pc-windows-msvc`，取自 PATH）；`:memory:` 数据库。项目固定工具链在本机不存在，见 R09 记录第 8 节 |
| 验证过程 | `cargo test --workspace`；9 项新增测试（第 3 节）；17 个 `scripts/check-*.cjs`；6 个契约示例退出码 |
| 结果 | 104 项 Rust 测试通过（较 S05 的 95 增加 9 项，均为本步新增）；17/17 检查通过；6 个示例退出码 0；7 条验收条件逐条见第 3 节 |
| 遗留问题 | 见第 5 节 |
| 下一步 | S06 通过后进入 S07（事实、动作与当前投影），它按 `source_bindings` 找实例并写字段事实 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 身份模块 | `crates/storage/src/identity.rs`——`DbLookup`（`InstanceLookup` 的库实现）、`apply`、`conclude_absence`、`summary` |
| 生命周期状态机 | 直接采用 R03 的 `identity::lifecycle`，本模块只负责把结论落到 `lifecycle` / `last_absent_at` 列，不另写一套规则 |
| 边界样例 | `crates/storage/src/identity.rs` 测试模块 9 项、`lib.rs` 的 3 个 Store 接口 |

## 2. 设计要点

- **不猜。** 歧义与未解析的候选**不写任何实例行**：在多个匹配里挑一个等于凭空合并，而没有任何来源这么说过。
- **绑定优先于键。** R03 规定来源绑定先于键匹配解析，因为「来源已经说明这些记录是同一条记录」。这不只是实现细节，它是一条身份规则：**来源记录即使报告的键变了，也仍是同一个实例**。第 4 节记录了由它引出、需要在接真实来源时逐来源确认的一个问题。
- **首见不是创建。** `first_observed_at` 记的是 PCScope 第一次看到它，`created_at` 只有来源真的说了才写；重复观察不会把首见时间前移（有测试）。
- **换代是新增一行。** 键在 ended/removed 之后重新出现，按 R03 是新一代，写 `incarnation+1` 的新行而不是复活旧行——旧实例的历史属于旧对象。

## 3. 验收条件对照

| 计划验收 | 结果 |
| --- | --- |
| 同一实例多个来源可归一 | 通过：`two_sources_converge_on_one_instance`——两个来源得到同一 `instance_id`，各自一条开放绑定 |
| 同名不同作用域不合并 | 通过：`the_same_name_under_another_scope_is_not_merged`——作用域是身份键的一部分，两个实例互不影响 |
| PID 复用、任务删除重建、用户改名按规则处理 | 部分通过：删除重建见 `a_key_that_reappears_after_removal_is_a_new_generation`（换代 +1，新行）；用户改名依赖类型策略的规范化，由 R03 的边界样例覆盖。**PID 复用未在本步实测**，见第 5 节第 1 项 |
| 缺少创建时间时不虚构 | 通过：`a_creation_time_is_never_invented`——`created_at` 保持 NULL，且后续观察不会把 `first_observed_at` 前移 |
| 不完整快照和名单变化不能确认删除 | 通过：`an_incomplete_observation_cannot_conclude_absence`——不完整观察、停用项、空采集范围三种情形都返回 `Unchanged`；只有完整且覆盖了该范围的观察才推进，且推进到 `not_observed` 而非删除 |
| 重复处理不新增实例 | 通过：`re_applying_a_request_changes_nothing`——同一请求两次，实例数不变、第二次不再报告新建、开放绑定仍为 1 |
| 歧义候选不产生实例 | 通过：`an_ambiguous_candidate_writes_nothing`——实例数为 0，理由保留在返回的 `transition` 里 |

## 4. 过程中修正的问题

**键变化时的归属：测试与冻结契约不符，改的是测试。** 我原本写了一条测试，断言「同一来源记录换了路径就归到另一个实例，旧绑定关闭」。实际行为是归到**同一实例**——因为 R03 明确规定来源绑定先于键匹配，且这条优先是有意的（「来源已经说明这些记录是同一条记录」）。

顺带发现我据此写的「关闭旧绑定」分支**不可达**：解析器总是先看同一三元组的开放绑定，所以走到 `bind` 时不可能存在一条指向别处的开放绑定。按「不留占位」的要求，删掉了那段死代码，并把不变量写进注释——看起来像保护却永远不执行的代码，比没有更糟。

## 5. 遗留问题

1. **来源复用 `native_record_id` 时，「绑定优先」是否总是正确，需要逐来源确认。** 例如一个进程来源若在 PID 复用时沿用同一个 `native_record_id`，绑定优先会把新进程接到旧实例上。R03 的 PID 复用边界样例是通过**不同的** `native_record_id`（含创建时间）表达的，所以契约本身没被证伪；但**真实来源是否都这样给记录号，只有接上 S09–S20 才知道**。这是本步最需要后续验证的一条，已在 S21 的对账清单里登记。
2. **缺席判定尚未接到采集循环。** `conclude_absence` 已实现并被测试，但还没有人按「完整快照结束后」的时机去调用它；接线属 S07/S08。
3. **实例的 `native_key` 在创建时固定。** 键变化后旧键仍在行上，后续解析靠绑定而不是靠键。这是契约的行为，但意味着若有来源绕过绑定直接按键查，会查不到——查询层（S22）需要知道这一点。
4. **本机未做真实采集实测**，全部验证在 `:memory:` 上构造候选完成。

## 6. 下一步

- S07 在本步的实例上写字段事实与动作，并消费 S05 的 `projection_queue`。
- S08 接入覆盖状态，并把 `conclude_absence` 接到完整观察结束的时机。
- 第 5 节第 1 项（来源记录号是否足以区分复用）并入 S21 全来源对账。
