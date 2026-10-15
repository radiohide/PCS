using System;
using System.Collections.Generic;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Management;
using System.IO;
using System.Text;

partial class Platform {
    [StructLayout(LayoutKind.Sequential)] struct TpmDeviceInfo {public uint structureVersion,tpmVersion,interfaceType,implementationRevision;}
    [DllImport("tbs.dll")]static extern uint Tbsi_GetDeviceInfo(uint size,out TpmDeviceInfo info);
    [DllImport("TpmCoreProvisioning.dll")]static extern uint TpmIsReady(out byte value);
    [DllImport("TpmCoreProvisioning.dll")]static extern uint TpmIsEnabled(out byte value);
    [DllImport("TpmCoreProvisioning.dll")]static extern uint TpmIsActivated(out byte value);
    [DllImport("TpmCoreProvisioning.dll")]static extern uint TpmIsOwned(out byte value);
    [DllImport("TpmCoreProvisioning.dll")]static extern uint TpmGet_ManufacturerId(out uint value);
    [DllImport("TpmCoreProvisioning.dll")]static extern uint TpmGet_ManufacturerVersion(ref uint count,[Out]ushort[] value);
    [DllImport("TpmCoreProvisioning.dll")]static extern uint TpmGetPhysicalPresenceRequest(out uint value);
    delegate uint TpmBooleanQuery(out byte value);
    static void TpmStatus(uint status){if(status!=0)throw new InvalidOperationException("TPM metadata query failed: 0x"+status.ToString("X8",CultureInfo.InvariantCulture));}
    static bool TpmBoolean(TpmBooleanQuery query){byte value;TpmStatus(query(out value));if(value>1)throw new InvalidOperationException("Invalid TPM boolean metadata");return value==1;}
    static string TpmManufacturerText(uint id) {
        if(id==0)return "";var chars=new char[4];for(int i=0;i<4;i++){byte value=(byte)(id>>(24-i*8));if(value!=0&&(value<32||value>126))return id.ToString(CultureInfo.InvariantCulture);chars[i]=(char)value;}return new string(chars);
    }
    static string TpmManufacturerVersion() {
        uint count=32;var buffer=new ushort[33];uint status=TpmGet_ManufacturerVersion(ref count,buffer);
        if(status==0x80290106){if(count==0||count>4096)throw new InvalidOperationException("TPM version exceeds metadata budget");buffer=new ushort[count+1];status=TpmGet_ManufacturerVersion(ref count,buffer);}
        TpmStatus(status);if(count>buffer.Length)throw new InvalidOperationException("Invalid TPM version result length");var chars=new char[count];for(int i=0;i<count;i++)chars[i]=(char)buffer[i];return new string(chars);
    }
    [StructLayout(LayoutKind.Sequential)] struct TpmLegacyContext {public uint version;}
    [DllImport("tbs.dll")]static extern uint Tbsi_Context_Create(ref TpmLegacyContext parameters,out IntPtr context);
    [StructLayout(LayoutKind.Sequential)] struct TpmModernContext {public uint version,flags;}
    [DllImport("tbs.dll")]static extern uint Tbsi_Context_Create(ref TpmModernContext parameters,out IntPtr context);
    [DllImport("tbs.dll")]static extern uint Tbsip_Context_Close(IntPtr context);
    [DllImport("tbs.dll")]static extern uint Tbsip_Submit_Command(IntPtr context,uint locality,uint priority,byte[] command,uint commandSize,[Out]byte[] result,ref uint resultSize);
    static uint TpmWire32(byte[] data,int offset){return ((uint)data[offset]<<24)|((uint)data[offset+1]<<16)|((uint)data[offset+2]<<8)|data[offset+3];}
    static ushort TpmWire16(byte[] data,int offset){return (ushort)((data[offset]<<8)|data[offset+1]);}
    static byte[] TpmLegacyLockoutCommand(ushort entity) {
        if(entity!=1&&entity!=2&&entity!=4)throw new InvalidOperationException("TPM legacy metadata entity is outside fixed scope");
        // Fixed unauthenticated TPM_GetCapability, TPM_CAP_DA_LOGIC; never accept arbitrary ordinals or payloads.
        return new byte[]{0,0xC1,0,0,0,20,0,0,0,0x65,0,0,0,0x19,0,0,0,2,0,(byte)entity};
    }
    static bool TpmLegacyLockoutResponse(byte[] data,uint length) {
        if(data==null||length<10||length>data.Length||length>4096||TpmWire16(data,0)!=0xC4||TpmWire32(data,2)!=length)throw new InvalidOperationException("Invalid TPM legacy metadata response framing");
        uint result=TpmWire32(data,6);if(result!=0)throw new InvalidOperationException("TPM legacy capability failed: 0x"+result.ToString("X8",CultureInfo.InvariantCulture));
        if(length<17||TpmWire32(data,10)!=length-14)throw new InvalidOperationException("Invalid TPM legacy capability length");
        int start=14;ushort tag=TpmWire16(data,start);int action,vendor;
        if(tag==0x37){action=start+7;vendor=start+17;}else if(tag==0x38){action=start+3;vendor=start+9;}else throw new InvalidOperationException("Unknown TPM legacy dictionary-attack structure");
        if(length<vendor+4||TpmWire16(data,action)!=0x39||TpmWire32(data,vendor)!=length-(uint)(vendor+4))throw new InvalidOperationException("Invalid TPM legacy dictionary-attack structure length");
        byte state=data[start+2];uint actions=TpmWire32(data,action+2);if(state>1||(actions&~15U)!=0||(state==1&&actions==0))throw new InvalidOperationException("Invalid TPM legacy dictionary-attack state");
        return state==1;
    }
    static bool TpmLegacyLockout() {
        var parameters=new TpmLegacyContext{version=1};IntPtr context;TpmStatus(Tbsi_Context_Create(ref parameters,out context));
        try {
            bool locked=false;foreach(ushort entity in new ushort[]{1,2,4}) {
                byte[] command=TpmLegacyLockoutCommand(entity),result=new byte[4096];uint size=(uint)result.Length;
                TpmStatus(Tbsip_Submit_Command(context,0,100,command,(uint)command.Length,result,ref size));
                locked|=TpmLegacyLockoutResponse(result,size);
            }
            return locked;
        }finally{Tbsip_Context_Close(context);}
    }
    static byte[] TpmModernLockoutCommand() {
        // TPM2_GetCapability(TPM_CAP_TPM_PROPERTIES, TPM_PT_PERMANENT, 1), no authorization session.
        return new byte[]{0x80,1,0,0,0,22,0,0,1,0x7A,0,0,0,6,0,0,2,0,0,0,0,1};
    }
    static bool TpmModernLockoutResponse(byte[] data,uint length) {
        if(data==null||length<10||length>data.Length||length>4096||TpmWire16(data,0)!=0x8001||TpmWire32(data,2)!=length)throw new InvalidOperationException("Invalid TPM 2.0 metadata response framing");
        uint result=TpmWire32(data,6);if(result!=0)throw new InvalidOperationException("TPM 2.0 capability failed: 0x"+result.ToString("X8",CultureInfo.InvariantCulture));
        if(length!=27||data[10]>1||TpmWire32(data,11)!=6||TpmWire32(data,15)!=1||TpmWire32(data,19)!=0x200)throw new InvalidOperationException("TPM 2.0 permanent property unavailable or ambiguous");
        uint flags=TpmWire32(data,23);if((flags&~0x707U)!=0)throw new InvalidOperationException("Unknown TPM 2.0 permanent property bits");
        return (flags&0x200)!=0;
    }
    static bool TpmModernLockout() {
        var parameters=new TpmModernContext{version=2,flags=4};IntPtr context;TpmStatus(Tbsi_Context_Create(ref parameters,out context));
        try {
            byte[] command=TpmModernLockoutCommand(),result=new byte[4096];uint size=(uint)result.Length;
            TpmStatus(Tbsip_Submit_Command(context,0,100,command,(uint)command.Length,result,ref size));
            return TpmModernLockoutResponse(result,size);
        }finally{Tbsip_Context_Close(context);}
    }
    static List<object> TpmMetadata() {
        RequireStateIntegrity();TpmDeviceInfo device;uint status=Tbsi_GetDeviceInfo((uint)Marshal.SizeOf(typeof(TpmDeviceInfo)),out device);
        if(status==0x8028400F)return new List<object>{Item("tpm",D("TpmPresent",false,"TpmReady",false,"TpmEnabled",false,"TpmActivated",false,"TpmOwned",false,"RestartPending",false,"ManufacturerIdTxt",null,"ManufacturerVersion",null,"LockedOut",false))};
        TpmStatus(status);
        if(device.tpmVersion!=1&&device.tpmVersion!=2)throw new PlatformNotSupportedException("Unknown TPM generation");
        uint manufacturer,request;TpmStatus(TpmGet_ManufacturerId(out manufacturer));string version=TpmManufacturerVersion();
        bool ready=TpmBoolean(TpmIsReady),enabled=TpmBoolean(TpmIsEnabled),activated=TpmBoolean(TpmIsActivated),owned=TpmBoolean(TpmIsOwned);
        TpmStatus(TpmGetPhysicalPresenceRequest(out request));bool locked;
        if(device.tpmVersion==1)locked=TpmLegacyLockout();else locked=TpmModernLockout();
        return new List<object>{Item("tpm",D("TpmPresent",true,"TpmReady",ready,"TpmEnabled",enabled,"TpmActivated",activated,"TpmOwned",owned,"RestartPending",request!=0,"ManufacturerIdTxt",TpmManufacturerText(manufacturer),"ManufacturerVersion",version,"LockedOut",locked))};
    }
    static void CollectTpmMetadata(string unit){if(unit=="security")Emit("pcscope.security.tpm",TpmMetadata);}
    static object TpmAssessmentQuery(string name,Func<object> read) {
        try{return D("query",name,"available",true,"value",read());}
        catch(Exception error){return D("query",name,"available",false,"error_type",error.GetType().Name,"error",error.Message);}
    }
    static uint TpmWmiPhysicalPresenceRequest() {
        // Fixed documented read-only method. No caller-selected class, namespace, operation,
        // credentials, pending operation write or TPM owner authorization.
        var connection=new ConnectionOptions{Impersonation=ImpersonationLevel.Impersonate,Authentication=AuthenticationLevel.PacketPrivacy,Timeout=TimeSpan.FromSeconds(15)};
        var scope=new ManagementScope(@"\\.\root\CIMV2\Security\MicrosoftTpm",connection);
        scope.Connect();
        using(var query=new ManagementObjectSearcher(scope,new ObjectQuery("SELECT * FROM Win32_Tpm"),new EnumerationOptions{Timeout=TimeSpan.FromSeconds(15),Rewindable=false}))
        using(var instances=query.Get()) {
            uint request=0;int count=0;
            foreach(ManagementObject instance in instances)using(instance){
                if(++count>1)throw new InvalidOperationException("TPM provider returned multiple physical devices");
                using(var result=instance.InvokeMethod("GetPhysicalPresenceRequest",(ManagementBaseObject)null,new InvokeMethodOptions{Timeout=TimeSpan.FromSeconds(15)})) {
                    if(result==null||!(result["ReturnValue"] is uint)||!(result["Request"] is uint))throw new InvalidOperationException("TPM physical presence result omitted status or request");
                    TpmStatus((uint)result["ReturnValue"]);request=(uint)result["Request"];
                }
            }
            if(count!=1)throw new InvalidOperationException("TPM provider returned no physical device");
            return request;
        }
    }
    static void TpmReadOnlyMetadataAssessment() {
        RequireStateIntegrity();if(Elevated())throw new InvalidOperationException("TPM diagnostic requires an ordinary application token");
        var reads=new List<object>();
        reads.Add(TpmAssessmentQuery("Tbsi_GetDeviceInfo",()=>{TpmDeviceInfo device;TpmStatus(Tbsi_GetDeviceInfo((uint)Marshal.SizeOf(typeof(TpmDeviceInfo)),out device));return D("structure_version",device.structureVersion,"tpm_version",device.tpmVersion,"interface_type",device.interfaceType,"implementation_revision",device.implementationRevision);}));
        reads.Add(TpmAssessmentQuery("TpmGet_ManufacturerId",()=>{uint value;TpmStatus(TpmGet_ManufacturerId(out value));return TpmManufacturerText(value);}));
        reads.Add(TpmAssessmentQuery("TpmGet_ManufacturerVersion",()=>TpmManufacturerVersion()));
        reads.Add(TpmAssessmentQuery("TpmIsReady",()=>TpmBoolean(TpmIsReady)));
        reads.Add(TpmAssessmentQuery("TpmIsEnabled",()=>TpmBoolean(TpmIsEnabled)));
        reads.Add(TpmAssessmentQuery("TpmIsActivated",()=>TpmBoolean(TpmIsActivated)));
        reads.Add(TpmAssessmentQuery("TpmIsOwned",()=>TpmBoolean(TpmIsOwned)));
        reads.Add(TpmAssessmentQuery("TpmGetPhysicalPresenceRequest",()=>{uint value;TpmStatus(TpmGetPhysicalPresenceRequest(out value));return value;}));
        reads.Add(TpmAssessmentQuery("Win32_Tpm.GetPhysicalPresenceRequest",()=>TpmWmiPhysicalPresenceRequest()));
        // The same fixed unauthenticated TPM 2 metadata query already used by the standalone
        // lockout assessment. Unsupported generations return an error, never a false state.
        reads.Add(TpmAssessmentQuery("TPM2_GetCapability/TPM_PT_PERMANENT",()=>TpmModernLockout()));
        Console.WriteLine(Json.Serialize(D("queries",reads,"elevated",false,"state_changes",false,"owner_auth_read",false,"self_test_run",false,"complete_source_validated",false)));
    }
    static void TpmMigrationAssessment(string[] args) {
        if(args.Length!=3||UserSid()!=args[2])throw new InvalidOperationException("TPM read-only migration assessment requires the original account SID");
        if(!Elevated())throw new UnauthorizedAccessException("TPM read-only migration assessment requires Windows administrator authorization");
        string output=Path.GetFullPath(args[1]);
        if(!Directory.Exists(output))throw new InvalidOperationException("TPM assessment directory must already exist");
        for(var directory=new DirectoryInfo(output);directory!=null;directory=directory.Parent)
            if((directory.Attributes&FileAttributes.ReparsePoint)!=0)throw new InvalidOperationException("TPM assessment directory must not pass through reparse points");
        // One fixed metadata source only; never run Get-Tpm, provisioning, self-test, owner-auth,
        // arbitrary TPM commands, the daemon, or any of the other migration sources.
        Request=D();Run=Guid.NewGuid().ToString();
        using(var root=Microsoft.Win32.RegistryKey.OpenBaseKey(Microsoft.Win32.RegistryHive.LocalMachine,Microsoft.Win32.RegistryView.Registry64))
        using(var key=root.OpenSubKey("SOFTWARE\\Microsoft\\Cryptography"))Machine=Text(key.GetValue("MachineGuid"));
        using(var file=new FileStream(Path.Combine(output,"tpm-sources.jsonl"),FileMode.CreateNew,FileAccess.Write,FileShare.Read))
        using(var writer=new StreamWriter(file,new UTF8Encoding(false))) {
            var previous=Console.Out;Console.SetOut(writer);
            try{Emit("pcscope.security.tpm",TpmMetadata);}finally{Console.SetOut(previous);}
        }
        using(var file=new FileStream(Path.Combine(output,"complete.json"),FileMode.CreateNew,FileAccess.Write,FileShare.Read))
        using(var writer=new StreamWriter(file,new UTF8Encoding(false)))writer.Write(Json.Serialize(D("complete",true,"source_queries",1,"elevated",true,"same_user",true,"state_changes",false,"owner_auth_read",false,"self_test_run",false,"source_result_validation","pending")));
    }
    static byte[] TpmContractBytes(string hex){var data=new byte[hex.Length/2];for(int i=0;i<data.Length;i++)data[i]=Byte.Parse(hex.Substring(i*2,2),NumberStyles.HexNumber,CultureInfo.InvariantCulture);return data;}
    static void TpmLegacyContractTest() {
        byte[] full=TpmContractBytes("00C4000000230000000000000015003701000200030039000000010000000800000000");
        byte[] limited=TpmContractBytes("00C40000001B000000000000000D00380000390000000100000000");
        if(!TpmLegacyLockoutResponse(full,(uint)full.Length)||TpmLegacyLockoutResponse(limited,(uint)limited.Length))throw new Exception("TPM legacy full/limited state decoding mismatch");
        limited[16]=1;if(!TpmLegacyLockoutResponse(limited,(uint)limited.Length))throw new Exception("TPM legacy limited active state missed");
        // Explicit device state wins over counter inference: inactive may still have a high count.
        full[16]=0;full[17]=255;full[18]=255;if(TpmLegacyLockoutResponse(full,(uint)full.Length))throw new Exception("TPM legacy lockout inferred from count");
        byte[] command=TpmLegacyLockoutCommand(2);if(BitConverter.ToString(command)!="00-C1-00-00-00-14-00-00-00-65-00-00-00-19-00-00-00-02-00-02")throw new Exception("TPM legacy readonly command framing mismatch");
        bool rejected=false;try{TpmLegacyLockoutCommand(3);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Unknown TPM legacy entity accepted");
        foreach(int mutation in new[]{0,2,10,15,16,19,24,26}) {
            byte[] malformed=(byte[])limited.Clone();malformed[mutation]=255;rejected=false;try{TpmLegacyLockoutResponse(malformed,(uint)malformed.Length);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Malformed TPM legacy response accepted at "+mutation);
        }
        rejected=false;try{TpmLegacyLockoutResponse(limited,(uint)limited.Length-1);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Truncated TPM legacy response accepted");
        byte[] unsupported=TpmContractBytes("00C40000000A0000003D");rejected=false;try{TpmLegacyLockoutResponse(unsupported,(uint)unsupported.Length);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Unsupported capability turned into unlocked state");
    }
    static void TpmContractTest() {
        if(Marshal.SizeOf(typeof(TpmDeviceInfo))!=16||TpmManufacturerText(0x49424D00)!="IBM\0"||TpmManufacturerText(0x01020304)!="16909060")throw new Exception("TPM metadata representation mismatch");
        bool rejected=false;try{TpmStatus(0x80284012);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("TPM access failure accepted");
        TpmLegacyContractTest();
        TpmModernContractTest();
        Console.WriteLine(Json.Serialize(D("device_layout",16,"manufacturer_representation",true,"modern_permanent_lockout_bit",true,"modern_fixed_readonly_command",true,"modern_wrong_property_and_malformed_rejected",true,"legacy_full_and_limited_state",true,"legacy_not_inferred_from_count",true,"legacy_fixed_readonly_command",true,"legacy_malformed_and_unsupported_rejected",true,"access_failure_rejected",true,"owner_auth_read",false,"state_changes",false)));
    }
    static void TpmModernContractTest() {
        if(Marshal.SizeOf(typeof(TpmModernContext))!=8||Marshal.SizeOf(typeof(TpmLegacyContext))!=4)throw new Exception("TPM context ABI mismatch");
        byte[] response=TpmContractBytes("80010000001B000000000100000006000000010000020000000200");
        if(!TpmModernLockoutResponse(response,(uint)response.Length))throw new Exception("TPM 2.0 reported lockout missed");
        response[25]=0;if(TpmModernLockoutResponse(response,(uint)response.Length))throw new Exception("TPM 2.0 lockout invented");
        response[25]=5;response[26]=7;if(TpmModernLockoutResponse(response,(uint)response.Length))throw new Exception("Other permanent flags interpreted as lockout");
        if(BitConverter.ToString(TpmModernLockoutCommand())!="80-01-00-00-00-16-00-00-01-7A-00-00-00-06-00-00-02-00-00-00-00-01")throw new Exception("TPM 2.0 fixed metadata command mismatch");
        foreach(int mutation in new[]{0,2,6,10,14,18,22,23}) {
            byte[] malformed=(byte[])response.Clone();malformed[mutation]=255;bool rejected=false;try{TpmModernLockoutResponse(malformed,(uint)malformed.Length);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Invalid TPM 2.0 response accepted at "+mutation);
        }
        bool truncated=false;try{TpmModernLockoutResponse(response,(uint)response.Length-1);}catch(InvalidOperationException){truncated=true;}if(!truncated)throw new Exception("Truncated TPM 2.0 response accepted");
    }
}
