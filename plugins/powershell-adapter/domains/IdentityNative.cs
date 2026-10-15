using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;

public static class PCScopeIdentity {
    [StructLayout(LayoutKind.Sequential)] struct Session { public int Id; public IntPtr Station; public int State; }
    public sealed class SessionRow { public int Id; public string Station; public int State; public string User; public string Domain; public int QueryError; }
    [DllImport("wtsapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool WTSEnumerateSessionsW(IntPtr server,int reserved,int version,out IntPtr buffer,out int count);
    [DllImport("wtsapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool WTSQuerySessionInformationW(IntPtr server,int session,int info,out IntPtr buffer,out int bytes);
    [DllImport("wtsapi32.dll")] static extern void WTSFreeMemory(IntPtr memory);
    static string SessionText(int id,int kind,ref int error) { IntPtr p;int size;if(!WTSQuerySessionInformationW(IntPtr.Zero,id,kind,out p,out size)){error=Marshal.GetLastWin32Error();return null;}try{return Marshal.PtrToStringUni(p);}finally{WTSFreeMemory(p);} }
    public static SessionRow[] Sessions() {
        IntPtr buffer;int count;if(!WTSEnumerateSessionsW(IntPtr.Zero,0,1,out buffer,out count))throw new Win32Exception();
        try{var rows=new List<SessionRow>();int size=Marshal.SizeOf(typeof(Session));for(int i=0;i<count;i++){var s=(Session)Marshal.PtrToStructure(IntPtr.Add(buffer,i*size),typeof(Session));int error=0;string user=SessionText(s.Id,5,ref error);string domain=SessionText(s.Id,7,ref error);rows.Add(new SessionRow{Id=s.Id,Station=Marshal.PtrToStringUni(s.Station),State=s.State,User=user,Domain=domain,QueryError=error});}return rows.ToArray();}finally{WTSFreeMemory(buffer);}
    }
    [StructLayout(LayoutKind.Sequential)] struct Attributes {public int Length;public IntPtr Root;public IntPtr ObjectName;public uint Flags;public IntPtr Security;public IntPtr Quality;}
    [StructLayout(LayoutKind.Sequential)] struct LsaString {public ushort Length;public ushort MaximumLength;public IntPtr Buffer;}
    public sealed class RightsRow {public string Sid;public string[] Rights;}
    [DllImport("advapi32.dll")] static extern uint LsaOpenPolicy(IntPtr system,ref Attributes attributes,uint access,out IntPtr policy);
    [DllImport("advapi32.dll")] static extern uint LsaEnumerateAccountsWithUserRight(IntPtr policy,IntPtr right,out IntPtr buffer,out uint count);
    [DllImport("advapi32.dll")] static extern uint LsaEnumerateAccountRights(IntPtr policy,IntPtr sid,out IntPtr rights,out uint count);
    [DllImport("advapi32.dll")] static extern uint LsaNtStatusToWinError(uint status);
    [DllImport("advapi32.dll")] static extern uint LsaFreeMemory(IntPtr memory);
    [DllImport("advapi32.dll")] static extern uint LsaClose(IntPtr policy);
    static void Check(uint status){if(status!=0)throw new Win32Exception((int)LsaNtStatusToWinError(status));}
    public static RightsRow[] Rights(){
        Attributes a=new Attributes();a.Length=Marshal.SizeOf(typeof(Attributes));IntPtr policy;Check(LsaOpenPolicy(IntPtr.Zero,ref a,0x801,out policy));
        try{IntPtr buffer;uint count;uint status=LsaEnumerateAccountsWithUserRight(policy,IntPtr.Zero,out buffer,out count);if(status==0x8000001a)return new RightsRow[0];Check(status);
            try{var rows=new List<RightsRow>();for(int i=0;i<count;i++){IntPtr sid=Marshal.ReadIntPtr(buffer,i*IntPtr.Size);IntPtr rights;uint n;Check(LsaEnumerateAccountRights(policy,sid,out rights,out n));try{var values=new List<string>();int size=Marshal.SizeOf(typeof(LsaString));for(int j=0;j<n;j++){var s=(LsaString)Marshal.PtrToStructure(IntPtr.Add(rights,j*size),typeof(LsaString));values.Add(Marshal.PtrToStringUni(s.Buffer,s.Length/2));}values.Sort(StringComparer.Ordinal);rows.Add(new RightsRow{Sid=new SecurityIdentifier(sid).Value,Rights=values.ToArray()});}finally{LsaFreeMemory(rights);}}return rows.ToArray();}finally{LsaFreeMemory(buffer);}
        }finally{LsaClose(policy);}
    }
}
