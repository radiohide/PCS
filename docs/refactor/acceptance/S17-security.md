# S17 验收记录：安全与信任

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S17 · 待验收 |
| 实现范围 | 安全类 10 个观察来源的适配器；另有**统一事件适配器**（本步的跨类别组成部分，见 `docs/refactor/event-adapter.md`） |
| 代码定位 | 新增 `crates/contracts/src/adapters/security.rs`、`crates/contracts/src/adapters/event.rs`；修改 `crates/contracts/src/adapters/mod.rs`、`crates/contracts/src/reference.rs`（事件来源改由统一适配器接管）、`crates/contracts/src/model.rs`（`registered_action_id`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；素材键名取自 `domains/security.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；18 项安全测试 + 8 项事件测试；17 个 `scripts/check-*.cjs`；契约用例与演练输出重新生成后逐字节未变；构建零警告 |
| 结果 | 302 项 Rust 测试通过；17/17 检查通过；6 个示例退出码 0 |
| 遗留问题 | 见第 4 节 |
| 下一步 | S21 全来源对账 |

## 1. 一个来源跳过了，理由成立

**`security.detections` 记为 `Skipped` 并说明原因。** 目录给该来源的处置是 `action_evidence`，而一条检出的字段（威胁 id、状态 id、处理是否成功、时间）**不属于任何已声明的类型**，也没有任何已登记的动作承载它。把它并到目录里为它标注的 `security.protection` 上，会**为每一个检出 id 造出一个防护产品**——那不是来源说的事。

原因字符串里保留了采集端自己的告诫：**下一次快照里没有某条检出，不等于它已被处置**。

## 2. 校验器第二次拦住了错误

`security.secure_boot_configuration` 的 `name` 是**身份键而不是声明字段**。第一版草稿把它当字段输出了（相邻的 `security.protection` 确实声明了 `name`，容易顺手写错），**实施时的校验拦下了这条**，测试现在断言它不出现在字段里。

前一处同类拦截在 S18（`security.tpm` 指向的类型没有 `name` 字段）。校验器在真实产出之前拦住错误，比事后核对便宜得多。

## 3. 两处「选了一个值」需要与其它类别的做法对照

本类有两处从多个值中选出一个，与 S18/S20 的「两个事实、一个字段就不填」看似矛盾，实际不同——差别在于**是不是同一类事实**：

| 位置 | 情况 | 处理 | 与 S18/S20 的差别 |
| --- | --- | --- | --- |
| `security.protection.scan_time` | 采集端给两个扫描结束时间，类型声明一个 | 取**较晚的**那个，标 `Derivation::Decoded`；两个时间无法比较时字段缺失 | 两者是**同一类事实**（都是「最近一次扫描」的候选），取较晚者不新增信息；而 S18 的内存四值是**不同口径**，S20 的页数是**两个不同计数**，挑任一个都会丢掉另一口径 |
| `security.protection.real_time` | 采集端给 `DisableRealtimeMonitoring`（否定式） | 取反，标 `Decoded`；非布尔值则字段缺失 | 取反是一次明确可解释的换算，且有标记可见 |

**若项目惯例更严（一处也不许选），`scan_time` 是唯一需要回退的地方**，已在此标明。

## 4. 遗留问题

1. **`scan_time` 取较晚者**（第 3 节）：判据已写明；若惯例要求一律不选，回退这一处即可。
2. **逐字段读取失败在本类没有通道。** 采集端没有「单个字段读不到」的键——BitLocker、Secure Boot、Device Guard 的拒绝读取会让**整个来源失败并以 `gap` 到达**（S05 记录第 5 节第 1 项、夹具 03）。适配器因此在契约能承载的地方尊重该陈述：`decoding.unreadable_fields`。**没有任何一处把拒绝读取变成「缺失的值」**。
3. **部分覆盖已在代码里写明，而不是填满**：策略来源只能触及 `security.protection` 的一个字段；Device Guard 的 `RequiredSecurityProperties`、证书的 `signature_algorithm`/`trust` 等没有声明字段，留在证据里。
4. **`security.detections` 若要产出对象，需要目录先为检出声明类型或为它登记动作**——属目录范围（S03/S21），不是适配器能定的。
5. **本机未做真实采集实测**：BitLocker、Secure Boot、Device Guard 在本机权限受限，全部验证在 `:memory:` 上用手工构造的素材完成。
