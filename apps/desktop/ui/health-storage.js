'use strict';
const PCScopeHealthStorage=(()=>{
  function bytes(value){
    if(typeof value!=='number'||!Number.isFinite(value)||value<0)return '暂未取得';
    const units=['B','KB','MB','GB','TB'];let i=0;
    while(value>=1000&&i<units.length-1){value/=1000;i++;}
    return `${value.toLocaleString('zh-CN',{maximumFractionDigits:i?2:0})} ${units[i]}`;
  }
  function mount(host,{api,localTime,canRefresh=()=>true}){
    let active=true,pending=null,lastAttempt=0,loaded=false,failures=0;
    host.innerHTML='<div class="settings-metrics health-storage-metrics"><div><span>历史数据占用</span><strong data-storage-bytes>正在读取…</strong></div><div><span>保留策略</span><strong data-storage-policy>正在读取…</strong></div></div><p class="muted" data-storage-impact></p><small class="muted" data-storage-checked></small><div data-storage-error role="status" hidden><span>存储占用读取失败，请重试。</span> <button type="button">重试</button></div><details class="settings-details"><summary>统计范围</summary><p>包含当前历史数据库及其写入缓存，不含备份、导出文件和程序文件。按文件大小统计，1 GB = 10 亿字节。</p></details>';
    const text=(selector,value)=>{const node=host.querySelector(selector);if(node.textContent!==value)node.textContent=value;};
    const error=host.querySelector('[data-storage-error]');
    function reload({automatic=false}={}){
      if(!active)return Promise.resolve();
      if(pending)return pending;
      lastAttempt=Date.now();
      pending=(async()=>{
        try{
          const data=await api('retention');
          if(!active||!host.isConnected)return;
          if(typeof data?.physical_bytes!=='number'||!Number.isFinite(data.physical_bytes)||data.physical_bytes<0)throw Error('missing storage size');
          text('[data-storage-bytes]',bytes(data.physical_bytes));
          const rolling=data.mode==='rolling',unlimited=data.mode==='unlimited';
          text('[data-storage-policy]',rolling?`上限 ${bytes(data.quota_bytes)}`:unlimited?'不限容量':'暂未取得');
          text('[data-storage-impact]',rolling?'达到容量限额后，自动清理最旧历史。':unlimited?'保留全部历史，存储占用会随采集增加。':'');
          text('[data-storage-checked]',`检查时间：${localTime(new Date().toISOString())}`);
          error.hidden=true;loaded=true;failures=0;
        }catch(e){
          if(!active||!host.isConnected)return;
          failures++;
          if(!loaded){text('[data-storage-bytes]','暂未取得');text('[data-storage-policy]','暂未取得');}
          if(!automatic||!loaded||failures>=2){error.hidden=false;error.querySelector('span').textContent=loaded?'存储占用暂未更新，显示上次结果。':'存储占用读取失败，请重试。';}
        }finally{pending=null;}
      })();
      return pending;
    }
    error.querySelector('button').onclick=()=>reload();
    return {reload,tick:()=>{if(canRefresh()&&Date.now()-lastAttempt>=30000)void reload({automatic:true});},dispose:()=>{active=false;}};
  }
  return {mount,bytes};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeHealthStorage;
