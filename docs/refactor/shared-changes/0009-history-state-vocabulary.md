# 0009 历史完整度词表收口，并加一条守它的检查

| 字段 | 内容 |
| --- | --- |
| 共享入口 | 公共契约（`crates/contracts/src/query.rs` 的 `response_shape()`）、契约检查（新增 `scripts/check-history-state.cjs`）、中立模型目录（`model/query-samples.json` 重新生成） |
| 整合责任人 | 基础数据线 |
| 提出工作线 | 基础数据线（S33 无代价清理时核对读取方发现） |
| 变更类型 | **兼容性新增**（`coverage_history` 由 4 词扩到 6 词；两个前端补齐取名；无词被删除） |
| 影响的工作线 | 查询与界面线（健康表与采集权限页的取名表）、迁移验证线（固定响应集重新生成） |

## 问题

`history_state` 有两个 Rust 写入方、两个前端读取方，而**没有任何东西把它们连起来**。契约声明 4 个词，写入方一共写出 6 个，其中 3 个未声明、2 个是同一件事的两个名字。

| 写入方 | 写的词 | 读它的人 | 未声明的词此前怎么显示 |
| --- | --- | --- | --- |
| `crates/storage/src/coverage.rs:39-44` | `gap` / `caught_up` / `unknown` | 健康表 `apps/desktop/ui/model-view.js:155`，经 `historyNames`（`:52`） | `gap`、`unknown` **原样显示英文码** |
| `crates/service/src/main.rs:159` | `blocked` / `catching_up` / `caught_up` | 采集权限页 `apps/desktop/ui/monitoring.js:16` | `blocked` 落到兜底文案「等待确认」 |

契约原本声明的 `source_missing` / `history_pruned` **没有任何写入方**，只有 `caught_up` 三处重叠。另有一处读取方拿第五个词当默认值：`crates/storage/src/flow.rs:140` 的 `unwrap_or("missing")`——这个词只在测试里存在过。

两类后果不同，都值得记下来：

- **显示层**：健康表把 `gap`、`unknown` 直接显示成英文码。`label(historyNames, row.history_state)` 在取名表里找不到时回落到键本身（`model-view.js:54`），所以缺词不是渲染崩掉，而是把内部码端给用户。
- **判定层**：`gap` 与 `blocked` 是**同一件事的两个名字**。两边各自都改说明文字没有用——只要词还是两个，就总有第三个写入方挑错那个。

没有任何检查守过这个词表。`scripts/` 里唯一出现 `caught_up` 的是 `check-monitoring-ui.cjs:8`，那是当夹具输入用的。**所以分叉既不报警也不失败**：契约少两个词，前端少两个名字，全仓库照常通过。

## 旧结构 / 新结构

| | 旧 | 新 |
| --- | --- | --- |
| `coverage_history` | `catching_up` / `caught_up` / `source_missing` / `history_pruned` | 同上 + `gap` / `unknown` |
| `history_state` 的文档 | 一行词表（`query.rs:241`） | 逐词写清含义，并写明 `caught_up` 是唯一不能由一次成功采集单独推出的词 |
| 采集受阻的词 | 事件日志结果写 `blocked` | 写 `gap`——覆盖表记的就是这个词，同一件事用两个名字正是前端落到兜底文案的原因 |
| 健康表的取名 | 4 个 | 6 个（补 `gap`、`unknown`） |
| 采集权限页的取名 | 两条三元表达式 + 兜底「等待确认」 | 一张表 + `gap`、`unknown`，兜底改为「状态未知」 |
| `flow.rs` 测试的默认值 | `"missing"` | `"unknown"` |
| 谁守着这个词表 | 无人 | `scripts/check-history-state.cjs` |

## 为什么是「加入词表」而不是「并入既有词」

收口有两条路：把未声明的词并进已声明的词，或把它们正式加进词表。按语义逐个看，**两个词都并不得**：

- **`gap` 并不得。** 它最接近的既有词是 `catching_up`，但两者正相反：`catching_up` 说的是「还在往日志末尾走」，`gap` 说的是「有一段已知没被观察到」。`coverage.rs:28-30` 自己的注释写明了理由——**重新开始采集不等于那段缺口被观察到了**，所以缺口在恢复之后依然留存。把它并进 `catching_up` 恰好抹掉这个区别。`S32-switch.md:288` 的规范性结论也是这一条（不能拿最新 checkpoint 替代旧日志 epoch，不能把历史材料的存在标为 `caught_up`）。
- **`unknown` 并不得。** 它最接近 `source_missing` 或 `history_pruned`，但这两个词都**断言了具体的事**：来源不可用、历史被保留策略清掉。`unknown` 断言的是「什么都还没确定」。把「没确定」写成「来源缺失」是把未知说成已知，方向正好反了。
- **`blocked` 该并。** 它与 `gap` 是同一件事的两个名字，没有语义差别，所以统一到 `gap`，`blocked` 就此消失。它是本轮唯一消失的词。

## 兼容性

- **词表只增不减**，故对契约消费者是纯扩宽。唯一消失的 `blocked` **从未被契约声明**，也就不在任何按契约实现的消费者手里。
- **`blocked` 只活在内存里。** 它是 `state.event_result` 里那一层 JSON 的值（`crates/service/src/main.rs:167`），该结构初始化即 `null`、不落库（`:517`）。所以不存在「某个库里的 `blocked` 需要改写」——重启即无。
- **采集权限页的兜底文案随词表一起改。** 旧文案「等待确认」正是 `blocked` 落下去的地方；现在未知名一律是「状态未知」，兜底的语义与它兜的那个词一致了。
- **两个前端读取方同批补名。** 若不补，健康表会继续显示英文码，新加的检查会直接失败——这正是加检查的用意。

## 新增检查的理由

按 `model/ownership.json:155`「检查脚本的添加与放宽都需要说明理由」，这条检查是**新增**（不是放宽）：

1. **它补的是一个真实的、已经被绕过一轮的分叉。** 上面那张表说明：契约少两个词、前端少两个名字，而 21 条既有检查全部通过。没有这条检查，本次收口只是一次手工对齐，下一个写入方仍可无声地引入第七个词。
2. **既有检查没有一条碰过这个词表。** `check-monitoring-ui.cjs` 把 `caught_up` 当夹具输入，`check-model-view.cjs` 只核对渲染路径存在与否（`data[].coverage[].history_state` 在 `model-view.js:17` 已声明），两者都不看词的值域。

检查的内容与它**刻意不做的事**：

- **基准是 `response_shape()`，不是注释。** 声明在哪，基准就在哪；检查同时核对 `model/query-samples.json` 的镜像与声明一致，不一致就要求重新生成而不是手改该文件。
- **写入方按区域捕获，不整文件扫描。** 三个站点各有一条正则（`coverage.rs` 的赋值、`main.rs` 的事件日志结果、`flow.rs` 的兜底值）。整文件扫字符串字面量会把无关词一起捞进来，那就成了一条动辄误报的检查。
- **捕获不到区域时会失败，而不是静默通过。** 规则过期就等于断言空转——每一条规则都必须先证明它抓到了东西（`site.length > 0`），否则报「规则可能已过期」。
- **读取方两个方向都查。** 健康表要为**契约声明的每个词**取名（它渲染覆盖表的原样输出）；采集权限页要为**事件日志写入方写出的每个词**取名（它渲染的是那个结果）。方向不同的原因是两个页面的输入不同。
- **它不管的**：词的语义是否正确、某个词是否真的有生产者（`source_missing` / `history_pruned` 目前无人写入，是留给后续的声明）。

## 样例变化

| 位置 | 变化 |
| --- | --- |
| `model/query-samples.json` | 重新生成，`shape.coverage_history` 由 4 词变 6 词 |
| `model/drill-output.json` | **未变**：替身的覆盖视图用 `caught_up`（`crates/contracts/examples/drill.rs:485`），本就是声明内的词 |
| `apps/desktop/ui/source-type-map.json` | **未变**，与本字段无关 |

## 数据迁移

不需要。

- **库里没有需要改写的值。** 覆盖表 `coverage_scopes.history_state` 只由 `coverage.rs` 写入，它写的三个词（`gap` / `caught_up` / `unknown`）在收口前后都是声明内的词——**这一列的值没有变过**。
- **消失的 `blocked` 不落库**（见「兼容性」）。
- **`flow.rs` 的 `"missing"` 只存在于测试的默认值里**，从未写进任何库。

## 检查结果

```text
node scripts/check-history-state.cjs                  → 通过（6 个声明词；写入方 coverage.rs=gap/caught_up/unknown、main.rs=gap/catching_up/caught_up、flow.rs=unknown）
```

阴性对照四组，两个方向都验过（临时改动均在同一轮内还原）：

| 方向 | 临时改动 | 结果 |
| --- | --- | --- |
| 写入方越界 | `coverage.rs` 写出词表外的词 | 失败 |
| 写入方回归 | `main.rs` 恢复写 `blocked` | 失败 2 项（词表外 + 前端无取名） |
| 读取方缺名 | `historyNames` 去掉 `gap` | 失败 |
| 基准漂移 | `query-samples.json` 与声明不一致 | 失败 |

按 `model/ownership.json` 的归属规则，新检查脚本已登记进「契约检查」共享入口的 `paths`（该入口的 `rule` 正是「检查脚本的添加与放宽都需要说明理由」——上面的「新增检查的理由」一节即为其说明）；`node scripts/check-ownership.cjs` 通过（298 个受跟踪文件全部有归属，0 个未归属）。
