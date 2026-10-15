'use strict';
// R06 contract-case check.
//
// Compares the observed results (produced by crates/contracts/examples/contract-cases.rs) against
// the assertions hand-written in fixtures/contract/*/case.json, and validates the case files
// themselves.
//
// What is mechanically enforced about provenance: this script never writes a case file, every
// assertion must carry a non-empty `basis` explaining which evidence supports it, and an assertion
// may not reference the observed output. What cannot be mechanically proven is that a human wrote
// the expectation — that is a review matter, and this comment says so rather than implying the
// check is stronger than it is.
const fs=require('node:fs');
const path=require('node:path');
const root=path.resolve(__dirname,'..');
const p=(...parts)=>path.join(root,...parts);

const casesDirectory=p('fixtures/contract');
const observed=JSON.parse(fs.readFileSync(p('model/contract-observed.json'),'utf8'));
const observedByName=new Map(observed.cases.map(entry=>[entry.case,entry]));
const failures=[];
const check=(condition,message)=>{if(!condition)failures.push(message);};

// ---------- path and match helpers ----------
function at(value,path){
  let current=value;
  for(const segment of path.split('.')){
    if(current===undefined||current===null)return undefined;
    const match=/^([^\[]*)((\[\d+\])*)$/.exec(segment);
    if(!match)return undefined;
    if(match[1])current=current[match[1]];
    for(const index of match[2].match(/\d+/g)||[])current=current===undefined?undefined:current[Number(index)];
  }
  return current;
}
function same(left,right){
  return JSON.stringify(left)===JSON.stringify(right);
}
// Partial match: objects match on the listed keys only, arrays match element-wise, scalars exactly.
function partial(actual,expected){
  if(Array.isArray(expected)){
    return Array.isArray(actual)&&actual.length===expected.length&&expected.every((item,index)=>partial(actual[index],item));
  }
  if(expected&&typeof expected==='object'){
    if(!actual||typeof actual!=='object'||Array.isArray(actual))return false;
    return Object.entries(expected).every(([key,value])=>partial(actual[key],value));
  }
  return same(actual,expected);
}

// ---------- case files ----------
const directories=fs.readdirSync(casesDirectory,{withFileTypes:true}).filter(entry=>entry.isDirectory).map(entry=>entry.name).sort();
check(directories.length>0,'fixtures/contract 下没有任何用例');

const REQUIRED_CASES=[
  '01-task-two-sources-and-run-log',
  '02-same-name-different-path-and-recreated',
  '03-pid-reuse-and-incomplete-event',
  '04-late-log-and-conflicting-value',
  '05-disabled-unreadable-pruned',
  '06-sensitive-value-and-mask',
  '07-unattributable-log-and-gap',
];
for(const required of REQUIRED_CASES){
  check(directories.includes(required),`缺少计划要求的最小样例：${required}`);
}

let assertionCount=0;
for(const directory of directories){
  const casePath=path.join(casesDirectory,directory);
  const entries=fs.readdirSync(casePath).sort();
  check(entries.length===2&&entries.includes('case.json')&&entries.includes('input.jsonl'),
    `${directory}: 用例目录必须恰好包含 case.json 与 input.jsonl，实际为 ${entries.join('、')}`);

  const text=fs.readFileSync(path.join(casePath,'case.json'),'utf8');
  const definition=JSON.parse(text);
  check(definition.name===directory,`${directory}: case.json 的 name ${definition.name} 与目录名不一致`);
  check(typeof definition.description==='string'&&definition.description.trim().length>0,`${directory}: 缺少说明`);
  check(Array.isArray(definition.assertions)&&definition.assertions.length>0,`${directory}: 缺少断言`);

  // A case file must not carry observed output or a generator marker: that is how a bootstrap of
  // "truth" from the implementation under test would look.
  for(const forbidden of ['"observed"','"actual"','"generated_by"','"result_snapshot"']){
    check(!text.includes(forbidden),`${directory}: case.json 含疑似由实现生成的内容 ${forbidden}`);
  }

  const observedCase=observedByName.get(directory);
  check(!!observedCase,`${directory}: 实测结果中没有该用例`);
  if(!observedCase)continue;

  for(const assertion of definition.assertions){
    assertionCount+=1;
    check(typeof assertion.what==='string'&&assertion.what.trim().length>0,`${directory}: 断言缺少 what`);
    check(typeof assertion.path==='string'&&assertion.path.length>0,`${directory}: 断言缺少 path`);
    // Every expectation states the evidence it rests on. An expectation without a basis is not
    // reviewable and would not have survived review.
    check(typeof assertion.basis==='string'&&assertion.basis.trim().length>0,`${directory}: 断言「${assertion.what}」缺少依据说明`);
    if(typeof assertion.path==='string'&&/observed|actual/i.test(assertion.path)){
      failures.push(`${directory}: 断言「${assertion.what}」引用了实测输出，而不是输入或规则`);
    }
    const actual=at(observedCase,assertion.path);
    let ok=false;
    if('equals' in assertion){ ok=partial(actual,assertion.equals); }
    else if('contains' in assertion){ ok=Array.isArray(actual)&&actual.some(entry=>partial(entry,assertion.contains)); }
    else if('min' in assertion){ ok=typeof actual==='number'&&actual>=assertion.min; }
    else if('absent' in assertion){ ok=actual===undefined||actual===null; }
    else if('none' in assertion){ ok=Array.isArray(actual)&&!actual.some(entry=>partial(entry,assertion.none)); }
    else { failures.push(`${directory}: 断言「${assertion.what}」没有可识别的比较方式`); continue; }
    if(!ok){
      failures.push(`${directory}: 「${assertion.what}」不成立 — 路径 ${assertion.path}，期望 ${JSON.stringify(assertion.equals??assertion.contains??assertion.min??assertion.none??'(存在性)')}，实测 ${JSON.stringify(actual)}`);
    }
  }

  // The input must not contain real secrets. The samples are invented data by construction; this
  // catches a careless paste.
  const input=fs.readFileSync(path.join(casePath,'input.jsonl'),'utf8');
  for(const pattern of [/-----BEGIN/,/AKIA[0-9A-Z]{16}/,/gh[pousr]_[A-Za-z0-9]{20,}/,/\b\d{3}-\d{2}-\d{4}\b/,/C:\\Users\\[A-Za-z0-9._-]+/i]){
    check(!pattern.test(input),`${directory}: 输入疑似含真实凭据或个人数据（${pattern}）`);
  }
}

// ---------- every case must actually exercise something ----------
for(const directory of directories){
  const entry=observedByName.get(directory);
  if(!entry)continue;
  const empty=entry.instances.length===0&&entry.actions.length===0&&entry.current_values.length===0&&entry.gaps.length===0;
  check(!empty,`${directory}: 实测结果为空，用例没有覆盖任何内容`);
}

console.log(`Contract cases: ${directories.length} cases, ${assertionCount} assertions, catalog ${observed.catalog_version}`);
if(failures.length){
  console.error(`Contract case check FAILED (${failures.length}):`);
  for(const failure of failures.slice(0,30))console.error(`  ! ${failure}`);
  if(failures.length>30)console.error(`  … and ${failures.length-30} more`);
  process.exit(1);
}
console.log('Contract cases verified.');
