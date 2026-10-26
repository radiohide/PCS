'use strict';
// Exercise sticky controls with the real styles, filtering and distant-card windowing.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const ui=path.resolve(__dirname,'../apps/desktop/ui');
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.PCSCOPE_CHROME_PATH});
 try{
  const page=await browser.newPage({viewport:{width:1440,height:800}}),errors=[],results=[];
  page.on('pageerror',e=>errors.push(e.message));
  for(const width of [1440,960,640])for(const virtual of [false,true]){
   await page.goto('about:blank');
   await page.setViewportSize({width,height:800});
   await page.setContent('<div id="app" data-theme="dark"><main><div id="timeline-results"><div class="timeline-list"></div></div></main></div>');
   await page.addStyleTag({path:path.join(ui,'style.css')});
   await page.addStyleTag({content:'html,body{margin:0;height:100%}#app{display:block;height:100vh}#app main{height:100vh;box-sizing:border-box;overflow:auto;padding:20px} .fixture-card{height:180px;padding:16px;box-sizing:border-box}'});
   for(const name of ['timeline-time.js','timeline-groups.js','timeline-window.js'])await page.addScriptTag({path:path.join(ui,name)});
   await page.evaluate(virtual=>{
    const group=(id,categories)=>`<section class="timeline-group" data-time-group="${id}" data-time-meta='{"key":"${id}"}'><aside class="timeline-time-rail"><div class="timeline-when"><div class="timeline-time-block"><div class="timeline-time-heading"><span>时间段</span><button data-timeline-toggle><span class="timeline-fold-label">收起</span></button></div><button data-timeline-time-toggle><span class="timeline-time-value">13:45 至 13:50</span></button></div><div class="timeline-category-actions"><button data-timeline-show-all>全选</button><button data-timeline-invert>反选</button></div><div class="timeline-category-controls"></div><div><span class="timeline-group-count"></span><div class="timeline-category-footer"><span data-timeline-visible></span></div></div></div></aside><div class="timeline-branches">${categories.map((cat,i)=>`<div class="timeline-branch" data-timeline-category="${cat}" data-timeline-order="${i}" data-timeline-moment='{"key":"${id}"}'><article class="fixture-card">${cat} ${i}</article></div>`).join('')}</div><div class="timeline-group-empty" hidden>本组类别均已隐藏</div></section>`;
    const categories=['计划任务','计划任务',...Array(8).fill('登录会话'),...Array(5).fill('网络路由'),...Array(5).fill('IP 地址')];
    document.querySelector('.timeline-list').innerHTML=group('previous',Array(5).fill('前组'))+group('target',categories)+group('next',Array(virtual?1100:8).fill('后组'));
    const root=document.querySelector('#timeline-results');window.win=PCScopeTimelineWindow.mount(root);window.groups=PCScopeTimelineGroups.mount(root,{track:win.track});groups.sync();
   },virtual);
   const target=page.locator('[data-time-group="target"]');
   const settle=()=>page.waitForTimeout(100);
   const position=()=>target.locator('.timeline-when').evaluate(el=>el.getBoundingClientRect().top);
   const middle=async()=>{await page.evaluate(()=>{const main=document.querySelector('main'),group=document.querySelector('[data-time-group="target"]');main.scrollTop+=group.getBoundingClientRect().top-main.getBoundingClientRect().top+1800;});await settle();};
   const click=async selector=>{const before=await position(),box=await target.locator(selector).boundingBox();await page.mouse.click(box.x+box.width/2,box.y+box.height/2);await settle();const after=await position();assert(Math.abs(before-after)<3,`${width}px virtual=${virtual} ${selector}: rail moved ${before} -> ${after}`);assert(after>=0&&after<650,'active controls stay in view');};
   await middle();
   // Reduce a long group to its first two rows while its rail is pinned halfway down.
   for(const cat of ['IP 地址','网络路由','登录会话'])await click(`[data-timeline-category-toggle="${cat}"]`);
   await click('[data-timeline-category-toggle="计划任务"]');
   assert.equal(await target.locator('.timeline-branch:visible').count(),0);
   await click('[data-timeline-show-all]');
   assert.equal(await target.locator('.timeline-branch:visible').count(),20);
   await middle();await click('[data-timeline-invert]');await click('[data-timeline-invert]');
   await middle();await click('[data-timeline-toggle]');await click('[data-timeline-toggle]');
   await middle();const beforeKeyboard=await position();
   const keyboard=target.locator('[data-timeline-invert]');await keyboard.evaluate(el=>el.focus({preventScroll:true}));await page.keyboard.press('Space');await settle();
   assert(Math.abs(beforeKeyboard-await position())<3,'keyboard filter keeps the time card in place');await click('[data-timeline-show-all]');
   results.push({width,virtual,passed:true});
   await page.evaluate(()=>{groups.dispose();win.dispose();});
  }
  assert.deepEqual(errors,[]);fs.mkdirSync(path.resolve(__dirname,'../target'),{recursive:true});
  fs.writeFileSync(path.resolve(__dirname,'../target/timeline-filter-scroll-results.json'),JSON.stringify(results,null,2));
  console.log('PASS sticky filter position: category toggles, empty/all, invert and fold, three widths, virtualized and mounted rows');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
