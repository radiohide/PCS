'use strict';
// Actual WebView2 + native IPC. Read-only, finite group-load measurements; no collector changes.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const arg=(name,fallback)=>{const i=process.argv.indexOf('--'+name);return i<0?fallback:process.argv[i+1];};
const endpoint=new URL(arg('endpoint')),expectedPid=Number(arg('expected-pid'));
const cases=arg('groups','2,4,6').split(',').map(Number),rounds=Number(arg('rounds','2')),pages=Number(arg('pages','3'));
assert.equal(endpoint.protocol,'http:');assert.equal(endpoint.hostname,'127.0.0.1');assert(endpoint.port);assert(Number.isSafeInteger(expectedPid)&&expectedPid>0);
assert(cases.length&&cases.every(n=>Number.isInteger(n)&&n>=2&&n<=6));assert(rounds>=1&&rounds<=5&&pages>=1&&pages<=10);
(async()=>{
 const browser=await chromium.connectOverCDP(endpoint.origin);let page;const errors=[],samples=[];
 try{
  for(let attempt=0;attempt<40&&!page;attempt++){page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));if(!page)await new Promise(resolve=>setTimeout(resolve,250));}assert(page,'desktop page did not open');
  await page.waitForFunction(()=>typeof state!=='undefined'&&state.status);
  page.on('pageerror',e=>errors.push(e.message));
  const identity=()=>page.evaluate(async()=>{const status=await api('status'),monitoring=await api('monitoring_settings');return {pid:status.pid,elevated:monitoring.elevated,data_dir:status.data_dir,revision:status.read_model_revision,counts:status.counts};});
  const before=await identity();assert.equal(before.pid,expectedPid);assert.equal(before.elevated,true);
  await page.evaluate(()=>{
   state.autoRefresh=false;state.refreshEpoch++;timelineCollapsed=false;PCScopeTimelineTime.setMinutes(5);
   const m=window.__groupMeasure={ready:false,current:null,originalInvoke:window.invoke,originalMount:PCScopeAutoPage.mount,originalGrouping:PCScopeTimelineTime.groupRows};
   window.invoke=async(command,args)=>{
    if(command!=='request'||!['status','query','monitoring_settings'].includes(args?.request?.operation))throw Error('Read-only benchmark blocked '+command);
    const trial=args?.request?.operation==='query'&&args.request.request?.query==='action_timeline'?m.current:null,start=performance.now();
    try{return await m.originalInvoke(command,args);}finally{if(trial){trial.transport_ms+=performance.now()-start;trial.requests++;}}
   };
   PCScopeTimelineTime.groupRows=function(rows){const start=performance.now();try{return m.originalGrouping(rows);}finally{if(m.current)m.current.grouping_ms+=performance.now()-start;}};
   PCScopeAutoPage.mount=function(anchor,options){
    if(anchor.id!=='timeline-more')return m.originalMount(anchor,options);
    return m.originalMount(anchor,{...options,canAutoLoad:()=>m.ready&&options.canAutoLoad(),
     query:async cursor=>{
      const trial=m.current;if(!trial)throw Error('Unexpected page');m.ready=false;
      trial.request_at=performance.now();trial.scheduling_ms=trial.request_at-trial.start;
      const root=document.querySelector('#timeline-results'),oldKeys=new Set([...root.querySelectorAll('.timeline-group')].map(g=>g.dataset.timeGroup));
      const observer=new MutationObserver(()=>{
       if(trial.first_content_ms===undefined&&[...root.querySelectorAll('.timeline-group')].some(g=>!oldKeys.has(g.dataset.timeGroup)&&g.querySelector('.timeline-entry'))){
        trial.first_dom_ms=performance.now()-trial.start;trial.first_content_ms=null;
        requestAnimationFrame(()=>requestAnimationFrame(()=>{trial.first_content_ms=performance.now()-trial.start;}));
       }
      });observer.observe(root,{subtree:true,childList:true});
      try{
       const response=await options.query(cursor);trial.rows=response.data.length;trial.has_more=!!response.page?.has_more;trial.group_page=response.group_page;
       trial.new_groups=[...root.querySelectorAll('.timeline-group')].filter(g=>!oldKeys.has(g.dataset.timeGroup)).length;
       trial.read_complete_ms=performance.now()-trial.start;return response;
      }finally{observer.disconnect();}
     },
     apply:async(response,context)=>{
      await options.apply(response,context);const trial=m.current;
      trial.height=content.scrollHeight;await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
      trial.total_ms=performance.now()-trial.start;trial.loaded_rows=document.querySelectorAll('.timeline-branch').length;
      trial.mounted_cards=document.querySelectorAll('.timeline-entry').length;
      trial.partial_labels=[...document.querySelectorAll('.timeline-group-count')].some(n=>n.textContent.includes('已加载'));
      trial.dom_nodes=content.querySelectorAll('*').length;trial.done=true;
     }
    });
   };
  });
  for(const groups of cases)for(let round=0;round<rounds;round++){
   await page.evaluate(async({groups,round})=>{
    const m=window.__groupMeasure;m.ready=false;timelineGroupsPerLoad=groups;
    Object.assign(timelineFilters,{action:'',category:'',type_id:'',period:'',unassigned:true,routine:false});timelineWalk={cursor:null,watermark:null};state.page='timeline';content.closest('main').scrollTop=0;
    m.current={groups,round,index:-1,phase:'initial',start:performance.now(),grouping_ms:0,transport_ms:0,requests:0};await render();
   },{groups,round});
   const collect=async()=>{
    await page.waitForFunction(()=>window.__groupMeasure.current?.done||!document.querySelector('#timeline-more button').hidden,{},{timeout:120000});
    const result=await page.evaluate(()=>({trial:window.__groupMeasure.current,error:document.querySelector('#timeline-more button').hidden?null:document.querySelector('#timeline-more').innerText}));
    assert.equal(result.error,null);assert(result.trial.done);assert(result.trial.group_page?.complete);assert.equal(result.trial.partial_labels,false);
    if(result.trial.has_more)assert.equal(result.trial.group_page.groups,result.trial.groups);
    assert.equal(result.trial.new_groups,result.trial.group_page.groups);
    samples.push(result.trial);console.log(JSON.stringify(result.trial));return result.trial.has_more;
   };
   let more=await collect();
   for(let index=0;index<pages&&more;index++){
    await page.evaluate(({groups,round,index})=>{
     const m=window.__groupMeasure,anchor=document.querySelector('#timeline-more'),scroller=content.closest('main');
     m.current={groups,round,index,phase:'scroll',start:performance.now(),grouping_ms:0,transport_ms:0,requests:0};m.ready=true;
     const bounds=scroller.getBoundingClientRect(),rect=anchor.getBoundingClientRect();
     scroller.scrollTop+=rect.top-bounds.bottom-Math.min(1200,scroller.clientHeight)*0.5;
     content.dispatchEvent(new WheelEvent('wheel',{deltaY:1,bubbles:true}));
    },{groups,round,index});more=await collect();
   }
  }
  const after=await identity();const summary=[];
  for(const groups of cases)for(const phase of ['initial','scroll']){
   const selected=samples.filter(s=>s.groups===groups&&s.phase===phase),stats={groups,phase,samples:selected.length};
   for(const key of ['rows','total_ms','first_content_ms','transport_ms','grouping_ms']){
    const values=selected.map(s=>s[key]).filter(Number.isFinite).sort((a,b)=>a-b);
    stats[key]=values.length?{min:values[0],p50:values[Math.ceil(values.length*.5)-1],p95:values[Math.ceil(values.length*.95)-1],max:values.at(-1)}:null;
   }summary.push(stats);
  }
  const report={at:new Date().toISOString(),measurement:'Actual fixed-directory WebView2, current full history, 5-minute groups, main changes, expanded mode, collection continues. Initial and automatic scroll batches; warm runs. First content uses two frames after first new card; total includes group completion, layout and two frames. No collector or product settings written.',before,after,samples,summary,errors,passed:before.pid===after.pid&&after.elevated&&!errors.length};
  const output=path.resolve(arg('output','target/timeline-group-performance.json')),root=fs.realpathSync(path.resolve(__dirname,'../target')),parent=fs.realpathSync(path.dirname(output));assert(parent===root||parent.startsWith(root+path.sep));
  fs.writeFileSync(output,JSON.stringify(report,null,2));console.log(JSON.stringify({passed:report.passed,summary}));if(!report.passed)process.exitCode=2;
 }finally{
  if(page)await page.evaluate(()=>{const m=window.__groupMeasure;if(m){m.ready=false;window.invoke=m.originalInvoke;PCScopeAutoPage.mount=m.originalMount;PCScopeTimelineTime.groupRows=m.originalGrouping;}}).catch(()=>{});
  await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1;});
