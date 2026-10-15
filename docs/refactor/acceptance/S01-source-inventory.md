# S01 验收记录：盘点来源与既有功能

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S01 · 待验收 |
| 实现范围 | 全部 13 个执行单元、128 个已实现来源、23 个日志通道、216 个采集项开关；未做任何采集行为、运行配置或数据库修改 |
| 代码定位 | 新增 `scripts/check-source-inventory.cjs`；生成 `model/sources.json`、`model/regression-checklist.json` |
| 验证环境 | Windows 11 Enterprise LTSC 26100；Node v24.20.0；对 `a2311a4` 工作区源码静态核查，未启动后台、未读取用户数据库 |
| 验证过程 | `node scripts/check-source-inventory.cjs`；`node scripts/check-plugin-sources.cjs`；`node scripts/check-collection-view.cjs`；两次人为注入测试 |
| 结果 | 见第 3、5、6 节；三项检查通过，注入测试两次均被拦下 |
| 遗留问题 | 见第 8 节 |
| 下一步 | S01 通过后进入 S02（固定样例与性能基线） |

本文只记录盘点结论。未实测的数值一律留空，不预填。

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 来源清单（机器可读） | `model/sources.json` |
| 功能回归清单（机器可读） | `model/regression-checklist.json` |
| 目录一致性检查脚本 | `scripts/check-source-inventory.cjs` |
| 本验收记录 | `docs/refactor/acceptance/S01-source-inventory.md` |

## 2. 盘点口径

来源事实以**采集器代码**为准，不以目录为准。脚本从代码提取发射点：

| 采集器 | 提取方式 |
| --- | --- |
| `collect-system.ps1` | `Emit-Collection 'X'` 调用点 |
| `domains/*.ps1` | `Emit 'X'` 与双引号插值 `Emit "X.$($var.ToLowerInvariant())"`（插值按同文件 `foreach` 取值展开；无法解析即失败，不静默丢弃） |
| `plugins/software`、`plugins/usn` | 消息构造处的 `source:` 字面量与 `format!("pcscope.…")` 模板 |
| `plugins/eventlog/channels.json` | 每个持久化通道一个来源 |

字段级枚举**不在本步做**：`model/sources.json` 对每个来源记录发射点（文件:行），字段清单按计划留给 S09–S20 逐类型对照真实输出核查。这不是省略，是为了避免在盘点阶段编造未经验证的字段。

## 3. 清单数量（全部由脚本计算）

```text
Execution units: 13 (9 domains + software, adapter, eventlog, usn)
Implemented sources: 128 (126 direct emitters + 2 virtual groups)
Catalog: 218 rows over 125 sources; definitions 219; item controls 216
Event channels: 23; expanded event item rules 108; catalog event rows 108
```

| 项 | 数量 | 说明 |
| --- | --- | --- |
| 执行单元 | 13 | `crates/service/src/main.rs:503-525` 声明，与 `plugin-sources.js` 的 13 个插件键完全一致 |
| 已实现来源 | 128 | 126 个有直接发射点 + 2 个虚拟前缀组（`pcscope.usn.changes`、`pcscope.eventlog.import`） |
| 来源目录行 | 218 | `plugin-sources.js`，13 个插件 |
| 采集定义 | 219 | `collection-catalog.json`，由 `build-collection-catalog.cjs` 生成 |
| 采集项开关 | 216 | `collection-controls.json`，与 `docs/item-controls.md` 声明的 216 一致 |
| 日志通道 | 23 | `channels.json`，与目录 108 条细分事件项展开一致 |

### 3.1 按执行单元分布

| 执行单元 | 来源数 | 可独立开关 | 消息形式 |
| --- | --- | --- | --- |
| `pcscope-software.exe` | 1 | 1 | snapshot, gap |
| `pcscope-powershell-adapter.exe` | 14 | 14 | snapshot, gap |
| `system` | 15 | 15 | snapshot, gap |
| `security` | 13 | 13 | snapshot, gap |
| `network` | 15 | 15 | snapshot, gap |
| `storage` | 8 | 8 | snapshot, gap |
| `startup` | 8 | 8 | snapshot, gap |
| `desktop` | 8 | 8 | snapshot, gap |
| `virtualization` | 5 | 5 | snapshot, gap |
| `applications` | 7 | 7 | snapshot, gap |
| `scoped` | 6 | 6 | snapshot, gap |
| `eventlog` | 23 | 23 | event_batch, gap |
| `usn` | 3 | 1 | snapshot, event_batch, gap |

### 3.2 与既有文档的对账

| 文档表述 | 实际 | 结论 |
| --- | --- | --- |
| `collector-coverage.md`：79 个自动快照来源，另有指定范围来源 | 9 个领域脚本共 85 个来源；其中 `scoped` 6 个 | 一致：85 − 6 = 79 |
| `collector-coverage.md` / `README.md`：11 个清单任务 | `software` + `adapter` + 9 个领域 = 11 | 一致 |
| `collector-coverage.md`：23 个通道 | 23 | 一致 |
| `item-controls.md`：216 个采集项开关 | 216 | 一致 |
| `README.md`：来源目录 133 行 | 实际 218 行 | **过时**，需更正 README |
| `collector-coverage.md`：日期 2026-09-10 的实测数字（3,269 扩展条目、12,325 对象等） | 未复测 | 该文自述为历史结果，不作为本次验收依据 |

## 4. 采集项归属校验

| 校验 | 结果 |
| --- | --- |
| 每个目录插件键是否为已声明执行单元 | 通过，13/13 |
| 每条采集项开关是否有对应采集定义 | 通过，无悬空开关 |
| 事件细分项展开数是否等于目录事件行数 | 通过，108 = 108 |
| 目录事件来源是否与 `channels.json` 逐一相同 | 通过，23/23 |

## 5. 差异清单与处理结论

### 5.1 已裁定差异（1 项）

| 来源 | 方向 | 原因 | 处理结论 | 归属步骤 |
| --- | --- | --- | --- | --- |
| `pcscope.usn.watch-list` | 代码有、目录无 | USN 文件名单为空时发出的状态快照，报 `files=[]`、`active=false`。默认配置下文件名单即为空，因此这是**默认状态下每轮都会产生的来源**。它属于采集状态，不是可停用的数据项，所以在来源目录中没有对应行。 | 补录到 `plugin-sources.js`，归入 `pcscope.usn.changes` 控制组；**不新增独立采集开关**（与 `usn.{drive}` 的“日志身份与断点/采集库文件自身排除”同类处理）。在补录完成前，该差异登记在 `scripts/check-source-inventory.cjs` 的 `KNOWN_DIFFERENCES` 中并显式打印。 | S14 |

该差异已在 `model/sources.json` 的 `discrepancies.accepted_differences` 中可见，不会因为检查通过而消失。

### 5.2 无控制开关的来源（2 项，均为 USN 机制项）

- `pcscope.usn.{drive}` —— USN 日志身份与断点校验；属于采集正确性机制。
- `pcscope.usn.watch-list` —— 见 5.1。

两者均不提供会破坏采集正确性的开关，与 `docs/item-controls.md` 的既有约定一致。

### 5.3 其余方向

代码与目录在其余 127 个来源上逐项一致，无“目录声明但代码未实现”的条目。

## 6. 注入测试（验收硬性条件）

检查脚本必须能发现人为加入的未登记测试项。两次实际注入均被拦下并按名报出，注入后已精确还原（还原后 `collector_digest` 与注入前一致，为 `7e2cfb1cd545`）。

| 注入 | 位置 | 结果 |
| --- | --- | --- |
| 新增未登记采集来源 | `domains/network.ps1` 增加 `Emit 'network.planted-test-item'` | 退出码 1，报 `implemented in code but not catalogued: pcscope.network.planted-test-item` |
| 新增无实现的目录行 | `plugin-sources.js` 增加 `network.planted-catalog-only` | 退出码 1，报 `catalogued but no emitter in code: pcscope.network.planted-catalog-only` |

脚本另有防呆：`KNOWN_DIFFERENCES` 中的差异一旦不再存在，检查同样失败，避免豁免清单比问题本身活得更久。

## 7. 既有功能回归清单

见 `model/regression-checklist.json`，共 13 项（R01–R13），覆盖重构计划 2.2 节全部条目，并对每项给出验证命令、代码位置与配置键。

配置键分布（`settings` 表）：

| 配置键 | 归属回归项 |
| --- | --- |
| `plugin_schedule` | R06 |
| `item_settings` | R06 |
| `file_watch` | R07 |
| `monitoring` | R08 |
| `show_sensitive` | R09 |
| `retention_mode`、`quota_bytes`、`pruned_units` | R10 |

当前实际取值的读取留给 S02（建立基线）。本步只确认取值位置，不预填未读取的值。

## 8. 遗留问题

1. **~~文档未就位~~（已处理）**：`README.md` 与 `full-object-refactor-plan.md` 引用的 17 篇文档原先只存在于 `docs/sensitive-display.zip`。已只解出缺失的 15 篇；
   `docs/` 中已有的 `object-model.md`（24263 字节，与 zip 内一致）与 `full-object-refactor-plan.md`（43976 字节，**比 zip 内的 21403 字节版本更新**，即当前 S01–S33 版本）**保留未动**，避免被旧版本覆盖。
   文档链接已全部解析通过（11 个 md 链接，0 未解析）。注意 zip 内的 `object-model-refactor-plan.md`（20559 字节）是当前计划的**前身版本**，已一并解出但不应作为执行依据。
2. **README 数字过时**：来源目录实际 218 行，README 写 133 行。
3. **字段级清单未盘**：按计划留给 S09–S20，逐类型对照真实输出核查。
4. **实测数字未复测**：`collector-coverage.md` 的对象数、条目数、权限受限来源均为 2026-09-10 结果，需要在 S02 用固定机器重测。
5. **开关数量耦合**：`scripts/check-item-controls.cjs` 硬编码断言 216 个开关。若 5.1 的处理结论落地导致开关数变化，该断言需同步更新。
6. **未验证运行时输出**：本次为静态核查，未启动后台核对实际消息。S01 实施内容第 2 条要求“核对运行时实际输出”，该项需要在 S02 具备可控样例后补做，届时才能确认静态提取的来源集合与运行输出完全一致。

## 9. 下一步

S02 的前置（S01）已满足：来源清单、回归清单、一致性检查脚本均可执行且可复现。S02 需要固定测试机器、数据规模与查询组合，并补做第 8 节第 4、6 项的实测。
