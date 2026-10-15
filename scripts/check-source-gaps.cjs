'use strict';
const assert=require('node:assert/strict');
const fs=require('node:fs');const path=require('node:path');
const root=path.resolve(__dirname,'..');
const view=require(path.join(root,'apps/desktop/ui/model-view.js'));
const readings={status:'loaded',counts:{source_records:5},data:[{
  source:'pcscope.execution.resources',scope:'machine:test',item_id:'memory',destination:'sampling',observed_at:'2026-10-12T00:00:00Z',
  evidence:{id:'sample-1',kind:'snapshot'},payload:{total_kib:1024,free_kib:512,unknown_nested:{long:'tail'.repeat(1000),hostile:'<img src=x onerror=alert(1)>'}},
}]};
const rendered=view.sourceReadings(readings);
assert.match(rendered,/总内存（KiB）/);assert.match(rendered,/共 5 条/);assert.match(rendered,/sample-1/);
assert(rendered.includes('tail'.repeat(1000)),'unknown nested payload was truncated');
assert(!rendered.includes('<img'),'raw source text must not become markup');
const moduleFile=view.moduleFiles({data:[{item_id:'fixture.dll',observed_at:'2026-10-12T00:00:00Z',evidence:{id:'module-proof'},payload:{path:'C:\\fixture\\<img>.dll',changes:['file_id','sha256'],check:{status:'stable',signature_status:'valid_offline',publisher:'Example Publisher'},module_bindings:{one:{pid:100,process_name:'sihost',process_start:'2026-10-12T00:00:00Z',last_observed:'2026-10-12T00:00:00Z'}},before:{file_id:'old',unknown_nested:{tail:'preserved'}}}}]});
assert.match(moduleFile,/磁盘文件身份变化/);assert.match(moduleFile,/文件内容指纹变化/);assert.match(moduleFile,/Example Publisher/);assert.match(moduleFile,/PID 100/);assert.match(moduleFile,/module-proof/);assert.match(moduleFile,/preserved/);assert(!moduleFile.includes('<img>'));assert(!moduleFile.includes('内存模块已损坏'));
const update=view.timeline({data:[{action_id:'software.update_history',title:'更新操作记录',result:'failed',phase:'installation',precision:'exact',occurred_at:'2026-10-11T00:00:00Z',observed_at:'2026-10-12T00:00:00Z',unassigned_reason:'历史操作不证明当前安装',source_record:{title:'<script>example</script>',update_id:'U',revision:2,result:4,hresult:-1}}]});
const updateVisible=update.slice(0,update.indexOf('<details>'));
assert.match(updateVisible,/系统更新/);assert.match(updateVisible,/更新失败/);assert.match(updateVisible,/相关功能可能受影响/);
assert.doesNotMatch(updateVisible,/hresult|revision|错误码 -1|修订 2|software\.update_history/,'internal update codes are not shown by default');
const originalUpdate=update.match(/<details><summary>记录详情<\/summary><pre>([\s\S]*?)<\/pre><\/details>/);
assert(originalUpdate,'the complete update operation must remain in collapsed details');
const decode=text=>text.replaceAll('&quot;','"').replaceAll('&#39;',"'").replaceAll('&lt;','<').replaceAll('&gt;','>').replaceAll('&amp;','&');
assert.deepEqual(JSON.parse(decode(originalUpdate[1]))['更新记录'],{title:'<script>example</script>',update_id:'U',revision:2,result:4,hresult:-1},'all original update fields remain exact, including revision and HRESULT');
assert(!update.includes('<script>'));
const manager=fs.readFileSync(path.join(root,'apps/desktop/ui/plugin-manager.js'),'utf8');
assert.match(manager,/jobs\[Number\(card\.dataset\.pluginIndex\)\]\.id==='scoped'/,'directory editor must belong to the scoped collector');
assert.match(manager,/set_directory_watch_settings/);assert.match(manager,/directories\.join/);
const scoped=fs.readFileSync(path.join(root,'plugins/powershell-adapter/domains/scoped.ps1'),'utf8');
assert.match(scoped,/Emit 'files\.directory-security'/);
const exceptions=require(path.join(root,'model/adapter-exceptions.cjs'));
assert.deepEqual(Object.keys(exceptions.sourceExceptions),['pcscope.usn.changes']);assert.deepEqual(exceptions.typeExceptions,{});
console.log('Source gaps UI verified: source readings, complete escaped payloads, update results, explicit directory settings and no implementation exceptions.');
