'use strict';
// Display buckets only: each record retains its source timestamps.
const PCScopeTimelineTime=(()=>{
  const options=Object.freeze([1,5,10,15,30,60]);
  let minutes=5;
  const normalise=value=>options.includes(Number(value))?Number(value):5;
  const setMinutes=value=>(minutes=normalise(value));
  const getMinutes=()=>minutes;
  const valid=value=>typeof value==='string'&&Number.isFinite(Date.parse(value));
  const pad=value=>String(value).padStart(2,'0');
  const local=value=>{const d=new Date(value);return d.getFullYear()+'-'+pad(d.getMonth()+1)+'-'+pad(d.getDate())+' '+pad(d.getHours())+':'+pad(d.getMinutes())+':'+pad(d.getSeconds());};
  const date=value=>valid(value)?local(value).slice(0,10):'日期未记录';
  const rank=group=>Number.isFinite(group.rank)?group.rank:-Infinity;
  function moment(row,step=minutes){
    step=normalise(step);
    const interval=row.precision==='interval'||row.time_derived;
    const complete=interval&&valid(row.interval_from)&&valid(row.interval_until)&&Date.parse(row.interval_from)<=Date.parse(row.interval_until);
    const recordKind=complete?'interval':!interval&&valid(row.occurred_at)?'exact':'recorded';
    const originalStart=complete?row.interval_from:recordKind==='exact'?row.occurred_at:row.observed_at;
    const originalEnd=complete?row.interval_until:originalStart;
    const source={recordKind,originalStart,originalEnd,minutes:step};
    if(!valid(originalEnd))return {...source,key:'unknown',kind:'bucket',known:false,start:null,end:null,rank:null,date:'日期未记录'};
    const anchor=Date.parse(originalEnd),clock=new Date(anchor);
    // Subtraction retains the offset in repeated local hours at a DST transition.
    const start=anchor-(clock.getMinutes()%step)*60000-clock.getSeconds()*1000-clock.getMilliseconds();
    return {...source,key:`bucket:${step}:${start}`,kind:'bucket',known:true,start:new Date(start).toISOString(),end:new Date(start+step*60000).toISOString(),rank:start,date:date(new Date(start).toISOString())};
  }
  const seed=moment=>({...moment});
  const matches=(group,moment)=>group.key===moment.key;
  const include=()=>{};
  function groupRows(rows){
    const groups=new Map();
    rows.forEach((row,index)=>{
      const when=moment(row);let group=groups.get(when.key);
      if(!group){group={...seed(when),order:index,entries:[]};groups.set(when.key,group);}
      group.entries.push({row,index,moment:when});
    });
    return [...groups.values()].sort((a,b)=>rank(b)-rank(a));
  }
  function range(group){
    if(!group.known)return {start:'时间未记录',end:null};
    const start=new Date(group.start),end=new Date(group.end);
    const repeated=[-3600000,3600000].some(delta=>local(start.getTime()+delta).slice(0,16)===local(start).slice(0,16));
    const offset=value=>{const n=-value.getTimezoneOffset();return ' UTC'+(n>=0?'+':'-')+String(Math.floor(Math.abs(n)/60)).padStart(2,'0')+':'+String(Math.abs(n)%60).padStart(2,'0');};
    const showOffset=repeated||start.getTimezoneOffset()!==end.getTimezoneOffset();
    return {start:local(start).slice(11,16)+(showOffset?offset(start):''),end:(date(group.start)!==date(group.end)?'次日 ':'')+local(end).slice(11,16)+(showOffset?offset(end):'')};
  }
  function recordTime(when){
    if(!valid(when.originalEnd))return '时间未记录';
    const cross=date(when.originalStart)!==date(when.originalEnd),stamp=value=>local(value).slice(cross?0:11);
    if(when.recordKind==='interval')return '变化区间 '+stamp(when.originalStart)+' — '+stamp(when.originalEnd);
    return (when.recordKind==='recorded'?'记录于 ':'')+stamp(when.originalEnd);
  }
  return {options,normalise,setMinutes,getMinutes,moment,seed,matches,include,groupRows,range,recordTime,rank};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeTimelineTime;
