using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Diagnostics;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Cryptography.X509Certificates;
using System.Security.Principal;

partial class Platform {
    [StructLayout(LayoutKind.Sequential)] struct ModuleInfo {public IntPtr address;public uint size;public IntPtr entry;}
    [DllImport("kernel32.dll",SetLastError=true)]static extern IntPtr OpenProcess(uint access,bool inherit,uint pid);
    [DllImport("kernel32.dll",SetLastError=true)]static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll",SetLastError=true)]static extern bool GetProcessTimes(IntPtr handle,out long created,out long exited,out long kernel,out long user);
    [DllImport("advapi32.dll",SetLastError=true)]static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
    [DllImport("kernel32.dll",SetLastError=true)]static extern bool ProcessIdToSessionId(uint pid,out uint session);
    [DllImport("kernel32.dll",SetLastError=true)]static extern bool K32EnumProcessModulesEx(IntPtr process,[Out]IntPtr[] modules,uint bytes,out uint needed,uint filter);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern uint K32GetModuleFileNameExW(IntPtr process,IntPtr module,StringBuilder path,uint length);
    [DllImport("kernel32.dll",SetLastError=true)]static extern bool K32GetModuleInformation(IntPtr process,IntPtr module,out ModuleInfo info,uint size);
    static string[] ModuleNames() {
        var names=new List<string>();var value=Get(Request,"module_process_names")??Get(Options(),"module_process_names");
        if(value is string)throw new Exception("Module process names must be an array");
        var configured=value as System.Collections.IEnumerable;
        if(value!=null&&configured==null)throw new Exception("Module process names must be an array");
        if(configured!=null)foreach(object entry in configured){string name=Text(entry);if(name.EndsWith(".exe",StringComparison.OrdinalIgnoreCase))name=name.Substring(0,name.Length-4);if(!System.Text.RegularExpressions.Regex.IsMatch(name,@"^[A-Za-z0-9_.-]{1,100}$"))throw new Exception("Only exact executable base names supported");names.Add(name.ToLowerInvariant());}
        names=names.Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(x=>x,StringComparer.Ordinal).ToList();if(names.Count>32)throw new Exception("Module process-name limit exceeded");return names.ToArray();
    }
    static IntPtr[] ModuleHandles(IntPtr process) {
        var buffer=new IntPtr[4096];uint needed;
        if(!K32EnumProcessModulesEx(process,buffer,(uint)(buffer.Length*IntPtr.Size),out needed,3))throw new Win32Exception(Marshal.GetLastWin32Error());
        if(needed>buffer.Length*IntPtr.Size||needed%IntPtr.Size!=0)throw new Exception("Module enumeration exceeds bounded buffer");
        return buffer.Take((int)needed/IntPtr.Size).ToArray();
    }
    static List<object> ModuleTargets() {
        var statuses=new List<object>();var names=ModuleNames();string sid=UserSid();uint session;
        if(!ProcessIdToSessionId((uint)Process.GetCurrentProcess().Id,out session))throw new Win32Exception(Marshal.GetLastWin32Error());
        int count=0;
        foreach(string name in names) {
            var instances=new List<object>();int skipped=0;bool discoveryComplete=true;
            Process[] processes;
            try{processes=Process.GetProcessesByName(name);}catch(Exception error){statuses.Add(Item(name,D("target",name,"status","discovery_failed","error",error.Message,"error_code",error.HResult,"observed_at",Now(),"session_id",session,"user_sid",sid)));continue;}
            foreach(var process in processes)using(process) {
                IntPtr handle=IntPtr.Zero;string context=Hash("module-target/"+name+"/pid:"+process.Id);string started=null;
                try {
                    if(process.SessionId!=session){skipped++;continue;}
                    handle=OpenProcess(0x1000,false,(uint)process.Id);if(handle==IntPtr.Zero)throw new Win32Exception(Marshal.GetLastWin32Error());
                    IntPtr token;if(!OpenProcessToken(handle,8,out token))throw new Win32Exception(Marshal.GetLastWin32Error());
                    string owner;try{using(var identity=new WindowsIdentity(token))owner=identity.User.Value;}finally{CloseHandle(token);}
                    if(owner!=sid){skipped++;continue;}
                    long created,exited,kernel,user;if(!GetProcessTimes(handle,out created,out exited,out kernel,out user))throw new Win32Exception(Marshal.GetLastWin32Error());
                    started=DateTime.FromFileTimeUtc(created).ToString("o");context=Hash("module-process/"+name+"/"+process.Id+"/"+started);
                    if(++count>32)throw new Exception("Selected process instance limit exceeded; coverage partial");
                    CloseHandle(handle);handle=OpenProcess(0x410,false,(uint)process.Id);if(handle==IntPtr.Zero)throw new Win32Exception(Marshal.GetLastWin32Error());
                    long reopened;if(!GetProcessTimes(handle,out reopened,out exited,out kernel,out user)||reopened!=created||exited!=0)throw new Exception("Target instance changed or exited before module enumeration");
                    var executable=new StringBuilder(32768);uint executableLength=K32GetModuleFileNameExW(handle,IntPtr.Zero,executable,(uint)executable.Capacity);
                    if(executableLength==0||executableLength>=executable.Capacity-1||!String.Equals(Path.GetFileNameWithoutExtension(executable.ToString()),name,StringComparison.OrdinalIgnoreCase))throw new Exception("Target executable identity could not be verified");
                    var handles=ModuleHandles(handle);if(handles.Length==0)throw new Exception("No module material returned for running target; complete snapshot withheld");
                    var material=new List<Dictionary<string,object>>();
                foreach(IntPtr module in handles) {
                    var path=new StringBuilder(32768);uint length=K32GetModuleFileNameExW(handle,module,path,(uint)path.Capacity);
                    if(length==0)throw new Win32Exception(Marshal.GetLastWin32Error());if(length>=path.Capacity-1)throw new Exception("Module path truncated");
                    ModuleInfo info;if(!K32GetModuleInformation(handle,module,out info,(uint)Marshal.SizeOf(typeof(ModuleInfo))))throw new Win32Exception(Marshal.GetLastWin32Error());
                    material.Add(D("pid",process.Id,"process_start",started,"process_name",name,"session_id",session,"user_sid",sid,"module_path",path.ToString(),"size",info.size,"module_base",info.address.ToInt64().ToString("x16"),"observed_at",Now(),"signature_status","pending","module_origin","unknown","basis","native selected process module snapshot; disk binding is observation-time material, not proof of loaded memory bytes; transient/manual mappings and data-file loads may be missed"));
                }
                if(!handles.SequenceEqual(ModuleHandles(handle)))throw new Exception("Module list changed during enumeration; complete snapshot withheld");
                if(!GetProcessTimes(handle,out created,out exited,out kernel,out user)||exited!=0)throw new Exception("Selected process exited during collection; complete snapshot withheld");
                // No file hashing or trust API here: a slow disk file must not erase module evidence.
                var rows=material.Select(data=>Item(process.Id+"/"+started+"/"+Text(data["module_path"]).ToLowerInvariant(),data)).ToList();
                bool emitted=Emit("pcscope.execution.modules",()=>rows,context);if(!emitted)discoveryComplete=false;
                instances.Add(D("pid",process.Id,"process_start",started,"process_path",executable.ToString(),"status",emitted?"observed":"snapshot_unavailable","module_count",rows.Count,"scope_context",context));
                }catch(Exception error){discoveryComplete=false;Gap("pcscope.execution.modules",context,error);instances.Add(D("pid",process.Id,"process_start",started,"status","unavailable","error",error.Message,"error_code",error is Win32Exception?((Win32Exception)error).NativeErrorCode:error.HResult,"scope_context",context));}
                finally{if(handle!=IntPtr.Zero)CloseHandle(handle);}
            }
            statuses.Add(Item(name,D("target",name,"status",!discoveryComplete?"partial":instances.Count==0?"not_observed":"observed","instances",instances,"skipped_other_scope",skipped,"session_id",session,"user_sid",sid,"scope_policy","current_user_current_session","observed_at",Now(),"basis","successful module snapshots are per process instance; no target does not prove crash or unload")));
        }
        return statuses;
    }

    static void ModuleFiles() {
        var files=Paths("files");if(files.Length>4)throw new Exception("Module file check batch exceeds 4 files");
        foreach(string file in files) {
            string path=file;
            Emit("pcscope.execution.module-file-state",()=>new List<object>{Item(path.ToLowerInvariant(),ModuleFileState(path))},Hash("module-file/"+path.ToLowerInvariant()));
        }
    }
    static Dictionary<string,object> ModuleFileState(string path) {
        var result=D("path",path,"checked_at",Now(),"status","unavailable","exists",null,"basis","current disk file only; memory bytes and next-load success unknown");
        try {
            path=ValidatePath(path);result["path"]=path;
            FileAttributes attributes;
            try{attributes=File.GetAttributes(path);}catch(FileNotFoundException){result["exists"]=false;result["status"]="missing";return result;}catch(DirectoryNotFoundException){result["exists"]=false;result["status"]="missing";return result;}
            if((attributes&(FileAttributes.Directory|FileAttributes.ReparsePoint))!=0)throw new Exception("Module file must be an ordinary file; reparse target not followed");
            string parent=Path.GetDirectoryName(path);
            while(parent!=null){if((File.GetAttributes(parent)&FileAttributes.ReparsePoint)!=0)throw new Exception("Module parent is a reparse point; target not followed");parent=Path.GetDirectoryName(parent);}
            result["exists"]=true;var before=new FileInfo(path);long size=before.Length;DateTime modified=before.LastWriteTimeUtc;
            var identity=PCScopeMetadata.FileIdentity(path);result["volume_identity"]=identity[0];result["file_id"]=identity[1];result["size"]=size;result["last_write_utc"]=modified.ToString("o");result["attributes"]=attributes.ToString();
            var acl=before.GetAccessControl(System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner);
            result["owner"]=Owner(acl);result["dacl_sddl"]=acl.GetSecurityDescriptorSddlForm(System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner);
            var version=FileVersionInfo.GetVersionInfo(path);result["version"]=version.FileVersion;result["company"]=version.CompanyName;result["product"]=version.ProductName;
            bool integrity=Get(Request,"integrity") is bool&&(bool)Get(Request,"integrity");
            result["hash_status"]=integrity?"pending":"not_requested";result["signature_status"]=integrity?"pending":"not_requested";
            if(integrity && size>67108864){result["hash_status"]="skipped_size_limit";result["signature_status"]="skipped_size_limit";}
            else if(integrity) {
                using(var stream=new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.Read|FileShare.Delete))using(var sha=System.Security.Cryptography.SHA256.Create()) {
                    if(stream.Length!=size)throw new Exception("Module file changed before hashing");
                    try{BasicPeHeaders(stream,result);result["pe_status"]="basic_headers_valid";}catch(InvalidDataException error){result["pe_status"]="basic_headers_invalid";result["pe_error"]=error.Message;}catch(EndOfStreamException error){result["pe_status"]="basic_headers_invalid";result["pe_error"]=error.Message;}
                    stream.Position=0;
                    result["sha256"]=BitConverter.ToString(sha.ComputeHash(stream)).Replace("-","").ToLowerInvariant();result["hash_status"]="collected";result["hash_checked_at"]=Now();
                }
                foreach(var entry in ModuleSignature(path))result[entry.Key]=entry.Value;
                result["signature_checked_at"]=Now();
            }
            before.Refresh();var after=PCScopeMetadata.FileIdentity(path);
            if(!before.Exists||before.Length!=size||before.LastWriteTimeUtc!=modified||!identity.SequenceEqual(after)||File.GetAttributes(path)!=attributes)throw new Exception("Module file changed during check; stable result withheld");
            var afterAcl=before.GetAccessControl(System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner);
            if(afterAcl.GetSecurityDescriptorSddlForm(System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner)!=(string)result["dacl_sddl"])throw new Exception("Module security changed during check; stable result withheld");
            result["status"]="stable";return result;
        }catch(Exception error){result["status"]="unavailable";result["error"]=error.Message;result["error_code"]=error is Win32Exception?((Win32Exception)error).NativeErrorCode:error.HResult;foreach(string key in new[]{"sha256","hash_checked_at","signature_checked_at","publisher","signer_subject","signer_thumbprint","signature_code"})result.Remove(key);result["module_origin"]="unknown";result["hash_status"]="unavailable";result["signature_status"]="unavailable";result["pe_status"]="unavailable";return result;}
    }
    static void BasicPeHeaders(FileStream stream,Dictionary<string,object> result) {
        // Only bounded on-disk headers/ranges are checked. No image loading or DLL initialization.
        var reader=new BinaryReader(stream,Encoding.UTF8,true);
        if(stream.Length<64||reader.ReadUInt16()!=0x5a4d)throw new InvalidDataException("Missing DOS header");
        stream.Position=0x3c;uint pe=reader.ReadUInt32();
        if(pe<64||(long)pe>stream.Length-24)throw new InvalidDataException("PE header outside file");
        stream.Position=pe;if(reader.ReadUInt32()!=0x00004550)throw new InvalidDataException("Missing PE signature");
        ushort machine=reader.ReadUInt16(),sections=reader.ReadUInt16();result["pe_machine"]="0x"+machine.ToString("X4");
        stream.Position=pe+20;ushort optional=reader.ReadUInt16();
        if(sections==0||sections>96||optional<2||(long)pe+24+optional+(long)sections*40>stream.Length)throw new InvalidDataException("Invalid section/header table range");
        stream.Position=pe+24;ushort magic=reader.ReadUInt16();
        if((magic!=0x10b&&magic!=0x20b)||(magic==0x10b&&optional<96)||(magic==0x20b&&optional<112))throw new InvalidDataException("Invalid PE optional header");
        result["pe_format"]=magic==0x20b?"PE32+":"PE32";result["pe_sections"]=sections;
        for(int i=0;i<sections;i++){stream.Position=(long)pe+24+optional+i*40+16;uint size=reader.ReadUInt32(),position=reader.ReadUInt32();if(size!=0&&(position==0||(long)position+size>stream.Length))throw new InvalidDataException("Section data extends outside file");}
        result["pe_basis"]="bounded basic PE headers and raw section ranges only; not full loader validity or initialization success";
    }

    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]struct TrustFile {public uint size;[MarshalAs(UnmanagedType.LPWStr)]public string path;public IntPtr file,subject;}
    [StructLayout(LayoutKind.Sequential)]struct TrustData {public uint size;public IntPtr callback,sip;public uint ui,revocation,choice;public IntPtr union;public uint action;public IntPtr state,url;public uint flags,context;public IntPtr settings;}
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]struct CatalogInfo {public uint size;[MarshalAs(UnmanagedType.ByValTStr,SizeConst=260)]public string path;}
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]struct TrustCatalog {public uint size,version;[MarshalAs(UnmanagedType.LPWStr)]public string path,tag,member;public IntPtr file,hash;public uint hashLength;public IntPtr catalog,admin;}
    [StructLayout(LayoutKind.Sequential)]struct SignerPrefix {public uint size,timeLow,timeHigh,count;public IntPtr chain;}
    [StructLayout(LayoutKind.Sequential)]struct CertificatePrefix {public uint size;public IntPtr certificate;}
    [DllImport("wintrust.dll",ExactSpelling=true)]static extern int WinVerifyTrust(IntPtr window,ref Guid action,ref TrustData data);
    [DllImport("wintrust.dll",ExactSpelling=true)]static extern IntPtr WTHelperProvDataFromStateData(IntPtr state);
    [DllImport("wintrust.dll",ExactSpelling=true)]static extern IntPtr WTHelperGetProvSignerFromChain(IntPtr data,uint index,bool counter,uint counterIndex);
    [DllImport("wintrust.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern bool CryptCATAdminAcquireContext2(out IntPtr admin,IntPtr subsystem,string algorithm,IntPtr policy,uint flags);
    [DllImport("wintrust.dll",SetLastError=true)]static extern bool CryptCATAdminCalcHashFromFileHandle2(IntPtr admin,IntPtr file,ref uint size,[Out]byte[] hash,uint flags);
    [DllImport("wintrust.dll",SetLastError=true)]static extern IntPtr CryptCATAdminEnumCatalogFromHash(IntPtr admin,byte[] hash,uint size,uint flags,ref IntPtr previous);
    [DllImport("wintrust.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern bool CryptCATCatalogInfoFromContext(IntPtr catalog,ref CatalogInfo info,uint flags);
    [DllImport("wintrust.dll")]static extern bool CryptCATAdminReleaseCatalogContext(IntPtr admin,IntPtr catalog,uint flags);
    [DllImport("wintrust.dll")]static extern bool CryptCATAdminReleaseContext(IntPtr admin,uint flags);
    static Dictionary<string,object> Verify<T>(T info,uint choice,string kind) {
        IntPtr buffer=Marshal.AllocHGlobal(Marshal.SizeOf(typeof(T)));Marshal.StructureToPtr(info,buffer,false);
        var data=new TrustData{size=(uint)Marshal.SizeOf(typeof(TrustData)),ui=2,choice=choice,union=buffer,action=1,flags=0x1000|0x10};Guid action=new Guid("00AAC56B-CD44-11D0-8CC2-00C04FC295EE");
        try {
            int code=WinVerifyTrust(new IntPtr(-1),ref action,ref data);string publisher=null,subject=null,thumbprint=null;
            if(data.state!=IntPtr.Zero){IntPtr provider=WTHelperProvDataFromStateData(data.state);IntPtr signer=provider==IntPtr.Zero?IntPtr.Zero:WTHelperGetProvSignerFromChain(provider,0,false,0);
                if(signer!=IntPtr.Zero){var first=(SignerPrefix)Marshal.PtrToStructure(signer,typeof(SignerPrefix));if(first.count>0&&first.chain!=IntPtr.Zero){var cert=(CertificatePrefix)Marshal.PtrToStructure(first.chain,typeof(CertificatePrefix));if(cert.certificate!=IntPtr.Zero)using(var certificate=new X509Certificate2(cert.certificate)){publisher=certificate.GetNameInfo(X509NameType.SimpleName,false);subject=certificate.Subject;thumbprint=certificate.Thumbprint;}}}}
            string status=code==0?"valid_offline":unchecked((uint)code)==0x800B0100?"not_signed":"verification_failed";
            string origin=code==0&&!String.IsNullOrEmpty(publisher)?(publisher=="Microsoft Windows"||publisher=="Microsoft Corporation"||publisher=="Microsoft Windows Publisher"?"microsoft_signed":"other_publisher_signed"):status=="not_signed"?"unsigned":"unknown";
            return D("signature_status",status,"signature_code","0x"+unchecked((uint)code).ToString("X8"),"signature_kind",kind,"publisher",publisher,"signer_subject",subject,"signer_thumbprint",thumbprint,"module_origin",origin,"signature_policy","local cache only; no UI; revocation freshness not established; current disk file only");
        }finally{data.action=2;if(data.state!=IntPtr.Zero)WinVerifyTrust(new IntPtr(-1),ref action,ref data);Marshal.DestroyStructure(buffer,typeof(T));Marshal.FreeHGlobal(buffer);}
    }
    static Dictionary<string,object> ModuleSignature(string path) {
        try {
            var before=new FileInfo(path);long size=before.Length;DateTime written=before.LastWriteTimeUtc;
            if(size>67108864)return D("signature_status","skipped_size_limit","module_origin","unknown","signature_error","Module file exceeds 64 MiB verification budget");
            var result=Verify(new TrustFile{size=(uint)Marshal.SizeOf(typeof(TrustFile)),path=path},1,"embedded");
            if(result["signature_status"].ToString()=="not_signed") {
                using(var stream=new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.Read|FileShare.Delete))foreach(string algorithm in new[]{"SHA256","SHA1"}) {
                    IntPtr admin;if(!CryptCATAdminAcquireContext2(out admin,IntPtr.Zero,algorithm,IntPtr.Zero,0))throw new Win32Exception(Marshal.GetLastWin32Error());
                    try {
                        stream.Position=0;uint length=0;if(!CryptCATAdminCalcHashFromFileHandle2(admin,stream.SafeFileHandle.DangerousGetHandle(),ref length,null,0))throw new Win32Exception(Marshal.GetLastWin32Error());
                        if(length==0||length>128)throw new Exception("Catalog hash size invalid");var hash=new byte[length];stream.Position=0;
                        if(!CryptCATAdminCalcHashFromFileHandle2(admin,stream.SafeFileHandle.DangerousGetHandle(),ref length,hash,0))throw new Win32Exception(Marshal.GetLastWin32Error());
                        IntPtr previous=IntPtr.Zero;IntPtr catalog=CryptCATAdminEnumCatalogFromHash(admin,hash,length,0,ref previous);
                        if(catalog==IntPtr.Zero){int error=Marshal.GetLastWin32Error();if(error!=0&&error!=1168)throw new Win32Exception(error);continue;}
                        try {
                            var info=new CatalogInfo{size=(uint)Marshal.SizeOf(typeof(CatalogInfo))};if(!CryptCATCatalogInfoFromContext(catalog,ref info,0))throw new Win32Exception(Marshal.GetLastWin32Error());
                            IntPtr nativeHash=Marshal.AllocHGlobal(hash.Length);try{Marshal.Copy(hash,0,nativeHash,hash.Length);result=Verify(new TrustCatalog{size=(uint)Marshal.SizeOf(typeof(TrustCatalog)),path=info.path,tag=BitConverter.ToString(hash).Replace("-",""),member=path,file=stream.SafeFileHandle.DangerousGetHandle(),hash=nativeHash,hashLength=length,admin=admin},2,"catalog");result["catalog_path"]=info.path;}finally{Marshal.FreeHGlobal(nativeHash);}
                        }finally{CryptCATAdminReleaseCatalogContext(admin,catalog,0);}
                        if(result["signature_status"].ToString()=="valid_offline")break;
                    }finally{CryptCATAdminReleaseContext(admin,0);}
                }
            }
            before.Refresh();if(!before.Exists||before.Length!=size||before.LastWriteTimeUtc!=written)throw new Exception("Module file changed during verification");
            result["signature_file_size"]=size;result["signature_file_last_write_utc"]=written.ToString("o");return result;
        }catch(Exception error){return D("signature_status","unavailable","module_origin","unknown","signature_error",error.Message);}
    }
}
