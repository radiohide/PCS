const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const controls=require('../apps/desktop/ui/collection-controls.json'),sources=require('../apps/desktop/ui/plugin-sources.js');
assert.equal(new Set(controls.map(c=>c.id)).size,controls.length);
for(const c of controls)assert(sources[c.plugin].some(r=>r.name===c.name));
// The page loads the generated source type map with fetch, as a browser would; the sandbox has to
// serve it or the module is being run somewhere it was never written for.
const sourceMap=fs.readFileSync('apps/desktop/ui/source-type-map.json','utf8');
const fetchStub=async()=>({ok:true,json:async()=>JSON.parse(sourceMap)});
const code=fs.readFileSync('apps/desktop/ui/plugin-manager.js','utf8'),context=vm.createContext({pluginSources:sources,fetch:fetchStub,esc:s=>String(s).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;')});
vm.runInContext(code+';globalThis.table=pluginSourceTable;globalThis.configure=items=>collectionItemControls=items;',context);
context.configure(controls.map(c=>({...c,enabled:c.id!=='pcscope.identity.logon-sessions'})));
const html=context.table('system');assert(html.includes('data-item-id="pcscope.identity.logon-sessions"'));assert(/aria-label="采集登录会话" aria-checked="false"/.test(html));assert(html.includes('关闭后保留历史和最近状态'));assert(html.includes('可能漏记之后的变化'));
assert.doesNotMatch(html,/<details class="plugin-source-details" open/,'advanced source information is collapsed by default');
assert.match(html,/<details><summary>技术详情<\/summary><p>pcscope\.identity\.logon-sessions<\/p>/,'exact source names remain available in collapsed technical details');
const eventHtml=context.table('eventlog');
assert.equal((eventHtml.match(/role="switch"/g)||[]).length,controls.filter(c=>c.plugin==='eventlog').length);
const auditSettings=controls.find(c=>c.id==='pcscope.eventlog.security.files-audit-settings');
assert(auditSettings,'object audit-rule changes have their own existing-Security-source control');
assert.equal(auditSettings.source,'pcscope.eventlog.security');
assert.equal(auditSettings.event_rule.provider,'Microsoft-Windows-Security-Auditing');
assert.deepEqual(auditSettings.event_rule.event_ids,[4907]);
assert(eventHtml.includes('data-item-id="pcscope.eventlog.security.files-audit-settings"'));
assert(eventHtml.includes('对象访问记录规则'),'the new rule is readable in the rendered controls');
assert.equal((context.table('usn').match(/role="switch"/g)||[]).length,2);
assert(code.includes("api('set_item_enabled'"));assert(code.includes('drawItems(result.items)'));
console.log(`PASS: ${controls.length} unique item controls, plugin association, explicit switch state, event subitems and USN dependencies`);
