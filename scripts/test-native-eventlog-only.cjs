'use strict';
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),assert=require('node:assert/strict');
const directory=path.resolve(process.argv[2]||'');assert(process.argv[2],'Explicit detached test directory required');
assert(!fs.existsSync(path.join(directory,'collect-eventlog.ps1')),'Test must not contain the legacy script');
fs.writeFileSync(path.join(directory,'eventlog-backend.json'),JSON.stringify({backend:'powershell'}));
function run(request){const result=cp.spawnSync(path.join(directory,'pcscope-eventlog.exe'),[],{input:JSON.stringify(request),encoding:'utf8',windowsHide:true,timeout:40000,maxBuffer:8*1024*1024});assert.ifError(result.error);assert.equal(result.status,0,result.stderr);const rows=result.stdout.split(/\r?\n/).filter(Boolean).map(JSON.parse);return rows.find(row=>row.type==='event_batch');}
const first=run({channel:'Application'});assert(first,'Native reader must ignore a stale PowerShell backend override');
const second=run({channel:'Application',resume:{epoch:first.epoch,batch_seq:first.batch_seq,checkpoint:first.checkpoint}});assert(second);assert.equal(second.epoch,first.epoch);assert.equal(second.batch_seq,first.batch_seq+1);
const ids=new Set(first.events.map(event=>event.id));assert(second.events.every(event=>!ids.has(event.id)),'Native bookmark must not repeat events');
const missing=path.join(directory,'missing-native');fs.mkdirSync(missing);fs.copyFileSync(path.join(directory,'pcscope-eventlog.exe'),path.join(missing,'pcscope-eventlog.exe'));
const failed=cp.spawnSync(path.join(missing,'pcscope-eventlog.exe'),[],{input:JSON.stringify({channel:'Application'}),encoding:'utf8',windowsHide:true,timeout:10000});assert.ifError(failed.error);assert.notEqual(failed.status,0);assert(!failed.stderr.includes('collect-eventlog.ps1'),'Failure must not attempt a legacy script');
const report={stale_powershell_override:'native route',native_bookmark_resume:'passed',duplicate_events:0,missing_native:'explicit failure; no script fallback',legacy_script_present:false,first_events:first.events.length,second_events:second.events.length};
fs.writeFileSync(path.join(directory,'summary.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
