'use strict';
// Labels composed only from supplied, unprotected facts. These never establish identity.
const PCScopeObjectNameRules=(()=>{
  const blocked=new Set(['masked','unreadable','conflicting','evidence_pruned']);
  const guid=/^\{?[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}\}?$/i;
  const hidden=v=>v==='******'||v&&typeof v==='object'&&(v.sensitive||v.redacted||v.display==='******');
  const leaf=v=>typeof v==='string'?v.replace(/[\\/]+$/,'').split(/[\\/]/).pop():'';
  const clean=v=>typeof v==='string'&&!hidden(v)&&v.trim()&&v!=='-'?v.trim():'';
  const human=v=>clean(v)&&!guid.test(v)&&!/^S-\d-/.test(v)?v:'';
  function describe(row,all,profile={}){
    const get=id=>{const f=all.get(id);return f?.status==='known'&&!hidden(f.value)?f.value:undefined;};
    const text=id=>clean(get(id)),type=row.type_id;
    const result=(name,basis='composed')=>name?{name,basis}:null;
    const record=row.presentation;
    if(type==='runtime.process'&&record?.record_status==='legacy_mixed_process')return result('旧程序记录（多次运行混在一起）','unresolved');
    if(type==='software.installation_registration'&&row.identity_status==='confirmed'&&record?.basis==='registration_container')return result('软件安装信息目录','registration_role');
    if(type==='runtime.process'&&['legacy_snapshot','snapshot_named'].includes(record?.record_status)){
      if(human(text('name')))return result(text('name'),'source_provided');
      return human(record.record_label)?result('程序记录 · '+record.record_label,'snapshot_evidence'):result('程序记录（旧资料缺少名称）','unresolved');
    }
    if(row.record_basis==='event_only'){
      if(record?.record_status==='legacy_misclassified')return result('旧版误归类记录','unresolved');
      const label=human(record?.record_label);
      if(label){const prefix=type==='runtime.logon_session'?'登录记录':type==='runtime.process'?'程序活动记录':type==='account.group_membership'?'用户组变更记录':'日志记录';return result(prefix+' · '+leaf(label),'event_evidence');}
      if(['runtime.process','runtime.logon_session','startup.scheduled_task','account.group_membership'].includes(type))return result(({'runtime.process':'程序活动记录','runtime.logon_session':'登录记录','startup.scheduled_task':'任务记录','account.group_membership':'用户组变更记录'})[type]+'（日志信息不完整）','unresolved');
    }
    if(type==='network.endpoint'){
      const port=get('local_port'),remote=get('remote_port'),local=text('local_address'),peer=text('remote_address'),protocol=text('protocol');
      const validPort=p=>Number.isInteger(p)&&p>=0&&p<=65535;
      if(local&&validPort(port)){
        const endpoint=(address,p)=>['0.0.0.0','::'].includes(address)?'本机 '+p+' 端口':(address.includes(':')?'['+address+']':address)+':'+p;
        const localName=endpoint(local,port),peerName=peer&&!['0.0.0.0','::'].includes(peer)&&validPort(remote)&&remote>0?endpoint(peer,remote):'';
        return result((['TCP','UDP'].includes(protocol.toUpperCase())?protocol.toUpperCase()+' · ':'')+localName+(peerName?' ↔ '+peerName:''));
      }
    }
    if(type==='runtime.process'&&text('image'))return result(leaf(text('image')),'source_path');
    if(type==='runtime.logon_session'){
      const label=profile.formats?.logon_type?.values?.[String(get('logon_type'))];
      const when=text('start_time'),date=when&&new Date(when);
      if(label)return result(label+(date&&Number.isFinite(date.getTime())?' · '+date.toLocaleString('zh-CN',{hour12:false}):''));
    }
    if(type==='account.group_membership'&&human(text('group'))){
      const member=human(record?.member_label)||human(text('member'))||({'S-1-5-4':'交互登录用户','S-1-5-11':'已通过身份验证的用户','S-1-5-17':'匿名网页访问账户'})[text('member')];
      if(member)return result(member+' · '+text('group')+'组成员');
      return result(text('group')+'组成员（账户名称未提供）','unresolved');
    }
    if(type==='hardware.driver_binding'){
      if(human(text('name')))return result(text('name')+'驱动','source_provided');
      if(text('inf'))return result('设备驱动 · '+leaf(text('inf')),'source_path');
      const device=text('device');
      if(device&&!device.includes('\\')&&human(device))return result(device+'驱动','source_provided');
      if(device==='HTREE\\ROOT\\0')return result('设备树根节点驱动登记','device_role');
      if(device)return result('设备驱动登记（系统未提供驱动名称）','unresolved');
    }
    if(type==='hardware.disk'&&Number.isInteger(get('number')))return result('磁盘 '+get('number')+(human(text('name'))?' · '+text('name'):''));
    if(type==='hardware.partition'&&Number.isInteger(get('disk_number'))&&Number.isInteger(get('partition_number')))return result('磁盘 '+get('disk_number')+' · 分区 '+get('partition_number'));
    if(type==='hardware.volume'){
      if(/^[a-z]:$/i.test(text('drive_letter')))return result(text('drive_letter').toUpperCase()+' 盘');
      if(Number.isFinite(get('size')))return result('未分配盘符的存储卷 · '+(get('size')/1048576).toFixed(0)+' MB');
    }
    if(type==='hardware.firmware')return result((human(text('manufacturer'))||'计算机')+' 固件');
    if(type==='hardware.device'&&!human(text('name'))){
      if(row.identity_status==='confirmed'&&text('device_id')==='HTREE\\ROOT\\0')return result('设备树根节点','device_role');
      if(get('config_error')===28)return result('未安装驱动的设备（名称未提供）','unresolved');
    }
    if(type==='network.route'&&text('destination'))return result('到 '+text('destination')+' 的路由'+(human(text('interface'))?' · '+text('interface'):''));
    if(type==='network.dns_configuration'){
      if(human(text('interface')))return result(text('interface')+'的域名解析设置');
      if(all.get('interface')?.status==='conflicting')return result('域名解析记录（网卡归属冲突）','unresolved');
      if(text('path').toLowerCase()==='system\\currentcontrolset\\services\\tcpip\\parameters')return result('全机域名解析设置','source_path');
    }
    if(type==='network.firewall_rule'&&!human(text('display_name'))&&!human(text('name')))return result('防火墙规则（旧资料缺少名称）','unresolved');
    if(type==='security.certificate_registration'){
      const subject=text('subject'),m=subject.match(/(?:^|,\s*)CN=("(?:[^"]|"")*"|[^,]+)/i);
      if(m&&human(m[1]))return result(m[1].replace(/^"|"$/g,'')+'证书','source_provided');
      const org=subject.match(/(?:^|,\s*)O=([^,]+)/i);if(org&&human(org[1]))return result(org[1]+'证书','source_provided');
    }
    if(type==='shell.com_registration'&&!human(text('name'))&&text('server_path'))return result(leaf(text('server_path'))+' 应用组件','source_path');
    if(type==='shell.icon_overlay_handler'&&!human(text('name'))&&text('path'))return result(leaf(text('path'))+' 图标标记','source_path');
    if(type==='shell.file_association'||type==='shell.context_menu_registration'){
      const path=text('path').toLowerCase();
      if(type==='shell.file_association'&&path==='software\\microsoft\\windows\\currentversion\\explorer\\fileexts')return result('文件打开方式设置目录','source_path');
      if(type==='shell.file_association'&&path==='software\\microsoft\\windows\\shell\\associations\\urlassociations')return result('链接打开方式设置目录','source_path');
      const parts=text('path').split('\\'),at=parts.findIndex(x=>x.toLowerCase()==='classes'),owner=parts[at+1];
      if(at>=0&&human(owner)){
        const target=({'*':'所有文件',AllFilesystemObjects:'文件和文件夹',Directory:'文件夹',Folder:'文件夹',Drive:'磁盘',Applications:'应用程序'})[owner]||(owner.startsWith('.')?owner+' 文件':owner+' 文件类型');
        const tail=parts.slice(at+2).map(x=>x.toLowerCase());
        const role=tail.includes('defaulticon')?'图标设置':tail.includes('contextmenuhandlers')?'右键菜单扩展':tail.includes('shell')?'操作菜单':tail.includes('shellex')?'扩展设置':'打开方式设置';
        const verb=tail.includes('shell')?parts[at+3]:'';
        const action=({open:'打开',edit:'编辑',print:'打印',runas:'以管理员身份运行',find:'搜索'})[verb?.toLowerCase()]||human(verb);
        return result(target+' · '+role+(action?' · '+action:''),'source_path');
      }
    }
    if(type==='startup.logon_initialization'){
      const path=text('path').toLowerCase();
      if(path==='software\\microsoft\\active setup\\installed components')return result('用户首次登录设置目录','source_path');
      if(path.includes('\\active setup\\')){
        const values=get('values'),raw=values?.['']?.value;
        if(hidden(raw)||values?.['']&&typeof raw!=='string')return result('用户首次登录设置（组件名称不可读）','unresolved');
        const name=human(raw);if(name&&!name.startsWith('@'))return result((profile.name_translations?.[name]||name)+' · 用户初始化','source_provided');
        const alias=row.identity_status==='confirmed'&&!name?profile.registration_names?.find(x=>x.path.toLowerCase()===path):null;
        if(alias)return result((profile.name_translations?.[alias.name]||alias.name)+' · 用户初始化','dictionary');
        return result('用户首次登录设置（组件名称未提供）','unresolved');
      }
      if(path.endsWith('\\winlogon'))return result('Windows 登录初始化设置');
      if(path.endsWith('\\windows'))return result('Windows 会话初始化设置');
    }
    if(type==='startup.wmi_binding'){
      const extract=s=>s.match(/\(Name\s*=\s*"([^"]+)"\)/)?.[1];
      const filter=extract(text('filter')),consumer=extract(text('consumer'));
      if(filter&&consumer)return result(filter+' → '+consumer);
    }
    if(type==='software.appx_package'&&guid.test(text('name'))&&text('install_location'))return result(leaf(text('install_location')).replace(/_[a-z0-9]{13}$/i,'')+' 应用','source_path');
    if(type==='software.installation_registration'&&!human(text('display_name'))&&/^(?:hklm\\|hkcu\\)?software\\microsoft\\windows\\currentversion\\uninstall$/i.test(text('path')))return result('软件安装信息目录','source_path');
    if(type==='desktop.shortcut'&&all.get('path')?.status==='conflicting')return result('快捷方式记录（多个路径混在一起）','unresolved');
    return null;
  }
  return {describe};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeObjectNameRules;
