'use strict';
const PCScopeStateCompare=(()=>{
  const storageKey='pcscope.compareTimes';
  const canonical=value=>Array.isArray(value)?value.map(canonical):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(k=>[k,canonical(value[k])])):value;
  const equal=(a,b)=>JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
  const localInput=date=>new Date(date.getTime()-date.getTimezoneOffset()*60000).toISOString().slice(0,16);
  function rangeIssue(from,until,boundary,now=Date.now()){
    const a=Date.parse(from),b=Date.parse(until),start=Date.parse(boundary);
    if(!Number.isFinite(a)||!Number.isFinite(b))return {kind:'invalid',text:'请选择完整的日期和时间。'};
    if(a>b)return {kind:'order',text:'基准时间不能晚于对比时间，请调整时间。'};
    if(a>now||b>now)return {kind:'future',text:`${a>now?'两个时间点都':'对比时间'}在未来，请选择当前或更早的时间。`};
    if(Number.isFinite(start)&&a<start)return {kind:'before_collection',left:true,right:b<start,text:b<start?'两个时间点都早于采集起点，没有可对比的状态。':'基准时间早于采集起点，缺少左侧状态。'};
    if(a===b)return {kind:'same',text:'两个时间点相同，请选择不同时间查看变化。'};
    return null;
  }
  function changes(pair){
    const a=pair.left,b=pair.right;
    if(['unavailable','event_only','ambiguous'].includes(a.availability)||['unavailable','event_only','ambiguous'].includes(b.availability))return 'unavailable';
    if(a.availability==='not_recorded')return b.availability==='recorded'?'added':'same';
    if(b.availability==='not_recorded')return 'unavailable';
    if(a.summary?.lifecycle!==b.summary?.lifecycle)return b.summary?.lifecycle==='not_observed'?'absent':'changed';
    const left=new Map((a.fields||[]).map(f=>[f.field_id,f])),right=new Map((b.fields||[]).map(f=>[f.field_id,f]));
    return [...new Set([...left.keys(),...right.keys()])].some(id=>{
      const l=left.get(id),r=right.get(id);return l?.status==='known'&&r?.status==='known'&&!equal(l.value,r.value);
    })?'changed':'same';
  }
  function mount(container,{query,esc,localTime,errorText,categoryNames}){
    let disposed=false,revision=0,busy=false,applied=null,collectionStart=null,pagers=[],branches=[],groups=[],loaded=0,totals={added:0,absent:0,changed:0},onlyChanges=true;
    try{onlyChanges=localStorage.getItem('pcscope.compareOnlyChanges')!=='false';}catch{}
    let expanded=new Set();try{const saved=JSON.parse(localStorage.getItem('pcscope.compareExpanded'));if(Array.isArray(saved))expanded=new Set(saved.filter(key=>typeof key==='string'));}catch{}
    const saveExpanded=()=>{try{localStorage.setItem('pcscope.compareExpanded',JSON.stringify([...expanded]));}catch{}};
    const now=new Date();let times={from:localInput(new Date(now.getTime()-3600000)),until:localInput(now)};
    try{const saved=JSON.parse(localStorage.getItem(storageKey));if(saved?.from&&saved?.until&&Number.isFinite(Date.parse(saved.from))&&Number.isFinite(Date.parse(saved.until)))times=saved;}catch{}
    container.innerHTML=`<div class="compare-times"><label>基准时间 <small>A · 左侧</small><input data-compare-from type="datetime-local" value="${esc(times.from)}"><span>截至此时的最近检查结果</span></label><label>对比时间 <small>B · 右侧</small><input data-compare-until type="datetime-local" value="${esc(times.until)}"><span>截至此时的最近检查结果</span></label></div><div class="compare-toolbar"><p data-compare-summary>选择两个时间点，查看对象和属性的变化。</p><button data-compare-run class="primary">对比</button></div><p data-compare-message role="status"></p><div data-compare-results></div>`;
    const results=container.querySelector('[data-compare-results]'),message=container.querySelector('[data-compare-message]'),button=container.querySelector('[data-compare-run]');
    const inputs=[container.querySelector('[data-compare-from]'),container.querySelector('[data-compare-until]')];
    const rangeNotice=document.createElement('p');rangeNotice.className='compare-range-notice';rangeNotice.hidden=true;rangeNotice.setAttribute('role','status');rangeNotice.dataset.compareRangeNotice='';container.querySelector('.compare-times').after(rangeNotice);
    function validateRange(){
      const issue=rangeIssue(inputs[0].value,inputs[1].value,collectionStart);
      rangeNotice.hidden=!issue;
      rangeNotice.textContent=issue?(issue.kind==='before_collection'?`${issue.text} 当前数据从 ${localTime(collectionStart)} 开始，请选择此后的时间。`:issue.text):'';
      inputs.forEach((input,i)=>{const early=issue?.kind==='before_collection'&&(i===0||issue.right);input.parentElement.classList.toggle('compare-time-warning',!!early);input.nextElementSibling.textContent=early?'此时尚未开始采集':'截至此时的最近检查结果';});
      return issue;
    }
    for(const input of inputs)input.addEventListener('input',()=>{validateRange();if(applied&&(inputs[0].value!==applied.from||inputs[1].value!==applied.until))message.textContent='时间已修改，请点击“对比”更新下方结果。';});
    validateRange();
    const filter=document.createElement('label');filter.className='compare-filter';filter.innerHTML='<input type="checkbox" role="switch" data-compare-only-changes>只看变化';button.before(filter);
    const toggle=filter.querySelector('input');toggle.checked=onlyChanges;
    const progress=document.createElement('div');progress.className='compare-progress';progress.hidden=true;progress.innerHTML='<span data-compare-progress-label role="status"></span><progress data-compare-progress aria-label="已展开类型的对比进度"></progress>';container.querySelector('.compare-toolbar').after(progress);
    const progressLabel=progress.querySelector('span'),progressBar=progress.querySelector('progress');
    const isChange=kind=>['added','absent','changed'].includes(kind);
    function filterObjects(){
      for(const shell of results.querySelectorAll('.compare-object'))shell.hidden=onlyChanges&&!isChange(shell.dataset.compareKind);
      for(const branch of branches)branch.paint();
    }
    toggle.onchange=()=>{onlyChanges=toggle.checked;try{localStorage.setItem('pcscope.compareOnlyChanges',String(onlyChanges));}catch{}filterObjects();summary();for(const pager of pagers)pager.resume();};
    const alive=()=>!disposed&&container.isConnected;
    function differenceCount(row,states){
      const count=states.reduce((n,s)=>n+s.different,0),unavailable=states.reduce((n,s)=>n+s.unavailable,0),read=states.reduce((n,s)=>n+s.read,0);
      const complete=states.length>0&&states.every(s=>s.complete),started=states.some(s=>s.started),failed=states.some(s=>s.failed);
      const text=!started?'待对比':!read&&!complete?(failed?'读取失败':'正在对比…'):`${complete&&!unavailable?'':'已发现 '}${count} 项不同${failed?' · 读取失败':unavailable?` · ${unavailable} 项缺少记录`:''}`;
      for(const badge of row.querySelectorAll('small')){badge.textContent=text;badge.classList.toggle('compare-difference-count',count>0);}
    }
    function summary(){
      for(const branch of branches)differenceCount(branch.row,[branch.status()]);
      for(const group of groups)differenceCount(group.row,branches.filter(b=>b.parent===group.body).map(b=>b.status()));
      const started=branches.map(b=>b.status()).filter(b=>b.started),pending=started.filter(b=>!b.complete),failed=started.filter(b=>b.failed).length,total=started.reduce((n,b)=>n+b.total,0),node=container.querySelector('[data-compare-summary]');
      progress.hidden=!started.length;progressBar.max=Math.max(1,total);progressBar.value=pending.length?Math.min(loaded,Math.max(0,total-1)):total;
      progressLabel.textContent=`${failed?'对比未完成':pending.length?(pending.some(b=>b.open)?'正在对比':'已暂停，展开类型后继续'):'已展开类型对比完成'} · ${loaded} / ${total} 个对象${failed?' · 读取失败，请重试':''}`;
      results.setAttribute('aria-busy',String(pending.some(b=>b.open&&!b.failed)));
      if(!loaded){node.textContent=failed?'部分类型读取失败，请在对应类型下重试。':pending.length?'正在对比对象…':branches.length?'展开对象类型，查看这两个时间点的变化。':'所选时间内没有对象记录。';return;}
      node.innerHTML=`${pending.length?'已发现':'已展开类型中'} <span class="compare-changed">${totals.changed} 项变化</span><span class="compare-added">${totals.added} 项首次记录</span><span class="compare-absent">${totals.absent} 项退出清单</span>${failed?`<span>${failed} 类读取失败，请重试</span>`:''}`;
    }
    function pairRow(parent,labelLeft,labelRight,kind='branch',tailLeft='',tailRight=''){
      const row=document.createElement('div');row.className='compare-pair compare-'+kind;
      row.innerHTML=[labelLeft,labelRight].map((label,i)=>`<button type="button" class="compare-cell" aria-expanded="false"><i class="compare-chevron" aria-hidden="true"></i><span>${esc(label)}</span><small>${esc(i?tailRight:tailLeft)}</small></button>`).join('');parent.append(row);return row;
    }
    function expandable(row,body,onOpen=()=>{},key=null){
      const controls=[...row.querySelectorAll('button')];body.hidden=true;
      const setOpen=open=>{body.hidden=!open;controls.forEach(b=>b.setAttribute('aria-expanded',String(open)));if(open)onOpen();if(key)summary();};
      controls.forEach(control=>control.onclick=()=>{const open=body.hidden;if(key){if(open)expanded.add(key);else expanded.delete(key);saveExpanded();}setOpen(open);});
      if(key&&expanded.has(key))setOpen(true);
    }
    function value(side,field){
      if(side.availability==='not_recorded')return '此前无记录';
      if(side.availability==='ambiguous')return '多条记录尚未分开';
      if(side.availability==='unavailable')return '历史信息不可用';
      return field?PCScopeObjectNames.valueLabel(field.value,field.field_id,side.summary,field.status):'未记录';
    }
    function object(parent,pair){
      const left=pair.left,right=pair.right,kind=changes(pair),source=right.summary||left.summary||{type_id:pair.type_id};
      const fallback=PCScopeObjectNames.name(source);
      const title=side=>side.summary?PCScopeObjectNames.name(side.summary,side.fields):fallback;
      const status=side=>side.availability==='not_recorded'?'此前无记录':side.availability==='ambiguous'?'多条记录尚未分开':side.availability==='unavailable'?'历史信息不可用':PCScopeTypePresentation.state(side.summary,side.fields);
      const shell=document.createElement('section');shell.className='compare-object';shell.dataset.compareObject=String(pair.instance_id);parent.append(shell);
      shell.dataset.compareKind=kind;shell.hidden=onlyChanges&&!isChange(kind);
      const row=pairRow(shell,title(left),title(right),'object-row',status(left),status(right));row.classList.add('compare-'+kind);
      const body=document.createElement('div');body.className='compare-fields';shell.append(body);
      // Build property rows only when the user opens this object.
      const buildFields=()=>{
      const checks=document.createElement('div');checks.className='compare-pair compare-checks';checks.innerHTML=[left,right].map(s=>`<div class="compare-cell">${esc(s.checked_at?(s.availability==='event_only'?'日志记录于 ':'检查于 ')+localTime(s.checked_at):s.availability==='not_recorded'?'此时间点尚无记录':s.availability==='ambiguous'?`此对象对应 ${s.source_record_count} 条来源记录，需先区分对象。`:'未保留可用的历史检查')}</div>`).join('');body.append(checks);
      const a=new Map((left.fields||[]).map(f=>[f.field_id,f])),b=new Map((right.fields||[]).map(f=>[f.field_id,f]));
      const ids=[...new Set([...a.keys(),...b.keys()])];
      ids.sort((x,y)=>Number(a.get(y)?.status==='known'&&b.get(y)?.status==='known'&&!equal(a.get(y).value,b.get(y).value))-Number(a.get(x)?.status==='known'&&b.get(x)?.status==='known'&&!equal(a.get(x).value,b.get(x).value)));
      for(const id of ids){
        const l=a.get(id),r=b.get(id),changed=l?.status==='known'&&r?.status==='known'&&!equal(l.value,r.value);
        const line=document.createElement('div');line.className='compare-pair compare-property'+(changed?' compare-changed':'');
        const label=PCScopeObjectNames.fieldLabel(source,r||l);
        line.innerHTML=[[left,l],[right,r]].map(([side,f])=>`<div class="compare-cell"><span class="compare-property-label">${esc(label)}</span><strong>${esc(value(side,f))}</strong>${changed?'<small>已变化</small>':''}</div>`).join('');body.append(line);
      }
      if(!ids.length){const empty=document.createElement('p');empty.className='muted';empty.textContent='这两个时间点没有可对比的属性。';body.append(empty);}
      };
      let built=false;expandable(row,body,()=>{if(!built){built=true;buildFields();}});loaded++;if(Object.hasOwn(totals,kind))totals[kind]++;
    }
    function branch(parent,entry,request){
      const shell=document.createElement('section');shell.className='compare-type';parent.append(shell);
      const label=PCScopeObjectNames.typeName(entry),row=pairRow(shell,label,label,'type-row',`${entry.count} 个历史对象`,`${entry.count} 个历史对象`),body=document.createElement('div');shell.append(body);
      for(const badge of row.querySelectorAll('small'))badge.title=`${entry.count} 个历史对象`;
      const objects=document.createElement('div'),note=document.createElement('p'),anchor=document.createElement('div');note.className='compare-filter-note';note.hidden=true;body.append(objects,note,anchor);let started=false,complete=false,reading=false,failed=false,different=0,unavailable=0,read=0;
      const paint=()=>{const nodes=[...objects.children],visible=nodes.some(n=>!n.hidden);read=nodes.length;different=nodes.filter(n=>isChange(n.dataset.compareKind)).length;unavailable=nodes.filter(n=>n.dataset.compareKind==='unavailable').length;note.hidden=!onlyChanges||(!complete&&!unavailable)||(visible&&!unavailable);note.textContent=[!visible&&complete?(unavailable?'暂无可显示的变化':'没有变化'):'',unavailable?`${unavailable} 个对象缺少可对比记录，关闭“只看变化”可查看`:''].filter(Boolean).join(' · ');};branches.push({row,parent,paint,status:()=>({started,complete,failed,different,unavailable,read,open:!body.hidden&&!parent.hidden,total:entry.count})});
      const pager=PCScopeAutoPage.mount(anchor,{eager:true,active:()=>alive()&&!body.hidden&&!parent.hidden&&request.revision===revision,query:cursor=>query({query:'state_compare',from:request.from,until:request.until,watermark:request.watermark,type_id:entry.type_id,limit:50,...(cursor?{cursor}:{})}),onStart:()=>{reading=true;failed=false;summary();},apply:(response,{append})=>{if(request.revision!==revision)return;if(!append){for(const shell of objects.children){loaded--;if(Object.hasOwn(totals,shell.dataset.compareKind))totals[shell.dataset.compareKind]--;}objects.replaceChildren();}for(const pair of response.data?.objects||[])object(objects,pair);complete=!response.page?.has_more;reading=false;failed=false;paint();summary();},onError:()=>{if(request.revision!==revision)return;reading=false;failed=true;summary();},endLabel:'此类型已全部检查'});pagers.push(pager);
      const start=()=>{if(parent.hidden||body.hidden)return;if(!started){started=true;pager.reset();}else pager.resume();};
      shell.startCompare=start;expandable(row,body,start,'type:'+entry.type_id);
    }
    async function reload({automatic=false}={}){
      // A selected historical cutoff is fixed. Background polling must not move the comparison.
      if(automatic||busy||!alive())return;
      const from=container.querySelector('[data-compare-from]').value,until=container.querySelector('[data-compare-until]').value;
      const issue=validateRange();
      if(issue){message.textContent=applied?'下方保留上次对比结果。':'';return;}
      busy=true;button.disabled=true;message.textContent='正在读取对比目录…';const current=revision+1;
      progress.hidden=false;progressBar.removeAttribute('value');progressLabel.textContent='正在读取对比目录…';
      try{
        const request={from:new Date(from).toISOString(),until:new Date(until).toISOString(),revision:current};
        const response=await query({query:'state_compare',from:request.from,until:request.until});
        if(!alive())return;
        if(inputs[0].value!==from||inputs[1].value!==until){summary();message.textContent='时间已修改，请点击“对比”更新下方结果。';return;}
        if(!['loaded','empty','partial'].includes(response.status))throw Error('历史对比暂时无法读取，请重试。');
        revision=current;for(const pager of pagers)pager.dispose();pagers=[];branches=[];groups=[];request.watermark=response.data?.watermark;
        collectionStart=response.data?.collection_started_at||null;
        applied={from,until};try{localStorage.setItem(storageKey,JSON.stringify(applied));}catch{}
        results.replaceChildren();loaded=0;totals={added:0,absent:0,changed:0};
        if(validateRange()){applied=null;summary();container.querySelector('[data-compare-summary]').textContent='请调整时间后重新对比。';message.textContent='';return;}
        const header=document.createElement('div');header.className='compare-pair compare-tree-head';header.innerHTML=[['基准',from],['对比',until]].map(([label,time])=>`<div class="compare-cell"><strong>本机 · ${label}</strong><span>${esc(localTime(time))}</span></div>`).join('');results.append(header);
        const categories=new Map();for(const entry of response.data?.branches||[]){
          if(!categories.has(entry.category)){
            const shell=document.createElement('section');shell.className='compare-category';results.append(shell);const label=categoryNames[entry.category]||'其他对象';const row=pairRow(shell,label,label,'category-row'),body=document.createElement('div');shell.append(body);groups.push({row,body});expandable(row,body,()=>{for(const type of body.children)type.startCompare?.();},'category:'+entry.category);categories.set(entry.category,body);
          }
          branch(categories.get(entry.category),entry,request);
        }
        message.textContent=categories.size?'左右同步展开，逐行查看对象和属性。':'所选时间点尚无对象记录，请选择更晚的时间或等待首次采集完成。';summary();
      }catch(error){if(alive()){summary();message.textContent=errorText(error)+(applied?' 已保留上次对比结果。':'');}}
      finally{busy=false;if(alive())button.disabled=false;}
    }
    button.onclick=()=>reload();
    return {reload,dispose(){disposed=true;revision++;for(const pager of pagers)pager.dispose();}};
  }
  return {mount,changes,equal,rangeIssue};
})();
if(typeof module!=='undefined')module.exports=PCScopeStateCompare;
