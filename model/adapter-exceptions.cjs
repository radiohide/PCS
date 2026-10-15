'use strict';
// Only the virtual prefix remains; all emitted sources have executable handling paths.
const sourceExceptions={
  'pcscope.usn.changes':'虚拟汇总前缀，无独立采集端；实际按盘来源由事件适配器处理。',
};
const typeExceptions={};
module.exports={sourceExceptions,typeExceptions};
