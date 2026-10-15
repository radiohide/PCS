const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const source=fs.readFileSync(require('node:path').join(__dirname,'../apps/desktop/ui/app.js'),'utf8');
const nodes=new Map();const node=id=>{if(!nodes.has(id))nodes.set(id,{setAttribute(k,v){this[k]=v;},removeAttribute(k){delete this[k];}});return nodes.get(id);};
const state={page:'timeline',autoRefresh:true,refreshEpoch:0,status:{collecting:true},refreshPolling:false,loadingLatest:false};
let calls=[],reloads=0,renders=0,tick,response=async()=>({collecting:false,last_collection:{ok:true}});
// Record what the refresh path asks the page for, so the counters it may overwrite stay pinned.
const selectors=[];
state.reloadPage=async()=>{reloads++;};
const context=vm.createContext({issues:{syncStatus(){}},state,connecting:null,recovery:{connected(){},disconnected(){}},connectionNotice(){},PCScopeMonitoring:{switching:false},document:{querySelector:node},content:{querySelector:s=>{selectors.push(s);return null;}},stats:()=>'',api:async op=>{calls.push(op);return response();},PCScopeView:{ready:Promise.resolve()},notice:()=>{throw Error("automatic refresh must not produce a notice");},errorText:String,render:async()=>{renders++;},setInterval:fn=>{tick=fn;}});
vm.runInContext(source.slice(source.indexOf('async function refreshStatus('),source.indexOf('function connectionNotice('))+source.slice(source.indexOf("document.querySelector('#auto-refresh').onclick"),source.indexOf('const timelineFilters=')),context);
(async()=>{
node('#auto-refresh').onclick();assert.equal(state.autoRefresh,false);await tick();assert.deepEqual(calls,['status']);assert.equal(renders,0);
await node('#load-latest').onclick();assert.deepEqual(calls,['status','status']);assert.equal(reloads,1);assert.equal(state.autoRefresh,false);
node('#auto-refresh').onclick();state.status={collecting:true};await tick();assert.equal(renders,1);
let release;response=()=>new Promise(resolve=>{release=resolve;});state.status={collecting:true};const pending=tick();node('#auto-refresh').onclick();release({collecting:false});await pending;assert.equal(state.status.collecting,false);assert.equal(renders,1);
assert(calls.every(op=>op==='status'));assert.equal(node('#load-latest').disabled,false);
// The refresh path may only rewrite the legacy counter block. The overview renders its counters
// from the object model, and a bare `.stats` selector matched that block too, replacing model
// counts with old records/objects/snapshots ones.
assert(selectors.includes('.stats-legacy'),'refresh path must target the legacy counter block');
assert(!selectors.includes('.stats'),'refresh path must not select a bare .stats, which the overview also uses');
console.log('PASS: pause keeps connection checks but stops page refresh; manual refresh stays paused; resume retains completion refresh; in-flight status cannot refresh a paused page; no collection request; refresh rewrites only the legacy counter block.');
})().catch(e=>{console.error(e);process.exitCode=1;});
