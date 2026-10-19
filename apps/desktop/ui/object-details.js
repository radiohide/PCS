'use strict';
// Local, deterministic explanations: never infer permissions or health from missing fields.
const PCScopeDetails=(()=>{
  const labels={name:'名称',displayname:'显示名称',displayversion:'版本',version:'版本',publisher:'发布者',company:'公司',manufacturer:'制造商',model:'型号',description:'说明',caption:'名称',enabled:'是否启用',disabled:'是否禁用',state:'状态',status:'状态',startmode:'启动方式',startaccount:'运行账户',servicetype:'服务类型',service:'关联服务',path:'路径',installlocation:'安装位置',installdate:'登记安装日期',installtime:'系统安装时间',createdat:'建立时间',creationdate:'建立时间',lastlogon:'最近登录时间',lastboottime:'最近启动时间',passwordlastset:'密码最近设置时间',accountexpires:'账户到期时间',passwordrequired:'是否要求密码',usermaychangepassword:'用户能否修改密码',principalsource:'账户来源',members:'组成员',rights:'分配的权限',sid:'安全标识符',userid:'运行用户',runlevel:'运行权限级别',actions:'执行操作',execute:'执行程序',workingdirectory:'工作目录',argumentscollected:'是否采集运行参数',commandlinecollected:'是否采集命令行',pid:'进程编号',parentpid:'父进程编号',starttime:'启动时间',sessionid:'会话编号',deviceid:'设备标识',driveletter:'盘符',filesystem:'文件系统',capacity:'容量（字节）',length:'大小（字节）',size:'大小',drivetype:'磁盘类型代码',label:'卷标',macaddress:'MAC 地址',linkspeed:'连接速度',driverversion:'驱动版本',address:'地址',ipaddress:'IP 地址',prefixlength:'网络前缀长度',addressfamily:'地址族',interfaceindex:'接口编号',interfacealias:'接口名称',prefixorigin:'地址前缀来源',suffixorigin:'地址后缀来源',addressstate:'地址状态',hypervisorpresent:'是否检测到虚拟机监控程序',firmwarevirtualization:'固件是否启用虚拟化',vmmonitorextensions:'是否支持虚拟机监控扩展',slat:'是否支持二级地址转换',architecture:'架构',build:'系统内部版本',computername:'计算机名称',timezone:'时区',class:'设备类别',classguid:'类别标识',errorcode:'错误代码',hotfixid:'更新编号',installedon:'登记安装时间',coverage:'采集覆盖说明',basis:'数据依据',createdatbasis:'建立时间依据',installtimebasis:'安装时间依据',hive:'注册表根',view:'注册表视图',registrypath:'注册表路径',registryview:'注册表视图',values:'注册表值',value:'值',kind:'类型',fileversion:'文件版本',lastwriteutc:'最近写入时间',signaturestatus:'签名检查结果',sha256:'SHA-256 摘要',redacted:'是否隐去原值',rawpath:'原始路径',reason:'原因',owner:'所有者',sddl:'安全描述符',acl:'访问控制列表',children:'子项',extensions:'扩展',id:'标识',sampledat:'采样时间',observedat:'观察时间'};
  const normalize=key=>String(key).replace(/[_\s-]/g,'').toLowerCase();
  const evidence=typeof PCScopeEvidence!=='undefined'?PCScopeEvidence:(typeof require==='function'?require('./evidence-explanations.js'):null);
  Object.assign(labels,{message:'来源原始说明',messagestatus:'正文读取状态',messageerror:'正文读取失败原因',collection:'采集范围与限制',policy:'采集策略版本',sourceproperties:'来源提供的完整属性（敏感原值加密保留）',system:'事件系统字段',fieldissues:'字段脱敏与截断记录',command:'启动命令',commandline:'进程命令行',arguments:'运行参数',query:'WMI 查询',documentname:'文档名称',unvisitedsubkeys:'深度范围外子键数',remainingdepth:'剩余遍历深度',rawxmlstored:'是否保留完整源 XML',rawxmlreason:'XML 保存策略'});
  Object.assign(labels,{before:'变化前',after:'变化后',scope:'采集范围',eventid:'事件编号',recordid:'日志记录编号',channel:'日志通道',provider:'事件提供程序',level:'事件级别代码',fields:'事件字段',usersid:'用户安全标识符',writerprocessid:'事件写入进程编号',writerthreadid:'事件写入线程编号',payloadcollected:'是否采集事件正文',sourcetimeavailable:'是否有来源时间',xmlsha256:'事件 XML 摘要',fieldschema:'事件字段格式'});
  Object.assign(labels,{keyname:'密钥名称',keytype:'密钥类型（来源代码）',keyfilepath:'密钥存储文件',algorithmname:'密码算法',providername:'密码存储提供程序',operation:'操作（来源值）',returncode:'操作返回码',clientprocessid:'客户端进程编号',clientcreationtime:'客户端进程建立时间',subjectusername:'操作账户',subjectdomainname:'操作账户域',subjectlogonid:'操作账户登录标识',subjectusersid:'操作账户安全标识符',readoperation:'凭据读取操作',countofcredentialsreturned:'返回凭据数量',resource:'涉及资源'});
  const label=key=>labels[normalize(key)]||String(key);
  function valueText(key,value){
    if(value===null||value===undefined)return '未记录';
    if(typeof value==='boolean')return value?'是':'否';
    if(value==='')return '空值';
    if(value==='******')return '已隐藏';
    const k=normalize(key);
    if(k==='messagestatus'&&value==='protected')return '完整正文已加密保存；是否显示明文由系统配置控制';
    if(value==='full_source_xml_encrypted_display_controlled_by_privacy_setting')return '完整源 XML 已加密保留；默认 ******，系统配置可开启明文';
    if(value==='encrypted_sensitive_field')return '敏感原值已加密保留，默认遮罩显示';
    if(value==='preview_limited_full_value_retained')return '仅预览受限，完整原值已加密保留';
    const evidenceValues={sensitive_field_or_script_excluded:'敏感字段或脚本正文未保存',nested_structure_depth_limit_8:'嵌套超过 8 层，未继续展开',configured_registry_paths_and_depth_only:'按指定注册表路径及深度读取',not_returned_system_process_access_or_exit:'来源未返回：可能是系统进程、权限限制或进程已退出',print_metadata_only_no_document_content:'只保存打印作业属性，不读取文档内容',structured_payload_retained_raw_xml_excluded_for_redaction:'保存结构化事件字段；源 XML 仅留摘要，避免绕过脱敏',reparse_target_outside_scan_policy:'记录目录链接，但不进入范围外的链接目标',sensitive_field:'敏感字段已隐去',secret_redacted:'字段中的秘密信息已脱敏',truncated:'超过长度限制，尾部未保存',payload_budget_exceeded:'结构化预览达到限额，完整内容仍保存在源 XML'};
    if(typeof value==='string'&&Object.hasOwn(evidenceValues,value))return evidenceValues[value];
    if(k==='level'&&evidence)return `${evidence.levels[value]??'未知'}（${value}）`;
    if(k==='messagestatus')return ({available:'已自动获取',not_provided:'来源未提供',unavailable:'自动读取失败',redacted:'已获取并脱敏',truncated:'已获取，超过限制的尾部未保存'})[value]||String(value);
    if(k==='basis')return ({snapshot:'采集时观察到的状态','configuration snapshot':'采集时观察到的配置','observed configuration':'采集时观察到的配置'})[value]||String(value);
    if(k==='createdatbasis'&&value==='not available from this source')return '此采集来源无法提供建立时间';
    const enumFields={state:{Running:'运行中',Stopped:'已停止',Ready:'就绪',Disabled:'已禁用',Unknown:'未知'},status:{Up:'已连接',Disconnected:'已断开',Disabled:'已禁用',Unknown:'未知',OK:'来源报告正常',observed:'已观察到',not_found:'未找到',not_read:'未读取',unknown:'未知'},startmode:{Auto:'自动',Automatic:'自动',Manual:'手动',Disabled:'已禁用',Boot:'引导时启动',System:'系统启动时加载'},runlevel:{Highest:'最高可用权限',Limited:'受限权限'},signaturestatus:{not_verified:'未验证'},hive:{LocalMachine:'本机（HKEY_LOCAL_MACHINE）',CurrentUser:'当前用户（HKEY_CURRENT_USER）'},view:{Registry64:'64 位注册表',Registry32:'32 位注册表'}};
    if(enumFields[k]&&Object.hasOwn(enumFields[k],String(value)))return `${enumFields[k][value]}（${value}）`;
    // Only format complete ISO timestamps; ambiguous numeric codes and dates stay literal.
    if(typeof value==='string'&&/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(value)&&Number.isFinite(Date.parse(value)))return new Date(value).toLocaleString('sv-SE',{hour12:false})+'（本机时间）';
    if(k==='installdate'&&/^\d{8}$/.test(String(value)))return `${String(value).slice(0,4)}-${String(value).slice(4,6)}-${String(value).slice(6)}（来源登记值）`;
    return String(value);
  }
  function explanation(row){
    const names={'object.changed':'属性变化','object.first_seen':'首次记录到此项','baseline.observed':'初始采集记录','object.no_longer_observed':'检查时已不在清单中','coverage.gap':'采集缺口','coverage.recovered':'采集恢复'};
    return names[row.kind]||'';
  }
  function fields(document,data){
    if(data===null||typeof data!=='object'){const p=document.createElement('p');p.textContent=valueText('',data);return p;}
    if(data.redacted===true||data.sensitive===true||data.display==='******'){const p=document.createElement('p');p.textContent='******'+(data.unavailable?'（'+data.unavailable+'）':data.redacted?'（旧版未保存原值，等待来源重新采集）':'（敏感原值已加密保存，可在设置中开启明文显示）');return p;}
    if(Array.isArray(data)){
      if(!data.length){const p=document.createElement('p');p.textContent='列表为空';return p;}
      const list=document.createElement('ol');list.className='readable-list';
      for(const item of data){const li=document.createElement('li');li.append(fields(document,item));list.append(li);}return list;
    }
    const list=document.createElement('dl');list.className='readable-fields';
    for(const [key,value] of Object.entries(data)){
      const term=document.createElement('dt');term.textContent=label(key);term.title=key;
      if(typeof PCScopePresentation!=='undefined'&&PCScopePresentation.help[key]){const hint=document.createElement('abbr');hint.textContent=' ⓘ';hint.title=PCScopePresentation.help[key];hint.tabIndex=0;term.append(hint);}
      const definition=document.createElement('dd');
      if(value!==null&&typeof value==='object')definition.append(fields(document,value));else definition.textContent=valueText(key,value);
      list.append(term,definition);
    }
    if(!list.childNodes.length){const p=document.createElement('p');p.textContent='未提供属性';return p;}return list;
  }
  function displayFields(entries){
    const visible=[],technical=[];
    for(const [key,value] of entries){
      const name=label(key);
      if(/路径|编号|标识|\bID\b|返回码|错误码|结果码|状态码|安全描述符|摘要|操作原码|登录会话|命名空间|任务实例|密钥文件|命令行|运行参数|注册表/i.test(name)) {technical.push([key,value]);continue;}
      let display=value;
      if(typeof value==='string'){
        if(/^(?:S-\d-\d+(?:-\d+)+|\{?[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}\}?)$/i.test(value)){technical.push([key,value]);continue;}
        if(/(?:^| · )PID \S+/.test(display))display=display.replace(/ · PID \S+$/,'').replace(/^PID \S+$/,'尚未识别程序').replace(/^未关联(?:程序)?$/,'尚未识别程序');
        if(/^(?:[a-z]:[\\/]|\\\\)/i.test(display)&&!/[<>]/.test(display))display=display.split(/[\\/]/).filter(Boolean).pop()||'位置待确认';
      }
      if(display!==value)technical.push([key,value]);
      visible.push([key,display]);
    }
    return {visible,technical};
  }
  function eventExplanation(read,row){
    const doc=read.ownerDocument,d=row.data||{};
    read.classList.add('event-readable');
    const line=doc.createElement('div');line.className='event-facts';
    const model=PCScopeView.eventModel?.(row);
    for(const text of [valueText('time',row.occurred_at),PCScopeView.itemName(row)].filter(Boolean)){const span=doc.createElement('span');span.textContent=text;line.append(span);}read.append(line);
    const isSnapshot=['object.changed','object.first_seen','baseline.observed','object.no_longer_observed'].includes(row.kind)&&Object.hasOwn(d,'before')&&Object.hasOwn(d,'after');
    if(model?.result){const badge=doc.createElement('span');badge.className='event-result result-'+model.result.tone;badge.textContent=model.result.label;read.append(badge);}
    const summary=doc.createElement('p');summary.className='event-summary';
    summary.textContent=isSnapshot||row.kind.startsWith('coverage.')?PCScopeView.summary(row):model?'':PCScopeView.summary(row);if(summary.textContent)read.append(summary);
    const technicalFields=[];
    for(const group of model?.groups||[]){const shown=displayFields(group.fields);technicalFields.push(...shown.technical);if(!shown.visible.length)continue;const section=doc.createElement('section');section.className='event-key-fields';const h=doc.createElement('h3');h.textContent=group.name;section.append(h,fields(doc,Object.fromEntries(shown.visible)));read.append(section);}
    const disclosure=(title,value)=>{const box=doc.createElement('details');box.className='event-section';const heading=doc.createElement('summary');heading.textContent=title;box.append(heading,fields(doc,value));read.append(box);return box;};
    if(row.kind.startsWith('coverage.')&&d.reason)disclosure('采集问题详情',{'原因':d.reason});
    if(technicalFields.length)disclosure('技术字段',Object.fromEntries(technicalFields));
    if(model?.technical?.length)disclosure('WMI 接口与原始操作',Object.fromEntries(model.technical));
    if(row.activity?.classification==='self'){
      const a=row.activity,badge=doc.createElement('span');badge.className='self-activity-badge';badge.textContent=PCScopeView.activityLabel?.(row)||'PCScope 自身活动';read.append(badge);
      disclosure('自身活动归属',{'所属组件':a.plugin,'采集运行编号':a.run_id,'已验证进程 ID':a.pid,'进程建立时间':valueText('time',new Date(a.created_ms).toISOString()),'程序路径':a.image,'判定依据':a.basis==='verified_powershell_host'?'PowerShell 4103/4104 的日志宿主 PID 落在 PCScope 已验证的 PowerShell 进程存活区间内':a.basis==='verified_parent_created_process'?'创建者的 PID、完整路径及存活区间与 PCScope 已验证进程匹配；归属指本次启动动作':'事件中的进程身份与 PCScope 通过 Windows 进程句柄验证的存活区间匹配'});
    }
    if(isSnapshot){
      const comparison=doc.createElement('section');comparison.className='snapshot-comparison';read.append(comparison);PCScopeDiff.mount(comparison,d.before,d.after,{fields,label,valueText});
    }else{
      if(d.message)disclosure('来源原始说明',d.message);
      if(d.fields)disclosure('全部事件字段',d.fields);
      const extra=Object.fromEntries(Object.entries(d).filter(([key])=>!['message','fields','provider','event_id'].includes(key)&&!(key==='reason'&&row.kind.startsWith('coverage.'))));
      if(Object.keys(extra).length)disclosure('其他采集属性',extra);
    }
    // Two dead blocks were removed here on 2026-09-12: an inline `task_evidence` call and an
    // `event_related` call, both reached only through `window.__TAURI__` and neither reachable in
    // the shipped app. Their operations are gone from the IPC enum (§19); related records are now
    // answered by the object model's `instance_actions` query.
    const warnings=evidence?evidence.coverage(row):[];
    if(warnings.length)disclosure('数据状态（'+warnings.length+' 项）',warnings);
    disclosure('来源与记录信息',{'提供程序':d.provider,'日志通道':d.channel,'事件编号':d.event_id,'写入进程编号':d.writer_process_id,'采集来源':row.source,'采集范围':d.scope,'记录类型':row.kind,'记录编号':row.seq,'对象标识':row.object_id,'记录时间':valueText('time',row.occurred_at)});
  }
  function mount(container,row,id){
    const doc=container.ownerDocument;
    container.innerHTML=`<div class="object-detail-tabs" role="tablist" aria-label="对象详情显示方式"><button type="button" role="tab" id="${id}-read-tab" aria-controls="${id}-read" aria-selected="true">易读说明</button><button type="button" role="tab" id="${id}-raw-tab" aria-controls="${id}-raw" aria-selected="false" tabindex="-1">原始数据</button></div><section class="object-detail-panel" role="tabpanel" id="${id}-read" aria-labelledby="${id}-read-tab" tabindex="0"></section><section class="object-detail-panel" role="tabpanel" id="${id}-raw" aria-labelledby="${id}-raw-tab" tabindex="0" hidden></section>`;
    const [read,raw]=container.querySelectorAll('[role=tabpanel]');
    const title=doc.createElement('h2');title.textContent=row.kind&&PCScopeView.title?PCScopeView.title(row):PCScopeView.objectName(row);read.append(title);
    if(row.kind)eventExplanation(read,row);else{
    const info=doc.createElement('div');info.className='event-facts';info.textContent=PCScopeView.itemName(row);read.append(info);
    read.append(fields(doc,{'最近成功采集':row.seen_at||null,'最近变化（观察时间）':row.changed_at||null}));
    const groups=typeof PCScopePresentation!=='undefined'?PCScopePresentation.objectGroups(row):[];
    for(const group of groups){const shown=displayFields(group.fields);if(shown.visible.length){const heading=doc.createElement('h3');heading.textContent=group.name;read.append(heading,fields(doc,Object.fromEntries(shown.visible)));}if(shown.technical.length){const technical=doc.createElement('details'),heading=doc.createElement('summary');heading.textContent=group.name+' · 技术详情';technical.append(heading,fields(doc,Object.fromEntries(shown.technical)));read.append(technical);}}
    const all=doc.createElement('details'),allTitle=doc.createElement('summary');allTitle.textContent='全部采集属性';all.append(allTitle,fields(doc,row.data));read.append(all);
    const warnings=evidence?.coverage(row)||[];
    if(warnings.length){const box=doc.createElement('details'),h=doc.createElement('summary');h.textContent='数据状态';box.append(h,fields(doc,warnings));read.append(box);}
    const metadata=doc.createElement('details'),headingMeta=doc.createElement('summary');headingMeta.textContent='来源与对象标识';metadata.append(headingMeta,fields(doc,{'采集来源':row.source,'对象标识':row.id}));read.append(metadata);

    }
    const pre=doc.createElement('pre');pre.textContent=JSON.stringify(row,null,2);
    const toolbar=doc.createElement('div'),copy=doc.createElement('button'),status=doc.createElement('span');
    toolbar.className='raw-data-toolbar';copy.type='button';copy.textContent='复制到剪贴板';status.setAttribute('role','status');status.setAttribute('aria-live','polite');
    copy.onclick=async()=>{
      copy.disabled=true;status.textContent='';
      try{
        const clipboard=doc.defaultView?.navigator?.clipboard;
        if(clipboard?.writeText)await clipboard.writeText(pre.textContent);
        else {
          const input=doc.createElement('textarea'),active=doc.activeElement;
          input.value=pre.textContent;input.setAttribute('readonly','');input.style.cssText='position:fixed;left:-10000px;top:0';doc.body.append(input);
          try{input.select();if(!doc.execCommand('copy'))throw new Error('copy unavailable');}finally{input.remove();active?.focus();}
        }
        status.textContent='已复制';
      }catch{status.textContent='复制失败，请重试或选中文本手动复制';}
      finally{copy.disabled=false;}
    };
    toolbar.append(copy,status);raw.append(toolbar,pre);
    const tabs=[...container.querySelectorAll('[role=tab]')];
    const select=index=>{tabs.forEach((tab,i)=>{tab.setAttribute('aria-selected',String(i===index));tab.tabIndex=i===index?0:-1;});read.hidden=index!==0;raw.hidden=index!==1;};
    tabs.forEach((tab,index)=>{tab.onclick=()=>select(index);tab.onkeydown=e=>{let next;if(e.key==='ArrowRight'||e.key==='ArrowLeft')next=1-index;else if(e.key==='Home')next=0;else if(e.key==='End')next=1;else return;e.preventDefault();select(next);tabs[next].focus();};});
  }
  return {mount,valueText,explanation,label};
})();
if(typeof module!=='undefined')module.exports=PCScopeDetails;
