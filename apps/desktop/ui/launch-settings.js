(function(){
  let draft=null;
  async function mount(container){
    const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
    const invoke=(name,args)=>window.__TAURI__.core.invoke(name,args);
    container.innerHTML='<details class="settings-details"><summary>数据与程序位置</summary><div data-launch-body>正在读取…</div></details>';
    const body=container.querySelector('[data-launch-body]');
    const load=async()=>{
      try{
        const result=await invoke('get_launch_settings');if(!body.isConnected)return;
        const values=draft||result.saved;
        body.innerHTML=`<p>位置设置保存在本机，保存后在下次启动时使用。</p>
          <div class="settings-row"><div><label for="launch-data-dir">数据目录</label><p>重新打开程序后显示此目录的记录。原有记录仍保留在原目录。</p></div><div class="settings-control"><input id="launch-data-dir" type="text" spellcheck="false" value="${esc(values.data_dir)}" ${result.overrides.data_dir?'disabled':''}>${result.overrides.data_dir?'<small>由启动参数或环境变量指定。</small>':''}</div></div>
          <div class="settings-row"><div><label for="launch-runtime-dir">后台及采集程序目录</label><p>重新打开程序后，新启动的后台使用此目录；已运行的后台继续使用原程序。</p></div><div class="settings-control"><input id="launch-runtime-dir" type="text" spellcheck="false" value="${esc(values.runtime_dir)}" ${result.overrides.runtime_dir?'disabled':''}>${result.overrides.runtime_dir?'<small>由启动参数或环境变量指定。</small>':''}</div></div>
          <div class="settings-actions"><button id="save-launch-paths" class="primary">保存位置</button><button id="reset-launch-paths">恢复已保存值</button><span id="launch-feedback" role="status">${result.restart_required?'已保存的位置尚未用于本次启动。':''}</span></div>
          <details class="settings-details"><summary>当前使用位置与配置文件</summary><p>本次数据目录：${esc(result.current.data_dir)}</p><p>本次后台启动目录：${esc(result.current.runtime_dir)}</p><p>配置文件：${esc(result.config_file)}</p></details>`;
        const data=body.querySelector('#launch-data-dir'),runtime=body.querySelector('#launch-runtime-dir');
        data.oninput=runtime.oninput=()=>{draft={data_dir:data.value,runtime_dir:runtime.value};};
        body.querySelector('#reset-launch-paths').onclick=()=>{draft=null;load();};
        body.querySelector('#save-launch-paths').onclick=async()=>{
          const button=body.querySelector('#save-launch-paths'),feedback=body.querySelector('#launch-feedback');button.disabled=true;
          try{
            const saved=await invoke('set_launch_settings',{dataDir:data.value.trim(),runtimeDir:runtime.value.trim()});
            draft=null;if(!body.isConnected)return;
            feedback.textContent=saved.restart_required?'已保存。重新打开程序后生效，已运行的后台不会自动替换。':'位置已保存。';
          }catch(error){if(body.isConnected)feedback.textContent=error?.message||String(error);}
          finally{if(body.isConnected)button.disabled=false;}
        };
      }catch(error){if(body.isConnected){body.innerHTML=`<p>${esc(error?.message||String(error))}</p><button data-launch-retry>重新读取</button>`;body.querySelector('[data-launch-retry]').onclick=load;}}
    };
    await load();
  }
  window.PCScopeLaunchSettings={mount};
})();
