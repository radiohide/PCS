'use strict';
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'),read=p=>JSON.parse(fs.readFileSync(path.resolve(root,p),'utf8').replace(/^\uFEFF/,''));
const request=read('target/native-provisioned-public-request.json');
assert.equal(crypto.createHash('sha256').update(fs.readFileSync(request.exe)).digest('hex'),request.sha256.toLowerCase());
assert.equal(read(path.join(request.output,'process-result.json')).exit_code,0);
const complete=read(path.join(request.output,'complete.json'));assert.equal(complete.complete,true);assert.equal(complete.elevated,true);assert.equal(complete.same_user,true);assert.equal(complete.system_configuration_changes,false);assert.equal(complete.recovery_material_read,false);
const referenceDirectory=process.argv[2];assert(referenceDirectory,'Reference directory is required');
const reference=read(path.join(referenceDirectory,'reference.json')).sources.provisioned_winrt_probe;
assert.equal(reference.available,true);assert.equal(reference.packages.length,reference.count);
const rows=fs.readFileSync(path.join(request.output,'migration-sources.jsonl'),'utf8').split(/\r?\n/).filter(Boolean).map(JSON.parse).filter(row=>row.source==='pcscope.software.provisioned');
assert.equal(rows.length,16);assert(rows.every(row=>row.type==='snapshot'&&row.complete===true));
assert.deepEqual(rows.map(row=>row.scope.match(/\/shard:([0-9a-f])$/)?.[1]).sort(),'0123456789abcdef'.split(''));
const items=rows.flatMap(row=>row.items),byId=new Map(items.map(item=>[item.id,item]));assert.equal(byId.size,items.length);assert.equal(items.length,reference.count);
for(const ref of reference.packages){
 const item=byId.get(ref.PackageName);assert(item,'Missing provisioned package identity');
 assert.deepEqual(Object.keys(item.data).sort(),['Architecture','DisplayName','InstallLocation','PackageName','Version']);
 const version=ref.PackageName.match(/_(\d+\.\d+\.\d+\.\d+)_(?:x86|x64|arm|arm64|neutral|x86onarm64)_/i)?.[1];assert(version,'Package full identity must contain its four-part version');
 assert.deepEqual(item.data,{DisplayName:ref.Name,Version:version,Architecture:ref.Architecture,PackageName:ref.PackageName,InstallLocation:ref.InstallLocation});
}
const report={source:'pcscope.software.provisioned',items:items.length,complete_fields:5,shards:16,identities_equal:true,public_api_properties_equal:['DisplayName via deployment name','Architecture','PackageName','InstallLocation'],version_validation:'four numeric Package.Id.Version components agree with version encoded by Windows in its full package identity; earlier PowerShell probe rendered the version struct as a type name, not a usable version reference',legacy_dism:'native and original command both denied; not used as an empty result or successful-reference claim',ordinary_elevation_still_required:true,configuration_changes:false};
fs.writeFileSync(path.join(request.output,'validation.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
