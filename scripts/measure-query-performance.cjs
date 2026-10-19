'use strict';
// Read-only real IPC benchmark. No collection, settings, process launch, or shutdown requests.
const fs=require('node:fs'),path=require('node:path'),net=require('node:net'),assert=require('node:assert/strict');
const arg=(name,fallback)=>{const i=process.argv.indexOf('--'+name);return i<0?fallback:process.argv[i+1];};
const pipe=arg('pipe'),samples=Number(arg('samples','100')),threshold=Number(arg('max-p95-ms','500'));
const listCounts=arg('list-counts','objects');assert.ok(['objects','full'].includes(listCounts),'list-counts must be objects or full');
const firstCounts=listCounts==='objects'?{include_counts:false,include_instance_total:true}:{include_counts:true};
assert.ok(pipe?.startsWith('\\\\.\\pipe\\PCScope.native.v1.'),'provide --pipe for the selected PCScope instance');
assert.ok(Number.isInteger(samples)&&samples>=1&&samples<=1000,'samples must be 1..1000');
const selected=arg('cases')?.split(','),percentile=(values,p)=>values[Math.max(0,Math.ceil(values.length*p)-1)];
function call(request){return new Promise((resolve,reject)=>{
 const start=performance.now(),socket=net.createConnection(pipe);let body='',done=false;socket.setEncoding('utf8');socket.setTimeout(10000,()=>socket.destroy(Error('IPC timeout')));
 socket.on('connect',()=>socket.write(JSON.stringify(request)+'\n'));
 socket.on('data',chunk=>{body+=chunk;if(Buffer.byteLength(body)>16*1024*1024)return socket.destroy(Error('response limit exceeded'));
  const end=body.indexOf('\n');if(end<0)return;
  try{const reply=JSON.parse(body.slice(0,end));done=true;socket.destroy();if(reply.ok!==true)throw Error(reply.error);resolve({data:reply.data,ms:performance.now()-start,bytes:Buffer.byteLength(body)});}catch(error){done=true;socket.destroy();reject(error);}
 });socket.on('error',reject);socket.on('close',()=>{if(!done)reject(Error('IPC closed before a complete reply'));});
});}
(async()=>{
 const before=(await call({operation:'status'})).data;
 const first=(await call({operation:'query',request:{query:'instance_list',limit:100,...firstCounts}})).data;
 const timeline=(await call({operation:'query',request:{query:'action_timeline',limit:200}})).data;
 const detail=first.data?.[0]?.instance_id;assert.ok(detail,'no object available for detail case');
 const cases=[
  ['objects_first',{query:'instance_list',limit:100,...firstCounts}],
  ['objects_second',first.page?.next_cursor?{query:'instance_list',limit:100,include_counts:false,cursor:first.page.next_cursor}:null],
  ['timeline_first',{query:'action_timeline',limit:200}],
  ['timeline_second',timeline.page?.next_cursor?{query:'action_timeline',limit:200,cursor:timeline.page.next_cursor}:null],
  ['object_detail',{query:'instance_detail',instance:String(detail)}],
  ['progress',{query:'projection_status'}],
  ['objects_network',{query:'instance_list',category:'network',limit:100,...firstCounts}],
  ['objects_processes',{query:'instance_list',type_id:'runtime.process',limit:100,...firstCounts}],
  ['objects_recent',{query:'instance_list',sort:'changed_desc',limit:100,...firstCounts}],
  ['objects_search',{query:'instance_list',search:'Windows',limit:100,...firstCounts}],
  ['timeline_routine',{query:'action_timeline',include_routine:true,limit:200}],
 ];
 const results=[];
 for(const [name,request] of cases){
  if(selected&&!selected.includes(name))continue;
  if(!request){results.push({case:name,passed:false,reason:'second page unavailable'});continue;}
  const elapsed=[],errors=[];let bytes=0;
  for(let i=0;i<samples;i++){
   try{const reply=await call({operation:'query',request});assert.ok(['loaded','empty'].includes(reply.data.status),'unexpected query status '+reply.data.status);if(request.include_instance_total)assert.ok(Number.isSafeInteger(reply.data.instance_total)&&reply.data.instance_total>=0,'exact object total missing');elapsed.push(reply.ms);bytes+=reply.bytes;}
   catch(error){errors.push(error.message);if(errors.length>=3)break;}
  }
  elapsed.sort((a,b)=>a-b);
  const result={case:name,samples:elapsed.length,errors:errors.length,error_details:[...new Set(errors)],p50_ms:elapsed.length?percentile(elapsed,.5):null,p95_ms:elapsed.length?percentile(elapsed,.95):null,p99_ms:elapsed.length?percentile(elapsed,.99):null,mean_bytes:elapsed.length?Math.round(bytes/elapsed.length):null};
  result.passed=result.samples===samples&&result.errors===0&&result.p95_ms<=threshold;results.push(result);console.log(JSON.stringify(result));
 }
 const after=(await call({operation:'status'})).data;
 const report={started_pid:before.pid,finished_pid:after.pid,server_changed:before.pid!==after.pid,list_counts:listCounts,initial_revision:before.read_model_revision,final_revision:after.read_model_revision,initial_counts:before.counts,final_counts:after.counts,includes_ipc:true,samples_per_case:samples,max_p95_ms:threshold,results,passed:before.pid===after.pid&&results.length>0&&results.every(result=>result.passed),at:new Date().toISOString()};
 const output=arg('output');if(output){const resolved=path.resolve(output),root=fs.realpathSync(path.resolve(__dirname,'../target')),parent=fs.realpathSync(path.dirname(resolved));assert.ok(parent===root||parent.startsWith(root+path.sep),'output must stay under the existing workspace target directory');const temporary=resolved+'.'+require('node:crypto').randomUUID()+'.pending';fs.writeFileSync(temporary,JSON.stringify(report,null,2),{flag:'wx'});fs.renameSync(temporary,resolved);}
 console.log(JSON.stringify({passed:report.passed,server_changed:report.server_changed,failed_cases:results.filter(result=>!result.passed).map(result=>result.case),samples_per_case:samples,includes_ipc:true}));
 if(!report.passed)process.exitCode=2;
})().catch(error=>{console.error(error.message);process.exitCode=1;});
