'use strict';
// Same retained sample in both real UIs. Fixed 60ms bridge delay isolates frontend work
// and request count; this is NOT a production end-to-end latency measurement.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const arg=(name,fallback)=>{const i=process.argv.indexOf('--'+name);return i<0?fallback:process.argv[i+1];};
const sample=JSON.parse(fs.readFileSync(arg('sample'),'utf8')),baseline=path.resolve(arg('baseline')),current=path.resolve('apps/desktop/ui');
const output=arg('output','target/timeline-replay.json'),transportLimit=Number(arg('limit','400')),cases=[];
const server=http.createServer((req,res)=>{
 const url=new URL(req.url,'http://localhost'),old=url.pathname.startsWith('/old/'),root=old?baseline:current;
 const file=path.resolve(root,url.pathname.slice(old?5:5)||'index.html');
 if(!file.startsWith(root+path.sep)||!fs.existsSync(file))return res.writeHead(404).end();
 res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));
});
(async()=>{
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const browser=await chromium.launch({executablePath:process.env.PCSCOPE_CHROME_PATH,headless:true});
 try{
  for(const variant of ['old','new'])for(const groups of [2,6]){
   const page=await browser.newPage({viewport:{width:1440,height:1000},timezoneId:'Asia/Singapore'}),errors=[];
   page.on('pageerror',e=>errors.push(e.message));
   await page.addInitScript(({sample,groups,transportLimit})=>{
    localStorage.setItem('pcscope.timelineGroupsPerLoad',String(groups));localStorage.setItem('pcscope.autoRefresh','false');
    window.__requests=0;window.__TAURI__={core:{invoke:async(_command,{request:r})=>{
     if(r.operation==='status')return {read_model_revision:'fixed',counts:{}};
     if(r.operation==='monitoring_settings')return {settings:{configured:true,mode:'low'},report:{audits:[],logs:[]}};
     if(r.operation!=='query')throw Error('read-only replay');
     const q=r.request;
     if(q.query!=='action_timeline')return {status:'empty',data:[],page:{has_more:false}};
     if(q.cards_only)q.limit=transportLimit;
     window.__requests++;await new Promise(resolve=>setTimeout(resolve,60));
     const from=Number(q.cursor||0),data=q.cards_only?sample.cards:sample.full,to=Math.min(from+q.limit,data.length);
     return {status:'loaded',data:data.slice(from,to),page:{watermark:'fixed',has_more:to<data.length,next_cursor:to<data.length?String(to):null}};
    }}};
   },{sample,groups,transportLimit});
   await page.goto('http://127.0.0.1:'+server.address().port+'/'+variant+'/index.html');
   await page.waitForFunction(()=>typeof state!=='undefined'&&state.status);
   await page.evaluate(()=>{state.autoRefresh=false;timelineCollapsed=false;PCScopeTimelineTime.setMinutes(5);});
   await page.waitForFunction(()=>document.querySelector('#timeline-results')?.getAttribute('aria-busy')==='false');
   for(let index=0;index<3;index++){
    const before=await page.evaluate(()=>({rows:document.querySelectorAll('.timeline-branch').length,requests:window.__requests}));
    if(index===0)before.rows=0;
    const start=performance.now();
    if(index===0)await page.locator('[data-page=timeline]').click();
    else await page.evaluate(()=>{const main=document.querySelector('main');main.dispatchEvent(new WheelEvent('wheel',{deltaY:240,bubbles:true}));main.scrollTop=main.scrollHeight-main.clientHeight;main.dispatchEvent(new Event('scroll'));});
    await page.waitForFunction(before=>document.querySelector('#timeline-results')?.getAttribute('aria-busy')==='false'&&document.querySelectorAll('.timeline-branch').length>before,before.rows);
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const metrics=await page.evaluate(()=>({rows:document.querySelectorAll('.timeline-branch').length,loaded_groups:document.querySelectorAll('.timeline-group').length,cards:document.querySelectorAll('.timeline-entry').length,nodes:document.querySelector('#timeline-results').querySelectorAll('*').length,requests:window.__requests,counts:[...document.querySelectorAll('.timeline-group-count')].map(n=>n.textContent)}));
    cases.push({variant,groups,index,ms:performance.now()-start,added:metrics.rows-before.rows,queries:metrics.requests-before.requests,...metrics});
    console.log(JSON.stringify(cases.at(-1)));
   }
   assert.deepEqual(errors,[]);
   await page.close();
  }
  for(const old of cases.filter(x=>x.variant==='old')){
   const fresh=cases.find(x=>x.variant==='new'&&x.groups===old.groups&&x.index===old.index);
   assert.equal(fresh.rows,old.rows);assert.deepEqual(fresh.counts,old.counts);
  }
  fs.writeFileSync(output,JSON.stringify({basis:'Same real 10000-row sample, real app.js, fixed 60ms mock bridge delay, no production latency claim',transportLimit,cases},null,2));
 }finally{await browser.close();await new Promise(resolve=>server.close(resolve));}
})().catch(e=>{console.error(e);process.exitCode=1;});
