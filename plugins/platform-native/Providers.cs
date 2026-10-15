using System;
using System.Collections.Generic;
using System.Collections;

partial class Platform {
    static string StorageLabel(object value,Dictionary<int,string> labels,string unknown="Unknown"){string label;return value!=null&&labels.TryGetValue(Convert.ToInt32(value),out label)?label:unknown;}
    static readonly Dictionary<int,string> StorageBus=new Dictionary<int,string>{{0,"Unknown"},{1,"SCSI"},{2,"ATAPI"},{3,"ATA"},{4,"1394"},{5,"SSA"},{6,"Fibre Channel"},{7,"USB"},{8,"RAID"},{9,"iSCSI"},{10,"SAS"},{11,"SATA"},{12,"SD"},{13,"MMC"},{14,"Virtual"},{15,"File Backed Virtual"},{16,"Spaces"},{17,"NVMe"},{18,"SCM"},{19,"UFS"}};
    static object StorageStatuses(object value,bool physical) {
        var labels=new Dictionary<int,string>{{1,"Other"},{2,"OK"},{3,"Degraded"},{4,"Stressed"},{5,"Predictive Failure"},{6,"Error"},{7,"Non-Recoverable Error"},{8,"Starting"},{9,"Stopping"},{10,"Stopped"},{11,physical?"In Service":"InService"},{12,"No Contact"},{13,"Lost Communication"},{14,"Aborted"},{15,"Dormant"},{16,"Supporting Entity in Error"},{17,"Completed"},{18,"Power Mode"},{19,"Relocating"}};
        var extra=physical?new Dictionary<int,string>{{53252,"Failed Media"},{53253,"Split"},{53254,"Stale Metadata"},{53255,"IO Error"},{53256,"Unrecognized Metadata"},{53269,"Removing From Pool"},{53270,"In Maintenance Mode"},{53271,"Updating Firmware"},{53272,"Device Hardware Error"},{53273,"Not Usable"},{53274,"Transient Error"},{53276,"Starting Maintenance Mode"},{53277,"Stopping Maintenance Mode"},{53285,"Threshold Exceeded"},{53286,"Abnormal Latency"}}:new Dictionary<int,string>{{53264,"Online"},{53265,"Not Ready"},{53266,"No Media"},{53267,"Offline"},{53268,"Failed"}};
        foreach(var pair in extra)labels[pair.Key]=pair.Value;
        var values=new List<object>();var array=value as IEnumerable;if(array!=null)foreach(object code in array)values.Add(StorageLabel(code,labels));
        return values.Count==0?null:values.Count==1?values[0]:(object)values.ToArray();
    }
    static string PartitionType(StateRow row) {
        if(Convert.ToInt32(row["MbrType"])>0)return StorageLabel(row["MbrType"],new Dictionary<int,string>{{1,"FAT12"},{2,"Xenix"},{3,"Xenix"},{4,"FAT16"},{5,"Extended"},{6,"Logical"},{7,"IFS"},{10,"OS/2 BootMgr"},{11,"FAT32"},{12,"FAT32 XINT13"},{14,"XINT13"},{15,"XINT13 Extended"},{65,"PReP"},{66,"LDM"},{99,"Unix"},{231,"Space Protective"}});
        var types=new Dictionary<string,string>(StringComparer.OrdinalIgnoreCase){{"{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}","System"},{"{e3c9e316-0b5c-4db8-817d-f92df00215ae}","Reserved"},{"{ebd0a0a2-b9e5-4433-87c0-68b6b72699c7}","Basic"},{"{5808c8aa-7e8f-42e0-85d2-e1e90434cfb3}","LDM Metadata"},{"{af9b60a0-1431-4f62-bc68-3311714a69ad}","LDM Data"},{"{de94bba4-06d1-4d40-a16a-bfd50179d6ac}","Recovery"},{"{e75caf8f-f680-4cee-afa3-b001e56efc2d}","Space Protective"},{"{eeff8352-dd2a-44db-ae83-bee1cf7481dc}","S2D Cache"},{"{03aaa829-ebfc-4e7e-aac9-c4d76c63b24b}","S2D Cache Metadata"}};string label;
        return types.TryGetValue(Text(row["GptType"])??"",out label)?label:"Unknown";
    }
    static List<object> StorageItems(string name,string[] fields) {
        return ReadWmi("root\\Microsoft\\Windows\\Storage",name,row=>{
            var data=WmiFields(row,fields);bool physical=name=="MSFT_PhysicalDisk";
            if(name=="MSFT_Partition")data["Type"]=PartitionType(row);
            else {
                data["BusType"]=physical&&row["BusType"]==null?null:StorageLabel(row["BusType"],StorageBus);
                data["HealthStatus"]=StorageLabel(row["HealthStatus"],new Dictionary<int,string>{{0,"Healthy"},{1,"Warning"},{2,"Unhealthy"},{5,"Unknown"}});
                data["OperationalStatus"]=StorageStatuses(row["OperationalStatus"],physical);
                if(physical)data["MediaType"]=StorageLabel(row["MediaType"],new Dictionary<int,string>{{0,"Unspecified"},{3,"HDD"},{4,"SSD"},{5,"SCM"}},"Unspecified");
                else data["PartitionStyle"]=StorageLabel(row["PartitionStyle"],new Dictionary<int,string>{{0,"RAW"},{1,"MBR"},{2,"GPT"}});
            }
            return Item(Text(row["ObjectId"]),data);
        });
    }
    static void CollectProviders(string unit) {
        const string defender="root\\Microsoft\\Windows\\Defender",smb="root\\Microsoft\\Windows\\SMB";
        switch(unit) {
            case "storage":
                Emit("pcscope.storage.disks",()=>StorageItems("MSFT_Disk",new[]{"ObjectId","UniqueId","Number","FriendlyName","BusType","PartitionStyle","Size","IsBoot","IsSystem","IsOffline","IsReadOnly","OperationalStatus","HealthStatus"}));
                Emit("pcscope.storage.partitions",()=>StorageItems("MSFT_Partition",new[]{"ObjectId","DiskId","DiskNumber","PartitionNumber","Guid","Offset","Size","Type","GptType","MbrType","AccessPaths","IsActive","IsBoot","IsHidden","IsReadOnly"}));
                Emit("pcscope.hardware.disk-health",()=>StorageItems("MSFT_PhysicalDisk",new[]{"ObjectId","FriendlyName","MediaType","BusType","Size","HealthStatus","OperationalStatus","FirmwareVersion"}));break;
            case "security":
                Emit("pcscope.security.protection",()=>ReadWmi(defender,"MSFT_MpComputerStatus",row=>Item("defender",WmiFields(row,new[]{"AMServiceEnabled","AntispywareEnabled","AntivirusEnabled","BehaviorMonitorEnabled","IoavProtectionEnabled","NISEnabled","OnAccessProtectionEnabled","RealTimeProtectionEnabled","IsTamperProtected","AMEngineVersion","AMProductVersion","AntivirusSignatureVersion","AntivirusSignatureLastUpdated","QuickScanEndTime","FullScanEndTime"}))));
                Emit("pcscope.security.protection-policy",()=>ReadWmi(defender,"MSFT_MpPreference",row=>Item("defender-policy",WmiFields(row,new[]{"DisableRealtimeMonitoring","DisableBehaviorMonitoring","DisableIOAVProtection","DisableScriptScanning","PUAProtection","ExclusionPath","ExclusionProcess","ExclusionExtension"}))));
                Emit("pcscope.security.products",()=>WmiItems("AntiVirusProduct","instanceGuid",new[]{"instanceGuid","displayName","productState","timestamp"},"root\\SecurityCenter2"));
                Emit("pcscope.security.detections",()=>WmiItems("MSFT_MpThreatDetection","DetectionID",new[]{"DetectionID","ThreatID","InitialDetectionTime","LastThreatStatusChangeTime","ActionSuccess","CleaningActionID","ThreatStatusID","CurrentThreatExecutionStatusID"},defender));
                Emit("pcscope.security.deviceguard",()=>WmiItems("Win32_DeviceGuard","InstanceIdentifier",new[]{"InstanceIdentifier","VirtualizationBasedSecurityStatus","SecurityServicesConfigured","SecurityServicesRunning","AvailableSecurityProperties","RequiredSecurityProperties","CodeIntegrityPolicyEnforcementStatus"},"root\\Microsoft\\Windows\\DeviceGuard"));break;
            case "network":
                Emit("pcscope.network.profiles",()=>ReadWmi("root\\StandardCimv2","MSFT_NetConnectionProfile",row=>Item(Text(row["InterfaceIndex"])+"/"+Text(row["Name"]),WmiFields(row,new[]{"InterfaceIndex","Name","NetworkCategory","IPv4Connectivity","IPv6Connectivity"}))));
                Emit("pcscope.network.mapped-drives",()=>ReadWmi(smb,"MSFT_SmbMapping",row=>Item(Text(row["LocalPath"])+"/"+Text(row["RemotePath"]),WmiFields(row,new[]{"LocalPath","RemotePath","Status"}))));
                Emit("pcscope.network.share-sessions",()=>WmiItems("MSFT_SMBSession","SessionId",new[]{"SessionId","ClientComputerName","ClientUserName","NumOpens","Dialect","Encrypted","ContinuouslyAvailable"},smb));break;
            case "startup":
                Emit("pcscope.startup.wmi-filters",()=>ReadWmi("root\\subscription","__EventFilter",row=>Item(Text(row["Name"]),D("name",row["Name"],"event_namespace",row["EventNamespace"],"query_language",row["QueryLanguage"],"query",EvidenceText(Text(row["Query"])),"source_properties",WmiPublic(row)))));
                Emit("pcscope.startup.wmi-consumers",()=>ReadWmi("root\\subscription","__EventConsumer",row=>{string name=row.Instance.CimSystemProperties.ClassName;return Item(name+"/"+Text(row["Name"]),D("name",row["Name"],"class",name,"command",EvidenceText(Text(row["CommandLineTemplate"])),"script",EvidenceValue("ScriptText",row["ScriptText"]),"script_engine",row["ScriptingEngine"],"executable_path",row["ExecutablePath"],"source_properties",WmiPublic(row)));}));
                Emit("pcscope.startup.wmi-bindings",()=>ReadWmi("root\\subscription","__FilterToConsumerBinding",row=>Item(Text(row["Filter"])+"/"+Text(row["Consumer"]),WmiFields(row,new[]{"Filter","Consumer","MaintainSecurityContext","SlowDownProviders","DeliverSynchronously"}))));break;
        }
    }
}
