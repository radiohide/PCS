'use strict';
// Read-only reference queries and loopback fixtures; no device/configuration changes.
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),assert=require('node:assert/strict'),net=require('node:net'),dgram=require('node:dgram');
const root=path.resolve(__dirname,'..'),inventory=require('../model/sources.json');
const bin=process.env.PCSCOPE_NATIVE_NETWORK_BIN||path.join(root,'target/native-network-test-bin');
const output=path.join(root,'target/native-network-tests',Date.now().toString());fs.mkdirSync(output,{recursive:true});
const command=`$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);
function Iso($v){if($null -ne $v){([datetime]$v).ToUniversalTime().ToString('o')}}
function Fields($row,$names){$d=@{};foreach($name in $names){$v=$row.$name;if($v -is [datetime]){$v=Iso $v};$d[$name]=$v};$raw=@{};foreach($p in $row.CimInstanceProperties){$v=$p.Value;if($v -is [datetime]){$v=Iso $v};$raw[$p.Name]=$v};$d.source_properties=$raw;return $d}
$r=@{};
$r.addresses=@(Get-NetIPAddress|ForEach-Object {@{id="$($_.InterfaceIndex)/$($_.AddressFamily)/$($_.IPAddress)";data=@{interface_index=$_.InterfaceIndex;interface_alias=$_.InterfaceAlias;address=$_.IPAddress;prefix_length=$_.PrefixLength;address_family=[string]$_.AddressFamily;prefix_origin=[string]$_.PrefixOrigin;suffix_origin=[string]$_.SuffixOrigin;address_state=[string]$_.AddressState}}});
$r.routes=@(Get-NetRoute|ForEach-Object {@{id="$($_.InterfaceIndex)/$($_.DestinationPrefix)/$($_.NextHop)/$($_.PolicyStore)";data=(Fields $_ @('InterfaceIndex','InterfaceAlias','DestinationPrefix','NextHop','RouteMetric','Protocol','AddressFamily','PolicyStore','Publish'))}});
$r.tcp=@(Get-NetTCPConnection|ForEach-Object {@{id="$($_.LocalAddress)/$($_.LocalPort)/$($_.RemoteAddress)/$($_.RemotePort)/$($_.OwningProcess)/$(Iso $_.CreationTime)";data=(Fields $_ @('LocalAddress','LocalPort','RemoteAddress','RemotePort','State','OwningProcess','CreationTime','AppliedSetting'))}});
$r.udp=@(Get-NetUDPEndpoint|ForEach-Object {@{id="$($_.LocalAddress)/$($_.LocalPort)/$($_.OwningProcess)/$(Iso $_.CreationTime)";data=(Fields $_ @('LocalAddress','LocalPort','OwningProcess','CreationTime'))}});
$r|ConvertTo-Json -Depth 18 -Compress`;
function run(file,args,input){const began=performance.now(),p=cp.spawnSync(file,args,{input,encoding:'utf8',windowsHide:true,timeout:55000,maxBuffer:32*1024*1024});assert.ifError(p.error);assert.equal(p.status,0,p.stderr);return {text:p.stdout,elapsed_ms:Math.round(performance.now()-began)};}
function collect(unit,selected,wrapper){const request={selection:{disabled_sources:inventory.sources.map(s=>s.source).filter(s=>!selected.includes(s))}};const result=run(path.join(bin,wrapper?'pcscope-powershell-adapter.exe':'pcscope-platform-native.exe'),unit==='base'&&wrapper?[]:[unit],JSON.stringify(request));const rows=result.text.trim().split(/\r?\n/).filter(Boolean).map(JSON.parse);for(const source of selected){const messages=rows.filter(r=>r.source===source);assert.equal(messages.filter(r=>r.type==='snapshot').length,unit==='base'?1:16,JSON.stringify(messages.filter(r=>r.type==='gap')));for(const m of messages)assert(m.complete);assert.equal(new Set(messages.flatMap(m=>m.items).map(i=>i.id)).size,messages.flatMap(m=>m.items).length);}assert(rows.every(r=>selected.includes(r.source)));fs.writeFileSync(path.join(output,unit+'.jsonl'),result.text);return {...result,rows};}
const servers=[],sockets=[],udps=[],fixtures=[];
async function fixturesFor(address,family){
 const server=net.createServer(socket=>sockets.push(socket));servers.push(server);
 await new Promise((resolve,reject)=>{server.once('error',reject);server.listen({host:address,port:0},resolve)});
 const accepted=new Promise(resolve=>server.once('connection',resolve));const port=server.address().port;
 const client=net.createConnection({host:address,port});sockets.push(client);await new Promise((resolve,reject)=>{client.once('connect',resolve);client.once('error',reject)});await accepted;
 const udp=dgram.createSocket(family);udps.push(udp);await new Promise((resolve,reject)=>{udp.once('error',reject);udp.bind(0,address,resolve)});
 fixtures.push({address,tcp_port:port,udp_port:udp.address().port});
}
async function main(){
 try {
  await fixturesFor('127.0.0.1','udp4');await fixturesFor('::1','udp6');
  const wrapper=process.argv.includes('--wrapper');
  const base=collect('base',['pcscope.network.addresses'],wrapper),network=collect('network',['pcscope.network.routes','pcscope.network.tcp','pcscope.network.udp'],wrapper);
  const reference=run(path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),['-NoLogo','-NoProfile','-NonInteractive','-Command',command]);
  const refs=JSON.parse(reference.text);fs.writeFileSync(path.join(output,'reference.json'),reference.text);
  const report=[];
  for(const name of ['addresses','routes','tcp','udp']){
   const source='pcscope.network.'+name,rows=[...base.rows,...network.rows].filter(r=>r.source===source),items=rows.flatMap(r=>r.items),byId=new Map(items.map(i=>[i.id,i]));let selected=refs[name];
   if(name==='addresses'||name==='routes'){assert.equal(items.length,selected.length,name);assert(selected.every(row=>byId.has(row.id)),name+' identity set');}
   else {selected=selected.filter(row=>row.data.OwningProcess===process.pid);assert(selected.length>=2,name+' fixtures missing');}
   for(const row of selected){const actual=byId.get(row.id);assert(actual,name+' missing creation-time identity');
    for(const [key,value] of Object.entries(row.data)){
     if(key==='source_properties'){
      assert.deepEqual(Object.keys(actual.data[key]).sort(),Object.keys(value).sort(),name+' raw property set');
      for(const rawKey of Object.keys(value))if(!['ValidLifetime','PreferredLifetime','State','OffloadState'].includes(rawKey))assert.deepEqual(actual.data[key][rawKey],value[rawKey],name+'/raw/'+rawKey);
     }else if(key!=='State')assert.deepEqual(actual.data[key],value,name+'/'+key);
    }
   }
   if(name==='tcp'||name==='udp')for(const fixture of fixtures){const port=name==='tcp'?fixture.tcp_port:fixture.udp_port;assert(selected.some(r=>r.data.LocalAddress===fixture.address&&r.data.LocalPort===port),name+' '+fixture.address+' fixture');}
   report.push({source,total_items:items.length,compared_items:selected.length,identity:name==='tcp'||name==='udp'?'controlled IPv4/IPv6 including creation time':'complete set',fields:'equal',volatile:'State/OffloadState and lifetimes excluded from across-time comparison'});
  }
  const summary={output,wrapper,no_legacy_scripts:!fs.existsSync(path.join(bin,'collect-domain.ps1'))&&!fs.existsSync(path.join(bin,'collect-system.ps1')),base_ms:base.elapsed_ms,network_ms:network.elapsed_ms,reference_ms:reference.elapsed_ms,sources:report};fs.writeFileSync(path.join(output,'summary.json'),JSON.stringify(summary,null,2));console.log(JSON.stringify(summary));
 }finally{for(const socket of sockets)socket.destroy();for(const udp of udps)udp.close();await Promise.all(servers.map(server=>new Promise(resolve=>server.close(resolve))));}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
