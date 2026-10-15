'use strict';
// Provider-qualified rules. Sources: Microsoft audit documentation and local provider
// message resources; observed operational events were checked against retained evidence.
const PCScopeEventSemantics=(()=>{
 const families=[];
 function add(channel,provider,suffix,leaf,text){
  const events={};for(const line of text.trim().split('\n')){const [id,name,explanation]=line.split('|');if(!explanation)throw Error('Missing event explanation: '+line);events[Number(id)]={name,explanation,action:`event.${channel.toLowerCase().replace(/[^a-z0-9]/g,'_')}.${provider.toLowerCase().replace(/[^a-z0-9]/g,'_')}.${id}`};}
  families.push({channel,provider,suffix,leaf,events});
 }
 const sec=(suffix,leaf,text)=>add('Security','Microsoft-Windows-Security-Auditing',suffix,leaf,text);
 sec('crypto-keys','加密密钥操作',`
5058|密钥文件操作|通过密钥存储提供程序访问密钥的存储文件；读取、写入或删除须根据操作字段确定。
5059|密钥导入与导出|通过密钥存储提供程序执行密钥迁移；具体为导入还是导出须根据操作字段确定。
5061|加密与密钥使用|密钥存储提供程序记录了密钥打开、创建、删除、签名、协商、派生或加解密操作；事件编号本身不能确定具体动作。
`);
 sec('crypto-integrity','密码组件自检与验证',`
5056|密码组件自检|密码组件执行了自检，结果须结合返回码判断。
5057|密码基础操作失败|密码组件报告基础加密操作失败；查看算法、失败原因及返回码。
5060|密码验证失败|密码组件报告验证操作失败；不能仅据此判定密钥泄露或程序被篡改。
5062|内核密码组件自检|内核模式密码组件执行了自检，结果须结合返回码判断。
6417|FIPS 密码自检成功|提供程序报告 FIPS 模式密码自检成功；只说明这次自检结果。
`);
 sec('crypto-configuration','密码提供程序与上下文配置',`
5063|密码提供程序操作|记录一次密码提供程序操作尝试，是否成功须查看结果。
5064|密码上下文操作|记录一次密码上下文操作尝试，不自动等同于配置修改。
5065|密码上下文修改|记录一次密码上下文修改尝试，是否生效须查看结果。
5066|密码函数操作|记录一次密码函数操作尝试，具体函数和结果由来源字段说明。
5067|密码函数修改|记录一次密码函数修改尝试，是否生效须查看结果。
5068|密码函数提供程序操作|记录一次密码函数提供程序操作尝试。
5069|密码函数属性操作|记录一次密码函数属性操作尝试，不自动等同于修改。
5070|密码函数属性修改|记录一次密码函数属性修改尝试，是否生效须查看结果。
`);
 sec('data-protection','DPAPI 数据保护与主密钥',`
4692|数据保护主密钥备份|记录一次 DPAPI 主密钥备份尝试。
4693|数据保护主密钥恢复|记录一次 DPAPI 主密钥恢复尝试，结果须查看状态码。
4694|数据保护操作|记录一次对可审计数据执行保护的尝试，不能由元数据还原被保护内容。
4695|解除数据保护操作|记录一次解除可审计数据保护的尝试，结果须查看状态码。
`);
 sec('credential-access','凭据读取与账户检查',`
5379|凭据管理器读取|发生了凭据管理器读取操作；可能是查询或枚举，不能仅凭事件标题认定成功取出了密码。
5382|凭据保管库读取|发生了凭据保管库读取操作；实际是否成功须结合返回码。
4797|账户空密码检查|尝试查询指定账户是否存在空密码。
4648|显式凭据登录尝试|进程尝试使用显式提供的凭据登录；须关联后续登录结果，不能把尝试当作成功。
`);
 sec('membership-queries','用户组成员查询',`
4798|查询用户所属本地组|查询了某个用户的本地组成员关系；这是读取操作。
4799|枚举本地用户组成员|枚举了启用安全性的本地用户组成员；这是读取操作。
`);
 sec('object-access','WMI 与目录对象访问审计',`
4662|受审计对象操作|记录对受审计对象的操作。ObjectServer 和 ObjectType 用于区分 WMI 命名空间、目录对象等；不能一律视为 Active Directory 修改。
`);
 // Microsoft Security 4907 documents a SACL change, distinct from a DACL or file-content change:
 // https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/auditing/event-4907
 sec('files-audit-settings','对象访问记录规则',`
4907|对象访问审计规则已修改|文件或注册表等对象的访问记录规则发生变化，影响后续哪些访问会写入安全日志。
`);
 sec('process-tokens','进程访问令牌分配',`
4696|进程主令牌分配|记录向目标进程分配主访问令牌；需区分发起进程与目标进程。
`);
 sec('system-audit','系统启动、时间与审计配置',`
4608|安全审计子系统启动|Windows 启动过程中初始化了安全审计子系统。
4616|系统时间变更|系统时间发生调整；可能由时间同步或管理操作触发，应查看前后时间和发起进程。
4826|启动配置加载|系统加载了启动配置数据。
4902|用户审计策略表建立|系统建立了按用户的审计策略表；查看元素数量及策略标识。
`);
 sec('firewall-lifecycle','防火墙组件生命周期',`
5024|防火墙服务启动成功|Windows 防火墙服务报告启动成功。
5033|防火墙驱动启动成功|Windows 防火墙驱动报告启动成功。
`);
 add('Security','Microsoft-Windows-Eventlog','audit-service','审计日志服务',`
1100|事件日志服务关闭|事件日志服务已关闭；需要结合关机记录判断背景，不能仅凭此事件认定日志被恶意清理。
`);
 const gp=(suffix,leaf,text)=>add('Microsoft-Windows-GroupPolicy/Operational','Microsoft-Windows-GroupPolicy',suffix,leaf,text);
 gp('service','组策略服务生命周期',`
4115|组策略服务启动|组策略组件报告服务启动；原因须结合启动、服务和相邻记录确认。
4116|组策略服务初始化开始|开始初始化组策略服务内部状态。
5116|组策略服务初始化完成|组策略服务初始化成功完成。
5115|组策略服务停止|组策略组件报告服务停止，不能仅凭此条判定故障。
`);
 gp('processing','策略处理开始与完成',`
4000|开机策略处理开始|开始处理计算机启动时的组策略。
8000|开机策略处理完成|计算机启动策略处理完成，错误码和耗时见来源字段。
4001|登录策略处理开始|开始处理用户登录时的组策略。
8001|登录策略处理完成|用户登录策略处理完成。
4004|手动计算机策略处理开始|开始手动触发的计算机策略处理；具体发起程序需要其他证据确认。
8004|手动计算机策略处理完成|手动触发的计算机策略处理完成。
4005|手动用户策略处理开始|开始手动触发的用户策略处理；具体发起程序需要其他证据确认。
8005|手动用户策略处理完成|手动触发的用户策略处理完成。
`);
 gp('sessions','策略会话与登录通知',`
4117|策略会话开始|开始一个策略处理会话。
5117|策略会话完成|组策略处理会话成功完成。
5324|收到登录会话通知|组策略收到 Winlogon 会话通知，具体通知类型见来源正文。
5325|收到关闭前通知|组策略收到服务控制管理器的关机前通知。
5351|返回登录组件|组策略会话向 Winlogon 返回控制权。
6339|登录桌面处理完成|组策略完成 Winlogon 启动桌面相关处理。
`);
 gp('applicability','策略获取、适用与排除',`
4126|策略获取开始|开始获取适用策略；来源模板提及域控制器不单独证明本机加入了域。
5126|策略获取完成|适用策略获取成功完成；应结合实际策略列表查看是否有内容。
5257|策略下载完成|策略下载阶段完成，不能仅据此判断策略发生变化。
5312|适用策略列表|列出本次适用的组策略对象；None 表示来源报告没有适用对象。
5313|被排除的策略列表|列出未应用的策略及排除原因。
`);
 gp('diagnostics','处理模式与内部诊断',`
5311|环回处理模式|记录组策略环回处理模式。
5320|组策略内部处理状态|记录组策略内部处理阶段及诊断信息。
5321|组策略实例诊断|记录服务实例检测等诊断信息；发现既有实例不单独表示服务故障。
5340|策略处理模式|记录前台、后台或同步方式等处理模式。
`);
 add('Microsoft-Windows-PowerShell/Operational','Microsoft-Windows-PowerShell','commands','PowerShell 命令与脚本',`
4103|模块命令调用记录|记录命令调用及参数绑定信息。
4104|脚本块内容记录|PowerShell 记录脚本块文本，长脚本可能拆为多条。
4100|PowerShell 执行错误|PowerShell 报告执行错误，失败命令和错误原因见来源字段。
`);
 add('Microsoft-Windows-PowerShell/Operational','Microsoft-Windows-PowerShell','host','PowerShell 宿主生命周期',`
40961|PowerShell 控制台启动|PowerShell 控制台正在启动。
40962|PowerShell 控制台就绪|PowerShell 控制台已准备接收输入。
53504|PowerShell IPC 监听启动|PowerShell 在本机进程中启动 IPC 监听线程。
`);
 add('Microsoft-Windows-WMI-Activity/Operational','Microsoft-Windows-WMI-Activity','operations','WMI 提供程序与调用',`
5857|WMI 提供程序启动|WMI 提供程序启动，宿主进程、组件路径和结果由来源字段给出。
5858|WMI 客户端调用失败|WMI 记录客户端操作失败；应查看客户端进程、操作和结果码，不能把日志写入进程当成调用方。
`);
 add('Microsoft-Windows-TaskScheduler/Operational','Microsoft-Windows-TaskScheduler','execution','任务触发与执行过程',`
100|任务实例开始|任务计划程序启动了一个任务实例。
102|任务实例完成|任务计划程序报告实例完成；业务操作是否成功仍需结合动作返回码与应用日志。
107|计划触发任务|任务由计划调度触发，任务名称和实例标识见来源字段。
110|用户触发任务|任务计划程序记录由用户请求触发任务；这不一定是用户亲自点击界面。
114|错过计划后补启动|任务错过原定时间，计划程序依据“错过后尽快启动”的配置补启动实例；不是最终启动失败。
129|任务执行进程启动|任务计划程序启动了任务对应的执行进程。
200|任务动作开始|任务实例中的一个动作开始执行。
201|任务动作结束|任务动作执行结束；返回码可能表示失败，不能只凭结束事件认定成功。
322|已有实例导致未启动|已有任务实例运行，本次触发未启动另一个实例；这是调度决定。
325|任务实例排队|任务实例已进入队列。
328|任务实例停止|任务计划程序停止了任务实例，停止原因见来源说明。
`);
 add('Microsoft-Windows-TaskScheduler/Operational','Microsoft-Windows-TaskScheduler','configuration','任务登记与配置变更',`
140|任务配置更新|任务计划程序记录任务定义更新；具体哪个字段变化需与配置快照对照。
`);
 add('Microsoft-Windows-AppXDeploymentServer/Operational','Microsoft-Windows-AppXDeployment-Server','deployment','应用包部署',`
404|应用包部署失败|应用包部署操作失败；查看包名、调用程序和错误码。
603|应用包部署开始|开始应用包部署阶段，例如暂存包。
613|应用包部署性能摘要|记录部署阶段耗时和状态；摘要可能对应失败。
`);
 add('Microsoft-Windows-AppXDeploymentServer/Operational','Microsoft-Windows-AppXDeployment-Server','service','应用包部署服务状态',`
939|部署服务正在启动|AppX 部署服务进入正在启动状态。
940|部署服务正在运行|AppX 部署服务进入运行状态。
941|部署服务正在停止|AppX 部署服务进入正在停止状态。
942|部署服务已停止|AppX 部署服务进入停止状态。
`);
 add('Microsoft-Windows-Windows Defender/Operational','Microsoft-Windows-Windows Defender','health','Defender 健康与情报',`
1150|Defender 客户端健康|Defender 报告客户端处于健康运行状态。
1151|Defender 健康报告|记录引擎、情报版本、防护开关与扫描历史。
2010|Defender 云情报获取|Defender 使用云保护获取补充安全情报；不单独表示发现恶意软件。
`);
 add('Microsoft-Windows-WindowsUpdateClient/Operational','Microsoft-Windows-WindowsUpdateClient','discovery','更新检查',`
26|更新检查完成|Windows 更新完成检查并报告找到的更新数量。
`);
 add('Application','NVWMI','nvwmi','NVIDIA WMI 服务状态',`
3|NVIDIA WMI 服务进度|NVWMI 报告服务检查点和状态进度；具体状态码保留，不猜测硬件故障。
`);
 add('Application','VSS','vss','卷影复制服务状态',`
8224|卷影复制服务空闲停止|VSS 因空闲超时关闭服务；不能仅凭此条认定备份失败。
`);
 add('Application','Windows Error Reporting','wer','Windows 错误报告',`
1001|错误报告登记|Windows 错误报告登记了故障或诊断报告，问题类别与签名见原始字段；不能仅凭事件编号定位故障组件。
`);
 add('Application','Microsoft-Windows-Security-SPP','licensing','软件保护与授权维护',`
16384|软件保护服务重启已安排|软件保护组件安排下次服务重启，来源原因可能为规则引擎。
16394|软件保护离线迁移完成|软件保护组件报告离线迁移完成。
`);
 add('Application','Microsoft-Windows-Defrag','optimization','存储优化作业',`
258|存储优化完成|存储优化程序报告作业完成，具体卷和优化类型见来源说明。
264|存储优化未完成|存储优化程序报告未能完成；可能包含取消等原因。
`);
 add('System','Microsoft-Windows-Kernel-General','registry-maintenance','系统注册表维护',`
11|注册表事务初始化|系统报告注册表配置单元事务初始化信息，结果和路径见来源说明。
15|注册表配置单元整理|系统报告注册表配置单元处理信息；具体操作由来源正文说明。
`);
 add('System','Microsoft-Windows-Time-Service','time-source','系统时间同步',`
37|时间源数据有效|时间服务正在接收有效时间源数据。
`);
 function find(row){const d=row.data||{};return families.find(f=>row.source==='pcscope.eventlog.'+f.channel.toLowerCase()&&d.provider===f.provider&&f.events[d.event_id]);}
 function definition(row){return find(row)?.events[row.data?.event_id];}
 return {families,find,definition};
})();
if(typeof module!=='undefined')module.exports=PCScopeEventSemantics;
