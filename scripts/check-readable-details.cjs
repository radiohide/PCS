const assert=require('node:assert/strict');
class Element{constructor(tag,doc){this.tagName=tag;this.ownerDocument=doc;this.children=[];this.attributes={};this.hidden=false;this.classList={add:()=>{}};this.isConnected=true;}append(...nodes){for(const n of nodes){this.children.push(n);n.parentNode=this;}}get childNodes(){return this.children;}set textContent(v){this.value=String(v??'');this.children=[];}get textContent(){return (this.value||'')+this.children.map(n=>n.textContent).join('');}set innerHTML(v){this.children=[];this.tabs=[new Element('button',this.ownerDocument),new Element('button',this.ownerDocument)];this.panels=[new Element('section',this.ownerDocument),new Element('section',this.ownerDocument)];this.panels[1].hidden=true;this.append(...this.tabs,...this.panels);}querySelectorAll(q){return q==='[role=tab]'?this.tabs:q==='[role=tabpanel]'?this.panels:[];}setAttribute(k,v){this.attributes[k]=v;}focus(){this.focused=true;}remove(){this.parentNode.children=this.parentNode.children.filter(c=>c!==this);}}
const doc={createElement:t=>new Element(t,doc)};
global.PCScopePresentation=require('../apps/desktop/ui/event-presentation.js');
global.PCScopeView={eventModel:r=>PCScopePresentation.model(r),title:r=>PCScopePresentation.model(r)?.title||r.kind,itemName:()=> 'Windows 安全审计 / 加密密钥操作',objectName:()=> 'Example',summary:()=> '摘要'};
const details=require('../apps/desktop/ui/object-details.js');
const row={seq:1,source:'pcscope.eventlog.security',kind:'windows.event',occurred_at:'2026-09-11T09:14:44Z',data:{provider:'Microsoft-Windows-Security-Auditing',event_id:5058,writer_process_id:1568,field_schema:'evidence-v2',message:'Source message',fields:{KeyName:'<script>Example</script>',Operation:'%%2457',ReturnCode:'0x0',ClientProcessId:7096}}};
const root=new Element('div',doc);details.mount(root,row,'test');const read=root.panels[0],raw=root.panels[1];
assert.match(read.children[0].textContent,/删除密钥文件/);assert.match(read.children[0].textContent,/<script>/);assert.match(read.textContent,/PID 7096/);
assert(!read.children.some(e=>e.tagName==='p'&&/不能|不代表|需要结合/.test(e.textContent)));
const folded=read.children.filter(e=>e.tagName==='details');assert(folded.some(e=>e.textContent.includes('Source message')));assert(folded.every(e=>!e.open));
const visibleText=node=>node.hidden?'':node.tagName==='details'&&!node.open?(node.children[0]?.textContent||''):(node.value||'')+node.children.map(visibleText).join('');
assert.doesNotMatch(visibleText(read),/PID 7096|0x0|%%2457/,'caller identifiers and raw codes belong in collapsed technical fields');
assert.equal(JSON.parse(raw.children.find(n=>n.tagName==='pre').textContent).data.fields.KeyName,'<script>Example</script>');
root.tabs[1].onclick();assert.equal(read.hidden,true);assert.equal(raw.hidden,false);root.tabs[0].onclick();assert.equal(read.hidden,false);assert.equal(raw.hidden,true);
let compared;global.PCScopeDiff={mount:(_,a,b)=>{compared=[a,b];}};
const snapshot={source:'pcscope.files.metadata',kind:'object.changed',data:{before:{size:1},after:{size:2}}};details.mount(new Element('div',doc),snapshot,'snapshot');assert.deepEqual(compared,[{size:1},{size:2}]);
console.log('PASS: readable DOM fields, folded source data, literal hostile text, raw tab preservation, tabs and snapshot comparison');

const ownRoot=new Element('div',doc);details.mount(ownRoot,{...row,activity:{classification:'self',role:'collector',plugin:'users',run_id:'users/test',pid:42,created_ms:Date.parse('2026-09-11T00:00:00Z'),image:'C:\\PCScope\\collector.exe',basis:'native_process_lifetime'}},'own');assert.match(ownRoot.panels[0].textContent,/自身活动归属/);assert.match(ownRoot.panels[0].textContent,/users\/test/);assert.deepEqual(JSON.parse(ownRoot.panels[1].children.find(n=>n.tagName==='pre').textContent).data,row.data);

const wmiRoot=new Element('div',doc),wmiRow={source:'pcscope.eventlog.microsoft-windows-wmi-activity/operational',kind:'windows.event',data:{provider:'Microsoft-Windows-WMI-Activity',event_id:5858,fields:{'Operation_ClientFailure/Operation':'Start IWbemServices::ExecQuery - root\\cimv2 : SELECT * FROM Win32_Service','Operation_ClientFailure/ResultCode':'0x80041032','Operation_ClientFailure/ClientProcessId':42}}};details.mount(wmiRoot,wmiRow,'wmi');assert.match(wmiRoot.panels[0].textContent,/查询对象列表 · Windows 服务/);const technical=wmiRoot.panels[0].children.find(n=>n.tagName==='details'&&n.textContent.includes('WMI 接口与原始操作'));assert(technical&&!technical.open);assert.match(technical.textContent,/SELECT/);assert.deepEqual(JSON.parse(wmiRoot.panels[1].children.find(n=>n.tagName==='pre').textContent).data,wmiRow.data);
assert.doesNotMatch(visibleText(wmiRoot.panels[0]),/PID 42|0x80041032|IWbemServices|root\\cimv2/);
const processRoot=new Element('div',doc),processRow={source:'pcscope.eventlog.security',kind:'windows.event',data:{provider:'Microsoft-Windows-Security-Auditing',event_id:4688,fields:{NewProcessName:'C:\\Program Files\\Demo\\child.exe',NewProcessId:'0x80'}}};
details.mount(processRoot,processRow,'process');
assert.match(visibleText(processRoot.panels[0]),/child.exe/);
assert.doesNotMatch(visibleText(processRoot.panels[0]),/Program Files|0x80|PID/);
assert.equal(JSON.parse(processRoot.panels[1].children.find(n=>n.tagName==='pre').textContent).data.fields.NewProcessName,processRow.data.fields.NewProcessName,'full paths remain available in original data');
const unknownRoot=new Element('div',doc),unknownRow={source:'pcscope.eventlog.application',kind:'windows.event',data:{provider:'unknown-provider',event_id:88,level:3,channel:'Application'}};
details.mount(unknownRoot,unknownRow,'unknown');
assert.match(visibleText(unknownRoot.panels[0]),/未识别事件/);
assert.doesNotMatch(visibleText(unknownRoot.panels[0]),/unknown-provider|88/);
const unknownSource=unknownRoot.panels[0].children.find(n=>n.tagName==='details'&&n.textContent.includes('来源与记录信息'));
assert(unknownSource&&!unknownSource.open);assert.match(unknownSource.textContent,/unknown-provider/);assert.match(unknownSource.textContent,/88/);
assert.deepEqual(JSON.parse(unknownRoot.panels[1].children.find(n=>n.tagName==='pre').textContent).data,unknownRow.data,'unknown event fields remain exact in the original data tab');

(async()=>{let copied;doc.defaultView={navigator:{clipboard:{writeText:async value=>{copied=value;}}}};const [button,status]=raw.children[0].children;await button.onclick();assert.equal(copied,raw.children.find(n=>n.tagName==='pre').textContent);assert.equal(status.textContent,'已复制');assert.equal(button.disabled,false);doc.defaultView.navigator.clipboard.writeText=async()=>{throw new Error('denied');};await button.onclick();assert.match(status.textContent,/复制失败/);assert.equal(button.disabled,false);console.log('PASS: raw JSON clipboard success, failure feedback and retry');})().catch(e=>{console.error(e);process.exitCode=1;});
