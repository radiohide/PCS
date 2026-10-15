'use strict';
// Renders the object model's own queries (S23).
//
// The model is not the old source-and-object list: an object has a type and an identity, its facts
// each carry the evidence they came from, and what happened to it is a list of actions. This module
// draws those, and only those — every value it shows comes from a field the query contract defines.
//
// `PATHS` lists every contract path this module reads. scripts/check-model-view.cjs verifies each
// one against the fixed response set, so a renamed field fails there and names the path instead of
// rendering a blank cell on a page nobody tested.
const PCScopeModelView=(()=>{
  const objectNames=typeof PCScopeObjectNames!=='undefined'?PCScopeObjectNames:require('./object-names.js');
  const presentation=objectNames.presentation;
  const timelineTime=typeof PCScopeTimelineTime!=='undefined'?PCScopeTimelineTime:require('./timeline-time.js');
  const readableEvents=typeof PCScopeReadableEvents!=='undefined'?PCScopeReadableEvents:require('./readable-events.js');
  const PATHS=[
    'data[].progress','data[].source_record','data[].item_id','data[].destination','data[].payload','data[].evidence.id',
    'data[].instance_id','data[].alias','data[].type_id','data[].category','data[].type_name',
    'data[].display_name','data[].scope','data[].identity_status','data[].lifecycle',
    'data[].key_facts[].field_id','data[].key_facts[].label','data[].key_facts[].value',
    'data[].key_facts[].status','data[].changed_at','data[].seen_at',
    'data[].coverage[].source','data[].coverage[].last_result','data[].coverage[].history_state','data[].coverage[].last_attempt_at',
    'data.summary.coverage',
    'data.summary.display_name','data.summary.type_name','data.summary.identity_status','data.summary.lifecycle',
    'data.summary.key_facts[].label','data.summary.key_facts[].value',
    'data.fields[].field_id','data.fields[].label','data.fields[].kind','data.fields[].value',
    'data.fields[].status','data.fields[].observed_at','data.fields[].derivation','data.fields[].conflicts[].value',
    'data.actions_preview[].action_id','data.actions_preview[].title','data.actions_preview[].result',
    'data.actions_preview[].precision','data.actions_preview[].interval_from','data.actions_preview[].interval_until',
    'data.actions_preview[].occurred_at','data.actions_preview[].time_derived','data.actions_preview[].field_id',
    'data.bindings[].source','data.bindings[].item_id','data.bindings[].valid_from',
    'page.has_more','page.next_cursor','page.watermark','counts.instances','counts.actions',
    'counts.source_records','counts.evidence','status',
    'data[].action_id','data[].title','data[].result','data[].phase','data[].precision',
    'data[].occurred_at','data[].interval_from','data[].interval_until','data[].observed_at',
    'data[].field_id','data[].before','data[].after','data[].time_derived',
    'data[].unassigned_reason','data[].instance_id',
    'data[].source','data[].scope','data[].configured','data[].last_result','data[].last_success_at',
    'data[].last_attempt_at','data[].history_state','data[].projection.pending','data[].projection.failed',
    'data[].projection.done',
    'data[].projection','data[].rule_version','data[].schema_version','data[].pending','data[].failed',
    'data[].last_error','data[].updated_at','catalog_version',
    'data.summary.alias','data[].type_id','data[].category','data[].name','data[].supplied_by_source',
  ];
  // Fields the contract defines and omits when they are empty, so no fixed sample carries them.
  // They are listed rather than dropped from PATHS: the renderer does read them, and a check that
  // silently skipped them would stop noticing if the contract renamed one.
  const OPTIONAL_PATHS=[
    ...['data[].record_basis','data.summary.record_basis'].map(field=>[field,'新版查询区分当前采样资料与仅有日志的记录；旧后台省略时不推测来源']),
    ['data.bindings[].valid_to','契约在绑定仍然开放时省略该字段；固定响应集里没有已关闭的绑定'],
    ...['data[].context.','data.actions_preview[].context.'].flatMap(prefix=>['type_id','type_name','display_name','key_facts','presentation','subject_label','subject_detail','subject_basis','field_label','logon_type','program_label','attribution_status','attribution_reason','classification_status','stored_instance_id','stored_action_id','identity_status'].map(field=>[prefix+field,'新版查询按动作证据提供上下文；旧后台及没有该类证据的记录省略'])),
    ...['data[].context.','data.actions_preview[].context.'].flatMap(prefix=>['history_quality','history_quality.status','history_quality.reason'].map(field=>[prefix+field,'新版查询明确标记旧映射需要复核的历史；字段省略时不从文字猜测旧映射问题'])),
    ...['data[].presentation.','data.summary.presentation.'].flatMap(prefix=>['name','name_status','description','basis','dictionary_id','dictionary_revision','dictionary_entry_id','match_basis','technical_key','references'].map(field=>[prefix+field,'新版查询提供名称依据及可选字典匹配详情；旧后台省略'])),
  ];
  // What a status means in words. A missing value is not the same as a value that could not be
  // read, and neither is the same as one that was never stated — the contract keeps them apart and
  // so does the page.
  const statusNames={known:'已知',not_provided:'来源未提供',not_applicable:'不适用',unreadable:'无法读取',conflicting:'存在冲突',masked:'已遮罩',evidence_pruned:'证据已清理'};
  const identityNames={confirmed:'对象已识别',provisional:'识别资料不完整',ambiguous:'记录归属不明确'};
  const coverageNames={success:'已读取',partial:'信息不全',denied:'权限不足',missing:'暂不可用',failed:'读取失败',collection_failed:'读取失败',never:'尚未检查'};
  const updateResults={0:'尚未开始',1:'执行中',2:'成功',3:'部分成功',4:'失败',5:'已中止'};
  const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const time=(value,fallback='未记录时间')=>{if(!value)return fallback;const date=new Date(value);return Number.isNaN(date.getTime())?String(value):date.toLocaleString('sv-SE',{hour12:false});};
  const label=(names,key,fallback)=>names[key]||fallback||key;
  const resultNames={succeeded:'成功',success:'成功',failed:'失败',unknown:'结果未知',observed:'已记录',partial:'部分完成',cancelled:'已取消'};
  const sourceName=source=>{
    const rows=typeof pluginSources!=='undefined'?Object.values(pluginSources).flat():[];
    return rows.find(row=>row.source===source)?.name||({'pcscope.execution.module-scan-status':'桌面进程监控','pcscope.execution.module-file-state':'模块文件监控','pcscope.execution.resources':'系统资源'})[source]||'其他采集项';
  };
  function coverageImpact(row){
    if(!row.configured)return '已暂停，后续变化可能漏记。';
    if(row.last_result==='never')return '尚未检查，暂不能判断。';
    if(row.last_result!=='success')return '信息不全，相关异常可能漏报。';
    if(Number(row.projection?.failed||0))return '部分记录未能显示，变化可能遗漏。';
    if(Number(row.projection?.pending||0)||row.history_state==='catching_up')return '正在补充信息，最新变化可能尚未显示。';
    return '已读取可用信息。';
  }

  function factFact(entry,row){
    const fact=entry.fact||entry;
    return `<li><span class="fact-label">${esc(entry.label||objectNames.fieldLabel(row,fact))}：</span><span class="fact-value">${esc(entry.text??objectNames.valueLabel(fact.value,fact.field_id,row,fact.status))}</span></li>`;
  }

  const accountRecord=row=>['account.user','account.right_assignment'].includes(row.type_id);
  function accountExplanation(summary,actions=[]){
    if(!accountRecord(summary))return '';
    const notes=[];
    if(summary.presentation?.record_status==='legacy_snapshot')notes.push('缺少启动时间，无法判断是否仍在运行。');
    if(summary.presentation?.record_status==='legacy_mixed_process')notes.push('多次运行记录被合并，暂不能准确对应到某次运行。');
    if(summary.record_basis==='event_only')notes.push('日志提到过此项，尚未取得可核对的当前资料。');
    else if(summary.identity_status==='provisional')notes.push('用于识别此项的资料还不完整，暂时无法准确关联相关记录。');
    else if(summary.identity_status==='ambiguous')notes.push('现有资料还不能确定这些记录是否属于同一项。');
    if(actions.some(action=>action.action_id==='windows.event'&&action.context?.classification_status==='unverified'&&action.context?.attribution_status==='mismatch'))notes.push('历史日志的类别与来源不符，已作为普通系统记录展示。');
    else if(actions.some(action=>action.context?.attribution_status==='mismatch'))notes.push('历史日志与这条记录的关联不一致，暂不能算作它的活动。');
    return notes.join('');
  }

  // Only a dated legacy failure without a scope can be superseded by a newer scoped attempt.
  // Different nonempty scopes may still be active users/partitions, so keep all their results.
  function objectCoverage(entries=[]){
    if(!entries.length)return '尚未检查';
    const groups=new Map();
    for(const entry of entries){const key=entry.source||'';if(!groups.has(key))groups.set(key,[]);groups.get(key).push(entry);}
    const states=[...groups.values()].flatMap(rows=>rows.flatMap(row=>{
      if(row.scope===''&&['collection_failed','failed'].includes(row.last_result)){
        const scoped=rows.filter(other=>typeof other.scope==='string'&&other.scope.trim());
        const attempt=Date.parse(row.last_attempt_at);
        if(scoped.some(other=>Date.parse(other.last_attempt_at)>attempt))return [];
        if(scoped.length&&(Number.isNaN(attempt)||scoped.some(other=>Number.isNaN(Date.parse(other.last_attempt_at)))))return ['unknown'];
      }
      return [row.last_result];
    }));
    for(const result of ['collection_failed','failed','denied','missing','partial'])if(states.includes(result))return coverageNames[result];
    if(states.every(result=>result==='success'))return coverageNames.success;
    if(states.every(result=>result==='never'))return coverageNames.never;
    return '采集结果不完整';
  }

  /// The list: one row per object, with the facts that identify it and what is known about how it
  /// was collected. `lifecycle` and `identity_status` are shown as words, never as codes.
  function treePage(response){
    return {total:response.counts?.instances??null,nodes:(response.data||[]).map(row=>({alias:row.alias,type:row.type_id,category:row.category,typeName:objectNames.typeName(row),name:objectNames.name(row),state:presentation.state(row)}))};
  }
  function list(response){
    const rows=response.data||[];
    if(!rows.length)return '<div class="empty">没有匹配的对象。可以调整筛选条件，或先让后台完成一次采集。</div>';
    const named=[],unnamed=[];rows.forEach((row,index)=>(objectNames.unnamedRegistration(row)?unnamed:named).push({row,index}));
    const table=items=>'<div class="object-table-wrap"><table class="table object-table"><thead><tr><th>对象</th><th>类型</th><th>最近状态</th><th>已采集信息</th><th>最近变化</th><th>最近检查</th><th>采集情况</th><th>详情</th></tr></thead><tbody>'+items.map(({row,index})=>{
      const facts=presentation.entries(row,[],{omitNames:true}).filter(entry=>!entry.state).slice(0,3).map(entry=>esc(entry.label)+'：'+esc(entry.text)).join('<br>');
      const dictionary=objectNames.dictionaryMatched(row),explanation=dictionary?esc(row.presentation.description||'这是系统保留的一项登记。'):esc(accountExplanation(row));
      const lifecycle=presentation.state(row);
      const coverage=esc(objectCoverage(row.coverage));
      return '<tr><td>'+esc(objectNames.name(row))+(dictionary?'<br><span class="badge">按登记识别</span>':'')+'</td><td>'+esc(objectNames.typeName(row))+'</td><td>'+esc(lifecycle)+(row.identity_status!=='confirmed'?'<br><small class="muted">'+esc(label(identityNames,row.identity_status,'识别资料不完整'))+'</small>':'')+'</td><td>'+([explanation,facts].filter(Boolean).join('<br>')||'—')+'</td><td>'+esc(time(row.changed_at,'尚无变化记录'))+'</td><td>'+esc(time(row.seen_at,'未记录检查时间'))+'</td><td>'+(coverage||'尚未检查')+'</td><td><button data-object="'+index+'" data-object-alias="'+esc(row.alias)+'" aria-expanded="false" aria-controls="object-detail-'+index+'">查看</button></td></tr>';
    }).join('')+'</tbody></table></div>';
    return (named.some(({row})=>objectNames.dictionaryMatched(row))?'<p class="notice">名称根据安装登记识别。</p>':'')+(named.length?table(named):'')+(unnamed.length?'<details class="unnamed-registrations"><summary>'+unnamed.length+' 条软件登记暂时没有可用名称</summary><p>这些是系统保留的安装登记，展开可查看原始信息。</p>'+table(unnamed)+'</details>':'');
  }

  /// One object in full: its facts with the evidence behind each, the actions observed on it, and
  /// the source records it is bound to.
  function detail(response){
    const data=response.data;
    if(!data||!data.summary)return '<div class="empty">没有该对象的详情。</div>';
    const summary=data.summary;
    const conflicts=field=>(field.conflicts||[]).map(candidate=>`<li>冲突候选：${esc(typeof candidate.value==='string'?candidate.value:JSON.stringify(candidate.value))}</li>`).join('');
    const fields=(data.fields||[]).map(field=>`<li><span class="fact-label">${esc(objectNames.fieldLabel(summary,field))}</span><span class="fact-value">${field.value===null||field.value===undefined?'—':esc(typeof field.value==='string'?field.value:JSON.stringify(field.value))}</span><span class="fact-status status-${esc(field.status)}">${esc(label(statusNames,field.status))}</span><br><small class="muted">${esc(field.kind||'')} · 观察于 ${esc(time(field.observed_at))} · ${esc(field.derivation||'')}</small>${conflicts(field)}</li>`).join('');
    // An action's time is marked when it is a fallback: showing an interval's bound as if it were
    // the moment something happened would claim a precision the source does not have.
    const renderAction=action=>{
      const when=action.time_derived?'发生于 '+time(action.interval_from)+' 至 '+time(action.interval_until):time(action.occurred_at||action.observed_at);
      const change=readableEvents.change(action);
      return '<li><span class="action-title">'+esc(readableEvents.title(action))+'</span><span class="event-result">'+esc(readableEvents.result(action))+'</span><p>'+esc(readableEvents.subject(action))+'</p><p class="timeline-type">类型：'+esc(readableEvents.typeLabel(action))+'</p><p>'+esc(change||readableEvents.impact(action))+'</p>'+(readableEvents.attribution(action)?'<p class="notice">'+esc(readableEvents.attribution(action))+'</p>':'')+'<small class="muted">'+esc(when)+(action.time_derived?'（发生在此期间）':'')+'</small><details><summary>原始变化记录</summary><pre>'+esc(JSON.stringify(action,null,2))+'</pre></details></li>';
    };
    const previews=data.actions_preview||[],uncertainActions=previews.filter(readableEvents.uncertainAssociation);
    const accountNote=accountExplanation(summary,previews);
    const mainFacts=presentation.entries(summary,data.fields||[],{omitNames:true});
    const actions=previews.filter(action=>!readableEvents.uncertainAssociation(action)).map(renderAction).join('');
    const bindings=(data.bindings||[]).map(binding=>`<li>${esc(binding.source)} · ${esc(binding.item_id)}<br><small class="muted">${esc(time(binding.valid_from))} → ${binding.valid_to?esc(time(binding.valid_to)):'现在'}</small></li>`).join('');
    return `<div class="model-detail">
      <h3>${esc(objectNames.name(summary,data.fields||[]))}</h3>${objectNames.dictionaryMatched(summary,data.fields||[])?'<span class="badge">按登记识别</span><p>'+esc(summary.presentation.description||'这是系统保留的一项登记。')+'</p><p class="muted">名称根据安装登记识别。</p>':objectNames.unnamedRegistration(summary,data.fields||[])?'<p class="notice">系统保留的安装登记，未取得软件名称。</p>':''}
      <p class="muted">${esc(objectNames.typeName(summary))} · ${esc(presentation.state(summary,data.fields||[]))}${summary.identity_status!=='confirmed'?` · ${esc(label(identityNames,summary.identity_status))}`:''}</p>
      ${accountNote?'<p class="notice">'+esc(accountNote)+'</p>':''}
      <h4>${presentation.lifecycle(summary)?'上次采集的信息':'主要信息'}</h4>${presentation.lifecycle(summary)?'<p class="muted">采集时间：'+esc(time(summary.seen_at))+'</p>':''}<ul class="fact-list">${mainFacts.map(fact=>factFact(fact,summary)).join('')||'<li class="muted">来源未提供更多资料</li>'}</ul>
      <h4>最近变化</h4><ul class="action-list">${actions||'<li class="muted">尚无该对象的变化记录</li>'}</ul>${uncertainActions.length?'<details><summary>'+uncertainActions.length+' 条历史日志未能对应到此对象</summary><p class="notice">这些日志保留在此供核对。</p><ul class="action-list">'+uncertainActions.map(renderAction).join('')+'</ul></details>':''}
      <details><summary>详细名称、全部属性与来源</summary><p>原始名称：${esc(summary.display_name||'未提供')}</p><p>内部标识：${esc(summary.alias)}</p><p>范围：${esc(summary.scope||'未提供')}</p>${summary.presentation?'<h4>名称说明与依据</h4><pre>'+esc(JSON.stringify(summary.presentation,null,2))+'</pre>':''}<ul class="fact-list">${fields||'<li class="muted">没有字段事实</li>'}</ul><h4>来源记录</h4><ul class="binding-list">${bindings||'<li class="muted">没有来源绑定</li>'}</ul><h4>全部采集记录</h4><pre>${esc(JSON.stringify(summary.coverage||[],null,2))}</pre></details>
    </div>`;
  }


  /// The timeline: what happened, newest first, across every object.
  ///
  /// An action that belongs to no object is shown as exactly that, with the reason it could not be
  /// attributed. Filing it under whichever object was nearest would be the one thing a timeline
  /// must not do — it would read as a fact about that object.
  ///
  /// `time_derived` decides how the time is worded. An interval says when the change was in effect
  /// between; only an exact time is shown as the moment something happened.
  function timelineChange(row){
    const display=readableEvents.card(row),field=display.field?'<div class="timeline-field">'+esc(display.field)+'</div>':'';
    let html;
    if(display.kind==='diff'){
      html=display.long?'<div class="timeline-value-stack"><div><span>之前</span><span>'+esc(display.before)+'</span></div><div class="timeline-value-new"><span>现在</span><span>'+esc(display.after)+'</span></div></div>':'<div class="timeline-value-pair"><span class="timeline-value-before">'+esc(display.before)+'</span><span class="timeline-value-arrow" aria-hidden="true">→</span><span class="timeline-value-after">'+esc(display.after)+'</span></div>';
    }else if(display.kind==='value')html='<div class="timeline-value-pair"><span class="timeline-value-after">'+esc(display.after)+'</span></div>';
    else html='<span class="timeline-event-label">'+esc(display.label)+'</span>';
    return '<div class="timeline-change timeline-tone-'+esc(display.tone)+'">'+field+html+'</div>'+(display.note?'<p class="timeline-impact">'+esc(display.note)+'</p>':'');
  }
  function timelineEntry(row,response,moment,index){
    const original={动作标识:row.action_id,原始标题:row.title,涉及对象:row.context,时间精度:row.precision,发生时间:row.occurred_at,区间开始:row.interval_from,区间结束:row.interval_until,记录时间:row.observed_at,阶段:row.phase,结果原值:row.result,属性标识:row.field_id,变化前:row.before,变化后:row.after,未关联原因:row.unassigned_reason,更新记录:row.source_record};
    const program=row.context?.program_label?'<p class="timeline-program">涉及程序：'+esc(objectNames.shortName(row.context.program_label))+'</p>':'';
    const attribution=readableEvents.attribution(row);
    return '<div class="timeline-branch" data-timeline-category="'+esc(readableEvents.typeLabel(row))+'" data-timeline-moment="'+esc(JSON.stringify(moment))+'" data-timeline-order="'+index+'"><article class="timeline-entry'+(!row.instance_id?' unattributed':'')+'"><div class="timeline-card-body"><h3 class="timeline-subject">'+esc(readableEvents.subject(row))+'</h3><p class="timeline-type"><span class="visually-hidden">类型：</span>'+esc(readableEvents.typeLabel(row))+'</p>'+program+timelineChange(row)+(attribution?'<p class="timeline-attribution">'+esc(attribution)+'</p>':'')+'</div><div class="timeline-card-actions">'+(row.instance_id&&!readableEvents.uncertainAssociation(row)?'<button data-object-alias="'+esc(instanceAlias(response,row))+'" class="timeline-object" aria-expanded="false">查看对象 <span aria-hidden="true">↗</span></button>':'')+'<details><summary>记录详情</summary><pre>'+esc(JSON.stringify(original,null,2))+'</pre></details></div></article></div>';
  }
  function timeline(response){
    const rows=response.data||[];
    if(!rows.length)return '<div class="empty">没有匹配的变化记录。可以调整筛选条件，或先完成一次采集。</div>';
    const days=new Map();
    for(const group of timelineTime.groupRows(rows)){
      if(!days.has(group.date))days.set(group.date,{date:group.date,rank:timelineTime.rank(group),groups:[]});
      days.get(group.date).groups.push(group);
    }
    return '<div class="timeline-list">'+[...days.values()].sort((a,b)=>b.rank-a.rank).map(day=>'<section class="timeline-day" data-timeline-day="'+esc(day.date)+'"><h2 class="timeline-day-label">'+esc(day.date)+'</h2>'+day.groups.sort((a,b)=>b.rank-a.rank).map(group=>{
      const range=timelineTime.range(group),{entries,...metadata}=group;
      const when=esc(range.start)+(range.end?'<span class="timeline-time-end"><span class="timeline-time-separator">至</span>'+esc(range.end)+'</span>':'');
      const precision=group.kind==='interval'?'区间内变化':group.kind==='recorded'?'记录时间':'';
      return '<section class="timeline-group" data-time-group="'+esc(group.key)+'" data-time-meta="'+esc(JSON.stringify(metadata))+'"><aside class="timeline-time-rail"><div class="timeline-when"><button class="timeline-group-toggle" data-timeline-toggle aria-expanded="true"><span class="timeline-time-heading">'+(precision?'<span class="timeline-precision">'+precision+'</span>':'<span class="timeline-precision">发生时间</span>')+'<span class="timeline-fold-icon" aria-hidden="true">⌄</span></span><span class="timeline-time-value">'+when+'</span><span class="timeline-time-footer"><span class="timeline-group-count">'+entries.length+' 条记录</span><span class="timeline-fold-label">收起</span></span></button><div class="timeline-group-options"><div class="timeline-category-controls" role="group" aria-label="本组记录类别"></div><div class="timeline-category-footer"><span data-timeline-visible></span><button data-timeline-show-all hidden>全部显示</button></div></div></div></aside><div class="timeline-branches">'+entries.map(entry=>timelineEntry(entry.row,response,entry.moment,entry.index)).join('')+'</div><div class="timeline-group-empty" hidden><span>本组类别均已隐藏</span><button data-timeline-show-all>全部显示</button></div></section>';
    }).join('')+'</section>').join('')+'</div>';
  }

  /// The alias a row's instance carries, so the same button the object list uses also works here.
  /// The timeline row names the instance by id; the detail query takes either, so the id is used.
  function instanceAlias(response,row){
    void response;
    return String(row.instance_id);
  }


  /// What every source is doing, and how far the projection has got.
  ///
  /// The page's job is to keep two failures apart that look alike and are not: a source that could
  /// not be read (collection failure) and material that arrived and could not be turned into
  /// objects (parse failure). They call for different responses — one is a permission or an
  /// availability problem, the other is an adapter that does not understand what it was given — and
  /// a page that reports one number for both tells the reader nothing about which to fix.
  function coverage(response){
    const rows=response.data||[];
    if(!rows.length)return '<div class="empty">还没有任何来源的采集记录。后台完成一次采集后这里会有内容。</div>';
    return `<div class="object-table-wrap"><table class="table"><thead><tr><th>监控项目</th><th>当前状态</th><th>影响</th><th>最近检查</th><th>详情</th></tr></thead><tbody>${rows.map(row=>{
      const projection=row.projection||{};
      return `<tr><td>${esc(sourceName(row.source))}</td><td>${row.configured?esc(label(coverageNames,row.last_result,'尚无采集结果')):'已暂停'}</td><td>${esc(coverageImpact(row))}</td><td>${esc(time(row.last_attempt_at))}</td><td><details><summary>查看</summary>来源：${esc(row.source)}<br>范围：${esc(row.scope||'来源级')}<br>最近成功：${esc(time(row.last_success_at))}<br>${Number(projection.pending||0)} 条待处理 · ${Number(projection.failed||0)} 条解析失败</details></td></tr>`;
    }).join('')}</tbody></table></div>`;
  }

  /// How far the projection has got. A backlog is not an error by itself; a failure is, and the
  /// two are counted apart.
  function projection(response){
    const rows=response.data||[];
    if(!rows.length)return '<div class="empty">尚无处理结果，暂不能判断信息是否已更新。</div>';
    const failed=rows.reduce((n,row)=>n+Number(row.failed||0),0),pending=rows.reduce((n,row)=>n+Number(row.pending||0),0);
    return `<p>${failed?`${failed} 条记录未能处理，部分变化可能无法显示。`:pending?`${pending} 条记录正在处理，最新变化可能稍后显示。`:'已收到的记录处理完成。'}</p><details><summary>处理详情</summary><div class="object-table-wrap"><table class="table"><thead><tr><th>投影</th><th>规则版本</th><th>结构版本</th><th>待办</th><th>失败</th><th>最近错误</th><th>更新于</th></tr></thead><tbody>${rows.map(row=>`<tr><td>${esc(row.projection)}</td><td>${esc(String(row.rule_version||'').slice(0,12))}</td><td>${esc(String(row.schema_version))}</td><td>${esc(String(row.pending))}</td><td>${esc(String(row.failed))}</td><td>${esc(row.last_error||'—')}</td><td>${esc(time(row.updated_at))}</td></tr>`).join('')}</tbody></table></div></details>`;
  }


  /// One instance's observation versions, compared.
  ///
  /// Three rules decide how this reads, and each of them is a rule the model already states rather
  /// than a presentation choice:
  ///
  ///   * a field that one observation did not carry is **not** a deletion. A source saying nothing
  ///     about a field is not the same as a source saying the field is gone, so a change whose side
  ///     is missing is shown as an incomplete pair rather than as "removed";
  ///   * an interval is the span a change was in effect over. Its end is a bound, and the row says
  ///     so instead of presenting it as when the change happened;
  ///   * an instance that has ended or been removed is not compared as though it were live. The
  ///     comparison is shown with a banner, because a difference against an object that is gone
  ///     means something different from a difference against one that is present.
  function comparison(detail,actions){
    if(!detail||!detail.summary)return '<div class="empty">选择一个对象后显示前后变化。</div>';
    const summary=detail.summary,current=Object.fromEntries((detail.fields||[]).map(field=>[field.field_id,field]));
    const banner=presentation.lifecycle(summary)?'<p class="notice">'+esc(presentation.lifecycle(summary))+'。下方显示历史变化。</p>':'';
    const rows=(actions.data||[]).filter(action=>action.field_id);
    if(!rows.length)return banner+'<div class="empty">尚未记录到属性变化，首次记录作为对比起点。</div>';
    return banner+'<div class="object-table-wrap"><table class="table"><thead><tr><th>变化项目</th><th>变化前</th><th>变化后</th><th>发生时间范围</th><th>检查情况</th></tr></thead><tbody>'+rows.map(action=>{
      if(readableEvents.historicalMappingIssue(action))return '<tr><td>'+esc(readableEvents.fieldLabel(action))+'</td><td colspan="4">'+esc(readableEvents.impact(action))+'<details><summary>查看旧版原始比较</summary><pre>'+esc(JSON.stringify({变化前:action.before,变化后:action.after},null,2))+'</pre></details></td></tr>';
      const field=current[action.field_id],incomplete=action.before===null||action.before===undefined||action.after===null||action.after===undefined;
      const event={...action,context:{...summary,...action.context,type_id:summary.type_id}};
      return '<tr><td>'+esc(readableEvents.fieldLabel(event))+'</td><td>'+esc(objectNames.valueLabel(action.before,action.field_id,event.context,action.before_status||'known'))+'</td><td>'+esc(objectNames.valueLabel(action.after,action.field_id,event.context,action.after_status||'known'))+'</td><td>'+esc(time(action.interval_from))+' 至 '+esc(time(action.interval_until))+(action.time_derived?'<br><small class="muted">发生在此期间</small>':'')+'</td><td>'+esc(field?label(statusNames,field.status,'尚无属性读取结果'):'当前没有这项信息')+'</td></tr><tr><td colspan="5"><p>'+esc(incomplete?'缺少前后对比值，可展开查看已取得的信息。':readableEvents.impact(event))+'</p><details><summary>完整前后值</summary><pre>'+esc(JSON.stringify({项目:action.field_id,变化前:action.before,变化后:action.after},null,2))+'</pre></details></td></tr>';
    }).join('')+'</tbody></table></div>';
  }

  /// The overview: what the model holds, what has happened lately, and how collection is doing.
  ///
  /// Counts are shown per kind and never added together. An object, a source record, an action and
  /// a piece of evidence are four different things counted four different ways; one number over all
  /// of them would be a number that means nothing. The type counts say how much of the catalog is
  /// wired to a source, which is the honest answer to "how much is collected" — a type with no
  /// source is not an empty collection, it is one nothing feeds.
  function overview(types, instances, actions, coverageResponse){
    const catalog=types.data||[];
    const categories=[...new Set(catalog.map(entry=>entry.category))];
    const wired=catalog.filter(entry=>entry.supplied_by_source).length;
    const counts=instances.counts||{};
    const rows=actions.data||[];
    const sources=coverageResponse.data||[];
    const failing=sources.filter(entry=>entry.configured&&(entry.last_result!=='success'||Number(entry.projection?.failed||0)>0)).length;
    const disabled=sources.filter(entry=>!entry.configured).length;
    const stat=(label,value,hint='')=>`<div class="stat"><span class="muted">${label}</span><strong>${value}</strong>${hint?`<br><small class="muted">${hint}</small>`:''}</div>`;
    const recent=rows.length?rows.slice(0,8).map(row=>{
      const when=row.precision==='interval'?`${esc(time(row.interval_from))} → ${esc(time(row.interval_until))}`:esc(time(row.occurred_at||row.observed_at));
      return `<li class="overview-action${row.instance_id?'':' unattributed'}"><span class="muted">${when}</span> <span class="action-title">${esc(readableEvents.title(row))}</span><p class="muted">${esc(readableEvents.subject(row))}</p></li>`;
    }).join(''):'<li class="muted">还没有观察到动作。</li>';
    return `<div class="stats">
        ${stat('当前对象',counts.instances??'—')}
        ${stat('已记录变化',counts.actions??'—')}
        ${stat('监控项目',sources.length,`${failing} 项信息待补充 · ${disabled} 项已暂停`)}
      </div>
      ${failing?'<p class="notice">部分监控信息不全，相关异常可能漏报。</p>':''}
      <details><summary>记录统计与支持范围</summary><p>对象类别 ${categories.length} · 对象类型 ${catalog.length}（${wired} 个已接入） · 来源记录 ${counts.source_records??'—'} · 证据 ${counts.evidence??'—'}</p></details>
      <div class="split"><section class="panel"><h2>最近发生</h2><ul class="overview-actions">${recent}</ul></section>
      <aside class="panel"><h2>来源采集</h2>${sources.length?coverage(coverageResponse):'<div class="empty">还没有来源记录。</div>'}</aside></div>`;
  }


  /// The one-line summary above a list: matched objects and the counts that belong with them.
  ///
  /// It lives here rather than in the page because it reads fields of the response, and this module
  /// is where every such read is declared and checked. A page that formatted the counts itself would
  /// be building its own view of the contract, which is the thing the checks exist to prevent.
  function listSummary(response){
    const counts=response.counts||{};
    return `<div class="object-count">找到 ${counts.instances||0} 个对象<details><summary>相关记录数量</summary>变化 ${counts.actions||0} 条 · 来源记录 ${counts.source_records||0} 条 · 原始材料 ${counts.evidence||0} 件</details></div>`;
  }

  /// Why a response carries no rows, in the contract's own words.
  ///
  /// A response that is not loaded says why in its warnings; falling back to a generic sentence only
  /// happens when the contract itself said nothing, which should not occur for a rejection.
  function messageOf(response,fallback){
    const warning=response.warnings&&response.warnings[0];
    return warning&&warning.message?warning.message:fallback;
  }

  /// The model's migration state, for the settings page to reach without it covering the page.
  ///
  /// The pair that matters when something looks wrong is the two versions: the catalog this frontend
  /// was built against and the rule version the background is processing material with. Showing them
  /// together is the point — differing means the frontend needs rebuilding.
  function migrationStatus(response){
    const rows=response.data||[];
    if(!rows.length)return '还没有投影状态记录：对象模型尚未处理任何素材。';
    return `<p>前端构建时的模型目录版本：<strong>${esc(response.catalog_version)}</strong>。下面的规则版本是后台处理素材时使用的目录版本；两者不一致说明前端需要重新构建。</p><ul>${rows.map(row=>`<li>${esc(row.projection)}：结构版本 ${esc(String(row.schema_version))} · 规则版本 ${esc(String(row.rule_version).slice(0,12))} · 待办 ${esc(String(row.pending))} · 失败 ${esc(String(row.failed))}${row.last_error?`<br><small class="muted">最近错误：${esc(row.last_error)}</small>`:''}</li>`).join('')}</ul>`;
  }

  /// Whether a list response carried no rows.
  function isEmpty(response){
    return !(response.data||[]).length;
  }

  /// The options for a picker over instances, from a list response.
  function instanceOptions(response){
    const rows=response.data||[];
    return '<option value="">选择一个对象</option>'+rows.map((row,index)=>`<option value="${esc(row.alias)}">${esc(objectNames.name(row))} · ${esc(presentation.state(row))} · 第 ${index+1} 项</option>`).join('');
  }

  function sourceReadings(response,{recordsOnly=false}={}){
    const names={'pcscope.execution.module-scan-status':'模块观察与覆盖','pcscope.execution.module-file-state':'模块关联文件检查','pcscope.execution.resources':'资源采样','pcscope.collector.scoped-configuration':'采集范围配置','pcscope.collector.event-channel-coverage':'日志通道覆盖','pcscope.usn.watch-list':'文件监控名单'};
    const rows=response.data||[];
    const total=response.counts?.source_records||0;
    const facts={total_kib:'总内存（KiB）',free_kib:'可用内存（KiB）',percent_processor_time:'处理器使用率（%）',sampled_at:'采样时间',channel:'日志通道',readable:'可读取',enabled:'已启用',record_count:'现存记录数',oldest_record:'最早记录号',maximum_bytes:'通道容量（字节）',reason:'原因',active:'已配置',files:'文件名单',watch_files:'文件名单',watch_directories:'目录名单',portable_roots:'便携程序范围',revision:'范围版本'};
    Object.assign(facts,{target:'目标进程',status:'观察结果',instances:'进程实例与读取结果',session_id:'会话',user_sid:'账户',file_monitoring_enabled:'文件跟踪已启用',tracked_files:'历史已观察文件数',watched_directories:'正在监听的目录数',pending_files:'待复核文件数',unavailable_files:'尚无有效检查或本次不可读的文件数',partial:'存在未完成检查或覆盖缺口',auto_watch_revision:'自动名单版本',error:'原因',reason:'原因',history_scope:'历史跟踪范围'});
    const render=payload=>Object.entries(payload||{}).filter(([key])=>facts[key]).map(([key,value])=>esc(facts[key])+'：'+esc(typeof value==='object'?JSON.stringify(value):value)).join('<br>');
    const latest=new Map();for(const row of rows)if(row.source==='pcscope.execution.module-scan-status'&&!latest.has(row.item_id))latest.set(row.item_id,row);
    const controller=latest.get('runtime-controller')?.payload;
    const scanSummary=[...latest.values()].map(row=>{
      const p=row.payload||{};
      const text=p.target?({observed:'进程正在运行，正在跟踪已加载的模块文件。',not_observed:'本次运行列表中没有该进程。',partial:'部分模块未能读取，相关文件变化可能漏报。',discovery_failed:'进程列表读取失败，本次模块扫描未完成。'})[p.status]||'进程信息读取不完整，相关变化可能漏报。'
        :p.enabled===false?'监控已暂停，暂停期间的变化可能漏记。':p.file_monitoring_enabled===false?'文件跟踪已暂停，模块文件变化可能漏记。':p.partial?'部分文件尚未完成检查，相关异常可能暂未显示。':'模块文件跟踪已启用。';
      return p.target&&controller?.enabled===false?'':`<p><strong>${esc(p.target||'文件跟踪')}</strong>：${esc(text)}<br><small class="muted">最近记录：${esc(time(row.observed_at))}</small></p>`;
    }).join('');
    const records=rows.map(row=>'<details><summary>'+esc(names[row.source]||sourceName(row.source))+' · '+esc(time(row.observed_at))+'</summary><p>'+render(row.payload)+'</p><small>'+esc(row.item_id)+' · '+esc(row.scope)+' · 证据 '+esc(row.evidence.id)+'</small><details><summary>完整原始字段</summary><pre>'+esc(JSON.stringify(row.payload,null,2))+'</pre></details>'+(row.progress?'<details><summary>读取进度与历史缺口</summary><pre>'+esc(JSON.stringify(row.progress,null,2))+'</pre></details>':'')+'</details>').join('');
    if(recordsOnly)return records;
    const history='<div class="source-reading-records">'+records+'</div>';
    return scanSummary+(total?'<p class="muted">共 '+total+' 条保留记录。</p>':'')+(latest.size?'<details><summary>观察记录与诊断详情</summary>'+history+'</details>':history);
  }

  function moduleFileAssessment(payload){
    const check=payload.check||{},changes=payload.changes||[];
    if(check.status==='missing')return '文件已缺失，下次启动或使用相关功能可能失败。';
    if(check.status!=='stable')return '本次文件检查失败，尚无本次检查结果。';
    if(check.pe_status==='basic_headers_invalid')return '文件结构异常，下次加载相关功能可能失败。';
    if(check.signature_status==='verification_failed')return '签名未通过验证，暂不能确认文件可信性。';
    if(changes.includes('file_moved'))return '文件已移动或改名，原路径的后续加载可能受影响。';
    if(changes.some(c=>['file_id','volume_identity','sha256','size','last_write_utc'].includes(c)))return '文件内容或属性已变化。';
    if(changes.includes('dacl_sddl'))return '访问权限已变化，可能影响相关功能读取文件。';
    if(check.signature_status==='not_signed')return '文件未签名，发布者未验证。';
    if(check.pe_status==='basic_headers_valid')return '文件基础结构检查通过。';
    return '文件存在，尚无结构检查结果。';
  }

  function moduleFiles(response){
    const labels={baseline_observed:'首次文件基线',file_moved:'原文件改名或移动（已核对文件身份）',exists:'文件存在状态变化',volume_identity:'卷身份变化',file_id:'磁盘文件身份变化',size:'文件大小变化',last_write_utc:'写入时间变化',attributes:'属性变化',owner:'所有者变化',dacl_sddl:'访问权限变化',version:'版本变化',sha256:'文件内容指纹变化',signature_status:'签名验证状态变化',signature_code:'签名结果变化',publisher:'发布者变化',pe_status:'PE 基础结构状态变化',pe_machine:'映像架构变化',check_unavailable:'本次无法稳定检查'};
    const statuses={stable:'已取得稳定磁盘观察',missing:'原磁盘路径不存在',unavailable:'无法稳定检查，保留前次有效结果'};
    const signatures={valid_offline:'离线签名验证通过',not_signed:'未发现签名',verification_failed:'签名验证失败（详见返回码）',unavailable:'未知',not_requested:'本次仅检查元数据',skipped_size_limit:'超过单文件检查上限',pending:'待验证'};
    const rows=response.data||[];
    if(!rows.length)return '<p class="muted">尚无文件检查结果，暂不能判断是否影响桌面功能。</p>';
    const origins={microsoft_signed:'微软签名；产品归属仍需其他证据',other_publisher_signed:'其他发布者签名（第三方候选）',unsigned:'未发现签名（第三方候选或归属未知）',unknown:'归属未知'};
    const pe={basic_headers_valid:'基础头部及节区范围检查通过',basic_headers_invalid:'基础结构检查异常',unavailable:'未知'};
    return rows.map(row=>{
      const payload=row.payload||{},check=payload.check||{},bindings=Object.values(payload.module_bindings||{});
      const name=String(payload.path||row.item_id).split(/[\\/]/).pop();
      return '<article class="panel module-file-card"><strong>'+esc(name)+'</strong><p>'+esc(moduleFileAssessment(payload))+'</p><small class="muted">检查时间：'+esc(time(row.observed_at))+' · 发布者：'+esc(check.publisher||'未取得发布者信息')+'</small><details><summary>文件与诊断详情</summary><p>'+esc(payload.path||row.item_id)+' · '+esc(statuses[check.status]||'未知')+'</p><p>变化：'+esc((payload.changes||[]).map(c=>labels[c]||c).join('、')||'无已确认差异')+'</p><p>发布者类别：'+esc(origins[check.module_origin]||'归属未知')+'</p><p>签名：'+esc(signatures[check.signature_status]||'未知')+'；发布者：'+esc(check.publisher||'未知')+'；版本：'+esc(check.version||'未知')+'</p>'+(check.pe_status?'<p>PE 文件：'+esc(pe[check.pe_status]||check.pe_status)+(check.pe_error?' · '+esc(check.pe_error):'')+'。</p>':'')+'<p>关联模块观察：'+bindings.map(b=>esc(b.process_name)+' / PID '+esc(b.pid)+' / 建立 '+esc(time(b.process_start))+' / 最近观察 '+esc(time(b.last_observed))+(b.module_path?' / 原观察路径 '+esc(b.module_path):'')).join('<br>')+'</p>'+(check.error?'<p>'+esc(check.error)+'</p>':'')+'<small>证据 '+esc(row.evidence?.id)+'</small><details><summary>前后状态与完整原始字段</summary><pre>'+esc(JSON.stringify(payload,null,2))+'</pre></details></details></article>';
    }).join('');
  }
  return {treePage,moduleFileAssessment,coverageImpact,moduleFiles,sourceReadings, PATHS, OPTIONAL_PATHS, list, detail, timeline, coverage, projection, comparison, overview, listSummary, messageOf, instanceOptions, isEmpty, migrationStatus};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeModelView;
