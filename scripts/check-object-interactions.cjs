'use strict';
// Exercise the real router and its event handlers, with read-only IPC responses.
// Set PCSCOPE_PLAYWRIGHT_MODULE to a Playwright module path if it is not installed normally;
// set PCSCOPE_CHROME_PATH to use a local Chrome. Neither is a product runtime dependency.
// Optional saved live inputs: --live-list <IPC list response> --live-details <detail array>.
// --state-live <eight-type IPC capture> also exercises typed state summaries and details.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PCSCOPE_PLAYWRIGHT_MODULE||'playwright');
const root=path.resolve(__dirname,'..'),ui=path.join(root,'apps/desktop/ui'),output=path.join(root,'target');
const samples=Object.fromEntries(JSON.parse(fs.readFileSync(path.join(root,'model/query-samples.json'),'utf8')).samples.map(s=>[s.name,s.response]));
const base={type_id:'software.installation_registration',type_name:'软件登记',category:'software',lifecycle:'active',identity_status:'confirmed',key_facts:[],coverage:[]};
const dictionary={name:'示例组件登记',description:'用于示例组件的安装维护。',name_status:'dictionary_matched',basis:'registration_dictionary',dictionary_id:'interaction-test',dictionary_revision:1,dictionary_entry_id:'fixture',match_basis:'测试登记',technical_key:'HKLM|test',references:[]};
const rows=[{...base,instance_id:1,alias:'normal&"<one>',display_name:'示例工具',key_facts:[{field_id:'display_name',status:'known',value:'示例工具'}]},{...base,instance_id:2,alias:'unnamed-two',display_name:'',presentation:{name_status:'not_provided'}},{...base,instance_id:3,alias:'dictionary-three',display_name:'',presentation:dictionary},{...base,instance_id:4,alias:'dictionary-four',display_name:'',presentation:dictionary}];
const argument=name=>{const index=process.argv.indexOf(name);return index<0?null:process.argv[index+1];};
const liveList=argument('--live-list'),liveDetails=argument('--live-details');
assert.equal(Boolean(liveList),Boolean(liveDetails),'provide both saved live input files');
const live=liveList?{list:JSON.parse(fs.readFileSync(path.resolve(liveList),'utf8')).data,details:JSON.parse(fs.readFileSync(path.resolve(liveDetails),'utf8'))}:null;
const stateLive=argument('--state-live');
const stateSamples=stateLive?JSON.parse(fs.readFileSync(path.resolve(stateLive),'utf8')).filter(entry=>entry.label?.startsWith('detail_')):[];
const server=http.createServer((req,res)=>{const file=path.resolve(ui,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname));if(!file.startsWith(ui+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.json')?'application/json':file.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));});
(async()=>{
 fs.mkdirSync(output,{recursive:true});await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;const checks=[],errors=[];
 try{
  browser=await chromium.launch({...(process.env.PCSCOPE_CHROME_PATH?{executablePath:process.env.PCSCOPE_CHROME_PATH}:{}),headless:true});
  const page=await browser.newPage({viewport:{width:1440,height:1000},timezoneId:'Asia/Singapore'});page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(({samples,rows})=>{
   window.__qa={calls:[],revision:0,collecting:false,failNext:false,pending:false,intervals:[],listFailures:1,listError:'查询超时，已停止本次查询。请增加采集项或变化类型筛选后重试；历史数据保留。'};
   window.setInterval=(callback,delay)=>{window.__qa.intervals.push({callback,delay});return window.__qa.intervals.length;};
   const empty={status:'empty',data:[],page:{has_more:false},counts:{source_records:0}};
   window.__TAURI__={core:{invoke:async(command,args)=>{
    const r=args?.request||{},q=r.request||{};window.__qa.calls.push({operation:r.operation,...q});
    if(r.operation==='status')return {collecting:window.__qa.collecting,read_model_revision:String(window.__qa.modelRevision||0),counts:{records:4,objects:4,snapshots:1}};
    if(r.operation==='monitoring_settings')return {elevated:false,settings:{configured:true,mode:'low',repair:false},report:{audits:[],logs:[]}};
    if(r.operation!=='query')throw Error('Unexpected operation: '+r.operation);
    if(q.query==='type_catalog')return samples.type_catalog;
    if(q.query==='source_readings'&&window.__qa.sourcePages)return window.__qa.sourcePages[q.cursor?1:0];
    if(q.query==='instance_actions'&&window.__qa.actionPages)return window.__qa.actionPages[q.cursor?1:0];
    if(q.query==='action_timeline'&&window.__qa.timelineDataset){
     const from=q.cursor?Number(q.cursor):0;
     if(window.__qa.failGroupAt===from){window.__qa.failGroupAt=null;throw Error('temporary grouped query failure');}
     const data=window.__qa.timelineDataset.slice(from,from+q.limit),more=from+q.limit<window.__qa.timelineDataset.length;
     return {status:'loaded',data,page:{has_more:more,next_cursor:more?String(from+q.limit):null,watermark:'group-fixture'}};
    }
    if(q.query==='action_timeline'&&window.__qa.timelinePages){
     if(q.cursor==='point-older'&&window.__qa.holdPointOlder)await new Promise(resolve=>window.__qa.releasePointOlder=resolve);
     if(q.cursor==='older-page'&&window.__qa.holdOlderPage)await new Promise(resolve=>window.__qa.releaseOlderPage=resolve);
     return window.__qa.timelinePages[q.cursor?1:0];
    }
    if(q.query==='action_timeline'&&window.__qa.routineFixture)return {...empty,status:'loaded',data:q.include_routine?[...window.__qa.routineFixture.main,...window.__qa.routineFixture.routine]:window.__qa.routineFixture.main};
    if(q.query==='action_timeline'&&window.__qa.timeline)return {...empty,status:'loaded',data:window.__qa.timeline};
    if(window.__qa.liveFixture&&q.query==='instance_list')return window.__qa.liveFixture.list;
    if(window.__qa.liveFixture&&q.query==='instance_detail'){
     const entry=window.__qa.liveFixture.details.find(entry=>entry.response.data.summary.alias===q.instance);
     if(!entry)throw Error('Unexpected live object identity');return entry.response;
    }
    if(q.query==='instance_list'){
     if(window.__qa.listFailures>0){window.__qa.listFailures--;throw Error(window.__qa.listError);}
     const data=q.search?rows.filter(row=>row.display_name.includes(q.search)):q.cursor?[rows[3]]:window.__qa.revision?[rows[2],rows[1],rows[0]]:rows.slice(0,3);
     return {status:'loaded',data,page:{has_more:!q.cursor&&!q.search,next_cursor:!q.cursor?'page-two':null,watermark:'fixed'},counts:{instances:4}};
    }
    if(q.query==='instance_detail'){
     if(window.__qa.detailFixture)return window.__qa.detailFixture;
     if(window.__qa.pending)await new Promise(resolve=>{window.__qa.resolve=resolve;});
     if(window.__qa.failNext){window.__qa.failNext=false;throw Error('interaction fixture failed <script>bad()</script>');}
     const summary=rows.find(row=>row.alias===q.instance);if(!summary)throw Error('Wrong alias '+q.instance);
     return {status:'loaded',data:{summary,fields:[{field_id:'display_version',label:'版本',status:'known',value:'detail-'+summary.instance_id+'-revision-'+window.__qa.revision}],actions_preview:[],bindings:[]}};
    }
    return empty;
   }}};
  },{samples,rows});
  await page.goto('http://127.0.0.1:'+server.address().port+'/index.html');await page.locator('[data-page="objects"]').click();await page.locator('#model-more button').waitFor();
  const timeoutText=await page.locator('#model-results').innerText();
  assert.match(timeoutText,/对象列表读取超时/);assert.match(timeoutText,/类别、类型、名称/);assert.doesNotMatch(timeoutText,/采集项|变化类型/);
  assert.match(await page.locator('#model-results details').textContent(),/采集项或变化类型/,'original backend error remains in collapsed diagnostic details');
  const categories=await page.locator('#model-category option').evaluateAll(options=>options.map(option=>({value:option.value,label:option.textContent})));
  for(const [value,label] of [['account','用户与账户'],['file','文件与文件夹'],['runtime','运行程序与会话']])assert(categories.some(option=>option.value===value&&option.label===label));
  assert(categories.every(option=>/[\u4e00-\u9fff]/.test(option.label)),'all catalog categories need Chinese labels while retaining query values');
  const typeLabels=await page.locator('#model-type').textContent();assert.doesNotMatch(typeLabels,/WMI 订阅|IFEO|命名空间扩展|Device Guard|备用数据流/);assert.match(typeLabels,/系统事件触发条件/);
  checks.push('all category labels and obvious technical type names are readable without changing query values');
  await page.locator('#model-more button').click();await page.locator('#model-results button[data-object]').first().waitFor();
  const defaultLists=await page.evaluate(()=>window.__qa.calls.filter(call=>call.query==='instance_list'&&!call.cursor));
  assert.equal(defaultLists.length,2);assert(defaultLists.every(call=>!call.category&&!call.type_id&&!call.search),'retry must work without forcing a filter');checks.push('default-list timeout gives page-specific guidance, retains original error, and retries unfiltered');
  assert(defaultLists.every(call=>call.limit===100),'object lists use the measured 100-row batch');
  const clickAlias=async alias=>{const buttons=page.locator('#model-results button[data-object-alias]');for(let i=0;i<await buttons.count();i++)if(await buttons.nth(i).getAttribute('data-object-alias')===alias){await buttons.nth(i).click();return;}throw Error('Missing alias button '+alias);};
  await page.locator('#model-results button[data-object]').first().click();
  await page.waitForTimeout(80);
  const baseline=await page.evaluate(()=>({detail_calls:window.__qa.calls.filter(c=>c.query==='instance_detail').length,visible_details:document.querySelectorAll('.object-detail-row').length}));
  if(process.argv.includes('--reproduce')){fs.writeFileSync(path.join(output,'object-interactions-before.json'),JSON.stringify(baseline,null,2));assert.equal(baseline.detail_calls,0);assert.equal(baseline.visible_details,0);console.log('REPRODUCED: real object View button issues no query and opens no details.');return;}
  assert.equal(baseline.detail_calls,1,'View must issue one detail query');assert.equal(baseline.visible_details,1);checks.push('normal object exact escaped alias click');
  assert.equal(await page.locator('[data-object-alias]').first().getAttribute('data-object-alias'),rows[0].alias);
  assert(!(await page.locator('#model-results').innerText()).includes(rows[0].alias),'raw alias must stay out of default text');
  await clickAlias(rows[0].alias);assert.equal(await page.locator('.object-detail-row').count(),0);checks.push('collapse removes detail');
  await page.locator('.unnamed-registrations>summary').click();await clickAlias(rows[1].alias);await page.locator('.object-detail-row .model-detail').waitFor();checks.push('unnamed registration detail opens');
  await clickAlias(rows[2].alias);await page.locator('#model-more').scrollIntoViewIfNeeded();await page.locator('[data-object-alias="dictionary-four"]').waitFor();await clickAlias(rows[3].alias);await page.waitForTimeout(80);
  assert.equal(await page.locator('.object-detail-row').count(),3);
  assert.equal(await page.evaluate(()=>new Set([...document.querySelectorAll('.object-detail-row')].map(e=>e.id)).size),3);
  assert.deepEqual(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='instance_detail').map(c=>c.instance)),rows.slice(0,4).map(r=>r.alias));checks.push('same dictionary names and appended page preserve distinct identity');
  const queriesBefore=await page.evaluate(()=>window.__qa.calls.length);
  await page.evaluate(async()=>{window.__qa.collecting=true;for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();window.__qa.collecting=false;for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();});
  assert.equal(await page.locator('.object-detail-row').count(),3);
  assert.equal(await page.evaluate(n=>window.__qa.calls.slice(n).filter(c=>c.query==='instance_detail').length,queriesBefore),0);checks.push('automatic collection completion preserves open detail without query fanout');
  const detailBefore=await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='instance_detail').length);
  await page.evaluate(()=>window.__qa.revision++);await page.locator('#load-latest').click();
  assert.equal(await page.locator('.object-detail-row').count(),0,'manual load replaces the list and closes old details');
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='instance_detail').length),detailBefore,'list refresh must not refetch every open detail');
  await clickAlias(rows[2].alias);await page.waitForTimeout(80);
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='instance_detail').at(-1).instance),rows[2].alias,'reordered rows still request their original identity');
  await clickAlias(rows[2].alias);checks.push('manual refresh closes old detail without fanout and reordered rows keep correct identity');
  await page.evaluate(()=>window.__qa.failNext=true);await clickAlias(rows[0].alias);await page.waitForTimeout(80);
  assert((await page.locator('.object-detail-row').innerText()).includes('操作未完成'));assert(!(await page.locator('.object-detail-row').innerText()).includes('<script>'));
  await page.locator('.object-detail-row').getByRole('button',{name:'重试'}).click();await page.locator('.object-detail-row .model-detail').waitFor();checks.push('escaped errors with working retry');
  await clickAlias(rows[0].alias);
  const beforeRapid=await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='instance_detail').length);
  await page.evaluate(()=>window.__qa.pending=true);await clickAlias(rows[0].alias);await clickAlias(rows[0].alias);await clickAlias(rows[0].alias);
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='instance_detail').length),beforeRapid+1,'rapid reopening reuses the in-flight query');
  await clickAlias(rows[0].alias);await page.evaluate(()=>{window.__qa.pending=false;window.__qa.resolve();});await page.waitForTimeout(80);assert.equal(await page.locator('.object-detail-row').count(),0);checks.push('rapid reopen shares one pending request; collapse cannot resurrect details');
  await page.evaluate(()=>window.__qa.pending=true);await clickAlias(rows[0].alias);await page.locator('#model-search').fill('没有匹配');await page.waitForTimeout(350);await page.evaluate(()=>{window.__qa.pending=false;window.__qa.resolve();});await page.waitForTimeout(80);assert.equal(await page.locator('.object-detail-row').count(),0);assert((await page.locator('#model-results').innerText()).includes('没有匹配'));checks.push('changing filters cannot attach a delayed old detail to a different row');
  await page.evaluate(()=>{window.__qa.listFailures=1;window.__qa.listError='querytimeout';});await page.locator('#model-clear').click();await page.locator('#model-more button').waitFor();
  assert.match(await page.locator('#model-results').innerText(),/对象列表读取超时/);
  await page.locator('#model-category').selectOption('software');await page.locator('#model-results button[data-object]').first().waitFor();
  await page.locator('#model-type').selectOption('software.installation_registration');await page.waitForTimeout(80);
  await page.locator('#model-search').fill('示例工具');await page.waitForTimeout(350);
  const filtered=await page.evaluate(()=>window.__qa.calls.filter(call=>call.query==='instance_list').at(-1));
  assert.equal(filtered.category,'software');assert.equal(filtered.type_id,'software.installation_registration');assert.equal(filtered.search,'示例工具');
  assert.equal(await page.locator('#model-results button[data-object]').count(),1);checks.push('querytimeout is readable and category/type/name controls remain usable after failure');
  if(live){
   await page.evaluate(live=>window.__qa.liveFixture=live,live);await page.locator('#model-clear').click();await page.waitForTimeout(100);
   for(const entry of live.details){
    const summary=entry.response.data.summary,group=page.locator('.unnamed-registrations');
    if(entry.kind==='not_provided'&&await group.count()&&await group.getAttribute('open')===null)await group.locator(':scope>summary').click();
    await clickAlias(summary.alias);await page.waitForTimeout(80);
    const detail=page.locator('.object-detail-row').filter({hasText:require('../apps/desktop/ui/object-names.js').name(summary,entry.response.data.fields)});
    assert(await detail.count()>0,'live '+entry.kind+' must render the expected title');
    assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='instance_detail').at(-1).instance),summary.alias);
    checks.push('saved live IPC '+entry.kind+' object '+summary.instance_id+' opens through actual router');
   }
   assert.equal(await page.locator('.object-detail-row').count(),3);await page.screenshot({path:path.join(output,'object-interactions-live.png'),fullPage:true});
  }
  if(stateLive){
   assert.equal(stateSamples.length,8,'all eight captured state types are required');
   const presentation=require('../apps/desktop/ui/type-presentation.js');
   const names=require('../apps/desktop/ui/object-names.js');
   const expected={
    'policy.power_plan':['平衡','正在使用'],'network.firewall_profile':['域网络','已开启','未配置'],
    'startup.service':['运行中','手动启动'],'network.connection_profile':['专用网络','可访问互联网','尚无流量'],
    'security.protection':['Windows 防护状态','正在保护'],'policy.os_configuration':['19044','10.0.19044'],
    'security.secure_boot_configuration':['安全启动','已开启'],'software.optional_feature':['已关闭']
   };
   const stateRows=stateSamples.map(entry=>{const d=entry.response.data,allowed=new Set(presentation.profile(d.summary).summary_fields);return {...d.summary,key_facts:d.fields.filter(f=>allowed.has(f.field_id)||f.kind==='identity'||['name','display_name','caption'].includes(f.field_id))};});
   const stateFixture={list:{status:'loaded',data:stateRows,page:{has_more:false},counts:{instances:8}},details:stateSamples};
   await page.evaluate(fixture=>window.__qa.liveFixture=fixture,stateFixture);await page.locator('#model-clear').click();await page.waitForTimeout(100);
   const listText=await page.locator('#model-results').innerText();
   assert.doesNotMatch(listText,/最近仍在|暂无更多信息/);
   for(const summary of stateRows){assert(listText.includes(names.name(summary)));assert(listText.includes(presentation.state(summary)),summary.type_id+' typed list state');}
   await page.screenshot({path:path.join(output,'state-presentation-list.png'),fullPage:true});
   for(const entry of stateSamples){
    const summary=entry.response.data.summary;
    await clickAlias(summary.alias);await page.locator('.object-detail-row .model-detail').waitFor();
    const text=await page.locator('.object-detail-row').innerText();
    for(const phrase of expected[summary.type_id])assert(text.includes(phrase),summary.type_id+' missing '+phrase);
    assert.doesNotMatch(text,/最近仍在|暂无更多信息/);
    assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='instance_detail').at(-1).instance),summary.alias);
    if(summary.type_id==='policy.power_plan')await page.screenshot({path:path.join(output,'state-presentation-power.png'),fullPage:true});
    if(summary.type_id==='startup.service')await page.screenshot({path:path.join(output,'state-presentation-service.png'),fullPage:true});
    await clickAlias(summary.alias);assert.equal(await page.locator('.object-detail-row').count(),0);
    checks.push('saved live state '+summary.type_id+' renders matching typed list and clicked detail');
   }
  }
  const timelineTypes=['runtime.process','file.watched_file','startup.service','startup.scheduled_task','runtime.module_instance','desktop.print_job','network.endpoint','security.certificate_registration'];
  const timelineRows=timelineTypes.map((type_id,index)=>({instance_id:index+1,action_id:'observed.no_longer_observed',result:'observed',precision:'interval',time_derived:true,interval_from:'2026-10-15T02:24:07Z',interval_until:'2026-10-15T02:29:32Z',observed_at:'2026-10-15T02:29:32Z',context:{type_id,subject_label:index<2?'conhost.exe':'示例项目',attribution_status:'stored'}}));
  await page.evaluate(data=>{window.__qa.timeline=data;},timelineRows);
  await page.locator('[data-page="timeline"]').click();await page.locator('.timeline-entry').first().waitFor();
  assert.equal(await page.locator('.timeline-entry').count(),timelineRows.length);
  assert.equal(await page.locator('.timeline-type').nth(0).textContent(),'类型：运行进程');
  assert.equal(await page.locator('.timeline-type').nth(1).textContent(),'类型：文件');
  assert.match(await page.locator('.timeline-entry').first().innerText(),/已不在运行列表/);
  assert.equal(await page.locator('.timeline-group').count(),1,'one interval label covers all eight records');
  assert.equal(await page.locator('.timeline-entry details[open]').count(),0,'raw type identifiers stay collapsed');
  assert.doesNotMatch((await page.locator('.timeline-card-body').allTextContents()).join(' '),/runtime\.process|file\.watched_file|尚不能据此确认已被删除/);
  await page.screenshot({path:path.join(output,'timeline-types-preview.png'),fullPage:false});
  checks.push('real timeline distinguishes process, file, service, task, module, queue, connection and certificate with visible types and collapsed raw details');
  const arrivalRows=['Established','Listen',null].map((connectionState,index)=>({...timelineRows[0],action_id:'observed.first_seen',instance_id:index+1,source_record:{snapshot_presence:{source:index===2?'pcscope.network.udp':'pcscope.network.tcp',protocol:index===2?null:'TCP',state:connectionState}},context:{type_id:'network.endpoint',subject_label:'本机端口 '+(61981+index),attribution_status:'stored',key_facts:[{field_id:'state',status:'known',value:'Closed'}]}}));
  await page.evaluate(data=>window.__qa.timeline=data,arrivalRows);await page.locator('[data-page="timeline"]').click();await page.locator('.timeline-entry').first().waitFor();
  assert.deepEqual(await page.locator('.timeline-event-label').allTextContents(),['首次记录到此连接','首次记录到此监听端口','首次记录到此 UDP 端口']);
  assert.doesNotMatch((await page.locator('.timeline-card-body').allTextContents()).join(' '),/已不在连接与监听列表|Closed/);
  checks.push('new TCP connections, listening ports and UDP ports have distinct arrival cards based on the original snapshot rather than later state');
  const moduleFirst={...timelineRows[0],instance_id:73579,action_id:'observed.first_seen',interval_from:'2026-10-16T08:34:29Z',interval_until:'2026-10-19T05:16:21Z',observed_at:'2026-10-19T05:16:21Z',source_record:{snapshot_presence:{source:'pcscope.execution.modules'}},context:{type_id:'runtime.module_instance',subject_label:'pdh.dll',attribution_status:'stored'}};
  const moduleGone={...moduleFirst,action_id:'observed.no_longer_observed',interval_from:'2026-10-19T05:25:19Z',interval_until:'2026-10-19T05:26:20Z',observed_at:'2026-10-19T05:26:20Z'};
  const moduleDetail={status:'loaded',data:{summary:{...base,type_id:'runtime.module_instance',type_name:'模块加载实例',category:'runtime',instance_id:73579,alias:'module-pdh',display_name:'pdh.dll',lifecycle:'not_observed',record_basis:'observation',seen_at:'2026-10-19T05:25:19Z'},fields:[{field_id:'process_name',status:'known',value:'sihost'},{field_id:'module_origin',status:'known',value:'unknown'},{field_id:'signature_status',status:'known',value:'pending'}],actions_preview:[moduleGone,moduleFirst],bindings:[]}};
  await page.evaluate(({first,detail})=>{window.__qa.timeline=[first];window.__qa.detailFixture=detail;},{first:moduleFirst,detail:moduleDetail});
  await page.locator('[data-page="timeline"]').click();await page.locator('.timeline-event-label').waitFor();
  assert.equal(await page.locator('.timeline-event-label').textContent(),'首次记录加载');
  assert.match(await page.locator('.timeline-object').innerText(),/查看当前状态/);
  await page.locator('.timeline-object').click();await page.locator('.model-detail .object-current-state').waitFor();
  assert.equal(await page.locator('.object-current-state').innerText(),'未加载');
  const moduleText=await page.locator('.model-detail').innerText();assert.match(moduleText,/最后加载时的信息.*历史变化/s);assert.match(moduleText,/尚未取得签名结果/);assert.doesNotMatch(moduleText,/\bunknown\b|\bpending\b|新发现/);
  await page.screenshot({path:path.join(output,'timeline-history-current-preview.png'),fullPage:false});
  checks.push('a historical module arrival opens the latest unloaded state, with historical fields and readable signature placeholders');
  await page.locator('.timeline-object').click();await page.evaluate(()=>{window.__qa.detailFixture.data.summary.lifecycle='active';});await page.locator('.timeline-object').click();await page.locator('.object-current-state').waitFor();
  assert.equal(await page.locator('.object-current-state').innerText(),'已加载');assert.equal(await page.locator('.timeline-event-label').textContent(),'首次记录加载');
  checks.push('reopening queries the current object state without rewriting the historical first observation');
  await page.locator('.timeline-object').click();await page.evaluate(()=>{window.__qa.detailFixture.data.summary.record_basis='event_only';});await page.locator('.timeline-object').click();await page.locator('.object-current-state').waitFor();
  assert.equal(await page.locator('.object-current-state').innerText(),'未取得当前状态');
  checks.push('a historical log alone does not claim a currently loaded module');
  await page.evaluate(()=>{delete window.__qa.detailFixture;});
  const readableUpdates={main:[
    {...timelineRows[0],field_id:'module_base',action_id:'observed.state_changed',before:4096,after:8192,context:{type_id:'runtime.module_instance',subject_label:'示例组件',attribution_status:'stored'}},
    {...timelineRows[0],field_id:'signature_version',action_id:'observed.state_changed',before:'1.0',after:'1.1',source_record:{protection_update:{engine_changed:true,signature_changed:true}},context:{type_id:'security.protection',subject_label:'系统防护',attribution_status:'stored'}},
    {...timelineRows[0],field_id:'charge',action_id:'observed.state_changed',before:21,after:20,context:{type_id:'hardware.battery',subject_label:'电池',attribution_status:'stored'}},
    {...timelineRows[0],action_id:'startup.task_ended',result:'failed',context:{type_id:'startup.scheduled_task',subject_label:'示例任务',attribution_status:'stored'}},
  ],routine:[
    {...timelineRows[0],field_id:'last_use',action_id:'observed.state_changed',before:'2026-10-15T08:08:26Z',after:'2026-10-15T08:14:27Z',context:{type_id:'account.user_profile',subject_label:'本地服务账户资料',attribution_status:'stored'}},
    {...timelineRows[0],field_id:'last_run',action_id:'observed.state_changed',before:'2026-10-15T08:08:26Z',after:'2026-10-15T08:14:27Z',context:{type_id:'startup.scheduled_task',subject_label:'示例任务',attribution_status:'stored'}},
  ]};
  await page.evaluate(data=>window.__qa.routineFixture=data,readableUpdates);await page.locator('[data-page="timeline"]').click();await page.locator('.timeline-entry').first().waitFor();
  assert.equal(await page.locator('#timeline-routine').inputValue(),'false');assert.equal(await page.locator('.timeline-entry').count(),4);
  const updateText=(await page.locator('.timeline-card-body').allTextContents()).join(' ');for(const expected of ['模块加载位置已变化','防护程序和病毒库已更新','电量偏低','运行失败'])assert(updateText.includes(expected));
  assert.doesNotMatch(updateText,/最近使用时间已更新|最近执行|1\.0|1\.1/);
  await page.locator('#timeline-routine').selectOption('true');await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===6);
  assert.match((await page.locator('.timeline-card-body').allTextContents()).join(' '),/最近使用时间已更新/);assert.match((await page.locator('.timeline-card-body').allTextContents()).join(' '),/最近执行/);
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(q=>q.query==='action_timeline').at(-1).include_routine),true);
  await page.locator('#timeline-clear').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===4);
  assert.equal(await page.locator('#timeline-routine').inputValue(),'false');
  await page.evaluate(()=>delete window.__qa.routineFixture);
  checks.push('main timeline retains module address changes, battery alerts, failures and security summaries; routine history is explicitly selectable and filters reset');
  const pointTimes=['2026-10-15T08:09:04.0982704Z','2026-10-15T08:09:04.0982607Z'];
  const pointRows=pointTimes.map((occurred_at,index)=>({instance_id:index+1,action_id:index?'identity.logon':'identity.special_privileges',result:'observed',precision:'exact',time_derived:false,occurred_at,observed_at:'2026-10-15T08:09:24.616Z',context:{type_id:'runtime.logon_session',subject_label:'登录会话示例 '+index,attribution_status:'stored'}}));
  await page.evaluate(data=>window.__qa.timeline=data,pointRows);await page.locator('[data-page="timeline"]').click();await page.locator('.timeline-entry').first().waitFor();
  assert.equal(await page.locator('.timeline-group').count(),1);assert.equal(await page.locator('.timeline-group-count').textContent(),'2 条记录');
  assert.equal(await page.locator('.timeline-precision').textContent(),'时间段');assert.equal(await page.locator('.timeline-time-end').count(),1);
  assert.equal(await page.locator('.timeline-category-count').textContent(),'2');
  assert.deepEqual(await page.locator('.timeline-branch').evaluateAll(rows=>rows.map(row=>JSON.parse(row.dataset.timelineMoment).originalStart)),pointTimes);
  checks.push('production logon and privilege timestamps differing by microseconds share one time card and retain both original times');
  await page.evaluate(async data=>{
    window.__qa.originalNow=Date.now;window.__qa.now=Date.now()+60000;Date.now=()=>window.__qa.now;
    window.__qa.collecting=true;window.__qa.modelRevision=1;window.__qa.timeline=data.slice(0,1);
    for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();
  },pointRows);await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===1);
  await page.evaluate(async data=>{window.__qa.timeline=data;window.__qa.modelRevision=2;window.__qa.now+=15000;for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();},pointRows);
  await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===2);
  checks.push('new timeline actions become visible during an ongoing collection');
  const quietRefreshCalls=await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='action_timeline').length);
  await page.evaluate(async()=>{window.__qa.now+=15000;for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();});
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='action_timeline').length),quietRefreshCalls);
  await page.evaluate(async data=>{window.__qa.timeline=data.slice(0,1);window.__qa.modelRevision=3;window.__qa.now+=3000;for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();},pointRows);
  // Last actual reload was over 15 seconds ago, so establish another recent refresh first.
  await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===1);
  const recentRefreshCalls=await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='action_timeline').length);
  await page.evaluate(async data=>{window.__qa.timeline=data;window.__qa.modelRevision=4;window.__qa.now+=3000;for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();},pointRows);
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='action_timeline').length),recentRefreshCalls);
  await page.evaluate(async()=>{window.__qa.now+=12000;for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();});
  await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===2);
  checks.push('unchanged model stays quiet and throttled timeline changes are eventually refreshed');
  await page.evaluate(async data=>{window.__qa.collecting=false;window.__qa.modelRevision=5;window.__qa.timeline=data.slice(0,1);window.__qa.now+=15000;for(const timer of window.__qa.intervals.filter(t=>t.delay===3000))await timer.callback();Date.now=window.__qa.originalNow;},pointRows);
  await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===1);
  checks.push('completed collection and later projection remain refreshable without another collection');
  const grouped79=Array.from({length:79},(_,index)=>({...timelineRows[0],instance_id:index+1,context:{type_id:index<50?'runtime.process':index<68?'startup.service':index<76?'network.adapter':'file.watched_file',subject_label:'分组示例 '+(index+1),attribution_status:'stored'}}));
  await page.evaluate(data=>{window.__qa.timeline=data;},grouped79);
  await page.locator('[data-page="timeline"]').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===79);
  const group79=page.locator('.timeline-group').first(),processChip=group79.locator('[data-timeline-category-toggle="运行进程"]');
  assert.equal(await group79.locator('.timeline-group-count').textContent(),'79 条记录');
  for(const [label,count] of [['运行进程',50],['系统服务',18],['网卡',8],['文件',3]])assert.equal(await group79.locator('[data-timeline-category-toggle="'+label+'"] .timeline-category-count').textContent(),String(count));
  await group79.locator('.timeline-entry details summary').first().click();
  const callsBeforeToggling=await page.evaluate(()=>window.__qa.calls.length);
  await processChip.click();assert.equal(await processChip.getAttribute('aria-pressed'),'false');
  assert.equal(await group79.locator('.timeline-branch:visible').count(),29);
  assert.equal(await group79.locator('[data-timeline-visible]').textContent(),'显示 29 / 79 条');
  await group79.locator('[data-timeline-invert]').click();
  assert.equal(await group79.locator('.timeline-branch:visible').count(),50,'invert swaps selected and hidden categories');
  assert.equal(await processChip.getAttribute('aria-pressed'),'true');
  await group79.locator('[data-timeline-invert]').click();
  assert.equal(await group79.locator('.timeline-branch:visible').count(),29,'inverting twice restores the exact selection');
  await group79.locator('.timeline-category-actions [data-timeline-show-all]').click();
  assert.equal(await group79.locator('.timeline-branch:visible').count(),79);
  await group79.locator('[data-timeline-invert]').click();
  assert(await group79.locator('.timeline-group-empty').isVisible(),'invert all selected hides all');
  await group79.locator('[data-timeline-invert]').focus();await page.keyboard.press('Enter');
  assert.equal(await group79.locator('.timeline-branch:visible').count(),79,'keyboard invert none selected shows all');
  await processChip.click();
  await page.screenshot({path:path.join(output,'timeline-group-controls-dark.png'),fullPage:false});
  await page.locator('#theme').click();await page.screenshot({path:path.join(output,'timeline-group-controls-light.png'),fullPage:false});await page.locator('#theme').click();
  for(const label of ['系统服务','网卡','文件'])await group79.locator('[data-timeline-category-toggle="'+label+'"]').click();
  assert.equal(await group79.locator('.timeline-branch:visible').count(),0);assert(await group79.locator('.timeline-group-empty').isVisible());
  await group79.locator('.timeline-group-empty [data-timeline-show-all]').click();
  assert.equal(await group79.locator('.timeline-branch:visible').count(),79);assert.equal(await group79.locator('.timeline-entry details[open]').count(),1,'local hiding preserves open details');
  await group79.locator('[data-timeline-toggle]').click();assert.equal(await group79.locator('[data-timeline-toggle]').getAttribute('aria-expanded'),'false');
  assert.equal(await group79.locator('.timeline-branch:visible').count(),0);assert.equal(await group79.locator('.timeline-category-toggle:visible').count(),4);
  await page.screenshot({path:path.join(output,'timeline-categories-collapsed-dark.png'),fullPage:false});
  await page.locator('#theme').click();await page.screenshot({path:path.join(output,'timeline-categories-collapsed-light.png'),fullPage:false});await page.locator('#theme').click();
  await group79.locator('[data-timeline-invert]').click();
  assert.equal(await group79.locator('.timeline-category-toggle[aria-pressed=false]').count(),4);
  await group79.locator('.timeline-category-actions [data-timeline-show-all]').click();
  assert.equal(await group79.locator('.timeline-category-toggle[aria-pressed=true]').count(),4);
  assert.equal(await group79.locator('[data-timeline-toggle]').getAttribute('aria-expanded'),'false','bulk category changes keep a collapsed group collapsed');
  const colorBefore=await processChip.evaluate(button=>getComputedStyle(button).color);
  await processChip.click();assert.equal(await group79.locator('.timeline-branch:visible').count(),0,'a collapsed category switch must not unfold the group');
  assert.equal(await group79.locator('[data-timeline-visible]').textContent(),'显示 29 / 79 条');
  assert.notEqual(await processChip.evaluate(button=>getComputedStyle(button).color),colorBefore,'a hidden category becomes neutral');
  await processChip.click();
  await group79.locator('[data-timeline-toggle]').focus();await page.keyboard.press('Enter');
  assert.equal(await group79.locator('.timeline-branch:visible').count(),79);assert.equal(await group79.locator('.timeline-entry details[open]').count(),1);
  await group79.locator('[data-timeline-time-toggle]').click();assert.equal(await group79.locator('.timeline-branch:visible').count(),0);
  await page.keyboard.press('Enter');assert.equal(await group79.locator('.timeline-branch:visible').count(),79,'the time label also expands by keyboard');
  assert.equal(await page.evaluate(()=>window.__qa.calls.length),callsBeforeToggling,'local category and collapse controls do not query or collect');
  checks.push('79 records have accurate per-type counts, independent toggle lights, an all-hidden recovery and keyboard group expansion without IPC');
  for(const width of [960,700]){
    await page.setViewportSize({width,height:760});
    assert(await group79.evaluate(group=>group.scrollWidth<=group.clientWidth+1),'time controls fit width '+width);
    await group79.locator('[data-timeline-toggle]').click();assert.equal(await group79.locator('.timeline-branch:visible').count(),0);
    assert.equal(await group79.locator('.timeline-category-toggle:visible').count(),4);
    assert(await group79.evaluate(group=>group.scrollWidth<=group.clientWidth+1),'collapsed category controls fit width '+width);
    await group79.locator('[data-timeline-toggle]').click();assert.equal(await group79.locator('.timeline-branch:visible').count(),79);
  }
  await page.setViewportSize({width:1440,height:1000});
  const olderFoldRows=Array.from({length:10},(_,index)=>({...timelineRows[0],instance_id:100+index,interval_from:'2026-10-15T02:00:00Z',interval_until:'2026-10-15T02:05:00Z',context:{type_id:'startup.service',subject_label:'下一组示例 '+index,attribution_status:'stored'}}));
  await page.evaluate(data=>window.__qa.timeline=data,[...grouped79,...olderFoldRows]);await page.locator('[data-page="timeline"]').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===89);
  const fixedGroup=page.locator('.timeline-group').first(),fixedToggle=fixedGroup.locator('[data-timeline-toggle]');
  const fixedCalls=await page.evaluate(()=>window.__qa.calls.length);
  for(const width of [1440,960,700]){
    await page.setViewportSize({width,height:760});
    await page.locator('main').evaluate(el=>{el.scrollTop+=el.querySelector('.timeline-group').getBoundingClientRect().top-el.getBoundingClientRect().top+500;});
    const before=await fixedToggle.boundingBox(),point={x:before.x+before.width/2,y:before.y+before.height/2};
    await page.mouse.click(point.x,point.y);assert.equal(await fixedToggle.getAttribute('aria-expanded'),'false');
    const after=await fixedToggle.boundingBox();
    for(const key of ['x','y','width','height'])assert(Math.abs(before[key]-after[key])<2,'fold control retains '+key+' at width '+width);
    await page.mouse.click(point.x,point.y);assert.equal(await fixedToggle.getAttribute('aria-expanded'),'true','the same pointer position expands the sticky group again');
    assert(await fixedGroup.evaluate(group=>group.scrollWidth<=group.clientWidth+1));
  }
  assert.equal(await page.evaluate(()=>window.__qa.calls.length),fixedCalls,'anchoring a single fold control never starts a query');
  checks.push('expanded and collapsed controls retain position and size at desktop, narrow and mobile widths; a sticky group can be toggled twice at the same pointer location without queries');
  await page.setViewportSize({width:1440,height:1000});
  const allTypes=require('../model/catalog.json').types.map((type,index)=>({...timelineRows[0],instance_id:index+1,context:{type_id:type.id,subject_label:'类型示例 '+index,attribution_status:'stored'}}));
  await page.evaluate(data=>{window.__qa.timeline=data;},allTypes);await page.locator('[data-page="timeline"]').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===101);
  assert.equal(await page.locator('.timeline-category-count').evaluateAll(nodes=>nodes.reduce((total,node)=>total+Number(node.textContent),0)),101,'every known type contributes to the counters');
  const categoryLayout=await page.locator('.timeline-category-controls').evaluate(el=>({height:el.clientHeight,scroll:el.scrollHeight,width:el.clientWidth,contentWidth:el.scrollWidth}));
  assert(categoryLayout.scroll>categoryLayout.height);assert(categoryLayout.contentWidth<=categoryLayout.width+1,'many category names remain within the rail');
  await page.locator('.timeline-category-toggle').last().click();assert.equal(await page.locator('.timeline-category-toggle[aria-pressed=false]').count(),1,'the last category remains reachable');
  checks.push('narrow layouts fold correctly and all 101 object types remain reachable in bounded category controls');

  const colorsByTheme={};
  for(const theme of ['dark','light']){
    await page.locator('#app').evaluate((app,value)=>app.dataset.theme=value,theme);
    const categories=await page.locator('.timeline-category-toggle').evaluateAll(buttons=>buttons.filter(button=>button.getAttribute('aria-pressed')==='true').map(button=>({label:button.dataset.timelineCategoryToggle,color:getComputedStyle(button).color,tone:button.dataset.timelineTone})));
    colorsByTheme[theme]=Object.fromEntries(categories.map(item=>[item.label,item.color]));
    const sample=['运行进程','用户资料','时间同步设置','网络连接','域名解析缓存','系统服务','进程加载模块'];
    assert(sample.every(label=>colorsByTheme[theme][label]),'all seven common categories are exercised');
    assert.equal(new Set(sample.map(label=>colorsByTheme[theme][label])).size,sample.length,'common categories use distinct colors in '+theme);
    assert(categories.every(item=>item.tone&&item.color),'every object category has a usable color');
    for(const label of sample){
      const chip=page.locator('[data-timeline-category-toggle="'+label+'"]');
      const row=page.locator('.timeline-branch[data-timeline-category="'+label+'"] .timeline-type').first();
      assert.equal(await row.evaluate(element=>getComputedStyle(element).color),await chip.evaluate(element=>getComputedStyle(element).color),'record type and switch colors match');
    }
  }
  await page.locator('#app').evaluate(app=>app.dataset.theme='dark');
  checks.push('category colors are stable and distinct for common types in both themes, match record types and stay usable when folded');
  await page.evaluate(data=>{window.__qa.timeline=data;window.__qa.renderWaits=[];window.__qa.holdRender=true;window.__qa.originalFrame=window.requestAnimationFrame;window.requestAnimationFrame=callback=>{if(window.__qa.holdRender&&document.querySelector('#timeline-results[aria-busy=true] .timeline-entry')){window.__qa.renderWaits.push(callback);return 0;}return window.__qa.originalFrame(callback);};document.querySelector('[data-page="timeline"]').click();},Array.from({length:200},(_,index)=>({...timelineRows[0],instance_id:index+1})));
  await page.waitForFunction(()=>window.__qa.renderWaits.length>0);
  assert.equal(await page.locator('.timeline-entry').count(),50,'the first render batch is bounded while the rest wait for the next frame');
  await page.evaluate(row=>window.__qa.timeline=[row],{...timelineRows[0],context:{type_id:'startup.service',subject_label:'新的筛选结果',attribution_status:'stored'}});await page.locator('#timeline-clear').click();await page.waitForTimeout(40);
  await page.evaluate(()=>{window.__qa.holdRender=false;window.requestAnimationFrame=window.__qa.originalFrame;for(const callback of window.__qa.renderWaits.splice(0))requestAnimationFrame(callback);});
  await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===1&&document.querySelector('#timeline-results').textContent.includes('新的筛选结果')&&document.querySelector('#timeline-results').getAttribute('aria-busy')==='false');
  assert.equal(await page.locator('#timeline-results').getAttribute('aria-busy'),'false');
  checks.push('a 200-row response renders in bounded 50-row batches and changing filters cancels stale pending render batches');
  const startupRows=Array.from({length:200},(_,index)=>({...timelineRows[0],instance_id:index+1,interval_from:index<80?'2026-10-15T03:00:30.1160614Z':index<83?'2026-10-15T03:00:29.9816849Z':'2026-10-15T02:00:00Z',interval_until:index<80?'2026-10-15T03:02:41.3133775Z':index<83?'2026-10-15T03:02:41.1726548Z':'2026-10-15T02:05:00Z',context:{type_id:index<80?'network.endpoint':index<83?'network.dns_cache_entry':'file.watched_file',subject_label:'首屏示例 '+index,attribution_status:'stored'}}));
  const afterStartup={...timelineRows[0],interval_from:'2026-10-15T01:00:00Z',interval_until:'2026-10-15T01:05:00Z'};
  await page.evaluate(({first,second})=>{window.__qa.timelinePages=[{status:'loaded',data:first,page:{has_more:true,next_cursor:'after-first-200'}},{status:'loaded',data:[second],page:{has_more:false}}];},{first:startupRows,second:afterStartup});
  const readsBeforeStartup=await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='action_timeline').length);
  await page.locator('[data-page="timeline"]').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===200&&document.querySelector('#timeline-results').getAttribute('aria-busy')==='false');
  assert.equal(await page.locator('.timeline-group-count').first().textContent(),'83 条记录','the first render merges network and DNS without scrolling');
  assert.equal(await page.locator('.timeline-group').first().locator('.timeline-category-toggle').count(),2);
  assert.equal(await page.locator('.timeline-group-count').last().textContent(),'117 条记录','lookahead proves the second group complete');
  await page.waitForTimeout(300);assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='action_timeline').length),readsBeforeStartup+2,'opening completes two groups and buffers only the transport lookahead');
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='action_timeline').at(-1).limit),200);
  await page.screenshot({path:path.join(output,'timeline-first-200-grouped.png'),fullPage:false});
  await page.locator('#timeline-more').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===201);
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.query==='action_timeline').at(-1).limit),200,'subsequent scrolling uses the measured 200-row batch');
  assert.equal(await page.locator('.timeline-group-count').nth(1).textContent(),'117 条记录','reading past the cutoff confirms and relabels the completed group');
  checks.push('startup completes two groups using bounded transport, merges categories, and reuses the buffered next group on scroll');
  // Keep this deliberately small first page outside the auto-loader's viewport margin
  // until the test scrolls, including when compilation makes browser steps slower.
  await page.setViewportSize({width:1440,height:640});
  const currentRange={precision:'interval',time_derived:true,interval_from:'2026-10-15T02:48:26Z',interval_until:'2026-10-15T02:50:18Z',observed_at:'2026-10-15T02:50:18Z',result:'observed'};
  const currentEvents=Array.from({length:5},(_,index)=>({...currentRange,...(index>=3?{interval_from:'2026-10-15T02:48:00Z',interval_until:'2026-10-15T02:50:05Z'}:{}),instance_id:index+1,action_id:'startup.service_state_changed',field_id:'state',before:'Stopped',after:'Running',context:{type_id:'startup.service',subject_label:index?'示例服务 '+index:'Volume Shadow Copy',attribution_status:'stored'}}));
  const olderEvents=Array.from({length:3},(_,index)=>({...currentRange,instance_id:index+6,interval_from:'2026-10-15T02:42:10Z',interval_until:'2026-10-15T02:45:00Z',action_id:'observed.configuration_changed',field_id:'target',before:'C:\\Applications\\Old Version\\bin\\tool.exe',after:'C:\\Applications\\New Version\\bin\\tool.exe',context:{type_id:'desktop.shortcut',subject_label:'开发工具快捷方式 '+index,attribution_status:'stored'}}));
  await page.evaluate(({first,second})=>{window.__qa.holdOlderPage=true;window.__qa.timelinePages=[{status:'loaded',data:first,page:{has_more:true,next_cursor:'older-page'}},{status:'loaded',data:second,page:{has_more:false}}];},{first:currentEvents.slice(0,3),second:[...currentEvents.slice(3),...olderEvents]});
  await page.locator('[data-page="timeline"]').click();await page.locator('.timeline-entry').first().waitFor();
  assert.equal(await page.locator('.timeline-group').count(),1);
  assert.doesNotMatch(await page.locator('.timeline-entry').first().innerText(),/状态发生变化|已记录|服务已开始运行/);
  await page.locator('.timeline-entry details summary').first().click();
  await page.evaluate(()=>{const group=document.querySelector('.timeline-group');group.querySelector('[data-timeline-invert]').click();group.querySelector('[data-timeline-toggle]').click();});
  await page.evaluate(()=>{window.__qa.holdOlderPage=false;window.__qa.releaseOlderPage?.();});
  await page.locator('#timeline-more').scrollIntoViewIfNeeded();await page.locator('#timeline-more').hover();await page.mouse.wheel(0,240);
  await page.locator('#timeline-more').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===8);
  assert.equal(await page.locator('.timeline-list').count(),1);assert.equal(await page.locator('.timeline-group').count(),2);
  assert.equal(await page.locator('.timeline-group-count').first().textContent(),'5 条记录');
  assert.match(await page.locator('.timeline-time-value').first().textContent(),/10:50.*10:55/,'bucket header stays aligned after appending older records');
  const mergedGroup=page.locator('.timeline-group').first();
  assert.equal(await mergedGroup.locator('.timeline-category-toggle:visible').count(),1,'appended counts stay visible while folded');
  assert.equal(await mergedGroup.locator('[data-timeline-toggle]').getAttribute('aria-expanded'),'false','append retains the group fold');
  assert.equal(await mergedGroup.locator('[data-timeline-category-toggle="系统服务"]').getAttribute('aria-pressed'),'false','append retains disabled types');
  assert.equal(await mergedGroup.locator('.timeline-category-count').textContent(),'5','append updates the type count');
  assert.equal(await page.locator('.timeline-group').nth(1).locator('.timeline-branch:visible').count(),3,'another interval is unaffected');
  await mergedGroup.locator('[data-timeline-toggle]').click();assert(await mergedGroup.locator('.timeline-group-empty').isVisible());
  await mergedGroup.locator('[data-timeline-category-toggle="系统服务"]').click();
  assert.equal(await page.locator('.timeline-entry details[open]').count(),1,'append preserves expanded details');
  assert.equal(await page.locator('.timeline-value-stack').count(),3);
  await page.locator('.timeline-entry details summary').first().click();
  const measure=()=>page.evaluate(()=>{const main=document.querySelector('main'),groups=[...document.querySelectorAll('.timeline-group')];return {main:main.getBoundingClientRect().top,inset:parseFloat(getComputedStyle(main).paddingTop),groups:groups.map(group=>({top:group.getBoundingClientRect().top,bottom:group.getBoundingClientRect().bottom,label:group.querySelector('.timeline-when').getBoundingClientRect().top}))};});
  await page.locator('main').evaluate(el=>{el.scrollTop+=el.querySelector('.timeline-group').getBoundingClientRect().top-el.getBoundingClientRect().top+100;});
  const pinned=await measure();assert(Math.abs(pinned.groups[0].label-pinned.main-pinned.inset-12)<3,'left time pins within main scroll: '+JSON.stringify(pinned));
  await page.locator('main').evaluate(el=>el.scrollTop+=150);const stillPinned=await measure();assert(Math.abs(stillPinned.groups[0].label-pinned.groups[0].label)<3);
  await page.locator('main').evaluate(el=>{const next=el.querySelectorAll('.timeline-group')[1];el.scrollTop+=next.getBoundingClientRect().top-el.getBoundingClientRect().top;});const handed=await measure();
  assert(handed.groups[0].bottom<handed.groups[1].label,'old interval ends before the next time rail');
  await page.locator('main').evaluate(el=>el.scrollTop=0);await page.screenshot({path:path.join(output,'timeline-grouped-product.png'),fullPage:false});
  await page.locator('#theme').click();await page.screenshot({path:path.join(output,'timeline-grouped-product-light.png'),fullPage:false});await page.locator('#theme').click();
  checks.push('grouped timeline merges paginated intervals, preserves disclosure state, fits long paths and pins time until the group ends');
  const shiftEvent=(row,seconds)=>({...row,interval_from:new Date(Date.parse(currentRange.interval_from)+seconds*1000).toISOString(),interval_until:new Date(Date.parse(currentRange.interval_until)+seconds*1000).toISOString()});
  await page.evaluate(({first,second})=>{window.__qa.timelinePages=[{status:'loaded',data:first,page:{has_more:true,next_cursor:'split-intervals'}},{status:'loaded',data:second,page:{has_more:false}}];},{first:currentEvents.slice(0,3),second:[shiftEvent(currentEvents[3],-20),shiftEvent(currentEvents[4],-40)]});
  await page.locator('[data-page="timeline"]').click();await page.locator('.timeline-entry').first().waitFor();
  await page.locator('#timeline-more').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===5);
  assert.deepEqual(await page.locator('.timeline-group-count').allTextContents(),['3 条记录','2 条记录'],'incoming records respect the fixed boundary across pages');
  const spans=await page.locator('.timeline-group').evaluateAll(groups=>groups.map(group=>JSON.parse(group.dataset.timeMeta)));
  for(const span of spans){assert.equal(Date.parse(span.end)-Date.parse(span.start),300000);}
  checks.push('fixed buckets remain stable across page breaks');
  const midnightEvents=[{...currentEvents[0],interval_from:'2026-10-14T15:58:00Z',interval_until:'2026-10-14T16:00:05Z'},{...currentEvents[1],interval_from:'2026-10-14T15:57:50Z',interval_until:'2026-10-14T15:59:55Z',context:{type_id:'network.dns_cache_entry',subject_label:'跨日示例',attribution_status:'stored'}}];
  await page.evaluate(data=>{window.__qa.timelinePages=[{status:'loaded',data:data.slice(0,1),page:{has_more:true,next_cursor:'across-midnight'}},{status:'loaded',data:data.slice(1),page:{has_more:false}}];},midnightEvents);
  await page.locator('[data-page="timeline"]').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===2);
  assert.equal(await page.locator('.timeline-group').count(),2);assert.equal(await page.locator('.timeline-day').count(),2,'midnight separates dates and buckets');
  assert.equal(await page.locator('.timeline-category-toggle').count(),2);assert.match(await page.locator('.timeline-time-value').last().textContent(),/23:55.*次日 00:00/);
  checks.push('midnight buckets stay separate while record intervals retain full dates');
  const pagedPoints=[{...pointRows[0],occurred_at:'2026-10-15T08:10:00Z'},{...pointRows[1],occurred_at:'2026-10-15T08:09:40Z'},{...pointRows[1],instance_id:3,occurred_at:'2026-10-15T08:09:20Z'}];
  await page.evaluate(data=>{window.__qa.holdPointOlder=true;window.__qa.timelinePages=[{status:'loaded',data:data.slice(0,1),page:{has_more:true,next_cursor:'point-older'}},{status:'loaded',data:data.slice(1),page:{has_more:false}}];},pagedPoints);
  await page.locator('[data-page="timeline"]').click();await page.locator('.timeline-entry').first().waitFor();
  assert.equal(await page.locator('.timeline-group-count').textContent(),'已加载 1 条','a point card near the page boundary remains open to nearby older records');
  await page.locator('.timeline-entry details summary').first().click();
  await page.evaluate(()=>{window.__qa.holdPointOlder=false;window.__qa.releasePointOlder?.();});
  await page.locator('#timeline-more').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===3);
  assert.deepEqual(await page.locator('.timeline-group-count').allTextContents(),['1 条记录','2 条记录']);
  assert.match(await page.locator('.timeline-time-value').first().textContent(),/16:10.*16:15/);
  assert.equal(await page.locator('.timeline-entry details[open]').count(),1);
  checks.push('point events use fixed boundaries across pages and preserve open details');
  await page.setViewportSize({width:1440,height:1000});
  // Keep the initial page longer than the screen-ahead buffer until folding is exercised.
  const foldRows=Array.from({length:45},(_,index)=>({...timelineRows[0],instance_id:index+1,interval_from:'2026-10-15T0'+(3-Math.floor(index/15))+':00:00Z',interval_until:'2026-10-15T0'+(3-Math.floor(index/15))+':05:00Z',context:{type_id:index%3===0?'runtime.process':index%3===1?'startup.service':'file.watched_file',subject_label:'模式示例 '+index,attribution_status:'stored'}}));
  const foldPages=[{status:'loaded',data:foldRows.slice(0,30),page:{has_more:true,next_cursor:'fold-older'}},{status:'loaded',data:foldRows.slice(30),page:{has_more:false,next_cursor:null}}];
  await page.locator('main').evaluate(el=>el.scrollTop=0);
  await page.evaluate(pages=>window.__qa.timelinePages=pages,foldPages);
  await page.locator('[data-page="timeline"]').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===30&&document.querySelector('#timeline-results').getAttribute('aria-busy')==='false');
  const foldButton=page.locator('#timeline-fold-all'),firstFoldGroup=page.locator('.timeline-group').first();
  assert.equal(await foldButton.textContent(),'全部折叠');
  await firstFoldGroup.locator('.timeline-entry details summary').first().click();
  await firstFoldGroup.locator('[data-timeline-category-toggle="运行进程"]').click();
  const foldCalls=await page.evaluate(()=>window.__qa.calls.length);
  await foldButton.click();await page.waitForTimeout(350);
  assert.equal(await page.locator('.timeline-group.is-collapsed').count(),2);assert.equal(await foldButton.textContent(),'全部展开');
  assert.equal(await page.evaluate(()=>localStorage.getItem('pcscope.timelineCollapsed')),'true');
  assert.equal(await page.locator('.timeline-category-toggle:visible').count(),6);
  assert.equal(await page.evaluate(()=>window.__qa.calls.length),foldCalls,'folding must not start a history walk when the page gets shorter');
  await firstFoldGroup.locator('[data-timeline-toggle]').click();
  assert.equal(await page.locator('.timeline-group.is-collapsed').count(),1,'individual groups remain independent in collapsed mode');
  await foldButton.click();assert.equal(await page.locator('.timeline-group.is-collapsed').count(),0);
  assert.equal(await firstFoldGroup.locator('[data-timeline-category-toggle="运行进程"]').getAttribute('aria-pressed'),'false');
  assert.equal(await firstFoldGroup.locator('details[open]').count(),1,'bulk switches preserve disclosed details');
  await foldButton.focus();await page.keyboard.press('Space');
  await firstFoldGroup.locator('[data-timeline-toggle]').click();
  assert.equal(await page.evaluate(()=>window.__qa.calls.length),foldCalls,'bulk and individual switches do not call IPC');
  await page.locator('#timeline-more').scrollIntoViewIfNeeded();
  await page.locator('#timeline-more').hover();await page.mouse.wheel(0,240);
  await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===45);
  assert.equal(await page.locator('.timeline-group.is-collapsed').count(),2,'new older groups follow collapsed mode while an existing manual expansion survives');
  assert.equal(await firstFoldGroup.locator('details[open]').count(),1);
  assert.equal(await firstFoldGroup.locator('[data-timeline-category-toggle="运行进程"]').getAttribute('aria-pressed'),'false');
  await page.waitForTimeout(350);assert.equal(await page.locator('.timeline-entry').count(),45,'one scroll must not drain pages while folded cards leave the sentinel visible');
  checks.push('bulk fold and expand preserve categories and details; manual exceptions survive and older groups follow the mode without a folding-triggered history walk');
  await firstFoldGroup.locator('[data-timeline-toggle]').click();
  await page.locator('#load-latest').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===30&&document.querySelector('#timeline-results').getAttribute('aria-busy')==='false');
  assert.equal(await page.locator('.timeline-group.is-collapsed').count(),2,'manual refresh retains the selected mode');
  const refreshFoldCalls=await page.evaluate(()=>window.__qa.calls.filter(call=>call.query==='action_timeline').length);
  await page.waitForTimeout(350);assert.equal(await page.evaluate(()=>window.__qa.calls.filter(call=>call.query==='action_timeline').length),refreshFoldCalls,'collapsed refresh reads two complete groups only');
  await page.screenshot({path:path.join(output,'timeline-all-collapsed.png'),fullPage:false});
  await page.reload();await page.locator('#timeline-fold-all').waitFor();
  assert.equal(await page.locator('#timeline-fold-all').textContent(),'全部展开','the mode survives a full page reload');
  await page.evaluate(pages=>{window.__qa.timelinePages=pages;window.__qa.listFailures=0;},foldPages);
  await page.locator('[data-page="timeline"]').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===30&&document.querySelector('#timeline-results').getAttribute('aria-busy')==='false');
  assert.equal(await page.locator('.timeline-group.is-collapsed').count(),2);
  await page.locator('#timeline-fold-all').click();
  assert.equal(await page.locator('.timeline-group.is-collapsed').count(),0);
  assert.equal(await page.evaluate(()=>localStorage.getItem('pcscope.timelineCollapsed')),'false');
  checks.push('bulk fold mode survives refresh and reload, restores one initial page only, and switches back to expanded mode');
  const groupSizes=[230,260,50,300,40,70];
  const completeRows=groupSizes.flatMap((count,g)=>Array.from({length:count},(_,i)=>({...pointRows[0],instance_id:g*1000+i+1,occurred_at:new Date(Date.UTC(2026,9,15,8,55)-g*300000).toISOString(),context:{type_id:i%2?'startup.service':'runtime.process',subject_label:'完整组 '+g+' / '+i,attribution_status:'stored'}})));
  await page.evaluate(data=>{window.__qa.timelineDataset=data;window.__qa.failGroupAt=200;},completeRows);
  await page.locator('[data-page="timeline"]').click();await page.locator('#timeline-more button:visible').waitFor();
  assert.equal(await page.locator('.timeline-entry').count(),200,'a failed continuation retains the partial group');
  await page.locator('#timeline-more button').click();
  await page.waitForFunction(()=>document.querySelector('#timeline-results').getAttribute('aria-busy')==='false'&&document.querySelectorAll('.timeline-entry').length===490);
  assert.deepEqual(await page.locator('.timeline-group-count').allTextContents(),['230 条记录','260 条记录']);
  assert.equal(await page.locator('.timeline-branch').evaluateAll(rows=>new Set(rows.map(r=>r.querySelector('[data-object-alias]').dataset.objectAlias)).size),490,'retry never duplicates streamed rows');
  const completeGroup=page.locator('.timeline-group').first();
  await completeGroup.locator('details summary').first().click();await completeGroup.locator('[data-timeline-category-toggle]').first().click();
  await page.locator('#timeline-fold-all').click();
  const beforeCompleteScroll=await page.evaluate(()=>window.__qa.calls.length);
  await page.waitForTimeout(300);assert.equal(await page.evaluate(()=>window.__qa.calls.length),beforeCompleteScroll,'folding does not load extra groups');
  await page.locator('#timeline-more').hover();await page.mouse.wheel(0,240);
  await page.waitForFunction(()=>document.querySelector('#timeline-results').getAttribute('aria-busy')==='false'&&document.querySelectorAll('.timeline-entry').length===840);
  assert.equal(await page.locator('.timeline-group').count(),4);
  assert.equal(await completeGroup.locator('details[open]').count(),1);assert.equal(await completeGroup.locator('[aria-pressed=false]').count(),1);
  await page.waitForTimeout(350);assert.equal(await page.locator('.timeline-entry').count(),840,'one intent loads two complete groups without walking the entire history');
  await page.locator('#timeline-more').hover();await page.mouse.wheel(0,240);
  await page.waitForFunction(()=>document.querySelector('#timeline-results').getAttribute('aria-busy')==='false'&&document.querySelectorAll('.timeline-entry').length===950);
  assert.equal(await page.locator('.timeline-group').count(),6);assert.match(await page.locator('#timeline-more').innerText(),/已显示全部/);
  checks.push('complete groups span transport pages, preserve local selections and details, recover partial failure exactly once and load two groups per scrolling intent');
  for(const n of [3,4,5,6,2]){
    await page.locator('#timeline-groups-per-load').selectOption(String(n));
    const total=groupSizes.slice(0,n).reduce((sum,count)=>sum+count,0);
    await page.waitForFunction(({n,total})=>document.querySelector('#timeline-results').getAttribute('aria-busy')==='false'&&document.querySelectorAll('.timeline-entry').length===total&&document.querySelectorAll('.timeline-group').length===n,{n,total});
    assert.equal(await page.evaluate(()=>localStorage.getItem('pcscope.timelineGroupsPerLoad')),String(n));
  }
  await page.locator('#timeline-groups-per-load').selectOption('4');await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===840&&document.querySelector('#timeline-results').getAttribute('aria-busy')==='false');
  await page.locator('#timeline-clear').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===840&&document.querySelector('#timeline-results').getAttribute('aria-busy')==='false');
  assert.equal(await page.locator('#timeline-groups-per-load').inputValue(),'4');
  await page.reload();await page.locator('#timeline-groups-per-load').waitFor();assert.equal(await page.locator('#timeline-groups-per-load').inputValue(),'4');
  await page.evaluate(()=>localStorage.setItem('pcscope.timelineGroupsPerLoad','7'));await page.reload();await page.locator('#timeline-groups-per-load').waitFor();assert.equal(await page.locator('#timeline-groups-per-load').inputValue(),'2');
  await page.evaluate(()=>window.__qa.listFailures=0);
  if(await page.locator('#timeline-fold-all').textContent()==='全部展开')await page.locator('#timeline-fold-all').click();
  checks.push('2–6 groups per load are exact, remember the choice across clear/reload and reject invalid stored values');
  const readings=Array.from({length:3},(_,index)=>({source:'pcscope.execution.module-file-state',item_id:'module-'+index+'.dll',observed_at:'2026-10-15T02:50:18Z',evidence:{id:'sample-'+index},payload:{path:'C:\\Example\\module-'+index+'.dll',check:{status:'stable'}}}));
  await page.evaluate(rows=>{window.__qa.sourcePages=[{status:'loaded',data:rows.slice(0,2),page:{has_more:true,next_cursor:'older-readings'}},{status:'loaded',data:rows.slice(2),page:{has_more:false}}];},readings);
  await page.locator('#module-file-history>summary').scrollIntoViewIfNeeded();await page.locator('#module-file-history>summary').click();
  await page.locator('.module-file-card').first().waitFor();await page.locator('#module-files-more').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('.module-file-card').length===3);
  assert.equal(await page.locator('#module-files-more button:visible').count(),0);
  checks.push('module-file history scroll appends earlier checks without a paging button');
  await page.locator('[data-page="health"]').click();await page.locator('#source-readings details').first().waitFor();await page.locator('#readings-next').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('#source-readings .source-reading-records > details').length===3);
  checks.push('resource readings append pages instead of replacing earlier records');
  const actionRows=[{action_id:'observed.configuration_changed',field_id:'display_version',before:'1.0',after:'2.0',context:{type_id:'software.installation_registration'}},{action_id:'observed.configuration_changed',field_id:'display_version',before:'2.0',after:'3.0',context:{type_id:'software.installation_registration'}}];
  await page.evaluate(rows=>{window.__qa.actionPages=[{status:'loaded',data:rows.slice(0,1),page:{has_more:true,next_cursor:'older-actions'}},{status:'loaded',data:rows.slice(1),page:{has_more:false}}];},actionRows);
  await page.locator('[data-page="compare"]').click();await page.locator('#compare-instance').selectOption(rows[0].alias);await page.locator('#compare-more').scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelectorAll('#compare-results tbody>tr').length===4);
  checks.push('state comparison automatically reads beyond the first action-history page');
  const bucketPerformance=await page.evaluate(base=>{
    const input=Array.from({length:1000},(_,i)=>({...base,instance_id:i+1,occurred_at:'2026-10-15T08:09:'+String(i%60).padStart(2,'0')+'Z'}));
    const root=document.createElement('div');root.style.display='none';document.body.append(root);
    const controls=PCScopeTimelineGroups.mount(root),start=performance.now();
    for(const {html} of PCScopeModelView.timelineBatches({data:input}))controls.append(html);
    controls.markBoundary(false);
    const result={rows:input.length,batches:20,append_ms:performance.now()-start,groups:root.querySelectorAll('.timeline-group').length,count:root.querySelector('.timeline-category-count').textContent};
    const grouping=[];for(let i=0;i<30;i++){const before=performance.now();PCScopeTimelineTime.groupRows(input);grouping.push(performance.now()-before);}
    grouping.sort((a,b)=>a-b);result.grouping_p95_ms=grouping[Math.ceil(grouping.length*0.95)-1];
    controls.dispose();root.remove();return result;
  },pointRows[0]);
  assert.equal(bucketPerformance.groups,1);assert.equal(bucketPerformance.count,'1000');
  fs.writeFileSync(path.join(output,'timeline-bucket-performance.json'),JSON.stringify(bucketPerformance,null,2));
  checks.push('1,000 records appended in 20 batches retain one bucket and exact incremental category totals');
  await page.evaluate(()=>{timelineGroupsPerLoad=6;localStorage.setItem('pcscope.timelineGroupsPerLoad','6');});
  const preferenceRows=Array.from({length:6},(_,index)=>({...pointRows[0],instance_id:index+1,occurred_at:'2026-10-15T08:'+String(index*10).padStart(2,'0')+':30Z'})).reverse();
  await page.evaluate(data=>{window.__qa.timelinePages=[{status:'loaded',data,page:{has_more:false}}];},preferenceRows);
  await page.locator('[data-page="timeline"]').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===6);
  assert.deepEqual(await page.locator('#timeline-group-minutes option').evaluateAll(nodes=>nodes.map(n=>n.value)),['1','5','10','15','30','60']);
  for(const [minutes,count] of [['1',6],['5',6],['10',6],['15',4],['30',2],['60',1]]){
    await page.locator('#timeline-group-minutes').selectOption(minutes);
    await page.waitForFunction(({minutes,count})=>document.querySelectorAll('.timeline-entry').length===6&&document.querySelectorAll('.timeline-group').length===count&&[...document.querySelectorAll('.timeline-group')].every(g=>JSON.parse(g.dataset.timeMeta).minutes===Number(minutes)),{minutes,count});
    assert.equal(await page.evaluate(()=>localStorage.getItem('pcscope.timelineGroupMinutes')),minutes);
  }
  await page.locator('#timeline-clear').click();await page.waitForFunction(()=>document.querySelectorAll('.timeline-entry').length===6);
  assert.equal(await page.locator('#timeline-group-minutes').inputValue(),'60','clearing filters preserves the grouping preference');
  await page.reload();await page.locator('#timeline-group-minutes').waitFor();
  assert.equal(await page.locator('#timeline-group-minutes').inputValue(),'60','full reload remembers the selected interval');
  await page.evaluate(()=>localStorage.setItem('pcscope.timelineGroupMinutes','7'));await page.reload();await page.locator('#timeline-group-minutes').waitFor();
  assert.equal(await page.locator('#timeline-group-minutes').inputValue(),'5','invalid saved selection falls back to five minutes');
  checks.push('all six grouping intervals regroup correctly; selection survives navigation, clear filters and reload, with invalid-value fallback');
  assert.deepEqual(errors,[]);assert.deepEqual(await page.evaluate(()=>[...new Set(window.__qa.calls.map(c=>c.operation))].filter(op=>!['status','query','monitoring_settings'].includes(op))),[]);
  fs.writeFileSync(path.join(output,'object-interactions-result.json'),JSON.stringify({passed:true,checks,page_errors:errors,basis:'Actual index.html and app.js in headless Chrome; fixed read-only IPC; no native collection or UAC.'},null,2));console.log('PASS: '+checks.length+' real app interaction scenarios; no collection calls or browser errors.');
 }finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
})().catch(error=>{console.error(error);process.exitCode=1;});
