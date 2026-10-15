'use strict';
// Derive overlap facts only. Implementation ownership remains an explicit readiness decision.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..');
// Hashes are of content, not of a checkout: with core.autocrlf the same commit can be checked out
// with CRLF in one work tree and LF in another, and a version identifier must not change with that.
const normalise=text=>text.includes('\r\n')?text.replace(/\r\n/g,'\n'):text;
const raw=fs.readFileSync(path.join(root,'model/catalog.json'));
const catalog=JSON.parse(raw);
const byType=new Map(catalog.types.map(t=>[t.id,new Set()]));
if(byType.size!==catalog.types.length)throw new Error('Duplicate type ID');
const bySource=new Map();
for(const row of catalog.sources){
  if(!bySource.has(row.source))bySource.set(row.source,new Set());
  for(const type of row.types||[]){
    if(!byType.has(type))throw new Error(`Unknown type ${type}`);
    byType.get(type).add(row.source);bySource.get(row.source).add(type);
  }
}
const entries=map=>[...map].map(([id,set])=>({id,items:[...set].sort()})).sort((a,b)=>b.items.length-a.items.length||a.id.localeCompare(b.id));
const types=entries(byType),sources=entries(bySource);
const multiTypes=types.filter(r=>r.items.length>1),multiSources=sources.filter(r=>r.items.length>1);
const table=(rows,left,right)=>`| ${left} | 数量 | ${right} |\n| --- | --- | --- |\n`+rows.map(r=>`| \`${r.id}\` | ${r.items.length} | ${r.items.map(v=>'`'+v+'`').join('<br>')||'无'} |`).join('\n');
const output=`# 并行类型与来源边界表（生成）

生成命令：\`node scripts/build-parallel-boundaries.cjs\`。

输入：\`model/catalog.json\`；版本：${catalog.version}。

输入 SHA-256：\`${crypto.createHash('sha256').update(normalise(raw)).digest('hex')}\`。

这是映射事实表，不代表类型已实现、源已实测或主实现责任已分配。统计对来源和类型去重；零来源类型和无类型来源也保留。目录变化后重新生成，不手工修改。

类型 ${types.length} 个，独立来源 ${sources.length} 个；多来源类型 ${multiTypes.length} 个，多类型来源 ${multiSources.length} 个。

## 多来源类型（需要明确归一与实施边界）

${table(multiTypes,'类型','供给来源')}

## 多类型来源（需要明确解析与分发责任）

${table(multiSources,'来源','输出类型')}

## 完整类型到来源映射

${table(types,'类型','供给来源')}

## 完整来源到类型映射

${table(sources,'来源','输出类型')}

## 实施责任登记要求

R07 另行记录每个重叠类型的主实现、公共解析责任、身份归一责任、待决边界和验证样例。本表不自动把类型或来源分配给工作线，也不据此将 R07 标记通过。
`;
const dest=path.join(root,'docs/refactor/parallel-boundaries.md');
if(process.argv.includes('--check')){
  if(!fs.existsSync(dest)||fs.readFileSync(dest,'utf8')!==output)throw new Error('Boundary report is stale; regenerate it');
}else fs.writeFileSync(dest,output);
console.log(`Types ${types.length}; sources ${sources.length}; multi-source types ${multiTypes.length}; multi-type sources ${multiSources.length}`);
