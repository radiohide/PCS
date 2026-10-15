# 五项增强采集原生迁移验收

2026-10-15。用户明确允许四项采集和 TPM 的只读 UAC 验收，Hyper-V 暂无环境，先完成其余来源。现为 127 原生 / 4 兼容 / 2 分组，初始为 40 / 91 / 2，累计新增 87 项原生归属。四个 Hyper-V 来源继续暂缓，不计为迁移完成。

## 真实读取及字段核对

| 来源 | 实际数量 | 完整字段 | 结果 |
|---|---:|---:|---|
| Windows 可选组件 | 134 | 2 | 原生与原参考逐项、逐字段一致 |
| 安全启动 | 1 | 1 | 原生与只读参考一致 |
| BitLocker 卷状态 | 1 | 9 | 与固定六种元数据方法参考一致，不调用会读取恢复密码的 Get-BitLockerVolume |
| TPM | 1 | 9 | 8 个可安全比较字段与原桥接类的审计后 getter 一致；LockedOut 用固定无认证能力查询及独立格式/边界测试，不运行原构造器的自检或授权尝试 |
| 预配应用 | 8 | 5 | 公开原生接口成功读取；身份、部署名称、架构和位置与独立探针一致，四段版本与 Windows 完整包身份一致 |

四项冻结程序为 `target/native-admin-migration-assessment/898c62e7faf840af920b1254b5ae8e87`；TPM 程序为 `target/native-tpm-admin-assessment/93e263e652724bdb9879caa1b258b5c2`。两个进程 exit=0、原账户、高完整性，原哈希吻合。完成标记先只表示结束，逐来源检查后才接受；首次预配来源确实失败，不将其当作空清单。

独立只读参考为 `target/native-enhanced-reference/d1b2f8d0e20747e8a4a0e4a59640138d/reference.json` 与 `validation.json`，校验脚本 `scripts/check-native-enhanced-assessment.cjs`。测试参考使用 PowerShell；产品采集后端不加载它。没有运行 Get-Tpm、TPM 自检、所有者授权读取、启用/禁用/清除或恢复密码读取，也未修改系统设置和正式数据库。

## 预配应用的接口修正

原生私有 DISM 接口和原 Get-AppxProvisionedPackage 均被系统提供程序拒绝，日志定位为 Appx 提供程序无法打开 Software 键；没有改变注册表权限、修复应用或绕过系统设置。改用 Windows 10 2004 起的公开 [FindProvisionedPackages](https://learn.microsoft.com/en-us/uwp/api/windows.management.deployment.packagemanager.findprovisionedpackages)；运行时检测接口存在，旧系统继续保留 DISM 只读路径，避免缩减较老 LTSC 的范围。公开接口查询失败不自动转脚本，不提交空快照。

五字段仍为 DisplayName、Version、Architecture、PackageName、InstallLocation；DisplayName 是部署身份名，不取本地化标题。原参考探针将 WinRT Version 结构转成类型名，不能当成有效版本参考，因此保留原材料，另明确用 Windows 完整包身份的四段版本核对原生数值组件格式。未宣称旧 DISM 在本机成功或所有系统上两种提供程序的字段完全等价。新公共路线的八个完整包及五字段通过，材料 `target/native-provisioned-public/fe4e4687387948348aaba9294aecfcf7/validation.json`，校验脚本 `scripts/check-native-provisioned-public.cjs`；旧布局和双记录/空字段契约也通过。

## 实际生产包装器与对象查询

`target/native-five-source-wrapper/09ef6f1b863942739eff42572d77abae` 的 bin 只有两个编译 exe，没有兼容脚本。普通令牌下每个来源各一个权限 gap、零快照；同账户增强令牌下每个来源均生成 16 个完整分片、零 gap，共 145 条对象记录。其余来源均禁用，没有启动正式后台或访问用户历史。脚本 `scripts/test-native-enhanced-wrapper.ps1`，两种令牌的 summary 和原始 jsonl 保留。

将增强包装器的 80 个真实信封回放到新隔离数据库，134 / 8 / 1 / 1 / 1 个对象全部绑定到原来源，并由界面同用 InstanceList 查询返回，各对象类型正确，处理积压/失败零。材料 `target/native-five-source-projection/28736-1792029094891348200/summary.json`，程序 `crates/storage/examples/native-five-source-projection.rs`。这是实际采集材料的隔离回放，不是正式历史迁移或全界面交互验收。

日志包装器已移除 PowerShell 分支，旧 backend=powershell 配置也走原生。`target/native-eventlog-only/93477d4ba688402c83c31a8a35195a04/summary.json` 证明无脚本、旧配置仍能原生读取 Application、续读无重复，缺原生宿主显式失败而不回退。新包仅保留四个 Hyper-V 来源使用的 collect-domain.ps1、evidence.ps1、domains/virtualization.ps1；未删除旧包和源代码参考。

## 范围限制

整合结果：最新包 `dist/native-five-migrated` 的 16 个运行文件哈希已核对，仅保留 Hyper-V 三个兼容脚本。新包再次通过基础/网络实际后台及受控缺席到时间线链路，材料 `target/native-package-smoke/35012-1792028957248461000/summary.json`；本轮网络仍有五个普通权限范围 gap，不能当成全范围成功。debug 运行依赖已同步，四个关键 exe 与包内哈希一致，原 pcscope-launch.json 哈希未变。没有启动正式后台或执行历史回补。

额外失败测试：删除原生宿主的独立测试目录中，五项各显式失败且不回退脚本，材料位于五项包装器目录的 `missing-native`。来源清单、127 / 4 / 2 迁移清单、采集目录、133 来源整改方案和文件归属检查通过；原方案内的 87 个后端声明及计数已按当前归属同步，保留原字段/范围/环境验收要求，不把文档覆盖检查当作全环境成功。

已完成当前环境的成功读取、普通失败、完整字段/身份契约和编译路由切换。旧 LTSC 的私有 DISM、TPM 1.2 真实硬件/厂商、各种 BitLocker 保护器/锁定/进度状态、跨版本和长期性能仍需环境矩阵验证；不能将本机数据扩大为全环境结论。正式后台不在本次验收中替换或提权，旧历史回补未执行。Hyper-V 四项按用户要求暂缓，原目标仍保留这些未完成项。
