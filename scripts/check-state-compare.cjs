'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const root=path.resolve(__dirname,'..'),ui=path.join(root,'apps/desktop/ui');
const samples=Object.fromEntries(JSON.parse(fs.readFileSync(path.join(root,'model/query-samples.json'),'utf8')).samples.map(s=>[s.name,s.response]));
const server=http.createServer((req,res)=>{const file=path.resolve(ui,'.'+new URL(req.url,'http://localhost').pathname);if(!file.startsWith(ui+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));});
(async()=>{await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;
try{
 browser=await chromium.launch({executablePath:process.env.PCSCOPE_CHROME_PATH,headless:true});const page=await browser.newPage({viewport:{width:1320,height:860}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(samples=>{
  window.setInterval=()=>1;window.qa={calls:[],fail:false,delay:0};
  localStorage.setItem('pcscope.compareTimes',JSON.stringify({from:'2026-10-22T13:00',until:'2026-10-22T15:00'}));
  window.__TAURI__={core:{invoke:async(command,args)=>{
   const r=args?.request||{};
   if(r.operation==='status')return {pid:1,collecting:false,read_model_revision:'1',counts:{}};
   if(r.operation==='monitoring_settings')return {elevated:true,settings:{configured:true,mode:'enhanced'}};
   if(r.operation!=='query')throw Error('unexpected operation');const q=r.request;
   if(q.query!=='state_compare')return structuredClone(samples[q.query]||{status:'empty',data:[]});
   qa.calls.push(q);if(qa.delay)await new Promise(r=>setTimeout(r,qa.delay));if(qa.fail){qa.fail=false;throw Error('读取超时');}
   const envelope=data=>({status:'loaded',data});
   if(!q.type_id)return envelope({watermark:'fixed',branches:[{type_id:'startup.service',category:'startup',count:qa.filterPages?5:2},{type_id:'network.adapter',category:'network',count:1}]});
   if(qa.branchFail){qa.branchFail=false;throw '查询超时，请重试';}
   if(q.cursor&&qa.holdNext)await new Promise(resolve=>{qa.releaseNext=resolve;});
   if(q.cursor&&qa.failNext){qa.failNext=false;throw '后续读取超时';}
   if(qa.branchDelay)await new Promise(r=>setTimeout(r,qa.branchDelay));
   function side(state){const fields=[{field_id:'display_name',label:'名称',value:'Volume Shadow Copy',status:'known'},{field_id:'state',label:'运行状态',value:state,status:'known'},{field_id:'start_mode',label:'启动方式',value:'Manual',status:'known'}];return {availability:'recorded',checked_at:'2026-10-22T05:00:00Z',fields,summary:{type_id:q.type_id,identity_status:'confirmed',display_name:'Volume Shadow Copy',lifecycle:'active',record_basis:'observation',key_facts:fields}};}
   if(qa.filterPages){const n=Number(q.cursor||1);const result=envelope({objects:[{instance_id:n,type_id:q.type_id,left:side('Stopped'),right:qa.same?side('Stopped'):n===5?{availability:'unavailable'}:side(n===4?'Running':'Stopped')}]});result.page={has_more:n<5,next_cursor:n<5?String(n+1):null};return result;}
   const result=envelope({objects:[{instance_id:q.cursor?2:1,type_id:q.type_id,left:side('Stopped'),right:side('Running')}]});result.page={has_more:!q.cursor,next_cursor:q.cursor?null:'next'};return result;
  }}};
 },samples);
 await page.goto('http://127.0.0.1:'+server.address().port+'/index.html');await page.locator('[data-page="compare"]').click();await page.locator('.compare-category').first().waitFor();
 const openTree=async()=>{for(const selector of ['.compare-category-row button','.compare-type-row button']){const button=page.locator(selector).first();if(await button.getAttribute('aria-expanded')==='false')await button.click();}};
 assert.equal(await page.locator('[data-compare-from]').inputValue(),'2026-10-22T13:00');
 assert.equal(await page.locator('[data-compare-only-changes]').isChecked(),true,'changes only is enabled by default');
 assert.equal(await page.evaluate(()=>qa.calls.filter(q=>q.type_id).length),0,'collapsed branches are lazy');
 assert.doesNotMatch(await page.locator('[data-compare-summary]').innerText(),/0 项变化/,'unread is not zero changes');
 await page.evaluate(()=>qa.holdNext=true);
 await openTree();await page.locator('[data-compare-object="1"]').waitFor();
 await page.waitForFunction(()=>qa.releaseNext);assert.equal(await page.locator('[data-compare-progress]').getAttribute('value'),'1');assert.equal(await page.locator('[data-compare-progress]').getAttribute('max'),'2');assert.match(await page.locator('[data-compare-summary]').innerText(),/已发现.*1 项变化/s);assert.doesNotMatch(await page.locator('[data-compare-progress-label]').innerText(),/完成/);
 assert.equal(await page.locator('.compare-property').count(),0,'collapsed properties are not rendered');
 await page.evaluate(()=>{qa.holdNext=false;qa.releaseNext();});await page.locator('[data-compare-object="2"]').waitFor();assert.equal(await page.locator('[data-compare-progress]').getAttribute('value'),'2');assert.match(await page.locator('[data-compare-progress-label]').innerText(),/已展开类型对比完成/);
 assert.equal(await page.evaluate(()=>qa.calls.find(q=>q.type_id).watermark),'fixed');
 const row=page.locator('[data-compare-object="1"]');await row.locator('button').nth(0).click();assert.deepEqual(await row.locator('button').evaluateAll(nodes=>nodes.map(n=>n.getAttribute('aria-expanded'))),['true','true']);
 assert.match(await row.innerText(),/已停止/);assert.match(await row.innerText(),/运行中/);
 const positions=await row.locator('.compare-property').first().locator('.compare-cell').evaluateAll(nodes=>nodes.map(n=>n.getBoundingClientRect().y));assert.equal(positions[0],positions[1]);
 await row.locator('button').nth(1).click();assert.equal(await row.locator('.compare-fields').isVisible(),false);await row.locator('button').nth(0).click();
 const before=await page.evaluate(()=>qa.calls.length);await page.evaluate(()=>state.reloadPage({automatic:true}));assert.equal(await page.evaluate(()=>qa.calls.length),before,'fixed cutoffs are not polled');
 await page.locator('[data-compare-until]').fill('2026-10-22T12:00');await page.locator('[data-compare-run]').click();assert.match(await page.locator('[data-compare-message]').innerText(),/不能晚于/);assert.equal(await row.count(),1);
 await page.locator('[data-compare-until]').fill('2026-10-22T15:00');await page.evaluate(()=>qa.fail=true);await page.locator('[data-compare-run]').click();await page.waitForFunction(()=>document.querySelector('[data-compare-message]').textContent.includes('保留上次'));assert.equal(await row.count(),1);
 await page.locator('[data-compare-run]').click();await page.waitForFunction(()=>!document.querySelector('[data-compare-run]').disabled);await openTree();await page.locator('[data-compare-object="2"]').waitFor();assert.equal(await page.locator('[data-compare-object]').count(),2,'automatic paging includes next objects');
 await page.locator('[data-compare-object="1"] button').first().click();await page.screenshot({path:path.join(root,'target/state-compare-dark.png')});
 await page.locator('#theme').click();await page.screenshot({path:path.join(root,'target/state-compare-light.png')});
 await page.setViewportSize({width:960,height:640});assert.equal(await page.evaluate(()=>document.querySelector('main').scrollWidth>document.querySelector('main').clientWidth),false,'minimum desktop size fits');
 await page.evaluate(()=>qa.filterPages=true);await page.locator('[data-compare-run]').click();await page.waitForFunction(()=>!document.querySelector('[data-compare-run]').disabled);await openTree();
 await page.locator('[data-compare-object="4"]').waitFor();await page.waitForFunction(()=>document.querySelector('[data-compare-object="5"]'));
 assert.equal(await page.locator('.compare-object:visible').count(),1,'unchanged pages do not hide later changes or stop paging');
 assert.equal(await page.locator('[data-compare-object="1"]').isVisible(),false);
 assert.match(await page.locator('.compare-filter-note:visible').innerText(),/1 个对象缺少可对比记录/);
 const changed=page.locator('[data-compare-object="4"]');await changed.locator('button').first().click();
 const filterCalls=await page.evaluate(()=>qa.calls.length);await page.locator('[data-compare-only-changes]').uncheck();assert.equal(await page.locator('.compare-object:visible').count(),5);assert.equal(await changed.locator('.compare-fields').isVisible(),true,'filter preserves expansion');
 await page.locator('[data-compare-only-changes]').check();assert.equal(await page.locator('.compare-object:visible').count(),1);assert.equal(await page.evaluate(()=>qa.calls.length),filterCalls,'filter reuses already loaded pairs');
 const filteredPositions=await changed.locator('.compare-object-row .compare-cell').evaluateAll(nodes=>nodes.map(n=>n.getBoundingClientRect().y));assert.equal(filteredPositions[0],filteredPositions[1],'filtered columns stay aligned');
 await page.locator('[data-compare-only-changes]').uncheck();await page.locator('[data-page="overview"]').click();await page.locator('[data-page="compare"]').click();await page.locator('.compare-category').first().waitFor();assert.equal(await page.locator('[data-compare-only-changes]').isChecked(),false,'filter preference survives navigation');
 await page.locator('[data-compare-only-changes]').check();await page.evaluate(()=>qa.same=true);await page.locator('[data-compare-run]').click();await page.waitForFunction(()=>!document.querySelector('[data-compare-run]').disabled);await openTree();await page.waitForFunction(()=>document.querySelector('[data-compare-object="5"]'));assert.equal(await page.locator('.compare-object:visible').count(),0);assert.equal(await page.locator('.compare-filter-note:visible').innerText(),'没有变化','complete unchanged branch has a clear empty state');
 await page.evaluate(()=>{qa.filterPages=false;qa.branchFail=true;});await page.locator('[data-compare-run]').click();await page.waitForFunction(()=>document.querySelector('[data-compare-summary]').textContent.includes('读取失败'));
 assert.doesNotMatch(await page.locator('[data-compare-summary]').innerText(),/0 项变化/,'read failure is not zero changes');assert.match(await page.locator('.compare-type').first().locator('.auto-page pre').textContent(),/查询超时/,'string IPC error is visible');
 await page.locator('.compare-type').first().locator('.auto-page button').click();await page.locator('[data-compare-object="2"]').waitFor();
 await page.evaluate(()=>qa.failNext=true);await page.locator('[data-compare-run]').click();await page.waitForFunction(()=>document.querySelector('[data-compare-progress-label]').textContent.includes('读取失败'));assert.equal(await page.locator('[data-compare-progress]').getAttribute('value'),'1','failed append preserves partial progress');assert.equal(await page.locator('[data-compare-object="1"]').count(),1);await page.locator('.compare-type').first().locator('.auto-page button').click();await page.locator('[data-compare-object="2"]').waitFor();assert.equal(await page.locator('[data-compare-progress]').getAttribute('value'),'2');
 await page.evaluate(()=>qa.branchDelay=300);await page.locator('[data-page="overview"]').click();await page.locator('[data-page="compare"]').click();await page.waitForFunction(()=>document.querySelector('[data-compare-summary]')?.textContent==='正在对比对象…');await page.locator('[data-compare-object="2"]').waitFor();assert.equal(await page.locator('.compare-type-row button').first().getAttribute('aria-expanded'),'true','tree reopens and loads after navigation');
 await page.locator('.compare-category-row button').first().click();await page.locator('[data-page="overview"]').click();const beforeCollapsed=await page.evaluate(()=>qa.calls.filter(q=>q.type_id).length);await page.locator('[data-page="compare"]').click();await page.locator('.compare-category').first().waitFor();assert.equal(await page.evaluate(()=>qa.calls.filter(q=>q.type_id).length),beforeCollapsed,'restored child stays idle under collapsed parent');await openTree();await page.locator('[data-compare-object="2"]').waitFor();
 await page.locator('[data-compare-from]').fill('2026-10-22T15:00');
 await page.evaluate(()=>qa.delay=100);await page.locator('[data-compare-run]').click();await page.locator('[data-page="overview"]').click();await page.waitForTimeout(200);assert.equal(await page.locator('#state-compare').count(),0,'late reply does not replace destination');
 assert.deepEqual(errors,[]);
 const {changes,equal}=require('../apps/desktop/ui/state-compare.js');assert(equal({b:2,a:1},{a:1,b:2}));assert.equal(changes({left:{availability:'not_recorded'},right:{availability:'recorded'}}),'added');assert.equal(changes({left:{availability:'unavailable'},right:{availability:'recorded'}}),'unavailable');
 console.log('State comparison: calendars, validation, lazy loading, synchronized rows, fixed watermark, paging, refresh stability, retries, navigation and both themes passed.');
}finally{await browser?.close();server.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
