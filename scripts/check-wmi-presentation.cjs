const assert=require('node:assert/strict'),fs=require('node:fs'),p=require('../apps/desktop/ui/event-presentation.js');
const row=(id,fields)=>({source:'pcscope.eventlog.microsoft-windows-wmi-activity/operational',kind:'windows.event',data:{provider:'Microsoft-Windows-WMI-Activity',event_id:id,writer_process_id:99999,fields}});
const wrapped=(prefix,fields)=>Object.fromEntries(Object.entries(fields).map(([k,v])=>[prefix+'/'+k,v]));
const failure=(operation,code='0x80041001')=>row(5858,wrapped('Operation_ClientFailure',{Operation:operation,ResultCode:code,ClientProcessId:42,User:'example'}));
let r=failure('Start IWbemServices::CreateInstanceEnum - ROOT\\StandardCimv2 : MSFT_Printer');
let m=p.model(r);assert.equal(m.title,'查询对象列表 · 打印机');assert.equal(m.result.label,'失败');assert.match(m.summary,/PID 42/);assert.match(m.summary,/未给出具体原因/);assert(!m.summary.includes('99999'));assert(!m.summary.includes('IWbemServices'));assert.equal(m.technical.find(([k])=>k==='命名空间')[1],'ROOT\\StandardCimv2');
assert.equal(p.model(failure('Start IWbemServices::ExecQuery - ROOT\\CIMV2 : SELECT * FROM Win32_Service','0x80041032')).title,'查询对象列表 · Windows 服务');
for(const code of ['0x80041032','2147749938','-2147217358'])assert.equal(p.model(failure('unknown',code)).result.label,'已取消');
assert.match(p.model(failure('unknown','0x80041003')).result.description,/权限/);
const vendorRow=failure('Start IWbemServices::ExecQueryAsync - root\\cimv2 : select Name from Vendor_Class WHERE Name="Win32_Service"');
const vendorModel=p.model(vendorRow);
assert.equal(vendorModel.object,'其他系统管理项目');
assert.doesNotMatch(vendorModel.title,/Vendor_Class|Win32_Service/,'unknown management classes must not be presented as readable names or guessed from query conditions');
assert.equal(vendorModel.technical.find(([key])=>key==='管理对象类')[1],'Vendor_Class','the exact class remains in folded technical details');
assert.equal(p.model(failure('Start IWbemServices::ExecMethod - root\\cimv2 : Win32_Process.Handle="42"::Terminate')).action,'调用管理方法');
assert.equal(p.model(failure('Start IWbemServices::ExecNotificationQuery - root\\cimv2 : SELECT * FROM __InstanceCreationEvent')).action,'订阅对象变化');
assert.equal(p.model(failure('malformed Win32_Service')).object,'未记录可识别对象');
assert.equal(p.model(failure('Start IWbemServices::ExecQuery - root\\cimv2 : SELECT " FROM Win32_Process " FROM Vendor_Class')).object,'未记录可识别对象');
assert.equal(p.model(failure('unknown','0x100000000')).result.label,'未解析');
assert.equal(p.model(failure('******')).object,'操作内容已隐藏');
for(const fields of [{},{Operation:{redacted:true},ResultCode:'invalid'},wrapped('WrongPrefix',{Operation:'Win32_Service',ClientProcessId:42})]){m=p.model(row(5858,fields));assert(m.title);assert(!m.summary.includes('99999'));}
const start=row(5857,wrapped('Operation_StartedOperational',{Code:'0x0',ProviderName:'ScheduledTaskProv',HostProcess:'wmiprvse.exe',ProcessID:25,ProviderPath:'example.dll'}));
m=p.model(start);assert.equal(m.title,'启动系统管理组件 · 计划任务管理组件');assert.equal(m.result.label,'成功');assert.match(m.summary,/宿主进程 ID：25/);assert(!m.summary.includes('发起进程'));assert(m.technical.some(([k,v])=>k==='组件文件'&&v==='example.dll'));
assert.equal(p.model({...r,source:'pcscope.eventlog.application'}).technical,undefined);
assert.equal(p.model({...r,data:{...r.data,provider:'Other'}}).technical,undefined);
assert.equal(p.model({...r,kind:'coverage.gap'}),null);
const copy=JSON.stringify(r);p.model(r);assert.equal(JSON.stringify(r),copy);
const history=process.argv[2];if(history){const h=JSON.parse(fs.readFileSync(history,'utf8').replace(/^\uFEFF/,''));for(const [key,g] of Object.entries(h.groups)){const id=Number(key.split(' / ')[1]);for(const sample of g.samples){m=p.model(row(id,sample.fields));assert(m.title);console.log(`${g.count} records: ${m.title} · ${m.result.label}`);}}}
console.log('PASS: WMI source-qualified aliases, queries, provider startup, caller/host distinction, HRESULTs, masking, unknowns and raw preservation');
