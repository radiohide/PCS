'use strict';
const assert=require('node:assert/strict');
const events=require('../apps/desktop/ui/readable-events.js');
const presentation=require('../apps/desktop/ui/type-presentation.js');
for(const [operation,phase] of [['installation','安装'],['uninstallation','卸载']]){
  for(const [result,label] of [[2,'成功'],[3,'部分完成'],[4,'失败'],[5,'已取消']]){
    const row={action_id:'software.update_history',phase:operation,result:result===4?'failed':'observed',source_record:{title:'示例更新',result}};
    assert.equal(events.card(row).label,phase+label);assert.equal(events.subject(row),'示例更新');assert.equal(events.impact(row),'');
  }
}
for(const [status,label] of [[3,'威胁已隔离'],[4,'威胁已移除'],[102,'威胁隔离失败']]){
  const row={action_id:'security.threat_status_recorded',source_record:{ThreatStatusID:status}};
  assert.equal(events.card(row).label,label);assert.equal(events.subject(row),'Microsoft Defender 检测');assert.equal(events.impact(row),'');
}
for(const type_id of ['software.update_history_subject','security.detection_history']){
  assert.equal(presentation.state({type_id,lifecycle:'not_observed'}),'历史记录');
}
console.log('Retained history: operation outcomes, detection stages and historical status verified.');
