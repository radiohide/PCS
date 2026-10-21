using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

partial class Platform {
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,EntryPoint="GetModuleHandleW")]
    static extern IntPtr BootModule(string name);
    [DllImport("kernel32.dll",CharSet=CharSet.Ansi,ExactSpelling=true,EntryPoint="GetProcAddress")]
    static extern IntPtr BootProcedure(IntPtr module,string name);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int QueryBootInformation(int kind,IntPtr data,int length,out int returned);

    // SystemBootEnvironmentInformation (90), GUID at offset zero. Resolve at runtime:
    // this native interface may be absent on another Windows build. Never substitute a
    // rounded wall-clock boot time for its identity (WMI time can drift between reads).
    static string ReadBootIdentifier() {
        var address=BootProcedure(BootModule("ntdll.dll"),"NtQuerySystemInformation");
        if(address==IntPtr.Zero)throw new NotSupportedException("Windows boot identifier is unavailable");
        var query=(QueryBootInformation)Marshal.GetDelegateForFunctionPointer(address,typeof(QueryBootInformation));
        int length=32;
        for(int attempt=0;attempt<2;attempt++) {
            var data=Marshal.AllocHGlobal(length);
            try {
                int returned;int status=query(90,data,length,out returned);
                if(status>=0 && returned>=16 && returned<=length) {
                    var bytes=new byte[16];Marshal.Copy(data,bytes,0,16);var id=new Guid(bytes);
                    if(id==Guid.Empty)throw new InvalidOperationException("Empty Windows boot identifier");
                    return id.ToString("D");
                }
                if(status==unchecked((int)0xC0000004) && returned>length && returned<=4096){length=returned;continue;}
                throw new InvalidOperationException("Windows boot identifier query failed: 0x"+status.ToString("x8"));
            } finally {Marshal.FreeHGlobal(data);}
        }
        throw new InvalidOperationException("Windows boot identifier layout changed");
    }
    static Dictionary<string,object> BootMetadata(Dictionary<string,object> data) {
        try{data["boot_id"]=ReadBootIdentifier();data["boot_id_basis"]="SystemBootEnvironmentInformation";}
        catch(Exception error){data["boot_id_error"]=error.Message;}
        return data;
    }
    static List<object> BootSessionItems(List<object> items) {
        var metadata=BootMetadata(D());
        foreach(Dictionary<string,object> item in items) {
            var data=(Dictionary<string,object>)item["data"];
            foreach(var pair in metadata)data[pair.Key]=pair.Value;
        }
        return items;
    }
    static void BootIdentityAssessment() {
        string first=ReadBootIdentifier();
        for(int i=0;i<2;i++)if(ReadBootIdentifier()!=first)throw new Exception("Boot identifier changed between reads");
        Console.WriteLine(Json.Serialize(D("boot_id",first,"reads",3,"stable",true,"elevated",Elevated(),"system_changes",false)));
    }
}
