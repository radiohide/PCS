'use strict';
// R09 end-to-end drill check.
//
// Runs the drill output through the stand-in detail renderer and asserts the acceptance conditions:
// one instance for a task seen by two sources, every field carrying its own evidence, unassigned
// events retained, and unknown / conflict / pruned-evidence states surviving to the rendered text.
//
// It also verifies that every contract path the renderer reads is actually produced — so a renamed
// field fails here instead of rendering blank on a page.
const fs=require('node:fs');
const path=require('node:path');
const root=path.resolve(__dirname,'..');
const p=(...parts)=>path.join(root,...parts);

const drill=JSON.parse(fs.readFileSync(p('model/drill-output.json'),'utf8'));
const renderer=require(p('apps/desktop/ui/drill-view.js'));
const catalog=JSON.parse(fs.readFileSync(p('model/catalog.json'),'utf8'));
const failures=[];
const check=(condition,message)=>{if(!condition)failures.push(message);};

const byCase=new Map(drill.cases.map(entry=>[entry.case,entry]));
const allResponses=drill.cases.flatMap(entry=>entry.responses.map(item=>item.response));
const responsesOf=(name,query)=>((byCase.get(name)||{}).responses||[]).filter(entry=>entry.query===query).map(entry=>entry.response);
const firstDetail=name=>responsesOf(name,'instance_detail')[0];

// ---------- 1. one instance for a task seen by two sources ----------
const task=firstDetail('01-task-two-sources-and-run-log');
check(!!task,'用例 01 没有产出实例详情响应');
if(task){
  const detail=task.data;
  check(detail.summary.identity_status==='confirmed',`用例 01：身份应已确认，实际 ${detail.summary.identity_status}`);
  // The same field observed by both configuration sources must say so, rather than one silently
  // winning. Two sources agreeing at the same effective time produce one value citing both.
  const name=detail.fields.find(field=>field.field_id==='name');
  check(!!name,'用例 01：详情缺少 name 字段');
  if(name){
    const sources=new Set((name.evidence||[]).map(ref=>ref.id.split('|')[0]));
    check(sources.size>=2,`用例 01：name 应引用两个来源的证据，实际 ${[...sources].join('、')||'无'}`);
  }
  // Every field points at its own evidence; a field with none must at least be an explicit absence.
  for(const field of detail.fields){
    const assured=field.status==='known'||field.status==='conflicting'||field.status==='masked';
    check((field.evidence||[]).length>0||!assured,
      `用例 01：字段 ${field.field_id} 状态为 ${field.status} 却没有证据`);
  }
  const actionIds=detail.actions_preview.map(action=>action.action_id);
  check(actionIds.includes('startup.task_started')&&actionIds.includes('startup.task_ended'),
    `用例 01：运行日志的开始与结束应是两条动作，实际 ${actionIds.join('、')}`);
}

// ---------- 2. unassigned events are retained ----------
const listOnly=responsesOf('07-unattributable-log-and-gap','instance_list')[0];
check(!!listOnly,'用例 07 没有产出实例列表响应');
if(listOnly){
  check(listOnly.status==='empty'||(listOnly.counts||{}).instances===0,'用例 07：无法归属的日志不应产生对象实例');
}
// The record that could not be attributed keeps its stated reason. It is not dropped and no target
// is invented for it.
const unattributable=byCase.get('07-unattributable-log-and-gap')||{};
check((unattributable.problems||[]).length>0,'用例 07：无法归属的记录必须留下原因，而不是静默丢弃');
check((unattributable.problems||[]).some(entry=>/not mapped to a target type/.test(entry.problem||'')),
  '用例 07：原因应说明事件未能映射到目标类型');
const invented=(unattributable.responses||[])
  .flatMap(entry=>Array.isArray(entry.response.data)?entry.response.data:[])
  .filter(row=>row&&row.unassigned_reason===undefined&&row.action_id);
check(invented.length===0,'用例 07：不得为无法归属的记录编造动作');

// ---------- 3. unknown, conflict and pruned states survive to the renderer ----------
const rendered=drill.cases.map(entry=>({
  case:entry.case,
  text:entry.responses.map(item=>renderer.renderResponse(item.response)).join('\n'),
}));
const textOf=name=>(rendered.find(entry=>entry.case===name)||{}).text||'';

check(textOf('05-disabled-unreadable-pruned').includes('unreadable'),
  '用例 05：无法读取的字段应以 unreadable 呈现，而不是留空');
check(textOf('05-disabled-unreadable-pruned').includes('evidence_pruned'),
  '用例 05：证据已清理的字段应以 evidence_pruned 呈现');
check(textOf('05-disabled-unreadable-pruned').includes('已停用'),
  '用例 05：采集项停用应显示为已停用');
check(textOf('04-late-log-and-conflicting-value').includes('conflicting'),
  '用例 04：冲突字段应以 conflicting 呈现并保留候选');
check(textOf('04-late-log-and-conflicting-value').includes('冲突'),
  '用例 04：冲突候选应出现在渲染结果里');
// A time the source did not state is marked, and a time it did state is not. Otherwise a page would
// either imply precision it does not have, or hide the precision it does.
const allActions=allResponses.flatMap(response=>
  response.data&&Array.isArray(response.data.actions_preview)?response.data.actions_preview:[]);
check(allActions.length>0,'演练没有产出任何动作，时间精度检查无从进行');
for(const action of allActions){
  if(action.precision==='exact'&&action.occurred_at){
    check(action.time_derived===false,`动作 ${action.action_id} 有精确发生时间，不应标记为派生`);
  }
  if(action.precision==='interval'||!action.occurred_at){
    check(action.time_derived===true,`动作 ${action.action_id} 的时间是派生的，必须标记`);
  }
}

// ---------- 4. every contract path the renderer reads is actually produced ----------
function resolvePath(source,expression){
  const segments=expression.split('.');
  let current=[source];
  for(const segment of segments){
    const field=segment.replace(/\[\]$/,'');
    const spread=segment.endsWith('[]');
    const next=[];
    for(const value of current){
      if(value===null||value===undefined)continue;
      const child=value[field];
      if(child===undefined)continue;
      if(spread&&Array.isArray(child))next.push(...child);else next.push(child);
    }
    current=next;
    if(current.length===0)return false;
  }
  return current.length>0;
}
for(const expression of renderer.PATHS){
  const found=allResponses.some(response=>resolvePath(response,expression));
  check(found,`渲染器读取的契约字段 ${expression} 在演练输出中从未出现——字段被改名或未产出`);
}

// ---------- 5. the drill is a stand-in and says so ----------
check(/stand-in/i.test(fs.readFileSync(p('apps/desktop/ui/drill-view.js'),'utf8')),'替身渲染器必须标明自己是替身');
check(/S23/.test(fs.readFileSync(p('apps/desktop/ui/drill-view.js'),'utf8')),'替身渲染器必须写明由哪一步替换');
check(drill.note.includes('替身'),'演练输出必须说明投影是替身');
check(allResponses.every(response=>response.mock!==true),'演练使用的是真实响应形状，不应带模拟标记');

// ---------- 6. per-case counts match the R09 record ----------
// The counts quoted in docs/refactor/acceptance/R09-drill.md 第 2 节 and docs/refactor/drill.md 第 3 节
// are asserted here rather than left as prose. Adding interval actions to the stand-in projection once
// moved case 01 from 2 actions to 6 and no check noticed — the run stayed green and only the document
// went stale. A number in a document that nothing computes is a number that quietly stops being true.
const expectedCounts={
  '01-task-two-sources-and-run-log':{instances:1,actions:2,responses:5},
  '02-same-name-different-path-and-recreated':{instances:3,actions:1,responses:5},
  '03-pid-reuse-and-incomplete-event':{instances:3,actions:1,responses:5},
  '04-late-log-and-conflicting-value':{instances:1,actions:1,responses:6},
  '05-disabled-unreadable-pruned':{instances:1,actions:0,responses:4},
  '06-sensitive-value-and-mask':{instances:2,actions:0,responses:4},
  '07-unattributable-log-and-gap':{instances:0,actions:0,responses:2},
};
for(const entry of drill.cases){
  const expected=expectedCounts[entry.case];
  check(!!expected,`用例 ${entry.case} 不在 R09 记录的计数表里——新增用例必须同时更新记录`);
  if(!expected)continue;
  for(const field of ['instances','actions','responses']){
    check(entry.counts&&entry.counts[field]===expected[field],
      `用例 ${entry.case} 的 ${field} 为 ${entry.counts&&entry.counts[field]}，记录写的是 ${expected[field]}——请同步更新 R09 记录与 drill.md`);
  }
}

// ---------- 7. a change action states a change that actually happened ----------
// The stand-in projection once emitted four observed.configuration_changed actions for case 01 by
// comparing a value against a field the other source simply did not carry, and by comparing two
// observations that take effect at the very same time. Both produced an interval of zero length and
// an `after` of null — an action claiming a configuration change with no evidence that anything
// changed. Case 01's whole point is that two sources agree; the check now says so.
const changed=allResponses.flatMap(response=>
  response.data&&Array.isArray(response.data)?response.data.filter(row=>row&&row.action_id&&row.field_id):[]);
for(const action of changed){
  check(action.interval_from!==action.interval_until,
    `动作 ${action.action_id}（字段 ${action.field_id}）的区间起止相同，零长度区间等于宣称一次没有发生的变化`);
  check(action.before!==null&&action.before!==undefined&&action.after!==null&&action.after!==undefined,
    `动作 ${action.action_id}（字段 ${action.field_id}）的 before/after 不完整——缺失不是值，不能当作变化的一侧`);
}

// ---------- 8. the drill agrees with the contract cases about the same fixtures ----------
// Both pipelines run these same seven fixtures: the contract-case runner goes through the adapters
// and the identity layer, the drill goes through those plus the stand-in projection. The contract
// cases state their expectations by hand, each with a stated basis, so they are an independent
// reading of the same input rather than a copy of whatever this run happened to produce. Until now
// nothing compared the two — and case 01's four phantom actions sat here contradicting
// `actions.length == 2` for a whole round with every check green.
const COUNT_PATHS={'instances.length':'instances','actions.length':'actions'};
// Assertions the drill cannot answer yet, named so that adding a new one is a decision rather than
// another silently uncompared expectation.
const DECLARED_UNMAPPED={
  'gaps.length':'采集缺口需要覆盖响应；演练不产出该响应，查询契约也还没有对应类型（R09 记录第 6 节第 4 项）',
};
const unmapped=[];
const contractRoot=p('fixtures/contract');
for(const entry of fs.readdirSync(contractRoot,{withFileTypes:true})){
  if(!entry.isDirectory())continue;
  const file=p('fixtures/contract',entry.name,'case.json');
  if(!fs.existsSync(file))continue;
  const declared=JSON.parse(fs.readFileSync(file,'utf8'));
  const run=byCase.get(declared.name);
  check(!!run,`契约用例 ${declared.name} 在演练输出里没有对应用例——两套夹具已经不同步`);
  if(!run)continue;
  for(const assertion of declared.assertions){
    if(assertion.equals===undefined)continue;  // only absolute expectations are comparable here
    const expected=assertion.equals;
    if(COUNT_PATHS[assertion.path]){
      const field=COUNT_PATHS[assertion.path];
      check(run.counts[field]===expected,
        `契约用例 ${declared.name} 要求 ${assertion.path} = ${expected}（${assertion.what}），演练产出的 ${field} 是 ${run.counts[field]}`);
      continue;
    }
    // With several instances the two pipelines order them independently and index 0 is not the same
    // row, so the comparison is only honest when there is exactly one.
    const summary=responsesOf(declared.name,'instance_detail')[0];
    if(/^instances\[0\]\.(identity_status|type_id)$/.test(assertion.path)){
      if(run.counts.instances!==1||!summary){ unmapped.push(`${declared.name} ${assertion.path}`); continue; }
      const field=assertion.path.split('.')[1];
      check(summary.data.summary[field]===expected,
        `契约用例 ${declared.name} 要求 ${assertion.path} = ${expected}（${assertion.what}），演练产出的是 ${summary.data.summary[field]}`);
      continue;
    }
    unmapped.push(`${declared.name} ${assertion.path}`);
  }
}
for(const item of unmapped){
  const path=item.split(' ')[1];
  check(!!DECLARED_UNMAPPED[path],
    `契约用例的断言 ${item} 没有被比较，也没有登记为已知未覆盖——请接进来，或在 DECLARED_UNMAPPED 里写明原因`);
}

// ---------- 9. contract version consistency ----------
for(const response of allResponses){
  check(response.catalog_version===catalog.version,
    `演练响应的目录版本 ${response.catalog_version} 与 catalog.json 的 ${catalog.version} 不一致`);
}

// ---------- report ----------
console.log(`Drill: ${drill.cases.length} cases, ${allResponses.length} responses, catalog ${drill.catalog_version}`);
console.log(`Renderer contract paths: ${renderer.PATHS.length} checked`);
console.log('');
console.log('--- 用例 01 的详情渲染（摘录） ---');
console.log(textOf('01-task-two-sources-and-run-log').split('\n').slice(0,14).join('\n'));

if(failures.length){
  console.error(`\nDrill check FAILED (${failures.length}):`);
  for(const failure of failures.slice(0,25))console.error(`  ! ${failure}`);
  if(failures.length>25)console.error(`  … and ${failures.length-25} more`);
  process.exit(1);
}
console.log('\nDrill check verified.');
