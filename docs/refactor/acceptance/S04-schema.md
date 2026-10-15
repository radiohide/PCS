# S04 验收记录：实现增量数据库结构

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S04 · 待验收 |
| 实现范围 | 数据库结构版本 6：新增 12 张表（实例、来源绑定、字段事实与历史、动作、证据引用、按作用域覆盖、投影状态、目录快照）；引用清理行为；结构版本与规则版本分离；旧库与空库升级。未修改 `records`、`snapshots`、`objects` 与用户设置 |
| 代码定位 | 新增 `crates/storage/src/model.rs`（结构、目录生成、引用清理、8 项测试）；修改 `crates/storage/src/lib.rs`（迁移块、读写版本校验、保留标记）、`crates/storage/src/search_index.rs`（移除版本号写入）、`crates/contracts/src/lib.rs`（`MODEL_CATALOG_VERSION`）、`scripts/check-model-catalog.cjs`（消费者检测）；新增 `crates/storage/examples/schema-info.rs` |
| 验证环境 | Windows 11 Enterprise LTSC 26100（x86_64）；Cargo / rustc 1.98.1，`x86_64-pc-windows-gnu`；工具链位于 `SYS/pcscope/.tools`；真实升级验证使用 S02 基线库副本（由改动前的二进制建立） |
| 验证过程 | 见第 3–6 节 |
| 结果 | 47 项 Rust 测试通过（新增 8 项）；13 个既有前端与目录检查全部通过；真实 v5 库升级成功且保留数据不变 |
| 遗留问题 | 见第 8 节 |
| 下一步 | S04 通过后进入 S05（实现证据入口与协议兼容） |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 增量迁移代码 | `crates/storage/src/model.rs`、`crates/storage/src/lib.rs` 的 `version < 6` 迁移块 |
| 逻辑结构说明 | `docs/refactor/object-model-schema.md` |
| 迁移验证 | `crates/storage/src/model.rs` 测试模块、`crates/storage/examples/schema-info.rs`、第 5 节的真实库升级 |
| 兼容性声明 | `docs/refactor/object-model-schema.md` 第 4 节 |

## 2. 结构规模

```text
新增表            12
目录类型          101   （来自 model/catalog.json）
目录动作          71
类型-动作关联     52
来源映射          129
投影状态行        4     （objects / facts / actions / coverage）
```

## 3. 空库与已有库均能升级

| 场景 | 结果 |
| --- | --- |
| 空库 | 打开后 `user_version=6`；12 张表全部存在；目录表按 `model/catalog.json` 填充，数量逐项相符（101/71/52/129） |
| 已有 v5 库（测试内构造） | 升级到 6；保留表逐行内容不变；用户设置不变 |
| 已有 v5 库（**真实**，见第 5 节） | 升级到 6；`records` 35840、`snapshots` 64、`objects` 14400、`event_ids` 20000、`checkpoints` 4、`batch_heads` 4 全部不变 |

## 4. 事务中途失败不留半套结构

测试 `failed_migration_rolls_back_and_leaves_the_previous_version` 在结构创建完成、版本号提交之前注入一次失败：

- 迁移返回错误；
- `PRAGMA user_version` 仍为 5，**未推进**；
- 表清单与失败前逐项相同，**没有留下任何新表**。

结构创建与版本号写在同一个事务里，提交或整体回滚。

## 5. 真实数据库升级验证

S02 建立的基线库（`target/s02-release-scaled/run-a.db`）由**改动前**的二进制写出，是真实的 schema 5 库。复制一份后用重建的 `pcscope-cli.exe` 打开，走的是服务与界面相同的迁移路径。

升级前：

```text
schema_version 5     records 35840   objects 14400   snapshots 64
model tables: 全部 absent
settings keys: pruned_units, quota_bytes, retention_mode, search_catalog_v1
```

升级后：

```text
schema_version 6     records 35840   objects 14400   snapshots 64   （不变）
catalog_version 1.0.0
object_instances … coverage_scopes    present, 0 行（尚未投影，符合 S04 范围）
projection_state                      present, 4 行
type_definitions 101 / action_definitions 71 / type_actions 52 / source_map 129
settings keys: model_catalog_v1, pruned_units, quota_bytes, retention_mode, search_catalog_v1
```

升级只新增一张设置项（`model_catalog_v1`），其余设置不变。

**重复打开**：第二次打开后状态逐项相同。数据库文件哈希会变化——这是 SQLite 打开时对文件头与 WAL 的正常写入，不是重新迁移；目录表未被重建成证据由测试 `reopening_does_not_remigrate_or_reload_the_catalog` 给出（写入哨兵值后重开，哨兵仍在）。

## 6. 引用清理与版本校验

| 检查 | 结果 |
| --- | --- |
| 证据被清理时引用被标记 | `pruning_marks_references_and_keeps_the_conclusion`：滚动清理后引用 `pruned_at` 非空，字段事实的值仍在，被清理的记录确实已删除 |
| 主体删除后引用不残留 | `orphan_references_are_removed_with_their_subject`：删除实例后引用归零 |
| 未归属动作必须写明原因 | `actions_must_be_assigned_or_explicitly_unassigned`：既无实例又无未归属原因的动作被 CHECK 约束拒绝 |
| 更新版本被写入方拒绝 | `newer_database_is_refused_by_writer_and_reader` |
| 更新版本被只读方拒绝 | 同上（`open_reader` 原先不做版本校验，本步补上） |
| 目录版本不一致被拒绝 | `scripts/check-model-catalog.cjs` 对 `crates/contracts/src/lib.rs` 的声明校验：改为 9.9.9 时退出码 1 并指出文件与两个版本号 |

### 6.1 同一目录版本的三方一致

S03 记录「前后端使用同一目录版本」只做到「可检查的基础、未完成接线」。本步加入第一个真实消费者后，同一份目录指纹在三处出现且一致：

| 位置 | 值 |
| --- | --- |
| `model/catalog.json` 的 SHA-256 | `348dbde1040d53eab0cfff1008a0f31234b166bea3c8cd572a35fd32f8ccde04` |
| Rust 内嵌计算并写入 `settings.model_catalog_v1` | 同上（真实升级验证中读出） |
| `docs/refactor/parallel-boundaries.md` 声明的输入摘要 | 同上 |

目录版本号 `1.0.0` 也在 `model/catalog.json`、`pcscope_contracts::MODEL_CATALOG_VERSION` 与 `scripts/check-model-catalog.cjs` 之间逐一对齐，任一处不一致即检查失败。

## 7. 过程中修正的三个缺陷

1. **结构版本有两个写入方。** `search_index::ensure` 硬编码 `PRAGMA user_version=5`，而它在迁移之后运行，把新版本号覆盖回 5（首轮测试即以「期望 6、实际 5」暴露）。派生表的重建是随时可能发生的指纹驱动操作，不应写结构版本；已移除该写入，结构版本现在只由迁移块写。
2. **目录消费者检测正则漏匹配。** `scripts/check-model-catalog.cjs` 只匹配 `KEY: "值"` 与 `KEY = "值"`，而 Rust 的写法是 `KEY: &str = "值"`，中间隔着类型标注。S03 时消费者数量为 0 所以没暴露；本步加入第一个真实消费者后才显示「未找到」。已改为容忍类型标注，并实测：声明 9.9.9 时退出码 1，回到 1.0.0 时通过。
3. **只读路径没有版本校验。** `open_reader` 原样打开任意版本，可能把只理解一部分的模型当作完整模型展示。已补上与写入方一致的拒绝逻辑。

## 8. 遗留问题

1. **新表未纳入容量预算。** 滚动模式下 `object_instances`、`object_facts`、`fact_history`、`object_actions`、`evidence_refs` 等表不计入 `maintain_capacity` 的配额计算。在此之前不能声称「新结构参与预算」。属于 S08，且计划 12.4 节的门槛要等它完成才谈得上。
2. **只读方在迁移窗口内仍会报 schema 错误。** 这是 S02 已记录的行为：写入方建表期间读取方得到 `no such table`。本步未改变它，服务进程内的顺序使其不暴露给界面，但跨进程与升级窗口存在。属于 S05/S08 的范畴。
3. **迁移前无自动备份。** 兼容性声明要求升级前做一致性备份，但没有强制或自动执行。计划 10.3 节的备份与回退演练属于 S32。
4. **投影状态尚未推进。** `projection_state` 只写入规则版本与结构版本，游标与水位为空；真正的投影进度属于 S07。
5. **`coverage_scopes` 与既有 `coverage_state` 并存。** 两者职责不同（按来源作用域的覆盖 vs 按来源原因的缺口），但尚未定义汇总关系。属于 S08。
6. **目录表随目录变化整体重建。** 重建会删除 `type_definitions` 等四张表的全部行再插入。这在数据量上是 101/71/52/129 行，代价可忽略；但若将来有外键指向这些表，需要改为增量更新。当前没有外键指向它们。
7. **`check-item-controls.cjs` 等检查的硬编码数量未变**（216），本步未触及采集项目录。

## 9. 下一步

S05 的前置（S04）已满足：结构就位、版本机制可校验、升级路径在真实库上验证过。S05 需要实现证据入口与协议兼容：接收 v1 消息、为事件与快照建立稳定证据定位、原始入库与投影任务登记同事务完成，以及断点幂等。
