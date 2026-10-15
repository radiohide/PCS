'use strict';
const PCScopeDiff=(()=>{
  const masked=v=>v==='******'||!!(v&&typeof v==='object'&&(v.sensitive===true||v.redacted===true||v.display==='******'));
  const branch=v=>v!==null&&typeof v==='object'&&!masked(v);
  function rows(before,after){
    const result=[];
    function walk(a,b,hasA,hasB,path,depth){
      if(depth<40&&(branch(a)||branch(b))&&(!hasA||branch(a))&&(!hasB||branch(b))&&(!hasA||!hasB||Array.isArray(a)===Array.isArray(b))){
        const keys=[...new Set([...Object.keys(hasA?a:{}),...Object.keys(hasB?b:{})])];
        if(keys.length){for(const key of keys)walk(hasA?a[key]:undefined,hasB?b[key]:undefined,hasA&&Object.hasOwn(a,key),hasB&&Object.hasOwn(b,key),[...path,{key,array:Array.isArray(hasA?a:b)}],depth+1);return;}
      }
      const status=!hasA?'added':!hasB?'removed':masked(a)||masked(b)?'unknown':JSON.stringify(a)===JSON.stringify(b)?'same':'changed';
      result.push({path,before:a,after:b,hasBefore:hasA,hasAfter:hasB,status});
    }
    walk(before,after,before!==null&&before!==undefined,after!==null&&after!==undefined,[],0);
    return result;
  }
  function mount(container,before,after,{fields,label,valueText,row}){
    const doc=container.ownerDocument,all=rows(before,after),counts={added:0,removed:0,changed:0,same:0,unknown:0};all.forEach(r=>counts[r.status]++);
    const statusNames={added:'新增字段',removed:'移除字段',changed:'已修改',same:'相同',unknown:'无法比较'};
    const controls=doc.createElement('div');controls.className='diff-controls';
    const summary=doc.createElement('span');summary.textContent=`修改 ${counts.changed} · 新增 ${counts.added} · 移除 ${counts.removed} · 相同 ${counts.same}`+(counts.unknown?` · 无法比较 ${counts.unknown}`:'');
    const toggleLabel=doc.createElement('label'),toggle=doc.createElement('input');toggle.type='checkbox';toggle.checked=true;toggleLabel.append(toggle,doc.createTextNode('仅看差异'));controls.append(summary,toggleLabel);container.append(controls);
    const note=doc.createElement('p');note.className='diff-note';note.textContent='左右同步滚动 · 同一行对应同一字段 · 数组按位置对照'+(before==null?' · 没有前态，本次为首次观察':after==null?' · 后续采集中不再观察到此对象':'')+(counts.unknown?' · 遮罩值不能判断是否相同':'');container.append(note);
    const scroll=doc.createElement('div');scroll.className='snapshot-diff-scroll';scroll.tabIndex=0;scroll.setAttribute('role','region');scroll.setAttribute('aria-label','变化前后字段对比，左右同步滚动');container.append(scroll);
    const table=doc.createElement('table');table.className='snapshot-diff-table';table.innerHTML='<colgroup><col><col><col></colgroup><thead><tr><th scope="col">字段</th><th scope="col">变化前</th><th scope="col">变化后</th></tr></thead><tbody></tbody>';scroll.append(table);
    const body=table.querySelector('tbody');
    function draw(){
      body.replaceChildren();const visible=toggle.checked?all.filter(r=>r.status!=='same'):all;
      if(!visible.length){const tr=doc.createElement('tr'),td=doc.createElement('td');td.colSpan=3;td.textContent='当前可见字段没有差异，可取消“仅看差异”查看全部。';tr.append(td);body.append(tr);return;}
      for(const r of visible){const tr=doc.createElement('tr');tr.className='diff-'+r.status;
        const name=doc.createElement('th');name.scope='row';const path=r.path.map(p=>p.array?`第 ${Number(p.key)+1} 项`:label(p.key));name.textContent=path.join(' / ')||'对象';name.title=r.path.map(p=>p.key).join('.');
        const badge=doc.createElement('small');badge.className='diff-status';badge.textContent=statusNames[r.status];name.append(badge);tr.append(name);
        for(const [has,value] of [[r.hasBefore,r.before],[r.hasAfter,r.after]]){const cell=doc.createElement('td');if(!has){cell.textContent='本侧无此字段';cell.className='diff-missing';}else if(value!==null&&typeof value==='object')cell.append(fields(doc,value));else{const p=doc.createElement('p');p.textContent=row?.type_id&&typeof PCScopeTypePresentation!=='undefined'?PCScopeTypePresentation.format(row,r.path.at(-1)?.key||'',value):valueText(r.path.at(-1)?.key||'',value);cell.append(p);}tr.append(cell);}body.append(tr);
      }
    }
    toggle.onchange=()=>{draw();scroll.scrollTop=0;};draw();
  }
  return {rows,mount};
})();
if(typeof module!=='undefined')module.exports=PCScopeDiff;
