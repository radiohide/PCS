'use strict';
// Local display controls. Record nodes stay mounted, including any open object or log details.
const PCScopeTimelineGroups=(()=>{
  const time=typeof PCScopeTimelineTime!=='undefined'?PCScopeTimelineTime:require('./timeline-time.js');
  // The same readable category is already the switch identity. Keep its color stable
  // across groups and pages, including categories first encountered on a later page.
  const categoryTones={'运行进程':'blue','用户配置文件':'teal','用户资料':'teal','时间同步配置':'amber','时间同步设置':'amber','网络连接':'cyan','域名解析缓存':'green','DNS 缓存项':'green','系统服务':'indigo','进程加载模块':'pink','已加载模块':'pink'};
  const tones=['blue','cyan','teal','green','olive','amber','orange','rose','pink','indigo'];
  function toneOf(category){
    if(Object.hasOwn(categoryTones,category))return categoryTones[category];
    let hash=0;for(const character of category)hash=(Math.imul(hash,31)+character.codePointAt(0))>>>0;
    return tones[hash%tones.length];
  }
  let sequence=0;
  function mount(root,{collapsed=false}={}){
    const states=new Map(),unknownStates=new WeakMap(),metadata=new WeakMap(),pending=new WeakSet(),bucketIndex=new Map(),categoryCounts=new WeakMap();
    let lastMoment=null;
    const read=group=>{if(!metadata.has(group))metadata.set(group,JSON.parse(group.dataset.timeMeta));return metadata.get(group);};
    function updateTime(group){
      const meta=read(group),range=time.range(meta),label=group.querySelector('.timeline-time-value');
      group.dataset.timeGroup=meta.key;group.dataset.timeMeta=JSON.stringify(meta);
      label.textContent=range.start;
      if(range.end){
        const end=document.createElement('span');end.className='timeline-time-end';
        const separator=document.createElement('span');separator.className='timeline-time-separator';separator.textContent='至';
        end.append(separator,document.createTextNode(range.end));label.append(end);
      }
    }
    function place(list,group){
      const meta=read(group),days=[...list.children];let day=days.find(node=>node.dataset.timelineDay===meta.date);
      if(!day){
        day=document.createElement('section');day.className='timeline-day';day.dataset.timelineDay=meta.date;
        const title=document.createElement('h2');title.className='timeline-day-label';title.textContent=meta.date;day.append(title);
        const dateKey=value=>/^\d{4}-\d{2}-\d{2}$/.test(value)?value:'';
        list.insertBefore(day,days.find(node=>dateKey(node.dataset.timelineDay)<dateKey(meta.date))||null);
      }
      const next=[...day.querySelectorAll(':scope > .timeline-group')].find(node=>node!==group&&time.rank(read(node))<time.rank(meta))||null;
      if(group.parentElement!==day||group.nextElementSibling!==next)day.insertBefore(group,next);
    }
    function append(html){
      const template=document.createElement('template');template.innerHTML=html;
      const incoming=template.content.querySelector('.timeline-list'),list=root.querySelector('.timeline-list');
      if(!incoming){lastMoment=null;return;}
      if(!list){root.replaceChildren(template.content);sync();return;}
      const groups=[...list.querySelectorAll('.timeline-group')].sort((a,b)=>read(a).order-read(b).order),changed=new Map();
      let order=groups.reduce((n,group)=>Math.max(n,read(group).order),-1)+1;
      // Repartition individual rows in query order, rather than merging pre-grouped pages whole.
      const rows=[...incoming.querySelectorAll('.timeline-branch')].sort((a,b)=>Number(a.dataset.timelineOrder)-Number(b.dataset.timelineOrder));
      lastMoment=null;
      for(const row of rows){
        const moment=JSON.parse(row.dataset.timelineMoment);let group=bucketIndex.get(moment.key);
        if(group)time.include(read(group),moment);
        else {
          const source=row.closest('.timeline-group');group=source.cloneNode(false);
          const body=source.querySelector(':scope > .timeline-branches').cloneNode(false);
          group.append(source.querySelector(':scope > .timeline-time-rail').cloneNode(true),body,source.querySelector(':scope > .timeline-group-empty').cloneNode(true));
          metadata.set(group,{...time.seed(moment),order:order++});groups.push(group);bucketIndex.set(moment.key,group);
        }
        group.querySelector(':scope > .timeline-branches').append(row);if(!changed.has(group))changed.set(group,[]);changed.get(group).push(row);lastMoment=moment;
      }
      for(const group of changed.keys()){updateTime(group);place(list,group);}
      for(const day of [...list.children])if(!day.querySelector('.timeline-group'))day.remove();
      for(const [group,rows] of changed)update(group,rows);
    }
    function stateOf(group){
      const key=group.dataset.timeGroup,store=key?states:unknownStates,identity=key||group;
      if(!store.has(identity))store.set(identity,{collapsed,muted:new Set()});
      return store.get(identity);
    }
    function count(group){
      const total=group.querySelector(':scope > .timeline-branches').children.length;
      group.querySelector('.timeline-group-count').textContent=pending.has(group)?'已加载 '+total+' 条':total+' 条记录';
    }
    function markBoundary(hasMore){
      if(hasMore&&!lastMoment)return;
      const last=lastMoment?.key;
      for(const group of root.querySelectorAll('.timeline-group')){
        const meta=read(group),open=hasMore&&meta.key===last;
        if(open)pending.add(group);else pending.delete(group);count(group);
      }
    }
    function update(group,added){
      const state=stateOf(group),body=group.querySelector(':scope > .timeline-branches');
      const controls=group.querySelector('.timeline-category-controls');
      const existing=categoryCounts.get(group),incremental=!!(added&&existing);
      const rows=incremental?added:[...body.children],counts=incremental?existing:new Map();let visible=0;
      if(!body.id)body.id='timeline-group-records-'+(++sequence);
      for(const row of rows){
        const category=row.dataset.timelineCategory||'其他记录';
        row.dataset.timelineTone=toneOf(category);
        counts.set(category,(counts.get(category)||0)+1);
        row.hidden=state.muted.has(category);
      }
      categoryCounts.set(group,counts);
      for(const [category,total] of counts)if(!state.muted.has(category))visible+=total;
      const total=body.children.length;
      const buttons=new Map([...controls.children].map(button=>[button.dataset.timelineCategoryToggle,button]));
      for(const [category,count] of counts){
        let button=buttons.get(category);
        if(!button){
          button=document.createElement('button');button.type='button';button.className='timeline-category-toggle';button.dataset.timelineCategoryToggle=category;
          button.innerHTML='<span class="timeline-category-dot" aria-hidden="true"></span><span class="timeline-category-name"></span><span class="timeline-category-count"></span>';
          button.querySelector('.timeline-category-name').textContent=category;controls.append(button);
        }
        const on=!state.muted.has(category);
        button.dataset.timelineTone=toneOf(category);
        button.setAttribute('aria-pressed',String(on));button.setAttribute('aria-controls',body.id);
        button.setAttribute('aria-label',category+'，'+count+' 条，'+(on?'点击隐藏':'点击显示'));
        button.title=on?'隐藏本组的'+category:'显示本组的'+category;
        button.querySelector('.timeline-category-count').textContent=count;
      }
      for(const [category,button] of buttons)if(!counts.has(category))button.remove();
      count(group);
      group.querySelector('[data-timeline-visible]').textContent=visible===total?'全部显示':'显示 '+visible+' / '+total+' 条';
      group.querySelector('.timeline-category-footer').hidden=visible===total;
      for(const toggle of group.querySelectorAll('[data-timeline-toggle],[data-timeline-time-toggle]')){
        toggle.setAttribute('aria-controls',body.id);toggle.setAttribute('aria-expanded',String(!state.collapsed));
        toggle.title=state.collapsed?'展开本组记录':'收起本组记录';
      }
      group.querySelector('.timeline-fold-label').textContent=state.collapsed?'展开':'收起';
      group.classList.toggle('is-collapsed',state.collapsed);
      body.hidden=state.collapsed||visible===0;
      group.querySelector(':scope > .timeline-group-empty').hidden=state.collapsed||visible!==0;
    }
    const onClick=event=>{
      const button=event.target.closest('button[data-timeline-toggle],button[data-timeline-time-toggle],button[data-timeline-category-toggle],button[data-timeline-show-all]');
      if(!button||!root.contains(button))return;
      const group=button.closest('.timeline-group');if(!group)return;
      const state=stateOf(group);
      const folding=button.hasAttribute('data-timeline-toggle')||button.hasAttribute('data-timeline-time-toggle');
      const scroller=folding?root.closest('main'):null,anchor=group.querySelector('[data-timeline-toggle]');
      const before=scroller?anchor.getBoundingClientRect().top:null;
      if(folding)state.collapsed=!state.collapsed;
      else if(button.hasAttribute('data-timeline-show-all'))state.muted.clear();
      else {const category=button.dataset.timelineCategoryToggle;if(state.muted.has(category))state.muted.delete(category);else state.muted.add(category);}
      update(group);
      // Keep a sticky time control within reach when its long list is collapsed or restored.
      if(scroller)scroller.scrollTop+=anchor.getBoundingClientRect().top-before;
    };
    root.addEventListener('click',onClick);
    function sync(groups){
      if(!groups){
        groups=root.querySelectorAll('.timeline-group');bucketIndex.clear();for(const group of groups)bucketIndex.set(read(group).key,group);lastMoment=null;let order=-1;
        for(const row of root.querySelectorAll('.timeline-branch'))if(Number(row.dataset.timelineOrder)>order){order=Number(row.dataset.timelineOrder);lastMoment=JSON.parse(row.dataset.timelineMoment);}
      }
      for(const group of groups)update(group);
    }
    function setCollapsed(value){
      collapsed=!!value;
      for(const group of root.querySelectorAll('.timeline-group')){stateOf(group).collapsed=collapsed;update(group);}
    }
    return {sync,append,markBoundary,setCollapsed,dispose:()=>root.removeEventListener('click',onClick)};
  }
  return {mount};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeTimelineGroups;
