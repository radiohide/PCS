'use strict';
// S01 source inventory: derive implemented sources from collector code, reconcile against the
// three catalogs (plugin-sources.js, collection-catalog.json, collection-controls.json) and the
// event channel list, then write the machine-readable inventory to model/sources.json.
// Fails when code and catalog disagree in either direction, so a newly added collector or a
// hand-added catalog row cannot pass unnoticed.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const crypto=require('node:crypto');
const root=path.resolve(__dirname,'..');
const p=(...parts)=>path.join(root,...parts);
const rel=file=>path.relative(root,file).split(path.sep).join('/');

// ---------- declared execution units (crates/service/src/main.rs) ----------
// Two fixed executables plus nine powershell-adapter domains, then the two log lanes.
const DOMAINS=['system','security','network','storage','startup','desktop','virtualization','applications','scoped'];
const UNITS=['pcscope-software.exe','pcscope-powershell-adapter.exe',...DOMAINS,'eventlog','usn'];

// ---------- ground truth: emit sites in collector code ----------
const sources=new Map();
function record(entry){
  if(!sources.has(entry.source))sources.set(entry.source,{source:entry.source,emitters:[],forms:new Set(),scope_forms:new Set(),units:new Set()});
  const target=sources.get(entry.source);
  target.emitters.push(entry.emitter);
  target.units.add(entry.unit);
  for(const form of entry.forms)target.forms.add(form);
  if(entry.scope)target.scope_forms.add(entry.scope);
}
function lineOf(text,index){return text.slice(0,index).split('\n').length;}
function scan(text,file,unit,pattern,toSource,forms,scope){
  for(const match of text.matchAll(pattern)){
    const expanded=toSource(match);
    for(const source of (Array.isArray(expanded)?expanded:[expanded]))
      record({source,unit,emitter:`${rel(file)}:${lineOf(text,match.index)}`,forms,scope});
  }
}
// Resolve a double-quoted Emit source such as "security.certificates.$($location.ToLowerInvariant())"
// by reading the values of its enclosing foreach loop. Unresolvable interpolation fails the check
// rather than silently dropping a source.
function interpolations(raw){
  const found=[];
  for(let i=0;i<raw.length;i++){
    if(raw[i]!=='$')continue;
    if(raw[i+1]==='('){
      // Parenthesis-balanced scan so nested calls such as $($x.ToLowerInvariant()) stay intact.
      let depth=0,end=i+1;
      for(;end<raw.length;end++){
        if(raw[end]==='(')depth++;
        else if(raw[end]===')'&&--depth===0)break;
      }
      found.push({start:i,end:end+1,expression:raw.slice(i+2,end)});
      i=end;
    }else if(/[A-Za-z_]/.test(raw[i+1]||'')){
      let end=i+2;
      while(end<raw.length&&/[A-Za-z0-9_]/.test(raw[end]))end++;
      found.push({start:i,end,expression:raw.slice(i+1,end)});
      i=end-1;
    }
  }
  return found;
}
function interpolatedValues(text,raw){
  const pieces=[];
  let last=0;
  for(const token of interpolations(raw)){
    pieces.push({literal:raw.slice(last,token.start)});
    const name=(/([A-Za-z_]\w*)/.exec(token.expression)||[])[1];
    const loop=new RegExp(`foreach\\s*\\(\\$${name}\\s+in\\s+@\\(([^)]*)\\)`).exec(text);
    if(!loop)throw new Error(`Unresolved emit interpolation $${name} in ${raw}`);
    let values=loop[1].split(',').map(value=>value.trim().replace(/^['"]|['"]$/g,''));
    if(/\.ToLowerInvariant\(\)/.test(token.expression))values=values.map(value=>value.toLowerCase());
    pieces.push({values});
    last=token.end;
  }
  pieces.push({literal:raw.slice(last)});
  let out=[''];
  for(const piece of pieces)out=piece.literal!==undefined?out.map(value=>value+piece.literal):out.flatMap(prefix=>piece.values.map(value=>prefix+value));
  return out;
}

// powershell-adapter base script: atomic snapshot per source, gap on failure.
{
  const file=p('plugins/powershell-adapter/collect-system.ps1');
  const text=fs.readFileSync(file,'utf8');
  scan(text,file,'pcscope-powershell-adapter.exe',/Emit-Collection\s+'([^']+)'/g,m=>m[1],['snapshot','gap'],'machine:<MachineGuid>/user:<SID>/schema:1/<scopeSuffix>');
}
// powershell-adapter domains: Emit prepends the pcscope. prefix inside collect-domain.ps1.
for(const domain of DOMAINS){
  const file=p(`plugins/powershell-adapter/domains/${domain}.ps1`);
  const text=fs.readFileSync(file,'utf8');
  scan(text,file,domain,/Emit\s+'([^']+)'/g,m=>'pcscope.'+m[1],['snapshot','gap'],'machine:<MachineGuid>/user:<SID>/schema:1/context:<scopeContext>/shard:<0-f>');
  scan(text,file,domain,/Emit\s+"([^"]+)"/g,m=>interpolatedValues(text,m[1]).map(value=>'pcscope.'+value),['snapshot','gap'],'machine:<MachineGuid>/user:<SID>/schema:1/context:<scopeContext>/shard:<0-f>');
}
// Native sources retain their existing unit/control IDs; ownership is compiled into the wrapper.
const nativeOwnership=JSON.parse(fs.readFileSync(p('plugins/platform-native/ownership.json'),'utf8'));
const nativeFile=p('plugins/platform-native/Sources.cs'),nativeText=fs.readFileSync(nativeFile,'utf8');
const nativeEmitFiles=[nativeFile,p('plugins/platform-native/Modules.cs'),p('plugins/platform-native/State.cs'),p('plugins/platform-native/RegistryState.cs'),p('plugins/platform-native/Providers.cs'),p('plugins/platform-native/NetworkConfiguration.cs'),p('plugins/platform-native/ApplicationFiles.cs'),p('plugins/platform-native/PrintingShares.cs'),p('plugins/platform-native/DesktopStartup.cs'),p('plugins/platform-native/ShellRegistrations.cs'),p('plugins/platform-native/Mihomo.cs'),p('plugins/platform-native/HypervDisks.cs'),p('plugins/platform-native/HypervSnapshots.cs'),p('plugins/platform-native/HypervMachines.cs'),p('plugins/platform-native/HypervSwitches.cs'),p('plugins/platform-native/TpmMetadata.cs'),p('plugins/platform-native/ProvisionedPackages.cs'),p('plugins/platform-native/AppPackages.cs'),p('plugins/platform-native/Encryption.cs'),p('plugins/platform-native/RemoteAccess.cs'),p('plugins/platform-native/LocalGroups.cs'),p('plugins/platform-native/LocalAccounts.cs'),p('plugins/platform-native/Docker.cs'),p('plugins/platform-native/ServicingFirmware.cs')];
const nativeOnly=new Set(nativeOwnership.native_only||[]);
for(const source of nativeOnly)assert(DOMAINS.some(domain=>(nativeOwnership[domain]||[]).includes(source)),`Native-only source lacks control unit: ${source}`);
for(const domain of [...DOMAINS,'pcscope-powershell-adapter.exe'])for(const source of nativeOwnership[domain]||[]){
  const base=domain==='pcscope-powershell-adapter.exe';const marker=`${base?'EmitBase':'Emit'}("${source}"`;const file=nativeEmitFiles.find(file=>fs.readFileSync(file,'utf8').includes(marker));
  assert(file,`Native ownership has no emitter: ${domain}/${source}`);
  assert(nativeOnly.has(source)||sources.has(source)&&sources.get(source).units.has(domain),`Native source changes its control unit: ${domain}/${source}`);
  const text=fs.readFileSync(file,'utf8');
  record({source,unit:domain,emitter:`${rel(file)}:${lineOf(text,text.indexOf(marker))}`,forms:['snapshot','gap'],scope:base?'machine:<MachineGuid>/user:<SID>/schema:1/<scopeSuffix>':'machine:<MachineGuid>/user:<SID>/schema:1/context:<scopeContext>/shard:<0-f>'});
}
// Rust collectors: literal and templated source strings next to a message constructor.
function scanRust(file,unit,forms){
  const text=fs.readFileSync(file,'utf8');
  scan(text,file,unit,/source:\s*"([^"]+)"/g,m=>m[1],forms);
  scan(text,file,unit,/format!\("(pcscope\.[^"]*)"/g,m=>m[1].replace(/\{\}/g,'{drive}'),forms);
}
scanRust(p('plugins/software/src/main.rs'),'pcscope-software.exe',['snapshot','gap']);
scanRust(p('plugins/usn/src/main.rs'),'usn',['snapshot','event_batch','gap']);
// EventLog: one source per persisted channel, read by the PowerShell or the native reader.
{
  const channels=JSON.parse(fs.readFileSync(p('plugins/eventlog/channels.json'),'utf8'));
  for(const channel of channels)record({source:'pcscope.eventlog.'+channel.toLowerCase(),unit:'eventlog',emitter:`plugins/eventlog/channels.json (${channel})`,forms:['event_batch','gap'],scope:'channel record id checkpoint'});
}
for(const value of sources.values()){value.forms=[...value.forms].sort();value.units=[...value.units].sort();value.scope_forms=[...value.scope_forms].sort();}

// ---------- catalogs ----------
const catalog=require(p('apps/desktop/ui/plugin-sources.js'));
const definitions=JSON.parse(fs.readFileSync(p('apps/desktop/ui/collection-catalog.json'),'utf8'));
const controls=JSON.parse(fs.readFileSync(p('apps/desktop/ui/collection-controls.json'),'utf8'));
const channels=JSON.parse(fs.readFileSync(p('plugins/eventlog/channels.json'),'utf8'));
const eventItems=require(p('apps/desktop/ui/event-items.js'));

const catalogRows=[];
for(const [plugin,rows] of Object.entries(catalog))for(const row of rows)catalogRows.push({plugin,...row});
const catalogSources=new Map();
for(const row of catalogRows){
  if(!catalogSources.has(row.source))catalogSources.set(row.source,[]);
  catalogSources.get(row.source).push(row);
}
// Per-item switch controls, keyed by the source each row points at.
const controlsBySource=new Map();
for(const row of controls){
  if(!controlsBySource.has(row.source))controlsBySource.set(row.source,[]);
  controlsBySource.get(row.source).push(row);
}

// ---------- reconciliation ----------
// Adjudicated code-versus-catalog differences. Each carries the reason it is accepted and the step
// that closes it; the check fails when a listed difference no longer exists, so an exemption cannot
// outlive the problem it records.
const KNOWN_DIFFERENCES={
  'pcscope.usn.watch-list':{
    direction:'implemented_not_catalogued',
    reason:'USN 空名单状态快照：文件名单为空时报 files=[] 与 active=false。属于采集状态来源，不是可停用的数据项，因此在来源目录中没有对应行。',
    resolution:'补录到 plugin-sources.js 并归入 pcscope.usn.changes 控制组，不新增独立采集开关。',
    step:'S14'
  }
};

const missingFromCatalog=[];   // implemented in code, absent from plugin-sources.js
const missingFromCode=[];      // catalogued, no emitter (excluding virtual prefix rows)
const VIRTUAL_SOURCES=new Set(['pcscope.usn.changes','pcscope.eventlog.import']);
for(const [source,value] of [...sources].sort()){
  const rows=catalogSources.get(source);
  if(!rows&&!VIRTUAL_SOURCES.has(source))missingFromCatalog.push(source);
  value.unit=value.units.join(',');
}
for(const [source] of [...catalogSources].sort()){
  if(sources.has(source))continue;
  // A template source catalogued as {drive} is matched against the implemented {drive} source.
  if(VIRTUAL_SOURCES.has(source))continue;
  missingFromCode.push(source);
}
const accepted=[];
const unaccepted=entries=>entries.filter(source=>{
  const known=KNOWN_DIFFERENCES[source];
  if(!known)return true;
  accepted.push({source,...known});
  return false;
});
const unaccountedNotCatalogued=unaccepted(missingFromCatalog);
const unaccountedNotImplemented=unaccepted(missingFromCode);
const resolvedDifferences=Object.keys(KNOWN_DIFFERENCES).filter(source=>!missingFromCatalog.includes(source)&&!missingFromCode.includes(source));

// Event channel coverage: the catalog's eventlog rows and channels.json must agree exactly.
const catalogChannels=[...new Set(catalog.eventlog.map(r=>r.source))];
const expectedChannels=channels.map(c=>'pcscope.eventlog.'+c.toLowerCase());
const channelDrift=catalogChannels.length!==expectedChannels.length||catalogChannels.some((s,i)=>s!==expectedChannels[i]);

// Plugin keys: every catalog plugin id must be a declared execution unit.
const unitDrift=Object.keys(catalog).filter(id=>!UNITS.includes(id));

// Item switch integrity: every control row must resolve to a catalog definition.
const danglingControls=controls.filter(c=>!definitions.some(d=>d.id===c.id));

// Event item expansion must reproduce the catalog's eventlog row count.
const expandedChannels=channels.reduce((n,channel)=>n+eventItems.expand({source:'x',name:'x',api:'x',method:'x'},channel).length,0);
const catalogEventRows=catalog.eventlog.length;

// ---------- inventory document ----------
const inventory=[...sources.values()].sort((a,b)=>a.source.localeCompare(b.source)).map(row=>{
  const rows=catalogSources.get(row.source)||[];
  const control=controlsBySource.get(row.source)||[];
  return {
    source:row.source,
    execution_units:row.units,
    emitters:row.emitters.sort(),
    message_forms:row.forms,
    scope_form:row.scope_forms[0]||null,
    catalogued:rows.length>0,
    catalog_plugin:rows[0]?rows[0].plugin:null,
    catalog_name:rows[0]?rows[0].name:null,
    switch_controls:control.map(c=>({plugin:c.plugin,item:c.id,name:c.name})).sort((a,b)=>a.item.localeCompare(b.item)),
    switchable:control.length>0,
    collection_backend:Object.values(nativeOwnership).filter(Array.isArray).flat().includes(row.source)?'native':row.units.includes('eventlog')||row.units.includes('usn')||row.units.includes('pcscope-software.exe')?'native':'powershell_compatibility',
    virtual:VIRTUAL_SOURCES.has(row.source),
    // Field-level enumeration is a per-type task (plan S09-S20); S01 records the emit site instead
    // of guessing keys, so the later step verifies against real output.
    fields_enumeration:'pending: read real output at the emitter above (plan S09-S20)'
  };
});
for(const source of VIRTUAL_SOURCES){
  const rows=catalogSources.get(source)||[];
  inventory.push({source,execution_units:[],emitters:[],message_forms:[],scope_form:null,catalogued:rows.length>0,catalog_plugin:rows[0]?rows[0].plugin:null,catalog_name:rows[0]?rows[0].name:null,switch_controls:(controlsBySource.get(source)||[]).map(c=>({plugin:c.plugin,item:c.id,name:c.name})),switchable:(controlsBySource.get(source)||[]).length>0,virtual:true,fields_enumeration:'pending: prefix row groups other sources (plan S09-S20)'});
}
inventory.sort((a,b)=>a.source.localeCompare(b.source));

// Line endings are normalised first: this digest identifies the collector code, and a checkout with
// different line endings is the same code. See crates/contracts/src/model.rs for the same rule.
const normalise=text=>text.includes('\r\n')?text.replace(/\r\n/g,'\n'):text;
const nativeFiles=['plugins/platform-native/Program.cs','plugins/platform-native/Modules.cs','plugins/platform-native/State.cs','plugins/platform-native/RegistryState.cs','plugins/platform-native/Providers.cs','plugins/platform-native/NetworkConfiguration.cs','plugins/platform-native/ApplicationFiles.cs','plugins/platform-native/PrintingShares.cs','plugins/platform-native/DesktopStartup.cs','plugins/platform-native/ShellRegistrations.cs','plugins/platform-native/Mihomo.cs','plugins/platform-native/HypervDisks.cs','plugins/platform-native/HypervSnapshots.cs','plugins/platform-native/HypervMachines.cs','plugins/platform-native/HypervSwitches.cs','plugins/platform-native/TpmMetadata.cs','plugins/platform-native/ProvisionedPackages.cs','plugins/platform-native/AppPackages.cs','plugins/platform-native/Encryption.cs','plugins/platform-native/RemoteAccess.cs','plugins/platform-native/LocalGroups.cs','plugins/platform-native/LocalAccounts.cs','plugins/platform-native/Docker.cs','plugins/platform-native/ServicingFirmware.cs','plugins/powershell-adapter/collector-options.json','plugins/powershell-adapter/domains/MetadataNative.cs','plugins/powershell-adapter/domains/IdentityNative.cs','plugins/powershell-adapter/src/main.rs','plugins/eventlog/src/main.rs','plugins/eventlog-native/Program.cs'];
const digestInput=[...Object.keys(catalog).map(id=>JSON.stringify(catalog[id])),JSON.stringify(channels),normalise(fs.readFileSync(p('plugins/powershell-adapter/collect-system.ps1'),'utf8')),...DOMAINS.map(d=>normalise(fs.readFileSync(p(`plugins/powershell-adapter/domains/${d}.ps1`),'utf8'))),JSON.stringify(nativeOwnership),normalise(nativeText),...nativeFiles.map(file=>normalise(fs.readFileSync(p(file),'utf8')))].join('\n');
const document={
  generated_by:'scripts/check-source-inventory.cjs',
  note:'Ground truth is derived from collector code; catalogs are compared against it. Regenerate after changing any collector.',
  collector_digest:crypto.createHash('sha256').update(digestInput).digest('hex'),
  counts:{
    execution_units:UNITS.length,
    execution_unit_domains:DOMAINS.length,
    implemented_sources:inventory.length,
    implemented_collector_sources:sources.size,
    catalog_rows:catalogRows.length,
    catalog_sources:catalogSources.size,
    collection_definitions:definitions.length,
    item_controls:controls.length,
    event_channels:channels.length,
    event_item_rules:expandedChannels,
    catalog_event_rows:catalogEventRows
  },
  discrepancies:{
    implemented_not_catalogued:missingFromCatalog,
    catalogued_not_implemented:missingFromCode,
    accepted_differences:accepted,
    event_channel_drift:channelDrift,
    unknown_execution_units:unitDrift,
    dangling_controls:danglingControls.length
  }
};
fs.mkdirSync(p('model'),{recursive:true});
fs.writeFileSync(p('model/sources.json'),JSON.stringify({...document,sources:inventory},null,2)+'\n');

// ---------- report ----------
const c=document.counts;
console.log(`Execution units: ${c.execution_units} (${c.execution_unit_domains} domains + software, adapter, eventlog, usn)`);
console.log(`Implemented sources: ${c.implemented_sources} (${c.implemented_collector_sources} direct emitters + ${c.implemented_sources-c.implemented_collector_sources} virtual groups)`);
console.log(`Catalog: ${c.catalog_rows} rows over ${c.catalog_sources} sources; definitions ${c.collection_definitions}; item controls ${c.item_controls}`);
console.log(`Event channels: ${c.event_channels}; expanded event item rules ${c.event_item_rules}; catalog event rows ${c.catalog_event_rows}`);
if(c.channelDrift)console.log('  ! event channel drift: catalog and channels.json disagree');

let failed=false;
const report=(label,list)=>{
  if(!list.length)return false;
  failed=true;
  console.log(`  ! ${label} (${list.length}):`);
  for(const item of list)console.log(`      ${typeof item==='string'?item:JSON.stringify(item)}`);
  return true;
};
for(const entry of accepted)console.log(`  - accepted difference: ${entry.source} (${entry.direction}) -> ${entry.step}`);
if(resolvedDifferences.length)failed=report('accepted differences that no longer exist; remove them from KNOWN_DIFFERENCES',resolvedDifferences)||failed;
report('implemented in code but not catalogued',unaccountedNotCatalogued);
report('catalogued but no emitter in code',unaccountedNotImplemented);
report('catalog plugins that are not declared execution units',unitDrift);
failed=report('event channel drift',channelDrift?['catalog eventlog rows do not match channels.json']:[])||failed;
if(c.danglingControls)failed=report('item controls without a collection definition',controls.filter(x=>!definitions.some(d=>d.id===x.id)).map(x=>x.id))||failed;
if(c.event_item_rules!==c.catalog_event_rows)failed=report('event item expansion mismatch',[`expanded ${c.event_item_rules} rules, catalog holds ${c.catalog_event_rows} event rows`])||failed;

if(failed){
  console.error('Source inventory check FAILED');
  process.exit(1);
}
console.log(`Source inventory verified. Written to model/sources.json (digest ${document.collector_digest.slice(0,12)}).`);
