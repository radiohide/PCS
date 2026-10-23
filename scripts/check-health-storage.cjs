const assert=require('node:assert/strict'),path=require('node:path');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const {bytes}=require('../apps/desktop/ui/health-storage.js');
assert.equal(bytes(0),'0 B');assert.equal(bytes(70780993536),'70.78 GB');assert.equal(bytes(1e12),'1 TB');
for(const value of [undefined,null,-1,NaN,Infinity,'100'])assert.equal(bytes(value),'暂未取得');
(async()=>{
  const browser=await chromium.launch({executablePath:process.env.PCSCOPE_CHROME_PATH,headless:true});
  try{
    const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.setContent('<main></main>');
    await page.addScriptTag({path:path.resolve(__dirname,'../apps/desktop/ui/health-storage.js')});
    await page.evaluate(()=>{
      const qa=window.qa={calls:[],allowed:true,fail:false,data:{physical_bytes:70780993536,mode:'unlimited',quota_bytes:5e9},now:100000};
      Date.now=()=>qa.now;
      qa.card=PCScopeHealthStorage.mount(document.querySelector('main'),{api:async op=>{qa.calls.push(op);if(qa.wait)await new Promise(resolve=>qa.release=resolve);if(qa.fail)throw Error('deadline');return qa.data;},canRefresh:()=>qa.allowed,localTime:()=> '12:00:00'});
      return qa.card.reload();
    });
    assert.match(await page.locator('main').innerText(),/70.78 GB/);
    assert.match(await page.locator('main').innerText(),/不限容量/);
    await page.locator('summary').click();
    await page.evaluate(async()=>{qa.wait=true;qa.pending=qa.card.reload();qa.card.reload();qa.card.tick();});
    assert.equal(await page.evaluate(()=>qa.calls.length),2,'overlapping requests coalesce');
    assert.match(await page.locator('main').innerText(),/70.78 GB/,'pending read retains result');
    await page.evaluate(async()=>{qa.wait=false;qa.release();await qa.pending;qa.fail=true;await qa.card.reload({automatic:true});});
    assert.equal(await page.locator('[data-storage-error]').isVisible(),false,'one transient failure stays quiet');
    await page.evaluate(()=>qa.card.reload({automatic:true}));
    assert.match(await page.locator('[data-storage-error]').innerText(),/显示上次结果/);
    assert.match(await page.locator('main').innerText(),/70.78 GB/);
    await page.evaluate(()=>{qa.fail=false;qa.data={physical_bytes:0,mode:'rolling',quota_bytes:5e9};});
    await page.getByRole('button',{name:'重试'}).click();
    assert.equal(await page.locator('[data-storage-bytes]').innerText(),'0 B');
    assert.equal(await page.locator('[data-storage-policy]').innerText(),'上限 5 GB');
    assert.equal(await page.locator('details').getAttribute('open'),'','refresh retains expanded detail');
    await page.evaluate(async()=>{qa.data={mode:'unlimited'};await qa.card.reload();});
    assert.equal(await page.locator('[data-storage-bytes]').innerText(),'0 B','missing value must not become zero or overwrite last result');
    const count=await page.evaluate(()=>qa.calls.length);
    await page.evaluate(()=>{qa.allowed=false;qa.now+=31000;qa.card.tick();});
    assert.equal(await page.evaluate(()=>qa.calls.length),count,'pause is honored');
    await page.evaluate(()=>{qa.allowed=true;qa.data={physical_bytes:1e9,mode:'unlimited'};qa.card.tick();});
    assert.equal(await page.locator('[data-storage-bytes]').innerText(),'1 GB');
    await page.evaluate(()=>{qa.card.tick();});
    assert.equal(await page.evaluate(()=>qa.calls.length),count+1,'periodic refresh is bounded');
    await page.evaluate(async()=>{qa.wait=true;qa.pending=qa.card.reload();qa.card.dispose();qa.data.physical_bytes=2e9;qa.release();await qa.pending;});
    assert.equal(await page.locator('[data-storage-bytes]').innerText(),'1 GB','disposed page ignores late response');
    assert((await page.evaluate(()=>qa.calls)).every(op=>op==='retention'),'read only');
    assert.deepEqual(errors,[]);
    console.log('PASS health storage: units, zero/missing values, retention modes, deduplication, quiet errors/recovery, pause/throttle, disposal, stable details, read-only API');
  }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
