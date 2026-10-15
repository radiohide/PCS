using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Management.Infrastructure;
using Microsoft.Management.Infrastructure.Options;

partial class Platform {
    const string EncryptionNamespace="root/cimv2/Security/MicrosoftVolumeEncryption";
    static readonly string[] EncryptionNames={"None","Aes128Diffuser","Aes256Diffuser","Aes128","Aes256","Hardware","XtsAes128","XtsAes256"};
    static readonly string[] ConversionNames={"FullyDecrypted","FullyEncrypted","EncryptionInProgress","DecryptionInProgress","EncryptionSuspended","DecryptionSuspended","FullyEncryptedWipeInProgress","FullyEncryptedWipeSuspended"};
    static readonly string[] ProtectorNames={"Unknown","Tpm","ExternalKey","RecoveryPassword","TpmPin","TpmStartupKey","TpmPinStartupKey","PublicKey","Password","TpmNetworkKey","AdAccountOrGroup"};
    static string EncryptionLabel(uint value,string[] names){return value<names.Length?names[value]:value.ToString(CultureInfo.InvariantCulture);}
    static Dictionary<string,object> EncryptionRead(CimSession session,CimInstance volume,CimOperationOptions options,string method,CimMethodParametersCollection parameters=null) {
        // Fixed allowlist: never accept caller-selected method names or read recovery material.
        if(method!="GetLockStatus"&&method!="GetEncryptionMethod"&&method!="GetConversionStatus"&&method!="GetProtectionStatus"&&method!="GetKeyProtectors"&&method!="GetKeyProtectorType")throw new InvalidOperationException("Unsupported encryption metadata operation");
        using(var result=session.InvokeMethod(EncryptionNamespace,volume,method,parameters??new CimMethodParametersCollection(),options)) {
            if(result.ReturnValue==null||result.ReturnValue.Value==null)throw new InvalidOperationException("Encryption query omitted its return status");
            uint status=Convert.ToUInt32(result.ReturnValue.Value);if(status!=0)throw new InvalidOperationException("Encryption metadata query failed: "+method+" / "+status.ToString(CultureInfo.InvariantCulture));
            var fields=new Dictionary<string,object>(StringComparer.OrdinalIgnoreCase);foreach(var field in result.OutParameters)fields.Add(field.Name,field.Value);return fields;
        }
    }
    static object EncryptionField(Dictionary<string,object> fields,string name){object value;if(!fields.TryGetValue(name,out value))throw new InvalidOperationException("Encryption query omitted metadata: "+name);return value;}
    static uint EncryptionNumber(Dictionary<string,object> fields,string name){object value=EncryptionField(fields,name);if(value==null)throw new InvalidOperationException("Encryption query returned unavailable metadata: "+name);return Convert.ToUInt32(value,CultureInfo.InvariantCulture);}
    static object EncryptionItem(string mount,bool operating,uint locked,uint method,uint protection,uint? conversion,uint? wiping,uint? percentage,List<uint> protectors) {
        if(String.IsNullOrWhiteSpace(mount))throw new InvalidOperationException("Encrypted volume has no identity");
        string state="";object progress=null;
        if(locked==0){if(!conversion.HasValue||!wiping.HasValue||!percentage.HasValue||percentage.Value>100)throw new InvalidOperationException("Incomplete encryption progress");uint status=conversion.Value;if(status==1&&wiping.Value==2)status=6;else if(status==1&&wiping.Value==3)status=7;state=EncryptionLabel(status,ConversionNames);progress=percentage.Value;}
        var types=new List<string>();foreach(uint type in protectors)types.Add(EncryptionLabel(type,ProtectorNames));types.Sort(StringComparer.Ordinal);
        return Item(mount,D("mount_point",mount,"volume_type",operating?"OperatingSystem":"Data","volume_status",state,"protection_status",EncryptionLabel(protection,new[]{"Off","On","Unknown"}),"encryption_method",method==UInt32.MaxValue?"Unknown":EncryptionLabel(method,EncryptionNames),"encryption_percentage",progress,"lock_status",EncryptionLabel(locked,new[]{"Unlocked","Locked"}),"protector_types",types.ToArray(),"recovery_material_collected",false));
    }
    static List<object> EncryptionVolumes() {
        RequireStateIntegrity();var rows=new List<object>();var storage=new Dictionary<string,bool>(StringComparer.OrdinalIgnoreCase);
        using(var options=new CimOperationOptions{Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions{Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            // Query encryption first: ordinary permission failure cannot commit an empty inventory.
            var volumes=new List<CimInstance>();
            try {
                foreach(var volume in session.QueryInstances(EncryptionNamespace,"WQL","SELECT * FROM Win32_EncryptableVolume",options)){volumes.Add(volume);if(volumes.Count>4096)throw new InvalidOperationException("Encryption volume budget exceeded");}
                if(volumes.Count==0)return rows;
                string systemDrive=Environment.GetEnvironmentVariable("SystemDrive");if(String.IsNullOrEmpty(systemDrive))throw new InvalidOperationException("System drive identity unavailable");
                foreach(var instance in session.QueryInstances("root/Microsoft/Windows/Storage","WQL","SELECT * FROM MSFT_Volume",options))using(instance){var row=new StateRow(instance);string id=Text(row["UniqueId"]);if(String.IsNullOrEmpty(id))throw new InvalidOperationException("Storage volume has no identity");object letter=row["DriveLetter"];bool operating=letter!=null&&Char.ToUpperInvariant((char)Convert.ToUInt16(letter,CultureInfo.InvariantCulture))==Char.ToUpperInvariant(systemDrive[0]);storage.Add(id,operating);if(storage.Count>50000)throw new InvalidOperationException("Storage volume budget exceeded");}
                foreach(var volume in volumes) {
                    var row=new StateRow(volume);string id=Text(row["DeviceID"]),mount=Text(row["DriveLetter"]);bool operating;if(id==null||!storage.TryGetValue(id,out operating))throw new InvalidOperationException("Encrypted volume has no matching storage metadata");if(String.IsNullOrEmpty(mount))mount=id;
                    uint locked=EncryptionNumber(EncryptionRead(session,volume,options,"GetLockStatus"),"LockStatus");
                    uint method=EncryptionNumber(EncryptionRead(session,volume,options,"GetEncryptionMethod"),"EncryptionMethod");
                    uint? conversion=null,wiping=null,percentage=null;
                    if(locked==0){var state=EncryptionRead(session,volume,options,"GetConversionStatus");conversion=EncryptionNumber(state,"ConversionStatus");wiping=EncryptionNumber(state,"WipingStatus");percentage=EncryptionNumber(state,"EncryptionPercentage");}
                    uint protection=EncryptionNumber(EncryptionRead(session,volume,options,"GetProtectionStatus"),"ProtectionStatus");
                    object ids=EncryptionField(EncryptionRead(session,volume,options,"GetKeyProtectors"),"VolumeKeyProtectorID");var names=ids as string[];if(ids!=null&&names==null)throw new InvalidOperationException("Invalid encryption protector list");if(names!=null&&names.Length>4096)throw new InvalidOperationException("Encryption protector budget exceeded");
                    var protectors=new List<uint>();if(names!=null)foreach(string name in names){if(String.IsNullOrEmpty(name))throw new InvalidOperationException("Encryption protector has no identity");var parameters=new CimMethodParametersCollection();parameters.Add(CimMethodParameter.Create("VolumeKeyProtectorID",name,CimFlags.In));protectors.Add(EncryptionNumber(EncryptionRead(session,volume,options,"GetKeyProtectorType",parameters),"KeyProtectorType"));}
                    rows.Add(EncryptionItem(mount,operating,locked,method,protection,conversion,wiping,percentage,protectors));
                }
            }finally{foreach(var volume in volumes)volume.Dispose();}
        }
        return rows;
    }
    static void CollectEncryption(string unit){if(unit=="security")Emit("pcscope.security.encryption",EncryptionVolumes);}
    static void EncryptionContractTest() {
        object data=Get(EncryptionItem("C:",true,0,7,1,1,0,100,new List<uint>{3,1}),"data");
        if(Text(Get(data,"volume_status"))!="FullyEncrypted"||Text(Get(data,"encryption_method"))!="XtsAes256"||Text(Get(data,"volume_type"))!="OperatingSystem"||(bool)Get(data,"recovery_material_collected"))throw new Exception("Encryption status mapping mismatch");
        var types=Get(data,"protector_types") as string[];if(types==null||types.Length!=2||types[0]!="RecoveryPassword"||types[1]!="Tpm")throw new Exception("Encryption protector mapping mismatch");
        object locked=Get(EncryptionItem("D:",false,1,UInt32.MaxValue,2,null,null,null,new List<uint>()),"data");if(Text(Get(locked,"volume_status"))!=""||Get(locked,"encryption_percentage")!=null||Text(Get(locked,"encryption_method"))!="Unknown")throw new Exception("Locked volume invented progress");
        object paused=Get(EncryptionItem("D:",false,0,6,1,1,3,100,new List<uint>()),"data");if(Text(Get(paused,"volume_status"))!="FullyEncryptedWipeSuspended")throw new Exception("Wipe suspended state mismatch");
        bool rejected=false;try{EncryptionItem("D:",false,0,6,1,2,0,101,new List<uint>());}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Invalid encryption percentage accepted");
        Console.WriteLine(Json.Serialize(D("status_mapping",true,"protector_types_only",true,"locked_progress_unavailable",true,"wipe_suspended",true,"invalid_progress_rejected",true,"no_recovery_read",true)));
    }
}
