'use strict';
// Versioned, deterministic rendering of evidence. No diagnosis from an event ID alone.
const PCScopeEvidence=(()=>{
  const version='evidence-rules-3';
  const levels={0:'未指定',1:'严重',2:'错误',3:'警告',4:'信息',5:'详细'};
  function eventSummary(row){return explain(row);}
  function explain(row){
    const presentation=typeof PCScopePresentation!=='undefined'?PCScopePresentation:(typeof require==='function'?require('./event-presentation.js'):null);
    const model=presentation?.model(row);if(model)return model.title+(model.result?' · '+model.result.label:'')+(model.summary?'。'+model.summary:'');
    const d=row.data?.after??row.data?.before??row.data??{};
    if(row.kind==='files.usn_changed'){
      const flags=[[1,'数据覆盖'],[2,'数据扩展'],[4,'数据截断'],[256,'建立'],[512,'删除'],[2048,'权限变更'],[4096,'重命名前名称'],[8192,'重命名后名称'],[32768,'基本属性变更'],[2147483648,'关闭记录']];
      return (d.name||'文件')+' · '+(flags.filter(([bit])=>(Number(d.reason_mask)>>>0)&bit).map(([,name])=>name).join('、')||'未解析 · '+d.reason_mask);
    }
    if(row.kind==='coverage.recovered')return '已恢复采集';
    if(row.kind?.startsWith('coverage.'))return d.reason||'采集缺口';
    return '';
  }
  function coverage(row){
    const d=row.data?.after??row.data?.before??row.data??{},notes=[];
    const scan=(value,path='',depth=0)=>{
      if(!value||typeof value!=='object'||depth>20)return;
      for(const [key,v] of Object.entries(value)){
        const p=path?path+' / '+key:key;
        if(key==='redacted'&&v===true)notes.push(path+'：已按隐私策略脱敏');
        if(/(?:_collected|_stored|_resolved)$/.test(key)&&v===false)notes.push(p+'：未采集或未解析');
        if(key==='unvisited_subkeys'&&v>0)notes.push(`${path}：还有 ${v} 个子键在当前深度范围外`);
        if(key==='fields_not_collected'&&Array.isArray(v)&&v.length)notes.push(`来源另有 ${v.length} 个字段未保存：${v.join('、')}`);
        if(key==='reparse_target_followed'&&v===false)notes.push(path+'：目录链接已记录，未进入链接目标');
        if(key==='signature_status'&&v==='not_verified')notes.push(p+'：签名尚未验证');
        if(key==='unavailable')notes.push(p+'：'+v);
        if(v&&typeof v==='object')scan(v,p,depth+1);
      }
    };scan(d);
    if(row.source?.startsWith('pcscope.eventlog.')&&!row.kind?.startsWith('coverage.')){
      const status={not_provided:'正文未记录',unavailable:'正文读取失败',redacted:'旧版正文已脱敏',protected:'正文已隐藏',truncated:'旧版正文超过长度限制，只有前部'};
      if(status[d.message_status])notes.unshift(status[d.message_status]);
      if(d.collection?.field_issues?.length)notes.unshift(`事件字段有 ${d.collection.field_issues.length} 项保护或预览限制，完整源 XML 的保存情况见采集记录`);
      if(!String(d.field_schema).startsWith('evidence-v'))notes.unshift('旧版记录：正文未保存');
    }
    return [...new Set(notes)];
  }
  return {version,explain,coverage,levels,eventSummary};
})();
if(typeof module!=='undefined')module.exports=PCScopeEvidence;
