# R02 验收记录：冻结标准适配器契约

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | R02 · 待验收 |
| 实现范围 | 契约版本 1：`EvidenceInput` / `AdapterContext` / 三类候选 / 四态结果 / 校验器 / 契约 schema；两个结构不同的示例适配器。目录内嵌从 `pcscope-storage` 上移到 `pcscope-contracts`，使存储、适配与界面共用一份定义 |
| 代码定位 | 新增 `crates/contracts/src/model.rs`、`crates/contracts/src/adapter.rs`、`crates/contracts/examples/adapter-contract.rs`；修改 `crates/contracts/src/lib.rs`、`crates/contracts/Cargo.toml`（+sha2）、`crates/storage/src/model.rs`（改为委托）、`Cargo.lock`（+1 行） |
| 验证环境 | Cargo / rustc 1.98.1（`x86_64-pc-windows-gnu`）；样例取自 S02 的 `fixtures/` |
| 验证过程 | `cargo test --workspace --offline --locked`；`cargo build --example adapter-contract`；`target/debug/examples/adapter-contract.exe fixtures` |
| 结果 | 58 项 Rust 测试通过（较 R01 基线 +11：适配器 9、目录 2）；示例 9 项验收检查全过，退出码 0；13 个既有检查通过 |
| 遗留问题 | 见第 6 节 |
| 下一步 | R03：冻结身份解析接口 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 共享 Rust 类型 | `crates/contracts/src/adapter.rs` |
| 序列化格式 | 同一文件的 serde 定义 + `contract_schema()`（可校验的枚举集合） |
| 适配器函数签名 | `trait SourceAdapter { fn adapter_id(&self) -> &'static str; fn adapt(&self, input: &EvidenceInput, context: &AdapterContext) -> AdapterResult; }` |
| 示例 | `crates/contracts/examples/adapter-contract.rs`（两个结构不同的来源） |
| 契约说明 | `docs/refactor/adapter-contract.md` |

## 2. 两个结构不同的来源使用同一接口

| 适配器 | 材料形态 | 产出 |
| --- | --- | --- |
| `adapter.startup.scheduled_task` | 快照条目：一个输入是一个完整对象，字段嵌在 payload 里 | 3 条观察（一条一个任务实例） |
| `adapter.eventlog.security` | 事件流：一个输入是一条扁平事件 | 7 条动作（4688/4689/4624/4634/4625/4720/4672） |

两者实现同一 trait，输入输出形态完全不同，证明接口不偏向任何一种来源结构。

## 3. 九项验收检查（实测输出）

```text
[通过] one interface, two structurally different sources
       adapter.startup.scheduled_task produced 3 observations; adapter.eventlog.security produced 7 actions
[通过] candidates validate against the catalog            no violations
[通过] unknown fields stay in the raw evidence and do not leak into candidates
       raw payload unchanged: true; present in candidates: false
[通过] a missing field is recorded absent and carries no value
       status NotProvided, value None
[通过] the same input yields the same candidates          byte-identical on re-run
[通过] failures and non-objects carry an explicit outcome
       not-an-object: NotAnObject{...}; failed: Failed{..., retryable: false};
       unclassifiable event produced 0 candidate(s) and a stated reason
[通过] an identity the source cannot complete is marked, not invented
       event 4688 without a process id -> missing keys ["pid", "created_at"]
[通过] an unregistered type is rejected                   7 violation(s)
[通过] the contract schema reports one version and the live catalog
       contract 1 against catalog "1.0.0"
```

对应的计划验收条件：

| 计划要求 | 覆盖 |
| --- | --- |
| 至少两种结构不同的来源使用同一接口 | 检查 1 |
| 未知字段保留在原始证据 | 检查 3（`adapt` 只拿 `&EvidenceInput`，结构上不可能改写原文） |
| 缺字段不填猜测值 | 检查 4 + 校验规则（非 known 状态带值即违规） |
| 重复输入得到等价候选 | 检查 5（逐字节一致） |
| 失败有明确结果 | 检查 6（四态 outcome + 原因 + 可重试标志） |
| 候选不可包含未登记类型或字段 | 检查 2、8 |

## 4. 单元测试覆盖的拒绝规则

`crates/contracts/src/adapter.rs` 的 8 项测试逐条验证「错误必须被拒绝」，而不只是「正确输入能通过」：

未登记类型、未声明字段、非 known 状态却带值、known 状态却不带值、区间动作声明精确时间、动作挂到错误类型、无证据候选、已 adapted 却无候选无问题、未归属动作无原因、上下文目录版本不符、序列化往返与确定性。

其中 `complete()` 在写示例检查时被发现语义不对：它原先只看「有没有高于 note 的问题」，于是 `Failed{retryable:false}` 和 `NotAnObject` 都会被判为 complete。**这是推演发现的，不是测试抓到的**——当时没有测试覆盖它。已改为必须 outcome 为 `adapted`，并补上 `only_an_adapted_result_without_problems_is_complete` 把这个语义固定下来，避免同类问题再次靠推演发现。

## 5. 目录内嵌上移

原 S04 把 `model/catalog.json` 内嵌在 `pcscope-storage`。契约需要按目录校验候选，而适配层不应依赖存储层，因此把内嵌上移到 `pcscope-contracts::model`，存储改为委托：

- 单一内嵌点，不产生两份 197 KB 副本；
- `sha2` 加入 contracts 依赖，`Cargo.lock` **只增加 1 行**（未引入新包）；
- `scripts/check-model-catalog.cjs` 的消费者检测随之指向 `crates/contracts/src/model.rs`（实测「Version consumers found: 1」）。

## 6. 遗留问题

1. **逐来源的适配器尚未实现。** 本步只交付接口与两个示例。S09–S20 的每一类都要按契约实现真实适配器，并补齐「缺字段 / 歧义」样例。
2. **事件到动作的映射是适配器本地知识。** 示例中的 `security_action()` 是手写表；正式实现应与 `apps/desktop/ui/event-items.js` / `event-semantics.js` 的既有映射对账，避免两处定义漂移（属于 S21）。
3. **契约未接入任何调用方。** `validate` 目前只在测试与示例中调用；S05 的证据入口与 S07 的投影才是真实调用点。
4. **`evidence_refs` 表与 `EvidenceRef` 尚未打通。** 表已存在（S04），契约产出的引用还没有写入路径。属于 S07。
5. **`decoding` 的上游尚未产出。** 现有采集器不报告解码可用性，S05/S09–S20 需要补。
6. **未验证「两种来源提出同一实例」的归一路径。** 那需要 S06 的身份引擎；本步只保证两侧的候选都合法且可比较。

## 7. 下一步

R03 的前置（R02）已满足。R03 需要冻结身份解析接口：输入、已确认/暂定/歧义/未解析四类结果、业务范围与采集范围的边界，以及五类样例（用户改名、任务不同路径、PID 复用、文件路径复用、同实例多来源）。
