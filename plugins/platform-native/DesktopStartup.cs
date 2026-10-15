using System;
using System.IO;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using Microsoft.Win32;

partial class Platform {
    [DllImport("kernel32.dll")]static extern ushort SetThreadUILanguage(ushort language);
    static bool ItemEnabled(string name){var disabled=Get(Get(Request,"selection"),"disabled_items") as IEnumerable;if(disabled!=null)foreach(object item in disabled)if(Text(item)==name)return false;return true;}
    static string ShellText(object value){if(value==null)return "";if(value is IEnumerable&&!(value is string))return String.Join(" ",((IEnumerable)value).Cast<object>().Select(Text));return Text(value);}
    static List<object> StartupApproved(RegistryKey root,string path,int depth) {
        var rows=new List<object>();using(var key=root.OpenSubKey(path,false)){
            if(key==null)return rows;var values=new Dictionary<string,object>();foreach(string name in key.GetValueNames())values[name]=D("kind",key.GetValueKind(name).ToString(),"value",EvidenceValue(name,key.GetValue(name,null,RegistryValueOptions.DoNotExpandEnvironmentNames)));
            rows.Add(Item(path.ToLowerInvariant(),D("registry_path",path,"values",values,"basis","registration only; not proof of loading or invocation","collection",D("policy","evidence-v2","remaining_depth",depth,"unvisited_subkeys",depth==0?key.SubKeyCount:0))));
            if(depth>0)foreach(string child in key.GetSubKeyNames())rows.AddRange(StartupApproved(root,path+"\\"+child,depth-1));
        }return rows;
    }
    static List<object> LogonStartup() {
        RequireStateIntegrity();var rows=new List<object>();
        if(ItemEnabled("pcscope.startup.logon.registry"))foreach(var hive in new[]{RegistryHive.LocalMachine,RegistryHive.CurrentUser})foreach(var view in new[]{RegistryView.Registry64,RegistryView.Registry32})using(var root=RegistryKey.OpenBaseKey(hive,view)){
            foreach(string suffix in new[]{"Run","RunOnce"}){
                string path="Software\\Microsoft\\Windows\\CurrentVersion\\"+suffix;using(var key=root.OpenSubKey(path,false)){if(key==null)continue;foreach(string name in key.GetValueNames())rows.Add(Item(hive+"/"+view+"/"+suffix+"/"+name,D("hive",hive.ToString(),"view",view.ToString(),"registry_path",path,"name",name,"command",EvidenceText(ShellText(key.GetValue(name,null,RegistryValueOptions.DoNotExpandEnvironmentNames))),"command_collected",true)));}
            }
            foreach(var item in StartupApproved(root,"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved",1))rows.Add(Item(hive+"/"+view+"/"+Get(item,"id"),Get(item,"data")));
        }
        if(ItemEnabled("pcscope.startup.logon.folders"))foreach(var folder in new[]{Environment.SpecialFolder.Startup,Environment.SpecialFolder.CommonStartup}){
            string path=Environment.GetFolderPath(folder);if(String.IsNullOrEmpty(path)||!Directory.Exists(path))continue;if(!path.StartsWith("\\\\",StringComparison.Ordinal))ValidatePath(path);
            foreach(var file in new DirectoryInfo(path).EnumerateFiles()){
                if(rows.Count>=100000)throw new Exception("Startup inventory exceeds source budget");if(!file.FullName.StartsWith("\\\\",StringComparison.Ordinal))ValidatePath(file.FullName);
                long size=file.Length;DateTime modified=file.LastWriteTimeUtc;string hash;using(var stream=File.OpenRead(file.FullName))using(var sha=SHA256.Create())hash=BitConverter.ToString(sha.ComputeHash(stream)).Replace("-","").ToLowerInvariant();
                file.Refresh();if(file.Length!=size||file.LastWriteTimeUtc!=modified)throw new Exception("Startup file changed during hashing; incomplete source discarded");
                rows.Add(Item(file.FullName.ToLowerInvariant(),D("path",file.FullName,"length",size,"last_write_utc",modified.ToString("o"),"sha256",hash,"shortcut_target_collected",false)));
            }
        }
        return rows;
    }
    static List<object> Shortcuts() {
        RequireStateIntegrity();SetThreadUILanguage(0x0409);System.Threading.Thread.CurrentThread.CurrentUICulture=System.Globalization.CultureInfo.GetCultureInfo("en-US");var rows=new List<object>();dynamic shell=Activator.CreateInstance(Type.GetTypeFromProgID("WScript.Shell",true));
        try{
            foreach(var folder in new[]{Environment.SpecialFolder.DesktopDirectory,Environment.SpecialFolder.CommonDesktopDirectory,Environment.SpecialFolder.StartMenu,Environment.SpecialFolder.CommonStartMenu}){
                string path=Environment.GetFolderPath(folder);if(String.IsNullOrEmpty(path))continue;if(path.StartsWith("\\\\",StringComparison.Ordinal))throw new Exception("Network shell folder not followed");ValidatePath(path);
                var queue=new Stack<DirectoryInfo>();queue.Push(new DirectoryInfo(path));int count=0;
                while(queue.Count>0)foreach(var entry in queue.Pop().EnumerateFileSystemInfos()){
                    ValidatePath(entry.FullName);if((entry.Attributes&FileAttributes.ReparsePoint)!=0){rows.Add(Item(entry.FullName.ToLowerInvariant(),D("path",entry.FullName,"attributes",entry.Attributes.ToString(),"collection",D("reparse_target_followed",false,"reason","reparse_target_outside_scan_policy"))));continue;}
                    if(entry is DirectoryInfo){queue.Push((DirectoryInfo)entry);continue;}if(!entry.Extension.Equals(".lnk",StringComparison.OrdinalIgnoreCase))continue;
                    if(++count>10000||rows.Count>=100000)throw new Exception("Shortcut inventory exceeds source budget");
                    dynamic link=shell.CreateShortcut(entry.FullName);try{rows.Add(Item(entry.FullName.ToLowerInvariant(),D("path",entry.FullName,"target",ShortcutProperty(link,"TargetPath",entry.FullName),"working_directory",ShortcutProperty(link,"WorkingDirectory",entry.FullName),"arguments",EvidenceText(Text(ShortcutProperty(link,"Arguments",entry.FullName))),"description",ShortcutProperty(link,"Description",entry.FullName),"window_style",ShortcutProperty(link,"WindowStyle",entry.FullName),"icon",ShortcutProperty(link,"IconLocation",entry.FullName),"hotkey",ShortcutProperty(link,"Hotkey",entry.FullName),"basis","existing shortcut metadata; target never executed")));}finally{Marshal.ReleaseComObject(link);}
                }
            }
        }finally{Marshal.ReleaseComObject(shell);}return rows;
    }
    static object ShortcutProperty(object link,string name,string path){try{return link.GetType().InvokeMember(name,System.Reflection.BindingFlags.GetProperty,null,link,null,System.Globalization.CultureInfo.GetCultureInfo("en-US"));}catch(Exception error){var failure=error.InnerException??error;if(failure is COMException&&failure.HResult==unchecked((int)0x80004005))return null;throw new Exception("Shortcut property read failed ["+name+"; "+path+"]: "+failure.Message,failure);}}
    static void CollectDesktopStartup(string unit){if(unit=="desktop")Emit("pcscope.desktop.shortcuts",Shortcuts);}
}
