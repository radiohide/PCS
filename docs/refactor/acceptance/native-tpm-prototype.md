# TPM 只读原生采集原型

2026-10-15 更新：用户授权的增强读取已完成，九字段成功，八个可安全比较字段与原审计 getter 一致，锁定状态由固定无认证查询和格式测试验证。已切换原生并通过无脚本实际路由与对象查询；硬件矩阵仍开放。详见[五项验收](native-five-enhanced-migration.md)。以下待授权/待迁移表述为此前记录。

2026-10-14。原型已编译并验证普通权限拒绝及离线状态边界，未切换生产归属。当前仍为 122 原生 / 9 待迁移 / 2 分组。

已核对本机 Microsoft.Tpm.Commands 的只读接口、原字段构造及 IL；审计仅反射元数据，没有调用 Get-Tpm。材料为 `target/tpm-read-interface-audit.txt` 和 `target/tpm-read-interface-plan.json`。发现原 Get-Tpm 构造结果时读取 OwnerAuth、运行 SelfTest，并在特定条件下调用 Enable/Disable。原生路径和安全参考必须避开这些操作，不能简单包装整个原命令后再截取字段。

原型用 Tbsi_GetDeviceInfo 判断兼容设备及代际，仅 TBS_E_TPM_NOT_FOUND 可表示未发现；访问拒绝、服务错误等都不能当作没有 TPM。接口与错误码见 [Tbsi_GetDeviceInfo](https://learn.microsoft.com/en-us/windows/win32/api/tbs/nf-tbs-tbsi_getdeviceinfo)。状态读取使用 TpmCoreProvisioning 的 TpmIsReady/Enabled/Activated/Owned、制造商 ID/版本、物理存在请求，锁定状态按代际使用固定 TBS 能力查询；不读取所有者授权、密钥，不运行自检、配置、清除、重置锁定或触发准备流程。

保留九个原字段。RestartPending 按原实现保持“物理存在请求代码非零”；它不等同于系统任意原因需要重启。制造商文字保留原四字节 ASCII/非 ASCII 数值形式，版本保留计数字符串，只允许一次有界扩容。

TPM 2.0 已删除计数阈值推断与 GetCapLockoutInfo 调用，改为一次固定、无认证会话的 TPM2_GetCapability，读取 TPM_PT_PERMANENT 的 inLockout 位。依据 [TCG TPM 2.0 Part 2，TPMA_PERMANENT](https://trustedcomputinggroup.org/wp-content/uploads/TCG_TPM2_r1p64_Part2_Structures_15may2021.pdf)。上下文只启用 TPM 2.0，未申请 raw 模式；布局依据 [TBS_CONTEXT_PARAMS2](https://learn.microsoft.com/en-us/windows/win32/api/tbs/ns-tbs-tbs_context_params2)。严格检查返回长度、能力类别、单一属性 ID 和保留位；不补读其他属性、不重试。

新增 ordinary-only `tpm-modern-lockout-assess` 验收入口，拒绝管理员令牌，只执行上述一个查询。本机普通权限实测成功，返回 LockedOut=false、elevated=false，exit=0，结果为 `target/native-hyperv-vm-fields/tpm-modern-ordinary.json`。没有 UAC、自检、授权尝试或配置修改。此证据只证明锁定标志成功读取，不代替九字段完整成功验收。

TPM 1.2 新增只读能力适配原型：通过 TBS 创建 1.2 上下文，低优先级提交固定的 TPM_GetCapability/TPM_CAP_DA_LOGIC，仅允许 key、owner、SRK 三种实体。不接受任意命令、认证材料或调用者提供的载荷，不运行自检、解锁或恢复。接口见 [TBS 提交查询](https://learn.microsoft.com/en-us/windows/win32/api/tbs/nf-tbs-tbsip_submit_command)，响应格式依据 [TCG TPM 1.2 Part 2](https://trustedcomputinggroup.org/wp-content/uploads/TPM-Main-Part-2-TPM-Structures_v1.2_rev116_01032011.pdf) 的完整/受限 DA_INFO 和显式 state 字段，不以计数推断状态。

只提交三个有界查询，每个响应限 4096 字节、不重试，最终关闭上下文；校验响应标签、总长、能力长度、动作标签、状态、保留位和厂商数据长度，厂商数据不存入采集结果。该能力在规范中为可选；不支持、权限失败、实体失败或格式未知仍拒绝快照并保留原状态，不假定未锁定。支持该能力的 1.2 硬件正常/锁定成功路径、无能力的厂商适配、安全完整字段比对仍开放，不能把这项迁移标为完成。

离线固定字节材料通过完整/受限响应、主动/非主动状态、显式状态优先于计数、固定只读命令、八类损坏、截断和不支持拒绝测试。输出在 `target/native-hyperv-vm-fields/tpm-contract.json`。此次仅解析构造字节，未调用任何真实 TPM/TBS 接口；原四项已准备增强验收的二进制和授权范围未改变。

此前完整来源的普通权限结果：`target/native-tpm-ordinary-summary.json` 和 `target/native-tpm-ordinary.jsonl`。系统返回 0x80284012 拒绝访问；原型发一个 gap、零快照，原状态保留，没有调用兼容命令。当前离线 `tpm-contract-test` 验证 16 字节设备结构、4/8 字节上下文、制造商表示、两代锁定响应、固定命令、错误属性、损坏/截断和访问错误拒绝。来源、迁移清单和文件归属检查通过。

已经准备并请求授权的增强验收仍是原先四项：可选功能、预配应用、安全启动、BitLocker。本轮没有替换那份隔离程序或扩大授权范围，TPM 未加入其中；必须继续按用户实际授权范围执行。

尚未完成：安全的成功路径参考、TPM 1.2 真实能力/厂商适配、TPM 2.0 正常/锁定/需重启状态、增强权限和目标版本实测。正式后台、TPM 配置和用户历史库均未改动。

2026-10-15 普通权限逐项诊断：新增固定 `tpm-readonly-metadata-assess`，拒绝管理员令牌，分别调用九个已审计只读接口，不调用 Get-Tpm、所有者授权、自检、启用/禁用或清除。真实运行材料在 `target/native-tpm-readonly-assessment/9bfe410447b146deb2df8726d7016097/result.json` 与 `summary.json`。设备信息确认 TPM 2；厂商、版本、Ready/Enabled/Activated/Owned 和固定 TPM 2 锁定查询均成功。唯一失败为 `TpmGetPhysicalPresenceRequest` 的 0x80284012，影响 RestartPending；不能据此将 TPM 当成不存在或全部接口不可读。

厂商版本按原 getter 的 counted UTF-16 行为保留，实际结果包含末尾 NUL；没有因方便比较而截断原字段。此诊断定位了完整快照被拒绝的具体接口，仍未证明九字段增强成功或跨硬件等价，不切换来源归属，也不把八个成功查询替代完整来源。原四项已准备 UAC 程序未增加 TPM、未修改或运行。

普通权限替代路径已实测：微软的 [Win32_Tpm.GetPhysicalPresenceRequest](https://learn.microsoft.com/en-us/windows/win32/secprov/getphysicalpresencerequest-win32-tpm) 只读取待处理物理确认操作，不需要执行 SetPhysicalPresenceRequest。诊断程序固定本机命名空间、类和方法，使用调用者身份与 PacketPrivacy，查询及调用各限 15 秒、最多一个设备，不接受外部凭据或操作参数。结果 `target/native-tpm-readonly-assessment/01ecde53023842b68d7b93b3a9ae3fcc/result.json`：新增 WMI 路径同样拒绝访问，原八个查询仍成功。本轮共十次查询，两次失败指向同一待处理状态，不能改为 false 或用一般系统待重启标志代替。

单独 TPM 增强只读程序已经冻结，准备清单 `target/native-tpm-admin-assessment-request.json`，SHA256 `BF3D1168F842D1F9A524BD70F2F3BD5CDF9D4266AC34C930E1CFB693B23B4410`，输出目录 `target/native-tpm-admin-assessment/93e263e652724bdb9879caa1b258b5c2`。固定参数 tpm-migration-assess、隔离输出目录与原账户 SID，只 Emit pcscope.security.tpm 的完整九字段读取，CreateNew 写材料及查询结束标记；结束标记不等于成功快照。校验原账户、管理员令牌、目录存在与无重解析祖先，不启动后台、不读所有者授权、不自检、不修改配置。

普通令牌预检已确认 exit=1 且不会生成来源结果或完成标记，材料 preflight.json 与 ordinary-denial.txt 保留，冻结哈希吻合。独立的一次 UAC 授权请求已呈现，尚未收到回复，未启动增强验收。原四项程序仍为原哈希和原范围，不把两份授权互相替代。新诊断/验收入口已编译，现有开发包仍保留先前版本。
