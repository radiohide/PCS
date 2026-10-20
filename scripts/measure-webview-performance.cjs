'use strict';
// Finite, read-only route/render measurements in a dedicated local WebView2 test window.
// This is not an operating-system cold-start benchmark. Database and browser caches stay warm.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const arg=(name,fallback)=>{const i=process.argv.indexOf('--'+name);return i<0?fallback:process.argv[i+1];};
const endpoint=new URL(arg('endpoint')),expectedPid=Number(arg('expected-pid')),samples=Number(arg('samples','100'));
assert.equal(endpoint.protocol,'http:');assert.equal(endpoint.hostname,'127.0.0.1');assert.ok(endpoint.port);
assert.ok(Number.isSafeInteger(expectedPid)&&expectedPid>0);assert.ok(Number.isInteger(samples)&&samples>=1&&samples<=1000);
const requested=arg('cases')?.split(','),cases=['timeline','objects','overview'].filter(name=>!requested||requested.includes(name));
assert.ok(cases.length);const percentile=(data,p)=>data[Math.max(0,Math.ceil(data.length*p)-1)];
(async()=>{
 const browser=await chromium.connectOverCDP(endpoint.origin);let page;const errors=[],results=[];
 try{
  page=browser.contexts().flatMap(context=>context.pages()).find(page=>/tauri|localhost/.test(page.url()));assert.ok(page,'dedicated desktop page not found');
  await page.waitForFunction(()=>typeof state!=='undefined'&&state.status&&typeof render==='function',{},{timeout:30000});
  const before=await page.evaluate(async()=>{const status=await api('status'),monitoring=await api('monitoring_settings');return {pid:status.pid,data_dir:status.data_dir,elevated:monitoring.elevated,revision:status.read_model_revision,counts:status.counts,worker_state:status.projection_worker?.state};});
  assert.equal(before.pid,expectedPid);assert.equal(before.elevated,true);
  // Prevent the test from submitting collection/configuration/privilege requests. Each route
  // goes through the real app, Tauri bridge, query queue, IPC and DOM/layout code.
  await page.evaluate(()=>{
   state.autoRefresh=false;state.refreshEpoch++;
   // Tauri's exported core object may be frozen; wrap the application's mutable bridge.
   window.__performanceOriginalInvoke=window.invoke;
   const guardedInvoke=(command,args)=>{
    if(command!=='request'||!['query','status','monitoring_settings'].includes(args?.request?.operation))throw Error('Read-only test blocked '+command);
    return window.__performanceOriginalInvoke(command,args);
   };
   window.invoke=guardedInvoke;
   if(window.invoke!==guardedInvoke)throw Error('Could not install read-only bridge');
  });
  page.on('pageerror',error=>errors.push(error.message));
  for(const name of cases){
   const elapsed=[],failures=[];let last;
   for(let i=0;i<samples;i++){
    try{
     last=await page.evaluate(async name=>{
      if(name==='objects'){Object.assign(modelFilters,{search:'',category:'',type_id:'',lifecycle:'',sort:'name_asc'});modelWalk={cursor:null,watermark:null};objectView='list';}
      if(name==='timeline'){Object.assign(timelineFilters,{action:'',category:'',type_id:'',period:'',unassigned:true,routine:false});timelineWalk={cursor:null,watermark:null};}
      content.scrollTop=0;state.page=name;
      const started=performance.now();await render();const rendered=performance.now();
      const height=content.scrollHeight;await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
      const selector=name==='timeline'?'.timeline-entry':name==='objects'?'button[data-object-alias]':'.overview-action';
      return {ms:performance.now()-started,rendered_ms:rendered-started,rows:content.querySelectorAll(selector).length,dom_nodes:content.querySelectorAll('*').length,height,page_feedback:content.querySelector('#page-feedback')?.textContent||'',text:content.innerText.slice(0,300)};
     },name);
     assert.ok(last.rows>0,`${name} has no rendered rows: ${last.text}`);assert.equal(last.page_feedback,'');elapsed.push(last.ms);
    }catch(error){failures.push(error.message);if(failures.length>=3)break;}
   }
   elapsed.sort((a,b)=>a-b);
   const result={case:name,samples:elapsed.length,errors:failures.length,error_details:[...new Set(failures)],p50_ms:elapsed.length?percentile(elapsed,.5):null,p95_ms:elapsed.length?percentile(elapsed,.95):null,p99_ms:elapsed.length?percentile(elapsed,.99):null,last_rendered_rows:last?.rows,last_dom_nodes:last?.dom_nodes};
   result.passed=elapsed.length===samples&&!failures.length&&result.p95_ms<=2000;results.push(result);console.log(JSON.stringify(result));
  }
  const after=await page.evaluate(async()=>{const status=await api('status');return {pid:status.pid,revision:status.read_model_revision,counts:status.counts,worker_state:status.projection_worker?.state};});
  const report={at:new Date().toISOString(),engine:'actual fixed-directory desktop WebView2; actual read-only Tauri/IPC; collection settings unchanged',measurement:'warm route navigation through two animation frames; not OS/process cold start; verify collection activity separately from backend recovery',samples_per_case:samples,before,after,page_errors:errors,results,passed:before.pid===after.pid&&!errors.length&&results.every(result=>result.passed)};
  const output=path.resolve(arg('output','target/performance-production-webview.json')),root=fs.realpathSync(path.resolve(__dirname,'../target')),parent=fs.realpathSync(path.dirname(output));assert.ok(parent===root||parent.startsWith(root+path.sep));
  fs.writeFileSync(output,JSON.stringify(report,null,2));console.log(JSON.stringify({passed:report.passed,server_changed:before.pid!==after.pid}));if(!report.passed)process.exitCode=2;
 }finally{
  if(page)await page.evaluate(()=>{if(window.__performanceOriginalInvoke){window.invoke=window.__performanceOriginalInvoke;delete window.__performanceOriginalInvoke;}}).catch(()=>{});
  await browser.close();
 }
})().catch(error=>{console.error(error.message);process.exitCode=1;});
