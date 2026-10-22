'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const root=path.resolve(__dirname,'..'),ui=path.join(root,'apps/desktop/ui');
const samples=Object.fromEntries(JSON.parse(fs.readFileSync(path.join(root,'model/query-samples.json'),'utf8')).samples.map(s=>[s.name,s.response]));
const server=http.createServer((req,res)=>{const file=path.resolve(ui,'.'+new URL(req.url,'http://localhost').pathname);if(!file.startsWith(ui+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.json')?'application/json':file.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));});
(async()=>{await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;
try{
 browser=await chromium.launch({executablePath:process.env.PCSCOPE_CHROME_PATH,headless:true});const page=await browser.newPage({viewport:{width:1320,height:860}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(samples=>{
  window.setInterval=fn=>{window.qaTick=fn;return 1;};window.qa={running:0,max:0,calls:[],failCoverage:0,failRecent:0,version:1,delay:30};
  window.__TAURI__={core:{invoke:async(command,args)=>{
   const r=args?.request||{};qa.calls.push({command,request:r});
   if(r.operation==='status')return {pid:1,collecting:false,read_model_revision:String(qa.version),counts:{}};
   if(r.operation==='monitoring_settings')return {elevated:true,settings:{configured:true,mode:'enhanced'}};
   if(r.operation!=='query')throw Error('unexpected operation');
   qa.running++;qa.max=Math.max(qa.max,qa.running);
   try{
    if(qa.running>2)throw Error('查询繁忙');
    await new Promise(resolve=>setTimeout(resolve,qa.delay));const q=r.request.query;
    if(q==='source_coverage'&&qa.failCoverage-->0)throw Error('读取超时');
    if(q==='action_timeline'&&qa.failRecent-->0)throw Error('读取超时');
    if(q==='instance_list')return {status:'loaded',data:[],instance_total:12000+qa.version};
    if(q==='source_coverage')return {...samples[q],data:[...samples[q].data,{...samples[q].data[0],scope:'another scope'}]};
    if(q==='action_timeline')return {status:'loaded',data:Array.from({length:8},(_,i)=>({...samples.action_timeline.data[0],detail_id:100-i,context:{type_id:'startup.service',display_name:i?'示例服务 '+i:'Volume Shadow Copy'},action_id:'startup.service_state_changed',field_id:'state',before:'stopped',after:qa.version===1?'running':'paused',occurred_at:'2026-10-22T07:00:00Z',title:'服务运行状态变化'})),page:{has_more:false}};
    return samples[q]||{status:'empty',data:[]};
   }finally{qa.running--;}
  }}};
 },samples);
 await page.goto('http://127.0.0.1:'+server.address().port+'/index.html');
 await page.locator('[data-page="overview"]').click();await page.locator('.overview-action').first().waitFor();
 assert.equal(await page.evaluate(()=>qa.max),1);
 assert.equal(await page.locator('[data-overview-enabled]').innerText(),String(new Set(samples.source_coverage.data.filter(row=>row.configured).map(row=>row.source)).size),'source scopes are not extra collectors');
 await page.evaluate(()=>{qa.calls=[];window.shell=document.querySelector('#overview-content');window.row=document.querySelector('.overview-action');window.mutations=[];new MutationObserver(records=>mutations.push(...records.filter(r=>[...r.removedNodes].includes(shell)))).observe(document.querySelector('#content'),{childList:true,subtree:true});});
 await page.evaluate(()=>Promise.all([state.reloadPage({automatic:true}),state.reloadPage({automatic:true})]));
 assert.equal(await page.evaluate(()=>qa.calls.filter(c=>c.request.operation==='query').length),3,'single flight');
 assert.equal(await page.evaluate(()=>row===document.querySelector('.overview-action')),true,'unchanged row retained');
 assert.equal(await page.evaluate(()=>qa.calls.some(c=>c.request.request?.query==='type_catalog')),false);
 assert.equal(await page.evaluate(()=>qa.calls.find(c=>c.request.request?.query==='instance_list').request.request.include_counts),false);
 await page.locator('.overview-scope summary').click();
 await page.evaluate(()=>{document.querySelector('main').scrollTop=220;window.beforeScroll=document.querySelector('main').scrollTop;qa.version++;qa.failCoverage=1;});
 await page.evaluate(async()=>{state.lastAutomaticPageRefresh=0;await qaTick();});
 assert.equal(await page.evaluate(()=>shell===document.querySelector('#overview-content')),true);
 assert.equal(await page.evaluate(()=>document.querySelector('main').scrollTop===beforeScroll),true);
 assert.equal(await page.locator('.overview-scope').getAttribute('open'),'');
 assert.equal(await page.locator('[data-overview-error="coverage"]').isVisible(),false,'transient error stays quiet');
 assert.equal(await page.locator('[data-overview-objects]').innerText(),'12002');
 assert.equal(await page.evaluate(()=>row===document.querySelector('.overview-action')),true,'reading defers changed rows');
 assert.equal(await page.evaluate(()=>state.pendingPageRefresh),true,'latest button offers deferred records');
 await page.evaluate(()=>{qa.failCoverage=1;});await page.evaluate(()=>state.reloadPage({automatic:true}));
 assert.equal(await page.locator('[data-overview-error="coverage"]').isVisible(),true,'repeated failure names section');
 assert.match(await page.locator('[data-overview-error="coverage"]').innerText(),/采集概况暂未更新/);
 await page.locator('[data-overview-error="coverage"] button').click();await page.evaluate(()=>state.reloadPage());
 assert.equal(await page.evaluate(()=>row===document.querySelector('.overview-action')),false,'explicit reload applies changed rows');
 await page.evaluate(()=>{qa.failRecent=1;});await page.locator('#load-latest').click();
 await page.locator('[data-overview-error="recent"] button').waitFor();assert.equal(await page.locator('.overview-action').count(),8);
 await page.locator('[data-overview-error="recent"] button').click();await page.waitForFunction(()=>document.querySelector('[data-overview-error="recent"]').hidden);
 assert.equal(await page.evaluate(()=>mutations.length),0,'no whole-page removal');
 await page.evaluate(async()=>{qa.version=1;qa.delay=50;const auto=state.reloadPage({automatic:true});await state.reloadPage();await auto;});
 assert.match(await page.locator('.overview-action').first().innerText(),/运行中/,'manual click during auto refresh still applies latest rows');
 assert(!await page.evaluate(()=>qa.calls.some(x=>['start_service','switch_service'].includes(x.command))),'query failure never restarts service');
 await page.evaluate(()=>{document.querySelector('main').scrollTop=0;document.activeElement.blur();});
 await page.screenshot({path:path.join(root,'target/overview-dark.png')});
 await page.locator('#theme').click();await page.screenshot({path:path.join(root,'target/overview-light.png')});
 // Leaving while queries are outstanding must not paint over the next page.
 await page.evaluate(()=>{qa.delay=100;state.reloadPage();});await page.locator('[data-page="objects"]').click();await page.locator('#model-results').waitFor();await page.waitForTimeout(500);assert.equal(await page.locator('#overview-content').count(),0);
 assert.deepEqual(errors,[]);
 console.log('PASS overview: stable shell/rows/scroll/expanded state, cheap single-flight queries, isolated retry and quiet transient failure, navigation race, dark/light screenshots.');
}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}})().catch(e=>{console.error(e);process.exitCode=1;});
