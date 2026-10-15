using System;
using System.IO;
using System.Text;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Security.Cryptography;
using System.Text.RegularExpressions;
using System.Runtime.InteropServices;
using System.Management;
using Microsoft.Win32;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.System32)]

partial class Platform {
    static object Request;static string Run,Machine;
    internal static string Hash(string value){using(var sha=SHA256.Create())return BitConverter.ToString(sha.ComputeHash(Encoding.UTF8.GetBytes(value))).Replace("-","").ToLowerInvariant();}
    static object Protect(object value) {
        byte[] plain=Encoding.UTF8.GetBytes(Json.Serialize(value));
        try{return D("sensitive",true,"protection","dpapi-current-user-v1","ciphertext",BitConverter.ToString(ProtectedData.Protect(plain,Encoding.UTF8.GetBytes("PCScope/evidence/v1"),DataProtectionScope.CurrentUser)).Replace("-","").ToLowerInvariant());}
        finally{Array.Clear(plain,0,plain.Length);}
    }
    static object Value(string name,object value) {
        if(value==null)return null;
        if(!Regex.IsMatch(name,"(?i)(required|enabled|lastset|expires|age|length|history|policy|type|status)$")&&Regex.IsMatch(name,"(?i)password|passwd|secret|credential|private.?key|recovery.?password|^(token|access.?token|refresh.?token|api.?key|authorization|ScriptBlockText|ScriptText|Script|Binary)$"))return Protect(value);
        if(value is byte[])return D("encoding","base64","bytes",((byte[])value).Length,"value",Convert.ToBase64String((byte[])value));
        if(value is DateTime)return ((DateTime)value).ToUniversalTime().ToString("o");
        if(value is string && Regex.IsMatch((string)value,"(?i)password|passwd|pwd\\s*=|secret|token|api[_-]?key|authorization|private key|bearer\\s|-(enc|encodedcommand)\\s|https?://[^\\s/]+@"))return Protect(value);
        return value;
    }
    static bool Enabled(string source) {
        var disabled=Get(Get(Request,"selection"),"disabled_sources") as IEnumerable;
        if(disabled!=null)foreach(var name in disabled)if(Text(name)==source)return false;
        return true;
    }
    static object Item(string id,object data){if(String.IsNullOrWhiteSpace(id))throw new Exception("Missing identity");return D("id",id,"data",data);}
    static void Gap(string source,string context,Exception error) {
        Console.WriteLine(Json.Serialize(D("protocol_version",1,"request_id",Run+"/"+source+"/"+context+"/error","type","gap","source",source,"observed_at",Now(),"reason","Native source unavailable; previous state retained ["+context+"]: "+error.Message)));
    }
    static bool Emit(string source,Func<List<object>> read,string context="") {
        if(!Enabled(source))return false;string observed=Now();
        try {
            var items=read();var ids=new HashSet<string>(StringComparer.Ordinal);var shards=new List<object>[16];
            for(int i=0;i<16;i++)shards[i]=new List<object>();
            foreach(var item in items){string id=Text(Get(item,"id"));if(!ids.Add(id))throw new Exception("Duplicate identity");shards[Convert.ToInt32(Hash(id).Substring(0,1),16)].Add(item);}
            var lines=new List<string>();
            for(int i=0;i<16;i++) {
                string shard=i.ToString("x");string scope="machine:"+Machine+"/user:"+UserSid()+"/schema:1/context:"+context+"/shard:"+shard;
                string line=Json.Serialize(D("protocol_version",1,"request_id",Run+"/"+source+"/"+context+"/"+shard,"type","snapshot","source",source,"scope",scope,"run_id",Run,"observed_at",observed,"complete",true,"items",shards[i].OrderBy(x=>Text(Get(x,"id")),StringComparer.Ordinal).ToArray()));
                if(Encoding.UTF8.GetByteCount(line)+1>1048576)throw new Exception("Native source shard exceeds protocol limit");lines.Add(line);
            }
            foreach(string line in lines)Console.WriteLine(line); // Encode every shard before committing any.
            return true;
        }catch(Exception error){Gap(source,context,error);return false;}
    }
    static List<object> RegistryRows(string path,int depth) {
        var rows=new List<object>();
        foreach(var view in new[]{RegistryView.Registry64,RegistryView.Registry32})using(var root=RegistryKey.OpenBaseKey(RegistryHive.LocalMachine,view))RegistryWalk(rows,root,"LocalMachine",view.ToString(),path,depth);
        return rows;
    }
    static void RegistryWalk(List<object> rows,RegistryKey root,string hive,string view,string path,int depth) {
        using(var key=root.OpenSubKey(path,false)) {
            if(key==null)return;var values=new Dictionary<string,object>();
            foreach(string name in key.GetValueNames().OrderBy(x=>x,StringComparer.Ordinal))values.Add(name,D("kind",key.GetValueKind(name).ToString(),"value",Value(name,key.GetValue(name,null,RegistryValueOptions.DoNotExpandEnvironmentNames))));
            rows.Add(Item(hive+"/"+view+"/"+path,D("path",path,"hive",hive,"view",view,"values",values,"basis","configuration snapshot","collection",D("policy","evidence-v2","scope","configured_registry_paths_and_depth_only","remaining_depth",depth,"unvisited_subkeys",depth==0?key.SubKeyCount:0))));
            if(depth>0)foreach(string child in key.GetSubKeyNames().OrderBy(x=>x,StringComparer.Ordinal))RegistryWalk(rows,root,hive,view,path+"\\"+child,depth-1);
        }
    }
    static IEnumerable<ManagementObject> Cim(string name) {
        using(var query=new ManagementObjectSearcher("root\\cimv2","SELECT * FROM "+name))using(var rows=query.Get())foreach(ManagementObject row in rows)yield return row;
    }
    static List<object> Resources() {
        var rows=new List<object>();
        foreach(var os in Cim("Win32_OperatingSystem"))using(os)rows.Add(Item("memory",D("total_kib",os["TotalVisibleMemorySize"],"free_kib",os["FreePhysicalMemory"],"sampled_at",Now())));
        foreach(var cpu in Cim("Win32_PerfFormattedData_PerfOS_Processor"))using(cpu)rows.Add(Item("cpu/"+cpu["Name"],D("percent_processor_time",cpu["PercentProcessorTime"],"sampled_at",Now())));
        return rows;
    }
    static List<object> UpdateHistory() {
        RequireStateIntegrity();
        var rows=new List<object>();var occurrences=new Dictionary<string,int>();
        dynamic session=Activator.CreateInstance(Type.GetTypeFromProgID("Microsoft.Update.Session",true));object searcherObject=null;
        try {
            dynamic searcher=session.CreateUpdateSearcher();searcherObject=searcher;int count=searcher.GetTotalHistoryCount();
            for(int offset=0;offset<count;offset+=128) {
                dynamic entries=searcher.QueryHistory(offset,Math.Min(128,count-offset));
                try {for(int i=0;i<entries.Count;i++) {
                    dynamic entry=entries[i];
                    try {
                        dynamic identity=entry.UpdateIdentity;
                        try {
                            string update=Text(identity.UpdateID);int revision=identity.RevisionNumber;DateTime date=entry.Date;int operation=entry.Operation,result=entry.ResultCode,hresult=entry.HResult;
                            string id=update+"/"+revision+"/"+date.Ticks+"/"+operation+"/"+result+"/"+hresult;int occurrence;occurrences.TryGetValue(id,out occurrence);occurrences[id]=++occurrence;
                            var raw=new Dictionary<string,object>();var errors=new Dictionary<string,object>();
                            foreach(string field in new[]{"ClientApplicationID","Date","Description","HResult","Operation","ResultCode","ServerSelection","ServiceID","SupportUrl","Title","UninstallationNotes","UninstallationSteps","UnmappedResultCode"}) {
                                try {object value=entry.GetType().InvokeMember(field,System.Reflection.BindingFlags.GetProperty,null,(object)entry,null);if(field=="UninstallationSteps"){var steps=new List<object>();foreach(object step in (IEnumerable)value)steps.Add(step);value=steps.ToArray();}raw[field]=Value(field,value);}
                                catch(Exception error){errors[field]=error.Message;}
                            }
                            raw["UpdateIdentity"]=D("UpdateID",update,"RevisionNumber",revision);
                            rows.Add(Item(id+"/"+occurrence,D("title",Value("Title",entry.Title),"source_fields",D("properties",raw,"field_errors",errors),"date",date.ToUniversalTime().ToString("o"),"operation",operation,"result",result,"hresult",hresult,"update_id",update,"revision",revision,"basis","retained Windows Update history")));
                        }finally{Marshal.ReleaseComObject(identity);}
                    }finally{Marshal.ReleaseComObject(entry);}
                }}finally{Marshal.ReleaseComObject(entries);}
            }
        }finally{if(searcherObject!=null)Marshal.ReleaseComObject(searcherObject);Marshal.ReleaseComObject(session);}
        return rows;
    }
    static object Options(){return Json.DeserializeObject(File.ReadAllText(Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"collector-options.json")));}
    static string[] Paths(string name){var array=Get(Request,name) as IEnumerable;if(array==null)return new string[0];return array.Cast<object>().Select(Text).ToArray();}
    static string ValidatePath(string path) {
        if(!Regex.IsMatch(path,"^[A-Za-z]:\\\\")||Regex.IsMatch(path.Substring(3),"[:*?<>|\"]"))throw new Exception("Only explicit local paths supported");
        string full=Path.GetFullPath(path);string internalData=Text(Get(Request,"internal_data_dir"));
        if(!String.IsNullOrEmpty(internalData)){string own=Path.GetFullPath(internalData).TrimEnd('\\');if(String.Equals(full,own,StringComparison.OrdinalIgnoreCase)||full.StartsWith(own+"\\",StringComparison.OrdinalIgnoreCase))throw new Exception("PCScope internal data cannot watch itself; scope retained to prevent a write/collection feedback loop");}
        return full;
    }
    static object FileRow(string path,bool directory) {
        string full=ValidatePath(path);
        FileAttributes attributes;
        try{attributes=File.GetAttributes(full);}catch(FileNotFoundException){return Item(full.ToLowerInvariant(),D("path",full,"exists",false,"basis","configured path absent; not proof of deletion"));}catch(DirectoryNotFoundException){return Item(full.ToLowerInvariant(),D("path",full,"exists",false,"basis","configured path absent; not proof of deletion"));}
        if(((attributes&FileAttributes.Directory)!=0)!=directory)throw new Exception("Configured path has the wrong type");
        FileSystemInfo info=directory?(FileSystemInfo)new DirectoryInfo(full):new FileInfo(full);info.Refresh();
        bool reparse=(info.Attributes&FileAttributes.ReparsePoint)!=0;if(directory&&reparse)throw new Exception("Directory must be ordinary; reparse targets not followed");
        var data=D("path",full,"exists",true,"attributes",info.Attributes.ToString(),"last_write_utc",info.LastWriteTimeUtc.ToString("o"));
        if(directory) {
            int limit=Convert.ToInt32(Get(Options(),"max_files_per_root"));var children=new List<object>();
            foreach(var child in ((DirectoryInfo)info).EnumerateFileSystemInfos()) {if(children.Count>=limit)throw new Exception("Directory entry limit reached before allocating further entries");children.Add(D("name",child.Name,"directory",child is DirectoryInfo,"size",child is FileInfo?(object)((FileInfo)child).Length:null,"last_write_utc",child.LastWriteTimeUtc.ToString("o"),"reparse_point",(child.Attributes&FileAttributes.ReparsePoint)!=0));}
            data["entries"]=children.OrderBy(x=>Text(Get(x,"name")),StringComparer.CurrentCulture).ToArray();
            var acl=((DirectoryInfo)info).GetAccessControl(System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner);data["owner"]=Owner(acl);data["dacl_sddl"]=acl.GetSecurityDescriptorSddlForm(System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner);
            data["basis"]="explicit directory and immediate child metadata; no recursion or file content";
        }else {
            var file=(FileInfo)info;data["directory"]=false;data["size"]=file.Length;data["creation_utc"]=file.CreationTimeUtc.ToString("o");data["content_stored"]=false;data["reparse_point"]=reparse;data["hash_status"]="disabled_by_configuration";data["basis"]="explicit file allowlist snapshot; no recursive directory scan";
            if(reparse){data["hash_status"]="reparse_target_not_read";data["security_status"]="reparse_target_not_read";}
            else {
                var acl=file.GetAccessControl(System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner);data["owner"]=Owner(acl);data["dacl_sddl"]=acl.GetSecurityDescriptorSddlForm(System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner);data["streams"]=Streams(full);
                if(Get(Options(),"hash_files") is bool && (bool)Get(Options(),"hash_files")) {
                    if(file.Length>Convert.ToInt64(Get(Options(),"max_hash_bytes")))data["hash_status"]="skipped_size_limit";
                    else {long length=file.Length;DateTime modified=file.LastWriteTimeUtc;using(var stream=File.OpenRead(full))using(var sha=SHA256.Create())data["sha256"]=BitConverter.ToString(sha.ComputeHash(stream)).Replace("-","").ToLowerInvariant();file.Refresh();if(length!=file.Length||modified!=file.LastWriteTimeUtc)throw new Exception("File changed while hashing; snapshot discarded");data["hash_status"]="collected";}
                }
            }
        }
        var identity=PCScopeMetadata.FileIdentity(full);data["volume_identity"]=identity[0];data["file_id"]=identity[1];return Item(full.ToLowerInvariant(),data);
    }
    static string Owner(System.Security.AccessControl.FileSystemSecurity acl) {
        var sid=acl.GetOwner(typeof(System.Security.Principal.SecurityIdentifier));
        try{return sid.Translate(typeof(System.Security.Principal.NTAccount)).Value;}catch(System.Security.Principal.IdentityNotMappedException){return sid.Value;}
    }
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct StreamData {public long size;[MarshalAs(UnmanagedType.ByValTStr,SizeConst=296)]public string name;}
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern IntPtr FindFirstStreamW(string path,int level,out StreamData data,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern bool FindNextStreamW(IntPtr find,out StreamData data);
    [DllImport("kernel32.dll")]static extern bool FindClose(IntPtr find);
    static object Streams(string path) {
        var rows=new List<object>();StreamData data;IntPtr find=FindFirstStreamW(path,0,out data,0);
        if(find==new IntPtr(-1))throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        try {do{string name=data.name=="::$DATA"?":$DATA":data.name.Substring(1,data.name.Length-7);rows.Add(D("name",name,"size",data.size));}while(FindNextStreamW(find,out data));int error=Marshal.GetLastWin32Error();if(error!=38)throw new System.ComponentModel.Win32Exception(error);}finally{FindClose(find);}return rows;
    }
    internal static void Collect(string unit,object request) {
        Request=request;Run=Guid.NewGuid().ToString();using(var root=RegistryKey.OpenBaseKey(RegistryHive.LocalMachine,RegistryView.Registry64))using(var key=root.OpenSubKey("SOFTWARE\\Microsoft\\Cryptography"))Machine=Text(key.GetValue("MachineGuid"));
        CollectRegistryState(unit);
        CollectProviders(unit);
        CollectNetworkConfiguration(unit);
        CollectApplicationFiles(unit);
        CollectPrintingShares(unit);
        CollectDesktopStartup(unit);
        CollectMihomo(unit);
        CollectDocker(unit);
        CollectHypervDisks(unit);
        CollectHypervSnapshots(unit);
        CollectHypervMachines(unit);
        CollectHypervSwitches(unit);
        CollectTpmMetadata(unit);
        CollectProvisionedPackages(unit);
        CollectAppPackages(unit);
        CollectEncryption(unit);
        CollectRemoteAccess(unit);
        CollectLocalGroups(unit);
        CollectLocalAccounts(unit);
        CollectServicingFirmware(unit);
        switch(unit) {
            case "desktop":case "virtualization":break;
            case "base":case "pcscope-powershell-adapter.exe":CollectBaseState();break;
            case "applications": Emit("pcscope.apps.dotnet-runtimes",()=>{var rows=RegistryRows("SOFTWARE\\dotnet\\Setup\\InstalledVersions",4);rows.AddRange(RegistryRows("SOFTWARE\\Microsoft\\NET Framework Setup\\NDP",3));return rows;});break;
            case "system":
                CollectSystemState();
                Emit("pcscope.system.update-history",UpdateHistory);
                Emit("pcscope.execution.resources",Resources);
                Emit("pcscope.identity.assigned-rights",()=>PCScopeIdentity.Rights().Select(x=>Item(x.Sid,D("sid",x.Sid,"rights",x.Rights,"basis","LSA account right assignment; not effective access on every resource"))).ToList());
                Emit("pcscope.identity.interactive-sessions",()=>PCScopeIdentity.Sessions().Select(x=>Item(x.Id.ToString(),D("session_id",x.Id,"station",x.Station,"state_code",x.State,"user",x.User,"domain",x.Domain,"identity_query_error",x.QueryError,"basis","WTS snapshot; logon/logoff times are supplied by event evidence"))).ToList());break;
            case "storage": CollectStorageState();Emit("pcscope.hardware.driver-packages",Drivers);break;
            case "network":CollectNetworkState();break;
            case "startup":CollectTaskState();break;
            case "security":
                CollectCertificateState();
                string[] providers=null;
                Emit("pcscope.security.key-metadata",()=>{providers=PCScopeMetadata.Providers();return new List<object>{Item("providers",D("providers",providers,"basis","registered CNG key storage providers; no private key material"))};});
                if(providers!=null)foreach(string provider in providers)foreach(string location in new[]{"user","machine"}) {
                    string p=provider,l=location;
                    Emit("pcscope.security.key-metadata",()=>PCScopeMetadata.Keys(p,l=="machine").Select(x=>Item(x.Name+"/"+x.LegacyKeySpec,D("provider",p,"location",l,"name",x.Name,"algorithm",x.Algorithm,"legacy_key_spec",x.LegacyKeySpec,"flags",x.Flags,"private_material_collected",false,"basis","NCryptEnumKeys with SILENT; key names and algorithm metadata only"))).ToList(),Hash("key-provider/"+p+"/"+l));
                }
                Emit("pcscope.collector.event-channel-coverage",()=>Channels().Select(channel=>{var row=LogCapability(channel);bool readable=(bool)row["readable"];return Item(channel,readable?D("channel",channel,"readable",true,"enabled",row["enabled"],"log_mode",row["mode"],"maximum_bytes",row["maximum_bytes"],"record_count",row["record_count"],"oldest_record",row["oldest_record"],"oldest_time",row["oldest_time"],"newest_record",row["newest_record"],"newest_time",row["newest_time"],"range_status","observed_retained_endpoints_only","basis","metadata plus actual read of retained endpoints; not proof of audit or historical completeness"):D("channel",channel,"readable",false,"reason",row["error"],"basis","capability check; not an empty history"));}).ToList());break;
            case "scoped":
                if(Enabled("pcscope.execution.modules")) {
                    try{var statuses=ModuleTargets();Emit("pcscope.execution.module-scan-status",()=>statuses,Hash(Json.Serialize(ModuleNames())));}
                    catch(Exception error){Gap("pcscope.execution.modules","target-discovery",error);if(Enabled("pcscope.execution.module-scan-status"))Gap("pcscope.execution.module-scan-status","target-discovery",error);}
                }
                var limits=Options();int maxFiles=Convert.ToInt32(Get(limits,"max_files_per_root"));long maxHash=Convert.ToInt64(Get(limits,"max_hash_bytes"));
                if(maxFiles<1||maxFiles>100000||maxHash<0||maxHash>268435456)throw new Exception("Invalid bounded scan configuration");
                if(Paths("files").Length>100||Paths("directories").Length>100)throw new Exception("Explicit watch list limit exceeded");
                CollectPortablePrograms(limits,maxFiles,maxHash);
                Emit("pcscope.collector.scoped-configuration",()=>{var options=Options();return new List<object>{Item("options",D("watch_files",Paths("files"),"watch_directories",Paths("directories"),"file_roots",new object[0],"portable_roots",Get(options,"portable_roots"),"module_process_names",ModuleNames(),"configured_module_process_names",Get(options,"module_process_names"),"hash_files",Get(options,"hash_files"),"max_files_per_root",Get(options,"max_files_per_root"),"max_hash_bytes",Get(options,"max_hash_bytes"),"docker_metadata",Get(options,"docker_metadata"),"mihomo_metadata",Get(options,"mihomo_metadata"),"content_versions","not implemented","basis","explicit file allowlist; optional exact module names in current user/session; empty module names disable the preset; legacy file_roots ignored"))};});
                foreach(string file in Paths("files")){string p=file;Emit("pcscope.files.metadata-security",()=>new List<object>{FileRow(p,false)},Hash("watch-file/"+p));}
                foreach(string directory in Paths("directories")){string p=directory;Emit("pcscope.files.directory-security",()=>new List<object>{FileRow(p,true)},Hash("watch-directory/"+p));}break;
            case "runtime-modules":
                if(Enabled("pcscope.execution.modules")) {
                    try{var statuses=ModuleTargets();Emit("pcscope.execution.module-scan-status",()=>statuses,Hash(Json.Serialize(ModuleNames())));}
                    catch(Exception error){Gap("pcscope.execution.modules","target-discovery",error);if(Enabled("pcscope.execution.module-scan-status"))Gap("pcscope.execution.module-scan-status","target-discovery",error);}
                }break;
            case "module-files":
                ModuleFiles();break;
            default: throw new Exception("Unsupported native collector unit");
        }
    }
    [StructLayout(LayoutKind.Sequential)] struct SystemTime {public ushort year,month,weekday,day,hour,minute,second,millisecond;}
    // DismAPI.h wraps all DISM payload structures in #pragma pack(push, 1), including x64.
    [StructLayout(LayoutKind.Sequential,Pack=1,CharSet=CharSet.Unicode)] struct Driver {
        public IntPtr published,original;public int inbox;public IntPtr catalog,klass,guid,description;public int bootCritical,signature;public IntPtr provider;public SystemTime date;public uint major,minor,build,revision;
    }
    [DllImport("dismapi.dll",CharSet=CharSet.Unicode)]static extern int DismInitialize(int level,string log,string scratch);
    [DllImport("dismapi.dll",CharSet=CharSet.Unicode)]static extern int DismOpenSession(string image,string windows,string drive,out uint session);
    [DllImport("dismapi.dll")]static extern int DismGetDrivers(uint session,[MarshalAs(UnmanagedType.Bool)]bool all,out IntPtr drivers,out uint count);
    [DllImport("dismapi.dll")]static extern int DismDelete(IntPtr buffer);
    [DllImport("dismapi.dll")]static extern int DismCloseSession(uint session);
    [DllImport("dismapi.dll")]static extern int DismShutdown();
    static void HResult(int value){if(value<0)Marshal.ThrowExceptionForHR(value);}
    static void DriverLayoutTest() {
        int size=Marshal.SizeOf(typeof(Driver));long provider=Marshal.OffsetOf(typeof(Driver),"provider").ToInt64(),date=Marshal.OffsetOf(typeof(Driver),"date").ToInt64();
        if(IntPtr.Size!=8||size!=100||provider!=60||date!=68)throw new Exception("DISM x64 SDK layout mismatch");
        IntPtr buffer=Marshal.AllocHGlobal(size*2),name=Marshal.StringToHGlobalUni("oem-fixture.inf");
        try {
            for(int index=0;index<2;index++)Marshal.StructureToPtr(new Driver{published=name,provider=name,major=(uint)(index+7),date=new SystemTime{year=2026,month=10,day=12}},IntPtr.Add(buffer,index*size),false);
            for(int index=0;index<2;index++){var row=(Driver)Marshal.PtrToStructure(IntPtr.Add(buffer,index*size),typeof(Driver));if(Marshal.PtrToStringUni(row.published)!="oem-fixture.inf"||row.major!=index+7||row.date.year!=2026)throw new Exception("DISM packed array stride failed");}
        }finally{Marshal.FreeHGlobal(name);Marshal.FreeHGlobal(buffer);}
        Console.WriteLine(Json.Serialize(D("driver_size",size,"provider_offset",provider,"date_offset",date,"packed_array",true)));
    }
    static List<object> Drivers() {
        if(!Elevated())throw new UnauthorizedAccessException("Driver store read requires an elevated Windows token");
        string log=Path.Combine(Path.GetTempPath(),"pcscope-native-driver-"+Guid.NewGuid().ToString("N")+".log");HResult(DismInitialize(0,log,null));uint session=0;bool opened=false;IntPtr buffer=IntPtr.Zero;
        try {
            HResult(DismOpenSession("DISM_{53BFAE52-B167-4E2F-A258-0A37B57FF845}",null,null,out session));opened=true;uint count;HResult(DismGetDrivers(session,true,out buffer,out count));var rows=new List<object>();int size=Marshal.SizeOf(typeof(Driver));
            for(uint i=0;i<count;i++) {
                var driver=(Driver)Marshal.PtrToStructure(IntPtr.Add(buffer,checked((int)i*size)),typeof(Driver));string name=Marshal.PtrToStringUni(driver.published);string date=null;
                if(driver.date.year!=0)date=new DateTime(driver.date.year,driver.date.month,driver.date.day,driver.date.hour,driver.date.minute,driver.date.second,driver.date.millisecond,DateTimeKind.Utc).ToString("o");
                var raw=D("PublishedName",name,"OriginalFileName",Marshal.PtrToStringUni(driver.original),"InBox",driver.inbox!=0,"CatalogFile",Marshal.PtrToStringUni(driver.catalog),"ClassName",Marshal.PtrToStringUni(driver.klass),"ClassGuid",Marshal.PtrToStringUni(driver.guid),"ClassDescription",Marshal.PtrToStringUni(driver.description),"BootCritical",driver.bootCritical!=0,"DriverSignature",driver.signature,"ProviderName",Marshal.PtrToStringUni(driver.provider),"Date",date,"MajorVersion",driver.major,"MinorVersion",driver.minor,"Build",driver.build,"Revision",driver.revision);
                // Inbox packages can share PublishedName across different DriverStore directories.
                string original=Text(raw["OriginalFileName"]);if(String.IsNullOrWhiteSpace(original))throw new Exception("Driver store package has no original INF identity");
                string package=name+"/"+Hash(original.Replace('/','\\').ToLowerInvariant());
                rows.Add(Item(package,D("native_package_id",package,"source_properties",raw,"Driver",name,"OriginalFileName",original,"ProviderName",raw["ProviderName"],"Version",driver.major+"."+driver.minor+"."+driver.build+"."+driver.revision,"Date",date,"ClassName",raw["ClassName"],"BootCritical",raw["BootCritical"],"Inbox",raw["InBox"],"DriverSignature",driver.signature==1?"Unsigned":driver.signature==2?"Signed":"Unknown","basis","native DISM driver store inventory; published INF plus original store INF path; not proof that the driver is loaded")));
            }
            return rows;
        }finally{if(buffer!=IntPtr.Zero)DismDelete(buffer);if(opened)DismCloseSession(session);DismShutdown();}
    }
}
