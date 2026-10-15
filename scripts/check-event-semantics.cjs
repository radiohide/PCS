const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const evidence=require('../apps/desktop/ui/evidence-explanations.js'),sem=require('../apps/desktop/ui/event-semantics.js'),catalog=require('../apps/desktop/ui/collection-catalog.json');
const row=(id,fields={},message)=>({source:'pcscope.eventlog.security',kind:'windows.event',data:{provider:'Microsoft-Windows-Security-Auditing',event_id:id,fields,message,writer_process_id:1568}});
const key=row(5058,{Operation:'%%2457',KeyName:'ExampleKey',ReturnCode:'0x0',ClientProcessId:'7096',KeyType:'%%2500'});
assert.match(evidence.explain(key),/删除密钥文件/);assert.match(evidence.explain(key),/成功/);assert.match(evidence.explain(key),/未关联 · PID 7096/);assert.doesNotMatch(evidence.explain(key),/事件写入进程|不能|需要结合/);
assert.match(evidence.eventSummary(row(5059,{Operation:'%%2464'})),/导出持久化密钥/);
assert.match(evidence.eventSummary(row(5061,{Operation:'%%2480'})),/打开密钥/);
assert.match(evidence.eventSummary(row(5061,{},'Cryptographic Operation:\n\tOperation:\tCreate Key.')),/创建密钥/);
assert.match(require('../apps/desktop/ui/event-presentation.js').model(row(5058,{Operation:'%%9999'})).groups[0].fields.flat().join(' '),/未解析/);
assert.match(evidence.eventSummary(row(5379,{ReturnCode:'3221226021'})),/失败/);
assert.match(evidence.eventSummary(row(5382,{ReturnCode:'1168'})),/失败/);
assert.match(evidence.eventSummary(row(5061,{ReturnCode:'******'})),/已隐藏/);
assert.match(evidence.eventSummary({...key,data:{...key.data,provider:'wrong'}}),/未识别事件/);
const auditSettings=row(4907,{ObjectType:'File',ObjectName:'C:\\Windows\\SysWOW64\\WindowManagementAPI.dll',OldSd:'S:AINO_ACCESS_CONTROL',NewSd:'S:ARAI(AU;SAFA;DCLCRPCRSDWDWO;;;WD)',SubjectUserName:'SYSTEM',ProcessId:'0x6d8',ProcessName:'C:\\Windows\\System32\\poqexec.exe'});
const presentation=require('../apps/desktop/ui/event-presentation.js'),auditModel=presentation.model(auditSettings);
assert.equal(auditModel.unknown,false);assert.equal(auditModel.action,'对象访问审计规则已修改');assert.equal(auditModel.object,'WindowManagementAPI.dll');assert.equal(auditModel.type,'files');assert.equal(auditModel.result,null);
assert.match(sem.definition(auditSettings).explanation,/后续哪些访问会写入安全日志/);assert.doesNotMatch(sem.definition(auditSettings).explanation,/不表示|不代表|不等于/);
assert.deepEqual(auditModel.groups.find(g=>g.name==='操作属性').fields.filter(([label])=>/安全描述符/.test(label)),[['原安全描述符',auditSettings.data.fields.OldSd],['新安全描述符',auditSettings.data.fields.NewSd]]);
assert.match(auditModel.summary,/poqexec\.exe/);assert(!('instance_id' in auditModel));
assert.match(auditModel.summary,/影响后续哪些访问会写入安全日志/);assert.match(evidence.eventSummary(auditSettings),/影响后续哪些访问会写入安全日志/);
for(const other of [{...auditSettings,source:'pcscope.eventlog.application'},{...auditSettings,data:{...auditSettings.data,provider:'Other'}}]){assert.equal(sem.definition(other),undefined);assert.equal(presentation.model(other).unknown,true);}
const hiddenAudit=presentation.model(row(4907,{ObjectName:'******',OldSd:'******',NewSd:{redacted:true},ProcessName:'******'}));
assert.equal(hiddenAudit.object,'已隐藏');assert(hiddenAudit.groups.flatMap(g=>g.fields).filter(([label])=>/安全描述符/.test(label)).every(([,value])=>value==='已隐藏'));
assert.equal(presentation.model(row(4907,{ObjectType:'Key',ObjectName:'\\REGISTRY\\MACHINE\\SOFTWARE\\Example'})).object,'注册表设置');
assert.equal(presentation.model(row(4907)).unknown,false);
const identities=new Set();for(const family of sem.families)for(const [id,event] of Object.entries(family.events)){
 const identity=family.channel+'|'+family.provider+'|'+id;assert(!identities.has(identity),identity);identities.add(identity);assert(event.explanation.trim().length>0,identity+' requires an explanation');assert.doesNotMatch(event.explanation,/不表示|不代表|不等于|不等同|不能据此/,identity+' should describe the event directly');
 const item=catalog.find(c=>c.source==='pcscope.eventlog.'+family.channel.toLowerCase()&&c.event_rule?.provider===family.provider&&c.event_rule?.event_ids?.includes(+id));assert(item,identity);assert.equal(item.event_rule.actions[id].name,event.name);
}
const context=vm.createContext({fetch:async()=>({ok:true,json:async()=>catalog}),PCScopeDetails:require('../apps/desktop/ui/object-details.js'),PCScopeEvidence:evidence,PCScopePresentation:require('../apps/desktop/ui/event-presentation.js')});
vm.runInContext(fs.readFileSync('apps/desktop/ui/collection-view.js','utf8')+';globalThis.view=PCScopeView',context);
(async()=>{await context.view.ready;assert.equal(context.view.itemName(key),'Windows 安全审计 / 加密密钥操作');assert.equal(context.view.objectName(key),'ExampleKey');assert.match(context.view.title(key),/删除密钥文件/);
 assert.equal(context.view.itemName(auditSettings),'Windows 安全审计 / 对象访问记录规则');assert.equal(context.view.objectName(auditSettings),'WindowManagementAPI.dll');assert.match(context.view.title(auditSettings),/对象访问审计规则已修改/);
 if(process.argv[2]){const rows=JSON.parse(fs.readFileSync(process.argv[2],'utf8').replace(/^\uFEFF/,'')),old=rows.filter(r=>r.item.endsWith('.other'));for(const r of old)assert(!context.view.itemName({source:r.source,kind:'windows.event',data:r}).endsWith('其他事件'),r.source+'/'+r.event_id);console.log('Historical audit: '+old.length+' formerly fallback types, '+old.reduce((n,r)=>n+r.count,0)+' records now classified');}
 console.log('PASS: '+identities.size+' provider-qualified semantic rules; crypto operations, failed credentials, masking, caller identity and catalog/UI consistency');})();
