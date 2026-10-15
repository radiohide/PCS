'use strict';
// Current routes come from the generated source inventory, never from a claimed migration.
const fs=require('node:fs');const path=require('node:path');const assert=require('node:assert/strict');
const root=path.resolve(__dirname,'..');const inventory=require('../model/sources.json');const catalog=require('../apps/desktop/ui/plugin-sources.js');
const plan=require('../model/native-collection-plan.cjs');
for(const source of plan.explicitSources)assert(inventory.sources.some(row=>row.source===source),'Stale collection decision: '+source);
const rows=Object.values(catalog).flat();const sources=inventory.sources.map(source=>{
  const definitions=rows.filter(row=>row.source===source.source);
  const backend=source.virtual?'virtual':source.collection_backend;
  assert(['native','powershell_compatibility','virtual'].includes(backend));
  return {source:source.source,name:source.catalog_name,execution_units:source.execution_units,backend,
    status:backend==='native'?'native_owned':backend==='virtual'?'group_only':'pending_native_port',
    existing_api:[...new Set(definitions.map(row=>row.api))],scope:[...new Set(definitions.map(row=>row.method))],
    plan:plan.forSource(source.source),
    completion_gate:backend==='powershell_compatibility'?'Preserve identities, raw fields, scopes, shards, protection and gaps; prove native parity and timeout behavior before changing ownership.':null};
});
const counts={};for(const source of sources)counts[source.backend]=(counts[source.backend]||0)+1;
const planned_modes={},pending_priorities={P0:0,P1:0,P2:0};for(const source of sources){planned_modes[source.plan.mode]=(planned_modes[source.plan.mode]||0)+1;if(source.backend==='powershell_compatibility')pending_priorities[source.plan.priority]=(pending_priorities[source.plan.priority]||0)+1;}
const output=JSON.stringify({generated_by:'scripts/build-native-migration.cjs',collector_digest:inventory.collector_digest,
  note:'Native-first requirement applies to all sources. Pending is implementation work, not evidence that the API requires PowerShell. Native failures never trigger automatic PowerShell fallback. Existing API descriptions are legacy catalog inputs, not native implementation claims. Plan fields identify targets and remaining validation, never completed migrations.',counts,planned_modes,pending_priorities,sources},null,2)+'\n';
const modes={state:'当前状态',capability:'采集能力',configuration:'采集配置',event_history:'事件历史',virtual:'分组',change_tracking:'变化跟踪',retained_history:'产品保留历史',change_journal:'文件日志'};
const backends={native:'已原生',powershell_compatibility:'待迁移',virtual:'不采集'};
const cell=value=>String(value??'').replace(/\|/g,'&#124;').replace(/[\r\n]+/g,' ');
const appendix=['# 后端原生采集逐项清单','','由 `scripts/build-native-migration.cjs` 生成；决策维护于 `model/native-collection-plan.cjs`。当前实现依据 `model/sources.json`，目标接口不表示已完成。','','返回[范围与实施方案](../backend-native-collection-plan.md)。','',`共 ${sources.length} 项：${counts.native} 项已原生、${counts.powershell_compatibility} 项待迁移、${counts.virtual} 项分组。未分类来源为 0；待迁移 P0 ${pending_priorities.P0} / P1 ${pending_priorities.P1} / P2 ${pending_priorities.P2}。`,'',`采集器摘要：\`${inventory.collector_digest}\`。`,''];
for(const group of [...new Set(sources.map(row=>row.plan.group))]){
  appendix.push('## '+group,'','| 来源 / 名称 | 当前实现 / 顺序 | 数据性质 | 原生路线 | 范围与待验收项 |','|---|---|---|---|---|');
  for(const row of sources.filter(row=>row.plan.group===group))appendix.push('| '+[row.source+' — '+row.name,backends[row.backend]+' / '+row.plan.priority,modes[row.plan.mode],row.plan.route,row.plan.boundary].map(cell).join(' | ')+' |');
  appendix.push('');
}
for(const [relative,content] of [['model/native-migration.json',output],['docs/refactor/native-collection-inventory.md',appendix.join('\n')]]){
  const file=path.join(root,relative);
  if(process.argv.includes('--check'))assert.equal(fs.readFileSync(file,'utf8').replace(/\r\n/g,'\n'),content,'Regenerate '+relative);else fs.writeFileSync(file,content);
}
console.log(`Native migration inventory: ${counts.native} native, ${counts.powershell_compatibility} pending, ${counts.virtual} groups`);
