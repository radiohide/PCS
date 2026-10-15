'use strict';
// This module formats evidence; it never queries Windows or changes object identity.
const PCScopeTypePresentation=(()=>{
  const data=typeof PCScopeTypePresentationData!=='undefined'?PCScopeTypePresentationData:require('./type-presentation-data.js');
  const protectedStatuses=new Set(['masked','unreadable','conflicting','evidence_pruned']);
  const statuses={masked:'已隐藏',unreadable:'无法读取',conflicting:'信息存在冲突',evidence_pruned:'原始依据已清理',not_applicable:'不适用',not_provided:'来源未提供',unknown:'状态未知'};
  const hidden=value=>value==='******'||Boolean(value&&typeof value==='object'&&(value.redacted||value.sensitive||value.display==='******'));
  const profile=row=>data.types[typeof row==='string'?row:row?.type_id];
  // The same collection outcome must read the same way in cards, trees and object details.
  const absenceGroups=[
    [['runtime.process'],'已不在运行列表','进程已不在运行列表','此前的进程已不在运行列表中。'],
    [['runtime.module_instance'],'已不在加载列表','模块已不在加载列表','该模块已不在进程的加载列表中。'],
    [['runtime.logon_session','runtime.terminal_session'],'已不在登录会话列表','登录会话已不在列表','此前的登录会话已不在当前列表中。'],
    [['desktop.print_job'],'已离开打印队列','打印任务已离开队列','作业可能已完成或被取消，日志未提供打印结果。'],
    [['network.endpoint'],'已不在连接与监听列表','连接或监听项已不在列表','此前的连接或监听项已不在当前列表中。'],
    [['network.smb_session'],'已不在文件共享连接列表','文件共享连接已不在列表','此前的连接已不在当前列表中。'],
    [['startup.service','hardware.driver_service'],'服务登记不再列出','服务登记不再列出','当前服务清单中已无此登记。'],
    [['startup.scheduled_task'],'计划任务登记不再列出','计划任务登记不再列出','当前计划任务清单中已无此登记。'],
    [['software.installation_registration','software.portable_registration'],'安装登记不再列出','安装登记不再列出','当前软件安装清单中已无此登记。'],
    [['software.appx_package'],'应用包不再列出','应用包不再列出','当前账户的应用包列表中已无此包。'],
    [['software.provisioned_package'],'预装应用登记不再列出','预装应用登记不再列出','新账户的预装应用清单中已无此项。'],
    [['file.watched_file','file.watched_directory'],'监控清单未再列出','监控清单未再列出此项','监控清单中已无此项。'],
    [['account.user'],'账户清单未再列出','账户清单未再列出','当前账户清单中已无此账户。'],
    [['account.group_membership'],'成员关系不再列出','用户组成员关系不再列出','当前清单中已无此成员关系。'],
    [['account.right_assignment'],'权限分配不再列出','账户权限分配不再列出','当前清单中已无此权限分配。'],
    [['account.user_profile'],'用户配置登记不再列出','用户配置登记不再列出','当前用户配置清单中已无此登记。'],
    [['network.dns_cache_entry'],'DNS 缓存中已无此项','DNS 缓存中已无此项','当前 DNS 缓存中已无此项。'],
    [['security.certificate_registration'],'证书登记不再列出','证书登记不再列出','所查证书存储区已无此登记。'],
    [['security.key_metadata'],'密钥清单未再列出','密钥清单未再列出','当前密钥清单中已无此登记。'],
    [['virtualization.virtual_machine'],'虚拟机登记不再列出','虚拟机登记不再列出','当前虚拟机清单中已无此登记。'],
    [['virtualization.disk_attachment'],'虚拟磁盘连接不再列出','虚拟磁盘连接不再列出','虚拟机不再列出这条磁盘连接。'],
    [['virtualization.container'],'容器清单未再列出','容器清单未再列出','当前容器清单中已无此项。'],
    [['network.adapter','hardware.device','hardware.disk','hardware.partition','hardware.volume','hardware.battery','hardware.firmware','hardware.tpm','desktop.printer'],'本次未检测到','本次未检测到此设备','当前设备清单中已无此项。'],
    [['hardware.driver_binding'],'设备驱动关联不再列出','设备驱动关联不再列出','当前清单中已无此设备与驱动的关联。']
  ];
  function absence(row){
    const entry=absenceGroups.find(group=>group[0].includes(row?.type_id));
    if(entry)return {label:entry[1],title:entry[2],impact:entry[3]};
    const name=profile(row)?.name;
    return {label:name?name+'未再列出':'本次清单未再列出',title:name?name+'未再列出':'本次清单未再列出此项',impact:'本次检查清单中没有此前记录的项目。'};
  }
  function lifecycle(row){
    if(row.lifecycle==='not_observed')return absence(row).label;
    if(row.lifecycle==='ended')return ({'runtime.process':'进程已退出','runtime.module_instance':'模块加载已结束','runtime.logon_session':'登录会话已结束','runtime.terminal_session':'登录会话已结束','network.endpoint':'连接已结束','network.smb_session':'文件共享连接已结束','desktop.print_job':'打印任务已结束'})[row.type_id]||(profile(row)?.name||'此项')+'已结束';
    if(row.lifecycle==='removed')return ({'network.adapter':'网卡已移除','hardware.device':'设备已移除','software.installation_registration':'安装登记已删除','software.portable_registration':'安装登记已删除','file.watched_file':'文件已删除','file.watched_directory':'文件夹已删除'})[row.type_id]||(profile(row)?.name||'此项')+'已移除';
    return '';
  }
  function facts(row,full=[]){
    const map=new Map();
    // Detailed fields are fresher than the summary, but protection wins in either direction.
    for(const f of [...full,...(row?.key_facts||[])]){if(!f?.field_id)continue;const previous=map.get(f.field_id);
      if(!previous||!protectedStatuses.has(previous.status)&&!hidden(previous.value)&&(protectedStatuses.has(f.status)||hidden(f.value)))map.set(f.field_id,f);
    }return map;
  }
  function sources(row){return [...new Set([...(row?.source_ids||[]),...(row?.coverage||[]).map(x=>x.source)].filter(x=>typeof x==='string'))];}
  function defenderKind(row,all=facts(row)){
    if(row?.type_id!=='security.protection')return '';
    const f=all.get('name'),n=f?.status==='known'&&!hidden(f.value)?f.value:'';
    const s=sources(row),policy=s.includes('pcscope.security.defender-policy'),runtime=s.includes('pcscope.security.protection');
    if(policy&&runtime||n==='defender-policy'&&runtime||n==='defender'&&policy)return 'ambiguous';
    if(n==='defender-policy'||policy)return 'configuration';
    if(n==='defender'||runtime)return 'runtime';
    return 'ambiguous';
  }
  function descriptor(row,id,all=facts(row)){
    const p=profile(row),base=p?.formats?.[id]||{kind:'text'};
    if(row?.type_id==='security.protection'&&id==='real_time'){
      const kind=defenderKind(row,all);
      if(kind==='configuration')return {kind:'boolean',true:'设置为开启',false:'设置为关闭'};
      if(kind!=='runtime')return {kind:'ambiguous'};
    }
    return base;
  }
  function label(row,id,all=facts(row)){
    if(row?.type_id==='security.protection'&&id==='real_time'&&defenderKind(row,all)==='configuration')return '实时防护设置';
    return profile(row)?.labels?.[id]||'';
  }
  const shortPath=value=>value.replace(/[\\/]+$/,'').split(/[\\/]/).pop()||'路径已记录';
  function format(row,id,value,status='known',all=facts(row)){
    if(hidden(value))return '已隐藏';
    if(status!=='known')return statuses[status]||'状态未知';
    if(value===null||value===undefined)return '未记录';
    const f=descriptor(row,id,all);
    if(f.kind==='ambiguous')return '含义待核实';
    if(f.unknown?.includes(value))return '状态未知';
    if(f.kind==='boolean')return typeof value==='boolean'?(value?f.true:f.false):'值尚未识别';
    if(f.kind==='enum'){
      const lookup=v=>{if(!['string','number','boolean'].includes(typeof v))return '值尚未识别';const key=Object.keys(f.values||{}).find(k=>k.toLowerCase()===String(v).toLowerCase());return key===undefined?'值尚未识别':f.values[key];};
      if(Array.isArray(value))return value.length?value.slice(0,4).map(lookup).join('、')+(value.length>4?'等':''):'未报告状态';
      return lookup(value);
    }
    if(f.kind==='details')return '详细内容已记录';
    if(f.kind==='count')return Array.isArray(value)?`${value.length} ${f.unit||'项'}`:typeof value==='number'&&Number.isFinite(value)&&value>=0?`${value} ${f.unit||'项'}`:'详细内容已记录';
    if(f.kind==='time'){
      if(typeof value!=='string'||!/^\d{4}-\d{2}-\d{2}[T ]/.test(value))return '时间尚未识别';
      const date=new Date(value);return Number.isNaN(date.getTime())?'时间尚未识别':date.toLocaleString('sv-SE',{hour12:false});
    }
    if(f.kind==='bytes'){
      if(typeof value!=='number'||!Number.isFinite(value)||value<0)return '大小尚未识别';
      const units=['B','KiB','MiB','GiB','TiB'];let n=value,i=0;while(n>=1024&&i<units.length-1){n/=1024;i++;}return `${Number(n.toFixed(i?1:0))} ${units[i]}`;
    }
    if(f.kind==='percent')return typeof value==='number'&&Number.isFinite(value)&&value>=0&&value<=100?`${value}%`:'比例尚未识别';
    if(f.kind==='duration')return typeof value==='number'&&Number.isFinite(value)&&value>=0?`${value} ${f.unit==='minute'?'分钟':'秒'}`:'时长尚未识别';
    if(typeof value==='object')return Array.isArray(value)?`包含 ${value.length} 项`:'详细内容已记录';
    if(typeof value==='boolean')return value?'是':'否';
    if(typeof value==='number')return Number.isFinite(value)?String(value):'值尚未识别';
    if(typeof value!=='string')return '值尚未识别';
    if(f.kind==='text'&&f.values){const key=Object.keys(f.values).find(key=>key.toLowerCase()===value.toLowerCase());if(key!==undefined)return f.values[key];}
    if(f.kind==='path')return shortPath(value);
    if(value.length>120||/^(?:S-\d-\d+(?:-\d+)+|\{?[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}\}?|0x[0-9a-f]+)$/i.test(value)||/^(?:HKLM|HKCU|HKEY_|machine:|user:)/i.test(value)||value.includes('|'))return '详细值已记录';
    return value.trim()||'空值';
  }
  function entries(row,full=[],options={}){
    const p=profile(row);if(!p)return [];
    const all=facts(row,full),out=[];
    for(const id of options.states?p.state_fields:p.summary_fields){
      const f=all.get(id);if(!f||f.status==='not_applicable'||f.status==='not_provided')continue;
      if(!options.states&&options.omitNames&&['name','display_name','caption','subject'].includes(id))continue;
      // New policy data supersedes the legacy policy field, without treating it as runtime state.
      if(row.type_id==='security.protection'&&id==='real_time'&&defenderKind(row,all)==='configuration'&&all.get('real_time_configured')?.status==='known')continue;
      out.push({fact:f,id,label:label(row,id,all)||f.label||'属性',text:format(row,id,f.value,f.status||'unknown',all),state:p.state_fields.includes(id)});
    }
    return out;
  }
  function state(row,full=[]){
    const outcome=lifecycle(row);if(outcome)return outcome;
    if(row.presentation?.record_status==='legacy_mixed_process')return '旧记录需整理';
    if(['legacy_snapshot','snapshot_named'].includes(row.presentation?.record_status))return '有程序记录';
    if(row.record_basis==='event_only')return row.presentation?.record_status==='legacy_misclassified'?'旧记录需复核':'有日志记录';
    const p=profile(row),known=entries(row,full,{states:true});
    if(known.length)return known.slice(0,2).map(e=>(p.state_fields.length>1?e.label+'：':'')+e.text).join(' · ');
    if(['account.user','account.right_assignment'].includes(row.type_id)&&row.record_basis!=='observation')return '最近有记录';
    return p?.fallback||'有采集记录';
  }
  function displayName(row,full=[],candidate=''){
    const p=profile(row),all=facts(row,full);
    if(!p)return '';
    if(['name','display_name'].some(id=>{const name=all.get(id);return name&&(protectedStatuses.has(name.status)||hidden(name.value));}))return '';
    if(p.fixed_name)return p.fixed_name;
    if(p.context==='defender'){const kind=defenderKind(row,all);return kind==='configuration'?'Windows 防护设置':kind==='runtime'?'Windows 防护状态':'';}
    for(const rule of p.name_aliases||[]){
      if(candidate!==rule.name)continue;
      if(rule.require_confirmed_identity&&row.identity_status!=='confirmed')continue;
      const f=rule.field?all.get(rule.field):null;
      if(!rule.field||f?.status==='known'&&!hidden(f.value)&&typeof f.value==='string'&&f.value.replace(/^\{(.*)\}$/,'$1').toLowerCase()===rule.value.toLowerCase())return rule.label;
    }
    return '';
  }
  return {data,profile,facts,format,label,entries,state,displayName,defenderKind,hidden,absence,lifecycle};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeTypePresentation;
