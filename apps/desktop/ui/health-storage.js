'use strict';
const PCScopeHealthStorage=(()=>{
  function bytes(value){
    if(typeof value!=='number'||!Number.isFinite(value)||value<0)return '暂未取得';
    const units=['B','KB','MB','GB','TB'];let i=0;
    while(value>=1000&&i<units.length-1){value/=1000;i++;}
    return `${value.toLocaleString('zh-CN',{maximumFractionDigits:i?2:0})} ${units[i]}`;
  }
  function mount(host,{api,localTime,categoryNames={},canRefresh=()=>true}){
    let active=true,pending=null,lastAttempt=0,loaded=false,failures=0;
    host.innerHTML='<div class="settings-metrics health-storage-metrics"><div><span>历史数据占用</span><strong data-storage-bytes>正在读取…</strong></div><div><span>保留策略</span><strong data-storage-policy>正在读取…</strong></div></div><p class="muted" data-storage-impact></p><small class="muted" data-storage-checked></small><div data-storage-error role="status" hidden><span>存储占用读取失败，请重试。</span> <button type="button">重试</button></div><details class="settings-details"><summary>统计范围</summary><p>包含当前历史数据库及其写入缓存，不含备份、导出文件和程序文件。按文件大小统计，1 GB = 10 亿字节。</p></details>';
    const text=(selector,value)=>{const node=host.querySelector(selector);if(node.textContent!==value)node.textContent=value;};
    const error=host.querySelector('[data-storage-error]');
    const usageHost=document.createElement('section');usageHost.className='health-storage-breakdown';
    usageHost.innerHTML='<h3>采集内容分布</h3><p class="muted" data-usage-scope></p><div data-usage-categories></div>';
    host.append(usageHost);
    const categoryRows=new Map();
    function updateUsage(usage){
      const scope=usageHost.querySelector('[data-usage-scope]');
      const valid=usage?.available===true&&Array.isArray(usage.sources)&&Array.isArray(usage.categories);
      scope.textContent=valid?`从 ${localTime(usage.started_at)} 开始统计新采集内容。共享内容只计一次，不含此前历史及数据库索引。`:'更新后台后可查看分类占用。';
      if(!valid)return;
      const categories=usage.categories.filter(c=>Number.isFinite(c.retained_bytes)&&c.retained_bytes>=0).sort((a,b)=>b.retained_bytes-a.retained_bytes);
      const total=categories.reduce((sum,c)=>sum+c.retained_bytes,0),present=new Set();
      const list=usageHost.querySelector('[data-usage-categories]');
      for(const category of categories){
        present.add(category.category);
        let row=categoryRows.get(category.category);
        if(!row){
          row=document.createElement('details');row.className='settings-details';
          row.innerHTML='<summary><span></span><strong></strong></summary><meter min="0" max="100"></meter><ul></ul>';
          categoryRows.set(category.category,row);list.append(row);
        }
        const name=categoryNames[category.category]||'其他';
        row.querySelector('summary span').textContent=name;
        row.querySelector('summary strong').textContent=bytes(category.retained_bytes);
        const meter=row.querySelector('meter');meter.value=total?100*category.retained_bytes/total:0;meter.setAttribute('aria-label',`${name}占已统计内容 ${meter.value.toFixed(1)}%`);
        const sources=usage.sources.filter(s=>s.category===category.category).sort((a,b)=>b.retained_bytes-a.retained_bytes);
        const key=JSON.stringify(sources);
        if(row.dataset.sources!==key){
          row.dataset.sources=key;const children=sources.map(source=>{const li=document.createElement('li');li.textContent=`${source.name||'其他采集内容'} · ${bytes(source.retained_bytes)}`;return li;});
          row.querySelector('ul').replaceChildren(...children);
        }
      }
      for(const [key,row] of categoryRows){if(!present.has(key)){row.remove();categoryRows.delete(key);}}
      if(!categories.length)scope.textContent='等待下一轮采集后显示分类占用。';
    }
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
          updateUsage(data.content_usage);
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
