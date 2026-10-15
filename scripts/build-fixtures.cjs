'use strict';
// S02 fixture corpus. Emits deterministic protocol v1 message lines so the same corpus always
// produces the same database contents. All identities, accounts, paths and values are invented:
// no real machine GUID, SID, user name, credential, key material or personal data appears here.
// Timestamps are fixed literals, never the current clock, so reloads are byte-identical.
const fs=require('node:fs');
const path=require('node:path');
const root=path.resolve(__dirname,'..');
const outDir=path.join(root,'fixtures');
const invalidDir=path.join(outDir,'invalid');

// ---------- fixed, invented identity ----------
const MACHINE='11111111-2222-3333-4444-555555555555';
const SID='S-1-5-21-1000000000-2000000000-3000000000-1001';
const SID2='S-1-5-21-1000000000-2000000000-3000000000-1002';
const USER='示例用户';
let requestSeq=0;
const rid=prefix=>`fixture-${prefix}-${String(++requestSeq).padStart(4,'0')}`;

function scope(context=''){return `machine:${MACHINE}/user:${SID}/schema:1/context:${context}/shard:0`;}
function snapshot(source,items,{complete=true,observedAt='2026-09-10T00:00:00Z',context='',runId='fixture-run-1'}={}){
  return {protocol_version:1,request_id:rid('snapshot'),type:'snapshot',source,scope:scope(context),run_id:runId,observed_at:observedAt,complete,items};
}
function eventBatch(source,events,{epoch='fixture-epoch-1',batchSeq=1,checkpoint='fixture-cp-1'}={}){
  return {protocol_version:1,request_id:rid('batch'),type:'event_batch',source,epoch,batch_seq:batchSeq,checkpoint,events};
}
function gap(source,reason,observedAt='2026-09-10T00:00:00Z'){
  return {protocol_version:1,request_id:rid('gap'),type:'gap',source,reason,observed_at:observedAt};
}
const item=(id,data)=>({id,data});
const event=(id,occurredAt,kind,objectId,data)=>({id,occurred_at:occurredAt,kind,object_id:objectId,data});

// ---------- scenario 1: first complete round ----------
const baseline=[
  // 账户与身份
  snapshot('pcscope.identity.accounts',[
    item(SID,{name:USER,enabled:true,last_logon:'2026-09-09T21:14:00Z',password_required:true,basis:'local account enumeration'}),
    item(SID2,{name:'示例管理员',enabled:true,last_logon:null,password_required:true,basis:'local account enumeration'}),
    item('S-1-5-18',{name:'SYSTEM',enabled:true,last_logon:null,password_required:false,basis:'local account enumeration'}),
  ]),
  snapshot('pcscope.identity.groups',[
    item('S-1-5-32-544',{name:'Administrators',members:[SID2]}),
    item('S-1-5-32-545',{name:'Users',members:[SID,SID2]}),
  ]),
  // 软件与组件
  snapshot('pcscope.software',[
    item('hklm\\registry64\\{00000000-0000-0000-0000-000000000001}',{display_name:'示例编辑器',display_version:'1.2.3',publisher:'示例软件公司',install_location:'C:\\示例\\编辑器',uninstall_command_protected:true}),
    item('hklm\\registry64\\{00000000-0000-0000-0000-000000000002}',{display_name:'示例运行时',display_version:'8.0.1',publisher:'示例软件公司'}),
    item('hklm\\registry32\\{00000000-0000-0000-0000-000000000003}',{display_name:'示例编辑器',display_version:'0.9.0',publisher:'示例软件公司',install_location:'C:\\示例\\旧版'}),
  ]),
  snapshot('pcscope.software.appx-user',[
    item('示例应用包_1.0.0.0_x64__abcdefgh',{name:'示例应用包',version:'1.0.0.0',publisher:'CN=示例发布者',status:'Ok'}),
  ]),
  // 运行实例
  snapshot('pcscope.execution.processes',[
    item('4100/2026-09-10T00:00:00Z',{pid:4100,parent_pid:900,image:'C:\\示例\\程序\\示例程序.exe',session:1,created_at:'2026-09-10T00:00:00Z',command_line_protected:'示例程序.exe --示例参数'}),
    item('4200/2026-09-10T00:00:00Z',{pid:4200,parent_pid:4100,image:'C:\\Windows\\System32\\示例服务.exe',session:0,created_at:'2026-09-10T00:00:00Z'}),
  ]),
  snapshot('pcscope.execution.resources',[item('machine',{memory_total_bytes:17179869184,memory_available_bytes:8589934592,cpu_percent:12,basis:'point-in-time sample'})]),
  // 自动执行入口
  snapshot('pcscope.startup.tasks',[
    item('\\示例文件夹\\示例任务',{name:'示例任务',path:'\\示例文件夹\\示例任务',enabled:true,principal:'示例用户',actions:[{execute:'C:\\示例\\程序\\示例程序.exe',arguments_protected:'--示例程序 A'}],triggers:[{type:'Daily',start_boundary:'2026-09-10T18:00:00Z'}]}),
    item('\\示例文件夹\\示例任务二',{name:'示例任务二',path:'\\示例文件夹\\示例任务二',enabled:false,principal:'SYSTEM',actions:[{execute:'C:\\示例\\程序\\示例工具.exe'}],triggers:[{type:'Boot'}]}),
  ]),
  snapshot('pcscope.startup.tasks',[item('\\示例文件夹\\示例任务三',{name:'示例任务三',path:'\\示例文件夹\\示例任务三',enabled:true,principal:SID,actions:[{execute:'C:\\示例\\程序\\示例服务.exe'}]})],{context:'task-detail'}),
  snapshot('pcscope.startup.services',[
    item('示例服务',{name:'示例服务',state:'Running',start_mode:'Auto',account:'LocalSystem',display_name:'示例后台服务'}),
    item('示例驱动服务',{name:'示例驱动服务',state:'Stopped',start_mode:'Manual',account:'LocalSystem'}),
  ]),
  snapshot('pcscope.startup.logon',[item('hklm\\registry64\\software\\microsoft\\windows\\currentversion\\run/示例启动项',{hive:'LocalMachine',value_name:'示例启动项',command_protected:'C:\\示例\\程序\\示例程序.exe /autostart'})]),
  snapshot('pcscope.startup.wmi-filters',[item('示例筛选器',{name:'示例筛选器',namespace:'root\\cimv2',query_language:'WQL',query_protected:'SELECT * FROM __InstanceCreationEvent'})]),
  // 系统扩展与关联
  snapshot('pcscope.shell.registrations',[item('hklm\\registry64\\software\\microsoft\\windows\\currentversion\\previewhandlers/示例预览处理程序',{hive:'LocalMachine',view:'Registry64',name:'示例预览处理程序',clsid:'{00000000-0000-0000-0000-0000000000aa}'})]),
  // 文件与访问控制
  snapshot('pcscope.files.metadata-security',[item('c:\\示例\\监控\\示例文件.txt',{path:'C:\\示例\\监控\\示例文件.txt',exists:true,directory:false,size:128,owner:'示例用户',dacl_sddl_protected:'D:(A;;FA;;;SY)',streams:[{name:':Zone.Identifier',size:26}],content_stored:false})]),
  // 策略与配置
  snapshot('pcscope.policy.configuration',[item('hklm\\registry64\\software\\microsoft\\windows\\currentversion\\policies',{hive:'LocalMachine',view:'Registry64',values:{EnableLUA:{kind:'DWord',value:1}}})]),
  snapshot('pcscope.environment.configuration',[item('hklm\\registry64\\system\\currentcontrolset\\control\\session manager\\environment',{hive:'LocalMachine',values:{TEMP:{kind:'ExpandString',value:'C:\\示例\\临时'}}})]),
  // 网络与远程访问
  snapshot('pcscope.network.tcp',[item('127.0.0.1:12345-127.0.0.1:80',{local_address:'127.0.0.1',local_port:12345,remote_address:'127.0.0.1',remote_port:80,state:'Established',owning_pid:4100,creation_time:'2026-09-10T00:00:00Z'})]),
  snapshot('pcscope.network.firewall-rules',[item('示例规则',{name:'示例规则',direction:'Inbound',action:'Allow',enabled:true,profile:'Domain,Private'})]),
  // 安全与信任
  snapshot('pcscope.security.certificates.localmachine',[item('Root/0000000000000000000000000000000000000001',{store:'Root',location:'LocalMachine',thumbprint:'0000000000000000000000000000000000000001',subject:'CN=示例根证书',issuer:'CN=示例根证书',not_after:'2030-01-01T00:00:00Z',has_private_key:false,trust:'not evaluated'})]),
  snapshot('pcscope.security.ssh-public-keys',[item('C:\\示例\\用户\\.ssh\\id_ed25519.pub',{path:'C:\\示例\\用户\\.ssh\\id_ed25519.pub',algorithm:'ssh-ed25519',fingerprint:'SHA256:示例指纹占位',comment_collected:false,key_material_collected:false})]),
  // 硬件与驱动
  snapshot('pcscope.storage.disks',[item('0',{number:0,bus_type:'NVMe',partition_style:'GPT',size:512110190592,is_boot:true,is_system:true})]),
  snapshot('pcscope.hardware.driver-services',[item('示例驱动',{name:'示例驱动',driver_path:'C:\\Windows\\System32\\drivers\\示例.sys',service_type:'Kernel Driver',start_mode:'Manual',state:'Stopped'})]),
  // 虚拟环境
  snapshot('pcscope.virtualization.wsl-registration',[item('示例发行版',{name:'示例发行版',version:2,location:'C:\\示例\\wsl',default_uid:1000})]),
  // 桌面与应用集成
  snapshot('pcscope.desktop.printing',[item('示例打印机',{name:'示例打印机',driver:'示例打印驱动',port:'示例端口',shared:false,status:'Idle'})]),
  snapshot('pcscope.desktop.shortcuts',[item('C:\\示例\\桌面\\示例快捷方式.lnk',{path:'C:\\示例\\桌面\\示例快捷方式.lnk',target:'C:\\示例\\程序\\示例程序.exe',working_directory:'C:\\示例\\程序',arguments_protected:'--示例参数'})]),
];

// ---------- scenario 2: second complete round ----------
// Covers config change, first observation, disappearance, PID reuse and a same-named object in
// another scope. `run_id` changes so the receiver treats this as a new complete observation.
const changes=[
  snapshot('pcscope.startup.tasks',[
    // 配置变化：执行命令 A -> B
    item('\\示例文件夹\\示例任务',{name:'示例任务',path:'\\示例文件夹\\示例任务',enabled:true,principal:'示例用户',actions:[{execute:'C:\\示例\\程序\\示例程序.exe',arguments_protected:'--示例程序 B'}],triggers:[{type:'Daily',start_boundary:'2026-09-10T18:00:00Z'}]}),
    item('\\示例文件夹\\示例任务二',{name:'示例任务二',path:'\\示例文件夹\\示例任务二',enabled:false,principal:'SYSTEM',actions:[{execute:'C:\\示例\\程序\\示例工具.exe'}],triggers:[{type:'Boot'}]}),
    // 首次观察到的对象
    item('\\示例文件夹\\新任务',{name:'新任务',path:'\\示例文件夹\\新任务',enabled:true,principal:'SYSTEM',actions:[{execute:'C:\\示例\\程序\\示例服务.exe'}]}),
    // 示例任务三 本轮未出现：完整快照中缺失，用于验证“本次未发现”
  ],{observedAt:'2026-09-10T01:00:00Z',runId:'fixture-run-2'}),
  snapshot('pcscope.startup.tasks',[item('\\示例文件夹\\示例任务三',{name:'示例任务三',path:'\\示例文件夹\\示例任务三',enabled:true,principal:SID,actions:[{execute:'C:\\示例\\程序\\示例服务.exe'}]})],{observedAt:'2026-09-10T01:00:00Z',runId:'fixture-run-2',context:'task-detail'}),
  snapshot('pcscope.execution.processes',[
    // PID 复用：同一个 PID，创建时间不同，必须是两个实例
    item('4100/2026-09-10T01:00:00Z',{pid:4100,parent_pid:900,image:'C:\\示例\\程序\\示例程序.exe',session:1,created_at:'2026-09-10T01:00:00Z'}),
    // 4200 本轮未出现
    item('4300/2026-09-10T01:00:00Z',{pid:4300,parent_pid:4100,image:'C:\\示例\\程序\\示例工具.exe',session:1,created_at:'2026-09-10T01:00:00Z'}),
  ],{observedAt:'2026-09-10T01:00:00Z',runId:'fixture-run-2'}),
  // 同名不同作用域：与 01 中的任务同名但 scope 的 context 不同
  snapshot('pcscope.startup.services',[item('示例服务',{name:'示例服务',state:'Stopped',start_mode:'Auto',account:'LocalSystem'})],{observedAt:'2026-09-10T01:00:00Z',runId:'fixture-run-2',context:'alternate-scope'}),
];

// ---------- scenario 3: declared gaps ----------
const gaps=[
  gap('pcscope.security.encryption','Source unavailable; previous state retained: 访问被拒绝。','2026-09-10T01:00:00Z'),
  gap('pcscope.network.firewall-rules','Source unavailable; previous state retained: ActiveStore 读取被拒绝。','2026-09-10T01:00:00Z'),
];

// ---------- scenario 4: event batches ----------
// Kept separate so scenario 7 can replay this exact batch: a replay must be byte-identical in
// content, otherwise the receiver rejects it as conflicting data for the same batch sequence.
const securityEvents=[
  event('1001','2026-09-10T02:00:00Z','windows.event','4688',{provider:'Microsoft-Windows-Security-Auditing',event_id:4688,new_process_name_protected:'C:\\示例\\程序\\示例程序.exe',subject_user_name:USER,subject_user_sid:SID}),
  event('1002','2026-09-10T02:00:01Z','windows.event','4689',{provider:'Microsoft-Windows-Security-Auditing',event_id:4689,process_name_protected:'C:\\示例\\程序\\示例程序.exe',status:'0x0'}),
  event('1003','2026-09-10T02:05:00Z','windows.event','4624',{provider:'Microsoft-Windows-Security-Auditing',event_id:4624,target_user_name:USER,target_user_sid:SID,logon_type:2}),
  event('1004','2026-09-10T02:05:30Z','windows.event','4634',{provider:'Microsoft-Windows-Security-Auditing',event_id:4634,target_user_name:USER,target_user_sid:SID}),
  // 登录失败：值不得被读成成功
  event('1005','2026-09-10T02:06:00Z','windows.event','4625',{provider:'Microsoft-Windows-Security-Auditing',event_id:4625,target_user_name:USER,status:'0xC000006D',sub_status:'0xC000006A'}),
  // 账户建立：真实创建动作
  event('1006','2026-09-10T02:07:00Z','windows.event','4720',{provider:'Microsoft-Windows-Security-Auditing',event_id:4720,target_user_name:'示例新账户',target_user_sid:SID2}),
  // 特权分配
  event('1007','2026-09-10T02:08:00Z','windows.event','4672',{provider:'Microsoft-Windows-Security-Auditing',event_id:4672,subject_user_name:'SYSTEM'}),
];
const events=[
  eventBatch('pcscope.eventlog.security',securityEvents,{epoch:'security-epoch-1',batchSeq:1,checkpoint:'security-cp-1'}),
  eventBatch('pcscope.eventlog.system',[
    event('2001','2026-09-10T02:10:00Z','windows.event','7036',{provider:'Service Control Manager',event_id:7036,param1:'示例服务',param2:'正在运行'}),
    event('2002','2026-09-10T02:11:00Z','windows.event','12',{provider:'Microsoft-Windows-Kernel-General',event_id:12}),
  ],{epoch:'system-epoch-1',batchSeq:1,checkpoint:'system-cp-1'}),
  eventBatch('pcscope.eventlog.microsoft-windows-taskscheduler/operational',[
    event('3001','2026-09-10T02:15:00Z','windows.event','100',{provider:'Microsoft-Windows-TaskScheduler',event_id:100,task_name:'\\示例文件夹\\示例任务'}),
    event('3002','2026-09-10T02:15:05Z','windows.event','102',{provider:'Microsoft-Windows-TaskScheduler',event_id:102,task_name:'\\示例文件夹\\示例任务',result_code:0}),
  ],{epoch:'task-epoch-1',batchSeq:1,checkpoint:'task-cp-1'}),
  // 缺字段事件：data 极简，用于验证缺失显示而不是猜测
  eventBatch('pcscope.eventlog.application',[
    event('4001','2026-09-10T02:20:00Z','windows.event','1000',{provider:'Application Error'}),
    // 无法确定目标对象的事件：object_id 保留源标识
    event('4002','2026-09-10T02:21:00Z','windows.event','未归属',{provider:'示例未知提供程序',event_id:9999}),
  ],{epoch:'app-epoch-1',batchSeq:1,checkpoint:'app-cp-1'}),
];

// ---------- scenario 5: USN ----------
const usn=[
  snapshot('pcscope.usn.volumes',[item('C:',{volume:'\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\',filesystem:'NTFS',eligible:true,scope:'fixed drives with drive letters only'})]),
  snapshot('pcscope.usn.watch-list',[item('configuration',{files:['C:\\示例\\监控\\示例文件.txt'],revision:1,active:true})],{context:'explicit-files/v1'}),
  eventBatch('pcscope.usn.c',[
    event('9001','2026-09-10T02:30:00Z','files.usn_changed','c:\\示例\\监控\\示例文件.txt',{volume:'\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\',drive:'C:',journal_id:'00000000000000c0',usn:9001,file_id:'0000000000000abc',name:'示例文件.txt',watch_path:'C:\\示例\\监控\\示例文件.txt',reason_mask:1,record_version:2,full_path_resolved:false}),
    event('9002','2026-09-10T02:31:00Z','files.usn_changed','c:\\示例\\监控\\示例文件.txt',{volume:'\\\\?\\Volume{00000000-0000-0000-0000-0000000000c0}\\',drive:'C:',journal_id:'00000000000000c0',usn:9002,file_id:'0000000000000abc',name:'示例文件.txt',watch_path:'C:\\示例\\监控\\示例文件.txt',reason_mask:2,record_version:2,full_path_resolved:false}),
  ],{epoch:'usn-epoch-1',batchSeq:1,checkpoint:'usn-cp-1'}),
];

// ---------- scenario 6: field edge cases ----------
const edge=[
  snapshot('pcscope.identity.accounts',[
    // 缺字段：只有身份，没有其他属性
    item(SID2,{}),
    // 显式 null 与不存在的区别
    item('S-1-5-18',{name:'SYSTEM',enabled:true,last_logon:null,description_present:false}),
  ],{observedAt:'2026-09-10T03:00:00Z',runId:'fixture-run-3'}),
  snapshot('pcscope.desktop.shortcuts',[
    // 空列表、零值、超长值与 Unicode
    item('C:\\示例\\桌面\\边界快捷方式.lnk',{path:'C:\\示例\\桌面\\边界快捷方式.lnk',target:null,working_directory:'',arguments_protected:'',hotkey:'',item_count:0,flags:[],note:'示例说明'.repeat(40),unicode_name:'示例名称 🧪 café 日本語'}),
  ],{observedAt:'2026-09-10T03:00:00Z',runId:'fixture-run-3'}),
];

// ---------- scenario 7: replay and late arrival ----------
const replay=[
  // 重复投递同一批次（内容逐字相同）：必须幂等，不新增记录
  {protocol_version:1,request_id:'fixture-replay-0001',type:'event_batch',source:'pcscope.eventlog.security',epoch:'security-epoch-1',batch_seq:1,checkpoint:'security-cp-1',events:securityEvents},
  // 迟到的旧事件：occurred_at 早于已入库记录，不得回退当前状态
  eventBatch('pcscope.eventlog.security',[
    event('5001','2026-09-09T23:00:00Z','windows.event','4688',{provider:'Microsoft-Windows-Security-Auditing',event_id:4688,new_process_name_protected:'C:\\示例\\程序\\迟到程序.exe',subject_user_name:USER,subject_user_sid:SID}),
  ],{epoch:'security-epoch-1',batchSeq:2,checkpoint:'security-cp-2'}),
];

// ---------- invalid inputs (must be rejected; never loaded by the baseline) ----------
const invalid={
  'incomplete-snapshot.jsonl':[`${JSON.stringify(snapshot('pcscope.software',[item('x',{display_name:'不完整'})],{complete:false}))}\n`],
  'duplicate-identity.jsonl':[`${JSON.stringify(snapshot('pcscope.identity.accounts',[item(SID,{name:USER}),item(SID,{name:USER})]))}\n`],
  'wrong-protocol-version.jsonl':[`${JSON.stringify({...snapshot('pcscope.software',[item('x',{})]),protocol_version:2})}\n`],
  'empty-source.jsonl':[`${JSON.stringify({...snapshot('pcscope.software',[]),source:''})}\n`],
  'unknown-event-field.jsonl':[`${JSON.stringify(eventBatch('pcscope.eventlog.system',[{...event('1','2026-09-10T00:00:00Z','windows.event','1',{}),unexpected_field:true}]))}\n`],
  'zero-batch-seq.jsonl':[`${JSON.stringify(eventBatch('pcscope.eventlog.system',[event('1','2026-09-10T00:00:00Z','windows.event','1',{})],{batchSeq:0}))}\n`],
};

// ---------- write ----------
const files={
  '01-baseline.jsonl':baseline,
  '02-changes.jsonl':changes,
  '03-gaps.jsonl':gaps,
  '04-events.jsonl':events,
  '05-usn.jsonl':usn,
  '06-edge-cases.jsonl':edge,
  '07-replay-and-late.jsonl':replay,
};
fs.mkdirSync(invalidDir,{recursive:true});
const manifest={generated_by:'scripts/build-fixtures.cjs',scenarios:[],invalid:[]};
for(const [name,messages] of Object.entries(files)){
  const lines=messages.map(message=>JSON.stringify(message)).join('\n')+'\n';
  fs.writeFileSync(path.join(outDir,name),lines);
  const kinds={};
  for(const message of messages)kinds[message.type]=(kinds[message.type]||0)+1;
  const sources=[...new Set(messages.map(message=>message.source))].sort();
  manifest.scenarios.push({file:name,messages:messages.length,kinds,sources,bytes:Buffer.byteLength(lines)});
}
for(const [name,lines] of Object.entries(invalid)){
  fs.writeFileSync(path.join(invalidDir,name),lines.join(''));
  manifest.invalid.push({file:`invalid/${name}`,expectation:'必须被协议校验或入库校验拒绝，不得写入任何记录'});
}
fs.writeFileSync(path.join(outDir,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');

// ---------- optional real-scale corpus ----------
// --scale builds a deterministic corpus at the size recorded in collector-coverage.md
// (~15.6k current objects / ~36k retained records) so query timings are measured at real scale
// rather than at the size of the readable base corpus. Same anonymised data rules apply.
const scaled={scenarios:[],sources:0,objects:0};
if(process.argv.includes('--scale')){
  const scaledDir=path.join(outDir,'scaled');
  fs.rmSync(scaledDir,{recursive:true,force:true});
  fs.mkdirSync(scaledDir,{recursive:true});
  const PER_SOURCE=1800, SHARDS=4, CHANGE_EVERY=10;
  const scaleSources=[
    ['pcscope.identity.accounts',i=>({name:`示例账户 ${i}`,enabled:i%7!==0,last_logon:null,password_required:true})],
    ['pcscope.software',i=>({display_name:`示例软件 ${i}`,display_version:`${1+i%9}.${i%10}.0`,publisher:'示例软件公司'})],
    ['pcscope.execution.processes',i=>({pid:5000+i,parent_pid:4000+i%50,image:`C:\\示例\\程序\\示例程序${i%20}.exe`,session:1,created_at:'2026-09-10T00:00:00Z'})],
    ['pcscope.startup.tasks',i=>({name:`示例任务 ${i}`,path:`\\示例文件夹\\示例任务 ${i}`,enabled:i%3!==0,principal:'示例用户',actions:[{execute:`C:\\示例\\程序\\示例程序${i%20}.exe`}]})],
    ['pcscope.startup.services',i=>({name:`示例服务 ${i}`,state:i%2?'Running':'Stopped',start_mode:'Auto',account:'LocalSystem'})],
    ['pcscope.storage.disks',i=>({number:i,bus_type:'NVMe',partition_style:'GPT',size:512110190592+i,is_boot:i===0,is_system:i===0})],
    ['pcscope.network.tcp',i=>({local_address:'127.0.0.1',local_port:20000+i,remote_address:'127.0.0.1',remote_port:80+i%1000,state:'Established',owning_pid:5000+i%200})],
    ['pcscope.desktop.shortcuts',i=>({path:`C:\\示例\\桌面\\示例快捷方式 ${i}.lnk`,target:`C:\\示例\\程序\\示例程序${i%20}.exe`,working_directory:'C:\\示例\\程序'})],
  ];
  const rowsFor=(source,make,round)=>{
    const rows=[];
    const prefix=source.split('.').pop();
    for(let i=0;i<PER_SOURCE;i++){
      const data=make(i);
      // Round 2 changes one object in ten. Every object is still reported, because a complete
      // snapshot reports the whole inventory: absence must stay a real signal, not a corpus
      // artefact of sending only the changed subset.
      if(round===2&&i%CHANGE_EVERY===0){
        const firstKey=Object.keys(data)[0];
        data[firstKey]=typeof data[firstKey]==='boolean'?!data[firstKey]:`${data[firstKey]}·r2`;
      }
      rows.push({shard:(i*7+3)%SHARDS,id:`${prefix}-${i}`,data});
    }
    return rows;
  };
  const emitRound=(round,filename)=>{
    const messages=[];
    for(const [source,make] of scaleSources){
      const rows=rowsFor(source,make,round);
      for(let shard=0;shard<SHARDS;shard++){
        const items=rows.filter(r=>r.shard===shard).map(r=>({id:r.id,data:r.data}));
        if(!items.length)continue;
        messages.push({protocol_version:1,request_id:`scale-${round}-${source}-${shard}`,type:'snapshot',source,
          scope:`machine:${MACHINE}/user:${SID}/schema:1/context:/shard:${shard}`,
          run_id:`scale-run-${round}`,observed_at:`2026-09-10T0${round}:00:00Z`,complete:true,items});
      }
      if(round===1)scaled.objects+=rows.length;
    }
    const lines=messages.map(m=>JSON.stringify(m)).join('\n')+'\n';
    fs.writeFileSync(path.join(scaledDir,filename),lines);
    scaled.scenarios.push({file:`scaled/${filename}`,messages:messages.length,bytes:Buffer.byteLength(lines),round});
  };
  emitRound(1,'10-scale-baseline.jsonl');
  emitRound(2,'11-scale-changes.jsonl');
  // Event history at comparable size.
  const channels=['pcscope.eventlog.security','pcscope.eventlog.system','pcscope.eventlog.application','pcscope.eventlog.microsoft-windows-taskscheduler/operational'];
  const EVENT_TOTAL=20000, BATCH=500;
  const batches=EVENT_TOTAL/BATCH, perChannel=batches/channels.length;
  const messages=[];
  channels.forEach((source,channelIndex)=>{
    for(let batch=0;batch<perChannel;batch++){
      const events=[];
      for(let n=0;n<BATCH;n++){
        const i=channelIndex*perChannel*BATCH+batch*BATCH+n;
        events.push(event(String(7000000+i),`2026-09-10T04:00:${String(i%60).padStart(2,'0')}Z`,'windows.event',String(4688+(i%20)),
          {provider:'Microsoft-Windows-Security-Auditing',event_id:4688+(i%20),subject_user_name:USER,subject_user_sid:SID,noise_payload:'示例填充'.repeat(6)}));
      }
      messages.push(eventBatch(source,events,{epoch:`scale-${channelIndex}`,batchSeq:batch+1,checkpoint:`scale-${channelIndex}-${batch+1}`}));
    }
  });
  const lines=messages.map(m=>JSON.stringify(m)).join('\n')+'\n';
  fs.writeFileSync(path.join(scaledDir,'12-scale-events.jsonl'),lines);
  scaled.scenarios.push({file:'scaled/12-scale-events.jsonl',messages:messages.length,bytes:Buffer.byteLength(lines),events:EVENT_TOTAL});
  scaled.sources=scaleSources.length;
  fs.writeFileSync(path.join(scaledDir,'manifest.json'),JSON.stringify({generated_by:'scripts/build-fixtures.cjs --scale',per_source:PER_SOURCE,shards:SHARDS,change_every:CHANGE_EVERY,scenarios:scaled.scenarios},null,2)+'\n');
}

console.log(JSON.stringify({
  scenarios:manifest.scenarios.map(s=>({file:s.file,messages:s.messages,bytes:s.bytes})),
  invalid:manifest.invalid.length,
  total:manifest.scenarios.reduce((n,s)=>n+s.messages,0),
  ...(scaled.scenarios.length?{scaled:{sources:scaled.sources,objects:scaled.objects,scenarios:scaled.scenarios}}:{})
},null,2));
