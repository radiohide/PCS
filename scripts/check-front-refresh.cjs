const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const source=fs.readFileSync(require('node:path').join(__dirname,'../apps/desktop/ui/app.js'),'utf8');
const nodes=new Map();const node=id=>{if(!nodes.has(id))nodes.set(id,{setAttribute(k,v){this[k]=v;},removeAttribute(k){delete this[k];}});return nodes.get(id);};
const state={page:'timeline',autoRefresh:true,refreshEpoch:0,status:{collecting:true},refreshPolling:false,loadingLatest:false};
let calls=[],reloads=0,renders=0,tick,now=100000,response=async()=>({collecting:false,last_collection:{ok:true}});
// Record what the refresh path asks the page for, so the counters it may overwrite stay pinned.
const selectors=[];
state.reloadPage=async()=>{reloads++;};
const context=vm.createContext({Date:{now:()=>now},issues:{syncStatus(){}},state,connecting:null,recovery:{connected(){},disconnected(){}},connectionNotice(){},PCScopeMonitoring:{switching:false},document:{querySelector:node},content:{querySelector:s=>{selectors.push(s);return null;}},stats:()=>'',api:async op=>{calls.push(op);return response();},PCScopeView:{ready:Promise.resolve()},notice:()=>{throw Error("automatic refresh must not produce a notice");},errorText:String,render:async()=>{renders++;},setInterval:fn=>{tick=fn;}});
vm.runInContext(source.slice(source.indexOf('function modelRefreshKey('),source.indexOf('function connectionNotice('))+source.slice(source.indexOf("document.querySelector('#auto-refresh').onclick"),source.indexOf('const timelineFilters=')),context);
(async()=>{
node('#auto-refresh').onclick();assert.equal(state.autoRefresh,false);await tick();assert.deepEqual(calls,['status']);assert.equal(renders,0);
await node('#load-latest').onclick();assert.deepEqual(calls,['status','status']);assert.equal(reloads,1);assert.equal(state.autoRefresh,false);
node('#auto-refresh').onclick();state.status={collecting:true};await tick();assert.equal(reloads,2);assert.equal(renders,0);
let release;response=()=>new Promise(resolve=>{release=resolve;});state.status={collecting:true};const pending=tick();node('#auto-refresh').onclick();release({collecting:false});await pending;assert.equal(state.status.collecting,false);assert.equal(reloads,2);
// New actions must appear even while collection remains active, and after it has finished.
node('#auto-refresh').onclick();state.status={collecting:true,read_model_revision:'1'};
response=async()=>({collecting:true,read_model_revision:'2'});now+=15000;await tick();assert.equal(reloads,3);
response=async()=>({collecting:true,read_model_revision:'3'});now+=3000;await tick();assert.equal(reloads,3);
now+=12000;await tick();assert.equal(reloads,4,'a throttled change must remain pending');
now+=15000;await tick();assert.equal(reloads,4,'unchanged model must not trigger repeated queries');
state.status={collecting:false,read_model_revision:'3'};response=async()=>({collecting:false,read_model_revision:'4'});await tick();assert.equal(reloads,5);
// Existing 0.2.11 backends still refresh when their projector commits material.
state.status={collecting:true,projection_worker:{at:'before'}};response=async()=>({collecting:true,projection_worker:{at:'after'}});now+=15000;await tick();assert.equal(reloads,6);
assert.equal(renders,0,'timeline refresh must retain its page controls');
node('#auto-refresh').onclick();response=async()=>({collecting:false,read_model_revision:'paused-change'});await tick();assert.equal(reloads,6);
node('#auto-refresh').onclick();await tick();assert.equal(reloads,7,'resuming must show changes received while paused');
assert(calls.every(op=>op==='status'));assert.equal(node('#load-latest').disabled,false);
// The refresh path may only rewrite the legacy counter block. The overview renders its counters
// from the object model, and a bare `.stats` selector matched that block too, replacing model
// counts with old records/objects/snapshots ones.
assert(selectors.includes('.stats-legacy'),'refresh path must target the legacy counter block');
assert(!selectors.includes('.stats'),'refresh path must not select a bare .stats, which the overview also uses');
console.log('PASS: collection-independent model refresh, bounded retries, quiet unchanged model, old-backend compatibility, pause and manual refresh, no collection request, retained page controls and model counters.');
})().catch(e=>{console.error(e);process.exitCode=1;});
