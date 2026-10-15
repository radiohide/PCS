'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const {create}=require('../apps/desktop/ui/backend-connection.js');
const {createModel}=require('../apps/desktop/ui/status-issues.js');
const app=fs.readFileSync(require.resolve('../apps/desktop/ui/app.js'),'utf8');
const connectionCode=app.slice(app.indexOf('function connectionNotice('),app.indexOf('async function collect(){'));
const timerCode=app.slice(app.indexOf('setInterval(async()=>{'),app.indexOf('},3000);')+10);
function harness({online=false,fail=false}={}){
  let time=0,tick,launches=0,renders=0,permissions=0,pid=1;
  const issues=createModel();
  const context={PCScopeBackendConnection:{create:args=>create({...args,now:()=>time})},PCScopeView:{ready:Promise.resolve()},
    PCScopeMonitoring:{switching:false,startup:async()=>{permissions++;}},state:{status:online?{}:null,autoRefresh:true,refreshEpoch:0},
    issues,esc:String,notice(){},
    refreshStatus:async()=>{context.state.status=online?{pid}:null;return online;},
    invoke:async()=>{launches++;if(fail)throw Error('后台程序缺失');online=true;return {};},render:async()=>{renders++;},
    setInterval:fn=>{tick=fn;}};
  vm.runInNewContext(connectionCode+'\n'+timerCode,context);
  return {context,issues,start:()=>context.startupDesktop(),tick:()=>tick(),advance:ms=>{time+=ms;},
    offline:()=>{online=false;},replace:()=>{pid++;},fail:value=>{fail=value;},get launches(){return launches;},get renders(){return renders;},get permissions(){return permissions;}};
}
(async()=>{
  let time=0,launches=0,release;
  const gate=create({now:()=>time,start:()=>{launches++;return new Promise(resolve=>{release=resolve;});}});
  const first=gate.run();assert.equal(gate.run(true),first);await Promise.resolve();assert.equal(launches,1);release(true);await first;
  gate.disconnected();assert.equal(await gate.run(),null,'cooldown must suppress a second launch');
  time=10000;const second=gate.run();await Promise.resolve();release(true);await second;
  gate.disconnected();time+=30000;const third=gate.run();await Promise.resolve();release(true);await third;
  gate.disconnected();time+=30000;assert.equal(await gate.run(),null,'short-lived successes cannot reset the crash-loop budget');
  gate.connected();time+=120000;gate.connected();assert.equal(gate.paused,false,'stable service restores the retry budget');

  const firstBoot=harness();await firstBoot.start();assert.equal(firstBoot.launches,1);assert.equal(firstBoot.renders,1);assert.equal(firstBoot.permissions,1);
  const existing=harness({online:true});await existing.start();assert.equal(existing.launches,0);
  existing.offline();existing.context.state.autoRefresh=false;await existing.tick();assert.equal(existing.launches,1,'connection recovery continues while page refresh is paused');
  const broken=harness({fail:true});await broken.start();assert.equal(broken.launches,1);assert.equal(broken.renders,0,'do not query data before connecting');
  await broken.tick();assert.equal(broken.launches,1);
  broken.advance(10000);await broken.tick();assert.equal(broken.launches,2);
  broken.advance(30000);await broken.tick();assert.equal(broken.launches,3);
  broken.advance(300000);await broken.tick();assert.equal(broken.launches,3);assert.match(broken.issues.list().find(i=>i.id==='backend').impact,/请重试/);
  broken.fail(false);await broken.context.connect(true);assert.equal(broken.launches,4);assert.equal(broken.renders,1);assert.equal(broken.issues.list().some(i=>i.id==='backend'),false);
  const replaced=harness({online:true});await replaced.start();replaced.replace();await replaced.tick();assert.equal(replaced.permissions,2,'a new backend PID must recheck privileges even if the disconnect was not observed');
  const switching=harness();switching.context.PCScopeMonitoring.switching=true;await switching.tick();assert.equal(switching.launches,0);
  const simultaneous=harness();await Promise.all(Array.from({length:20},()=>simultaneous.context.connect(true)));assert.equal(simultaneous.launches,1);assert.equal(simultaneous.renders,1);
  console.log('PASS: auto-start, reconnect, paused refresh, single flight, bounded retries, crash-loop protection, manual retry, stable reset and permission handover.');
})().catch(error=>{console.error(error);process.exitCode=1;});
