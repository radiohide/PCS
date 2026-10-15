# 远程访问原生采集

2026-10-14。新增远程访问原生路由，当前为 121 原生 / 10 待迁移 / 2 分组，累计迁移 81 项。正在运行的后台尚未更新。

代理与远程桌面配置直接读取原注册表路径，保持双视图、深度和完整属性。VPN 通过编译后端的 MI 客户端调用系统 PS_VpnConnection.Get；PS 是提供程序类的名称，并不需要 PowerShell 引擎。只调用固定 Get 方法，不创建、连接、删除或调整 VPN。方法定义见 [系统提供程序文档](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/vpnclientpsprov/get-ps-vpnconnection)。

范围与原采集一致：Get-VpnConnection 原调用没有 AllUserConnection，只读取当前用户电话簿。本次保留该范围和 Name 身份；不把全用户 VPN 混入同一快照。AllUserConnection 原字段继续保留，意外收到全用户对象时拒绝快照。新增全用户覆盖需要明确新范围与身份后另行实现，不能暗中扩大当前数据范围。

保留 Name、TunnelType、ConnectionStatus、SplitTunneling、AllUserConnection、AuthenticationMethod 及 source_properties 和原隐私保护。先校验提供程序方法的输出结构，再读取结果。该提供程序在成功空电话簿时不返回结果参数，已经实际核对；缺少提供程序、错误状态、未知输出字段、格式错误与查询异常不能冒充空清单。操作有 15 秒查询超时，并受采集宿主总时限限制；没有重试循环或脚本回退。

正式无脚本隔离路由材料：`target/native-registry-tests/1791979411957/summary.json`。4 条真实配置登记的身份和全部字段一致，本轮原生读取 237 ms、原参考 1066 ms；单轮结果不能代表长期性能。当前用户没有 VPN 配置，因此真实 VPN 非空数据比对仍开放。

内存契约使用非空 VpnConnection 实例，验证认证方式数组、原始属性保护、方法返回数组、成功空结果、错误码拒绝以及意外全用户范围拒绝，没有创建真实 VPN。复验命令为 `pcscope-platform-native.exe vpn-contract-test` 与 `node scripts/test-native-registry.cjs --remote-access --wrapper`。

同一网络单元回归材料：`target/native-network-tests/1791979413296/summary.json`。12 条地址、40 条路由及保持在线的 IPv4/IPv6 TCP/UDP 样本身份与稳定字段一致。Rust 归属、来源库存和迁移清单验证通过。真实有配置/有连接 VPN、认证方式差异、增强权限及目标 Windows 版本矩阵、长期性能和最终部署继续开放。
