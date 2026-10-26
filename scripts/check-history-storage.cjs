'use strict';
const assert=require('node:assert/strict'),path=require('node:path');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.PCSCOPE_CHROME_PATH});
 try{
  const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.setContent('<main></main>');
  for(const file of ['health-storage.js','history-delete.js','settings.js'])await page.addScriptTag({path:path.resolve(__dirname,'../apps/desktop/ui',file)});
  await page.evaluate(()=>{
   window.qa={calls:[],result:{state:'idle'},complete:0,retention:{can_delete_history:true,history_id:'original',physical_bytes:20000000000,quota_bytes:5e9,mode:'unlimited',content_usage:{available:true,started_at:'2026-10-26T00:00:00Z',categories:[{category:'network',retained_bytes:1000}],sources:[{name:'网络连接',category:'network',retained_bytes:1000}]}}};
   qa.mount=()=>{qa.settings=PCScopeSettings.mount(document.querySelector('main'),{api:async(op,args)=>{
    qa.calls.push({op,args});if(op==='retention')return structuredClone(qa.retention);
    if(op==='history_deletion_status'){if(qa.disconnected)throw Error('disconnected');return structuredClone(qa.result);}
    if(op==='delete_history'){if(qa.reject)throw Error('stale history');qa.result={state:'pending',id:'op1',history_id:args.history_id};if(qa.lostAck)throw Error('lost acknowledgment');return structuredClone(qa.result);}
    if(op==='set_retention')return {};throw Error(op);
   },esc:v=>String(v??''),localTime:v=>v,notice:()=>{},errorText:String,getTheme:()=> 'dark',setTheme:()=>{},onPrivacy:()=>qa.complete++,categoryNames:{network:'网络'}});};qa.mount();
  });
  await page.locator('#settings-tab-storage').click();await page.locator('[data-storage-bytes]').waitFor();
  assert.match(await page.locator('#settings-storage').innerText(),/20 GB/);await page.locator('.health-storage-breakdown summary').click();assert.match(await page.locator('#settings-storage').innerText(),/网络连接/);
  await page.locator('[data-delete-open]').click();assert(await page.locator('[data-delete-submit]').isDisabled());
  await page.locator('[data-delete-cancel]').click();assert.equal(await page.evaluate(()=>qa.calls.filter(c=>c.op==='delete_history').length),0);
  await page.locator('[data-delete-open]').click();await page.locator('[data-delete-confirm]').check();await page.locator('[data-delete-submit]').click();
  assert(await page.locator('[data-delete-open]').isDisabled());assert.equal(await page.evaluate(()=>qa.calls.filter(c=>c.op==='delete_history').length),1);
  await page.locator('[data-delete-submit]').evaluate(b=>b.click());assert.equal(await page.evaluate(()=>qa.calls.filter(c=>c.op==='delete_history').length),1);
  await page.evaluate(()=>{qa.disconnected=true;});await page.waitForTimeout(1650);assert.match(await page.locator('[data-delete-status]').textContent(),/等待重新连接/);
  await page.evaluate(()=>{qa.disconnected=false;qa.retention.history_id='new';qa.retention.physical_bytes=1000000;qa.result={state:'completed',id:'op1',history_id:'original'};});
  await page.waitForFunction(()=>qa.complete===1);assert.match(await page.locator('[data-storage-bytes]').textContent(),/1 MB/);
  assert.equal(await page.evaluate(()=>qa.calls.find(c=>c.op==='delete_history').args.history_id),'original');
  await page.evaluate(()=>{qa.lostAck=true;});await page.locator('[data-delete-open]').click();await page.locator('[data-delete-confirm]').check();await page.locator('[data-delete-submit]').click();
  assert.equal(await page.evaluate(()=>qa.calls.filter(c=>c.op==='delete_history').length),2,'lost acknowledgment never resends deletion');
  await page.evaluate(()=>{qa.result={state:'failed',id:'op1',history_id:'new',error:'文件仍在使用，请关闭数据库工具后重试。'};});
  await page.waitForFunction(()=>document.querySelector('[data-delete-status]').textContent.includes('文件仍在使用'));
  assert(await page.locator('[data-delete-open]').isEnabled());
  await page.evaluate(()=>{qa.reject=true;qa.result={state:'completed',id:'old',history_id:'original'};});
  await page.locator('[data-delete-open]').click();await page.locator('[data-delete-confirm]').check();await page.locator('[data-delete-submit]').click();
  assert.match(await page.locator('[data-delete-status]').textContent(),/删除未提交/);assert.equal(await page.evaluate(()=>qa.complete),1,'old receipt cannot acknowledge a different deletion');
  await page.evaluate(()=>{qa.result={state:'pending',id:'ongoing'};qa.settings.dispose();qa.mount();});
  await page.waitForFunction(()=>document.querySelector('[data-delete-status]')?.textContent.includes('结束采集'));
  await page.evaluate(()=>qa.settings.dispose());const calls=await page.evaluate(()=>qa.calls.length);await page.waitForTimeout(1700);assert.equal(await page.evaluate(()=>qa.calls.length),calls,'leaving settings stops polling');
  assert.deepEqual(errors,[]);console.log('PASS storage settings: relocated metrics, explicit confirmation/cancel, single submission, restart/reconnect, lost acknowledgment, failure, fresh history identity, resumed progress and disposal');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
