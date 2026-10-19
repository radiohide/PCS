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
  window.setInterval=()=>1;window.qa={calls:[],active:0,max:0,fail:false,delay:0,detailDelay:0};
  const types=[{type_id:'hardware.device',category:'hardware',name:'硬件设备',supplied_by_source:true},{type_id:'policy.power_plan',category:'policy',name:'电源计划',supplied_by_source:true},{type_id:'future.object',category:'future',name:'新增对象',supplied_by_source:true}];
  const rows=Array.from({length:155},(_,i)=>({alias:'device-'+i,type_id:'hardware.device',category:'hardware',type_name:'硬件设备',display_name:'设备 '+String(i).padStart(2,'0'),lifecycle:i===154?'removed':'active',identity_status:'confirmed',key_facts:[],coverage:[]}));
  rows.push({alias:'power',type_id:'policy.power_plan',category:'policy',type_name:'电源计划',display_name:'Balanced',lifecycle:'active',identity_status:'confirmed',key_facts:[{field_id:'active',status:'known',value:true}],coverage:[]});
  rows.push({alias:'future',type_id:'future.object',category:'future',type_name:'新增对象',display_name:'<img src=x onerror=window.bad=true>',lifecycle:'active',identity_status:'confirmed',key_facts:[],coverage:[]});
  qa.rows=rows;qa.types=types;
  window.__TAURI__={core:{invoke:async(command,args)=>{
   const r=args?.request||{};if(r.operation==='status')return {pid:1,collecting:false,counts:{}};
   if(r.operation==='monitoring_settings')return {elevated:true,settings:{configured:true,mode:'enhanced'}};
   if(r.operation!=='query')throw Error('unexpected operation '+command+':'+r.operation);
   const q=r.request;qa.calls.push(structuredClone(q));qa.active++;qa.max=Math.max(qa.max,qa.active);
   try{
    if(q.query==='type_catalog')return {status:'loaded',data:types};
    if(q.query==='instance_detail'){if(qa.detailDelay)await new Promise(r=>setTimeout(r,qa.detailDelay));return {status:'loaded',data:{summary:rows.find(x=>x.alias===q.instance),fields:[],bindings:[],actions_preview:[]}};}
    if(q.query==='instance_list'){
     if(qa.delay)await new Promise(r=>setTimeout(r,qa.delay));if(qa.fail){qa.fail=false;throw Error('deadline has elapsed');}
     let matched=rows.filter(x=>(!q.category||x.category===q.category)&&(!q.type_id||x.type_id===q.type_id)&&(!q.search||x.display_name.includes(q.search))&&(!q.lifecycle?['active','not_observed'].includes(x.lifecycle):q.lifecycle==='any'||x.lifecycle===q.lifecycle));
     if(q.sort==='name_desc')matched.reverse();const offset=Number(q.cursor||0),limit=q.limit||50,next=offset+limit;
     return {status:matched.length?'loaded':'empty',data:matched.slice(offset,next),...(q.include_counts===false?{}:{counts:{instances:matched.length}}),...(q.include_instance_total?{instance_total:matched.length}:{}),page:{has_more:next<matched.length,next_cursor:next<matched.length?String(next):null,watermark:'test'}};
    }
    return samples[q.query]||{status:'empty',data:[],counts:{}};
   }finally{qa.active--;}
  }}};
 },samples);
 await page.goto('http://127.0.0.1:'+server.address().port+'/index.html');await page.locator('[data-page="objects"]').click();await page.locator('[data-object-alias]').first().waitFor();
 await page.evaluate(()=>qa.calls=[]);await page.locator('[data-object-view="tree"]').click();await page.locator('.object-tree-layout').waitFor();
 assert.equal(await page.locator('[data-tree-type]').count(),3);assert.equal(await page.evaluate(()=>qa.calls.filter(q=>q.query==='instance_list').length),0,'closed branches must not trigger object queries');
 await page.locator('[data-tree-category="hardware"]>summary').click();await page.locator('[data-tree-type="hardware.device"]>summary').click();await page.locator('[data-tree-object]').first().waitFor();
 assert.equal(await page.locator('[data-tree-object]').count(),100);await page.locator('[data-branch-more]:visible').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('[data-tree-object]').length===154);
 assert(await page.evaluate(()=>qa.calls.filter(q=>q.query==='instance_list').every(q=>q.limit===100)),'tree branches use 100-row requests');
 assert(await page.evaluate(()=>qa.calls.filter(q=>q.query==='instance_list').every(q=>q.include_counts===false)),'opening branches must not count related history');
 assert(await page.evaluate(()=>qa.calls.filter(q=>q.query==='instance_list').every(q=>q.include_instance_total===!q.cursor)),'only first branch pages request exact object totals');
 assert.equal(await page.locator('[data-tree-type="hardware.device"] [data-tree-count]').innerText(),'154 个');
 await page.locator('[data-tree-object="device-0"]').click();await page.locator('.object-tree-detail h3').waitFor();assert.equal(await page.locator('.object-tree-detail details[open]').count(),0);
 await page.locator('[data-tree-type="hardware.device"]>summary').click();const before=await page.evaluate(()=>qa.calls.length);await page.locator('[data-tree-type="hardware.device"]>summary').click();assert.equal(await page.evaluate(()=>qa.calls.length),before,'cached expansion does not requery');
 await page.locator('#model-lifecycle').selectOption('any');await page.waitForFunction(()=>document.querySelector('[data-tree-type="hardware.device"] [data-tree-count]')?.textContent==='155 个');await page.locator('[data-branch-more]:visible').scrollIntoViewIfNeeded();await page.locator('[data-tree-object="device-154"]').waitFor();
 await page.locator('[data-tree-category="other"]>summary').click();await page.locator('[data-tree-type="future.object"]>summary').click();await page.locator('[data-tree-object="future"]').waitFor();assert.equal(await page.locator('.object-tree-browser img').count(),0);
 await page.locator('[data-tree-category="policy"]>summary').click();await page.locator('[data-tree-type="policy.power_plan"]>summary').click();await page.locator('[data-tree-object="power"]').click();await page.waitForFunction(()=>document.querySelector('.object-tree-detail h3')?.textContent===PCScopeObjectNames.name(qa.rows.find(x=>x.alias==='power')));
 await page.screenshot({path:path.join(root,'target/object-tree-dark.png')});
 const aliases=await page.locator('[data-tree-object]').evaluateAll(nodes=>nodes.map(x=>x.dataset.treeObject).sort());assert.deepEqual(aliases,await page.evaluate(()=>qa.rows.map(x=>x.alias).sort()),'all lazy branches cover exactly the list population including history and new categories');
 await page.locator('#model-search').fill('设备');await page.waitForFunction(()=>document.querySelector('[data-tree-feedback]')?.textContent==='找到 155 个对象，已显示 100 个');await page.locator('[data-tree-more]').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('[data-tree-object]').length===155);
 const treeSearch=await page.locator('[data-tree-object]').evaluateAll(nodes=>nodes.map(x=>x.dataset.treeObject).sort());
 await page.locator('[data-object-view="list"]').click();await page.locator('[data-object-alias]').first().waitFor();await page.locator('#model-more').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('[data-object-alias]').length===155);assert.deepEqual(await page.locator('[data-object-alias]').evaluateAll(nodes=>nodes.map(x=>x.dataset.objectAlias).sort()),treeSearch);
 await page.evaluate(()=>qa.fail=true);await page.locator('[data-object-view="tree"]').click();await page.locator('[data-tree-more] button').waitFor();await page.locator('[data-tree-more] button').click();await page.locator('[data-tree-object]').first().waitFor();
 await page.locator('#model-search').fill('不存在的对象');await page.waitForFunction(()=>document.querySelector('[data-tree-feedback]')?.textContent.includes('没有匹配'));
 await page.evaluate(()=>qa.delay=150);await page.locator('#model-search').fill('设备');await page.waitForFunction(()=>qa.active>0);await page.locator('#model-search').fill('Balanced');await page.waitForFunction(()=>document.querySelectorAll('[data-tree-object]').length===1&&document.querySelector('[data-tree-object="power"]'));assert.equal(await page.locator('[data-tree-object="device-0"]').count(),0);
 await page.evaluate(()=>qa.delay=0);await page.locator('[data-tree-object="power"]').click();await page.locator('.object-tree-detail h3').waitFor();await page.setViewportSize({width:800,height:600});await page.locator('#theme').click();await page.screenshot({path:path.join(root,'target/object-tree-light-small.png')});
 assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'no horizontal viewport overflow');assert.deepEqual(errors,[]);assert.equal(await page.evaluate(()=>qa.max),1);
 fs.writeFileSync(path.join(root,'target/object-tree-ui-qa.json'),JSON.stringify({passed:true,checks:['lazy loading','branch pagination','readable shared detail','cached expansion','history filter','unknown category fallback','escaping','all-object coverage','search ancestry and pagination','list/tree set equality','failure retry','empty search','stale response rejection','both themes and small viewport','serial queries'],page_errors:errors},null,2));
 console.log('PASS object tree: 15 interaction checks');
}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}})().catch(e=>{console.error(e);process.exitCode=1;});
