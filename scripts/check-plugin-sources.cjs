const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const root=path.resolve(__dirname,'..');
const catalog=require(path.join(root,'apps/desktop/ui/plugin-sources.js'));
const base=path.join(root,'plugins/powershell-adapter');
function checkSources(file,id,pattern,prefix){
 const actual=[...fs.readFileSync(file,'utf8').matchAll(pattern)].map(m=>prefix+m[1]);
 const documented=new Set(catalog[id].map(r=>r.source));
 for(const source of actual)assert(documented.has(source),`${id}: missing ${source}`);
}
checkSources(path.join(base,'collect-system.ps1'),'pcscope-powershell-adapter.exe',/Emit-Collection\s+'([^']+)'/g,'');
for(const file of fs.readdirSync(path.join(base,'domains')).filter(f=>f.endsWith('.ps1'))){
 checkSources(path.join(base,'domains',file),file.slice(0,-4),/Emit\s+'([^']+)'/g,'pcscope.');
}
for(const location of ['localmachine','currentuser'])assert(catalog.security.some(r=>r.source==='pcscope.security.certificates.'+location));
const channels=JSON.parse(fs.readFileSync(path.join(root,'plugins/eventlog/channels.json'),'utf8'));
assert.deepEqual([...new Set(catalog.eventlog.map(r=>r.source))],channels.map(c=>'pcscope.eventlog.'+c.toLowerCase()));
const manager=fs.readFileSync(path.join(root,'apps/desktop/ui/plugin-manager.js'),'utf8');
const vm=require('node:vm');
// The module reads the generated source type map through fetch, as it does in the app.
const sourceMap=fs.readFileSync(path.join(root,'apps/desktop/ui/source-type-map.json'),'utf8');
const fetchStub=async()=>({ok:true,json:async()=>JSON.parse(sourceMap)});
const descriptions=vm.runInNewContext(manager+';pluginDescriptions',{fetch:fetchStub});
assert.deepEqual(Object.keys(catalog).sort(),Object.keys(descriptions).sort());
for(const [id,rows] of Object.entries(catalog)){
 assert(rows.length>0,id);
 for(const row of rows)for(const key of ['source','name','api','method'])assert(typeof row[key]==='string'&&row[key].trim(),`${id}: ${key}`);
}
console.log(`Source catalog verified: ${Object.keys(catalog).length} plugins, ${Object.values(catalog).flat().length} rows, ${channels.length} event channels.`);
