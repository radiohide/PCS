# 缺名软件登记的名称解释字典

日期：2026-10-12。用户要求说明“未命名的软件登记”对应什么对象。代码基准为根目录 `pcscope`。

## 展示与判断边界

新增 12 条精确匹配规则，使用已观察的卸载登记完整路径、注册表视图及可用发布者作匹配。界面显示中文名称、一句用途和“按登记识别”，当前使用情况保持未确认。真实来源名称优先；字段被保护、有冲突或读取失败时不使用字典补出名称。没有匹配的记录继续折叠为未识别登记。

这些是带依据的名称解释，**不是安装状态或真实性认证**。除特定本机 Active Setup 组件名外，没有把官方组件说明说成官方提供的完整卸载键映射。不能由字典判断文件存在、组件已安装、仍在使用、健康、可以删除或必须保留。具体用途未证实的 6 项保持未知，不靠缩写编造说明。

| 登记标识 | 默认名称 | 依据性质 |
| --- | --- | --- |
| AddressBook | 旧版 Windows 通讯簿相关登记 | [微软通讯簿说明](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/wab/-wab-overview)，按名称解释 |
| Connection Manager | 网络连接管理组件相关登记 | [微软连接管理器说明](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2012-r2-and-2012/hh831583%28v%3Dws.11%29)，按同词标识解释 |
| DirectDrawEx | 旧版图形组件相关登记 | 本机 Active Setup 精确 ComponentID=DirectDrawEx；图形用途按 [DirectDraw 文档](https://learn.microsoft.com/en-us/windows/win32/api/ddraw/nf-ddraw-directdrawcreateex)作组件家族推断，信度较低 |
| Fontcore | 旧版浏览器字体组件登记 | 本机 Active Setup 精确 ComponentID=Fontcore，默认名称=Internet Explorer Core Fonts |
| MPlayer2 | 旧版媒体播放器组件相关登记 | [微软日志属性说明](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/dd874350%28v%3Dvs.85%29)明确 mplayer2.exe 是独立媒体播放器 |
| mspaint 加本次观察到的完整 GUID | 画图组件登记 | 本机同名前缀文件资源“画图”、登记的 Microsoft 发布者及 [微软产品说明](https://www.microsoft.com/en-us/windows/paint)；无任意 GUID 通配 |
| mstsc 加本次观察到的完整 GUID | 远程桌面连接组件登记 | 本机文件资源与发布者，以及 [微软客户端说明](https://learn.microsoft.com/en-us/troubleshoot/windows-client/remote/use-mstsc-universal-remote-desktop-client-instead-rdman) |
| SnippingTool 加本次观察到的完整 GUID | 截图工具组件登记 | 本机文件资源与发布者，以及 [微软截图工具说明](https://support.microsoft.com/en-au/windows/apps/use-snipping-tool-to-capture-screenshots) |
| SchedulingAgent | 任务计划组件相关登记 | [微软任务计划协议](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-tsch/2d1fbbab-fe6c-4ae5-bdf5-41dc526b2439)使用该配置名称；不等于卸载路径的官方映射 |
| WIC | 图像处理组件相关登记 | [微软图像组件说明](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-about-windows-imaging-codec)，按官方组件缩写解释 |
| Microsoft Edge Update | Edge 更新组件登记 | [微软更新组件说明](https://learn.microsoft.com/en-us/troubleshoot/microsoft-edge/manageability/update-install-rollback-failures)及本机同名程序资源 |
| 本次观察到的完整 GUID 加 Display.NVWMI | NVIDIA 显卡管理组件登记 | [NVIDIA 白皮书](https://www.nvidia.com/content/quadro_fx_product_literature/wp-06953-001-v02.pdf)第 1 页说明用途、第 7 页明确包名；不匹配其他 GUID |

待确认：DXM_Runtime、IE40、IE4Data、IE5BAKEX、IEData、MobileOptionPack。本轮本机研究为一次性、限定范围、只读核对，未把 PowerShell 扫描加入产品周期任务。

## 实现与维护

字典位于 `crates/storage/src/registration_dictionary.json`，编译内嵌到原生后端。新增条目需写明名称、用途、完整匹配条件、发布者白名单和参考依据，变更时递增字典修订。相同字典名称不会合并不同对象。显示、中文搜索、排序和游标用同一个有效名称表达式，字典修订参与游标指纹。

没有修改数据库身份、原始材料、安装状态或采集调度；没有增加注册表轮询、文件扫描、联网查询或脚本子进程。展示扩展修订为 2，基础查询信封仍为 1，保留旧响应兼容。共享变更登记见 [0011](../shared-changes/0011-registration-dictionary.md)。

证据资料：`target/dictionary-source-research.json`、`target/dictionary-windows-active-setup-evidence.json`、`target/dictionary-registration-input.json`、`target/dictionary-windows-file-identities.json`。它们支持解释规则，不能用同机器上的独立文件存在反向证明该空登记有效。

## 验收

本轮验证已通过：

| 检查 | 结果 | 覆盖范围 |
| --- | --- | --- |
| Rust 查询回归 | 27/27 | 名称优先级、精确登记与发布者匹配、保护字段、搜索、排序、分页、旧字典游标拒绝及既有时间线归属检查 |
| Rust 字典模块 | 3/3 | 内嵌字典有效性、路径与视图边界、重复规则拒绝、SQL 字符串引用和保护状态 |
| Node 界面回归 | 24/24 | 界面与查询契约相关检查 |
| Chrome 展示验收 | 13/13 | 12 个受控场景和 1 个保存的真实查询样本；页面脚本无报错 |
| 最终构建与打包 | 通过 | 原生后台、二进制与示例、桌面构建完成；29 个文件复制后 SHA256 全部一致 |
| 归属检查 | 360/360 | 无未分配文件，原有归属约束保留 |

浏览器验收使用真实界面渲染代码和保存的查询响应，没有将受控场景当成正在运行的桌面程序验收。结果见 [浏览器记录](../../../target/software-registration-dictionary-browser-qa-result.json)；Rust 记录见 [查询回归](../../../target/dictionary-query-tests.log)和[字典模块回归](../../../target/dictionary-module-tests.log)。

本轮[只读实库快照](../../../target/dictionary-presentation-audit.json)包含 **49 条软件登记：29 条使用来源名称、12 条使用字典解释、8 条未匹配**。与上一轮的 39 条记录按对象编号逐条比较，原有 21 个来源名称 **21/21 保留**；原有 18 条缺名登记中，12 条匹配字典，6 条继续未知。期间后台新增了 10 条记录，其中 8 条已有来源名称、2 条未匹配；数量增长不是字典生成新对象。对比结果见[名称保留记录](../../../target/dictionary-names-comparison.json)。

性能在原有 6 秒查询预算内验收，没有提高超时：软件列表 61 ms；12 个字典名称逐一搜索均命中原对象，单次 69–82 ms；按每页 7 条遍历全部 49 条耗时 473 ms，与一次性排序结果完全一致，无漏项或重复。80 条时间线查询为 407 ms。所有实库操作使用只读连接，一致读取事务未修改历史、权限或采集设置。

### 待修复的来源建模问题

新增的两条未匹配记录分别来自当前用户的 32 位和 64 位注册表视图，身份路径都是 `SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall` 根目录。这是软件登记的目录容器，不是某个软件的独立子项；现有用户软件来源将根容器纳入了软件登记模型。后续需修复来源映射及对应历史呈现。本轮没有用字典或虚构名称掩盖这两条记录，也没有删除原始证据。

### 运行更新

开发包为 `dist/registration-dictionary`，文件校验清单见 [产物记录](registration-dictionary-artifacts.json)，预览入口已指向新版。桌面构建仍有此前的 MinGW 多 manifest 链接警告，不能据当前开发包运行验收视作交付问题已解决。

完整包已经复制到当前用户的普通临时应用目录 `PCScope-Preview-826abd1a6a814ff695941744b8f65035`，29 个文件逐一哈希一致。新版界面 PID 6116 已打开，窗口标题 PCScope、Responding=true；旧界面 PID 20904 已正常关闭。启动记录见 [界面启动](../../../target/dictionary-desktop-launch.json)。

本轮已发起一次同账户 UAC 后台替换，截至本轮最终核对，Windows 授权窗口仍待人工确认，原增强后台 PID 7920 正常运行。因此**不宣称新后台的字典已经在运行窗口生效**；浏览器实库显示证据来自已编译的新查询实现。更新前后只读回查的数据目录、增强采集及已有 repair 设置一致，见 [更新前](../../../target/dictionary-settings-before.json)和[最终核对](../../../target/dictionary-settings-after.json)。没有提升 Codex 或界面权限，没有重复发起授权，也没有强制终止旧后台。待用户处理已打开的授权窗口后，仍需核对新后台 PID 和实时查询字典字段。

2026-10-13 追加核对：此前 UAC 已完成，替换请求记录的后台 PID 为 20844，当前仅此后台运行。本轮直接 IPC 返回 49 条软件记录，其中 12 条带字典名称；字典对象 32 的详情返回“Edge 更新组件登记”，耗时 69 ms，身份一致。字典已在实际后台生效，证据为 `target/dictionary-live-software.json`、`target/object-detail-live-responses.json` 和 `target/dictionary-replacement-request.json`。本次没有再次提权。独立发现的状态接口超时与容量问题见 [运行环境补充](object-detail-runtime-notes.md)，不据字典查询成功宣称全部后台接口稳定。
