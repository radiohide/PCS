'use strict';
const PCScopeOverview=(()=>{
  function mount(host,{api,getStatus,view,esc,localTime,errorText,navigate}){
    let active=true,pending=null,pendingAutomatic=false,previousRows='',hasRows=false;
    const loaded=new Set(),failures=new Map();
    host.innerHTML=view.overview({},{},{},{});
    const text=(selector,value)=>{const node=host.querySelector(selector);if(node&&node.textContent!==String(value))node.textContent=value;};
    const status=()=>{
      if(!active)return;
      const current=getStatus();
      text('[data-overview-status]',!current?'连接中断':current.collecting?'正在采集':'监控中');
      host.querySelector('.overview-status').dataset.connected=String(Boolean(current));
    };
    host.querySelectorAll('[data-overview-page]').forEach(button=>button.onclick=()=>navigate(button.dataset.overviewPage));
    const sectionError=(name,error,automatic)=>{
      const node=host.querySelector(`[data-overview-error="${name}"]`);
      if(!error){failures.delete(name);node.hidden=true;node.replaceChildren();return;}
      const count=(failures.get(name)||0)+1;failures.set(name,count);
      // Keep the last useful result through a single transient automatic failure.
      if(automatic&&loaded.has(name)&&count<2)return;
      const label={objects:'对象数量',coverage:'采集概况',recent:'最近变化'}[name];
      node.hidden=false;
      node.innerHTML=`<span>${label}${loaded.has(name)?'暂未更新':'读取失败'}。${esc(errorText(error))}</span> <button type="button">重试</button>`;
      node.querySelector('button').onclick=()=>reload();
    };
    async function load({automatic=false}={}){
      status();
      const requests=[
        ['objects',{query:'instance_list',sort:'seen_desc',limit:1,include_counts:false,include_instance_total:true}],
        ['coverage',{query:'source_coverage'}],
        ['recent',{query:'action_timeline',limit:8,cards_only:true}]
      ];
      let complete=true;
      await Promise.all(requests.map(async([name,request])=>{
        try{
          const response=await api('query',{request});
          if(!active||!host.isConnected)return;
          if(response.status&&!['loaded','empty','partial'].includes(response.status))throw Error(view.messageOf(response)||'读取失败');
          if(name==='objects')text('[data-overview-objects]',response.instance_total??response.counts?.instances??'—');
          if(name==='coverage'){
            const rows=response.data||[],sources=new Map();
            for(const row of rows)sources.set(row.source,Boolean(sources.get(row.source)||row.configured));
            const enabled=[...sources.values()].filter(Boolean).length;
            text('[data-overview-enabled]',enabled);
            text('[data-overview-scope]',`${sources.size} 项已登记 · ${sources.size-enabled} 项已暂停`);
            const latest=rows.map(row=>row.last_attempt_at).filter(value=>Number.isFinite(Date.parse(value))).sort((a,b)=>Date.parse(b)-Date.parse(a))[0];
            text('[data-overview-check]',latest?localTime(latest):'尚无检查记录');
          }
          if(name==='recent'){
            const html=view.overviewRecent(response),list=host.querySelector('.overview-actions');
            const reading=host.closest('main').scrollTop>24||host.contains(document.activeElement)||Boolean(window.getSelection()?.toString());
            if(automatic&&hasRows&&html!==previousRows&&reading){complete=false;}
            else if(html!==previousRows){
              // Reconcile by record identity so unchanged rows retain their DOM and focus.
              const template=document.createElement('template');template.innerHTML=html;
              const existing=new Map([...list.children].map(row=>[row.dataset.overviewRow,row]));
              const keep=new Set();let cursor=list.firstElementChild;
              for(const incoming of [...template.content.children]){
                const old=existing.get(incoming.dataset.overviewRow);
                const row=old&&old.outerHTML===incoming.outerHTML?old:incoming;keep.add(row);
                if(row!==cursor)list.insertBefore(row,cursor);cursor=row.nextElementSibling;
              }
              for(const row of [...list.children])if(!keep.has(row))row.remove();
              previousRows=html;hasRows=true;
            }
          }
          loaded.add(name);sectionError(name,null,automatic);
        }catch(error){if(active&&host.isConnected){complete=false;sectionError(name,error,automatic);}}
      }));
      return complete;
    }
    function reload(options={}){
      if(!active)return Promise.resolve(false);
      // A manual click must still apply deferred rows if an automatic read is in flight.
      if(pending&&!options.automatic&&pendingAutomatic)return pending.then(()=>reload(options));
      if(!pending){pendingAutomatic=Boolean(options.automatic);pending=load(options).finally(()=>{pending=null;});}
      return pending;
    }
    status();
    return {reload,status,dispose(){active=false;}};
  }
  return {mount};
})();
