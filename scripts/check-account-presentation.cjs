'use strict';
// Optional local-only real IPC check: node scripts/check-account-presentation.cjs --live-details <file>
// Nothing from the supplied private sample is printed or written into reusable fixtures.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const names=require('../apps/desktop/ui/object-names.js');
const view=require('../apps/desktop/ui/model-view.js');
const summary=(type_id,extra={})=>({type_id,alias:'fixture-'+type_id,identity_status:'provisional',lifecycle:'active',key_facts:[],...extra});
const fact=(field_id,value,status='known')=>({field_id,value,status});
const response=(row,fields=[],actions=[])=>({data:{summary:row,fields,actions_preview:actions,bindings:[]}});
// Remove closed details (including their nested details), then strip tags to inspect default text.
function visible(html){let depth=0,out='';for(const part of html.split(/(<\/?details\b[^>]*>)/i)){if(/^<details\b/i.test(part)){depth++;continue;}if(/^<\/details/i.test(part)){depth--;continue;}if(!depth)out+=part;}return out.replace(/<[^>]*>/g,'');}
const account=summary('account.user'),rights=summary('account.right_assignment');
assert.equal(names.name(account),'账户记录（资料不完整）');
assert.equal(names.name(rights),'账户权限记录（资料不完整）');
for(const row of [account,rights]){
  const list=visible(view.list({data:[row]}));
  assert.match(list,/最近有记录/);assert.match(list,/识别资料不完整/);
  assert.doesNotMatch(list,/最近仍在|对象尚未确认|账户不存在|运行正常/);
  assert.match(visible(view.detail(response(row))),/资料还不完整/);
  assert.doesNotMatch(visible(view.detail(response(row))),/历史日志|类别与来源不符/,'missing information alone cannot establish a classification defect');
}
for(const [row,title] of [[account,'日志中的账户记录'],[rights,'日志中的权限记录']]){
  const event={...row,record_basis:'event_only'};
  assert.equal(names.name(event),title);
  assert.match(visible(view.list({data:[event]})),/尚未取得可核对的当前资料/);
  assert.match(visible(view.detail(response(event))),/尚未取得可核对的当前资料/);
  assert.equal(names.name({...event,identity_status:'confirmed'}),title,'old confirmed event placeholders are not current account inventory');
}
assert.equal(names.name({...account,record_basis:'unknown'}),'账户记录（资料不完整）','unknown source is not assumed to be a log');
const named={...account,key_facts:[fact('name','已提供的账户名称')],record_basis:'event_only'};
assert.equal(names.name(named),'已提供的账户名称','available source names keep priority');
const confirmed={...rights,identity_status:'confirmed',record_basis:'observation',key_facts:[fact('privilege','SeRemoteInteractiveLogonRight')]};
assert.equal(names.name(confirmed),'远程桌面登录权限');
assert.match(visible(view.list({data:[confirmed]})),/远程桌面登录权限/);
const privilegeDetail=view.detail(response({...confirmed,key_facts:[]},confirmed.key_facts));
assert.match(visible(privilegeDetail),/远程桌面登录权限/);
assert.doesNotMatch(visible(privilegeDetail),/SeRemoteInteractiveLogonRight/);
assert.match(privilegeDetail,/SeRemoteInteractiveLogonRight/,'original privilege remains in closed technical details');
assert.equal(names.name({...confirmed,key_facts:[fact('privilege','SeFutureUnknownPrivilege')]}),'账户权限记录');
assert.doesNotMatch(visible(view.list({data:[{...confirmed,key_facts:[fact('privilege','SeFutureUnknownPrivilege')]}]})),/SeFutureUnknownPrivilege/);
for(const status of ['masked','unreadable','conflicting','evidence_pruned']){
  assert.equal(names.name({...confirmed,key_facts:[fact('privilege','SeRemoteInteractiveLogonRight',status)]}),'账户权限记录');
  assert.equal(names.name(confirmed,[fact('privilege','SeRemoteInteractiveLogonRight',status)]),'账户权限记录','detail protection must not fall through to summary facts');
  const protectedSummary={...confirmed,key_facts:[fact('privilege',null,status)]};
  assert.doesNotMatch(visible(view.detail(response(protectedSummary,confirmed.key_facts))),/远程桌面登录权限/,'summary protection must also suppress default detail facts');
  assert.doesNotMatch(visible(view.detail(response(confirmed,[fact('privilege',null,status)]))),/远程桌面登录权限/,'detail protection must suppress stale readable summary facts');
}
const hiddenPrivilege={...confirmed,key_facts:[fact('privilege','******')]};
assert.doesNotMatch(visible(view.detail(response(hiddenPrivilege,confirmed.key_facts))),/远程桌面登录权限/,'mask sentinels cannot be bypassed in the title or main facts');
assert.equal(names.name({...confirmed,key_facts:[fact('name','真实来源名称'),...confirmed.key_facts]}),'真实来源名称');
const userFields=[fact('name','示例账户'),fact('enabled',false),fact('last_logon','2026-10-13T00:00:00Z')];
assert.match(visible(view.detail(response({...account,identity_status:'confirmed'},userFields))),/是否启用.*已停用/);
assert.match(visible(view.detail(response({...account,identity_status:'confirmed'},userFields))),/最近登录.*2026-10-13/);
const mismatch={action_id:'identity.special_privileges',context:{attribution_status:'mismatch',subject_label:'系统账户',subject_basis:'event_evidence'}};
const badClass={action_id:'windows.event',context:{attribution_status:'mismatch',classification_status:'unverified',stored_action_id:'identity.logon_failed'}};
assert.match(visible(view.detail(response(rights,[],[mismatch]))),/关联不一致/);
assert.doesNotMatch(visible(view.detail(response(rights,[],[mismatch]))),/系统账户/,'mismatched event subject must not name the current object');
assert.match(visible(view.detail(response(account,[],[badClass]))),/历史日志的类别与来源不符/);
const unclear={action_id:'windows.event',context:{classification_status:'unverified'}};
assert.doesNotMatch(visible(view.detail(response(account,[],[unclear]))),/类别与来源不符/,'classification uncertainty alone does not prove a source mismatch');
const hostile={...confirmed,alias:'fixture-" onclick="bad',key_facts:[fact('name','<img src=x onerror=bad>'),fact('privilege','<script>bad</script>')]};
assert(!view.list({data:[hostile]}).includes('<img'));assert(!view.detail(response(hostile,hostile.key_facts)).includes('<script>'));
assert(view.list({data:[hostile]}).includes('fixture-&quot; onclick=&quot;bad'),'identity is escaped in its DOM attribute');
const oldFailure={source:'fixture.source',last_result:'collection_failed',last_attempt_at:'2026-10-12T00:00:00Z',history_state:'gap',scope:''};
const newSuccess={source:'fixture.source',last_result:'success',last_attempt_at:'2026-10-13T00:00:00Z',history_state:'caught_up',scope:'new-epoch'};
const covered={...confirmed,coverage:[oldFailure,newSuccess]};
const coverageText=rows=>visible(view.list({data:[{...confirmed,coverage:rows}]}));
assert.match(coverageText(covered.coverage),/已读取/);assert.doesNotMatch(coverageText(covered.coverage),/读取失败|尚未确认/,'an old epoch failure cannot override its newer same-source success');
assert.match(coverageText([newSuccess,{...oldFailure,source:'second.source'}]),/读取失败/,'success from one source must not hide another current source failure');
assert.match(coverageText([{...oldFailure,scope:'another-user-or-partition'},newSuccess]),/读取失败/,'a newer success in one nonempty scope cannot hide another same-source scope failure');
assert.match(coverageText([{...oldFailure,scope:'new-epoch'},newSuccess]),/读取失败/,'only the proven legacy empty scope is superseded; other historical scope records remain conservative');
assert.match(coverageText([newSuccess,{...oldFailure,last_attempt_at:newSuccess.last_attempt_at}]),/读取失败/,'conflicting attempts at the same timestamp must retain failure');
assert.match(coverageText([{...oldFailure,last_attempt_at:null},newSuccess]),/采集结果不完整/,'undated competing records do not establish the latest state');
assert.match(coverageText([newSuccess,{...oldFailure,source:'denied.source',last_result:'denied'}]),/权限不足/);
const coverageDetail=view.detail(response(covered));
assert.doesNotMatch(visible(coverageDetail),/部分历史记录不完整/);assert.doesNotMatch(coverageText(covered.coverage),/部分历史记录不完整/,'historical markers do not create object-level completeness warnings');
assert.match(coverageDetail,/collection_failed/);assert.match(coverageDetail,/new-epoch/);assert.doesNotMatch(visible(coverageDetail),/collection_failed|new-epoch/,'full coverage attempts stay in closed original details');
const args=process.argv.slice(2),liveAt=args.indexOf('--live-details');
let liveCount=0;
if(liveAt>=0){
  assert(args[liveAt+1],'--live-details requires a local JSON path');
  const rows=JSON.parse(fs.readFileSync(args[liveAt+1],'utf8'));
  for(const entry of rows){
    const data=entry.response?.data;if(!data?.summary)continue;
    // Keep only the requested bounded excerpt, even when the input carries thousands of actions.
    const excerpt={data:{summary:data.summary,fields:data.fields||[],actions_preview:(data.actions_preview||[]).slice(0,1),bindings:(data.bindings||[]).slice(0,1)}};
    const html=view.detail(excerpt),text=visible(html),row=data.summary;
    assert.doesNotMatch(text,/最近仍在|对象尚未确认|名称尚未确认/);
    if(row.identity_status==='provisional')assert.match(text,/识别资料不完整/);
    if(row.record_basis==='event_only')assert.match(text,/尚未取得可核对的当前资料/);
    const action=excerpt.data.actions_preview[0];
    if(action?.context?.classification_status==='unverified'&&action.context.attribution_status==='mismatch'&&action.action_id==='windows.event')assert.match(text,/历史日志的类别与来源不符/);
    else if(action?.context?.attribution_status==='mismatch')assert.match(text,/关联不一致/);
    if(excerpt.data.fields.some(field=>field.field_id==='privilege'&&field.status==='known'&&field.value==='SeRemoteInteractiveLogonRight'))assert.match(text,/远程桌面登录权限/);
    assert(html.includes(row.alias.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;')),'original identity remains available');
    liveCount++;
  }
  assert(liveCount>0,'local sample must contain actual detail responses');
}
console.log(`PASS: account names, log-only basis, incomplete identity, grounded explanations, readable rights, protection, exact identities and escaping; ${liveCount} local IPC excerpts.`);
