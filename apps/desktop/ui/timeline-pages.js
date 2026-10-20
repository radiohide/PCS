'use strict';
// A logical page contains complete time buckets; transport row and byte budgets are independent.
// Keep one lookahead page and the same backend cursor watermark throughout a walk.
const PCScopeTimelinePages=(()=>{
  const options=Object.freeze([2,3,4,5,6]);
  const normalise=value=>options.includes(Number(value))?Number(value):2;
  function create({query,keyOf,groups=2,emit=async()=>{},progress=()=>{},active=()=>true}){
    const target=normalise(groups),seenCursors=new Set();
    let cursor=null,more=true,buffer=[],offset=0,envelope={},watermark=null,emptyPages=0,batch=null,sequence=0,reading=false,prefetch=null;
    const check=()=>{if(!active())throw Object.assign(new Error('本次加载已取消'),{name:'AbortError'});};
    function startFetch(){
      check();
      // Resolve failures into values immediately: a cancelled view may never await this job.
      prefetch??=Promise.resolve().then(()=>{check();return query(cursor);}).then(response=>({response}),error=>({error}));
    }
    async function fetch(){
      startFetch();const result=await prefetch;prefetch=null;check();
      if(result.error)throw result.error;const response=result.response;
      if(!['loaded','empty','partial'].includes(response.status)||!Array.isArray(response.data))throw Error('本次读取未完成，请重试。');
      const next=response.page?.next_cursor||null,hasMore=!!response.page?.has_more,mark=response.page?.watermark;
      if(hasMore&&(!next||next===cursor||seenCursors.has(next)))throw Error('分页未能继续，请载入最新后重试。');
      if(watermark&&mark&&mark!==watermark)throw Error('读取范围发生变化，请载入最新后重试。');
      if(!response.data.length&&hasMore&&emptyPages>=2)throw Error('连续读取未返回记录，请载入最新后重试。');
      emptyPages=response.data.length?0:emptyPages+1;
      if(next)seenCursors.add(next);
      cursor=next;more=hasMore;watermark=mark||watermark;envelope=response;buffer=response.data;offset=0;
    }
    async function read(){
      if(reading)throw Error('正在读取当前时间组');
      reading=true;
      // Retain the batch after a transport failure. Retry resumes without duplicating emitted rows.
      batch??={keys:[],rows:[],id:++sequence};
      try{
        for(;;){
          check();
          if(offset>=buffer.length){buffer=[];offset=0;if(!more)break;await fetch();if(!buffer.length)continue;}
          const start=offset;let end=start;
          while(end<buffer.length){
            const key=keyOf(buffer[end]);
            if(batch.keys.at(-1)!==key){
              if(batch.keys.length===target)break;
              if(batch.keys.includes(key))throw Error('时间顺序发生变化，请载入最新后重试。');
              batch.keys.push(key);
            }
            end++;
          }
          if(end===start)break; // The first row of the next bucket proves this page is complete.
          const rows=buffer.slice(start,end);
          // Only prefetch when this entire transport page is consumed. It is needed either
          // to fill the requested groups or to prove the last group ended. Never fan out.
          if(end===buffer.length&&more)startFetch();
          await emit({...envelope,data:rows}, {groups:batch.keys.length,target,rows:batch.rows.length+rows.length});check();
          batch.rows.push(...rows);offset=end;
          progress({groups:batch.keys.length,target,rows:batch.rows.length});
        }
        const hasMore=offset<buffer.length||more;
        const result={...envelope,status:batch.rows.length?'loaded':'empty',data:batch.rows,
          page:{limit:target,watermark,next_cursor:hasMore?'groups:'+batch.id:null,has_more:hasMore},
          group_page:{groups:batch.keys.length,complete:true}};
        batch=null;return result;
      }finally{reading=false;}
    }
    return {read};
  }
  return {options,normalise,create};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeTimelinePages;
