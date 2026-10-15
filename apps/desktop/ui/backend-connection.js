(function(root){
  'use strict';
  // Connection recovery is independent of data refresh. A manual click and the timer share
  // one promise; three attempts cover an unstable session, reset only after two steady minutes.
  function create({start,now=Date.now}){
    let pending=null,attempts=0,nextAt=0,stableSince=null;
    const connected=()=>{if(stableSince===null)stableSince=now();if(now()-stableSince>=120000){attempts=0;nextAt=0;}};
    const disconnected=()=>{stableSince=null;};
    function run(manual=false){
      if(pending)return pending;
      if(!manual&&(attempts>=3||now()<nextAt))return Promise.resolve(null);
      if(manual)attempts=0;
      attempts++;
      pending=Promise.resolve().then(()=>start(manual)).then(value=>{connected();return value;}).finally(()=>{
        nextAt=now()+(attempts===1?10000:30000);pending=null;
      });
      return pending;
    }
    return {run,connected,disconnected,get pending(){return Boolean(pending);},get paused(){return attempts>=3;}};
  }
  const api={create};if(typeof module==='object'&&module.exports)module.exports=api;else root.PCScopeBackendConnection=api;
})(typeof window==='undefined'?globalThis:window);
