(function(){
  let active='general';
  const tabs=[['general','常规'],['storage','历史存储'],['monitoring','采集权限'],['privacy','隐私显示']];
  function mount(container,{api,esc,localTime,notice,errorText,getTheme,setTheme,onPrivacy}){
    const row=(title,description,control)=>`<div class="settings-row"><div>${title}<p>${description}</p></div><div class="settings-control">${control}</div></div>`;
    container.innerHTML=`<div class="settings-tabs" role="tablist" aria-label="系统配置分类">${tabs.map(([id,label])=>`<button id="settings-tab-${id}" role="tab" aria-controls="settings-${id}" data-settings-tab="${id}">${label}</button>`).join('')}</div>
      <section id="settings-general" class="panel settings-section" role="tabpanel" aria-labelledby="settings-tab-general"><h2>常规</h2>${row('<label for="setting-theme">界面主题</label>','立即应用，并记住你的选择。','<select id="setting-theme"><option value="dark">深色</option><option value="light">浅色</option></select>')}</section>
      <section id="settings-storage" class="panel settings-section" role="tabpanel" aria-labelledby="settings-tab-storage"></section>
      <section id="settings-monitoring" class="panel settings-section monitoring-panel" role="tabpanel" aria-labelledby="settings-tab-monitoring"></section>
      <section id="settings-privacy" class="panel settings-section" role="tabpanel" aria-labelledby="settings-tab-privacy"></section>`;
    const loaded=new Set(['general']),pending=new Set();
    async function load(id){
      if(loaded.has(id)||pending.has(id))return;
      const panel=container.querySelector(`#settings-${id}`);
      pending.add(id);panel.innerHTML='<p role="status">正在读取…</p>';
      try{
        if(id==='storage'){const retention=await api('retention');if(!container.isConnected)return;mountStorage(panel,retention);}
        else if(id==='privacy'){const privacy=await api('privacy_settings');if(!container.isConnected)return;mountPrivacy(panel,privacy);}
        else if(id==='monitoring'){await PCScopeMonitoring.mount(panel);}
        loaded.add(id);
      }catch(error){if(panel.isConnected){
        const title=tabs.find(t=>t[0]===id)[1];
        panel.innerHTML=`<p>${esc(title)}暂时无法读取，其他设置仍可使用。</p><p role="status">${esc(errorText(error))}</p><button>重新读取</button>`;
        panel.querySelector('button').onclick=()=>load(id);
      }}finally{pending.delete(id);}
    }
    const buttons=[...container.querySelectorAll('[data-settings-tab]')];
    function select(id,focus=false){active=id;for(const b of buttons){const chosen=b.dataset.settingsTab===id;b.setAttribute('aria-selected',String(chosen));b.tabIndex=chosen?0:-1;container.querySelector(`#settings-${b.dataset.settingsTab}`).hidden=!chosen;if(chosen&&focus)b.focus();}load(id);}
    for(const [i,b] of buttons.entries()){b.onclick=()=>select(b.dataset.settingsTab);b.onkeydown=e=>{let index;if(e.key==='ArrowRight')index=(i+1)%buttons.length;else if(e.key==='ArrowLeft')index=(i+buttons.length-1)%buttons.length;else if(e.key==='Home')index=0;else if(e.key==='End')index=buttons.length-1;else return;e.preventDefault();select(buttons[index].dataset.settingsTab,true);};}select(active);
    const theme=container.querySelector('#setting-theme');theme.value=getTheme();theme.onchange=()=>setTheme(theme.value);
    function mountStorage(container,retention){
      container.innerHTML=`<h2>历史存储</h2>
      <div class="settings-metrics"><div><span>当前占用</span><strong>${(retention.physical_bytes/1e9).toFixed(3)} GB</strong></div><div><span>已清理记录批次</span><strong>${retention.pruned_units}</strong></div><div><span>最早保留记录</span><strong>${esc(retention.earliest_retained_observation?localTime(retention.earliest_retained_observation):'暂无历史')}</strong></div></div>
      ${row('<label for="retention-mode">保留策略</label>','达到限额后删除最旧记录；无限模式会持续占用磁盘。',`<select id="retention-mode"><option value="rolling" ${retention.mode==='rolling'?'selected':''}>限额 · 滚动删除</option><option value="unlimited" ${retention.mode==='unlimited'?'selected':''}>无限增长</option></select>`)}
      ${row('<label for="quota">容量上限</label>','默认 5 GB。超过上限会清理最旧记录。',`<div class="settings-number"><input id="quota" type="number" min="0.034" max="16000" step="0.001" value="${retention.quota_bytes/1e9}"><span>GB</span></div>`)}
      <details class="settings-details"><summary>空间计算说明</summary><p>当前版本将约三分之一配额用于数据库，其余预留给事务日志与整理。导出文件不计入。无限模式仍受物理磁盘剩余空间限制。</p></details>
      <details class="settings-details" id="migration-status"><summary>数据处理详情</summary><div id="migration-body" role="status">展开后读取。</div></details>
      <div class="settings-actions"><label id="prune-confirm"><input id="allow-prune" type="checkbox"> 我理解启用或降低限额可能立即删除最旧历史。</label><button id="save-retention" class="primary">保存容量策略</button><span role="status" id="retention-feedback"></span></div>`;
    const mode=container.querySelector('#retention-mode'),quota=container.querySelector('#quota'),confirm=container.querySelector('#allow-prune');
    function sync(){quota.disabled=mode.value==='unlimited';const required=mode.value==='rolling'&&(retention.mode!=='rolling'||Number(quota.value)*1e9<retention.quota_bytes);container.querySelector('#prune-confirm').hidden=!required;return required;}
    mode.onchange=()=>{confirm.checked=false;sync();};quota.oninput=()=>{confirm.checked=false;sync();};sync();
    container.querySelector('#save-retention').onclick=async()=>{const button=container.querySelector('#save-retention'),feedback=container.querySelector('#retention-feedback');button.disabled=true;try{const value=mode.value==='unlimited'?retention.quota_bytes/1e9:Number(quota.value);if(!Number.isFinite(value)||value<0.034||value>16000)throw new Error('容量请输入 0.034 至 16000 GB。');if(sync()&&!confirm.checked)throw new Error('请先确认最旧历史可能被清理。');await api('set_retention',{mode:mode.value,quota_bytes:Math.round(value*1e9)});retention.mode=mode.value;retention.quota_bytes=Math.round(value*1e9);sync();feedback.textContent='容量策略已保存';}catch(e){feedback.textContent=errorText(e);}finally{button.disabled=false;}};
    // Loaded when the reader opens it. The plan asks the settings page to keep its four tabs and
    // link the migration state rather than cover the page with internal parameters, and this is
    // that link: one collapsed line, and the detail behind it only for someone asking for it.
    const migration=container.querySelector('#migration-status'),migrationBody=container.querySelector('#migration-body');
    let migrationLoaded=false;
    migration.addEventListener('toggle',async()=>{
      if(!migration.open||migrationLoaded)return;
      migrationBody.textContent='正在读取…';
      try{
        const response=await api('query',{request:{query:'projection_status'}});
        migrationLoaded=true;
        // The rendering belongs to the module that declares the contract paths it reads; this page
        // routes the response to it rather than formatting fields of its own.
        migrationBody.innerHTML=PCScopeModelView.migrationStatus(response);
      }catch(error){migrationBody.textContent=errorText(error);}
    });
    }
    function mountPrivacy(container,privacy){
      container.innerHTML=`<h2>隐私显示</h2>
      ${row('<label for="show-sensitive">显示敏感信息明文</label>','影响详情、时间线、原始数据及复制内容。默认使用 ****** 遮罩。',`<input id="show-sensitive" type="checkbox" role="switch" ${privacy.show_sensitive?'checked':''}>`)}
      <details class="settings-details"><summary>加密保存与显示范围</summary><p>已采集的敏感字段使用当前 Windows 账户加密保存。开启明文显示后，屏幕共享和复制可能暴露这些内容。旧版已丢弃的值无法还原。</p></details>
      <div class="settings-actions"><button id="save-privacy" class="primary">保存显示设置</button><span role="status" id="privacy-feedback"></span></div>`;
    container.querySelector('#save-privacy').onclick=async()=>{const button=container.querySelector('#save-privacy'),feedback=container.querySelector('#privacy-feedback');button.disabled=true;try{const show_sensitive=container.querySelector('#show-sensitive').checked;await api('set_privacy_settings',{show_sensitive});onPrivacy();feedback.textContent=show_sensitive?'已开启明文显示':'已恢复敏感信息遮罩';}catch(e){feedback.textContent=errorText(e);}finally{button.disabled=false;}};
    }
  }
  window.PCScopeSettings={mount,openPermissions(){active='monitoring';}};
})();
