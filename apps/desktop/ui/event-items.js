'use strict';
// Shared by the plugin source catalog, generated collection catalog and indexed history.
const PCScopeEventItems=(()=>{
 const semantics=typeof module!=='undefined'?require('./event-semantics.js'):PCScopeEventSemantics;
 const security=[
  ['sessions','登录、登出与会话',[[4624,'identity.logon','登录'],[4625,'identity.logon_failed','登录失败'],[4634,'identity.logoff','注销'],[4647,'identity.logoff_requested','请求注销'],[4800,'identity.session_locked','会话锁定'],[4801,'identity.session_unlocked','会话解锁'],[4778,'identity.session_reconnected','会话重连'],[4779,'identity.session_disconnected','会话断开']]],
  ['accounts','账户管理',[[4720,'identity.account_created','账户建立'],[4722,'identity.account_enabled','账户启用'],[4725,'identity.account_disabled','账户禁用'],[4726,'identity.account_deleted','账户删除'],[4738,'identity.account_changed','账户变更'],[4740,'identity.account_locked','账户锁定'],[4767,'identity.account_unlocked','账户解锁'],[4781,'identity.account_renamed','账户重命名']]],
  ['groups','用户组成员变更',[[4732,'identity.group_member_added','组成员加入'],[4733,'identity.group_member_removed','组成员移除']]],
  ['privileges','特殊权限分配',[[4672,'identity.special_privileges','特殊权限分配']]],
  ['processes','进程生命周期',[[4688,'execution.process_started','进程启动'],[4689,'execution.process_stopped','进程退出']]],
  ['files','文件访问与权限',[[4663,'files.access_audit','对象访问审计'],[4670,'files.permissions_audit','对象权限变更审计']]],
  ['registry','注册表操作',[[4657,'registry.value_operation','注册表值操作']]],
  ['services','服务安装',[[4697,'startup.service_installed','服务安装']]],
  ['tasks','计划任务变更',[[4698,'startup.task_created','任务创建'],[4699,'startup.task_deleted','任务删除'],[4702,'startup.task_updated','任务更新']]],
  ['audit-policy','审计策略变更',[[4719,'policy.audit_changed','审计策略变更']]],
  ['network','网络连接与共享访问',[[5140,'network.share_access','共享访问'],[5156,'network.connection_allowed','连接允许'],[5157,'network.connection_blocked','连接阻止']]],
 ];
 const sysmon=[
  ['processes','进程生命周期',[[1,'execution.process_started','进程启动'],[5,'execution.process_stopped','进程退出']]],
  ['process-access','进程访问与异常活动',[[8,'execution.remote_thread','远程线程创建'],[10,'execution.process_access','进程访问'],[25,'execution.process_tampering','进程映像异常']]],
  ['network','网络连接',[[3,'network.connection_observed','网络连接']]],
  ['dns','DNS 查询',[[22,'network.dns_query','DNS 查询']]],
  ['files','文件操作',[[2,'files.creation_time_changed','文件创建时间修改'],[11,'files.created','文件创建或覆盖'],[15,'files.stream_created','文件流创建'],[23,'files.deleted_archived','文件删除并归档'],[26,'files.deleted','文件删除'],[27,'files.executable_blocked','可执行文件创建被阻止'],[28,'files.shredding_blocked','文件粉碎被阻止'],[29,'files.executable_detected','可执行文件创建']]],
  ['registry','注册表操作',[[12,'registry.object_changed','注册表对象创建或删除'],[13,'registry.value_set','注册表值设置'],[14,'registry.renamed','注册表重命名']]],
  ['modules','模块与驱动加载',[[6,'execution.driver_loaded','驱动加载'],[7,'execution.module_loaded','模块加载']]],
  ['wmi','WMI 持久订阅',[[19,'execution.wmi_filter','WMI 筛选器活动'],[20,'execution.wmi_consumer','WMI 消费者活动'],[21,'execution.wmi_binding','WMI 绑定活动']]],
  ['pipes','命名管道',[[17,'execution.pipe_created','管道创建'],[18,'execution.pipe_connected','管道连接']]],
  ['raw-disk','原始磁盘读取',[[9,'storage.raw_read','原始磁盘读取']]],
  ['clipboard','剪贴板活动',[[24,'execution.clipboard_changed','剪贴板变化']]],
  ['monitor','Sysmon 服务与配置',[[4,'sysmon.service_changed','Sysmon 服务状态变化'],[16,'sysmon.config_changed','Sysmon 配置变化'],[255,'sysmon.error','Sysmon 内部错误']]],
 ];
 function expand(row,channel){
  const group=channel==='Security'?'Windows 安全审计':channel.includes('Sysmon')?'Sysmon':row.name;
  const rules=[];
  const add=(suffix,leaf,events,provider)=>rules.push({suffix,leaf,event_ids:events.map(e=>e[0]),actions:Object.fromEntries(events.map(e=>[e[0],{id:e[1],name:e[2]}])),provider});
  if(channel==='Security'){
   for(const [id,name,events] of security)add(id,name,events,'Microsoft-Windows-Security-Auditing');
   add('log-clear','审计日志清除',[[1102,'coverage.security_log_cleared','安全审计日志清除']],'Microsoft-Windows-Eventlog');
  }else if(channel==='Microsoft-Windows-Sysmon/Operational'){
   for(const [id,name,events] of sysmon)add(id,name,events,'Microsoft-Windows-Sysmon');
  }else if(channel==='System'){
   rules.push({suffix:'services',leaf:'服务运行事件',provider:'Service Control Manager'});
   rules.push({suffix:'lifecycle',leaf:'系统启动与关闭',provider:'Microsoft-Windows-Kernel-General',event_ids:[12,13]});
   rules.push({suffix:'power',leaf:'电源与异常重启',provider:'Microsoft-Windows-Kernel-Power'});
  }else if(channel==='Application'){
   rules.push({suffix:'installer',leaf:'软件安装事件',provider:'MsiInstaller'});
   rules.push({suffix:'errors',leaf:'应用程序错误',provider:'Application Error'});
   rules.push({suffix:'hangs',leaf:'应用程序无响应',provider:'Application Hang'});
  }
  for(const family of semantics.families.filter(f=>f.channel===channel))rules.push({suffix:family.suffix,leaf:family.leaf,provider:family.provider,event_ids:Object.keys(family.events).map(Number),actions:Object.fromEntries(Object.entries(family.events).map(([id,e])=>[id,{id:e.action,name:e.name,explanation:e.explanation}]))});
  rules.push({suffix:'status',leaf:'来源采集状态',status:true});
  rules.push({suffix:'other',leaf:rules.length>1?'其他事件':'运行事件'});
  return rules.map(rule=>({...row,id:row.source+'.'+rule.suffix,group,leaf:rule.leaf,name:group+' / '+rule.leaf,category:'eventlog',event_rule:rule,
   method:row.method+' 分类规则：'+(rule.status?'本采集器报告的缺口、恢复及断点状态。':rule.provider?'提供程序 '+rule.provider+(rule.event_ids?'；事件 ID '+rule.event_ids.join('、'):'；该提供程序的事件')+'。':rules.length>2?'未命中本来源已定义规则的事件，保留原始提供程序和事件编号。':'该通道的运行记录。')
  }));
 }
 return {expand};
})();
if(typeof module!=='undefined')module.exports=PCScopeEventItems;
