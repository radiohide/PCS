using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Management;
using Microsoft.Management.Infrastructure;
using Microsoft.Management.Infrastructure.Options;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Text.RegularExpressions;
using System.Runtime.InteropServices;
using System.Security.Principal;

partial class Platform {
    [DllImport("advapi32.dll",SetLastError=true)]static extern bool GetTokenInformation(IntPtr token,int kind,IntPtr buffer,int size,out int required);
    [DllImport("advapi32.dll")]static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);
    [DllImport("advapi32.dll")]static extern IntPtr GetSidSubAuthority(IntPtr sid,uint index);
    static void RequireStateIntegrity() {
        // A low-integrity executable can receive a silently filtered successful
        // enumeration. Such results must never declare missing objects deleted.
        using(var identity=WindowsIdentity.GetCurrent()) {
            int size;GetTokenInformation(identity.Token,25,IntPtr.Zero,0,out size);
            if(size<=0)throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            IntPtr buffer=Marshal.AllocHGlobal(size);
            try {
                if(!GetTokenInformation(identity.Token,25,buffer,size,out size))throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
                IntPtr sid=Marshal.ReadIntPtr(buffer);byte count=Marshal.ReadByte(GetSidSubAuthorityCount(sid));
                if(count==0||Marshal.ReadInt32(GetSidSubAuthority(sid,(uint)count-1))<8192)throw new Exception("Collector process integrity below ordinary application level; partial system visibility rejected; previous state retained");
            }finally{Marshal.FreeHGlobal(buffer);}
        }
    }
    sealed class StateRow {
        internal readonly CimInstance Instance;
        internal StateRow(CimInstance instance){Instance=instance;}
        internal object this[string name]{get{var property=Instance.CimInstanceProperties[name];return property==null?null:property.Value;}}
    }
    static void StateContractTest() {
        var previous=Console.Out;var captured=new System.IO.StringWriter();Console.SetOut(captured);
        int checks=0;
        Action<bool,string> check=(ok,name)=>{if(!ok)throw new Exception("State contract: "+name);checks++;};
        try {
            Request=D();Run="fixture";Machine="fixture-machine";
            check(EmitBase("fixture.base",()=>new List<object>{Item("one",D("state","ok"))}),"base success");
            var message=Json.DeserializeObject(captured.ToString().Trim());
            check(Text(Get(message,"scope"))=="machine:fixture-machine/user:"+UserSid()+"/schema:1/","base scope unchanged");
            captured.GetStringBuilder().Clear();
            check(!EmitBase("fixture.base",()=>new List<object>{Item("one",D()),Item("one",D())}),"duplicate rejection");
            check(Text(Get(Json.DeserializeObject(captured.ToString().Trim()),"type"))=="gap","duplicate never commits snapshot");
            captured.GetStringBuilder().Clear();
            check(!EmitBase("fixture.base",()=>new List<object>{Item("one",D("large",new string('x',1048576)))}),"size rejection");
            check(Text(Get(Json.DeserializeObject(captured.ToString().Trim()),"type"))=="gap","oversize never commits snapshot");
            captured.GetStringBuilder().Clear();
            check(!Emit("fixture.sharded",()=>{var rows=new List<object>{Item("one",D())};throw new Exception("enumeration failed after first row");}),"partial enumeration rejection");
            check(Text(Get(Json.DeserializeObject(captured.ToString().Trim()),"type"))=="gap","partial enumeration never commits shards");
            captured.GetStringBuilder().Clear();Request=D("selection",D("disabled_sources",new[]{"fixture.base","fixture.sharded"}));bool called=false;
            EmitBase("fixture.base",()=>{called=true;return new List<object>();});Emit("fixture.sharded",()=>{called=true;return new List<object>();});
            check(!called&&captured.ToString()=="","disabled source does no query");
            check(Text(WmiDate("20261014155837.123456+480"))=="2026-10-14T07:58:37.1234560Z","DMTF date and offset");
            check(Text(EvidenceText("C:\\Windows\\System32\\svchost.exe -k netsvcs"))=="C:\\Windows\\System32\\svchost.exe -k netsvcs","ordinary command retained");
            foreach(string value in new[]{"tool --password=secret-value","powershell -EncodedArguments hidden","https://user:secret@example.invalid/"})check(Get(EvidenceText(value),"sensitive") is bool&&(bool)Get(EvidenceText(value),"sensitive"),"sensitive command protected");
            check(!AppDomain.CurrentDomain.GetAssemblies().Any(assembly=>assembly.GetName().Name=="System.Management.Automation"),"PowerShell engine not loaded");
            using(var job=new CimInstance("MSFT_PrintJob")){
                job.CimInstanceProperties.Add(CimProperty.Create("ID",(uint)7,CimFlags.None));job.CimInstanceProperties.Add(CimProperty.Create("JobStatus",(uint)24,CimFlags.None));
                job.CimInstanceProperties.Add(CimProperty.Create("SubmittedTime",new DateTime(2026,10,14,10,0,0,DateTimeKind.Utc),CimFlags.None));job.CimInstanceProperties.Add(CimProperty.Create("DocumentName","fixture password=secret",CimFlags.None));
                var item=PrintJobItem("fixture-printer",new StateRow(job));var data=Get(item,"data");
                check(Text(Get(item,"id"))=="fixture-printer/7/2026-10-14T10:00:00.0000000Z"&&Text(Get(data,"status"))=="Spooling, Printing","print job identity and multiple state flags");
                check((bool)Get(Get(data,"document_name"),"sensitive")&&!(bool)Get(data,"document_content_collected"),"print document name protected and content excluded");
            }
            using(var fixture=new CimInstance("__EventFilter")){
                fixture.CimInstanceProperties.Add(CimProperty.Create("CreatorSID",new byte[]{1,2,3},CimFlags.None));
                var row=new StateRow(fixture);
                check(Get(WmiProperty(row,"CreatorSID"),"encoding")!=null&&Get(WmiPublic(row),"CreatorSID") is object[],"CIM public byte array preserves numeric evidence");
            }
            check(Text(StorageStatuses(new ushort[]{53264},false))=="Online"&&Text(StorageStatuses(new ushort[]{65535},false))=="Unknown","storage operational status and unknown code");
            check(Text(NullableNetworkValue(null,false))=="Any"&&Text(NullableNetworkValue("",false))=="Any","empty firewall text means Any");
            check(Text(NullableNetworkValue(new string[0],true))=="Any"&&Text(NullableNetworkValue(new[]{"80"},true))=="80"&&NullableNetworkValue(new[]{"80","443"},true) is object[],"firewall array scalar and multi-value representation");
            using(var cache=new CimInstance("MSFT_DNSClientCache")){
                cache.CimInstanceProperties.Add(CimProperty.Create("Entry","example.invalid",CimFlags.None));cache.CimInstanceProperties.Add(CimProperty.Create("Name","example.invalid",CimFlags.None));
                cache.CimInstanceProperties.Add(CimProperty.Create("Type",(ushort)16,CimFlags.None));cache.CimInstanceProperties.Add(CimProperty.Create("Data","fixture",CimFlags.None));cache.CimInstanceProperties.Add(CimProperty.Create("Status",(uint)0,CimFlags.None));cache.CimInstanceProperties.Add(CimProperty.Create("Section",(byte)1,CimFlags.None));
                var item=DnsCacheItem(new StateRow(cache));check(Text(Get(item,"id"))=="example.invalid/example.invalid/16/fixture"&&Text(Get(Get(item,"data"),"status"))=="0","DNS raw numeric identity including types absent from cmdlet enum");
            }
            var protectedPaths=(object[])EvidenceValue("paths",new[]{"C:\\safe","https://user:secret@example.invalid/"});
            check(Text(protectedPaths[0])=="C:\\safe"&&Get(protectedPaths[1],"sensitive") is bool&&(bool)Get(protectedPaths[1],"sensitive"),"multi-string sensitive element protected");
            check((bool)Get(Get(EvidenceValue("nested",D("api_key","fixture-secret")),"api_key"),"sensitive"),"nested sensitive value protected");
            using(var fixture=new CimInstance("MSFT_NetTCPConnection")) {
                fixture.CimInstanceProperties.Add(CimProperty.Create("AppliedSetting",(byte)6,CimFlags.None));
                var fields=TcpFields(new StateRow(fixture));
                check(fields["AppliedSetting"]==null&&Convert.ToInt32(Get(fields["source_properties"],"AppliedSetting"))==6,"unknown TCP setting retains raw provider code");
            }
        }finally{Console.SetOut(previous);captured.Dispose();}
        Console.WriteLine(Json.Serialize(D("state_contract","passed","checks",checks,"system_queries",0)));
    }
    // Preserve the original base collector's unsharded scope. Changing this scope
    // would create another baseline instead of comparing with existing objects.
    static bool EmitBase(string source,Func<List<object>> read,string suffix="") {
        if(!Enabled(source))return false;
        string observed=Now();
        try {
            var items=read();var ids=new HashSet<string>(StringComparer.Ordinal);
            foreach(var item in items)if(!ids.Add(Text(Get(item,"id"))))throw new Exception("Duplicate identity");
            string line=Json.Serialize(D("protocol_version",1,"request_id",Run+"/"+source+"/"+suffix,"type","snapshot","source",source,"scope","machine:"+Machine+"/user:"+UserSid()+"/schema:1/"+suffix,"run_id",Run,"observed_at",observed,"complete",true,"items",items.OrderBy(row=>Text(Get(row,"id")),StringComparer.Ordinal).ToArray()));
            if(Encoding.UTF8.GetByteCount(line)+1>1048576)throw new Exception("Atomic snapshot exceeds protocol limit");
            Console.WriteLine(line);return true;
        }catch(Exception error){Gap(source,suffix,error);return false;}
    }
    static List<object> ReadWmi(string ns,string name,Func<StateRow,object> adapt,bool includeHidden=false,IDictionary<string,object> custom=null) {
        RequireStateIntegrity();
        var result=new List<object>();
        // Call the system MI client directly. It preserves provider operation
        // options and locale semantics without a PowerShell host or runspace.
        using(var options=new CimOperationOptions {Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions {Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            if(includeHidden)options.SetCustomOption("IncludeHidden",true,Microsoft.Management.Infrastructure.CimType.Boolean,false);
            if(custom!=null)foreach(var option in custom)options.SetCustomOption(option.Key,option.Value,option.Value is bool?Microsoft.Management.Infrastructure.CimType.Boolean:Microsoft.Management.Infrastructure.CimType.String,false);
            foreach(CimInstance instance in session.QueryInstances(ns.Replace('\\','/'),"WQL","SELECT * FROM "+name,options))using(instance) {
                if(result.Count>=50000)throw new Exception("WMI enumeration exceeds source budget");
                result.Add(adapt(new StateRow(instance)));
            }
        }
        return result;
    }
    static object WmiProperty(StateRow row,string name) {
        foreach(CimProperty property in row.Instance.CimInstanceProperties)if(String.Equals(property.Name,name,StringComparison.OrdinalIgnoreCase))return WmiValue(property,0);
        return null; // Missing provider field is retained as unavailable, never as healthy.
    }
    static object WmiValue(CimProperty property,int depth,bool publicField=false) {
        object value=property.Value;if(value==null)return null;
        if(depth>8)throw new Exception("Nested WMI structure exceeds evidence depth");
        var nested=value as CimInstance;if(nested!=null)return publicField?EvidenceValue(property.Name,WmiPublic(nested,depth+1)):EvidenceValue(property.Name,Text(nested));
        if(value is byte[]&&!publicField)return Value(property.Name,value);
        var array=value as Array;if(array!=null)return EvidenceValue(property.Name,array.Cast<object>().Select(v=>v is CimInstance?(publicField?(object)WmiPublic((CimInstance)v,depth+1):Text(v)):v).ToArray());
        return EvidenceValue(property.Name,value);
    }
    static object WmiDate(string value) {
        if(value==null)return null;
        if(value.Length==25&&value[21]==':')return ManagementDateTimeConverter.ToTimeSpan(value).ToString();
        return ManagementDateTimeConverter.ToDateTime(value).ToUniversalTime().ToString("o");
    }
    static Dictionary<string,object> WmiPublic(StateRow row){return WmiPublic(row.Instance,0);}
    static Dictionary<string,object> WmiPublic(CimInstance row,int depth) {
        var result=new Dictionary<string,object>();
        foreach(CimProperty property in row.CimInstanceProperties)result[property.Name]=WmiValue(property,depth,true);
        return result;
    }
    static object EvidenceValue(string name,object value) {
        if(value==null)return null;
        if(!Regex.IsMatch(name,"(?i)(required|enabled|lastset|expires|age|length|history|policy|type|status)$")&&Regex.IsMatch(name,"(?i)password|passwd|secret|credential|private.?key|recovery.?password|^(token|access.?token|refresh.?token|api.?key|authorization|ScriptBlockText|ScriptText|Script|Binary)$"))return Protect(value);
        var map=value as IDictionary;if(map!=null){var result=new Dictionary<string,object>();foreach(DictionaryEntry entry in map)result[Text(entry.Key)]=EvidenceValue(Text(entry.Key),entry.Value);return result;}
        if(value is IEnumerable&&!(value is string)&&!(value is byte[]))return ((IEnumerable)value).Cast<object>().Select(item=>EvidenceValue(name,item)).ToArray();
        if(value is string) {
            if(!Regex.IsMatch(name,"(?i)(required|enabled|lastset|expires|age|length|history|policy|type|status)$")&&Regex.IsMatch(name,"(?i)password|passwd|secret|credential|private.?key|recovery.?password|^(token|access.?token|refresh.?token|api.?key|authorization|ScriptBlockText|ScriptText|Script|Binary)$"))return Protect(value);
            return EvidenceText((string)value);
        }
        return Value(name,value);
    }
    // Match the compatibility collector's protected-text decision. Store originals
    // under the existing DPAPI policy, never execute or publish collected commands.
    static object EvidenceText(string text) {
        if(text==null)return "";
        string redacted=Regex.Replace(text,"(?is)-----BEGIN [^-]*PRIVATE KEY-----.*?-----END [^-]*PRIVATE KEY-----","[REDACTED PRIVATE KEY]");
        redacted=Regex.Replace(redacted,@"(?im)(authorization\s*[:=]\s*)(?:Basic|Bearer|Digest|Negotiate)\s+[^\r\n]+","$1[REDACTED]");
        redacted=Regex.Replace(redacted,@"(?i)(\bBearer\s+)[A-Za-z0-9._~+/-]+=*","$1[REDACTED]");
        redacted=Regex.Replace(redacted,"(?i)((?:-|/)(?:enc|encodedcommand|encodedarguments)\\s+)(\"[^\"]*\"|'[^']*'|\\S+)","$1[REDACTED]");
        redacted=Regex.Replace(redacted,"(?i)(\\b(?:password|passwd|pwd|secret|token|access[_-]?token|refresh[_-]?token|api[_-]?key|authorization|client[_-]?secret)\\b[\"']?\\s*(?:[:=]|\\s)\\s*)(\"[^\"]*\"|'[^']*'|[^\\s,;]+)","$1[REDACTED]");
        redacted=Regex.Replace(redacted,@"(?i)(https?://)[^\s/:@]+:[^\s/@]+@","$1[REDACTED]@");
        return redacted==text?(object)text:Protect(text);
    }
    static List<object> WmiItems(string name,string identity,string[] fields,string ns="root\\cimv2") {
        return ReadWmi(ns,name,row=>{
            return Item(Text(WmiProperty(row,identity)),WmiFields(row,fields));
        });
    }
    static Dictionary<string,object> WmiFields(StateRow row,string[] fields) {
        var data=new Dictionary<string,object>();foreach(string field in fields)data[field]=WmiProperty(row,field);
        data["source_properties"]=WmiPublic(row);return data;
    }
    static string EnumLabel(object value,IDictionary<int,string> labels) {
        if(value==null)return "";int code=Convert.ToInt32(value);string label;
        return labels.TryGetValue(code,out label)?label:code.ToString(CultureInfo.InvariantCulture);
    }
    static List<object> Addresses() {
        var families=new Dictionary<int,string>{{2,"IPv4"},{23,"IPv6"}};
        var prefixes=new Dictionary<int,string>{{0,"Other"},{1,"Manual"},{2,"WellKnown"},{3,"Dhcp"},{4,"RouterAdvertisement"}};
        var suffixes=new Dictionary<int,string>{{0,"Other"},{1,"Manual"},{2,"WellKnown"},{3,"Dhcp"},{4,"Link"},{5,"Random"}};
        var states=new Dictionary<int,string>{{0,"Invalid"},{1,"Tentative"},{2,"Duplicate"},{3,"Deprecated"},{4,"Preferred"}};
        return ReadWmi("root\\StandardCimv2","MSFT_NetIPAddress",row=>{
            string family=EnumLabel(row["AddressFamily"],families);string address=Text(row["IPAddress"]);
            return Item(Text(row["InterfaceIndex"])+"/"+family+"/"+address,D("interface_index",row["InterfaceIndex"],"interface_alias",row["InterfaceAlias"],"address",address,"prefix_length",row["PrefixLength"],"address_family",family,"prefix_origin",EnumLabel(row["PrefixOrigin"],prefixes),"suffix_origin",EnumLabel(row["SuffixOrigin"],suffixes),"address_state",EnumLabel(row["AddressState"],states)));
        });
    }
    static void CollectNetworkState() {
        Emit("pcscope.network.routes",()=>ReadWmi("root\\StandardCimv2","MSFT_NetRoute",row=>Item(Text(row["InterfaceIndex"])+"/"+Text(row["DestinationPrefix"])+"/"+Text(row["NextHop"])+"/"+Text(row["PolicyStore"]),WmiFields(row,new[]{"InterfaceIndex","InterfaceAlias","DestinationPrefix","NextHop","RouteMetric","Protocol","AddressFamily","PolicyStore","Publish"}))));
        Emit("pcscope.network.tcp",()=>ReadWmi("root\\StandardCimv2","MSFT_NetTCPConnection",row=>Item(Text(row["LocalAddress"])+"/"+Text(row["LocalPort"])+"/"+Text(row["RemoteAddress"])+"/"+Text(row["RemotePort"])+"/"+Text(row["OwningProcess"])+"/"+Text(WmiProperty(row,"CreationTime")),TcpFields(row))));
        Emit("pcscope.network.udp",()=>ReadWmi("root\\StandardCimv2","MSFT_NetUDPEndpoint",row=>Item(Text(row["LocalAddress"])+"/"+Text(row["LocalPort"])+"/"+Text(row["OwningProcess"])+"/"+Text(WmiProperty(row,"CreationTime")),WmiFields(row,new[]{"LocalAddress","LocalPort","OwningProcess","CreationTime"}))));
    }
    static List<object> Tasks() {
        var states=new Dictionary<int,string>{{0,"Unknown"},{1,"Disabled"},{2,"Queued"},{3,"Ready"},{4,"Running"}};
        var levels=new Dictionary<int,string>{{0,"Limited"},{1,"Highest"}};
        return ReadWmi("root\\Microsoft\\Windows\\TaskScheduler","MSFT_ScheduledTask",row=>{
            var principal=row["Principal"] as CimInstance;var settings=row["Settings"] as CimInstance;
            if(principal==null||settings==null)throw new Exception("Task provider omitted principal or settings");
            var p=new StateRow(principal);var s=new StateRow(settings);
            return Item((Text(row["TaskPath"])+Text(row["TaskName"])).ToLowerInvariant(),D("name",row["TaskName"],"path",row["TaskPath"],"state",EnumLabel(row["State"],states),"enabled",s["Enabled"],"user_id",p["UserId"],"run_level",EnumLabel(p["RunLevel"],levels),"actions",TaskArray(row["Actions"])));
        });
    }
    static object[] TaskArray(object value) {
        // The previous public-CIM pipeline represented a null collection as one
        // empty structure. Keep that representation to avoid a migration delta.
        if(value==null)return new object[]{D()};
        var array=value as IEnumerable;if(array==null)throw new Exception("Unexpected task collection");var rows=new List<object>();
        foreach(object item in array){var instance=item as CimInstance;if(instance==null)throw new Exception("Unexpected task structure");rows.Add(WmiPublic(instance,0));}
        return rows.ToArray();
    }
    static List<object> TaskDetails() {
        RequireStateIntegrity();var rows=new List<object>();var clock=System.Diagnostics.Stopwatch.StartNew();
        const string ns="root/Microsoft/Windows/TaskScheduler";
        using(var options=new CimOperationOptions {Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions {Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            foreach(var instance in session.QueryInstances(ns,"WQL","SELECT * FROM MSFT_ScheduledTask",options))using(instance) {
                if(rows.Count>=10000||clock.Elapsed>TimeSpan.FromSeconds(35))throw new Exception("Task detail enumeration exceeds source budget; incomplete result discarded");
                var parameters=new CimMethodParametersCollection();parameters.Add(CimMethodParameter.Create("InputObject",instance,Microsoft.Management.Infrastructure.CimType.Instance,CimFlags.In));
                // This fixed provider method only reads scheduler metadata. It
                // cannot run, register, enable, disable, or modify a task.
                using(var result=session.InvokeMethod(ns,"PS_ScheduledTask","GetInfoByObject",parameters,options)) {
                    uint status=Convert.ToUInt32(result.ReturnValue.Value);if(status!=0)throw new Exception("Task information read failed: "+status);
                    var info=result.OutParameters["CmdletOutput"].Value as CimInstance;if(info==null)throw new Exception("Task dynamic information missing");
                    using(info) {
                        var task=new StateRow(instance);var dynamicInfo=new StateRow(info);
                        var settings=task["Settings"] as CimInstance;var principal=task["Principal"] as CimInstance;
                        if(settings==null||principal==null)throw new Exception("Task detail provider omitted structures");
                        rows.Add(Item(Text(task["TaskPath"])+Text(task["TaskName"]),D("path",task["TaskPath"],"name",task["TaskName"],"last_run",WmiProperty(dynamicInfo,"LastRunTime"),"last_result",dynamicInfo["LastTaskResult"],"next_run",WmiProperty(dynamicInfo,"NextRunTime"),"missed_runs",dynamicInfo["NumberOfMissedRuns"],"triggers",TaskArray(task["Triggers"]),"conditions",WmiPublic(settings,0),"principal",WmiPublic(principal,0),"registration",WmiFields(task,new[]{"Author","Description","URI","Version","Source"}),"actions",TaskArray(task["Actions"]))));
                    }
                }
            }
        }
        return rows;
    }
    static void CollectTaskState(){Emit("pcscope.startup.task-details",TaskDetails);}
    static Dictionary<string,object> TcpFields(StateRow row) {
        var data=WmiFields(row,new[]{"LocalAddress","LocalPort","RemoteAddress","RemotePort","State","OwningProcess","CreationTime","AppliedSetting"});
        // The shipped cmdlet has five named settings. Its getter returns null for
        // newer provider codes; preserve that interpreted field and keep raw data.
        if(row["AppliedSetting"]!=null&&Convert.ToUInt32(row["AppliedSetting"])>4)data["AppliedSetting"]=null;
        return data;
    }
    static string AdapterStatus(StateRow row) {
        int state=Convert.ToInt32(row["InterfaceOperationalStatus"]);
        if(state==2) {
            if(Convert.ToInt32(row["InterfaceAdminStatus"])==2)return "Disabled";
            var parts=new List<string>();
            if(Convert.ToBoolean(row["OperationalStatusDownDefaultPortNotAuthenticated"]))parts.Add("Not Authenticated");
            if(Convert.ToBoolean(row["OperationalStatusDownMediaDisconnected"]))parts.Add("Disconnected");
            if(Convert.ToBoolean(row["OperationalStatusDownLowPowerState"]))parts.Add("Low Power State");
            return String.Join(", ",parts);
        }
        switch(state){case 1:return "Up";case 3:return "Testing";case 4:return "Unknown";case 5:return "Dormant";case 6:return "Not Present";case 7:return "Lower Layer Down";default:return "";}
    }
    static string AdapterSpeed(object value) {
        decimal speed=Convert.ToDecimal(value,CultureInfo.InvariantCulture);string[] units={"bps","Kbps","Mbps","Gbps","Tbps"};int unit=0;
        while(speed>=1000&&unit<units.Length-1){speed/=1000;unit++;}
        return Decimal.Round(speed,1).ToString(CultureInfo.InvariantCulture)+" "+units[unit];
    }
    static List<object> Adapters() {
        return ReadWmi("root\\StandardCimv2","MSFT_NetAdapter",row=> {
            var addresses=row["NetworkAddresses"] as string[];string mac=addresses!=null&&addresses.Length>0?addresses[0]:"";
            mac=String.Join("-",Enumerable.Range(0,(mac.Length+1)/2).Select(i=>mac.Substring(i*2,Math.Min(2,mac.Length-i*2))));
            return Item(Text(row["InterfaceGuid"]),D("name",row["Name"],"description",row["InterfaceDescription"],"status",AdapterStatus(row),"mac_address",mac,"link_speed",AdapterSpeed(row["TransmitLinkSpeed"]),"driver_version",row["DriverVersionString"]));
        },true);
    }
    static void CollectBaseState() {
        CollectShellRegistrations();
        EmitBase("pcscope.startup.logon",LogonStartup);
        EmitBase("pcscope.system.version",()=>ReadWmi("root\\cimv2","Win32_OperatingSystem",row=>Item("operating-system",D("caption",row["Caption"],"version",row["Version"],"build",row["BuildNumber"],"architecture",row["OSArchitecture"],"install_time",WmiProperty(row,"InstallDate"),"install_time_basis","Win32_OperatingSystem.InstallDate","computer_name",row["CSName"]))));
        EmitBase("pcscope.system.boot-state",()=>ReadWmi("root\\cimv2","Win32_OperatingSystem",row=>Item("boot-state",D("last_boot_time",WmiProperty(row,"LastBootUpTime"),"basis","snapshot","timezone",TimeZoneInfo.Local.Id))));
        EmitBase("pcscope.startup.services",()=>ReadWmi("root\\cimv2","Win32_Service",row=>Item(Text(row["Name"]).ToLowerInvariant(),D("name",row["Name"],"display_name",row["DisplayName"],"state",row["State"],"start_mode",row["StartMode"],"start_account",row["StartName"],"service_type",row["ServiceType"],"description",row["Description"],"command",EvidenceText(Text(row["PathName"])),"pid",row["ProcessId"],"exit_code",row["ExitCode"],"service_specific_exit_code",row["ServiceSpecificExitCode"]))));
        EmitBase("pcscope.startup.tasks",Tasks);
        EmitBase("pcscope.storage.volumes",()=>ReadWmi("root\\cimv2","Win32_Volume",row=>Item(Text(row["DeviceID"]),D("device_id",row["DeviceID"],"drive_letter",row["DriveLetter"],"label",row["Label"],"file_system",row["FileSystem"],"capacity",row["Capacity"],"drive_type",row["DriveType"]))));
        EmitBase("pcscope.network.adapters",Adapters);
        EmitBase("pcscope.network.addresses",Addresses);
        EmitBase("pcscope.hardware.devices",()=>ReadWmi("root\\cimv2","Win32_PnPEntity",row=>Item(Text(row["DeviceID"]),D("name",row["Name"],"class",row["PNPClass"],"class_guid",row["ClassGuid"],"manufacturer",row["Manufacturer"],"service",row["Service"],"status",row["Status"],"error_code",row["ConfigManagerErrorCode"]))));
        EmitBase("pcscope.system.hotfixes",()=>ReadWmi("root\\cimv2","Win32_QuickFixEngineering",row=>Item(Text(row["HotFixID"])+"/"+Text(row["ServicePackInEffect"]),D("hotfix_id",row["HotFixID"],"description",row["Description"],"installed_on",Text(row["InstalledOn"]),"coverage","QFE only; not the complete Windows Update inventory"))));
        EmitBase("pcscope.virtualization.platform",()=>{
            var rows=ReadWmi("root\\cimv2","Win32_ComputerSystem",row=>Item("hypervisor",D("hypervisor_present",row["HypervisorPresent"],"manufacturer",row["Manufacturer"],"model",row["Model"])));
            rows.AddRange(ReadWmi("root\\cimv2","Win32_Processor",row=>Item(Text(row["DeviceID"]),D("name",row["Name"],"firmware_virtualization",row["VirtualizationFirmwareEnabled"],"vm_monitor_extensions",row["VMMonitorModeExtensions"],"slat",row["SecondLevelAddressTranslationExtensions"]))));return rows;
        });
    }
    static void CollectSystemState() {
        Emit("pcscope.execution.processes",()=>ReadWmi("root\\cimv2","Win32_Process",row=> {
            object start=WmiProperty(row,"CreationDate");string command=Text(row["CommandLine"]);
            return Item(Text(row["ProcessId"])+"/"+Text(start),D("pid",row["ProcessId"],"parent_pid",row["ParentProcessId"],"name",row["Name"],"path",row["ExecutablePath"],"start_time",start,"session_id",row["SessionId"],"command_line",EvidenceText(command),"command_line_collected",command!=null,"command_line_status",command!=null?"available_redaction_applied":"not_returned_system_process_access_or_exit","basis","snapshot; short-lived processes may be missed"));
        }));
        Emit("pcscope.identity.profiles",()=>WmiItems("Win32_UserProfile","SID",new[]{"SID","LocalPath","Loaded","Special","Status","LastUseTime"}));
        Emit("pcscope.identity.logon-sessions",()=>WmiItems("Win32_LogonSession","LogonId",new[]{"LogonId","LogonType","StartTime","AuthenticationPackage","Caption"}));
        Emit("pcscope.identity.machine",()=>{var rows=WmiItems("Win32_ComputerSystem","Name",new[]{"Name","Domain","PartOfDomain","DomainRole","Manufacturer","Model","SystemType"});rows.AddRange(WmiItems("Win32_ComputerSystemProduct","UUID",new[]{"UUID","Vendor","Name","Version"}));return rows;});
        Emit("pcscope.system.restore-points",()=>WmiItems("SystemRestore","SequenceNumber",new[]{"SequenceNumber","Description","CreationTime","RestorePointType","EventType"},"root\\default"));
    }
    static void CollectStorageState() {
        Emit("pcscope.power.configuration",()=>{var rows=WmiItems("Win32_PowerPlan","InstanceID",new[]{"InstanceID","ElementName","IsActive","Description"},"root\\cimv2\\power");rows.AddRange(Configuration("LocalMachine","SYSTEM\\CurrentControlSet\\Control\\Power"));return rows;});
        Emit("pcscope.hardware.driver-services",()=>WmiItems("Win32_SystemDriver","Name",new[]{"Name","DisplayName","PathName","ServiceType","StartMode","State","Status","Started"}));
        Emit("pcscope.hardware.driver-bindings",()=>WmiItems("Win32_PnPSignedDriver","DeviceID",new[]{"DeviceID","DeviceName","DriverVersion","DriverDate","DriverProviderName","InfName","IsSigned","Signer","DriverName"}));
        Emit("pcscope.hardware.firmware",()=>WmiItems("Win32_BIOS","Name",new[]{"Manufacturer","Name","SMBIOSBIOSVersion","SMBIOSMajorVersion","SMBIOSMinorVersion","ReleaseDate","Version"}));
        Emit("pcscope.hardware.battery",()=>WmiItems("Win32_Battery","DeviceID",new[]{"DeviceID","Name","Status","BatteryStatus","Chemistry","DesignCapacity","FullChargeCapacity","EstimatedChargeRemaining","EstimatedRunTime"}));
    }
    static List<object> Certificates(StoreLocation location) {
        RequireStateIntegrity();
        var rows=new List<object>();
        foreach(string name in new[]{"Root","CA","My","TrustedPublisher","Disallowed","TrustedPeople"})using(var store=new X509Store(name,location)) {
            store.Open(OpenFlags.ReadOnly|OpenFlags.OpenExistingOnly);
            foreach(var cert in store.Certificates)using(cert)rows.Add(Item(name+"/"+cert.Thumbprint,D("store",name,"location",location.ToString(),"thumbprint",cert.Thumbprint,"subject",cert.Subject,"issuer",cert.Issuer,"not_before",cert.NotBefore.ToUniversalTime().ToString("o"),"not_after",cert.NotAfter.ToUniversalTime().ToString("o"),"has_private_key",cert.HasPrivateKey,"signature_algorithm",cert.SignatureAlgorithm.Value,"trust","not evaluated")));
        }
        return rows;
    }
    static void CollectCertificateState() {
        Emit("pcscope.security.certificates.currentuser",()=>Certificates(StoreLocation.CurrentUser));
        Emit("pcscope.security.certificates.localmachine",()=>Certificates(StoreLocation.LocalMachine));
    }
}
