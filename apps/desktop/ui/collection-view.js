'use strict';
const PCScopeView=(()=>{
  const presentation=typeof PCScopePresentation!=='undefined'?PCScopePresentation:(typeof require==='function'?require('./event-presentation.js'):null);
  let definitions=[];
  const ready=fetch('collection-catalog.json').then(r=>{if(!r.ok)throw new Error('无法读取采集项目录');return r.json();}).then(rows=>{presentation?.configure(rows);definitions=rows.sort((a,b)=>Number(!!a.prefix)-Number(!!b.prefix)||Number(!a.contains)-Number(!b.contains)||(a.priority||0)-(b.priority||0));});
  const normalize=s=>String(s).replace(/[_\s-]/g,'').toLowerCase();
  function data(row){return row.data?.after??row.data?.before??row.data??{};}
  function flattened(d){return {...Object.fromEntries(Object.entries(d?.values||{}).map(([k,v])=>[k,v&&typeof v==='object'&&Object.hasOwn(v,'value')?v.value:v])),...d};}
  function get(object,...keys){for(const key of keys){const entry=Object.entries(object||{}).find(([k,v])=>normalize(k)===normalize(key)&&v!==null&&v!==''&&typeof v!=='object');if(entry)return entry[1];}return '';}
  function definition(row){const id=String(row.object_id??row.id??'').toLowerCase();return definitions.find(d=>{
    if(!(d.prefix?String(row.source).startsWith(d.source):row.source===d.source)||d.contains&&!d.contains.some(s=>id.includes(s)))return false;
    const rule=d.event_rule;if(!rule)return true;
    if(!row.kind)return rule.suffix==='other';
    if(rule.status&&(!row.kind.startsWith('coverage.')||row.kind==='coverage.security_log_cleared'))return false;
    if(rule.provider&&row.data?.provider!==rule.provider)return false;
    if(rule.event_ids&&!rule.event_ids.includes(row.data?.event_id))return false;
    return true;
  });}
  const eventItems=()=>definitions.filter(d=>d.event_rule).map(d=>({item:d.id,item_name:d.name,category:d.category,source:d.source,group:d.group}));
  const groupFor=item=>definitions.find(d=>d.id===item);
  const readable=value=>typeof value==='string'&&value.trim()&&!/^(?:pcscope\.|S-\d-\d+(?:-\d+)+$|\{?[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}\}?$|HKLM|HKCU|HKEY_)/i.test(value)&&!/[|\r\n]/.test(value)?value.trim():'';
  const shortName=value=>{const name=readable(value);return name?readable(name.split(/[\\/]/).filter(Boolean).pop()):'';};
  const itemName=row=>readable(row.item_name)||definition(row)?.name||'其他系统记录';
  function objectName(row){
    const model=presentation?.model(row);if(model)return model.object;
    const d=flattened(data(row)),f=d.fields||{};
    const value=get(d,'DisplayName','name','Caption','TaskName','PackageFullName','HotFixID','FriendlyName','Subject','DriveLetter','MountPoint','InterfaceAlias','User')||get(f,'KeyName','ProductName','TaskName','ServiceName','TargetUserName','Image','NewProcessName','DeviceInstanceId','TargetFilename');
    if(shortName(value))return shortName(value);
    const location=get(d,'path');
    if(shortName(location))return shortName(location);
    if(row.source?.startsWith('pcscope.eventlog.')&&!row.kind?.startsWith('coverage.'))return '系统日志记录';
    if(shortName(row.name))return shortName(row.name);
    return row.kind?.startsWith('coverage.')?'采集状态':'未识别的系统项目';
  }
  function compact(k,v){if(v&&typeof v==='object')return Array.isArray(v)?`${v.length} 项`:'展开查看内容';if(v===undefined)return '未提供';const value=PCScopeDetails.valueText(k,v);if(/^(state|status|startmode|runlevel|signaturestatus)$/.test(normalize(k))){if(typeof v==='string'&&value===v&&/^[A-Za-z_][A-Za-z0-9_. -]*$/.test(v))return '尚未识别';return value.replace(/（[A-Za-z_]+）$/,'');}return value;}
  const important=['DisplayVersion','version','state','status','enabled','start_mode','publisher','manufacturer','model','address','drive_letter','file_system','capacity','path','last_logon'];
  function attributes(row){
    const d=data(row),entries=Object.entries(flattened(d));
    const usable=entries.filter(([k,v])=>v!==null&&v!==''&&typeof v!=='object'&&important.some(key=>normalize(key)===normalize(k))&&PCScopeDetails.label(k)!==k&&(typeof v!=='string'||readable(v)&&!/[\\/]/.test(v)));
    usable.sort((a,b)=>{const rank=k=>{const n=important.findIndex(v=>normalize(v)===normalize(k));return n<0?999:n;};return rank(a[0])-rank(b[0]);});
    return usable.slice(0,3).map(([k,v])=>`${PCScopeDetails.label(k)}：${compact(k,v)}`).join(' · ')||'已保存记录，可展开查看详情';
  }
  const actions={'baseline.observed':'初始状态','object.first_seen':'新发现','object.changed':'属性变化','object.no_longer_observed':'不再发现','windows.event':'系统事件','files.usn_changed':'文件变更日志','identity.logon':'登录事件','identity.logon_failed':'登录失败','identity.logoff':'注销事件','identity.logoff_requested':'请求注销','identity.account_created':'账户建立','identity.account_enabled':'账户启用','identity.account_disabled':'账户禁用','identity.account_deleted':'账户删除','identity.account_changed':'账户变更','identity.account_locked':'账户锁定','identity.account_unlocked':'账户解锁','identity.account_renamed':'账户重命名','identity.group_member_added':'组成员加入','identity.group_member_removed':'组成员移除','identity.special_privileges':'特殊权限分配事件','identity.session_locked':'会话锁定','identity.session_unlocked':'会话解锁','identity.session_reconnected':'会话重连','identity.session_disconnected':'会话断开','execution.process_started':'进程启动事件','execution.process_stopped':'进程结束事件','software.installer_event':'安装程序事件','startup.service_event':'服务事件','startup.task_event':'计划任务事件','security.defender_event':'Defender 事件','execution.powershell_event':'PowerShell 事件','system.update_event':'系统更新事件','execution.sysmon_event':'Sysmon 事件','system.lifecycle_event':'系统启动与电源事件'};
  function actionName(id){
    if(id==='coverage.recovered')return '采集恢复';
    if(id==='coverage.gap')return '部分变化未能记录';
    if(id==='coverage.log_reset')return '日志记录中断';
    for(const d of definitions)for(const value of Object.values(d.event_rule?.actions||{}))if(value.id===id)return value.name;
    return actions[id]||'系统活动';
  }
  const action=row=>actionName(row.action||definition(row)?.event_rule?.actions?.[row.data?.event_id]?.id||row.kind);
  function changes(before,after,prefix='',result=[]){
    if(!before||!after||typeof before!=='object'||typeof after!=='object')return result;
    for(const key of new Set([...Object.keys(before),...Object.keys(after)])){
      if(['basis','sampledat','observedat'].includes(normalize(key)))continue;
      const a=before[key],b=after[key];if(JSON.stringify(a)===JSON.stringify(b))continue;
      const label=prefix+PCScopeDetails.label(key);
      if(a?.redacted===true||b?.redacted===true){result.push(label+'：摘要变化（原值未保存）');continue;}
      if(PCScopeDetails.label(key)===key&&!/[\u3400-\u9fff]/.test(key)){result.push('其他属性已变化');continue;}
      if(/path|location|sha256|hash|sddl|registry|sid|^id$|pid|command|argument/i.test(normalize(key))){result.push(label+'已变化');continue;}
      if(a&&b&&typeof a==='object'&&typeof b==='object'&&!Array.isArray(a)&&!Array.isArray(b))changes(a,b,label+' / ',result);
      else result.push(`${label}：${compact(key,a)} → ${compact(key,b)}`);
    }return result;
  }
  function summary(row){
    if(row.kind==='object.changed'){const list=changes(row.data?.before,row.data?.after);return list.length?list.slice(0,2).join('；')+(list.length>2?`；另有 ${list.length-2} 项变化`:''):'采样或其他属性发生变化，展开查看前后值';}
    if(row.kind==='object.no_longer_observed')return '本次检查清单中没有此前记录的项目';
    if(row.kind==='coverage.recovered')return '已恢复读取。';
    if(row.kind?.startsWith('coverage.'))return '采集状态发生变化，可展开查看当时的记录。';
    if(row.kind==='baseline.observed')return '首次采集时已存在 · '+attributes(row);
    if(row.kind==='object.first_seen')return '新发现 · '+attributes(row);
    const model=presentation?.model(row);if(model)return model.summary;
    if(row.source?.startsWith('pcscope.eventlog.')&&typeof PCScopeEvidence!=='undefined'){const text=PCScopeEvidence.eventSummary(row);if(text)return text;}
    if(row.source?.startsWith('pcscope.eventlog.'))return '已保留系统日志，可展开查看活动内容。';
    return attributes(row);
  }
  const eventModel=row=>presentation?.model(row);
  const title=row=>eventModel(row)?.title||action(row)+' · '+objectName(row);
  const activityLabel=row=>row.activity?.classification==='self'?({'powershell-host':'PCScope PowerShell 宿主日志','service':'PCScope 后台','frontend':'PCScope 界面','collector':'PCScope 采集器','descendant':'PCScope 采集子进程','child-launch':'PCScope 启动的进程'}[row.activity.role]||'PCScope 自身活动'):'';
  return {activityLabel,title,eventModel,ready,itemName,objectName,attributes,action,actionName,summary,eventItems,groupFor};
})();
