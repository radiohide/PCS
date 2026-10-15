'use strict';
// R09 stand-in detail renderer. It turns a query response into plain text so the drill can be
// reviewed without a browser, and it names every contract field it reads.
//
// This is a **stand-in**: the real object pages are S23–S25. It is deliberately not wired into any
// page, and it renders text rather than DOM so the drill check can run it under Node.
const PCScopeDrillView=(()=>{
  // Every contract path this renderer reads. scripts/check-drill.cjs verifies each one is actually
  // produced by the drill, so renaming a field in the contract fails loudly instead of rendering a
  // blank — the point of "modules no longer depend on undocumented fields".
  const PATHS=[
    'contract_version','catalog_version','status',
    'data.summary.alias','data.summary.display_name','data.summary.category','data.summary.type_name',
    'data.summary.type_id','data.summary.identity_status','data.summary.lifecycle','data.summary.scope',
    'data.summary.key_facts','data.summary.changed_at','data.summary.seen_at','data.summary.coverage',
    'data.fields[].field_id','data.fields[].label','data.fields[].kind','data.fields[].status',
    'data.fields[].value','data.fields[].evidence','data.fields[].conflicts',
    'data.actions_preview[].action_id','data.actions_preview[].title','data.actions_preview[].result',
    'data.actions_preview[].phase','data.actions_preview[].precision','data.actions_preview[].occurred_at',
    'data.actions_preview[].interval_from','data.actions_preview[].interval_until',
    'data.actions_preview[].time_derived','data.actions_preview[].evidence',
    'data.actions_preview[].unassigned_reason',
    'data.reference','data.state','data.source','data.observed_at','data.raw_available','data.problems',
    'page.watermark','page.limit','counts.instances','counts.actions','counts.evidence','counts.source_records',
    'warnings[].message',
  ];

  const text=value=>value===null||value===undefined?'':String(value);

  function renderField(row){
    // Status is shown as-is: "not_provided" and "unreadable" are different things and the page must
    // not flatten them into an empty cell.
    const value=row.value===null||row.value===undefined?'':JSON.stringify(row.value);
    const evidence=(row.evidence||[]).map(ref=>ref.kind+':'+ref.id).join('、');
    const conflicts=(row.conflicts||[]).map(entry=>entry.source+'='+JSON.stringify(entry.value)).join(' / ');
    return [row.label,'['+row.status+']',value,evidence?'证据 '+evidence:'',conflicts?'冲突 '+conflicts:'']
      .filter(Boolean).join('  ');
  }

  function renderSummary(summary){
    const lines=[
      summary.display_name+'  ('+summary.type_name+' · '+summary.category+')',
      '  身份 '+summary.alias,
      '  状态 '+summary.identity_status+' / 生命周期 '+summary.lifecycle,
    ];
    if(summary.changed_at)lines.push('  最近变化 '+summary.changed_at);
    if(summary.seen_at)lines.push('  最近观察 '+summary.seen_at);
    for(const fact of summary.key_facts||[])lines.push('  · '+renderField(fact));
    for(const entry of summary.coverage||[])lines.push('  采集 '+(entry.configured?'已启用':'已停用')+' 最近结果 '+entry.last_result);
    return lines;
  }

  function renderActions(actions){
    return (actions||[]).map(action=>{
      const when=action.precision==='interval'
        ? action.interval_from+' → '+action.interval_until
        : text(action.occurred_at);
      const marker=action.time_derived?'（派生）':'';
      const where=action.unassigned_reason?'未归属：'+action.unassigned_reason:'目标已归属';
      return [action.title,'['+action.result+']',when+marker,where,'证据 '+(action.evidence||[]).length+' 份']
        .filter(Boolean).join('  ');
    });
  }

  function renderEvidence(view){
    return [
      view.reference.kind+':'+view.reference.id,
      '['+view.state+']',
      view.source,
      view.observed_at,
      view.raw_available?'原始材料可读':'原始材料已清理',
      (view.problems||[]).join('；'),
    ].filter(Boolean).join('  ');
  }

  function renderDetail(response){
    const lines=[];
    if(response.warnings&&response.warnings.length)lines.push('提示：'+response.warnings.map(entry=>entry.message).join('；'));
    const summary=response.data&&response.data.summary;
    if(summary)lines.push(...renderSummary(summary));
    const fields=response.data&&response.data.fields;
    if(fields&&fields.length){
      lines.push('  字段：');
      for(const row of fields)lines.push('    '+renderField(row));
    }
    const actions=response.data&&response.data.actions_preview;
    if(actions&&actions.length){
      lines.push('  动作：');
      for(const line of renderActions(actions))lines.push('    '+line);
    }
    return lines;
  }

  /// Dispatches on what the response actually carries rather than on the query that produced it.
  /// An earlier version assumed every array was a list of instances and crashed on a list of
  /// actions — the drill is what surfaced that.
  function renderResponse(response){
    const data=response.data;
    if(data&&typeof data==='object'&&!Array.isArray(data)){
      if(data.reference)return renderEvidence(data);
      if(data.summary)return renderDetail(response).join('\n');
    }
    if(Array.isArray(data)){
      if(data.length&&data[0].action_id)return renderActions(data).join('\n');
      if(data.length&&data[0].alias)return data.map(row=>renderSummary(row).join('\n')).join('\n---\n');
      return '('+response.status+'，无条目)';
    }
    return '('+response.status+')';
  }

  return {PATHS,renderField,renderSummary,renderActions,renderEvidence,renderDetail,renderResponse};
})();
if(typeof module!=='undefined')module.exports=PCScopeDrillView;
