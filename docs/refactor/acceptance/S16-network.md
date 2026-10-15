# S16 验收记录：网络与远程访问

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | S16 · 待验收 |
| 实现范围 | 网络类 18 个观察来源的适配器，为本批类别中最多 |
| 代码定位 | 新增 `crates/contracts/src/adapters/network.rs`；修改 `crates/contracts/src/adapters/mod.rs`（注册 `network::SOURCES`） |
| 验证环境 | Windows 11；cargo / rustc 1.96.0；素材键名取自 `domains/network.ps1`（本类绝大部分）、`collect-system.ps1`、`domains/scoped.ps1` 的 emit 声明 |
| 验证过程 | `cargo test --workspace`；25 项新增测试；17 个 `scripts/check-*.cjs`；契约示例退出码与生成物逐字节一致；构建零警告 |
| 结果 | 248 项 Rust 测试通过（含并行进行的 S18、S19）；17/17 检查通过；`model/contract-observed.json` 与 `model/drill-output.json` 重新生成后逐字节未变 |
| 遗留问题 | 见第 3 节；其中第 1 项为结构性缺陷 |
| 下一步 | S17 安全与信任 |

## 1. 本类的判断要点

- **`network.proxy_configuration` 的键与字段互相拉扯。** 键是值级的 `[scope, path, value_name]`，字段却是三个具名设置（`enabled`/`server`/`bypass`）。处理方式是：一个键展开为该键下每个值一个候选，**每个候选只陈述它自己名字对应的那个设置**（`ProxyEnable`→`enabled`，`ProxyServer`→`server`，`ProxyOverride`→`bypass`），其余值仍是候选且三项皆为缺失——**不去借用邻近值的数据**。`ProxyServer` 被采集，尽管 emitter 的 `$allowed` 列表里没有它（该列表不是过滤器）。
- **`network.ipsec` 把策略存储折进作用域**，因为类型的作用域说明就是「策略存储作用域」而它的键里没有这一项；`network.firewall_rule` 没有这样做，因为 `policy_store` 是它声明的键之一。两者的存储都取自 emitter 自己的 `-PolicyStore ActiveStore`。
- **TCP 的 `protocol` 由适配器陈述，UDP 的不陈述。** 两个 emitter 都没有 `Protocol` 键。适配器对自己来源的知识（这是 `npm`、`root/subscription` 同一类做法）让它为 TCP 填 `"TCP"`；UDP 则记为缺失。**这个不对称是刻意的，但也应由目录而非适配器来定**，已列入 S21。
- **`service` 取自键路径的最后一段**（`Terminal Server`、`WSMAN`），与 `startup.rs` 读服务键的方式一致。

## 2. 本类未跳过任何素材流

18 个来源发出的每条流都归属该来源已声明的某个类型，因此没有需要 `policy.rs` 那种「跳过并说明原因」的处理。未映射的是**载荷键**（留在证据里，无对应声明字段）：网卡的 `description`、防火墙配置的 `Allow*Rules`/`Log*` 标志与日志大小、防火墙规则的七张过滤器关联表与状态列、IPsec 的 `InboundSecurity`/`OutboundSecurity`/`Profile`、VPN 的 `SplitTunneling`/`AuthenticationMethod`、共享的 `encrypt_data`、会话的 `SessionId`、DNS 缓存的 `section`、路由的 `Protocol`/`Publish`、地址的 `prefix_origin` 等。

`unreadable` 在本类只用于一处（注册表 item 的 `values` 存在但不是表）：逐 emitter 检查过，本类没有哪个采集端把**单个字段**标为读不到，因此没有为此发明状态。

## 3. 遗留问题

1. **【结构性】五个来源能陈述的比类型声明的键少**，已在勘察文档第 3 节第 2 项汇总：

   | 类型 | 缺的键 | 素材实际提供 | 后果 |
   | --- | --- | --- | --- |
   | `network.address_configuration`、`network.route`、`network.dns_configuration` | `interface_guid` | 只有接口索引与别名 | 三个类型无法按身份与网卡清单归一，一律暂定身份 |
   | `network.smb_session` | `server` | 客户端与用户名 | 同客户端同用户的多个会话共用一个键 |
   | `network.endpoint`（UDP 来源） | `protocol`、远端地址与端口 | 只有本地四项 | 端点身份不完整 |

   适配器把这些列为 `missing` 而不是拿别名或索引顶替——顶替会让两个从未被证明相同的对象静默合并。**需要在「采集端补发」与「改类型身份策略」之间逐项决定**，这不是适配器能解决的。

2. **`network.dns_configuration` 的注册表形态实际上只按作用域成键**：`Tcpip\Parameters` 是机器级键，既无接口也无地址族，两个键都缺失。
3. **非值级形态没有把注册表视图折进作用域**，因此 `winrm-configuration` 与 DNS 的 `Parameters` 键在两个视图下会到达同一个实例。代码中已注明，未擅自修复——修它等于在三处各加一条约定，应由 §3.5 收拢后再统一处理。
4. **本机未做真实采集实测**；键名来自 emit 声明的静态核对，真实输出核对并入 S21。
