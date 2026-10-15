# 本机用户组原生采集

2026-10-14。用户组新增原生路由，当前为 119 原生 / 12 待迁移 / 2 分组，累计迁移 79 项。实际运行后台尚未更新。

编译后端通过 NetLocalGroupEnum(level 1) 读取本机组及说明、LookupAccountName 指定本机解析 SID，再通过 NetLocalGroupGetMembers(level 0) 读取完整成员 SID。保持原 name/description/members 字段、SID 身份和基础采集的单快照范围。成员排序与原结果一致；成员 SID 无需先解析成名称，域离线或孤立成员不会因为无法显示名称而被当作不存在。接口见 [NetLocalGroupEnum](https://learn.microsoft.com/en-us/windows/win32/api/lmaccess/nf-lmaccess-netlocalgroupenum) 和 [NetLocalGroupGetMembers](https://learn.microsoft.com/en-us/windows/win32/api/lmaccess/nf-lmaccess-netlocalgroupgetmembers)。

返回 MORE_DATA 时继续读取并释放每页内存；分页必须推进，页数和对象数有界。成员查询错误拒绝整个快照，保留原数据，不能提交空成员冒充读取成功。不创建、删除或调整用户组，不加载 PowerShell，无脚本失败回退。低完整性进程拒绝提交可能被过滤的清单。

普通权限下真实 20 个组的身份、名称、说明、全部成员 SID 和 scope 与原读取一致。正式调度器的无脚本隔离目录验收通过，关闭采集没有输出。最新材料为 `target/native-logon-tests/1791978485109/summary.json`。同一基础单元的登录启动项四组开关回归通过，材料为 `target/native-logon-tests/1791978433068/summary.json`。Rust 归属测试、来源库存与迁移清单验证通过。

尚待实际环境验收：域成员工作站、无法解析名称的成员、超大组分页、增强权限及目标系统版本。当前桌面用户组成功比对不能代表域控制器环境；后端明确按本机组读取。账户来源仍待迁移，密码最后修改时间不能用查询时刻减 PasswordAge 近似代替。

复验命令：`node scripts/test-native-logon.cjs --groups --wrapper`。测试只读取现有组和成员，不修改真实账户或正式数据库。
