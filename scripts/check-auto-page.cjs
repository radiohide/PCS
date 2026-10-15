'use strict';
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const root=path.resolve(__dirname,'..');
(async()=>{
 const browser=await chromium.launch({...(process.env.PCSCOPE_CHROME_PATH?{executablePath:process.env.PCSCOPE_CHROME_PATH}:{}),headless:true});
 try{
  const page=await browser.newPage({viewport:{width:900,height:700}}),errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.setContent('<style>.box{height:220px;overflow:auto;border:1px solid} .item{height:30px}.auto-page{min-height:40px}[hidden]{display:none!important}</style><div id="fixture"></div>');
  await page.addScriptTag({path:path.join(root,'apps/desktop/ui/auto-page.js')});
  await page.evaluate(()=>{
   window.qa={calls:[],active:0,max:0,fail:false,repeat:false,empty:false,delay:0};
   window.install=(name,{count=20,hasMore=true,visible=true}={})=>{
    const box=document.createElement('div');box.className='box';box.hidden=!visible;
    const list=document.createElement('div'),anchor=document.createElement('div');box.append(list,anchor);document.querySelector('#fixture').append(box);
    const record={name,box,list,anchor,pages:0};
    record.pager=PCScopeAutoPage.mount(anchor,{
     query:async cursor=>{
      qa.calls.push({name,cursor});qa.active++;qa.max=Math.max(qa.max,qa.active);
      try{if(qa.delay)await new Promise(resolve=>setTimeout(resolve,qa.delay));if(qa.fail&&cursor)throw Error('failure');
       return {status:'loaded',data:qa.empty?[]:Array.from({length:count},(_,i)=>({id:name+'-'+(cursor||'first')+'-'+i})),page:{has_more:hasMore&&(qa.empty||qa.repeat||!cursor),next_cursor:qa.repeat&&cursor?cursor:qa.empty?'empty-'+qa.calls.length:'next'}};
      }finally{qa.active--;}
     },
     apply:(response,{append})=>{record.pages++;if(!append)list.replaceChildren();for(const row of response.data){const item=document.createElement('div');item.className='item';item.textContent=row.id;list.append(item);}}
    });return record;
   };
   window.clear=()=>{PCScopeAutoPage.disposeWithin(document.querySelector('#fixture'));document.querySelector('#fixture').replaceChildren();qa.calls=[];qa.max=0;qa.fail=false;qa.repeat=false;qa.empty=false;qa.delay=0;};
   window.example=install('scroll');return example.pager.reset();
  });
  await page.waitForTimeout(300);assert.equal(await page.evaluate(()=>qa.calls.length),1,'offscreen sentinel must not fetch more');
  await page.evaluate(()=>{example.box.scrollTop=10000;});await page.waitForFunction(()=>example.pages===2);
  await page.waitForTimeout(300);assert.equal(await page.evaluate(()=>qa.calls.length),2,'terminal page must stop');
  await page.evaluate(async()=>{clear();qa.fail=true;window.example=install('failure');await example.pager.reset();example.box.scrollTop=10000;});
  await page.waitForFunction(()=>!example.anchor.querySelector('button').hidden);const failed=await page.evaluate(()=>qa.calls.length);
  await page.waitForTimeout(500);assert.equal(await page.evaluate(()=>qa.calls.length),failed,'errors must not auto-retry');assert.equal(await page.locator('.item').count(),20,'append failure retains existing rows');
  await page.evaluate(()=>qa.fail=false);await page.locator('.auto-page button').click();await page.waitForFunction(()=>example.pages===2);
  assert.equal(await page.evaluate(()=>qa.calls.length),failed+1,'explicit retry sends one request');
  await page.evaluate(async()=>{clear();qa.repeat=true;window.example=install('repeat',{count:1});await example.pager.reset();});
  await page.waitForFunction(()=>!example.anchor.querySelector('button').hidden);await page.waitForTimeout(300);assert.equal(await page.evaluate(()=>qa.calls.length),2,'unchanged cursor must stop');
  await page.evaluate(async()=>{clear();qa.empty=true;window.example=install('empty',{count:0});await example.pager.reset();});
  await page.waitForFunction(()=>!example.anchor.querySelector('button').hidden);assert.equal(await page.evaluate(()=>qa.calls.length),3,'empty pages are bounded');
  await page.evaluate(async()=>{clear();window.example=install('hidden',{count:1,visible:false});await example.pager.reset();});await page.waitForTimeout(250);assert.equal(await page.evaluate(()=>qa.calls.length),1);
  await page.evaluate(()=>{example.box.hidden=false;example.pager.resume();});await page.waitForFunction(()=>example.pages===2);
  await page.evaluate(async()=>{clear();window.example=install('disposed');await example.pager.reset();example.pager.dispose();example.box.scrollTop=10000;});await page.waitForTimeout(250);assert.equal(await page.evaluate(()=>qa.calls.length),1);
  await page.evaluate(async()=>{clear();qa.delay=60;window.one=install('one');window.two=install('two');await one.pager.reset();await two.pager.reset();qa.max=0;one.box.scrollTop=10000;two.box.scrollTop=10000;});
  await page.waitForFunction(()=>one.pages===2&&two.pages===2,null,{timeout:6000}).catch(async error=>{console.error(JSON.stringify(await page.evaluate(()=>({calls:qa.calls,pages:[one.pages,two.pages],rects:[one,two].map(x=>({box:x.box.getBoundingClientRect().toJSON(),anchor:x.anchor.getBoundingClientRect().toJSON(),hidden:x.anchor.hidden,scroll:x.box.scrollTop}))}))));throw error;});assert.equal(await page.evaluate(()=>qa.max),1,'automatic reads from different lists are serialized');
  await page.evaluate(async()=>{clear();qa.delay=120;window.example=install('stale',{count:1,hasMore:false});const first=example.pager.reset();example.pager.reset();await first;});await page.waitForFunction(()=>example.pages===1);assert.equal(await page.locator('.item').count(),1,'a reset rejects old results');
  assert.deepEqual(errors,[]);console.log('PASS auto pagination: visibility, end, retry, cursor progress, empty-page bound, hidden views, disposal, serial reads and stale responses');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
