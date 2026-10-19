'use strict';
// Reusable semantics checks. --live <local IPC capture> adds real samples without copying them.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const root=path.resolve(__dirname,'..'),read=p=>JSON.parse(fs.readFileSync(path.resolve(root,p),'utf8'));
const p=require('../apps/desktop/ui/type-presentation.js'),names=require('../apps/desktop/ui/object-names.js'),view=require('../apps/desktop/ui/model-view.js'),events=require('../apps/desktop/ui/readable-events.js');
const catalog=read('model/catalog.json'),config=read('model/presentation.json');
const fact=(field_id,value,status='known')=>({field_id,value,status});
const row=(type_id,key_facts=[],extra={})=>({type_id,key_facts,alias:'fixture-'+type_id,identity_status:'confirmed',record_basis:'observation',lifecycle:'active',...extra});
const detail=(summary,fields)=>({data:{summary,fields,actions_preview:[],bindings:[]}});
function visible(html){let depth=0,result='';for(const part of html.split(/(<\/?details\b[^>]*>)/i)){if(/^<details\b/i.test(part)){depth++;continue;}if(/^<\/details/i.test(part)){depth--;continue;}if(!depth)result+=part;}return result.replace(/<[^>]*>/g,'');}
assert.deepEqual(Object.keys(config.types).sort(),catalog.types.map(t=>t.id).sort());
assert.deepEqual(p.data,config,'runtime bundle must be exactly the backend configuration');
const adapterDevice=row('hardware.device',[fact('device_id','ACPI\\ACPI0003\\0'),fact('name','Microsoft AC Adapter'),fact('class','Battery')]);
assert.equal(names.name(adapterDevice),'电源适配器');
assert.equal(p.displayName({...adapterDevice,identity_status:'provisional'},[],'Microsoft AC Adapter'),'');
assert.equal(p.displayName(row('hardware.device',[fact('device_id','ACPI\\OTHER\\0')]),[],'Microsoft AC Adapter'),'');
assert.equal(p.displayName(adapterDevice,[],'自定义设备名称'),'');
assert.equal(p.displayName({...adapterDevice,key_facts:[fact('device_id','ACPI\\ACPI0003\\0'),fact('name','Microsoft AC Adapter','masked')]},[],'Microsoft AC Adapter'),'');
assert.equal(p.format(adapterDevice,'class','Battery'),'电池与电源');
assert.equal(p.format(adapterDevice,'class','VendorCustomClass'),'VendorCustomClass');
for(const t of catalog.types){const profile=config.types[t.id];assert(profile.summary_fields.length,t.id);
  for(const f of [...profile.summary_fields,...profile.state_fields])assert(t.fields.some(x=>x.id===f),t.id+' / '+f);
  assert(profile.state_fields.every(f=>profile.summary_fields.includes(f)));
  assert.match(names.typeName(row(t.id)),/[\u4e00-\u9fff]/);
  for(const id of profile.state_fields){
    const r=row(t.id,[fact(id,'FUTURE_STATE_937')]);
    assert(!p.state(r).includes('FUTURE_STATE_937'),t.id+' unknown state must stay in raw details');
    for(const status of ['masked','unreadable','conflicting','evidence_pruned']){
      const protectedRow=row(t.id,[fact(id,'PRIVATE_STATE',status)]);
      assert(!visible(view.list({data:[protectedRow]})).includes('PRIVATE_STATE'));
      assert(!visible(view.detail(detail(protectedRow,[fact(id,'PRIVATE_STATE')]))).includes('PRIVATE_STATE'),'summary protection must win over full detail');
      assert(!visible(view.detail(detail(row(t.id,[fact(id,'PRIVATE_STATE')]),[fact(id,'PRIVATE_STATE',status)]))).includes('PRIVATE_STATE'),'detail protection must win over summary');
    }
    const format=profile.formats[id];if(format.kind==='boolean'){
      const booleanContext=row(t.id,t.id==='security.protection'?[fact('name',id==='real_time'?'defender':'defender-policy')]:[]);
      assert.equal(p.format(booleanContext,id,true),format.true);
      assert.equal(p.format(booleanContext,id,false),format.false);
      assert.equal(p.format(booleanContext,id,1),'值尚未识别','boolean sources never coerce numeric enum values');
    }
  }
}
const power=row('policy.power_plan',[fact('name','Balanced'),fact('guid','381b4222-f694-41f0-9685-ff5bb260df2e'),fact('active',true)]);
assert.equal(names.name(power),'平衡');assert.equal(p.state(power),'正在使用');
assert.equal(p.state({...power,key_facts:[fact('active',false)]}),'未在使用');
assert.equal(names.name({...power,key_facts:[fact('name','Balanced'),fact('guid','other-guid')]}),'Balanced','same name does not establish preset identity');
assert.equal(names.name({...power,key_facts:[fact('name','我的平衡'),power.key_facts[1]]}),'我的平衡','custom preset names are preserved');
assert.equal(names.name({...power,key_facts:[fact('name','Balanced')]}),'Balanced','missing GUID cannot enable preset translation');
assert.match(view.detail(detail(power,power.key_facts)),/Balanced/,'original name remains in details');
const service=row('startup.service',[fact('name','fixture'),fact('state','Stopped'),fact('start_mode','Auto')]);
assert.match(visible(view.list({data:[service]})),/已停止/);assert.match(visible(view.list({data:[service]})),/自动启动/);
assert(!p.state({...service,lifecycle:'active',key_facts:[]}).includes('运行中'),'lifecycle cannot replace a missing runtime state');
assert.equal(p.state({...service,key_facts:[]}), '有服务配置','a missing runtime field retains the known configuration scope');
assert.equal(p.state(row('policy.os_configuration',[fact('caption','Windows')])), '有系统资料');
assert.equal(p.state({...service,key_facts:[fact('state',null,'unknown')]}), '状态未知','explicit uncertainty must not be hidden by the fallback');
assert(!names.typeName(row('runtime.process',[],{lifecycle:'ended'})).includes('运行程序'));
for(const type of ['network.firewall_profile','network.firewall_rule']){
  assert.equal(p.format(row(type),'enabled',1),'已开启');assert.equal(p.format(row(type),'enabled',2),'已关闭');
}
assert.equal(p.format(row('network.firewall_profile'),'inbound',0),'未配置');
assert.equal(p.format(row('network.firewall_rule'),'action',2),'允许');assert.equal(p.format(row('network.firewall_rule'),'action',4),'阻止');
assert.equal(p.format(row('network.connection_profile'),'category',1),'专用网络');
assert.equal(p.format(row('network.connection_profile'),'ipv4_connectivity',4),'可访问互联网');
assert.equal(p.format(row('network.connection_profile'),'ipv6_connectivity',1),'尚无流量');
assert.notEqual(p.format(row('network.connection_profile'),'category',1),p.format(row('network.firewall_profile'),'enabled',1),'same number has type-specific meaning');
const policy=row('security.protection',[fact('name','defender-policy'),fact('real_time',true)]);
assert.equal(names.name(policy),'Windows 防护设置');assert.match(p.state(policy),/设置为开启/);assert.doesNotMatch(p.state(policy),/正在保护/);
const runtime=row('security.protection',[fact('name','defender'),fact('real_time',true)]);
assert.match(p.state(runtime),/正在保护/);assert.equal(names.name(runtime),'Windows 防护状态');
assert.match(p.state({...policy,coverage:[{source:'pcscope.security.protection'}]}),/含义待核实/,'conflicting source and identity cannot classify policy as runtime');
const newPolicy={...policy,key_facts:[fact('name','defender-policy'),fact('real_time',null,'not_applicable'),fact('real_time_configured',false)]};
assert.match(p.state(newPolicy),/设置为关闭/);assert.doesNotMatch(p.state(newPolicy),/正在保护|未开启/);
for(const v of ['******',{redacted:true,value:true},{sensitive:true,value:true},{display:'******',value:true}])assert.equal(p.format(power,'active',v),'已隐藏');
const timeline={action_id:'observed.state_changed',field_id:'active',before:false,after:true,context:power};
assert.match(events.change(timeline),/使用状态：未在使用 → 正在使用/);
assert.match(visible(view.timeline({data:[timeline]})),/使用状态.*未在使用.*正在使用/);
assert.match(events.change({...timeline,before_status:'masked'}),/已隐藏 → 正在使用/);
assert.match(events.impact(timeline),/当时切换为此电源方案/);
for(const status of ['masked','unreadable','conflicting','evidence_pruned','unknown'])assert.doesNotMatch(events.impact({...timeline,after_status:status}),/切换为|不再使用/,'protected state must not leak through impact wording');
assert.doesNotMatch(events.impact({...timeline,after:{redacted:true,value:true}}),/切换为|不再使用/);
for(const [after,readable,impact] of [['Running','运行中','服务已开始运行'],['Stopped','已停止','依赖它的功能可能暂不可用']]){
 const action={action_id:'startup.service_state_changed',field_id:'state',before:null,after,result:'observed',context:service};
 assert.equal(events.title(action),'服务运行状态变化');assert(events.options().some(option=>option.id===action.action_id&&option.label==='服务运行状态变化'));
 const rendered=visible(view.timeline({data:[action]}));assert(rendered.includes(readable));if(after==='Stopped')assert.match(rendered,/功能可能暂不可用/);else assert(!rendered.includes('服务已开始运行'),'current state does not need a duplicate sentence');assert(!rendered.includes(action.action_id));
 assert(!events.change(action).includes('→'),'an explicit resulting state does not invent a previous value');
 assert(events.impact({...action,context:{...service,attribution_status:'unassigned'}}).includes(impact),'known event meaning survives an unresolved object link');
}
const unsafe=row('startup.service',[fact('name','<img src=x onerror=bad>'),fact('state','<script>bad</script>'),fact('start_mode','FUTURE_937')]);
assert(!view.list({data:[unsafe]}).includes('<img'));assert(!visible(view.list({data:[unsafe]})).includes('FUTURE_937'));assert(view.detail(detail(unsafe,unsafe.key_facts)).includes('FUTURE_937'),'unknown original remains in closed details');
const compare=view.comparison({summary:power,fields:power.key_facts},{data:[timeline]});assert.match(visible(compare),/未在使用/);assert.match(visible(compare),/正在使用/);
// History quality is an explicit provenance finding, never guessed from a path's shape.
const com=row('shell.com_registration',[fact('name','Library Folder Context Menu'),fact('server_path','C:\\Windows\\system32\\shell32.dll')]);
assert.equal(names.name(com),'Library Folder Context Menu');assert.equal(p.format(com,'local_server_command','"C:\\Program Files\\Example\\server.exe" /Embedding'),'详细内容已记录');
const comAliases=config.types['shell.com_registration'].name_aliases;
assert.equal(comAliases.length,125);
assert.equal(new Set(comAliases.map(rule=>rule.value.toLowerCase()+'|'+rule.name)).size,comAliases.length,'COM translations cannot duplicate an exact matching rule');
for(const alias of comAliases){
 assert.equal(alias.require_confirmed_identity,true);assert.equal(alias.field,'clsid');
 const registered=row('shell.com_registration',[fact('name',alias.name),fact('clsid','{'+alias.value.toUpperCase()+'}')]);
 assert.equal(names.name(registered),alias.label);
 assert.equal(events.subject({context:{...registered,subject_label:alias.name,subject_basis:'collected_registry_label'}}),alias.label);
 assert.equal(names.name({...registered,key_facts:[fact('name',alias.name),fact('clsid','00000000-0000-0000-0000-000000000000')]}),alias.name,'matching description alone cannot identify a COM class');
 assert.equal(names.name({...registered,key_facts:[fact('name','自定义组件名称'),registered.key_facts[1]]}),'自定义组件名称','custom descriptions remain source names');
 assert.equal(names.name({...registered,key_facts:[fact('name',alias.name),fact('clsid',alias.value.slice(0,2)+'{'+alias.value.slice(2)+'}')]}),alias.name,'malformed brace placement cannot match an exact class id');
 for(const identity_status of ['provisional','ambiguous',undefined]){
  assert.equal(names.name({...registered,identity_status}),alias.name);
  assert.equal(events.subject({context:{...registered,identity_status,subject_label:alias.name}}),alias.name);
 }
 for(const status of ['masked','unreadable','conflicting','evidence_pruned']){
  const protectedRow={...registered,key_facts:[fact('name',alias.name,status),registered.key_facts[1]]};
  assert.notEqual(names.name(protectedRow),alias.label);
  const subject=events.subject({context:{...protectedRow,subject_label:alias.name}});
  assert.notEqual(subject,alias.label);assert.notEqual(subject,alias.name,'protected names must not escape through a history subject');
  assert.equal(names.name({...registered,key_facts:[registered.key_facts[0],fact('clsid',alias.value,status)]}),alias.name);
 }
 assert.equal(p.displayName(registered,[fact('name',alias.name,'masked')],alias.name),'','detailed protection wins over summary');
 assert.equal(p.displayName({...registered,key_facts:[fact('name',alias.name,'masked'),registered.key_facts[1]]},registered.key_facts,alias.name),'','summary protection wins over details');
}
const comAction={action_id:'observed.configuration_changed',field_id:'server_path',before:'Library Folder Context Menu',after:'C:\\Windows\\system32\\shell32.dll',result:'observed',context:com};
assert.equal(events.historicalMappingIssue(comAction),false,'description-like text alone is not proof of an old mapping defect');
assert(events.change(comAction).includes('→'));
const legacy={...comAction,before:'LEGACY_DESCRIPTION_SENTINEL <img src=x onerror=bad>',context:{...com,history_quality:{status:'legacy_mapping_ambiguous',reason:'旧映射需复核 <script>bad()</script>'}}};
assert.equal(events.title(legacy),'旧版采集记录需复核');assert.equal(events.result(legacy),'待复核');assert.equal(events.change(legacy),'');
assert.match(events.impact(legacy),/暂不能用于判断系统变化/);
const legacyDetail={data:{summary:com,fields:com.key_facts,actions_preview:[legacy],bindings:[]}};
for(const html of [view.timeline({data:[legacy]}),view.detail(legacyDetail),view.comparison(legacyDetail.data,{data:[legacy]})]){
 const text=visible(html);assert(text.includes('暂不能用于判断系统变化'));assert(!text.includes('LEGACY_DESCRIPTION_SENTINEL'));assert(!text.includes('→'));
 assert(html.includes('LEGACY_DESCRIPTION_SENTINEL'),'original value is retained in a closed raw disclosure');assert(!html.includes('<img'));assert(!html.includes('<script>'));
}
assert(!visible(view.timeline({data:[{...legacy,after_status:'masked',after:'PRIVATE_SERVER_SENTINEL'}]})).includes('PRIVATE_SERVER_SENTINEL'));
assert.equal(events.historicalMappingIssue({...comAction,context:{...com,history_quality:{status:'future_quality'}}}),false,'unknown future quality must not be reclassified as this specific legacy defect');
const dictionaryContext=row('software.installation_registration',[fact('hive','LocalMachine')],{presentation:{name_status:'dictionary_matched',basis:'registration_dictionary',name:'组件维护登记',description:'供组件维护使用。'},attribution_status:'stored'});
const dictionaryAction={action_id:'observed.configuration_changed',instance_id:17,context:dictionaryContext};
assert.equal(events.subject(dictionaryAction),'组件维护登记','confirmed dictionary identity survives historical context rendering');
assert.notEqual(events.subject({...dictionaryAction,context:{...dictionaryContext,identity_status:null}}),'组件维护登记','missing identity cannot authorize dictionary matching');
assert.equal(events.subject({...dictionaryAction,context:{...dictionaryContext,key_facts:[fact('display_name','来源原名')]}}),'来源原名','a real source name wins over the dictionary');
assert.notEqual(events.subject({...dictionaryAction,context:{...dictionaryContext,key_facts:[fact('name','PRIVATE_NAME','masked')]}}),'组件维护登记','dictionary fallback cannot bypass protected names');
const clearedContext={type_id:'software.installation_registration',identity_status:null,attribution_status:'mismatch',key_facts:[],presentation:{name_status:'not_provided',basis:'unverified_attribution'}};
assert.notEqual(events.subject({...dictionaryAction,instance_id:null,context:clearedContext}),'组件维护登记');
assert.equal(events.subject({action_id:'coverage.recovered'}),'PCScope 采集监控');
let live=0;const args=process.argv.slice(2),at=args.indexOf('--live');
if(at>=0){assert(args[at+1],'--live requires a path');const samples=read(args[at+1]);
  const expected={
    'policy.power_plan':['平衡','正在使用'],'network.firewall_profile':['域网络','已开启','未配置'],
    'startup.service':['运行中','手动启动'],'network.connection_profile':['专用网络','可访问互联网','尚无流量'],
    'security.protection':['Windows 防护状态','正在保护'],'policy.os_configuration':['19044','10.0.19044'],
    'security.secure_boot_configuration':['安全启动','已开启'],'software.optional_feature':['已关闭']
  };
  for(const sample of samples.filter(x=>x.label?.startsWith('detail_'))){const d=sample.response.data;if(!expected[d.summary.type_id])continue;
    const text=visible(view.detail(sample.response));for(const phrase of expected[d.summary.type_id])assert(text.includes(phrase),d.summary.type_id+' missing '+phrase);
    assert(!text.includes('暂无更多信息'));assert(!text.includes('最近仍在'));
    // Emulate the backend's approved summary selection, retaining original value statuses.
    const allowed=new Set(config.types[d.summary.type_id].summary_fields);
    const summary={...d.summary,key_facts:d.fields.filter(f=>allowed.has(f.field_id)||f.kind==='identity'||['name','display_name','caption'].includes(f.field_id))};
    const list=visible(view.list({data:[summary]}));assert(!list.includes('最近仍在'));
    for(const id of config.types[d.summary.type_id].state_fields){const f=d.fields.find(f=>f.field_id===id&&f.status==='known');if(f)assert(list.includes(p.format(summary,id,f.value,f.status)),d.summary.type_id+' state absent from list');}
    live++;
  }assert.equal(live,8,'all eight real regression types must be checked');
}
console.log(JSON.stringify({profiles:Object.keys(config.types).length,live_samples:live,typed_states:'passed',protection:'passed',timeline_and_comparison:'passed'}));
