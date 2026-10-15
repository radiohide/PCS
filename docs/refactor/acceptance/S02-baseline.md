# S02 验收记录：固定样例与性能基线

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S02 · 待验收 |
| 实现范围 | 去标识样例语料（可读基础样例 + 真实规模样例）、可重复加载与测量工具、查询/容量/并发基线；未修改采集行为、运行配置或既有数据库 |
| 代码定位 | 新增 `scripts/build-fixtures.cjs`、`crates/storage/examples/refactor-baseline.rs`；样例 `fixtures/`；报告 `model/baseline-base.json`、`model/baseline-scaled.json` |
| 验证环境 | Windows 11 Enterprise LTSC 26100（x86_64）；Cargo / rustc 1.98.1，host `x86_64-pc-windows-gnu`；工具链位于 `SYS/pcscope/.tools`（见第 2 节）；`--release` 构建；未启动后台服务、未读取用户数据库 |
| 验证过程 | `node scripts/build-fixtures.cjs --scale`；`cargo build --release -p pcscope-storage --example refactor-baseline`；`refactor-baseline fixtures target\s02-release-base`；`refactor-baseline fixtures\scaled target\s02-release-scaled` |
| 结果 | 见第 4、5 节；全部内置断言通过（可复现、重放幂等、非法输入 6/6 拒绝、补采期间零错误） |
| 遗留问题 | 见第 7 节 |
| 下一步 | S02 通过后进入 S03（冻结类型、身份和字段契约） |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 去标识样例目录 | `fixtures/`（基础语料，已提交）+ `fixtures/scaled/`（真实规模语料，由脚本生成，已加入 `.gitignore`） |
| 可重复加载工具 | `scripts/build-fixtures.cjs`（生成）+ `crates/storage/examples/refactor-baseline.rs`（加载、校验、测量） |
| 基线报告（原始数据） | `model/baseline-base.json`、`model/baseline-scaled.json` |
| 本验收记录 | `docs/refactor/acceptance/S02-baseline.md` |

## 2. 固定测试环境

`build.ps1` 期望仓库内 `.tools/`，本检出不含该目录（`.tools/` 被 gitignore）。工具链实际位于兄弟目录，与 `target/build-status.md` 记录一致：

```text
C:\Users\radiohide\Desktop\SYS\pcscope\.tools\cargo   (Cargo 1.98.1)
C:\Users\radiohide\Desktop\SYS\pcscope\.tools\rustup
C:\Users\radiohide\Desktop\SYS\pcscope\.tools\mingw\mingw64\bin
```

复现命令（PowerShell）：

```powershell
$t="C:\Users\radiohide\Desktop\SYS\pcscope\.tools"
$env:CARGO_HOME="$t\cargo"; $env:RUSTUP_HOME="$t\rustup"
$env:PATH="$env:CARGO_HOME\bin;$t\mingw\mingw64\bin;$env:PATH"
cargo build --offline --locked --release -p pcscope-storage --example refactor-baseline
```

**须记录的两点**：`.tools` 在本检出之外，`build.ps1` 直接运行会失败；以及性能数字来自 `--release` 构建，debug 构建的最慢 p95 约为 release 的 3 倍（607 ms 对 193 ms），不可用 debug 数字做判断。

## 3. 样例语料

### 3.1 基础语料（已提交，约 33 KB）

7 个场景 41 条协议消息，外加 6 个必须被拒绝的非法输入。时间戳、GUID、SID、账户、路径全部为虚构值，重放不依赖当前时钟。

| 文件 | 消息 | 覆盖 |
| --- | --- | --- |
| `01-baseline.jsonl` | 24 | 首轮完整快照，覆盖计划 12 个对象类别的代表来源（账户、软件、进程、启动项、Shell 扩展、文件 ACL、策略、网络、证书、驱动、WSL、打印、快捷方式） |
| `02-changes.jsonl` | 4 | 配置变化 A→B、首次观察、完整快照中缺失、**PID 复用**、同名不同作用域 |
| `03-gaps.jsonl` | 2 | 读取失败（来源不可用），验证保留旧状态而非清空 |
| `04-events.jsonl` | 4 | 登录/登出/登录失败/账户建立/特权分配/进程生命周期/计划任务执行/服务状态 |
| `05-usn.jsonl` | 3 | 卷身份快照、监控名单状态快照、文件变更事件批次 |
| `06-edge-cases.jsonl` | 2 | 缺字段、显式 null 与字段不存在、空列表、零值、超长值、Unicode |
| `07-replay-and-late.jsonl` | 2 | 逐字重复的批次（幂等）+ 迟到的旧事件（不得回退当前状态） |

非法输入（`fixtures/invalid/`，加载器不读取，仅用于校验拒绝行为）：不完整快照、快照内重复身份、协议版本不符、空 source、事件含未知字段、batch_seq 为 0。

### 3.2 真实规模语料（`--scale` 生成）

对照 `collector-coverage.md` 记录的规模（约 15.6k 当前对象 / 36k 保留记录）生成：

| 文件 | 消息 | 内容 |
| --- | --- | --- |
| `10-scale-baseline.jsonl` | 32 | 8 个来源 × 1,800 对象，按 4 个分片投递 = 14,400 对象 |
| `11-scale-changes.jsonl` | 32 | 第二轮完整快照，其中十分之一发生变化 = 1,440 变化 |
| `12-scale-events.jsonl` | 40 | 4 个通道 × 20,000 条事件 |

第二轮投递**全部**对象而非仅变化子集——完整快照本就报告全部清单。最初写成只投变化子集时，应用把其余对象正确地判为「本次未发现」，这暴露了语料缺陷而不是产品缺陷（见 5.3）。

## 4. 基线数字

### 4.1 输入规模与容量

| 指标 | 基础语料 | 真实规模语料 |
| --- | --- | --- |
| 协议消息 | 41 | 104 |
| 当前对象 | 35 | 14,400 |
| 保留记录 | 64 | 35,840 |
| 快照 | 32 | 64 |
| 加载耗时 | 199 ms | 35.7 s |
| 数据库 | 507,904 B | 53,010,432 B（50.6 MiB） |
| WAL / SHM | 0 / 32,768 B | 0 / 32,768 B |

真实规模语料无 WAL 残留（加载后已合并），数据库体积与 5 GB 默认预算相比占用约 1%。

### 4.2 查询基线（真实规模语料，release）

`cold` = 打开 reader 后查询，即服务处理每个请求的真实路径（`crates/service/src/main.rs:315` 为每次 timeline/object 请求新建 reader）；`warm` = 复用已打开的 reader，用于分离查询本身的开销。计划 12.4 节的门槛适用于 cold。

| 查询 | cold p95 (ms) | warm p95 (ms) | 结果 |
| --- | --- | --- | --- |
| `objects.keyword_absent` | 205.9 | 180.6 | total=0 |
| `objects.keyword_hit` | 189.5 | 183.5 | total=1800 |
| `objects.detail_first_row` | 141.0 | 139.6 | — |
| `objects.default` | 140.7 | 142.1 | total=14400 |
| `objects.by_category` | 123.0 | 100.9 | total=1800 |
| `objects.by_item` | 110.8 | 99.3 | total=0 |
| `timeline.keyword_absent` | 97.6 | 104.7 | rows=0 |
| `timeline.keyword_hit` | 87.4 | 69.2 | rows=100 |
| `objects.by_source` | 86.4 | 76.1 | total=1800 |
| `timeline.detail_task_evidence` | 60.1 | 41.3 | keys=7 |
| `timeline.kind_coverage` | 53.5 | 25.8 | rows=0 |
| `timeline.by_source` | 42.0 | 40.8 | rows=100 |
| `snapshots.list` | 40.4 | 27.5 | rows=50 |
| `timeline.default` | 35.7 | 26.3 | rows=100 |
| `timeline.by_item` | 29.2 | 23.8 | rows=100 |
| `retention.status` | 19.9 | 10.7 | mode=unlimited |

**基线结论：**

- 全部查询 cold p95 均低于 1 秒门槛，最慢 205.9 ms，约为门槛的 21%，余量约 5 倍。
- **对象列表是热点**：`objects.*` 系列 cold p95 为 86–206 ms，是 `timeline.*`（29–98 ms）的 2–7 倍。
- **开销在投影而非取行**：`objects.keyword_absent`（total=0，零行返回）耗时 205.9 ms，与 `objects.default`（total=14400，返回 100 行）的 140.7 ms 同量级甚至更高。说明成本来自每次查询重算的对象投影，不来自结果集大小。
- **cold 与 warm 差距很小**（多数在 5–25 ms），说明连接建立不是主要成本；这与 `open_reader` 的实现一致（仅打开只读连接、设 PRAGMA 与超时）。

这两点共同指向计划已识别的改造点：§3「UI 专属目录被后端用于分类」、§11 的「当前展示投影」，以及 S22 的「建立窄索引」和「先过滤后计数分页」。本基线可作为该改造的对照。

### 4.3 补采期间查询（并发）

写入线程投递完整语料，读取线程持续查询（真实规模语料）：

| 指标 | 数值 |
| --- | --- |
| 读取次数 | 2,558 |
| 中位延迟 | 10.0 ms |
| p95 延迟 | 17.2 ms |
| 错误 | 0 |

补采期间的读取延迟低于静态查询的 cold p95，因为读取方复用的是刚建立的连接路径；**零错误**表示并发读写正确。

## 5. 验证结论

### 5.1 相同工具可重复加载相同数据

语料加载进两个全新数据库，比较 `records`、`objects`、`object_times` 三张表的逐行内容摘要：

```text
BASE    digest 相同；行数 {objects:35, records:64, object_times:35}
SCALED  digest 相同；行数 {objects:14400, records:35840, object_times:14400}
```

两次加载逐行一致。`collector_digest` 之外的任何差异都会使检查以非零码退出。

### 5.2 幂等与拒绝

| 检查 | 结果 |
| --- | --- |
| 快照重复投递（同一 `run_id`） | 新增 0 条，状态摘要不变 |
| 事件批次逐字重放 | 新增 0 条 |
| 非法输入 | 6/6 被拒绝，且是协议校验或入库校验拦下，未写入任何记录 |

事件批次的**过期批次**由协议设计拒绝（`docs/protocol-v1.md`：stale sequences are rejected），因此「重复加载」在事件通道上表现为可复现，而非无操作。语料编写时曾把「重放」写成只含 1 条事件的批次，被正确拒绝为「同一批次序列的不同数据」——这说明拒绝规则在起作用。

### 5.3 已证实的行为：完整快照中缺失的对象

`staged_load` 逐场景加载并对比当前投影的精确差分，基础语料结果：

| 阶段 | 当前对象 | 保留记录 | 未发现记录 | 投影新增 | 投影移除 |
| --- | --- | --- | --- | --- | --- |
| 01-baseline | 32 | 32 | 0 | 32 | 0 |
| 02-changes | 34 | 39 | 2 | 4 | 2 |
| 03-gaps | 34 | 41 | 2 | 0 | 0 |
| 04-events | 34 | 54 | 2 | 0 | 0 |
| 05-usn | 36 | 58 | 2 | 2 | 0 |
| 06-edge-cases | 35 | 63 | 4 | 1 | 2 |
| 07-replay-and-late | 35 | 64 | 4 | 0 | 0 |

移除项之一为进程实例 `pcscope.execution.processes | … | 4100/2026-09-10T00:00:00Z`：第二轮以同一 PID、不同创建时间上报，旧实例离开当前投影。这正是 PID 复用应有的处理，与应用模型「只有 PID 不足以唯一识别」一致。

结论：**对象在其（来源、作用域）的完整快照中缺失时，会离开当前投影（`objects` 表），同时保留一条 `object.no_longer_observed` 记录。** 该行为由语料实测确认，非推测。

## 6. 发现与影响

1. **对象列表查询成本来自投影计算，不来自数据量或结果行数**（4.2 节）。这是 S22「先过滤后计数分页 / 窄索引」的直接依据，也印证计划 §3 关于「UI 专属目录被后端用于分类」的现状描述。
2. **每次查询新建 reader 是真实路径**（`service/main.rs:315`，并发上限 2）。改造时若改为复用连接，需要重新测 cold/warm——本基线已把两者分开记录。
3. **读取方在建表窗口内拿到 schema 错误**：写入方首次建表期间，读取方得到 `no such table: record_search` 而非可知的空状态。服务进程内的顺序（先 `Store::open` 再开始服务请求）使其不暴露给当前 UI，但跨进程与升级窗口存在。本项交给 S04 验收，不在 S02 修改。
4. **对象数变化与投影差分的对应关系**已按精确差分记录，避免用总数变化反推行为（第 5.3 节即为一次推算失败后的改正）。

## 7. 遗留问题

1. **`build-status.md` 的记录已过期**：其记 22 项 Rust 测试通过（3 contracts、2 schedule、1 software、13 storage、3 USN）。本次实测 **39 项通过**（分布 3、3、1、27、1、4），无失败。
2. **真实规模语料是合成的**，不是真实数据库的匿名化副本。计划 12.4 节要求「真实规模匿名化数据副本」；本机未找到 PCScope 运行库（默认目录与 `PCSCOPE_DATA_DIR` 均无 `history.db`），因此以同规模合成语料代替。启动服务后应补做一次真实副本测量。
3. **字段级样例尚未覆盖**：语料按「正常 / 缺字段 / 读取失败」三类结构构造，未逐来源覆盖 128 个来源的全部字段形态。按计划这属于 S09–S20 的「核查真实输出结构」。
4. **01/02 语料的差异未逐条解释**：基础语料的投影差分已精确记录，但个别条目（如 06 阶段移除两条快捷方式）源于语料只投递一项造成的「未发现」，不是产品行为差异。真实规模语料已避免该写法。
5. **性能门槛的正式判定留给 S30**：本步只建立可比基线，不做通过判定。

## 8. 下一步

S03 的前置（S01、S02）已满足：来源清单、回归清单、去标识样例、可重复加载工具与基线报告均可复现。S03 需要冻结十二类的类型 ID、身份字段与字段契约，并产出中立模型目录与目录校验器。
