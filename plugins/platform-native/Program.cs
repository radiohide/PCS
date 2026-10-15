using System;
using System.IO;
using System.Text;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics.Eventing.Reader;
using System.Security.Principal;
using System.Runtime.InteropServices;
using System.ComponentModel;
using System.Web.Script.Serialization;

// Fixed native operations only. Collection never changes audit policy, logs, keys or journal.
// --repair is an explicit capability-configuration operation, separate from collection.
partial class Platform {
    static readonly JavaScriptSerializer Json=new JavaScriptSerializer {MaxJsonLength=67108864,RecursionLimit=64};
    internal static Dictionary<string,object> D(params object[] pairs) {
        var result=new Dictionary<string,object>();
        for(int i=0;i<pairs.Length;i+=2) result.Add((string)pairs[i],pairs[i+1]);
        return result;
    }
    internal static object Get(object value,string key) {
        var map=value as Dictionary<string,object>;object answer;
        return map!=null && map.TryGetValue(key,out answer)?answer:null;
    }
    internal static string Text(object value) {return value==null?null:Convert.ToString(value,System.Globalization.CultureInfo.InvariantCulture);}
    internal static string Now() {return DateTime.UtcNow.ToString("o");}
    internal static string UserSid() {using(var identity=WindowsIdentity.GetCurrent())return identity.User.Value;}
    internal static bool Elevated() {using(var identity=WindowsIdentity.GetCurrent())return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);}
    [StructLayout(LayoutKind.Sequential)] struct AuditPolicy {public Guid sub;public uint flags;public Guid category;}
    [DllImport("advapi32.dll",SetLastError=true)] [return:MarshalAs(UnmanagedType.U1)] static extern bool AuditQuerySystemPolicy([In] Guid[] ids,uint count,out IntPtr buffer);
    [DllImport("advapi32.dll",SetLastError=true)] [return:MarshalAs(UnmanagedType.U1)] static extern bool AuditSetSystemPolicy([In] AuditPolicy[] policies,uint count);
    [DllImport("advapi32.dll")] static extern void AuditFree(IntPtr buffer);
    static AuditPolicy Audit(string id) {
        IntPtr buffer;if(!AuditQuerySystemPolicy(new[]{new Guid(id)},1,out buffer))throw new Win32Exception(Marshal.GetLastWin32Error());
        try{return (AuditPolicy)Marshal.PtrToStructure(buffer,typeof(AuditPolicy));}finally{AuditFree(buffer);}
    }
    internal static Dictionary<string,object> LogCapability(string channel) {
        var row=D("name",channel,"source","EventLogConfiguration / EventLogReader (native)","enabled",null,"readable",false,"mode",null,"maximum_bytes",null,"record_count",null,"oldest_record",null,"newest_record",null,"oldest_time",null,"newest_time",null,"error",null);
        try {
            using(var config=new EventLogConfiguration(channel)) {row["enabled"]=config.IsEnabled;row["mode"]=config.LogMode.ToString();row["maximum_bytes"]=config.MaximumSizeInBytes;}
            var info=EventLogSession.GlobalSession.GetLogInformation(channel,PathType.LogName);
            row["record_count"]=info.RecordCount;row["oldest_record"]=info.OldestRecordNumber;
            foreach(bool reverse in new[]{false,true}) {
                var query=new EventLogQuery(channel,PathType.LogName,"*") {ReverseDirection=reverse};
                using(var reader=new EventLogReader(query))using(var entry=reader.ReadEvent()) {
                    if(entry!=null) {row[reverse?"newest_record":"oldest_record"]=entry.RecordId;if(entry.TimeCreated.HasValue)row[reverse?"newest_time":"oldest_time"]=entry.TimeCreated.Value.ToUniversalTime().ToString("o");}
                }
            }
            row["readable"]=true;
        }catch(Exception error) {row["error"]=error.Message;row["error_code"]=error.HResult;}
        return row;
    }
    static string[] Channels() {return Json.Deserialize<string[]>(File.ReadAllText(Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"channels.json")));}
    internal static object Capabilities(bool repair) {
        bool elevated=Elevated();var changes=new List<object>();var audits=new List<object>();var logs=new List<object>();
        string[][] policies={
            new[]{"计划任务修改审计","0cce9227-69ae-11d9-bed3-505054503030","4698–4702：任务创建、删除、启停和修改账户"},
            new[]{"进程创建审计","0cce922b-69ae-11d9-bed3-505054503030","4688：进程路径、进程 ID、父进程"},
            new[]{"登录审计","0cce9215-69ae-11d9-bed3-505054503030","4624：关联登录会话"}
        };
        foreach(var policy in policies) {
            var row=D("name",policy[0],"guid",policy[1],"purpose",policy[2],"source","AuditQuerySystemPolicy (native)","success_enabled",null,"error",null);
            try {
                var flags=Audit(policy[1]);row["success_enabled"]=(flags.flags&1)!=0;
                if(repair && elevated && (flags.flags&1)==0) {
                    uint before=flags.flags;flags.flags=(flags.flags&~4u)|1u; // preserve failure auditing; clear POLICY_AUDIT_EVENT_NONE.
                    if(!AuditSetSystemPolicy(new[]{flags},1))throw new Win32Exception(Marshal.GetLastWin32Error());
                    uint after=Audit(policy[1]).flags;row["success_enabled"]=(after&1)!=0;
                    changes.Add(D("name",policy[0],"before",before,"after",after));
                }
            }catch(Exception error){row["error"]=error.Message;row["error_code"]=error.HResult;}
            audits.Add(row);
        }
        foreach(string channel in Channels()) {
            string changeError=null;
            if(repair && elevated && channel=="Microsoft-Windows-TaskScheduler/Operational") {
                try {using(var config=new EventLogConfiguration(channel)){if(!config.IsEnabled){config.IsEnabled=true;config.SaveChanges();changes.Add(D("name",channel,"before","disabled","after","enabled"));}}}
                catch(Exception error){changeError=error.Message;}
            }
            var row=LogCapability(channel);if(changeError!=null)row["error"]=changeError;logs.Add(row);
        }
        return D("checked_at",Now(),"elevated",elevated,"user_sid",UserSid(),"backend","native","audits",audits,"logs",logs,"changes",changes,"history_warning","检查通过只说明当前能力；未记录、已覆盖、已清空的历史无法恢复。");
    }
    static object Input() {
        var value=new StringBuilder();char[] buffer=new char[4096];int count;
        while((count=Console.In.Read(buffer,0,buffer.Length))>0){value.Append(buffer,0,count);if(Encoding.UTF8.GetByteCount(value.ToString())>65536)throw new Exception("Request exceeds 64 KiB");}
        return value.Length==0?D():Json.DeserializeObject(value.ToString());
    }
    [DllImport("ole32.dll")] static extern int CoInitializeSecurity(IntPtr descriptor,int services,IntPtr authenticationServices,IntPtr reserved,uint authentication,uint impersonation,IntPtr authenticationList,uint capabilities,IntPtr reserved2);
    [STAThread]
    static void Main(string[] args) {
        Console.InputEncoding=Encoding.UTF8;Console.OutputEncoding=new UTF8Encoding(false);
        // Native APIs can stall. The supervisor also bounds child lifetimes; standalone tests get this guard.
        var deadline=new System.Threading.Thread(()=>{System.Threading.Thread.Sleep(110000);Environment.Exit(124);});deadline.IsBackground=true;deadline.Start();
        try {
            // Client-only process. WUA/WMI servers must impersonate the actual caller, not identify-only.
            int security=CoInitializeSecurity(IntPtr.Zero,-1,IntPtr.Zero,IntPtr.Zero,0,3,IntPtr.Zero,0,IntPtr.Zero);
            if(security<0 && security!=unchecked((int)0x80010119))Marshal.ThrowExceptionForHR(security);
            if(args.Length>0 && args[0]=="capabilities") {if(args.Length>2 || (args.Length==2&&args[1]!="--repair"))throw new Exception("Unsupported capability arguments");Console.WriteLine(Json.Serialize(Capabilities(args.Length==2)));return;}
            if(args.Length==1 && args[0]=="layout-test") {DriverLayoutTest();return;}
            if(args.Length==1 && args[0]=="servicing-firmware-test") {ServicingFirmwareTest();return;}
            if(args.Length==1 && args[0]=="account-contract-test") {AccountContractTest();return;}
            if(args.Length==1 && args[0]=="vpn-contract-test") {VpnContractTest();return;}
            if(args.Length==1 && args[0]=="encryption-contract-test") {EncryptionContractTest();return;}
            if(args.Length==1 && args[0]=="app-package-contract-test") {AppPackageContractTest();return;}
            if(args.Length==1 && args[0]=="provisioned-contract-test") {ProvisionedContractTest();return;}
            if(args.Length==1 && args[0]=="tpm-contract-test") {TpmContractTest();return;}
            if(args.Length==1 && args[0]=="tpm-modern-lockout-assess") {RequireStateIntegrity();if(Elevated())throw new InvalidOperationException("This assessment requires an ordinary application token");Console.WriteLine(Json.Serialize(D("query","TPM2_GetCapability/TPM_PT_PERMANENT","LockedOut",TpmModernLockout(),"elevated",false,"state_changes",false)));return;}
            if(args.Length==1 && args[0]=="tpm-readonly-metadata-assess") {TpmReadOnlyMetadataAssessment();return;}
            if(args.Length>0 && args[0]=="tpm-migration-assess") {TpmMigrationAssessment(args);return;}
            if(args.Length==1 && args[0]=="hyperv-switch-contract-test") {HypervSwitchContractTest();return;}
            if(args.Length==1 && args[0]=="hyperv-vm-contract-test") {HypervVmContractTest();return;}
            if(args.Length==1 && args[0]=="hyperv-snapshot-contract-test") {HypervSnapshotContractTest();return;}
            if(args.Length==1 && args[0]=="hyperv-disk-contract-test") {HypervDiskContractTest();return;}
            if(args.Length==1 && args[0]=="state-test") {StateContractTest();return;}
            if(args.Length>0 && args[0]=="assess") {Assessment(args);return;}
            if(args.Length>0 && args[0]=="migration-assess") {MigrationAssessment(args);return;}
            if(args.Length==2 && args[0]=="docker-fixture" && System.Text.RegularExpressions.Regex.IsMatch(args[1],@"^pcscope-docker-test-[a-f0-9]{32}$")){DockerFixturePipe=args[1];Collect("scoped",Input());return;}
            if(args.Length>1)throw new Exception("Exactly one collector unit allowed");
            Collect(args.Length==0?"base":args[0],Input());
        }catch(Exception error){Console.Error.WriteLine(error.GetType().Name+": "+error.Message);Environment.ExitCode=1;}
    }
    static void Assessment(string[] args) {
        if(args.Length!=3)throw new Exception("assess requires output directory and original user SID");
        if(UserSid()!=args[2])throw new Exception("Elevation must use the original Windows account; no other user's encrypted data will be opened");
        string output=Path.GetFullPath(args[1]);
        if(!Directory.Exists(output)||(File.GetAttributes(output)&FileAttributes.ReparsePoint)!=0)throw new Exception("Assessment output must be an existing ordinary directory");
        // Read-only capability assessment. No repair, service launch, journal creation or arbitrary command.
        File.WriteAllText(Path.Combine(output,"capabilities.json"),Json.Serialize(Capabilities(false)),new UTF8Encoding(false));
        // Preserve original API material for offline identity/adapter checks, even if emission is rejected.
        List<object> driverItems=null;Exception driverError=null;
        try{driverItems=Drivers();File.WriteAllText(Path.Combine(output,"driver-api-items.json"),Json.Serialize(driverItems),new UTF8Encoding(false));}
        catch(Exception error){driverError=error;File.WriteAllText(Path.Combine(output,"driver-api-error.json"),Json.Serialize(D("error",error.Message)),new UTF8Encoding(false));}
        using(var writer=new StreamWriter(Path.Combine(output,"native-sources.jsonl"),false,new UTF8Encoding(false))) {
            var old=Console.Out;Console.SetOut(writer);
            try {
                Request=D();Run=Guid.NewGuid().ToString();using(var root=Microsoft.Win32.RegistryKey.OpenBaseKey(Microsoft.Win32.RegistryHive.LocalMachine,Microsoft.Win32.RegistryView.Registry64))using(var key=root.OpenSubKey("SOFTWARE\\Microsoft\\Cryptography"))Machine=Text(key.GetValue("MachineGuid"));
                if(driverItems!=null)Emit("pcscope.hardware.driver-packages",()=>driverItems);else Gap("pcscope.hardware.driver-packages","",driverError);
                Collect("security",D("selection",D("disabled_sources",new[]{"pcscope.collector.event-channel-coverage"})));
            }
            finally{Console.SetOut(old);}
        }
        File.WriteAllText(Path.Combine(output,"complete.json"),Json.Serialize(D("elevated",Elevated(),"same_user",true,"repair",false,"complete",true)),new UTF8Encoding(false));
    }
    static void MigrationAssessment(string[] args) {
        if(args.Length!=3||UserSid()!=args[2])throw new Exception("Read-only migration assessment requires the original account SID");
        if(!Elevated())throw new UnauthorizedAccessException("Read-only migration assessment requires Windows administrator authorization");
        string output=Path.GetFullPath(args[1]);if(!Directory.Exists(output))throw new Exception("Assessment output directory must already exist");
        for(var directory=new DirectoryInfo(output);directory!=null;directory=directory.Parent)if((directory.Attributes&FileAttributes.ReparsePoint)!=0)throw new Exception("Assessment output must not pass through reparse points");
        // Only these four metadata reads are allowed. No daemon, repair, commands or system configuration changes.
        Environment.SetEnvironmentVariable("TEMP",output);Environment.SetEnvironmentVariable("TMP",output);
        Request=D();Run=Guid.NewGuid().ToString();using(var root=Microsoft.Win32.RegistryKey.OpenBaseKey(Microsoft.Win32.RegistryHive.LocalMachine,Microsoft.Win32.RegistryView.Registry64))using(var key=root.OpenSubKey("SOFTWARE\\Microsoft\\Cryptography"))Machine=Text(key.GetValue("MachineGuid"));
        using(var file=new FileStream(Path.Combine(output,"migration-sources.jsonl"),FileMode.CreateNew,FileAccess.Write,FileShare.Read))using(var writer=new StreamWriter(file,new UTF8Encoding(false))) {
            var previous=Console.Out;Console.SetOut(writer);
            try {
                Emit("pcscope.software.optional-features",OptionalFeatures);
                Emit("pcscope.software.provisioned",ProvisionedPackages);
                Emit("pcscope.security.secureboot",SecureBoot);
                Emit("pcscope.security.encryption",EncryptionVolumes);
            }finally{Console.SetOut(previous);}
        }
        using(var file=new FileStream(Path.Combine(output,"complete.json"),FileMode.CreateNew,FileAccess.Write,FileShare.Read))using(var writer=new StreamWriter(file,new UTF8Encoding(false)))writer.Write(Json.Serialize(D("complete",true,"source_queries",4,"elevated",true,"same_user",true,"system_configuration_changes",false,"recovery_material_read",false,"source_result_validation","pending")));
    }
}
