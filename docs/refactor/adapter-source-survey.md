# 来源素材结构勘察（S10–S20 前置）

日期：2026-09-12。方法：对照采集端自身的 emit 声明逐来源核对，不凭目录描述推断。

本文只记录**结构**——素材长什么样、身份键从哪里来、哪些结论会影响多个类别。逐步的实现与验收留在各自的 `acceptance/Sxx-*.md`。

## 1. 四种素材形状

S09 建立的引擎覆盖前三种；勘察显示还需要第四种。

| 形状 | 出现处 | 结构 |
| --- | --- | --- |
| 一条素材 = 一个对象 | 多数来源 | `Item(id, data)`，`data` 是具名属性表 |
| 一条素材 = 多个对象 | 组成员、账户权限、注册表**值级**类型 | 素材内的一张列表或一张值表展开为多个候选 |
| 多条素材 = 一个对象 | 机器身份（两个 CIM 类） | 身份取自采集作用域 |
| **一条素材 = 一棵树** | `software.portable`、`files.metadata-security`、`shell.registrations` | 一个 item 同时是对象、权限项与数据流的来源 |

第四种是在 S10 之后才完全看清的：`files.metadata-security` 的一个 item 里，`dacl_sddl` 是若干 `file.acl_entry` 的来源，`streams` 是若干 `file.alternate_data_stream` 的来源，`sha256`/`size` 属于 `file.watched_file` 本身。S14 要按这些字段各自展开。

## 2. 跨类别的共同事实

这些事实影响多个类别，写在这里以免每类重新发现。

1. **`pcscope.eventlog.*` 不是对象素材。** 它们的消息是 `event_batch`，每条事件带 `kind`（就是动作 id）与 `object_id`；目标类型由目录中该动作的 `target_type` 决定，不存在「每个来源一个类型」。因此它们归**事件适配器**（S17 的统一入口），不属于各类别的对象适配器。把事件硬塞进对象适配器会重复实现分派。
2. **注册表型 item 的形状是固定的**，由 `collect-domain.ps1` 的 `Registry-Key` 生成：`Item "$hive/$view/$path" @{path;hive;view;values;basis;collection}`，其中 `values` 是 `名称 -> {kind,value}`。**`$allowed` 参数不用于过滤**——代码注释写明「保留配置值，不只是历史上的显示字段白名单」，所以每个键下的全部值都被采集。
3. **注册表型来源的身份策略按类型分三种**，不能一律套用：
   - `registry_registration` → `[scope, hive, view, registry_path]`（`shell.*`、`startup.session-hooks`、`environment.configuration` 的文件关联部分）；
   - `native_path` → `[scope, path]`（`desktop.shell-settings`、`desktop.input`、`network.winrm-configuration`、`policy.os_configuration`）；
   - `value_key` → `[scope, path, value_name]`，**值级身份**，要按 `values` 表展开（`policy.browser_policy`、`policy.uac_policy`、`environment_variable`、`proxy_configuration`）。
4. **`Fields` 会给 CIM 实例追加 `source_properties`**，非 CIM 对象没有。它属于材料，不是被声明的字段，适配器不把它当作字段映射。
5. **`software` 类 8 个类型没有任何动作**；`security`、`hardware` 等类别多数类型也没有。勘察同时显示多个类别的动作只来自事件来源，不来自快照来源。

## 3. 已看清但尚未解决的结构问题

1. **`file_identity` 的键在素材里不存在。** `file.watched_file`、`file.acl_entry`、`file.alternate_data_stream`、`desktop.shortcut`、`software.portable_registration` 的身份策略都是 `file_identity`，键为 `[volume_identity, file_id]`，但采集端只发 `path` 与元数据，**从不发卷标识与文件 ID**。R02 明确禁止适配器另行调用 Windows 补采。因此在 S14 之前必须决定：要么采集端补发这两个值，要么改这类类型的身份策略。**这不是适配器能自己解决的**，列入 S14 的前置与 S21 对账。
2. **身份键在素材里根本不存在——同类问题共发现八例。** 除 `file_identity`（第 1 项）之外，S16/S18/S20 实施时又发现下表各例：

   | 类型 | 缺的键 | 素材实际提供 | 后果 |
   | --- | --- | --- | --- |
   | `network.address_configuration`、`network.route`、`network.dns_configuration` | `interface_guid` | 只有接口**索引与别名**（`InterfaceIndex`/`InterfaceAlias`） | 三个网络类型无法按身份与网卡清单归一，只能暂定身份 |
   | `network.smb_session` | `server` | 客户端与用户名 | 同一客户端同一用户的多个会话共用一个键 |
   | `network.endpoint`（UDP 来源） | `protocol`、远端地址与端口 | 只有本地四项 | UDP 端点身份不完整 |
   | `apps.mihomo` | `value_name` | 一张扁平的运行配置表 | 值是整表而非逐值 |
   | `desktop.shortcut` | `volume_identity`、`file_id` | 路径与快捷方式元数据 | 每个快捷方式都是暂定身份 |
   | `desktop.editor_extension` | `editor`、`extension_id` | 只有 `name`/`publisher`/`version`/`engines` | 编辑器与扩展 ID 均缺失（S20 实施时发现，本文原未列出） |

   统一性质：**类型声明的键出自模型设计，采集端从未发过**。这不是适配器能补的——补就等于编造。需要在「采集端补发」与「改类型身份策略」之间逐项决定，列入 S21。`network.endpoint` 的 TCP 来源能给出 `protocol`（适配器知道自己是什么来源），UDP 来源不能，这个不对称也应由目录而非适配器来定。

3. **同一对象被两个来源以不同作用域采集。** `hardware.volume` 同时由 `storage.volumes`（机器作用域）与 `usn.volumes`（Rust 插件，作用域是 `fixed-drive-letters/v1`）提供，两者作用域不同，同一个卷会成为两个实例。与共享改动 0005 修的同类问题，但这次的差异在**来源之间**而不是同一来源的采集切分。列入 S21。

4. **三个目录类型没有任何来源。** `file.watched_directory`（目录监控未接入来源）、`security.key_metadata`、`hardware.driver_package`（驱动仓库枚举未接入来源）在 `source-map.cjs` 中没有对应行。按计划「未接入类型单独标记，不以空列表冒充已采集完成」，它们应在对账中显式标记为未接入，而不是补一个空适配器。

### 3.5 值级身份的作用域约定（S12 与 S15 各自独立得出，结论一致）

`value_key` 策略的键是 `[scope, path, value_name]`，**没有给 hive 和注册表视图留位置**。但采集端在同一个路径上会读到两个视图（`Registry64`/`Registry32`），对策略类还会读到两个配置单元（`LocalMachine`/`CurrentUser`）。不把这两者放进作用域，32 位与 64 位视图下同路径同名的值就会撞成一个对象——而它们是两条不同的登记。

**约定：** 值级身份的作用域 = 
- 配置单元为 `CurrentUser` → 保留交来的作用域（含采集用户段）；否则 → `installation_scope`（去掉采集用户段，机器级归属）；
- 若视图存在 → 追加 `/view:<view>`。

`registry_registration` 类**不**这样做：它的键里本来就有 `hive` 与 `view`。

S12（启动项）与 S15（策略）两条线各自独立推导出这一条，结论一致。**但两处各写了一份实现**（`startup.rs` 的 `logon_scope` 与 `policy.rs` 的 `value_scope`/`hive_scope`），规则相同、签名不同，对「hive 未知」的处理也有细微差别。应在该差别定清之后收拢进引擎，避免两份实现日后各自漂移。已登记为待办。

## 4. 分类别要点

| 步骤 | 类别 | 对象来源 | 备注 |
| --- | --- | --- | --- |
| S11 | runtime | 4 | 进程、模块、登录会话、终端会话；进程身份 `[scope,pid,created_at]` |
| S12 | startup | 10 | 服务、计划任务、登录启动项、会话钩子、IFEO、WMI 过滤器/消费者/绑定；`startup.logon` 有**三种** item 形态（Run 键、StartupApproved、启动文件夹） |
| S13 | shell | 3（+1 跨类） | `shell.registrations` 是**一棵树**，`Read-RegistryTree` 逐键产出，一个来源覆盖七个类型；`com.clsid` 用 `composite_native`，其余用 `registry_registration`。item 里**不含** hive/view，要从消息级 scope 取 |
| S14 | file | 1 | `files.metadata-security` 一个来源覆盖三个类型，见第 3 节第 1 项 |
| S15 | policy | 9 | 多为注册表型；`system.clock`/`restart-pending` 是 `[scope,path]`，`browser-policies` 是值级 |
| S16 | network | 18 | 类型最多；端点身份是七元组，TCP/UDP 两源共用 `network.endpoint` |
| S17 | security | 10 + 事件 | 证书来源有 currentuser/localmachine 两个，同一类型 |
| S18 | hardware | 11 | 含存储与 USN 卷（`pcscope.usn.volumes` 由 Rust 插件发出，形状与 PS 不同） |
| S19 | virtualization | 7 | Hyper-V 四源 + WSL 注册表 + Docker（一个来源覆盖四个类型）+ 平台能力两项式 |
| S20 | desktop | 9 | 打印机四源 + 注册表三项 + 快捷方式（`file_identity`，见第 3 节） |

## 5. 勘察未发现遗漏

除第 3 节列出的各项外，`source-map.cjs` 中每个来源的采集端都已定位。**本机未做真实采集实测**，本文的键名均来自采集端源码中的 emit 声明；键名与真实输出的核对并入 S21。

另有一处样本与采集端不一致，记录以免误判：`fixtures/01-baseline.jsonl` 里有若干 item 的键用的是**目录字段名**（`local_address`、`name`、`version`…），而采集端发的是它自己的键名（`LocalAddress`、`DisplayName`…）。样本是按类型定义构造的，不是从采集端抄的。没有示例读它们，因此没有回归；S21 核对键名时以采集端为准。

