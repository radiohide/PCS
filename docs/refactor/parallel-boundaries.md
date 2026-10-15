# 并行类型与来源边界表（生成）

生成命令：`node scripts/build-parallel-boundaries.cjs`。

输入：`model/catalog.json`；版本：1.1.0。

输入 SHA-256：`c5f29bb316f3810100cb014c006c8564a45b6877e6b87f7012a5f39bd132ed67`。

这是映射事实表，不代表类型已实现、源已实测或主实现责任已分配。统计对来源和类型去重；零来源类型和无类型来源也保留。目录变化后重新生成，不手工修改。

类型 101 个，独立来源 128 个；多来源类型 35 个，多类型来源 13 个。

## 多来源类型（需要明确归一与实施边界）

| 类型 | 数量 | 供给来源 |
| --- | --- | --- |
| `file.watched_file` | 7 | `pcscope.eventlog.microsoft-windows-codeintegrity/operational`<br>`pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.microsoft-windows-windows defender/operational`<br>`pcscope.eventlog.security`<br>`pcscope.files.metadata-security`<br>`pcscope.usn.changes`<br>`pcscope.usn.{drive}` |
| `policy.os_configuration` | 7 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational`<br>`pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.security`<br>`pcscope.eventlog.system`<br>`pcscope.system.boot-state`<br>`pcscope.system.reboot-pending`<br>`pcscope.system.version` |
| `network.endpoint` | 4 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.security`<br>`pcscope.network.tcp`<br>`pcscope.network.udp` |
| `runtime.terminal_session` | 4 | `pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational`<br>`pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational`<br>`pcscope.eventlog.security`<br>`pcscope.identity.interactive-sessions` |
| `security.protection` | 4 | `pcscope.eventlog.microsoft-windows-windows defender/operational`<br>`pcscope.security.detections`<br>`pcscope.security.protection`<br>`pcscope.security.protection-policy` |
| `startup.scheduled_task` | 4 | `pcscope.eventlog.microsoft-windows-taskscheduler/operational`<br>`pcscope.eventlog.security`<br>`pcscope.startup.task-details`<br>`pcscope.startup.tasks` |
| `startup.service` | 4 | `pcscope.eventlog.security`<br>`pcscope.eventlog.system`<br>`pcscope.startup.service-configuration`<br>`pcscope.startup.services` |
| `desktop.shell_configuration` | 3 | `pcscope.desktop.clipboard`<br>`pcscope.desktop.shell-settings`<br>`pcscope.eventlog.microsoft-windows-sysmon/operational` |
| `hardware.disk` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.hardware.disk-health`<br>`pcscope.storage.disks` |
| `network.connection_profile` | 3 | `pcscope.eventlog.microsoft-windows-networkprofile/operational`<br>`pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational`<br>`pcscope.network.profiles` |
| `runtime.process` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.security`<br>`pcscope.execution.processes` |
| `software.system_update` | 3 | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational`<br>`pcscope.system.hotfixes`<br>`pcscope.system.update-history` |
| `startup.wmi_binding` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational`<br>`pcscope.startup.wmi-bindings` |
| `startup.wmi_consumer` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational`<br>`pcscope.startup.wmi-consumers` |
| `startup.wmi_filter` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational`<br>`pcscope.startup.wmi-filters` |
| `account.group_membership` | 2 | `pcscope.eventlog.security`<br>`pcscope.identity.groups` |
| `account.right_assignment` | 2 | `pcscope.eventlog.security`<br>`pcscope.identity.assigned-rights` |
| `account.user` | 2 | `pcscope.eventlog.security`<br>`pcscope.identity.accounts` |
| `desktop.print_job` | 2 | `pcscope.desktop.print-jobs`<br>`pcscope.eventlog.microsoft-windows-printservice/operational` |
| `file.acl_entry` | 2 | `pcscope.eventlog.security`<br>`pcscope.files.metadata-security` |
| `file.alternate_data_stream` | 2 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.files.metadata-security` |
| `hardware.device` | 2 | `pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational`<br>`pcscope.hardware.devices` |
| `hardware.driver_service` | 2 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.hardware.driver-services` |
| `hardware.volume` | 2 | `pcscope.storage.volumes`<br>`pcscope.usn.volumes` |
| `network.proxy_configuration` | 2 | `pcscope.apps.mihomo`<br>`pcscope.network.access` |
| `network.remote_access_configuration` | 2 | `pcscope.network.access`<br>`pcscope.network.winrm-configuration` |
| `runtime.logon_session` | 2 | `pcscope.eventlog.security`<br>`pcscope.identity.logon-sessions` |
| `runtime.module_instance` | 2 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.execution.modules` |
| `security.bitlocker_configuration` | 2 | `pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management`<br>`pcscope.security.encryption` |
| `security.certificate_registration` | 2 | `pcscope.security.certificates.currentuser`<br>`pcscope.security.certificates.localmachine` |
| `shell.browser_extension` | 2 | `pcscope.apps.chromium-extensions`<br>`pcscope.apps.firefox-extensions` |
| `shell.file_association` | 2 | `pcscope.environment.configuration`<br>`pcscope.shell.registrations` |
| `software.appx_package` | 2 | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational`<br>`pcscope.software.appx-user` |
| `software.installation_registration` | 2 | `pcscope.software`<br>`pcscope.software.user-inventory` |
| `virtualization.virtual_machine` | 2 | `pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin`<br>`pcscope.virtualization.hyperv-vms` |

## 多类型来源（需要明确解析与分发责任）

| 来源 | 数量 | 输出类型 |
| --- | --- | --- |
| `pcscope.eventlog.microsoft-windows-sysmon/operational` | 12 | `desktop.shell_configuration`<br>`file.alternate_data_stream`<br>`file.watched_file`<br>`hardware.disk`<br>`hardware.driver_service`<br>`network.endpoint`<br>`policy.os_configuration`<br>`runtime.module_instance`<br>`runtime.process`<br>`startup.wmi_binding`<br>`startup.wmi_consumer`<br>`startup.wmi_filter` |
| `pcscope.eventlog.security` | 12 | `account.group_membership`<br>`account.right_assignment`<br>`account.user`<br>`file.acl_entry`<br>`file.watched_file`<br>`network.endpoint`<br>`policy.os_configuration`<br>`runtime.logon_session`<br>`runtime.process`<br>`runtime.terminal_session`<br>`startup.scheduled_task`<br>`startup.service` |
| `pcscope.shell.registrations` | 7 | `shell.com_registration`<br>`shell.context_menu_registration`<br>`shell.file_association`<br>`shell.icon_overlay_handler`<br>`shell.namespace_extension`<br>`shell.preview_handler`<br>`shell.shell_extension` |
| `pcscope.virtualization.docker` | 4 | `virtualization.container`<br>`virtualization.container_network`<br>`virtualization.container_volume`<br>`virtualization.image` |
| `pcscope.eventlog.microsoft-windows-wmi-activity/operational` | 3 | `startup.wmi_binding`<br>`startup.wmi_consumer`<br>`startup.wmi_filter` |
| `pcscope.files.metadata-security` | 3 | `file.acl_entry`<br>`file.alternate_data_stream`<br>`file.watched_file` |
| `pcscope.network.access` | 3 | `network.proxy_configuration`<br>`network.remote_access_configuration`<br>`network.vpn_configuration` |
| `pcscope.environment.configuration` | 2 | `policy.environment_variable`<br>`shell.file_association` |
| `pcscope.eventlog.microsoft-windows-windows defender/operational` | 2 | `file.watched_file`<br>`security.protection` |
| `pcscope.eventlog.system` | 2 | `policy.os_configuration`<br>`startup.service` |
| `pcscope.identity.groups` | 2 | `account.group`<br>`account.group_membership` |
| `pcscope.policy.configuration` | 2 | `policy.software_restriction_policy`<br>`policy.uac_policy` |
| `pcscope.security.protection-policy` | 2 | `security.exclusion`<br>`security.protection` |

## 完整类型到来源映射

| 类型 | 数量 | 供给来源 |
| --- | --- | --- |
| `file.watched_file` | 7 | `pcscope.eventlog.microsoft-windows-codeintegrity/operational`<br>`pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.microsoft-windows-windows defender/operational`<br>`pcscope.eventlog.security`<br>`pcscope.files.metadata-security`<br>`pcscope.usn.changes`<br>`pcscope.usn.{drive}` |
| `policy.os_configuration` | 7 | `pcscope.eventlog.microsoft-windows-grouppolicy/operational`<br>`pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.security`<br>`pcscope.eventlog.system`<br>`pcscope.system.boot-state`<br>`pcscope.system.reboot-pending`<br>`pcscope.system.version` |
| `network.endpoint` | 4 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.security`<br>`pcscope.network.tcp`<br>`pcscope.network.udp` |
| `runtime.terminal_session` | 4 | `pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational`<br>`pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational`<br>`pcscope.eventlog.security`<br>`pcscope.identity.interactive-sessions` |
| `security.protection` | 4 | `pcscope.eventlog.microsoft-windows-windows defender/operational`<br>`pcscope.security.detections`<br>`pcscope.security.protection`<br>`pcscope.security.protection-policy` |
| `startup.scheduled_task` | 4 | `pcscope.eventlog.microsoft-windows-taskscheduler/operational`<br>`pcscope.eventlog.security`<br>`pcscope.startup.task-details`<br>`pcscope.startup.tasks` |
| `startup.service` | 4 | `pcscope.eventlog.security`<br>`pcscope.eventlog.system`<br>`pcscope.startup.service-configuration`<br>`pcscope.startup.services` |
| `desktop.shell_configuration` | 3 | `pcscope.desktop.clipboard`<br>`pcscope.desktop.shell-settings`<br>`pcscope.eventlog.microsoft-windows-sysmon/operational` |
| `hardware.disk` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.hardware.disk-health`<br>`pcscope.storage.disks` |
| `network.connection_profile` | 3 | `pcscope.eventlog.microsoft-windows-networkprofile/operational`<br>`pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational`<br>`pcscope.network.profiles` |
| `runtime.process` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.security`<br>`pcscope.execution.processes` |
| `software.system_update` | 3 | `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational`<br>`pcscope.system.hotfixes`<br>`pcscope.system.update-history` |
| `startup.wmi_binding` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational`<br>`pcscope.startup.wmi-bindings` |
| `startup.wmi_consumer` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational`<br>`pcscope.startup.wmi-consumers` |
| `startup.wmi_filter` | 3 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.eventlog.microsoft-windows-wmi-activity/operational`<br>`pcscope.startup.wmi-filters` |
| `account.group_membership` | 2 | `pcscope.eventlog.security`<br>`pcscope.identity.groups` |
| `account.right_assignment` | 2 | `pcscope.eventlog.security`<br>`pcscope.identity.assigned-rights` |
| `account.user` | 2 | `pcscope.eventlog.security`<br>`pcscope.identity.accounts` |
| `desktop.print_job` | 2 | `pcscope.desktop.print-jobs`<br>`pcscope.eventlog.microsoft-windows-printservice/operational` |
| `file.acl_entry` | 2 | `pcscope.eventlog.security`<br>`pcscope.files.metadata-security` |
| `file.alternate_data_stream` | 2 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.files.metadata-security` |
| `hardware.device` | 2 | `pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational`<br>`pcscope.hardware.devices` |
| `hardware.driver_service` | 2 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.hardware.driver-services` |
| `hardware.volume` | 2 | `pcscope.storage.volumes`<br>`pcscope.usn.volumes` |
| `network.proxy_configuration` | 2 | `pcscope.apps.mihomo`<br>`pcscope.network.access` |
| `network.remote_access_configuration` | 2 | `pcscope.network.access`<br>`pcscope.network.winrm-configuration` |
| `runtime.logon_session` | 2 | `pcscope.eventlog.security`<br>`pcscope.identity.logon-sessions` |
| `runtime.module_instance` | 2 | `pcscope.eventlog.microsoft-windows-sysmon/operational`<br>`pcscope.execution.modules` |
| `security.bitlocker_configuration` | 2 | `pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management`<br>`pcscope.security.encryption` |
| `security.certificate_registration` | 2 | `pcscope.security.certificates.currentuser`<br>`pcscope.security.certificates.localmachine` |
| `shell.browser_extension` | 2 | `pcscope.apps.chromium-extensions`<br>`pcscope.apps.firefox-extensions` |
| `shell.file_association` | 2 | `pcscope.environment.configuration`<br>`pcscope.shell.registrations` |
| `software.appx_package` | 2 | `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational`<br>`pcscope.software.appx-user` |
| `software.installation_registration` | 2 | `pcscope.software`<br>`pcscope.software.user-inventory` |
| `virtualization.virtual_machine` | 2 | `pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin`<br>`pcscope.virtualization.hyperv-vms` |
| `account.group` | 1 | `pcscope.identity.groups` |
| `account.machine_identity` | 1 | `pcscope.identity.machine` |
| `account.user_profile` | 1 | `pcscope.identity.profiles` |
| `desktop.editor_extension` | 1 | `pcscope.apps.ide-extensions` |
| `desktop.input_configuration` | 1 | `pcscope.desktop.input` |
| `desktop.printer` | 1 | `pcscope.desktop.printing` |
| `desktop.printer_driver` | 1 | `pcscope.desktop.printer-drivers` |
| `desktop.printer_port` | 1 | `pcscope.desktop.printer-ports` |
| `desktop.shortcut` | 1 | `pcscope.desktop.shortcuts` |
| `desktop.sync_account` | 1 | `pcscope.apps.onedrive-registration` |
| `hardware.battery` | 1 | `pcscope.hardware.battery` |
| `hardware.driver_binding` | 1 | `pcscope.hardware.driver-bindings` |
| `hardware.firmware` | 1 | `pcscope.hardware.firmware` |
| `hardware.partition` | 1 | `pcscope.storage.partitions` |
| `hardware.tpm` | 1 | `pcscope.security.tpm` |
| `network.adapter` | 1 | `pcscope.network.adapters` |
| `network.address_configuration` | 1 | `pcscope.network.addresses` |
| `network.dns_cache_entry` | 1 | `pcscope.network.dns-cache` |
| `network.dns_configuration` | 1 | `pcscope.network.dns-configuration` |
| `network.firewall_profile` | 1 | `pcscope.network.firewall-profiles` |
| `network.firewall_rule` | 1 | `pcscope.network.firewall-rules` |
| `network.hosts_entry` | 1 | `pcscope.network.hosts` |
| `network.ipsec_rule` | 1 | `pcscope.network.ipsec` |
| `network.route` | 1 | `pcscope.network.routes` |
| `network.smb_mapping` | 1 | `pcscope.network.mapped-drives` |
| `network.smb_session` | 1 | `pcscope.network.share-sessions` |
| `network.smb_share` | 1 | `pcscope.network.shares` |
| `network.vpn_configuration` | 1 | `pcscope.network.access` |
| `policy.browser_policy` | 1 | `pcscope.apps.browser-policies` |
| `policy.environment_variable` | 1 | `pcscope.environment.configuration` |
| `policy.power_plan` | 1 | `pcscope.power.configuration` |
| `policy.software_restriction_policy` | 1 | `pcscope.policy.configuration` |
| `policy.system_restore_point` | 1 | `pcscope.system.restore-points` |
| `policy.time_sync_configuration` | 1 | `pcscope.system.clock` |
| `policy.uac_policy` | 1 | `pcscope.policy.configuration` |
| `security.device_guard_configuration` | 1 | `pcscope.security.deviceguard` |
| `security.exclusion` | 1 | `pcscope.security.protection-policy` |
| `security.secure_boot_configuration` | 1 | `pcscope.security.secureboot` |
| `security.security_product` | 1 | `pcscope.security.products` |
| `security.ssh_public_key` | 1 | `pcscope.security.ssh-public-keys` |
| `shell.com_registration` | 1 | `pcscope.shell.registrations` |
| `shell.context_menu_registration` | 1 | `pcscope.shell.registrations` |
| `shell.icon_overlay_handler` | 1 | `pcscope.shell.registrations` |
| `shell.namespace_extension` | 1 | `pcscope.shell.registrations` |
| `shell.preview_handler` | 1 | `pcscope.shell.registrations` |
| `shell.shell_extension` | 1 | `pcscope.shell.registrations` |
| `software.language_package` | 1 | `pcscope.apps.npm-global-packages` |
| `software.optional_feature` | 1 | `pcscope.software.optional-features` |
| `software.portable_registration` | 1 | `pcscope.software.portable` |
| `software.provisioned_package` | 1 | `pcscope.software.provisioned` |
| `software.runtime` | 1 | `pcscope.apps.dotnet-runtimes` |
| `startup.ifeo_configuration` | 1 | `pcscope.startup.advanced-extensions` |
| `startup.logon_initialization` | 1 | `pcscope.startup.session-hooks` |
| `startup.logon_startup_entry` | 1 | `pcscope.startup.logon` |
| `virtualization.checkpoint` | 1 | `pcscope.virtualization.hyperv-checkpoints` |
| `virtualization.container` | 1 | `pcscope.virtualization.docker` |
| `virtualization.container_network` | 1 | `pcscope.virtualization.docker` |
| `virtualization.container_volume` | 1 | `pcscope.virtualization.docker` |
| `virtualization.disk_attachment` | 1 | `pcscope.virtualization.hyperv-disks` |
| `virtualization.image` | 1 | `pcscope.virtualization.docker` |
| `virtualization.platform_capability` | 1 | `pcscope.virtualization.platform` |
| `virtualization.virtual_switch` | 1 | `pcscope.virtualization.hyperv-switches` |
| `virtualization.wsl_distribution` | 1 | `pcscope.virtualization.wsl-registration` |
| `file.watched_directory` | 0 | 无 |
| `hardware.driver_package` | 0 | 无 |
| `security.key_metadata` | 0 | 无 |

## 完整来源到类型映射

| 来源 | 数量 | 输出类型 |
| --- | --- | --- |
| `pcscope.eventlog.microsoft-windows-sysmon/operational` | 12 | `desktop.shell_configuration`<br>`file.alternate_data_stream`<br>`file.watched_file`<br>`hardware.disk`<br>`hardware.driver_service`<br>`network.endpoint`<br>`policy.os_configuration`<br>`runtime.module_instance`<br>`runtime.process`<br>`startup.wmi_binding`<br>`startup.wmi_consumer`<br>`startup.wmi_filter` |
| `pcscope.eventlog.security` | 12 | `account.group_membership`<br>`account.right_assignment`<br>`account.user`<br>`file.acl_entry`<br>`file.watched_file`<br>`network.endpoint`<br>`policy.os_configuration`<br>`runtime.logon_session`<br>`runtime.process`<br>`runtime.terminal_session`<br>`startup.scheduled_task`<br>`startup.service` |
| `pcscope.shell.registrations` | 7 | `shell.com_registration`<br>`shell.context_menu_registration`<br>`shell.file_association`<br>`shell.icon_overlay_handler`<br>`shell.namespace_extension`<br>`shell.preview_handler`<br>`shell.shell_extension` |
| `pcscope.virtualization.docker` | 4 | `virtualization.container`<br>`virtualization.container_network`<br>`virtualization.container_volume`<br>`virtualization.image` |
| `pcscope.eventlog.microsoft-windows-wmi-activity/operational` | 3 | `startup.wmi_binding`<br>`startup.wmi_consumer`<br>`startup.wmi_filter` |
| `pcscope.files.metadata-security` | 3 | `file.acl_entry`<br>`file.alternate_data_stream`<br>`file.watched_file` |
| `pcscope.network.access` | 3 | `network.proxy_configuration`<br>`network.remote_access_configuration`<br>`network.vpn_configuration` |
| `pcscope.environment.configuration` | 2 | `policy.environment_variable`<br>`shell.file_association` |
| `pcscope.eventlog.microsoft-windows-windows defender/operational` | 2 | `file.watched_file`<br>`security.protection` |
| `pcscope.eventlog.system` | 2 | `policy.os_configuration`<br>`startup.service` |
| `pcscope.identity.groups` | 2 | `account.group`<br>`account.group_membership` |
| `pcscope.policy.configuration` | 2 | `policy.software_restriction_policy`<br>`policy.uac_policy` |
| `pcscope.security.protection-policy` | 2 | `security.exclusion`<br>`security.protection` |
| `pcscope.apps.browser-policies` | 1 | `policy.browser_policy` |
| `pcscope.apps.chromium-extensions` | 1 | `shell.browser_extension` |
| `pcscope.apps.dotnet-runtimes` | 1 | `software.runtime` |
| `pcscope.apps.firefox-extensions` | 1 | `shell.browser_extension` |
| `pcscope.apps.ide-extensions` | 1 | `desktop.editor_extension` |
| `pcscope.apps.mihomo` | 1 | `network.proxy_configuration` |
| `pcscope.apps.npm-global-packages` | 1 | `software.language_package` |
| `pcscope.apps.onedrive-registration` | 1 | `desktop.sync_account` |
| `pcscope.desktop.clipboard` | 1 | `desktop.shell_configuration` |
| `pcscope.desktop.input` | 1 | `desktop.input_configuration` |
| `pcscope.desktop.print-jobs` | 1 | `desktop.print_job` |
| `pcscope.desktop.printer-drivers` | 1 | `desktop.printer_driver` |
| `pcscope.desktop.printer-ports` | 1 | `desktop.printer_port` |
| `pcscope.desktop.printing` | 1 | `desktop.printer` |
| `pcscope.desktop.shell-settings` | 1 | `desktop.shell_configuration` |
| `pcscope.desktop.shortcuts` | 1 | `desktop.shortcut` |
| `pcscope.eventlog.microsoft-windows-appxdeploymentserver/operational` | 1 | `software.appx_package` |
| `pcscope.eventlog.microsoft-windows-bitlocker/bitlocker management` | 1 | `security.bitlocker_configuration` |
| `pcscope.eventlog.microsoft-windows-codeintegrity/operational` | 1 | `file.watched_file` |
| `pcscope.eventlog.microsoft-windows-driverframeworks-usermode/operational` | 1 | `hardware.device` |
| `pcscope.eventlog.microsoft-windows-grouppolicy/operational` | 1 | `policy.os_configuration` |
| `pcscope.eventlog.microsoft-windows-hyper-v-vmms/admin` | 1 | `virtualization.virtual_machine` |
| `pcscope.eventlog.microsoft-windows-networkprofile/operational` | 1 | `network.connection_profile` |
| `pcscope.eventlog.microsoft-windows-printservice/operational` | 1 | `desktop.print_job` |
| `pcscope.eventlog.microsoft-windows-taskscheduler/operational` | 1 | `startup.scheduled_task` |
| `pcscope.eventlog.microsoft-windows-terminalservices-localsessionmanager/operational` | 1 | `runtime.terminal_session` |
| `pcscope.eventlog.microsoft-windows-terminalservices-remoteconnectionmanager/operational` | 1 | `runtime.terminal_session` |
| `pcscope.eventlog.microsoft-windows-windowsupdateclient/operational` | 1 | `software.system_update` |
| `pcscope.eventlog.microsoft-windows-wlan-autoconfig/operational` | 1 | `network.connection_profile` |
| `pcscope.execution.modules` | 1 | `runtime.module_instance` |
| `pcscope.execution.processes` | 1 | `runtime.process` |
| `pcscope.hardware.battery` | 1 | `hardware.battery` |
| `pcscope.hardware.devices` | 1 | `hardware.device` |
| `pcscope.hardware.disk-health` | 1 | `hardware.disk` |
| `pcscope.hardware.driver-bindings` | 1 | `hardware.driver_binding` |
| `pcscope.hardware.driver-services` | 1 | `hardware.driver_service` |
| `pcscope.hardware.firmware` | 1 | `hardware.firmware` |
| `pcscope.identity.accounts` | 1 | `account.user` |
| `pcscope.identity.assigned-rights` | 1 | `account.right_assignment` |
| `pcscope.identity.interactive-sessions` | 1 | `runtime.terminal_session` |
| `pcscope.identity.logon-sessions` | 1 | `runtime.logon_session` |
| `pcscope.identity.machine` | 1 | `account.machine_identity` |
| `pcscope.identity.profiles` | 1 | `account.user_profile` |
| `pcscope.network.adapters` | 1 | `network.adapter` |
| `pcscope.network.addresses` | 1 | `network.address_configuration` |
| `pcscope.network.dns-cache` | 1 | `network.dns_cache_entry` |
| `pcscope.network.dns-configuration` | 1 | `network.dns_configuration` |
| `pcscope.network.firewall-profiles` | 1 | `network.firewall_profile` |
| `pcscope.network.firewall-rules` | 1 | `network.firewall_rule` |
| `pcscope.network.hosts` | 1 | `network.hosts_entry` |
| `pcscope.network.ipsec` | 1 | `network.ipsec_rule` |
| `pcscope.network.mapped-drives` | 1 | `network.smb_mapping` |
| `pcscope.network.profiles` | 1 | `network.connection_profile` |
| `pcscope.network.routes` | 1 | `network.route` |
| `pcscope.network.share-sessions` | 1 | `network.smb_session` |
| `pcscope.network.shares` | 1 | `network.smb_share` |
| `pcscope.network.tcp` | 1 | `network.endpoint` |
| `pcscope.network.udp` | 1 | `network.endpoint` |
| `pcscope.network.winrm-configuration` | 1 | `network.remote_access_configuration` |
| `pcscope.power.configuration` | 1 | `policy.power_plan` |
| `pcscope.security.certificates.currentuser` | 1 | `security.certificate_registration` |
| `pcscope.security.certificates.localmachine` | 1 | `security.certificate_registration` |
| `pcscope.security.detections` | 1 | `security.protection` |
| `pcscope.security.deviceguard` | 1 | `security.device_guard_configuration` |
| `pcscope.security.encryption` | 1 | `security.bitlocker_configuration` |
| `pcscope.security.products` | 1 | `security.security_product` |
| `pcscope.security.protection` | 1 | `security.protection` |
| `pcscope.security.secureboot` | 1 | `security.secure_boot_configuration` |
| `pcscope.security.ssh-public-keys` | 1 | `security.ssh_public_key` |
| `pcscope.security.tpm` | 1 | `hardware.tpm` |
| `pcscope.software` | 1 | `software.installation_registration` |
| `pcscope.software.appx-user` | 1 | `software.appx_package` |
| `pcscope.software.optional-features` | 1 | `software.optional_feature` |
| `pcscope.software.portable` | 1 | `software.portable_registration` |
| `pcscope.software.provisioned` | 1 | `software.provisioned_package` |
| `pcscope.software.user-inventory` | 1 | `software.installation_registration` |
| `pcscope.startup.advanced-extensions` | 1 | `startup.ifeo_configuration` |
| `pcscope.startup.logon` | 1 | `startup.logon_startup_entry` |
| `pcscope.startup.service-configuration` | 1 | `startup.service` |
| `pcscope.startup.services` | 1 | `startup.service` |
| `pcscope.startup.session-hooks` | 1 | `startup.logon_initialization` |
| `pcscope.startup.task-details` | 1 | `startup.scheduled_task` |
| `pcscope.startup.tasks` | 1 | `startup.scheduled_task` |
| `pcscope.startup.wmi-bindings` | 1 | `startup.wmi_binding` |
| `pcscope.startup.wmi-consumers` | 1 | `startup.wmi_consumer` |
| `pcscope.startup.wmi-filters` | 1 | `startup.wmi_filter` |
| `pcscope.storage.disks` | 1 | `hardware.disk` |
| `pcscope.storage.partitions` | 1 | `hardware.partition` |
| `pcscope.storage.volumes` | 1 | `hardware.volume` |
| `pcscope.system.boot-state` | 1 | `policy.os_configuration` |
| `pcscope.system.clock` | 1 | `policy.time_sync_configuration` |
| `pcscope.system.hotfixes` | 1 | `software.system_update` |
| `pcscope.system.reboot-pending` | 1 | `policy.os_configuration` |
| `pcscope.system.restore-points` | 1 | `policy.system_restore_point` |
| `pcscope.system.update-history` | 1 | `software.system_update` |
| `pcscope.system.version` | 1 | `policy.os_configuration` |
| `pcscope.usn.{drive}` | 1 | `file.watched_file` |
| `pcscope.usn.changes` | 1 | `file.watched_file` |
| `pcscope.usn.volumes` | 1 | `hardware.volume` |
| `pcscope.virtualization.hyperv-checkpoints` | 1 | `virtualization.checkpoint` |
| `pcscope.virtualization.hyperv-disks` | 1 | `virtualization.disk_attachment` |
| `pcscope.virtualization.hyperv-switches` | 1 | `virtualization.virtual_switch` |
| `pcscope.virtualization.hyperv-vms` | 1 | `virtualization.virtual_machine` |
| `pcscope.virtualization.platform` | 1 | `virtualization.platform_capability` |
| `pcscope.virtualization.wsl-registration` | 1 | `virtualization.wsl_distribution` |
| `pcscope.collector.event-channel-coverage` | 0 | 无 |
| `pcscope.collector.scoped-configuration` | 0 | 无 |
| `pcscope.eventlog.application` | 0 | 无 |
| `pcscope.eventlog.import` | 0 | 无 |
| `pcscope.eventlog.microsoft-windows-applocker/exe and dll` | 0 | 无 |
| `pcscope.eventlog.microsoft-windows-applocker/msi and script` | 0 | 无 |
| `pcscope.eventlog.microsoft-windows-backup` | 0 | 无 |
| `pcscope.eventlog.microsoft-windows-powershell/operational` | 0 | 无 |
| `pcscope.execution.resources` | 0 | 无 |
| `pcscope.usn.watch-list` | 0 | 无 |

## 实施责任登记要求

R07 另行记录每个重叠类型的主实现、公共解析责任、身份归一责任、待决边界和验证样例。本表不自动把类型或来源分配给工作线，也不据此将 R07 标记通过。
