(function(){
  let startupChecked=false, switching=false, startupDialog=false;
  const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
  const escape=value=>String(value??'—').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const request=async(operation,args={})=>{const value=await window.__TAURI__.core.invoke('request',{request:{operation,...args}});if(operation==='monitoring_settings')window.PCScopeIssues?.monitoring(value);return value;};
  const warning='普通采集可能漏记部分系统变化。增强采集需要 Windows 管理员授权。';
  const timeText=(value,fallback='等待检查')=>{if(!value)return fallback;const date=new Date(value);return Number.isNaN(date.getTime())?'时间未识别':date.toLocaleString('zh-CN',{hour12:false});};
  const logName=name=>({Security:'登录与安全记录',System:'系统运行记录',Application:'应用程序记录','Microsoft-Windows-TaskScheduler/Operational':'计划任务记录','Microsoft-Windows-PowerShell/Operational':'脚本运行记录','Microsoft-Windows-Windows Defender/Operational':'安全防护记录','Microsoft-Windows-Sysmon/Operational':'系统活动监控记录'})[name]||'其他系统日志';
  function reportHtml(data){
    const r=data.report||{},settings=data.settings||{};
    const history=data.history?.channels||{};
    const auditIssues=(r.audits||[]).filter(a=>a.success_enabled!==true).length,logIssues=(r.logs||[]).filter(l=>l.enabled!==true||l.readable!==true||l.error||history[l.name]?.ok===false).length;
    const confirmed=Boolean(r.checked_at)&&Boolean((r.audits||[]).length+(r.logs||[]).length)&&!data.checking&&!r.error;
    const impact=data.checking?'正在检查采集权限与日志设置。':r.error?'检查未完成，尚无本次采集范围的检查结果。':!confirmed?'尚无完整的采集权限与日志检查结果。':auditIssues||logIssues?'部分系统变化可能漏记，相关异常可能漏报。':'本次检查未发现读取限制。';
    const yes=value=>value===true?'通过':value===false?'未通过':'未知';
    return `<div class="monitoring-status"><strong>当前后台：${data.elevated?'管理员权限':'普通权限'}</strong><span>保存模式：${settings.mode==='enhanced'?'增强采集':'普通采集'}${settings.configured?'':' · 尚未选择'}</span><span>${data.checking?'正在检查采集能力…':`检查时间：${escape(timeText(r.checked_at))}`}</span></div>
      <p class="notice">${escape(impact)}</p>${r.error?`<details><summary>检查失败详情</summary><pre>${escape(r.error)}</pre></details>`:''}
      <div class="settings-metrics"><div><span>可能漏记的变化类型</span><strong>${confirmed?auditIssues+' 项':'尚无检查结果'}</strong></div><div><span>可能漏记的日志</span><strong>${confirmed?logIssues+' 项':'尚无检查结果'}</strong></div></div><details class="settings-details"><summary>系统记录设置（${(r.audits||[]).length} 项）</summary><div class="object-table-wrap"><table class="table"><thead><tr><th>检查项 / 数据用途</th><th>成功审计</th><th>检查接口 / 问题</th></tr></thead><tbody>${(r.audits||[]).map(a=>`<tr><td>${escape(a.name)}<br><small>${escape(a.purpose)}</small></td><td>${yes(a.success_enabled)}</td><td>${escape(a.source)}${a.error?`<br>${escape(a.error)}`:''}</td></tr>`).join('')}</tbody></table></div></details>
      <details class="settings-details"><summary>日志读取详情（${(r.logs||[]).length} 项）</summary><p>查看当前可读取的日志范围与采集进度。</p><div class="object-table-wrap"><table class="table monitoring-logs"><thead><tr><th>日志</th><th>启用 / 可读</th><th>Windows 保留策略</th><th>现存时间范围 / 数量</th><th>补采状态 / 问题</th></tr></thead><tbody>${(r.logs||[]).map(l=>{
        const h=history[l.name],status=!h?'等待采集':h.ok===false?'采集受阻':h.history_state==='catching_up'?'正在分批补采':h.ok===true?'最近读取完成':'等待采集结果';
        return `<tr><td>${escape(logName(l.name))}<details><summary>日志名称</summary>${escape(l.name)}</details></td><td>启用：${yes(l.enabled)}<br>读取：${yes(l.readable)}</td><td>${escape(({Circular:'满后覆盖最旧日志',AutoBackup:'满后自动归档',Retain:'满后停止写入'})[l.mode]||'未取得保留策略')}<br>${l.maximum_bytes?`${(l.maximum_bytes/1048576).toFixed(1)} MiB`:'—'}</td><td>${escape(timeText(l.oldest_time,'无可读记录'))}<br>至 ${escape(timeText(l.newest_time,'—'))}<br>${escape(l.record_count)} 条</td><td>${status}${l.error||h?.error||(h?.ok===false&&h?.gaps?.length)?`<details><summary>问题详情</summary>${escape(l.error||h?.error||(h?.gaps||[]).join('；'))}</details>`:''}<small>${h?.at?`<br>进度更新：${escape(timeText(h.at))}`:''}</small></td></tr>`;
      }).join('')}</tbody></table></div></details>`;
  }
  function showFailure(message,error){
    const detail=error?.message||String(error);
    const impact=/unable to open database file/i.test(detail)?'记录暂时无法访问，采集和设置可能受影响。':'操作未完成，请刷新检查结果后重试。';
    message.innerHTML=escape(impact)+'<details><summary>问题详情</summary><pre>'+escape(detail)+'</pre></details>';
  }
  const accessDenied=error=>/access.?denied|permission denied|拒绝访问|连接被拒绝|os error 5/i.test(error?.message||String(error));
  async function waitForSwitch(result,message){
    if(!result.switching)return;
    message.textContent='正在连接授权后的后台…';
    const deadline=Date.now()+45000;
    while(Date.now()<deadline){
      await sleep(1000);
      try{const fresh=await request('monitoring_settings');if(fresh.pid===result.pid&&fresh.elevated===result.elevated)return;}
      catch(error){if(accessDenied(error))throw new Error('界面连接被拒绝，监控信息无法更新。请重新打开程序；无需重复申请授权。');}
    }
    throw new Error('未取得新后台的连接状态，请刷新后重试。');
  }
  async function applyMode(mode,repair,message){
    if(switching)throw new Error('正在切换采集权限，请稍候。');
    switching=true;
    try{
    await request('set_monitoring_settings',{mode,repair});
    message.textContent=mode==='enhanced'?'正在申请权限，请在 Windows 授权提示中使用当前账户确认。':'正在切换为普通采集…';
    const result=await window.__TAURI__.core.invoke('switch_service',{mode});
    await waitForSwitch(result,message);
    const actual=await request('monitoring_settings');
    if(actual.elevated!==(mode==='enhanced'))throw new Error('尚未取得所选权限，当前后台继续按原权限运行。');
    return actual;
    }finally{switching=false;}
  }
  async function mount(container){
    container.innerHTML='<h2>启动检测与采集权限</h2><p>正在读取检测结果…</p>';
    const data=await request('monitoring_settings');if(!container.isConnected)return;
    container.innerHTML=`<h2>启动检测与采集权限</h2><p>增强采集能读取更多系统记录。实际范围以检查结果为准。</p><div class="settings-row"><div><label for="monitoring-mode">运行模式</label><p>选择后台采集使用的权限。</p></div><div class="settings-control"><select id="monitoring-mode"><option value="enhanced" ${(!data.settings.configured||data.settings.mode==='enhanced')?'selected':''}>增强采集 · 需要管理员授权</option><option value="low" ${(data.settings.configured&&data.settings.mode==='low')?'selected':''}>普通采集 · 可能漏记系统变化</option></select></div></div><div class="settings-row"><div><label for="monitoring-repair">记录更多系统变化</label><p>增强模式下调整 Windows 记录设置，用于跟踪登录、进程和计划任务变化。</p></div><div class="settings-control"><input role="switch" id="monitoring-repair" type="checkbox" ${(!data.settings.configured||data.settings.repair)?'checked':''}></div></div><details class="settings-details"><summary>将更改哪些 Windows 设置</summary><p>开启后会启用计划任务修改、进程创建和登录成功审计，以及 TaskScheduler/Operational 日志。现有失败审计和日志容量保留原设置。切回低权限不会关闭 Windows 已启用的审计。</p></details><div class="toolbar settings-actions"><button id="monitoring-save" class="primary" ${data.checking?'disabled':''}>保存并应用运行方式</button><button id="monitoring-check" ${data.checking?'disabled':''}>检查并补充日志</button><button id="monitoring-update">刷新检查结果</button></div><p id="monitoring-message" role="status"></p><div id="monitoring-report">${reportHtml(data)}</div>`;
    const mode=container.querySelector('#monitoring-mode'),repair=container.querySelector('#monitoring-repair'),message=container.querySelector('#monitoring-message');
    const sync=()=>{repair.disabled=mode.value==='low';if(repair.disabled)repair.checked=false;};sync();mode.onchange=sync;
    container.querySelector('#monitoring-update').onclick=()=>mount(container).catch(e=>showFailure(message,e));
    container.querySelector('#monitoring-check').onclick=async()=>{try{await request('check_monitoring');await request('collect_plugin',{id:'eventlog'});await mount(container);}catch(e){showFailure(message,e);}};
    container.querySelector('#monitoring-save').onclick=async()=>{
      const button=container.querySelector('#monitoring-save');button.disabled=true;
      try{
        await applyMode(mode.value,repair.checked,message);
        await mount(container);
      }catch(e){showFailure(message,e);message.insertAdjacentHTML('afterbegin','权限切换未完成，请重试。');button.disabled=false;}
    };
    if(data.checking)setTimeout(()=>{if(container.isConnected)mount(container).catch(e=>showFailure(message,e));},2000);
  }
  async function startup(recheck=false){
    if(startupDialog||(startupChecked&&!recheck))return;startupChecked=true;
    try{
      const data=await request('monitoring_settings');
      if(data.settings.configured&&!(data.settings.mode==='enhanced'&&!data.elevated))return;
      const dialog=document.createElement('dialog');dialog.className='monitoring-dialog';
      dialog.innerHTML=`<h2>${data.settings.configured?'增强采集尚未取得权限':'首次启动 · 选择采集权限'}</h2><p>${escape(warning)}</p><p>授权后可读取更多系统信息。额外记录设置可在系统配置中开启。</p><div class="toolbar"><button class="primary" data-config>申请管理员授权</button><button data-low>选择普通采集</button><button data-later>稍后设置</button></div><p role="status"></p>`;
      document.querySelector('#app').append(dialog);dialog.showModal();startupDialog=true;
      const dismiss=()=>{dialog.close();dialog.remove();startupDialog=false;};
      dialog.querySelector('[data-later]').onclick=dismiss;
      dialog.oncancel=event=>{event.preventDefault();dismiss();};
      const choose=async(mode,repair)=>{
        const primary=dialog.querySelector('[data-config]'),low=dialog.querySelector('[data-low]'),message=dialog.querySelector('[role="status"]');
        primary.disabled=low.disabled=true;
        try{await applyMode(mode,repair,message);dismiss();document.querySelector('#load-latest')?.click();}
        catch(error){showFailure(message,error);primary.disabled=low.disabled=false;}
      };
      dialog.querySelector('[data-config]').onclick=()=>choose('enhanced',data.settings.repair===true);
      dialog.querySelector('[data-low]').onclick=()=>choose('low',false);
    }catch{startupChecked=false;return false;}
  }
  window.PCScopeMonitoring={mount,startup,reportHtml,applyMode,get switching(){return switching;}};
})();
