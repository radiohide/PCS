using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Security.Principal;

partial class Platform {
    [StructLayout(LayoutKind.Sequential)] struct AccountUnicode {public ushort length,maximum;public IntPtr text;}
    [StructLayout(LayoutKind.Sequential)] struct AccountAttributes {public uint length;public IntPtr root,name;public uint attributes;public IntPtr descriptor,quality;}
    [StructLayout(LayoutKind.Sequential)] struct AccountDomainInfo {public AccountUnicode name;public IntPtr sid;}
    [StructLayout(LayoutKind.Sequential)] struct AccountRid {public uint rid;public AccountUnicode name;}
    [StructLayout(LayoutKind.Sequential)] struct AccountHours {public ushort units;public IntPtr hours;}
    // UserAccountInformation (5) contains metadata, not password hashes or private data.
    [StructLayout(LayoutKind.Sequential,Pack=4)] struct AccountInfo {
        public AccountUnicode name,fullName;public uint rid,primaryGroup;
        public AccountUnicode home,drive,script,profile,comment,workstations;
        public long lastLogon,lastLogoff;public AccountHours hours;public ushort badPasswords,logons;
        public long passwordLastSet,expires;public uint control;
    }
    [DllImport("advapi32.dll")]static extern uint LsaOpenPolicy(IntPtr system,ref AccountAttributes attributes,uint access,out IntPtr policy);
    [DllImport("advapi32.dll")]static extern uint LsaQueryInformationPolicy(IntPtr policy,int kind,out IntPtr buffer);
    [DllImport("advapi32.dll")]static extern uint LsaClose(IntPtr policy);
    [DllImport("advapi32.dll")]static extern uint LsaFreeMemory(IntPtr buffer);
    [DllImport("advapi32.dll")]static extern uint LsaNtStatusToWinError(uint status);
    [DllImport("api-ms-win-security-lsalookup-l1-1-2.dll")]static extern uint LsaLookupUserAccountType(byte[] sid,out int type);
    [DllImport("advapi32.dll",SetLastError=true)]static extern bool GetSecurityDescriptorDacl(IntPtr descriptor,out bool present,out IntPtr dacl,out bool defaulted);
    [DllImport("samlib.dll")]static extern uint SamConnect(ref AccountUnicode server,out IntPtr handle,uint access,ref AccountAttributes attributes);
    [DllImport("samlib.dll")]static extern uint SamOpenDomain(IntPtr server,uint access,byte[] sid,out IntPtr domain);
    [DllImport("samlib.dll")]static extern uint SamEnumerateUsersInDomain(IntPtr domain,ref uint context,uint filter,out IntPtr buffer,uint preferred,out uint count);
    [DllImport("samlib.dll")]static extern uint SamOpenUser(IntPtr domain,uint access,uint rid,out IntPtr user);
    [DllImport("samlib.dll")]static extern uint SamQueryInformationUser(IntPtr user,int kind,out IntPtr buffer);
    [DllImport("samlib.dll")]static extern uint SamQuerySecurityObject(IntPtr user,uint information,out IntPtr buffer);
    [DllImport("samlib.dll")]static extern uint SamFreeMemory(IntPtr buffer);
    [DllImport("samlib.dll")]static extern uint SamCloseHandle(IntPtr handle);
    static void AccountStatus(uint status){if(status!=0)throw new Win32Exception((int)LsaNtStatusToWinError(status));}
    static string AccountString(AccountUnicode value){if(value.length%2!=0||value.length>value.maximum||(value.length>0&&value.text==IntPtr.Zero))throw new InvalidOperationException("Invalid account metadata string");return value.length==0?"":Marshal.PtrToStringUni(value.text,value.length/2);}
    static byte[] AccountSidBytes(SecurityIdentifier sid){var bytes=new byte[sid.BinaryLength];sid.GetBinaryForm(bytes,0);return bytes;}
    static string AccountDate(long value){return value==0||value==Int64.MaxValue?null:DateTime.FromFileTimeUtc(value).ToString("o");}
    static string AccountSource(SecurityIdentifier sid) {
        int type;uint status;
        try{status=LsaLookupUserAccountType(AccountSidBytes(sid),out type);}catch(EntryPointNotFoundException){return "";}
        if((status&0x80000000)!=0)return "Unknown";
        switch(type){case 1:return "Local";case 2:case 3:return "ActiveDirectory";case 4:case 7:return "MicrosoftAccount";case 5:return "AzureAD";case 6:return sid.Value.StartsWith("S-1-11-",StringComparison.Ordinal)?"MicrosoftAccount":"Unknown";default:return "Unknown";}
    }
    static bool AccountMayChangePassword(IntPtr user,SecurityIdentifier sid) {
        IntPtr buffer=IntPtr.Zero;
        try {
            AccountStatus(SamQuerySecurityObject(user,4,out buffer));
            bool present,defaulted;IntPtr dacl;if(!GetSecurityDescriptorDacl(buffer,out present,out dacl,out defaulted))throw new Win32Exception(Marshal.GetLastWin32Error());
            if(!present||dacl==IntPtr.Zero)return true;
            ushort size=(ushort)Marshal.ReadInt16(dacl,2);if(size<8)throw new InvalidOperationException("Invalid account DACL");
            var bytes=new byte[size];Marshal.Copy(dacl,bytes,0,size);var acl=new RawAcl(bytes,0);var everyone=new SecurityIdentifier(WellKnownSidType.WorldSid,null);
            foreach(GenericAce entry in acl){var ace=entry as CommonAce;if(ace!=null&&ace.AceType==AceType.AccessAllowed&&(ace.SecurityIdentifier==sid||ace.SecurityIdentifier==everyone)&&(ace.AccessMask&0x40)!=0)return true;}
            return false;
        }finally{if(buffer!=IntPtr.Zero)SamFreeMemory(buffer);}
    }
    static object ReadAccount(IntPtr domain,string domainSid,uint rid) {
        IntPtr user=IntPtr.Zero,buffer=IntPtr.Zero;
        try {
            AccountStatus(SamOpenUser(domain,0x2001b,rid,out user));
            AccountStatus(SamQueryInformationUser(user,5,out buffer));if(buffer==IntPtr.Zero)throw new InvalidOperationException("Account query returned no metadata");
            var row=(AccountInfo)Marshal.PtrToStructure(buffer,typeof(AccountInfo));if(row.rid!=rid)throw new InvalidOperationException("Account identity changed while reading");
            var sid=new SecurityIdentifier(domainSid+"-"+rid.ToString(System.Globalization.CultureInfo.InvariantCulture));
            return Item(sid.Value,D("name",AccountString(row.name),"enabled",(row.control&1)==0,"description",AccountString(row.comment),"last_logon",AccountDate(row.lastLogon),"password_last_set",AccountDate(row.passwordLastSet),"account_expires",AccountDate(row.expires),"password_required",(row.control&4)==0,"user_may_change_password",AccountMayChangePassword(user,sid),"principal_source",AccountSource(sid),"created_at",null,"created_at_basis","not available from this source"));
        }finally{if(buffer!=IntPtr.Zero)SamFreeMemory(buffer);if(user!=IntPtr.Zero)SamCloseHandle(user);}
    }
    static void AccountsInDomain(IntPtr server,SecurityIdentifier sid,List<object> rows) {
        IntPtr domain=IntPtr.Zero;
        try {
            AccountStatus(SamOpenDomain(server,0x300,AccountSidBytes(sid),out domain));
            uint context=0;int pages=0,size=Marshal.SizeOf(typeof(AccountRid));
            for(;;) {
                if(++pages>10000)throw new InvalidOperationException("Account enumeration page budget exceeded");
                IntPtr buffer=IntPtr.Zero;uint count,previous=context;uint status=SamEnumerateUsersInDomain(domain,ref context,0,out buffer,65536,out count);
                try {
                    if(status!=0x105)AccountStatus(status);if(status!=0&&status!=0x105)throw new InvalidOperationException("Unexpected account enumeration status");
                    if(count>50000||rows.Count+count>50000||(count>0&&buffer==IntPtr.Zero))throw new InvalidOperationException("Account enumeration budget exceeded");
                    for(uint i=0;i<count;i++){var entry=(AccountRid)Marshal.PtrToStructure(IntPtr.Add(buffer,checked((int)i*size)),typeof(AccountRid));rows.Add(ReadAccount(domain,sid.Value,entry.rid));}
                    if(status==0)break;if(count==0||context==previous)throw new InvalidOperationException("Account enumeration did not advance");
                }finally{if(buffer!=IntPtr.Zero)SamFreeMemory(buffer);}
            }
        }finally{if(domain!=IntPtr.Zero)SamCloseHandle(domain);}
    }
    static List<object> LocalAccounts() {
        RequireStateIntegrity();AccountLayout();IntPtr policy=IntPtr.Zero,info=IntPtr.Zero,server=IntPtr.Zero;
        var attributes=new AccountAttributes{length=(uint)Marshal.SizeOf(typeof(AccountAttributes))};var name=new AccountUnicode();
        try {
            AccountStatus(LsaOpenPolicy(IntPtr.Zero,ref attributes,1,out policy));AccountStatus(LsaQueryInformationPolicy(policy,5,out info));
            if(info==IntPtr.Zero)throw new InvalidOperationException("Local account domain query returned no metadata");var domain=(AccountDomainInfo)Marshal.PtrToStructure(info,typeof(AccountDomainInfo));var sid=new SecurityIdentifier(domain.sid);
            AccountStatus(SamConnect(ref name,out server,0x20,ref attributes));var rows=new List<object>();
            AccountsInDomain(server,sid,rows);AccountsInDomain(server,new SecurityIdentifier("S-1-5-32"),rows);return rows;
        }finally{if(server!=IntPtr.Zero)SamCloseHandle(server);if(info!=IntPtr.Zero)LsaFreeMemory(info);if(policy!=IntPtr.Zero)LsaClose(policy);}
    }
    static void AccountLayout(){if(Marshal.SizeOf(typeof(AccountInfo))!=192||Marshal.OffsetOf(typeof(AccountInfo),"passwordLastSet").ToInt64()!=172||Marshal.OffsetOf(typeof(AccountInfo),"control").ToInt64()!=188||Marshal.SizeOf(typeof(AccountRid))!=24)throw new InvalidOperationException("SAM account metadata layout mismatch");}
    static void AccountContractTest() {
        AccountLayout();int size=Marshal.SizeOf(typeof(AccountInfo));IntPtr buffer=Marshal.AllocHGlobal(size*2),text=Marshal.StringToHGlobalUni("账户样本");
        try {
            long timestamp=new DateTime(2026,10,14,1,2,3,DateTimeKind.Utc).ToFileTimeUtc();
            for(int i=0;i<2;i++)Marshal.StructureToPtr(new AccountInfo{name=new AccountUnicode{length=8,maximum=10,text=text},rid=(uint)(1000+i),passwordLastSet=timestamp+i,control=(uint)(1+i)},IntPtr.Add(buffer,size*i),false);
            for(int i=0;i<2;i++){var row=(AccountInfo)Marshal.PtrToStructure(IntPtr.Add(buffer,size*i),typeof(AccountInfo));if(row.rid!=1000+i||row.passwordLastSet!=timestamp+i||row.control!=1+i||AccountString(row.name)!="账户样本")throw new Exception("SAM packed metadata array mismatch");}
            if(AccountDate(0)!=null||AccountDate(Int64.MaxValue)!=null||AccountDate(timestamp)!="2026-10-14T01:02:03.0000000Z")throw new Exception("SAM timestamp mapping mismatch");
            bool rejected=false;try{AccountString(new AccountUnicode{length=3,maximum=4,text=text});}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Malformed SAM string accepted");
            Console.WriteLine(Json.Serialize(D("packed_account_size",size,"packed_array",true,"exact_utc_timestamp",true,"sentinel_dates",true,"malformed_string_rejected",true)));
        }finally{Marshal.FreeHGlobal(text);Marshal.FreeHGlobal(buffer);}
    }
    static void CollectLocalAccounts(string unit){if(unit=="base"||unit=="pcscope-powershell-adapter.exe")EmitBase("pcscope.identity.accounts",LocalAccounts);}
}
