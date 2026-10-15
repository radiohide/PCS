'use strict';
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),net=require('node:net'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'..'),inventory=require('../model/sources.json'),output=path.join(root,'target/native-docker-tests',Date.now().toString()),bin=path.join(output,'bin');fs.mkdirSync(bin,{recursive:true});
for(const name of ['pcscope-platform-native.exe','pcscope-powershell-adapter.exe']){const file=path.join(bin,name);fs.copyFileSync(path.join(root,'target/native-network-test-bin',name),file);cp.execFileSync('icacls.exe',[file,'/setintegritylevel','M'],{windowsHide:true,timeout:5000,stdio:'pipe'});}
const pipe='pcscope-docker-test-'+crypto.randomBytes(16).toString('hex'),endpoint='\\\\.\\pipe\\'+pipe;
const fixtures={
 '/version':{ApiVersion:'1.51',MinAPIVersion:'1.24'},
 '/v1.51/containers/json?all=1':[{Id:'container-full-id',Names:['/测试容器','/other/link'],Image:'example:latest',State:'running',Status:'Up 2 hours',Command:'private-command',Labels:{secret:'never collect'}}],
 '/v1.51/images/json':[{Id:'sha256:one',RepoTags:['docker.io/library/example:latest','example:stable','registry:5000/team/image:v1'],RepoDigests:['example@sha256:abc','other@sha256:def']},{Id:'sha256:two',RepoTags:null,RepoDigests:[]},{Id:'sha256:three',RepoTags:['<none>:<none>'],RepoDigests:['<none>@<none>']}],
 '/v1.51/volumes':{Volumes:[{Name:'volume-one',Driver:'local',Mountpoint:'never collect'}],Warnings:null},
 '/v1.51/networks':[{Id:'network-one',Name:'bridge',Driver:'bridge',Scope:'local',Options:{secret:'never collect'}}]
};
let mode='normal';const routes=[],sockets=new Set();
const server=net.createServer(socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket));socket.on('error',()=>{});let data='';socket.on('data',bytes=>{data+=bytes.toString('ascii');if(!data.includes('\r\n\r\n'))return;socket.removeAllListeners('data');const route=data.split(' ')[1];routes.push(route);assert(data.startsWith('GET '));assert(!data.includes('Authorization:'));
 if(mode==='timeout')return;
 if(mode==='reject'&&route.endsWith('/networks')){socket.end('HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n');return;}
 if(mode==='large'){socket.end('HTTP/1.1 200 OK\r\nContent-Length: 8388609\r\n\r\n');return;}
 if(mode==='truncated'){socket.end('HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{}');return;}
 let value=fixtures[route];assert(value,'unexpected request '+route);if(mode==='incompatible'&&route==='/version')value={ApiVersion:'1.99',MinAPIVersion:'1.60'};if(mode==='missing-volume'&&route.endsWith('/volumes'))value={};
 const body=Buffer.from(mode==='invalid-json'?'private-secret{':JSON.stringify(value));
 if(mode==='chunked'){socket.write('HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n');for(let offset=0;offset<body.length;offset+=7){const chunk=body.subarray(offset,offset+7);socket.write(chunk.length.toString(16)+'\r\n');socket.write(chunk);socket.write('\r\n');}socket.end('0\r\n\r\n');}
 else socket.end(Buffer.concat([Buffer.from('HTTP/1.1 200 OK\r\nContent-Length: '+body.length+'\r\n\r\n'),body]));
 });});
function run(exe,args,input){return new Promise((resolve,reject)=>{const began=performance.now(),p=cp.spawn(exe,args,{windowsHide:true,stdio:'pipe'});let out='',err='';const timer=setTimeout(()=>{p.kill();reject(Error('Docker test exceeded process deadline'));},15000);p.stdout.on('data',v=>out+=v);p.stderr.on('data',v=>err+=v);p.on('error',reject);p.on('close',code=>{clearTimeout(timer);if(code)reject(Error(err));else resolve({text:out,elapsed_ms:Math.round(performance.now()-began)});});p.stdin.end(input);});}
const selected='pcscope.virtualization.docker',request=JSON.stringify({selection:{disabled_sources:inventory.sources.map(s=>s.source).filter(s=>s!==selected)}});
function item(kind,id,data){return {id:kind+'/'+id,data:{kind,data,basis:'local Docker API metadata; no logs, arguments, environment or container contents'}};}
const expected=[item('container','container-full-id',{id:'container-full-id',names:'测试容器,other/link',image:'example:latest',state:'running',status:'Up 2 hours'}),...['latest','stable'].map(tag=>item('image','sha256:one/example/'+tag,{id:'sha256:one',repository:'example',tag})),item('image','sha256:one/registry:5000/team/image/v1',{id:'sha256:one',repository:'registry:5000/team/image',tag:'v1'}),item('image','sha256:one/other/<none>',{id:'sha256:one',repository:'other',tag:'<none>'}),...['two','three'].map(id=>item('image','sha256:'+id+'/<none>/<none>',{id:'sha256:'+id,repository:'<none>',tag:'<none>'})),item('volume','volume-one',{id:'volume-one',driver:'local'}),item('network','network-one',{id:'network-one',name:'bridge',driver:'bridge',scope:'local'})];
async function main(){await new Promise(resolve=>server.listen(endpoint,resolve));const reports=[];try{
 const options={docker_metadata:true,mihomo_metadata:false,portable_roots:[],module_process_names:[],max_files_per_root:100,max_hash_bytes:16777216,hash_files:false};fs.writeFileSync(path.join(bin,'collector-options.json'),JSON.stringify(options));const exe=path.join(bin,'pcscope-platform-native.exe');
 for(mode of ['normal','chunked','reject','large','truncated','timeout','incompatible','invalid-json','missing-volume']){const result=await run(exe,['docker-fixture',pipe],request),messages=result.text.trim().split(/\r?\n/).filter(Boolean).map(JSON.parse);fs.writeFileSync(path.join(output,mode+'.jsonl'),result.text);
  if(['normal','chunked'].includes(mode)){assert.equal(messages.length,16);assert(messages.every(m=>m.type==='snapshot'&&m.complete&&m.source===selected&&m.scope.includes('/context:/shard:')));const items=messages.flatMap(m=>m.items).map(({id,data})=>({id,data})).sort((a,b)=>a.id.localeCompare(b.id));assert.deepEqual(items,expected.toSorted((a,b)=>a.id.localeCompare(b.id)));assert(!result.text.includes('never collect')&&!result.text.includes('private-command'));}
  else {assert.equal(messages.length,1);assert.equal(messages[0].type,'gap');assert(!result.text.includes('private-secret'));}
  reports.push({case:mode,status:'passed',elapsed_ms:result.elapsed_ms});
 }
 options.docker_metadata=false;fs.writeFileSync(path.join(bin,'collector-options.json'),JSON.stringify(options));const count=routes.length;assert.equal((await run(exe,['docker-fixture',pipe],request)).text,'');assert.equal(routes.length,count);
 if(process.argv.includes('--wrapper'))assert.equal((await run(path.join(bin,'pcscope-powershell-adapter.exe'),['scoped'],request)).text,'');
 const summary={output,fixture_items:expected.length,fields:'independent expected records match',disabled:'no requests; wrapper checked when requested',production_daemon:'not installed; successful real-daemon acceptance remains open',no_legacy_scripts:true,cases:reports};fs.writeFileSync(path.join(output,'summary.json'),JSON.stringify(summary,null,2));console.log(JSON.stringify(summary));
 }finally{for(const socket of sockets)socket.destroy();await new Promise(resolve=>server.close(resolve));}}
main().catch(error=>{console.error(error);process.exitCode=1;});
