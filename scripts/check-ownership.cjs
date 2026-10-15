'use strict';
// R07 ownership check.
//
// Makes three of the readiness plan's conditions mechanical rather than declarative:
//   * every tracked file is claimed by exactly one workstream or shared entry point;
//   * every catalog category and every collection source has exactly one implementation owner, so
//     an overlapping type cannot end up with two;
//   * only the declared integrator writes a database schema version, and generated artifacts are
//     not hand-maintained.
const fs=require('node:fs');
const path=require('node:path');
const {execFileSync}=require('node:child_process');
const root=path.resolve(__dirname,'..');
// Support both the original parent checkout and a Git repository initialized in this project.
// Ownership paths keep their stable pcscope/ prefix in either arrangement.
const projectRepository=fs.existsSync(path.join(root,'.git'));
const repo=projectRepository?root:path.resolve(root,'..');
const ownershipRoot=path.resolve(root,'..');
const p=(...parts)=>path.join(root,...parts);

const ownership=JSON.parse(fs.readFileSync(p('model/ownership.json'),'utf8'));
const catalog=JSON.parse(fs.readFileSync(p('model/catalog.json'),'utf8'));
const inventory=JSON.parse(fs.readFileSync(p('model/sources.json'),'utf8'));
const failures=[];
const check=(condition,message)=>{if(!condition)failures.push(message);};

// ---------- globs ----------
// Translated character by character rather than through a placeholder substitution. A placeholder
// has to be a character that cannot occur in a path, and an earlier version of this function
// carried a literal NUL byte for that purpose — which made git treat the whole file as binary.
function globToRegExp(glob){
  const escaped=glob.replace(/[.+^${}()|[\]\\]/g,'\\$&');
  let pattern='';
  for(let index=0;index<escaped.length;index++){
    if(escaped.startsWith('**',index)){ pattern+='.*'; index+=1; }
    else if(escaped[index]==='*'){ pattern+='[^/]*'; }
    else pattern+=escaped[index];
  }
  return new RegExp('^'+pattern+'$');
}
function matchesAny(file,globs){return globs.some(glob=>globToRegExp(glob).test(file));}

// ---------- tracked files ----------
// Git is an external dependency here, not something on PATH: resolve it the same way build.ps1
// resolves the Rust toolchain, and say what was searched when it is missing.
function findGit(){
  const onPath=(process.env.PATH||'').split(path.delimiter).filter(Boolean)
    .map(directory=>path.join(directory,'git.exe')).find(candidate=>fs.existsSync(candidate));
  if(onPath)return onPath;
  const candidates=[
    path.join(process.env.LOCALAPPDATA||'','Programs','PortableGit','cmd','git.exe'),
    path.join(process.env.ProgramFiles||'','Git','cmd','git.exe'),
    path.join(process.env['ProgramFiles(x86)']||'','Git','cmd','git.exe'),
  ];
  return candidates.find(candidate=>fs.existsSync(candidate))||null;
}
const git=findGit();
if(!git){
  console.error('ownership check: git not found.');
  console.error('It is required to enumerate tracked files. Searched PATH and the standard Git and PortableGit locations;');
  console.error('set PATH to include git.exe, or install Git for Windows.');
  process.exit(2);
}
let tracked;
try{
  // -c lists tracked files, -o lists untracked ones, --exclude-standard honours .gitignore. An
  // untracked file is exactly what this check must catch: it is about to be added, and nothing has
  // decided who owns it yet.
  tracked=execFileSync(git,['-C',repo,'ls-files','-co','--exclude-standard',projectRepository?'.':'pcscope'],{encoding:'utf8'})
    .split('\n').map(line=>line.trim()).filter(Boolean).map(file=>projectRepository?'pcscope/'+file:file);
}catch(error){
  console.error(`ownership check: "${git} ls-files" failed — is ${repo} a git work tree?`);
  throw error;
}

// ---------- structure ----------
const workstreamIds=new Set();
for(const workstream of ownership.workstreams){
  check(!workstreamIds.has(workstream.id),`工作线 ID 重复：${workstream.id}`);
  workstreamIds.add(workstream.id);
  check(Array.isArray(workstream.owns)&&workstream.owns.length>0,`${workstream.id}: 没有声明任何主责范围`);
  for(const glob of workstream.owns){
    check(tracked.some(file=>globToRegExp(glob).test(file)),`${workstream.id}: 主责范围 ${glob} 没有匹配任何文件（范围写错，或该目录已不存在）`);
  }
  for(const reserved of workstream.reserved||[]){
    check(!tracked.some(file=>globToRegExp(reserved).test(file)),`${workstream.id}: 预留范围 ${reserved} 已经有文件，应从 reserved 移入 owns`);
  }
}

// ---------- every file claimed exactly once ----------
const claims=new Map();
const claim=(file,by)=>{
  if(claims.has(file))failures.push(`${file} 被两处认领：${claims.get(file)} 与 ${by}`);
  else claims.set(file,by);
};
for(const workstream of ownership.workstreams){
  for(const file of tracked)if(matchesAny(file,workstream.owns))claim(file,`工作线 ${workstream.id}`);
}
for(const entry of ownership.shared_entry_points){
  check(typeof entry.integrator==='string'&&workstreamIds.has(entry.integrator),`共享入口「${entry.area}」的整合责任人 ${entry.integrator} 不是已声明的工作线`);
  for(const file of tracked)if(matchesAny(file,entry.paths))claim(file,`共享入口「${entry.area}」（整合责任人 ${entry.integrator}）`);
}
const unclaimed=tracked.filter(file=>!claims.has(file));
check(unclaimed.length===0,`有 ${unclaimed.length} 个文件没有任何归属，例如：${unclaimed.slice(0,8).join('、')}`);

// Exactly one integrator per shared entry point.
const areasWithIntegrator=new Map();
for(const entry of ownership.shared_entry_points){
  const key=entry.integrator;
  areasWithIntegrator.set(key,(areasWithIntegrator.get(key)||0)+1);
}

// ---------- one implementation owner per type and per source ----------
const categories=catalog.categories.map(category=>category.id);
for(const category of categories){
  const owner=ownership.type_owners[category];
  check(!!owner,`类别 ${category} 没有类型归属`);
  if(owner){
    check(workstreamIds.has(owner.workstream),`类别 ${category} 的归属工作线 ${owner.workstream} 未声明`);
    check(/^S\d{2}$/.test(owner.step),`类别 ${category} 的实现步骤 ${owner.step} 不是 Sxx 形式`);
  }
}
for(const declared of Object.keys(ownership.type_owners)){
  check(categories.includes(declared),`类型归属里出现了目录中不存在的类别：${declared}`);
}

const rules=ownership.adapter_owner_rules;
for(const source of inventory.sources.map(entry=>entry.source)){
  // A rule matches exactly unless it ends in `.*`, which is a deliberate prefix rule. Allowing an
  // implicit prefix would make `pcscope.software` also claim `pcscope.software.appx-user`.
  const matched=rules.filter(rule=>rule.pattern.endsWith('.*')
    ? source.startsWith(rule.pattern.slice(0,-1))
    : source===rule.pattern);
  if(matched.length===0)failures.push(`来源 ${source} 没有适配器归属`);
  else if(matched.length>1)failures.push(`来源 ${source} 有多个适配器归属：${matched.map(rule=>rule.pattern).join('、')}`);
}
for(const rule of rules){
  check(workstreamIds.has(rule.workstream),`适配器归属 ${rule.pattern} 的工作线 ${rule.workstream} 未声明`);
  check(typeof rule.note==='string'&&rule.note.trim().length>0,`适配器归属 ${rule.pattern} 缺少说明`);
}

// ---------- migration numbers are allocated in one place ----------
const migration=ownership.migration_numbers;
check(!!migration&&Array.isArray(migration.paths)&&migration.paths.length>0,'缺少迁移号归属声明');
const schemaOwners=(migration.paths||[]).map(globToRegExp);
const versionWriters=[];
const unlistedWriters=[];
for(const file of tracked){
  if(!/\.(rs|sql)$/.test(file))continue;
  const text=fs.readFileSync(path.join(ownershipRoot,file),'utf8');
  if(!/PRAGMA\s+user_version\s*=/.test(text))continue;
  versionWriters.push(file);
  if(!schemaOwners.some(owner=>owner.test(file)))unlistedWriters.push(file);
}
// A second writer would let someone allocate a migration number without coordinating it.
for(const file of unlistedWriters){
  failures.push(`${file} 写入 PRAGMA user_version，但它不在声明的迁移号归属内（${(migration.paths||[]).join('、')}）`);
}
check(versionWriters.length>0,'没有任何文件写入 PRAGMA user_version，迁移号归属声明可能已过时');
for(const declared of (migration.paths||[])){
  check(versionWriters.includes(declared),`迁移号归属声明的 ${declared} 实际上不写入 user_version，声明已过时`);
}

// ---------- generated artifacts are not hand-maintained ----------
check(Array.isArray(ownership.generated)&&ownership.generated.length>0,'没有声明任何生成物');
for(const artifact of ownership.generated){
  check(fs.existsSync(path.join(ownershipRoot,artifact.path)),`生成物 ${artifact.path} 不存在，声明已过时`);
  check(fs.existsSync(path.join(ownershipRoot,artifact.generator)),`生成物 ${artifact.path} 声明的生成器 ${artifact.generator} 不存在`);
  // A generated file may live inside a workstream's area — collection-catalog.json is written by the
  // existing collector tooling under the UI directory. What matters is that it is declared here
  // with a real generator, so nobody has to guess whether it may be edited by hand.
}

// ---------- report ----------
const byClaim={};
for(const [,by] of claims)byClaim[by]=(byClaim[by]||0)+1;
console.log(`Ownership: ${ownership.workstreams.length} workstreams, ${ownership.shared_entry_points.length} shared entry points`);
console.log(`Tracked files ${tracked.length}, claimed ${claims.size}, unclaimed ${unclaimed.length}`);
for(const [by,count] of Object.entries(byClaim).sort((a,b)=>b[1]-a[1])){
  console.log(`  ${String(count).padStart(3)}  ${by}`);
}
console.log(`Type owners ${Object.keys(ownership.type_owners).length} categories; adapter rules ${rules.length} covering ${new Set(inventory.sources.map(e=>e.source)).size} sources`);
console.log(`Schema version writers: ${versionWriters.join('、')}`);

if(failures.length){
  console.error(`Ownership check FAILED (${failures.length}):`);
  for(const failure of failures.slice(0,30))console.error(`  ! ${failure}`);
  if(failures.length>30)console.error(`  … and ${failures.length-30} more`);
  process.exit(1);
}
console.log('Ownership verified.');
