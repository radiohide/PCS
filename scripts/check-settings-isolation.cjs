'use strict';
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT||'playwright');
(async()=>{
 const browser=await chromium.launch({executablePath:process.env.PCSCOPE_CHROME||undefined,headless:true});
 try{
  const page=await browser.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.setContent('<main></main>');
  await page.addScriptTag({path:path.resolve(__dirname,'../apps/desktop/ui/settings.js')});
  await page.evaluate(()=>{
   const qa=window.qa={calls:[],retentionAttempts:0};
   window.PCScopeMonitoring={mount:async panel=>{qa.calls.push('monitoring_settings');panel.textContent='当前后台：管理员权限';}};
   window.PCScopeSettings.mount(document.querySelector('main'),{api:async op=>{
    qa.calls.push(op);
    if(op==='retention'){
     qa.retentionAttempts++;
     if(qa.retentionAttempts===1)return await new Promise((resolve,reject)=>{qa.fail=()=>reject(Error('deadline has elapsed'));});
     return {mode:'unlimited',physical_bytes:1000,quota_bytes:5000000000,pruned_units:0};
    }
    if(op==='privacy_settings')return {show_sensitive:false};
    throw Error('unexpected mutation '+op);
   },esc:v=>String(v??''),localTime:v=>v,errorText:()=> '读取超时，请稍后重试；已有记录仍保留。',getTheme:()=> 'dark',setTheme:v=>qa.theme=v,onPrivacy:()=>{},notice:()=>{}});
  });
  assert.deepEqual(await page.evaluate(()=>qa.calls),[],'opening general settings must not wait for backend');
  await page.locator('#settings-tab-storage').click();
  await page.locator('#settings-tab-general').click();await page.locator('#setting-theme').selectOption('light');
  await page.locator('#settings-tab-privacy').click();await page.locator('#show-sensitive').waitFor();
  assert.equal(await page.locator('#show-sensitive').isChecked(),false);
  await page.locator('#settings-tab-monitoring').click();assert.match(await page.locator('#settings-monitoring').innerText(),/管理员权限/);
  await page.locator('#settings-tab-storage').click();assert.equal(await page.evaluate(()=>qa.retentionAttempts),1,'in-flight requests must not multiply');
  await page.evaluate(()=>qa.fail());await page.getByRole('button',{name:'重新读取'}).waitFor();
  assert.match(await page.locator('#settings-storage').innerText(),/其他设置仍可使用/);assert.doesNotMatch(await page.locator('#settings-storage').innerText(),/deadline/);
  assert.equal(await page.evaluate(()=>qa.theme),'light');
  await page.getByRole('button',{name:'重新读取'}).click();await page.locator('#retention-mode').waitFor();
  assert.equal(await page.locator('#retention-mode').inputValue(),'unlimited');assert.equal(await page.evaluate(()=>qa.retentionAttempts),2);
  assert.deepEqual(errors,[]);console.log('PASS settings: independent tabs, pending-read deduplication, local timeout, explicit retry, no inferred defaults or writes');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
