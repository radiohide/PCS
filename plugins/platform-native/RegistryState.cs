using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.Win32;

partial class Platform {
    static List<object> Configuration(string hive,string path,int depth=0) {
        RequireStateIntegrity();var rows=new List<object>();
        foreach(var view in new[]{RegistryView.Registry64,RegistryView.Registry32})using(var root=RegistryKey.OpenBaseKey((RegistryHive)Enum.Parse(typeof(RegistryHive),hive),view))ConfigurationWalk(rows,root,hive,view.ToString(),path,depth);
        return rows;
    }
    static void ConfigurationWalk(List<object> rows,RegistryKey root,string hive,string view,string path,int depth) {
        using(var key=root.OpenSubKey(path,false)) {
            if(key==null)return;if(rows.Count>=50000)throw new Exception("Registry enumeration exceeds source budget; incomplete result discarded");
            var values=new Dictionary<string,object>();
            foreach(string name in key.GetValueNames().OrderBy(value=>value,StringComparer.Ordinal))values[name]=D("kind",key.GetValueKind(name).ToString(),"value",EvidenceValue(name,key.GetValue(name,null,RegistryValueOptions.DoNotExpandEnvironmentNames)));
            rows.Add(Item(hive+"/"+view+"/"+path,D("path",path,"hive",hive,"view",view,"values",values,"basis","configuration snapshot","collection",D("policy","evidence-v2","scope","configured_registry_paths_and_depth_only","remaining_depth",depth,"unvisited_subkeys",depth==0?key.SubKeyCount:0))));
            if(depth>0)foreach(string child in key.GetSubKeyNames().OrderBy(value=>value,StringComparer.Ordinal))ConfigurationWalk(rows,root,hive,view,path+"\\"+child,depth-1);
        }
    }
    static List<object> Configurations(params object[] paths) {
        var rows=new List<object>();for(int i=0;i<paths.Length;i+=3){rows.AddRange(Configuration((string)paths[i],(string)paths[i+1],(int)paths[i+2]));if(rows.Count>100000)throw new Exception("Combined registry source exceeds budget");}return rows;
    }
    static void CollectRegistryState(string unit) {
        switch(unit) {
            case "applications":
                Emit("pcscope.apps.browser-policies",()=>{var rows=new List<object>();foreach(string hive in new[]{"LocalMachine","CurrentUser"})foreach(string path in new[]{"Software\\Policies\\Google\\Chrome","Software\\Policies\\Microsoft\\Edge","Software\\Policies\\Mozilla\\Firefox"})rows.AddRange(Configuration(hive,path,2));return rows;});
                Emit("pcscope.apps.onedrive-registration",()=>Configurations("CurrentUser","Software\\Microsoft\\OneDrive\\Accounts",1,"CurrentUser","Software\\SyncEngines\\Providers\\OneDrive",1,"LocalMachine","Software\\Policies\\Microsoft\\OneDrive",0));break;
            case "desktop":
                Emit("pcscope.desktop.input",()=>Configurations("CurrentUser","Keyboard Layout",2,"LocalMachine","SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts",1,"LocalMachine","SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e96b-e325-11ce-bfc1-08002be10318}",1));
                Emit("pcscope.desktop.clipboard",()=>Configurations("CurrentUser","Software\\Microsoft\\Clipboard",0,"LocalMachine","Software\\Policies\\Microsoft\\Windows\\System",0));
                Emit("pcscope.desktop.shell-settings",()=>Configurations("CurrentUser","Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced",0,"CurrentUser","Control Panel\\Desktop",0,"CurrentUser","Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders",0));break;
            case "startup":
                Emit("pcscope.startup.service-configuration",()=>Configuration("LocalMachine","SYSTEM\\CurrentControlSet\\Services",2));
                Emit("pcscope.startup.session-hooks",()=>Configurations("LocalMachine","SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon",0,"LocalMachine","SYSTEM\\CurrentControlSet\\Control\\Session Manager",0,"LocalMachine","Software\\Microsoft\\Active Setup\\Installed Components",1,"LocalMachine","Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer\\Run",0,"CurrentUser","Software\\Microsoft\\Active Setup\\Installed Components",1,"CurrentUser","Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer\\Run",0));
                Emit("pcscope.startup.advanced-extensions",()=>{var rows=new List<object>();foreach(string path in new[]{"SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Image File Execution Options","SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SilentProcessExit","SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Windows","SYSTEM\\CurrentControlSet\\Control\\Session Manager\\AppCertDlls"})rows.AddRange(Configuration("LocalMachine",path,2));return rows;});
                Emit("pcscope.environment.configuration",()=>Configurations("LocalMachine","SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",0,"CurrentUser","Environment",0,"CurrentUser","Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts",2,"CurrentUser","Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations",2));break;
            case "network":Emit("pcscope.network.winrm-configuration",()=>Configuration("LocalMachine","SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WSMAN",3));break;
            case "security":Emit("pcscope.policy.configuration",()=>Configurations("LocalMachine","SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies",3,"CurrentUser","SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies",3,"LocalMachine","SOFTWARE\\Policies\\Microsoft\\Windows\\Safer",3));break;
            case "system":
                Emit("pcscope.system.clock",()=>{var rows=Configuration("LocalMachine","SYSTEM\\CurrentControlSet\\Services\\W32Time",3);rows.Add(Item("timezone",D("id",TimeZoneInfo.Local.Id,"basis","observed configuration")));return rows;});
                Emit("pcscope.system.reboot-pending",()=>Configurations("LocalMachine","SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Component Based Servicing\\RebootPending",0,"LocalMachine","SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WindowsUpdate\\Auto Update\\RebootRequired",0,"LocalMachine","SYSTEM\\CurrentControlSet\\Control\\Session Manager",0));
                Emit("pcscope.software.user-inventory",()=>Configuration("CurrentUser","Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",1));break;
            case "virtualization":Emit("pcscope.virtualization.wsl-registration",()=>Configuration("CurrentUser","Software\\Microsoft\\Windows\\CurrentVersion\\Lxss",1));break;
        }
    }
}
