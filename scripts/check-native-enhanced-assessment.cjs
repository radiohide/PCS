'use strict';
// Validate retained, authorized read-only acceptance material. Does not collect or elevate.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..');
const read=p=>JSON.parse(fs.readFileSync(path.resolve(root,p),'utf8').replace(/^\uFEFF/,''));
const expected={
 'pcscope.software.optional-features':['name','state'],
 'pcscope.software.provisioned':['DisplayName','Version','Architecture','PackageName','InstallLocation'],
 'pcscope.security.secureboot':['enabled'],
 'pcscope.security.encryption':['mount_point','volume_type','volume_status','protection_status','encryption_method','encryption_percentage','lock_status','protector_types','recovery_material_collected'],
 'pcscope.security.tpm':['TpmPresent','TpmReady','TpmEnabled','TpmActivated','TpmOwned','RestartPending','ManufacturerIdTxt','ManufacturerVersion','LockedOut']
};
const referenceDirectory=process.argv[2];assert(referenceDirectory,'Reference directory is required');
const reference=read(path.join(referenceDirectory,'reference.json'));
for(const flag of ['system_configuration_changes','owner_auth_read','recovery_secrets_read','self_test_run'])assert.equal(reference[flag],false,flag);
assert.equal(reference.original_account,true);assert.equal(reference.elevated,true);
assert.deepEqual(reference.tpm_reference_excluded_fields,['LockedOut']);
const messages=[];
for(const requestFile of ['target/native-admin-migration-assessment-request.json','target/native-tpm-admin-assessment-request.json']){
 const request=read(requestFile),complete=read(path.join(request.output,'complete.json')),processResult=read(path.join(request.output,'process-result.json'));
 assert.equal(crypto.createHash('sha256').update(fs.readFileSync(request.exe)).digest('hex'),request.sha256.toLowerCase(),'Frozen binary hash');
 assert.equal(processResult.exit_code,0);assert.equal(complete.complete,true);assert.equal(complete.elevated,true);assert.equal(complete.same_user,true);assert.equal(complete.source_queries,request.source_count);
 for(const field of ['system_configuration_changes','recovery_material_read','state_changes','owner_auth_read','self_test_run'])if(field in complete)assert.equal(complete[field],false,field);
 const rows=fs.readFileSync(path.join(request.output,request.source_count===4?'migration-sources.jsonl':'tpm-sources.jsonl'),'utf8').replace(/^\uFEFF/,'').split(/\r?\n/).filter(Boolean).map(JSON.parse);
 assert.deepEqual([...new Set(rows.map(row=>row.source))].sort(),[...request.sources].sort());
 for(const row of rows){assert.equal(row.protocol_version,1);assert(['snapshot','gap'].includes(row.type));assert(expected[row.source]);}
 messages.push(...rows);
}
const results=[];
for(const [source,fields] of Object.entries(expected)){
 const rows=messages.filter(row=>row.source===source),snapshots=rows.filter(row=>row.type==='snapshot'),gaps=rows.filter(row=>row.type==='gap'),ref=reference.sources[source];assert(ref,source+' reference missing');
 if(gaps.length){assert.equal(gaps.length,1);assert.equal(snapshots.length,0);assert.equal(ref.available,false,source+' native failed but reference succeeded');results.push({source,status:'both interfaces unavailable; previous state retained',native_error:gaps[0].reason,reference_error:ref.error});continue;}
 assert.equal(snapshots.length,16,source);assert(snapshots.every(row=>row.complete===true));
 assert.deepEqual(snapshots.map(row=>row.scope.match(/\/shard:([0-9a-f])$/)?.[1]).sort(),'0123456789abcdef'.split(''));
 assert.equal(new Set(snapshots.map(row=>row.scope.replace(/\/shard:[0-9a-f]$/,''))).size,1);
 const items=snapshots.flatMap(row=>row.items),byId=new Map(items.map(item=>[item.id,item]));assert.equal(byId.size,items.length,source+' duplicate identity');
 for(const item of items){assert.equal(typeof item.id,'string');assert(item.id.length);assert.deepEqual(Object.keys(item.data).sort(),[...fields].sort(),source+' complete field set');
  if(source.endsWith('optional-features')){assert.equal(item.id,item.data.name);assert(['Disabled','DisablePending','DisabledWithPayloadRemoved','Enabled','EnablePending','Superseded','PartiallyInstalled'].includes(item.data.state));}
  if(source.endsWith('secureboot')){assert.equal(item.id,'secureboot');assert.equal(typeof item.data.enabled,'boolean');}
  if(source.endsWith('encryption')){assert.equal(item.id,item.data.mount_point);assert.equal(item.data.recovery_material_collected,false);assert(Array.isArray(item.data.protector_types));assert(item.data.encryption_percentage===null||Number.isInteger(item.data.encryption_percentage)&&item.data.encryption_percentage>=0&&item.data.encryption_percentage<=100);}
  if(source.endsWith('.tpm')){assert.equal(item.id,'tpm');for(const key of ['TpmPresent','TpmReady','TpmEnabled','TpmActivated','TpmOwned','RestartPending','LockedOut'])assert.equal(typeof item.data[key],'boolean');}
 }
 assert.equal(ref.available,true,source+' reference unavailable');assert.equal(ref.items.length,items.length,source+' count parity');
 for(const item of ref.items){assert(byId.has(item.id),source+' identity parity');const actual={...byId.get(item.id).data};if(source.endsWith('.tpm'))delete actual.LockedOut;assert.deepEqual(actual,item.data,source+'/'+item.id+' raw field parity');}
 results.push({source,status:'successful complete native read',items:items.length,complete_fields:fields.length,reference_fields:source.endsWith('.tpm')?fields.length-1:fields.length,reference_parity:true});
}
const report={sources:results,formal_database_accessed:false,collection_performed:false,product_powershell_fallback:false,tpm_lockout_reference:'separate fixed unauthenticated capability parser and real ordinary-token query; original self-test/auth constructor intentionally not invoked',provisioned_winrt_probe:reference.sources.provisioned_winrt_probe};
fs.writeFileSync(path.resolve(root,referenceDirectory,'validation.json'),JSON.stringify(report,null,2));
console.log(JSON.stringify(report));
