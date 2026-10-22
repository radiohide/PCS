'use strict';
const assert=require('node:assert/strict'),types=require('../apps/desktop/ui/type-presentation.js');
const row={type_id:'runtime.process',lifecycle:'not_observed',key_facts:[{field_id:'identity_basis',status:'known',value:'event_history'}]};
assert.equal(types.lifecycle(row),'历史进程');
assert.equal(types.lifecycle({...row,key_facts:[...row.key_facts,{field_id:'ended_at',status:'known',value:'2026-10-22T00:00:00Z'}]}),'进程已退出');
assert.equal(types.lifecycle({...row,key_facts:[]}),'已不在运行列表');
console.log('process history lifecycle: history, exit, and native absence remain distinct');
