# 全部采集定义与开关的审查

[总评与整改顺序](collection-lifecycle-audit.md) · [101 类对象](collection-lifecycle-types.md) · [133 个来源](collection-lifecycle-sources.md) · [225 个采集定义](collection-lifecycle-items.md) · [可核对数据](collection-lifecycle-audit.json)

本表逐项审查当前代码与来源，**覆盖不等于功能已完整或实机通过**。新增、离开清单、状态切换、操作开始/结果分别核对；明确缺口见总评。

225 个定义逐行列出；其中 222 个有单独开关。状态/其他事件分类也计入，避免只检查有明确事件编号的规则。事件标题列出编号与来源名称；动作在模型中注册仍不代表已完成跨记录关联。

| 采集定义 | 性质/开关 | 事件范围或对象 | 审查结果 |
|---|---|---|---|
| 本机软件安装登记<br>`pcscope.software` | 状态清单<br>有开关 | software.installation_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07, G09 |
| 操作系统版本与安装时间<br>`pcscope.system.version` | 状态清单<br>有开关 | policy.os_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06, G07, G12 |
| 最近启动时间与时区<br>`pcscope.system.boot-state` | 状态清单<br>有开关 | policy.os_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06, G07, G12 |
| 本机用户账户<br>`pcscope.identity.accounts` | 状态清单<br>有开关 | account.user | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| 本机用户组与成员<br>`pcscope.identity.groups` | 状态清单<br>有开关 | account.group<br>account.group_membership | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G01, G02, G07, G03 |
| 服务运行状态<br>`pcscope.startup.services` | 状态清单<br>有开关 | startup.service | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| 计划任务基础配置<br>`pcscope.startup.tasks` | 状态清单<br>有开关 | startup.scheduled_task | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| 卷与文件系统<br>`pcscope.storage.volumes` | 状态清单<br>有开关 | hardware.volume | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G10 |
| 网络适配器<br>`pcscope.network.adapters` | 状态清单<br>有开关 | network.adapter | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| IP 地址<br>`pcscope.network.addresses` | 状态清单<br>有开关 | network.address_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| 虚拟化能力与平台<br>`pcscope.virtualization.platform` | 状态清单<br>有开关 | virtualization.platform_capability | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06 |
| 即插即用设备<br>`pcscope.hardware.devices` | 状态清单<br>有开关 | hardware.device | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06, G07 |
| QFE 更新登记<br>`pcscope.system.hotfixes` | 状态清单<br>有开关 | software.system_update | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G07, G09 |
| 第三方预览处理程序<br>`pcscope.shell.registrations.preview` | 状态清单<br>有开关 | shell.preview_handler<br>shell.icon_overlay_handler<br>shell.shell_extension<br>shell.namespace_extension<br>shell.context_menu_registration<br>shell.file_association<br>shell.com_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G10 |
| 图标覆盖处理程序<br>`pcscope.shell.registrations.overlay` | 状态清单<br>有开关 | shell.preview_handler<br>shell.icon_overlay_handler<br>shell.shell_extension<br>shell.namespace_extension<br>shell.context_menu_registration<br>shell.file_association<br>shell.com_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G10 |
| Shell 扩展允许与阻止列表<br>`pcscope.shell.registrations.approval` | 状态清单<br>有开关 | shell.preview_handler<br>shell.icon_overlay_handler<br>shell.shell_extension<br>shell.namespace_extension<br>shell.context_menu_registration<br>shell.file_association<br>shell.com_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G10 |
| 桌面与此电脑命名空间<br>`pcscope.shell.registrations.namespace` | 状态清单<br>有开关 | shell.preview_handler<br>shell.icon_overlay_handler<br>shell.shell_extension<br>shell.namespace_extension<br>shell.context_menu_registration<br>shell.file_association<br>shell.com_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G10 |
| 右键菜单、文件关联与默认图标<br>`pcscope.shell.registrations.file-association` | 状态清单<br>有开关 | shell.preview_handler<br>shell.icon_overlay_handler<br>shell.shell_extension<br>shell.namespace_extension<br>shell.context_menu_registration<br>shell.file_association<br>shell.com_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G10 |
| COM 实现与扩展文件元数据<br>`pcscope.shell.registrations.com` | 状态清单<br>有开关 | shell.preview_handler<br>shell.icon_overlay_handler<br>shell.shell_extension<br>shell.namespace_extension<br>shell.context_menu_registration<br>shell.file_association<br>shell.com_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G10 |
| 登录启动登记<br>`pcscope.startup.logon.registry` | 状态清单<br>有开关 | startup.logon_startup_entry | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G01, G02, G03 |
| 用户与公共启动文件夹<br>`pcscope.startup.logon.folders` | 状态清单<br>有开关 | startup.logon_startup_entry | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G01, G02, G03 |
| 计算机身份与域信息<br>`pcscope.identity.machine` | 状态清单<br>有开关 | account.machine_identity | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06 |
| 时间同步配置与时区<br>`pcscope.system.clock` | 状态清单<br>有开关 | policy.time_sync_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06, G07 |
| Windows Update 历史<br>`pcscope.system.update-history` | 历史动作<br>有开关 | software.system_update | 原生 WUA 历史有安装/卸载 operation 与 result 结果；记录为独立操作，不把保留历史条目消失当作卸载。<br>G07, G09 |
| 待重启标志<br>`pcscope.system.reboot-pending` | 状态清单<br>有开关 | policy.os_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06, G07, G12 |
| 系统还原点<br>`pcscope.system.restore-points` | 状态清单<br>有开关 | policy.system_restore_point | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02 |
| 当前用户 AppX 包<br>`pcscope.software.appx-user` | 状态清单<br>有开关 | software.appx_package | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07, G09 |
| 预配 AppX 包<br>`pcscope.software.provisioned` | 状态清单<br>有开关 | software.provisioned_package | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G09 |
| Windows 可选组件<br>`pcscope.software.optional-features` | 状态清单<br>有开关 | software.optional_feature | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06, G09 |
| 当前用户软件登记<br>`pcscope.software.user-inventory` | 状态清单<br>有开关 | software.installation_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07, G09 |
| 用户配置文件<br>`pcscope.identity.profiles` | 状态清单<br>有开关 | account.user_profile | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02 |
| 登录会话<br>`pcscope.identity.logon-sessions` | 状态清单<br>有开关 | runtime.logon_session | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G04, G07 |
| 交互式终端会话<br>`pcscope.identity.interactive-sessions` | 状态清单<br>有开关 | runtime.terminal_session | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| 账户权限分配<br>`pcscope.identity.assigned-rights` | 状态清单<br>有开关 | account.right_assignment | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G03, G07 |
| 进程清单<br>`pcscope.execution.processes` | 状态清单<br>有开关 | runtime.process | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G04, G07, G12 |
| 内存与 CPU 采样<br>`pcscope.execution.resources` | 资源采样<br>有开关 | — | CPU/内存采样是数值序列，无新增/删除成对要求。<br> |
| Defender 防护状态<br>`pcscope.security.protection` | 状态清单<br>有开关 | security.protection | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G03, G06, G07 |
| Defender 策略与排除项<br>`pcscope.security.protection-policy` | 状态清单<br>有开关 | security.protection<br>security.exclusion | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G03, G06, G07 |
| 安全产品登记<br>`pcscope.security.products` | 状态清单<br>有开关 | security.security_product | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| Defender 检测记录<br>`pcscope.security.detections` | 历史动作<br>有开关 | security.protection | 原生读取 Defender 已保留的检测记录，适配为检测证据；不是当前感染状态的清单，不生成历史记录的新增/消失来推断感染/治愈。<br>G03, G06, G07 |
| Device Guard / VBS<br>`pcscope.security.deviceguard` | 状态清单<br>有开关 | security.device_guard_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06 |
| TPM 状态<br>`pcscope.security.tpm` | 状态清单<br>有开关 | hardware.tpm | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06 |
| 安全启动状态<br>`pcscope.security.secureboot` | 状态清单<br>有开关 | security.secure_boot_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06 |
| CNG 密钥元数据<br>`pcscope.security.key-metadata` | 状态清单<br>有开关 | security.key_metadata | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| 计算机证书<br>`pcscope.security.certificates.localmachine` | 状态清单<br>有开关 | security.certificate_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| 当前用户证书<br>`pcscope.security.certificates.currentuser` | 状态清单<br>有开关 | security.certificate_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| BitLocker 卷状态<br>`pcscope.security.encryption` | 状态清单<br>有开关 | security.bitlocker_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06, G07 |
| UAC、资源管理器与软件限制策略<br>`pcscope.policy.configuration` | 状态清单<br>有开关 | policy.uac_policy<br>policy.software_restriction_policy | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G03, G06, G01, G02 |
| SSH 公钥指纹<br>`pcscope.security.ssh-public-keys` | 状态清单<br>有开关 | security.ssh_public_key | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03 |
| 事件通道可读性<br>`pcscope.collector.event-channel-coverage` | 采集健康<br>有开关 | — | 逐通道权限/可用性与保留信息；无安装/删除过程。<br> |
| 路由表<br>`pcscope.network.routes` | 状态清单<br>有开关 | network.route | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02 |
| DNS 配置<br>`pcscope.network.dns-configuration` | 状态清单<br>有开关 | network.dns_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G03, G06 |
| 网络连接配置文件<br>`pcscope.network.profiles` | 状态清单<br>有开关 | network.connection_profile | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06, G07 |
| TCP 连接与监听<br>`pcscope.network.tcp` | 状态清单<br>有开关 | network.endpoint | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G04, G07, G12 |
| UDP 端点<br>`pcscope.network.udp` | 状态清单<br>有开关 | network.endpoint | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G04, G07, G12 |
| 防火墙配置文件<br>`pcscope.network.firewall-profiles` | 状态清单<br>有开关 | network.firewall_profile | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06 |
| 防火墙规则及过滤条件<br>`pcscope.network.firewall-rules` | 状态清单<br>有开关 | network.firewall_rule | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| IPsec 规则<br>`pcscope.network.ipsec` | 状态清单<br>有开关 | network.ipsec_rule | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| 代理、远程桌面与 VPN<br>`pcscope.network.access` | 状态清单<br>有开关 | network.proxy_configuration<br>network.vpn_configuration<br>network.remote_access_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G03, G06, G01, G02, G07 |
| SMB 共享与访问授权<br>`pcscope.network.shares` | 状态清单<br>有开关 | network.smb_share | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G07 |
| SMB 驱动器映射<br>`pcscope.network.mapped-drives` | 状态清单<br>有开关 | network.smb_mapping | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02 |
| DNS 缓存<br>`pcscope.network.dns-cache` | 状态清单<br>有开关 | network.dns_cache_entry | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G04, G07 |
| hosts 静态映射<br>`pcscope.network.hosts` | 状态清单<br>有开关 | network.hosts_entry | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02 |
| SMB 客户端会话<br>`pcscope.network.share-sessions` | 状态清单<br>有开关 | network.smb_session | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G04 |
| WinRM 配置<br>`pcscope.network.winrm-configuration` | 状态清单<br>有开关 | network.remote_access_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G03, G06 |
| 磁盘清单<br>`pcscope.storage.disks` | 状态清单<br>有开关 | hardware.disk | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06, G10, G12 |
| 分区清单<br>`pcscope.storage.partitions` | 状态清单<br>有开关 | hardware.partition | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02 |
| 驱动仓库包<br>`pcscope.hardware.driver-packages` | 状态清单<br>有开关 | hardware.driver_package | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07, G09 |
| 设备驱动绑定<br>`pcscope.hardware.driver-bindings` | 状态清单<br>有开关 | hardware.driver_binding | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G08, G09 |
| 驱动服务<br>`pcscope.hardware.driver-services` | 状态清单<br>有开关 | hardware.driver_service | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06, G07, G12 |
| BIOS 固件<br>`pcscope.hardware.firmware` | 状态清单<br>有开关 | hardware.firmware | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G06, G09 |
| 物理磁盘健康<br>`pcscope.hardware.disk-health` | 状态清单<br>有开关 | hardware.disk | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06, G10, G12 |
| 电池状态<br>`pcscope.hardware.battery` | 状态清单<br>有开关 | hardware.battery | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| 电源计划与休眠配置<br>`pcscope.power.configuration` | 状态清单<br>有开关 | policy.power_plan | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| 服务详细配置<br>`pcscope.startup.service-configuration` | 状态清单<br>有开关 | startup.service | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| 会话与登录初始化入口<br>`pcscope.startup.session-hooks` | 状态清单<br>有开关 | startup.logon_initialization | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03 |
| IFEO、AppInit 与 AppCert 入口<br>`pcscope.startup.advanced-extensions` | 状态清单<br>有开关 | startup.ifeo_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03 |
| WMI 永久订阅过滤器<br>`pcscope.startup.wmi-filters` | 状态清单<br>有开关 | startup.wmi_filter | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07, G12 |
| WMI 永久订阅消费者<br>`pcscope.startup.wmi-consumers` | 状态清单<br>有开关 | startup.wmi_consumer | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07, G12 |
| WMI 过滤器与消费者绑定<br>`pcscope.startup.wmi-bindings` | 状态清单<br>有开关 | startup.wmi_binding | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07, G12 |
| 计划任务执行记录与条件<br>`pcscope.startup.task-details` | 状态清单<br>有开关 | startup.scheduled_task | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| 环境变量与默认文件关联<br>`pcscope.environment.configuration` | 状态清单<br>有开关 | policy.environment_variable<br>shell.file_association | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G03, G06, G01, G02 |
| 打印机清单<br>`pcscope.desktop.printing` | 状态清单<br>有开关 | desktop.printer | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06 |
| 打印驱动<br>`pcscope.desktop.printer-drivers` | 状态清单<br>有开关 | desktop.printer_driver | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G09 |
| 打印端口<br>`pcscope.desktop.printer-ports` | 状态清单<br>有开关 | desktop.printer_port | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02 |
| 打印作业队列<br>`pcscope.desktop.print-jobs` | 状态清单<br>有开关 | desktop.print_job | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G04, G07 |
| 键盘、输入法与过滤驱动配置<br>`pcscope.desktop.input` | 状态清单<br>有开关 | desktop.input_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G06 |
| 剪贴板设置<br>`pcscope.desktop.clipboard` | 状态清单<br>有开关 | desktop.shell_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G03, G06, G07, G12 |
| 桌面与资源管理器设置<br>`pcscope.desktop.shell-settings` | 状态清单<br>有开关 | desktop.shell_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G03, G06, G07, G12 |
| 桌面与开始菜单快捷方式<br>`pcscope.desktop.shortcuts` | 状态清单<br>有开关 | desktop.shortcut | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G05 |
| Hyper-V 虚拟机<br>`pcscope.virtualization.hyperv-vms` | 状态清单<br>有开关 | virtualization.virtual_machine | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G06, G07, G11 |
| Hyper-V 虚拟交换机<br>`pcscope.virtualization.hyperv-switches` | 状态清单<br>有开关 | virtualization.virtual_switch | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G11 |
| 虚拟机磁盘连接<br>`pcscope.virtualization.hyperv-disks` | 状态清单<br>有开关 | virtualization.disk_attachment | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G11 |
| 虚拟机检查点<br>`pcscope.virtualization.hyperv-checkpoints` | 状态清单<br>有开关 | virtualization.checkpoint | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G11 |
| WSL 发行版登记<br>`pcscope.virtualization.wsl-registration` | 状态清单<br>有开关 | virtualization.wsl_distribution | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| Chrome、Edge 与 Firefox 策略<br>`pcscope.apps.browser-policies` | 状态清单<br>有开关 | policy.browser_policy | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G03, G06 |
| VS Code 扩展清单<br>`pcscope.apps.ide-extensions` | 状态清单<br>有开关 | desktop.editor_extension | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G09 |
| .NET 运行时登记<br>`pcscope.apps.dotnet-runtimes` | 状态清单<br>有开关 | software.runtime | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G03, G09 |
| 全局 npm 包清单<br>`pcscope.apps.npm-global-packages` | 状态清单<br>有开关 | software.language_package | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G09 |
| OneDrive 账户与同步登记<br>`pcscope.apps.onedrive-registration` | 状态清单<br>有开关 | desktop.sync_account | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G07 |
| Chrome / Edge 扩展<br>`pcscope.apps.chromium-extensions` | 状态清单<br>有开关 | shell.browser_extension | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G09 |
| Firefox 扩展<br>`pcscope.apps.firefox-extensions` | 状态清单<br>有开关 | shell.browser_extension | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G09 |
| 指定范围配置<br>`pcscope.collector.scoped-configuration` | 采集健康<br>有开关 | — | 监控范围配置；名单关闭/清空不产生被监控对象删除。<br> |
| 指定目录与直接子项<br>`pcscope.files.directory-security` | 状态清单<br>有开关 | file.watched_directory | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G05 |
| 指定文件、ACL 与 ADS<br>`pcscope.files.metadata-security` | 状态清单<br>有开关 | file.watched_file<br>file.acl_entry<br>file.alternate_data_stream | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G01, G02, G03, G05, G07, G12 |
| 便携软件文件登记<br>`pcscope.software.portable` | 状态清单<br>有开关 | software.portable_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G05 |
| 指定进程加载模块<br>`pcscope.execution.modules` | 状态清单<br>有开关 | runtime.module_instance | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G04, G07, G12 |
| 模块发现与读取范围<br>`pcscope.execution.module-scan-status` | 采集健康<br>有开关 | — | 模块发现/读取结果；读取失败与进程未发现分开，无模块卸载推断。<br> |
| 已观察模块的磁盘文件变化<br>`pcscope.execution.module-file-state` | 状态清单<br>有开关 | file.watched_file | 从成功模块观察建立文件名单，跟踪磁盘稳定复核；变化/恢复留在 source_readings，稳定存在时也可形成文件对象；通用新增/缺席证据主动排除此来源。<br>G01, G02, G03, G05, G07, G12 |
| Mihomo 本地运行配置<br>`pcscope.apps.mihomo` | 状态清单<br>有开关 | network.proxy_configuration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G03, G06 |
| Docker 容器、镜像、卷与网络<br>`pcscope.virtualization.docker` | 状态清单<br>有开关 | virtualization.container<br>virtualization.image<br>virtualization.container_volume<br>virtualization.container_network | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G04, G06, G09, G03 |
| 固定 NTFS 卷发现<br>`pcscope.usn.volumes` | 状态清单<br>有开关 | hardware.volume | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G10 |
| 系统与服务事件 / 服务运行事件<br>`pcscope.eventlog.system.services` | 事件<br>有开关 | 提供程序：Service Control Manager；编号：按分类/兜底读取 | 登记加入/移除、运行/停止、启动失败分别核对；7036 状态有专门适配，安装/删除日志过程尚未统一。<br>G01, G02, G07, G06, G12 |
| 系统与服务事件 / 系统启动与关闭<br>`pcscope.eventlog.system.lifecycle` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Kernel-General；编号：12, 13 | 12/13 说明系统启动/关闭；非正常重启可无正常关闭端，按本次启动周期记录，不伪补结束。<br>G01, G02, G07, G06, G12 |
| 系统与服务事件 / 电源与异常重启<br>`pcscope.eventlog.system.power` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Kernel-Power；编号：按分类/兜底读取 | 电源/异常重启不是每次均有开始结束；保留来源类型和结果，需结合启动周期，不当作服务启停。<br>G01, G02, G07, G06, G12 |
| 系统与服务事件 / 系统注册表维护<br>`pcscope.eventlog.system.registry-maintenance` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Kernel-General；编号：11, 15<br>11 注册表事务初始化〔事件解释〕<br>15 注册表配置单元整理〔事件解释〕 | 11/15 是注册表维护诊断，不是同一对象创建/删除一对。<br>G01, G02, G07, G06, G12 |
| 系统与服务事件 / 系统时间同步<br>`pcscope.eventlog.system.time-source` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Time-Service；编号：37<br>37 时间源数据有效〔事件解释〕 | 37 是收到有效时间源，只有状态证据；未实现同步失败→恢复的完整过程。<br>G01, G02, G07, G06, G12 |
| 系统与服务事件 / 来源采集状态<br>`pcscope.eventlog.system.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 系统与服务事件 / 其他事件<br>`pcscope.eventlog.system.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | SCM 7036 明确运行状态可唯一显示名匹配服务；其余服务/系统事件可读原始记录。缺服务卸载专门动作和统一启动失败关联。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G07, G06, G12 |
| 应用程序事件 / 软件安装事件<br>`pcscope.eventlog.application.installer` | 事件<br>有开关 | 提供程序：MsiInstaller；编号：按分类/兜底读取 | 需要区分安装/修复/卸载、尝试/成功/失败；当前通用 MSI kind 未接统一对象行动或一次操作关联。<br>G07 |
| 应用程序事件 / 应用程序错误<br>`pcscope.eventlog.application.errors` | 事件<br>有开关 | 提供程序：Application Error；编号：按分类/兜底读取 | 应用崩溃是单次故障；没有自然反向事件。后续启动/恢复另作运行证据，不凭缺少新错误宣布恢复。<br>G07 |
| 应用程序事件 / 应用程序无响应<br>`pcscope.eventlog.application.hangs` | 事件<br>有开关 | 提供程序：Application Hang；编号：按分类/兜底读取 | 无响应是故障证据；进程退出、恢复响应、被终止是不同结果，当前未组织过程。<br>G07 |
| 应用程序事件 / NVIDIA WMI 服务状态<br>`pcscope.eventlog.application.nvwmi` | 事件<br>有开关 | 提供程序：NVWMI；编号：3<br>3 NVIDIA WMI 服务进度〔事件解释〕 | 3 是服务进度/检查点，需解码状态字段；不能当作固定启动或停止端。<br>G07 |
| 应用程序事件 / 卷影复制服务状态<br>`pcscope.eventlog.application.vss` | 事件<br>有开关 | 提供程序：VSS；编号：8224<br>8224 卷影复制服务空闲停止〔事件解释〕 | 8224 是空闲停止；只有停止端，配对需同服务运行状态/日志，不能作为备份失败。<br>G07 |
| 应用程序事件 / Windows 错误报告<br>`pcscope.eventlog.application.wer` | 事件<br>有开关 | 提供程序：Windows Error Reporting；编号：1001<br>1001 错误报告登记〔事件解释〕 | 1001 是错误报告登记，可能重复引用同一故障；按报告/进程身份关联，不能作为另一场独立崩溃。<br>G07 |
| 应用程序事件 / 软件保护与授权维护<br>`pcscope.eventlog.application.licensing` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-SPP；编号：16384, 16394<br>16384 软件保护服务重启已安排〔事件解释〕<br>16394 软件保护离线迁移完成〔事件解释〕 | 安排重启与离线迁移完成是不同操作，不能按这两个事件组成开始/结束。<br>G07 |
| 应用程序事件 / 存储优化作业<br>`pcscope.eventlog.application.optimization` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Defrag；编号：258, 264<br>258 存储优化完成〔事件解释〕<br>264 存储优化未完成〔事件解释〕 | 258/264 是完成/未完成两个结果分支，不能把它们作为开始/结束一对。<br>G07 |
| 应用程序事件 / 来源采集状态<br>`pcscope.eventlog.application.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 应用程序事件 / 其他事件<br>`pcscope.eventlog.application.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | MSI、错误、无响应、WER 等原始日志与可读解释；未实现通用安装/卸载开始与结果的过程关联。 兜底原始事件，不计为已实现的成对过程。<br>G07 |
| Windows 安全审计 / 登录、登出与会话<br>`pcscope.eventlog.security.sessions` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4624, 4625, 4634, 4647, 4800, 4801, 4778, 4779<br>4624 登录〔模型动作〕<br>4625 登录失败〔模型动作〕<br>4634 注销〔模型动作〕<br>4647 请求注销〔模型动作〕<br>4778 会话重连〔模型动作〕<br>4779 会话断开〔模型动作〕<br>4800 会话锁定〔模型动作〕<br>4801 会话解锁〔模型动作〕 | 4624/4634 登录注销、4800/4801 锁定解锁、4778/4779 重连断开分三对；4625 是失败，4647 是注销请求。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 账户管理<br>`pcscope.eventlog.security.accounts` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4720, 4722, 4725, 4726, 4738, 4740, 4767, 4781<br>4720 账户建立〔模型动作〕<br>4722 账户启用〔模型动作〕<br>4725 账户禁用〔模型动作〕<br>4726 账户删除〔模型动作〕<br>4738 账户变更〔模型动作〕<br>4740 账户锁定〔模型动作〕<br>4767 账户解锁〔模型动作〕<br>4781 账户重命名〔模型动作〕 | 4720/4726 创建删除，4722/4725 启用停用，4740/4767 锁定解锁；4738/4781 更新/改名不是创建删除。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 用户组成员变更<br>`pcscope.eventlog.security.groups` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4732, 4733<br>4732 组成员加入〔模型动作〕<br>4733 组成员移除〔模型动作〕 | 4732/4733 成员加入移除；须完整组 SID＋成员 SID；不是用户组本身创建删除。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 特殊权限分配<br>`pcscope.eventlog.security.privileges` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4672<br>4672 特殊权限分配〔模型动作〕 | 4672 是登录令牌获得特殊权限，无长期权限撤销另一端；不得充当 LSA 账户权限分配。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 进程生命周期<br>`pcscope.eventlog.security.processes` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4688, 4689<br>4688 进程启动〔模型动作〕<br>4689 进程退出〔模型动作〕 | 4688/4689 是启动退出两端，PID 复用须分代；日志身份不全不能硬拼到当前进程。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 文件访问与权限<br>`pcscope.eventlog.security.files` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4663, 4670<br>4663 对象访问审计〔模型动作〕<br>4670 对象权限变更审计〔模型动作〕 | 4663 访问、4670 权限审计是不同性质；不是文件创建删除一对，仍受配置文件名单范围限制。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 注册表操作<br>`pcscope.eventlog.security.registry` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4657<br>4657 注册表值操作〔模型动作〕 | 4657 须解码值操作/前后内容；单一事件编号并不固定表示创建或删除。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 服务安装<br>`pcscope.eventlog.security.services` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4697<br>4697 服务安装〔模型动作〕 | 4697 只有服务安装；没有对应删除事件规则，移除由完整服务清单确认。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 计划任务变更<br>`pcscope.eventlog.security.tasks` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4698, 4699, 4702<br>4698 任务创建〔模型动作〕<br>4699 任务删除〔模型动作〕<br>4702 任务更新〔模型动作〕 | 4698/4699 创建删除，4702 更新；任务配置过程与每次运行开始结束分开。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 审计策略变更<br>`pcscope.eventlog.security.audit-policy` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4719<br>4719 审计策略变更〔模型动作〕 | 4719 是审计设置变化，以具体类别和前后值解释，无开始结束要求。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 网络连接与共享访问<br>`pcscope.eventlog.security.network` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：5140, 5156, 5157<br>5140 共享访问〔模型动作〕<br>5156 连接允许〔模型动作〕<br>5157 连接阻止〔模型动作〕 | 5156/5157 是允许/阻止的判定结果，5140 是共享访问；三者均不是连接建立关闭一对。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 审计日志清除<br>`pcscope.eventlog.security.log-clear` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Eventlog；编号：1102<br>1102 安全审计日志清除〔模型动作〕 | 1102 是审计日志清除记录；无自然反向端，不在用户界面推测不可修复的历史完整性。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 加密密钥操作<br>`pcscope.eventlog.security.crypto-keys` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：5058, 5059, 5061<br>5058 密钥文件操作〔事件解释〕<br>5059 密钥导入与导出〔事件解释〕<br>5061 加密与密钥使用〔事件解释〕 | 5058/5059/5061 可含多种密钥操作，必须读取操作/结果字段后才称创建删除/导入导出；当前仅有事件解释。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 密码组件自检与验证<br>`pcscope.eventlog.security.crypto-integrity` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：5056, 5057, 5060, 5062, 6417<br>5056 密码组件自检〔事件解释〕<br>5057 密码基础操作失败〔事件解释〕<br>5060 密码验证失败〔事件解释〕<br>5062 内核密码组件自检〔事件解释〕<br>6417 FIPS 密码自检成功〔事件解释〕 | 组件自检、验证、加载等诊断；按返回码说明结果，无通用启停/增删配对。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 密码提供程序与上下文配置<br>`pcscope.eventlog.security.crypto-configuration` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：5063, 5064, 5065, 5066, 5067, 5068, 5069, 5070<br>5063 密码提供程序操作〔事件解释〕<br>5064 密码上下文操作〔事件解释〕<br>5065 密码上下文修改〔事件解释〕<br>5066 密码函数操作〔事件解释〕<br>5067 密码函数修改〔事件解释〕<br>5068 密码函数提供程序操作〔事件解释〕<br>5069 密码函数属性操作〔事件解释〕<br>5070 密码函数属性修改〔事件解释〕 | 5063–5070 涉及提供程序/上下文/函数配置，应解码对象和操作，尚未形成对应配置子项过程。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / DPAPI 数据保护与主密钥<br>`pcscope.eventlog.security.data-protection` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4692, 4693, 4694, 4695<br>4692 数据保护主密钥备份〔事件解释〕<br>4693 数据保护主密钥恢复〔事件解释〕<br>4694 数据保护操作〔事件解释〕<br>4695 解除数据保护操作〔事件解释〕 | DPAPI 备份/恢复与数据保护/解保护是不同操作，不能仅按相邻编号配对；不采集密钥材料。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 凭据读取与账户检查<br>`pcscope.eventlog.security.credential-access` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4648, 4797, 5379, 5382<br>4648 显式凭据登录尝试〔事件解释〕<br>4797 账户空密码检查〔事件解释〕<br>5379 凭据管理器读取〔事件解释〕<br>5382 凭据保管库读取〔事件解释〕 | 凭据读取、显式身份使用、账户检查为访问行为，不存在自然创建删除对；敏感内容保持保护。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 用户组成员查询<br>`pcscope.eventlog.security.membership-queries` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4798, 4799<br>4798 查询用户所属本地组〔事件解释〕<br>4799 枚举本地用户组成员〔事件解释〕 | 4798/4799 是成员查询，不能充当成员加入/移除。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / WMI 与目录对象访问审计<br>`pcscope.eventlog.security.object-access` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4662<br>4662 受审计对象操作〔事件解释〕 | 4662 是对象访问审计；按对象/权限字段解释，不自动写为 WMI 订阅增删。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 对象访问记录规则<br>`pcscope.eventlog.security.files-audit-settings` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4907<br>4907 对象访问审计规则已修改〔事件解释〕 | 4907 是对象审计规则变化；SACL 与 DACL 权限不同，不能当作允许/拒绝访问结果。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 进程访问令牌分配<br>`pcscope.eventlog.security.process-tokens` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4696<br>4696 进程主令牌分配〔事件解释〕 | 4696 是令牌分配记录；不当作进程启动、永久权限授予或其反向端。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 系统启动、时间与审计配置<br>`pcscope.eventlog.security.system-audit` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：4608, 4616, 4826, 4902<br>4608 安全审计子系统启动〔事件解释〕<br>4616 系统时间变更〔事件解释〕<br>4826 启动配置加载〔事件解释〕<br>4902 用户审计策略表建立〔事件解释〕 | 4608 启动、4616 时间更改、4826 启动配置、4902 审计表初始化分别保留；不是同一个生命周期的四阶段。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 防火墙组件生命周期<br>`pcscope.eventlog.security.firewall-lifecycle` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Security-Auditing；编号：5024, 5033<br>5024 防火墙服务启动成功〔事件解释〕<br>5033 防火墙驱动启动成功〔事件解释〕 | 5024 服务启动成功、5033 驱动启动成功属于两个组件的同一侧；目前规则缺停止另一端。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 审计日志服务<br>`pcscope.eventlog.security.audit-service` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Eventlog；编号：1100<br>1100 事件日志服务关闭〔事件解释〕 | 1100 是事件日志服务停止；缺该分类中的启动另一端。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| Windows 安全审计 / 来源采集状态<br>`pcscope.eventlog.security.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| Windows 安全审计 / 其他事件<br>`pcscope.eventlog.security.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 账户/成员/任务创建删除、进程启退、会话登录注销等有显式动作；4697 仅服务安装，5156/5157 允许/阻止不组成连接建立/关闭；密码操作须解码操作字段。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G07, G03, G04, G12, G05, G06 |
| 计划任务执行事件 / 任务触发与执行过程<br>`pcscope.eventlog.microsoft-windows-taskscheduler/operational.execution` | 事件<br>有开关 | 提供程序：Microsoft-Windows-TaskScheduler；编号：100, 102, 107, 110, 114, 129, 200, 201, 322, 325, 328<br>100 任务实例开始〔事件解释〕<br>102 任务实例完成〔事件解释〕<br>107 计划触发任务〔事件解释〕<br>110 用户触发任务〔事件解释〕<br>114 错过计划后补启动〔事件解释〕<br>129 任务执行进程启动〔事件解释〕<br>200 任务动作开始〔事件解释〕<br>201 任务动作结束〔事件解释〕<br>322 已有实例导致未启动〔事件解释〕<br>325 任务实例排队〔事件解释〕<br>328 任务实例停止〔事件解释〕 | 100/102 任务运行开始结束，200/201 动作开始/返回；触发、进程创建、忽略/排队/中止分别保留，同运行实例关联尚缺。<br>G01, G02, G07 |
| 计划任务执行事件 / 任务登记与配置变更<br>`pcscope.eventlog.microsoft-windows-taskscheduler/operational.configuration` | 事件<br>有开关 | 提供程序：Microsoft-Windows-TaskScheduler；编号：140<br>140 任务配置更新〔事件解释〕 | 140 是更新任务登记；创建删除由 Security/任务快照提供，需保持同任务路径。<br>G01, G02, G07 |
| 计划任务执行事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-taskscheduler/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 计划任务执行事件 / 其他事件<br>`pcscope.eventlog.microsoft-windows-taskscheduler/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 100/102 开始/结束可分类，100 携带实例；未按同一次运行组织开始、动作返回、结束及失败。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G07 |
| Windows 更新事件 / 更新检查<br>`pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.discovery` | 事件<br>有开关 | 提供程序：Microsoft-Windows-WindowsUpdateClient；编号：26<br>26 更新检查完成〔事件解释〕 | 26 是检查结果，只有检查阶段；更新安装/卸载及结果从 WUA 历史补，尚未跨来源配对。<br>G07, G09 |
| Windows 更新事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| Windows 更新事件 / 其他事件<br>`pcscope.eventlog.microsoft-windows-windowsupdateclient/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 日志可读安装/检查阶段；原生 WUA 历史另有安装/卸载和结果，尚未跨来源关联一次更新操作。 兜底原始事件，不计为已实现的成对过程。<br>G07, G09 |
| Defender 防护事件 / Defender 健康与情报<br>`pcscope.eventlog.microsoft-windows-windows defender/operational.health` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Windows Defender；编号：1150, 1151, 2010<br>1150 Defender 客户端健康〔事件解释〕<br>1151 Defender 健康报告〔事件解释〕<br>2010 Defender 云情报获取〔事件解释〕 | 1150/1151/2010 健康/情报不是检测处置的一对；防护状态字段另核对开关与恢复。<br>G03, G06, G07, G01, G02, G05, G12 |
| Defender 防护事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-windows defender/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| Defender 防护事件 / 其他事件<br>`pcscope.eventlog.microsoft-windows-windows defender/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 检测、处置、健康事件保留；与防护开关及当前威胁状态分开，缺通用检测→处置/恢复过程。 兜底原始事件，不计为已实现的成对过程。<br>G03, G06, G07, G01, G02, G05, G12 |
| PowerShell 运行事件 / PowerShell 命令与脚本<br>`pcscope.eventlog.microsoft-windows-powershell/operational.commands` | 事件<br>有开关 | 提供程序：Microsoft-Windows-PowerShell；编号：4100, 4103, 4104<br>4100 PowerShell 执行错误〔事件解释〕<br>4103 模块命令调用记录〔事件解释〕<br>4104 脚本块内容记录〔事件解释〕 | 4100 错误、4103 命令调用、4104 脚本块元数据不是三阶段同一次执行；不读取受保护正文补过程。<br>G07 |
| PowerShell 运行事件 / PowerShell 宿主生命周期<br>`pcscope.eventlog.microsoft-windows-powershell/operational.host` | 事件<br>有开关 | 提供程序：Microsoft-Windows-PowerShell；编号：40961, 40962, 53504<br>40961 PowerShell 控制台启动〔事件解释〕<br>40962 PowerShell 控制台就绪〔事件解释〕<br>53504 PowerShell IPC 监听启动〔事件解释〕 | 40961 正在启动、40962 就绪、53504 IPC 监听启动均不是退出；目前没有完整宿主启退对。<br>G07 |
| PowerShell 运行事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-powershell/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| PowerShell 运行事件 / 其他事件<br>`pcscope.eventlog.microsoft-windows-powershell/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 宿主开始/就绪、命令/脚本元数据；不执行脚本或读取正文来补过程，不是完整开始/退出配对。 兜底原始事件，不计为已实现的成对过程。<br>G07 |
| WMI 活动事件 / WMI 提供程序与调用<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational.operations` | 事件<br>有开关 | 提供程序：Microsoft-Windows-WMI-Activity；编号：5857, 5858<br>5857 WMI 提供程序启动〔事件解释〕<br>5858 WMI 客户端调用失败〔事件解释〕 | 5857 提供程序启动、5858 客户端失败属于不同活动；不能配成开始/失败或永久订阅新增删除。<br>G01, G02, G07, G12 |
| WMI 活动事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| WMI 活动事件 / 其他事件<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 提供程序启动/调用失败等原始记录；不是永久订阅新增/删除的一对。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G07, G12 |
| 组策略处理事件 / 组策略服务生命周期<br>`pcscope.eventlog.microsoft-windows-grouppolicy/operational.service` | 事件<br>有开关 | 提供程序：Microsoft-Windows-GroupPolicy；编号：4115, 4116, 5115, 5116<br>4115 组策略服务启动〔事件解释〕<br>4116 组策略服务初始化开始〔事件解释〕<br>5115 组策略服务停止〔事件解释〕<br>5116 组策略服务初始化完成〔事件解释〕 | 4115/5115 服务启停，4116/5116 初始化开始完成是两对；目前保留解释，缺一次过程关联。<br>G06, G07, G12 |
| 组策略处理事件 / 策略处理开始与完成<br>`pcscope.eventlog.microsoft-windows-grouppolicy/operational.processing` | 事件<br>有开关 | 提供程序：Microsoft-Windows-GroupPolicy；编号：4000, 4001, 4004, 4005, 8000, 8001, 8004, 8005<br>4000 开机策略处理开始〔事件解释〕<br>4001 登录策略处理开始〔事件解释〕<br>4004 手动计算机策略处理开始〔事件解释〕<br>4005 手动用户策略处理开始〔事件解释〕<br>8000 开机策略处理完成〔事件解释〕<br>8001 登录策略处理完成〔事件解释〕<br>8004 手动计算机策略处理完成〔事件解释〕<br>8005 手动用户策略处理完成〔事件解释〕 | 4000/8000、4001/8001、4004/8004、4005/8005 各是对应处理开始完成；须同 ActivityId/对象范围关联。<br>G06, G07, G12 |
| 组策略处理事件 / 策略会话与登录通知<br>`pcscope.eventlog.microsoft-windows-grouppolicy/operational.sessions` | 事件<br>有开关 | 提供程序：Microsoft-Windows-GroupPolicy；编号：4117, 5117, 5324, 5325, 5351, 6339<br>4117 策略会话开始〔事件解释〕<br>5117 策略会话完成〔事件解释〕<br>5324 收到登录会话通知〔事件解释〕<br>5325 收到关闭前通知〔事件解释〕<br>5351 返回登录组件〔事件解释〕<br>6339 登录桌面处理完成〔事件解释〕 | 4117/5117 是策略会话开始完成，其余是通知/返回/桌面处理；不能只按会话词语归成一对。<br>G06, G07, G12 |
| 组策略处理事件 / 策略获取、适用与排除<br>`pcscope.eventlog.microsoft-windows-grouppolicy/operational.applicability` | 事件<br>有开关 | 提供程序：Microsoft-Windows-GroupPolicy；编号：4126, 5126, 5257, 5312, 5313<br>4126 策略获取开始〔事件解释〕<br>5126 策略获取完成〔事件解释〕<br>5257 策略下载完成〔事件解释〕<br>5312 适用策略列表〔事件解释〕<br>5313 被排除的策略列表〔事件解释〕 | 4126/5126 获取开始完成；5257 下载完成，5312/5313 适用/排除列表不同于启用/禁用结果。<br>G06, G07, G12 |
| 组策略处理事件 / 处理模式与内部诊断<br>`pcscope.eventlog.microsoft-windows-grouppolicy/operational.diagnostics` | 事件<br>有开关 | 提供程序：Microsoft-Windows-GroupPolicy；编号：5311, 5320, 5321, 5340<br>5311 环回处理模式〔事件解释〕<br>5320 组策略内部处理状态〔事件解释〕<br>5321 组策略实例诊断〔事件解释〕<br>5340 策略处理模式〔事件解释〕 | 环回模式、内部状态、实例诊断是诊断字段，无通用开始结束要求。<br>G06, G07, G12 |
| 组策略处理事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-grouppolicy/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 组策略处理事件 / 其他事件<br>`pcscope.eventlog.microsoft-windows-grouppolicy/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 已有服务启停、400x/800x 处理开始/完成等事件说明；未按处理活动关联到一次过程。 兜底原始事件，不计为已实现的成对过程。<br>G06, G07, G12 |
| AppX 部署事件 / 应用包部署<br>`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.deployment` | 事件<br>有开关 | 提供程序：Microsoft-Windows-AppXDeployment-Server；编号：404, 603, 613<br>404 应用包部署失败〔事件解释〕<br>603 应用包部署开始〔事件解释〕<br>613 应用包部署性能摘要〔事件解释〕 | 603 开始、404 失败，613 是性能摘要不能直接标成功；缺包级安装/卸载完整结果关联。<br>G01, G02, G07, G09 |
| AppX 部署事件 / 应用包部署服务状态<br>`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.service` | 事件<br>有开关 | 提供程序：Microsoft-Windows-AppXDeployment-Server；编号：939, 940, 941, 942<br>939 部署服务正在启动〔事件解释〕<br>940 部署服务正在运行〔事件解释〕<br>941 部署服务正在停止〔事件解释〕<br>942 部署服务已停止〔事件解释〕 | 939/940 正在启动/运行，941/942 正在停止/停止；描述部署服务，不表示应用包安装卸载成功。<br>G01, G02, G07, G09 |
| AppX 部署事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| AppX 部署事件 / 其他事件<br>`pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 603 开始、404 失败、613 性能摘要已有文案；939–942 是部署服务状态；未构成应用包安装/卸载的统一结果过程。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G07, G09 |
| 终端会话登录、断开与注销 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 终端会话登录、断开与注销 / 运行事件<br>`pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | LSM 24/25 有明确断开/重连状态规则；21–23 等其余原始记录尚未完整适配，会话连接/断开和登录/注销须分开。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G07 |
| 远程连接管理事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 远程连接管理事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 保留远程连接管理记录；认证/连接请求不能直接充当完整会话开始/结束。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G07 |
| 无线网络连接事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 无线网络连接事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 原始连接事件保留；缺连接/断开/失败的统一对象过程适配。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G06, G07 |
| 网络配置文件事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-networkprofile/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 网络配置文件事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-networkprofile/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 原始网络配置文件事件保留；缺可靠接口/配置文件身份下的连接过程关联。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G06, G07 |
| 用户模式驱动事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 用户模式驱动事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | UMDF 运行/故障原始记录；未有驱动包安装/卸载过程解析；通道存在不代表另一端已实现。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G06, G07 |
| 打印服务事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-printservice/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 打印服务事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-printservice/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 原始打印事件保留；尚缺打印作业完成/取消/失败的统一结构化结果关联。 兜底原始事件，不计为已实现的成对过程。<br>G01, G04, G07 |
| BitLocker 管理事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| BitLocker 管理事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 原始管理日志保留；加密进度、保护暂停和卷锁定须分过程，当前主要依赖原生状态字段。 兜底原始事件，不计为已实现的成对过程。<br>G06, G07 |
| 代码完整性事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-codeintegrity/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| 代码完整性事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-codeintegrity/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 校验失败/阻止等异常证据，通常没有天然成功配对；需按文件身份和一次加载关联，不推定启动根因。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G03, G05, G07, G12 |
| AppLocker 程序与 DLL 事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-applocker/exe and dll.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| AppLocker 程序与 DLL 事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-applocker/exe and dll.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 程序/DLL 允许、审计、阻止记录；是判定结果，不能把允许与阻止当作启停一对。 兜底原始事件，不计为已实现的成对过程。<br>G07 |
| AppLocker 安装包与脚本事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-applocker/msi and script.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| AppLocker 安装包与脚本事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-applocker/msi and script.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 安装包/脚本允许、审计、阻止记录；不代表安装结果。 兜底原始事件，不计为已实现的成对过程。<br>G07 |
| Sysmon / 进程生命周期<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.processes` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：1, 5<br>1 进程启动〔模型动作〕<br>5 进程退出〔模型动作〕 | 1/5 是启退对应分类；原生统一 kind 未接到对象动作（G12），PID/ProcessGuid 仍须可靠关联。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 进程访问与异常活动<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.process-access` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：8, 10, 25<br>8 远程线程创建〔模型动作〕<br>10 进程访问〔模型动作〕<br>25 进程映像异常〔模型动作〕 | 8/10/25 是远程线程、访问和映像异常行为；无天然反向端；生产动作链路仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 网络连接<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.network` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：3<br>3 网络连接〔模型动作〕 | 3 只有连接观察，无关闭事件分类；生产对象行动链路还缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / DNS 查询<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.dns` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：22<br>22 DNS 查询〔模型动作〕 | 22 是查询行为和结果，不是缓存加入/删除的一对；生产动作链路仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 文件操作<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.files` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：2, 11, 15, 23, 26, 27, 28, 29<br>2 文件创建时间修改〔模型动作〕<br>11 文件创建或覆盖〔模型动作〕<br>15 文件流创建〔模型动作〕<br>23 文件删除并归档〔模型动作〕<br>26 文件删除〔模型动作〕<br>27 可执行文件创建被阻止〔模型动作〕<br>28 文件粉碎被阻止〔模型动作〕<br>29 可执行文件创建〔模型动作〕 | 11 创建/覆盖、23/26 删除等已有分类，须名单内文件身份；15 流创建缺移除，生产行动仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 注册表操作<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.registry` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：12, 13, 14<br>12 注册表对象创建或删除〔模型动作〕<br>13 注册表值设置〔模型动作〕<br>14 注册表重命名〔模型动作〕 | 12 须读 EventType 才区分创建/删除，13 设置、14 改名；当前只有事件分类，生产行动仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 模块与驱动加载<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.modules` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：6, 7<br>6 驱动加载〔模型动作〕<br>7 模块加载〔模型动作〕 | 6 驱动加载、7 模块加载是两类对象的同一侧，不是加载卸载一对；生产行动仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / WMI 持久订阅<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.wmi` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：19, 20, 21<br>19 WMI 筛选器活动〔模型动作〕<br>20 WMI 消费者活动〔模型动作〕<br>21 WMI 绑定活动〔模型动作〕 | 19/20/21 是筛选器、消费者、绑定三类记录，须读 Operation 区分增删；生产行动仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 命名管道<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.pipes` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：17, 18<br>17 管道创建〔模型动作〕<br>18 管道连接〔模型动作〕 | 17 创建和18连接不是创建删除一对；无关闭另一端，生产行动仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 原始磁盘读取<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.raw-disk` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：9<br>9 原始磁盘读取〔模型动作〕 | 9 是读取行为，无自然反向端；不能当作磁盘出现或消失，生产行动仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 剪贴板活动<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.clipboard` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：24<br>24 剪贴板变化〔模型动作〕 | 24 是剪贴板变化元数据，无开始结束要求；正文不采集，生产行动仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / Sysmon 服务与配置<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.monitor` | 事件<br>有开关 | 提供程序：Microsoft-Windows-Sysmon；编号：4, 16, 255<br>4 Sysmon 服务状态变化〔模型动作〕<br>16 Sysmon 配置变化〔模型动作〕<br>255 Sysmon 内部错误〔模型动作〕 | 4 需读服务状态区分启停，16 配置变化，255 内部错误不同过程；生产行动仍缺 G12。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Sysmon / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| Sysmon / 其他事件<br>`pcscope.eventlog.microsoft-windows-sysmon/operational.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 前端/目录有进程 1/5、文件 11/23/26 等分类，但生产原生 Kind() 统一给 execution.sysmon_event，未接入对象行动（G12）。6/7 本身也只有加载，3 只有连接观察，没有卸载/关闭另一端。 兜底原始事件，不计为已实现的成对过程。<br>G01, G04, G07, G12, G02, G06, G03, G05, G10 |
| Hyper-V 管理事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| Hyper-V 管理事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 原始虚拟化管理事件保留；未验收虚拟机启停与任务结果过程，环境缺口仍开放。 兜底原始事件，不计为已实现的成对过程。<br>G01, G02, G06, G07, G11 |
| Windows 备份事件 / 来源采集状态<br>`pcscope.eventlog.microsoft-windows-backup.status` | 采集健康<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 来源错误/恢复及断点信息；不当作对象生命周期。<br> |
| Windows 备份事件 / 运行事件<br>`pcscope.eventlog.microsoft-windows-backup.other` | 事件<br>有开关 | 提供程序：通道范围；编号：按分类/兜底读取 | 原始备份记录；尚未实现一次备份开始/完成/失败的过程关联。 兜底原始事件，不计为已实现的成对过程。<br>G07 |
| 文件变更记录<br>`pcscope.usn.changes` | 汇总/导入<br>有开关 | file.watched_file | 逐卷 USN 事件汇总开关，不是另一个独立采集器。<br>G01, G02, G03, G05, G07, G12 |
| 手动导入的事件日志<br>`pcscope.eventlog.import` | 汇总/导入<br>无独立开关（汇总/诊断范围） | — | 手动历史事件导入分组；保留当时事件，不冒充当前运行状态。<br>G07 |
| 资源管理器扩展（未细分／来源状态）<br>`pcscope.shell.registrations` | 状态清单<br>无独立开关（汇总/诊断范围） | shell.preview_handler<br>shell.icon_overlay_handler<br>shell.shell_extension<br>shell.namespace_extension<br>shell.context_menu_registration<br>shell.file_association<br>shell.com_registration | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。<br>G01, G02, G03, G10 |
| 登录启动项（未细分／来源状态）<br>`pcscope.startup.logon` | 状态清单<br>无独立开关（汇总/诊断范围） | startup.logon_startup_entry | 完整原生/兼容清单提供对象和字段；加入/移除沿通用快照链路，必须分别通过对象类型列出的场景。此来源包含父记录展开，必须补子集合增减行动。<br>G01, G02, G03 |
