'use strict';
// Keep lightweight row anchors for counts/filtering/sticky rails. Distant card DOM is
// released into text, not detached DOM trees. Open details and keyboard focus stay mounted.
const PCScopeTimelineWindow=(()=>{
  function mount(root){
    const saved=new Map(),observed=new Set();
    let disposed=false,enabled=false;
    const scroller=root.closest('main');
    function restore(row){
      const html=saved.get(row);if(html===undefined)return;
      row.innerHTML=html;saved.delete(row);row.style.removeProperty('height');delete row.dataset.timelineParked;
    }
    const observer=typeof IntersectionObserver==='undefined'?null:new IntersectionObserver(entries=>{
      if(disposed)return;
      for(const entry of entries){
        const row=entry.target;
        if(!row.isConnected){observer.unobserve(row);observed.delete(row);saved.delete(row);continue;}
        if(entry.isIntersecting){restore(row);continue;}
        if(saved.has(row)||row.contains(document.activeElement)||row.querySelector('details[open],.timeline-object-detail'))continue;
        // Hidden categories and collapsed groups have no geometry. Keep a modest estimate
        // so restoring them does not require mounting every card at once.
        const height=entry.boundingClientRect.height||212;
        saved.set(row,row.innerHTML);row.replaceChildren();row.style.height=height+'px';row.dataset.timelineParked='true';
      }
    },{root:scroller,rootMargin:'1500px 0px'});
    function track(rows){
      if(!observer)return;
      const added=[];
      for(const row of rows)if(!observed.has(row)){observed.add(row);added.push(row);}
      if(!enabled&&observed.size>1000){enabled=true;for(const row of observed)observer.observe(row);}
      else if(enabled)for(const row of added)observer.observe(row);
    }
    function clear(){observer?.disconnect();saved.clear();observed.clear();enabled=false;}
    // Width changes invalidate measured heights; visible anchors are restored by the observer.
    // Distant estimates are intentionally retained until approached to bound resize work.
    return {track,clear,restore,dispose(){disposed=true;clear();}};
  }
  return {mount};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeTimelineWindow;
