#!/usr/bin/env node
'use strict';
// Offline audit of retained action_timeline responses, not a database reader or collector.
// Labels come from the actual renderer. Primary metrics use contract fields and typed formats;
// phrase matching is ONLY the separately labelled visible-uncertainty metric.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),readline=require('node:readline');
const root=path.resolve(__dirname,'..'),target=path.join(root,'target');
const catalog=require('../model/catalog.json'),actions=new Map(catalog.actions.map(a=>[a.id,a]));
const types=new Map(catalog.types.map(t=>[t.id,t]));
const DIMENSIONS=['subject','activity','association','value','result'];
const CLASSIFIER='history-readability-v1';
const resultCodes=new Set(['succeeded','success','failed','failure','observed','partial','cancelled','started','ended','allowed','blocked','requested']);
const protectedStatuses=new Set(['masked','unreadable','conflicting','evidence_pruned']);
const uncertainty=/(?:尚未确认|尚不能确认|暂不能确认|未能确认|未确认|待确认|待核实|未识别|尚未识别|未知|未取得|暂不可用|没有可用名称|未命名|未记录|未提供|无法读取|信息存在冲突)/;
const hash=value=>crypto.createHash('sha256').update(value).digest('hex');
const clip=(value,n=240)=>String(value??'').slice(0,n);
const state=(status,insufficient=false,applicable=true,extra={})=>({status,applicable,insufficient,...extra});
const own=(o,k)=>Object.hasOwn(o||{},k);
const present=value=>value!==null&&value!==undefined;
const nameFactIds=new Set(['display_name','name','caption','description','subject']);
const plainLabel=value=>typeof value==='string'&&value.trim()&&value!=='******'&&value!=='-'&&!/^[{]?[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}[}]?$|^S-\d-|^(?:HKLM|HKCU|HKEY_|machine:)|\|/i.test(value);
const UI_FILES=['readable-events.js','object-names.js','type-presentation.js','type-presentation-data.js'];
let defaultUi;
function loadUi(directory='apps/desktop/ui'){
  const dir=path.resolve(root,directory),files=UI_FILES.map(file=>path.join(dir,file));
  const implementation=Object.fromEntries(files.map(file=>[path.relative(root,file),hash(fs.readFileSync(file))]));
  // A CLI run loads once. Clearing these four modules also makes repeated programmatic runs
  // use the chosen snapshot rather than a previous run's require cache.
  for(const file of files)delete require.cache[require.resolve(file)];
  const names=require(path.join(dir,'object-names.js')),events=require(path.join(dir,'readable-events.js'));
  return {directory:path.relative(root,dir),events,names,implementation};
}
function currentUi(){return defaultUi||(defaultUi=loadUi());}

function canonical(value){
  if(Array.isArray(value))return value.map(canonical);
  if(value&&typeof value==='object')return Object.fromEntries(Object.keys(value).sort().map(key=>[key,canonical(value[key])]));
  return value;
}
function identity(row){
  // action_id/instance_id may be corrected by read-time enrichment. Prefer the stored row ID.
  const id=row.action_row??row._action_row??row.audit_action_row;
  const anchors={occurred_at:row.occurred_at??null,observed_at:row.observed_at??null,interval_from:row.interval_from??null,interval_until:row.interval_until??null};
  if(present(id))return {basis:'action_row_and_time',key:hash(JSON.stringify(canonical({id,...anchors})))};
  const source={action_id:row.context?.stored_action_id||row.action_id,field_id:row.field_id??null,
    stored_instance_id:row.context?.stored_instance_id??row.instance_id??null,...anchors,
    evidence:row.evidence??[],source_record:row.source_record??null,before:row.before??null,after:row.after??null};
  if(!Object.values(anchors).some(Boolean))throw Error('An action without a stored row ID or time anchors cannot establish a frozen comparison identity.');
  return {basis:'stable_action_content',key:hash(JSON.stringify(canonical(source)))};
}
function subject(row,typeId,applicable,visible,ui){
  const {events,names}=ui;
  if(!applicable)return state('no_target_required',false,false);
  const c=row.context||{},facts=c.key_facts||[];
  const noSubject=events.subject({...row,context:{type_id:typeId}});
  if(plainLabel(c.subject_label)&&visible===names.shortName(c.subject_label))
    return state(c.subject_basis==='event_evidence'?'event_evidence_name':c.subject_basis==='current_object_facts'?'current_object_name':'supplied_subject_label');
  if(plainLabel(c.subject_label)&&names.presentation.displayName({...c,type_id:typeId},[],c.subject_label)===visible)
    return state('source_name_alias_explanation',false,true,{certifies_identity:false});
  if(row.action_id==='software.update_history'&&plainLabel(row.source_record?.title)&&visible===row.source_record.title.trim())return state('source_operation_title');
  const protectedName=facts.some(f=>nameFactIds.has(f.field_id)&&(protectedStatuses.has(f.status)||names.presentation.hidden(f.value)||f.status==='known'&&present(f.value)&&typeof f.value!=='string'));
  if(protectedName)return state('name_protected_or_unreadable',true);
  if(visible===noSubject)return state('no_usable_subject_name',true);
  if(names.dictionaryMatched({...c,type_id:typeId}))return state('dictionary_explanation',false,true,{certifies_identity:false});
  const profile=names.presentation.profile(typeId);
  if(profile?.fixed_name||names.presentation.displayName({...c,type_id:typeId},[],c.presentation?.name||c.display_name||''))
    return state('type_or_context_label',false,true,{certifies_identity:false});
  if(facts.some(f=>nameFactIds.has(f.field_id)&&f.status==='known'&&plainLabel(f.value))||c.presentation?.name_status==='source_provided'&&plainLabel(c.presentation.name)||plainLabel(c.display_name))
    return state('source_fact_name');
  // Compare with the renderer's empty-object fallback; a generic purpose label is not a name.
  const empty=names.name({type_id:typeId,record_basis:c.record_basis,identity_status:c.identity_status,key_facts:[]});
  if(visible===empty||visible===names.typeName({type_id:typeId}))return state('purpose_only_or_missing_name',true);
  const derived=facts.some(f=>f.status==='known'&&present(f.value)&&['module_path','path','family','version','session_id','process_name','extension','address','mount_point','local_name','privilege'].includes(f.field_id));
  return derived?state('evidence_derived_label',false,true,{certifies_identity:false}):state('unexplained_subject_label',true);
}
function valueSide(row,typeId,side,ui){
  const {names}=ui;
  const status=row[side+'_status']||'known',value=row[side],c={...row.context,type_id:typeId};
  if(names.presentation.hidden(value))return state('masked',true);
  if(status!=='known')return state(status==='not_applicable'?'not_applicable':`value_${status}`,status!=='not_applicable',status!=='not_applicable');
  if(!present(value))return state('value_not_recorded',true);
  const format=names.presentation.profile(typeId)?.formats?.[row.field_id]||{kind:'text'};
  if(typeId==='security.protection'&&row.field_id==='real_time'&&names.presentation.defenderKind(c)==='ambiguous')return state('value_context_ambiguous',true);
  if(format.unknown?.includes(value))return state('source_reports_unknown',true);
  if(format.kind==='boolean'&&typeof value!=='boolean')return state('boolean_shape_unmapped',true);
  if(format.kind==='enum'){
    const values=Array.isArray(value)?value:[value];
    const mapped=v=>['string','number','boolean'].includes(typeof v)&&Object.keys(format.values||{}).some(k=>k.toLowerCase()===String(v).toLowerCase());
    if(!values.length)return state('enum_not_reported',true);
    if(!values.every(mapped))return state('enum_unmapped',true);
    // An explicitly documented Unknown enum is a recognized source value, but it
    // still does not establish the state. This checks enum descriptors, not prose.
    const unknown=v=>{const key=Object.keys(format.values||{}).find(k=>k.toLowerCase()===String(v).toLowerCase());return key?.toLowerCase()==='unknown'||['状态未知','保护状态未知'].includes(format.values?.[key]);};
    if(values.some(unknown))return state('source_reports_unknown',true);
  }
  if(format.kind==='time'&&(typeof value!=='string'||!/^\d{4}-\d{2}-\d{2}[T ]/.test(value)||Number.isNaN(Date.parse(value))))return state('time_value_unmapped',true);
  if(['bytes','percent','duration'].includes(format.kind)&&(typeof value!=='number'||!Number.isFinite(value)||value<0||format.kind==='percent'&&value>100))return state('numeric_shape_unmapped',true);
  if(format.kind==='details'||typeof value==='object')return state('technical_details_retained');
  return state('value_explained');
}
function analyse(row,index,ui=currentUi()){
  const {events}=ui;
  if(!row||typeof row!=='object'||typeof row.action_id!=='string')throw Error(`Action ${index} is not an action_timeline row.`);
  const c=row.context||{},definition=actions.get(row.action_id),typeId=c.type_id||definition?.target_type||'';
  const targetExpected=Boolean(typeId||present(row.instance_id)||present(c.stored_instance_id)||row.field_id);
  const visible={title:events.title(row),subject:events.subject(row),result:events.result(row),change:events.change(row),impact:events.impact(row),association:events.attribution(row)};
  const dimensions={subject:subject(row,typeId,targetExpected,visible.subject,ui)};
  const historicalMappingIssue=c.history_quality?.status==='legacy_mapping_ambiguous';
  dimensions.activity=historicalMappingIssue?state('legacy_mapping_ambiguous',true):c.classification_status==='unverified'?state('classification_unverified',true):!definition?state('action_unmapped',true):state('activity_explained');
  const standaloneUpdate=row.action_id==='software.update_history'&&plainLabel(row.source_record?.title)&&!present(row.instance_id)&&!present(c.stored_instance_id)&&!['unverified','mismatch'].includes(c.attribution_status);
  if(!targetExpected||standaloneUpdate)dimensions.association=state(standaloneUpdate?'standalone_operation_no_object_link_required':'no_target_required',false,false);
  else if(c.classification_status==='unverified')dimensions.association=state('classification_unverified',true);
  else if(c.attribution_status==='verified')dimensions.association=state(present(row.instance_id)?'verified':'verified_without_target',!present(row.instance_id));
  else if(c.attribution_status==='stored'&&present(row.instance_id)&&!(row.evidence||[]).some(e=>e.kind==='event'))dimensions.association=state('stored_snapshot_association');
  else dimensions.association=state(['unverified','mismatch','unassigned'].includes(c.attribution_status)?c.attribution_status:'association_not_verified',true);
  if(!row.field_id)dimensions.value=state('no_field_change',false,false);
  else {
    const fieldKnown=types.get(typeId)?.fields.some(f=>f.id===row.field_id)||visible.change&&events.fieldLabel(row)!=='属性';
    const sides=row.action_id==='startup.service_state_changed'?['after']:['before','after'];
    const detail=Object.fromEntries(sides.map(side=>[side,valueSide(row,typeId,side,ui)]));
    dimensions.value=state(!fieldKnown?'field_unmapped':Object.values(detail).some(d=>d.insufficient)?'value_incomplete_or_unmapped':'change_values_explained',!fieldKnown||Object.values(detail).some(d=>d.insufficient),true,{sides:detail});
  }
  dimensions.result=state(resultCodes.has(row.result)?`stage_${row.result}`:'result_not_reported_or_unmapped',!resultCodes.has(row.result));
  const secondary={
    historical_mapping_issue:historicalMappingIssue,
    interval_time:row.precision==='interval',
    time_not_exact:row.precision!=='exact'||row.time_derived===true||!row.occurred_at,
    completion_not_established:['started','requested','ended'].includes(row.result),
    impact_inference:uncertainty.test(visible.impact),
    visible_uncertainty_components:Object.keys(visible).filter(key=>uncertainty.test(visible[key])),
  };
  if(row.precision==='interval'||row.time_derived)secondary.visible_uncertainty_components.push('time');
  secondary.visible_uncertainty=secondary.visible_uncertainty_components.length>0;
  const id=identity(row);
  return {index,identity:id.key,identity_basis:id.basis,group:{action:row.action_id,type:typeId||'(none)',field:row.field_id||'(none)'},dimensions,
    substantive_insufficient:DIMENSIONS.some(k=>dimensions[k].applicable&&dimensions[k].insufficient),secondary,
    display:{title:clip(visible.title),subject:clip(visible.subject),change:clip(visible.change),result:clip(visible.result)}};
}
function unwrap(value){
  if(Array.isArray(value))return value.flatMap(unwrap);
  if(value&&typeof value.action_id==='string')return [value];
  if(value?.response)return unwrap(value.response);
  if(value?.ok===false)throw Error('Input contains a failed IPC response.');
  if(value&&own(value,'data')&&(Array.isArray(value.data)||value.data&&typeof value.data==='object'))return unwrap(value.data);
  throw Error('JSONL line must contain an action row or a successful response with a data array.');
}
async function* lines(file,rawHash){
  const stream=fs.createReadStream(file);if(rawHash)stream.on('data',chunk=>rawHash.update(chunk));
  const reader=readline.createInterface({input:stream,crlfDelay:Infinity});let number=0;
  try{for await(let line of reader){number++;if(number===1)line=line.replace(/^\uFEFF/,'');if(!line.trim())continue;let value;try{value=JSON.parse(line);}catch(error){throw Error(`${path.basename(file)} line ${number}: ${error.message}`);}yield {value,line:number};}}
  finally{reader.close();stream.destroy();}
}
function add(obj,key,n=1){obj[key]=(obj[key]||0)+n;}
function newCounts(){return {applicable:0,insufficient:0,statuses:{}};}
function accumulate(groups,key,d){
  let g=groups.get(key);if(!g){g={key,rows:0,substantive_insufficient:0,visible_uncertainty:0,dimension_insufficient:Object.fromEntries(DIMENSIONS.map(k=>[k,0])),samples:[]};groups.set(key,g);}
  g.rows++;if(d.secondary.visible_uncertainty)g.visible_uncertainty++;
  if(d.substantive_insufficient){g.substantive_insufficient++;if(g.samples.length<3)g.samples.push({index:d.index,identity:d.identity,display:d.display,dimensions:d.dimensions});}
  for(const k of DIMENSIONS)if(d.dimensions[k].applicable&&d.dimensions[k].insufficient)g.dimension_insufficient[k]++;
}
const rate=(n,d)=>d?Number((n/d).toFixed(6)):null;
function insideTarget(file){const resolved=path.resolve(root,file);if(!resolved.startsWith(target+path.sep))throw Error('Audit outputs must stay under this repository target/ directory.');return resolved;}
function parseArgs(args){
  const options={};for(let i=0;i<args.length;i++){const arg=args[i];if(arg==='--force')options.force=true;else if(arg==='--help'||arg==='-h')options.help=true;else if(['--out','--compare','--ui'].includes(arg)){if(!args[i+1]||args[i+1].startsWith('--'))throw Error(`${arg} requires a path.`);options[arg.slice(2)]=args[++i];}else if(!arg.startsWith('-')&&!options.input)options.input=arg;else throw Error(`Unexpected argument ${arg}`);}return options;
}
async function run(options){
  const ui=loadUi(options.ui);
  const implementation={...ui.implementation,...Object.fromEntries(['scripts/audit-history-readability.cjs','model/catalog.json'].map(file=>[file,hash(fs.readFileSync(path.join(root,file)))]))};
  const input=path.resolve(root,options.input),beforeStat=fs.statSync(input),inputHash=crypto.createHash('sha256');
  const summaryFile=insideTarget(options.out||path.join('target',path.basename(input).replace(/\.[^.]+$/,'')+'-summary.json'));
  const decisionsFile=summaryFile.replace(/(?:-summary)?\.json$/,'')+'-decisions.jsonl';
  if(summaryFile===decisionsFile||!summaryFile.endsWith('.json'))throw Error('--out must end in .json.');
  let baseline=null,prior=null,priorHash=null,priorFile=null;
  if(options.compare){
    const compareFile=path.resolve(root,options.compare);baseline=JSON.parse(fs.readFileSync(compareFile,'utf8'));
    if(baseline.classifier_version!==CLASSIFIER)throw Error('Comparison requires the same classifier version.');
    priorFile=path.resolve(root,baseline.decisions.path);
    if([summaryFile,decisionsFile].includes(compareFile)||[summaryFile,decisionsFile].includes(priorFile))throw Error('Comparison output must not overwrite the frozen baseline; provide a distinct --out.');
    priorHash=crypto.createHash('sha256');prior=lines(priorFile,priorHash)[Symbol.asyncIterator]();
  }
  if([summaryFile,decisionsFile].includes(input))throw Error('Output cannot overwrite input.');
  if(!options.force&&[summaryFile,decisionsFile].some(f=>fs.existsSync(f)))throw Error('Output already exists; choose --out or explicitly use --force.');
  fs.mkdirSync(path.dirname(summaryFile),{recursive:true});
  const temporary=decisionsFile+`.tmp-${process.pid}`,summaryTemp=summaryFile+`.tmp-${process.pid}`;const fd=fs.openSync(temporary,'wx'),decisionHash=crypto.createHash('sha256');
  const stats={rows:0,substantive_insufficient:0,dimensions:Object.fromEntries(DIMENSIONS.map(k=>[k,newCounts()])),secondary:{},identity_basis:{},partitions:{historical_mapping_ambiguous:0,other_non_name_gaps:0,subject_only_gap:0,no_primary_gap:0}};
  const groups={action:new Map(),type:new Map(),field:new Map()};
  const transitions={rows:0,union:{},dimensions:Object.fromEntries(DIMENSIONS.map(k=>[k,{transitions:{},resolved:0,regressed:0,applicability_changed:0}])),samples:[]};
  let committed=false;
  try{
    for await(const line of lines(input,inputHash))for(const row of unwrap(line.value)){
      const d=analyse(row,++stats.rows,ui);d.source_line=line.line;
      if(prior){
        const oldLine=await prior.next();if(oldLine.done)throw Error(`Comparison has more actions than baseline at row ${d.index}.`);
        const old=oldLine.value.value;
        if(old.index!==d.index||old.identity!==d.identity)throw Error(`Frozen action identity/order differs at row ${d.index}; comparison rejected.`);
        transitions.rows++;add(transitions.union,`${old.substantive_insufficient?'insufficient':'clear'} -> ${d.substantive_insufficient?'insufficient':'clear'}`);
        let changed=false;
        for(const k of DIMENSIONS){const a=old.dimensions[k],b=d.dimensions[k],t=transitions.dimensions[k];add(t.transitions,`${a.status} -> ${b.status}`);
          if(a.applicable!==b.applicable)t.applicability_changed++;
          else if(a.applicable&&a.insufficient!==b.insufficient){t[a.insufficient?'resolved':'regressed']++;changed=true;}
        }
        if(changed&&transitions.samples.length<3)transitions.samples.push({index:d.index,identity:d.identity,before:old.dimensions,after:d.dimensions,display:d.display});
      }
      add(stats.identity_basis,d.identity_basis);if(d.substantive_insufficient)stats.substantive_insufficient++;
      const partition=d.secondary.historical_mapping_issue?'historical_mapping_ambiguous':DIMENSIONS.filter(k=>k!=='subject').some(k=>d.dimensions[k].applicable&&d.dimensions[k].insufficient)?'other_non_name_gaps':d.substantive_insufficient?'subject_only_gap':'no_primary_gap';
      stats.partitions[partition]++;
      for(const k of DIMENSIONS){const s=stats.dimensions[k],v=d.dimensions[k];if(v.applicable)s.applicable++;if(v.applicable&&v.insufficient)s.insufficient++;add(s.statuses,v.status);}
      for(const [key,value]of Object.entries(d.secondary))if(typeof value==='boolean'&&value)add(stats.secondary,key);
      for(const component of d.secondary.visible_uncertainty_components)add(stats.secondary,`visible_component:${component}`);
      for(const kind of Object.keys(groups))accumulate(groups[kind],d.group[kind],d);
      const text=JSON.stringify(d)+'\n';fs.writeSync(fd,text);decisionHash.update(text);
    }
    if(prior&&!(await prior.next()).done)throw Error('Comparison has fewer actions than baseline.');
    if(baseline&&stats.rows!==baseline.denominator.all_retained_actions)throw Error('Baseline summary row count does not match its decision file.');
    if(priorHash&&priorHash.digest('hex')!==baseline.decisions.sha256)throw Error('Baseline decision file changed after its summary was created.');
    const afterStat=fs.statSync(input);if(afterStat.size!==beforeStat.size||afterStat.mtimeMs!==beforeStat.mtimeMs)throw Error('Input changed during the audit; freeze the JSONL export first.');
    for(const [file,digest]of Object.entries(implementation))if(hash(fs.readFileSync(path.join(root,file)))!==digest)throw Error(`Audit implementation changed during the run: ${file}`);
    fs.closeSync(fd);
    for(const s of Object.values(stats.dimensions)){s.rate_all_actions=rate(s.insufficient,stats.rows);s.rate_applicable=rate(s.insufficient,s.applicable);}
    const summary={schema_version:1,classifier_version:CLASSIFIER,generated_at:new Date().toISOString(),
      input:{path:path.relative(root,input),sha256:inputHash.digest('hex'),bytes:beforeStat.size},
      ui:ui.directory,implementation,
      denominator:{all_retained_actions:stats.rows,note:'Every input action is retained, including unknown, protected, unassigned and targetless actions. Applicable denominators are shown separately.'},
      substantive_union:{rows:stats.substantive_insufficient,rate_all_actions:rate(stats.substantive_insufficient,stats.rows),note:'Union of the five applicable information dimensions, not their sum. Time precision and impact inference are excluded.'},
      mutually_exclusive_groups:Object.fromEntries(Object.entries(stats.partitions).map(([key,rows])=>[key,{rows,rate_all_actions:rate(rows,stats.rows)}])),
      dimensions:stats.dimensions,secondary:Object.fromEntries(Object.entries(stats.secondary).map(([key,rows])=>[key,{rows,rate_all_actions:rate(rows,stats.rows)}])),
      identity_basis:stats.identity_basis,
      top_groups:Object.fromEntries(Object.entries(groups).map(([kind,values])=>[kind,[...values.values()].sort((a,b)=>b.substantive_insufficient-a.substantive_insufficient||b.rows-a.rows||a.key.localeCompare(b.key)).slice(0,20)])),
      decisions:{path:path.relative(root,decisionsFile),rows:stats.rows,sha256:decisionHash.digest('hex')},
      comparison:baseline?{baseline:path.relative(root,path.resolve(root,options.compare)),same_count:true,same_order_and_action_identity:true,...transitions}:null,
      limits:[
        'This is a frozen action_timeline presentation audit, not proof that every source was collected or every log schema was understood.',
        'Current object-state coverage and collector health are not measurable from action_timeline alone.',
        'Dictionary and type/purpose labels explain names only; they never certify identity, installation, active use or health.',
        'A legacy_mapping_ambiguous history_quality flag makes activity insufficient even when its name and values are readable; such a row does not prove a system change.',
        'The mutually exclusive groups distinguish a name-only gap from other substantive gaps; no_primary_gap means this audit found none, not that every underlying fact was independently proven.',
        'Visible uncertainty uses phrase matching as a secondary copy metric; it is not the primary quality score.',
        'Unknown activities with no stated target do not invent a subject/association requirement; activity remains insufficient.',
        'Raw technical values deliberately retained in details are not treated as missing values.',
        'Stage started/ended/requested is meaningful; completion uncertainty is reported separately.',
        'If no action_row is supplied, stable-content identity may reject legitimate changed enrichment; retain audit_action_row when exporting for stronger comparisons.',
        'Without a stored row ID, identical duplicate rows cannot be distinguished from one another; stable content, ordinal and total count are still checked.',
      ],privacy:'Local target artifacts may contain account and object display names. Do not publish these samples to public documentation.'};
    fs.writeFileSync(summaryTemp,JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
    fs.renameSync(temporary,decisionsFile);fs.renameSync(summaryTemp,summaryFile);committed=true;
    return {summary:path.relative(root,summaryFile),decisions:path.relative(root,decisionsFile),actions:stats.rows,substantive_insufficient:stats.substantive_insufficient,rate_all_actions:summary.substantive_union.rate_all_actions,compared:Boolean(baseline)};
  }finally{
    if(prior?.return)await prior.return();
    if(!committed){try{fs.closeSync(fd);}catch{}for(const file of [temporary,summaryTemp])if(fs.existsSync(file))fs.unlinkSync(file);}
  }
}
const usage='Usage: node scripts/audit-history-readability.cjs target/history-audit-before.jsonl [--ui target/history-ui-before] [--out target/before-summary.json] [--compare target/before-summary.json] [--force]\nDefault UI is apps/desktop/ui. Use the frozen --ui directory for the baseline. Comparison requires distinct output files and identical ordered action identities. No database or daemon is accessed.';
if(require.main===module){
  Promise.resolve().then(()=>{const options=parseArgs(process.argv.slice(2));if(options.help){console.log(usage);return;}if(!options.input)throw Error(usage);return run(options).then(result=>console.log(JSON.stringify(result)));}).catch(error=>{console.error(error.message);process.exitCode=1;});
}
module.exports={analyse,identity,unwrap,run,parseArgs,loadUi,DIMENSIONS,CLASSIFIER};
