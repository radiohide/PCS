# S09 验收记录：账户与身份

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S09 · 待验收 |
| 实现范围 | 本类 5 个观察型来源的适配器（`pcscope.eventlog.security` 的事件动作在 S17 与参考适配器内，本步不重做）；同时建立 S10–S20 共用的声明式适配器引擎与注册表 |
| 代码定位 | 新增 `crates/contracts/src/adapters/mod.rs`（引擎与注册表）、`crates/contracts/src/adapters/account.rs`（5 个来源）；修改 `crates/contracts/src/lib.rs`（模块声明）、`crates/contracts/src/reference.rs`（`adapter_for` 接注册表）、`model/ownership.json`（新文件归属类型适配线） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；`:memory:` 数据库；采集端输出结构取自 `plugins/powershell-adapter/collect-system.ps1` 与 `domains/system.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；7 项新增测试（第 3 节）；17 个 `scripts/check-*.cjs`；6 个契约示例退出码 |
| 结果 | 126 项 Rust 测试通过（较 S08 的 119 增加 7 项）；17/17 检查通过；6 个示例退出码 0；S09 专项验收 6 条逐条见第 3 节 |
| 遗留问题 | 见第 5 节 |
| 下一步 | S10–S20 按同一引擎逐类补齐；S21 用 `adapters::all_specs()` 对账 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 来源适配器 | `account.rs` 的 5 个 `SourceSpec` |
| 目录映射 | 每个 spec 声明 `source` 与 `type_id`；身份键、字段键的对应写在函数内并注明取自采集端的哪条 emit |
| 实际输出结构样例 | 第 3 节的测试即为每种结构的样例（平面项、成员展开、权限展开、双流合一） |
| 缺字段样例 | `a_field_the_source_did_not_send_is_absent_rather_than_defaulted` |
| 类别覆盖报告 | `adapters::all_specs()` 输出全部已声明来源，供 S21 对账 |

## 2. 引擎：三种素材形状

计划要求每类「核查真实输出结构」。核查下来只有三种形状，引擎各给一个入口：

| 形状 | 例子 | 处理 |
| --- | --- | --- |
| 一条素材 = 一个对象 | 本地账户、用户配置文件 | `object_candidate`：身份取自 item，字段取自 data 表 |
| 一条素材 = 多个对象 | 组的成员表、账户的权限表 | 展开为每个成员/权限一个候选，各自的复合身份 |
| 多个素材 = 一个对象 | 机器身份来自 `Win32_ComputerSystem` 与 `Win32_ComputerSystemProduct` 两条 | 身份取采集作用域里的机器，两条各自贡献自己带的字段 |

**没有任何 spec 可以填来源没给的字段。** 键缺失或值为 null 一律记为 `NotProvided`；采集端显式说「本来源无法提供建立时间」并附上依据，那个说法就是答案。

## 3. 专项验收对照

| 计划验收 | 结果 |
| --- | --- |
| 用户改名不新建实例 | 通过：`a_rename_does_not_change_the_identity`——身份由 SID 构成，改名只改一个字段 |
| 同名不同 SID 分开 | 通过：`two_accounts_with_one_name_stay_apart` |
| 机器级组不会因采集运行账户变化重复生成 | 通过：`a_machine_level_group_does_not_depend_on_the_collecting_account`——见第 4 节的作用域决定 |
| 拒绝读取某组成员不表示清空成员 | 通过（由结构保证）：采集端在 `Get-LocalGroupMember` 失败时会让整个 `Emit-Collection` 抛错并改为发出 `gap`，该来源**根本不产出素材**，因此适配器不会看到「空成员表」。适配器另有一条分支：`members` 存在但不是列表时记为 `unreadable` 而非空表 |
| 来源未提供建立时间则为空 | 通过：采集端发送 `created_at=$null` 与 `created_at_basis='not available from this source'`；`account.user` 类型未声明建立时间字段，因此不写入任何值，也不虚构 |
| 成员变化展示为登记变化或组属性动作 | 通过（结构）：成员表展开为 `account.group_membership` 实例，其增删由 S06 的生命周期与 S07 的变化动作表达；本步不新增专用动作类型 |

## 4. 必须现在定的一个作用域问题

S09 的验收里有一条把共享改动 0005 留下的空白逼了出来：**「机器级组不会因采集运行账户变化重复生成」**。

采集作用域形如 `machine:<id>/user:<采集账户 SID>/schema:1/context:…`。对机器级对象（本地组、本地账户、机器身份）而言，`user:` 段是**采集者**，不是对象所属的作用域。把它留在身份作用域里，同一台机器上的同一个本地组，会随采集运行账户的不同而变成不同实例——收集者换个账户，组就"消失又出现"。

**决定：** 作用域按类型收窄，由该类型的适配器决定。新增 `adapters::installation_scope`：去掉 `user:` 段，保留机器与安装段。五个账户类来源全部使用它。真正属于用户的对象（某用户的注册表配置等）的适配器可以继续使用交给它的完整作用域。

这正是共享改动 0005 里标明「尚未决定、列入 S21」的那一项，S09 的验收条件要求现在决定。以 5 个来源的实际行为为准补测，未改动任何契约函数。

## 5. 遗留问题

1. **`pcscope.eventlog.security` 的分派仍未收口。** 该来源同时映射到 12 个类型（见 source-map），目前由 S17 的事件适配器处理；账户类相关的动作（4624/4625/4720 等）在其中，S09 未重复实现。归属在 S17 记录中说明。
2. **每个来源仍需真实机器核对。** 本步的键名来自采集端 emit 声明这一静态来源；**本机未做真实采集实测**（本机权限受限，且按用户要求需要权限的采集不实测）。键名与真实输出的核对并入 S21。
3. **`account.user_profile` 的 `Special` / `Status` 两个采集端字段未映射**：类型未声明对应字段。是否要补进目录属 S03/S21 的目录范围，不在本步自行添加。
4. **S10–S20 尚未开始**，注册表目前只有本类。`adapters::all_specs()` 已可供 S21 计算覆盖率。
