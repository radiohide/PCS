'use strict';
const PCScopeObjectTree=(()=>{
  // Expansion is a presentation preference. Data and page cursors belong to one mounted view.
  const expanded=new Set();
  let rememberedSelection=null;
  function mount(container,{catalog,filters,categoryNames,query,esc,errorHtml}){
    const searchMode=!!filters.search||!catalog.length;
    const filterKey=JSON.stringify(filters);
    const groups=new Map(),branches=new Map();let selected=rememberedSelection?.filterKey===filterKey?rememberedSelection.alias:null,detailRevision=0,disposed=false;
    container.innerHTML=`<div class="object-tree-layout"><section class="object-tree-browser" aria-label="对象分类"><div class="object-tree-heading"><strong>本机对象</strong><p class="muted">${searchMode?'按分类显示匹配结果。':'展开类型查看对象，点击名称查看详情。'}</p></div><div data-tree-groups></div><div data-tree-feedback role="status"></div><div data-tree-more></div></section><section class="object-tree-detail" aria-label="对象详情"><p class="empty">选择左侧对象，查看状态与最近变化。</p></section></div>`;
    const tree=container.querySelector('[data-tree-groups]'),detail=container.querySelector('.object-tree-detail'),feedback=container.querySelector('[data-tree-feedback]'),more=container.querySelector('[data-tree-more]');
    const alive=()=>!disposed&&container.isConnected;
    const base=()=>{const request={query:'instance_list',sort:filters.sort,limit:100};for(const key of ['search','category','type_id','lifecycle'])if(filters[key])request[key]=filters[key];return request;};
    function group(category){
      // Unknown categories share a readable fallback, while retaining each object's type.
      const key=categoryNames[category]?category:'other';
      if(groups.has(key))return groups.get(key);
      const element=document.createElement('details');element.className='object-tree-category';element.dataset.treeCategory=key;
      element.innerHTML=`<summary>${esc(categoryNames[key]||'其他对象')}</summary><div class="object-tree-types"></div>`;
      element.open=searchMode||!!filters.category||!!filters.type_id||expanded.has('category:'+key);
      element.addEventListener('toggle',()=>{if(searchMode)return;if(element.open)expanded.add('category:'+key);else expanded.delete('category:'+key);});
      tree.append(element);groups.set(key,element.querySelector('.object-tree-types'));return groups.get(key);
    }
    function branch(type,category,label){
      if(branches.has(type))return branches.get(type);
      const element=document.createElement('details');element.className='object-tree-type';element.dataset.treeType=type;
      element.innerHTML=`<summary><span>${esc(label||'其他对象')}</span><small data-tree-count></small></summary><div data-tree-objects></div><p data-branch-feedback role="status"></p><div data-branch-more></div>`;
      const entry={element,body:element.querySelector('[data-tree-objects]'),feedback:element.querySelector('[data-branch-feedback]'),more:element.querySelector('[data-branch-more]'),count:element.querySelector('[data-tree-count]'),cursor:null,loaded:false,pending:false,aliases:new Set()};
      branches.set(type,entry);group(category).append(element);
      entry.pager=PCScopeAutoPage.mount(entry.more,{
        active:()=>!searchMode&&element.open&&alive(),
        query:cursor=>query({...base(),type_id:type,include_counts:!cursor,...(cursor?{cursor}:{})}),
        onStart:()=>{entry.feedback.textContent='正在读取…';},
        apply:(response,{append:adding})=>{
          if(!adding){entry.body.replaceChildren();entry.aliases.clear();}
          const page=PCScopeModelView.treePage(response);append(entry,page.nodes);
          if(!adding)entry.total=page.total;
          entry.count.textContent=entry.total==null?`已显示 ${entry.aliases.size}`:`${entry.total} 个`;
          entry.feedback.textContent=entry.aliases.size?'':'没有符合筛选条件的对象。';
        },
        onError:error=>{entry.feedback.innerHTML=errorHtml(error);}
      });
      element.addEventListener('toggle',()=>{if(searchMode)return;if(element.open){expanded.add('type:'+type);if(!entry.loaded)loadBranch(type);else entry.pager.resume();}else expanded.delete('type:'+type);});
      return entry;
    }
    async function select(node,button){
      selected=node.alias;const revision=++detailRevision;
      rememberedSelection={filterKey,alias:selected};
      for(const item of tree.querySelectorAll('[data-tree-object]'))item.setAttribute('aria-pressed',String(item===button));
      detail.innerHTML='<p role="status">正在读取对象详情…</p>';
      try{const response=await query({query:'instance_detail',instance:node.alias});if(!alive()||revision!==detailRevision)return;detail.innerHTML=PCScopeModelView.detail(response);}
      catch(error){if(!alive()||revision!==detailRevision)return;detail.innerHTML=`${errorHtml(error)}<button data-detail-retry>重试</button>`;detail.querySelector('button').onclick=()=>select(node,button);}
    }
    function append(entry,nodes){
      for(const node of nodes){if(entry.aliases.has(node.alias))continue;entry.aliases.add(node.alias);
        const button=document.createElement('button');button.type='button';button.className='object-tree-object';button.dataset.treeObject=node.alias;button.setAttribute('aria-pressed',String(node.alias===selected));
        button.innerHTML=`<span>${esc(node.name)}</span><small>${esc(node.state)}</small>`;button.onclick=()=>select(node,button);entry.body.append(button);
        if(node.alias===selected)select(node,button);
      }
    }
    async function loadBranch(type){
      const entry=branches.get(type);if(!alive()||entry.loaded)return;
      entry.loaded=true;return entry.pager.reset();
    }
    let matchesTotal=null;
    const matchesPager=PCScopeAutoPage.mount(more,{
      active:()=>alive()&&searchMode,
      query:cursor=>query({...base(),include_counts:!cursor,...(cursor?{cursor}:{})}),
      onStart:()=>{feedback.textContent='正在查找对象…';},
      apply:(response,{append:adding})=>{
        if(!adding){for(const entry of branches.values())entry.pager.dispose();tree.replaceChildren();branches.clear();groups.clear();}
        const page=PCScopeModelView.treePage(response);
        if(!adding)matchesTotal=page.total;
        for(const node of page.nodes){const entry=branch(node.type,node.category,node.typeName);append(entry,[node]);entry.element.open=true;entry.element.parentElement.parentElement.open=true;entry.count.textContent=`已显示 ${entry.aliases.size}`;}
        const loaded=[...branches.values()].reduce((n,b)=>n+b.aliases.size,0);
        feedback.textContent=matchesTotal==null?`已显示 ${loaded} 个对象`:`找到 ${matchesTotal} 个对象，已显示 ${loaded} 个`;
        if(!loaded)feedback.textContent='没有匹配的对象。可以调整筛选条件。';
      },
      onError:error=>{feedback.innerHTML=errorHtml(error);}
    });
    if(searchMode)matchesPager.reset();
    else{
      for(const type of catalog){if(filters.category&&type.category!==filters.category||filters.type_id&&type.type_id!==filters.type_id)continue;branch(type.type_id,type.category,PCScopeObjectNames.typeName(type));}
      // Restore sequentially, never fan out one query per catalog type on page load.
      (async()=>{for(const [type,entry] of branches){if(!alive())return;if(filters.type_id===type||expanded.has('type:'+type)){entry.element.open=true;await loadBranch(type);}}})();
      if(!branches.size)matchesPager.reset();
    }
    return {dispose(){disposed=true;++detailRevision;matchesPager.dispose();for(const entry of branches.values())entry.pager.dispose();}};
  }
  return {mount};
})();
