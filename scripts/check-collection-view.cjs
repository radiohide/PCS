const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict'),path=require('node:path');
const root=path.join(__dirname,'..'),catalog=JSON.parse(fs.readFileSync(path.join(root,'apps/desktop/ui/collection-catalog.json')));
const context=vm.createContext({fetch:async()=>({ok:true,json:async()=>catalog}),PCScopeDetails:require(path.join(root,'apps/desktop/ui/object-details.js'))});
vm.runInContext(fs.readFileSync(path.join(root,'apps/desktop/ui/collection-view.js'),'utf8')+';globalThis.view=PCScopeView;',context);
(async()=>{
const v=context.view;await v.ready;
assert.equal(v.itemName({source:'pcscope.shell.registrations',id:'software\\classes\\clsid\\abc'}),'COM 实现与扩展文件元数据');
const unknownSource={source:'unknown.plugin',id:'S-1-5-21-123',kind:'vendor.unknown_operation',data:{reason:'EvtQuery failed: access denied'}};
const originalUnknown=JSON.stringify(unknownSource);
assert.equal(v.itemName(unknownSource),'其他系统记录');
assert.equal(v.objectName(unknownSource),'未识别的系统项目');
assert.equal(v.action(unknownSource),'系统活动');
assert.equal(JSON.stringify(unknownSource),originalUnknown,'readable fallbacks must preserve original source and object identity');
assert.doesNotMatch(v.summary({...unknownSource,kind:'coverage.gap'}),/EvtQuery|access denied/);
assert.match(v.summary({...unknownSource,kind:'coverage.gap'}),/采集状态发生变化/);
assert.equal(v.objectName({data:{path:'C:\\Program Files\\Demo\\app.exe'}}),'app.exe');
assert.equal(v.objectName({data:{path:'C:\\Registry\\{01234567-89ab-cdef-0123-456789abcdef}'}}),'未识别的系统项目');
assert.match(v.attributes({data:{status:'future_unknown_state'}}),/尚未识别/);
assert.doesNotMatch(v.attributes({data:{status:'future_unknown_state'}}),/future_unknown_state/);
assert.equal(v.objectName({source:'pcscope.software.user-inventory',data:{values:{DisplayName:{kind:'String',value:'Demo'}}}}),'Demo');
assert.equal(v.objectName({data:{after:{display_name:'当前服务'},before:{display_name:'旧服务'}}}),'当前服务');
assert(v.summary({kind:'object.changed',data:{before:{enabled:false},after:{enabled:true}}}).includes('否 → 是'));
assert(v.summary({kind:'object.changed',data:{before:{value:{redacted:true,sha256:'old'}},after:{value:{redacted:true,sha256:'new'}}}}).includes('原值未保存'));
assert(v.summary({kind:'object.first_seen',data:{after:{DisplayName:'Demo'}}}).startsWith('当时首次记录到此项'));
assert(v.summary({kind:'object.no_longer_observed',data:{}}).includes('检查时，清单中已无此前记录的项目'));
assert.equal(new Set(catalog.map(r=>r.id)).size,catalog.length);
const pluginSources=require(path.join(root,'apps/desktop/ui/plugin-sources.js'));
for(const row of pluginSources.eventlog){const item=catalog.find(d=>d.id===row.id);assert.equal(item.name,row.name);assert.equal(item.category,'eventlog');assert.equal(item.group,row.group);}
const security={source:'pcscope.eventlog.security',kind:'execution.process_started',data:{provider:'Microsoft-Windows-Security-Auditing',event_id:4688}};
assert.equal(v.itemName(security),'Windows 安全审计 / 进程生命周期');
assert.equal(v.action(security),'进程启动');
assert.equal(v.itemName({...security,data:{provider:'wrong-provider',event_id:4688}}),'Windows 安全审计 / 其他事件');
assert.equal(v.itemName({source:security.source,kind:'coverage.gap',data:{}}),'Windows 安全审计 / 来源采集状态');
assert.equal(v.itemName({source:'pcscope.eventlog.microsoft-windows-sysmon/operational',kind:'execution.sysmon_event',data:{provider:'Microsoft-Windows-Sysmon',event_id:22}}),'Sysmon / DNS 查询');
assert.equal(v.action({kind:'coverage.recovered'}),'采集恢复');
// Exercise the timeline page's filter handlers without invoking collection or native UI.
//
// The page reads the model's action query now rather than the old record search, so what is checked
// is the request it builds. That is the thing worth pinning: a filter that quietly stopped being
// sent would be a filter that does nothing, and the page would look like it was narrowing results
// when it was not.
const nodes=new Map();let pending,lastQuery;
const content={addEventListener(){},querySelector(selector){if(!nodes.has(selector))nodes.set(selector,{isConnected:true,dataset:{},value:'',innerHTML:'',textContent:'',addEventListener(){},setAttribute(){},insertAdjacentHTML(){},querySelector(){return null;},querySelectorAll(){return [];}});return nodes.get(selector);}};
Object.assign(context,{
  content,state:{page:'timeline',reloadPage:null},categoryNames:{startup:'启动项'},
  PCScopeModelView:require(path.join(root,'apps/desktop/ui/model-view.js')),
  PCScopeObjectNames:require(path.join(root,'apps/desktop/ui/object-names.js')),
  PCScopeReadableEvents:require(path.join(root,'apps/desktop/ui/readable-events.js')),
  head:()=>'',esc:String,errorText:String,notice:()=>{},collect:()=>{throw Error('filter must not collect');},
  mountModuleFileHistory:()=>{}, // Separate lazy history panel; this fixture isolates action filters.
  clearTimeout:()=>{},setTimeout:fn=>{pending=fn;return 1;},
  localStorage:{getItem:()=>null,setItem(){}},
  AbortController,
  PCScopeTimelinePages:require('../apps/desktop/ui/timeline-pages.js'),
  PCScopeTimelineTime:require('../apps/desktop/ui/timeline-time.js'),
  PCScopeTimelineGroups:{mount:()=>({append(){},sync(){},markBoundary(){},dispose(){},setAllCollapsed(){}})},
  PCScopeAutoPage:{mount:(_node,options)=>({pause(){},reset:async()=>options.apply(await options.query(null),{append:false})})},
  api:async(operation,args)=>{
    const request=args&&args.request?args.request:{};
    // The page loads the type catalog once to build its two dropdowns.
    if(request.query==='type_catalog')return {status:'loaded',data:[{type_id:'startup.scheduled_task',category:'startup',name:'计划任务',supplied_by_source:true}]};
    lastQuery=JSON.parse(JSON.stringify(request));
    return {status:'loaded',data:[],page:{has_more:false,watermark:'w-1'},counts:{}};
  }});
const app=fs.readFileSync(path.join(root,'apps/desktop/ui/app.js'),'utf8');
vm.runInContext(app.slice(app.indexOf('const timelineFilters=')),context);
await context.renderTimeline();

// The first page: the model's action query, with unattributed actions included by default. An
// action nobody could attribute is still something that happened.
assert.equal(lastQuery.query,'action_timeline');
assert.equal(lastQuery.include_unassigned,true,'未归属动作必须默认包含');
assert.equal(lastQuery.limit,200);

content.querySelector('#timeline-category').onchange({target:{value:'startup'}});await pending();
assert.equal(lastQuery.category,'startup');
content.querySelector('#timeline-type').onchange({target:{value:'startup.scheduled_task'}});await pending();
assert.equal(lastQuery.type_id,'startup.scheduled_task');
content.querySelector('#timeline-action').oninput({target:{value:'startup.task_started'}});await pending();
assert.equal(lastQuery.action_id,'startup.task_started');

// A time range becomes a lower bound on the time things happened, not a filter the backend guesses.
content.querySelector('#timeline-period').onchange({target:{value:'7'}});await pending();
assert(!Number.isNaN(Date.parse(lastQuery.since)),'时间范围必须变成 since 下界');

// Excluding unattributed actions has to be asked for explicitly.
content.querySelector('#timeline-unassigned').onchange({target:{value:'false'}});await pending();
assert.equal(lastQuery.include_unassigned,false,'排除未归属必须是显式的');

content.querySelector('#timeline-clear').onclick();await pending();
assert.equal(lastQuery.action_id,undefined);assert.equal(lastQuery.category,undefined);assert.equal(lastQuery.type_id,undefined);
assert.equal(lastQuery.include_unassigned,true,'清空筛选后回到默认的包含未归属');

console.log('Collection display checks passed: subitems, unknown sources, registry names, before/after, evidence wording, unique IDs, and the timeline request filters.');
})().catch(e=>{console.error(e);process.exitCode=1;});
