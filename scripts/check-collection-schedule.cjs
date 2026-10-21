'use strict';
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const root=path.resolve(__dirname,'..');
(async()=>{
 const context=await chromium.launchPersistentContext(path.join(root,'target/collection-settings-browser-profile'),{headless:true,executablePath:process.env.PCSCOPE_CHROME_PATH||undefined,viewport:{width:1280,height:900}});
 try{
  const page=await context.newPage(),errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.setContent('<div id="app" data-theme="dark"><main></main></div>');
  await page.addStyleTag({path:path.join(root,'apps/desktop/ui/style.css')});
  await page.evaluate(()=>{
   window.content=document.querySelector('main');window.state={page:'plugins',autoRefresh:true,refreshEpoch:0};
   window.esc=value=>String(value??'').replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;');
   window.head=(title,text)=>'<h1>'+title+'</h1><p>'+text+'</p>';window.localTime=value=>value;window.errorText=error=>error.message;window.notice=()=>{};
   window.pluginSources={};window.fetch=async()=>({ok:true,json:async()=>({sources:[]})});window.setInterval=()=>0;
   window.qa={global:null,jobs:[{id:'scoped',enabled:true,seconds:300,minimum_seconds:30,phase:'idle'},{id:'network',enabled:false,seconds:900,minimum_seconds:30,phase:'idle'},{id:'eventlog',enabled:true,seconds:10,minimum_seconds:10,phase:'idle'}],calls:[]};
   window.api=async(operation,args)=>{
    qa.calls.push({operation,args});
    if(operation==='plugins')return structuredClone(qa.jobs);
    if(operation==='item_controls')return {items:[]};
    if(operation==='file_watch_settings')return {files:[]};
    if(operation==='directory_watch_settings')return {directories:[]};
    if(operation==='runtime_watch_settings')return {enabled:true,module_process_names:['sihost.exe']};
    if(operation==='global_collection_schedule')return {seconds:qa.global};
    if(operation==='set_global_collection_schedule'){
     if(qa.fail){qa.fail=false;throw Error('保存失败，请重试');}
     if(qa.hold)await new Promise(resolve=>qa.release=resolve);
     qa.global=args.seconds;qa.jobs.forEach(job=>job.seconds=args.seconds);
     return {settings:{seconds:qa.global},jobs:structuredClone(qa.jobs)};
    }
    if(operation==='set_plugin'){
     const job=qa.jobs.find(job=>job.id===args.id);Object.assign(job,args);return {jobs:structuredClone(qa.jobs)};
    }
    throw Error('unexpected request '+operation);
   };
  });
  await page.addScriptTag({path:path.join(root,'apps/desktop/ui/plugin-manager.js')});
  await page.evaluate(()=>renderPluginManager());
  const input=page.locator('[data-global-seconds]'),save=page.locator('[data-global-save]'),feedback=page.locator('[data-global-feedback]');
  await page.waitForFunction(()=>!document.querySelector('[data-global-save]').disabled);
  assert.equal(await input.inputValue(),'');
  await input.fill('29');await save.click();assert.match(await feedback.innerText(),/30–604800/);
  assert.equal(await page.evaluate(()=>qa.calls.filter(call=>call.operation==='set_global_collection_schedule').length),0);
  await input.fill('60');await page.evaluate(()=>qa.hold=true);await save.click();
  assert(await save.isDisabled());assert.equal(await page.locator('[data-save]:disabled').count(),3);
  await page.evaluate(()=>{qa.hold=false;qa.release();});await page.waitForFunction(()=>!document.querySelector('[data-global-save]').disabled);
  assert.deepEqual(await page.locator('[data-seconds]').evaluateAll(inputs=>inputs.map(input=>input.value)),['60','60','60']);
  assert.equal(await page.evaluate(()=>qa.jobs[1].enabled),false);
  const scoped=page.locator('[data-plugin-index="0"]');await scoped.locator('[data-details]').click();
  await scoped.locator('[data-seconds]').fill('120');await scoped.locator('[data-save]').click();
  await page.waitForFunction(()=>qa.jobs[0].seconds===120);assert.match(await feedback.innerText(),/1 项已单独调整/);
  const request=await page.evaluate(()=>qa.calls.filter(call=>call.operation==='set_plugin').at(-1));assert.deepEqual(request.args,{id:'scoped',seconds:120});
  await page.locator('[data-plugin-index="1"] [data-toggle]').click();
  assert.deepEqual((await page.evaluate(()=>qa.calls.filter(call=>call.operation==='set_plugin').at(-1))).args,{id:'network',enabled:true});
  await page.evaluate(()=>renderPluginManager());await page.waitForFunction(()=>!document.querySelector('[data-global-save]').disabled);
  assert.equal(await input.inputValue(),'60');assert.equal(await page.locator('[data-seconds]').first().inputValue(),'120');
  await save.click();await page.waitForFunction(()=>qa.jobs.every(job=>job.seconds===60));
  await page.evaluate(()=>qa.fail=true);await input.fill('90');await save.click();assert.match(await feedback.innerText(),/保存失败/);
  assert.deepEqual(await page.evaluate(()=>qa.jobs.map(job=>job.seconds)),[60,60,60]);
  await input.fill('60');await save.click();await page.waitForFunction(()=>!document.querySelector('[data-global-save]').disabled);
  assert.deepEqual(errors,[]);await page.screenshot({path:path.join(root,'target/collection-schedule-ui.png'),fullPage:true});
  console.log('PASS: global validation, write serialization, all periods, disabled preservation, later individual override, toggle-only patch, reload, global reapply, failed save');
 }finally{await context.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
