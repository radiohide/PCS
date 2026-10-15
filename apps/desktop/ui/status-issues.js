(function(root){
  function createModel(changed=()=>{}){
    const entries=new Map(),collection=new Map(),channels=new Map();
    const detail=value=>typeof value==='string'?value:JSON.stringify(value??'',null,2);
    const set=(id,issue)=>{const value={id,...issue,busy:issue.busy??entries.get(id)?.busy??false};if(JSON.stringify(entries.get(id))!==JSON.stringify(value)){entries.set(id,value);changed();}};
    const remove=id=>{if(entries.delete(id))changed();};
    const group=(id,failures,title,impact)=>{
      if(failures.size)set(id,{title,impact,detail:[...failures].map(([name,value])=>name+'：'+detail(value)).join('\n'),action:'health',actionLabel:'查看采集健康'});
      else remove(id);
    };
    function syncStatus(status){
      remove('backend');
      const last=status.last_collection;
      if(Array.isArray(last?.plugins)){
        for(const item of last.plugins){if(item.ok===false)collection.set(item.plugin,item.error||item.gaps||'读取未完成');else if(item.ok===true)collection.delete(item.plugin);}
        if(last.ok===true)collection.delete('采集任务');
        group('collection',collection,'部分监控信息未取得','相关系统变化可能漏记。');
      }else if(last?.ok===false){collection.set('采集任务',last.error||'读取未完成');group('collection',collection,'部分监控信息未取得','相关系统变化可能漏记。');}
      else if(last?.ok===true){collection.delete('采集任务');group('collection',collection,'部分监控信息未取得','相关系统变化可能漏记。');}
      for(const [name,result] of Object.entries(status.event_collection?.channels||{})){
        if(result?.ok===false)channels.set(name,result.error||result.gaps||'读取未完成');else if(result?.ok===true)channels.delete(name);
      }
      group('logs',channels,'部分系统日志未读取','登录、程序运行等变化可能漏记。');
      const files=status.usn_collection;
      if(files?.ok===false)set('files',{title:'文件变化记录未读取',impact:'部分文件变化可能漏记。',detail:detail(files.error||files.gaps),action:'health',actionLabel:'查看采集健康'});
      else if(files?.ok===true)remove('files');
      const projection=status.projection_worker;
      if(projection?.state==='retrying')set('processing',{title:'记录处理遇到问题',impact:'部分最新变化暂时无法显示，后台正在重试。',detail:detail(projection.error),action:'health',actionLabel:'查看采集健康'});
      else if(['idle','projecting'].includes(projection?.state))remove('processing');
    }
    function monitoring(data){
      if(data.settings?.configured===false)set('permissions',{title:'尚未选择采集权限',impact:'监控范围尚未设置。',action:'permissions',actionLabel:'选择采集权限'});
      else if(data.settings?.mode==='enhanced'&&data.elevated===false)set('permissions',{title:'增强采集尚未获授权',impact:'部分系统信息无法读取。',action:'permissions',actionLabel:'检查采集权限'});
      else if(data.elevated===true||data.settings?.mode==='low')remove('permissions');
      if(!data.checking&&data.report?.error)set('permission-check',{title:'采集检查未完成',impact:'暂时无法确认监控范围。',detail:detail(data.report.error),action:'permissions',actionLabel:'查看检查结果'});
      else if(!data.checking&&data.report?.checked_at)remove('permission-check');
    }
    return {set,remove,syncStatus,monitoring,list:()=>[...entries.values()],setBusy(id,busy){const issue=entries.get(id);if(issue)set(id,{...issue,busy});}};
  }
  function mount({button,panel,onAction}){
    const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
    const body=panel.querySelector('[data-issues-list]'),close=panel.querySelector('[data-issues-close]');
    function hide(){panel.hidden=true;button.setAttribute('aria-expanded','false');if(!button.hidden)button.focus();else document.querySelector('#operation').focus();}
    function draw(){
      const issues=model.list(),focused=panel.contains(document.activeElement)?document.activeElement:null;
      const focusedId=focused?.closest('[data-issue]')?.dataset.issue;
      const expanded=new Set([...body.querySelectorAll('details[open]')].map(el=>el.closest('[data-issue]').dataset.issue));
      button.hidden=!issues.length;button.textContent=`⚠ ${issues.length} 项需要处理`;
      if(!issues.length){if(!panel.hidden){panel.hidden=true;button.setAttribute('aria-expanded','false');if(focused)document.querySelector('#operation').focus();}body.innerHTML='';return;}
      body.innerHTML=issues.map(issue=>`<article data-issue="${esc(issue.id)}"><h3>${esc(issue.title)}</h3><p>${esc(issue.impact)}</p><button data-issue-action ${issue.busy?'disabled':''}>${esc(issue.busy?'正在处理…':issue.actionLabel||'查看')}</button>${issue.detail?`<details ${expanded.has(issue.id)?'open':''}><summary>问题详情</summary><pre>${esc(issue.detail)}</pre></details>`:''}</article>`).join('');
      for(const action of body.querySelectorAll('[data-issue-action]'))action.onclick=async()=>{
        const id=action.closest('[data-issue]').dataset.issue,issue=model.list().find(item=>item.id===id);if(!issue||issue.busy)return;
        model.setBusy(id,true);
        try{await onAction(issue.action);}
        catch(error){if(model.list().some(item=>item.id===id))model.set(id,{...issue,detail:error?.message||String(error)});}
        finally{model.setBusy(id,false);}
      };
      if(focusedId){const row=[...body.querySelectorAll('[data-issue]')].find(row=>row.dataset.issue===focusedId);(row?.querySelector(focused?.tagName==='SUMMARY'?'summary':'[data-issue-action]')||close).focus();}
    }
    const model=createModel(draw);
    button.onclick=()=>{if(!panel.hidden){hide();return;}panel.hidden=false;button.setAttribute('aria-expanded','true');close.focus();};
    close.onclick=hide;
    panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();hide();}});
    return {...model,hide};
  }
  root.PCScopeStatusIssues={createModel,mount};
  if(typeof module!=='undefined')module.exports={createModel};
})(typeof window!=='undefined'?window:globalThis);
