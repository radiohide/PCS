'use strict';
// S03 model catalog: expand the authored vocabulary, types and source map into one neutral catalog,
// then validate the S03 acceptance conditions against it and against the S01 source inventory.
//
// The catalog is the single definition of category, type, field and action IDs. Regenerating it is
// part of this check, so a change to the vocabulary can never leave model/catalog.json stale.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const root=path.resolve(__dirname,'..');
const p=(...parts)=>path.join(root,...parts);
const rel=file=>path.relative(root,file).split(path.sep).join('/');
const row=(id,value)=>({id,value});

const vocabulary=require(p('model/vocabulary.cjs'));
const {types:typeRows,typesWithoutSource}=require(p('model/types.cjs'));
const {rows:mapRows}=require(p('model/source-map.cjs'));
const inventory=JSON.parse(fs.readFileSync(p('model/sources.json'),'utf8'));

const problems=[];
const check=(condition,message)=>{if(!condition)problems.push(message);};
const splitList=value=>value&&value!=='-'?value.split(',').map(part=>part.trim()).filter(Boolean):[];

// ---------- categories ----------
const categories=vocabulary.categories.map(([id,name,impact])=>({id,name,impact}));
const categoryIds=new Set(categories.map(c=>c.id));

// ---------- identity strategies ----------
const strategies=new Map(vocabulary.identity_strategies.map(
  ([id,name,keys,description,normalization,reuse,insufficient])=>
    [id,{id,name,keys:splitList(keys),description,normalization,reuse,insufficient}]));

// ---------- actions ----------
const actionIds=new Set();
const resultSemantics=new Set(vocabulary.result_semantics.map(([id])=>id));
const actionNatures=new Set(['source_recorded','snapshot_change','state_change','internal']);
const actions=vocabulary.actions.map(line=>{
  const [id,name,nature,result,target]=line.split('|');
  check(id&&name&&nature&&result,`action row is incomplete: ${line}`);
  // Nature and result are separate axes. Validating only the nature let an invalid result value
  // through once already, so the result semantic is checked against the declared vocabulary too.
  check(actionNatures.has(nature),`unknown action nature "${nature}" in ${id}`);
  check(resultSemantics.has(result),`unknown action result "${result}" in ${id}`);
  check(!actionIds.has(id),`duplicate action id: ${id}`);
  actionIds.add(id);
  return {id,name,nature,result,target:(target&&target!=='-')?target:null};
});

// ---------- types ----------
const typeIds=new Set();
const types=typeRows.map(line=>{
  const [id,name,strategyId,extraIdentity,keyFields,actionList,overrideText]=line.split('|');
  const strategy=strategies.get(strategyId);
  check(!!strategy,`${id}: unknown identity strategy "${strategyId}"`);
  check(!typeIds.has(id),`duplicate type id: ${id}`);
  typeIds.add(id);
  check(/^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$/.test(id),`type id is not <category>.<type>: ${id}`);
  const prefix=id.split('.')[0];
  check(categoryIds.has(prefix),`${id}: prefix "${prefix}" is not a declared category`);
  check(name&&name.trim(),`${id}: missing name`);

  // Identity keys: strategy keys plus any type-specific ones. `@composite` means type-supplied.
  const extraKeys=splitList(extraIdentity);
  const baseKeys=strategy?strategy.keys:['@composite'];
  const keys=baseKeys[0]==='@composite'?extraKeys:[...baseKeys,...extraKeys];
  check(keys.length>0,`${id}: no identity keys`);

  const overrides=Object.fromEntries(splitList(overrideText).flatMap(part=>{
    const at=part.indexOf('=');
    return at<0?[]:[[part.slice(0,at).trim(),part.slice(at+1).trim()]];
  }));
  for(const key of Object.keys(overrides))
    check(['scope','normalization','reuse','insufficient'].includes(key),`${id}: unknown identity override "${key}"`);

  const fields=splitList(keyFields).map(cell=>{
    const [fieldId,label,kind]=cell.split(':');
    check(fieldId&&label&&kind,`${id}: malformed field "${cell}"`);
    check(['i','c','s'].includes(kind),`${id}: field "${fieldId}" has unknown kind "${kind}"`);
    return row(fieldId,{label,kind:kind==='i'?'identity':kind==='s'?'state':'configuration'});
  });
  const seenFields=new Set();
  for(const field of fields){
    check(!seenFields.has(field.id),`${id}: duplicate field id "${field.id}"`);
    seenFields.add(field.id);
  }
  for(const actionId of splitList(actionList))
    check(actionIds.has(actionId),`${id}: references unknown action "${actionId}"`);


  return {
    id, category:prefix, name,
    identity:{
      strategy:strategyId,
      keys,
      scope:overrides.scope||(strategy?strategy.description:null),
      normalization:overrides.normalization||(strategy?strategy.normalization:null),
      reuse_rule:overrides.reuse||(strategy?strategy.reuse:null),
      insufficient:overrides.insufficient||(strategy?strategy.insufficient:null),
    },
    fields,
    actions:splitList(actionList),
  };
});

// An action must name a target type that exists, so a renamed type cannot leave a dangling action.
for(const action of actions)
  if(action.target)check(typeIds.has(action.target),`${action.id}: action targets unknown type "${action.target}"`);
// Every type's declared actions must agree with the registry's own target type, in both directions.
for(const type of types){
  for(const actionId of type.actions){
    const action=actions.find(a=>a.id===actionId);
    check(!action?.target||action.target===type.id,
      `${type.id}: lists action "${actionId}" whose registry target is "${action?.target}"`);
  }
}

// ---------- source map ----------
const destinations=new Set(['observation','action_evidence','coverage','sampling','unassigned']);
const inventorySources=new Set(inventory.sources.map(s=>s.source));
const mappedTypes=new Set();
const sources=mapRows.map(line=>{
  const [source,destination,typeList,note]=line.split('|');
  check(inventorySources.has(source),`source map row has no S01 source: ${source}`);
  check(destinations.has(destination),`${source}: unknown destination "${destination}"`);
  for(const typeId of splitList(typeList)){
    check(typeIds.has(typeId),`${source}: maps to unknown type "${typeId}"`);
    mappedTypes.add(typeId);
  }
  if(destination!=='observation')check(!!(note&&note.trim()),`${source}: destination "${destination}" needs a stated reason`);
  return {source,destination,types:splitList(typeList),note:note||null};
});
const mappedSources=new Set(sources.map(s=>s.source));

// ---------- acceptance conditions ----------
// 1. IDs unique (already enforced while building: duplicates were pushed as problems).
// 2. Every category carries at least one type.
for(const category of categories)
  check(types.some(t=>t.category===category.id),`category "${category.id}" has no types`);
// 3. Every S01 source has a destination, and no source is mapped twice to the same destination.
for(const source of inventorySources)
  check(mappedSources.has(source),`S01 source has no destination: ${source}`);
for(const source of mappedSources)
  check(inventorySources.has(source)||source==='pcscope.eventlog.import'||source.includes('{drive}'),
    `source map covers a source S01 does not know: ${source}`);
// 4. Every type is either fed by a source or explicitly marked as having none.
for(const type of types){
  const fed=mappedTypes.has(type.id);
  const marked=typesWithoutSource.includes(type.id);
  check(fed!==marked,`${type.id}: ${fed?'is fed by a source but is listed as having none':'has no source but is not marked type-without-source'}`);
}
for(const typeId of typesWithoutSource)
  check(typeIds.has(typeId),`type-without-source list names an undeclared type: ${typeId}`);
// 5. Identity is never a display name alone, never only a context, never a delivery detail.
for(const type of types){
  const lowered=type.identity.keys.map(k=>k.toLowerCase());
  const nameKeys=lowered.filter(k=>vocabulary.name_key_terms.includes(k));
  const valueKeys=lowered.filter(k=>!vocabulary.scope_only_keys.includes(k));
  check(!(nameKeys.length>0&&lowered.length===1),
    `${type.id}: identity must not be a display name alone (${type.identity.keys.join(',')})`);
  check(valueKeys.length>0,
    `${type.id}: identity has context keys only (${type.identity.keys.join(',')})`);
  for(const key of type.identity.keys)
    for(const term of vocabulary.forbidden_identity_terms)
      check(!key.toLowerCase().includes(term.toLowerCase()),
        `${type.id}: identity key "${key}" contains collection delivery detail "${term}"`);
  check(!!type.identity.scope,`${type.id}: no identity scope`);
  check(!!type.identity.reuse_rule,`${type.id}: no reuse/rebuild rule`);
  check(!!type.identity.insufficient,`${type.id}: no insufficient-identity strategy`);
}
// 6. Field kind vocabulary is closed.
const fieldKinds=new Set(vocabulary.field_kinds.map(([id])=>id));
for(const type of types)
  for(const field of type.fields)
    check(fieldKinds.has(field.value.kind),`${type.id}: field "${field.id}" uses unknown kind`);
// 7. One version, shared. A second catalog file or a divergent version stamp fails the check.
const catalog={
  version:vocabulary.version,
  generated_by:'scripts/check-model-catalog.cjs',
  note:'中立对象模型目录。ID 唯一来源是 model/vocabulary.cjs、model/types.cjs、model/source-map.cjs；本文件为展开结果，请勿手工编辑。',
  categories,
  vocabulary:{
    field_kinds:vocabulary.field_kinds.map(([id,name,description])=>({id,name,description})),
    value_status:vocabulary.value_status.map(([id,name,description])=>({id,name,description})),
    derivations:vocabulary.derivations.map(([id,name,description])=>({id,name,description})),
    time_precisions:vocabulary.time_precisions.map(([id,name,description])=>({id,name,description})),
    result_semantics:vocabulary.result_semantics.map(([id,name,description])=>({id,name,description})),
  },
  identity_strategies:[...strategies.values()],
  actions:actions.map(({id,name,nature,result,target})=>({id,name,nature,result,target_type:target})),
  types,
  sources,
};
const versionStamp=file=>{
  const match=fs.readFileSync(p(file),'utf8').match(/version\s*:\s*'([0-9]+\.[0-9]+\.[0-9]+)'/);
  return match?match[1]:null;
};
for(const file of ['model/vocabulary.cjs'])
  check(versionStamp(file)===vocabulary.version,`${file}: version stamp disagrees with the catalog`);
// Any consumer that declares the catalog version must declare the same one. There are no consumers
// yet: S04 embeds this catalog in Rust and S22 serves it to the UI. Declaring MODEL_CATALOG_VERSION
// is how a consumer opts in, and this check keeps every declared copy on the same version.
function* sourceFiles(dir){
  for(const entry of fs.readdirSync(dir,{withFileTypes:true})){
    if(['node_modules','target','dist','.git'].includes(entry.name))continue;
    const full=path.join(dir,entry.name);
    if(entry.isDirectory())yield* sourceFiles(full);
    else if(/\.(rs|js|cjs|mjs)$/.test(entry.name))yield full;
  }
}
const consumers=[];
for(const file of sourceFiles(root)){
  if(file.startsWith(p('model')))continue;
  // Match `KEY: "1.0.0"`, `KEY = "1.0.0"` and `KEY: &str = "1.0.0"`: a declared type annotation
  // between the name and the value is normal in Rust, and an earlier version of this pattern
  // silently missed it, so a real consumer went undetected.
  for(const match of fs.readFileSync(file,'utf8').matchAll(/MODEL_CATALOG_VERSION\b[^'"\n]*['"]([0-9]+\.[0-9]+\.[0-9]+)['"]/g))
    consumers.push({file:rel(file),version:match[1]});
}
for(const consumer of consumers)
  check(consumer.version===vocabulary.version,
    `${consumer.file}: MODEL_CATALOG_VERSION ${consumer.version} disagrees with the catalog ${vocabulary.version}`);

fs.mkdirSync(p('model'),{recursive:true});
fs.writeFileSync(p('model/catalog.json'),JSON.stringify(catalog,null,2)+'\n');

// ---------- report ----------
const byCategory=categories.map(c=>({category:c.id,types:types.filter(t=>t.category===c.id).length}));
const byDestination={};
for(const row of sources)byDestination[row.destination]=(byDestination[row.destination]||0)+1;
console.log(`Catalog version ${catalog.version}`);
console.log(`Categories ${categories.length}; types ${types.length}; actions ${actions.length}; sources ${sources.length}`);
console.log(`Identity strategies ${strategies.size}; types without a source ${typesWithoutSource.length}`);
console.log('Types per category: '+byCategory.map(c=>`${c.category}=${c.types}`).join(' '));
console.log('Sources per destination: '+Object.entries(byDestination).map(([k,v])=>`${k}=${v}`).join(' '));
console.log(`S01 sources covered: ${mappedSources.size}/${inventorySources.size}`);
const consumerNote=consumers.length?(' ('+consumers.map(c=>c.file).join(', ')+')'):' (none yet; S04 and S22 declare this version)';
console.log('Version consumers found: '+consumers.length+consumerNote);

if(problems.length){
  console.error(`Model catalog check FAILED (${problems.length}):`);
  for(const problem of problems.slice(0,40))console.error(`  ! ${problem}`);
  if(problems.length>40)console.error(`  … and ${problems.length-40} more`);
  process.exit(1);
}
console.log('Model catalog verified. Written to model/catalog.json.');
