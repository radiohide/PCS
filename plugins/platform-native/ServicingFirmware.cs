using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;

partial class Platform {
    [StructLayout(LayoutKind.Sequential,Pack=1)] struct NativeFeature {public IntPtr name;public int state;}
    [DllImport("dismapi.dll",CharSet=CharSet.Unicode)]static extern int DismGetFeatures(uint session,string identifier,int packageIdentifier,out IntPtr features,out uint count);
    static string FeatureState(int state) {
        switch(state) {
            case 0:case 2:return "Disabled";
            case 1:return "DisablePending";
            case 3:return "DisabledWithPayloadRemoved";
            case 4:return "Enabled";
            case 5:return "EnablePending";
            case 6:return "Superseded";
            case 7:return "PartiallyInstalled";
            default:throw new InvalidOperationException("Unrecognized DISM feature state: "+state);
        }
    }
    static List<object> OptionalFeatures() {
        RequireStateIntegrity();
        if(!Elevated())throw new UnauthorizedAccessException("Reading online Windows optional features requires administrator authorization");
        string log=Path.Combine(Path.GetTempPath(),"pcscope-native-features-"+Guid.NewGuid().ToString("N")+".log");
        HResult(DismInitialize(0,log,null));uint session=0;bool opened=false;IntPtr buffer=IntPtr.Zero;
        try {
            HResult(DismOpenSession("DISM_{53BFAE52-B167-4E2F-A258-0A37B57FF845}",null,null,out session));opened=true;
            uint count;HResult(DismGetFeatures(session,null,0,out buffer,out count));
            if(count>50000||(count>0&&buffer==IntPtr.Zero))throw new InvalidOperationException("Invalid optional feature result size");
            int size=Marshal.SizeOf(typeof(NativeFeature));var rows=new List<object>();
            for(uint i=0;i<count;i++) {
                var feature=(NativeFeature)Marshal.PtrToStructure(IntPtr.Add(buffer,checked((int)i*size)),typeof(NativeFeature));
                string name=Marshal.PtrToStringUni(feature.name);
                if(String.IsNullOrWhiteSpace(name))throw new InvalidOperationException("Windows optional feature has no identity");
                rows.Add(Item(name,D("name",name,"state",FeatureState(feature.state))));
            }
            return rows;
        }finally{if(buffer!=IntPtr.Zero)DismDelete(buffer);if(opened)DismCloseSession(session);DismShutdown();}
    }
    [StructLayout(LayoutKind.Sequential)] struct FirmwareLuid {public uint low;public int high;}
    [StructLayout(LayoutKind.Sequential)] struct FirmwarePrivilege {public uint count;public FirmwareLuid luid;public uint attributes;}
    [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern bool LookupPrivilegeValue(string system,string name,out FirmwareLuid luid);
    [DllImport("advapi32.dll",SetLastError=true)]static extern bool AdjustTokenPrivileges(IntPtr token,bool disable,ref FirmwarePrivilege privileges,uint length,out FirmwarePrivilege previous,out uint required);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern uint GetFirmwareEnvironmentVariableExW(string name,string guid,[Out]byte[] buffer,uint size,out uint attributes);
    [DllImport("kernel32.dll",SetLastError=true)]static extern bool GetFirmwareType(out uint type);
    static bool DecodeSecureBoot(byte[] value,uint length) {
        if(length!=1||value.Length<1||value[0]>1)throw new InvalidOperationException("Firmware returned an invalid SecureBoot value");
        return value[0]==1;
    }
    static List<object> SecureBoot() {
        RequireStateIntegrity();uint firmware;
        if(!GetFirmwareType(out firmware))throw new Win32Exception(Marshal.GetLastWin32Error());
        if(firmware!=2)throw new PlatformNotSupportedException("Secure boot is unavailable when Windows starts using legacy firmware");
        IntPtr token;if(!OpenProcessToken(Process.GetCurrentProcess().Handle,0x28,out token))throw new Win32Exception(Marshal.GetLastWin32Error());
        FirmwarePrivilege previous=new FirmwarePrivilege();bool changed=false;
        try {
            FirmwareLuid luid;if(!LookupPrivilegeValue(null,"SeSystemEnvironmentPrivilege",out luid))throw new Win32Exception(Marshal.GetLastWin32Error());
            var desired=new FirmwarePrivilege{count=1,luid=luid,attributes=2};uint required;
            bool adjusted=AdjustTokenPrivileges(token,false,ref desired,(uint)Marshal.SizeOf(typeof(FirmwarePrivilege)),out previous,out required);
            int error=Marshal.GetLastWin32Error();changed=adjusted&&previous.count>0;
            if(!adjusted||error!=0)throw new Win32Exception(error);
            // Reads the firmware variable only. No firmware writes or system policy changes.
            var value=new byte[16];uint attributes;uint length=GetFirmwareEnvironmentVariableExW("SecureBoot","{8BE4DF61-93CA-11D2-AA0D-00E098032B8C}",value,(uint)value.Length,out attributes);
            if(length==0)throw new Win32Exception(Marshal.GetLastWin32Error());
            return new List<object>{Item("secureboot",D("enabled",DecodeSecureBoot(value,length)))};
        }finally {
            if(changed){FirmwarePrivilege discarded;uint required;AdjustTokenPrivileges(token,false,ref previous,(uint)Marshal.SizeOf(typeof(FirmwarePrivilege)),out discarded,out required);}
            CloseHandle(token);
        }
    }
    static void ServicingFirmwareTest() {
        if(Marshal.SizeOf(typeof(NativeFeature))!=12||Marshal.OffsetOf(typeof(NativeFeature),"state").ToInt64()!=8)throw new Exception("DISM feature packed layout mismatch");
        string[] expected={"Disabled","DisablePending","Disabled","DisabledWithPayloadRemoved","Enabled","EnablePending","Superseded","PartiallyInstalled"};
        for(int i=0;i<expected.Length;i++)if(FeatureState(i)!=expected[i])throw new Exception("Feature state mismatch");
        bool rejected=false;try{FeatureState(99);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Unknown feature state accepted");
        if(DecodeSecureBoot(new byte[]{0},1)||!DecodeSecureBoot(new byte[]{1},1))throw new Exception("SecureBoot boolean mismatch");
        foreach(byte[] invalid in new[]{new byte[]{2},new byte[]{0,1},new byte[0]}){rejected=false;try{DecodeSecureBoot(invalid,(uint)invalid.Length);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Malformed firmware value accepted");}
        Console.WriteLine(Json.Serialize(D("checks",14,"packed_feature_size",12,"unknown_states_rejected",true,"firmware_values_validated",true)));
    }
    // Prototype routes: production ownership is switched after platform acceptance.
    static void CollectServicingFirmware(string unit) {
        if(unit=="system")Emit("pcscope.software.optional-features",OptionalFeatures);
        if(unit=="security")Emit("pcscope.security.secureboot",SecureBoot);
    }
}
