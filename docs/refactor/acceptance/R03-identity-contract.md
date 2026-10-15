# R03 验收记录：冻结身份解析接口

> **后续更正（2026-09-12，R09 发现）**：第 3 节列出的测试 `a_disabled_item_resolves_to_unresolved_rather_than_absent` 所断言的行为**已被修正**。`resolve` 不再因采集项停用而拒绝解析——停用影响的是能否把缺失判定为未发现，不是能否识别一个已经拿到的对象；拒绝会丢弃「停用后保留最后状态」所要求保留的事实。
>
> 该测试已改名为 `a_disabled_item_still_resolves_so_retained_facts_survive` 并断言相反结果。依据与影响见[共享改动 0002](../shared-changes/0002-identity-disabled-item.md)。本文件其余结论不改写。

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | R03 · 待验收 |
| 实现范围 | 契约版本 1：`IdentityRequest` / `TypeStrategy` / 四类结果 / 规范化规则 / `InstanceLookup` / `LifecycleTransition` / 纯函数 `resolve` 与 `lifecycle`。未做持久化分配（属 S06） |
| 代码定位 | 新增 `crates/contracts/src/identity.rs`、`crates/contracts/examples/identity-contract.rs`；修改 `crates/contracts/src/lib.rs`（声明模块） |
| 验证环境 | Cargo / rustc 1.98.1（`x86_64-pc-windows-gnu`） |
| 验证过程 | `cargo test --workspace --offline --locked`；`target/debug/examples/identity-contract.exe` |
| 结果 | 69 项 Rust 测试通过（较 R02 +11）；示例 8 项样例全过，退出码 0；13 个既有检查通过；适配器示例仍退出码 0 |
| 遗留问题 | 见第 6 节 |
| 下一步 | R04：冻结事实、动作、时间和清理语义 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 身份接口 | `crates/contracts/src/identity.rs` 的 `resolve` / `InstanceLookup` / `IdentityRequest` |
| 规范化规则 | 同文件的 `case_rule` / `normalize_component` / `normalise` |
| 实例 ID 生命周期说明 | `docs/refactor/identity-contract.md` 第 6、7 节 |
| 五类样例（固定预期） | `crates/contracts/examples/identity-contract.rs` |

## 2. 八项样例的实测结果

```text
PASS  user renamed, same SID                     confirmed / existing_by_key / 同一实例
PASS  tasks with the same name in different folders
      distinct:true  case_folded_same:true  basis:existing_by_key
PASS  process id reuse                           same_pid_distinct:true  pid_only:provisional
PASS  file identity and path reuse               survives_rename:true  basis:existing_by_key
                                                 path_only:provisional
PASS  one instance observed by two sources       same_instance:true  bindings:2
PASS  a provisional identity does not fork       两次均 provisional，同一实例
PASS  absence is not deletion                    not_observed / unchanged / removed_stated
PASS  missing installation or boot identifier degrades clearly
```

计划要求覆盖的五类：

| 计划样例 | 覆盖 | 固定预期 |
| --- | --- | --- |
| 用户改名 | 用例 1 | 同一实例；名称是字段，永不参与身份 |
| 任务不同路径 | 用例 2 | 不同文件夹的同名任务两个实例；路径仅大小写不同是同一实例（路径命名空间折叠） |
| PID 复用 | 用例 3 | 同 PID 不同创建时间是两个实例；只有 PID 时为暂定，待补 `created_at` |
| 文件路径复用 | 用例 4 | 同一文件 ID 跨改名仍是同一实例；只有路径时为暂定 |
| 同实例多来源 | 用例 5 | 一个实例，按来源绑定 2 条 |

另三项为计划隐含要求，一并固定：暂定身份不因重复观察被升级（用例 6）、缺安装/启动标识明确降级而不伪造上下文（用例 8）、未发现不冒充删除（用例 7）。

## 3. 单元测试覆盖的规则

11 项，逐条验证判定规则本身：

- 目录中每个身份策略都有对应的规范化规则（新增策略若漏配规则即失败）；
- 大小写按命名空间折叠，而非全局规则——任务路径两种写法归一，来源原生标识原样比较；
- 缺键占位为 `?键名` 并列入 `missing`，不默认值；
- 键完整且无既有实例时判为新实例；
- 多个既有实例匹配时判为歧义，候选原样返回，**不任选一个**；
- 采集项停用时判为无法解析，而非缺失；
- 观察不完整或缺采集范围时，生命周期不做任何推进；
- 只有来源明确声明才进入已删除；
- 首次观察的依据中明确否认创建时间；
- alias 在暂定转确认前后一致；
- 非键字段（如 `name`）变化不改变身份键。

## 4. 过程中修正的两处

1. **接口缺口：查来源绑定时没有来源记录标识。** `resolve` 最初用候选类型与作用域去查绑定，而「同实例多来源」和「暂定不分裂」都依赖证据自身的来源记录身份。首版 `IdentityRequest` 根本没携带它。已补 `binding: SourceRecordRef { source, item_id, native_record_id }`，并明确该字段**永不进入身份键**。

2. **语义错误：绑定命中即判为已确认。** 来源绑定匹配说明的是「是哪一个实例」，不是「键已经完整」。原实现让暂定身份在第二次观察时被升级为已确认——等于用重复观察伪造了身份的完整性。已改为：**键不完整就仍是暂定**，即便实例已知（`ProvisionalIdentity` 因此新增 `instance_id`）。

   这条是**写样例时发现的**：用例 6 的预期断言把它暴露了出来（`first:provisional` 而 `second:confirmed`）。

同时修正了生命周期里两处代码缺陷：一处恒假条件（`x != x`）导致分支不可达，以及重生代际被写成 0 而不是上一代 +1，并补上了「从未观察过且缺失」这一未覆盖分支。

## 5. 与 R02 契约的衔接

`resolve` 的输入是 R02 的 `IdentityCandidate`。两个契约的字段状态、推导方式与结果语义是同一套枚举，没有第二份定义。候选中的 `missing` 与解析结果的 `could_be_refined_by` 是同一组键，适配器提出的缺口与身份引擎需要的补充信息对齐。

## 6. 遗留问题

1. **未接入任何调用方。** `resolve` 目前只在测试与样例中调用；S06 才会真正接线。
2. **`InstanceLookup` 没有数据库实现。** 两个查询目前由内存替身提供。
3. **绑定查询没有携带时间范围。** 模型要求来源绑定带有效范围以区分代际，本契约只按来源记录身份查询，时间维度留给 S06。
4. **`hints` 的部分字段上游尚不产出。** `boot_id`、`history_retained_from`、`states_object_ended` 现有采集器都不提供，需要 S05/S09–S20 补。
5. **复合策略的键顺序依赖目录声明顺序。** 目前稳定，但若目录中键的书写顺序变化，同一实例的键串会变化。S06 落地前应确认是否需要改为按键名排序（当前 `composite_native` 的附加项已排序，声明键按声明顺序）。
6. **规范化规则的完备性未逐类型核实。** 例如 `named_entity` 一律折叠，但个别命名空间可能区分大小写；逐类型核查属于 S09–S20。

## 7. 下一步

R04 的前置（R03）已满足。R04 需要冻结事实、动作、时间与清理语义：时间编码与精度、六种缺失状态的区别、当前值选择与冲突处理、动作去重与阶段、证据清理后的引用行为，以及完整快照的范围判定。
