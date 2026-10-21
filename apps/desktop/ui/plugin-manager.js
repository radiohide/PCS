'use strict';
const pluginDescriptions={
  'pcscope-software.exe':{title:'软件安装信息',data:'软件名称、版本、发布者、安装位置和登记安装日期；覆盖本机 32/64 位卸载登记。',meaning:'发现软件安装信息和版本的变化。',limit:'采集范围为软件安装登记。'},
  'pcscope-powershell-adapter.exe':{title:'基础系统与资源管理器扩展',data:'操作系统版本、启动时间、账户与组、服务和计划任务、网卡地址、卷、硬件、虚拟化能力；Shell 预览、图标及右键扩展登记。',meaning:'可发现账户启停、组成员变化、服务状态变化及 Shell 扩展注册变化。',limit:'按次检查扩展登记和账户权限配置。'},
  system:{title:'系统、软件与账户扩展',data:'Windows 更新历史、待重启状态、还原点、AppX 与可选组件、用户配置文件、登录会话、分配权限、进程和资源采样。',meaning:'可发现组件、会话、权限分配及进程清单变化，保留来源提供的更新记录。',limit:'短命进程和两次采集之间的会话可能遗漏；快照无法单独证明精确登录登出时间。'},
  security:{title:'安全配置与证书',data:'安全产品状态、Defender 设置与检测记录、Device Guard、TPM、Secure Boot、BitLocker 状态、证书元数据、策略及日志通道覆盖。',meaning:'可发现防护开关、策略、证书与加密状态变化。',limit:'仅查询，不开启审计或修改安全设置；无权限会报告缺口，不采集私钥和恢复密钥。'},
  network:{title:'网络、防火墙与共享',data:'网络配置、DNS 与路由、防火墙规则、连接监听、代理及共享相关配置。',meaning:'可发现地址、路由、规则、代理和共享配置变化，以及采集时存在的连接。',limit:'不采集通信正文；两次检查之间的短连接可能遗漏。'},
  storage:{title:'磁盘、驱动与电源',data:'磁盘与分区、设备驱动绑定和驱动服务、BIOS、磁盘健康、电池及电源计划。',meaning:'可发现分区配置、驱动版本与绑定、电源计划和设备状态变化。',limit:'健康值由系统与设备提供；无法据此保证硬件无故障。'},
  startup:{title:'服务、任务与高级启动入口',data:'服务与计划任务详细配置、登录启动登记，以及高级自动启动相关注册配置。',meaning:'可发现新增启动入口、执行目标及运行账户变化。',limit:'采集启动配置，执行记录来自事件日志。'},
  desktop:{title:'打印、输入与桌面配置',data:'打印机、驱动、端口和打印队列，输入法、剪贴板设置、桌面与资源管理器设置、快捷方式。',meaning:'可发现打印设备、输入组件、桌面配置和快捷方式目标变化。',limit:'不采集剪贴板正文、打印文档内容或屏幕画面；队列快照可能遗漏已结束作业。'},
  virtualization:{title:'Hyper-V 与 WSL',data:'Hyper-V 与 WSL 可用组件、配置及来源允许读取的虚拟环境状态。',meaning:'可发现虚拟环境登记及配置变化。',limit:'适用于 Hyper-V 和 WSL；需要相应组件及读取权限。'},
  applications:{title:'浏览器与开发工具扩展',data:'浏览器扩展和策略、VS Code 扩展、.NET 运行时及标准位置的全局 npm 包登记。',meaning:'可发现扩展、运行时和开发工具包安装清单或版本变化。',limit:'检查标准安装目录；自定义目录需另行配置。不读取浏览内容。'},
  scoped:{title:'指定文件与运行中的程序',data:'列表内文件的基本信息、访问权限、附加数据和可选内容摘要；便携软件、程序已加载的组件，以及可选容器和代理软件信息。',meaning:'发现指定文件、访问权限和程序组件的变化。',limit:'文件列表默认空，在详细设置中添加。其他可选功能由程序配置文件控制；不保存文件正文版本。'},
  eventlog:{title:'事件日志',data:'按 23 个来源分组展示细分采集项，与时间线标题及筛选名称一致。包括事件时间、提供程序、事件编号及保存的字段。每轮读取 System、Application 和轮换的一个扩展通道，每通道最多 128 条。',meaning:'依据来源可提供登录登出、任务执行、进程或系统更新等动作证据；具体含义需结合提供程序和事件编号。',limit:'周期表示一轮增量读取，不是每个通道的更新周期，也不是即时事件触发。未启用审计、权限不足或日志覆盖会形成缺口。'},
  usn:{title:'文件变化记录',data:'读取 Windows 文件变更日志，跟踪列表内文件的新增、删除、改名和写入。',meaning:'发现指定文件的新增、删除、改名和写入记录。',limit:'仅适用于 NTFS 磁盘，需要磁盘读取权限。列表默认空；父文件夹被移动时可能漏记。不保存文件正文，也不创建或清空 Windows 文件变更日志。'}
};
let collectionItemControls=[];
// Which object types each source feeds, and what those types declare. Generated from the source
// map, the catalog and the adapter coverage by scripts/build-source-type-map.cjs, so the page
// cannot drift from them by being maintained separately.
let sourceTypes=new Map();
const sourceTypesReady=fetch('source-type-map.json').then(r=>{if(!r.ok)throw new Error('无法读取来源类型映射');return r.json();}).then(document=>{for(const entry of document.sources)sourceTypes.set(entry.source,entry);}).catch(()=>{sourceTypes=new Map();});
/// What one source provides, as the generated map states it.
///
/// A source with no handling path says so. That is the honest answer: it is collected and its
/// material is retained, but nothing turns it into objects yet, and a blank cell would read as a
/// source that provides nothing at all.
function providedTypes(source){
  const entry=sourceTypes.get(source);
  if(!entry)return '<small>来源映射中没有该来源</small>';
  const handling={object_adapter:'',action_adapter:'<small>处理为来源明确记录的动作，保留结果与原始证据</small>',source_reading:'<small>处理为采集范围或资源采样，在采集健康页查询，不计为对象</small>',virtual_group:'<small>实际按盘来源的汇总前缀，不是独立采集来源</small>',event_adapter:'<small>按事件适配器处理：产出动作，目标类型由动作在目录中的声明决定</small>',unmapped:'<small class="muted">尚未接入：素材留存，但还没有处理路径</small>'}[entry.handling]||'';
  if(!entry.types.length)return handling||'<small>未映射到对象类型</small>';
  return entry.types.map(type=>`<div>${esc(type.name)}<small>${esc(type.category)} · ${type.fields.length} 个字段 · ${type.actions.length} 个动作</small></div>`).join('')+handling;
}
function pluginSourceTable(id){
  const rows=pluginSources[id]||[];
  return `<details class="plugin-source-details"><summary>选择具体采集项（${rows.length} 项）</summary><p class="plugin-source-note">开关即时保存，关闭后保留历史和最近状态，但可能漏记之后的变化。重新开启不会自动补齐之前遗漏的日志。最近一次是否成功读取，请查看执行结果。</p><div class="plugin-source-scroll"><table class="plugin-source-table"><caption class="sr-only">${esc(pluginDescriptions[id]?.title||'扩展采集')}采集来源</caption><colgroup><col><col><col><col><col></colgroup><thead><tr><th scope="col">采集项名称</th><th scope="col">采集开关</th><th scope="col">数据来源</th><th scope="col">采集方式</th><th scope="col">提供哪些信息</th></tr></thead><tbody>${rows.map(row=>{const control=collectionItemControls.find(c=>c.plugin===id&&c.name===row.name);return `<tr><th scope="row">${esc(row.name)}</th><td>${control?`<button type="button" role="switch" class="refresh-switch item-switch" data-item-id="${esc(control.id)}" data-item-plugin="${esc(id)}" aria-label="采集${esc(row.name)}" aria-checked="${control.enabled}"><span data-item-state aria-hidden="true">${control.enabled?'开':'关'}</span><span class="refresh-switch-thumb" aria-hidden="true"></span></button>`:'<small>跟踪文件变化时需要</small>'}</td><td><details><summary>技术详情</summary><p>${esc(row.source)}</p><p>${esc(row.api)}</p></details></td><td>${esc(row.method)}</td><td>${providedTypes(row.source)}</td></tr>`;}).join('')}</tbody></table></div></details>`;
}
function fileWatchEditor(id,watch){
  if(!['usn','scoped'].includes(id))return '';
  return `<section class="file-watch-editor"><h3>文件监测列表</h3><p>用于跟踪文件属性和变化。默认空列表，不监测任何文件。每行一个本机文件完整路径，最多 100 个；不递归目录，不支持通配符。尚不存在的文件也可添加。</p><label>要监测的文件<textarea data-watch-list rows="5" spellcheck="false" placeholder="例如 C:\\Users\\name\\Documents\\example.txt">${esc(watch.files.join('\n'))}</textarea></label><div class="toolbar"><button data-watch-save>保存文件列表</button><button data-watch-clear>清空列表草稿</button><span data-watch-feedback role="status">已保存 ${watch.files.length} 个文件${watch.files.length?'':' · 文件监测范围为空'}</span></div><p>保存后用于下一次采集；进行中的旧列表结果不会继续入库。移除仅停止后续监测，保留已有历史。新列表从首次读取时开始跟踪文件变化；文件属性由“指定文件与运行中的程序”采集，可分别设置周期或手动采集。</p></section>`;
}
function mountModuleMonitorSettings(panel){
  panel.insertAdjacentHTML('beforebegin','<section class="panel status-panel" id="module-monitor-settings"><h2>程序模块文件监控</h2><p>跟踪程序已加载的组件文件，发现可能影响下次启动或使用的文件异常。</p><div data-module-settings-body>正在读取监控范围…</div></section>');
  const section=content.querySelector('#module-monitor-settings'),body=section.querySelector('[data-module-settings-body]');
  const load=async()=>{
    try{
      const config=await api('runtime_watch_settings');if(!section.isConnected)return;
      body.innerHTML=`<label><input data-module-enabled type="checkbox" ${config.enabled?'checked':''}>启用模块文件监控</label><details><summary>监控范围</summary><label>程序文件名（逗号分隔）<input data-module-names maxlength="4096" value="${esc((config.module_process_names||[]).join(', '))}"></label><p>仅监控当前账户和会话。Windows 桌面程序的文件名是 sihost.exe。</p></details><p class="muted">定期检查随“指定文件与运行中的程序”的周期调整，文件变化仍及时检查。只覆盖曾成功发现的模块文件。首次就无法加载的组件可能遗漏。</p><button data-module-save>保存监控范围</button><span data-module-feedback role="status"></span><p class="muted">检查记录在时间线查看，读取问题在采集健康查看。</p>`;
      body.querySelector('[data-module-save]').onclick=async event=>{
        const button=event.currentTarget,feedback=body.querySelector('[data-module-feedback]');button.disabled=true;feedback.textContent='正在保存…';
        try{
          const names=body.querySelector('[data-module-names]').value.split(/[,，\n]/).map(value=>value.trim()).filter(Boolean);
          await api('set_runtime_watch_settings',{enabled:body.querySelector('[data-module-enabled]').checked,module_process_names:names});
          feedback.textContent='监控范围已保存。';
        }catch(error){feedback.textContent=errorText(error);}finally{button.disabled=false;}
      };
    }catch(error){if(section.isConnected){body.innerHTML=`<p>${esc(errorText(error))}</p><button data-module-retry>重试读取监控范围</button>`;body.querySelector('[data-module-retry]').onclick=load;}}
  };
  load();
}
async function renderPluginManager(){
  await sourceTypesReady;
  const [jobs,watch,controls,directoryWatch]=await Promise.all([api('plugins'),api('file_watch_settings'),api('item_controls'),api('directory_watch_settings')]);collectionItemControls=controls.items;if(state.page!=='plugins')return;
  content.innerHTML=head('采集插件','选择要监控的项目。暂停后保留记录，但可能漏记后续变化。')+`<details><summary>采集周期与执行方式</summary><p class="plugin-plan-note">状态采集最低 30 秒，日志采集最低 10 秒，最长 7 天。状态任务与日志任务各自依次执行；繁忙时单次触发会提示重试。已在执行的任务会完成，尚未开始的自动任务可被停用。</p></details><div id="plugin-manager">${jobs.map((job,i)=>{const d=pluginDescriptions[job.id]||{title:'扩展采集',data:'由插件提供',meaning:'采集范围尚未说明，请展开查看来源。',limit:'未知来源需核对原始记录'};return `<section class="panel plugin-card" data-plugin-index="${i}"><div class="plugin-heading"><h2>${esc(d.title)}</h2><span class="badge" data-enabled-state></span><button data-toggle></button><button data-details aria-expanded="false" aria-controls="plugin-details-${i}">详细</button></div><p data-status-impact role="status"></p><p class="muted">${esc(d.meaning)}</p><div data-toggle-feedback role="status" hidden></div><div id="plugin-details-${i}" class="plugin-details" hidden><div class="plugin-detail-meta"><small>${esc(job.id)}</small><span class="badge" data-phase></span></div><dl class="plugin-description"><dt>会产生哪些数据</dt><dd>${esc(d.data)}</dd><dt>记录代表什么</dt><dd>${esc(d.meaning)}</dd><dt>覆盖范围与限制</dt><dd>${esc(d.limit)}</dd></dl><div class="plugin-controls"><label>采集周期（秒）<input data-seconds type="number" min="${job.minimum_seconds}" max="604800" step="1" value="${job.seconds}"></label><button data-save>保存周期</button><button class="primary" data-run>采集一次</button><span data-feedback role="status"></span></div><div class="plugin-runtime" data-runtime></div>${fileWatchEditor(job.id,watch)}${pluginSourceTable(job.id)}<details class="plugin-run-detail"><summary>最近执行结果</summary><div data-result></div></details></div></section>`;}).join('')}</div>`;
  const panel=content.querySelector('#plugin-manager');let polling=false, scheduleSaving=false, scheduleEpoch=0;
  panel.insertAdjacentHTML('beforebegin','<section class="panel status-panel" id="global-collection-schedule"><h2>全局采集周期</h2><p>应用到所有采集项；之后单独修改某一项，以该项最新设置为准。</p><div class="plugin-controls"><label>采集周期（秒）<input data-global-seconds type="number" min="30" max="604800" step="1" placeholder="尚未统一设置"></label><button data-global-save disabled>应用到全部</button><span data-global-feedback role="status">正在读取…</span></div><p class="muted">范围：30 秒至 7 天。保留各项开关，正在执行的采集完成后使用新周期。</p></section>');
  const globalPanel=content.querySelector('#global-collection-schedule'),globalInput=globalPanel.querySelector('[data-global-seconds]'),globalButton=globalPanel.querySelector('[data-global-save]'),globalFeedback=globalPanel.querySelector('[data-global-feedback]');
  let globalSeconds=null, globalReady=false;
  const globalSummary=()=>{const count=globalSeconds==null?0:jobs.filter(job=>job.seconds!==globalSeconds).length;globalFeedback.textContent=globalSeconds==null?'尚未统一设置，使用各项周期':count?'上次统一设置：'+globalSeconds+' 秒 · '+count+' 项已单独调整':'全部采集项：'+globalSeconds+' 秒';};
  const scheduleBusy=value=>{scheduleSaving=value;globalButton.disabled=value||!globalReady;panel.querySelectorAll('[data-save],[data-toggle]').forEach(button=>button.disabled=value);};
  api('global_collection_schedule').then(settings=>{if(!panel.isConnected)return;globalSeconds=settings.seconds;globalInput.value=globalSeconds??'';globalReady=true;globalButton.disabled=scheduleSaving;globalSummary();}).catch(error=>{if(panel.isConnected)globalFeedback.textContent='全局周期暂时无法读取：'+errorText(error);});
  mountModuleMonitorSettings(panel);
  const draw=rows=>{if(!panel.isConnected)return;for(const [i,job] of rows.entries()){
    const card=panel.querySelector(`[data-plugin-index="${i}"]`);if(!card||jobs[i].id!==job.id)continue;
    Object.assign(jobs[i],job);
    if(card.dataset.dirty!=='true')card.querySelector('[data-seconds]').value=job.seconds;
    card.querySelector('[data-enabled-state]').textContent=job.enabled?'已启用':'已停用';
    card.querySelector('[data-enabled-state]').classList.toggle('accent',job.enabled);
    card.querySelector('[data-toggle]').textContent=job.enabled?'停用':'启用';
    card.querySelector('[data-toggle]').setAttribute('aria-label',`${job.enabled?'停用':'启用'}${pluginDescriptions[job.id]?.title||'扩展采集'}`);
    card.querySelector('[data-phase]').textContent=job.phase==='running'?'执行中':job.phase==='queued'?'等待执行':job.enabled?'自动采集已启用':'自动采集已停用';
    const last=job.last_result;
    card.querySelector('[data-status-impact]').textContent=!job.enabled?'已暂停，后续变化可能漏记。':job.phase==='running'?'正在采集，最新变化可能稍后显示。':!last?'尚未完成检查，暂不能判断监控范围。':last.ok?'最近一次采集完成。':'部分信息未取得，相关异常可能漏报。';
    card.querySelector('[data-runtime]').textContent=`已保存周期：${job.seconds} 秒 · 上次开始：${job.last_started?localTime(job.last_started):'尚未执行'} · 上次结束：${job.last_finished?localTime(job.last_finished):'—'} · 下次计划：${job.next_at?localTime(job.next_at):job.phase!=='idle'?'本轮结束后计算':'已停用'}${last?.duration_ms!=null?` · 耗时 ${(last.duration_ms/1000).toFixed(1)} 秒`:''}`;
    card.querySelector('[data-run]').disabled=job.phase!=='idle';
    const result=card.querySelector('[data-result]');
    result.innerHTML=last?`<p>${last.ok?'本次采集完成':'部分信息未取得，相关异常可能漏报'}${last.changes!=null?` · 新增 ${last.changes} 条记录`:''}</p><pre>${esc(JSON.stringify(last,null,2))}</pre>`:'<p>当前后台启动后尚未执行。</p>';
  }};
  const refresh=async(automatic=false)=>{if(polling){if(automatic)return;await polling;}if(!panel.isConnected)return;if(scheduleSaving)return;const epoch=state.refreshEpoch, capturedScheduleEpoch=scheduleEpoch;polling=(async()=>{try{const [rows,controls]=await Promise.all([api('plugins'),api('item_controls')]);if(capturedScheduleEpoch===scheduleEpoch&&(!automatic||(state.autoRefresh&&epoch===state.refreshEpoch))){draw(rows);drawItems(controls.items);notice('');}}catch(error){if(panel.isConnected&&(!automatic||(state.autoRefresh&&epoch===state.refreshEpoch)))notice(errorText(error));}})();try{await polling;}finally{polling=false;}};
  const drawItems=items=>{collectionItemControls=items;panel.querySelectorAll('[data-item-id]').forEach(button=>{const item=items.find(i=>i.id===button.dataset.itemId);if(!item)return;button.setAttribute('aria-checked',String(item.enabled));button.querySelector('[data-item-state]').textContent=item.enabled?'开':'关';});};
  panel.querySelectorAll('[data-item-id]').forEach(button=>{button.onclick=async()=>{button.disabled=true;const feedback=button.closest('.plugin-card').querySelector('[data-feedback]');try{const result=await api('set_item_enabled',{plugin:button.dataset.itemPlugin,id:button.dataset.itemId,enabled:button.getAttribute('aria-checked')!=='true'});drawItems(result.items);feedback.textContent='采集项设置已保存';}catch(error){feedback.textContent=errorText(error);}finally{button.disabled=false;}};});
  draw(jobs);
  globalInput.oninput=()=>{globalFeedback.textContent='有未保存修改';};
  globalButton.onclick=async()=>{
    const seconds=Number(globalInput.value);
    if(!Number.isInteger(seconds)||seconds<30||seconds>604800){globalFeedback.textContent='请输入 30–604800 之间的整数秒数';return;}
    if(scheduleSaving)return;scheduleEpoch++;scheduleBusy(true);globalFeedback.textContent='正在保存…';
    try{const result=await api('set_global_collection_schedule',{seconds});if(!panel.isConnected)return;
      panel.querySelectorAll('[data-plugin-index]').forEach(card=>{card.dataset.dirty='false';card.querySelector('[data-feedback]').textContent='';});
      globalSeconds=result.settings.seconds;globalInput.value=globalSeconds;draw(result.jobs);globalSummary();
    }catch(error){globalFeedback.textContent=errorText(error);}finally{scheduleBusy(false);}
  };
  panel.querySelectorAll('.file-watch-editor').forEach(editor=>{
    // Only the scoped collector reads directories; USN's explicit file list remains separate.
    const card=editor.closest('[data-plugin-index]');
    if(card && jobs[Number(card.dataset.pluginIndex)].id==='scoped'){
      editor.insertAdjacentHTML('beforeend','<h3>目录监测列表</h3><p>每行一个本机目录完整路径，默认空。采集目录属性、权限与直接子项变化，不递归、不读取正文；周期沿用本插件。</p><textarea data-directory-list rows="4" spellcheck="false">'+esc(directoryWatch.directories.join('\n'))+'</textarea><button data-directory-save>保存目录列表</button><span data-directory-feedback role="status"></span>');
      editor.querySelector('[data-directory-save]').onclick=async event=>{
        event.target.disabled=true;const feedback=editor.querySelector('[data-directory-feedback]');
        try{const directories=editor.querySelector('[data-directory-list]').value.split(/\r?\n/).map(value=>value.trim()).filter(Boolean);const saved=await api('set_directory_watch_settings',{directories});editor.querySelector('[data-directory-list]').value=saved.directories.join('\n');feedback.textContent='已保存 '+saved.directories.length+' 个目录';}catch(error){feedback.textContent=errorText(error);}finally{event.target.disabled=false;}
      };
    }
    const list=editor.querySelector('[data-watch-list]'),feedback=editor.querySelector('[data-watch-feedback]');
    list.oninput=()=>{feedback.textContent='文件列表有未保存修改';};
    editor.querySelector('[data-watch-clear]').onclick=()=>{list.value='';feedback.textContent='列表草稿已清空，保存后停止文件监测';};
    editor.querySelector('[data-watch-save]').onclick=async e=>{
      e.target.disabled=true;
      try{
        const files=list.value.split(/\r?\n/).map(p=>p.trim()).filter(Boolean);
        const saved=await api('set_file_watch_settings',{files});
        panel.querySelectorAll('.file-watch-editor').forEach(other=>{other.querySelector('[data-watch-list]').value=saved.files.join('\n');other.querySelector('[data-watch-feedback]').textContent=`已保存 ${saved.files.length} 个文件${saved.files.length?'，下一次采集使用此列表':'，已停止后续文件变更采集'}`;});
      }catch(error){feedback.textContent=errorText(error);}finally{e.target.disabled=false;}
    };
  });
  panel.querySelectorAll('[data-plugin-index]').forEach(card=>{
    const job=jobs[Number(card.dataset.pluginIndex)],feedback=card.querySelector('[data-feedback]');
    card.querySelector('[data-details]').onclick=e=>{
      const expanded=e.currentTarget.getAttribute('aria-expanded')!=='true';
      e.currentTarget.setAttribute('aria-expanded',String(expanded));
      e.currentTarget.textContent=expanded?'收起':'详细';
      card.querySelector('.plugin-details').hidden=!expanded;
    };
    card.querySelector('[data-toggle]').onclick=async e=>{
      const message=card.querySelector('[data-toggle-feedback]');
      if(scheduleSaving)return;scheduleEpoch++;scheduleBusy(true);message.hidden=true;
      try{
        const enabled=!job.enabled;
        const result=await api('set_plugin',{id:job.id,enabled});
        draw(result.jobs);globalSummary();
      }catch(error){message.textContent=errorText(error);message.hidden=false;}
      finally{scheduleBusy(false);}
    };
    card.querySelectorAll('input').forEach(input=>input.oninput=()=>{card.dataset.dirty='true';feedback.textContent='有未保存修改';});
    card.querySelector('[data-save]').onclick=async e=>{
      const seconds=Number(card.querySelector('[data-seconds]').value);
      if(!Number.isInteger(seconds)||seconds<job.minimum_seconds||seconds>604800){feedback.textContent=`请输入 ${job.minimum_seconds}–604800 之间的整数秒数`;return;}
      if(scheduleSaving)return;scheduleEpoch++;scheduleBusy(true);try{const result=await api('set_plugin',{id:job.id,seconds});card.dataset.dirty='false';draw(result.jobs);globalSummary();feedback.textContent='已保存，使用此项新周期';}catch(error){feedback.textContent=errorText(error);}finally{scheduleBusy(false);}
    };
    card.querySelector('[data-run]').onclick=async e=>{e.target.disabled=true;try{await api('collect_plugin',{id:job.id});feedback.textContent='已提交单次采集';await refresh();}catch(error){feedback.textContent=errorText(error);e.target.disabled=false;}};
  });
  state.reloadPage=()=>refresh();
  const timer=setInterval(()=>{if(!panel.isConnected){clearInterval(timer);return;}if(state.autoRefresh&&!state.loadingLatest)refresh(true);},3000);
}
