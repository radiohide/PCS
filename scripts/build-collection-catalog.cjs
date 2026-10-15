const fs=require('node:fs');
const catalog=require('../apps/desktop/ui/plugin-sources.js');
const rows=[];
const shell=['preview','overlay','approval','namespace','file-association','com'];
const tokens=[['\\previewhandlers'],['\\shelliconoverlayidentifiers'],['\\shell extensions\\approved','\\shell extensions\\blocked'],['\\desktop\\namespace','\\mycomputer\\namespace'],['\\classes\\'],['\\classes\\clsid\\']];
for(const list of Object.values(catalog))for(const row of list){
 if(row.source==='pcscope.usn.{drive}')continue;
 let category=row.source.split('.')[1];if(!row.source.startsWith('pcscope.eventlog.')&&row.source.includes('driver'))category='drivers';if(row.source==='pcscope.startup.services')category='services';
 if(category==='eventlog')category='eventlog';
 let item={id:row.id||row.source,source:row.source,name:row.name,category,...(row.event_rule?{event_rule:row.event_rule,group:row.group,leaf:row.leaf}: {})};
 if(row.source==='pcscope.shell.registrations'){
   const i=list.filter(r=>r.source===row.source).indexOf(row);item.id+='.'+shell[i];item.contains=tokens[i];item.priority=i===5?0:i+1;
 }
 if(row.source==='pcscope.startup.logon'){
   const folder=row.name.includes('文件夹');item.id+=folder?'.folders':'.registry';item.contains=folder?[':\\']:['/run/','/runonce/','\\startupapproved'];
 }
 rows.push(item);
}
rows.push({id:'pcscope.usn.changes',source:'pcscope.usn.',prefix:true,name:'文件变更记录',category:'files'});
rows.push({id:'pcscope.eventlog.import',source:'pcscope.eventlog.import.',prefix:true,name:'手动导入的事件日志',category:'eventlog'});
rows.push({id:'pcscope.shell.registrations',source:'pcscope.shell.registrations',name:'资源管理器扩展（未细分／来源状态）',category:'shell'});
rows.push({id:'pcscope.startup.logon',source:'pcscope.startup.logon',name:'登录启动项（未细分／来源状态）',category:'startup'});
fs.writeFileSync(require('node:path').join(__dirname,'../apps/desktop/ui/collection-catalog.json'),JSON.stringify(rows,null,2)+'\n');
console.log('Collection definitions: '+rows.length);
const controls=[];
for(const [plugin,list] of Object.entries(catalog))for(const row of list){
 const definition=rows.find(r=>r.source===row.source&&r.name===row.name)||(row.name==='文件变更记录'?rows.find(r=>r.id==='pcscope.usn.changes'):null);
 if(definition)controls.push({plugin,...definition,event_rule:definition.event_rule?Object.fromEntries(Object.entries(definition.event_rule).filter(([k])=>k!=='actions')):undefined});
}
fs.writeFileSync(require('node:path').join(__dirname,'../apps/desktop/ui/collection-controls.json'),JSON.stringify(controls,null,2)+'\n');
