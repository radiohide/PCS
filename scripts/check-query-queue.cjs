'use strict';
const assert=require('node:assert/strict');const {create}=require('../apps/desktop/ui/query-queue.js');
(async()=>{
 let running=0,max=0;const started=[],gates=[];
 const send=create(request=>{running++;max=Math.max(max,running);started.push(request.query);return new Promise((resolve,reject)=>gates.push({resolve:value=>{running--;resolve(value);},reject:error=>{running--;reject(error);}}));});
 const requests=['types','instances','actions','coverage'].map(query=>({query}));
 const reads=requests.map(send);assert.equal(send(requests[0]),reads[0],'same in-flight query is shared');
 await new Promise(setImmediate);assert.deepEqual(started,['types']);
 for(let n=0;n<4;n++){gates[n].resolve(n);await new Promise(setImmediate);}
 assert.deepEqual(await Promise.all(reads),[0,1,2,3]);assert.equal(max,1);
 const failed=send({query:'failure'});const rejects=assert.rejects(failed,/test failure/);const next=send({query:'after failure'});
 await new Promise(setImmediate);gates[4].reject(Error('test failure'));await rejects;await new Promise(setImmediate);gates[5].resolve('recovered');assert.equal(await next,'recovered');
 console.log('PASS query scheduling: four overview queries fit the service limit, duplicate requests share a read, failures release the slot');
})().catch(e=>{console.error(e);process.exitCode=1;});
