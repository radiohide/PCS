'use strict';
const PCScopeStateCompare=(()=>{
  const storageKey='pcscope.compareTimes';
  const canonical=value=>Array.isArray(value)?value.map(canonical):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(k=>[k,canonical(value[k])])):value;
  const equal=(a,b)=>JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
  const localInput=date=>new Date(date.getTime()-date.getTimezoneOffset()*60000).toISOString().slice(0,16);
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
    let disposed=false,revision=0,busy=false,applied=null,pagers=[],branches=[],loaded=0,totals={added:0,absent:0,changed:0},onlyChanges=true;
    try{onlyChanges=localStorage.getItem('pcscope.compareOnlyChanges')!=='false';}catch{}
    const now=new Date();let times={from:localInput(new Date(now.getTime()-3600000)),until:localInput(now)};
    try{const saved=JSON.parse(localStorage.getItem(storageKey));if(saved?.from&&saved?.until&&Number.isFinite(Date.parse(saved.from))&&Number.isFinite(Date.parse(saved.until)))times=saved;}catch{}
    container.innerHTML=`<div class="compare-times"><label>基准时间 <small>A · 左侧</small><input data-compare-from type="datetime-local" value="${esc(times.from)}"><span>截至此时的最近检查结果</span></label><label>对比时间 <small>B · 右侧</small><input data-compare-until type="datetime-local" value="${esc(times.until)}"><span>截至此时的最近检查结果</span></label></div><div class="compare-toolbar"><p data-compare-summary>选择两个时间点，查看对象和属性的变化。</p><button data-compare-run class="primary">对比</button></div><p data-compare-message role="status"></p><div data-compare-results></div>`;
    const results=container.querySelector('[data-compare-results]'),message=container.querySelector('[data-compare-message]'),button=container.querySelector('[data-compare-run]');
    const filter=document.createElement('label');filter.className='compare-filter';filter.innerHTML='<input type="checkbox" role="switch" data-compare-only-changes>只看变化';button.before(filter);
    const toggle=filter.querySelector('input');toggle.checked=onlyChanges;
    const isChange=kind=>['added','absent','changed'].includes(kind);
    function filterObjects(){
      for(const shell of results.querySelectorAll('.compare-object'))shell.hidden=onlyChanges&&!isChange(shell.dataset.compareKind);
      for(const branch of branches)branch.paint();
    }
    toggle.onchange=()=>{onlyChanges=toggle.checked;try{localStorage.setItem('pcscope.compareOnlyChanges',String(onlyChanges));}catch{}filterObjects();summary();for(const pager of pagers)pager.resume();};
    const alive=()=>!disposed&&container.isConnected;
    function summary(){container.querySelector('[data-compare-summary]').innerHTML=`${onlyChanges?'已检查':'已载入'} ${loaded} 个对象 <span class="compare-changed">${totals.changed} 项变化</span><span class="compare-added">${totals.added} 项首次记录</span><span class="compare-absent">${totals.absent} 项退出清单</span>`;}
    function pairRow(parent,labelLeft,labelRight,kind='branch',tailLeft='',tailRight=''){
      const row=document.createElement('div');row.className='compare-pair compare-'+kind;
      row.innerHTML=[labelLeft,labelRight].map((label,i)=>`<button type="button" class="compare-cell" aria-expanded="false"><i class="compare-chevron" aria-hidden="true"></i><span>${esc(label)}</span><small>${esc(i?tailRight:tailLeft)}</small></button>`).join('');parent.append(row);return row;
    }
    function expandable(row,body,onOpen=()=>{}){
      const controls=[...row.querySelectorAll('button')];body.hidden=true;
      controls.forEach(control=>control.onclick=()=>{const open=body.hidden;body.hidden=!open;controls.forEach(b=>b.setAttribute('aria-expanded',String(open)));if(open)onOpen();});
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
      expandable(row,body);loaded++;if(Object.hasOwn(totals,kind))totals[kind]++;
    }
    function branch(parent,entry,request){
      const shell=document.createElement('section');shell.className='compare-type';parent.append(shell);
      const label=PCScopeObjectNames.typeName(entry),row=pairRow(shell,label,label,'type-row',`${entry.count} 个历史对象`,`${entry.count} 个历史对象`),body=document.createElement('div');shell.append(body);
      const objects=document.createElement('div'),note=document.createElement('p'),anchor=document.createElement('div');note.className='compare-filter-note';note.hidden=true;body.append(objects,note,anchor);let started=false,complete=false;
      const paint=()=>{const nodes=[...objects.children],visible=nodes.some(n=>!n.hidden),unavailable=nodes.filter(n=>n.dataset.compareKind==='unavailable').length;note.hidden=!onlyChanges||(!complete&&!unavailable)||(visible&&!unavailable);note.textContent=[!visible&&complete?(unavailable?'暂无可显示的变化':'没有变化'):'',unavailable?`${unavailable} 个对象缺少可对比记录，关闭“只看变化”可查看`:''].filter(Boolean).join(' · ');};branches.push({paint});
      const pager=PCScopeAutoPage.mount(anchor,{active:()=>alive()&&!body.hidden&&request.revision===revision,query:cursor=>query({query:'state_compare',from:request.from,until:request.until,watermark:request.watermark,type_id:entry.type_id,limit:50,...(cursor?{cursor}:{})}),apply:(response,{append})=>{if(request.revision!==revision)return;if(!append)objects.replaceChildren();for(const pair of response.data?.objects||[])object(objects,pair);complete=!response.page?.has_more;paint();summary();},onError:()=>{},endLabel:'此类型已全部检查'});pagers.push(pager);
      expandable(row,body,()=>{if(!started){started=true;pager.reset();}else pager.resume();});
    }
    async function reload({automatic=false}={}){
      // A selected historical cutoff is fixed. Background polling must not move the comparison.
      if(automatic||busy||!alive())return;
      const from=container.querySelector('[data-compare-from]').value,until=container.querySelector('[data-compare-until]').value;
      if(!from||!until||!Number.isFinite(Date.parse(from))||!Number.isFinite(Date.parse(until))){message.textContent='请选择完整的日期和时间。';return;}
      if(new Date(from)>new Date(until)){message.textContent='基准时间不能晚于对比时间。';return;}
      busy=true;button.disabled=true;message.textContent='正在读取对比目录…';const current=revision+1;
      try{
        const request={from:new Date(from).toISOString(),until:new Date(until).toISOString(),revision:current};
        const response=await query({query:'state_compare',from:request.from,until:request.until});
        if(!alive())return;
        if(!['loaded','empty','partial'].includes(response.status))throw Error('历史对比暂时无法读取，请重试。');
        revision=current;for(const pager of pagers)pager.dispose();pagers=[];branches=[];request.watermark=response.data?.watermark;
        applied={from,until};try{localStorage.setItem(storageKey,JSON.stringify(applied));}catch{}
        results.replaceChildren();loaded=0;totals={added:0,absent:0,changed:0};
        const header=document.createElement('div');header.className='compare-pair compare-tree-head';header.innerHTML=[['基准',from],['对比',until]].map(([label,time])=>`<div class="compare-cell"><strong>本机 · ${label}</strong><span>${esc(localTime(time))}</span></div>`).join('');results.append(header);
        const categories=new Map();for(const entry of response.data?.branches||[]){
          if(!categories.has(entry.category)){
            const shell=document.createElement('section');shell.className='compare-category';results.append(shell);const label=categoryNames[entry.category]||'其他对象';const row=pairRow(shell,label,label,'category-row'),body=document.createElement('div');shell.append(body);expandable(row,body);categories.set(entry.category,body);
          }
          branch(categories.get(entry.category),entry,request);
        }
        message.textContent=categories.size?'左右同步展开，逐行查看对象和属性。':'所选时间内没有对象记录。';summary();
      }catch(error){if(alive())message.textContent=errorText(error)+(applied?' 已保留上次对比结果。':'');}
      finally{busy=false;if(alive())button.disabled=false;}
    }
    button.onclick=()=>reload();
    return {reload,dispose(){disposed=true;revision++;for(const pager of pagers)pager.dispose();}};
  }
  return {mount,changes,equal};
})();
if(typeof module!=='undefined')module.exports=PCScopeStateCompare;
