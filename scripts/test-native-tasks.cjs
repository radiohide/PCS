'use strict';
// Bounded, read-only comparison. Never creates, starts or modifies a task.
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'..'),inventory=require('../model/sources.json');
const bin=process.env.PCSCOPE_NATIVE_TASK_BIN||path.join(root,'target/native-network-test-bin');
const output=path.join(root,'target/native-task-tests',Date.now().toString());fs.mkdirSync(output,{recursive:true});
function run(exe,args,input){const began=performance.now(),p=cp.spawnSync(exe,args,{input,encoding:'utf8',windowsHide:true,timeout:55000,maxBuffer:32*1024*1024});assert.ifError(p.error);assert.equal(p.status,0,p.stderr);return {text:p.stdout,elapsed_ms:Math.round(performance.now()-began)};}
const wrapper=process.argv.includes('--wrapper');
function collect(unit,source){const req={selection:{disabled_sources:inventory.sources.map(r=>r.source).filter(s=>s!==source)}};const r=run(path.join(bin,wrapper?'pcscope-powershell-adapter.exe':'pcscope-platform-native.exe'),unit==='base'&&wrapper?[]:[unit],JSON.stringify(req));const rows=r.text.trim().split(/\r?\n/).filter(Boolean).map(JSON.parse);assert(rows.every(r=>r.source===source));assert.equal(rows.filter(r=>r.type==='snapshot').length,unit==='base'?1:16,JSON.stringify(rows.filter(r=>r.type==='gap')));assert(rows.every(r=>r.complete));const items=rows.flatMap(r=>r.items);assert.equal(items.length,new Set(items.map(i=>i.id)).size);fs.writeFileSync(path.join(output,unit+'.jsonl'),r.text);return {items,elapsed_ms:r.elapsed_ms};}
const base=collect('base','pcscope.startup.tasks'),details=collect('startup','pcscope.startup.task-details');
// Use existing evidence policy in this reference harness so reference output does
// not publish sensitive commands. This is never a production collector fallback.
const evidence=fs.readFileSync(path.join(root,'plugins/powershell-adapter/evidence.ps1'),'utf8');
const command=`$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);\n${evidence}\n
function Iso($v){if($null -ne $v){([datetime]$v).ToUniversalTime().ToString('o')}}
function Fields($object,$names){$result=@{};foreach($name in $names){$result[$name]=Protect-Value $name $object.$name};$result.source_properties=Public-CimFields $object;return $result}
$tasks=@(Get-ScheduledTask);$r=@{};
$r.base=@($tasks|ForEach-Object {$t=$_;@{id=($t.TaskPath+$t.TaskName).ToLowerInvariant();data=@{name=$t.TaskName;path=$t.TaskPath;state=[string]$t.State;enabled=$t.Settings.Enabled;user_id=$t.Principal.UserId;run_level=[string]$t.Principal.RunLevel;actions=@($t.Actions|ForEach-Object {Public-CimFields $_})}}});
$r.details=@($tasks|ForEach-Object {$t=$_;$info=Get-ScheduledTaskInfo -InputObject $t;@{id=($t.TaskPath+$t.TaskName);data=@{path=$t.TaskPath;name=$t.TaskName;last_run=(Iso $info.LastRunTime);last_result=$info.LastTaskResult;next_run=(Iso $info.NextRunTime);missed_runs=$info.NumberOfMissedRuns;triggers=@($t.Triggers|ForEach-Object {Public-CimFields $_});conditions=(Public-CimFields $t.Settings);principal=(Public-CimFields $t.Principal);registration=(Fields $t @('Author','Description','URI','Version','Source'));actions=@($t.Actions|ForEach-Object {Public-CimFields $_})}}});
$r|ConvertTo-Json -Depth 24 -Compress`;
const reference=run(path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),['-NoLogo','-NoProfile','-NonInteractive','-Command',command]);
const refs=JSON.parse(reference.text);fs.writeFileSync(path.join(output,'reference.json'),reference.text);
function normalize(value){if(value?.sensitive===true&&value.protection)return {sensitive:true,protection:value.protection};if(Array.isArray(value))return value.map(normalize);if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([k,v])=>[k,normalize(v)]));return value;}
const reports=[];
for(const [name,native] of [['base',base],['details',details]]){
 const byId=new Map(native.items.map(i=>[i.id,i]));assert.equal(native.items.length,refs[name].length,name);
 let stableFields=0;
 for(const row of refs[name]){const actual=byId.get(row.id);assert(actual,name+' missing full task path');assert.deepEqual(Object.keys(actual.data).sort(),Object.keys(row.data).sort());for(const [key,value] of Object.entries(row.data)){if(['state','last_run','last_result','next_run','missed_runs'].includes(key))continue;assert.deepEqual(normalize(actual.data[key]),normalize(value),name+'/'+row.id+'/'+key);stableFields++;}}
 reports.push({source:name==='base'?'pcscope.startup.tasks':'pcscope.startup.task-details',tasks:native.items.length,identity_set:'equal',stable_fields_compared:stableFields,volatile:'state and latest execution info omitted from across-time comparison',elapsed_ms:native.elapsed_ms});
}
const summary={output,wrapper,no_legacy_scripts:!fs.existsSync(path.join(bin,'collect-domain.ps1'))&&!fs.existsSync(path.join(bin,'collect-system.ps1')),reference_ms:reference.elapsed_ms,sources:reports};fs.writeFileSync(path.join(output,'summary.json'),JSON.stringify(summary,null,2));console.log(JSON.stringify(summary));
