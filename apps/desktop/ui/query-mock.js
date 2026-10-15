'use strict';
// Development-only query mock (R05). Serves the fixed response set in model/query-samples.json so
// the object pages can be built and reviewed before the real queries exist.
//
// This module holds no data of its own and is not imported by any page. It refuses to run unless a
// caller explicitly opts in, so a production build cannot reach it by accident — and
// scripts/check-query-contract.cjs fails if any other UI file starts referencing it.
const PCScopeQueryMock=(()=>{
  const FLAG='PCSCOPE_QUERY_MOCK';
  function samples(){
    const provided=globalThis[FLAG];
    if(!provided||!Array.isArray(provided.samples)||provided.samples.length===0){
      throw new Error('查询模拟仅限开发：需显式设置 globalThis.'+FLAG+' = { samples: [...] }，且样本不得为空');
    }
    if(provided.samples.some(sample=>!sample||!sample.response||sample.response.mock!==true)){
      throw new Error('查询模拟仅限开发：每个样本都必须是带 mock:true 的固定响应');
    }
    return provided.samples;
  }
  function names(){
    return samples().map(sample=>sample.name);
  }
  function byName(name){
    const found=samples().find(sample=>sample.name===name);
    if(!found)throw new Error('没有名为 '+name+' 的模拟响应；可用：'+names().join('、'));
    // Deep copy so a page cannot mutate the fixed set and affect the next call.
    return JSON.parse(JSON.stringify(found.response));
  }
  // Routes a query request to the sample that represents its outcome. Only the states the pages
  // must render are routed; anything else is a programming error, not a silent empty response.
  function request(query){
    const name=ROUTES[query&&query.query];
    if(!name)throw new Error('模拟服务未覆盖该请求：'+JSON.stringify(query)+'；可用：'+Object.keys(ROUTES).join('、'));
    return byName(name);
  }
  const ROUTES={
    type_catalog:'type_catalog',
    instance_list:'instance_list',
    instance_detail:'instance_detail',
    instance_actions:'instance_actions',
    evidence_detail:'evidence_available',
    source_coverage:'source_coverage',
    projection_status:'projection_status',
  };
  // Named outcomes the pages must handle, for building states without a live backend.
  const OUTCOMES=['instance_list_empty','unsupported','permission_denied','version_mismatch','evidence_pruned'];
  return {names,byName,request,outcomes:OUTCOMES,flag:FLAG};
})();
if(typeof module!=='undefined')module.exports=PCScopeQueryMock;
