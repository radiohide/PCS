using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;

partial class Platform {
    [StructLayout(LayoutKind.Sequential)] struct LocalGroupInfo {public IntPtr name,description;}
    [DllImport("netapi32.dll",CharSet=CharSet.Unicode)]static extern uint NetLocalGroupEnum(string server,uint level,out IntPtr buffer,uint preferred,out uint read,out uint total,ref UIntPtr resume);
    [DllImport("netapi32.dll",CharSet=CharSet.Unicode)]static extern uint NetLocalGroupGetMembers(string server,string group,uint level,out IntPtr buffer,uint preferred,out uint read,out uint total,ref UIntPtr resume);
    [DllImport("netapi32.dll")]static extern uint NetApiBufferFree(IntPtr buffer);
    [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern bool LookupAccountName(string system,string name,byte[] sid,ref uint sidLength,StringBuilder domain,ref uint domainLength,out int use);
    static string LocalGroupSid(string name) {
        uint sidLength=0,domainLength=0;int use;
        bool first=LookupAccountName(Environment.MachineName,name,null,ref sidLength,null,ref domainLength,out use);
        int error=Marshal.GetLastWin32Error();if(first||error!=122)throw new Win32Exception(error);
        if(sidLength==0||sidLength>256||domainLength>1024)throw new InvalidOperationException("Invalid local group identity size");
        var sid=new byte[sidLength];var domain=new StringBuilder((int)domainLength);
        if(!LookupAccountName(Environment.MachineName,name,sid,ref sidLength,domain,ref domainLength,out use))throw new Win32Exception(Marshal.GetLastWin32Error());
        if(use!=4&&use!=2&&use!=5)throw new InvalidOperationException("Local group identity resolved to another object type");
        return new SecurityIdentifier(sid,0).Value;
    }
    static object[] LocalGroupMembers(string name) {
        var members=new List<string>();UIntPtr resume=UIntPtr.Zero;int pages=0;
        for(;;) {
            if(++pages>10000)throw new InvalidOperationException("Local group member page budget exceeded");
            IntPtr buffer=IntPtr.Zero;uint read,total;UIntPtr previous=resume;
            uint status=NetLocalGroupGetMembers(null,name,0,out buffer,65536,out read,out total,ref resume);
            try {
                if(status!=0&&status!=234)throw new Win32Exception((int)status);
                if(read>50000||members.Count+read>50000||(read>0&&buffer==IntPtr.Zero))throw new InvalidOperationException("Local group member budget exceeded");
                for(uint index=0;index<read;index++){IntPtr sid=Marshal.ReadIntPtr(buffer,checked((int)index*IntPtr.Size));if(sid==IntPtr.Zero)throw new InvalidOperationException("Local group member has no SID");members.Add(new SecurityIdentifier(sid).Value);}
                if(status==0)break;if(read==0||resume==previous)throw new InvalidOperationException("Local group member enumeration did not advance");
            }finally{if(buffer!=IntPtr.Zero)NetApiBufferFree(buffer);}
        }
        members.Sort(StringComparer.Ordinal);return members.ConvertAll<object>(sid=>sid).ToArray();
    }
    static List<object> LocalGroups() {
        RequireStateIntegrity();var groups=new List<object>();UIntPtr resume=UIntPtr.Zero;int pages=0;int size=Marshal.SizeOf(typeof(LocalGroupInfo));
        for(;;) {
            if(++pages>10000)throw new InvalidOperationException("Local group page budget exceeded");
            IntPtr buffer=IntPtr.Zero;uint read,total;UIntPtr previous=resume;
            uint status=NetLocalGroupEnum(null,1,out buffer,65536,out read,out total,ref resume);
            try {
                if(status!=0&&status!=234)throw new Win32Exception((int)status);
                if(read>50000||groups.Count+read>50000||(read>0&&buffer==IntPtr.Zero))throw new InvalidOperationException("Local group budget exceeded");
                for(uint index=0;index<read;index++) {
                    var row=(LocalGroupInfo)Marshal.PtrToStructure(IntPtr.Add(buffer,checked((int)index*size)),typeof(LocalGroupInfo));string name=Marshal.PtrToStringUni(row.name);
                    if(String.IsNullOrWhiteSpace(name))throw new InvalidOperationException("Local group has no name");
                    groups.Add(Item(LocalGroupSid(name),D("name",name,"description",Marshal.PtrToStringUni(row.description),"members",LocalGroupMembers(name))));
                }
                if(status==0)break;if(read==0||resume==previous)throw new InvalidOperationException("Local group enumeration did not advance");
            }finally{if(buffer!=IntPtr.Zero)NetApiBufferFree(buffer);}
        }
        return groups;
    }
    static void CollectLocalGroups(string unit) {
        if(unit=="base"||unit=="pcscope-powershell-adapter.exe")EmitBase("pcscope.identity.groups",LocalGroups);
    }
}
