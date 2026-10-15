(function(root){
  // The service has two read slots. Use one per window and leave room for another reader;
  // status checks and configuration writes never wait behind history queries.
  function create(send){
    const waiting=[],shared=new Map();let running=false;
    function next(){
      if(running||!waiting.length)return;
      running=true;const job=waiting.shift();
      Promise.resolve().then(()=>send(job.request)).then(job.resolve,job.reject).finally(()=>{
        shared.delete(job.key);running=false;next();
      });
    }
    return request=>{
      const key=JSON.stringify(request);
      if(shared.has(key))return shared.get(key);
      if(waiting.length>=32)return Promise.reject(new Error('页面正在读取，请稍后再试。'));
      const promise=new Promise((resolve,reject)=>waiting.push({key,request,resolve,reject}));
      shared.set(key,promise);next();return promise;
    };
  }
  root.PCScopeQueryQueue={create};
  if(typeof module!=='undefined')module.exports={create};
})(typeof window!=='undefined'?window:globalThis);
