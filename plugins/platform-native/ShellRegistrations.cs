using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Text.RegularExpressions;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using Microsoft.Win32;

partial class Platform {
    static readonly Regex ShellGuid=new Regex("^\\{[0-9a-fA-F-]{36}\\}$");
    static readonly Dictionary<string,object> ShellFileCache=new Dictionary<string,object>(StringComparer.OrdinalIgnoreCase);
    static object ShellImplementationFile(string raw) {
        string path=Environment.ExpandEnvironmentVariables(raw.Trim().Trim('"'));
        if(!Path.IsPathRooted(path)||path.StartsWith("\\\\",StringComparison.Ordinal))return D("status","not_read","reason","relative or network path","raw_path",raw);
        object cached;if(ShellFileCache.TryGetValue(path,out cached))return cached;var data=D("path",path,"status","unknown","signature_status","not_verified");
        try {
            var file=new FileInfo(path);if(!file.Exists)data["status"]="not_found";
            else {
                data["status"]="observed";data["length"]=file.Length;data["last_write_utc"]=file.LastWriteTimeUtc.ToString("o");
                var version=System.Diagnostics.FileVersionInfo.GetVersionInfo(path);data["file_version"]=version.FileVersion;data["company"]=version.CompanyName;
                if(file.Length<=268435456){using(var stream=File.OpenRead(path))using(var sha=SHA256.Create())data["sha256"]=BitConverter.ToString(sha.ComputeHash(stream)).Replace("-","").ToLowerInvariant();}else data["hash_status"]="skipped_size_limit";
                try{using(var cert=X509Certificate.CreateFromSignedFile(path))data["embedded_signer"]=cert.Subject;}catch{data["embedded_signer"]=null;}
                file.Refresh();if(Convert.ToInt64(data["length"])!=file.Length||Text(data["last_write_utc"])!=file.LastWriteTimeUtc.ToString("o")){data["status"]="changed_during_read";data.Remove("sha256");}
            }
        }catch{data["status"]="unavailable";data["error_type"]="MethodInvocationException";}
        ShellFileCache[path]=data;return data;
    }
    static bool ShellPathEnabled(string path) {
        if(ItemEnabled("pcscope.shell.registrations.com"))return true;
        if(Regex.IsMatch(path,@"\\Classes\\CLSID\\",RegexOptions.IgnoreCase))return false;
        var checks=new Dictionary<string,string>{{"preview",@"\\PreviewHandlers"},{"overlay",@"\\ShellIconOverlayIdentifiers"},{"approval",@"\\Shell Extensions\\"},{"namespace",@"\\(Desktop|MyComputer)\\NameSpace"},{"file-association",@"\\Classes\\"}};
        foreach(var check in checks)if(!ItemEnabled("pcscope.shell.registrations."+check.Key)&&Regex.IsMatch(path,check.Value,RegexOptions.IgnoreCase))return false;return true;
    }
    static void ShellRegistryWalk(Dictionary<string,object> items,HashSet<string> clsids,RegistryKey root,string path,int depth) {
        if(!ShellPathEnabled(path))return;using(var key=root.OpenSubKey(path,false)){
            if(key==null)return;if(items.Count>=100000)throw new Exception("Shell registry source exceeds entry budget");var values=new Dictionary<string,object>();
            foreach(string name in key.GetValueNames().OrderBy(x=>x,StringComparer.CurrentCulture)){
                if(ShellGuid.IsMatch(name))clsids.Add(name.ToLowerInvariant());object value=EvidenceValue(name,key.GetValue(name,null,RegistryValueOptions.DoNotExpandEnvironmentNames));values[name]=D("kind",key.GetValueKind(name).ToString(),"value",value);
                if(value is string&&ShellGuid.IsMatch((string)value))clsids.Add(((string)value).ToLowerInvariant());
            }
            var data=D("registry_path",path,"values",values,"basis","registration only; not proof of loading or invocation","collection",D("policy","evidence-v2","remaining_depth",depth,"unvisited_subkeys",depth==0?key.SubKeyCount:0));
            if(path.EndsWith("\\InprocServer32",StringComparison.OrdinalIgnoreCase)){string server=ShellText(key.GetValue("",null,RegistryValueOptions.DoNotExpandEnvironmentNames));if(server.Length>0)data["implementation_file"]=ShellImplementationFile(server);}
            string id=path.ToLowerInvariant();items[id]=Item(id,data);
            if(depth>0)foreach(string child in key.GetSubKeyNames().OrderBy(x=>x,StringComparer.CurrentCulture))ShellRegistryWalk(items,clsids,root,path+"\\"+child,depth-1);
        }
    }
    static List<object> ShellRegistryView(RegistryHive hive,RegistryView view) {
        RequireStateIntegrity();var items=new Dictionary<string,object>(StringComparer.OrdinalIgnoreCase);var clsids=new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        using(var root=RegistryKey.OpenBaseKey(hive,view)){
            foreach(string path in new[]{"Software\\Microsoft\\Windows\\CurrentVersion\\PreviewHandlers","Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\ShellIconOverlayIdentifiers","Software\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Approved","Software\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Blocked","Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Desktop\\NameSpace","Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\MyComputer\\NameSpace"})ShellRegistryWalk(items,clsids,root,path,3);
            using(var classes=root.OpenSubKey("Software\\Classes",false))if(classes!=null){foreach(string type in classes.GetSubKeyNames().Where(x=>!new[]{"CLSID","Interface","TypeLib","AppID"}.Contains(x,StringComparer.OrdinalIgnoreCase)).OrderBy(x=>x,StringComparer.CurrentCulture)){
                foreach(string handler in new[]{"shell","shellex","DefaultIcon"})ShellRegistryWalk(items,clsids,root,"Software\\Classes\\"+type+"\\"+handler,5);
                if(type.StartsWith(".",StringComparison.Ordinal))ShellRegistryWalk(items,clsids,root,"Software\\Classes\\"+type,0);
            }ShellRegistryWalk(items,clsids,root,"Software\\Classes\\Directory\\Background\\shell",5);ShellRegistryWalk(items,clsids,root,"Software\\Classes\\Directory\\Background\\shellex",5);}
            foreach(string clsid in clsids.OrderBy(x=>x,StringComparer.CurrentCulture).ToArray())foreach(string suffix in new[]{"","\\InprocServer32","\\LocalServer32"})ShellRegistryWalk(items,clsids,root,"Software\\Classes\\CLSID\\"+clsid+suffix,0);
        }return items.Values.ToList();
    }
    static void CollectShellRegistrations() {
        const string source="pcscope.shell.registrations";if(!Enabled(source))return;SetThreadUILanguage(0x0409);System.Threading.Thread.CurrentThread.CurrentUICulture=System.Globalization.CultureInfo.GetCultureInfo("en-US");
        foreach(var hive in new[]{RegistryHive.LocalMachine,RegistryHive.CurrentUser})foreach(var view in new[]{RegistryView.Registry64,RegistryView.Registry32}){
            string prefix=hive+"/"+view;var previous=Console.Out;var encoded=new StringWriter();bool complete=true;
            try {
                var rows=ShellRegistryView(hive,view);var shards=new List<object>[16];for(int i=0;i<16;i++)shards[i]=new List<object>();foreach(var row in rows)shards[Convert.ToInt32(Hash(Text(Get(row,"id"))).Substring(0,1),16)].Add(row);
                Console.SetOut(encoded);for(int i=0;i<16;i++){int index=i;if(!EmitBase("pcscope.shell.registrations",()=>shards[index],prefix+"/shard:"+i)){complete=false;break;}}
                Console.SetOut(previous);if(complete)Console.Write(encoded.ToString());else Gap(source,prefix,new Exception("Shell registry shard encoding failed; entire view retained"));
            }catch(Exception error){Console.SetOut(previous);Gap(source,prefix,error);}finally{Console.SetOut(previous);encoded.Dispose();}
        }
    }
}
