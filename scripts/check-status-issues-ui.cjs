'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const root=path.resolve(__dirname,'..'),ui=path.join(root,'apps/desktop/ui');
const samples=Object.fromEntries(JSON.parse(fs.readFileSync(path.join(root,'model/query-samples.json'),'utf8')).samples.map(s=>[s.name,s.response]));
const server=http.createServer((req,res)=>{const file=path.resolve(ui,'.'+new URL(req.url,'http://localhost').pathname);if(!file.startsWith(ui+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.json')?'application/json':file.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));});
(async()=>{await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;
try{
 browser=await chromium.launch({executablePath:process.env.PCSCOPE_CHROME_PATH,headless:true});const page=await browser.newPage({viewport:{width:1360,height:900}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(samples=>{
  window.qa={timers:[],online:true,reconnect:false,monitoring:0,calls:[],report:{pid:1,collecting:false,last_collection:{ok:true},counts:{}}};
  window.setInterval=(fn,delay)=>{qa.timers.push({fn,delay});return qa.timers.length;};
  window.__TAURI__={core:{invoke:async(command,args)=>{
   const r=args?.request||{};qa.calls.push(command+':'+r.operation);
   if(command==='start_service'){if(!qa.reconnect)throw Error('connect failed');qa.online=true;return {};}
   if(r.operation==='status'){if(!qa.online)throw Error('connect failed');return structuredClone(qa.report);}
   if(r.operation==='monitoring_settings'){qa.monitoring++;return {elevated:true,settings:{configured:true,mode:'enhanced'},report:{}};}
   if(r.operation==='collect')return {scheduled:true};
   if(r.operation==='query')return samples[r.request.query]||{status:'empty',data:[],counts:{}};
   throw Error('unexpected operation '+command+':'+r.operation);
  }}};
 },samples);
 await page.goto('http://127.0.0.1:'+server.address().port+'/index.html');await page.waitForFunction(()=>qa.monitoring>0);
 const tick=()=>page.evaluate(async()=>{for(const t of qa.timers.filter(t=>t.delay===3000))await t.fn();});
 assert.equal(await page.locator('#issues-toggle').isVisible(),false);assert.equal(await page.locator('#notice,#backend-notice').count(),0);
 await page.evaluate(()=>qa.report.collecting=true);await tick();await page.evaluate(()=>qa.report.collecting=false);await tick();assert.doesNotMatch(await page.locator('body').innerText(),/采集已结束/);
 await page.locator('#collect').click();assert.equal(await page.locator('#collect-feedback').innerText(),'已提交采集');assert.equal(await page.locator('#issues-toggle').isVisible(),false);
 await page.evaluate(()=>{qa.report.last_collection={ok:false,plugins:[{plugin:'system',ok:false,error:'<img src=x onerror="window.bad=true">'}]};qa.report.event_collection={channels:{System:{ok:false,error:'denied'}}};});await tick();
 assert.match(await page.locator('#issues-toggle').innerText(),/2 项/);assert.equal(await page.locator('#issues-panel').isVisible(),false);
 await page.locator('#issues-toggle').click();await page.locator('[data-issue="collection"] summary').click();await tick();assert.equal(await page.locator('[data-issue="collection"] details').getAttribute('open'),'');assert.equal(await page.locator('#issues-panel img').count(),0);
 await page.screenshot({path:path.join(root,'target/status-issues-dark.png')});
 await page.locator('[data-issues-close]').click();assert.match(await page.locator('#issues-toggle').innerText(),/2 项/);
 await page.locator('[data-page="overview"]').click();await page.locator('.overview-actions').waitFor();assert.match(await page.locator('#issues-toggle').innerText(),/2 项/);
 await page.locator('#auto-refresh').click();await page.evaluate(()=>{qa.report.last_collection={ok:true,plugins:[{plugin:'system',ok:true}]};qa.report.event_collection={channels:{System:{ok:true}}};});await tick();assert.equal(await page.locator('#issues-toggle').isVisible(),false,'recovery still clears exceptions while page refresh is paused');
 await page.evaluate(()=>qa.online=false);await tick();assert.match(await page.locator('#issues-toggle').innerText(),/1 项/);await page.locator('#issues-toggle').click();await page.evaluate(()=>qa.reconnect=true);await page.locator('[data-issue="backend"] [data-issue-action]').click();await page.waitForFunction(()=>document.querySelector('#issues-toggle').hidden);assert.equal(await page.locator('#issues-panel').isVisible(),false);
 await page.evaluate(()=>PCScopeIssues.monitoring({settings:{configured:true,mode:'enhanced'},elevated:false}));await page.locator('#issues-toggle').click();assert.match(await page.locator('[data-issue="permissions"]').innerText(),/检查采集权限/);await page.keyboard.press('Escape');assert.equal(await page.locator('#issues-toggle').getAttribute('aria-expanded'),'false');
 await page.setViewportSize({width:800,height:600});await page.locator('#theme').click();await page.locator('#issues-toggle').click();const box=await page.locator('#issues-panel').boundingBox();assert(box.x>=0&&box.y>=0&&box.x+box.width<=800&&box.y+box.height<=600);await page.screenshot({path:path.join(root,'target/status-issues-light-small.png')});
 assert.deepEqual(errors,[]);assert(!await page.evaluate(()=>qa.calls.some(call=>call.startsWith('switch_service'))));
 fs.writeFileSync(path.join(root,'target/status-issues-ui-qa.json'),JSON.stringify({passed:true,checks:['quiet normal collection','manual result near button','grouped failures','collapsed technical details and escaping','unchanged refresh preserves expanded details','closing and navigation retain unresolved issues','recovery while paused','backend reconnect action','permission navigation available','keyboard dismissal','small viewport and both themes'],page_errors:errors},null,2));
 console.log('PASS status bar issue list: 11 UI checks');
}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}})().catch(e=>{console.error(e);process.exitCode=1;});
