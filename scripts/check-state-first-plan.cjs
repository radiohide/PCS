'use strict';
// Keep the remediation plan accountable to every real source, collection item and object type.
// This checks a plan, not production correctness or live collection completeness.
const fs=require('node:fs');
const path=require('node:path');
const assert=require('node:assert/strict');
const root=path.resolve(__dirname,'..');
const read=file=>JSON.parse(fs.readFileSync(path.join(root,file),'utf8').replace(/^\uFEFF/,''));
const plan=read('docs/refactor/state-first-coverage.json');
const inventory=read('model/sources.json');
const catalog=read('model/catalog.json');
const definitions=read('apps/desktop/ui/collection-catalog.json');
const controls=read('apps/desktop/ui/collection-controls.json');
const validTasks=new Set(Array.from({length:9},(_,i)=>'SP'+String(i+1).padStart(2,'0')));
const byId=(rows,key)=>new Map(rows.map(row=>[row[key],row]));
function sameIds(actual,expected,label){
  assert.equal(actual.length,new Set(actual).size,`${label}: duplicate ids`);
  assert.deepEqual([...actual].sort(),[...expected].sort(),`${label}: missing or obsolete ids`);
}
sameIds(plan.sources.map(r=>r.source),inventory.sources.map(r=>r.source),'sources');
sameIds(plan.items.map(r=>r.id),definitions.map(r=>r.id),'collection definitions');
sameIds(plan.types.map(r=>r.id),catalog.types.map(r=>r.id),'object types');
sameIds(plan.items.filter(r=>r.event_rule).map(r=>r.id),definitions.filter(r=>r.event_rule).map(r=>r.id),'event rules');
const sourceMap=byId(plan.sources,'source'),itemMap=byId(plan.items,'id'),typeMap=byId(catalog.types,'id');
const sourceKey=row=>row.prefix&&row.id==='pcscope.usn.changes'?'pcscope.usn.changes':row.prefix&&row.id==='pcscope.eventlog.import'?'pcscope.eventlog.import':row.source;
const nonempty=(value,label)=>assert.equal(typeof value==='string'&&Boolean(value.trim()),true,label);
function refsExist(refs,label){
  assert.ok(Array.isArray(refs)&&refs.length,`${label}: missing references`);
  for(const ref of refs){
    const match=ref.match(/^(.*?)(?::(\d+))?$/),file=match[1];
    assert.ok(!path.isAbsolute(file)&&!file.includes('..'),`${label}: unsafe reference ${ref}`);
    const full=path.join(root,file);assert.ok(fs.existsSync(full),`${label}: missing ${ref}`);
    if(match[2])assert.ok(Number(match[2])<=fs.readFileSync(full,'utf8').split(/\r?\n/).length,`${label}: invalid line ${ref}`);
  }
}
for(const row of plan.sources){
  for(const key of ['current','target','rule','acceptance'])nonempty(row[key],`${row.source}: ${key}`);
  assert.ok(['snapshot','event','measurement','health','mixed'].includes(row.purpose),`${row.source}: invalid purpose`);
  assert.ok(Array.isArray(row.gaps),`${row.source}: gaps must be explicit, including []`);
  assert.ok(row.tasks.length&&row.tasks.every(t=>validTasks.has(t)),`${row.source}: invalid tasks`);
  refsExist(row.refs,row.source);
  const actual=inventory.sources.find(s=>s.source===row.source);
  assert.equal(row.backend,actual.collection_backend||'virtual',`${row.source}: backend drift`);
  sameIds(row.item_ids,definitions.filter(d=>sourceKey(d)===row.source).map(d=>d.id),`${row.source} items`);
  sameIds(row.type_ids,[...new Set(catalog.sources.filter(s=>s.source===row.source).flatMap(s=>s.types))],`${row.source} type mapping`);
}
for(const definition of definitions){
  const row=itemMap.get(definition.id);assert.equal(row.source_key,sourceKey(definition),`${row.id}: source association`);
  assert.ok(sourceMap.has(row.source_key));assert.equal(row.name,definition.name);
  assert.deepEqual(row.event_rule,definition.event_rule,`${row.id}: event classification changed`);
  assert.deepEqual(row.contains,definition.contains,`${row.id}: item selection changed`);
  assert.equal(row.prefix,definition.prefix,`${row.id}: prefix selection changed`);
  for(const key of ['target','state_effect','acceptance','verification'])nonempty(row[key],`${row.id}: ${key}`);
  assert.ok(['pending_remediation','partially_implemented','implemented_pending_acceptance'].includes(row.implementation_status),`${row.id}: unknown implementation status`);
  if(row.implementation_status!=='pending_remediation'){
    nonempty(row.implementation_note,`${row.id}: implementation evidence`);
    refsExist(row.implementation_refs,`${row.id}: implementation references`);
  }
  if(row.event_rule){nonempty(row.gap,`${row.id}: event gap status`);refsExist(row.refs,row.id);}
}
for(const row of plan.types){
  const type=typeMap.get(row.id);nonempty(row.display_name,row.id);nonempty(row.meaning,row.id);
  assert.ok(row.summary_fields.length&&row.summary_fields.every(f=>type.fields.some(d=>d.id===f)),`${row.id}: unknown summary field`);
  sameIds(row.source_ids,[...new Set(catalog.sources.filter(s=>s.types.includes(row.id)).map(s=>s.source))],`${row.id}: source mapping`);
  assert.ok(row.source_ids.length,`${row.id}: no source path`);
}
const counts={collection_definitions:definitions.length,control_entries:controls.length,sources:inventory.sources.length,direct_sources:inventory.sources.filter(s=>!s.virtual).length,virtual_sources:inventory.sources.filter(s=>s.virtual).length,source_mapping_rows:catalog.sources.length,object_types:catalog.types.length,categories:catalog.categories.length,event_channels:inventory.counts.event_channels,event_rules:definitions.filter(r=>r.event_rule).length,native_sources:inventory.sources.filter(s=>s.collection_backend==='native').length,compatibility_sources:inventory.sources.filter(s=>s.collection_backend==='powershell_compatibility').length};
assert.deepEqual(plan.counts,counts,'baseline counts changed');
assert.equal(plan.catalog_version,catalog.version,'catalog version changed');
for(const control of controls)assert.ok(itemMap.has(control.id),`control ${control.id} not represented`);

const esc=value=>String(value??'').replace(/\|/g,'\\|').replace(/\r?\n/g,'<br>');
const link=ref=>{const [file,line]=ref.split(/:(?=\d+$)/);return `[${esc(file)}${line?':'+line:''}](../../${file}${line?'#L'+line:''})`;};
const purpose={snapshot:'状态或配置快照',event:'变化与事件',measurement:'资源采样',health:'采集健康',mixed:'多种材料'};
const backend={native:'原生',powershell_compatibility:'PowerShell 兼容',virtual:'虚拟汇总/导入'};
const sourceNumber=new Map(plan.sources.map((s,i)=>[s.source,String(i+1).padStart(3,'0')]));
const sourceAnchor=s=>'source-'+sourceNumber.get(s);
const sourceLink=s=>`[${esc(s)}](state-first-sources.md#${sourceAnchor(s)})`;
const banner='本清单规定整改目标和验收门槛，**不是实现完成或实机验收通过证明**。正常状态直接表达，技术范围与缺口在本工程清单中说明；不把这些限制逐条堆到产品主界面。\n\n';
let sourceDoc='# 逐来源整改清单\n\n'+banner+'[整改方案](state-first-remediation-plan.md) · [覆盖与缺口总表](state-first-coverage.md) · [采集项目](state-first-items.md) · [对象类型](state-first-types.md)\n\n';
for(const row of plan.sources){
  sourceDoc+=`<a id="${sourceAnchor(row.source)}"></a>\n\n## ${sourceNumber.get(row.source)} ${row.name}\n\n`;
  sourceDoc+=`来源：\`${row.source}\`。路线：${backend[row.backend]}。用途：${purpose[row.purpose]}。任务：${row.tasks.join('、')}。\n\n`;
  sourceDoc+=`**现状：** ${row.current}\n\n**整改后输出：** ${row.target}\n\n**判断规则：** ${row.rule}\n\n`;
  if(row.implementation_note)sourceDoc+=`**本轮实施进展：** ${row.implementation_note}\n\n`;
  sourceDoc+=`**具体缺口与范围：** ${row.gaps.length?'\n\n'+row.gaps.map(g=>'- '+g).join('\n'):'本轮没有单独发现来源特有缺口；仍需完成共用摘要、时效和全项验收。'}\n\n`;
  sourceDoc+=`**验收：** ${row.acceptance}\n\n`;
  sourceDoc+=`采集定义：${row.item_ids.length?row.item_ids.map(id=>'`'+id+'`').join('、'):'内部/动态来源，由对应汇总入口承接，不新增独立轮询或开关'}。\n\n`;
  sourceDoc+=`对象类型：${row.type_ids.length?row.type_ids.map(id=>'`'+id+'`').join('、'):'事件、采样或健康记录；按具体内容处理，无需强造对象类型'}。\n\n依据：${row.refs.map(link).join('、')}。\n\n`;
}
let itemDoc='# 全部采集项目整改清单\n\n'+banner+'[整改方案](state-first-remediation-plan.md) · [覆盖总表](state-first-coverage.md)\n\n'+counts.collection_definitions+' 个项目按定义 ID 各列一次，'+counts.event_rules+' 条日志分类规则均有单独结论与状态用途。未细分/来源状态项目、虚拟前缀项目同样保留。全部项目的新标准验收均待完成，既有验收作为输入材料保留。\n\n';
for(const source of plan.sources){
  const rows=plan.items.filter(i=>i.source_key===source.source);if(!rows.length)continue;
  itemDoc+=`## ${source.name}\n\n来源方案：${sourceLink(source.source)}。\n\n`;
  for(const row of rows){
    itemDoc+=`### ${row.number} ${row.name}\n\n定义：\`${row.id}\`。${row.prefix?'前缀归属：`'+row.source+'`。':''}\n\n`;
    if(row.event_rule){const r=row.event_rule;itemDoc+=`匹配范围：提供程序 ${esc(r.provider||'按通道/剩余事件分类')}；事件 ID ${r.event_ids?.join('、')||'整组/兜底，需具体语义规则'}。\n\n`;}
    else if(row.contains)itemDoc+=`细分范围：${row.contains.map(v=>'`'+v+'`').join('、')}。\n\n`;
    itemDoc+=`**输出目标：** ${row.target}\n\n**状态或事件用途：** ${row.state_effect}\n\n`;
    if(row.implementation_note)itemDoc+=`**本轮实施进展：** ${row.implementation_note}\n\n`;
    if(row.gap)itemDoc+=`**当前缺口：** ${row.gap}\n\n`;
    itemDoc+=`**验收：** ${row.acceptance}\n\n`;
  }
}
let typeDoc='# 全部对象类型展示规则\n\n'+banner+'[整改方案](state-first-remediation-plan.md) · [覆盖总表](state-first-coverage.md)\n\n以下 '+counts.object_types+' 类各有一条展示配置。字段是从当前目录选择的候选摘要，实施时按值状态、隐私和来源含义解释；不能只把白名单放宽。需要新字段或拆分语义的情况在“表达规则”与来源缺口中列出。\n\n';
for(const category of catalog.categories){
  typeDoc+=`## ${category.name}\n\n| 类型与显示名称 | 主要信息候选 | 表达规则 | 对应来源 |\n| --- | --- | --- | --- |\n`;
  for(const row of plan.types.filter(t=>t.category===category.id)){
    const t=typeMap.get(row.id);typeDoc+=`| ${esc(row.display_name)}<br>\`${row.id}\` | ${row.summary_fields.map(f=>esc(t.fields.find(x=>x.id===f).value.label)).join('、')} | ${esc(row.meaning)} | ${row.source_ids.map(sourceLink).join('<br>')} |\n`;
  }typeDoc+='\n';
}
const gaps=plan.sources.filter(r=>r.gaps.length);
let indexDoc='# 全量覆盖与未覆盖部分\n\n'+banner+'[整改方案](state-first-remediation-plan.md) · [逐来源方案](state-first-sources.md) · ['+counts.collection_definitions+' 项采集定义](state-first-items.md) · ['+counts.object_types+' 类对象规则](state-first-types.md) · [可核对数据](state-first-coverage.json)\n\n';
if(plan.implementation_round)indexDoc+='当前实施记录：[第一批实现与验证](state-first-implementation.md)。未完成的专项缺口和实机验收仍保留在各项中。\n\n';
indexDoc+='## 方案覆盖结果\n\n| 核对维度 | 范围 | 已列入方案 | 未列入方案 |\n| --- | ---: | ---: | ---: |\n';
for(const[name,n]of [['采集定义',counts.collection_definitions],['控制条目',counts.control_entries],['唯一来源',counts.sources],['对象类型',counts.object_types],['日志分类规则',counts.event_rules]])indexDoc+=`| ${name} | ${n} | ${n} | 0 |\n`;
indexDoc+=`\n源代码对账为 ${counts.direct_sources} 个实际来源声明加 ${counts.virtual_sources} 个虚拟来源，当前代码路线为 ${counts.native_sources} 原生、${counts.compatibility_sources} PowerShell 兼容。${counts.source_mapping_rows} 是来源映射行数。动态卷字母/导入文件的实例数没有被混入功能数量。\n\n`;
indexDoc+='## 尚未完成实现或未采集的能力\n\n下面逐来源列出已发现的字段、含义、关联、范围或验收缺口。它们已经纳入方案，但尚未因本轮文档工作得到修复。范围限制有的属于明确选择，不全部视为采集故障；每条的目标和完成门槛见来源方案。\n\n';
indexDoc+='| 来源 | 当前缺口或未覆盖范围 | 整改任务 |\n| --- | --- | --- |\n';
for(const row of gaps)indexDoc+=`| ${esc(row.name)}<br>${sourceLink(row.source)} | ${row.gaps.map((g,i)=>(i+1)+'. '+esc(g)).join('<br>')} | ${row.tasks.join('、')} |\n`;
indexDoc+='\n## 没有单独发现来源特有缺口的项目\n\n';
const noGaps=plan.sources.filter(r=>!r.gaps.length);indexDoc+=noGaps.length?noGaps.map(r=>'- '+r.name+'：'+sourceLink(r.source)).join('\n'):'本轮各来源均记录了至少一项整改、范围或验收说明；不是所有来源都缺失采集功能。';
indexDoc+='\n\n无特有缺口不表示新标准验收通过；共用摘要、值解释、时效、搜索/导出及失败恢复仍由 SP01–SP09 负责。\n\n';
indexDoc+='## 现有采集未覆盖或需另行扩展的能力\n\n'+plan.outside_scope.map(r=>'- **'+r.capability+'**：'+r.reason+' 对应 '+r.tracking+'。').join('\n')+'\n\n';
indexDoc+='## 尚未完成的实机覆盖\n\n本轮没有对 '+counts.collection_definitions+' 项逐一重新运行采集。已有 8 类实际对象样本用于复现摘要缺失，现有历史验收材料可供复用；所有项目仍须按新标准登记适用环境与结果。Windows 10 LTSC、Windows 11、普通/增强权限、相关组件存在/缺失/停用及跨用户/会话场景按各项验收门槛安排。不具备环境的项目逐项标为未验收，不以固定样本代替。\n\n';
indexDoc+='## 特殊来源与维护\n\n- `pcscope.usn.watch-list` 是已实现但未进入独立采集目录的内部状态来源，已纳入本方案；承接到文件监控范围说明，不增加独立轮询。\n- `pcscope.usn.changes` 和 `pcscope.eventlog.import` 的定义通过前缀归属，已显式映射到虚拟来源。精确来源优先于宽前缀，避免把卷能力/名单状态当文件变化。\n- 来源、定义、类型或日志分类变化后运行 `node scripts/check-state-first-plan.cjs`；增删项必须同步完善方案，不能仅更新计数。文档由同份 JSON 生成，`--write` 可更新附表。\n';
const outputs={'docs/refactor/state-first-sources.md':sourceDoc,'docs/refactor/state-first-items.md':itemDoc,'docs/refactor/state-first-types.md':typeDoc,'docs/refactor/state-first-coverage.md':indexDoc};
for(const[file,contents]of Object.entries(outputs)){
  if(process.argv.includes('--write'))fs.writeFileSync(path.join(root,file),contents);
  else assert.equal(fs.readFileSync(path.join(root,file),'utf8'),contents,`${file}: generated document drift; use --write`);
}
console.log(JSON.stringify({result:'plan_coverage_verified',...counts,uncovered_sources:0,uncovered_items:0,uncovered_types:0,implementation_and_live_acceptance:'pending'},null,2));
