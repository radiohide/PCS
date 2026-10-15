# 本机账户原生采集

2026-10-14。账户新增原生路由，当前为 120 原生 / 11 待迁移 / 2 分组，累计迁移 80 项。实际运行后台尚未更新。

后端直接打开本机 LSA/SAM，只读枚举本机账户域和 Builtin 域，按域 SID/RID 构造稳定身份。SamQueryInformationUser 使用 UserAccountInformation(5) 读取账户元数据；不查询密码哈希、私有数据或密码值。不使用 PowerShell 或 NetUser 的 PasswordAge 来估算修改时刻。字段定义见 [SAMPR_USER_ACCOUNT_INFORMATION](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-samr/7aef58f2-474a-46ab-aa2a-54f7eb8625e9) 和 [公共账户字段](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-samr/899d68a9-8145-4334-a52f-2fbd92bec94f)。

保留名称、启用状态、说明、最近登录、密码修改时间、账户到期、密码要求、允许本人修改密码、账户来源及原 created_at 空值依据。本人修改密码字段通过只读 DACL 查询保持原来源判定；账户来源直接使用系统 LsaLookupUserAccountType，保留 Local/ActiveDirectory/AzureAD/MicrosoftAccount/Unknown 的原解释，见 [Microsoft 原实现](https://github.com/PowerShell/PowerShell/blob/v7.4.0/src/Microsoft.PowerShell.LocalAccounts/LocalAccounts/Sam.cs)。系统接口不支持时不自行猜测来源。

结构已核对为 SAM 4 字节对齐：AccountInformation 192 字节、passwordLastSet 偏移 172、control 偏移 188；计数字符串和 RID 枚举保留自身布局。运行前校验布局。离线契约通过双记录数组、精确 UTC 时间、0/最大值哨兵与非法字符串拒绝测试。

只请求账户查询和读取 DACL 权限，不请求写权限。分页、数量与单快照大小受限；枚举不推进、字段错误或任何账户读取失败均拒绝整批快照，保留旧状态。所有页缓冲、句柄及 LSA 材料在成功和异常路径释放。没有脚本回退，关闭项目后不执行读取。

真实普通权限下 7 个账户的全部字段、身份、单快照 scope 与原实现一致。样本包含 1 个非空最近登录时间、4 个非空密码修改时间，当前账户到期均为空；非空到期等其它时间值通过共同时间转换的离线契约验证，但真实到期账户环境仍待验收。真实样本来源均为 Local，不代表已经覆盖域、云账户。

正式隔离无脚本调度器材料：`target/native-logon-tests/1791979022812/summary.json`。用户组回归：`target/native-logon-tests/1791978918313/summary.json`。关闭开关、Rust 归属、来源和迁移清单检查通过。账户契约命令为 `pcscope-platform-native.exe account-contract-test`；字段比对命令为 `node scripts/test-native-logon.cjs --accounts --wrapper`。本次没有创建或调整账户，没有修改正式历史库。

域/云账户、真实有到期账户、超大枚举、增强权限及目标 Windows 版本矩阵仍开放，最终部署和全量性能验收仍开放。
