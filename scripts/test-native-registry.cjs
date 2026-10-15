'use strict';
// Compare original read-only registry definitions with compiled output.
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'..'),inventory=require('../model/sources.json');
const bin=process.env.PCSCOPE_NATIVE_REGISTRY_BIN||path.join(root,'target/native-network-test-bin');
const output=path.join(root,'target/native-registry-tests',Date.now().toString());fs.mkdirSync(output,{recursive:true});
const units={applications:['apps.browser-policies','apps.onedrive-registration'],desktop:['desktop.input','desktop.clipboard','desktop.shell-settings'],startup:['startup.service-configuration','startup.session-hooks','startup.advanced-extensions','environment.configuration'],network:['network.winrm-configuration'],security:['policy.configuration'],system:['system.clock','system.reboot-pending','software.user-inventory'],virtualization:['virtualization.wsl-registration']};
units.storage=['power.configuration'];
if(process.argv.includes('--providers')){
 for(const unit of Object.keys(units))delete units[unit];
 units.storage=['storage.disks','storage.partitions','hardware.disk-health'];
 units.security=['security.protection','security.protection-policy','security.products','security.detections','security.deviceguard'];
 units.network=['network.profiles','network.mapped-drives','network.share-sessions'];
 units.startup=['startup.wmi-filters','startup.wmi-consumers','startup.wmi-bindings'];
}
if(process.argv.includes('--network-config')){for(const unit of Object.keys(units))delete units[unit];units.network=['network.firewall-profiles','network.firewall-rules','network.ipsec','network.dns-configuration','network.dns-cache','network.hosts'];}
if(process.argv.includes('--application-files')){for(const unit of Object.keys(units))delete units[unit];units.applications=['apps.ide-extensions','apps.npm-global-packages','apps.chromium-extensions','apps.firefox-extensions'];units.security=['security.ssh-public-keys'];}
if(process.argv.includes('--printing-shares')){for(const unit of Object.keys(units))delete units[unit];units.desktop=['desktop.printing','desktop.printer-drivers','desktop.printer-ports','desktop.print-jobs'];units.network=['network.shares'];}
if(process.argv.includes('--update-history')){for(const unit of Object.keys(units))delete units[unit];units.system=['system.update-history'];}
if(process.argv.includes('--shortcuts')){for(const unit of Object.keys(units))delete units[unit];units.desktop=['desktop.shortcuts'];}
if(process.argv.includes('--servicing-firmware')){for(const unit of Object.keys(units))delete units[unit];units.system=['software.optional-features'];units.security=['security.secureboot'];}
if(process.argv.includes('--remote-access')){for(const unit of Object.keys(units))delete units[unit];units.network=['network.access'];}
if(process.argv.includes('--encryption')){for(const unit of Object.keys(units))delete units[unit];units.security=['security.encryption'];}
if(process.argv.includes('--appx-user')){for(const unit of Object.keys(units))delete units[unit];units.system=['software.appx-user'];}
if(process.argv.includes('--provisioned')){for(const unit of Object.keys(units))delete units[unit];units.system=['software.provisioned'];}
const evidence=fs.readFileSync(path.join(root,'plugins/powershell-adapter/evidence.ps1'),'utf8');
const domain=fs.readFileSync(path.join(root,'plugins/powershell-adapter/collect-domain.ps1'),'utf8');
const helpers=domain.slice(domain.indexOf('function Item('),domain.lastIndexOf('. (Join-Path $PSScriptRoot'));
const wrapper=process.argv.includes('--wrapper');
function run(file,args,input){const began=performance.now(),p=cp.spawnSync(file,args,{input,encoding:'utf8',windowsHide:true,timeout:55000,maxBuffer:64*1024*1024});assert.ifError(p.error);assert.equal(p.status,0,p.stderr);return {text:p.stdout,elapsed_ms:Math.round(performance.now()-began)};}
function normalize(value){if(value?.sensitive===true&&value.protection)return {sensitive:true,protection:value.protection};if(Array.isArray(value))return value.map(normalize);if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([k,v])=>[k,normalize(v)]));return value;}
const reports=[];
for(const [unit,ids] of Object.entries(units)){
 const only=process.argv.find(arg=>arg.startsWith('--unit='));if(only&&unit!==only.slice(7))continue;
 const selected=ids.map(id=>'pcscope.'+id),request={selection:{disabled_sources:inventory.sources.map(r=>r.source).filter(s=>!selected.includes(s))}};
 const native=run(path.join(bin,wrapper?'pcscope-powershell-adapter.exe':'pcscope-platform-native.exe'),[unit],JSON.stringify(request));
 const messages=native.text.trim().split(/\r?\n/).filter(Boolean).map(JSON.parse);assert(messages.every(r=>selected.includes(r.source)));
 fs.writeFileSync(path.join(output,unit+'.jsonl'),native.text);
 // Reference only, no script files are executed or execution policies changed.
 // Emit invokes only the selected registry definitions; every other body stays inert.
 const body=process.argv.includes('--encryption')?fs.readFileSync(path.join(root,'scripts/test-native-encryption-reference.ps1'),'utf8'):fs.readFileSync(path.join(root,'plugins/powershell-adapter/domains',unit+'.ps1'),'utf8');
 const command=`$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);\n${evidence}\n${helpers}\n$selected=${selected.map(s=>"'"+s+"'").join(',')};$reference=@{};function Emit($name,$query){$source='pcscope.'+$name;if($source -in $selected){try{$reference[$source]=@(& $query)}catch{$reference[$source]=@{failure=$_.Exception.GetType().Name;reason=$_.Exception.Message}}}}\n${body}\n$reference|ConvertTo-Json -Depth 24 -Compress`;
 const refResult=run(path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),['-NoLogo','-NoProfile','-NonInteractive','-Command',command]);
 const refs=JSON.parse(refResult.text);fs.writeFileSync(path.join(output,unit+'-reference.json'),refResult.text);
 for(const source of selected){const snapshots=messages.filter(r=>r.source===source&&r.type==='snapshot'),gaps=messages.filter(r=>r.source===source&&r.type==='gap');if(refs[source].failure){assert.equal(snapshots.length,0,source+' failed reference committed native snapshot');assert.equal(gaps.length,1,source+' missing failure');reports.push({source,status:'failed consistently; no snapshot',reference_error:refs[source].reason,native_error:gaps[0].reason});continue;}assert.equal(gaps.length,0,JSON.stringify(gaps));assert.equal(snapshots.length,16,source);assert(snapshots.every(r=>r.complete));const items=snapshots.flatMap(r=>r.items),byId=new Map(items.map(i=>[i.id,i]));assert.equal(items.length,byId.size,source+' duplicate');const volatile=source==='pcscope.network.dns-cache';const expected=volatile?refs[source].filter(row=>byId.has(row.id)):refs[source];if(!volatile)assert.equal(items.length,refs[source].length,source+' count');for(const row of expected){assert(byId.has(row.id),source+' identity');assert.deepEqual(normalize(byId.get(row.id).data),normalize(row.data),source+'/'+row.id);}reports.push({source,items:items.length,reference_items:refs[source].length,compared_items:expected.length,identities:volatile?'overlapping volatile entries':'equal',fields:volatile&&!expected.length?'no stable cache witness; requires fixture validation':'equal',unit_ms:native.elapsed_ms,reference_ms:refResult.elapsed_ms});}
}
const summary={output,wrapper,no_legacy_scripts:!fs.existsSync(path.join(bin,'collect-domain.ps1')),source_count:reports.length,sources:reports};fs.writeFileSync(path.join(output,'summary.json'),JSON.stringify(summary,null,2));console.log(JSON.stringify(summary));
