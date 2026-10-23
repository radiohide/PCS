(function(global){
  'use strict';
  const MAX_LOOKAHEAD=1200;
  // One automatic request per window. IPC also serializes queries, but this avoids filling its queue.
  const controllers=new Set(),waiting=[];let running=false,timer=null,frame=null;
  function recheck(){
    if(frame!==null)return;
    frame=requestAnimationFrame(()=>{frame=null;for(const controller of [...controllers])controller.check();});
  }
  // Scroll can occur before IntersectionObserver samples a sentinel moving out and back in.
  // One passive listener also covers nested object-tree scroll areas.
  document.addEventListener('scroll',recheck,{capture:true,passive:true});
  global.addEventListener('resize',recheck,{passive:true});
  function pump(){
    if(running||timer)return;
    timer=setTimeout(async()=>{
      timer=null;
      const controller=waiting.shift();if(!controller)return;
      running=true;
      try{await controller.advance();}finally{running=false;pump();}
    },120);
  }
  function mount(anchor,{query,apply,onStart=()=>{},onError=()=>{},active=()=>true,canAutoLoad=()=>true,eager=false,endLabel='已显示全部结果'}){
    let generation=0,cursor=null,more=false,loaded=false,busy=false,disposed=false,fault=null,emptyPages=0,queued=false,restart=false,restartOnRetry=false;
    const requested=new Set();
    anchor.classList.add('auto-page');anchor.hidden=true;
    const status=document.createElement('span');status.setAttribute('role','status');
    const retry=document.createElement('button');retry.type='button';retry.textContent='重试';retry.hidden=true;
    const details=document.createElement('details'),summary=document.createElement('summary'),reason=document.createElement('pre');
    summary.textContent='问题详情';details.append(summary,reason);details.hidden=true;
    anchor.replaceChildren(status,retry,details);
    const alive=()=>!disposed&&anchor.isConnected;
    let scrollRoot=null;
    for(let parent=anchor.parentElement;parent;parent=parent.parentElement){
      if(/auto|scroll/.test(getComputedStyle(parent).overflowY)){scrollRoot=parent;break;}
    }
    const near=()=>{
      if(!alive()||!active()||!anchor.getClientRects().length)return false;
      const rect=anchor.getBoundingClientRect();if(!rect.width||!rect.height)return false;
      let top=0,bottom=innerHeight;
      for(let parent=anchor.parentElement;parent;parent=parent.parentElement){
        const style=getComputedStyle(parent);
        if(/auto|scroll|hidden|clip/.test(style.overflowY)){
          const bounds=parent.getBoundingClientRect();
          top=Math.max(top,bounds.top+parent.clientTop);bottom=Math.min(bottom,bounds.top+parent.clientTop+parent.clientHeight);
        }
      }
      if(bottom<=top)return false;
      // Read one page about a visible screen before the end, including nested scrollers.
      // The cap bounds lookahead on large displays; normal paging guards still serialize work.
      const lookahead=Math.min(MAX_LOOKAHEAD,Math.max(160,bottom-top));
      return rect.bottom>=top&&rect.top<=bottom+lookahead;
    };
    function paint(){
      anchor.hidden=!loaded&&!busy&&!fault||!active();
      status.textContent=fault?(loaded?'后续内容暂时无法读取。':'读取未完成。'):busy?'正在读取…':more?(eager?'正在继续对比…':'向下滚动自动加载'):endLabel;
      retry.hidden=!fault;retry.disabled=busy;
      details.hidden=!fault;reason.textContent=fault?.message||'';
    }
    function enqueue(){
      if(!alive()){dispose();return;}
      if(queued||busy||fault||!more||!canAutoLoad()||!active()||(!eager&&!near()))return;
      queued=true;waiting.push(controller);pump();
    }
    async function run(append){
      if(!alive()||busy||!active())return;
      const revision=generation,previous=append?cursor:null;
      busy=true;fault=null;restartOnRetry=false;paint();
      try{
        onStart({append});
        const response=await query(previous);
        if(!alive()||revision!==generation)return;
        if(!['loaded','empty','partial'].includes(response.status))throw Error('本次读取未完成，请重试。');
        await apply(response,{append});
        if(!alive()||revision!==generation)return;
        loaded=true;requested.add(previous);
        more=!!response.page?.has_more;cursor=response.page?.next_cursor||null;
        const items=Array.isArray(response.data)?response.data:response.data?.objects;
        emptyPages=Array.isArray(items)&&items.length?0:emptyPages+1;
        if(more&&(!cursor||requested.has(cursor)||emptyPages>=3)){restartOnRetry=true;throw Error('分页未能继续，已停止自动读取。');}
      }catch(error){
        if(alive()&&revision===generation){fault=error instanceof Error?error:new Error(typeof error==='string'?error:error?.message||'读取失败，请重试。');onError(fault,{append:append||loaded});}
      }finally{
        busy=false;
        if(alive()){
          paint();
          if(restart){restart=false;run(false);}else if(revision===generation)enqueue();
        }
      }
    }
    async function reset({load=true}={}){
      generation++;cursor=null;more=false;loaded=false;fault=null;emptyPages=0;restartOnRetry=false;requested.clear();restart=busy&&load;details.open=false;paint();
      if(load&&!busy)return run(false);
    }
    function dispose(){if(disposed)return;disposed=true;generation++;observer.disconnect();controllers.delete(controller);}
    const controller={reset,pause:()=>reset({load:false}),resume:()=>{if(alive()){paint();enqueue();}},check:enqueue,dispose,advance:async()=>{queued=false;if(alive()&&!busy&&!fault&&more&&canAutoLoad()&&active()&&(eager||near()))await run(true);}};
    retry.onclick=()=>{if(!busy){if(restartOnRetry)reset();else{fault=null;run(loaded);}}};
    const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting))enqueue();},{root:scrollRoot,rootMargin:'0px 0px '+MAX_LOOKAHEAD+'px 0px'});
    observer.observe(anchor);controllers.add(controller);return controller;
  }
  function disposeWithin(container){for(const controller of [...controllers])if(controller.anchor&&container.contains(controller.anchor))controller.dispose();}
  // Keep ownership of anchors explicit so page changes release observers and pending continuations.
  const create=(anchor,options)=>{const controller=mount(anchor,options);controller.anchor=anchor;return controller;};
  global.PCScopeAutoPage={mount:create,disposeWithin};
})(typeof window!=='undefined'?window:globalThis);
