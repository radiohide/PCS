# 最小贯通演练（R09）

日期：2026-09-12。运行器：`crates/contracts/examples/drill.rs`；前端替身：`apps/desktop/ui/drill-view.js`；检查：`scripts/check-drill.cjs`。

依据：[并行开发前置条件与准入方案](parallel-readiness.md) R09。契约：[适配器](adapter-contract.md)、[身份](identity-contract.md)、[语义](semantics-decisions.md)、[查询](query-contract.md)。

## 1. 一条命令

```powershell
cargo build --offline --locked -p pcscope-contracts --examples
.\target\debug\examples\drill.exe        # 证据 → 候选 → 身份 → 最小投影 → 查询响应
node scripts/check-drill.cjs             # 用替身渲染器渲染并核对验收条件
```

## 2. 链路

```text
fixtures/contract/*/input.jsonl     协议消息，S02 建立的去标识夹具
        ↓  reference::adapter_for     按来源选参考适配器（R02）
   候选事实与动作
        ↓  identity::resolve          身份判定（R03）
   实例、暂定身份、歧义
        ↓  semantics::select_current  当前值与冲突（R04）
   字段视图、动作视图、证据视图
        ↓  Projection（本步的内存替身）
   InstanceSummary / InstanceDetail / ActionView / EvidenceView
        ↓  QueryResponse              查询契约的信封（R05）
   apps/desktop/ui/drill-view.js     替身渲染，纯文本
```

**投影是替身。** 实例、事实与动作在内存中从流水线结果直接构建，不经过存储层，不读也不写数据库。S05–S08 会替换它。本步**不宣称**那些步骤已完成。

## 3. 走通了什么

七个契约用例各跑一遍，产出 31 个查询响应：

| 用例 | 实例 | 动作 | 响应 | 覆盖的验收点 |
| --- | --- | --- | --- | --- |
| 01 两个配置来源与运行日志 | 1 | 2 | 5 | **同一任务多来源只有一个实例**，字段指向各自证据 |
| 02 同名不同路径与重建 | 3 | 1 | 5 | 删除后重建是新代际 |
| 03 PID 复用与不完整事件 | 3 | 1 | 5 | 暂定身份可见 |
| 04 迟到日志与冲突 | 1 | 1 | 6 | **冲突保留候选**；**区间动作**（快照差异） |
| 05 停用、不可读、证据已清理 | 1 | 0 | 4 | **未知、冲突、证据清理状态贯通** |
| 06 敏感值与遮罩 | 2 | 0 | 4 | 遮罩不作为值 |
| 07 无法归属的日志与缺口 | 0 | 0 | 2 | **未归属记录保留原因**，不编造对象 |

用例 01 的详情渲染（摘录）：

```text
\示例文件夹\示例任务  (计划任务 · startup)
  身份 startup.scheduled_task@example-machine|…|\示例文件夹\示例任务#1
  状态 confirmed / 生命周期 active
  · 任务路径  [known]  "\\示例文件夹\\示例任务"
      证据 snapshot:pcscope.startup.tasks|…|c1-run-1、snapshot:pcscope.startup.task-details|…|c1-run-1
  · 执行动作  [known]  [{"execute":"C:\\示例\\程序\\示例程序.exe"}]  证据（同上，两份）
  采集 已启用 最近结果 success
```

字段的**证据引用同时指向两个来源**——这就是「字段能指向各自证据」的可见结果。

用例 04 的区间动作是另一类：同一来源在 10:00 与 10:05 的两次真实观察，字段 `actions` 由 `--示例程序 A` 变为 `--示例程序 B`。它是全套演练里唯一一条快照差异动作，且**只有它是应该被产生的**——用例 01 的两个来源各带不同字段，沉默不是变化（R09 记录第 4 节第 5 项）。

## 4. 替身渲染器与「不再依赖未文档化字段」

`drill-view.js` 的第一行就写明它是 R09 的替身、由 S23–S25 替换，并声明它读取的**全部 46 条契约路径**（`PATHS`）。

`check-drill.cjs` 逐条验证这些路径在演练输出中**确实存在**。因此：

- 契约里改名一个字段 → 检查失败并点名该路径，而不是页面上渲染出一片空白；
- 渲染器想读一个契约没定义的字段 → 同样失败。

这是「各模块之间不再依赖口头约定或未文档化字段」的可执行落点：依赖被列出来，并被验证。

## 5. 检查覆盖

| 验收条件 | 检查方式 |
| --- | --- |
| 同一任务两个来源只有一个实例 | 用例 01 的实例数与身份状态；`name` 字段引用 ≥2 个来源 |
| 字段能指向各自证据 | 每个 `known`/`conflicting`/`masked` 字段必须有证据引用 |
| 未归属事件保留 | 用例 07 不产生对象与动作，原因被记录并作为响应警告 surfaced |
| 未知状态贯通 | 用例 05 渲染结果含 `unreadable` |
| 冲突状态贯通 | 用例 04 渲染结果含 `conflicting` 与「冲突」候选 |
| 证据清理状态贯通 | 用例 05 渲染结果含 `evidence_pruned`；对应证据视图报 `pruned` |
| 时间精度不被伪造 | 精确时间的动作不得标记派生；区间或缺少发生时间的动作必须标记 |
| 变化动作只宣称真实发生的变化 | 区间动作的起止时间不得相同；`before`/`after` 不得缺失——缺失不是值，不能当作变化的一侧（见 R09 记录第 4 节第 5 项） |
| 记录里的每用例计数与产物一致 | `cases[].counts` 逐用例对照 R09 记录与本文档第 3 节的计数表；不一致时点名要同步的两个文件 |
| 演练结果与契约用例一致 | 逐用例对照 `fixtures/contract/*/case.json` 的 `equals` 断言（计数，以及实例唯一时的 `instances[0]` 身份与类型）；无法比较的必须登记原因，目前只有 `gaps.length` |
| 不依赖未文档化字段 | 46 条声明路径逐条验证存在 |
| 目录版本一致 | 每个响应的 `catalog_version` 等于目录版本 |

## 6. 不在本步范围

| 事项 | 归属 |
| --- | --- |
| 真实投影、字段事实与动作入库 | S07 |
| 覆盖状态汇总与预算 | S08 |
| 真实来源适配器 | S09–S20 |
| 真实对象页面与交互 | S23–S25 |
| 真实查询与索引 | S22 |
