'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const ui=path.resolve(__dirname,'../apps/desktop/ui');
const samples=Object.fromEntries(JSON.parse(fs.readFileSync(path.resolve(__dirname,'../model/query-samples.json'),'utf8')).samples.map(s=>[s.name,s.response]));
const server=http.createServer((req,res)=>{const file=path.resolve(ui,'.'+new URL(req.url,'http://localhost').pathname);if(!file.startsWith(ui+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.json')?'application/json':file.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));});
(async()=>{await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;
try{
 browser=await chromium.launch({executablePath:process.env.PCSCOPE_CHROME_PATH,headless:true});const page=await browser.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(samples=>{
  window.setInterval=()=>1;window.qa={running:0,max:0,calls:[],failCoverage:false};
  window.__TAURI__={core:{invoke:async(command,args)=>{
   const r=args?.request||{};qa.calls.push(command+':'+r.operation);
   if(r.operation==='status')return {pid:1,collecting:false,counts:{}};
   if(r.operation==='monitoring_settings')return {elevated:true,settings:{configured:true,mode:'enhanced'}};
   if(r.operation!=='query')throw Error('unexpected operation');
   qa.running++;qa.max=Math.max(qa.max,qa.running);
   try{
    if(qa.running>2)throw Error('查询繁忙，请稍后重试；后台采集继续运行');
    await new Promise(resolve=>setTimeout(resolve,60));
    const q=r.request.query;
    if(q==='source_coverage'&&qa.failCoverage){qa.failCoverage=false;throw Error('读取超时');}
    return samples[q]||{status:'empty',data:[],counts:{instances:0,actions:0}};
   }finally{qa.running--;}
  }}};
 },samples);
 await page.goto('http://127.0.0.1:'+server.address().port+'/index.html');
 await page.locator('[data-page="overview"]').click();await page.locator('.overview-actions').waitFor();assert.equal(await page.evaluate(()=>qa.max),1);assert.doesNotMatch(await page.locator('#content').innerText(),/查询繁忙/);
 await page.evaluate(()=>{for(let n=0;n<6;n++)document.querySelector('[data-page="overview"]').click();});
 await page.locator('.overview-actions').waitFor();assert.equal(await page.evaluate(()=>qa.max),1);
 await page.evaluate(()=>qa.failCoverage=true);await page.locator('#load-latest').click();await page.locator('#page-retry').waitFor();await page.locator('#page-retry').click();await page.locator('.overview-actions').waitFor();
 assert(!await page.evaluate(()=>qa.calls.some(x=>x.startsWith('start_service:')||x.startsWith('switch_service:'))),'query failure must not restart backend');assert.deepEqual(errors,[]);
 console.log('PASS real overview router: service capacity respected, rapid repeated navigation succeeds, failed query retry does not restart backend');
}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}})().catch(e=>{console.error(e);process.exitCode=1;});
