'use strict';
const queryTimedOut=error=>/deadline has elapsed|operation timed out|query[ _-]?timeout|查询超时|读取超时/i.test(error?.message||String(error));
const errorText = error => {
  const text=error?.message||String(error);
  if(queryTimedOut(error))return '读取超时，请稍后重试；已有记录仍保留。';
  if(/access.?denied|permission denied/i.test(text))return '权限不足，相关信息可能无法读取。';
  if(/broken pipe|pipe.*not found|connect.*fail|service.*unavailable/i.test(text))return '后台暂未连接，监控信息无法更新。';
  return /[\u4e00-\u9fff]/.test(text)&&text.length<=200?text:'操作未完成，请重试或查看问题详情。';
};
const root=document.querySelector('#app'), content=document.querySelector('#content');
const pages=[['overview','◫','系统总览'],['timeline','≡','时间线'],['objects','▦','系统对象'],['compare','⇄','状态对比'],['plugins','◇','采集插件'],['health','⌁','采集健康'],['settings','⚙','系统配置']];
const state={page:'timeline',status:null,permissionBackendPid:null,events:[],selected:null,search:'',theme:localStorage.getItem('pcscope.theme')||'dark',objectOffset:0,autoRefresh:true,refreshEpoch:0,refreshPolling:false,loadingLatest:false,reloadPage:null};
const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const localTime=value=>{const date=new Date(value);return Number.isNaN(date.getTime())?String(value??''):date.toLocaleString('sv-SE',{hour12:false});};
const names={'baseline.observed':'初始采集记录','object.changed':'属性变化','object.first_seen':'首次记录到此项','object.no_longer_observed':'检查时已不在清单中','coverage.gap':'采集缺口','coverage.recovered':'采集恢复','coverage.log_reset':'日志断点失效','coverage.unclean_shutdown':'后台异常中断','collector.started':'后台启动','collector.stopped':'后台停止','windows.event':'系统事件'};
const issues=window.PCScopeIssues=PCScopeStatusIssues.mount({button:document.querySelector('#issues-toggle'),panel:document.querySelector('#issues-panel'),onAction:async action=>{
  if(action==='connect')return connect(true);
  if(action==='startup'){await startupDesktop();issues.remove('startup');return;}
  issues.hide();
  if(action==='permissions'){PCScopeSettings.openPermissions();state.page='settings';}
  else state.page='health';
  await render();
}});
// Page errors stay with the page. Routine operation feedback belongs beside its button.
function notice(text,detail){
  let el=content.querySelector('#page-feedback');
  if(!text){el?.remove();return;}
  if(!el){el=document.createElement('div');el.id='page-feedback';el.className='page-feedback';el.setAttribute('role','status');content.prepend(el);}
  el.innerHTML=esc(text)+(detail?`<details><summary>问题详情</summary><pre>${esc(detail?.message||String(detail))}</pre></details>`:'');
}
function errorHtml(error,message){const detail=error?.message||String(error);return esc(message||errorText(error))+`<details><summary>问题详情</summary><pre>${esc(detail)}</pre></details>`;}
function theme(){root.dataset.theme=state.theme;const settingTheme=content.querySelector('#setting-theme');if(settingTheme)settingTheme.value=state.theme;document.querySelector('#theme').textContent=state.theme==='dark'?'切换浅色':'切换深色';}
async function invoke(command,args){if(!window.__TAURI__?.core)throw new Error('请通过 PCScope 桌面程序打开；浏览器预览未连接后台。');return window.__TAURI__.core.invoke(command,args);}
const queuedQuery=PCScopeQueryQueue.create(request=>invoke('request',{request}));
async function api(operation,args={}){const request={operation,...args};return operation==='query'?queuedQuery(request):invoke('request',{request});}
function head(title,description,action=''){return `<div class="page-head"><div><div class="eyebrow">本机状态与变化</div><h1>${title}</h1><p>${description}</p></div>${action}</div>`;}
// Counts from the old records/objects/snapshots tables, kept for the pages that still read them.
// The extra class is load-bearing: the overview renders its own counter block from the object
// model, so the refresh path below must be able to tell the two apart. Selector `.stats` alone
// matched the overview's block and replaced model counts with these legacy ones.
function stats(){const c=state.status?.counts;return `<div class="stats stats-legacy">${[['保留记录',c?.records],['当前对象',c?.objects],['状态采集批次',c?.snapshots]].map(([label,n])=>`<div class="stat"><span class="muted">${label}</span><strong>${n??'—'}</strong></div>`).join('')}</div>`;}
const objectFilters={search:'',category:'',item:'',period:'',sort:'item_asc'};
const categoryNames={account:'用户与账户',file:'文件与文件夹',runtime:'运行程序与会话',collector:'采集配置',environment:'环境变量',policy:'系统策略',identity:'用户与身份',software:'软件',apps:'应用与扩展',startup:'启动项',services:'服务',drivers:'驱动',network:'网络',hardware:'硬件',storage:'存储',system:'操作系统',security:'安全',shell:'资源管理器扩展',desktop:'桌面',virtualization:'虚拟化',execution:'进程与资源',files:'文件',usn:'文件变更日志',eventlog:'事件日志',other:'其他'};
const periodNames={'':'全部时间','1':'最近 24 小时','7':'最近 7 天','30':'最近 30 天'};
// S23: the objects page reads the object model rather than the old source-and-object list.
//
// The difference a user sees: an object is shown with its type, its identity status and its
// lifecycle as words; every field says where it came from and whether it is known, unreadable or
// conflicting; and the actions observed on it carry the interval they were derived over rather
// than a moment no source recorded.
const modelFilters={search:'',category:'',type_id:'',lifecycle:'',sort:'name_asc'};
let objectView=localStorage.getItem('pcscope.objectView')==='tree'?'tree':'list';
// One walk at a time: every page answers under the watermark its first page was issued with, so
// rows arriving mid-walk appear on the next query rather than shifting this one.
let modelWalk={cursor:null,watermark:null};
async function renderObjects(){
  content.innerHTML=head('系统对象','查看设备、程序和配置的最近状态。',`<div class="object-view-switch" role="group" aria-label="对象显示方式"><button data-object-view="list" aria-pressed="${objectView==='list'}">列表</button><button data-object-view="tree" aria-pressed="${objectView==='tree'}">树形</button></div>`)+`<section class="panel"><div class="object-filters">
    <label class="object-search">搜索对象<input id="model-search" type="search" maxlength="1024" placeholder="搜索名称" value="${esc(modelFilters.search)}"></label>
    <label>类别<select id="model-category"><option value="">全部类别</option></select></label>
    <label>类型<select id="model-type"><option value="">全部类型</option></select></label>
    <label>记录状态<select id="model-lifecycle">${[['','最近记录'],['active','最近有记录'],['not_observed','本次清单未列出'],['ended','运行或会话已结束'],['removed','已删除或移除'],['any','全部（含历史）']].map(([v,n])=>`<option value="${v}" ${v===modelFilters.lifecycle?'selected':''}>${n}</option>`).join('')}</select></label>
    <label>排序<select id="model-sort">${[['name_asc','名称 A → Z'],['name_desc','名称 Z → A'],['changed_desc','最近变化优先'],['seen_desc','最近观察优先'],['type_asc','按类型']].map(([v,n])=>`<option value="${v}" ${v===modelFilters.sort?'selected':''}>${n}</option>`).join('')}</select></label>
    <button id="model-clear">清空筛选</button></div><div id="model-results" aria-live="polite"></div><div id="model-more"></div></section>`;
  const results=content.querySelector('#model-results'),more=content.querySelector('#model-more');
  let catalog=[],requestId=0,detailSequence=0,timer,treeView;
  const options=()=>{
    const categories=[...new Set(catalog.map(t=>t.category))];
    content.querySelector('#model-category').innerHTML='<option value="">全部类别</option>'+categories.map(c=>`<option value="${esc(c)}" ${c===modelFilters.category?'selected':''}>${esc(categoryNames[c]||'其他类别')}</option>`).join('');
    content.querySelector('#model-type').innerHTML='<option value="">全部类型</option>'+catalog.filter(t=>!modelFilters.category||t.category===modelFilters.category).map(t=>`<option value="${esc(t.type_id)}" ${t.type_id===modelFilters.type_id?'selected':''}>${esc(PCScopeObjectNames.typeName(t))}${t.supplied_by_source?'':'（尚未接入来源）'}</option>`).join('');
  };
  const request=()=>{
    const body={query:'instance_list',sort:modelFilters.sort,limit:100,include_counts:false,include_instance_total:!modelWalk.cursor};
    if(modelFilters.category)body.category=modelFilters.category;
    if(modelFilters.type_id)body.type_id=modelFilters.type_id;
    if(modelFilters.search)body.search=modelFilters.search;
    if(modelFilters.lifecycle)body.lifecycle=modelFilters.lifecycle;
    if(modelWalk.cursor)body.cursor=modelWalk.cursor;
    return body;
  };
  // The alias is what a reference carries before an id exists and after one does, so the detail
  // links use it rather than the row's position.
  const bindDetails=container=>{
    container.querySelectorAll('[data-object-alias]').forEach(button=>{
      if(button.onclick)return;
      // Each response page starts its row positions at zero. The control target must remain
      // unique across appended pages, while the query always uses the original object alias.
      button.setAttribute('aria-controls','object-detail-'+(++detailSequence));
      let pendingDetail;
      const readDetail=()=>{
        if(!pendingDetail)pendingDetail=api('query',{request:{query:'instance_detail',instance:button.dataset.objectAlias}}).finally(()=>{pendingDetail=null;});
        return pendingDetail;
      };
      button.onclick=async()=>{
      const expanded=button.getAttribute('aria-expanded')==='true',detailId=button.getAttribute('aria-controls');
      if(expanded){container.querySelector('#'+CSS.escape(detailId))?.remove();button.setAttribute('aria-expanded','false');button.textContent='查看';return;}
      const row=button.closest('tr'),line=document.createElement('tr'),cell=document.createElement('td');
      cell.colSpan=8;cell.innerHTML='<div class="empty">正在读取对象详情…</div>';
      line.id=detailId;line.className='object-detail-row';line.append(cell);row.after(line);
      button.setAttribute('aria-expanded','true');button.textContent='收起';
      const load=async()=>{
        if(!cell.isConnected)return;
        cell.innerHTML='<div class="empty">正在读取对象详情…</div>';
        try{
          const response=await readDetail();
          if(!cell.isConnected)return;
          cell.innerHTML=response.status==='empty'?`<div class="empty">${esc(PCScopeModelView.messageOf(response,'没有该对象的详情'))}</div>`:PCScopeModelView.detail(response);
        }catch(error){
          if(!cell.isConnected)return;
          cell.innerHTML=`<div class="empty">${errorHtml(error)}<br><button class="object-detail-retry">重试</button></div>`;
          cell.querySelector('.object-detail-retry').onclick=load;
        }
      };
      await load();
      };
    });
  };
  const pager=PCScopeAutoPage.mount(more,{
    active:()=>objectView==='list',
    query:cursor=>{modelWalk.cursor=cursor;return api('query',{request:request()});},
    onStart:({append})=>{if(!append){results.setAttribute('aria-busy','true');results.innerHTML='<div class="empty">正在读取对象…</div>';}},
    apply:(response,{append})=>{
      modelWalk.watermark=response.page?.watermark||null;
      const html=PCScopeModelView.list(response);
      if(append){if(!PCScopeModelView.isEmpty(response))results.insertAdjacentHTML('beforeend',html);}else results.innerHTML=PCScopeModelView.listSummary(response)+html;
      results.setAttribute('aria-busy','false');bindDetails(results);
    },
    onError:(error,{append})=>{results.setAttribute('aria-busy','false');if(!append){const message=queryTimedOut(error)?'对象列表读取超时。可重试，或按类别、类型、名称筛选；已有记录仍保留。':null;results.innerHTML=`<div class="empty">${errorHtml(error,message)}</div>`;}}
  });
  const refresh=async()=>{
    if(!results.isConnected)return;
    treeView?.dispose();treeView=null;
    if(objectView==='tree'){
      pager.pause();results.setAttribute('aria-busy','false');
      treeView=PCScopeObjectTree.mount(results,{catalog,filters:{...modelFilters},categoryNames,query:request=>api('query',{request}),esc,errorHtml});return;
    }
    return pager.reset();
  };
  const reset=()=>{modelWalk={cursor:null,watermark:null};};
  const schedule=(delay=250)=>{clearTimeout(timer);++requestId;pager.pause();treeView?.dispose();reset();timer=setTimeout(()=>refresh(),delay);};
  for(const button of content.querySelectorAll('[data-object-view]'))button.onclick=()=>{
    objectView=button.dataset.objectView;localStorage.setItem('pcscope.objectView',objectView);
    for(const item of content.querySelectorAll('[data-object-view]'))item.setAttribute('aria-pressed',String(item===button));
    schedule(0);
  };
  content.querySelector('#model-search').oninput=e=>{modelFilters.search=e.target.value;schedule();};
  for(const key of ['category','type_id','lifecycle','sort'])content.querySelector('#model-'+(key==='type_id'?'type':key)).onchange=e=>{modelFilters[key]=e.target.value;if(key==='category')modelFilters.type_id='';options();schedule(0);};
  content.querySelector('#model-clear').onclick=()=>{Object.assign(modelFilters,{search:'',category:'',type_id:'',lifecycle:'',sort:'name_asc'});content.querySelector('#model-search').value='';content.querySelector('#model-lifecycle').value='';content.querySelector('#model-sort').value='name_asc';options();schedule(0);};
  state.reloadPage=()=>{reset();return refresh();};
  try{catalog=(await api('query',{request:{query:'type_catalog'}})).data||[];}catch(error){catalog=[];notice(errorText(error),error);}
  options();await refresh();
}
// S24: comparison, from the model's own observations of one instance.
//
// The old page compared two snapshot payloads of the same source. This compares one object's
// observation versions: what a field was, what it became, and the interval the change was in
// effect over. A field one observation did not carry is shown as an incomplete pair, never as a
// removal -- a source that says nothing about a field has not said the field is gone.
const compareFilters={search:'',type_id:''};
async function renderCompare(){
  content.innerHTML=head('状态对比','查看同一对象前后的变化。没有读到的信息不会当作删除。')+`<section class="panel"><div class="object-filters">
    <label class="object-search">查找对象<input id="compare-search" type="search" maxlength="1024" placeholder="名称或标识" value="${esc(compareFilters.search)}"></label>
    <label>类型<select id="compare-type"><option value="">全部类型</option></select></label>
    <label>对象<select id="compare-instance"><option value="">选择一个对象</option></select></label></div><p id="compare-picker-note" class="muted"></p></section><div id="compare-results" aria-live="polite"><div class="empty">选择一个对象后显示它的观察版本对比。</div></div><div id="compare-more"></div>`;
  const results=content.querySelector('#compare-results'),picker=content.querySelector('#compare-instance');
  let catalog=[],requestId=0,timer,selectedAlias='',comparisonDetail=null;
  const historyPager=PCScopeAutoPage.mount(content.querySelector('#compare-more'),{
    active:()=>!!selectedAlias,
    query:async cursor=>{
      const instance=selectedAlias;
      if(cursor)return api('query',{request:{query:'instance_actions',instance,cursor,limit:200}});
      const [detail,actions]=await Promise.all([api('query',{request:{query:'instance_detail',instance}}),api('query',{request:{query:'instance_actions',instance,limit:200}})]);
      comparisonDetail=detail.status==='empty'?null:detail.data;return actions;
    },
    onStart:({append})=>{if(!append){results.setAttribute('aria-busy','true');results.innerHTML='<div class="empty">正在读取前后变化…</div>';}},
    apply:(response,{append})=>{
      const html=PCScopeModelView.comparison(comparisonDetail,response);
      if(!append)results.innerHTML=html;
      else if(!PCScopeModelView.isEmpty(response)){const template=document.createElement('template');template.innerHTML=html;const current=results.querySelector('tbody'),next=template.content.querySelector('tbody');if(current&&next)current.append(...next.children);else results.innerHTML=html;}
      results.setAttribute('aria-busy','false');
    },
    onError:(error,{append})=>{results.setAttribute('aria-busy','false');if(!append)results.innerHTML=errorHtml(error);}
  });
  const options=()=>{
    content.querySelector('#compare-type').innerHTML='<option value="">全部类型</option>'+catalog.map(t=>`<option value="${esc(t.type_id)}" ${t.type_id===compareFilters.type_id?'selected':''}>${esc(PCScopeObjectNames.typeName(t))}</option>`).join('');
  };
  const load=async()=>{
    const id=++requestId;
    selectedAlias='';historyPager.pause();
    const body={query:'instance_list',limit:200,sort:'name_asc',include_counts:false};
    if(compareFilters.search)body.search=compareFilters.search;
    if(compareFilters.type_id)body.type_id=compareFilters.type_id;
    const response=await api('query',{request:body});
    if(id!==requestId)return;
    picker.innerHTML=PCScopeModelView.instanceOptions(response);
    content.querySelector('#compare-picker-note').textContent=response.page?.has_more?'还有其他对象，可搜索名称或按类型查找。':'';
    results.innerHTML=PCScopeModelView.isEmpty(response)?'<div class="empty">没有匹配的对象。</div>':'<div class="empty">选择一个对象后显示它的观察版本对比。</div>';
  };
  const show=async alias=>{
    selectedAlias=alias;
    if(!alias){historyPager.pause();results.innerHTML='<div class="empty">选择一个对象后显示它的观察版本对比。</div>';return;}
    return historyPager.reset();
  };
  content.querySelector('#compare-search').oninput=e=>{compareFilters.search=e.target.value;clearTimeout(timer);timer=setTimeout(load,250);};
  content.querySelector('#compare-type').onchange=e=>{compareFilters.type_id=e.target.value;load();};
  picker.onchange=e=>show(e.target.value);
  state.reloadPage=()=>load();
  try{catalog=(await api('query',{request:{query:'type_catalog'}})).data||[];}catch(error){catalog=[];notice(errorText(error),error);}
  options();await load();
}

function mountModuleFileHistory(){
  content.insertAdjacentHTML('beforeend','<details class="panel status-panel" id="module-file-history"><summary>程序模块文件检查</summary><p>查看已加载组件对应文件的检查历史，包括缺失、内容和权限变化。按检查时间排列，独立于上方变化筛选。</p><p><button data-module-objects="runtime.module_instance">查看已加载模块</button> <button data-module-objects="file.watched_file">查看监控文件</button></p><button id="module-files-refresh">刷新检查记录</button><div id="module-files"></div><div id="module-files-more"></div><p id="module-files-feedback" role="status"></p></details>');
  const section=content.querySelector('#module-file-history'),files=section.querySelector('#module-files'),more=section.querySelector('#module-files-more'),refresh=section.querySelector('#module-files-refresh'),feedback=section.querySelector('#module-files-feedback');
  let loaded=false;
  const pager=PCScopeAutoPage.mount(more,{
    active:()=>section.open,
    query:cursor=>api('query',{request:{query:'source_readings',source:'pcscope.execution.module-file-state',cursor,limit:100}}),
    onStart:()=>{refresh.disabled=true;feedback.textContent='正在读取检查记录…';},
    apply:(response,{append})=>{
      const html=PCScopeModelView.moduleFiles(response);
      if(append){if(!PCScopeModelView.isEmpty(response))files.insertAdjacentHTML('beforeend',html);}else files.innerHTML=html;
      loaded=true;feedback.textContent='';refresh.disabled=false;
    },
    onError:error=>{feedback.textContent=errorText(error);refresh.disabled=false;}
  });
  section.addEventListener('toggle',()=>{if(section.open){if(!loaded)pager.reset();else pager.resume();}});
  refresh.onclick=()=>pager.reset();
  section.querySelectorAll('[data-module-objects]').forEach(button=>button.onclick=()=>{
    Object.assign(modelFilters,{search:'',category:'',type_id:button.dataset.moduleObjects,lifecycle:'',sort:'seen_desc'});modelWalk={cursor:null,watermark:null};state.page='objects';render();
  });
}

let renderRevision=0;
async function render(){
  state.disposePage?.();state.disposePage=null;
  PCScopeAutoPage.disposeWithin(content);
  const revision=++renderRevision;
  state.reloadPage=null;
  const page=state.page;document.querySelectorAll('[data-page]').forEach(b=>b.setAttribute('aria-current',b.dataset.page===page?'page':'false'));
  content.innerHTML='<div class="empty">正在读取本机历史…</div>';
  try{
    if(page==='timeline'){
      await renderTimeline();
    }else if(page==='overview'){
      // S25: the overview answers from the model, and keeps the counts apart. An object, a source
      // record, an action and a piece of evidence are four things counted four ways; adding them
      // into one number would produce a number that means nothing.
      const [types,instances,actions,coverage]=await Promise.all([
        api('query',{request:{query:'type_catalog'}}),
        api('query',{request:{query:'instance_list',sort:'seen_desc',limit:1}}),
        api('query',{request:{query:'action_timeline',limit:8}}),
        api('query',{request:{query:'source_coverage'}}),
      ]);if(state.page!==page||revision!==renderRevision)return;
      content.innerHTML=head('系统总览','查看最近变化与监控状态。信息不足时，部分异常可能漏报。')+PCScopeModelView.overview(types,instances,actions,coverage);
    }else if(page==='objects'){
      await renderObjects();
    }else if(page==='compare'){
      await renderCompare();
    }else if(page==='plugins'){
      await renderPluginManager();
    }else if(page==='health'){
      // S25: the health page reads coverage and projection from the model.
      //
      // Two failure kinds are kept apart on purpose. A source that could not be read is a
      // permission or availability problem; material that arrived and could not be turned into
      // objects is an adapter that does not understand what it was given. One number for both
      // would tell the reader nothing about which to fix.
      content.innerHTML=head('采集健康','查看哪些信息已读取、哪些可能遗漏，以及对异常判断的影响。')+`<section class="panel status-panel"><h2>监控覆盖</h2><div id="health-coverage">正在读取…</div></section><section class="panel status-panel"><h2>记录处理</h2><div id="health-projection">正在读取…</div></section>`;
      const loadHealth=async(selector,query,format)=>{
        const target=content.querySelector(selector);if(!target?.isConnected)return;
        target.textContent='正在读取…';
        try{const response=await api('query',{request:{query}});if(target.isConnected)target.innerHTML=format(response);}
        catch(error){if(target.isConnected){target.innerHTML=`${errorHtml(error)}<button>重试</button>`;target.querySelector('button').onclick=()=>loadHealth(selector,query,format);}}
      };
      content.insertAdjacentHTML('beforeend','<section class="panel status-panel"><h2>资源与监控范围</h2><label>来源 <select id="reading-source"><option value="">全部</option><option value="pcscope.execution.module-scan-status">程序模块监控情况</option><option value="pcscope.execution.resources">资源采样</option><option value="pcscope.collector.scoped-configuration">采集范围配置</option><option value="pcscope.collector.event-channel-coverage">日志可用情况</option><option value="pcscope.usn.watch-list">文件监控名单</option></select></label><div id="source-readings"></div><div id="readings-next"></div></section>');
      const readingPanel=content.querySelector('#source-readings'),readingSelect=content.querySelector('#reading-source'),readingNext=content.querySelector('#readings-next');
      const readingsPager=PCScopeAutoPage.mount(readingNext,{
        query:cursor=>api('query',{request:{query:'source_readings',source:readingSelect.value||null,cursor,limit:100}}),
        onStart:({append})=>{if(!append)readingPanel.textContent='正在读取…';},
        apply:(response,{append})=>{
          if(!append)readingPanel.innerHTML=PCScopeModelView.sourceReadings(response);
          else if(!PCScopeModelView.isEmpty(response)){const records=readingPanel.querySelector('.source-reading-records');if(records)records.insertAdjacentHTML('beforeend',PCScopeModelView.sourceReadings(response,{recordsOnly:true}));}
        },
        onError:(error,{append})=>{if(!append)readingPanel.innerHTML=errorHtml(error);}
      });
      const loadReadings=()=>readingsPager.reset();
      readingSelect.onchange=loadReadings;
      await Promise.all([loadHealth('#health-coverage','source_coverage',PCScopeModelView.coverage),loadHealth('#health-projection','projection_status',PCScopeModelView.projection),loadReadings()]);
    }else if(page==='settings'){
      content.innerHTML=head('系统配置','管理界面、历史存储、采集权限与隐私显示。')+'<section id="launch-settings" class="panel"></section><div id="settings-content">正在读取监控设置…</div>';
      PCScopeLaunchSettings.mount(content.querySelector('#launch-settings'));
      const settingsContent=content.querySelector('#settings-content');
      PCScopeSettings.mount(settingsContent,{api,esc,localTime,notice,errorText,getTheme:()=>state.theme,setTheme:value=>{state.theme=value;localStorage.setItem('pcscope.theme',value);theme();},onPrivacy:()=>{state.events=[];state.selected=null;}});

    }
  }catch(error){if(state.page===page&&revision===renderRevision){content.innerHTML=head(pages.find(p=>p[0]===page)[2],'')+`<section class="panel empty">${errorHtml(error)}<br><br><button id="page-retry">重新读取</button></section>`;content.querySelector('#page-retry').onclick=()=>render();}}
}
function modelRefreshKey(status){
  // Older backends have no revision: committed material and projection progress are a
  // compatibility signal. A new backend reports actual model/display changes directly.
  return status?.read_model_revision??JSON.stringify([status?.pid,status?.counts?.records,status?.counts?.snapshots,status?.projection_worker?.at]);
}
async function refreshStatus(automatic=false){
  const epoch=state.refreshEpoch;const valid=()=>!automatic||(state.autoRefresh&&epoch===state.refreshEpoch);
  const operation=document.querySelector('#operation'),progress=document.querySelector('#progress');
  try{
    const status=await api('status');if(!valid())return false;state.status=status;issues.syncStatus(status);
    const text=status.collecting?'正在采集':status.projection_worker?.display?.pending===true?'正在更新记录':'监控中';if(operation.textContent!==text)operation.textContent=text;
    const last=status.last_collection;progress.hidden=!status.collecting;
    if(status.collecting&&last?.total)progress.value=100*last.completed/last.total;else if(status.collecting)progress.removeAttribute('value');else progress.value=0;
    return true;
  }catch(error){
    if(!valid())return false;state.status=null;operation.textContent='连接中断';progress.hidden=true;progress.value=0;
    connectionNotice('监控信息暂时无法更新，正在尝试恢复连接。',error);return false;
  }
}
function connectionNotice(text,detail){
  if(!text){issues.remove('backend');return;}
  issues.set('backend',{title:'后台连接中断',impact:text,detail:detail?.message||(detail?String(detail):''),action:'connect',actionLabel:'重新连接',busy:recovery.pending});
}
const recovery=PCScopeBackendConnection.create({start:async(manual)=>{
  connectionNotice('正在恢复后台连接，监控信息暂时无法更新…');
  issues.setBusy('backend',true);
  await invoke('start_service',{manual});
  if(!await refreshStatus())throw new Error('后台尚未就绪。');
  return true;
}});
let connecting=null;
function connect(manual=true){
  if(connecting)return connecting;
  connecting=(async()=>{
    try{
      if(!await recovery.run(manual)){connectionNotice(recovery.paused?'后台未能恢复，监控信息暂时无法更新。请重试。':'后台暂未就绪，将自动重试。已有记录仍保留。');return;}
      connectionNotice('');await render();if(await PCScopeMonitoring.startup(true)!==false)state.permissionBackendPid=state.status?.pid;
    }catch(error){connectionNotice(recovery.paused?'后台未能恢复，监控信息暂时无法更新。请重试。':'后台暂未就绪，将自动重试。已有记录仍保留。',error);}
    finally{connecting=null;issues.setBusy('backend',false);}
  })();
  return connecting;
}
async function startupDesktop(){
  await PCScopeView.ready;
  if(!await refreshStatus()){await connect(false);return;}
  recovery.connected();await render();if(await PCScopeMonitoring.startup()!==false)state.permissionBackendPid=state.status?.pid;
}
async function collect(){
  const button=content.querySelector('#collect'),feedback=content.querySelector('#collect-feedback');
  if(button?.disabled)return;if(button)button.disabled=true;
  if(feedback)feedback.textContent='正在提交…';
  try{await api('collect');await refreshStatus();if(feedback?.isConnected){feedback.textContent='已提交采集';setTimeout(()=>{if(feedback.isConnected&&feedback.textContent==='已提交采集')feedback.textContent='';},4000);}}
  catch(error){if(feedback?.isConnected)feedback.innerHTML=errorHtml(error);}
  finally{if(button?.isConnected)button.disabled=false;}
}
function toggleTheme(){state.theme=state.theme==='dark'?'light':'dark';localStorage.setItem('pcscope.theme',state.theme);theme();}
document.querySelector('#theme').onclick=toggleTheme;theme();
document.querySelector('#navigation').innerHTML=pages.map(([id,icon,title])=>`<button data-page="${id}"><span class="nav-icon">${icon}</span>${title}</button>`).join('');
document.querySelectorAll('[data-page]').forEach(b=>b.onclick=()=>{state.page=b.dataset.page;notice('');render();});
startupDesktop().catch(e=>issues.set('startup',{title:'页面未能打开',impact:'监控界面暂时不可用。',detail:e?.message||String(e),action:'startup',actionLabel:'重新读取'}));
document.querySelector('#auto-refresh').onclick=()=>{
  state.autoRefresh=!state.autoRefresh;state.refreshEpoch++;
  if(state.autoRefresh){state.pendingPageRefresh=true;state.lastAutomaticPageRefresh=0;}
  const button=document.querySelector('#auto-refresh');
  document.querySelector('#auto-refresh-state').textContent=state.autoRefresh?'开':'关';
  button.setAttribute('aria-checked',String(state.autoRefresh));
  document.querySelector('#auto-refresh').title='暂停刷新不会停止监控，连接与异常状态仍会更新';
};
document.querySelector('#load-latest').onclick=async()=>{
  if(state.loadingLatest)return;
  state.loadingLatest=true;state.refreshEpoch++;
  const button=document.querySelector('#load-latest'),page=state.page;
  button.disabled=true;button.textContent='载入中…';
  try{await PCScopeView.ready;if(!await refreshStatus()){await connect(true);return;}if(state.page===page){const counters=content.querySelector('.stats-legacy');if(counters)counters.outerHTML=stats();if(state.reloadPage)await state.reloadPage();else await render();state.pendingPageRefresh=false;state.lastAutomaticPageRefresh=Date.now();}}
  catch(e){notice(errorText(e),e);}
  finally{state.loadingLatest=false;button.disabled=false;button.textContent='载入最新';}
};
setInterval(async()=>{
  // Pausing page refresh does not stop connection recovery. Skip permission switches so a
  // temporary handover gap never starts a competing ordinary daemon.
  if(state.refreshPolling||state.loadingLatest||connecting||PCScopeMonitoring.switching)return;
  state.refreshPolling=true;const epoch=state.refreshEpoch,previous=state.status,was=previous?.collecting,previouslyConnected=Boolean(previous);
  try{
    const connected=await refreshStatus();
    if(!connected){recovery.disconnected();await connect(false);return;}
    recovery.connected();connectionNotice('');
    if(!previouslyConnected||state.permissionBackendPid!==state.status?.pid){await render();if(await PCScopeMonitoring.startup(true)!==false)state.permissionBackendPid=state.status?.pid;return;}
    if(!state.autoRefresh||epoch!==state.refreshEpoch)return;
    if(modelRefreshKey(previous)!==modelRefreshKey(state.status)||(was&&!state.status?.collecting))state.pendingPageRefresh=true;
    // Projection can commit new actions during a long collection, or after collection ends.
    // Keep a dirty flag across throttled ticks; never start overlapping page requests.
    if(state.pendingPageRefresh&&['overview','timeline','health'].includes(state.page)
      &&Date.now()-(state.lastAutomaticPageRefresh||0)>=15000
      &&!(state.page==='timeline'&&content.querySelector('#timeline-results[aria-busy=true]'))){
      if(state.reloadPage)await state.reloadPage();else await render();
      state.pendingPageRefresh=false;state.lastAutomaticPageRefresh=Date.now();
    }
  }finally{state.refreshPolling=false;}
},3000);




// S24: the timeline reads the model's action query rather than the old record list.
//
// What it shows is what happened, newest first, across objects. An action that belongs to no object
// is shown as such with the reason it could not be attributed, rather than filed under whichever
// object was nearest; and an action whose time is a fallback says so, because an interval's end is
// a bound on when a change took effect and not the moment it happened.
const timelineFilters={action:'',category:'',type_id:'',period:'',unassigned:true,routine:false};
let timelineCollapsed=localStorage.getItem('pcscope.timelineCollapsed')==='true';
PCScopeTimelineTime.setMinutes(localStorage.getItem('pcscope.timelineGroupMinutes'));
let timelineGroupsPerLoad=PCScopeTimelinePages.normalise(localStorage.getItem('pcscope.timelineGroupsPerLoad'));
let timelineWalk={cursor:null,watermark:null};
async function renderTimeline(){
  content.innerHTML=head('时间线','按时间查看系统变化。','<div class="operation-feedback"><button type="button" id="timeline-fold-all">全部折叠</button><button id="collect" class="primary">立即采集</button><span id="collect-feedback" role="status"></span></div>')+`<section class="panel"><div class="object-filters">
    <label class="object-search">发生了什么<select id="timeline-action"><option value="">全部变化</option>${PCScopeReadableEvents.options().map(action=>`<option value="${esc(action.id)}" ${action.id===timelineFilters.action?'selected':''}>${esc(action.label)}</option>`).join('')}</select></label>
    <label>类别<select id="timeline-category"><option value="">全部类别</option></select></label>
    <label>类型<select id="timeline-type"><option value="">全部类型</option></select></label>
    <label>时间范围<select id="timeline-period">${[['','全部时间'],['1','最近 24 小时'],['7','最近 7 天'],['30','最近 30 天']].map(([v,n])=>`<option value="${v}" ${v===timelineFilters.period?'selected':''}>${n}</option>`).join('')}</select></label>
    <label>分组间隔<select id="timeline-group-minutes">${PCScopeTimelineTime.options.map(n=>`<option value="${n}" ${n===PCScopeTimelineTime.getMinutes()?'selected':''}>${n} 分钟</option>`).join('')}</select></label>
    <label>每次加载<select id="timeline-groups-per-load">${PCScopeTimelinePages.options.map(n=>`<option value="${n}" ${n===timelineGroupsPerLoad?'selected':''}>${n} 个时间组</option>`).join('')}</select></label>
    <label>显示内容<select id="timeline-routine"><option value="false" ${timelineFilters.routine?'':'selected'}>主要变化</option><option value="true" ${timelineFilters.routine?'selected':''}>含日常更新</option></select></label>
    <label class="object-search">未对应到具体对象的日志<select id="timeline-unassigned"><option value="true" ${timelineFilters.unassigned?'selected':''}>也显示</option><option value="false" ${timelineFilters.unassigned?'':'selected'}>暂时隐藏</option></select></label>
    <button id="timeline-clear">清空筛选</button></div><p id="timeline-loading" class="muted" role="status" hidden></p><div id="timeline-results" aria-live="polite"></div><div id="timeline-more"></div></section>`;
  const results=content.querySelector('#timeline-results'),more=content.querySelector('#timeline-more');
  const windowed=PCScopeTimelineWindow.mount(results);
  const groupControls=PCScopeTimelineGroups.mount(results,{collapsed:timelineCollapsed,track:windowed.track});
  const foldAll=content.querySelector('#timeline-fold-all');
  const showFoldMode=()=>{foldAll.textContent=timelineCollapsed?'全部展开':'全部折叠';foldAll.setAttribute('aria-controls',results.id);};
  showFoldMode();
  let pagingReady=false;
  const scrolling=content.closest?.('main')||content;
  const pagingInput=new AbortController();
  results.addEventListener('click',event=>{if(event.target.closest('[data-timeline-toggle],[data-timeline-time-toggle],[data-timeline-category-toggle],[data-timeline-show-all],[data-timeline-invert]'))pagingReady=false;},{capture:true,signal:pagingInput.signal});
  state.disposePage=()=>{clearTimeout(timer);++requestId;pagingInput.abort();groupControls.dispose();windowed.dispose();};
  mountModuleFileHistory();
  let catalog=[],requestId=0,timer,reader=null,renderedRows=0;
  const loading=content.querySelector('#timeline-loading');
  const options=()=>{
    content.querySelector('#timeline-category').innerHTML='<option value="">全部类别</option>'+[...new Set(catalog.map(t=>t.category))].map(c=>`<option value="${esc(c)}" ${c===timelineFilters.category?'selected':''}>${esc(categoryNames[c]||c)}</option>`).join('');
    content.querySelector('#timeline-type').innerHTML='<option value="">全部类型</option>'+catalog.filter(t=>!timelineFilters.category||t.category===timelineFilters.category).map(t=>`<option value="${esc(t.type_id)}" ${t.type_id===timelineFilters.type_id?'selected':''}>${esc(PCScopeObjectNames.typeName(t))}</option>`).join('');
  };
  const request=()=>{
    const body={query:'action_timeline',limit:400,cards_only:true,include_unassigned:timelineFilters.unassigned,include_routine:timelineFilters.routine};
    if(timelineFilters.action)body.action_id=timelineFilters.action;
    if(timelineFilters.category)body.category=timelineFilters.category;
    if(timelineFilters.type_id)body.type_id=timelineFilters.type_id;
    if(timelineFilters.period)body.since=new Date(Date.now()-Number(timelineFilters.period)*86400000).toISOString();

    return body;
  };
  results.addEventListener('toggle',async event=>{
    const details=event.target;
    if(!details.matches?.('details[data-action-detail]')||!details.open||details.dataset.loaded||details.dataset.loading)return;
    details.dataset.loading='true';
    const body=details.querySelector('[data-action-detail-body]');
    body.innerHTML='<p class="muted">正在读取记录…</p>';
    try{
      const response=await api('query',{request:{query:'action_detail',action_row:Number(details.dataset.actionDetail)}});
      if(!details.isConnected)return;
      if(response.status!=='loaded'||!response.data)throw Error(PCScopeModelView.messageOf(response,'该记录的详情已不可用。'));
      body.innerHTML=PCScopeModelView.timelineRecord(response.data);details.dataset.loaded='true';
    }catch(error){if(details.isConnected)body.innerHTML='<p class="muted">'+errorHtml(error)+' 收起后重新展开可重试。</p>';}
    finally{delete details.dataset.loading;}
  },{capture:true,signal:pagingInput.signal});
  results.addEventListener('click',async event=>{
      const button=event.target.closest('[data-object-alias]');if(!button||!results.contains(button))return;
      const entry=button.closest('.timeline-entry');
      let panel=entry.querySelector('.timeline-object-detail');
      if(panel){panel.remove();button.setAttribute('aria-expanded','false');return;}
      panel=document.createElement('div');panel.className='timeline-object-detail';panel.innerHTML='<div class="empty">正在读取对象信息…</div>';
      entry.append(panel);button.setAttribute('aria-expanded','true');
      try{
        const response=await api('query',{request:{query:'instance_detail',instance:button.dataset.objectAlias}});
        if(!panel.isConnected)return;
        panel.innerHTML=response.status==='empty'?`<div class="empty">${esc(PCScopeModelView.messageOf(response,'没有该对象的详情'))}</div>`:PCScopeModelView.detail(response);
      }catch(error){if(panel.isConnected)panel.innerHTML=`<div class="empty">${errorHtml(error)}</div>`;}
  },{signal:pagingInput.signal});
  const createReader=()=>{
    const revision=requestId,body=request(),minutes=PCScopeTimelineTime.getMinutes();
    const current=()=>!pagingInput.signal.aborted&&results.isConnected&&revision===requestId;
    return PCScopeTimelinePages.create({groups:timelineGroupsPerLoad,active:current,
      keyOf:row=>PCScopeTimelineTime.moment(row,minutes).key,
      query:cursor=>api('query',{request:{...body,...(cursor?{cursor}:{})}}),
      progress:({groups,target,rows})=>{if(current()){loading.hidden=false;loading.textContent='已读取 '+rows+' 条，正在补齐第 '+groups+' / '+target+' 组…';}},
      emit:async response=>{
        let emitted=0,frameStarted=performance.now(),first=true;
        for(const {html} of PCScopeModelView.timelineBatches(response)){
          if(!current())return;
          if(renderedRows)groupControls.append(html);else {windowed.clear();results.innerHTML=html;groupControls.sync();}
          const added=Math.min(50,response.data.length-emitted);renderedRows+=added;emitted+=added;groupControls.markBoundary(true);
          // Paint the first cards promptly, then yield by work time instead of paying
          // an entire display frame for every tiny batch on a fast machine.
          if(first||performance.now()-frameStarted>=8){
            await new Promise(resolve=>requestAnimationFrame(resolve));frameStarted=performance.now();first=false;
          }
        }
      }
    });
  };
  const pager=PCScopeAutoPage.mount(more,{
    // Folding can bring the sentinel into view. Wait for scrolling intent before
    // continuing, rather than walking history just because the layout got shorter.
    canAutoLoad:()=>pagingReady,
    query:()=>{reader??=createReader();return reader.read();},
    onStart:()=>{results.setAttribute('aria-busy','true');loading.hidden=false;loading.textContent='正在读取 '+timelineGroupsPerLoad+' 个时间组…';if(!renderedRows)results.innerHTML='<div class="empty">正在读取变化记录…</div>';},
    apply:async(response,{append})=>{
      timelineWalk.watermark=response.page?.watermark||null;
      if(!renderedRows){results.innerHTML=PCScopeModelView.timeline(response);groupControls.sync();}
      // Every logical page finishes on a bucket boundary, even when more history exists.
      groupControls.markBoundary(false);loading.hidden=true;results.setAttribute('aria-busy','false');
      // Completing a short batch can expose the sentinel. Require continued scroll intent.
      pagingReady=false;
    },
    onError:error=>{results.setAttribute('aria-busy','false');loading.hidden=true;if(!renderedRows)results.innerHTML=`<div class="empty">${errorHtml(error)}</div>`;}
  });
  const refresh=()=>{++requestId;reader=null;renderedRows=0;pagingReady=false;windowed.clear();return pager.reset();};
  const allowPaging=event=>{
    if(event.type==='keydown'&&(!['ArrowDown','PageDown','End',' '].includes(event.key)||event.target.closest('input,select,textarea,[contenteditable]')||event.key===' '&&event.target.closest('button,summary')))return;
    if(event.type==='wheel'&&event.deltaY<=0)return;
    if(event.type==='pointerdown'&&(event.target!==scrolling||event.offsetX<scrolling.clientWidth))return;
    pagingReady=true;pager.resume();
  };
  for(const event of ['wheel','touchmove','keydown','pointerdown'])scrolling.addEventListener(event,allowPaging,{passive:true,signal:pagingInput.signal});
  foldAll.onclick=()=>{
    timelineCollapsed=!timelineCollapsed;pagingReady=false;
    localStorage.setItem('pcscope.timelineCollapsed',String(timelineCollapsed));
    groupControls.setCollapsed(timelineCollapsed);showFoldMode();
  };
  const reset=()=>{timelineWalk={cursor:null,watermark:null};};
  const schedule=(delay=250)=>{clearTimeout(timer);++requestId;pager.pause();reset();timer=setTimeout(()=>refresh(),delay);};
  content.querySelector('#timeline-action').oninput=e=>{timelineFilters.action=e.target.value;schedule();};
  content.querySelector('#timeline-groups-per-load').onchange=e=>{timelineGroupsPerLoad=PCScopeTimelinePages.normalise(e.target.value);localStorage.setItem('pcscope.timelineGroupsPerLoad',String(timelineGroupsPerLoad));schedule(0);};
  content.querySelector('#timeline-group-minutes').onchange=e=>{PCScopeTimelineTime.setMinutes(e.target.value);localStorage.setItem('pcscope.timelineGroupMinutes',String(PCScopeTimelineTime.getMinutes()));schedule(0);};
  content.querySelector('#timeline-period').onchange=e=>{timelineFilters.period=e.target.value;reset();schedule(0);};
  content.querySelector('#timeline-unassigned').onchange=e=>{timelineFilters.unassigned=e.target.value==='true';reset();schedule(0);};
  content.querySelector('#timeline-routine').onchange=e=>{timelineFilters.routine=e.target.value==='true';reset();schedule(0);};
  for(const key of ['category','type_id'])content.querySelector('#timeline-'+(key==='type_id'?'type':key)).onchange=e=>{timelineFilters[key]=e.target.value;if(key==='category')timelineFilters.type_id='';options();schedule(0);};
  content.querySelector('#timeline-clear').onclick=()=>{Object.assign(timelineFilters,{action:'',category:'',type_id:'',period:'',unassigned:true,routine:false});content.querySelector('#timeline-action').value='';content.querySelector('#timeline-period').value='';content.querySelector('#timeline-unassigned').value='true';content.querySelector('#timeline-routine').value='false';options();schedule(0);};
  content.querySelector('#collect').onclick=collect;
  state.reloadPage=()=>{reset();return refresh();};
  try{catalog=(await api('query',{request:{query:'type_catalog'}})).data||[];}catch(error){catalog=[];notice(errorText(error),error);}
  options();await refresh();
}
