'use strict';
// One presentation model for timeline titles, readable details and field selection.
const PCScopePresentation=(()=>{
 let catalog=typeof module!=='undefined'?require('./collection-catalog.json'):[];
 const configure=rows=>{catalog=rows;};
 const security='Microsoft-Windows-Security-Auditing';
 const present=v=>v!==undefined&&v!==null&&v!==''&&v!=='-';
 const norm=s=>String(s).replace(/[_\s-]/g,'').toLowerCase();
 const text=v=>v==='******'||v?.redacted||v?.sensitive||v?.display==='******'?'已隐藏':typeof v==='object'?JSON.stringify(v):String(v);
 const objectKinds={keys:'系统密钥',credentials:'保存的登录信息',sessions:'用户账户',identity:'账户或用户组',process:'运行程序',files:'文件',registry:'注册表设置',tasks:'计划任务',services:'系统服务',software:'软件',network:'网络活动',policy:'系统策略',wmi:'系统管理项目',security:'安全防护',script:'运行脚本',system:'系统活动'};
 function readableObject(value,type){
  if(!present(value))return objectKinds[type]||'系统活动';
  const label=text(value).trim();if(label==='已隐藏')return label;
  if(!label||/^(?:S-\d-\d+(?:-\d+)+|\{?[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}\}?)$/i.test(label)||/^%%|[\r\n]|[|]/.test(label))return objectKinds[type]||'系统活动';
  if(/^(?:HKLM|HKCU|HKCR|HKU|HKEY_|\\REGISTRY\\)/i.test(label))return '注册表设置';
  if(/[\\/]/.test(label)&&!/[<>]/.test(label)&&type!=='network')return label.split(/[\\/]/).filter(Boolean).pop()||objectKinds[type]||'系统活动';
  return label;
 }
 function get(o,...keys){for(const k of keys){const e=Object.entries(o||{}).find(([n,v])=>norm(n)===norm(k)&&present(v));if(e)return e[1];}}
 function definition(row){return catalog.find(c=>{const r=c.event_rule;if(c.source!==row.source||!r)return false;if(r.status)return row.kind?.startsWith('coverage.')&&row.kind!=='coverage.security_log_cleared';return (!r.provider||r.provider===row.data?.provider)&&(!r.event_ids||r.event_ids.includes(row.data?.event_id));});}
 const snapshots={'baseline.observed':'初始状态','object.first_seen':'新发现','object.changed':'属性变化','object.no_longer_observed':'不再发现'};
 const help={密钥类型:'用户密钥归属于用户；计算机密钥归属于本机。',密钥文件:'Windows 密码服务用于保存密钥的文件。',登录类型:'Windows 对交互登录、网络登录、服务登录等方式的分类。',登录会话:'用于关联同一登录会话的日志标识。',主访问令牌:'进程执行时使用的账户身份及权限信息。',观察时间:'PCScope 采集到该状态的时间。'};
 // Labels and source aliases stay together so selected fields keep their meaning.
 const templates={
  keys:[['密钥名称','KeyName'],['密钥类型','KeyType'],['密钥文件','KeyFilePath'],['算法','AlgorithmName'],['存储提供程序','ProviderName'],['操作原码','Operation']],
  credentials:[['凭据目标','TargetName','Resource'],['读取操作','ReadOperation'],['返回数量','CountOfCredentialsReturned'],['目标账户','TargetUserName']],
  sessions:[['目标账户','TargetUserName','AccountName'],['登录类型','LogonType'],['来源地址','IpAddress','SourceNetworkAddress'],['来源端口','IpPort'],['工作站','WorkstationName'],['登录会话','TargetLogonId','SubjectLogonId'],['认证方式','AuthenticationPackageName'],['失败原因','FailureReason'],['状态码','Status'],['子状态码','SubStatus']],
  identity:[['目标账户或组','TargetUserName','GroupName'],['成员','MemberName','MemberSid'],['权限','PrivilegeList'],['登录会话','SubjectLogonId'],['目标进程','TargetProcessName'],['目标进程编号','TargetProcessId']],
  process:[['程序路径','NewProcessName','Image','ProcessName'],['进程编号','NewProcessId','ProcessId'],['父进程','ParentProcessName','ParentImage'],['父进程编号','CreatorProcessId','ParentProcessId'],['命令行','CommandLine'],['模块或驱动','ImageLoaded'],['目标程序','TargetImage'],['目标进程编号','TargetProcessId']],
  files:[['路径','TargetFilename','ObjectName','Path'],['操作','OperationType','AccessList','AccessMask'],['对象类型','ObjectType'],['原安全描述符','OldSd'],['新安全描述符','NewSd'],['文件摘要','Hashes']],
  registry:[['注册表对象','TargetObject','ObjectName'],['值名称','ObjectValueName'],['变化前','OldValue'],['变化后','NewValue','Details'],['重命名后','NewName']],
  tasks:[['任务名称','TaskName'],['任务实例','TaskInstanceId','InstanceId'],['执行动作','ActionName'],['运行账户','UserContext'],['执行进程编号','EnginePID','ProcessID'],['返回码','ResultCode']],
  services:[['服务名称','ServiceName','param1'],['服务状态','State','param2'],['执行程序','ServiceFileName','ImagePath'],['运行账户','ServiceAccount','AccountName'],['启动方式','ServiceStartType','StartType']],
  software:[['软件或包','ProductName','PackageFullName','PackageName'],['版本','ProductVersion','Version'],['操作','DeploymentOperation','Operation'],['错误码','ErrorCode','Error'],['路径','Path']],
  network:[['程序','Application','Image'],['来源地址','SourceAddress','SourceIp'],['来源端口','SourcePort'],['目标地址','DestAddress','DestinationIp','DestinationHostname'],['目标端口','DestPort','DestinationPort'],['协议','Protocol'],['方向','Direction'],['查询名称','QueryName'],['DNS 结果','QueryResults'],['DNS 状态码','QueryStatus'],['共享名称','ShareName']],
  policy:[['策略对象','GPOList','GPOLIst','GPOName'],['排除原因','Reason'],['处理模式','ProcessingMode'],['会话','SessionId'],['耗时（毫秒）','ProcessingTimeInMilliseconds','ElapsedTimeInMilliseconds'],['错误码','ErrorCode']],
  wmi:[['对象服务器','ObjectServer'],['对象类型','ObjectType'],['对象','ObjectName'],['操作','Operation','AdditionalInfo'],['命名空间','NamespaceName'],['提供程序','ProviderName'],['客户端进程编号','ClientProcessId'],['结果码','ResultCode']],
  security:[['威胁名称','ThreatName','Threat Name'],['检测对象','Path','DetectionPath'],['处置动作','ActionName','Action Name'],['错误码','ErrorCode','Error Code'],['引擎版本','EngineVersion']],
  script:[['脚本路径','Path','ScriptName'],['命令','CommandName'],['脚本块标识','ScriptBlockId'],['分段编号','MessageNumber'],['总段数','MessageTotal']],
  system:[['涉及对象','ObjectName','DeviceName','Name'],['变化前时间','PreviousTime'],['变化后时间','NewTime'],['原因','Reason'],['状态','State','Status'],['结果码','ResultCode','ErrorCode']],
 };
 function family(row,def){const s=row.source||'',p=row.data?.provider||'',id=row.data?.event_id,suffix=def?.event_rule?.suffix||'';
  if(p===security){if([5058,5059,5061].includes(id))return 'keys';if([5379,5382,4797,4648].includes(id))return 'credentials';if(id===4662)return 'wmi';if(/session/.test(suffix))return 'sessions';if(/accounts|groups|privileges|membership|tokens/.test(suffix))return 'identity';if(/process/.test(suffix))return 'process';if(/files/.test(suffix))return 'files';if(/registry/.test(suffix))return 'registry';if(/tasks/.test(suffix))return 'tasks';if(/services/.test(suffix))return 'services';if(/network/.test(suffix))return 'network';}
  if(/TaskScheduler/.test(p))return 'tasks';if(/GroupPolicy/.test(p))return 'policy';if(/WMI-Activity/.test(p))return 'wmi';if(/PowerShell/.test(p))return 'script';if(/Defender/.test(p))return 'security';if(/AppX|MsiInstaller|WindowsUpdate/.test(p))return 'software';if(p==='Service Control Manager')return 'services';
  if(/Sysmon/.test(p)){if(/files/.test(suffix))return 'files';if(/registry/.test(suffix))return 'registry';if(/network|dns/.test(suffix))return 'network';if(suffix==='wmi')return 'wmi';return 'process';}
  return 'system';
 }
 function operation(d,f){
  const verified={5058:{'%%2457':'删除密钥文件'},5059:{'%%2464':'导出持久化密钥'},5061:{'%%2480':'打开密钥'}};
  if(verified[d.event_id]?.[f.Operation])return verified[d.event_id][f.Operation];
  const raw=String(d.message||'').match(/(?:^|[\r\n])[\t ]*(?:Operation|操作)[\t ]*[:：][\t ]*([^\r\n]+)/i)?.[1]?.trim();
  const names={'Delete key file.':'删除密钥文件','Read persisted key from file.':'读取密钥文件','Write persisted key to file.':'写入密钥文件','Open Key.':'打开密钥','Create Key.':'创建密钥','Delete Key.':'删除密钥','Import Key.':'导入密钥','Export Key.':'导出密钥','Export of persistent cryptographic key.':'导出持久化密钥','Import of persistent cryptographic key.':'导入持久化密钥','Sign hash.':'对摘要签名','Decrypt.':'解密','Encrypt.':'加密'};
  return names[raw]||null;
 }
 function outcome(row){const d=row.data||{},f=d.fields||{},id=d.event_id,p=d.provider;
  if(isWmi(row))return wmiResult(row);
  if(row.source==='pcscope.eventlog.security'&&p===security&&([5058,5059,5061,5379,5382].includes(id))){const code=f.ReturnCode;if(!present(code))return {label:'未记录',tone:'neutral'};if(text(code)==='已隐藏')return {label:'已隐藏',tone:'neutral'};if(!/^(0x[\da-f]+|\d+)$/i.test(String(code)))return {label:'未解析',tone:'neutral',code};return {label:BigInt(code)===0n?'成功':'失败',tone:BigInt(code)===0n?'success':'failure',code};}
  if(p===security&&id===4624)return {label:'成功',tone:'success'};
  if(p===security&&id===4625)return {label:'失败',tone:'failure',code:f.Status};
  if(p===security&&id===5157)return {label:'已阻止',tone:'failure'};
  if(p===security&&id===5156)return {label:'已允许',tone:'success'};
  if(p==='Microsoft-Windows-TaskScheduler'&&id===201)return {label:'已结束',tone:'neutral',code:f.ResultCode};
  return null;
 }
 function fieldValue(label,value){if(text(value)==='已隐藏')return '已隐藏';if(label==='密钥类型'&&value==='%%2500')return '用户密钥';if(label==='登录类型')return ({2:'交互登录',3:'网络登录',4:'批处理登录',5:'服务登录',7:'解锁',8:'网络明文登录',9:'新凭据登录',10:'远程交互登录',11:'缓存交互登录'})[value]||'未解析 · '+value;if(typeof value==='string'&&/^%%/.test(value))return '未解析 · '+value;return value;}
 const isWmi=row=>row.source==='pcscope.eventlog.microsoft-windows-wmi-activity/operational'&&row.data?.provider==='Microsoft-Windows-WMI-Activity'&&[5857,5858].includes(row.data?.event_id);
 function wmiField(row,key){const f=row.data.fields||{},prefix=row.data.event_id===5857?'Operation_StartedOperational':'Operation_ClientFailure';return get(f,key,prefix+'/'+key)??get(f[prefix],key);}
 function wmiResult(row){
  const code=wmiField(row,row.data.event_id===5857?'Code':'ResultCode'),failure=row.data.event_id===5858;
  if(!present(code))return {label:failure?'失败':'未记录',tone:failure?'failure':'neutral',description:'来源未提供结果码'};
  if(text(code)==='已隐藏')return {label:'已隐藏',tone:'neutral',description:'结果码已隐藏'};
  if(!/^(?:0x[\da-f]+|-?\d+)$/i.test(String(code)))return {label:failure?'失败':'未解析',tone:failure?'failure':'neutral',code,description:'结果码未解析'};
  const rawCode=BigInt(code);if(rawCode< -2147483648n||rawCode>4294967295n)return {label:'未解析',tone:'neutral',code,description:'结果码超出 32 位范围'};
  const n=BigInt.asUintN(32,rawCode),hex=n.toString(16);
  const reasons={'80041001':'调用失败，来源未给出具体原因','80041002':'找不到请求的对象','80041003':'调用账户没有执行权限','8004100e':'找不到指定的管理命名空间','80041010':'指定的管理对象类无效','80041017':'查询语句无效','80041032':'调用已取消'};
  if(hex==='80041032')return {label:'已取消',tone:'neutral',code,description:reasons[hex]};
  if(n===0n&&!failure)return {label:'成功',tone:'success',code,description:'管理组件已启动'};
  return {label:failure||n>=0x80000000n?'失败':'已返回',tone:failure||n>=0x80000000n?'failure':'neutral',code,description:reasons[hex]||(failure&&n===0n?'来源报告调用失败，结果码为 0':'返回码尚无专门解释')};
 }
 function wmiModel(row){
  const startup=row.data.event_id===5857,read=k=>wmiField(row,k),result=wmiResult(row),technical=[];
  const classes={win32_service:'Windows 服务',win32_process:'进程',win32_operatingsystem:'操作系统',win32_computersystem:'计算机系统',win32_processor:'处理器',win32_physicalmemory:'物理内存',win32_logicaldisk:'逻辑磁盘',win32_diskdrive:'磁盘设备',win32_networkadapter:'网卡',win32_printer:'打印机',msft_printer:'打印机',msft_printerport:'打印端口',msft_scheduledtask:'计划任务',win32_startupcommand:'启动项',win32_useraccount:'用户账户',win32_bios:'BIOS 信息'};
  const providers={msft_printer:'打印机管理组件',scheduledtaskprov:'计划任务管理组件',cimwin32:'Windows 系统信息组件'};
  let action,object,nature,namespace,method,rawOperation=read('Operation');
  if(startup){action='启动系统管理组件';const provider=read('ProviderName');object=present(provider)?providers[String(provider).toLowerCase()]||'系统管理组件':'未记录组件名称';nature='启动系统信息提供组件';}
  else {
   const raw=typeof rawOperation==='string'&&text(rawOperation)!=='已隐藏'?rawOperation:'';
   const match=raw.match(/^(?:Start\s+)?IWbemServices::([A-Za-z]+)\s*-\s*([^:]+?)\s*:\s*([\s\S]*)$/i);
   method=match?.[1];namespace=match?.[2];const body=match?.[3]||'',base=method?.replace(/Async$/i,'').toLowerCase();
   const names={execquery:['查询对象列表','读取信息'],createinstanceenum:['查询对象列表','读取信息'],getobject:['读取对象信息','读取信息'],execnotificationquery:['订阅对象变化','监听变化'],execmethod:['调用管理方法','执行管理方法'],putinstance:['创建或更新管理对象','写入管理对象'],deleteinstance:['删除管理对象','删除管理对象'],createclassenum:['查询对象类型','读取类型定义'],getclass:['读取对象类型','读取类型定义'],putclass:['创建或更新对象类型','写入类型定义'],deleteclass:['删除对象类型','删除类型定义']};
   [action,nature]=names[base]||['系统管理操作','未解析操作方式'];
   // Only parse the subject of a recognized query/object API. Never search arbitrary
   // message strings for a class name, and never derive a caller from the writer PID.
   let className;
   if(['execquery','execnotificationquery'].includes(base))className=body.match(/^\s*SELECT\s+(?:\*|[A-Za-z_][\w.]*(?:\s*,\s*[A-Za-z_][\w.]*)*)\s+FROM\s+([A-Za-z_][\w]*)\b/i)?.[1];
   else if(['createinstanceenum','getobject','execmethod','putinstance','deleteinstance','createclassenum','getclass','putclass','deleteclass'].includes(base))className=body.match(/^\s*([A-Za-z_][\w]*)(?=\s|[.:=]|$)/)?.[1];
   object=className?(classes[className.toLowerCase()]||'其他系统管理项目'):text(rawOperation)==='已隐藏'?'操作内容已隐藏':'未记录可识别对象';
   if(className)technical.push(['管理对象类',className]);
  }
  const main=[];if(startup){if(present(read('HostProcess')))main.push(['宿主程序',text(read('HostProcess'))]);if(present(read('ProcessID')))main.push(['宿主进程 ID',text(read('ProcessID'))]);}
  else {if(present(read('User')))main.push(['操作账户',text(read('User'))]);if(present(read('ClientProcessId')))main.push(['发起进程','未关联程序 · PID '+text(read('ClientProcessId'))]);if(present(read('ClientMachine')))main.push(['发起计算机',text(read('ClientMachine'))]);}
  const selected=[['行为性质',nature],['结果说明',result.description]];if(present(result.code))selected.push(['结果码',text(result.code)]);
  for(const [label,value] of [['接口方法',method],['命名空间',namespace],['原始操作',rawOperation],['组件名称',read('ProviderName')],['组件文件',read('ProviderPath')]])if(present(value))technical.push([label,text(value)]);
  const groups=[];if(main.length)groups.push({name:startup?'运行宿主':'发起信息',fields:main});groups.push({name:'操作说明',fields:selected});
  return {action,object,title:action+' · '+object,result,groups,technical,summary:main.filter(([k])=>k!=='发起计算机').map(([k,v])=>k+'：'+v).concat(result.description).join(' · '),unknown:false,type:'wmi',help};
 }
 function model(row){const d=row.data||{},f=d.fields||{},def=definition(row),type=family(row,def),event=row.source?.startsWith('pcscope.eventlog.')&&(!row.kind?.startsWith('coverage.')||row.kind==='coverage.security_log_cleared');
  if(!event)return null;
  if(isWmi(row))return wmiModel(row);
  let action=def?.event_rule?.actions?.[d.event_id]?.name;
  const unknown=!action;action=action||'未识别事件';if(type==='keys')action=operation(d,f)||action;
  const objectKeys={keys:['KeyName','KeyFilePath'],credentials:['TargetName','Resource','TargetUserName'],sessions:['TargetUserName'],identity:['TargetUserName','MemberName'],process:['NewProcessName','Image','ProcessName'],files:['TargetFilename','ObjectName'],registry:['TargetObject','ObjectName'],tasks:['TaskName'],services:['ServiceName','param1'],software:['ProductName','PackageFullName','PackageName'],network:['QueryName','DestinationIp','DestAddress','ShareName'],policy:['GPOName'],wmi:['ObjectName','NamespaceName'],security:['ThreatName','Threat Name','Path'],script:['Path','ScriptName','CommandName'],system:['ObjectName','Name']};
  const object=readableObject(get(f,...objectKeys[type]),type);
  const isCreation=d.provider===security&&d.event_id===4688;
  const pid=get(f,'ClientProcessId',...(isCreation?['NewProcessId']:['CallerProcessId','ProcessId','ProcessID']));
  const program=get(f,...(isCreation?['NewProcessName']:['ClientProcessName','CallerProcessName','ProcessName','Image']));
  const actor=get(f,'SubjectUserName','User','UserContext','UserName');
  const result=outcome(row),groups=[];
  const main=[];if(present(actor))main.push(['操作账户',text(actor)]);
  if(present(program)||present(pid))main.push([isCreation?'启动程序':'程序',present(program)?readableObject(program,'process')+(present(pid)?' · PID '+text(pid):''):'未关联 · PID '+text(pid)]);
  if(present(f.ClientCreationTime))main.push(['进程建立时间',f.ClientCreationTime]);
  if(result?.code!==undefined)main.push(['返回码',text(result.code)]);
  if(main.length)groups.push({name:'执行信息',fields:main});
  const selected=(templates[type]||[]).flatMap(([label,...keys])=>{const v=get(f,...keys);return present(v)?[[label,fieldValue(label,v)]]:[];});
  if(selected.length)groups.push({name:'操作属性',fields:selected});
  const summary=main.filter(([k])=>['操作账户','程序','启动程序'].includes(k)).map(([k,v])=>k+'：'+v);
  if(row.source==='pcscope.eventlog.security'&&d.provider===security&&d.event_id===4907&&def?.event_rule?.suffix==='files-audit-settings')summary.unshift(def.event_rule.actions[4907].explanation);
  const extra=selected.filter(([k])=>!['密钥名称','任务名称','程序路径','脚本内容','计划任务定义','操作原码'].includes(k)&&!/[码]|编号|标识|实例|登录会话|原安全描述符|新安全描述符|摘要/.test(k)).slice(0,2).map(([k,v])=>k+'：'+readableObject(v,type));
  return {action,object,title:action+' · '+object,result,groups,summary:summary.concat(extra).join(' · ')||'展开查看记录',unknown,type,help};
 }
 function objectGroups(row){
  const raw=row.data||{},d={...raw,...Object.fromEntries(Object.entries(raw.values||{}).map(([k,v])=>[k,v?.value??v]))};
  const profiles=[
   ['身份与权限',/identity/,['Name','FullName','Enabled','Disabled','PrincipalSource','Members','Rights','LastLogon','PasswordLastSet']],
   ['运行与启动配置',/startup|execution/,['Name','DisplayName','State','Status','StartMode','StartAccount','Path','Execute','UserId','RunLevel','LastRunTime','NextRunTime','LastTaskResult']],
   ['软件登记',/software|apps|applications/,['DisplayName','DisplayVersion','Publisher','InstallDate','InstallLocation','Version','PackageFullName']],
   ['网络配置',/network/,['Name','InterfaceAlias','Status','Address','IPAddress','PrefixLength','MacAddress','LinkSpeed','RemoteAddress','RemotePort']],
   ['扩展登记',/shell|environment/,['Name','DisplayName','Description','Path','RegistryPath','Enabled','CLSID','InprocServer32','LocalServer32']],
   ['设备与系统状态',/system|hardware|storage|power|virtualization|desktop|security|policy|files/,['Name','Caption','State','Status','Enabled','Manufacturer','Model','Version','Path','Capacity','FileSystem','HypervisorPresent','FirmwareVirtualization']]
  ];const profile=profiles.find(([,re])=>re.test(row.source||''));if(!profile)return [];
  const selected=profile[2].flatMap(k=>{const v=get(d,k);return present(v)?[[k,v]]:[];});return selected.length?[{name:profile[0],fields:selected}]:[];
 }
 return {configure,model,outcome,help,snapshots,text,objectGroups};
})();
if(typeof module!=='undefined')module.exports=PCScopePresentation;
