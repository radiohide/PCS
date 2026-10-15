using System;
using System.IO;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Security.Cryptography;

partial class Platform {
    static object JsonField(object value,string name) {
        var map=value as Dictionary<string,object>;if(map==null)return null;object found;if(map.TryGetValue(name,out found))return found;
        foreach(var pair in map)if(String.Equals(pair.Key,name,StringComparison.OrdinalIgnoreCase))return pair.Value;return null;
    }
    static void CheckJsonKeys(object value,int depth=0) {
        if(depth>64)throw new Exception("Manifest exceeds JSON depth budget");var map=value as Dictionary<string,object>;
        if(map!=null){var keys=new HashSet<string>(StringComparer.OrdinalIgnoreCase);foreach(var pair in map){if(!keys.Add(pair.Key))throw new Exception("Case-ambiguous JSON keys");CheckJsonKeys(pair.Value,depth+1);}}
        else if(value is IEnumerable&&!(value is string))foreach(object entry in (IEnumerable)value)CheckJsonKeys(entry,depth+1);
    }
    static object ReadManifest(string path,long limit,bool allowReparse=false) {
        ValidatePath(path);var file=new FileInfo(path);if(file.Length>limit||(!allowReparse&&(file.Attributes&FileAttributes.ReparsePoint)!=0))throw new Exception("Manifest outside bounded read policy");
        long size=file.Length;DateTime modified=file.LastWriteTimeUtc;object value=Json.DeserializeObject(File.ReadAllText(path,Encoding.Default));CheckJsonKeys(value);
        file.Refresh();if(!file.Exists||file.Length!=size||file.LastWriteTimeUtc!=modified)throw new Exception("Manifest changed during read; incomplete source discarded");return value;
    }
    static IEnumerable<DirectoryInfo> ApplicationDirectories(string path,bool allowReparse=false,Func<DirectoryInfo,bool> filter=null) {
        ValidatePath(path);if(!Directory.Exists(path))return new DirectoryInfo[0];var root=new DirectoryInfo(path);
        return root.EnumerateDirectories().Where(directory=>filter==null||filter(directory)).Select(directory=>{if(!allowReparse&&(directory.Attributes&FileAttributes.ReparsePoint)!=0)throw new Exception("Application directory reparse point not followed");return directory;});
    }
    static object[] ManifestArray(object value) {
        if(value==null)return new object[]{null};var array=value as IEnumerable;if(array==null||value is string)return new[]{value};return array.Cast<object>().ToArray();
    }
    static void ApplicationBudget(List<object> rows){if(rows.Count>=100000)throw new Exception("Application inventory exceeds source budget");}
    static List<object> IdeExtensions() {
        RequireStateIntegrity();var rows=new List<object>();string home=Environment.GetEnvironmentVariable("USERPROFILE");
        foreach(string folder in new[]{".vscode\\extensions",".vscode-insiders\\extensions"})foreach(var directory in ApplicationDirectories(Path.Combine(home,folder),true)) {
            ApplicationBudget(rows);string manifest=Path.Combine(directory.FullName,"package.json");if(!File.Exists(manifest))continue;object data=ReadManifest(manifest,1048576,true);
            rows.Add(Item(directory.FullName,D("name",JsonField(data,"name"),"publisher",JsonField(data,"publisher"),"version",JsonField(data,"version"),"engines",JsonField(data,"engines"),"basis","installed manifest; does not establish activation")));
        }
        return rows;
    }
    static List<object> NpmPackages() {
        RequireStateIntegrity();var rows=new List<object>();
        foreach(string root in new[]{Path.Combine(Environment.GetEnvironmentVariable("APPDATA"),"npm\\node_modules"),Path.Combine(Environment.GetEnvironmentVariable("ProgramFiles"),"nodejs\\node_modules")})foreach(var directory in ApplicationDirectories(root)) {
            IEnumerable<DirectoryInfo> packages=directory.Name.StartsWith("@",StringComparison.Ordinal)?ApplicationDirectories(directory.FullName):new[]{directory};
            foreach(var package in packages){ApplicationBudget(rows);string path=Path.Combine(package.FullName,"package.json");if(!File.Exists(path))continue;object data=ReadManifest(path,1048576);rows.Add(Item(package.FullName,D("name",JsonField(data,"name"),"version",JsonField(data,"version"),"path",package.FullName,"basis","standard global npm directory; custom prefixes not scanned")));}
        }
        return rows;
    }
    static List<object> ChromiumExtensions() {
        RequireStateIntegrity();var rows=new List<object>();string local=Environment.GetEnvironmentVariable("LOCALAPPDATA");
        foreach(string browser in new[]{"Chrome","Edge"}) {
            string root=Path.Combine(local,browser=="Chrome"?"Google\\Chrome\\User Data":"Microsoft\\Edge\\User Data");
            foreach(var profile in ApplicationDirectories(root,false,p=>p.Name=="Default"||Regex.IsMatch(p.Name,"^Profile [0-9]+$"))) {
                var settings=new Dictionary<string,object>(StringComparer.OrdinalIgnoreCase);string status="not_provided";
                foreach(string name in new[]{"Preferences","Secure Preferences"}){
                    string path=Path.Combine(profile.FullName,name);if(!File.Exists(path))continue;var file=new FileInfo(path);
                    if(file.Length>16777216||(file.Attributes&FileAttributes.ReparsePoint)!=0){status="preferences_outside_read_policy";continue;}
                    try{object preferences=ReadManifest(path,16777216);var entries=JsonField(JsonField(preferences,"extensions"),"settings") as Dictionary<string,object>;if(entries!=null)foreach(var pair in entries)settings[pair.Key]=pair.Value;status="preferences_read";}catch{status="preferences_unavailable";}
                }
                foreach(var extension in ApplicationDirectories(Path.Combine(profile.FullName,"Extensions")))foreach(var version in ApplicationDirectories(extension.FullName)) {
                    ApplicationBudget(rows);string manifest=Path.Combine(version.FullName,"manifest.json");if(!File.Exists(manifest))continue;object data=ReadManifest(manifest,1048576),configuration;settings.TryGetValue(extension.Name,out configuration);
                    rows.Add(Item(browser+"/"+profile.Name+"/"+extension.Name+"/"+version.Name,D("browser",browser,"profile",profile.Name,"extension_id",extension.Name,"name",JsonField(data,"name"),"version",JsonField(data,"version"),"manifest_version",JsonField(data,"manifest_version"),"description",EvidenceText(Text(JsonField(data,"description"))),"permissions",ManifestArray(JsonField(data,"permissions")),"host_permissions",ManifestArray(JsonField(data,"host_permissions")),"optional_permissions",ManifestArray(JsonField(data,"optional_permissions")),"recorded_state",JsonField(configuration,"state"),"disable_reasons",JsonField(configuration,"disable_reasons"),"registered_path",JsonField(configuration,"path"),"preferences_status",status,"basis","on-disk manifest and recorded preferences; state code is not proof of current activation")));
                }
            }
        }
        return rows;
    }
    static List<object> FirefoxExtensions() {
        RequireStateIntegrity();var rows=new List<object>();string root=Path.Combine(Environment.GetEnvironmentVariable("APPDATA"),"Mozilla\\Firefox\\Profiles");
        foreach(var profile in ApplicationDirectories(root)){string path=Path.Combine(profile.FullName,"extensions.json");if(!File.Exists(path))continue;object data=ReadManifest(path,16777216);var addons=JsonField(data,"addons") as IEnumerable;if(addons==null)continue;
            foreach(object addon in addons){ApplicationBudget(rows);rows.Add(Item(profile.Name+"/"+Text(JsonField(addon,"id")),D("profile",profile.Name,"id",JsonField(addon,"id"),"version",JsonField(addon,"version"),"type",JsonField(addon,"type"),"active",JsonField(addon,"active"),"user_disabled",JsonField(addon,"userDisabled"),"app_disabled",JsonField(addon,"appDisabled"),"signed_state",JsonField(addon,"signedState"),"install_date",JsonField(addon,"installDate"),"update_date",JsonField(addon,"updateDate"),"basis","Firefox extension registry")));}
        }
        return rows;
    }
    static List<object> SshPublicKeys() {
        RequireStateIntegrity();var rows=new List<object>();string root=Path.Combine(Environment.GetEnvironmentVariable("USERPROFILE"),".ssh");ValidatePath(root);if(!Directory.Exists(root))return rows;
        foreach(var file in new DirectoryInfo(root).EnumerateFiles("*.pub")){
            ApplicationBudget(rows);ValidatePath(file.FullName);if(file.Length>65536||(file.Attributes&FileAttributes.ReparsePoint)!=0)throw new Exception("Public key outside read policy");
            string[] parts=Regex.Split(File.ReadAllText(file.FullName).Trim(),@"\s+",RegexOptions.None);if(parts.Length<2||!Regex.IsMatch(parts[0],"^(ssh-(rsa|ed25519)|ecdsa-sha2-nistp(256|384|521)|sk-ssh-ed25519@openssh.com|sk-ecdsa-sha2-nistp256@openssh.com)$"))throw new Exception("Unsupported public key encoding");
            byte[] material=Convert.FromBase64String(parts[1]);string fingerprint;try{using(var sha=SHA256.Create())fingerprint="SHA256:"+Convert.ToBase64String(sha.ComputeHash(material)).TrimEnd('=');}finally{Array.Clear(material,0,material.Length);}
            rows.Add(Item(file.FullName,D("path",file.FullName,"algorithm",parts[0],"fingerprint",fingerprint,"comment_collected",false,"key_material_collected",false)));
        }
        return rows;
    }
    static void CollectApplicationFiles(string unit) {
        if(unit=="applications") {Emit("pcscope.apps.ide-extensions",IdeExtensions);Emit("pcscope.apps.npm-global-packages",NpmPackages);Emit("pcscope.apps.chromium-extensions",ChromiumExtensions);Emit("pcscope.apps.firefox-extensions",FirefoxExtensions);}
        if(unit=="security")Emit("pcscope.security.ssh-public-keys",SshPublicKeys);
    }
    static List<object> PortablePrograms(string path,int maximum,long maxHash,bool hashFiles) {
        RequireStateIntegrity();string full=ValidatePath(path);var root=new DirectoryInfo(full);
        if(!root.Exists||(root.Attributes&FileAttributes.ReparsePoint)!=0)throw new Exception("Portable root must be an ordinary local directory");
        var queue=new Stack<DirectoryInfo>();queue.Push(root);var rows=new List<object>();int count=0;
        while(queue.Count>0)foreach(var entry in queue.Pop().EnumerateFileSystemInfos()) {
            if(++count>maximum)throw new Exception("Portable file scan limit reached; incomplete source discarded");ValidatePath(entry.FullName);
            bool directory=entry is DirectoryInfo,reparse=(entry.Attributes&FileAttributes.ReparsePoint)!=0;
            if(directory&&!reparse)queue.Push((DirectoryInfo)entry);
            if(directory||!new[]{".exe",".dll",".msi"}.Contains(entry.Extension,StringComparer.OrdinalIgnoreCase))continue;
            var file=(FileInfo)entry;long size=file.Length;DateTime modified=file.LastWriteTimeUtc;
            var data=D("path",file.FullName,"directory",false,"size",size,"creation_utc",file.CreationTimeUtc.ToString("o"),"last_write_utc",modified.ToString("o"),"attributes",file.Attributes.ToString(),"reparse_point",reparse,"basis","bounded snapshot; creation time is filesystem metadata, not installation time","content_stored",false,"hash_status",!hashFiles?"disabled_by_configuration":reparse?"reparse_target_not_read":size>maxHash?"skipped_size_limit":"pending","security_status",reparse?"reparse_target_not_read":"owner_and_dacl_only_no_sacl");
            if(!reparse) {
                var sections=System.Security.AccessControl.AccessControlSections.Access|System.Security.AccessControl.AccessControlSections.Owner;var acl=file.GetAccessControl(sections);
                data["owner"]=Owner(acl);data["dacl_sddl"]=acl.GetSecurityDescriptorSddlForm(sections);data["streams"]=Streams(file.FullName);
                var version=System.Diagnostics.FileVersionInfo.GetVersionInfo(file.FullName);data["version"]=version.FileVersion;data["company"]=version.CompanyName;data["signature_status"]="not_verified";
                if(hashFiles&&size<=maxHash) {using(var stream=File.OpenRead(file.FullName))using(var sha=SHA256.Create())data["sha256"]=BitConverter.ToString(sha.ComputeHash(stream)).Replace("-","").ToLowerInvariant();file.Refresh();if(file.Length!=size||file.LastWriteTimeUtc!=modified)throw new Exception("Portable file changed during hashing; incomplete source discarded");data["hash_status"]="collected";}
            }
            var identity=PCScopeMetadata.FileIdentity(file.FullName);data["volume_identity"]=identity[0];data["file_id"]=identity[1];rows.Add(Item(file.FullName.ToLowerInvariant(),data));
        }
        return rows;
    }
    static void CollectPortablePrograms(object options,int maximum,long maxHash) {
        if(!Enabled("pcscope.software.portable"))return;var roots=Get(options,"portable_roots") as IEnumerable;if(roots==null)return;
        bool hashFiles=Get(options,"hash_files") is bool&&(bool)Get(options,"hash_files");int count=0;
        foreach(object value in roots){if(++count>100)throw new Exception("Portable root count exceeds budget");string path=Text(value);string context=Hash(path+"/"+hashFiles+"/"+maximum+"/"+maxHash);Emit("pcscope.software.portable",()=>PortablePrograms(path,maximum,maxHash,hashFiles),context);}
    }
}
