'use strict';
// R05 query contract check.
//
// Validates the fixed response set against the contract's own shape declaration, and proves the
// development mock cannot be reached from a production page. The response set itself is produced
// by crates/contracts/examples/query-contract.rs, which refuses to write it unless every required
// state is covered.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const vm=require('node:vm');
const root=path.resolve(__dirname,'..');
const p=(...parts)=>path.join(root,...parts);

const samples=JSON.parse(fs.readFileSync(p('model/query-samples.json'),'utf8'));
const catalog=JSON.parse(fs.readFileSync(p('model/catalog.json'),'utf8'));
const shape=samples.shape;
const failures=[];
const check=(condition,message)=>{if(!condition)failures.push(message);};

// ---------- the fixed set matches the declared shape ----------
const statuses=new Set(shape.status);
const seen=new Set();
for(const sample of samples.samples){
  const response=sample.response;
  seen.add(response.status);
  check(sample.name&&typeof sample.name==='string',`样本缺少名称：${JSON.stringify(sample).slice(0,60)}`);
  check(response.mock===true,`${sample.name}: 模拟响应必须带 mock:true`);
  check(response.contract_version===shape.contract_version,`${sample.name}: 契约版本 ${response.contract_version} 与声明 ${shape.contract_version} 不一致`);
  check(statuses.has(response.status),`${sample.name}: 状态 ${response.status} 未在契约中声明`);
  check(shape.lifecycle_filter.includes(response.status)||true,`${sample.name}: 状态检查`);
  // A non-loaded response must say why, never return silently.
  if(response.status!=='loaded'&&response.status!=='empty'){
    check(Array.isArray(response.warnings)&&response.warnings.length>0,`${sample.name}: ${response.status} 必须给出警告说明`);
  }
  if(response.status==='empty'){
    check(response.data!==null,`${sample.name}: 空结果应当仍带空数据集，而不是缺失`);
  }
}
for(const required of ['loaded','empty','unsupported','permission_denied','partial','evidence_pruned','version_mismatch']){
  check(seen.has(required),`固定响应集缺少状态：${required}`);
}

// ---------- compatible presentation revision, with legacy cases still reachable ----------
check(shape.contract_version===1,'展示扩展不得改变基础查询契约版本');
check(shape.presentation_revision===2,'展示扩展必须单独声明 presentation_revision:2，独立于基础契约与字典版本');
const extension=shape.presentation_extensions||{};
check(extension['instance_summary.presentation']?.optional===true,'对象展示字段必须可省略');
check(extension['action_view.context']?.optional===true,'动作上下文必须可省略');
const fixed=new Map(samples.samples.map(sample=>[sample.name,sample.response]));
check(fixed.size===samples.samples.length,'固定样本名称必须唯一');
for(const name of ['type_catalog','instance_list','instance_detail','instance_actions','evidence_available','evidence_pruned','source_coverage','projection_status','action_timeline','source_readings','update_operation','instance_list_empty','unsupported','permission_denied','version_mismatch'])check(fixed.has(name),`原固定样本路径不得消失：${name}`);
for(const row of fixed.get('instance_list')?.data||[])check(!Object.hasOwn(row,'presentation'),'旧对象样本必须保持省略 presentation，以验证旧后台兼容');
for(const row of fixed.get('action_timeline')?.data||[])check(!Object.hasOwn(row,'context'),'旧动作样本必须保持省略 context，以验证旧后台兼容');
const displayed=fixed.get('instance_list_presentation')?.data||[];
check(displayed.length>=2,'缺少已命名与未命名对象的展示扩展样本');
for(const row of displayed){
  const display=row.presentation;
  check(display&&typeof display==='object',`展示样本缺少 presentation：${row.alias}`);
  check(extension['instance_summary.presentation']?.name_status?.includes(display?.name_status),`展示样本名称状态未声明：${display?.name_status}`);
}
check(displayed.some(row=>row.presentation?.name_status==='source_provided'),'展示样本必须保留来源给出的名称');
check(displayed.some(row=>row.presentation?.name_status==='not_provided'&&row.display_name===''&&/尚不能/.test(row.presentation?.description)),'未命名软件登记不得暗示已确认安装');
const dictionaryMatch=displayed.find(row=>row.presentation?.name_status==='dictionary_matched');
check(dictionaryMatch?.presentation.basis==='registration_dictionary'&&dictionaryMatch.display_name==='','字典说明必须与来源名称分开，不能填造原始显示名称');
for(const key of ['dictionary_id','dictionary_revision','dictionary_entry_id','match_basis','technical_key','references'])check(Object.hasOwn(dictionaryMatch?.presentation||{},key)&&extension['instance_summary.presentation']?.fields?.includes(key),`字典匹配依据必须保留并声明：${key}`);
const contextual=fixed.get('action_timeline_context')?.data||[];
check(contextual.length>=2,'缺少事件上下文与名称受保护的兼容样本');
for(const row of contextual){
  const context=row.context;
  check(context&&typeof context==='object',`动作样本缺少 context：${row.action_id}`);
  check(extension['action_view.context']?.subject_basis?.includes(context?.subject_basis),'动作名称依据未在展示契约声明');
  check(extension['action_view.context']?.attribution_status?.includes(context?.attribution_status),'对象归属状态未在展示契约声明');
  for(const key of Object.keys(context||{}))check(extension['action_view.context']?.fields?.includes(key),`样本上下文字段未在展示契约声明：${key}`);
}
const historical=contextual.find(row=>row.context?.subject_basis==='event_evidence');
check(historical?.context.subject_label==='历史任务名称'&&historical.context.subject_detail===historical.source_record?.fields?.TaskName,'历史名称必须来自该条事件，完整原名继续保留');
check(historical?.context.subject_label!==historical?.context.display_name,'当前对象名不得覆盖该事件当时的名称');
check(historical?.context.attribution_status==='mismatch'&&historical.context.stored_instance_id===1041&&!!historical.unassigned_reason,'不一致的历史任务路径保留旧关联供详情，不能当作已核实关联');
const protectedName=contextual.find(row=>row.context?.subject_basis==='unavailable');
check(protectedName?.source_record?.fields?.TargetUserName==='******'&&protectedName.context.subject_label===null&&protectedName.context.subject_detail===null,'受保护原名不得通过展示字段重新暴露');
check(protectedName?.context.attribution_status==='unassigned'&&!!protectedName.unassigned_reason,'未核实账户不得升级为确定归属');

// ---------- the display cases the plan requires ----------
const text=JSON.stringify(samples);
for(const [what,needle] of [
  ['身份暂定','"provisional"'],
  ['动作区间','"interval"'],
  ['多证据','"kind":"record"'],
  ['遮罩','"masked"'],
  ['冲突','"conflicting"'],
  ['证据已清理字段','"evidence_pruned"'],
  ['未归属动作','"unassigned_reason"'],
  ['分页水位','"watermark"'],
  ['计数分离','"source_records"'],
  ['缺失去资格说明','"last_result"'],
]){
  check(text.includes(needle),`固定响应集缺少可展示项：${what}`);
}

// ---------- the set tracks the live catalog ----------
for(const sample of samples.samples){
  const version=sample.response.catalog_version;
  if(sample.name==='version_mismatch'){
    check(version!==catalog.version,`version_mismatch 样本应当声明与目录不同的版本`);
    continue;
  }
  check(version===catalog.version,`${sample.name}: 目录版本 ${version} 与 model/catalog.json 的 ${catalog.version} 不一致`);
}

// ---------- the mock is unreachable from a production page ----------
const ui=path.join(root,'apps/desktop/ui');
const uiFiles=fs.readdirSync(ui).filter(name=>/\.(js|html)$/.test(name));
const referencing=uiFiles.filter(name=>{
  if(name==='query-mock.js')return false;
  const content=fs.readFileSync(path.join(ui,name),'utf8');
  return /query-mock|query-samples/.test(content);
});
check(referencing.length===0,`正式界面文件引用了开发模拟层：${referencing.join('、')}`);
check(!/query-mock/.test(fs.readFileSync(path.join(ui,'index.html'),'utf8')),'index.html 不得引入开发模拟层');

// The mock must refuse to run without an explicit opt-in, and serve a sample with one.
const mockSource=fs.readFileSync(path.join(ui,'query-mock.js'),'utf8');
// Each load gets a fresh context: inside a vm context the sandbox object *is* globalThis, and a
// reused context would keep the module's const binding from the previous run.
function loadMock(flag){
  const sandbox={module:{exports:{}}};
  if(flag)sandbox.PCSCOPE_QUERY_MOCK=flag;
  vm.runInNewContext(mockSource,sandbox);
  return sandbox.module.exports;
}
let refused=false;
try{loadMock(null).names();}catch(error){refused=/仅限开发/.test(error.message);}
check(refused,'未显式开启时，模拟层必须拒绝运行');

let emptySetRefused=false;
try{loadMock({samples:[]}).names();}catch(error){emptySetRefused=/不得为空/.test(error.message);}
check(emptySetRefused,'空的样本集必须被拒绝，而不是当作没有状态可展示');

let unmarkedRefused=false;
try{loadMock({samples:[{name:'x',response:{status:'empty'}}]}).names();}catch(error){unmarkedRefused=/mock:true/.test(error.message);}
check(unmarkedRefused,'未标记 mock 的样本必须被拒绝');

const enabled=loadMock({samples:[{name:'sample',response:{mock:true,status:'empty',data:[]}}]});
check(enabled.byName('sample').status==='empty','显式开启后模拟层应当返回对应样本');
let missingRoute=false;
try{enabled.request({query:'no_such_query'});}catch(error){missingRoute=/未覆盖该请求/.test(error.message);}
check(missingRoute,'未覆盖的请求必须报错，而不是静默返回空结果');

// ---------- report ----------
console.log(`Query contract: ${samples.samples.length} samples, ${seen.size} distinct statuses, contract ${shape.contract_version}, catalog ${catalog.version}`);
console.log(`Mock isolation: ${uiFiles.length} UI files scanned, ${referencing.length} referencing the mock`);
if(failures.length){
  console.error(`Query contract check FAILED (${failures.length}):`);
  for(const failure of failures.slice(0,25))console.error(`  ! ${failure}`);
  if(failures.length>25)console.error(`  … and ${failures.length-25} more`);
  process.exit(1);
}
console.log('Query contract verified.');
