# PCScope 采集原始信息与界面展示映射全册

生成日期：2026-09-11。依据当前工作区代码（包含未提交修改），不是仅依据上一次 Git 提交。实现指纹：`9465f7050fc959d7b5ea253374e8ab31a59efa7aefb1efd34d8d51667c76d26d`。

覆盖：219 个采集项、13 个插件目录分组、23 个日志通道、16 个事件字段模板、6 个对象属性模板。每个采集项均有接口、字段说明、展示规则和示例。字段表将“文档中的含义解释”与“当前界面的标签”分别列出，避免把补充说明误写成界面已实现的翻译。

**阅读约定**：本文描述已经实现的代码，不把规划当成现状。示例均为合成数据；只使用字段名、类型形状及公开的演示值，不复制本机账户、密钥名、路径、正文或凭据。示例 JSON 是省略公共元数据后的展示输入，不是可直接投递插件协议的完整消息。

**“完整”的范围**：完整列举当前项目的目录、固定字段、动态字段容器和界面选择规则。Windows/CIM/注册表字段可以随系统版本及厂商扩展，不能用有限字典列完未来字段；动态部分明确以通配容器描述，原始数据入口保留。各来源的声明字段集合包含不同对象分支和嵌套成员，不能视为所有对象都具有的同级属性。

## 阅读索引

- [数据流与采集调度](#dataflow)
- [原始消息与字段字典](#raw-schema)
- [字段处理、隐私和缺失状态](#field-policy)
- [界面结构与全部模板](#ui-templates)
- [全部采集项逐项映射](#all-items)
- [兜底、差异和已知缺口](#fallback)
- [源码片段与重建方法](#source-appendix)

<a id="dataflow"></a>
## 1. 数据流与采集调度

操作系统 API／注册表／文件／日志 → 采集器投影和保护 → JSON Lines 协议 → SQLite 原始记录与分类索引 → 隐私显示投影 → 展示模板 → 时间线／系统对象。

| 阶段 | 当前实现 | 例子 |
| --- | --- | --- |
| 快照插件 | 默认 300 秒，允许 30–604800 秒；可单独启停和手动触发 | 服务清单两次采集 State 从 Running 变为 Stopped |
| 事件日志、USN | 默认 10 秒，允许 10–604800 秒；持久化断点增量轮询 | Security 5058 作为一次操作记录入库 |
| 日志轮换 | 每通道每轮最多 128 条；System/Application 主通道与扩展通道轮换 | 配置 10 秒不等于每个扩展通道每 10 秒都完成一次读取 |
| 事件驱动含义 | 数据来源是事件，但当前采集由周期轮询驱动；不要写成全部使用实时订阅 | GetLogInformation + EventLogReader.ReadEvent |
| 文件范围 | 文件名单默认空；匹配显式本地文件路径 | 加入 C:\Example\watched.txt 后才采集范围内文件变化 |
| 历史保留 | 默认 5GB 滚动删除，用户可改无限；删除原始历史不会把对象最新状态直接全部清空 | 旧事件被清理后，关联查询可能无匹配记录 |
| 前端刷新 | 自动刷新／暂停／载入最新只控制前端 | 暂停后后台仍继续采集 |

<a id="raw-schema"></a>
## 2. 原始消息与字段字典

### 2.1 三类采集消息

| 类型 | 字段 | 含义与例子 |
| --- | --- | --- |
| 快照 snapshot | source、scope、run_id、observed_at、complete、items[{id,data}] | scope 为机器/用户/分片范围；complete=true 表示该范围枚举完成。例：items=[{id:"ExampleService",data:{state:"Running"}}] |
| 事件 event_batch | source、epoch、batch_seq、checkpoint、events[{id,occurred_at,kind,object_id,data}] | epoch 区分日志轮次，checkpoint 保存来源断点。例：事件 id="1234"、kind="windows.event" |
| 缺口 gap | source、observed_at、reason | 例：reason="Access denied"；此前对象状态保留 |
| 公共协议 | protocol_version、request_id、type | 例：protocol_version=1；请求和批次身份用于验证与幂等 |

快照会生成初始状态、新发现、属性变化、不再发现等记录。变化数据包含 before/after；界面时间是观察时间。日志事件优先用来源事件时间，没有来源时间时使用采集时间并保留 source_time_available=false。

### 2.2 入库后与界面使用的外层字段

| 字段 | 含义 | 例子／界面 |
| --- | --- | --- |
| seq | PCScope 记录编号 | 340540；来源与记录信息 |
| source | 来源唯一标识 | pcscope.eventlog.security；分类匹配 |
| kind | 采集时记录类型 | windows.event；保留原类型 |
| action | 分类索引中的归一化动作 | event.security.microsoft_windows_security_auditing.5058；动作筛选 |
| item / item_name | 采集项标识和中文名称 | pcscope.eventlog.security.crypto-keys / Windows 安全审计 / 加密密钥操作 |
| object_id / id | 事件对象／快照对象身份 | Security/提供程序/5058 或服务名；不一定直接用作可读标题 |
| occurred_at | 事件来源时间或观察时间 | 2026-09-11T09:14:44Z → 本机时间 |
| seen_at / changed_at | 对象最近成功采集／最近变化观察时间 | 系统对象固定显示 |
| data | 采集到的业务数据 | 快照属性或事件详情 |
| before / after | 变化前后属性 | state: Running → Stopped；左右比较 |
| before_observed_at / scope | 变化关联区间及快照范围 | 用于任务快照关联 |

### 2.3 Windows 事件 data 的全部固定字段

| 字段 | 含义 | 例子 |
| --- | --- | --- |
| channel | 日志通道 | "Security" |
| record_id | Windows 日志记录编号 | 1234 |
| provider | 提供程序 | "Microsoft-Windows-Security-Auditing" |
| event_id | 事件编号 | 5058 |
| level | 来源级别，0 未指定；1 严重、2 错误、3 警告、4 信息、5 详细 | 0 |
| user_sid | System/Security 的 SID；与 EventData 操作账户字段不同 | null |
| writer_process_id | 日志写入进程编号 | 1568 |
| writer_thread_id | 日志写入线程编号 | 100 |
| xml_sha256 | 源 XML 摘要 | "示例摘要" |
| raw_xml | 完整源 XML；DPAPI 加密保存，显示由隐私设置控制 | "******" |
| payload_collected | 当前实现按正文是否非空设置；不是“fields 是否完整”的判断 | true |
| message | FormatDescription 返回的来源正文 | "Key file operation." |
| message_status | available/not_provided/unavailable/protected 等 | "available" |
| message_error | 正文读取错误；可为空 | null |
| fields | EventData/UserData 动态叶字段 | "{\"KeyName\":\"ExampleKey\"}" |
| system | System 子元素文本和属性 | "{\"EventID\":{\"value\":\"5058\"}}" |
| collection.policy | 采集策略版本 | "evidence-v2" |
| collection.field_issues | 字段保护、预览长度和投影预算状态 | [] |
| collection.raw_xml_stored | 是否保存完整 XML | true |
| collection.raw_xml_reason | XML 保存和显示策略 | "full_source_xml_encrypted_display_controlled_by_privacy_setting" |
| field_schema | 投影格式版本 | "evidence-v2" |
| basis | 数据依据标记 | "persisted Windows event evidence" |
| source_time_available | 是否使用了来源事件时间 | true |

### 2.4 动态字段命名与保留

| 来源结构 | 保存形式 | 例子 |
| --- | --- | --- |
| 有 Name 的 EventData/Data | fields.<Name> | <Data Name="KeyName">ExampleKey</Data> → fields.KeyName |
| 匿名 Data | Data[顺序号] | fields["Data[1]"]="示例值" |
| 重名字段 | 字段名#顺序号 | Name、Name#3 分别保留 |
| UserData 叶节点 | 祖先路径/节点名 | fields["Operation/Result"]="0" |
| System 子元素 | {value,属性…} | Correlation.ActivityID、Execution.ProcessID |
| CIM 动态属性 | source_properties.* | 选定属性之外的 CimInstanceProperties 递归保存 |
| 注册表 | values.{登记名}.{kind,value} | values.DisplayName={kind:"String",value:"Example"} |
| 二进制值 | encoding、bytes、value | PowerShell 使用 base64；Rust 软件采集器使用 byte-array |

<a id="field-policy"></a>
## 3. 字段处理、隐私与缺失状态

| 情况 | 保存／读取行为 | 界面和例子 |
| --- | --- | --- |
| 敏感字段 | DPAPI CurrentUser 加密对象；服务按显示设置投影 | 默认 ****** → 易读字段“已隐藏”；允许显示时解密 |
| 较长事件字段 | 超过 16384 字符保存 preview 和加密 full_value | 全部字段可见预览结构；不应写成完整正文已丢弃 |
| 结构化事件预算 | 累计超过 262144 字节时停止继续投影，并记录 field_issues | 完整 raw_xml 仍保留 |
| 单事件／单消息预算 | 单事件约 950000 字节，协议消息 1MiB；超限不推进相应断点 | 形成采集缺口 |
| 嵌套 CIM | 深度超过 8 层失败并不提交不完整来源 | 此前状态保留 |
| 缺失/null/空串/短横线 | 模板选择器跳过；完整字段渲染 null 为未记录、空串为空值 | 关键字段未返回时通常直接不显示该行 |
| false / 0 | 保留，不能按缺失处理 | enabled=false → 否；返回码 0 → 仅按指定事件规则解释 |
| 未知枚举 | 保留原码；部分 %% 码显示未解析 | %%9999 → 未解析 · %%9999 |
| 历史旧数据 | 新解释规则不增加当时未保存的数据 | 旧版记录：正文未保存 |
| 真实缺口 | 来源状态变化持久化；同一缺口持续不重复生成时间线条目 | 缺口 → 恢复 → 再次缺口是不同阶段 |

示例：敏感值存储结构为 `{sensitive:true, protection:"dpapi-current-user-v1", ciphertext:"示例密文"}`；本文不会生成真实密文或展示本机秘密。

<a id="ui-templates"></a>
## 4. 界面结构与全部字段模板

### 4.1 共同布局

| 位置 | 选择规则 | 例子 |
| --- | --- | --- |
| 标题 | 已定义动作 + 首个匹配对象字段；密钥操作可进一步解码 | 删除密钥文件 · ExampleKey |
| 结果标签 | 仅 outcome() 明确支持的事件 | 成功；其余不显示标签 |
| 摘要 | 操作账户、程序，加最多两项操作属性；某些长字段/重复名称排除 | 操作账户：demo · 程序：未关联 · PID 7096 |
| 执行信息 | SubjectUserName/User/UserContext/UserName；ClientProcessId 优先；4688 用 NewProcessId；有值才显示 | 写入 PID 不用于调用程序认领 |
| 操作属性 | 下列 16 类模板；别名按从左到右第一个非空值选择 | TargetName 优先于 Resource |
| 原始与技术信息 | 默认折叠 | 来源正文、全部字段、其他属性、来源与记录信息 |
| 原始数据 Tab | 整个界面收到的 JSON | 保留字段名与结构，遵循隐私显示策略 |
| 系统对象 | 对象名、时间、领域优选属性；完整属性折叠 | 无优选字段时完整属性默认展开 |
| 快照变化 | 初始状态/新发现/属性变化/不再发现 | 左右前后属性对照及差异高亮 |

字段名匹配忽略大小写、下划线、空格与连字符；不自动寻找更深层结构，也不是自动把任意同义字段映射过来。下列字段别名是界面接受的输入，不表示每个事件都返回全部字段。

### 4.2 密钥操作（keys）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 密钥名称 | `KeyName` | KeyName="ExampleKey" → ExampleKey | 关键属性中省略；原始字段仍在折叠区 |
| 密钥类型 | `KeyType` | KeyType="%%2500" → 用户密钥 | 关键属性中省略；原始字段仍在折叠区 |
| 密钥文件 | `KeyFilePath` | KeyFilePath="C:\\Users\\demo\\AppData\\Roaming\\Microsoft\\Crypto\\Keys\\example" → C:\Users\demo\AppData\Roaming\Microsoft\Crypto\Keys\example | 关键属性中省略；原始字段仍在折叠区 |
| 算法 | `AlgorithmName` | AlgorithmName="ECDSA_P256" → ECDSA_P256 | 关键属性中省略；原始字段仍在折叠区 |
| 存储提供程序 | `ProviderName` | ProviderName="Microsoft Software Key Storage Provider" → Microsoft Software Key Storage Provider | 关键属性中省略；原始字段仍在折叠区 |
| 操作原码 | `Operation` | Operation="%%2457" → 未解析 · %%2457 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`KeyName="ExampleKey"` → 密钥名称；`KeyType="%%2500"` → 密钥类型。

### 4.3 凭据操作（credentials）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 凭据目标 | `TargetName` → `Resource` | TargetName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 读取操作 | `ReadOperation` | ReadOperation="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 返回数量 | `CountOfCredentialsReturned` | CountOfCredentialsReturned="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 目标账户 | `TargetUserName` | TargetUserName="demo" → demo | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`TargetName="示例值"` → 凭据目标；`ReadOperation="示例值"` → 读取操作。

### 4.4 登录与会话（sessions）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 目标账户 | `TargetUserName` → `AccountName` | TargetUserName="demo" → demo | 关键属性中省略；原始字段仍在折叠区 |
| 登录类型 | `LogonType` | LogonType="2" → 交互登录 | 关键属性中省略；原始字段仍在折叠区 |
| 来源地址 | `IpAddress` → `SourceNetworkAddress` | IpAddress="192.0.2.10" → 192.0.2.10 | 关键属性中省略；原始字段仍在折叠区 |
| 来源端口 | `IpPort` | IpPort="50000" → 50000 | 关键属性中省略；原始字段仍在折叠区 |
| 工作站 | `WorkstationName` | WorkstationName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 登录会话 | `TargetLogonId` → `SubjectLogonId` | TargetLogonId="0x12345" → 0x12345 | 关键属性中省略；原始字段仍在折叠区 |
| 认证方式 | `AuthenticationPackageName` | AuthenticationPackageName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 失败原因 | `FailureReason` | FailureReason="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 状态码 | `Status` | Status="0x0" → 0x0 | 关键属性中省略；原始字段仍在折叠区 |
| 子状态码 | `SubStatus` | SubStatus="0x0" → 0x0 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`TargetUserName="demo"` → 目标账户；`LogonType="2"` → 登录类型。

### 4.5 账户、组与权限（identity）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 目标账户或组 | `TargetUserName` → `GroupName` | TargetUserName="demo" → demo | 关键属性中省略；原始字段仍在折叠区 |
| 成员 | `MemberName` → `MemberSid` | MemberName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 权限 | `PrivilegeList` | PrivilegeList="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 登录会话 | `SubjectLogonId` | SubjectLogonId="0x12345" → 0x12345 | 关键属性中省略；原始字段仍在折叠区 |
| 目标进程 | `TargetProcessName` | TargetProcessName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 目标进程编号 | `TargetProcessId` | TargetProcessId="S-1-5-21-100-200-300-1001" → S-1-5-21-100-200-300-1001 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`TargetUserName="demo"` → 目标账户或组；`MemberName="示例值"` → 成员。

### 4.6 进程与模块（process）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 程序路径 | `NewProcessName` → `Image` → `ProcessName` | NewProcessName="C:\\Example\\app.exe" → C:\Example\app.exe | 关键属性中省略；原始字段仍在折叠区 |
| 进程编号 | `NewProcessId` → `ProcessId` | NewProcessId="0x1bb8" → 0x1bb8 | 关键属性中省略；原始字段仍在折叠区 |
| 父进程 | `ParentProcessName` → `ParentImage` | ParentProcessName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 父进程编号 | `CreatorProcessId` → `ParentProcessId` | CreatorProcessId="S-1-5-21-100-200-300-1001" → S-1-5-21-100-200-300-1001 | 关键属性中省略；原始字段仍在折叠区 |
| 命令行 | `CommandLine` | CommandLine="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 模块或驱动 | `ImageLoaded` | ImageLoaded="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 目标程序 | `TargetImage` | TargetImage="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 目标进程编号 | `TargetProcessId` | TargetProcessId="S-1-5-21-100-200-300-1001" → S-1-5-21-100-200-300-1001 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`NewProcessName="C:\\Example\\app.exe"` → 程序路径；`NewProcessId="0x1bb8"` → 进程编号。

### 4.7 文件操作（files）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 路径 | `TargetFilename` → `ObjectName` → `Path` | TargetFilename="C:\\Example\\watched.txt" → C:\Example\watched.txt | 关键属性中省略；原始字段仍在折叠区 |
| 操作 | `OperationType` → `AccessList` → `AccessMask` | OperationType="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 对象类型 | `ObjectType` | ObjectType="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 原安全描述符 | `OldSd` | OldSd="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 新安全描述符 | `NewSd` | NewSd="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 文件摘要 | `Hashes` | Hashes="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`TargetFilename="C:\\Example\\watched.txt"` → 路径；`OperationType="示例值"` → 操作。

### 4.8 注册表操作（registry）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 注册表对象 | `TargetObject` → `ObjectName` | TargetObject="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 值名称 | `ObjectValueName` | ObjectValueName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 变化前 | `OldValue` | OldValue="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 变化后 | `NewValue` → `Details` | NewValue="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 重命名后 | `NewName` | NewName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`TargetObject="示例值"` → 注册表对象；`ObjectValueName="示例值"` → 值名称。

### 4.9 计划任务（tasks）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 任务名称 | `TaskName` | TaskName="\\Example\\DailyTask" → \Example\DailyTask | 关键属性中省略；原始字段仍在折叠区 |
| 任务实例 | `TaskInstanceId` → `InstanceId` | TaskInstanceId="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 执行动作 | `ActionName` | ActionName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 运行账户 | `UserContext` | UserContext="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 执行进程编号 | `EnginePID` → `ProcessID` | EnginePID="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 返回码 | `ResultCode` | ResultCode="0" → 0 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`TaskName="\\Example\\DailyTask"` → 任务名称；`TaskInstanceId="示例值"` → 任务实例。

### 4.10 服务（services）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 服务名称 | `ServiceName` → `param1` | ServiceName="ExampleService" → ExampleService | 关键属性中省略；原始字段仍在折叠区 |
| 服务状态 | `State` → `param2` | State="示例状态" → 示例状态 | 关键属性中省略；原始字段仍在折叠区 |
| 执行程序 | `ServiceFileName` → `ImagePath` | ServiceFileName="C:\\Example\\item" → C:\Example\item | 关键属性中省略；原始字段仍在折叠区 |
| 运行账户 | `ServiceAccount` → `AccountName` | ServiceAccount=1024 → 1024 | 关键属性中省略；原始字段仍在折叠区 |
| 启动方式 | `ServiceStartType` → `StartType` | ServiceStartType="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`ServiceName="ExampleService"` → 服务名称；`State="示例状态"` → 服务状态。

### 4.11 软件、应用包与更新（software）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 软件或包 | `ProductName` → `PackageFullName` → `PackageName` | ProductName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 版本 | `ProductVersion` → `Version` | ProductVersion="1.0.0" → 1.0.0 | 关键属性中省略；原始字段仍在折叠区 |
| 操作 | `DeploymentOperation` → `Operation` | DeploymentOperation="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 错误码 | `ErrorCode` → `Error` | ErrorCode="0" → 0 | 关键属性中省略；原始字段仍在折叠区 |
| 路径 | `Path` | Path="C:\\Example\\item" → C:\Example\item | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`ProductName="示例值"` → 软件或包；`ProductVersion="1.0.0"` → 版本。

### 4.12 网络、DNS 与共享（network）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 程序 | `Application` → `Image` | Application="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 来源地址 | `SourceAddress` → `SourceIp` | SourceAddress="192.0.2.10" → 192.0.2.10 | 关键属性中省略；原始字段仍在折叠区 |
| 来源端口 | `SourcePort` | SourcePort="50000" → 50000 | 关键属性中省略；原始字段仍在折叠区 |
| 目标地址 | `DestAddress` → `DestinationIp` → `DestinationHostname` | DestAddress="192.0.2.10" → 192.0.2.10 | 关键属性中省略；原始字段仍在折叠区 |
| 目标端口 | `DestPort` → `DestinationPort` | DestPort="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 协议 | `Protocol` | Protocol="6" → 6 | 关键属性中省略；原始字段仍在折叠区 |
| 方向 | `Direction` | Direction="%%14593" → 未解析 · %%14593 | 关键属性中省略；原始字段仍在折叠区 |
| 查询名称 | `QueryName` | QueryName="example.test" → example.test | 关键属性中省略；原始字段仍在折叠区 |
| DNS 结果 | `QueryResults` | QueryResults="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| DNS 状态码 | `QueryStatus` | QueryStatus="示例状态" → 示例状态 | 关键属性中省略；原始字段仍在折叠区 |
| 共享名称 | `ShareName` | ShareName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`Application="示例值"` → 程序；`SourceAddress="192.0.2.10"` → 来源地址。

### 4.13 组策略（policy）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 策略对象 | `GPOList` → `GPOLIst` → `GPOName` | GPOList="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 排除原因 | `Reason` | Reason="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 处理模式 | `ProcessingMode` | ProcessingMode="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 会话 | `SessionId` | SessionId="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 耗时（毫秒） | `ProcessingTimeInMilliseconds` → `ElapsedTimeInMilliseconds` | ProcessingTimeInMilliseconds="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 错误码 | `ErrorCode` | ErrorCode="0" → 0 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`GPOList="示例值"` → 策略对象；`Reason="示例值"` → 排除原因。

### 4.14 WMI 与对象访问（wmi）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 对象服务器 | `ObjectServer` | ObjectServer="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 对象类型 | `ObjectType` | ObjectType="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 对象 | `ObjectName` | ObjectName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 操作 | `Operation` → `AdditionalInfo` | Operation="%%2457" → 未解析 · %%2457 | 关键属性中省略；原始字段仍在折叠区 |
| 命名空间 | `NamespaceName` | NamespaceName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 提供程序 | `ProviderName` | ProviderName="Microsoft Software Key Storage Provider" → Microsoft Software Key Storage Provider | 关键属性中省略；原始字段仍在折叠区 |
| 客户端进程编号 | `ClientProcessId` | ClientProcessId="7096" → 7096 | 关键属性中省略；原始字段仍在折叠区 |
| 结果码 | `ResultCode` | ResultCode="0" → 0 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`ObjectServer="示例值"` → 对象服务器；`ObjectType="示例值"` → 对象类型。

### 4.15 安全防护（security）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 威胁名称 | `ThreatName` → `Threat Name` | ThreatName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 检测对象 | `Path` → `DetectionPath` | Path="C:\\Example\\item" → C:\Example\item | 关键属性中省略；原始字段仍在折叠区 |
| 处置动作 | `ActionName` → `Action Name` | ActionName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 错误码 | `ErrorCode` → `Error Code` | ErrorCode="0" → 0 | 关键属性中省略；原始字段仍在折叠区 |
| 引擎版本 | `EngineVersion` | EngineVersion="1.0.0" → 1.0.0 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`ThreatName="示例值"` → 威胁名称；`Path="C:\\Example\\item"` → 检测对象。

### 4.16 脚本与命令（script）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 脚本路径 | `Path` → `ScriptName` | Path="C:\\Example\\item" → C:\Example\item | 关键属性中省略；原始字段仍在折叠区 |
| 命令 | `CommandName` | CommandName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 脚本块标识 | `ScriptBlockId` | ScriptBlockId="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 分段编号 | `MessageNumber` | MessageNumber="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 总段数 | `MessageTotal` | MessageTotal="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`Path="C:\\Example\\item"` → 脚本路径；`CommandName="示例值"` → 命令。

### 4.17 系统通用／模板兜底（system）

| 界面字段 | 原始 fields 别名（依次选择） | 每字段示例 | 未命中时 |
| --- | --- | --- | --- |
| 涉及对象 | `ObjectName` → `DeviceName` → `Name` | ObjectName="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 变化前时间 | `PreviousTime` | PreviousTime="2026-09-11T09:00:00Z" → 2026-09-11 17:00:00（本机时间） | 关键属性中省略；原始字段仍在折叠区 |
| 变化后时间 | `NewTime` | NewTime="2026-09-11T09:00:00Z" → 2026-09-11 17:00:00（本机时间） | 关键属性中省略；原始字段仍在折叠区 |
| 原因 | `Reason` | Reason="示例值" → 示例值 | 关键属性中省略；原始字段仍在折叠区 |
| 状态 | `State` → `Status` | State="示例状态" → 示例状态 | 关键属性中省略；原始字段仍在折叠区 |
| 结果码 | `ResultCode` → `ErrorCode` | ResultCode="0" → 0 | 关键属性中省略；原始字段仍在折叠区 |

**字段例子**：`ObjectName="示例值"` → 涉及对象；`PreviousTime="2026-09-11T09:00:00Z"` → 变化前时间。

### 4.18 结果标签规则

| 限定事件 | 原始依据 | 显示例子 |
| --- | --- | --- |
| Security 5058/5059/5061/5379/5382 | ReturnCode 十进制或十六进制整数 | 0x0→成功；5→失败；缺失→未记录；******→已隐藏 |
| Security 4624 / 4625 | 事件定义 | 成功／失败；失败可显示 Status |
| Security 5156 / 5157 | 事件定义 | 已允许／已阻止 |
| TaskScheduler 201 | 动作结束事件，保留 ResultCode | 已结束；ResultCode=5 不自动显示“Windows 错误 5” |
| WMI-Activity 5858 | 失败事件定义和 ResultCode | 失败 |
| 其余事件 | 没有专门结果规则 | 不显示独立标签，即便字段中包含 0 |

### 4.19 六类系统对象属性模板

| 对象来源匹配 | 优先展示字段 | 例子 |
| --- | --- | --- |
| identity | Name、FullName、Enabled、Disabled、PrincipalSource、Members、Rights、LastLogon、PasswordLastSet | Name=demo；Enabled=true |
| startup / execution | Name、DisplayName、State、Status、StartMode、StartAccount、Path、Execute、UserId、RunLevel、LastRunTime、NextRunTime、LastTaskResult | State=Running；StartMode=Auto |
| software / apps / applications | DisplayName、DisplayVersion、Publisher、InstallDate、InstallLocation、Version、PackageFullName | DisplayName=Example；DisplayVersion=1.0 |
| network | Name、InterfaceAlias、Status、Address、IPAddress、PrefixLength、MacAddress、LinkSpeed、RemoteAddress、RemotePort | Address=192.0.2.10 |
| shell / environment | Name、DisplayName、Description、Path、RegistryPath、Enabled、CLSID、InprocServer32、LocalServer32 | CLSID={示例标识} |
| system / hardware / storage / power / virtualization / desktop / security / policy / files | Name、Caption、State、Status、Enabled、Manufacturer、Model、Version、Path、Capacity、FileSystem、HypervisorPresent、FirmwareVirtualization | Capacity=1024；FileSystem=NTFS |

例：Win32_Processor 的 `slat` 已采集，但不在此对象优选列表内，因此需要展开全部属性。仅列入优选列表的属性默认显示，完整属性不因未入优选列表而被删除。

<a id="all-items"></a>
## 5. 全部采集项逐项映射

| 编号 | 采集项 | source |
| --- | --- | --- |
| [001](#item-001) | 本机软件安装登记 | pcscope.software |
| [002](#item-002) | 操作系统版本与安装时间 | pcscope.system.version |
| [003](#item-003) | 最近启动时间与时区 | pcscope.system.boot-state |
| [004](#item-004) | 本机用户账户 | pcscope.identity.accounts |
| [005](#item-005) | 本机用户组与成员 | pcscope.identity.groups |
| [006](#item-006) | 服务运行状态 | pcscope.startup.services |
| [007](#item-007) | 计划任务基础配置 | pcscope.startup.tasks |
| [008](#item-008) | 卷与文件系统 | pcscope.storage.volumes |
| [009](#item-009) | 网络适配器 | pcscope.network.adapters |
| [010](#item-010) | IP 地址 | pcscope.network.addresses |
| [011](#item-011) | 虚拟化能力与平台 | pcscope.virtualization.platform |
| [012](#item-012) | 即插即用设备 | pcscope.hardware.devices |
| [013](#item-013) | QFE 更新登记 | pcscope.system.hotfixes |
| [014](#item-014) | 第三方预览处理程序 | pcscope.shell.registrations |
| [015](#item-015) | 图标覆盖处理程序 | pcscope.shell.registrations |
| [016](#item-016) | Shell 扩展允许与阻止列表 | pcscope.shell.registrations |
| [017](#item-017) | 桌面与此电脑命名空间 | pcscope.shell.registrations |
| [018](#item-018) | 右键菜单、文件关联与默认图标 | pcscope.shell.registrations |
| [019](#item-019) | COM 实现与扩展文件元数据 | pcscope.shell.registrations |
| [020](#item-020) | 登录启动登记 | pcscope.startup.logon |
| [021](#item-021) | 用户与公共启动文件夹 | pcscope.startup.logon |
| [022](#item-022) | 计算机身份与域信息 | pcscope.identity.machine |
| [023](#item-023) | 时间同步配置与时区 | pcscope.system.clock |
| [024](#item-024) | Windows Update 历史 | pcscope.system.update-history |
| [025](#item-025) | 待重启标志 | pcscope.system.reboot-pending |
| [026](#item-026) | 系统还原点 | pcscope.system.restore-points |
| [027](#item-027) | 当前用户 AppX 包 | pcscope.software.appx-user |
| [028](#item-028) | 预配 AppX 包 | pcscope.software.provisioned |
| [029](#item-029) | Windows 可选组件 | pcscope.software.optional-features |
| [030](#item-030) | 当前用户软件登记 | pcscope.software.user-inventory |
| [031](#item-031) | 用户配置文件 | pcscope.identity.profiles |
| [032](#item-032) | 登录会话 | pcscope.identity.logon-sessions |
| [033](#item-033) | 交互式终端会话 | pcscope.identity.interactive-sessions |
| [034](#item-034) | 账户权限分配 | pcscope.identity.assigned-rights |
| [035](#item-035) | 进程清单 | pcscope.execution.processes |
| [036](#item-036) | 内存与 CPU 采样 | pcscope.execution.resources |
| [037](#item-037) | Defender 防护状态 | pcscope.security.protection |
| [038](#item-038) | Defender 策略与排除项 | pcscope.security.protection-policy |
| [039](#item-039) | 安全产品登记 | pcscope.security.products |
| [040](#item-040) | Defender 检测记录 | pcscope.security.detections |
| [041](#item-041) | Device Guard / VBS | pcscope.security.deviceguard |
| [042](#item-042) | TPM 状态 | pcscope.security.tpm |
| [043](#item-043) | 安全启动状态 | pcscope.security.secureboot |
| [044](#item-044) | 计算机证书 | pcscope.security.certificates.localmachine |
| [045](#item-045) | 当前用户证书 | pcscope.security.certificates.currentuser |
| [046](#item-046) | BitLocker 卷状态 | pcscope.security.encryption |
| [047](#item-047) | UAC、资源管理器与软件限制策略 | pcscope.policy.configuration |
| [048](#item-048) | SSH 公钥指纹 | pcscope.security.ssh-public-keys |
| [049](#item-049) | 事件通道可读性 | pcscope.collector.event-channel-coverage |
| [050](#item-050) | 路由表 | pcscope.network.routes |
| [051](#item-051) | DNS 配置 | pcscope.network.dns-configuration |
| [052](#item-052) | 网络连接配置文件 | pcscope.network.profiles |
| [053](#item-053) | TCP 连接与监听 | pcscope.network.tcp |
| [054](#item-054) | UDP 端点 | pcscope.network.udp |
| [055](#item-055) | 防火墙配置文件 | pcscope.network.firewall-profiles |
| [056](#item-056) | 防火墙规则及过滤条件 | pcscope.network.firewall-rules |
| [057](#item-057) | IPsec 规则 | pcscope.network.ipsec |
| [058](#item-058) | 代理、远程桌面与 VPN | pcscope.network.access |
| [059](#item-059) | SMB 共享与访问授权 | pcscope.network.shares |
| [060](#item-060) | SMB 驱动器映射 | pcscope.network.mapped-drives |
| [061](#item-061) | DNS 缓存 | pcscope.network.dns-cache |
| [062](#item-062) | hosts 静态映射 | pcscope.network.hosts |
| [063](#item-063) | SMB 客户端会话 | pcscope.network.share-sessions |
| [064](#item-064) | WinRM 配置 | pcscope.network.winrm-configuration |
| [065](#item-065) | 磁盘清单 | pcscope.storage.disks |
| [066](#item-066) | 分区清单 | pcscope.storage.partitions |
| [067](#item-067) | 设备驱动绑定 | pcscope.hardware.driver-bindings |
| [068](#item-068) | 驱动服务 | pcscope.hardware.driver-services |
| [069](#item-069) | BIOS 固件 | pcscope.hardware.firmware |
| [070](#item-070) | 物理磁盘健康 | pcscope.hardware.disk-health |
| [071](#item-071) | 电池状态 | pcscope.hardware.battery |
| [072](#item-072) | 电源计划与休眠配置 | pcscope.power.configuration |
| [073](#item-073) | 服务详细配置 | pcscope.startup.service-configuration |
| [074](#item-074) | 会话与登录初始化入口 | pcscope.startup.session-hooks |
| [075](#item-075) | IFEO、AppInit 与 AppCert 入口 | pcscope.startup.advanced-extensions |
| [076](#item-076) | WMI 永久订阅过滤器 | pcscope.startup.wmi-filters |
| [077](#item-077) | WMI 永久订阅消费者 | pcscope.startup.wmi-consumers |
| [078](#item-078) | WMI 过滤器与消费者绑定 | pcscope.startup.wmi-bindings |
| [079](#item-079) | 计划任务执行记录与条件 | pcscope.startup.task-details |
| [080](#item-080) | 环境变量与默认文件关联 | pcscope.environment.configuration |
| [081](#item-081) | 打印机清单 | pcscope.desktop.printing |
| [082](#item-082) | 打印驱动 | pcscope.desktop.printer-drivers |
| [083](#item-083) | 打印端口 | pcscope.desktop.printer-ports |
| [084](#item-084) | 打印作业队列 | pcscope.desktop.print-jobs |
| [085](#item-085) | 键盘、输入法与过滤驱动配置 | pcscope.desktop.input |
| [086](#item-086) | 剪贴板设置 | pcscope.desktop.clipboard |
| [087](#item-087) | 桌面与资源管理器设置 | pcscope.desktop.shell-settings |
| [088](#item-088) | 桌面与开始菜单快捷方式 | pcscope.desktop.shortcuts |
| [089](#item-089) | Hyper-V 虚拟机 | pcscope.virtualization.hyperv-vms |
| [090](#item-090) | Hyper-V 虚拟交换机 | pcscope.virtualization.hyperv-switches |
| [091](#item-091) | 虚拟机磁盘连接 | pcscope.virtualization.hyperv-disks |
| [092](#item-092) | 虚拟机检查点 | pcscope.virtualization.hyperv-checkpoints |
| [093](#item-093) | WSL 发行版登记 | pcscope.virtualization.wsl-registration |
| [094](#item-094) | Chrome、Edge 与 Firefox 策略 | pcscope.apps.browser-policies |
| [095](#item-095) | VS Code 扩展清单 | pcscope.apps.ide-extensions |
| [096](#item-096) | .NET 运行时登记 | pcscope.apps.dotnet-runtimes |
| [097](#item-097) | 全局 npm 包清单 | pcscope.apps.npm-global-packages |
| [098](#item-098) | OneDrive 账户与同步登记 | pcscope.apps.onedrive-registration |
| [099](#item-099) | Chrome / Edge 扩展 | pcscope.apps.chromium-extensions |
| [100](#item-100) | Firefox 扩展 | pcscope.apps.firefox-extensions |
| [101](#item-101) | 指定范围配置 | pcscope.collector.scoped-configuration |
| [102](#item-102) | 指定文件、ACL 与 ADS | pcscope.files.metadata-security |
| [103](#item-103) | 便携软件文件登记 | pcscope.software.portable |
| [104](#item-104) | 指定进程加载模块 | pcscope.execution.modules |
| [105](#item-105) | Mihomo 本地运行配置 | pcscope.apps.mihomo |
| [106](#item-106) | Docker 容器、镜像、卷与网络 | pcscope.virtualization.docker |
| [107](#item-107) | 固定 NTFS 卷发现 | pcscope.usn.volumes |
| [108](#item-108) | 系统与服务事件 / 服务运行事件 | pcscope.eventlog.system |
| [109](#item-109) | 系统与服务事件 / 系统启动与关闭 | pcscope.eventlog.system |
| [110](#item-110) | 系统与服务事件 / 电源与异常重启 | pcscope.eventlog.system |
| [111](#item-111) | 系统与服务事件 / 系统注册表维护 | pcscope.eventlog.system |
| [112](#item-112) | 系统与服务事件 / 系统时间同步 | pcscope.eventlog.system |
| [113](#item-113) | 系统与服务事件 / 来源采集状态 | pcscope.eventlog.system |
| [114](#item-114) | 系统与服务事件 / 其他事件 | pcscope.eventlog.system |
| [115](#item-115) | 应用程序事件 / 软件安装事件 | pcscope.eventlog.application |
| [116](#item-116) | 应用程序事件 / 应用程序错误 | pcscope.eventlog.application |
| [117](#item-117) | 应用程序事件 / 应用程序无响应 | pcscope.eventlog.application |
| [118](#item-118) | 应用程序事件 / NVIDIA WMI 服务状态 | pcscope.eventlog.application |
| [119](#item-119) | 应用程序事件 / 卷影复制服务状态 | pcscope.eventlog.application |
| [120](#item-120) | 应用程序事件 / Windows 错误报告 | pcscope.eventlog.application |
| [121](#item-121) | 应用程序事件 / 软件保护与授权维护 | pcscope.eventlog.application |
| [122](#item-122) | 应用程序事件 / 存储优化作业 | pcscope.eventlog.application |
| [123](#item-123) | 应用程序事件 / 来源采集状态 | pcscope.eventlog.application |
| [124](#item-124) | 应用程序事件 / 其他事件 | pcscope.eventlog.application |
| [125](#item-125) | Windows 安全审计 / 登录、登出与会话 | pcscope.eventlog.security |
| [126](#item-126) | Windows 安全审计 / 账户管理 | pcscope.eventlog.security |
| [127](#item-127) | Windows 安全审计 / 用户组成员变更 | pcscope.eventlog.security |
| [128](#item-128) | Windows 安全审计 / 特殊权限分配 | pcscope.eventlog.security |
| [129](#item-129) | Windows 安全审计 / 进程生命周期 | pcscope.eventlog.security |
| [130](#item-130) | Windows 安全审计 / 文件访问与权限 | pcscope.eventlog.security |
| [131](#item-131) | Windows 安全审计 / 注册表操作 | pcscope.eventlog.security |
| [132](#item-132) | Windows 安全审计 / 服务安装 | pcscope.eventlog.security |
| [133](#item-133) | Windows 安全审计 / 计划任务变更 | pcscope.eventlog.security |
| [134](#item-134) | Windows 安全审计 / 审计策略变更 | pcscope.eventlog.security |
| [135](#item-135) | Windows 安全审计 / 网络连接与共享访问 | pcscope.eventlog.security |
| [136](#item-136) | Windows 安全审计 / 审计日志清除 | pcscope.eventlog.security |
| [137](#item-137) | Windows 安全审计 / 加密密钥操作 | pcscope.eventlog.security |
| [138](#item-138) | Windows 安全审计 / 密码组件自检与验证 | pcscope.eventlog.security |
| [139](#item-139) | Windows 安全审计 / 密码提供程序与上下文配置 | pcscope.eventlog.security |
| [140](#item-140) | Windows 安全审计 / DPAPI 数据保护与主密钥 | pcscope.eventlog.security |
| [141](#item-141) | Windows 安全审计 / 凭据读取与账户检查 | pcscope.eventlog.security |
| [142](#item-142) | Windows 安全审计 / 用户组成员查询 | pcscope.eventlog.security |
| [143](#item-143) | Windows 安全审计 / WMI 与目录对象访问审计 | pcscope.eventlog.security |
| [144](#item-144) | Windows 安全审计 / 进程访问令牌分配 | pcscope.eventlog.security |
| [145](#item-145) | Windows 安全审计 / 系统启动、时间与审计配置 | pcscope.eventlog.security |
| [146](#item-146) | Windows 安全审计 / 防火墙组件生命周期 | pcscope.eventlog.security |
| [147](#item-147) | Windows 安全审计 / 审计日志服务 | pcscope.eventlog.security |
| [148](#item-148) | Windows 安全审计 / 来源采集状态 | pcscope.eventlog.security |
| [149](#item-149) | Windows 安全审计 / 其他事件 | pcscope.eventlog.security |
| [150](#item-150) | 计划任务执行事件 / 任务触发与执行过程 | pcscope.eventlog.microsoft-windows-taskscheduler/operational |
| [151](#item-151) | 计划任务执行事件 / 任务登记与配置变更 | pcscope.eventlog.microsoft-windows-taskscheduler/operational |
| [152](#item-152) | 计划任务执行事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-taskscheduler/operational |
| [153](#item-153) | 计划任务执行事件 / 其他事件 | pcscope.eventlog.microsoft-windows-taskscheduler/operational |
| [154](#item-154) | Windows 更新事件 / 更新检查 | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational |
| [155](#item-155) | Windows 更新事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational |
| [156](#item-156) | Windows 更新事件 / 其他事件 | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational |
| [157](#item-157) | Defender 防护事件 / Defender 健康与情报 | pcscope.eventlog.microsoft-windows-windows defender/operational |
| [158](#item-158) | Defender 防护事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-windows defender/operational |
| [159](#item-159) | Defender 防护事件 / 其他事件 | pcscope.eventlog.microsoft-windows-windows defender/operational |
| [160](#item-160) | PowerShell 运行事件 / PowerShell 命令与脚本 | pcscope.eventlog.microsoft-windows-powershell/operational |
| [161](#item-161) | PowerShell 运行事件 / PowerShell 宿主生命周期 | pcscope.eventlog.microsoft-windows-powershell/operational |
| [162](#item-162) | PowerShell 运行事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-powershell/operational |
| [163](#item-163) | PowerShell 运行事件 / 其他事件 | pcscope.eventlog.microsoft-windows-powershell/operational |
| [164](#item-164) | WMI 活动事件 / WMI 提供程序与调用 | pcscope.eventlog.microsoft-windows-wmi-activity/operational |
| [165](#item-165) | WMI 活动事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-wmi-activity/operational |
| [166](#item-166) | WMI 活动事件 / 其他事件 | pcscope.eventlog.microsoft-windows-wmi-activity/operational |
| [167](#item-167) | 组策略处理事件 / 组策略服务生命周期 | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| [168](#item-168) | 组策略处理事件 / 策略处理开始与完成 | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| [169](#item-169) | 组策略处理事件 / 策略会话与登录通知 | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| [170](#item-170) | 组策略处理事件 / 策略获取、适用与排除 | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| [171](#item-171) | 组策略处理事件 / 处理模式与内部诊断 | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| [172](#item-172) | 组策略处理事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| [173](#item-173) | 组策略处理事件 / 其他事件 | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| [174](#item-174) | AppX 部署事件 / 应用包部署 | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational |
| [175](#item-175) | AppX 部署事件 / 应用包部署服务状态 | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational |
| [176](#item-176) | AppX 部署事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational |
| [177](#item-177) | AppX 部署事件 / 其他事件 | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational |
| [178](#item-178) | 终端会话登录、断开与注销 / 来源采集状态 | pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational |
| [179](#item-179) | 终端会话登录、断开与注销 / 运行事件 | pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational |
| [180](#item-180) | 远程连接管理事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational |
| [181](#item-181) | 远程连接管理事件 / 运行事件 | pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational |
| [182](#item-182) | 无线网络连接事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational |
| [183](#item-183) | 无线网络连接事件 / 运行事件 | pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational |
| [184](#item-184) | 网络配置文件事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-networkprofile/operational |
| [185](#item-185) | 网络配置文件事件 / 运行事件 | pcscope.eventlog.microsoft-windows-networkprofile/operational |
| [186](#item-186) | 用户模式驱动事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational |
| [187](#item-187) | 用户模式驱动事件 / 运行事件 | pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational |
| [188](#item-188) | 打印服务事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-printservice/operational |
| [189](#item-189) | 打印服务事件 / 运行事件 | pcscope.eventlog.microsoft-windows-printservice/operational |
| [190](#item-190) | BitLocker 管理事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management |
| [191](#item-191) | BitLocker 管理事件 / 运行事件 | pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management |
| [192](#item-192) | 代码完整性事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-codeintegrity/operational |
| [193](#item-193) | 代码完整性事件 / 运行事件 | pcscope.eventlog.microsoft-windows-codeintegrity/operational |
| [194](#item-194) | AppLocker 程序与 DLL 事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-applocker/exe and dll |
| [195](#item-195) | AppLocker 程序与 DLL 事件 / 运行事件 | pcscope.eventlog.microsoft-windows-applocker/exe and dll |
| [196](#item-196) | AppLocker 安装包与脚本事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-applocker/msi and script |
| [197](#item-197) | AppLocker 安装包与脚本事件 / 运行事件 | pcscope.eventlog.microsoft-windows-applocker/msi and script |
| [198](#item-198) | Sysmon / 进程生命周期 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [199](#item-199) | Sysmon / 进程访问与异常活动 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [200](#item-200) | Sysmon / 网络连接 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [201](#item-201) | Sysmon / DNS 查询 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [202](#item-202) | Sysmon / 文件操作 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [203](#item-203) | Sysmon / 注册表操作 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [204](#item-204) | Sysmon / 模块与驱动加载 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [205](#item-205) | Sysmon / WMI 持久订阅 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [206](#item-206) | Sysmon / 命名管道 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [207](#item-207) | Sysmon / 原始磁盘读取 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [208](#item-208) | Sysmon / 剪贴板活动 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [209](#item-209) | Sysmon / Sysmon 服务与配置 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [210](#item-210) | Sysmon / 来源采集状态 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [211](#item-211) | Sysmon / 其他事件 | pcscope.eventlog.microsoft-windows-sysmon/operational |
| [212](#item-212) | Hyper-V 管理事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin |
| [213](#item-213) | Hyper-V 管理事件 / 运行事件 | pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin |
| [214](#item-214) | Windows 备份事件 / 来源采集状态 | pcscope.eventlog.microsoft-windows-backup |
| [215](#item-215) | Windows 备份事件 / 运行事件 | pcscope.eventlog.microsoft-windows-backup |
| [216](#item-216) | 文件变更记录 | pcscope.usn. |
| [217](#item-217) | 手动导入的事件日志 | pcscope.eventlog.import. |
| [218](#item-218) | 资源管理器扩展（未细分／来源状态） | pcscope.shell.registrations |
| [219](#item-219) | 登录启动项（未细分／来源状态） | pcscope.startup.logon |

<a id="item-001"></a>
### 001 本机软件安装登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.software |
| source | pcscope.software |
| 分类 | software |
| 插件目录分组 | pcscope-software.exe |
| 渠道／接口 | 注册表 · Rust winreg::RegKey；HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall |
| 采集方式（目录原文） | 分别枚举 32/64 位视图，读取键内全部登记值与类型（敏感值脱敏）；完整快照对比 |

**实现位置**：[软件采集器](../plugins/software/src/main.rs)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| DisplayName | 显示名称 | 显示名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| DisplayVersion | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| InstallDate | 登记安装日期 | 登记安装日期 | "20260911" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| InstallLocation | 安装位置 | 安装位置 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| Publisher | 发布者 | 发布者 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.scope | 采集范围 | 采集范围 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| registry_view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.software",
  "id": "example-object",
  "item_name": "本机软件安装登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "values": {
      "ExampleValue": {
        "kind": "REG_SZ",
        "value": "示例登记值"
      },
      "DisplayName": {
        "kind": "REG_SZ",
        "value": "Example Application"
      }
    },
    "DisplayName": "Example Application",
    "DisplayVersion": "1.0.0",
    "InstallDate": "20260911",
    "InstallLocation": "C:\\Example\\item",
    "Publisher": "示例值",
    "registry_path": "SOFTWARE\\Example",
    "registry_view": "Registry64"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example Application |
| 属性摘要 | 版本：1.0.0 · 发布者：示例值 · ExampleValue：示例登记值 |
| 默认属性组 | 软件登记：显示名称、版本、发布者、登记安装日期、安装位置 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

<a id="item-002"></a>
### 002 操作系统版本与安装时间

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.system.version |
| source | pcscope.system.version |
| 分类 | system |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | CIM · root/cimv2: Win32_OperatingSystem |
| 采集方式（目录原文） | Get-CimInstance 查询 Caption、Version、BuildNumber、OSArchitecture、InstallDate 等；状态快照 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:26](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| architecture | 架构 | 架构 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| build | 系统内部版本 | 系统内部版本 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| caption | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| computer_name | 计算机名称 | 计算机名称 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| install_time | 系统安装时间 | 系统安装时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| install_time_basis | 安装时间依据 | 安装时间依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.system.version",
  "id": "example-object",
  "item_name": "操作系统版本与安装时间",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "architecture": "示例值",
    "build": "示例值",
    "caption": "Example",
    "computer_name": "示例值",
    "install_time": "2026-09-11T09:00:00Z",
    "install_time_basis": "示例值",
    "version": "1.0.0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 版本：1.0.0 · 架构：示例值 · 系统内部版本：示例值 |
| 默认属性组 | 设备与系统状态：名称、版本 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.system.version`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-003"></a>
### 003 最近启动时间与时区

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.system.boot-state |
| source | pcscope.system.boot-state |
| 分类 | system |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | CIM · Win32_OperatingSystem.LastBootUpTime；PowerShell Get-TimeZone |
| 采集方式（目录原文） | 读取系统启动时间与时区 ID；状态快照 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:30](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| last_boot_time | 最近启动时间 | 最近启动时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| timezone | 时区 | 时区 | "China Standard Time" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.system.boot-state",
  "id": "example-object",
  "item_name": "最近启动时间与时区",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "last_boot_time": "2026-09-11T09:00:00Z",
    "timezone": "China Standard Time"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 最近启动时间：2026-09-11 17:00:00（本机时间） · 时区：China Standard Time |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.system.boot-state`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-004"></a>
### 004 本机用户账户

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.identity.accounts |
| source | pcscope.identity.accounts |
| 分类 | identity |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | PowerShell · Get-LocalUser |
| 采集方式（目录原文） | 枚举账户、SID、启用状态、最近登录及密码策略；来源不提供建立时间 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:34](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| account_expires | 账户到期时间 | 账户到期时间 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| created_at | 建立时间 | 建立时间 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| created_at_basis | 建立时间依据 | 建立时间依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| enabled | 是否启用 | 是否启用 | true | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| last_logon | 最近登录时间 | 最近登录时间 | "2026-09-11T09:00:00Z" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| password_last_set | 密码最近设置时间 | 密码最近设置时间 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| password_required | 是否要求密码 | 是否要求密码 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| principal_source | 账户来源 | 账户来源 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| user_may_change_password | 用户能否修改密码 | 用户能否修改密码 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.identity.accounts",
  "id": "example-object",
  "item_name": "本机用户账户",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "account_expires": "示例值",
    "created_at": "示例值",
    "created_at_basis": "示例值",
    "description": "示例值",
    "enabled": true,
    "last_logon": "2026-09-11T09:00:00Z",
    "name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 是否启用：是 · 最近登录时间：2026-09-11 17:00:00（本机时间） · 账户到期时间：示例值 |
| 默认属性组 | 身份与权限：名称、是否启用、最近登录时间 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.identity.accounts`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-005"></a>
### 005 本机用户组与成员

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.identity.groups |
| source | pcscope.identity.groups |
| 分类 | identity |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | PowerShell · Get-LocalGroup / Get-LocalGroupMember -SID |
| 采集方式（目录原文） | 按组 SID 枚举成员 SID；快照对比，不计算全部有效权限 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:37](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| members | 组成员 | 组成员 | ["示例成员"] | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.identity.groups",
  "id": "example-object",
  "item_name": "本机用户组与成员",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "description": "示例值",
    "members": [
      "示例成员"
    ],
    "name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 说明：示例值 |
| 默认属性组 | 身份与权限：名称、组成员 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.identity.groups`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-006"></a>
### 006 服务运行状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.services |
| source | pcscope.startup.services |
| 分类 | services |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | CIM · root/cimv2: Win32_Service |
| 采集方式（目录原文） | 查询名称、状态、启动模式、运行账户和服务说明；非服务事件订阅 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:40](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| command | 启动命令 | 启动命令 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| display_name | 显示名称 | 显示名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| exit_code | 进程或服务退出码，保留来源数值 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| pid | 进程编号 | 进程编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| service_specific_exit_code | 服务自行定义的退出码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| service_type | 服务类型 | 服务类型 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| start_account | 运行账户 | 运行账户 | 1024 | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| start_mode | 启动方式 | 启动方式 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| state | 状态 | 状态 | "示例状态" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.services",
  "id": "example-object",
  "item_name": "服务运行状态",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "command": "示例值",
    "description": "示例值",
    "display_name": "Example",
    "exit_code": "示例值",
    "name": "Example",
    "pid": "示例值",
    "service_specific_exit_code": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 启动命令：示例值 · 说明：示例值 · exit_code：示例值 |
| 默认属性组 | 运行与启动配置：名称、显示名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.services`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-007"></a>
### 007 计划任务基础配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.tasks |
| source | pcscope.startup.tasks |
| 分类 | startup |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | PowerShell · Get-ScheduledTask |
| 采集方式（目录原文） | 枚举任务、Principal、Settings 与 Actions；保存动作全部 CIM 属性及脱敏参数 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:43](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| actions | 执行操作 | 执行操作 | [{"Execute":"C:\\Example\\app.exe"}] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| enabled | 是否启用 | 是否启用 | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| run_level | 运行权限级别 | 运行权限级别 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| state | 状态 | 状态 | "示例状态" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| user_id | 运行用户 | 运行用户 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.tasks",
  "id": "example-object",
  "item_name": "计划任务基础配置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "actions": [
      {
        "Execute": "C:\\Example\\app.exe"
      }
    ],
    "enabled": true,
    "name": "Example",
    "path": "C:\\Example\\item",
    "run_level": "示例值",
    "state": "示例状态",
    "user_id": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 状态：示例状态 · 是否启用：是 · 运行权限级别：示例值 |
| 默认属性组 | 运行与启动配置：名称、状态、路径、运行用户、运行权限级别 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.tasks`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-008"></a>
### 008 卷与文件系统

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.storage.volumes |
| source | pcscope.storage.volumes |
| 分类 | storage |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | CIM · root/cimv2: Win32_Volume |
| 采集方式（目录原文） | 枚举卷 ID、盘符、文件系统、容量和卷类型；状态快照 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:46](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| capacity | 容量（字节） | 容量（字节） | 1024 | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| device_id | 设备标识 | 设备标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| drive_letter | 盘符 | 盘符 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| drive_type | 磁盘类型代码 | 磁盘类型代码 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| file_system | 文件系统名称 | 文件系统 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| label | 卷标 | 卷标 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.storage.volumes",
  "id": "example-object",
  "item_name": "卷与文件系统",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "capacity": 1024,
    "device_id": "示例值",
    "drive_letter": "示例值",
    "drive_type": "示例值",
    "file_system": "示例值",
    "label": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | 盘符：示例值 · 文件系统：示例值 · 容量（字节）：1024 |
| 默认属性组 | 设备与系统状态：容量（字节）、文件系统 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.storage.volumes`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-009"></a>
### 009 网络适配器

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.adapters |
| source | pcscope.network.adapters |
| 分类 | network |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | PowerShell · Get-NetAdapter -IncludeHidden |
| 采集方式（目录原文） | 包括隐藏网卡，读取 GUID、状态、MAC、速度与驱动版本 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:49](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| driver_version | 驱动版本 | 驱动版本 | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| link_speed | 连接速度 | 连接速度 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| mac_address | MAC 地址 | MAC 地址 | "192.0.2.10" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| status | 状态 | 状态 | "示例状态" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.adapters",
  "id": "example-object",
  "item_name": "网络适配器",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "description": "示例值",
    "driver_version": "1.0.0",
    "link_speed": "示例值",
    "mac_address": "192.0.2.10",
    "name": "Example",
    "status": "示例状态"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 状态：示例状态 · 说明：示例值 · 驱动版本：1.0.0 |
| 默认属性组 | 网络配置：名称、状态、MAC 地址、连接速度 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.adapters`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-010"></a>
### 010 IP 地址

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.addresses |
| source | pcscope.network.addresses |
| 分类 | network |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | PowerShell · Get-NetIPAddress |
| 采集方式（目录原文） | 枚举接口地址、前缀长度、地址族和地址状态；状态快照 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:52](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| address | 地址 | 地址 | "192.0.2.10" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| address_family | 地址族 | 地址族 | "192.0.2.10" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| address_state | 地址状态 | 地址状态 | "192.0.2.10" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| interface_alias | 接口名称 | 接口名称 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| interface_index | 接口编号 | 接口编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| prefix_length | 网络前缀长度 | 网络前缀长度 | 1024 | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| prefix_origin | 地址前缀来源 | 地址前缀来源 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| suffix_origin | 地址后缀来源 | 地址后缀来源 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.addresses",
  "id": "example-object",
  "item_name": "IP 地址",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "address": "192.0.2.10",
    "address_family": "192.0.2.10",
    "address_state": "192.0.2.10",
    "interface_alias": "示例值",
    "interface_index": "示例值",
    "prefix_length": 1024,
    "prefix_origin": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | 地址：192.0.2.10 · 地址族：192.0.2.10 · 地址状态：192.0.2.10 |
| 默认属性组 | 网络配置：接口名称、地址、网络前缀长度 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.addresses`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-011"></a>
### 011 虚拟化能力与平台

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.virtualization.platform |
| source | pcscope.virtualization.platform |
| 分类 | virtualization |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | CIM · Win32_ComputerSystem / Win32_Processor |
| 采集方式（目录原文） | 查询 HypervisorPresent、固件虚拟化、监控扩展与 SLAT 能力；不枚举运行虚拟机 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:55](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| firmware_virtualization | 固件是否启用虚拟化 | 固件是否启用虚拟化 | true | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| hypervisor_present | 是否检测到虚拟机监控程序 | 是否检测到虚拟机监控程序 | true | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| manufacturer | 制造商 | 制造商 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| model | 型号 | 型号 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| slat | 是否支持二级地址转换 | 是否支持二级地址转换 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| vm_monitor_extensions | 是否支持虚拟机监控扩展 | 是否支持虚拟机监控扩展 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.virtualization.platform",
  "id": "example-object",
  "item_name": "虚拟化能力与平台",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "firmware_virtualization": true,
    "hypervisor_present": true,
    "manufacturer": "示例值",
    "model": "示例值",
    "name": "Example",
    "slat": "示例值",
    "vm_monitor_extensions": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 制造商：示例值 · 型号：示例值 · 固件是否启用虚拟化：是 |
| 默认属性组 | 设备与系统状态：名称、制造商、型号、是否检测到虚拟机监控程序、固件是否启用虚拟化 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.virtualization.platform`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-012"></a>
### 012 即插即用设备

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.hardware.devices |
| source | pcscope.hardware.devices |
| 分类 | hardware |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | CIM · root/cimv2: Win32_PnPEntity |
| 采集方式（目录原文） | 枚举设备 ID、类别、制造商、服务、状态与配置错误码 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:60](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| class | 设备类别 | 设备类别 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| class_guid | 类别标识 | 类别标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| error_code | 错误代码 | 错误代码 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| manufacturer | 制造商 | 制造商 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| service | 关联服务 | 关联服务 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| status | 状态 | 状态 | "示例状态" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.hardware.devices",
  "id": "example-object",
  "item_name": "即插即用设备",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "class": "示例值",
    "class_guid": "示例值",
    "error_code": "示例值",
    "manufacturer": "示例值",
    "name": "Example",
    "service": "示例值",
    "status": "示例状态"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 状态：示例状态 · 制造商：示例值 · 设备类别：示例值 |
| 默认属性组 | 设备与系统状态：名称、状态、制造商 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.hardware.devices`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-013"></a>
### 013 QFE 更新登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.system.hotfixes |
| source | pcscope.system.hotfixes |
| 分类 | system |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | CIM · root/cimv2: Win32_QuickFixEngineering |
| 采集方式（目录原文） | 枚举 HotFixID、InstalledOn 等；仅 QFE 范围，不等于完整更新历史 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:63](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| coverage | 采集覆盖说明 | 采集覆盖说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hotfix_id | 更新编号 | 更新编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| installed_on | 登记安装时间 | 登记安装时间 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.system.hotfixes",
  "id": "example-object",
  "item_name": "QFE 更新登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "coverage": "示例值",
    "description": "示例值",
    "hotfix_id": "示例值",
    "installed_on": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | 采集覆盖说明：示例值 · 说明：示例值 · 更新编号：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.system.hotfixes`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-014"></a>
### 014 第三方预览处理程序

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.shell.registrations.preview |
| source | pcscope.shell.registrations |
| 分类 | shell |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · Microsoft.Win32.RegistryKey；HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\PreviewHandlers |
| 采集方式（目录原文） | 只读扫描 32/64 位视图；提取 CLSID 并关联 COM 注册 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:151](../plugins/powershell-adapter/collect-system.ps1)、[plugins/powershell-adapter/collect-system.ps1:154](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.company | 公司 | 公司 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.embedded_signer | 文件嵌入签名的主题 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.error_type | 读取失败的异常类型 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.file_version | 文件版本 | 文件版本 | "1.0.0" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.length | 大小（字节） | 大小（字节） | 1024 | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.path | 路径 | 路径 | "C:\\Example\\item" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.signature_status | 签名检查结果 | 签名检查结果 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.status | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.shell.registrations",
  "id": "example\\previewhandlersexample",
  "item_name": "第三方预览处理程序",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "registry_path": "example\\previewhandlersexample",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example / previewhandlersexample |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：注册表路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.shell.registrations`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-015"></a>
### 015 图标覆盖处理程序

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.shell.registrations.overlay |
| source | pcscope.shell.registrations |
| 分类 | shell |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\ShellIconOverlayIdentifiers |
| 采集方式（目录原文） | 只读递归登记项；表示注册关系，不证明加载或调用 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:151](../plugins/powershell-adapter/collect-system.ps1)、[plugins/powershell-adapter/collect-system.ps1:154](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.company | 公司 | 公司 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.embedded_signer | 文件嵌入签名的主题 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.error_type | 读取失败的异常类型 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.file_version | 文件版本 | 文件版本 | "1.0.0" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.length | 大小（字节） | 大小（字节） | 1024 | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.path | 路径 | 路径 | "C:\\Example\\item" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.signature_status | 签名检查结果 | 签名检查结果 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.status | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.shell.registrations",
  "id": "example\\shelliconoverlayidentifiersexample",
  "item_name": "图标覆盖处理程序",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "registry_path": "example\\shelliconoverlayidentifiersexample",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example / shelliconoverlayidentifiersexample |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：注册表路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.shell.registrations`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-016"></a>
### 016 Shell 扩展允许与阻止列表

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.shell.registrations.approval |
| source | pcscope.shell.registrations |
| 分类 | shell |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved、Blocked |
| 采集方式（目录原文） | 读取 32/64 位视图中的登记值 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:151](../plugins/powershell-adapter/collect-system.ps1)、[plugins/powershell-adapter/collect-system.ps1:154](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.company | 公司 | 公司 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.embedded_signer | 文件嵌入签名的主题 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.error_type | 读取失败的异常类型 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.file_version | 文件版本 | 文件版本 | "1.0.0" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.length | 大小（字节） | 大小（字节） | 1024 | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.path | 路径 | 路径 | "C:\\Example\\item" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.signature_status | 签名检查结果 | 签名检查结果 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.status | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.shell.registrations",
  "id": "example\\shell extensions\\approvedexample",
  "item_name": "Shell 扩展允许与阻止列表",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "registry_path": "example\\shell extensions\\approvedexample",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | shell extensions / approvedexample |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：注册表路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.shell.registrations`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-017"></a>
### 017 桌面与此电脑命名空间

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.shell.registrations.namespace |
| source | pcscope.shell.registrations |
| 分类 | shell |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace、MyComputer\NameSpace |
| 采集方式（目录原文） | 递归读取命名空间及关联 CLSID |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:151](../plugins/powershell-adapter/collect-system.ps1)、[plugins/powershell-adapter/collect-system.ps1:154](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.company | 公司 | 公司 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.embedded_signer | 文件嵌入签名的主题 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.error_type | 读取失败的异常类型 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.file_version | 文件版本 | 文件版本 | "1.0.0" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.length | 大小（字节） | 大小（字节） | 1024 | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.path | 路径 | 路径 | "C:\\Example\\item" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.signature_status | 签名检查结果 | 签名检查结果 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.status | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.shell.registrations",
  "id": "example\\desktop\\namespaceexample",
  "item_name": "桌面与此电脑命名空间",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "registry_path": "example\\desktop\\namespaceexample",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | desktop / namespaceexample |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：注册表路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.shell.registrations`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-018"></a>
### 018 右键菜单、文件关联与默认图标

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.shell.registrations.file-association |
| source | pcscope.shell.registrations |
| 分类 | shell |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · HKLM/HKCU\Software\Classes\{类型}\shell、shellex、DefaultIcon；Directory\Background\shell、shellex |
| 采集方式（目录原文） | 逐类型枚举，递归最多 5 层；扩展名根读取默认关联；命令字段保存脱敏原值 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:151](../plugins/powershell-adapter/collect-system.ps1)、[plugins/powershell-adapter/collect-system.ps1:154](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.company | 公司 | 公司 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.embedded_signer | 文件嵌入签名的主题 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.error_type | 读取失败的异常类型 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.file_version | 文件版本 | 文件版本 | "1.0.0" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.length | 大小（字节） | 大小（字节） | 1024 | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.path | 路径 | 路径 | "C:\\Example\\item" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.signature_status | 签名检查结果 | 签名检查结果 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.status | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.shell.registrations",
  "id": "example\\classes\\example",
  "item_name": "右键菜单、文件关联与默认图标",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "registry_path": "example\\classes\\example",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | classes / example |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：注册表路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.shell.registrations`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-019"></a>
### 019 COM 实现与扩展文件元数据

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.shell.registrations.com |
| source | pcscope.shell.registrations |
| 分类 | shell |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · Software\Classes\CLSID\{已发现CLSID}\InprocServer32、LocalServer32；.NET FileInfo / FileVersionInfo / SHA256 |
| 采集方式（目录原文） | 关联已发现的 CLSID；本地 DLL 读取版本、大小、写入时间与哈希；不执行 DLL、不验证签名 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:151](../plugins/powershell-adapter/collect-system.ps1)、[plugins/powershell-adapter/collect-system.ps1:154](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.company | 公司 | 公司 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.embedded_signer | 文件嵌入签名的主题 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.error_type | 读取失败的异常类型 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.file_version | 文件版本 | 文件版本 | "1.0.0" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.length | 大小（字节） | 大小（字节） | 1024 | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.path | 路径 | 路径 | "C:\\Example\\item" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.signature_status | 签名检查结果 | 签名检查结果 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.status | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.shell.registrations",
  "id": "example\\classes\\clsid\\example",
  "item_name": "COM 实现与扩展文件元数据",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "registry_path": "example\\classes\\clsid\\example",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | clsid / example |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：注册表路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.shell.registrations`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-020"></a>
### 020 登录启动登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.logon.registry |
| source | pcscope.startup.logon |
| 分类 | startup |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Run、RunOnce、Explorer\StartupApproved |
| 采集方式（目录原文） | 枚举 32/64 位视图；Run 命令保存脱敏原值，StartupApproved 保存登记信息 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:159](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| command | 启动命令 | 启动命令 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| command_collected | 是否取得运行命令 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| length | 大小（字节） | 大小（字节） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| policy | 采集策略版本 | 采集策略版本 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| shortcut_target_collected | 是否解析快捷方式目标 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.logon",
  "id": "example/run/example",
  "item_name": "登录启动登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    },
    "command": "示例值",
    "command_collected": false,
    "last_write_utc": "2026-09-11T09:00:00Z",
    "length": 1024,
    "name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | ExampleValue：示例登记值 · 启动命令：示例值 · 最近写入时间：2026-09-11 17:00:00（本机时间） |
| 默认属性组 | 运行与启动配置：名称、路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.logon`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-021"></a>
### 021 用户与公共启动文件夹

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.logon.folders |
| source | pcscope.startup.logon |
| 分类 | startup |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 文件系统 · Environment.GetFolderPath(Startup / CommonStartup)；Get-ChildItem / SHA256 |
| 采集方式（目录原文） | 枚举文件并读取大小、时间与摘要；不解析快捷方式目标 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:159](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| command | 启动命令 | 启动命令 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| command_collected | 是否取得运行命令 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| length | 大小（字节） | 大小（字节） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| policy | 采集策略版本 | 采集策略版本 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| shortcut_target_collected | 是否解析快捷方式目标 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.logon",
  "id": "C:\\Example\\Startup\\Example.lnk",
  "item_name": "用户与公共启动文件夹",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    },
    "command": "示例值",
    "command_collected": false,
    "last_write_utc": "2026-09-11T09:00:00Z",
    "length": 1024,
    "name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | ExampleValue：示例登记值 · 启动命令：示例值 · 最近写入时间：2026-09-11 17:00:00（本机时间） |
| 默认属性组 | 运行与启动配置：名称、路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.logon`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-022"></a>
### 022 计算机身份与域信息

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.identity.machine |
| source | pcscope.identity.machine |
| 分类 | identity |
| 插件目录分组 | system |
| 渠道／接口 | CIM · root/cimv2: Win32_ComputerSystem / Win32_ComputerSystemProduct |
| 采集方式（目录原文） | 查询名称、域、角色、制造商、型号及 UUID；状态快照 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:1](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Domain | 计算机所属域、工作组或账户域；按来源区分 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DomainRole | 计算机域角色代码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Manufacturer | 制造商 | 制造商 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Model | 型号 | 型号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| PartOfDomain | 是否加入域 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SystemType | 系统类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| UUID | 系统产品 UUID | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Vendor | 厂商 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Version | 版本 | 版本 | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.identity.machine",
  "id": "example-object",
  "item_name": "计算机身份与域信息",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "Domain": "示例值",
    "DomainRole": "示例值",
    "Manufacturer": "示例值",
    "Model": "示例值",
    "Name": "Example",
    "PartOfDomain": false,
    "SystemType": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 制造商：示例值 · 型号：示例值 · Domain：示例值 |
| 默认属性组 | 身份与权限：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.identity.machine`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-023"></a>
### 023 时间同步配置与时区

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.system.clock |
| source | pcscope.system.clock |
| 分类 | system |
| 插件目录分组 | system |
| 渠道／接口 | 注册表 · HKLM\SYSTEM\CurrentControlSet\Services\W32Time；Get-TimeZone |
| 采集方式（目录原文） | 递归配置 3 层；读取键内值并脱敏，标注限定深度外的子键数量 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:2](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| id | 标识 | 标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.system.clock",
  "id": "example-object",
  "item_name": "时间同步配置与时区",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    },
    "id": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 · 标识：示例值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.system.clock`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-024"></a>
### 024 Windows Update 历史

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.system.update-history |
| source | pcscope.system.update-history |
| 分类 | system |
| 插件目录分组 | system |
| 渠道／接口 | COM · Microsoft.Update.Session → CreateUpdateSearcher → GetTotalHistoryCount / QueryHistory |
| 采集方式（目录原文） | 每批 128 条读取本机保留的更新历史；非在线检查更新，不安装更新 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:3](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| date | 来源登记日期或时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hresult | HRESULT 结果代码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| operation | 操作（来源值） | 操作（来源值） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| result | 来源结果代码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| revision | 修订号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| title | 标题 | 保留原名（无专用中文标签） | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| update_id | 更新标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.system.update-history",
  "id": "example-object",
  "item_name": "Windows Update 历史",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "date": "2026-09-11T09:00:00Z",
    "hresult": "示例值",
    "operation": "示例值",
    "result": "示例值",
    "revision": "示例值",
    "title": "Example",
    "update_id": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | date：2026-09-11 17:00:00（本机时间） · hresult：示例值 · 操作（来源值）：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.system.update-history`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-025"></a>
### 025 待重启标志

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.system.reboot-pending |
| source | pcscope.system.reboot-pending |
| 分类 | system |
| 插件目录分组 | system |
| 渠道／接口 | 注册表 · HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending；WindowsUpdate\Auto Update\RebootRequired；SYSTEM\CurrentControlSet\Control\Session Manager |
| 采集方式（目录原文） | 读取标志键与 PendingFileRenameOperations；状态快照 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:7](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.system.reboot-pending",
  "id": "example-object",
  "item_name": "待重启标志",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.system.reboot-pending`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-026"></a>
### 026 系统还原点

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.system.restore-points |
| source | pcscope.system.restore-points |
| 分类 | system |
| 插件目录分组 | system |
| 渠道／接口 | CIM · root/default: SystemRestore |
| 采集方式（目录原文） | 枚举序号、说明、建立时间与类型；不创建还原点 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:8](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| CreationTime | 来源提供的创建时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| EventType | 事件类型代码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| RestorePointType | 还原点类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SequenceNumber | 序列编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.system.restore-points",
  "id": "example-object",
  "item_name": "系统还原点",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "CreationTime": "2026-09-11T09:00:00Z",
    "Description": "示例值",
    "EventType": "示例值",
    "RestorePointType": "示例值",
    "SequenceNumber": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | CreationTime：2026-09-11 17:00:00（本机时间） · 说明：示例值 · EventType：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.system.restore-points`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-027"></a>
### 027 当前用户 AppX 包

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.software.appx-user |
| source | pcscope.software.appx-user |
| 分类 | software |
| 插件目录分组 | system |
| 渠道／接口 | PowerShell · Get-AppxPackage |
| 采集方式（目录原文） | 枚举当前用户包登记与版本、发布者、位置、状态 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:9](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Architecture | 架构 | 架构 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InstallLocation | 安装位置 | 安装位置 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| IsBundle | 是否为应用包捆绑包 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsFramework | 是否为框架包 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| NonRemovable | 是否标记不可移除 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PackageFamilyName | 包系列名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PackageFullName | 包完整名称 | 保留原名（无专用中文标签） | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| Publisher | 发布者 | 发布者 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| SignatureKind | 包签名类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Status | 状态 | 状态 | "0x0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.software.appx-user",
  "id": "example-object",
  "item_name": "当前用户 AppX 包",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "Architecture": "示例值",
    "InstallLocation": "C:\\Example\\item",
    "IsBundle": "示例值",
    "IsFramework": "示例值",
    "Name": "Example",
    "NonRemovable": "示例值",
    "PackageFamilyName": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 架构：示例值 · 安装位置：C:\Example\item · IsBundle：示例值 |
| 默认属性组 | 软件登记：安装位置 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.software.appx-user`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-028"></a>
### 028 预配 AppX 包

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.software.provisioned |
| source | pcscope.software.provisioned |
| 分类 | software |
| 插件目录分组 | system |
| 渠道／接口 | PowerShell · Get-AppxProvisionedPackage -Online |
| 采集方式（目录原文） | 读取本机映像预配包清单；需要相应权限，不执行安装 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:10](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Architecture | 架构 | 架构 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DisplayName | 显示名称 | 显示名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| InstallLocation | 安装位置 | 安装位置 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| PackageName | 包名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.software.provisioned",
  "id": "example-object",
  "item_name": "预配 AppX 包",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "Architecture": "示例值",
    "DisplayName": "Example",
    "InstallLocation": "C:\\Example\\item",
    "PackageName": "示例值",
    "Version": "1.0.0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 版本：1.0.0 · 架构：示例值 · 安装位置：C:\Example\item |
| 默认属性组 | 软件登记：显示名称、安装位置、版本 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.software.provisioned`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-029"></a>
### 029 Windows 可选组件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.software.optional-features |
| source | pcscope.software.optional-features |
| 分类 | software |
| 插件目录分组 | system |
| 渠道／接口 | PowerShell · Get-WindowsOptionalFeature -Online |
| 采集方式（目录原文） | 读取组件名称与状态；不启用或禁用组件 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:11](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| name | 名称 | 名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| state | 状态 | 状态 | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.software.optional-features",
  "id": "example-object",
  "item_name": "Windows 可选组件",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "name": "Example",
    "state": "示例状态"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 状态：示例状态 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.software.optional-features`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-030"></a>
### 030 当前用户软件登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.software.user-inventory |
| source | pcscope.software.user-inventory |
| 分类 | software |
| 插件目录分组 | system |
| 渠道／接口 | 注册表 · HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall |
| 采集方式（目录原文） | 只读枚举 32/64 位视图下一层子键，键内软件登记值保留类型与脱敏原值 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:12](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.software.user-inventory",
  "id": "example-object",
  "item_name": "当前用户软件登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.software.user-inventory`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-031"></a>
### 031 用户配置文件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.identity.profiles |
| source | pcscope.identity.profiles |
| 分类 | identity |
| 插件目录分组 | system |
| 渠道／接口 | CIM · root/cimv2: Win32_UserProfile |
| 采集方式（目录原文） | 读取 SID、路径、加载状态与 LastUseTime；状态快照 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:13](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| LastUseTime | 最近使用时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Loaded | 配置文件是否已加载 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LocalPath | 本地路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SID | 安全标识符 | 安全标识符 | "S-1-5-21-100-200-300-1001" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Special | 是否为系统特殊对象 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Status | 状态 | 状态 | "0x0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.identity.profiles",
  "id": "example-object",
  "item_name": "用户配置文件",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "LastUseTime": "2026-09-11T09:00:00Z",
    "Loaded": false,
    "LocalPath": "C:\\Example\\item",
    "SID": "S-1-5-21-100-200-300-1001",
    "Special": false,
    "Status": "0x0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 状态：0x0 · LastUseTime：2026-09-11 17:00:00（本机时间） · Loaded：否 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.identity.profiles`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-032"></a>
### 032 登录会话

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.identity.logon-sessions |
| source | pcscope.identity.logon-sessions |
| 分类 | identity |
| 插件目录分组 | system |
| 渠道／接口 | CIM · root/cimv2: Win32_LogonSession |
| 采集方式（目录原文） | 读取 LogonId、类型、开始时间、认证包；不是完整登录登出历史 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:14](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AuthenticationPackage | 认证包 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Caption | 名称 | 名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LogonId | 登录会话标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LogonType | 登录类型代码 | 保留原名（无专用中文标签） | "2" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| StartTime | 启动时间 | 启动时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.identity.logon-sessions",
  "id": "example-object",
  "item_name": "登录会话",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AuthenticationPackage": "示例值",
    "Caption": "Example",
    "LogonId": "示例值",
    "LogonType": "2",
    "StartTime": "2026-09-11T09:00:00Z"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | AuthenticationPackage：示例值 · LogonId：示例值 · LogonType：2 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.identity.logon-sessions`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-033"></a>
### 033 交互式终端会话

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.identity.interactive-sessions |
| source | pcscope.identity.interactive-sessions |
| 分类 | identity |
| 插件目录分组 | system |
| 渠道／接口 | Win32 API · wtsapi32!WTSEnumerateSessionsW / WTSQuerySessionInformationW |
| 采集方式（目录原文） | 通过 IdentityNative.cs P/Invoke 枚举会话、站点、用户、域与状态码 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:15](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| domain | 计算机所属域、工作组或账户域；按来源区分 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| identity_query_error | 身份查询错误码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| session_id | 会话编号 | 会话编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| state_code | 来源状态代码 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| station | 终端站点名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| user | 用户名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.identity.interactive-sessions",
  "id": "example-object",
  "item_name": "交互式终端会话",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "domain": "示例值",
    "identity_query_error": "示例值",
    "session_id": "示例值",
    "state_code": "示例状态",
    "station": "示例值",
    "user": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | domain：示例值 · identity_query_error：示例值 · 会话编号：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.identity.interactive-sessions`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-034"></a>
### 034 账户权限分配

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.identity.assigned-rights |
| source | pcscope.identity.assigned-rights |
| 分类 | identity |
| 插件目录分组 | system |
| 渠道／接口 | Win32 API · advapi32!LsaOpenPolicy / LsaEnumerateAccountsWithUserRight / LsaEnumerateAccountRights |
| 采集方式（目录原文） | 通过 IdentityNative.cs 读取 SID 的 LSA 权限分配；需策略读取权限，非有效访问判定 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:16](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| rights | 分配的权限 | 分配的权限 | ["示例成员"] | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| sid | 安全标识符 | 安全标识符 | "S-1-5-21-100-200-300-1001" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.identity.assigned-rights",
  "id": "example-object",
  "item_name": "账户权限分配",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "rights": [
      "示例成员"
    ],
    "sid": "S-1-5-21-100-200-300-1001"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 安全标识符：S-1-5-21-100-200-300-1001 |
| 默认属性组 | 身份与权限：分配的权限 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.identity.assigned-rights`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-035"></a>
### 035 进程清单

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.execution.processes |
| source | pcscope.execution.processes |
| 分类 | execution |
| 插件目录分组 | system |
| 渠道／接口 | CIM · root/cimv2: Win32_Process |
| 采集方式（目录原文） | 按 PID 与创建时间记录父 PID、程序路径、会话；保存脱敏命令行；来源未返回时标记原因 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:17](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| command_line | 进程命令行 | 进程命令行 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| command_line_collected | 是否采集命令行 | 是否采集命令行 | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| command_line_status | 命令行字段读取状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| parent_pid | 父进程编号 | 父进程编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| pid | 进程编号 | 进程编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| session_id | 会话编号 | 会话编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| start_time | 启动时间 | 启动时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.execution.processes",
  "id": "example-object",
  "item_name": "进程清单",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "command_line": "示例值",
    "command_line_collected": false,
    "command_line_status": "示例状态",
    "name": "Example",
    "parent_pid": "示例值",
    "path": "C:\\Example\\item",
    "pid": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 进程命令行：示例值 · command_line_status：示例状态 · 父进程编号：示例值 |
| 默认属性组 | 运行与启动配置：名称、路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.execution.processes`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-036"></a>
### 036 内存与 CPU 采样

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.execution.resources |
| source | pcscope.execution.resources |
| 分类 | execution |
| 插件目录分组 | system |
| 渠道／接口 | CIM · Win32_OperatingSystem / Win32_PerfFormattedData_PerfOS_Processor |
| 采集方式（目录原文） | 读取内存总量/可用量与 CPU 百分比；时点采样，不是连续性能曲线 |

**实现位置**：[plugins/powershell-adapter/domains/system.ps1:18](../plugins/powershell-adapter/domains/system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| free_kib | 可用内存，KiB | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| percent_processor_time | CPU 使用百分比 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| sampled_at | 采样时间 | 采样时间 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| total_kib | 总内存，KiB | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.execution.resources",
  "id": "example-object",
  "item_name": "内存与 CPU 采样",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "free_kib": 1024,
    "percent_processor_time": "2026-09-11T09:00:00Z",
    "sampled_at": "示例值",
    "total_kib": 1024
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | free_kib：1024 · percent_processor_time：2026-09-11 17:00:00（本机时间） · total_kib：1024 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.execution.resources`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-037"></a>
### 037 Defender 防护状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.protection |
| source | pcscope.security.protection |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | PowerShell · Get-MpComputerStatus |
| 采集方式（目录原文） | 读取引擎、签名、防护开关与最近扫描时间；状态快照 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:1](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AMEngineVersion | Defender 引擎版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AMProductVersion | Defender 产品版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AMServiceEnabled | 反恶意软件服务是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AntispywareEnabled | 反间谍软件功能是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AntivirusEnabled | 防病毒功能是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AntivirusSignatureLastUpdated | 病毒情报最近更新时间 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AntivirusSignatureVersion | 病毒情报版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| BehaviorMonitorEnabled | 行为监控是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| FullScanEndTime | 最近完整扫描结束时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IoavProtectionEnabled | 下载文件与附件防护是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsTamperProtected | 篡改防护是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| NISEnabled | 网络检查系统是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| OnAccessProtectionEnabled | 访问时防护是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| QuickScanEndTime | 最近快速扫描结束时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| RealTimeProtectionEnabled | 实时防护是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.protection",
  "id": "example-object",
  "item_name": "Defender 防护状态",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AMEngineVersion": "1.0.0",
    "AMProductVersion": "1.0.0",
    "AMServiceEnabled": "示例值",
    "AntispywareEnabled": "示例值",
    "AntivirusEnabled": "示例值",
    "AntivirusSignatureLastUpdated": "示例值",
    "AntivirusSignatureVersion": "1.0.0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | AMEngineVersion：1.0.0 · AMProductVersion：1.0.0 · AMServiceEnabled：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.protection`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-038"></a>
### 038 Defender 策略与排除项

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.protection-policy |
| source | pcscope.security.protection-policy |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | PowerShell · Get-MpPreference |
| 采集方式（目录原文） | 读取指定策略、排除路径/进程/扩展名；不修改防护配置 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:2](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| DisableBehaviorMonitoring | 是否禁用行为监控 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DisableIOAVProtection | 是否禁用下载文件与附件防护 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DisableRealtimeMonitoring | 是否禁用实时监控 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DisableScriptScanning | 是否禁用脚本扫描 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ExclusionExtension | 排除的文件扩展名 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ExclusionPath | 排除路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ExclusionProcess | 排除进程 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PUAProtection | 潜在不受欢迎应用防护设置 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.protection-policy",
  "id": "example-object",
  "item_name": "Defender 策略与排除项",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "DisableBehaviorMonitoring": "示例值",
    "DisableIOAVProtection": "示例值",
    "DisableRealtimeMonitoring": "示例值",
    "DisableScriptScanning": "示例值",
    "ExclusionExtension": "示例值",
    "ExclusionPath": "C:\\Example\\item",
    "ExclusionProcess": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | DisableBehaviorMonitoring：示例值 · DisableIOAVProtection：示例值 · DisableRealtimeMonitoring：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.protection-policy`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-039"></a>
### 039 安全产品登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.products |
| source | pcscope.security.products |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | CIM · root/SecurityCenter2: AntiVirusProduct |
| 采集方式（目录原文） | 读取产品名称、实例 ID 与产品状态码；依赖系统提供该命名空间 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:3](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| displayName | 显示名称 | 显示名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| instanceGuid | 实例 GUID | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| productState | 安全产品登记状态代码 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |
| timestamp | 来源时间戳 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.products",
  "id": "example-object",
  "item_name": "安全产品登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "displayName": "Example",
    "instanceGuid": "示例值",
    "productState": "示例状态",
    "timestamp": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | instanceGuid：示例值 · productState：示例状态 · timestamp：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.products`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-040"></a>
### 040 Defender 检测记录

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.detections |
| source | pcscope.security.detections |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | PowerShell · Get-MpThreatDetection |
| 采集方式（目录原文） | 读取来源保留的检测时间、威胁与处理状态；非主动扫描 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:4](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| ActionSuccess | 处置动作是否成功 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| CleaningActionID | 清理动作编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| CurrentThreatExecutionStatusID | 威胁执行状态编号 | 保留原名（无专用中文标签） | "S-1-5-21-100-200-300-1001" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DetectionID | 检测记录标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InitialDetectionTime | 首次检测时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LastThreatStatusChangeTime | 威胁状态最近变化时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ThreatID | 威胁标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ThreatStatusID | 威胁状态编号 | 保留原名（无专用中文标签） | "S-1-5-21-100-200-300-1001" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.detections",
  "id": "example-object",
  "item_name": "Defender 检测记录",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "ActionSuccess": "示例值",
    "CleaningActionID": "示例值",
    "CurrentThreatExecutionStatusID": "S-1-5-21-100-200-300-1001",
    "DetectionID": "示例值",
    "InitialDetectionTime": "2026-09-11T09:00:00Z",
    "LastThreatStatusChangeTime": "2026-09-11T09:00:00Z",
    "ThreatID": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | ActionSuccess：示例值 · CleaningActionID：示例值 · CurrentThreatExecutionStatusID：S-1-5-21-100-200-300-1001 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.detections`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-041"></a>
### 041 Device Guard / VBS

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.deviceguard |
| source | pcscope.security.deviceguard |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | CIM · root/Microsoft/Windows/DeviceGuard: Win32_DeviceGuard |
| 采集方式（目录原文） | 读取安全能力、配置与运行服务编号；状态快照 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:5](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AvailableSecurityProperties | 设备可用安全能力编号列表 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| CodeIntegrityPolicyEnforcementStatus | 代码完整性策略执行状态代码 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InstanceIdentifier | 实例标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| RequiredSecurityProperties | 所需安全能力编号列表 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SecurityServicesConfigured | 已配置的安全服务编号列表 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SecurityServicesRunning | 正在运行的安全服务编号列表 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| VirtualizationBasedSecurityStatus | 基于虚拟化的安全状态代码 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.deviceguard",
  "id": "example-object",
  "item_name": "Device Guard / VBS",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AvailableSecurityProperties": "示例值",
    "CodeIntegrityPolicyEnforcementStatus": "示例状态",
    "InstanceIdentifier": "示例值",
    "RequiredSecurityProperties": "示例值",
    "SecurityServicesConfigured": "示例值",
    "SecurityServicesRunning": "示例值",
    "VirtualizationBasedSecurityStatus": "示例状态"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | AvailableSecurityProperties：示例值 · CodeIntegrityPolicyEnforcementStatus：示例状态 · InstanceIdentifier：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.deviceguard`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-042"></a>
### 042 TPM 状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.tpm |
| source | pcscope.security.tpm |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | PowerShell · Get-Tpm |
| 采集方式（目录原文） | 读取 TPM 存在、就绪、开启、锁定及制造商信息 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:6](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| LockedOut | 是否处于锁定状态 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ManufacturerIdTxt | 制造商文本标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ManufacturerVersion | 制造商版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| RestartPending | 是否等待重启 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| TpmActivated | TPM 是否激活 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| TpmEnabled | TPM 是否启用 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| TpmOwned | TPM 是否已取得所有权 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| TpmPresent | TPM 是否存在 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| TpmReady | TPM 是否就绪 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.tpm",
  "id": "example-object",
  "item_name": "TPM 状态",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "LockedOut": false,
    "ManufacturerIdTxt": "示例值",
    "ManufacturerVersion": "1.0.0",
    "RestartPending": false,
    "TpmActivated": false,
    "TpmEnabled": false,
    "TpmOwned": false
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | LockedOut：否 · ManufacturerIdTxt：示例值 · ManufacturerVersion：1.0.0 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.tpm`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-043"></a>
### 043 安全启动状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.secureboot |
| source | pcscope.security.secureboot |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | PowerShell · Confirm-SecureBootUEFI |
| 采集方式（目录原文） | 查询 UEFI Secure Boot；组件不支持或权限不足报告缺口 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:7](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| enabled | 是否启用 | 是否启用 | true | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.secureboot",
  "id": "example-object",
  "item_name": "安全启动状态",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "enabled": true
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 是否启用：是 |
| 默认属性组 | 设备与系统状态：是否启用 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.secureboot`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-044"></a>
### 044 计算机证书

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.certificates.localmachine |
| source | pcscope.security.certificates.localmachine |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | .NET API · X509Store(StoreLocation.LocalMachine)，Root / CA / My / TrustedPublisher / Disallowed / TrustedPeople |
| 采集方式（目录原文） | ReadOnly + OpenExistingOnly 枚举证书元数据；不导出私钥，不执行信任评估 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:8](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| has_private_key | 证书是否关联私钥，仅记录存在标志 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| issuer | 证书颁发者 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| location | 证书存储位置或来源指定位置 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| not_after | 证书有效期截止时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| not_before | 证书有效期开始时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| signature_algorithm | 签名算法标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| store | 证书存储区名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| subject | 证书主题 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| thumbprint | 证书指纹 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| trust | 本次是否执行信任评估的标记 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.certificates.localmachine",
  "id": "example-object",
  "item_name": "计算机证书",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "has_private_key": false,
    "issuer": "示例值",
    "location": "C:\\Example\\item",
    "not_after": "2026-09-11T09:00:00Z",
    "not_before": "2026-09-11T09:00:00Z",
    "signature_algorithm": "示例值",
    "store": "My",
    "subject": "CN=Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | CN=Example |
| 属性摘要 | has_private_key：否 · issuer：示例值 · location：C:\Example\item |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.certificates.localmachine`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-045"></a>
### 045 当前用户证书

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.certificates.currentuser |
| source | pcscope.security.certificates.currentuser |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | .NET API · X509Store(StoreLocation.CurrentUser)，Root / CA / My / TrustedPublisher / Disallowed / TrustedPeople |
| 采集方式（目录原文） | 只读枚举指纹、主题、颁发者、有效期与私钥存在标志 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:8](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| has_private_key | 证书是否关联私钥，仅记录存在标志 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| issuer | 证书颁发者 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| location | 证书存储位置或来源指定位置 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| not_after | 证书有效期截止时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| not_before | 证书有效期开始时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| signature_algorithm | 签名算法标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| store | 证书存储区名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| subject | 证书主题 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| thumbprint | 证书指纹 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| trust | 本次是否执行信任评估的标记 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.certificates.currentuser",
  "id": "example-object",
  "item_name": "当前用户证书",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "has_private_key": false,
    "issuer": "示例值",
    "location": "C:\\Example\\item",
    "not_after": "2026-09-11T09:00:00Z",
    "not_before": "2026-09-11T09:00:00Z",
    "signature_algorithm": "示例值",
    "store": "My",
    "subject": "CN=Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | CN=Example |
| 属性摘要 | has_private_key：否 · issuer：示例值 · location：C:\Example\item |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.certificates.currentuser`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-046"></a>
### 046 BitLocker 卷状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.encryption |
| source | pcscope.security.encryption |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | PowerShell · Get-BitLockerVolume |
| 采集方式（目录原文） | 读取加密方法、进度、保护与锁定状态、保护器类型；不采集恢复材料 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:14](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| encryption_method | 加密方法 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| encryption_percentage | 加密完成百分比 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| lock_status | 卷锁定状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| mount_point | 挂载点 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| protection_status | 保护状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| protector_types | 密钥保护器类型列表 | 保留原名（无专用中文标签） | ["示例成员"] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| recovery_material_collected | 是否采集恢复材料 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| volume_status | 卷状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| volume_type | 卷类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.encryption",
  "id": "example-object",
  "item_name": "BitLocker 卷状态",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "encryption_method": "示例值",
    "encryption_percentage": "示例值",
    "lock_status": "示例状态",
    "mount_point": "示例值",
    "protection_status": "示例状态",
    "protector_types": [
      "示例成员"
    ],
    "recovery_material_collected": false
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | encryption_method：示例值 · encryption_percentage：示例值 · lock_status：示例状态 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.encryption`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-047"></a>
### 047 UAC、资源管理器与软件限制策略

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.policy.configuration |
| source | pcscope.policy.configuration |
| 分类 | policy |
| 插件目录分组 | security |
| 渠道／接口 | 注册表 · HKLM/HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies；HKLM\SOFTWARE\Policies\Microsoft\Windows\Safer |
| 采集方式（目录原文） | 递归最多 3 层；白名单字段保留原值，其他值保存摘要 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:15](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.policy.configuration",
  "id": "example-object",
  "item_name": "UAC、资源管理器与软件限制策略",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.policy.configuration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-048"></a>
### 048 SSH 公钥指纹

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.security.ssh-public-keys |
| source | pcscope.security.ssh-public-keys |
| 分类 | security |
| 插件目录分组 | security |
| 渠道／接口 | 文件系统 · %USERPROFILE%\.ssh\*.pub；File.ReadAllText / SHA256 |
| 采集方式（目录原文） | 读取支持的公钥编码并生成算法与指纹；不保存密钥材料和注释 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:16](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| algorithm | 算法名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| comment_collected | 是否采集注释 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| fingerprint | 指纹 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| key_material_collected | 是否保存密钥材料 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.security.ssh-public-keys",
  "id": "example-object",
  "item_name": "SSH 公钥指纹",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "algorithm": "示例值",
    "comment_collected": false,
    "fingerprint": "示例值",
    "key_material_collected": false,
    "path": "C:\\Example\\item"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | algorithm：示例值 · fingerprint：示例值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.security.ssh-public-keys`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-049"></a>
### 049 事件通道可读性

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.collector.event-channel-coverage |
| source | pcscope.collector.event-channel-coverage |
| 分类 | collector |
| 插件目录分组 | security |
| 渠道／接口 | PowerShell · Get-WinEvent -ListLog；随包 channels.json |
| 采集方式（目录原文） | 逐通道查询启用、容量、保留策略、记录数与错误；只查元数据，不代表审计完整 |

**实现位置**：[plugins/powershell-adapter/domains/security.ps1:25](../plugins/powershell-adapter/domains/security.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| channel | 日志通道 | 日志通道 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| enabled | 是否启用 | 是否启用 | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| log_mode | 日志保留模式 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| maximum_bytes | 日志最大容量，字节 | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| oldest_record | 最早可用记录编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| readable | 通道是否可读 | 保留原名（无专用中文标签） | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| reason | 原因 | 原因 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| record_count | 日志记录数量 | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.collector.event-channel-coverage",
  "id": "example-object",
  "item_name": "事件通道可读性",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "channel": "示例值",
    "enabled": true,
    "log_mode": "示例值",
    "maximum_bytes": 1024,
    "oldest_record": "示例值",
    "readable": true,
    "reason": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 是否启用：是 · 日志通道：示例值 · log_mode：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.collector.event-channel-coverage`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-050"></a>
### 050 路由表

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.routes |
| source | pcscope.network.routes |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-NetRoute |
| 采集方式（目录原文） | 枚举接口、目的前缀、下一跳、度量与策略来源；状态快照 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:1](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AddressFamily | 地址族 | 地址族 | "192.0.2.10" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DestinationPrefix | 目标网络前缀 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InterfaceAlias | 接口名称 | 接口名称 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| InterfaceIndex | 接口编号 | 接口编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| NextHop | 下一跳地址 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PolicyStore | 配置所属的策略存储区 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Protocol | 协议名称或协议编号，保持来源格式 | 保留原名（无专用中文标签） | "6" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Publish | 路由发布设置 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| RouteMetric | 路由度量值 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.routes",
  "id": "example-object",
  "item_name": "路由表",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AddressFamily": "192.0.2.10",
    "DestinationPrefix": "示例值",
    "InterfaceAlias": "示例值",
    "InterfaceIndex": "示例值",
    "NextHop": "示例值",
    "PolicyStore": "示例值",
    "Protocol": "6"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | 地址族：192.0.2.10 · DestinationPrefix：示例值 · 接口名称：示例值 |
| 默认属性组 | 网络配置：接口名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.routes`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-051"></a>
### 051 DNS 配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.dns-configuration |
| source | pcscope.network.dns-configuration |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | Get-DnsClientServerAddress；注册表 HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters |
| 采集方式（目录原文） | 查询接口 DNS 服务器及域/搜索列表配置 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:2](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AddressFamily | 地址族 | 地址族 | "192.0.2.10" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InterfaceAlias | 接口名称 | 接口名称 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| InterfaceIndex | 接口编号 | 接口编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ServerAddresses | 服务器地址列表；DNS 来源中为 DNS 服务器 | 保留原名（无专用中文标签） | "192.0.2.10" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.dns-configuration",
  "id": "example-object",
  "item_name": "DNS 配置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    },
    "AddressFamily": "192.0.2.10",
    "InterfaceAlias": "示例值",
    "InterfaceIndex": "示例值",
    "ServerAddresses": "192.0.2.10"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | ExampleValue：示例登记值 · 地址族：192.0.2.10 · 接口名称：示例值 |
| 默认属性组 | 网络配置：接口名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.dns-configuration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-052"></a>
### 052 网络连接配置文件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.profiles |
| source | pcscope.network.profiles |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-NetConnectionProfile |
| 采集方式（目录原文） | 读取网络名称、类别、IPv4/IPv6 连通性 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:3](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| IPv4Connectivity | IPv4 连通性状态 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IPv6Connectivity | IPv6 连通性状态 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InterfaceIndex | 接口编号 | 接口编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| NetworkCategory | 网络配置文件类别 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.profiles",
  "id": "example-object",
  "item_name": "网络连接配置文件",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "IPv4Connectivity": "示例值",
    "IPv6Connectivity": "示例值",
    "InterfaceIndex": "示例值",
    "Name": "Example",
    "NetworkCategory": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | IPv4Connectivity：示例值 · IPv6Connectivity：示例值 · 接口编号：示例值 |
| 默认属性组 | 网络配置：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.profiles`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-053"></a>
### 053 TCP 连接与监听

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.tcp |
| source | pcscope.network.tcp |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-NetTCPConnection |
| 采集方式（目录原文） | 时点枚举本地/远端地址端口、状态、所属 PID 与创建时间；不抓包 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:4](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AppliedSetting | 应用到连接的设置类别 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| CreationTime | 来源提供的创建时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LocalAddress | 本地地址 | 保留原名（无专用中文标签） | "192.0.2.10" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LocalPort | 本地端口 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| OwningProcess | 拥有该连接或端点的进程编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| RemoteAddress | 远端地址 | 保留原名（无专用中文标签） | "192.0.2.10" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| RemotePort | 远端端口 | 保留原名（无专用中文标签） | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| State | 状态 | 状态 | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.tcp",
  "id": "example-object",
  "item_name": "TCP 连接与监听",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AppliedSetting": "示例值",
    "CreationTime": "2026-09-11T09:00:00Z",
    "LocalAddress": "192.0.2.10",
    "LocalPort": "示例值",
    "OwningProcess": "示例值",
    "RemoteAddress": "192.0.2.10",
    "RemotePort": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | AppliedSetting：示例值 · CreationTime：2026-09-11 17:00:00（本机时间） · LocalAddress：192.0.2.10 |
| 默认属性组 | 网络配置：RemoteAddress、RemotePort |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.tcp`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-054"></a>
### 054 UDP 端点

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.udp |
| source | pcscope.network.udp |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-NetUDPEndpoint |
| 采集方式（目录原文） | 时点枚举本地地址端口、PID 与创建时间；不读取报文 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:5](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| CreationTime | 来源提供的创建时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LocalAddress | 本地地址 | 保留原名（无专用中文标签） | "192.0.2.10" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LocalPort | 本地端口 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| OwningProcess | 拥有该连接或端点的进程编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.udp",
  "id": "example-object",
  "item_name": "UDP 端点",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "CreationTime": "2026-09-11T09:00:00Z",
    "LocalAddress": "192.0.2.10",
    "LocalPort": "示例值",
    "OwningProcess": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | CreationTime：2026-09-11 17:00:00（本机时间） · LocalAddress：192.0.2.10 · LocalPort：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.udp`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-055"></a>
### 055 防火墙配置文件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.firewall-profiles |
| source | pcscope.network.firewall-profiles |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-NetFirewallProfile |
| 采集方式（目录原文） | 查询默认入站/出站动作、防火墙开关与日志配置；不读取流量日志正文 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:6](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AllowInboundRules | 是否允许入站规则生效 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AllowLocalFirewallRules | 是否合并本地防火墙规则 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AllowLocalIPsecRules | 是否合并本地 IPsec 规则 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DefaultInboundAction | 默认入站动作 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DefaultOutboundAction | 默认出站动作 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Enabled | 是否启用 | 是否启用 | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LogAllowed | 是否记录允许的连接 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LogBlocked | 是否记录被阻止的连接 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LogFileName | 日志文件路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| LogMaxSizeKilobytes | 日志最大大小，单位 KiB | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.firewall-profiles",
  "id": "example-object",
  "item_name": "防火墙配置文件",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AllowInboundRules": "示例值",
    "AllowLocalFirewallRules": "示例值",
    "AllowLocalIPsecRules": "示例值",
    "DefaultInboundAction": "示例值",
    "DefaultOutboundAction": "示例值",
    "Enabled": true,
    "LogAllowed": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 是否启用：是 · AllowInboundRules：示例值 · AllowLocalFirewallRules：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.firewall-profiles`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-056"></a>
### 056 防火墙规则及过滤条件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.firewall-rules |
| source | pcscope.network.firewall-rules |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | Get-NetFirewallRule；Get-NetFirewall{Port,Address,Application,Service,Interface,InterfaceType,Security}Filter -PolicyStore ActiveStore |
| 采集方式（目录原文） | 分别枚举 7 类过滤器，按 InstanceID 关联规则；缺失关联报告缺口 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:7](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Action | 来源定义的执行、允许或阻止动作 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Direction | 方向，来源可能使用枚举或资源代码 | 保留原名（无专用中文标签） | "%%14593" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DisplayName | 显示名称 | 显示名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| EdgeTraversalPolicy | 防火墙边缘遍历策略 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Enabled | 是否启用 | 是否启用 | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| PolicyStoreSource | 策略存储来源 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PolicyStoreSourceType | 策略存储来源类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PrimaryStatus | 来源报告的主要状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Profile | 用户配置文件或配置文件名称；具体类型由来源决定 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Status | 状态 | 状态 | "0x0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.firewall-rules",
  "id": "example-object",
  "item_name": "防火墙规则及过滤条件",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "Action": "示例值",
    "Direction": "%%14593",
    "DisplayName": "Example",
    "EdgeTraversalPolicy": "示例值",
    "Enabled": true,
    "Name": "Example",
    "PolicyStoreSource": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 是否启用：是 · Action：示例值 · Direction：%%14593 |
| 默认属性组 | 网络配置：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.firewall-rules`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-057"></a>
### 057 IPsec 规则

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.ipsec |
| source | pcscope.network.ipsec |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-NetIPsecRule -PolicyStore ActiveStore |
| 采集方式（目录原文） | 枚举规则、安全模式与策略来源；状态快照 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:21](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| DisplayName | 显示名称 | 显示名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Enabled | 是否启用 | 是否启用 | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InboundSecurity | 入站安全要求 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Mode | 来源运行或连接模式 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| OutboundSecurity | 出站安全要求 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PolicyStoreSource | 策略存储来源 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PolicyStoreSourceType | 策略存储来源类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Profile | 用户配置文件或配置文件名称；具体类型由来源决定 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.ipsec",
  "id": "example-object",
  "item_name": "IPsec 规则",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "DisplayName": "Example",
    "Enabled": true,
    "InboundSecurity": "示例值",
    "Mode": "示例值",
    "Name": "Example",
    "OutboundSecurity": "示例值",
    "PolicyStoreSource": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 是否启用：是 · InboundSecurity：示例值 · Mode：示例值 |
| 默认属性组 | 网络配置：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.ipsec`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-058"></a>
### 058 代理、远程桌面与 VPN

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.access |
| source | pcscope.network.access |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | Get-VpnConnection；HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings；HKLM\SYSTEM\CurrentControlSet\Control\Terminal Server |
| 采集方式（目录原文） | 读取当前用户 VPN 和代理、RDP 配置；注册表非白名单值保存摘要 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:22](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AllUserConnection | VPN 是否为所有用户连接 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AuthenticationMethod | 认证方法 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ConnectionStatus | 连接状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| SplitTunneling | 是否启用分流隧道 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| TunnelType | 隧道类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.access",
  "id": "example-object",
  "item_name": "代理、远程桌面与 VPN",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "SOFTWARE\\Example",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    },
    "AllUserConnection": "示例值",
    "AuthenticationMethod": "示例值",
    "ConnectionStatus": "示例状态",
    "Name": "Example",
    "SplitTunneling": "示例值",
    "TunnelType": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | ExampleValue：示例登记值 · AllUserConnection：示例值 · AuthenticationMethod：示例值 |
| 默认属性组 | 网络配置：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.access`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-059"></a>
### 059 SMB 共享与访问授权

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.shares |
| source | pcscope.network.shares |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-SmbShare / Get-SmbShareAccess -Name |
| 采集方式（目录原文） | 枚举共享路径与共享层 ACL；非文件系统全部有效权限 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:23](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AccessControlType | 访问控制项类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AccessRight | 访问权限 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AccountName | 账户名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| access | 访问授权列表 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| encrypt_data | 共享是否要求数据加密 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |
| special | 是否为系统特殊对象 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.shares",
  "id": "example-object",
  "item_name": "SMB 共享与访问授权",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AccessControlType": "示例值",
    "AccessRight": "示例值",
    "AccountName": "示例值",
    "access": "示例值",
    "description": "示例值",
    "encrypt_data": "示例值",
    "name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | AccessControlType：示例值 · AccessRight：示例值 · AccountName：示例值 |
| 默认属性组 | 网络配置：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.shares`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-060"></a>
### 060 SMB 驱动器映射

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.mapped-drives |
| source | pcscope.network.mapped-drives |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-SmbMapping |
| 采集方式（目录原文） | 读取本地盘符、远端路径与状态 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:24](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| LocalPath | 本地路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| RemotePath | 远端路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Status | 状态 | 状态 | "0x0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.mapped-drives",
  "id": "example-object",
  "item_name": "SMB 驱动器映射",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "LocalPath": "C:\\Example\\item",
    "RemotePath": "C:\\Example\\item",
    "Status": "0x0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 状态：0x0 · LocalPath：C:\Example\item · RemotePath：C:\Example\item |
| 默认属性组 | 网络配置：状态 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.mapped-drives`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-061"></a>
### 061 DNS 缓存

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.dns-cache |
| source | pcscope.network.dns-cache |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-DnsClientCache |
| 采集方式（目录原文） | 时点缓存条目与解析值；非完整查询历史或查询发生时间 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:25](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| data | 嵌套业务数据；具体结构随对象类型变化 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| entry | 条目名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| section | 来源分组或配置节 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| status | 状态 | 状态 | "示例状态" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| type | 来源定义的类型；具体含义取决于本项对象 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.dns-cache",
  "id": "example-object",
  "item_name": "DNS 缓存",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "data": "示例值",
    "entry": "示例值",
    "name": "Example",
    "section": "示例值",
    "status": "示例状态",
    "type": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 状态：示例状态 · data：示例值 · entry：示例值 |
| 默认属性组 | 网络配置：名称、状态 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.dns-cache`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-062"></a>
### 062 hosts 静态映射

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.hosts |
| source | pcscope.network.hosts |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | 文件系统 · %SystemRoot%\System32\drivers\etc\hosts；Get-Content |
| 采集方式（目录原文） | 解析有效地址和名称、排除注释；拒绝超大文件与重解析点 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:26](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| address | 地址 | 地址 | "192.0.2.10" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| line | 来源文件行信息 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.hosts",
  "id": "example-object",
  "item_name": "hosts 静态映射",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "address": "192.0.2.10",
    "line": "示例值",
    "name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 地址：192.0.2.10 · line：示例值 |
| 默认属性组 | 网络配置：名称、地址 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.hosts`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-063"></a>
### 063 SMB 客户端会话

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.share-sessions |
| source | pcscope.network.share-sessions |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | PowerShell · Get-SmbSession |
| 采集方式（目录原文） | 读取会话、客户端、用户、打开数、协议与加密状态 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:27](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| ClientComputerName | 客户端计算机名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ClientUserName | 客户端用户名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ContinuouslyAvailable | 共享是否支持持续可用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Dialect | SMB 协议版本 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Encrypted | 会话是否加密 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| NumOpens | 打开的对象数量 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SessionId | 会话编号 | 会话编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.share-sessions",
  "id": "example-object",
  "item_name": "SMB 客户端会话",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "ClientComputerName": "示例值",
    "ClientUserName": "示例值",
    "ContinuouslyAvailable": "示例值",
    "Dialect": "示例值",
    "Encrypted": "示例值",
    "NumOpens": "示例值",
    "SessionId": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | ClientComputerName：示例值 · ClientUserName：示例值 · ContinuouslyAvailable：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.share-sessions`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-064"></a>
### 064 WinRM 配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.network.winrm-configuration |
| source | pcscope.network.winrm-configuration |
| 分类 | network |
| 插件目录分组 | network |
| 渠道／接口 | 注册表 · HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\WSMAN |
| 采集方式（目录原文） | 递归最多 3 层，保存指定认证、监听与传输配置；不连接远端机器 |

**实现位置**：[plugins/powershell-adapter/domains/network.ps1:28](../plugins/powershell-adapter/domains/network.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.network.winrm-configuration",
  "id": "example-object",
  "item_name": "WinRM 配置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.network.winrm-configuration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-065"></a>
### 065 磁盘清单

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.storage.disks |
| source | pcscope.storage.disks |
| 分类 | storage |
| 插件目录分组 | storage |
| 渠道／接口 | PowerShell · Get-Disk |
| 采集方式（目录原文） | 读取磁盘 ID、总线、分区样式、容量、启动和读写状态 |

**实现位置**：[plugins/powershell-adapter/domains/storage.ps1:1](../plugins/powershell-adapter/domains/storage.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| BusType | 存储总线类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| FriendlyName | 友好名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| HealthStatus | 来源报告的健康状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsBoot | 是否为启动相关磁盘 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsOffline | 是否脱机 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsReadOnly | 是否只读 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsSystem | 是否为系统磁盘 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Number | 来源对象编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ObjectId | 对象标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| OperationalStatus | 操作状态列表 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PartitionStyle | 分区表样式 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Size | 大小 | 大小 | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| UniqueId | 唯一标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.storage.disks",
  "id": "example-object",
  "item_name": "磁盘清单",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "BusType": "示例值",
    "FriendlyName": "示例值",
    "HealthStatus": "示例状态",
    "IsBoot": "示例值",
    "IsOffline": "示例值",
    "IsReadOnly": "示例值",
    "IsSystem": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | BusType：示例值 · FriendlyName：示例值 · HealthStatus：示例状态 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.storage.disks`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-066"></a>
### 066 分区清单

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.storage.partitions |
| source | pcscope.storage.partitions |
| 分类 | storage |
| 插件目录分组 | storage |
| 渠道／接口 | PowerShell · Get-Partition |
| 采集方式（目录原文） | 读取磁盘/分区编号、GUID、偏移、大小与访问路径 |

**实现位置**：[plugins/powershell-adapter/domains/storage.ps1:2](../plugins/powershell-adapter/domains/storage.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AccessPaths | 访问路径列表 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DiskId | 磁盘标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DiskNumber | 磁盘编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| GptType | GPT 分区类型标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Guid | GUID 标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsActive | 是否为活动分区 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsBoot | 是否为启动相关磁盘 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsHidden | 是否隐藏 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsReadOnly | 是否只读 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| MbrType | MBR 分区类型代码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ObjectId | 对象标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Offset | 分区起始偏移 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PartitionNumber | 分区编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Size | 大小 | 大小 | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Type | 来源定义的类型；具体含义取决于本项对象 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.storage.partitions",
  "id": "example-object",
  "item_name": "分区清单",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AccessPaths": "C:\\Example\\item",
    "DiskId": "示例值",
    "DiskNumber": "示例值",
    "GptType": "示例值",
    "Guid": "示例值",
    "IsActive": "示例值",
    "IsBoot": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | AccessPaths：C:\Example\item · DiskId：示例值 · DiskNumber：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.storage.partitions`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-067"></a>
### 067 设备驱动绑定

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.hardware.driver-bindings |
| source | pcscope.hardware.driver-bindings |
| 分类 | drivers |
| 插件目录分组 | storage |
| 渠道／接口 | CIM · root/cimv2: Win32_PnPSignedDriver |
| 采集方式（目录原文） | 读取设备与驱动版本、日期、提供方、INF 及来源签名标志 |

**实现位置**：[plugins/powershell-adapter/domains/storage.ps1:3](../plugins/powershell-adapter/domains/storage.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| DeviceID | 设备标识 | 设备标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DeviceName | 设备名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DriverDate | 驱动日期 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DriverName | 驱动名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DriverProviderName | 驱动提供方 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DriverVersion | 驱动版本 | 驱动版本 | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InfName | 驱动 INF 名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsSigned | 来源报告是否签名 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Signer | 签名者 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.hardware.driver-bindings",
  "id": "example-object",
  "item_name": "设备驱动绑定",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "DeviceID": "示例值",
    "DeviceName": "示例值",
    "DriverDate": "2026-09-11T09:00:00Z",
    "DriverName": "示例值",
    "DriverProviderName": "示例值",
    "DriverVersion": "1.0.0",
    "InfName": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 设备标识：示例值 · DeviceName：示例值 · DriverDate：2026-09-11 17:00:00（本机时间） |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.hardware.driver-bindings`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-068"></a>
### 068 驱动服务

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.hardware.driver-services |
| source | pcscope.hardware.driver-services |
| 分类 | drivers |
| 插件目录分组 | storage |
| 渠道／接口 | CIM · root/cimv2: Win32_SystemDriver |
| 采集方式（目录原文） | 读取驱动路径、服务类型、启动模式与运行状态 |

**实现位置**：[plugins/powershell-adapter/domains/storage.ps1:4](../plugins/powershell-adapter/domains/storage.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| DisplayName | 显示名称 | 显示名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| PathName | 程序或驱动路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ServiceType | 服务类型 | 服务类型 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| StartMode | 启动方式 | 启动方式 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Started | 是否已启动 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| State | 状态 | 状态 | "示例状态" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| Status | 状态 | 状态 | "0x0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.hardware.driver-services",
  "id": "example-object",
  "item_name": "驱动服务",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "DisplayName": "Example",
    "Name": "Example",
    "PathName": "C:\\Example\\item",
    "ServiceType": "示例值",
    "StartMode": "示例值",
    "Started": "示例值",
    "State": "示例状态"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 状态：示例状态 · 启动方式：示例值 · PathName：C:\Example\item |
| 默认属性组 | 设备与系统状态：名称、状态 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.hardware.driver-services`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-069"></a>
### 069 BIOS 固件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.hardware.firmware |
| source | pcscope.hardware.firmware |
| 分类 | hardware |
| 插件目录分组 | storage |
| 渠道／接口 | CIM · root/cimv2: Win32_BIOS |
| 采集方式（目录原文） | 读取制造商、BIOS/SMBIOS 版本与发布日期 |

**实现位置**：[plugins/powershell-adapter/domains/storage.ps1:5](../plugins/powershell-adapter/domains/storage.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Manufacturer | 制造商 | 制造商 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| ReleaseDate | 发布日期 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SMBIOSBIOSVersion | SMBIOS 中的 BIOS 版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SMBIOSMajorVersion | SMBIOS 主版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SMBIOSMinorVersion | SMBIOS 次版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.hardware.firmware",
  "id": "example-object",
  "item_name": "BIOS 固件",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "Manufacturer": "示例值",
    "Name": "Example",
    "ReleaseDate": "2026-09-11T09:00:00Z",
    "SMBIOSBIOSVersion": "1.0.0",
    "SMBIOSMajorVersion": "1.0.0",
    "SMBIOSMinorVersion": "1.0.0",
    "Version": "1.0.0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 版本：1.0.0 · 制造商：示例值 · ReleaseDate：2026-09-11 17:00:00（本机时间） |
| 默认属性组 | 设备与系统状态：名称、制造商、版本 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.hardware.firmware`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-070"></a>
### 070 物理磁盘健康

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.hardware.disk-health |
| source | pcscope.hardware.disk-health |
| 分类 | hardware |
| 插件目录分组 | storage |
| 渠道／接口 | PowerShell · Get-PhysicalDisk |
| 采集方式（目录原文） | 读取固件版本、介质类型、健康与操作状态；不是完整 SMART 原始属性 |

**实现位置**：[plugins/powershell-adapter/domains/storage.ps1:6](../plugins/powershell-adapter/domains/storage.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| BusType | 存储总线类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| FirmwareVersion | 固件版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| FriendlyName | 友好名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| HealthStatus | 来源报告的健康状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| MediaType | 介质类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ObjectId | 对象标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| OperationalStatus | 操作状态列表 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Size | 大小 | 大小 | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.hardware.disk-health",
  "id": "example-object",
  "item_name": "物理磁盘健康",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "BusType": "示例值",
    "FirmwareVersion": "1.0.0",
    "FriendlyName": "示例值",
    "HealthStatus": "示例状态",
    "MediaType": "示例值",
    "ObjectId": "示例值",
    "OperationalStatus": "示例状态"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | 示例值 |
| 属性摘要 | BusType：示例值 · FirmwareVersion：1.0.0 · FriendlyName：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.hardware.disk-health`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-071"></a>
### 071 电池状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.hardware.battery |
| source | pcscope.hardware.battery |
| 分类 | hardware |
| 插件目录分组 | storage |
| 渠道／接口 | CIM · root/cimv2: Win32_Battery |
| 采集方式（目录原文） | 读取来源提供的容量、化学类型、余量与估算时间 |

**实现位置**：[plugins/powershell-adapter/domains/storage.ps1:7](../plugins/powershell-adapter/domains/storage.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| BatteryStatus | 电池状态代码 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Chemistry | 电池化学类型代码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DesignCapacity | 设计容量，单位依来源接口 | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DeviceID | 设备标识 | 设备标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| EstimatedChargeRemaining | 估算剩余电量百分比 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| EstimatedRunTime | 估算剩余运行时间，单位依来源接口 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| FullChargeCapacity | 充满时容量，单位依来源接口 | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| Status | 状态 | 状态 | "0x0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.hardware.battery",
  "id": "example-object",
  "item_name": "电池状态",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "BatteryStatus": "示例状态",
    "Chemistry": "示例值",
    "DesignCapacity": 1024,
    "DeviceID": "示例值",
    "EstimatedChargeRemaining": "示例值",
    "EstimatedRunTime": "2026-09-11T09:00:00Z",
    "FullChargeCapacity": 1024
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | BatteryStatus：示例状态 · Chemistry：示例值 · DesignCapacity：1024 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.hardware.battery`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-072"></a>
### 072 电源计划与休眠配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.power.configuration |
| source | pcscope.power.configuration |
| 分类 | power |
| 插件目录分组 | storage |
| 渠道／接口 | CIM root/cimv2/power: Win32_PowerPlan；HKLM\SYSTEM\CurrentControlSet\Control\Power |
| 采集方式（目录原文） | 查询激活计划和指定休眠/待机配置；不切换计划 |

**实现位置**：[plugins/powershell-adapter/domains/storage.ps1:8](../plugins/powershell-adapter/domains/storage.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ElementName | 元素显示名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InstanceID | 实例标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| IsActive | 是否为活动分区 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.power.configuration",
  "id": "example-object",
  "item_name": "电源计划与休眠配置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    },
    "Description": "示例值",
    "ElementName": "示例值",
    "InstanceID": "示例值",
    "IsActive": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 · 说明：示例值 · ElementName：示例值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.power.configuration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-073"></a>
### 073 服务详细配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.service-configuration |
| source | pcscope.startup.service-configuration |
| 分类 | startup |
| 插件目录分组 | startup |
| 渠道／接口 | 注册表 · HKLM\SYSTEM\CurrentControlSet\Services |
| 采集方式（目录原文） | 递归最多 2 层，读取启动类型、账户、依赖与 ServiceDll 等指定值 |

**实现位置**：[plugins/powershell-adapter/domains/startup.ps1:1](../plugins/powershell-adapter/domains/startup.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.service-configuration",
  "id": "example-object",
  "item_name": "服务详细配置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 运行与启动配置：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.service-configuration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-074"></a>
### 074 会话与登录初始化入口

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.session-hooks |
| source | pcscope.startup.session-hooks |
| 分类 | startup |
| 插件目录分组 | startup |
| 渠道／接口 | HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon；HKLM\SYSTEM\CurrentControlSet\Control\Session Manager；HKLM/HKCU\Software\Microsoft\Active Setup\Installed Components、Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run |
| 采集方式（目录原文） | 注册表快照，读取 Shell/Userinit/BootExecute 等；非白名单内容仅摘要 |

**实现位置**：[plugins/powershell-adapter/domains/startup.ps1:2](../plugins/powershell-adapter/domains/startup.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.session-hooks",
  "id": "example-object",
  "item_name": "会话与登录初始化入口",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 运行与启动配置：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.session-hooks`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-075"></a>
### 075 IFEO、AppInit 与 AppCert 入口

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.advanced-extensions |
| source | pcscope.startup.advanced-extensions |
| 分类 | startup |
| 插件目录分组 | startup |
| 渠道／接口 | 注册表 · HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options、SilentProcessExit、Windows；HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\AppCertDlls |
| 采集方式（目录原文） | 递归最多 2 层；读取指定开关及其他值摘要，不加载扩展 |

**实现位置**：[plugins/powershell-adapter/domains/startup.ps1:3](../plugins/powershell-adapter/domains/startup.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.advanced-extensions",
  "id": "example-object",
  "item_name": "IFEO、AppInit 与 AppCert 入口",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 运行与启动配置：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.advanced-extensions`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-076"></a>
### 076 WMI 永久订阅过滤器

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.wmi-filters |
| source | pcscope.startup.wmi-filters |
| 分类 | startup |
| 插件目录分组 | startup |
| 渠道／接口 | CIM · root/subscription: __EventFilter |
| 采集方式（目录原文） | 读取名称、命名空间、查询语言与查询摘要；这是订阅配置清单，不是主动订阅事件 |

**实现位置**：[plugins/powershell-adapter/domains/startup.ps1:4](../plugins/powershell-adapter/domains/startup.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| event_namespace | WMI 事件命名空间 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| query | WMI 查询 | WMI 查询 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| query_language | WMI 查询语言 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties | 来源提供的完整属性（敏感原值加密保留） | 来源提供的完整属性（敏感原值加密保留） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.wmi-filters",
  "id": "example-object",
  "item_name": "WMI 永久订阅过滤器",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "event_namespace": "示例值",
    "name": "Example",
    "query": "示例值",
    "query_language": "示例值",
    "source_properties": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | event_namespace：示例值 · WMI 查询：示例值 · query_language：示例值 |
| 默认属性组 | 运行与启动配置：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.wmi-filters`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-077"></a>
### 077 WMI 永久订阅消费者

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.wmi-consumers |
| source | pcscope.startup.wmi-consumers |
| 分类 | startup |
| 插件目录分组 | startup |
| 渠道／接口 | CIM · root/subscription: __EventConsumer |
| 采集方式（目录原文） | 读取类、脚本引擎、程序路径；命令/脚本只保存摘要 |

**实现位置**：[plugins/powershell-adapter/domains/startup.ps1:5](../plugins/powershell-adapter/domains/startup.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| class | 设备类别 | 设备类别 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| command | 启动命令 | 启动命令 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| executable_path | 可执行程序路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| script | 脚本内容或受保护的脚本字段 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| script_engine | 脚本引擎 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties | 来源提供的完整属性（敏感原值加密保留） | 来源提供的完整属性（敏感原值加密保留） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.wmi-consumers",
  "id": "example-object",
  "item_name": "WMI 永久订阅消费者",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "class": "示例值",
    "command": "示例值",
    "executable_path": "C:\\Example\\item",
    "name": "Example",
    "script": "示例值",
    "script_engine": "示例值",
    "source_properties": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 设备类别：示例值 · 启动命令：示例值 · executable_path：C:\Example\item |
| 默认属性组 | 运行与启动配置：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.wmi-consumers`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-078"></a>
### 078 WMI 过滤器与消费者绑定

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.wmi-bindings |
| source | pcscope.startup.wmi-bindings |
| 分类 | startup |
| 插件目录分组 | startup |
| 渠道／接口 | CIM · root/subscription: __FilterToConsumerBinding |
| 采集方式（目录原文） | 枚举 Filter/Consumer 关联与安全上下文配置 |

**实现位置**：[plugins/powershell-adapter/domains/startup.ps1:6](../plugins/powershell-adapter/domains/startup.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Consumer | WMI 消费者引用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DeliverSynchronously | 是否同步交付事件 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Filter | WMI 过滤器引用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| MaintainSecurityContext | 是否保持安全上下文 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SlowDownProviders | 是否要求提供程序降低事件产生速度 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.wmi-bindings",
  "id": "example-object",
  "item_name": "WMI 过滤器与消费者绑定",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "Consumer": "示例值",
    "DeliverSynchronously": "示例值",
    "Filter": "示例值",
    "MaintainSecurityContext": "示例值",
    "SlowDownProviders": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | Consumer：示例值 · DeliverSynchronously：示例值 · Filter：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.wmi-bindings`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-079"></a>
### 079 计划任务执行记录与条件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.task-details |
| source | pcscope.startup.task-details |
| 分类 | startup |
| 插件目录分组 | startup |
| 渠道／接口 | PowerShell · Get-ScheduledTask / Get-ScheduledTaskInfo |
| 采集方式（目录原文） | 读取最近/下次运行、返回码、漏跑次数、触发器、条件、执行目标；参数仅摘要 |

**实现位置**：[plugins/powershell-adapter/domains/startup.ps1:7](../plugins/powershell-adapter/domains/startup.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Author | 作者登记值 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Source | 登记来源 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| URI | 任务资源标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Version | 版本 | 版本 | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| actions | 执行操作 | 执行操作 | [{"Execute":"C:\\Example\\app.exe"}] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| conditions | 计划任务运行条件 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| last_result | 最近执行结果代码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| last_run | 最近执行时间 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| missed_runs | 错过执行次数 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| next_run | 下次计划执行时间 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| principal | 任务运行身份和权限配置 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| registration | 任务登记元数据 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |
| triggers | 任务触发器列表 | 保留原名（无专用中文标签） | ["示例成员"] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.task-details",
  "id": "example-object",
  "item_name": "计划任务执行记录与条件",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "Author": "示例值",
    "Description": "示例值",
    "Source": "示例值",
    "URI": "示例值",
    "Version": "1.0.0",
    "actions": [
      {
        "Execute": "C:\\Example\\app.exe"
      }
    ],
    "conditions": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 版本：1.0.0 · Author：示例值 · 说明：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.task-details`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-080"></a>
### 080 环境变量与默认文件关联

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.environment.configuration |
| source | pcscope.environment.configuration |
| 分类 | environment |
| 插件目录分组 | startup |
| 渠道／接口 | HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment；HKCU\Environment、Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts、Software\Microsoft\Windows\Shell\Associations\UrlAssociations |
| 采集方式（目录原文） | 注册表快照，读取指定环境变量及 ProgId/Application；其他内容摘要 |

**实现位置**：[plugins/powershell-adapter/domains/startup.ps1:8](../plugins/powershell-adapter/domains/startup.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.environment.configuration",
  "id": "example-object",
  "item_name": "环境变量与默认文件关联",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.environment.configuration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-081"></a>
### 081 打印机清单

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.desktop.printing |
| source | pcscope.desktop.printing |
| 分类 | desktop |
| 插件目录分组 | desktop |
| 渠道／接口 | PowerShell · Get-Printer |
| 采集方式（目录原文） | 枚举打印机、共享、驱动、端口与状态；依赖打印服务 |

**实现位置**：[plugins/powershell-adapter/domains/desktop.ps1:1](../plugins/powershell-adapter/domains/desktop.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| DriverName | 驱动名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| PortName | 端口名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PrinterStatus | 打印机状态代码 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Published | 是否发布到目录服务 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| RenderingMode | 打印渲染模式 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ShareName | 共享名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Shared | 是否共享 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Type | 来源定义的类型；具体含义取决于本项对象 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.desktop.printing",
  "id": "example-object",
  "item_name": "打印机清单",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "DriverName": "示例值",
    "Name": "Example",
    "PortName": "示例值",
    "PrinterStatus": "示例状态",
    "Published": "示例值",
    "RenderingMode": "示例值",
    "ShareName": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | DriverName：示例值 · PortName：示例值 · PrinterStatus：示例状态 |
| 默认属性组 | 设备与系统状态：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.desktop.printing`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-082"></a>
### 082 打印驱动

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.desktop.printer-drivers |
| source | pcscope.desktop.printer-drivers |
| 分类 | drivers |
| 插件目录分组 | desktop |
| 渠道／接口 | PowerShell · Get-PrinterDriver |
| 采集方式（目录原文） | 读取名称、环境、制造商、驱动/INF/配置文件路径 |

**实现位置**：[plugins/powershell-adapter/domains/desktop.ps1:2](../plugins/powershell-adapter/domains/desktop.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| ConfigFile | 配置文件路径 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DataFile | 驱动数据文件路径 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DriverPath | 驱动文件路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| InfPath | 驱动安装信息文件路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| MajorVersion | 主版本号 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Manufacturer | 制造商 | 制造商 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| PrinterEnvironment | 打印驱动适用的系统环境 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.desktop.printer-drivers",
  "id": "example-object",
  "item_name": "打印驱动",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "ConfigFile": "示例值",
    "DataFile": "示例值",
    "DriverPath": "C:\\Example\\item",
    "InfPath": "C:\\Example\\item",
    "MajorVersion": "1.0.0",
    "Manufacturer": "示例值",
    "Name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 制造商：示例值 · ConfigFile：示例值 · DataFile：示例值 |
| 默认属性组 | 设备与系统状态：名称、制造商 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.desktop.printer-drivers`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-083"></a>
### 083 打印端口

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.desktop.printer-ports |
| source | pcscope.desktop.printer-ports |
| 分类 | desktop |
| 插件目录分组 | desktop |
| 渠道／接口 | PowerShell · Get-PrinterPort |
| 采集方式（目录原文） | 读取端口类型、主机地址、协议、端口号与 SNMP 配置 |

**实现位置**：[plugins/powershell-adapter/domains/desktop.ps1:3](../plugins/powershell-adapter/domains/desktop.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| Description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| PortMonitor | 打印端口监视器 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PortNumber | 端口号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| PrinterHostAddress | 打印设备主机地址 | 保留原名（无专用中文标签） | "192.0.2.10" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Protocol | 协议名称或协议编号，保持来源格式 | 保留原名（无专用中文标签） | "6" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SNMPEnabled | 是否启用 SNMP | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.desktop.printer-ports",
  "id": "example-object",
  "item_name": "打印端口",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "Description": "示例值",
    "Name": "Example",
    "PortMonitor": "示例值",
    "PortNumber": "示例值",
    "PrinterHostAddress": "192.0.2.10",
    "Protocol": "6",
    "SNMPEnabled": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 说明：示例值 · PortMonitor：示例值 · PortNumber：示例值 |
| 默认属性组 | 设备与系统状态：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.desktop.printer-ports`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-084"></a>
### 084 打印作业队列

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.desktop.print-jobs |
| source | pcscope.desktop.print-jobs |
| 分类 | desktop |
| 插件目录分组 | desktop |
| 渠道／接口 | PowerShell · Get-Printer / Get-PrintJob -PrinterName |
| 采集方式（目录原文） | 逐打印机读取队列、提交时间、大小、页数和状态；不保存文档名/正文 |

**实现位置**：[plugins/powershell-adapter/domains/desktop.ps1:4](../plugins/powershell-adapter/domains/desktop.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| content_exclusion_reason | 文档内容的采集范围说明 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| document_content_collected | 是否采集文档正文 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| document_name | 文档名称 | 文档名称 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| id | 标识 | 标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| pages_printed | 已打印页数 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| printer | 打印机名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| size | 大小 | 大小 | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| status | 状态 | 状态 | "示例状态" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| submitted_at | 作业提交时间 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| total_pages | 作业总页数 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.desktop.print-jobs",
  "id": "example-object",
  "item_name": "打印作业队列",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "content_exclusion_reason": "示例值",
    "document_content_collected": false,
    "document_name": "示例值",
    "id": "示例值",
    "pages_printed": "示例值",
    "printer": "示例值",
    "size": 1024
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | content_exclusion_reason：示例值 · 文档名称：示例值 · 标识：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.desktop.print-jobs`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-085"></a>
### 085 键盘、输入法与过滤驱动配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.desktop.input |
| source | pcscope.desktop.input |
| 分类 | desktop |
| 插件目录分组 | desktop |
| 渠道／接口 | HKCU\Keyboard Layout；HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts；HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e96b-e325-11ce-bfc1-08002be10318} |
| 采集方式（目录原文） | 注册表快照，读取布局文件、IME 文件和 UpperFilters/LowerFilters 等 |

**实现位置**：[plugins/powershell-adapter/domains/desktop.ps1:5](../plugins/powershell-adapter/domains/desktop.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.desktop.input",
  "id": "example-object",
  "item_name": "键盘、输入法与过滤驱动配置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.desktop.input`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-086"></a>
### 086 剪贴板设置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.desktop.clipboard |
| source | pcscope.desktop.clipboard |
| 分类 | desktop |
| 插件目录分组 | desktop |
| 渠道／接口 | HKCU\Software\Microsoft\Clipboard；HKLM\Software\Policies\Microsoft\Windows\System |
| 采集方式（目录原文） | 读取历史/同步开关与策略；不读取剪贴板正文 |

**实现位置**：[plugins/powershell-adapter/domains/desktop.ps1:6](../plugins/powershell-adapter/domains/desktop.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.desktop.clipboard",
  "id": "example-object",
  "item_name": "剪贴板设置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.desktop.clipboard`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-087"></a>
### 087 桌面与资源管理器设置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.desktop.shell-settings |
| source | pcscope.desktop.shell-settings |
| 分类 | desktop |
| 插件目录分组 | desktop |
| 渠道／接口 | HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced、User Shell Folders；HKCU\Control Panel\Desktop |
| 采集方式（目录原文） | 注册表快照，读取指定显示、壁纸与系统文件夹配置 |

**实现位置**：[plugins/powershell-adapter/domains/desktop.ps1:7](../plugins/powershell-adapter/domains/desktop.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.desktop.shell-settings",
  "id": "example-object",
  "item_name": "桌面与资源管理器设置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.desktop.shell-settings`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-088"></a>
### 088 桌面与开始菜单快捷方式

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.desktop.shortcuts |
| source | pcscope.desktop.shortcuts |
| 分类 | desktop |
| 插件目录分组 | desktop |
| 渠道／接口 | COM · WScript.Shell.CreateShortcut；Environment.GetFolderPath(DesktopDirectory / CommonDesktopDirectory / StartMenu / CommonStartMenu) |
| 采集方式（目录原文） | 只读取现有 .lnk 目标、工作目录、图标与热键；不 Save、不执行目标；参数摘要 |

**实现位置**：[plugins/powershell-adapter/domains/desktop.ps1:8](../plugins/powershell-adapter/domains/desktop.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| arguments | 运行参数 | 运行参数 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| attributes | 文件或对象属性标志 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hotkey | 快捷方式热键 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| icon | 图标位置 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| reason | 原因 | 原因 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| reparse_target_followed | 是否进入重解析点指向的目标 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| target | 快捷方式或操作目标 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| window_style | 快捷方式窗口显示方式 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| working_directory | 工作目录 | 工作目录 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.desktop.shortcuts",
  "id": "example-object",
  "item_name": "桌面与开始菜单快捷方式",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "arguments": "示例值",
    "attributes": "示例值",
    "description": "示例值",
    "hotkey": "示例值",
    "icon": "示例值",
    "path": "C:\\Example\\item",
    "reason": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | 运行参数：示例值 · attributes：示例值 · 说明：示例值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.desktop.shortcuts`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-089"></a>
### 089 Hyper-V 虚拟机

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.virtualization.hyperv-vms |
| source | pcscope.virtualization.hyperv-vms |
| 分类 | virtualization |
| 插件目录分组 | virtualization |
| 渠道／接口 | PowerShell · Get-VM |
| 采集方式（目录原文） | 读取虚拟机 ID、状态、代数、CPU、内存与启动行为；需 Hyper-V 模块和访问权限 |

**实现位置**：[plugins/powershell-adapter/domains/virtualization.ps1:1](../plugins/powershell-adapter/domains/virtualization.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AutomaticStartAction | 虚拟机自动启动动作 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| AutomaticStopAction | 虚拟机自动停止动作 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| CheckpointType | 检查点类型配置 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ConfigurationLocation | 虚拟机配置位置 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DynamicMemoryEnabled | 是否启用动态内存 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Generation | 虚拟机代数 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Id | 标识 | 标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| MemoryMaximum | 最大内存 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| MemoryMinimum | 最小内存 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| MemoryStartup | 启动内存 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| ProcessorCount | 虚拟处理器数量 | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| State | 状态 | 状态 | "示例状态" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| Version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.virtualization.hyperv-vms",
  "id": "example-object",
  "item_name": "Hyper-V 虚拟机",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AutomaticStartAction": "示例值",
    "AutomaticStopAction": "示例值",
    "CheckpointType": "示例值",
    "ConfigurationLocation": "C:\\Example\\item",
    "DynamicMemoryEnabled": "示例值",
    "Generation": "示例值",
    "Id": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | AutomaticStartAction：示例值 · AutomaticStopAction：示例值 · CheckpointType：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.virtualization.hyperv-vms`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-090"></a>
### 090 Hyper-V 虚拟交换机

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.virtualization.hyperv-switches |
| source | pcscope.virtualization.hyperv-switches |
| 分类 | virtualization |
| 插件目录分组 | virtualization |
| 渠道／接口 | PowerShell · Get-VMSwitch |
| 采集方式（目录原文） | 枚举交换机类型、网卡描述与管理系统访问配置 |

**实现位置**：[plugins/powershell-adapter/domains/virtualization.ps1:2](../plugins/powershell-adapter/domains/virtualization.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| AllowManagementOS | 是否允许管理操作系统使用交换机 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Id | 标识 | 标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| NetAdapterInterfaceDescription | 物理网卡接口说明 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SwitchType | 虚拟交换机类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.virtualization.hyperv-switches",
  "id": "example-object",
  "item_name": "Hyper-V 虚拟交换机",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "AllowManagementOS": "示例值",
    "Id": "示例值",
    "Name": "Example",
    "NetAdapterInterfaceDescription": "示例值",
    "SwitchType": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | AllowManagementOS：示例值 · 标识：示例值 · NetAdapterInterfaceDescription：示例值 |
| 默认属性组 | 设备与系统状态：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.virtualization.hyperv-switches`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-091"></a>
### 091 虚拟机磁盘连接

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.virtualization.hyperv-disks |
| source | pcscope.virtualization.hyperv-disks |
| 分类 | virtualization |
| 插件目录分组 | virtualization |
| 渠道／接口 | PowerShell · Get-VM / Get-VMHardDiskDrive |
| 采集方式（目录原文） | 逐虚拟机读取虚拟磁盘路径与控制器位置；不读取磁盘内容 |

**实现位置**：[plugins/powershell-adapter/domains/virtualization.ps1:3](../plugins/powershell-adapter/domains/virtualization.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| ControllerLocation | 虚拟磁盘在控制器上的位置 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ControllerNumber | 控制器编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ControllerType | 控制器类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| DiskNumber | 磁盘编号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| VMId | 虚拟机标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| VMName | 虚拟机名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.virtualization.hyperv-disks",
  "id": "example-object",
  "item_name": "虚拟机磁盘连接",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "ControllerLocation": "C:\\Example\\item",
    "ControllerNumber": "示例值",
    "ControllerType": "示例值",
    "DiskNumber": "示例值",
    "Path": "C:\\Example\\item",
    "VMId": "示例值",
    "VMName": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ControllerLocation：C:\Example\item · ControllerNumber：示例值 · ControllerType：示例值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.virtualization.hyperv-disks`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-092"></a>
### 092 虚拟机检查点

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.virtualization.hyperv-checkpoints |
| source | pcscope.virtualization.hyperv-checkpoints |
| 分类 | virtualization |
| 插件目录分组 | virtualization |
| 渠道／接口 | PowerShell · Get-VM / Get-VMSnapshot |
| 采集方式（目录原文） | 枚举检查点 ID、建立时间、类型与父检查点；不创建检查点 |

**实现位置**：[plugins/powershell-adapter/domains/virtualization.ps1:4](../plugins/powershell-adapter/domains/virtualization.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| CreationTime | 来源提供的创建时间 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Id | 标识 | 标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| Name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| ParentSnapshotId | 父检查点标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| SnapshotType | 检查点类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| VMId | 虚拟机标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_properties.* | CIM 返回的完整动态属性树；具体字段随类和系统版本变化 | 保留原名（无专用中文标签） | {"ExampleProperty":"示例值"} | 嵌套／动态内容：全部采集属性及原始数据 |

**动态扩展**：`Fields` 对 CIM 对象还调用 `Public-CimFields` 保存 `source_properties`；对非 CIM 对象只保存所列字段。不能将此项写成“所有 API 属性一定采集”。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.virtualization.hyperv-checkpoints",
  "id": "example-object",
  "item_name": "虚拟机检查点",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "CreationTime": "2026-09-11T09:00:00Z",
    "Id": "示例值",
    "Name": "Example",
    "ParentSnapshotId": "示例值",
    "SnapshotType": "示例值",
    "VMId": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | CreationTime：2026-09-11 17:00:00（本机时间） · 标识：示例值 · ParentSnapshotId：示例值 |
| 默认属性组 | 设备与系统状态：名称 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.virtualization.hyperv-checkpoints`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-093"></a>
### 093 WSL 发行版登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.virtualization.wsl-registration |
| source | pcscope.virtualization.wsl-registration |
| 分类 | virtualization |
| 插件目录分组 | virtualization |
| 渠道／接口 | 注册表 · HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss |
| 采集方式（目录原文） | 读取下一层发行版名称、版本、位置与默认 UID；不启动 WSL |

**实现位置**：[plugins/powershell-adapter/domains/virtualization.ps1:5](../plugins/powershell-adapter/domains/virtualization.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.virtualization.wsl-registration",
  "id": "example-object",
  "item_name": "WSL 发行版登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 设备与系统状态：路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.virtualization.wsl-registration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-094"></a>
### 094 Chrome、Edge 与 Firefox 策略

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.apps.browser-policies |
| source | pcscope.apps.browser-policies |
| 分类 | apps |
| 插件目录分组 | applications |
| 渠道／接口 | 注册表 · HKLM/HKCU\Software\Policies\Google\Chrome、Microsoft\Edge、Mozilla\Firefox |
| 采集方式（目录原文） | 递归最多 2 层；指定策略原值与其他内容摘要 |

**实现位置**：[plugins/powershell-adapter/domains/applications.ps1:1](../plugins/powershell-adapter/domains/applications.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.apps.browser-policies",
  "id": "example-object",
  "item_name": "Chrome、Edge 与 Firefox 策略",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.apps.browser-policies`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-095"></a>
### 095 VS Code 扩展清单

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.apps.ide-extensions |
| source | pcscope.apps.ide-extensions |
| 分类 | apps |
| 插件目录分组 | applications |
| 渠道／接口 | 文件系统 · %USERPROFILE%\.vscode\extensions、.vscode-insiders\extensions 下 package.json |
| 采集方式（目录原文） | 枚举扩展目录并解析包清单；读取名称、发布者、版本和引擎约束 |

**实现位置**：[plugins/powershell-adapter/domains/applications.ps1:2](../plugins/powershell-adapter/domains/applications.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| engines | 扩展或包声明的运行引擎及版本约束 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| publisher | 发布者 | 发布者 | "示例值" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.apps.ide-extensions",
  "id": "example-object",
  "item_name": "VS Code 扩展清单",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "engines": "示例值",
    "name": "Example",
    "publisher": "示例值",
    "version": "1.0.0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 版本：1.0.0 · 发布者：示例值 · engines：示例值 |
| 默认属性组 | 软件登记：发布者、版本 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.apps.ide-extensions`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-096"></a>
### 096 .NET 运行时登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.apps.dotnet-runtimes |
| source | pcscope.apps.dotnet-runtimes |
| 分类 | apps |
| 插件目录分组 | applications |
| 渠道／接口 | 注册表 · HKLM\SOFTWARE\dotnet\Setup\InstalledVersions；HKLM\SOFTWARE\Microsoft\NET Framework Setup\NDP |
| 采集方式（目录原文） | 读取 32/64 位视图，分别递归最多 4/3 层 |

**实现位置**：[plugins/powershell-adapter/domains/applications.ps1:3](../plugins/powershell-adapter/domains/applications.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.apps.dotnet-runtimes",
  "id": "example-object",
  "item_name": ".NET 运行时登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.apps.dotnet-runtimes`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-097"></a>
### 097 全局 npm 包清单

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.apps.npm-global-packages |
| source | pcscope.apps.npm-global-packages |
| 分类 | apps |
| 插件目录分组 | applications |
| 渠道／接口 | 文件系统 · %APPDATA%\npm\node_modules；%ProgramFiles%\nodejs\node_modules 下 package.json |
| 采集方式（目录原文） | 解析普通包与 scope 包清单；不运行 npm，自定义 prefix 不覆盖 |

**实现位置**：[plugins/powershell-adapter/domains/applications.ps1:4](../plugins/powershell-adapter/domains/applications.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.apps.npm-global-packages",
  "id": "example-object",
  "item_name": "全局 npm 包清单",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "name": "Example",
    "path": "C:\\Example\\item",
    "version": "1.0.0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | 版本：1.0.0 |
| 默认属性组 | 软件登记：版本 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.apps.npm-global-packages`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-098"></a>
### 098 OneDrive 账户与同步登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.apps.onedrive-registration |
| source | pcscope.apps.onedrive-registration |
| 分类 | apps |
| 插件目录分组 | applications |
| 渠道／接口 | HKCU\Software\Microsoft\OneDrive\Accounts、Software\SyncEngines\Providers\OneDrive；HKLM\Software\Policies\Microsoft\OneDrive |
| 采集方式（目录原文） | 注册表快照，读取指定账户、挂载与策略字段；不访问云端 |

**实现位置**：[plugins/powershell-adapter/domains/applications.ps1:11](../plugins/powershell-adapter/domains/applications.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**注册表实际行为**：指定键内所有登记值按名称保留 kind/value，`allowed` 参数不再决定丢弃其他值；遍历深度和路径仍有限。目录中旧的“其他值只保存摘要”措辞应以当前保护实现为准。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.apps.onedrive-registration",
  "id": "example-object",
  "item_name": "OneDrive 账户与同步登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example / item |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.apps.onedrive-registration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-099"></a>
### 099 Chrome / Edge 扩展

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.apps.chromium-extensions |
| source | pcscope.apps.chromium-extensions |
| 分类 | apps |
| 插件目录分组 | applications |
| 渠道／接口 | 文件系统 · %LOCALAPPDATA%\Google\Chrome\User Data、Microsoft\Edge\User Data 下 Default / Profile N\Extensions\{ID}\{版本}\manifest.json |
| 采集方式（目录原文） | 解析名称、版本与权限清单；不判定是否启用，拒绝重解析目录 |

**实现位置**：[plugins/powershell-adapter/domains/applications.ps1:12](../plugins/powershell-adapter/domains/applications.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| browser | 浏览器类型 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| description | 说明 | 说明 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| disable_reasons | 浏览器记录的扩展禁用原因代码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| extension_id | 扩展唯一标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| host_permissions | 扩展声明的主机访问权限 | 保留原名（无专用中文标签） | ["示例成员"] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| manifest_version | 扩展清单格式版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| optional_permissions | 可选权限声明 | 保留原名（无专用中文标签） | ["示例成员"] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| permissions | 权限声明列表 | 保留原名（无专用中文标签） | ["示例成员"] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| preferences_status | 扩展偏好配置的读取状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| profile | 用户配置文件或配置文件名称；具体类型由来源决定 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| recorded_state | 浏览器登记的扩展状态代码 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| registered_path | 登记的扩展目录 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.apps.chromium-extensions",
  "id": "example-object",
  "item_name": "Chrome / Edge 扩展",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "browser": "示例值",
    "description": "示例值",
    "disable_reasons": "示例值",
    "extension_id": "示例值",
    "host_permissions": [
      "示例成员"
    ],
    "manifest_version": "1.0.0",
    "name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | browser：示例值 · 说明：示例值 · disable_reasons：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.apps.chromium-extensions`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-100"></a>
### 100 Firefox 扩展

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.apps.firefox-extensions |
| source | pcscope.apps.firefox-extensions |
| 分类 | apps |
| 插件目录分组 | applications |
| 渠道／接口 | 文件系统 · %APPDATA%\Mozilla\Firefox\Profiles\{配置文件}\extensions.json |
| 采集方式（目录原文） | 解析插件版本、启用/禁用、签名状态与安装/更新时间字段 |

**实现位置**：[plugins/powershell-adapter/domains/applications.ps1:45](../plugins/powershell-adapter/domains/applications.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| active | 来源报告是否处于活动状态 | 保留原名（无专用中文标签） | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| app_disabled | 是否由应用程序禁用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| id | 标识 | 标识 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| install_date | 登记安装日期 | 登记安装日期 | "2026-09-11T09:00:00Z" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| profile | 用户配置文件或配置文件名称；具体类型由来源决定 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| signed_state | 扩展签名状态代码 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| type | 来源定义的类型；具体含义取决于本项对象 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| update_date | 来源登记的更新时间，原始时间格式按来源保留 | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| user_disabled | 是否被用户禁用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.apps.firefox-extensions",
  "id": "example-object",
  "item_name": "Firefox 扩展",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "active": true,
    "app_disabled": "示例值",
    "id": "示例值",
    "install_date": "2026-09-11T09:00:00Z",
    "profile": "示例值",
    "signed_state": "示例状态",
    "type": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | active：是 · app_disabled：示例值 · 标识：示例值 |
| 默认属性组 | 软件登记：登记安装日期 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.apps.firefox-extensions`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-101"></a>
### 101 指定范围配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.collector.scoped-configuration |
| source | pcscope.collector.scoped-configuration |
| 分类 | collector |
| 插件目录分组 | scoped |
| 渠道／接口 | 插件配置文件列表 + 本地 JSON · collector-options.json |
| 采集方式（目录原文） | 文件列表由后台持久保存并传给采集器；默认为空；其他可选运行时读取 JSON |

**实现位置**：[plugins/powershell-adapter/domains/scoped.ps1:51](../plugins/powershell-adapter/domains/scoped.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| content_versions | 是否保存文件内容版本的能力配置 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| docker_metadata | 是否启用 Docker 元数据采集 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| file_roots | 配置的文件根目录 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hash_files | 是否启用文件哈希 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| max_files_per_root | 每个根目录允许扫描的条目上限 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| max_hash_bytes | 允许计算哈希的文件大小上限 | 保留原名（无专用中文标签） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| mihomo_metadata | 是否启用 Mihomo 元数据采集 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| module_process_names | 指定采集模块列表的进程名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| portable_roots | 便携软件扫描目录 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| watch_files | 显式文件监测名单 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.collector.scoped-configuration",
  "id": "example-object",
  "item_name": "指定范围配置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "content_versions": "1.0.0",
    "docker_metadata": "示例值",
    "file_roots": "示例值",
    "hash_files": "示例值",
    "max_files_per_root": "示例值",
    "max_hash_bytes": 1024,
    "mihomo_metadata": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | content_versions：1.0.0 · docker_metadata：示例值 · file_roots：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.collector.scoped-configuration`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-102"></a>
### 102 指定文件、ACL 与 ADS

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.files.metadata-security |
| source | pcscope.files.metadata-security |
| 分类 | files |
| 插件目录分组 | scoped |
| 渠道／接口 | Get-Item -LiteralPath；FileSystemInfo.GetAccessControl；Get-Item -Stream *；SHA256 |
| 采集方式（目录原文） | 仅逐项读取插件文件列表，忽略旧 file_roots，不扫描相邻文件；读取时间、属性、Owner/DACL、流名称/大小；hash_files 开启才计算限额内哈希，不保存正文 |

**实现位置**：[plugins/powershell-adapter/domains/scoped.ps1:75](../plugins/powershell-adapter/domains/scoped.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| attributes | 文件或对象属性标志 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| content_stored | 是否保存文件正文 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| creation_utc | 文件系统创建时间 UTC | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| dacl_sddl | 所有者和 DACL 的 SDDL 表示 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| directory | 该条目是否为目录 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| exists | 指定路径在采集时是否存在 | 保留原名（无专用中文标签） | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| owner | 所有者 | 所有者 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| reparse_point | 是否为文件系统重解析点 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| security_status | 安全属性读取状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| size | 大小 | 大小 | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| streams | 文件数据流元数据列表 | 保留原名（无专用中文标签） | [{"name":":$DATA","size":1024}] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.files.metadata-security",
  "id": "example-object",
  "item_name": "指定文件、ACL 与 ADS",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "attributes": "示例值",
    "content_stored": false,
    "creation_utc": "2026-09-11T09:00:00Z",
    "dacl_sddl": "示例值",
    "directory": false,
    "exists": true,
    "hash_status": "示例状态"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | attributes：示例值 · content_stored：否 · creation_utc：2026-09-11 17:00:00（本机时间） |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.files.metadata-security`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-103"></a>
### 103 便携软件文件登记

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.software.portable |
| source | pcscope.software.portable |
| 分类 | software |
| 插件目录分组 | scoped |
| 渠道／接口 | 文件系统 · portable_roots；.NET FileVersionInfo.GetVersionInfo |
| 采集方式（目录原文） | 扫描 EXE/DLL/MSI 的版本、公司及文件元数据；共享 ACL/ADS/可选哈希流程，不验证签名 |

**实现位置**：[plugins/powershell-adapter/domains/scoped.ps1:76](../plugins/powershell-adapter/domains/scoped.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| attributes | 文件或对象属性标志 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| company | 公司 | 公司 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| content_stored | 是否保存文件正文 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| creation_utc | 文件系统创建时间 UTC | 保留原名（无专用中文标签） | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| dacl_sddl | 所有者和 DACL 的 SDDL 表示 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| directory | 该条目是否为目录 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| owner | 所有者 | 所有者 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| path | 路径 | 路径 | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| reparse_point | 是否为文件系统重解析点 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| security_status | 安全属性读取状态 | 保留原名（无专用中文标签） | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| signature_status | 签名检查结果 | 签名检查结果 | "示例状态" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| size | 大小 | 大小 | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| streams | 文件数据流元数据列表 | 保留原名（无专用中文标签） | [{"name":":$DATA","size":1024}] | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.software.portable",
  "id": "example-object",
  "item_name": "便携软件文件登记",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "attributes": "示例值",
    "company": "示例值",
    "content_stored": false,
    "creation_utc": "2026-09-11T09:00:00Z",
    "dacl_sddl": "示例值",
    "directory": false,
    "hash_status": "示例状态"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | attributes：示例值 · 公司：示例值 · content_stored：否 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.software.portable`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-104"></a>
### 104 指定进程加载模块

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.execution.modules |
| source | pcscope.execution.modules |
| 分类 | execution |
| 插件目录分组 | scoped |
| 渠道／接口 | PowerShell Get-Process；.NET Process.Modules / ProcessModule.FileVersionInfo |
| 采集方式（目录原文） | 仅按 module_process_names 指定名称查询模块路径、大小、版本；需相应进程访问权限 |

**实现位置**：[plugins/powershell-adapter/domains/scoped.ps1:78](../plugins/powershell-adapter/domains/scoped.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| company | 公司 | 公司 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| module_path | 模块文件路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| pid | 进程编号 | 进程编号 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| process_name | 进程名称 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| process_start | 进程建立时间 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| size | 大小 | 大小 | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| version | 版本 | 版本 | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.execution.modules",
  "id": "example-object",
  "item_name": "指定进程加载模块",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "company": "示例值",
    "module_path": "C:\\Example\\item",
    "pid": "示例值",
    "process_name": "示例值",
    "process_start": "示例值",
    "size": 1024,
    "version": "1.0.0"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 版本：1.0.0 · 公司：示例值 · module_path：C:\Example\item |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.execution.modules`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-105"></a>
### 105 Mihomo 本地运行配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.apps.mihomo |
| source | pcscope.apps.mihomo |
| 分类 | apps |
| 插件目录分组 | scoped |
| 渠道／接口 | HTTP API · GET http://127.0.0.1:{mihomo_port}/version、/configs；Invoke-RestMethod |
| 采集方式（目录原文） | mihomo_metadata 开启才查询；默认端口 9090，支持 PCSCOPE_MIHOMO_TOKEN；不跟随重定向、不读取节点凭据 |

**实现位置**：[plugins/powershell-adapter/domains/scoped.ps1:52](../plugins/powershell-adapter/domains/scoped.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| allow_lan | 是否允许局域网连接 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| ipv6 | IPv6 设置 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| log_level | 日志级别 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| meta | 来源返回的附加标志 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| mixed_port | 混合协议代理端口 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| mode | 来源运行或连接模式 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| port | 端口 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| socks_port | SOCKS 代理端口 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| tun_enabled | TUN 是否启用 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| version | 版本 | 版本 | "1.0.0" | 命中领域优选字段时默认显示；其他内容仍完整保留 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.apps.mihomo",
  "id": "example-object",
  "item_name": "Mihomo 本地运行配置",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "allow_lan": "示例值",
    "ipv6": "示例值",
    "log_level": "示例值",
    "meta": "示例值",
    "mixed_port": "示例值",
    "mode": "示例值",
    "port": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | allow_lan：示例值 · ipv6：示例值 · log_level：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.apps.mihomo`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-106"></a>
### 106 Docker 容器、镜像、卷与网络

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.virtualization.docker |
| source | pcscope.virtualization.docker |
| 分类 | virtualization |
| 插件目录分组 | scoped |
| 渠道／接口 | Docker Desktop docker.exe --host npipe:////./pipe/docker_engine；container ls --all / image ls / volume ls / network ls |
| 采集方式（目录原文） | docker_metadata 开启才经本机命名管道读取格式化清单；不读取日志、环境变量和容器内容 |

**实现位置**：[plugins/powershell-adapter/domains/scoped.ps1:60](../plugins/powershell-adapter/domains/scoped.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| data | 嵌套业务数据；具体结构随对象类型变化 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| data.driver | 驱动名称 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.id | 标识 | 标识 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.image | 镜像标识或程序路径，按来源区分 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.name | 名称 | 名称 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.names | 容器名称 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.repository | 容器镜像仓库 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.scope | 采集范围 | 采集范围 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.state | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.status | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| data.tag | 容器镜像标签 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| kind | 类型 | 类型 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.virtualization.docker",
  "id": "example-object",
  "item_name": "Docker 容器、镜像、卷与网络",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "data": "示例值",
    "kind": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | data：示例值 · 类型：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.virtualization.docker`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-107"></a>
### 107 固定 NTFS 卷发现

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.usn.volumes |
| source | pcscope.usn.volumes |
| 分类 | usn |
| 插件目录分组 | usn |
| 渠道／接口 | Win32 API · GetLogicalDrives / GetDriveTypeW / GetVolumeInformationW / GetVolumeNameForVolumeMountPointW |
| 采集方式（目录原文） | 只检查文件列表涉及的固定 NTFS 卷与卷身份；空列表不枚举或读取卷日志；不扫描无盘符卷 |

**实现位置**：[USN 采集器](../plugins/usn/src/main.rs)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| eligible | 是否符合本采集器条件 | 保留原名（无专用中文标签） | true | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| filesystem | 文件系统名称 | 文件系统 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| scope | 采集范围 | 采集范围 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| volume | 卷标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.usn.volumes",
  "id": "example-object",
  "item_name": "固定 NTFS 卷发现",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "eligible": true,
    "filesystem": "示例值",
    "volume": "示例值"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | example-object |
| 属性摘要 | 文件系统：示例值 · eligible：是 · volume：示例值 |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

<a id="item-108"></a>
### 108 系统与服务事件 / 服务运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.system.services |
| source | pcscope.eventlog.system |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · System；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Service Control Manager；该提供程序的事件。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `system`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Service Control Manager`；该提供程序的事件

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[服务](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.system",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "System",
    "provider": "Service Control Manager",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · Service Control Manager · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 系统与服务事件 / 服务运行事件 |

<a id="item-109"></a>
### 109 系统与服务事件 / 系统启动与关闭

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.system.lifecycle |
| source | pcscope.eventlog.system |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · System；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-Kernel-General；事件 ID 12、13。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `system`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Kernel-General`；事件编号 12、13

**例子：事件 12**

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.system",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "System",
    "provider": "Microsoft-Windows-Kernel-General",
    "event_id": 12,
    "fields": {
      "BootMode": "示例值",
      "BuildVersion": "1.0.0",
      "MajorVersion": "1.0.0",
      "MinorVersion": "1.0.0",
      "QfeVersion": "1.0.0",
      "ServiceVersion": "1.0.0",
      "StartTime": "2026-09-11T09:00:00Z"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · Microsoft-Windows-Kernel-General · 12 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 系统与服务事件 / 系统启动与关闭 |

**例子：事件 13**

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.system",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "System",
    "provider": "Microsoft-Windows-Kernel-General",
    "event_id": 13,
    "fields": {
      "StopTime": "2026-09-11T09:00:00Z"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · Microsoft-Windows-Kernel-General · 13 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 系统与服务事件 / 系统启动与关闭 |

<a id="item-110"></a>
### 110 系统与服务事件 / 电源与异常重启

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.system.power |
| source | pcscope.eventlog.system |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · System；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-Kernel-Power；该提供程序的事件。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `system`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Kernel-Power`；该提供程序的事件

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.system",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "System",
    "provider": "Microsoft-Windows-Kernel-Power",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · Microsoft-Windows-Kernel-Power · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 系统与服务事件 / 电源与异常重启 |

<a id="item-111"></a>
### 111 系统与服务事件 / 系统注册表维护

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.system.registry-maintenance |
| source | pcscope.eventlog.system |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · System；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-Kernel-General；事件 ID 11、15。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `system`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Kernel-General`；事件编号 11、15

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 11 | event.system.microsoft_windows_kernel_general.11 | 注册表事务初始化 | 系统报告注册表配置单元事务初始化信息，结果和路径见来源说明。 |
| 15 | event.system.microsoft_windows_kernel_general.15 | 注册表配置单元整理 | 系统报告注册表配置单元处理信息；具体操作由来源正文说明，不推断配置项被人工修改。 |

**例子：事件 11**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.system",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "System",
    "provider": "Microsoft-Windows-Kernel-General",
    "event_id": 11,
    "fields": {
      "ExtraString": "示例值",
      "ExtraStringLength": 1024,
      "InternalCode": "示例值",
      "RmId": "示例值",
      "Status": "0x0",
      "TmId": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 注册表事务初始化 · 事件 11 |
| 结果标签 | 不显示 |
| 摘要 | 状态：0x0 |
| 关键字段 | 状态：0x0 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 系统与服务事件 / 系统注册表维护 |

**例子：事件 15**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.system",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "System",
    "provider": "Microsoft-Windows-Kernel-General",
    "event_id": 15,
    "fields": {
      "HiveName": "示例值",
      "HiveNameLength": 1024,
      "NewSize": 1024,
      "OriginalSize": 1024
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 注册表配置单元整理 · 事件 15 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 系统与服务事件 / 系统注册表维护 |

<a id="item-112"></a>
### 112 系统与服务事件 / 系统时间同步

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.system.time-source |
| source | pcscope.eventlog.system |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · System；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-Time-Service；事件 ID 37。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `system`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Time-Service`；事件编号 37

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 37 | event.system.microsoft_windows_time_service.37 | 时间源数据有效 | 时间服务正在接收有效时间源数据，不表示此次必然调整了系统时钟。 |

**例子：事件 37**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.system",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "System",
    "provider": "Microsoft-Windows-Time-Service",
    "event_id": 37,
    "fields": {
      "TimeSource": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 时间源数据有效 · 事件 37 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 系统与服务事件 / 系统时间同步 |

<a id="item-113"></a>
### 113 系统与服务事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.system.status |
| source | pcscope.eventlog.system |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · System；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `system`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.system",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-114"></a>
### 114 系统与服务事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.system.other |
| source | pcscope.eventlog.system |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · System；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `system`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.system",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "System",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 系统与服务事件 / 其他事件 |

<a id="item-115"></a>
### 115 应用程序事件 / 软件安装事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.installer |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 MsiInstaller；该提供程序的事件。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `MsiInstaller`；该提供程序的事件

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[软件、应用包与更新](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "MsiInstaller",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · MsiInstaller · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 软件安装事件 |

<a id="item-116"></a>
### 116 应用程序事件 / 应用程序错误

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.errors |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Application Error；该提供程序的事件。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Application Error`；该提供程序的事件

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "Application Error",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · Application Error · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 应用程序错误 |

<a id="item-117"></a>
### 117 应用程序事件 / 应用程序无响应

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.hangs |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Application Hang；该提供程序的事件。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Application Hang`；该提供程序的事件

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "Application Hang",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · Application Hang · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 应用程序无响应 |

<a id="item-118"></a>
### 118 应用程序事件 / NVIDIA WMI 服务状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.nvwmi |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 NVWMI；事件 ID 3。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `NVWMI`；事件编号 3

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 3 | event.application.nvwmi.3 | NVIDIA WMI 服务进度 | NVWMI 报告服务检查点和状态进度；具体状态码保留，不猜测硬件故障。 |

**例子：事件 3**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "NVWMI",
    "event_id": 3,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | NVIDIA WMI 服务进度 · 事件 3 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / NVIDIA WMI 服务状态 |

<a id="item-119"></a>
### 119 应用程序事件 / 卷影复制服务状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.vss |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 VSS；事件 ID 8224。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `VSS`；事件编号 8224

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 8224 | event.application.vss.8224 | 卷影复制服务空闲停止 | VSS 因空闲超时关闭服务；不能仅凭此条认定备份失败。 |

**例子：事件 8224**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "VSS",
    "event_id": 8224,
    "fields": {
      "Binary": {
        "display": "******",
        "unavailable": "示例值"
      },
      "Data[1]": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 卷影复制服务空闲停止 · 事件 8224 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 卷影复制服务状态 |

<a id="item-120"></a>
### 120 应用程序事件 / Windows 错误报告

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.wer |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Windows Error Reporting；事件 ID 1001。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Windows Error Reporting`；事件编号 1001

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 1001 | event.application.windows_error_reporting.1001 | 错误报告登记 | Windows 错误报告登记了故障或诊断报告，问题类别与签名见原始字段；不能仅凭事件编号定位故障组件。 |

**例子：事件 1001**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "Windows Error Reporting",
    "event_id": 1001,
    "fields": {
      "AnalysisSymbol": "示例值",
      "AttachedFiles": "示例值",
      "Bucket": "示例值",
      "BucketType": "示例值",
      "CabGuid": "示例值",
      "CabId": "示例值",
      "EventName": "示例值",
      "HashedBucket": "示例值",
      "P1": "示例值",
      "P10": "示例值",
      "P2": "示例值",
      "P3": "示例值",
      "P4": "示例值",
      "P5": "示例值",
      "P6": "示例值",
      "P7": "示例值",
      "P8": "示例值",
      "P9": "示例值",
      "Rechecking": "示例值",
      "ReportId": "示例值",
      "ReportStatus": "示例状态",
      "Response": "示例值",
      "StorePath": "C:\\Example\\item"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 错误报告登记 · 事件 1001 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / Windows 错误报告 |

<a id="item-121"></a>
### 121 应用程序事件 / 软件保护与授权维护

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.licensing |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-Security-SPP；事件 ID 16384、16394。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-SPP`；事件编号 16384、16394

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 16384 | event.application.microsoft_windows_security_spp.16384 | 软件保护服务重启已安排 | 软件保护组件安排下次服务重启，来源原因可能为规则引擎；不等同于有人修改任务计划。 |
| 16394 | event.application.microsoft_windows_security_spp.16394 | 软件保护离线迁移完成 | 软件保护组件报告离线迁移完成，不表示整个操作系统发生迁移。 |

**例子：事件 16384**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "Microsoft-Windows-Security-SPP",
    "event_id": 16384,
    "fields": {
      "Data[1]": "示例值",
      "Data[2]": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 软件保护服务重启已安排 · 事件 16384 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 软件保护与授权维护 |

**例子：事件 16394**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "Microsoft-Windows-Security-SPP",
    "event_id": 16394,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 软件保护离线迁移完成 · 事件 16394 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 软件保护与授权维护 |

<a id="item-122"></a>
### 122 应用程序事件 / 存储优化作业

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.optimization |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-Defrag；事件 ID 258、264。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Defrag`；事件编号 258、264

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 258 | event.application.microsoft_windows_defrag.258 | 存储优化完成 | 存储优化程序报告作业完成，具体卷和优化类型见来源说明。 |
| 264 | event.application.microsoft_windows_defrag.264 | 存储优化未完成 | 存储优化程序报告未能完成；可能包含取消等原因，不直接证明磁盘损坏。 |

**例子：事件 258**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "Microsoft-Windows-Defrag",
    "event_id": 258,
    "fields": {
      "Binary": {
        "display": "******",
        "unavailable": "示例值"
      },
      "Data[1]": "示例值",
      "Data[2]": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 存储优化完成 · 事件 258 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 存储优化作业 |

**例子：事件 264**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "Microsoft-Windows-Defrag",
    "event_id": 264,
    "fields": {
      "Binary": {
        "display": "******",
        "unavailable": "示例值"
      },
      "Data[1]": "示例值",
      "Data[2]": "示例值",
      "Data[3]": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 存储优化未完成 · 事件 264 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 存储优化作业 |

<a id="item-123"></a>
### 123 应用程序事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.status |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.application",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-124"></a>
### 124 应用程序事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.application.other |
| source | pcscope.eventlog.application |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Application；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；每轮查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `application`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Application",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 应用程序事件 / 其他事件 |

<a id="item-125"></a>
### 125 Windows 安全审计 / 登录、登出与会话

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.sessions |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4624、4625、4634、4647、4800、4801、4778、4779。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4624、4625、4634、4647、4800、4801、4778、4779

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4624 | identity.logon | 登录 | 动作名称已定义；关键字段由展示模板选择。 |
| 4625 | identity.logon_failed | 登录失败 | 动作名称已定义；关键字段由展示模板选择。 |
| 4634 | identity.logoff | 注销 | 动作名称已定义；关键字段由展示模板选择。 |
| 4647 | identity.logoff_requested | 请求注销 | 动作名称已定义；关键字段由展示模板选择。 |
| 4778 | identity.session_reconnected | 会话重连 | 动作名称已定义；关键字段由展示模板选择。 |
| 4779 | identity.session_disconnected | 会话断开 | 动作名称已定义；关键字段由展示模板选择。 |
| 4800 | identity.session_locked | 会话锁定 | 动作名称已定义；关键字段由展示模板选择。 |
| 4801 | identity.session_unlocked | 会话解锁 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4624**

使用字段模板：[登录与会话](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4624,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 登录 · 事件 4624 |
| 结果标签 | 成功 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 登录、登出与会话 |

**例子：事件 4625**

使用字段模板：[登录与会话](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4625,
    "fields": {
      "Status": "0xC000006D",
      "SubStatus": "0xC000006A"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 登录失败 · 事件 4625 |
| 结果标签 | 失败 |
| 摘要 | 状态码：0xC000006D · 子状态码：0xC000006A |
| 关键字段 | 返回码：0xC000006D；状态码：0xC000006D；子状态码：0xC000006A |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 登录、登出与会话 |

**例子：事件 4634**

使用字段模板：[登录与会话](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4634,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 注销 · 事件 4634 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 登录、登出与会话 |

**例子：事件 4647**

使用字段模板：[登录与会话](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4647,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 请求注销 · 事件 4647 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 登录、登出与会话 |

**例子：事件 4800**

使用字段模板：[登录与会话](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4800,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 会话锁定 · 事件 4800 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 登录、登出与会话 |

**例子：事件 4801**

使用字段模板：[登录与会话](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4801,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 会话解锁 · 事件 4801 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 登录、登出与会话 |

**例子：事件 4778**

使用字段模板：[登录与会话](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4778,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 会话重连 · 事件 4778 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 登录、登出与会话 |

**例子：事件 4779**

使用字段模板：[登录与会话](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4779,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 会话断开 · 事件 4779 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 登录、登出与会话 |

<a id="item-126"></a>
### 126 Windows 安全审计 / 账户管理

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.accounts |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4720、4722、4725、4726、4738、4740、4767、4781。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4720、4722、4725、4726、4738、4740、4767、4781

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4720 | identity.account_created | 账户建立 | 动作名称已定义；关键字段由展示模板选择。 |
| 4722 | identity.account_enabled | 账户启用 | 动作名称已定义；关键字段由展示模板选择。 |
| 4725 | identity.account_disabled | 账户禁用 | 动作名称已定义；关键字段由展示模板选择。 |
| 4726 | identity.account_deleted | 账户删除 | 动作名称已定义；关键字段由展示模板选择。 |
| 4738 | identity.account_changed | 账户变更 | 动作名称已定义；关键字段由展示模板选择。 |
| 4740 | identity.account_locked | 账户锁定 | 动作名称已定义；关键字段由展示模板选择。 |
| 4767 | identity.account_unlocked | 账户解锁 | 动作名称已定义；关键字段由展示模板选择。 |
| 4781 | identity.account_renamed | 账户重命名 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4720**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4720,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户建立 · 事件 4720 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 账户管理 |

**例子：事件 4722**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4722,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户启用 · 事件 4722 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 账户管理 |

**例子：事件 4725**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4725,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户禁用 · 事件 4725 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 账户管理 |

**例子：事件 4726**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4726,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户删除 · 事件 4726 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 账户管理 |

**例子：事件 4738**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4738,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户变更 · 事件 4738 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 账户管理 |

**例子：事件 4740**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4740,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户锁定 · 事件 4740 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 账户管理 |

**例子：事件 4767**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4767,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户解锁 · 事件 4767 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 账户管理 |

**例子：事件 4781**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4781,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户重命名 · 事件 4781 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 账户管理 |

<a id="item-127"></a>
### 127 Windows 安全审计 / 用户组成员变更

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.groups |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4732、4733。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4732、4733

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4732 | identity.group_member_added | 组成员加入 | 动作名称已定义；关键字段由展示模板选择。 |
| 4733 | identity.group_member_removed | 组成员移除 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4732**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4732,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 组成员加入 · 事件 4732 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 用户组成员变更 |

**例子：事件 4733**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4733,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 组成员移除 · 事件 4733 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 用户组成员变更 |

<a id="item-128"></a>
### 128 Windows 安全审计 / 特殊权限分配

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.privileges |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4672。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4672

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4672 | identity.special_privileges | 特殊权限分配 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4672**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4672,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 特殊权限分配 · 事件 4672 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 特殊权限分配 |

<a id="item-129"></a>
### 129 Windows 安全审计 / 进程生命周期

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.processes |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4688、4689。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4688、4689

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4688 | execution.process_started | 进程启动 | 动作名称已定义；关键字段由展示模板选择。 |
| 4689 | execution.process_stopped | 进程退出 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4688**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4688,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 进程启动 · 事件 4688 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 进程生命周期 |

**例子：事件 4689**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4689,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 进程退出 · 事件 4689 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 进程生命周期 |

<a id="item-130"></a>
### 130 Windows 安全审计 / 文件访问与权限

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.files |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4663、4670。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4663、4670

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4663 | files.access_audit | 对象访问审计 | 动作名称已定义；关键字段由展示模板选择。 |
| 4670 | files.permissions_audit | 对象权限变更审计 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4663**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4663,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 对象访问审计 · 事件 4663 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 文件访问与权限 |

**例子：事件 4670**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4670,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 对象权限变更审计 · 事件 4670 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 文件访问与权限 |

<a id="item-131"></a>
### 131 Windows 安全审计 / 注册表操作

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.registry |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4657。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4657

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4657 | registry.value_operation | 注册表值操作 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4657**

使用字段模板：[注册表操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4657,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 注册表值操作 · 事件 4657 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 注册表操作 |

<a id="item-132"></a>
### 132 Windows 安全审计 / 服务安装

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.services |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4697。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4697

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4697 | startup.service_installed | 服务安装 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4697**

使用字段模板：[服务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4697,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 服务安装 · 事件 4697 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 服务安装 |

<a id="item-133"></a>
### 133 Windows 安全审计 / 计划任务变更

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.tasks |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4698、4699、4702。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4698、4699、4702

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4698 | startup.task_created | 任务创建 | 动作名称已定义；关键字段由展示模板选择。 |
| 4699 | startup.task_deleted | 任务删除 | 动作名称已定义；关键字段由展示模板选择。 |
| 4702 | startup.task_updated | 任务更新 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4698**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4698,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务创建 · 事件 4698 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 计划任务变更 |

**例子：事件 4699**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4699,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务删除 · 事件 4699 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 计划任务变更 |

**例子：事件 4702**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4702,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务更新 · 事件 4702 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 计划任务变更 |

<a id="item-134"></a>
### 134 Windows 安全审计 / 审计策略变更

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.audit-policy |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4719。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4719

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4719 | policy.audit_changed | 审计策略变更 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4719**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4719,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 审计策略变更 · 事件 4719 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 审计策略变更 |

<a id="item-135"></a>
### 135 Windows 安全审计 / 网络连接与共享访问

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.network |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 5140、5156、5157。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 5140、5156、5157

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 5140 | network.share_access | 共享访问 | 动作名称已定义；关键字段由展示模板选择。 |
| 5156 | network.connection_allowed | 连接允许 | 动作名称已定义；关键字段由展示模板选择。 |
| 5157 | network.connection_blocked | 连接阻止 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 5140**

使用字段模板：[网络、DNS 与共享](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5140,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 共享访问 · 事件 5140 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 网络连接与共享访问 |

**例子：事件 5156**

使用字段模板：[网络、DNS 与共享](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5156,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 连接允许 · 事件 5156 |
| 结果标签 | 已允许 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 网络连接与共享访问 |

**例子：事件 5157**

使用字段模板：[网络、DNS 与共享](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5157,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 连接阻止 · 事件 5157 |
| 结果标签 | 已阻止 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 网络连接与共享访问 |

<a id="item-136"></a>
### 136 Windows 安全审计 / 审计日志清除

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.log-clear |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Eventlog；事件 ID 1102。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Eventlog`；事件编号 1102

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 1102 | coverage.security_log_cleared | 安全审计日志清除 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 1102**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Eventlog",
    "event_id": 1102,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 安全审计日志清除 · 事件 1102 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 审计日志清除 |

<a id="item-137"></a>
### 137 Windows 安全审计 / 加密密钥操作

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.crypto-keys |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 5058、5059、5061。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 5058、5059、5061

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 5058 | event.security.microsoft_windows_security_auditing.5058 | 密钥文件操作 | 通过密钥存储提供程序访问密钥的存储文件；读取、写入或删除须根据操作字段确定。 |
| 5059 | event.security.microsoft_windows_security_auditing.5059 | 密钥导入与导出 | 通过密钥存储提供程序执行密钥迁移；具体为导入还是导出须根据操作字段确定。 |
| 5061 | event.security.microsoft_windows_security_auditing.5061 | 加密与密钥使用 | 密钥存储提供程序记录了密钥打开、创建、删除、签名、协商、派生或加解密操作；事件编号本身不能确定具体动作。 |

**例子：事件 5058**

使用字段模板：[密钥操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5058,
    "fields": {
      "KeyName": "ExampleKey",
      "ClientProcessId": "7096",
      "ClientCreationTime": "2026-09-11T09:14:42.206Z",
      "ReturnCode": "0x0",
      "Operation": "%%2457"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 删除密钥文件 · ExampleKey |
| 结果标签 | 成功 |
| 摘要 | 程序：未关联 · PID 7096 |
| 关键字段 | 程序：未关联 · PID 7096；进程建立时间：2026-09-11T09:14:42.206Z；返回码：0x0；密钥名称：ExampleKey；操作原码：未解析 · %%2457 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 加密密钥操作 |

**例子：事件 5059**

使用字段模板：[密钥操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5059,
    "fields": {
      "KeyName": "ExampleKey",
      "ClientProcessId": "7096",
      "ClientCreationTime": "2026-09-11T09:14:42.206Z",
      "ReturnCode": "0x0",
      "Operation": "%%2464"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 导出持久化密钥 · ExampleKey |
| 结果标签 | 成功 |
| 摘要 | 程序：未关联 · PID 7096 |
| 关键字段 | 程序：未关联 · PID 7096；进程建立时间：2026-09-11T09:14:42.206Z；返回码：0x0；密钥名称：ExampleKey；操作原码：未解析 · %%2464 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 加密密钥操作 |

**例子：事件 5061**

使用字段模板：[密钥操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5061,
    "fields": {
      "KeyName": "ExampleKey",
      "ClientProcessId": "7096",
      "ClientCreationTime": "2026-09-11T09:14:42.206Z",
      "ReturnCode": "0x0",
      "Operation": "%%2480"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 打开密钥 · ExampleKey |
| 结果标签 | 成功 |
| 摘要 | 程序：未关联 · PID 7096 |
| 关键字段 | 程序：未关联 · PID 7096；进程建立时间：2026-09-11T09:14:42.206Z；返回码：0x0；密钥名称：ExampleKey；操作原码：未解析 · %%2480 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 加密密钥操作 |

<a id="item-138"></a>
### 138 Windows 安全审计 / 密码组件自检与验证

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.crypto-integrity |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 5056、5057、5060、5062、6417。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 5056、5057、5060、5062、6417

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 5056 | event.security.microsoft_windows_security_auditing.5056 | 密码组件自检 | 密码组件执行了自检，结果须结合返回码判断。 |
| 5057 | event.security.microsoft_windows_security_auditing.5057 | 密码基础操作失败 | 密码组件报告基础加密操作失败；查看算法、失败原因及返回码。 |
| 5060 | event.security.microsoft_windows_security_auditing.5060 | 密码验证失败 | 密码组件报告验证操作失败；不能仅据此判定密钥泄露或程序被篡改。 |
| 5062 | event.security.microsoft_windows_security_auditing.5062 | 内核密码组件自检 | 内核模式密码组件执行了自检，结果须结合返回码判断。 |
| 6417 | event.security.microsoft_windows_security_auditing.6417 | FIPS 密码自检成功 | 提供程序报告 FIPS 模式密码自检成功；只说明这次自检结果。 |

**例子：事件 5056**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5056,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码组件自检 · 事件 5056 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码组件自检与验证 |

**例子：事件 5057**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5057,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码基础操作失败 · 事件 5057 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码组件自检与验证 |

**例子：事件 5060**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5060,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码验证失败 · 事件 5060 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码组件自检与验证 |

**例子：事件 5062**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5062,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 内核密码组件自检 · 事件 5062 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码组件自检与验证 |

**例子：事件 6417**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 6417,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | FIPS 密码自检成功 · 事件 6417 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码组件自检与验证 |

<a id="item-139"></a>
### 139 Windows 安全审计 / 密码提供程序与上下文配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.crypto-configuration |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 5063、5064、5065、5066、5067、5068、5069、5070。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 5063、5064、5065、5066、5067、5068、5069、5070

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 5063 | event.security.microsoft_windows_security_auditing.5063 | 密码提供程序操作 | 记录一次密码提供程序操作尝试，是否成功须查看结果。 |
| 5064 | event.security.microsoft_windows_security_auditing.5064 | 密码上下文操作 | 记录一次密码上下文操作尝试，不自动等同于配置修改。 |
| 5065 | event.security.microsoft_windows_security_auditing.5065 | 密码上下文修改 | 记录一次密码上下文修改尝试，是否生效须查看结果。 |
| 5066 | event.security.microsoft_windows_security_auditing.5066 | 密码函数操作 | 记录一次密码函数操作尝试，具体函数和结果由来源字段说明。 |
| 5067 | event.security.microsoft_windows_security_auditing.5067 | 密码函数修改 | 记录一次密码函数修改尝试，是否生效须查看结果。 |
| 5068 | event.security.microsoft_windows_security_auditing.5068 | 密码函数提供程序操作 | 记录一次密码函数提供程序操作尝试。 |
| 5069 | event.security.microsoft_windows_security_auditing.5069 | 密码函数属性操作 | 记录一次密码函数属性操作尝试，不自动等同于修改。 |
| 5070 | event.security.microsoft_windows_security_auditing.5070 | 密码函数属性修改 | 记录一次密码函数属性修改尝试，是否生效须查看结果。 |

**例子：事件 5063**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5063,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码提供程序操作 · 事件 5063 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码提供程序与上下文配置 |

**例子：事件 5064**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5064,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码上下文操作 · 事件 5064 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码提供程序与上下文配置 |

**例子：事件 5065**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5065,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码上下文修改 · 事件 5065 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码提供程序与上下文配置 |

**例子：事件 5066**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5066,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码函数操作 · 事件 5066 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码提供程序与上下文配置 |

**例子：事件 5067**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5067,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码函数修改 · 事件 5067 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码提供程序与上下文配置 |

**例子：事件 5068**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5068,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码函数提供程序操作 · 事件 5068 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码提供程序与上下文配置 |

**例子：事件 5069**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5069,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码函数属性操作 · 事件 5069 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码提供程序与上下文配置 |

**例子：事件 5070**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5070,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 密码函数属性修改 · 事件 5070 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 密码提供程序与上下文配置 |

<a id="item-140"></a>
### 140 Windows 安全审计 / DPAPI 数据保护与主密钥

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.data-protection |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4692、4693、4694、4695。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4692、4693、4694、4695

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4692 | event.security.microsoft_windows_security_auditing.4692 | 数据保护主密钥备份 | 记录一次 DPAPI 主密钥备份尝试；不表示已获得或展示密钥内容。 |
| 4693 | event.security.microsoft_windows_security_auditing.4693 | 数据保护主密钥恢复 | 记录一次 DPAPI 主密钥恢复尝试，结果须查看状态码。 |
| 4694 | event.security.microsoft_windows_security_auditing.4694 | 数据保护操作 | 记录一次对可审计数据执行保护的尝试，不能由元数据还原被保护内容。 |
| 4695 | event.security.microsoft_windows_security_auditing.4695 | 解除数据保护操作 | 记录一次解除可审计数据保护的尝试，结果须查看状态码。 |

**例子：事件 4692**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4692,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 数据保护主密钥备份 · 事件 4692 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / DPAPI 数据保护与主密钥 |

**例子：事件 4693**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4693,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 数据保护主密钥恢复 · 事件 4693 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / DPAPI 数据保护与主密钥 |

**例子：事件 4694**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4694,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 数据保护操作 · 事件 4694 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / DPAPI 数据保护与主密钥 |

**例子：事件 4695**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4695,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 解除数据保护操作 · 事件 4695 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / DPAPI 数据保护与主密钥 |

<a id="item-141"></a>
### 141 Windows 安全审计 / 凭据读取与账户检查

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.credential-access |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4648、4797、5379、5382。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4648、4797、5379、5382

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4648 | event.security.microsoft_windows_security_auditing.4648 | 显式凭据登录尝试 | 进程尝试使用显式提供的凭据登录；须关联后续登录结果，不能把尝试当作成功。 |
| 4797 | event.security.microsoft_windows_security_auditing.4797 | 账户空密码检查 | 尝试查询指定账户是否存在空密码；事件本身不表示账户确实使用空密码。 |
| 5379 | event.security.microsoft_windows_security_auditing.5379 | 凭据管理器读取 | 发生了凭据管理器读取操作；可能是查询或枚举，不能仅凭事件标题认定成功取出了密码。 |
| 5382 | event.security.microsoft_windows_security_auditing.5382 | 凭据保管库读取 | 发生了凭据保管库读取操作；实际是否成功须结合返回码，记录不等于凭据内容泄露。 |

**例子：事件 4648**

使用字段模板：[凭据操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4648,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 显式凭据登录尝试 · 事件 4648 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 凭据读取与账户检查 |

**例子：事件 4797**

使用字段模板：[凭据操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4797,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 账户空密码检查 · 事件 4797 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 凭据读取与账户检查 |

**例子：事件 5379**

使用字段模板：[凭据操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5379,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 凭据管理器读取 · 事件 5379 |
| 结果标签 | 未记录 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 凭据读取与账户检查 |

**例子：事件 5382**

使用字段模板：[凭据操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5382,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 凭据保管库读取 · 事件 5382 |
| 结果标签 | 未记录 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 凭据读取与账户检查 |

<a id="item-142"></a>
### 142 Windows 安全审计 / 用户组成员查询

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.membership-queries |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4798、4799。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4798、4799

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4798 | event.security.microsoft_windows_security_auditing.4798 | 查询用户所属本地组 | 查询了某个用户的本地组成员关系；这是读取操作，不代表加入或退出用户组。 |
| 4799 | event.security.microsoft_windows_security_auditing.4799 | 枚举本地用户组成员 | 枚举了启用安全性的本地用户组成员；这是读取操作，不代表成员被修改。 |

**例子：事件 4798**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4798,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 查询用户所属本地组 · 事件 4798 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 用户组成员查询 |

**例子：事件 4799**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4799,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 枚举本地用户组成员 · 事件 4799 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 用户组成员查询 |

<a id="item-143"></a>
### 143 Windows 安全审计 / WMI 与目录对象访问审计

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.object-access |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4662。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4662

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4662 | event.security.microsoft_windows_security_auditing.4662 | 受审计对象操作 | 记录对受审计对象的操作。ObjectServer 和 ObjectType 用于区分 WMI 命名空间、目录对象等；不能一律视为 Active Directory 修改。 |

**例子：事件 4662**

使用字段模板：[WMI 与对象访问](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4662,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 受审计对象操作 · 事件 4662 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / WMI 与目录对象访问审计 |

<a id="item-144"></a>
### 144 Windows 安全审计 / 进程访问令牌分配

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.process-tokens |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4696。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4696

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4696 | event.security.microsoft_windows_security_auditing.4696 | 进程主令牌分配 | 记录向目标进程分配主访问令牌；需区分发起进程与目标进程，不表示令牌被窃取。 |

**例子：事件 4696**

使用字段模板：[账户、组与权限](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4696,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 进程主令牌分配 · 事件 4696 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 进程访问令牌分配 |

<a id="item-145"></a>
### 145 Windows 安全审计 / 系统启动、时间与审计配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.system-audit |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 4608、4616、4826、4902。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 4608、4616、4826、4902

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4608 | event.security.microsoft_windows_security_auditing.4608 | 安全审计子系统启动 | Windows 启动过程中初始化了安全审计子系统。 |
| 4616 | event.security.microsoft_windows_security_auditing.4616 | 系统时间变更 | 系统时间发生调整；可能由时间同步或管理操作触发，应查看前后时间和发起进程。 |
| 4826 | event.security.microsoft_windows_security_auditing.4826 | 启动配置加载 | 系统加载了启动配置数据；读取配置不表示本次修改了配置。 |
| 4902 | event.security.microsoft_windows_security_auditing.4902 | 用户审计策略表建立 | 系统建立了按用户的审计策略表；查看元素数量及策略标识。 |

**例子：事件 4608**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4608,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 安全审计子系统启动 · 事件 4608 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 系统启动、时间与审计配置 |

**例子：事件 4616**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4616,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 系统时间变更 · 事件 4616 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 系统启动、时间与审计配置 |

**例子：事件 4826**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4826,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 启动配置加载 · 事件 4826 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 系统启动、时间与审计配置 |

**例子：事件 4902**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 4902,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 用户审计策略表建立 · 事件 4902 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 系统启动、时间与审计配置 |

<a id="item-146"></a>
### 146 Windows 安全审计 / 防火墙组件生命周期

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.firewall-lifecycle |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Security-Auditing；事件 ID 5024、5033。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Security-Auditing`；事件编号 5024、5033

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 5024 | event.security.microsoft_windows_security_auditing.5024 | 防火墙服务启动成功 | Windows 防火墙服务报告启动成功，不表示所有防火墙规则均已核验。 |
| 5033 | event.security.microsoft_windows_security_auditing.5033 | 防火墙驱动启动成功 | Windows 防火墙驱动报告启动成功。 |

**例子：事件 5024**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5024,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 防火墙服务启动成功 · 事件 5024 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 防火墙组件生命周期 |

**例子：事件 5033**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5033,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 防火墙驱动启动成功 · 事件 5033 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 防火墙组件生命周期 |

<a id="item-147"></a>
### 147 Windows 安全审计 / 审计日志服务

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.audit-service |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：提供程序 Microsoft-Windows-Eventlog；事件 ID 1100。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Eventlog`；事件编号 1100

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 1100 | event.security.microsoft_windows_eventlog.1100 | 事件日志服务关闭 | 事件日志服务已关闭；需要结合关机记录判断背景，不能仅凭此事件认定日志被恶意清理。 |

**例子：事件 1100**

使用字段模板：[系统通用／模板兜底](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Eventlog",
    "event_id": 1100,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 事件日志服务关闭 · 事件 1100 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 审计日志服务 |

<a id="item-148"></a>
### 148 Windows 安全审计 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.status |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.security",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-149"></a>
### 149 Windows 安全审计 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.security.other |
| source | pcscope.eventlog.security |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Security；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需相应审计策略和读取权限。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `security`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.security",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 安全审计 / 其他事件 |

<a id="item-150"></a>
### 150 计划任务执行事件 / 任务触发与执行过程

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-taskscheduler/operational.execution |
| source | pcscope.eventlog.microsoft-windows-taskscheduler/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-TaskScheduler/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-TaskScheduler；事件 ID 100、102、107、110、114、129、200、201、322、325、328。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-taskscheduler/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-TaskScheduler`；事件编号 100、102、107、110、114、129、200、201、322、325、328

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 100 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.100 | 任务实例开始 | 任务计划程序启动了一个任务实例，不表示任务动作已经完成。 |
| 102 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.102 | 任务实例完成 | 任务计划程序报告实例完成；业务操作是否成功仍需结合动作返回码与应用日志。 |
| 107 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.107 | 计划触发任务 | 任务由计划调度触发，任务名称和实例标识见来源字段。 |
| 110 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.110 | 用户触发任务 | 任务计划程序记录由用户请求触发任务；这不一定是用户亲自点击界面。 |
| 114 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.114 | 错过计划后补启动 | 任务错过原定时间，计划程序依据“错过后尽快启动”的配置补启动实例；不是最终启动失败。 |
| 129 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.129 | 任务执行进程启动 | 任务计划程序启动了任务对应的执行进程。 |
| 200 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.200 | 任务动作开始 | 任务实例中的一个动作开始执行。 |
| 201 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.201 | 任务动作结束 | 任务动作执行结束；返回码可能表示失败，不能只凭结束事件认定成功。 |
| 322 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.322 | 已有实例导致未启动 | 已有任务实例运行，本次触发未启动另一个实例；这是调度决定。 |
| 325 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.325 | 任务实例排队 | 任务实例已进入队列，不表示已经开始执行。 |
| 328 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.328 | 任务实例停止 | 任务计划程序停止了任务实例，停止原因见来源说明。 |

**例子：事件 100**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 100,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务实例开始 · 事件 100 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 102**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 102,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务实例完成 · 事件 102 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 107**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 107,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 计划触发任务 · 事件 107 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 110**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 110,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 用户触发任务 · 事件 110 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 114**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 114,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 错过计划后补启动 · 事件 114 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 129**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 129,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务执行进程启动 · 事件 129 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 200**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 200,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务动作开始 · 事件 200 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 201**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 201,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务动作结束 · 事件 201 |
| 结果标签 | 已结束 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 322**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 322,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 已有实例导致未启动 · 事件 322 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 325**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 325,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务实例排队 · 事件 325 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

**例子：事件 328**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 328,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务实例停止 · 事件 328 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务触发与执行过程 |

<a id="item-151"></a>
### 151 计划任务执行事件 / 任务登记与配置变更

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-taskscheduler/operational.configuration |
| source | pcscope.eventlog.microsoft-windows-taskscheduler/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-TaskScheduler/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-TaskScheduler；事件 ID 140。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-taskscheduler/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-TaskScheduler`；事件编号 140

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 140 | event.microsoft_windows_taskscheduler_operational.microsoft_windows_taskscheduler.140 | 任务配置更新 | 任务计划程序记录任务定义更新；具体哪个字段变化需与配置快照对照。 |

**例子：事件 140**

使用字段模板：[计划任务](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "Microsoft-Windows-TaskScheduler",
    "event_id": 140,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 任务配置更新 · 事件 140 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 任务登记与配置变更 |

<a id="item-152"></a>
### 152 计划任务执行事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-taskscheduler/operational.status |
| source | pcscope.eventlog.microsoft-windows-taskscheduler/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-TaskScheduler/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-taskscheduler/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-153"></a>
### 153 计划任务执行事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-taskscheduler/operational.other |
| source | pcscope.eventlog.microsoft-windows-taskscheduler/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-TaskScheduler/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-taskscheduler/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TaskScheduler/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 计划任务执行事件 / 其他事件 |

<a id="item-154"></a>
### 154 Windows 更新事件 / 更新检查

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.discovery |
| source | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-WindowsUpdateClient/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-WindowsUpdateClient；事件 ID 26。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-windowsupdateclient/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-WindowsUpdateClient`；事件编号 26

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 26 | event.microsoft_windows_windowsupdateclient_operational.microsoft_windows_windowsupdateclient.26 | 更新检查完成 | Windows 更新完成检查并报告找到的更新数量；不表示这些更新已安装。 |

**例子：事件 26**

使用字段模板：[软件、应用包与更新](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-windowsupdateclient/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-WindowsUpdateClient/Operational",
    "provider": "Microsoft-Windows-WindowsUpdateClient",
    "event_id": 26,
    "fields": {
      "serviceGuid": "示例值",
      "updateCount": 1024
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 更新检查完成 · 事件 26 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 更新事件 / 更新检查 |

<a id="item-155"></a>
### 155 Windows 更新事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.status |
| source | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-WindowsUpdateClient/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-windowsupdateclient/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-windowsupdateclient/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-156"></a>
### 156 Windows 更新事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.other |
| source | pcscope.eventlog.microsoft-windows-windowsupdateclient/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-WindowsUpdateClient/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-windowsupdateclient/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-windowsupdateclient/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-WindowsUpdateClient/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 更新事件 / 其他事件 |

<a id="item-157"></a>
### 157 Defender 防护事件 / Defender 健康与情报

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-windows defender/operational.health |
| source | pcscope.eventlog.microsoft-windows-windows defender/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Windows Defender/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-Windows Defender；事件 ID 1150、1151、2010。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-windows defender/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Windows Defender`；事件编号 1150、1151、2010

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 1150 | event.microsoft_windows_windows_defender_operational.microsoft_windows_windows_defender.1150 | Defender 客户端健康 | Defender 报告客户端处于健康运行状态；不等于已排除所有威胁。 |
| 1151 | event.microsoft_windows_windows_defender_operational.microsoft_windows_windows_defender.1151 | Defender 健康报告 | 记录引擎、情报版本、防护开关与扫描历史，不直接表示威胁检出。 |
| 2010 | event.microsoft_windows_windows_defender_operational.microsoft_windows_windows_defender.2010 | Defender 云情报获取 | Defender 使用云保护获取补充安全情报；不单独表示发现恶意软件。 |

**例子：事件 1150**

使用字段模板：[安全防护](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-windows defender/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Windows Defender/Operational",
    "provider": "Microsoft-Windows-Windows Defender",
    "event_id": 1150,
    "fields": {
      "Engine version": "1.0.0",
      "Platform version": "1.0.0",
      "Product Name": "示例值",
      "Security intelligence version": "1.0.0",
      "Unused": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | Defender 客户端健康 · 事件 1150 |
| 结果标签 | 不显示 |
| 摘要 | 引擎版本：1.0.0 |
| 关键字段 | 引擎版本：1.0.0 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Defender 防护事件 / Defender 健康与情报 |

**例子：事件 1151**

使用字段模板：[安全防护](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-windows defender/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Windows Defender/Operational",
    "provider": "Microsoft-Windows-Windows Defender",
    "event_id": 1151,
    "fields": {
      "AS security intelligence creation time": "2026-09-11T09:00:00Z",
      "AS security intelligence version": "1.0.0",
      "AV security intelligence creation time": "2026-09-11T09:00:00Z",
      "AV security intelligence version": "1.0.0",
      "BM state": "示例状态",
      "Engine up-to-date": "2026-09-11T09:00:00Z",
      "Engine version": "1.0.0",
      "IOAV state": "示例状态",
      "Last AS security intelligence age": "示例值",
      "Last AV security intelligence age": "示例值",
      "Last full scan age": "示例值",
      "Last full scan end time": "2026-09-11T09:00:00Z",
      "Last full scan source": "示例值",
      "Last full scan start time": "2026-09-11T09:00:00Z",
      "Last quick scan age": "示例值",
      "Last quick scan end time": "2026-09-11T09:00:00Z",
      "Last quick scan source": "示例值",
      "Last quick scan start time": "2026-09-11T09:00:00Z",
      "Latest engine version": "1.0.0",
      "Latest platform version": "1.0.0",
      "NRI engine version": "1.0.0",
      "NRI security intelligence version": "1.0.0",
      "OA state": "示例状态",
      "Platform up-to-date": "2026-09-11T09:00:00Z",
      "Platform version": "1.0.0",
      "Product Name": "示例值",
      "Product status": "示例状态",
      "RTP state": "示例状态",
      "Unused": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | Defender 健康报告 · 事件 1151 |
| 结果标签 | 不显示 |
| 摘要 | 引擎版本：1.0.0 |
| 关键字段 | 引擎版本：1.0.0 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Defender 防护事件 / Defender 健康与情报 |

**例子：事件 2010**

使用字段模板：[安全防护](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-windows defender/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Windows Defender/Operational",
    "provider": "Microsoft-Windows-Windows Defender",
    "event_id": 2010,
    "fields": {
      "Cloud protection intelligence Compilation Timestamp": "示例值",
      "Cloud protection intelligence Type": "示例值",
      "Cloud protection intelligence Type Index": "示例值",
      "Cloud protection intelligence Version": "1.0.0",
      "Current Engine Version": "1.0.0",
      "Current security intelligence Version": "1.0.0",
      "Domain": "示例值",
      "Persistence Limit Type": "示例值",
      "Persistence Limit Type Index": "示例值",
      "Persistence Limit Value": "示例值",
      "Persistence Path": "C:\\Example\\item",
      "Product Name": "示例值",
      "Product Version": "1.0.0",
      "SID": "S-1-5-21-100-200-300-1001",
      "Security intelligence Type": "示例值",
      "Security intelligence Type Index": "示例值",
      "Unused": "示例值",
      "Unused10": "示例值",
      "Unused11": "示例值",
      "Unused12": "示例值",
      "Unused2": "示例值",
      "Unused3": "示例值",
      "Unused4": "示例值",
      "Unused5": "示例值",
      "Unused6": "示例值",
      "Unused7": "示例值",
      "Unused8": "示例值",
      "Unused9": "示例值",
      "User": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | Defender 云情报获取 · 事件 2010 |
| 结果标签 | 不显示 |
| 摘要 | 操作账户：示例值 |
| 关键字段 | 操作账户：示例值 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Defender 防护事件 / Defender 健康与情报 |

<a id="item-158"></a>
### 158 Defender 防护事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-windows defender/operational.status |
| source | pcscope.eventlog.microsoft-windows-windows defender/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Windows Defender/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-windows defender/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-windows defender/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-159"></a>
### 159 Defender 防护事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-windows defender/operational.other |
| source | pcscope.eventlog.microsoft-windows-windows defender/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Windows Defender/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-windows defender/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-windows defender/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Windows Defender/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Defender 防护事件 / 其他事件 |

<a id="item-160"></a>
### 160 PowerShell 运行事件 / PowerShell 命令与脚本

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-powershell/operational.commands |
| source | pcscope.eventlog.microsoft-windows-powershell/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-PowerShell/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-PowerShell；事件 ID 4100、4103、4104。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-powershell/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-PowerShell`；事件编号 4100、4103、4104

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4100 | event.microsoft_windows_powershell_operational.microsoft_windows_powershell.4100 | PowerShell 执行错误 | PowerShell 报告执行错误，失败命令和错误原因见来源字段。 |
| 4103 | event.microsoft_windows_powershell_operational.microsoft_windows_powershell.4103 | 模块命令调用记录 | 记录命令调用和参数绑定；出现命令文本不保证命令执行成功。 |
| 4104 | event.microsoft_windows_powershell_operational.microsoft_windows_powershell.4104 | 脚本块内容记录 | PowerShell 记录脚本块文本，长脚本可能拆为多条；不表示文本中的每条语句均已执行。 |

**例子：事件 4100**

使用字段模板：[脚本与命令](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-powershell/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-PowerShell/Operational",
    "provider": "Microsoft-Windows-PowerShell",
    "event_id": 4100,
    "fields": {
      "ContextInfo": "示例值",
      "Payload": "示例值",
      "UserData": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | PowerShell 执行错误 · 事件 4100 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | PowerShell 运行事件 / PowerShell 命令与脚本 |

**例子：事件 4103**

使用字段模板：[脚本与命令](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-powershell/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-PowerShell/Operational",
    "provider": "Microsoft-Windows-PowerShell",
    "event_id": 4103,
    "fields": {
      "ContextInfo": "示例值",
      "Payload": "示例值",
      "UserData": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 模块命令调用记录 · 事件 4103 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | PowerShell 运行事件 / PowerShell 命令与脚本 |

**例子：事件 4104**

使用字段模板：[脚本与命令](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-powershell/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-PowerShell/Operational",
    "provider": "Microsoft-Windows-PowerShell",
    "event_id": 4104,
    "fields": {
      "MessageNumber": "示例值",
      "MessageTotal": "示例值",
      "Path": "C:\\Example\\item",
      "ScriptBlockId": "示例值",
      "ScriptBlockText": "******"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 脚本块内容记录 · C:\Example\item |
| 结果标签 | 不显示 |
| 摘要 | 脚本路径：C:\Example\item · 脚本块标识：示例值 |
| 关键字段 | 脚本路径：C:\Example\item；脚本块标识：示例值；分段编号：示例值；总段数：示例值 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | PowerShell 运行事件 / PowerShell 命令与脚本 |

<a id="item-161"></a>
### 161 PowerShell 运行事件 / PowerShell 宿主生命周期

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-powershell/operational.host |
| source | pcscope.eventlog.microsoft-windows-powershell/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-PowerShell/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-PowerShell；事件 ID 40961、40962、53504。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-powershell/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-PowerShell`；事件编号 40961、40962、53504

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 40961 | event.microsoft_windows_powershell_operational.microsoft_windows_powershell.40961 | PowerShell 控制台启动 | PowerShell 控制台正在启动。 |
| 40962 | event.microsoft_windows_powershell_operational.microsoft_windows_powershell.40962 | PowerShell 控制台就绪 | PowerShell 控制台已准备接收输入，不表示有用户实际输入命令。 |
| 53504 | event.microsoft_windows_powershell_operational.microsoft_windows_powershell.53504 | PowerShell IPC 监听启动 | PowerShell 在本机进程中启动 IPC 监听线程，不等同于开放远程网络端口。 |

**例子：事件 40961**

使用字段模板：[脚本与命令](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-powershell/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-PowerShell/Operational",
    "provider": "Microsoft-Windows-PowerShell",
    "event_id": 40961,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | PowerShell 控制台启动 · 事件 40961 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | PowerShell 运行事件 / PowerShell 宿主生命周期 |

**例子：事件 40962**

使用字段模板：[脚本与命令](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-powershell/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-PowerShell/Operational",
    "provider": "Microsoft-Windows-PowerShell",
    "event_id": 40962,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | PowerShell 控制台就绪 · 事件 40962 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | PowerShell 运行事件 / PowerShell 宿主生命周期 |

**例子：事件 53504**

使用字段模板：[脚本与命令](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-powershell/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-PowerShell/Operational",
    "provider": "Microsoft-Windows-PowerShell",
    "event_id": 53504,
    "fields": {
      "param1": "示例值",
      "param2": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | PowerShell IPC 监听启动 · 事件 53504 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | PowerShell 运行事件 / PowerShell 宿主生命周期 |

<a id="item-162"></a>
### 162 PowerShell 运行事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-powershell/operational.status |
| source | pcscope.eventlog.microsoft-windows-powershell/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-PowerShell/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-powershell/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-powershell/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-163"></a>
### 163 PowerShell 运行事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-powershell/operational.other |
| source | pcscope.eventlog.microsoft-windows-powershell/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-PowerShell/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-powershell/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-powershell/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-PowerShell/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | PowerShell 运行事件 / 其他事件 |

<a id="item-164"></a>
### 164 WMI 活动事件 / WMI 提供程序与调用

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-wmi-activity/operational.operations |
| source | pcscope.eventlog.microsoft-windows-wmi-activity/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-WMI-Activity/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-WMI-Activity；事件 ID 5857、5858。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-wmi-activity/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-WMI-Activity`；事件编号 5857、5858

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 5857 | event.microsoft_windows_wmi_activity_operational.microsoft_windows_wmi_activity.5857 | WMI 提供程序启动 | WMI 提供程序启动，宿主进程、组件路径和结果由来源字段给出。 |
| 5858 | event.microsoft_windows_wmi_activity_operational.microsoft_windows_wmi_activity.5858 | WMI 客户端调用失败 | WMI 记录客户端操作失败；应查看客户端进程、操作和结果码，不能把日志写入进程当成调用方。 |

**例子：事件 5857**

使用字段模板：[WMI 与对象访问](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-wmi-activity/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-WMI-Activity/Operational",
    "provider": "Microsoft-Windows-WMI-Activity",
    "event_id": 5857,
    "fields": {
      "Operation_StartedOperational/Code": "示例值",
      "Operation_StartedOperational/HostProcess": "示例值",
      "Operation_StartedOperational/ProcessID": "S-1-5-21-100-200-300-1001",
      "Operation_StartedOperational/ProviderName": "示例值",
      "Operation_StartedOperational/ProviderPath": "C:\\Example\\item"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 启动系统管理组件 · 示例值 |
| 结果标签 | 未解析 |
| 摘要 | 宿主程序：示例值 · 宿主进程 ID：S-1-5-21-100-200-300-1001 · 结果码未解析 |
| 关键字段 | 宿主程序：示例值；宿主进程 ID：S-1-5-21-100-200-300-1001；行为性质：启动 WMI 信息提供组件；结果说明：结果码未解析；结果码：示例值 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | WMI 活动事件 / WMI 提供程序与调用 |

**例子：事件 5858**

使用字段模板：[WMI 与对象访问](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-wmi-activity/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-WMI-Activity/Operational",
    "provider": "Microsoft-Windows-WMI-Activity",
    "event_id": 5858,
    "fields": {
      "Operation_ClientFailure/ClientMachine": "示例值",
      "Operation_ClientFailure/ClientProcessId": "S-1-5-21-100-200-300-1001",
      "Operation_ClientFailure/Component": "示例值",
      "Operation_ClientFailure/Id": "示例值",
      "Operation_ClientFailure/Operation": "示例值",
      "Operation_ClientFailure/PossibleCause": "示例值",
      "Operation_ClientFailure/ResultCode": "示例值",
      "Operation_ClientFailure/User": "示例值",
      "ResultCode": "0x80041010"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | WMI 管理调用 · 未记录可识别对象 |
| 结果标签 | 失败 |
| 摘要 | 操作账户：示例值 · 发起进程：未关联程序 · PID S-1-5-21-100-200-300-1001 · 指定的管理对象类无效 |
| 关键字段 | 操作账户：示例值；发起进程：未关联程序 · PID S-1-5-21-100-200-300-1001；发起计算机：示例值；行为性质：未解析操作方式；结果说明：指定的管理对象类无效；结果码：0x80041010 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | WMI 活动事件 / WMI 提供程序与调用 |

<a id="item-165"></a>
### 165 WMI 活动事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-wmi-activity/operational.status |
| source | pcscope.eventlog.microsoft-windows-wmi-activity/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-WMI-Activity/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-wmi-activity/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-wmi-activity/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-166"></a>
### 166 WMI 活动事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-wmi-activity/operational.other |
| source | pcscope.eventlog.microsoft-windows-wmi-activity/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-WMI-Activity/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-wmi-activity/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-wmi-activity/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-WMI-Activity/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | WMI 活动事件 / 其他事件 |

<a id="item-167"></a>
### 167 组策略处理事件 / 组策略服务生命周期

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-grouppolicy/operational.service |
| source | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-GroupPolicy/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-GroupPolicy；事件 ID 4115、4116、5115、5116。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-grouppolicy/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-GroupPolicy`；事件编号 4115、4116、5115、5116

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4115 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.4115 | 组策略服务启动 | 组策略组件报告服务启动；原因须结合启动、服务和相邻记录确认。 |
| 4116 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.4116 | 组策略服务初始化开始 | 开始初始化组策略服务内部状态，不代表策略内容发生变化。 |
| 5115 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5115 | 组策略服务停止 | 组策略组件报告服务停止，不能仅凭此条判定故障。 |
| 5116 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5116 | 组策略服务初始化完成 | 组策略服务初始化成功完成。 |

**例子：事件 4115**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 4115,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 组策略服务启动 · 事件 4115 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 组策略服务生命周期 |

**例子：事件 4116**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 4116,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 组策略服务初始化开始 · 事件 4116 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 组策略服务生命周期 |

**例子：事件 5115**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5115,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 组策略服务停止 · 事件 5115 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 组策略服务生命周期 |

**例子：事件 5116**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5116,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 组策略服务初始化完成 · 事件 5116 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 组策略服务生命周期 |

<a id="item-168"></a>
### 168 组策略处理事件 / 策略处理开始与完成

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-grouppolicy/operational.processing |
| source | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-GroupPolicy/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-GroupPolicy；事件 ID 4000、4001、4004、4005、8000、8001、8004、8005。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-grouppolicy/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-GroupPolicy`；事件编号 4000、4001、4004、4005、8000、8001、8004、8005

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4000 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.4000 | 开机策略处理开始 | 开始处理计算机启动时的组策略。 |
| 4001 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.4001 | 登录策略处理开始 | 开始处理用户登录时的组策略。 |
| 4004 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.4004 | 手动计算机策略处理开始 | 开始手动触发的计算机策略处理；具体发起程序需要其他证据确认。 |
| 4005 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.4005 | 手动用户策略处理开始 | 开始手动触发的用户策略处理；具体发起程序需要其他证据确认。 |
| 8000 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.8000 | 开机策略处理完成 | 计算机启动策略处理完成，错误码和耗时见来源字段。 |
| 8001 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.8001 | 登录策略处理完成 | 用户登录策略处理完成，不表示一定有策略配置发生变化。 |
| 8004 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.8004 | 手动计算机策略处理完成 | 手动触发的计算机策略处理完成。 |
| 8005 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.8005 | 手动用户策略处理完成 | 手动触发的用户策略处理完成。 |

**例子：事件 4000**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 4000,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 开机策略处理开始 · 事件 4000 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略处理开始与完成 |

**例子：事件 4001**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 4001,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 登录策略处理开始 · 事件 4001 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略处理开始与完成 |

**例子：事件 4004**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 4004,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 手动计算机策略处理开始 · 事件 4004 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略处理开始与完成 |

**例子：事件 4005**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 4005,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 手动用户策略处理开始 · 事件 4005 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略处理开始与完成 |

**例子：事件 8000**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 8000,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 开机策略处理完成 · 事件 8000 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略处理开始与完成 |

**例子：事件 8001**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 8001,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 登录策略处理完成 · 事件 8001 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略处理开始与完成 |

**例子：事件 8004**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 8004,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 手动计算机策略处理完成 · 事件 8004 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略处理开始与完成 |

**例子：事件 8005**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 8005,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 手动用户策略处理完成 · 事件 8005 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略处理开始与完成 |

<a id="item-169"></a>
### 169 组策略处理事件 / 策略会话与登录通知

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-grouppolicy/operational.sessions |
| source | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-GroupPolicy/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-GroupPolicy；事件 ID 4117、5117、5324、5325、5351、6339。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-grouppolicy/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-GroupPolicy`；事件编号 4117、5117、5324、5325、5351、6339

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4117 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.4117 | 策略会话开始 | 开始一个策略处理会话；这里的会话不等同于用户新登录。 |
| 5117 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5117 | 策略会话完成 | 组策略处理会话成功完成。 |
| 5324 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5324 | 收到登录会话通知 | 组策略收到 Winlogon 会话通知，具体通知类型见来源正文。 |
| 5325 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5325 | 收到关闭前通知 | 组策略收到服务控制管理器的关机前通知。 |
| 5351 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5351 | 返回登录组件 | 组策略会话向 Winlogon 返回控制权，不表示策略内容被修改。 |
| 6339 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.6339 | 登录桌面处理完成 | 组策略完成 Winlogon 启动桌面相关处理。 |

**例子：事件 4117**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 4117,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 策略会话开始 · 事件 4117 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略会话与登录通知 |

**例子：事件 5117**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5117,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 策略会话完成 · 事件 5117 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略会话与登录通知 |

**例子：事件 5324**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5324,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 收到登录会话通知 · 事件 5324 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略会话与登录通知 |

**例子：事件 5325**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5325,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 收到关闭前通知 · 事件 5325 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略会话与登录通知 |

**例子：事件 5351**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5351,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 返回登录组件 · 事件 5351 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略会话与登录通知 |

**例子：事件 6339**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 6339,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 登录桌面处理完成 · 事件 6339 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略会话与登录通知 |

<a id="item-170"></a>
### 170 组策略处理事件 / 策略获取、适用与排除

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-grouppolicy/operational.applicability |
| source | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-GroupPolicy/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-GroupPolicy；事件 ID 4126、5126、5257、5312、5313。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-grouppolicy/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-GroupPolicy`；事件编号 4126、5126、5257、5312、5313

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4126 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.4126 | 策略获取开始 | 开始获取适用策略；来源模板提及域控制器不单独证明本机加入了域。 |
| 5126 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5126 | 策略获取完成 | 适用策略获取成功完成；应结合实际策略列表查看是否有内容。 |
| 5257 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5257 | 策略下载完成 | 策略下载阶段完成，不能仅据此判断策略发生变化。 |
| 5312 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5312 | 适用策略列表 | 列出本次适用的组策略对象；None 表示来源报告没有适用对象。 |
| 5313 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5313 | 被排除的策略列表 | 列出未应用的策略及排除原因；例如空策略未应用不等于执行失败。 |

**例子：事件 4126**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 4126,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 策略获取开始 · 事件 4126 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略获取、适用与排除 |

**例子：事件 5126**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5126,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 策略获取完成 · 事件 5126 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略获取、适用与排除 |

**例子：事件 5257**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5257,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 策略下载完成 · 事件 5257 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略获取、适用与排除 |

**例子：事件 5312**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5312,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 适用策略列表 · 事件 5312 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略获取、适用与排除 |

**例子：事件 5313**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5313,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 被排除的策略列表 · 事件 5313 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 策略获取、适用与排除 |

<a id="item-171"></a>
### 171 组策略处理事件 / 处理模式与内部诊断

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-grouppolicy/operational.diagnostics |
| source | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-GroupPolicy/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-GroupPolicy；事件 ID 5311、5320、5321、5340。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-grouppolicy/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-GroupPolicy`；事件编号 5311、5320、5321、5340

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 5311 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5311 | 环回处理模式 | 记录组策略环回处理模式，不表示模式在本次发生改变。 |
| 5320 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5320 | 组策略内部处理状态 | 记录内部处理阶段或诊断信息；具体内容见来源说明，不直接代表策略变更。 |
| 5321 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5321 | 组策略实例诊断 | 记录服务实例检测等诊断信息；发现既有实例不单独表示服务故障。 |
| 5340 | event.microsoft_windows_grouppolicy_operational.microsoft_windows_grouppolicy.5340 | 策略处理模式 | 记录前台、后台或同步方式等处理模式。 |

**例子：事件 5311**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5311,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 环回处理模式 · 事件 5311 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 处理模式与内部诊断 |

**例子：事件 5320**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5320,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 组策略内部处理状态 · 事件 5320 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 处理模式与内部诊断 |

**例子：事件 5321**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5321,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 组策略实例诊断 · 事件 5321 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 处理模式与内部诊断 |

**例子：事件 5340**

使用字段模板：[组策略](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "Microsoft-Windows-GroupPolicy",
    "event_id": 5340,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 策略处理模式 · 事件 5340 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 处理模式与内部诊断 |

<a id="item-172"></a>
### 172 组策略处理事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-grouppolicy/operational.status |
| source | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-GroupPolicy/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-grouppolicy/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-173"></a>
### 173 组策略处理事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-grouppolicy/operational.other |
| source | pcscope.eventlog.microsoft-windows-grouppolicy/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-GroupPolicy/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-grouppolicy/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-grouppolicy/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-GroupPolicy/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 组策略处理事件 / 其他事件 |

<a id="item-174"></a>
### 174 AppX 部署事件 / 应用包部署

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.deployment |
| source | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-AppXDeploymentServer/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-AppXDeployment-Server；事件 ID 404、603、613。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-appxdeploymentserver/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-AppXDeployment-Server`；事件编号 404、603、613

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 404 | event.microsoft_windows_appxdeploymentserver_operational.microsoft_windows_appxdeployment_server.404 | 应用包部署失败 | 应用包部署操作失败；查看包名、调用程序和错误码。 |
| 603 | event.microsoft_windows_appxdeploymentserver_operational.microsoft_windows_appxdeployment_server.603 | 应用包部署开始 | 开始应用包部署阶段，例如暂存包；不表示安装已完成。 |
| 613 | event.microsoft_windows_appxdeploymentserver_operational.microsoft_windows_appxdeployment_server.613 | 应用包部署性能摘要 | 记录部署阶段耗时和状态；摘要可能对应失败，不能据此认定部署成功。 |

**例子：事件 404**

使用字段模板：[软件、应用包与更新](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppXDeploymentServer/Operational",
    "provider": "Microsoft-Windows-AppXDeployment-Server",
    "event_id": 404,
    "fields": {
      "CallingProcess": "示例值",
      "ErrorCode": "0",
      "ExternalLocation": "C:\\Example\\item",
      "MountPoint": "示例值",
      "PackageFullName": "示例值",
      "SummaryError": "示例值",
      "TargetPlatform": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 应用包部署失败 · 示例值 |
| 结果标签 | 不显示 |
| 摘要 | 软件或包：示例值 · 错误码：0 |
| 关键字段 | 软件或包：示例值；错误码：0 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppX 部署事件 / 应用包部署 |

**例子：事件 603**

使用字段模板：[软件、应用包与更新](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppXDeploymentServer/Operational",
    "provider": "Microsoft-Windows-AppXDeployment-Server",
    "event_id": 603,
    "fields": {
      "CallingProcess": "示例值",
      "DeploymentOperation": "示例值",
      "ExternalLocation": "C:\\Example\\item",
      "Flags": "示例值",
      "FlagsHigh": "示例值",
      "Path": "C:\\Example\\item"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 应用包部署开始 · 事件 603 |
| 结果标签 | 不显示 |
| 摘要 | 操作：示例值 · 路径：C:\Example\item |
| 关键字段 | 操作：示例值；路径：C:\Example\item |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppX 部署事件 / 应用包部署 |

**例子：事件 613**

使用字段模板：[软件、应用包与更新](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppXDeploymentServer/Operational",
    "provider": "Microsoft-Windows-AppXDeployment-Server",
    "event_id": 613,
    "fields": {
      "DeploymentOperation": "示例值",
      "PackageFullName": "示例值",
      "Summary": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 应用包部署性能摘要 · 示例值 |
| 结果标签 | 不显示 |
| 摘要 | 软件或包：示例值 · 操作：示例值 |
| 关键字段 | 软件或包：示例值；操作：示例值 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppX 部署事件 / 应用包部署 |

<a id="item-175"></a>
### 175 AppX 部署事件 / 应用包部署服务状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.service |
| source | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-AppXDeploymentServer/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：提供程序 Microsoft-Windows-AppXDeployment-Server；事件 ID 939、940、941、942。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-appxdeploymentserver/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-AppXDeployment-Server`；事件编号 939、940、941、942

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 939 | event.microsoft_windows_appxdeploymentserver_operational.microsoft_windows_appxdeployment_server.939 | 部署服务正在启动 | AppX 部署服务进入正在启动状态。 |
| 940 | event.microsoft_windows_appxdeploymentserver_operational.microsoft_windows_appxdeployment_server.940 | 部署服务正在运行 | AppX 部署服务进入运行状态。 |
| 941 | event.microsoft_windows_appxdeploymentserver_operational.microsoft_windows_appxdeployment_server.941 | 部署服务正在停止 | AppX 部署服务进入正在停止状态。 |
| 942 | event.microsoft_windows_appxdeploymentserver_operational.microsoft_windows_appxdeployment_server.942 | 部署服务已停止 | AppX 部署服务进入停止状态。 |

**例子：事件 939**

使用字段模板：[软件、应用包与更新](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppXDeploymentServer/Operational",
    "provider": "Microsoft-Windows-AppXDeployment-Server",
    "event_id": 939,
    "fields": {
      "Checkpoint": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 部署服务正在启动 · 事件 939 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppX 部署事件 / 应用包部署服务状态 |

**例子：事件 940**

使用字段模板：[软件、应用包与更新](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppXDeploymentServer/Operational",
    "provider": "Microsoft-Windows-AppXDeployment-Server",
    "event_id": 940,
    "fields": {
      "Checkpoint": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 部署服务正在运行 · 事件 940 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppX 部署事件 / 应用包部署服务状态 |

**例子：事件 941**

使用字段模板：[软件、应用包与更新](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppXDeploymentServer/Operational",
    "provider": "Microsoft-Windows-AppXDeployment-Server",
    "event_id": 941,
    "fields": {
      "Checkpoint": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 部署服务正在停止 · 事件 941 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppX 部署事件 / 应用包部署服务状态 |

**例子：事件 942**

使用字段模板：[软件、应用包与更新](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppXDeploymentServer/Operational",
    "provider": "Microsoft-Windows-AppXDeployment-Server",
    "event_id": 942,
    "fields": {
      "Checkpoint": "示例值"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 部署服务已停止 · 事件 942 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppX 部署事件 / 应用包部署服务状态 |

<a id="item-176"></a>
### 176 AppX 部署事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.status |
| source | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-AppXDeploymentServer/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-appxdeploymentserver/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-177"></a>
### 177 AppX 部署事件 / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.other |
| source | pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-AppXDeploymentServer/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-appxdeploymentserver/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppXDeploymentServer/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppX 部署事件 / 其他事件 |

<a id="item-178"></a>
### 178 终端会话登录、断开与注销 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational.status |
| source | pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-TerminalServices-LocalSessionManager/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-terminalservices-localsessionmanager/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-179"></a>
### 179 终端会话登录、断开与注销 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational.other |
| source | pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-TerminalServices-LocalSessionManager/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-terminalservices-localsessionmanager/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TerminalServices-LocalSessionManager/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 终端会话登录、断开与注销 / 运行事件 |

<a id="item-180"></a>
### 180 远程连接管理事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational.status |
| source | pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-TerminalServices-RemoteConnectionManager/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-terminalservices-remoteconnectionmanager/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-181"></a>
### 181 远程连接管理事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational.other |
| source | pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-TerminalServices-RemoteConnectionManager/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-terminalservices-remoteconnectionmanager/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-TerminalServices-RemoteConnectionManager/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 远程连接管理事件 / 运行事件 |

<a id="item-182"></a>
### 182 无线网络连接事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational.status |
| source | pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-WLAN-AutoConfig/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-wlan-autoconfig/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-183"></a>
### 183 无线网络连接事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational.other |
| source | pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-WLAN-AutoConfig/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-wlan-autoconfig/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-WLAN-AutoConfig/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 无线网络连接事件 / 运行事件 |

<a id="item-184"></a>
### 184 网络配置文件事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-networkprofile/operational.status |
| source | pcscope.eventlog.microsoft-windows-networkprofile/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-NetworkProfile/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-networkprofile/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-networkprofile/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-185"></a>
### 185 网络配置文件事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-networkprofile/operational.other |
| source | pcscope.eventlog.microsoft-windows-networkprofile/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-NetworkProfile/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-networkprofile/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-networkprofile/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-NetworkProfile/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 网络配置文件事件 / 运行事件 |

<a id="item-186"></a>
### 186 用户模式驱动事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational.status |
| source | pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-DriverFrameworks-UserMode/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-driverframeworks-usermode/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-187"></a>
### 187 用户模式驱动事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational.other |
| source | pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-DriverFrameworks-UserMode/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-driverframeworks-usermode/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-DriverFrameworks-UserMode/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 用户模式驱动事件 / 运行事件 |

<a id="item-188"></a>
### 188 打印服务事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-printservice/operational.status |
| source | pcscope.eventlog.microsoft-windows-printservice/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-PrintService/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-printservice/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-printservice/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-189"></a>
### 189 打印服务事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-printservice/operational.other |
| source | pcscope.eventlog.microsoft-windows-printservice/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-PrintService/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-printservice/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-printservice/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-PrintService/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 打印服务事件 / 运行事件 |

<a id="item-190"></a>
### 190 BitLocker 管理事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management.status |
| source | pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-BitLocker/BitLocker Management；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-bitlocker/bitlocker management`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-191"></a>
### 191 BitLocker 管理事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management.other |
| source | pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-BitLocker/BitLocker Management；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-bitlocker/bitlocker management`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-BitLocker/BitLocker Management",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | BitLocker 管理事件 / 运行事件 |

<a id="item-192"></a>
### 192 代码完整性事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-codeintegrity/operational.status |
| source | pcscope.eventlog.microsoft-windows-codeintegrity/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-CodeIntegrity/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-codeintegrity/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-codeintegrity/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-193"></a>
### 193 代码完整性事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-codeintegrity/operational.other |
| source | pcscope.eventlog.microsoft-windows-codeintegrity/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-CodeIntegrity/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-codeintegrity/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-codeintegrity/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-CodeIntegrity/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 代码完整性事件 / 运行事件 |

<a id="item-194"></a>
### 194 AppLocker 程序与 DLL 事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-applocker/exe and dll.status |
| source | pcscope.eventlog.microsoft-windows-applocker/exe and dll |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-AppLocker/EXE and DLL；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-applocker/exe and dll`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-applocker/exe and dll",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-195"></a>
### 195 AppLocker 程序与 DLL 事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-applocker/exe and dll.other |
| source | pcscope.eventlog.microsoft-windows-applocker/exe and dll |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-AppLocker/EXE and DLL；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-applocker/exe and dll`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-applocker/exe and dll",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppLocker/EXE and DLL",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppLocker 程序与 DLL 事件 / 运行事件 |

<a id="item-196"></a>
### 196 AppLocker 安装包与脚本事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-applocker/msi and script.status |
| source | pcscope.eventlog.microsoft-windows-applocker/msi and script |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-AppLocker/MSI and Script；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-applocker/msi and script`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-applocker/msi and script",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-197"></a>
### 197 AppLocker 安装包与脚本事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-applocker/msi and script.other |
| source | pcscope.eventlog.microsoft-windows-applocker/msi and script |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-AppLocker/MSI and Script；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-applocker/msi and script`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-applocker/msi and script",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-AppLocker/MSI and Script",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | AppLocker 安装包与脚本事件 / 运行事件 |

<a id="item-198"></a>
### 198 Sysmon / 进程生命周期

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.processes |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 1、5。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 1、5

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 1 | execution.process_started | 进程启动 | 动作名称已定义；关键字段由展示模板选择。 |
| 5 | execution.process_stopped | 进程退出 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 1**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 1,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 进程启动 · 事件 1 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 进程生命周期 |

**例子：事件 5**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 5,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 进程退出 · 事件 5 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 进程生命周期 |

<a id="item-199"></a>
### 199 Sysmon / 进程访问与异常活动

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.process-access |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 8、10、25。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 8、10、25

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 8 | execution.remote_thread | 远程线程创建 | 动作名称已定义；关键字段由展示模板选择。 |
| 10 | execution.process_access | 进程访问 | 动作名称已定义；关键字段由展示模板选择。 |
| 25 | execution.process_tampering | 进程映像异常 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 8**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 8,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 远程线程创建 · 事件 8 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 进程访问与异常活动 |

**例子：事件 10**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 10,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 进程访问 · 事件 10 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 进程访问与异常活动 |

**例子：事件 25**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 25,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 进程映像异常 · 事件 25 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 进程访问与异常活动 |

<a id="item-200"></a>
### 200 Sysmon / 网络连接

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.network |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 3。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 3

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 3 | network.connection_observed | 网络连接 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 3**

使用字段模板：[网络、DNS 与共享](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 3,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 网络连接 · 事件 3 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 网络连接 |

<a id="item-201"></a>
### 201 Sysmon / DNS 查询

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.dns |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 22。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 22

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 22 | network.dns_query | DNS 查询 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 22**

使用字段模板：[网络、DNS 与共享](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 22,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | DNS 查询 · 事件 22 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / DNS 查询 |

<a id="item-202"></a>
### 202 Sysmon / 文件操作

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.files |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 2、11、15、23、26、27、28、29。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 2、11、15、23、26、27、28、29

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 2 | files.creation_time_changed | 文件创建时间修改 | 动作名称已定义；关键字段由展示模板选择。 |
| 11 | files.created | 文件创建或覆盖 | 动作名称已定义；关键字段由展示模板选择。 |
| 15 | files.stream_created | 文件流创建 | 动作名称已定义；关键字段由展示模板选择。 |
| 23 | files.deleted_archived | 文件删除并归档 | 动作名称已定义；关键字段由展示模板选择。 |
| 26 | files.deleted | 文件删除 | 动作名称已定义；关键字段由展示模板选择。 |
| 27 | files.executable_blocked | 可执行文件创建被阻止 | 动作名称已定义；关键字段由展示模板选择。 |
| 28 | files.shredding_blocked | 文件粉碎被阻止 | 动作名称已定义；关键字段由展示模板选择。 |
| 29 | files.executable_detected | 可执行文件创建 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 2**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 2,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 文件创建时间修改 · 事件 2 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 文件操作 |

**例子：事件 11**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 11,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 文件创建或覆盖 · 事件 11 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 文件操作 |

**例子：事件 15**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 15,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 文件流创建 · 事件 15 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 文件操作 |

**例子：事件 23**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 23,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 文件删除并归档 · 事件 23 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 文件操作 |

**例子：事件 26**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 26,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 文件删除 · 事件 26 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 文件操作 |

**例子：事件 27**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 27,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 可执行文件创建被阻止 · 事件 27 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 文件操作 |

**例子：事件 28**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 28,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 文件粉碎被阻止 · 事件 28 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 文件操作 |

**例子：事件 29**

使用字段模板：[文件操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 29,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 可执行文件创建 · 事件 29 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 文件操作 |

<a id="item-203"></a>
### 203 Sysmon / 注册表操作

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.registry |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 12、13、14。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 12、13、14

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 12 | registry.object_changed | 注册表对象创建或删除 | 动作名称已定义；关键字段由展示模板选择。 |
| 13 | registry.value_set | 注册表值设置 | 动作名称已定义；关键字段由展示模板选择。 |
| 14 | registry.renamed | 注册表重命名 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 12**

使用字段模板：[注册表操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 12,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 注册表对象创建或删除 · 事件 12 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 注册表操作 |

**例子：事件 13**

使用字段模板：[注册表操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 13,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 注册表值设置 · 事件 13 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 注册表操作 |

**例子：事件 14**

使用字段模板：[注册表操作](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 14,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 注册表重命名 · 事件 14 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 注册表操作 |

<a id="item-204"></a>
### 204 Sysmon / 模块与驱动加载

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.modules |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 6、7。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 6、7

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 6 | execution.driver_loaded | 驱动加载 | 动作名称已定义；关键字段由展示模板选择。 |
| 7 | execution.module_loaded | 模块加载 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 6**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 6,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 驱动加载 · 事件 6 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 模块与驱动加载 |

**例子：事件 7**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 7,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 模块加载 · 事件 7 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 模块与驱动加载 |

<a id="item-205"></a>
### 205 Sysmon / WMI 持久订阅

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.wmi |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 19、20、21。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 19、20、21

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 19 | execution.wmi_filter | WMI 筛选器活动 | 动作名称已定义；关键字段由展示模板选择。 |
| 20 | execution.wmi_consumer | WMI 消费者活动 | 动作名称已定义；关键字段由展示模板选择。 |
| 21 | execution.wmi_binding | WMI 绑定活动 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 19**

使用字段模板：[WMI 与对象访问](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 19,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | WMI 筛选器活动 · 事件 19 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / WMI 持久订阅 |

**例子：事件 20**

使用字段模板：[WMI 与对象访问](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 20,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | WMI 消费者活动 · 事件 20 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / WMI 持久订阅 |

**例子：事件 21**

使用字段模板：[WMI 与对象访问](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 21,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | WMI 绑定活动 · 事件 21 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / WMI 持久订阅 |

<a id="item-206"></a>
### 206 Sysmon / 命名管道

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.pipes |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 17、18。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 17、18

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 17 | execution.pipe_created | 管道创建 | 动作名称已定义；关键字段由展示模板选择。 |
| 18 | execution.pipe_connected | 管道连接 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 17**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 17,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 管道创建 · 事件 17 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 命名管道 |

**例子：事件 18**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 18,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 管道连接 · 事件 18 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 命名管道 |

<a id="item-207"></a>
### 207 Sysmon / 原始磁盘读取

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.raw-disk |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 9。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 9

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 9 | storage.raw_read | 原始磁盘读取 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 9**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 9,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 原始磁盘读取 · 事件 9 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 原始磁盘读取 |

<a id="item-208"></a>
### 208 Sysmon / 剪贴板活动

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.clipboard |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 24。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 24

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 24 | execution.clipboard_changed | 剪贴板变化 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 24**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 24,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 剪贴板变化 · 事件 24 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 剪贴板活动 |

<a id="item-209"></a>
### 209 Sysmon / Sysmon 服务与配置

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.monitor |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：提供程序 Microsoft-Windows-Sysmon；事件 ID 4、16、255。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：提供程序精确匹配 `Microsoft-Windows-Sysmon`；事件编号 4、16、255

| 事件 ID | 动作 ID | 动作名称 | 规则说明（不是默认展示正文） |
| --- | --- | --- | --- |
| 4 | sysmon.service_changed | Sysmon 服务状态变化 | 动作名称已定义；关键字段由展示模板选择。 |
| 16 | sysmon.config_changed | Sysmon 配置变化 | 动作名称已定义；关键字段由展示模板选择。 |
| 255 | sysmon.error | Sysmon 内部错误 | 动作名称已定义；关键字段由展示模板选择。 |

**例子：事件 4**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 4,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | Sysmon 服务状态变化 · 事件 4 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / Sysmon 服务与配置 |

**例子：事件 16**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 16,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | Sysmon 配置变化 · 事件 16 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / Sysmon 服务与配置 |

**例子：事件 255**

使用字段模板：[进程与模块](#ui-templates)。动作规则已匹配。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "Microsoft-Windows-Sysmon",
    "event_id": 255,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | Sysmon 内部错误 · 事件 255 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / Sysmon 服务与配置 |

<a id="item-210"></a>
### 210 Sysmon / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.status |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-211"></a>
### 211 Sysmon / 其他事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-sysmon/operational.other |
| source | pcscope.eventlog.microsoft-windows-sysmon/operational |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Sysmon/Operational；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；需已安装 Sysmon 并启用该通道。 分类规则：未命中本来源已定义规则的事件，保留原始提供程序和事件编号。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-sysmon/operational`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-sysmon/operational",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Sysmon/Operational",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Sysmon / 其他事件 |

<a id="item-212"></a>
### 212 Hyper-V 管理事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin.status |
| source | pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Hyper-V-VMMS/Admin；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-hyper-v-vmms/admin`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-213"></a>
### 213 Hyper-V 管理事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin.other |
| source | pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Hyper-V-VMMS/Admin；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-hyper-v-vmms/admin`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Hyper-V-VMMS/Admin",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Hyper-V 管理事件 / 运行事件 |

<a id="item-214"></a>
### 214 Windows 备份事件 / 来源采集状态

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-backup.status |
| source | pcscope.eventlog.microsoft-windows-backup |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Backup；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：本采集器报告的缺口、恢复及断点状态。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-backup`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：coverage.* 来源状态（排除安全日志清除事件）。

**显示**：标题为采集缺口／采集恢复等；原因和时间直接显示，其余属性折叠。该采集项表示采集器自身状态，不是 Windows EventData 的一种字段类型。

**例子**：
```json
{
  "source": "pcscope.eventlog.microsoft-windows-backup",
  "kind": "coverage.gap",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "reason": "示例：日志通道不存在"
  }
}
```

界面：采集缺口 · 来源采集状态；原因“示例：日志通道不存在”。

<a id="item-215"></a>
### 215 Windows 备份事件 / 运行事件

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.microsoft-windows-backup.other |
| source | pcscope.eventlog.microsoft-windows-backup |
| 分类 | eventlog |
| 插件目录分组 | eventlog |
| 渠道／接口 | Windows 事件日志 · Microsoft-Windows-Backup；.NET System.Diagnostics.Eventing.Reader.EventLogQuery / EventLogReader.ReadEvent / EventRecord.ToXml |
| 采集方式（目录原文） | 按持久化 RecordID 断点增量读取，每通道每轮最多 128 条；扩展通道轮换查询。 首次自动读取正文，保存 System/EventData/UserData、脱敏/截断状态与 XML 摘要；通道需存在且可读取；禁用通道也尝试读取保留历史。 分类规则：该通道的运行记录。 |

**原始字段结构**：使用 §2.3 的所有固定事件字段；`data.fields` 与 `data.system` 为动态结构。通道标识为 `microsoft-windows-backup`（程序 source 中为小写；原始 channel 保留来源值）。

**匹配规则**：未命中本通道其他规则的事件。

**例子：事件 999999**（未登记编号，用于演示兜底）

使用字段模板：[系统通用／模板兜底](#ui-templates)。当前没有该编号的动作规则。 字段以本例 JSON 为准；没有历史字段形状样本的编号采用空 fields 的最小结构示例，不能当成该 Windows 事件的完整 schema。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.microsoft-windows-backup",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Microsoft-Windows-Backup",
    "provider": "ExampleProvider",
    "event_id": 999999,
    "fields": {},
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 未识别事件 · ExampleProvider · 999999 |
| 结果标签 | 不显示 |
| 摘要 | 展开查看记录 |
| 关键字段 | 无匹配字段，不显示关键字段区 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | Windows 备份事件 / 运行事件 |

<a id="item-216"></a>
### 216 文件变更记录

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.usn.changes |
| source | pcscope.usn. |
| 分类 | files |
| 插件目录分组 | 共享／前缀兜底 |
| 渠道／接口 | Win32 DeviceIoControl / NTFS USN 日志；详见 USN 采集器 |
| 采集方式（目录原文） | 复用对应来源；以实现代码和下方说明为准 |

**实现位置**：[USN 采集器](../plugins/usn/src/main.rs)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| attributes | 文件或对象属性标志 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| drive | 盘符 | 保留原名（无专用中文标签） | "C:" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| file_id | 文件标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| full_path_resolved | 是否还原完整历史路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| journal_id | USN 日志身份标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| parent_id | 父目录标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| reason_mask | USN 变更原因位掩码 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| record_version | USN 记录格式版本 | 保留原名（无专用中文标签） | "1.0.0" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| source_time_available | 是否取得来源时间 | 是否有来源时间 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| usn | USN 记录位置 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| volume | 卷标识 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| watch_path | 匹配的监测文件路径 | 保留原名（无专用中文标签） | "C:\\Example\\item" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| watch_revision | 监测名单修订号 | 保留原名（无专用中文标签） | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**USN 展示现状**：记录保留 reason_mask 等原始值。原因位解码函数存在，但当前列表走 collection-view 的通用属性摘要；不能把“已有解码函数”写成默认界面已展示所有中文操作。

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.usn.c",
  "id": "example-object",
  "item_name": "文件变更记录",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "volume": "ExampleVolume",
    "drive": "C:",
    "name": "watched.txt",
    "watch_path": "C:\\Example\\watched.txt",
    "reason_mask": 256,
    "full_path_resolved": false
  },
  "kind": "files.usn_changed",
  "occurred_at": "2026-09-11T09:00:00Z",
  "object_id": "c:\\example\\watched.txt"
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | watched.txt |
| 属性摘要 | volume：ExampleVolume · drive：C: · watch_path：C:\Example\watched.txt |
| 默认属性组 | 无匹配属性组；全部采集属性默认展开 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

<a id="item-217"></a>
### 217 手动导入的事件日志

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.eventlog.import |
| source | pcscope.eventlog.import. |
| 分类 | eventlog |
| 插件目录分组 | 共享／前缀兜底 |
| 渠道／接口 | 用户指定本地 EVTX；EventLogQuery FilePath |
| 采集方式（目录原文） | 复用对应来源；以实现代码和下方说明为准 |

**实现位置**：[事件采集器](../plugins/eventlog/collect-eventlog.ps1)

手动导入 source 使用归档前缀，不等于在线 Security source。当前按在线 source 编写的动作规则不会自动全部套用于归档来源。固定原始字段见 §2.3。

```json
{
  "seq": 1,
  "source": "pcscope.eventlog.import.",
  "kind": "windows.event",
  "object_id": "example-event",
  "occurred_at": "2026-09-11T09:14:44Z",
  "data": {
    "channel": "Security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "event_id": 5058,
    "fields": {
      "KeyName": "ExampleKey",
      "ClientProcessId": "7096",
      "ClientCreationTime": "2026-09-11T09:14:42.206Z",
      "ReturnCode": "0x0",
      "Operation": "%%2457"
    },
    "message": "示例正文：实际内容由 Windows 消息资源提供。",
    "message_status": "available",
    "field_schema": "evidence-v2",
    "writer_process_id": 1568
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 标题 | 删除密钥文件 · ExampleKey |
| 结果标签 | 不显示 |
| 摘要 | 程序：未关联 · PID 7096 |
| 关键字段 | 程序：未关联 · PID 7096；进程建立时间：2026-09-11T09:14:42.206Z；密钥名称：ExampleKey；操作原码：未解析 · %%2457 |
| 折叠内容 | 来源原始说明、全部事件字段、其他采集属性、来源与记录信息；有数据状态问题时另显示数据状态 |
| 分类 | 手动导入的事件日志 |

<a id="item-218"></a>
### 218 资源管理器扩展（未细分／来源状态）

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.shell.registrations |
| source | pcscope.shell.registrations |
| 分类 | shell |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · Microsoft.Win32.RegistryKey；HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\PreviewHandlers |
| 采集方式（目录原文） | 只读扫描 32/64 位视图；提取 CLSID 并关联 COM 注册 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:151](../plugins/powershell-adapter/collect-system.ps1)、[plugins/powershell-adapter/collect-system.ps1:154](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.company | 公司 | 公司 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.embedded_signer | 文件嵌入签名的主题 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.error_type | 读取失败的异常类型 | 保留原名（无专用中文标签） | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.file_version | 文件版本 | 文件版本 | "1.0.0" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.hash_status | 哈希计算状态 | 保留原名（无专用中文标签） | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.length | 大小（字节） | 大小（字节） | 1024 | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.path | 路径 | 路径 | "C:\\Example\\item" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.signature_status | 签名检查结果 | 签名检查结果 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| implementation_file.status | 状态 | 状态 | "示例状态" | 嵌套／动态内容：全部采集属性及原始数据 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.shell.registrations",
  "id": "example-object",
  "item_name": "资源管理器扩展（未细分／来源状态）",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "registry_path": "SOFTWARE\\Example",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    }
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | SOFTWARE / Example |
| 属性摘要 | ExampleValue：示例登记值 |
| 默认属性组 | 扩展登记：注册表路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.shell.registrations`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

<a id="item-219"></a>
### 219 登录启动项（未细分／来源状态）

| 属性 | 内容 |
| --- | --- |
| 采集项 ID | pcscope.startup.logon |
| source | pcscope.startup.logon |
| 分类 | startup |
| 插件目录分组 | pcscope-powershell-adapter.exe |
| 渠道／接口 | 注册表 · HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Run、RunOnce、Explorer\StartupApproved |
| 采集方式（目录原文） | 枚举 32/64 位视图；Run 命令保存脱敏原值，StartupApproved 保存登记信息 |

**实现位置**：[plugins/powershell-adapter/collect-system.ps1:159](../plugins/powershell-adapter/collect-system.ps1)

**声明字段**：下表为采集源码中直接声明的属性、Fields/Cim-Items 指定属性和公共容器。它是跨分支字段集合，包含嵌套成员；字段实际层级以本节链接的采集代码和原始 JSON 为准。`source_properties.*`、`values.{名称}.*` 表示动态扩展，不是仅采集白名单。

| 原始字段／容器 | 字段含义（文档解释） | 当前界面字段名 | 每字段演示值 | 界面去向／说明 |
| --- | --- | --- | --- | --- |
| basis | 数据依据 | 数据依据 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection | 采集范围与限制 | 采集范围与限制 | {"policy":"evidence-v2"} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| collection.policy | 采集策略版本 | 采集策略版本 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| collection.unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| command | 启动命令 | 启动命令 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| command_collected | 是否取得运行命令 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| hive | 注册表根 | 注册表根 | "LocalMachine" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| last_write_utc | 文件系统最近写入时间 UTC | 最近写入时间 | "2026-09-11T09:00:00Z" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| length | 大小（字节） | 大小（字节） | 1024 | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| name | 名称 | 名称 | "Example" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| path | 路径 | 路径 | "C:\\Example\\item" | 命中领域优选字段时默认显示；其他内容仍完整保留 |
| policy | 采集策略版本 | 采集策略版本 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| registry_path | 注册表路径 | 注册表路径 | "SOFTWARE\\Example" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| remaining_depth | 剩余注册表扫描深度 | 剩余遍历深度 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| sha256 | SHA-256 摘要 | SHA-256 摘要 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| shortcut_target_collected | 是否解析快捷方式目标 | 保留原名（无专用中文标签） | false | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| unvisited_subkeys | 深度范围外子键数 | 深度范围外子键数 | "示例值" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values | 注册表值 | 注册表值 | {"ExampleValue":{"kind":"String","value":"示例值"}} | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |
| values.{名称}.kind | 类型 | 类型 | "String" | 嵌套／动态内容：全部采集属性及原始数据 |
| values.{名称}.value | 值 | 值 | "示例值" | 嵌套／动态内容：全部采集属性及原始数据 |
| view | 注册表视图 | 注册表视图 | "Registry64" | 全部采集属性及原始数据；摘要是否选中还受排序和数量限制 |

**合成例子（只节选部分声明字段，不代表该来源每条对象都有这些字段）**：

```json
{
  "source": "pcscope.startup.logon",
  "id": "example-object",
  "item_name": "登录启动项（未细分／来源状态）",
  "seen_at": "2026-09-11T09:00:00Z",
  "changed_at": "2026-09-11T08:00:00Z",
  "data": {
    "path": "C:\\Example\\item",
    "hive": "LocalMachine",
    "view": "Registry64",
    "values": {
      "ExampleValue": {
        "kind": "String",
        "value": "示例登记值"
      }
    },
    "command": "示例值",
    "command_collected": false,
    "last_write_utc": "2026-09-11T09:00:00Z",
    "length": 1024,
    "name": "Example"
  }
}
```

| 界面位置 | 该示例实际输出 |
| --- | --- |
| 对象名称 | Example |
| 属性摘要 | ExampleValue：示例登记值 · 启动命令：示例值 · 最近写入时间：2026-09-11 17:00:00（本机时间） |
| 默认属性组 | 运行与启动配置：名称、路径 |
| 完整数据 | 全部采集属性；原始数据 Tab |
| 时间 | 最近成功采集、最近变化（观察时间） |

**完整采集表达式**：见[源码附录](#source-appendix)中 source `pcscope.startup.logon`；作用域、对象 ID 构造、字段层级及条件分支以该表达式为准。

## 5A. 三个贯穿采集与界面的完整例子

### 例 A：密钥删除，从 XML 到界面

```xml
<Event><System><Provider Name="Microsoft-Windows-Security-Auditing"/><EventID>5058</EventID><Execution ProcessID="1568"/></System><EventData><Data Name="KeyName">ExampleKey</Data><Data Name="Operation">%%2457</Data><Data Name="ReturnCode">0x0</Data><Data Name="ClientProcessId">7096</Data><Data Name="SubjectUserName">demo</Data></EventData></Event>
```

这是只为说明字段映射而节选的 XML。EventData/KeyName → data.fields.KeyName → 标题对象 ExampleKey；Operation=%%2457 → 删除密钥文件；ReturnCode=0x0 → 成功标签；ClientProcessId → 程序未关联 · PID 7096；System/Execution 的 1568 → 来源信息中的写入进程编号。完整 XML 进入 raw_xml 加密容器；原始数据 Tab 的显示由隐私设置控制。

### 例 B：服务状态快照变化

```json
{
  "source": "pcscope.startup.services",
  "kind": "object.changed",
  "object_id": "ExampleService",
  "occurred_at": "2026-09-11T09:00:00Z",
  "data": {
    "before": {
      "name": "ExampleService",
      "state": "Running"
    },
    "after": {
      "name": "ExampleService",
      "state": "Stopped"
    }
  }
}
```

界面标题：属性变化 · ExampleService；摘要：状态：运行中（Running）→ 已停止（Stopped）；详情将 before 与 after 左右对照并高亮 state。这里不会生成“某账户停止了服务”的字段，因为这条快照没有操作账户。

### 例 C：未知厂商事件，有正文和字段但未专门解析

```json
{
  "source": "pcscope.eventlog.application",
  "kind": "windows.event",
  "data": {
    "provider": "ExampleProvider",
    "event_id": 999999,
    "message": "Example application completed a maintenance cycle.",
    "fields": {
      "MaintenanceToken": "demo-operation"
    }
  }
}
```

界面标题：未识别事件 · ExampleProvider · 999999；摘要：展开查看记录；没有结果标签，也没有匹配的操作属性。展开来源原始说明后能看正文，展开全部事件字段能看 MaintenanceToken。数据已采集，但中文关键说明尚未实现。

<a id="fallback"></a>
## 6. 兜底、差异和已知缺口

| 情况 | 实际表现 | 例子 |
| --- | --- | --- |
| 有动作、没有关键字段 | 有明确标题；无关键字段区；正文和全部字段折叠 | 某组策略事件 fields 只有模板未覆盖的字段 |
| 没有动作规则 | 未识别事件 · 对象；对象缺失时用提供程序和编号 | 未识别事件 · ExampleProvider · 999999 |
| 没有摘要字段 | 摘要为展开查看记录 | fields={UnmappedField:"示例值"} |
| 对象无优选字段 | 全部采集属性默认展开 | 安全证书对象的 subject/issuer 不在通用对象优选列表 |
| 没有结果解析 | 无结果标签，不因字段等于 0 自动认定成功 | 系统通用事件 ReturnCode=0 |
| 导入来源 | 源前缀与在线日志不同，可能不命中在线动作 | pcscope.eventlog.import.<archive> |
| 程序关联 | 同 PID 且有 ClientCreationTime，±2 秒内匹配启动记录；作为候选展示 | 7096 + 09:14:42 对应的 Security 4688 |
| 任务关联 | 同完整任务路径前后 5 分钟；快照另按观察区间 | \Example\DailyTask |
| 关联局限 | 尚无通用账户/登录会话/任意对象关系图；仅 PID 不自动认领程序 | ClientProcessId=7096 且缺少建立时间 → 不进行该匹配 |
| 目录措辞可能过时 | 部分 method 仍写摘要／正文不采集，但实际已改为加密保存 | WMI 脚本、打印作业名称等须查当前源码 |
| 模板宽泛 | 多个来源共用系统通用模板，并非每个采集项有专用字段布局 | BitLocker/打印/Hyper-V 事件 |
| 字段名未命中 | 关键区省略，原值仍在全部字段；文档表格明确标出 | Parent PID 或厂商自定义字段 |

### 6.1 每个日志来源的分类兜底

| source | 当前兜底采集项 | 显示例子 |
| --- | --- | --- |
| pcscope.eventlog.system | 系统与服务事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.application | 应用程序事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.security | Windows 安全审计 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-taskscheduler/operational | 计划任务执行事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-windowsupdateclient/operational | Windows 更新事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-windows defender/operational | Defender 防护事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-powershell/operational | PowerShell 运行事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-wmi-activity/operational | WMI 活动事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-grouppolicy/operational | 组策略处理事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational | AppX 部署事件 / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational | 终端会话登录、断开与注销 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational | 远程连接管理事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational | 无线网络连接事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-networkprofile/operational | 网络配置文件事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational | 用户模式驱动事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-printservice/operational | 打印服务事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management | BitLocker 管理事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-codeintegrity/operational | 代码完整性事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-applocker/exe and dll | AppLocker 程序与 DLL 事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-applocker/msi and script | AppLocker 安装包与脚本事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-sysmon/operational | Sysmon / 其他事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin | Hyper-V 管理事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |
| pcscope.eventlog.microsoft-windows-backup | Windows 备份事件 / 运行事件 | 未识别事件 · ExampleProvider · 999999；来源正文和全部字段可展开 |

### 6.2 不在目录中的辅助来源

USN 空名单会返回 `pcscope.usn.watch-list`，数据 `{files:[],revision:0,active:false,reason:"empty allowlist; no file changes collected"}`。它没有独立中文采集项目录项，采用来源／通用对象兜底。后台生命周期 `pcscope.core` 的 collector.started、collector.stopped、coverage.unclean_shutdown 使用通用记录展示，例 `{kind:"collector.started",data:{mode:"development-user-daemon"}}`。这些不应被遗漏为“没有采集到任何内容”。

<a id="source-appendix"></a>
## 7. 源码表达式与重建

以下直接复制当前采集声明，用于核对接口、ID、字段层级、条件分支和转换。代码仅作说明，不要把它当成本文要求执行的命令。重复 source 的不同作用域仍分别保留。动态 helper 的完整行为查看其源码文件和本册 §2–3。

### pcscope.system.version · plugins/powershell-adapter/collect-system.ps1:26

```powershell
Emit-Collection 'pcscope.system.version' {
    $os=Get-CimInstance Win32_OperatingSystem
    Item 'operating-system' ([ordered]@{caption=$os.Caption;version=$os.Version;build=$os.BuildNumber;architecture=$os.OSArchitecture;install_time=(Iso $os.InstallDate);install_time_basis='Win32_OperatingSystem.InstallDate';computer_name=$os.CSName})
}
```

### pcscope.system.boot-state · plugins/powershell-adapter/collect-system.ps1:30

```powershell
Emit-Collection 'pcscope.system.boot-state' {
    $os=Get-CimInstance Win32_OperatingSystem
    Item 'boot-state' ([ordered]@{last_boot_time=(Iso $os.LastBootUpTime);basis='snapshot';timezone=(Get-TimeZone).Id})
}
```

### pcscope.identity.accounts · plugins/powershell-adapter/collect-system.ps1:34

```powershell
Emit-Collection 'pcscope.identity.accounts' {
    Get-LocalUser | ForEach-Object {Item $_.SID.Value ([ordered]@{name=$_.Name;enabled=$_.Enabled;description=$_.Description;last_logon=(Iso $_.LastLogon);password_last_set=(Iso $_.PasswordLastSet);account_expires=(Iso $_.AccountExpires);password_required=$_.PasswordRequired;user_may_change_password=$_.UserMayChangePassword;principal_source=[string]$_.PrincipalSource;created_at=$null;created_at_basis='not available from this source'})}
}
```

### pcscope.identity.groups · plugins/powershell-adapter/collect-system.ps1:37

```powershell
Emit-Collection 'pcscope.identity.groups' {
    Get-LocalGroup | ForEach-Object {$group=$_;$members=@(Get-LocalGroupMember -SID $group.SID | ForEach-Object {$_.SID.Value} | Sort-Object);Item $group.SID.Value ([ordered]@{name=$group.Name;description=$group.Description;members=$members})}
}
```

### pcscope.startup.services · plugins/powershell-adapter/collect-system.ps1:40

```powershell
Emit-Collection 'pcscope.startup.services' {
    Get-CimInstance Win32_Service | ForEach-Object {Item $_.Name.ToLowerInvariant() ([ordered]@{name=$_.Name;display_name=$_.DisplayName;state=$_.State;start_mode=$_.StartMode;start_account=$_.StartName;service_type=$_.ServiceType;description=$_.Description;command=(Protect-Text $_.PathName);pid=$_.ProcessId;exit_code=$_.ExitCode;service_specific_exit_code=$_.ServiceSpecificExitCode})}
}
```

### pcscope.startup.tasks · plugins/powershell-adapter/collect-system.ps1:43

```powershell
Emit-Collection 'pcscope.startup.tasks' {
    Get-ScheduledTask | ForEach-Object {$task=$_;$actions=@($task.Actions | ForEach-Object {Public-CimFields $_});Item ($task.TaskPath+$task.TaskName).ToLowerInvariant() ([ordered]@{name=$task.TaskName;path=$task.TaskPath;state=[string]$task.State;enabled=$task.Settings.Enabled;user_id=$task.Principal.UserId;run_level=[string]$task.Principal.RunLevel;actions=$actions})}
}
```

### pcscope.storage.volumes · plugins/powershell-adapter/collect-system.ps1:46

```powershell
Emit-Collection 'pcscope.storage.volumes' {
    Get-CimInstance Win32_Volume | ForEach-Object {Item $_.DeviceID ([ordered]@{device_id=$_.DeviceID;drive_letter=$_.DriveLetter;label=$_.Label;file_system=$_.FileSystem;capacity=$_.Capacity;drive_type=$_.DriveType})}
}
```

### pcscope.network.adapters · plugins/powershell-adapter/collect-system.ps1:49

```powershell
Emit-Collection 'pcscope.network.adapters' {
    Get-NetAdapter -IncludeHidden | ForEach-Object {Item ([string]$_.InterfaceGuid) ([ordered]@{name=$_.Name;description=$_.InterfaceDescription;status=[string]$_.Status;mac_address=$_.MacAddress;link_speed=[string]$_.LinkSpeed;driver_version=$_.DriverVersion})}
}
```

### pcscope.network.addresses · plugins/powershell-adapter/collect-system.ps1:52

```powershell
Emit-Collection 'pcscope.network.addresses' {
    Get-NetIPAddress | ForEach-Object {Item "$($_.InterfaceIndex)/$($_.AddressFamily)/$($_.IPAddress)" ([ordered]@{interface_index=$_.InterfaceIndex;interface_alias=$_.InterfaceAlias;address=$_.IPAddress;prefix_length=$_.PrefixLength;address_family=[string]$_.AddressFamily;prefix_origin=[string]$_.PrefixOrigin;suffix_origin=[string]$_.SuffixOrigin;address_state=[string]$_.AddressState})}
}
```

### pcscope.virtualization.platform · plugins/powershell-adapter/collect-system.ps1:55

```powershell
Emit-Collection 'pcscope.virtualization.platform' {
    $computer=Get-CimInstance Win32_ComputerSystem
    Item 'hypervisor' ([ordered]@{hypervisor_present=$computer.HypervisorPresent;manufacturer=$computer.Manufacturer;model=$computer.Model})
    Get-CimInstance Win32_Processor | ForEach-Object {Item $_.DeviceID ([ordered]@{name=$_.Name;firmware_virtualization=$_.VirtualizationFirmwareEnabled;vm_monitor_extensions=$_.VMMonitorModeExtensions;slat=$_.SecondLevelAddressTranslationExtensions})}
}
```

### pcscope.hardware.devices · plugins/powershell-adapter/collect-system.ps1:60

```powershell
Emit-Collection 'pcscope.hardware.devices' {
    Get-CimInstance Win32_PnPEntity | ForEach-Object {Item $_.DeviceID ([ordered]@{name=$_.Name;class=$_.PNPClass;class_guid=$_.ClassGuid;manufacturer=$_.Manufacturer;service=$_.Service;status=$_.Status;error_code=$_.ConfigManagerErrorCode})}
}
```

### pcscope.system.hotfixes · plugins/powershell-adapter/collect-system.ps1:63

```powershell
Emit-Collection 'pcscope.system.hotfixes' {
    Get-CimInstance Win32_QuickFixEngineering | ForEach-Object {Item "$($_.HotFixID)/$($_.ServicePackInEffect)" ([ordered]@{hotfix_id=$_.HotFixID;description=$_.Description;installed_on=[string]$_.InstalledOn;coverage='QFE only; not the complete Windows Update inventory'})}
}
```

### pcscope.shell.registrations · plugins/powershell-adapter/collect-system.ps1:151

```powershell
Emit-Collection 'pcscope.shell.registrations' { $shards[$bucket] | Sort-Object id } "$hive/$view/shard:$bucket"
```

### pcscope.shell.registrations · plugins/powershell-adapter/collect-system.ps1:154

```powershell
Emit-Collection 'pcscope.shell.registrations' {throw $failure} "$hive/$view"
```

### pcscope.startup.logon · plugins/powershell-adapter/collect-system.ps1:159

```powershell
Emit-Collection 'pcscope.startup.logon' {
    foreach($hive in @('LocalMachine','CurrentUser')){foreach($view in @('Registry64','Registry32')){
        if('pcscope.startup.logon.registry' -in $selection.disabled_items){continue}
        $root=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::$hive,[Microsoft.Win32.RegistryView]::$view)
        try{
            foreach($suffix in @('Run','RunOnce')){
                $path="Software\Microsoft\Windows\CurrentVersion\$suffix";$key=$root.OpenSubKey($path,$false)
                if($null -ne $key){try{foreach($name in $key.GetValueNames()){Item "$hive/$view/$suffix/$name" ([ordered]@{hive=$hive;view=$view;registry_path=$path;name=$name;command=(Protect-Text ([string]$key.GetValue($name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)));command_collected=$true})}}finally{$key.Dispose()}}
            }
            foreach($item in @(Read-RegistryTree $root 'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved' 1)){Item "$hive/$view/$($item.id)" $item.data}
        }finally{$root.Dispose()}
    }}
    foreach($folder in @([Environment]::GetFolderPath('Startup'),[Environment]::GetFolderPath('CommonStartup'))){
        if('pcscope.startup.logon.folders' -in $selection.disabled_items){continue}
        if($folder -and (Test-Path -LiteralPath $folder)){
            Get-ChildItem -LiteralPath $folder -File -Force | ForEach-Object {Item $_.FullName.ToLowerInvariant() ([ordered]@{path=$_.FullName;length=$_.Length;last_write_utc=$_.LastWriteTimeUtc.ToString('o');sha256=(Hash-File $_.FullName);shortcut_target_collected=$false})}
        }
    }
}
```

### pcscope.apps.browser-policies · plugins/powershell-adapter/domains/applications.ps1:1

```powershell
Emit 'apps.browser-policies' {foreach($hive in @('LocalMachine','CurrentUser')){foreach($path in @('Software\Policies\Google\Chrome','Software\Policies\Microsoft\Edge','Software\Policies\Mozilla\Firefox')){Registry $hive $path @('ExtensionInstallBlocklist','ExtensionInstallAllowlist','PasswordManagerEnabled','BrowserSignin','SyncDisabled') 2}}}
```

### pcscope.apps.ide-extensions · plugins/powershell-adapter/domains/applications.ps1:2

```powershell
Emit 'apps.ide-extensions' {foreach($base in @((Join-Path $env:USERPROFILE '.vscode/extensions'),(Join-Path $env:USERPROFILE '.vscode-insiders/extensions'))){if(-not (Test-Path -LiteralPath $base)){continue};Get-ChildItem -LiteralPath $base -Directory | ForEach-Object {$manifest=Join-Path $_.FullName 'package.json';if(Test-Path -LiteralPath $manifest){$file=Get-Item -LiteralPath $manifest;if($file.Length -gt 1048576){throw 'Extension manifest too large'};$data=Get-Content -Raw -LiteralPath $manifest|ConvertFrom-Json;Item $_.FullName @{name=$data.name;publisher=$data.publisher;version=$data.version;engines=$data.engines;basis='installed manifest; does not establish activation'}}}}}
```

### pcscope.apps.dotnet-runtimes · plugins/powershell-adapter/domains/applications.ps1:3

```powershell
Emit 'apps.dotnet-runtimes' {Registry LocalMachine 'SOFTWARE\dotnet\Setup\InstalledVersions' @('InstallLocation','Version') 4;Registry LocalMachine 'SOFTWARE\Microsoft\NET Framework Setup\NDP' @('Version','Release','Install','InstallPath') 3}
```

### pcscope.apps.npm-global-packages · plugins/powershell-adapter/domains/applications.ps1:4

```powershell
Emit 'apps.npm-global-packages' {
    foreach($base in @((Join-Path $env:APPDATA 'npm/node_modules'),(Join-Path $env:ProgramFiles 'nodejs/node_modules'))){
        if(-not (Test-Path -LiteralPath $base)){continue}
        $directories=@(Get-ChildItem -LiteralPath $base -Directory|ForEach-Object {if($_.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Package reparse point not followed'};if($_.Name.StartsWith('@')){Get-ChildItem -LiteralPath $_.FullName -Directory}else{$_}})
        foreach($directory in $directories){if($directory.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Package directory reparse point not followed'};$path=Join-Path $directory.FullName 'package.json';if(Test-Path -LiteralPath $path){$file=Get-Item -LiteralPath $path;if($file.Length -gt 1048576 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Package manifest outside read policy'};$package=Get-Content -Raw -LiteralPath $path|ConvertFrom-Json;Item $directory.FullName @{name=$package.name;version=$package.version;path=$directory.FullName;basis='standard global npm directory; custom prefixes not scanned'}}}
    }
}
```

### pcscope.apps.onedrive-registration · plugins/powershell-adapter/domains/applications.ps1:11

```powershell
Emit 'apps.onedrive-registration' {Registry CurrentUser 'Software\Microsoft\OneDrive\Accounts' @('UserFolder','Business','DisplayName','AccountName','ConfiguredTenantId','TenantId','UserEmail') 1;Registry CurrentUser 'Software\SyncEngines\Providers\OneDrive' @('MountPoint','DisplayName','NamespaceID') 1;Registry LocalMachine 'Software\Policies\Microsoft\OneDrive' @('DisableFileSyncNGSC','FilesOnDemandEnabled','SilentAccountConfig') 0}
```

### pcscope.apps.chromium-extensions · plugins/powershell-adapter/domains/applications.ps1:12

```powershell
Emit 'apps.chromium-extensions' {
    foreach($browser in @(@{name='Chrome';path=(Join-Path $env:LOCALAPPDATA 'Google/Chrome/User Data')},@{name='Edge';path=(Join-Path $env:LOCALAPPDATA 'Microsoft/Edge/User Data')})){
        if(-not (Test-Path -LiteralPath $browser.path)){continue}
        foreach($profile in @(Get-ChildItem -LiteralPath $browser.path -Directory|Where-Object {$_.Name -eq 'Default' -or $_.Name -match '^Profile [0-9]+$'})){
            if($profile.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Browser profile is a reparse point; not followed'}
            $settings=@{};$settingsStatus='not_provided'
            foreach($settingsName in @('Preferences','Secure Preferences')){
                $settingsPath=Join-Path $profile.FullName $settingsName
                if(Test-Path -LiteralPath $settingsPath){
                    $settingsFile=Get-Item -LiteralPath $settingsPath
                    if($settingsFile.Length -gt 16777216 -or ($settingsFile.Attributes -band [IO.FileAttributes]::ReparsePoint)){$settingsStatus='preferences_outside_read_policy';continue}
                    try{
                        $preferences=Get-Content -Raw -LiteralPath $settingsPath|ConvertFrom-Json
                        foreach($property in $preferences.extensions.settings.PSObject.Properties){$settings[$property.Name]=$property.Value}
                        $settingsStatus='preferences_read'
                    }catch{$settingsStatus='preferences_unavailable'}
                }
            }
            $extensions=Join-Path $profile.FullName 'Extensions';if(-not (Test-Path -LiteralPath $extensions)){continue}
            foreach($extension in @(Get-ChildItem -LiteralPath $extensions -Directory)){
                if($extension.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Extension directory is a reparse point'}
                foreach($version in @(Get-ChildItem -LiteralPath $extension.FullName -Directory)){
                    if($version.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Extension version is a reparse point'}
                    $manifest=Join-Path $version.FullName 'manifest.json';if(-not (Test-Path -LiteralPath $manifest)){continue}
                    $file=Get-Item -LiteralPath $manifest;if($file.Length -gt 1048576 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Extension manifest not safely readable'}
                    $data=Get-Content -Raw -LiteralPath $manifest|ConvertFrom-Json
                    $configuration=$settings[$extension.Name]
                    Item "$($browser.name)/$($profile.Name)/$($extension.Name)/$($version.Name)" @{browser=$browser.name;profile=$profile.Name;extension_id=$extension.Name;name=$data.name;version=$data.version;manifest_version=$data.manifest_version;description=(Protect-Text $data.description);permissions=@($data.permissions);host_permissions=@($data.host_permissions);optional_permissions=@($data.optional_permissions);recorded_state=$configuration.state;disable_reasons=$configuration.disable_reasons;registered_path=$configuration.path;preferences_status=$settingsStatus;basis='on-disk manifest and recorded preferences; state code is not proof of current activation'}
                }
            }
        }
    }
}
```

### pcscope.apps.firefox-extensions · plugins/powershell-adapter/domains/applications.ps1:45

```powershell
Emit 'apps.firefox-extensions' {
    $base=Join-Path $env:APPDATA 'Mozilla/Firefox/Profiles';if(Test-Path -LiteralPath $base){foreach($profile in @(Get-ChildItem -LiteralPath $base -Directory)){
        if($profile.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Firefox profile reparse point not followed'}
        $path=Join-Path $profile.FullName 'extensions.json';if(Test-Path -LiteralPath $path){$file=Get-Item -LiteralPath $path;if($file.Length -gt 16777216 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Firefox extension index exceeds read policy'};$data=Get-Content -Raw -LiteralPath $path|ConvertFrom-Json;foreach($addon in $data.addons){Item "$($profile.Name)/$($addon.id)" @{profile=$profile.Name;id=$addon.id;version=$addon.version;type=$addon.type;active=$addon.active;user_disabled=$addon.userDisabled;app_disabled=$addon.appDisabled;signed_state=$addon.signedState;install_date=$addon.installDate;update_date=$addon.updateDate;basis='Firefox extension registry'}}}
    }}
}
```

### pcscope.desktop.printing · plugins/powershell-adapter/domains/desktop.ps1:1

```powershell
Emit 'desktop.printing' {Get-Printer | ForEach-Object {Item $_.Name (Fields $_ @('Name','Type','DriverName','PortName','Shared','ShareName','Published','PrinterStatus','RenderingMode'))}}
```

### pcscope.desktop.printer-drivers · plugins/powershell-adapter/domains/desktop.ps1:2

```powershell
Emit 'desktop.printer-drivers' {Get-PrinterDriver | ForEach-Object {Item "$($_.Name)/$($_.PrinterEnvironment)" (Fields $_ @('Name','Manufacturer','PrinterEnvironment','MajorVersion','DriverPath','InfPath','ConfigFile','DataFile'))}}
```

### pcscope.desktop.printer-ports · plugins/powershell-adapter/domains/desktop.ps1:3

```powershell
Emit 'desktop.printer-ports' {Get-PrinterPort | ForEach-Object {Item $_.Name (Fields $_ @('Name','Description','PortMonitor','PrinterHostAddress','PortNumber','Protocol','SNMPEnabled'))}}
```

### pcscope.desktop.print-jobs · plugins/powershell-adapter/domains/desktop.ps1:4

```powershell
Emit 'desktop.print-jobs' {Get-Printer|ForEach-Object {$printer=$_;Get-PrintJob -PrinterName $printer.Name|ForEach-Object {Item "$($printer.Name)/$($_.ID)/$(Iso $_.SubmittedTime)" @{printer=$printer.Name;id=$_.ID;submitted_at=(Iso $_.SubmittedTime);status=[string]$_.JobStatus;size=$_.Size;total_pages=$_.TotalPages;pages_printed=$_.PagesPrinted;document_name=(Protect-Text $_.DocumentName);document_content_collected=$false;content_exclusion_reason='print_metadata_only_no_document_content'}}}}
```

### pcscope.desktop.input · plugins/powershell-adapter/domains/desktop.ps1:5

```powershell
Emit 'desktop.input' {Registry CurrentUser 'Keyboard Layout' @('','1','2','3','4','5') 2;Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Keyboard Layouts' @('Layout File','Layout Text','Layout Id','Ime File') 1;Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Class\{4d36e96b-e325-11ce-bfc1-08002be10318}' @('UpperFilters','LowerFilters') 1}
```

### pcscope.desktop.clipboard · plugins/powershell-adapter/domains/desktop.ps1:6

```powershell
Emit 'desktop.clipboard' {Registry CurrentUser 'Software\Microsoft\Clipboard' @('EnableClipboardHistory','CloudClipboardAutomaticUpload','EnableCloudClipboard');Registry LocalMachine 'Software\Policies\Microsoft\Windows\System' @('AllowClipboardHistory','AllowCrossDeviceClipboard')}
```

### pcscope.desktop.shell-settings · plugins/powershell-adapter/domains/desktop.ps1:7

```powershell
Emit 'desktop.shell-settings' {Registry CurrentUser 'Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' @('Hidden','HideFileExt','ShowSuperHidden','LaunchTo','TaskbarAl','ShowTaskViewButton');Registry CurrentUser 'Control Panel\Desktop' @('Wallpaper','WallpaperStyle','TileWallpaper','ScreenSaveActive','ScreenSaveTimeOut');Registry CurrentUser 'Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders' @('Desktop','Personal','Start Menu','Startup','Programs','AppData','Local AppData')}
```

### pcscope.desktop.shortcuts · plugins/powershell-adapter/domains/desktop.ps1:8

```powershell
Emit 'desktop.shortcuts' {
    $shell=New-Object -ComObject WScript.Shell
    try{foreach($folder in @('DesktopDirectory','CommonDesktopDirectory','StartMenu','CommonStartMenu')){
        $path=[Environment]::GetFolderPath([Environment+SpecialFolder]::$folder);if(-not $path){continue};if($path.StartsWith('\\')){throw 'Network shell folder not followed'}
        $queue=[Collections.Generic.Stack[string]]::new();$queue.Push($path);$count=0
        while($queue.Count){$current=$queue.Pop();foreach($entry in @(Get-ChildItem -LiteralPath $current -Force)){
            if($entry.Attributes -band [IO.FileAttributes]::ReparsePoint){$data=@{path=$entry.FullName;attributes=[string]$entry.Attributes;collection=@{reparse_target_followed=$false;reason='reparse_target_outside_scan_policy'}};Add-FileIdentity $data $entry.FullName|Out-Null;Item $entry.FullName.ToLowerInvariant() $data;continue}
            if($entry.PSIsContainer){$queue.Push($entry.FullName);continue};if($entry.Extension -ine '.lnk'){continue}
            $count++;if($count -gt 10000){throw 'Shortcut inventory limit reached'}
            $link=$shell.CreateShortcut($entry.FullName)
            try{$data=@{path=$entry.FullName;target=$link.TargetPath;working_directory=$link.WorkingDirectory;arguments=(Protect-Text ([string]$link.Arguments));description=$link.Description;window_style=$link.WindowStyle;icon=$link.IconLocation;hotkey=$link.Hotkey;basis='existing shortcut metadata; target never executed'};Add-FileIdentity $data $entry.FullName|Out-Null;Item $entry.FullName.ToLowerInvariant() $data}finally{[void][Runtime.InteropServices.Marshal]::ReleaseComObject($link)}
        }}
    }}finally{[void][Runtime.InteropServices.Marshal]::ReleaseComObject($shell)}
}
```

### pcscope.network.routes · plugins/powershell-adapter/domains/network.ps1:1

```powershell
Emit 'network.routes' {Get-NetRoute | ForEach-Object {Item "$($_.InterfaceIndex)/$($_.DestinationPrefix)/$($_.NextHop)/$($_.PolicyStore)" (Fields $_ @('InterfaceIndex','InterfaceAlias','DestinationPrefix','NextHop','RouteMetric','Protocol','AddressFamily','PolicyStore','Publish'))}}
```

### pcscope.network.dns-configuration · plugins/powershell-adapter/domains/network.ps1:2

```powershell
Emit 'network.dns-configuration' {Get-DnsClientServerAddress | ForEach-Object {Item "$($_.InterfaceIndex)/$($_.AddressFamily)" (Fields $_ @('InterfaceIndex','InterfaceAlias','AddressFamily','ServerAddresses'))};Registry LocalMachine 'SYSTEM\CurrentControlSet\Services\Tcpip\Parameters' @('Domain','SearchList','NV Domain','UseDomainNameDevolution')}
```

### pcscope.network.profiles · plugins/powershell-adapter/domains/network.ps1:3

```powershell
Emit 'network.profiles' {Get-NetConnectionProfile | ForEach-Object {Item "$($_.InterfaceIndex)/$($_.Name)" (Fields $_ @('InterfaceIndex','Name','NetworkCategory','IPv4Connectivity','IPv6Connectivity'))}}
```

### pcscope.network.tcp · plugins/powershell-adapter/domains/network.ps1:4

```powershell
Emit 'network.tcp' {Get-NetTCPConnection | ForEach-Object {Item "$($_.LocalAddress)/$($_.LocalPort)/$($_.RemoteAddress)/$($_.RemotePort)/$($_.OwningProcess)/$(Iso $_.CreationTime)" (Fields $_ @('LocalAddress','LocalPort','RemoteAddress','RemotePort','State','OwningProcess','CreationTime','AppliedSetting'))}}
```

### pcscope.network.udp · plugins/powershell-adapter/domains/network.ps1:5

```powershell
Emit 'network.udp' {Get-NetUDPEndpoint | ForEach-Object {Item "$($_.LocalAddress)/$($_.LocalPort)/$($_.OwningProcess)/$(Iso $_.CreationTime)" (Fields $_ @('LocalAddress','LocalPort','OwningProcess','CreationTime'))}}
```

### pcscope.network.firewall-profiles · plugins/powershell-adapter/domains/network.ps1:6

```powershell
Emit 'network.firewall-profiles' {Get-NetFirewallProfile | ForEach-Object {Item $_.Name (Fields $_ @('Name','Enabled','DefaultInboundAction','DefaultOutboundAction','AllowInboundRules','AllowLocalFirewallRules','AllowLocalIPsecRules','LogAllowed','LogBlocked','LogFileName','LogMaxSizeKilobytes'))}}
```

### pcscope.network.firewall-rules · plugins/powershell-adapter/domains/network.ps1:7

```powershell
Emit 'network.firewall-rules' {
    $filters=@{}
    foreach($type in @('Port','Address','Application','Service','Interface','InterfaceType','Security')){
        $command='Get-NetFirewall'+$type+'Filter';$map=@{}
        & $command -PolicyStore ActiveStore | ForEach-Object {$map[$_.InstanceID]=$_}
        $filters[$type]=$map
    }
    Get-NetFirewallRule -PolicyStore ActiveStore | ForEach-Object {
        $rule=$_;$data=Fields $rule @('Name','DisplayName','Enabled','Direction','Action','Profile','EdgeTraversalPolicy','PolicyStoreSource','PolicyStoreSourceType','PrimaryStatus','Status')
        $data.filters=[ordered]@{}
        foreach($type in $filters.Keys){$filter=$filters[$type][$rule.InstanceID];if($null -eq $filter){throw "Missing $type filter for rule $($rule.Name)"};$fields=switch($type){Port {@('Protocol','LocalPort','RemotePort','IcmpType','DynamicTarget')};Address {@('LocalAddress','RemoteAddress')};Application {@('Program','Package')};Service {@('Service')};Interface {@('InterfaceAlias')};InterfaceType {@('InterfaceType')};Security {@('Authentication','Encryption','OverrideBlockRules','LocalUser','RemoteUser','RemoteMachine')}};$data.filters[$type]=Fields $filter $fields}
        Item $rule.Name $data
    }
}
```

### pcscope.network.ipsec · plugins/powershell-adapter/domains/network.ps1:21

```powershell
Emit 'network.ipsec' {Get-NetIPsecRule -PolicyStore ActiveStore | ForEach-Object {Item $_.Name (Fields $_ @('Name','DisplayName','Enabled','Profile','Mode','InboundSecurity','OutboundSecurity','PolicyStoreSource','PolicyStoreSourceType'))}}
```

### pcscope.network.access · plugins/powershell-adapter/domains/network.ps1:22

```powershell
Emit 'network.access' {Registry CurrentUser 'Software\Microsoft\Windows\CurrentVersion\Internet Settings' @('ProxyEnable','ProxyOverride','AutoDetect') 0;Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Terminal Server' @('fDenyTSConnections') 0;Get-VpnConnection | ForEach-Object {Item $_.Name (Fields $_ @('Name','TunnelType','ConnectionStatus','SplitTunneling','AllUserConnection','AuthenticationMethod'))}}
```

### pcscope.network.shares · plugins/powershell-adapter/domains/network.ps1:23

```powershell
Emit 'network.shares' {Get-SmbShare | ForEach-Object {$share=$_;Item $share.Name @{name=$share.Name;path=$share.Path;description=$share.Description;encrypt_data=$share.EncryptData;special=$share.Special;access=@(Get-SmbShareAccess -Name $share.Name|ForEach-Object {Fields $_ @('AccountName','AccessControlType','AccessRight')})}}}
```

### pcscope.network.mapped-drives · plugins/powershell-adapter/domains/network.ps1:24

```powershell
Emit 'network.mapped-drives' {Get-SmbMapping | ForEach-Object {Item "$($_.LocalPath)/$($_.RemotePath)" (Fields $_ @('LocalPath','RemotePath','Status'))}}
```

### pcscope.network.dns-cache · plugins/powershell-adapter/domains/network.ps1:25

```powershell
Emit 'network.dns-cache' {Get-DnsClientCache|ForEach-Object {Item "$($_.Entry)/$($_.Name)/$($_.Type)/$($_.Data)" @{entry=$_.Entry;name=$_.Name;type=[string]$_.Type;data=$_.Data;status=[string]$_.Status;section=[string]$_.Section;basis='cache snapshot; not query time or complete DNS traffic'}}}
```

### pcscope.network.hosts · plugins/powershell-adapter/domains/network.ps1:26

```powershell
Emit 'network.hosts' {$path=Join-Path $env:SystemRoot 'System32/drivers/etc/hosts';$file=Get-Item -LiteralPath $path;if($file.Length -gt 1048576 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Hosts file outside safe read policy'};$lineNumber=0;foreach($line in @(Get-Content -LiteralPath $path)){$lineNumber++;$text=($line -split '#',2)[0].Trim();if(-not $text){continue};$parts=@($text -split '\s+');$address=$null;if($parts.Count -lt 2 -or -not [Net.IPAddress]::TryParse($parts[0],[ref]$address)){throw "Hosts syntax not recognized at line $lineNumber"};foreach($name in $parts[1..($parts.Count-1)]){Item "$lineNumber/$name" @{address=$address.ToString();name=$name;line=$lineNumber;basis='hosts configuration; comments excluded'}}}}
```

### pcscope.network.share-sessions · plugins/powershell-adapter/domains/network.ps1:27

```powershell
Emit 'network.share-sessions' {Get-SmbSession | ForEach-Object {Item ([string]$_.SessionId) (Fields $_ @('SessionId','ClientComputerName','ClientUserName','NumOpens','Dialect','Encrypted','ContinuouslyAvailable'))}}
```

### pcscope.network.winrm-configuration · plugins/powershell-adapter/domains/network.ps1:28

```powershell
Emit 'network.winrm-configuration' {Registry LocalMachine 'SOFTWARE\Microsoft\Windows\CurrentVersion\WSMAN' @('AllowUnencrypted','Basic','Kerberos','Negotiate','Certificate','CredSSP','IPv4Filter','IPv6Filter','EnableCompatibilityHttpListener','EnableCompatibilityHttpsListener','Port','Transport','Enabled') 3}
```

### pcscope.collector.scoped-configuration · plugins/powershell-adapter/domains/scoped.ps1:51

```powershell
Emit 'collector.scoped-configuration' {Item 'options' @{watch_files=$watchFiles;file_roots=@();portable_roots=@($options.portable_roots);module_process_names=@($options.module_process_names);hash_files=$options.hash_files;max_files_per_root=$options.max_files_per_root;max_hash_bytes=$options.max_hash_bytes;docker_metadata=[bool]$options.docker_metadata;mihomo_metadata=[bool]$options.mihomo_metadata;content_versions='not implemented';basis='explicit file allowlist from plugin configuration; empty list means no file monitoring; legacy file_roots ignored'}}
```

### pcscope.apps.mihomo · plugins/powershell-adapter/domains/scoped.ps1:52

```powershell
Emit 'apps.mihomo' {
    if($options.mihomo_port -lt 1 -or $options.mihomo_port -gt 65535){throw 'Invalid Mihomo loopback port'}
    $headers=@{};if($env:PCSCOPE_MIHOMO_TOKEN){$headers.Authorization='Bearer '+$env:PCSCOPE_MIHOMO_TOKEN}
    $base='http://127.0.0.1:'+[string]$options.mihomo_port
    $version=Invoke-RestMethod -Uri ($base+'/version') -Headers $headers -Method Get -TimeoutSec 5 -MaximumRedirection 0
    $config=Invoke-RestMethod -Uri ($base+'/configs') -Headers $headers -Method Get -TimeoutSec 5 -MaximumRedirection 0
    Item 'runtime' @{version=$version.version;meta=$version.meta;mode=$config.mode;log_level=$config.'log-level';ipv6=$config.ipv6;allow_lan=$config.'allow-lan';port=$config.port;socks_port=$config.'socks-port';mixed_port=$config.'mixed-port';tun_enabled=$config.tun.enable;basis='explicit local Mihomo API; no subscription, node credentials or traffic content'}
}
```

### pcscope.virtualization.docker · plugins/powershell-adapter/domains/scoped.ps1:60

```powershell
Emit 'virtualization.docker' {
    $docker=Join-Path $env:ProgramFiles 'Docker/Docker/resources/bin/docker.exe'
    if(-not (Test-Path -LiteralPath $docker)){throw 'Supported Docker Desktop CLI is not installed'}
    $endpoint='npipe:////./pipe/docker_engine'
    foreach($spec in @(
        @{type='container';args=@('container','ls','--all','--no-trunc');template='{"id":{{json .ID}},"names":{{json .Names}},"image":{{json .Image}},"state":{{json .State}},"status":{{json .Status}}}'},
        @{type='image';args=@('image','ls','--no-trunc');template='{"id":{{json .ID}},"repository":{{json .Repository}},"tag":{{json .Tag}}}'},
        @{type='volume';args=@('volume','ls');template='{"id":{{json .Name}},"driver":{{json .Driver}}}'},
        @{type='network';args=@('network','ls','--no-trunc');template='{"id":{{json .ID}},"name":{{json .Name}},"driver":{{json .Driver}},"scope":{{json .Scope}}}'})
    ){
        $arguments=@('--host',$endpoint)+$spec.args+@('--format',$spec.template)
        $lines=@(& $docker @arguments 2>$null);if($LASTEXITCODE -ne 0){throw "Docker $($spec.type) query failed"}
        foreach($line in $lines){$data=ConvertFrom-Json $line;$identity=if($spec.type -eq 'image'){"$($data.id)/$($data.repository)/$($data.tag)"}else{$data.id};Item "$($spec.type)/$identity" @{kind=$spec.type;data=$data;basis='local Docker API metadata; no logs, arguments, environment or container contents'}}
    }
}
```

### pcscope.files.metadata-security · plugins/powershell-adapter/domains/scoped.ps1:75

```powershell
Emit 'files.metadata-security' {Scan-WatchedFile $path}
```

### pcscope.software.portable · plugins/powershell-adapter/domains/scoped.ps1:76

```powershell
Emit 'software.portable' {Scan-Root $path $true}
```

### pcscope.execution.modules · plugins/powershell-adapter/domains/scoped.ps1:78

```powershell
Emit 'execution.modules' {foreach($name in @($options.module_process_names)){if($name -match '[*?\[\]]'){throw 'Wildcard process selections are not supported'};foreach($process in @(Get-Process|Where-Object ProcessName -eq $name)){try{$started=Iso $process.StartTime;foreach($module in $process.Modules){Item "$($process.Id)/$started/$($module.FileName)" @{pid=$process.Id;process_start=$started;process_name=$name;module_path=$module.FileName;size=$module.ModuleMemorySize;version=$module.FileVersionInfo.FileVersion;company=$module.FileVersionInfo.CompanyName;basis='selected process module snapshot'}}}finally{$process.Dispose()}}}}
```

### pcscope.security.protection · plugins/powershell-adapter/domains/security.ps1:1

```powershell
Emit 'security.protection' {Get-MpComputerStatus | ForEach-Object {Item 'defender' (Fields $_ @('AMServiceEnabled','AntispywareEnabled','AntivirusEnabled','BehaviorMonitorEnabled','IoavProtectionEnabled','NISEnabled','OnAccessProtectionEnabled','RealTimeProtectionEnabled','IsTamperProtected','AMEngineVersion','AMProductVersion','AntivirusSignatureVersion','AntivirusSignatureLastUpdated','QuickScanEndTime','FullScanEndTime'))}}
```

### pcscope.security.protection-policy · plugins/powershell-adapter/domains/security.ps1:2

```powershell
Emit 'security.protection-policy' {Get-MpPreference | ForEach-Object {Item 'defender-policy' (Fields $_ @('DisableRealtimeMonitoring','DisableBehaviorMonitoring','DisableIOAVProtection','DisableScriptScanning','PUAProtection','ExclusionPath','ExclusionProcess','ExclusionExtension'))}}
```

### pcscope.security.products · plugins/powershell-adapter/domains/security.ps1:3

```powershell
Emit 'security.products' {Cim-Items AntiVirusProduct instanceGuid @('instanceGuid','displayName','productState','timestamp') 'root/SecurityCenter2'}
```

### pcscope.security.detections · plugins/powershell-adapter/domains/security.ps1:4

```powershell
Emit 'security.detections' {Get-MpThreatDetection | ForEach-Object {Item ([string]$_.DetectionID) (Fields $_ @('DetectionID','ThreatID','InitialDetectionTime','LastThreatStatusChangeTime','ActionSuccess','CleaningActionID','ThreatStatusID','CurrentThreatExecutionStatusID'))}}
```

### pcscope.security.deviceguard · plugins/powershell-adapter/domains/security.ps1:5

```powershell
Emit 'security.deviceguard' {Cim-Items Win32_DeviceGuard InstanceIdentifier @('InstanceIdentifier','VirtualizationBasedSecurityStatus','SecurityServicesConfigured','SecurityServicesRunning','AvailableSecurityProperties','RequiredSecurityProperties','CodeIntegrityPolicyEnforcementStatus') 'root/Microsoft/Windows/DeviceGuard'}
```

### pcscope.security.tpm · plugins/powershell-adapter/domains/security.ps1:6

```powershell
Emit 'security.tpm' {Get-Tpm | ForEach-Object {Item 'tpm' (Fields $_ @('TpmPresent','TpmReady','TpmEnabled','TpmActivated','TpmOwned','RestartPending','ManufacturerIdTxt','ManufacturerVersion','LockedOut'))}}
```

### pcscope.security.secureboot · plugins/powershell-adapter/domains/security.ps1:7

```powershell
Emit 'security.secureboot' {Item 'secureboot' @{enabled=(Confirm-SecureBootUEFI)}}
```

### pcscope.security.certificates.$($location.ToLowerInvariant()) · plugins/powershell-adapter/domains/security.ps1:8

```powershell
Emit "security.certificates.$($location.ToLowerInvariant())" {
    foreach($storeName in @('Root','CA','My','TrustedPublisher','Disallowed','TrustedPeople')){
        $store=[Security.Cryptography.X509Certificates.X509Store]::new($storeName,[Security.Cryptography.X509Certificates.StoreLocation]::$location)
        try{$store.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadOnly -bor [Security.Cryptography.X509Certificates.OpenFlags]::OpenExistingOnly);foreach($cert in $store.Certificates){Item "$storeName/$($cert.Thumbprint)" @{store=$storeName;location=$location;thumbprint=$cert.Thumbprint;subject=$cert.Subject;issuer=$cert.Issuer;not_before=(Iso $cert.NotBefore);not_after=(Iso $cert.NotAfter);has_private_key=$cert.HasPrivateKey;signature_algorithm=$cert.SignatureAlgorithm.Value;trust='not evaluated'}}}finally{$store.Close()}
    }
}
```

### pcscope.security.encryption · plugins/powershell-adapter/domains/security.ps1:14

```powershell
Emit 'security.encryption' {Get-BitLockerVolume | ForEach-Object {Item $_.MountPoint @{mount_point=$_.MountPoint;volume_type=[string]$_.VolumeType;volume_status=[string]$_.VolumeStatus;protection_status=[string]$_.ProtectionStatus;encryption_method=[string]$_.EncryptionMethod;encryption_percentage=$_.EncryptionPercentage;lock_status=[string]$_.LockStatus;protector_types=@($_.KeyProtector|ForEach-Object {[string]$_.KeyProtectorType}|Sort-Object);recovery_material_collected=$false}}}
```

### pcscope.policy.configuration · plugins/powershell-adapter/domains/security.ps1:15

```powershell
Emit 'policy.configuration' {Registry LocalMachine 'SOFTWARE\Microsoft\Windows\CurrentVersion\Policies' @('EnableLUA','ConsentPromptBehaviorAdmin','ConsentPromptBehaviorUser','PromptOnSecureDesktop','EnableInstallerDetection','FilterAdministratorToken') 3;Registry CurrentUser 'SOFTWARE\Microsoft\Windows\CurrentVersion\Policies' @() 3;Registry LocalMachine 'SOFTWARE\Policies\Microsoft\Windows\Safer' @() 3}
```

### pcscope.security.ssh-public-keys · plugins/powershell-adapter/domains/security.ps1:16

```powershell
Emit 'security.ssh-public-keys' {
    $directory=Join-Path $env:USERPROFILE '.ssh';if(Test-Path -LiteralPath $directory){foreach($file in @(Get-ChildItem -LiteralPath $directory -Filter '*.pub' -File)){
        if($file.Length -gt 65536 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Public key file outside safe read policy'}
        $line=[IO.File]::ReadAllText($file.FullName).Trim();$parts=@($line -split '\s+',3)
        if($parts.Count -lt 2 -or $parts[0] -notmatch '^(ssh-(rsa|ed25519)|ecdsa-sha2-nistp(256|384|521)|sk-ssh-ed25519@openssh.com|sk-ecdsa-sha2-nistp256@openssh.com)$'){throw 'Unsupported public-key encoding; file content not retained'}
        $sha=[Security.Cryptography.SHA256]::Create();try{$digest=$sha.ComputeHash([Convert]::FromBase64String($parts[1]));$fingerprint='SHA256:'+[Convert]::ToBase64String($digest).TrimEnd('=')}finally{$sha.Dispose()}
        Item $file.FullName @{path=$file.FullName;algorithm=$parts[0];fingerprint=$fingerprint;comment_collected=$false;key_material_collected=$false}
    }}
}
```

### pcscope.collector.event-channel-coverage · plugins/powershell-adapter/domains/security.ps1:25

```powershell
Emit 'collector.event-channel-coverage' {foreach($channel in (Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot '../channels.json')|ConvertFrom-Json)){try{$log=Get-WinEvent -ListLog $channel -ErrorAction Stop;Item $channel @{channel=$channel;readable=$true;enabled=$log.IsEnabled;log_mode=[string]$log.LogMode;maximum_bytes=$log.MaximumSizeInBytes;record_count=$log.RecordCount;oldest_record=$log.OldestRecordNumber;basis='channel metadata; enabled does not imply audit policy coverage'}}catch{Item $channel @{channel=$channel;readable=$false;reason=$_.Exception.Message;basis='capability check; not an empty history'}}}}
```

### pcscope.startup.service-configuration · plugins/powershell-adapter/domains/startup.ps1:1

```powershell
Emit 'startup.service-configuration' {Registry LocalMachine 'SYSTEM\CurrentControlSet\Services' @('Start','Type','ErrorControl','ObjectName','DisplayName','Description','DelayedAutoStart','DependOnService','DependOnGroup','ServiceDll','ServiceMain','FailureActionsOnNonCrashFailures','PreshutdownTimeout') 2}
```

### pcscope.startup.session-hooks · plugins/powershell-adapter/domains/startup.ps1:2

```powershell
Emit 'startup.session-hooks' {Registry LocalMachine 'SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon' @('Shell','Userinit') 0;Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Session Manager' @('BootExecute') 0;foreach($hive in @('LocalMachine','CurrentUser')){Registry $hive 'Software\Microsoft\Active Setup\Installed Components' @('Version','IsInstalled','Locale') 1;Registry $hive 'Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run' @() 0}}
```

### pcscope.startup.advanced-extensions · plugins/powershell-adapter/domains/startup.ps1:3

```powershell
Emit 'startup.advanced-extensions' {foreach($path in @('SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options','SOFTWARE\Microsoft\Windows NT\CurrentVersion\SilentProcessExit','SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows','SYSTEM\CurrentControlSet\Control\Session Manager\AppCertDlls')){Registry LocalMachine $path @('GlobalFlag','UseFilter','VerifierFlags','LoadAppInit_DLLs','RequireSignedAppInit_DLLs','ReportingMode','MonitorProcess') 2}}
```

### pcscope.startup.wmi-filters · plugins/powershell-adapter/domains/startup.ps1:4

```powershell
Emit 'startup.wmi-filters' {Get-CimInstance -Namespace root/subscription -ClassName __EventFilter | ForEach-Object {Item $_.Name @{name=$_.Name;event_namespace=$_.EventNamespace;query_language=$_.QueryLanguage;query=(Protect-Text $_.Query);source_properties=(Public-CimFields $_)}}}
```

### pcscope.startup.wmi-consumers · plugins/powershell-adapter/domains/startup.ps1:5

```powershell
Emit 'startup.wmi-consumers' {Get-CimInstance -Namespace root/subscription -ClassName __EventConsumer | ForEach-Object {Item "$($_.CimClass.CimClassName)/$($_.Name)" @{name=$_.Name;class=$_.CimClass.CimClassName;command=(Protect-Text ([string]$_.CommandLineTemplate));script=(Protect-Value ScriptText $_.ScriptText);script_engine=$_.ScriptingEngine;executable_path=$_.ExecutablePath;source_properties=(Public-CimFields $_)}}}
```

### pcscope.startup.wmi-bindings · plugins/powershell-adapter/domains/startup.ps1:6

```powershell
Emit 'startup.wmi-bindings' {Get-CimInstance -Namespace root/subscription -ClassName __FilterToConsumerBinding | ForEach-Object {Item "$($_.Filter)/$($_.Consumer)" (Fields $_ @('Filter','Consumer','MaintainSecurityContext','SlowDownProviders','DeliverSynchronously'))}}
```

### pcscope.startup.task-details · plugins/powershell-adapter/domains/startup.ps1:7

```powershell
Emit 'startup.task-details' {Get-ScheduledTask | ForEach-Object {$task=$_;$info=Get-ScheduledTaskInfo -InputObject $task;Item ($task.TaskPath+$task.TaskName) @{path=$task.TaskPath;name=$task.TaskName;last_run=(Iso $info.LastRunTime);last_result=$info.LastTaskResult;next_run=(Iso $info.NextRunTime);missed_runs=$info.NumberOfMissedRuns;triggers=@($task.Triggers|ForEach-Object {Public-CimFields $_});conditions=(Public-CimFields $task.Settings);principal=(Public-CimFields $task.Principal);registration=(Fields $task @('Author','Description','URI','Version','Source'));actions=@($task.Actions|ForEach-Object {Public-CimFields $_})}}}
```

### pcscope.environment.configuration · plugins/powershell-adapter/domains/startup.ps1:8

```powershell
Emit 'environment.configuration' {Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment' @('Path','PATHEXT','TEMP','TMP','windir','OS','PROCESSOR_ARCHITECTURE','NUMBER_OF_PROCESSORS');Registry CurrentUser 'Environment' @('Path','TEMP','TMP');Registry CurrentUser 'Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts' @('ProgId','Application') 2;Registry CurrentUser 'Software\Microsoft\Windows\Shell\Associations\UrlAssociations' @('ProgId') 2}
```

### pcscope.storage.disks · plugins/powershell-adapter/domains/storage.ps1:1

```powershell
Emit 'storage.disks' {Get-Disk | ForEach-Object {Item ([string]$_.ObjectId) (Fields $_ @('ObjectId','UniqueId','Number','FriendlyName','BusType','PartitionStyle','Size','IsBoot','IsSystem','IsOffline','IsReadOnly','OperationalStatus','HealthStatus'))}}
```

### pcscope.storage.partitions · plugins/powershell-adapter/domains/storage.ps1:2

```powershell
Emit 'storage.partitions' {Get-Partition | ForEach-Object {Item ([string]$_.ObjectId) (Fields $_ @('ObjectId','DiskId','DiskNumber','PartitionNumber','Guid','Offset','Size','Type','GptType','MbrType','AccessPaths','IsActive','IsBoot','IsHidden','IsReadOnly'))}}
```

### pcscope.hardware.driver-bindings · plugins/powershell-adapter/domains/storage.ps1:3

```powershell
Emit 'hardware.driver-bindings' {Get-CimInstance Win32_PnPSignedDriver | ForEach-Object {Item $_.DeviceID (Fields $_ @('DeviceID','DeviceName','DriverVersion','DriverDate','DriverProviderName','InfName','IsSigned','Signer','DriverName'))}}
```

### pcscope.hardware.driver-services · plugins/powershell-adapter/domains/storage.ps1:4

```powershell
Emit 'hardware.driver-services' {Cim-Items Win32_SystemDriver Name @('Name','DisplayName','PathName','ServiceType','StartMode','State','Status','Started')}
```

### pcscope.hardware.firmware · plugins/powershell-adapter/domains/storage.ps1:5

```powershell
Emit 'hardware.firmware' {Cim-Items Win32_BIOS Name @('Manufacturer','Name','SMBIOSBIOSVersion','SMBIOSMajorVersion','SMBIOSMinorVersion','ReleaseDate','Version')}
```

### pcscope.hardware.disk-health · plugins/powershell-adapter/domains/storage.ps1:6

```powershell
Emit 'hardware.disk-health' {Get-PhysicalDisk | ForEach-Object {Item ([string]$_.ObjectId) (Fields $_ @('ObjectId','FriendlyName','MediaType','BusType','Size','HealthStatus','OperationalStatus','FirmwareVersion'))}}
```

### pcscope.hardware.battery · plugins/powershell-adapter/domains/storage.ps1:7

```powershell
Emit 'hardware.battery' {Cim-Items Win32_Battery DeviceID @('DeviceID','Name','Status','BatteryStatus','Chemistry','DesignCapacity','FullChargeCapacity','EstimatedChargeRemaining','EstimatedRunTime')}
```

### pcscope.power.configuration · plugins/powershell-adapter/domains/storage.ps1:8

```powershell
Emit 'power.configuration' {Cim-Items Win32_PowerPlan InstanceID @('InstanceID','ElementName','IsActive','Description') 'root/cimv2/power';Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Power' @('HibernateEnabled','HibernateEnabledDefault','CsEnabled','PlatformAoAcOverride')}
```

### pcscope.identity.machine · plugins/powershell-adapter/domains/system.ps1:1

```powershell
Emit 'identity.machine' {Cim-Items Win32_ComputerSystem Name @('Name','Domain','PartOfDomain','DomainRole','Manufacturer','Model','SystemType');Cim-Items Win32_ComputerSystemProduct UUID @('UUID','Vendor','Name','Version')}
```

### pcscope.system.clock · plugins/powershell-adapter/domains/system.ps1:2

```powershell
Emit 'system.clock' {Registry LocalMachine 'SYSTEM\CurrentControlSet\Services\W32Time' @('Type','NtpServer','Enabled','SpecialPollInterval') 3;Item 'timezone' @{id=(Get-TimeZone).Id;basis='observed configuration'}}
```

### pcscope.system.update-history · plugins/powershell-adapter/domains/system.ps1:3

```powershell
Emit 'system.update-history' {
    $session=New-Object -ComObject Microsoft.Update.Session
    try{$searcher=$session.CreateUpdateSearcher();try{$count=$searcher.GetTotalHistoryCount();$identities=@{};for($offset=0;$offset -lt $count;$offset+=128){$entries=$searcher.QueryHistory($offset,[Math]::Min(128,$count-$offset));foreach($entry in $entries){$identity="$($entry.UpdateIdentity.UpdateID)/$($entry.UpdateIdentity.RevisionNumber)/$($entry.Date.Ticks)/$($entry.Operation)/$($entry.ResultCode)/$($entry.HResult)";if(-not $identities.ContainsKey($identity)){$identities[$identity]=0};$identities[$identity]++;Item "$identity/$($identities[$identity])" @{title=$entry.Title;date=(Iso $entry.Date);operation=[int]$entry.Operation;result=[int]$entry.ResultCode;hresult=$entry.HResult;update_id=$entry.UpdateIdentity.UpdateID;revision=$entry.UpdateIdentity.RevisionNumber;basis='retained Windows Update history'}}}}finally{[void][Runtime.InteropServices.Marshal]::ReleaseComObject($searcher)}}finally{[void][Runtime.InteropServices.Marshal]::ReleaseComObject($session)}
}
```

### pcscope.system.reboot-pending · plugins/powershell-adapter/domains/system.ps1:7

```powershell
Emit 'system.reboot-pending' {foreach($path in @('SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending','SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')){Registry LocalMachine $path @()};Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Session Manager' @('PendingFileRenameOperations')}
```

### pcscope.system.restore-points · plugins/powershell-adapter/domains/system.ps1:8

```powershell
Emit 'system.restore-points' {Cim-Items SystemRestore SequenceNumber @('SequenceNumber','Description','CreationTime','RestorePointType','EventType') 'root/default'}
```

### pcscope.software.appx-user · plugins/powershell-adapter/domains/system.ps1:9

```powershell
Emit 'software.appx-user' {Get-AppxPackage | ForEach-Object {Item $_.PackageFullName (Fields $_ @('Name','PackageFullName','PackageFamilyName','Version','Architecture','Publisher','InstallLocation','IsFramework','IsBundle','NonRemovable','SignatureKind','Status'))}}
```

### pcscope.software.provisioned · plugins/powershell-adapter/domains/system.ps1:10

```powershell
Emit 'software.provisioned' {Get-AppxProvisionedPackage -Online | ForEach-Object {Item $_.PackageName (Fields $_ @('DisplayName','Version','Architecture','PackageName','InstallLocation'))}}
```

### pcscope.software.optional-features · plugins/powershell-adapter/domains/system.ps1:11

```powershell
Emit 'software.optional-features' {Get-WindowsOptionalFeature -Online | ForEach-Object {Item $_.FeatureName @{name=$_.FeatureName;state=[string]$_.State}}}
```

### pcscope.software.user-inventory · plugins/powershell-adapter/domains/system.ps1:12

```powershell
Emit 'software.user-inventory' {Registry CurrentUser 'Software\Microsoft\Windows\CurrentVersion\Uninstall' @('DisplayName','DisplayVersion','Publisher','InstallLocation','InstallDate','SystemComponent','WindowsInstaller','ReleaseType') 1}
```

### pcscope.identity.profiles · plugins/powershell-adapter/domains/system.ps1:13

```powershell
Emit 'identity.profiles' {Cim-Items Win32_UserProfile SID @('SID','LocalPath','Loaded','Special','Status','LastUseTime')}
```

### pcscope.identity.logon-sessions · plugins/powershell-adapter/domains/system.ps1:14

```powershell
Emit 'identity.logon-sessions' {Cim-Items Win32_LogonSession LogonId @('LogonId','LogonType','StartTime','AuthenticationPackage','Caption')}
```

### pcscope.identity.interactive-sessions · plugins/powershell-adapter/domains/system.ps1:15

```powershell
Emit 'identity.interactive-sessions' {if(-not ('PCScopeIdentity' -as [type])){Add-Type -Path (Join-Path $PSScriptRoot 'IdentityNative.cs')};[PCScopeIdentity]::Sessions()|ForEach-Object {Item ([string]$_.Id) @{session_id=$_.Id;station=$_.Station;state_code=$_.State;user=$_.User;domain=$_.Domain;identity_query_error=$_.QueryError;basis='WTS snapshot; logon/logoff times are supplied by event evidence'}}}
```

### pcscope.identity.assigned-rights · plugins/powershell-adapter/domains/system.ps1:16

```powershell
Emit 'identity.assigned-rights' {if(-not ('PCScopeIdentity' -as [type])){Add-Type -Path (Join-Path $PSScriptRoot 'IdentityNative.cs')};[PCScopeIdentity]::Rights()|ForEach-Object {Item $_.Sid @{sid=$_.Sid;rights=$_.Rights;basis='LSA account right assignment; not effective access on every resource'}}}
```

### pcscope.execution.processes · plugins/powershell-adapter/domains/system.ps1:17

```powershell
Emit 'execution.processes' {Get-CimInstance Win32_Process | ForEach-Object {Item "$($_.ProcessId)/$(Iso $_.CreationDate)" @{pid=$_.ProcessId;parent_pid=$_.ParentProcessId;name=$_.Name;path=$_.ExecutablePath;start_time=(Iso $_.CreationDate);session_id=$_.SessionId;command_line=(Protect-Text $_.CommandLine);command_line_collected=($null -ne $_.CommandLine);command_line_status=if($null -ne $_.CommandLine){'available_redaction_applied'}else{'not_returned_system_process_access_or_exit'};basis='snapshot; short-lived processes may be missed'}}}
```

### pcscope.execution.resources · plugins/powershell-adapter/domains/system.ps1:18

```powershell
Emit 'execution.resources' {Get-CimInstance Win32_OperatingSystem | ForEach-Object {Item 'memory' @{total_kib=$_.TotalVisibleMemorySize;free_kib=$_.FreePhysicalMemory;sampled_at=[DateTime]::UtcNow.ToString('o')}};Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor | ForEach-Object {Item "cpu/$($_.Name)" @{percent_processor_time=$_.PercentProcessorTime;sampled_at=[DateTime]::UtcNow.ToString('o')}}}
```

### pcscope.virtualization.hyperv-vms · plugins/powershell-adapter/domains/virtualization.ps1:1

```powershell
Emit 'virtualization.hyperv-vms' {Get-VM | ForEach-Object {Item ([string]$_.Id) (Fields $_ @('Id','Name','State','Generation','Version','ProcessorCount','MemoryStartup','DynamicMemoryEnabled','MemoryMinimum','MemoryMaximum','AutomaticStartAction','AutomaticStopAction','ConfigurationLocation','CheckpointType'))}}
```

### pcscope.virtualization.hyperv-switches · plugins/powershell-adapter/domains/virtualization.ps1:2

```powershell
Emit 'virtualization.hyperv-switches' {Get-VMSwitch | ForEach-Object {Item ([string]$_.Id) (Fields $_ @('Id','Name','SwitchType','NetAdapterInterfaceDescription','AllowManagementOS'))}}
```

### pcscope.virtualization.hyperv-disks · plugins/powershell-adapter/domains/virtualization.ps1:3

```powershell
Emit 'virtualization.hyperv-disks' {Get-VM | Get-VMHardDiskDrive | ForEach-Object {Item "$($_.VMId)/$($_.ControllerType)/$($_.ControllerNumber)/$($_.ControllerLocation)" (Fields $_ @('VMId','VMName','Path','ControllerType','ControllerNumber','ControllerLocation','DiskNumber'))}}
```

### pcscope.virtualization.hyperv-checkpoints · plugins/powershell-adapter/domains/virtualization.ps1:4

```powershell
Emit 'virtualization.hyperv-checkpoints' {Get-VM | Get-VMSnapshot | ForEach-Object {Item ([string]$_.Id) (Fields $_ @('Id','VMId','Name','CreationTime','SnapshotType','ParentSnapshotId'))}}
```

### pcscope.virtualization.wsl-registration · plugins/powershell-adapter/domains/virtualization.ps1:5

```powershell
Emit 'virtualization.wsl-registration' {Registry CurrentUser 'Software\Microsoft\Windows\CurrentVersion\Lxss' @('DistributionName','Version','BasePath','DefaultUid','Flags','State','DefaultDistribution') 1}
```

### 重建与验证

重建分三步，缺一步生成器会直接报错，不会产出降级文档。第一步运行 `scripts/export-collector-schema.ps1` 做只读 PowerShell AST 提取，得到 `target/collector-schema.json`。第二步用 `cargo run -p pcscope-storage --example event-audit -- <含事件历史的库>` 只读审查真实事件，得到 `target/event-audit.json`——**事件字段名只能来自真实事件，无法从源码推出**；该样本含本机元数据，只留在被忽略的 `target/` 目录，不提交，因此在另一台机器上重生成时，§5 事件示例中的字段名集合会与本册不同。第三步运行 `node scripts/build-data-display-reference.cjs`。生成器不执行采集脚本、不修改运行配置、不连接外部服务。它使用当前界面函数计算合成示例输出，并校验每个目录项存在且有例子。

| 统计 | 数量 |
| --- | --- |
| 采集目录项 | 219 |
| 插件目录分组 | 13 |
| 事件字段模板 | 16 |
| 采集项／事件规则示例 | 332 |
| 采集源码声明 | 99 |

文档由固定源码指纹标识；修改采集器、目录或界面后应重新生成。指纹只覆盖源码，不含事件审查样本，所以同样的源码在采样到不同事件时会得到不同的 §5 事件示例字段名——这是示例的取样范围差异，不是界面模板差异；§4 的模板别名始终由界面代码决定，与本样本无关。本册展示的是字段映射现状，不能替代具体 Windows 提供程序的所有版本 schema。
