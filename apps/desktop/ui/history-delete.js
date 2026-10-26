'use strict';
const PCScopeHistoryDelete=(()=>{
 function mount(host,{api,retention,onComplete=()=>{},errorText=String}){
  let disposed=false,busy=false,timer=null,operation=null,requestedHistory=null,started=0,pollSerial=0;
  host.innerHTML='<h3>删除全部历史数据</h3><p>清空当前采集记录、对象历史和统计缓存，释放占用空间。保留采集设置，完成后重新记录。</p><button data-delete-open>删除全部历史数据…</button><p role="status" data-delete-status></p><button data-delete-check hidden>检查进度</button><dialog class="monitoring-dialog history-delete-dialog" data-delete-dialog><h2>删除全部历史数据？</h2><p>删除后无法恢复。备份、导出文件和 Windows 系统日志不会被删除。</p><label><input type="checkbox" data-delete-confirm> 我确认清空当前历史数据</label><div class="settings-actions"><button data-delete-cancel>取消</button><button data-delete-submit disabled>立即删除</button></div></dialog>';
  const open=host.querySelector('[data-delete-open]'),dialog=host.querySelector('dialog'),confirm=host.querySelector('[data-delete-confirm]'),submit=host.querySelector('[data-delete-submit]'),message=host.querySelector('[data-delete-status]'),check=host.querySelector('[data-delete-check]');
  const supported=retention.can_delete_history===true&&typeof retention.history_id==='string';
  open.disabled=!supported;if(!supported)message.textContent='更新后台后可使用手动删除。';
  function setBusy(value){busy=value;open.disabled=value||!supported;submit.disabled=value||!confirm.checked;}
  function schedule(){if(!disposed&&Date.now()-started<240000)timer=setTimeout(()=>poll(),1500);else if(!disposed){message.textContent='后台仍在处理，请稍后检查进度。';check.hidden=false;}}
  async function poll(){
   if(disposed)return;clearTimeout(timer);const serial=++pollSerial;
   try{
    const result=await api('history_deletion_status');if(disposed||serial!==pollSerial)return;
    if(requestedHistory&&result.state!=='idle'&&result.history_id!==requestedHistory){setBusy(false);requestedHistory=null;message.textContent='删除未提交，请重新打开历史存储后重试。';check.hidden=false;return 'unmatched';}
    if(operation&&result.id!==operation){message.textContent='删除进度暂未对应，请重新检查。';check.hidden=false;return;}
    if(result.state==='pending'){operation=result.id;setBusy(true);message.textContent='正在结束采集并清空历史，请稍候…';schedule();}
    else if(result.state==='completed'){
     operation=null;requestedHistory=null;check.hidden=true;message.textContent='历史数据已删除，正在重新记录。';await onComplete();if(disposed||serial!==pollSerial)return;setBusy(false);
    }else if(result.state==='failed'){
     operation=null;requestedHistory=null;check.hidden=true;setBusy(false);message.textContent=result.error||'删除未完成，请稍后重试。';
    }else{setBusy(false);check.hidden=true;}
    return result.state;
   }catch(error){if(disposed||serial!==pollSerial)return;if(busy){message.textContent='后台正在切换，正在等待重新连接…';schedule();}else{message.textContent='无法读取删除进度，请重试。';check.hidden=false;}}
  }
  open.onclick=()=>{confirm.checked=false;submit.disabled=true;dialog.showModal();};
  confirm.onchange=()=>{submit.disabled=busy||!confirm.checked;};
  host.querySelector('[data-delete-cancel]').onclick=()=>dialog.close();
  submit.onclick=async()=>{
   if(busy||!confirm.checked)return;
   ++pollSerial;clearTimeout(timer);requestedHistory=retention.history_id;setBusy(true);dialog.close();check.hidden=true;message.textContent='正在准备删除…';started=Date.now();
   try{
    const result=await api('delete_history',{history_id:retention.history_id,confirmation:'DELETE ALL HISTORY'});
    if(disposed)return;operation=result.id;await poll();
   }catch(error){
    if(disposed)return;
    // A lost acknowledgment is ambiguous: inspect progress instead of repeating the deletion.
    message.textContent='正在确认删除进度…';const status=await poll();
    if(status==='idle')message.textContent='删除未提交：'+errorText(error);
   }
  };
  check.onclick=()=>{started=Date.now();check.hidden=true;void poll();};
  if(supported){started=Date.now();void poll();}
  return {dispose(){disposed=true;clearTimeout(timer);if(dialog.open)dialog.close();}};
 }
 return {mount};
})();
