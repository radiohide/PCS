# S05 验收记录：实现证据入口与协议兼容

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S05 · 待验收 |
| 实现范围 | 数据库结构版本 7：新增 `evidence` 与 `projection_queue` 两张表；每次入库在同一事务内为每份材料登记稳定证据地址与投影工作项；`Store::evidence_inputs` 从库里读回 R02 的 `EvidenceInput`（`reference::evidence_from` 的存储版）；`Store::evidence_queue` 汇报积压。v1 消息格式与采集端未改动 |
| 代码定位 | 新增 `crates/storage/src/evidence.rs`、`docs/refactor/acceptance/S05-evidence-ingress.md`；修改 `crates/storage/src/lib.rs`（迁移块、入库事务、查询接口、6 项测试）、`crates/storage/src/model.rs`（`SCHEMA_VERSION` 6 → 7）、`model/ownership.json`（新文件归属基础数据线） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0（`x86_64-pc-windows-msvc`，取自 PATH）；`:memory:` 数据库。项目固定工具链在本机不存在，见 R09 记录第 8 节 |
| 验证过程 | `cargo test --workspace`；6 项新增测试（见第 3 节）；17 个 `scripts/check-*.cjs`；6 个契约示例退出码 |
| 结果 | 95 项 Rust 测试通过（较 S04 的 89 增加 6 项，均为本步新增）；17/17 检查通过；6 个示例退出码 0；6 条验收条件逐条见第 3 节 |
| 遗留问题 | 见第 5 节 |
| 下一步 | S05 通过后进入 S06（实例身份与生命周期引擎）；S07 消费本步建立的队列 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 证据与队列结构 | `crates/storage/src/evidence.rs` 的 `ensure` |
| 兼容适配器 | 同文件的 `inputs`（存储 → `EvidenceInput`）、`record_snapshot_items`、`record_event` |
| 证据查询基础接口 | `Store::evidence_inputs`（带 `after_seq` 可续读）、`Store::evidence_queue` |
| 事务验证 | `crates/storage/src/lib.rs` 测试模块「S05 evidence ingress」6 项 |

## 2. 结构

新增两张表，都是纯增量，不改动 `records`、`snapshots`、`objects` 与用户设置：

```sql
evidence(seq AUTOINCREMENT, evidence_id, item_id, kind, source, structure_version,
         source_scope, native_record_id, source_time, observed_at, ingested_at,
         decoding, payload_ref, pruned_at, UNIQUE(evidence_id,item_id))
projection_queue(queue_id, evidence_id, item_id, projection, state, attempts,
                 last_error, enqueued_at, updated_at, UNIQUE(evidence_id,item_id,projection))
```

三个设计点：

- **地址与位置分开。** `evidence_id` 是来源对这条记录的**身份**（事件为 `source|epoch|event_id`，快照为 `source|scope|run_id`），`payload_ref` 是这份材料在本库里**存放的位置**（`record:<seq>` 或 `snapshot:<source>|<scope>|<run>|<item>`）。两者是不同的东西：来源换代时地址变化，而位置始终指向真正存了材料的那一行。
- **材料只存一份。** 证据行不含 payload，只存指向它的地址。大段清单或 XML 存一次，被引用它的每个字段共享（对应重构计划 §7.3）。
- **地址即去重键。** 重放同一批材料时 `ON CONFLICT DO NOTHING`，证据与工作项都不再写第二次；`projection_queue` 的唯一键让「同一份材料在同一投影上只有一个工作项」。

## 3. 验收条件对照

| 计划验收 | 结果 |
| --- | --- |
| 重放相同批次不重复产生证据和任务 | 通过：`replaying_a_batch_registers_neither_evidence_nor_work_twice`——快照两次入库后证据与待办各为 2，重放后不变；事件同理 |
| 事务失败不推进断点 | 通过：`a_failed_transaction_advances_nothing`——同一事件身份换内容被拒，证据数不变且断点仍是 `cp-1` 而非该批带来的 `cp-must-not-land` |
| 旧插件无需立即更新即可运行 | 通过：v1 消息格式、解析器与采集端均未改动；S04 的 35 项存储测试在升版后逐项通过 |
| XML 等大内容不复制到每个字段 | 通过：`evidence_points_at_the_material_instead_of_copying_it`——单据里放一个标记串，证据行的三个字段都不含它，而材料仍能经 `evidence_inputs` 原样取回 |
| 日志重置后记录 ID 复用不碰撞 | 通过：`a_reset_log_reuses_an_event_id_under_a_new_epoch`——同一事件 id 在新 epoch 下产生 `log|2|1`，与旧的 `log|1|1` 并存互不覆盖 |
| 敏感数据不因新入口绕过保护 | 通过（结构性）：证据行不存材料本身，因此新入口没有引入新的暴露面；采集端在发送前是否按既有策略加密属 S09–S20 逐来源核查。第 5 节列为待续事项 |

## 4. 过程中修正的问题

**迁移块会跳过自己。** `version < 6` 那块原本写的是 `PRAGMA user_version=<当前常量>`。`SCHEMA_VERSION` 一升到 7，从 v5 升上来的库就会在跑完 v6 的结构后被直接标成 7，**跳过 v7 的建表块**——库自称版本 7 却没有 `evidence` 表，直到有人读它才会暴露。已改为每个块盖自己迁移到的版本号（v6 块盖 6），v7 块再盖当前常量。

这个缺陷不是本步引入的，但它只在版本号第二次递增时才会显形，所以第一次递增就必须修掉。

## 5. 遗留问题

1. **v1 协议没有承载采集端读取问题的字段。** `Decoding` 有 `problems` / `unreadable_fields` / `masked_fields`，但 v1 信封里无处表达它们；采集端目前只能用 `gap` 消息声明整段缺口。因此本步入库的 `decoding` 恒为「已解码、无问题」——这是**来源先天不提供**，不是未实现（验收原则允许保持未知）。要让逐项读取失败进库，需要先扩协议，属协议版本工作。
2. **队列只登记、尚未有人消费。** 待办项由 S07 的投影引擎取走；在那之前 `pending` 会随采集单调增长，属于预期。
3. **快照证据读回要重新解析整份快照。** 同一 run 的连续 item 共用一次解析，但单份大清单仍会整体读入内存；这是协议文档已列的「流式快照」待办，不是本步遗漏。
4. **本机未做真实采集实测。** 全部验证在 `:memory:` 上用样例消息完成；真实插件链路要等 S09–S20。

## 6. 下一步

- S06 可在本步的证据地址上实现持久身份与生命周期。
- S07 直接消费 `projection_queue`，把 `evidence_inputs` 交给适配器。
- 计划验收第 6 条的逐来源结论并入 S21 的全来源对账。
