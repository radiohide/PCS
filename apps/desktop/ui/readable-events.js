'use strict';
const PCScopeReadableEvents=(()=>{
  const actions={"software.update_history":{"name":"Windows 更新操作记录","type_id":"software.system_update"},"identity.logon":{"name":"登录","type_id":"runtime.logon_session"},"identity.logon_failed":{"name":"登录失败","type_id":"account.user"},"identity.logoff":{"name":"注销","type_id":"runtime.logon_session"},"identity.logoff_requested":{"name":"请求注销","type_id":"runtime.logon_session"},"identity.session_locked":{"name":"会话锁定","type_id":"runtime.terminal_session"},"identity.session_unlocked":{"name":"会话解锁","type_id":"runtime.terminal_session"},"identity.session_reconnected":{"name":"会话重连","type_id":"runtime.terminal_session"},"identity.session_disconnected":{"name":"会话断开","type_id":"runtime.terminal_session"},"identity.account_created":{"name":"账户建立","type_id":"account.user"},"identity.account_enabled":{"name":"账户启用","type_id":"account.user"},"identity.account_disabled":{"name":"账户禁用","type_id":"account.user"},"identity.account_deleted":{"name":"账户删除","type_id":"account.user"},"identity.account_changed":{"name":"账户变更","type_id":"account.user"},"identity.account_locked":{"name":"账户锁定","type_id":"account.user"},"identity.account_unlocked":{"name":"账户解锁","type_id":"account.user"},"identity.account_renamed":{"name":"账户重命名","type_id":"account.user"},"identity.group_member_added":{"name":"组成员加入","type_id":"account.group_membership"},"identity.group_member_removed":{"name":"组成员移除","type_id":"account.group_membership"},"identity.special_privileges":{"name":"登录获得特殊权限","type_id":"runtime.logon_session"},"execution.process_started":{"name":"进程启动","type_id":"runtime.process"},"execution.process_stopped":{"name":"进程退出","type_id":"runtime.process"},"execution.remote_thread":{"name":"远程线程创建","type_id":"runtime.process"},"execution.process_access":{"name":"进程访问","type_id":"runtime.process"},"execution.process_tampering":{"name":"进程映像异常","type_id":"runtime.process"},"execution.driver_loaded":{"name":"驱动加载","type_id":"hardware.driver_service"},"execution.module_loaded":{"name":"模块加载","type_id":"runtime.module_instance"},"execution.wmi_filter":{"name":"WMI 筛选器活动","type_id":"startup.wmi_filter"},"execution.wmi_consumer":{"name":"WMI 消费者活动","type_id":"startup.wmi_consumer"},"execution.wmi_binding":{"name":"WMI 绑定活动","type_id":"startup.wmi_binding"},"execution.pipe_created":{"name":"管道创建","type_id":"runtime.process"},"execution.pipe_connected":{"name":"管道连接","type_id":"runtime.process"},"execution.clipboard_changed":{"name":"剪贴板变化","type_id":"desktop.shell_configuration"},"files.access_audit":{"name":"对象访问审计","type_id":"file.watched_file"},"files.permissions_audit":{"name":"对象权限变更审计","type_id":"file.acl_entry"},"files.creation_time_changed":{"name":"文件创建时间修改","type_id":"file.watched_file"},"files.created":{"name":"文件创建或覆盖","type_id":"file.watched_file"},"files.stream_created":{"name":"文件流创建","type_id":"file.alternate_data_stream"},"files.deleted_archived":{"name":"文件删除并归档","type_id":"file.watched_file"},"files.deleted":{"name":"文件删除","type_id":"file.watched_file"},"files.executable_blocked":{"name":"可执行文件创建被阻止","type_id":"file.watched_file"},"files.shredding_blocked":{"name":"文件粉碎被阻止","type_id":"file.watched_file"},"files.executable_detected":{"name":"可执行文件创建","type_id":"file.watched_file"},"files.usn_changed":{"name":"文件变更记录","type_id":"file.watched_file"},"registry.value_operation":{"name":"注册表值操作","type_id":"policy.os_configuration"},"registry.object_changed":{"name":"注册表对象创建或删除","type_id":"policy.os_configuration"},"registry.value_set":{"name":"注册表值设置","type_id":"policy.os_configuration"},"registry.renamed":{"name":"注册表重命名","type_id":"policy.os_configuration"},"policy.audit_changed":{"name":"审计策略变更","type_id":"policy.os_configuration"},"startup.service_installed":{"name":"服务安装","type_id":"startup.service"},"startup.task_created":{"name":"任务创建","type_id":"startup.scheduled_task"},"startup.task_deleted":{"name":"任务删除","type_id":"startup.scheduled_task"},"startup.task_updated":{"name":"任务更新","type_id":"startup.scheduled_task"},"startup.task_started":{"name":"任务开始执行","type_id":"startup.scheduled_task"},"startup.task_ended":{"name":"任务执行结束","type_id":"startup.scheduled_task"},"network.share_access":{"name":"共享访问","type_id":"network.smb_share"},"network.connection_allowed":{"name":"连接允许","type_id":"network.endpoint"},"network.connection_blocked":{"name":"连接阻止","type_id":"network.endpoint"},"network.connection_observed":{"name":"网络连接","type_id":"network.endpoint"},"network.dns_query":{"name":"DNS 查询","type_id":"network.dns_cache_entry"},"storage.raw_read":{"name":"原始磁盘读取","type_id":"hardware.disk"},"sysmon.service_changed":{"name":"Sysmon 服务状态变化","type_id":"policy.os_configuration"},"sysmon.config_changed":{"name":"Sysmon 配置变化","type_id":"policy.os_configuration"},"sysmon.error":{"name":"Sysmon 内部错误","type_id":"policy.os_configuration"},"observed.configuration_changed":{"name":"观察到配置变化","type_id":null},"observed.state_changed":{"name":"观察到状态变化","type_id":null},"observed.first_seen":{"name":"首次观察到","type_id":null},"observed.no_longer_observed":{"name":"本次未发现","type_id":null},"coverage.gap":{"name":"采集缺口","type_id":null},"coverage.recovered":{"name":"采集恢复","type_id":null},"coverage.usn_gap":{"name":"USN 断点缺口","type_id":null},"coverage.security_log_cleared":{"name":"安全审计日志清除","type_id":"policy.os_configuration"},"coverage.unclean_shutdown":{"name":"非正常退出","type_id":null},"collector.item_configured":{"name":"采集项配置变化","type_id":null}};
  const objects=typeof PCScopeObjectNames!=='undefined'?PCScopeObjectNames:require('./object-names.js');
  actions['startup.service_state_changed']={name:'服务运行状态变化',type_id:'startup.service'};
  const titles={'startup.task_started':'计划任务开始运行','startup.task_ended':'计划任务运行结束','identity.logon':'账户登录','identity.logoff':'账户退出登录','identity.special_privileges':'登录会话获得较高权限','observed.configuration_changed':'设置发生变化','observed.state_changed':'状态发生变化','observed.first_seen':'首次记录到此对象','observed.no_longer_observed':'本次检查未找到此对象'};
  const results={succeeded:'成功',success:'成功',failed:'失败',failure:'失败',unknown:'未取得操作结果',observed:'已记录',partial:'部分完成',cancelled:'已取消',started:'已开始',ended:'已结束',allowed:'已允许',blocked:'已阻止',requested:'已请求'};
  Object.assign(titles,{
    'startup.service_state_changed':'服务运行状态变化',
    'execution.process_started':'程序开始运行','execution.process_stopped':'程序退出','execution.remote_thread':'跨程序执行活动','execution.process_access':'程序访问其他程序','execution.process_tampering':'程序运行内容异常','execution.driver_loaded':'驱动已加载','execution.module_loaded':'程序加载了组件',
    'execution.wmi_filter':'系统自动执行条件的操作记录','execution.wmi_consumer':'系统自动执行内容的操作记录','execution.wmi_binding':'系统自动执行关联的操作记录','execution.pipe_created':'程序间通信通道建立','execution.pipe_connected':'程序间通信通道连接',
    'files.access_audit':'文件或系统对象被访问','files.permissions_audit':'访问权限的修改记录','files.stream_created':'文件附加内容被创建','files.usn_changed':'文件变化记录','files.executable_blocked':'程序文件的创建被阻止','files.executable_detected':'程序文件被创建',
    'registry.value_operation':'系统配置值的操作记录','registry.object_changed':'系统配置项被创建或删除','registry.value_set':'系统配置值被设置','registry.renamed':'系统配置项被重命名','policy.audit_changed':'系统活动记录规则有变化',
    'network.dns_query':'查询网络名称对应的地址','storage.raw_read':'程序直接读取磁盘','sysmon.service_changed':'系统活动监控组件的状态变化','sysmon.config_changed':'系统活动监控组件的设置变化','sysmon.error':'系统活动监控组件报告错误',
    'coverage.gap':'部分系统变化可能漏记','coverage.recovered':'采集已恢复','coverage.usn_gap':'部分文件变化记录缺失','coverage.security_log_cleared':'系统安全日志被清除','coverage.unclean_shutdown':'采集后台未正常退出','collector.item_configured':'采集设置已调整'
  });
  const readable=value=>typeof value==='string'&&value.trim()&&!/[|]|(?:machine:|S-1-\d-|pcscope\.)/i.test(value)&&value.length<180;
  const historicalMappingIssue=row=>row.context?.history_quality?.status==='legacy_mapping_ambiguous';
  function title(row){if(historicalMappingIssue(row))return '旧版采集记录需复核';if(row.action_id==='observed.first_seen')return firstSeen(row);if(row.action_id==='observed.no_longer_observed')return absence(row).title;if(['identity.logon','identity.logoff'].includes(row.action_id)&&Number(row.context?.logon_type)===5)return row.action_id==='identity.logon'?'系统服务登录':'系统服务结束登录';if(row.field_id==='module_base')return '模块加载位置发生变化';return titles[row.action_id]||actions[row.action_id]?.name||(readable(row.title)&&/[\u4e00-\u9fff]/.test(row.title)?row.title:'记录到系统活动');}
  function type(row){return row.context?.type_id||actions[row.action_id]?.type_id||'';}
  function typeLabel(row){
    if(historicalMappingIssue(row))return '历史采集记录';
    if(row.action_id?.startsWith('coverage.'))return '采集状态';
    if(row.action_id==='collector.item_configured')return '采集设置';
    if(row.context?.classification_status==='unverified')return '系统事件';
    const id=uncertainAssociation(row)?actions[row.action_id]?.type_id||'':type(row);
    const names={'runtime.process':'运行进程','runtime.module_instance':'进程加载模块','file.watched_file':'文件','file.watched_directory':'文件夹','hardware.driver_binding':'设备驱动关联'};
    if(id){const name=names[id]||objects.typeName({...row.context,type_id:id});return readable(name)&&/[\u4e00-\u9fff]/.test(name)&&!/[<>]/.test(name)?name:'系统项目';}
    return readable(row.context?.type_name)&&/[\u4e00-\u9fff]/.test(row.context.type_name)&&!/[<>]/.test(row.context.type_name)?row.context.type_name:'系统事件';
  }
  function absence(row){
    const result=objects.presentation.absence({type_id:uncertainAssociation(row)?'':type(row)});
    return {...result,impact:'检查时，'+result.impact.replace(/当前/g,'').replace(/^本次检查/,'')};
  }
  function firstSeen(row){return objects.presentation.firstSeen({type_id:uncertainAssociation(row)?'':type(row)},row.source_record?.snapshot_presence||{});}
  function subject(row){
    if(readable(row.context?.subject_label)){
      const c=row.context;
      if(c.type_id==='shell.com_registration'){
        const all=objects.presentation.facts(c),protectedName=['name','display_name'].some(id=>{
          const f=all.get(id);return f&&(['masked','unreadable','conflicting','evidence_pruned'].includes(f.status)||objects.presentation.hidden(f.value));
        });
        if(protectedName)return objects.name(c);
        const alias=objects.presentation.displayName(c,[],c.subject_label);if(alias)return alias;
      }
      return objects.shortName(c.subject_label);
    }
    // Update history names its own operation; it does not need an installed-update instance.
    if(row.action_id==='software.update_history'&&readable(row.source_record?.title))return row.source_record.title.trim();
    if(row.action_id?.startsWith('coverage.')||row.action_id==='collector.item_configured')return 'PCScope 采集监控';
    const c=row.context;if(c&&(c.display_name||(c.key_facts||[]).length))return objects.name(c);
    return row.instance_id?'未取得对象名称':'日志未提供可识别的对象';
  }
  function fieldLabel(row){const context={...row.context,type_id:type(row)};return ({module_base:'模块加载位置',sha256:'文件内容',dacl:'访问权限',dacl_sddl:'访问权限'})[row.field_id]||objects.presentation.label(context,row.field_id)||(readable(row.context?.field_label)&&/[\u4e00-\u9fff]/.test(row.context.field_label)?row.context.field_label:objects.fieldLabel(context,{field_id:row.field_id}));}
  function change(row){if(historicalMappingIssue(row)||!row.field_id)return '';const context={...row.context,type_id:type(row)},before=objects.valueLabel(row.before,row.field_id,context,row.before_status||'known'),after=objects.valueLabel(row.after,row.field_id,context,row.after_status||'known');if(row.action_id==='startup.service_state_changed'&&row.before==null)return fieldLabel(row)+'：'+after;if(before===after)return fieldLabel(row)+'：详细变化可展开查看';return fieldLabel(row)+'：'+before+' → '+after;}
  function impact(row){
    if(historicalMappingIssue(row))return '旧版混用了组件说明和文件路径，这条比较暂不能用于判断系统变化。';
    if(row.field_id&&(row.after_status&&row.after_status!=='known'||objects.presentation.hidden(row.after)))return '本次未取得变化后的属性值。';
    if(!uncertainAssociation(row)&&type(row)==='policy.power_plan'&&row.field_id==='active'&&typeof row.after==='boolean')return row.after?'当时切换为此电源方案。':'当时已不再使用此电源方案。';
    if((!uncertainAssociation(row)||row.action_id==='startup.service_state_changed'&&row.context?.classification_status!=='unverified')&&type(row)==='startup.service'&&row.field_id==='state'&&typeof row.after==='string'){
      if(row.after.toLowerCase()==='stopped')return '服务已停止，依赖它的功能可能暂不可用。';
      if(row.after.toLowerCase()==='running')return '服务已开始运行。';
    }
    if(row.field_id==='module_base')return '程序的模块加载位置已变化。';
    if(['identity.logon','identity.logoff'].includes(row.action_id)&&Number(row.context?.logon_type)===5)return '后台服务使用了此账户。';
    if(row.action_id==='startup.task_started')return '任务已开始运行。';
    if(row.action_id==='startup.task_ended')return ['failed','failure'].includes(row.result)?'本次任务未成功完成，相关自动处理可能受影响。':['succeeded','success'].includes(row.result)?'本次任务已成功完成。':row.result==='cancelled'?'本次任务已取消。':'本次运行已结束，日志未提供运行结果。';
    if(row.action_id==='identity.logon_failed')return '本次登录未成功，相关账户未通过这次登录验证。';
    if(row.action_id==='identity.special_privileges')return '这次登录获准执行部分需要较高权限的操作。';
    if(row.action_id==='network.connection_blocked')return '这次连接被阻止，相关访问可能无法完成。';
    if(row.action_id==='observed.no_longer_observed')return absence(row).impact;
    if(!uncertainAssociation(row)&&row.field_id==='exists'&&row.after===false&&['file.watched_file','file.watched_directory'].includes(type(row)))return type(row)==='file.watched_directory'?'检查时文件夹已不在原位置，依赖该路径的功能可能受影响。':'检查时文件已不在原位置，依赖它的功能可能无法启动。';
    if(['sha256','version','display_version'].includes(row.field_id)&&([null,undefined].includes(row.before)||[null,undefined].includes(row.after)||row.before_status&&row.before_status!=='known'||objects.presentation.hidden(row.before)))return '缺少前后对比值，可展开查看已取得的信息。';
    if(row.field_id==='sha256')return '文件内容已变化。';
    if(['version','display_version'].includes(row.field_id))return '版本信息已变化。';
    if(['dacl','owner','access','permissions'].includes(row.field_id))return '访问权限有变化，可能影响谁能使用或修改相关内容。';
    if(row.field_id&&[null,undefined].includes(row.before)||row.field_id&&[null,undefined].includes(row.after))return '缺少前后对比值，可展开查看已取得的信息。';
    if(row.field_id)return '属性值已变化。';
    if(row.result==='failed')return '这次操作未成功，相关功能可能受影响。';
    return '';
  }
  const uncertainAssociation=row=>['unverified','mismatch'].includes(row.context?.attribution_status)||row.context?.classification_status==='unverified';
  const attribution=row=>row.context?.classification_status==='unverified'?'日志内容尚未识别，已保留原始记录。':uncertainAssociation(row)?'日志未能对应到具体对象。':'';
  // A timeline card has one conclusion. Missing/protected values are reading states, not changes.
  function card(row){
    const note=impact(row),id=type(row),field=row.field_id,context={...row.context,type_id:id};
    const result={kind:'event',label:title(row),tone:'neutral',note:''};
    if(historicalMappingIssue(row))return {...result,label:'旧记录需复核',tone:'attention',note};
    const status=(value,explicit)=>objects.presentation.hidden(value)?'masked':explicit&&explicit!=='known'?explicit:value===null||value===undefined?'not_provided':'known';
    const beforeStatus=status(row.before,row.before_status),afterStatus=status(row.after,row.after_status);
    const format=(value,state)=>{
      // A changed file path is useful here; registry identities, commands and secrets stay in details.
      if(state==='known'&&['path','server_path','module_path','binary_path','image','target','install_location','working_directory','driver_path'].includes(field)&&typeof value==='string'&&value.length<=1024&&/^(?:[a-z]:[\\/]|\\\\)/i.test(value)&&!/[\x00-\x1f]/.test(value))return value;
      return objects.valueLabel(value,field,context,state);
    };
    if(field){
      result.field=fieldLabel(row);
      if(afterStatus==='masked')return {...result,label:'信息已隐藏'};
      if(afterStatus!=='known')return {...result,label:({unreadable:'本次读取失败',conflicting:'信息存在冲突',evidence_pruned:'原始依据已清理',not_applicable:'不适用',not_provided:'本次未提供'})[afterStatus]||'本次状态未知',tone:afterStatus==='unreadable'?'attention':'neutral'};
      if(!uncertainAssociation(row)&&id==='startup.service'&&field==='state'){
        if(String(row.after).toLowerCase()==='running')result.tone='positive';
        if(String(row.after).toLowerCase()==='stopped')result.note='当时依赖此服务的功能可能暂不可用。';
      }
      if(field==='exists'&&row.after===false&&['file.watched_file','file.watched_directory'].includes(id))result.note='依赖此路径的功能可能受影响。';
      const after=format(row.after,afterStatus),before=format(row.before,beforeStatus);
      if(!uncertainAssociation(row)&&['observed.state_changed','observed.configuration_changed'].includes(row.action_id)){
        if(id==='security.protection'&&['signature_version','engine_version'].includes(field))return {...result,field:'',label:row.source_record?.protection_update?.engine_changed&&row.source_record?.protection_update?.signature_changed?'防护程序和病毒库已更新':field==='signature_version'?'病毒库已更新':'防护程序已更新',tone:'positive'};
        if(id==='hardware.battery'&&field==='charge'){
          const band=value=>{if(typeof value!=='number'&&!(typeof value==='string'&&value.trim()))return null;const n=Number(value);return Number.isFinite(n)&&n>=0&&n<=100?(n<=10?0:n<=20?1:2):null;};
          const old=band(row.before),now=band(row.after);
          if(old!==null&&now!==null&&old!==now)return {...result,field:'',label:(now===0?'电量很低':now===1?'电量偏低':'电量已恢复')+' · '+after,tone:now<2?'attention':'positive'};
        }
        if(id==='account.user_profile'&&field==='last_use')return {...result,kind:'value',field:'最近使用时间已更新',after};
        if(id==='startup.scheduled_task'&&field==='last_run')return {...result,kind:'value',field:'最近执行时间已更新',after};
        if(id==='startup.scheduled_task'&&field==='next_run')return {...result,kind:'value',field:'下次执行时间已调整',after};
        if(id==='network.adapter'&&field==='speed')return {...result,kind:'value',field:'连接速率已变化',after};
      }
      if(field==='sha256'||field==='module_base')return {...result,label:beforeStatus==='known'?(field==='sha256'?'文件内容已变化':'模块加载位置已变化'):after};
      if(beforeStatus!=='known')return {...result,kind:'value',after};
      const opaque=/详细|展开查看|尚未识别|含义待核实|状态未知|已隐藏/;
      if(opaque.test(before)||opaque.test(after))return {...result,label:before===after&&JSON.stringify(row.before)!==JSON.stringify(row.after)?result.field+'已变化':'详细变化已记录'};
      if(before===after)return {...result,kind:'value',after};
      result.kind='diff';result.before=before;result.after=after;
      result.long=before.length+after.length>64||/[\\/;\n]/.test(before+after);
      return result;
    }
    const labels={'execution.process_started':'开始运行','execution.process_stopped':'已退出','execution.module_loaded':'已加载','files.deleted':'已删除','files.deleted_archived':'已删除并归档','startup.task_started':'开始运行','network.connection_allowed':'已允许','network.connection_blocked':'已阻止','identity.logon_failed':'登录失败'};
    result.label=labels[row.action_id]||result.label;
    if(row.action_id==='observed.first_seen')result.label=firstSeen(row);
    if(row.action_id==='observed.no_longer_observed')result.label=absence(row).title;
    if(row.action_id==='startup.task_ended'){
      result.label=['succeeded','success'].includes(row.result)?'运行成功':['failed','failure'].includes(row.result)?'运行失败':row.result==='cancelled'?'本次运行已取消':'本次运行已结束';
      if(!['succeeded','success','failed','failure','cancelled'].includes(row.result))result.note='日志未提供运行结果。';
    }
    if(row.action_id==='software.update_history')result.label=({failed:'更新失败',failure:'更新失败',succeeded:'更新成功',success:'更新成功',partial:'部分完成',cancelled:'已取消'})[row.result]||result.label;
    if(['succeeded','success'].includes(row.result))result.tone='positive';
    if(['failed','failure'].includes(row.result)){result.tone='negative';result.note=note;}
    if(row.action_id?.startsWith('coverage.')&&row.action_id!=='coverage.recovered')result.tone='attention';
    if(row.action_id==='coverage.recovered')result.tone='positive';
    if(row.action_id==='network.connection_blocked')result.note='相关访问可能无法完成。';
    if(!labels[row.action_id]&&['observed','unknown',undefined].includes(row.result)&&!['observed.no_longer_observed','startup.task_ended'].includes(row.action_id)&&note&&/可能.*(?:漏|失败|不可用|受影响)/.test(note))result.note=note;
    return result;
  }
  const options=()=>Object.keys(actions).map(id=>({id,label:title({action_id:id})}));
  return {card,title,typeLabel,subject,fieldLabel,change,impact,options,uncertainAssociation,attribution,historicalMappingIssue,result:row=>historicalMappingIssue(row)?'待复核':results[row.result]||'未取得操作结果'};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeReadableEvents;
