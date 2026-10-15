using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Resources;
using System.Management;
using System.Text;
using System.Text.RegularExpressions;
using Microsoft.Management.Infrastructure;
using Microsoft.Management.Infrastructure.Options;

partial class Platform {
    const string HypervNamespace="root/virtualization/v2";
    static CimInstance HypervReference(string value) {
        if(String.IsNullOrWhiteSpace(value)||value.Length>8192)throw new InvalidOperationException("Invalid Hyper-V reference size");
        var path=new ManagementPath(value);if(!path.IsInstance)throw new InvalidOperationException("Hyper-V reference is not an instance");
        string server=path.Server;if(!String.IsNullOrEmpty(server)&&server!="."&&!String.Equals(server,"localhost",StringComparison.OrdinalIgnoreCase)&&!String.Equals(server,Environment.MachineName,StringComparison.OrdinalIgnoreCase))throw new InvalidOperationException("Remote Hyper-V references are outside collection scope");
        string ns=path.NamespacePath.Replace('/','\\');if(ns.StartsWith("\\\\",StringComparison.Ordinal)){int slash=ns.IndexOf('\\',2);if(slash<0)throw new InvalidOperationException("Invalid Hyper-V namespace path");ns=ns.Substring(slash+1);}if(!String.IsNullOrEmpty(ns)&&!String.Equals(ns,"root\\virtualization\\v2",StringComparison.OrdinalIgnoreCase))throw new InvalidOperationException("Hyper-V reference has an unexpected namespace");
        string[] keys;
        if(String.Equals(path.ClassName,"Msvm_ComputerSystem",StringComparison.OrdinalIgnoreCase))keys=new[]{"CreationClassName","Name"};
        else if(String.Equals(path.ClassName,"Msvm_ExternalEthernetPort",StringComparison.OrdinalIgnoreCase))keys=new[]{"SystemCreationClassName","SystemName","CreationClassName","DeviceID"};
        else if(String.Equals(path.ClassName,"Msvm_DiskDrive",StringComparison.OrdinalIgnoreCase))keys=new[]{"SystemCreationClassName","SystemName","CreationClassName","DeviceID"};
        else if(String.Equals(path.ClassName,"Msvm_ResourceAllocationSettingData",StringComparison.OrdinalIgnoreCase))keys=new[]{"InstanceID"};
        else throw new InvalidOperationException("Unsupported Hyper-V host resource class");
        string relative=path.RelativePath;int dot=relative.IndexOf('.');if(dot<0)throw new InvalidOperationException("Hyper-V reference has no keys");string text=relative.Substring(dot+1);if(text.TrimEnd().EndsWith(",",StringComparison.Ordinal))throw new InvalidOperationException("Hyper-V reference has an incomplete key list");int offset=0;
        var fields=new Dictionary<string,string>(StringComparer.OrdinalIgnoreCase);var pattern=new Regex(@"\G\s*(?<key>[A-Za-z][A-Za-z0-9_]*)\s*=\s*""(?<value>(?:\\.|[^""\\])*)""\s*(?:,|$)",RegexOptions.CultureInvariant,TimeSpan.FromMilliseconds(200));
        while(offset<text.Length) {
            var match=pattern.Match(text,offset);if(!match.Success||match.Index!=offset)throw new InvalidOperationException("Unsupported Hyper-V reference key encoding");offset+=match.Length;
            string encoded=match.Groups["value"].Value;var decoded=new StringBuilder();for(int i=0;i<encoded.Length;i++){char c=encoded[i];if(c=='\\'){if(++i>=encoded.Length||(encoded[i]!='\\'&&encoded[i]!='"'))throw new InvalidOperationException("Unsupported Hyper-V reference escape");c=encoded[i];}decoded.Append(c);}
            fields.Add(match.Groups["key"].Value,decoded.ToString());if(fields.Count>4)throw new InvalidOperationException("Hyper-V reference key budget exceeded");
        }
        if(fields.Count!=keys.Length)throw new InvalidOperationException("Hyper-V reference key set mismatch");var instance=new CimInstance(path.ClassName,HypervNamespace);
        try {foreach(string key in keys){string content;if(!fields.TryGetValue(key,out content))throw new InvalidOperationException("Hyper-V reference omitted a required key");instance.CimInstanceProperties.Add(CimProperty.Create(key,content,CimFlags.Key));}return instance;}catch{instance.Dispose();throw;}
    }
    static object HypervSwitchItem(string id,string name,bool external,bool internalConnection,string description) {
        Guid guid;if(!Guid.TryParse(id,out guid)||String.IsNullOrWhiteSpace(name))throw new InvalidOperationException("Hyper-V switch identity unavailable");
        if(external&&description==null)throw new InvalidOperationException("External Hyper-V switch adapter information unavailable");
        return Item(guid.ToString(),D("Id",guid.ToString(),"Name",EvidenceText(name),"SwitchType",external?2:internalConnection?1:0,"NetAdapterInterfaceDescription",description==null?null:EvidenceText(description),"AllowManagementOS",internalConnection));
    }
    static string HypervSwitchDescription(bool teaming,string physical,CultureInfo culture) {
        if(!teaming)return physical;
        // Resource-only metadata lookup; never load the management module for execution.
        for(var language=culture;language.Name.Length>0;language=language.Parent) {
            string root=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows),@"System32\WindowsPowerShell\v1.0\Modules\Hyper-V");
            string path=Path.Combine(root,language.Name,"Microsoft.HyperV.PowerShell.Objects.resources.dll");
            string localized=HypervTeamResource(path);if(localized!=null)return localized;
            string gac=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows),@"Microsoft.NET\assembly\GAC_MSIL\Microsoft.HyperV.PowerShell.Objects.resources");
            if(Directory.Exists(gac)) {
                string[] versions=Directory.GetDirectories(gac);if(versions.Length>64)throw new InvalidOperationException("Hyper-V resource lookup exceeds version budget");
                Array.Sort(versions,StringComparer.OrdinalIgnoreCase);Array.Reverse(versions);
                foreach(string version in versions) {
                    if(Path.GetFileName(version).IndexOf("_"+language.Name+"_",StringComparison.OrdinalIgnoreCase)<0)continue;
                    localized=HypervTeamResource(Path.Combine(version,"Microsoft.HyperV.PowerShell.Objects.resources.dll"));if(localized!=null)return localized;
                }
            }
        }
        // Verified neutral/zh-Hans fallback when optional management resources are absent.
        string name=culture.Name;if(name.StartsWith("zh",StringComparison.OrdinalIgnoreCase)&&name!="zh-TW"&&name!="zh-HK"&&name!="zh-MO"&&name!="zh-Hant")return "组合接口";
        return "Teamed-Interface";
    }
    static string HypervTeamResource(string path) {
        if(!File.Exists(path))return null;
        // A ResourceReader value accessor can deserialize arbitrary objects. Fetch only raw
        // ResourceTypeCode.String bytes and decode them without accessing enumerator values.
        try {
            if(new FileInfo(path).Length>4194304)return null;
            var assembly=Assembly.ReflectionOnlyLoadFrom(path);
            foreach(string name in assembly.GetManifestResourceNames()) {
                if(!name.StartsWith("Microsoft.HyperV.PowerShell.Objects.Common.ObjectDescriptions",StringComparison.Ordinal)||!name.EndsWith(".resources",StringComparison.Ordinal))continue;
                using(var stream=assembly.GetManifestResourceStream(name)) {
                    if(stream==null||stream.Length>1048576)continue;
                    using(var reader=new ResourceReader(stream)) {
                        string kind;byte[] bytes;reader.GetResourceData("VMSwitch_SETInterfaceDescription",out kind,out bytes);
                        return HypervTeamResourceString(kind,bytes);
                    }
                }
            }
        }catch(IOException){}catch(UnauthorizedAccessException){}catch(ArgumentException){}catch(BadImageFormatException){}catch(System.Security.SecurityException){}
        return null;
    }
    static string HypervTeamResourceString(string kind,byte[] bytes) {
        if(kind!="ResourceTypeCode.String"||bytes==null||bytes.Length>1024)return null;
        using(var stream=new MemoryStream(bytes))using(var reader=new BinaryReader(stream,Encoding.UTF8)) {
            string value=reader.ReadString();if(stream.Position!=stream.Length||String.IsNullOrWhiteSpace(value)||value.Length>256)return null;
            return value;
        }
    }
    static List<object> HypervSwitches() {
        RequireStateIntegrity();var rows=new List<object>();
        using(var options=new CimOperationOptions{Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions{Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            foreach(var sw in session.QueryInstances(HypervNamespace,"WQL","SELECT * FROM Msvm_VirtualEthernetSwitch",options))using(sw) {
                if(rows.Count>=4096)throw new InvalidOperationException("Hyper-V switch budget exceeded");var row=new StateRow(sw);bool external=false,internalConnection=false;string description=null;bool teaming=false;int externalCount=0,settingsCount=0,connections=0;
                foreach(var settings in session.EnumerateAssociatedInstances(HypervNamespace,sw,null,"Msvm_VirtualEthernetSwitchSettingData",null,null,options))using(settings) {
                    if(++settingsCount>1)throw new InvalidOperationException("Hyper-V switch settings are ambiguous");
                    object teamValue=VmRequired(new StateRow(settings),"TeamingEnabled");if(!(teamValue is bool))throw new InvalidOperationException("Invalid Hyper-V teaming metadata");teaming=(bool)teamValue;
                    foreach(var allocation in session.EnumerateAssociatedInstances(HypervNamespace,settings,"Msvm_VirtualSystemSettingDataComponent","Msvm_EthernetPortAllocationSettingData",null,null,options))using(allocation) {
                        if(++connections>50000)throw new InvalidOperationException("Hyper-V switch connection budget exceeded");object value=new StateRow(allocation)["HostResource"];if(value==null)continue;var resources=value as string[];if(resources!=null&&resources.Length==0)continue;if(resources==null||resources.Length>64||(!teaming&&resources.Length!=1))throw new InvalidOperationException("Invalid Hyper-V host resource layout");
                        foreach(string resourcePath in resources){
                        var path=new ManagementPath(resourcePath);if(!String.Equals(path.ClassName,"Msvm_ComputerSystem",StringComparison.OrdinalIgnoreCase)&&!String.Equals(path.ClassName,"Msvm_ExternalEthernetPort",StringComparison.OrdinalIgnoreCase)){if(String.Equals(path.ClassName,"Msvm_SyntheticEthernetPort",StringComparison.OrdinalIgnoreCase)||String.Equals(path.ClassName,"Msvm_EmulatedEthernetPort",StringComparison.OrdinalIgnoreCase))continue;throw new InvalidOperationException("Hyper-V connection class requires separate validation");}
                        using(var reference=HypervReference(resourcePath)) {
                            if(String.Equals(path.ClassName,"Msvm_ComputerSystem",StringComparison.OrdinalIgnoreCase)){if(String.Equals(Text(reference.CimInstanceProperties["Name"].Value),Environment.MachineName,StringComparison.OrdinalIgnoreCase))internalConnection=true;}
                            else {if(++externalCount>64||(!teaming&&externalCount>1))throw new InvalidOperationException("Invalid Hyper-V external binding count");using(var port=session.GetInstance(HypervNamespace,reference,options)){description=Text(VmRequired(new StateRow(port),"Name"));external=true;}}
                        }
                        }
                    }
                }
                if(settingsCount!=1)throw new InvalidOperationException("Hyper-V switch settings missing");description=HypervSwitchDescription(teaming,description,CultureInfo.CurrentUICulture);rows.Add(HypervSwitchItem(Text(row["Name"]),Text(row["ElementName"]),external,internalConnection,description));
            }
        }
        return rows;
    }
    static void CollectHypervSwitches(string unit){if(unit=="virtualization")Emit("pcscope.virtualization.hyperv-switches",HypervSwitches);}
    static void HypervSwitchContractTest() {
        string id="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        if(HypervSwitchDescription(true,"physical",CultureInfo.GetCultureInfo("en-US"))!="Teamed-Interface"||HypervSwitchDescription(true,"physical",CultureInfo.GetCultureInfo("zh-CN"))!="组合接口"||HypervSwitchDescription(false,"physical",CultureInfo.InvariantCulture)!="physical")throw new Exception("Teaming display mapping mismatch");
        using(var stream=new MemoryStream()) {
            using(var writer=new BinaryWriter(stream,Encoding.UTF8,true))writer.Write("组队资源样本");
            if(HypervTeamResourceString("ResourceTypeCode.String",stream.ToArray())!="组队资源样本"||HypervTeamResourceString("Custom.SerializableObject",stream.ToArray())!=null)throw new Exception("Resource decoding executed an unsupported type");
        }
        object internalData=Get(HypervSwitchItem(id,"内网",false,true,null),"data");if(Convert.ToInt32(Get(internalData,"SwitchType"))!=1||!(bool)Get(internalData,"AllowManagementOS"))throw new Exception("Internal switch management connection mapping mismatch");
        object privateData=Get(HypervSwitchItem(id,"私网",false,false,null),"data");if(Convert.ToInt32(Get(privateData,"SwitchType"))!=0||(bool)Get(privateData,"AllowManagementOS"))throw new Exception("Private switch management connection was invented");
        object externalData=Get(HypervSwitchItem(id,"外网",true,true,"Fixture adapter"),"data");if(Convert.ToInt32(Get(externalData,"SwitchType"))!=2||!(bool)Get(externalData,"AllowManagementOS"))throw new Exception("External switch mapping mismatch");
        using(var reference=HypervReference("Msvm_ComputerSystem.CreationClassName=\"Msvm_ComputerSystem\",Name=\""+Environment.MachineName+"\"")){if(Text(reference.CimInstanceProperties["Name"].Value)!=Environment.MachineName)throw new Exception("Host resource key mapping mismatch");}
        bool rejected=false;try{using(var reference=HypervReference("\\\\example.invalid\\root\\virtualization\\v2:Msvm_ComputerSystem.CreationClassName=\"Msvm_ComputerSystem\",Name=\"other\"")){} }catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Remote Hyper-V resource accepted");
        rejected=false;try{HypervSwitchItem(id,"外网",true,false,null);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Missing physical adapter information accepted");
        Console.WriteLine(Json.Serialize(D("switch_fields",true,"local_reference",true,"remote_reference_rejected",true,"missing_adapter_rejected",true,"system_changes",false)));
    }
}
