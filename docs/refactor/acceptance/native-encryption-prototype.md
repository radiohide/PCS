# BitLocker 原生采集原型

2026-10-15 更新：增强读取一个真实卷的九字段与安全参考一致，已切换原生；实际无脚本路由和对象查询通过，不读取恢复材料。其它卷状态与保护器矩阵仍开放。详见[五项验收](native-five-enhanced-migration.md)。以下为此前原型记录。

2026-10-14。原生实现已编译，普通权限失败路径与安全参考测试一致。尚未通过增强权限实际读取验收，不切换生产归属，当前仍为 121 原生 / 10 待迁移 / 2 分组。

编译后端直接使用 MI 查询 Win32_EncryptableVolume，并与 MSFT_Volume 的 UniqueId 对应。保留有盘符与没有盘符卷的原身份、系统/数据卷分类、加密方式、进度、保护状态、锁定状态和保护方式数组。无盘符卷仍以 DeviceID 作为 MountPoint，不因界面上没有盘符而省略。

只允许 GetLockStatus、GetEncryptionMethod、GetConversionStatus、GetProtectionStatus、GetKeyProtectors 和 GetKeyProtectorType 六种固定查询。保护器 ID 仅用于读取类型，不进入输出。不调用恢复密码、外部密钥、自动解锁密钥或任何更改加密设置的方法。接口参考 [Win32_EncryptableVolume](https://learn.microsoft.com/en-us/windows/win32/secprov/win32-encryptablevolume)。

原 Get-BitLockerVolume 脚本在构造结果时会内部调用 GetKeyProtectorNumericalPassword 等查询，即使本产品最终只提取保护器类型。原生实现避开这些材料。增强测试也不使用该命令；专用安全参考脚本只读取相同的状态元数据，并使用本机 Microsoft.BitLocker.Structures 的枚举名称核对显示值。普通权限本次查询被系统拒绝，两个实现均不产生快照；没有执行恢复材料查询。

锁定卷不查询不可用的转换进度，volume_status 保持空字符串、encryption_percentage 保持 null，不默认填为“已解密”。未知枚举保留数值，算法 UINT_MAX 保持 Unknown。另修正旧脚本中的擦除挂起条件：完全加密且 WipingStatus=3 时显示 FullyEncryptedWipeSuspended；旧条件误把 ConversionStatus=6 作为前提，可能漏记挂起状态。状态定义见 [GetConversionStatus](https://learn.microsoft.com/en-us/windows/win32/secprov/getconversionstatus-win32-encryptablevolume)。

查询有 15 秒操作时限及宿主总时限，枚举、保护器数量、快照大小有上限；不循环重试。每种读取的返回码和必要字段必须有效，失败保留旧状态。不把缺少存储关联或权限不足当作卷正常、关闭或已消失。

离线契约验证完整加密/XTS、保护器排序、锁定卷进度不可用、擦除挂起和非法进度拒绝。命令为 `pcscope-platform-native.exe encryption-contract-test`。普通权限材料：`target/native-registry-tests/1791979695668/summary.json`，原生与安全参考均被拒绝、零快照。测试命令为 `node scripts/test-native-registry.cjs --encryption`。来源、迁移清单和文件归属检查通过。

尚待验收：增强权限下成功读取并逐字段核对；真实加密/解密进度、锁定、无盘符、未知算法、各种保护器和擦除状态；真实目标版本与长期性能。当前权限失败和内存样本不能代替这些成功路径，继续计为待迁移。正式后台及用户数据库没有修改。
