'use strict';
// Display grouping only. Every row keeps its original timestamps in record details.
const PCScopeTimelineTime=(()=>{
  const tolerance=30000;
  const valid=value=>typeof value==='string'&&Number.isFinite(Date.parse(value));
  const local=value=>new Date(value).toLocaleString('sv-SE',{hour12:false});
  const date=value=>valid(value)?local(value).slice(0,10):'日期未记录';
  const rank=group=>Number.isFinite(group.rank)?group.rank:-Infinity;
  function moment(row){
    const interval=row.precision==='interval'||row.time_derived;
    const complete=interval&&valid(row.interval_from)&&valid(row.interval_until)&&Date.parse(row.interval_from)<=Date.parse(row.interval_until);
    const kind=interval?'interval':valid(row.occurred_at)?'exact':'recorded';
    const start=interval?row.interval_from:kind==='exact'?row.occurred_at:row.observed_at;
    const end=interval?row.interval_until:start;
    const known=interval?complete:valid(start),anchor=valid(end)?end:valid(row.observed_at)?row.observed_at:null;
    return {key:known?JSON.stringify([kind,row.precision||'',start,end]):'',kind,start,end,known,date:date(anchor),rank:anchor?Date.parse(anchor):null};
  }
  function seed(moment){
    const start=moment.known?Date.parse(moment.start):null,end=moment.known?Date.parse(moment.end):null;
    return {...moment,minStart:start,maxStart:start,minEnd:end,maxEnd:end};
  }
  function matches(group,moment){
    if(!group.known||!moment.known||group.kind!==moment.kind)return false;
    const start=Date.parse(moment.start),end=Date.parse(moment.end);
    // Point records use the same bounded tolerance as ranges. Never compare raw timestamp
    // strings: even two source timestamps within one millisecond may have different fractions.
    if(group.kind!=='interval')return Math.max(group.maxEnd,end)-Math.min(group.minStart,start)<=tolerance;
    if(Math.max(group.maxStart,start)-Math.min(group.minStart,start)>tolerance||Math.max(group.maxEnd,end)-Math.min(group.minEnd,end)>tolerance)return false;
    // Identical zero-length source intervals can share a card; adjacent intervals cannot.
    if(group.minStart===start&&group.maxStart===start&&group.minEnd===end&&group.maxEnd===end)return true;
    return Math.max(group.maxStart,start)<Math.min(group.minEnd,end);
  }
  function include(group,moment){
    const start=Date.parse(moment.start),end=Date.parse(moment.end);
    if(start<group.minStart)group.start=moment.start;
    if(end>group.maxEnd)group.end=moment.end;
    group.minStart=Math.min(group.minStart,start);group.maxStart=Math.max(group.maxStart,start);
    group.minEnd=Math.min(group.minEnd,end);group.maxEnd=Math.max(group.maxEnd,end);
    group.rank=group.maxEnd;group.date=date(group.end);
  }
  function groupRows(rows){
    const groups=[];
    // Follow query order so a page break cannot change the greedy grouping decision.
    rows.forEach((row,index)=>{
      const when=moment(row);let group=groups.find(candidate=>matches(candidate,when));
      if(!group){group={...seed(when),order:index,entries:[]};groups.push(group);}else include(group,when);
      group.entries.push({row,index,moment:when});
    });
    return groups.sort((a,b)=>rank(b)-rank(a));
  }
  function range(group){
    if(!group.known)return {start:'时间未记录',end:null};
    const cross=date(group.start)!==date(group.end),stamp=value=>local(value).slice(cross?0:11);
    const multipleSeconds=Math.floor(Date.parse(group.start)/1000)!==Math.floor(Date.parse(group.end)/1000);
    return {start:stamp(group.start),end:group.kind==='interval'||multipleSeconds?stamp(group.end):null};
  }
  return {tolerance,moment,seed,matches,include,groupRows,range,rank};
})();
if(typeof module!=='undefined'&&module.exports)module.exports=PCScopeTimelineTime;
