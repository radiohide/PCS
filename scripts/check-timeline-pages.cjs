'use strict';
const assert=require('node:assert/strict');
const pages=require('../apps/desktop/ui/timeline-pages.js');
const time=require('../apps/desktop/ui/timeline-time.js');
const row=(group,id)=>({id,precision:'exact',occurred_at:new Date(Date.UTC(2026,9,20,10,0)-group*300000).toISOString()});
const source=sizes=>sizes.flatMap((n,g)=>Array.from({length:n},(_,i)=>row(g,g+':'+i)));
function fixture(rows,{groups=2,pageSize=200,failAt=null,active=()=>true}={}){
 const calls=[],emitted=[];let fail=failAt,concurrent=0,maxConcurrent=0;
 const reader=pages.create({groups,active,keyOf:r=>time.moment(r).key,query:async cursor=>{
  concurrent++;maxConcurrent=Math.max(maxConcurrent,concurrent);try{
   const from=cursor?Number(cursor):0;calls.push(from);if(from===fail){fail=null;throw Error('temporary query failure');}
   return {status:'loaded',data:rows.slice(from,from+pageSize),page:{watermark:'frozen',has_more:from+pageSize<rows.length,next_cursor:from+pageSize<rows.length?String(from+pageSize):null}};
  }finally{concurrent--;}
 },emit:async response=>{emitted.push(...response.data.map(r=>r.id));}});
 return {reader,calls,emitted,maxConcurrent:()=>maxConcurrent};
}
(async()=>{
 assert.deepEqual(pages.options,[2,3,4,5,6]);for(const n of [null,0,1,7,'invalid'])assert.equal(pages.normalise(n),2);
 for(const groups of pages.options){
  const rows=source([7,8,9,10,11,12,13,14,15]),f=fixture(rows,{groups});let all=[];
  for(;;){const result=await f.reader.read();assert.equal(result.group_page.complete,true);assert(result.group_page.groups<=groups);if(result.page.has_more)assert.equal(result.group_page.groups,groups);all.push(...result.data.map(r=>r.id));if(!result.page.has_more)break;}
  assert.deepEqual(all,rows.map(r=>r.id));assert.deepEqual(f.emitted,all);assert.equal(f.calls.length,1,'lookahead rows reused without requery');
 }
 const denseRows=source([1050,1,7,9]),dense=fixture(denseRows);
 const first=await dense.reader.read();assert.equal(first.data.length,1051);assert.equal(first.group_page.groups,2);assert.equal(dense.calls.length,6);assert.equal(first.page.has_more,true);
 const second=await dense.reader.read();assert.equal(second.data.length,16);assert.equal(second.page.has_more,false);assert.equal(dense.calls.length,6);assert.equal(dense.maxConcurrent(),1);
 assert.deepEqual(dense.emitted,denseRows.map(r=>r.id));
 const retryRows=source([450,70,6]),retry=fixture(retryRows,{failAt:200});
 await assert.rejects(retry.reader.read(),/temporary/);assert.equal(retry.emitted.length,200);
 const resumed=await retry.reader.read();assert.equal(resumed.data.length,520);assert.equal(new Set(retry.emitted).size,retry.emitted.length);assert.deepEqual(retry.calls,[0,200,200,400]);
 let alive=true;const cancelled=pages.create({groups:2,active:()=>alive,keyOf:r=>time.moment(r).key,query:async()=>{alive=false;return {status:'loaded',data:[row(0,'cancel')],page:{has_more:false}};},emit:()=>assert.fail('cancelled results must not render')});
 await assert.rejects(cancelled.read(),e=>e.name==='AbortError');
 const invalid=pages.create({keyOf:r=>time.moment(r).key,query:async()=>({status:'loaded',data:[row(0,'x')],page:{has_more:true,next_cursor:'same'}})});
 await assert.rejects(invalid.read(),/分页未能继续/);
 let emptyCalls=0;const empty=pages.create({keyOf:r=>time.moment(r).key,query:async()=>({status:'empty',data:[],page:{has_more:true,next_cursor:String(++emptyCalls)}})});
 await assert.rejects(empty.read(),/连续读取/);assert.equal(emptyCalls,3);
 let mark=0;const changed=pages.create({keyOf:r=>time.moment(r).key,query:async()=>({status:'loaded',data:[row(0,mark)],page:{has_more:true,next_cursor:String(++mark),watermark:'mark'+mark}})});
 await assert.rejects(changed.read(),/读取范围/);
 const noRows=fixture([]);assert.equal((await noRows.reader.read()).status,'empty');
 const unknown=fixture([{id:1},{id:2}]);assert.equal((await unknown.reader.read()).group_page.groups,1);
 const huge=fixture(source([8000,2,1]));assert.equal((await huge.reader.read()).data.length,8002);assert.equal(huge.calls.length,41);assert.equal(new Set(huge.emitted).size,8002);
 console.log('PASS: complete group paging for 2–6 groups, bounded transport/lookahead, dense groups, exact-once retry, cancellation, watermark/cursor/empty-page guards.');
})().catch(error=>{console.error(error);process.exitCode=1;});
