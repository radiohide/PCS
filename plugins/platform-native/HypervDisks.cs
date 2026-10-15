using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Management.Infrastructure;
using Microsoft.Management.Infrastructure.Options;

partial class Platform {
    static string HypervDiskParentId(string path) {
        using(var reference=HypervReference(path)){if(!String.Equals(reference.CimSystemProperties.ClassName,"Msvm_ResourceAllocationSettingData",StringComparison.OrdinalIgnoreCase))throw new InvalidOperationException("Unexpected Hyper-V disk parent class");return Text(reference.CimInstanceProperties["InstanceID"].Value);}
    }
    static string HypervDiskResource(StateRow row) {
        object value=row["HostResource"];if(value==null)return null;var resources=value as string[];if(resources==null||resources.Length>1)throw new InvalidOperationException("Ambiguous Hyper-V disk host resource");return resources.Length==0?null:resources[0];
    }
    static int HypervDiskAddress(StateRow row,string field) {
        int value;if(!Int32.TryParse(Text(VmRequired(row,field)),NumberStyles.Integer,CultureInfo.InvariantCulture,out value)||value<0)throw new InvalidOperationException("Hyper-V disk controller address unavailable");return value;
    }
    static void HypervControllerNumber(Dictionary<string,int> numbers,string id,int type,string subtype,ref int ide,ref int scsi) {
        int number;if(type==5&&subtype=="Microsoft:Hyper-V:Emulated IDE Controller")number=ide++;
        else if(type==6&&subtype=="Microsoft:Hyper-V:Synthetic SCSI Controller")number=scsi++;
        else return;
        if(String.IsNullOrWhiteSpace(id)||numbers.ContainsKey(id))throw new InvalidOperationException("Hyper-V controller identity is missing or duplicated");numbers.Add(id,number);
    }
    static Dictionary<string,int> HypervControllerNumbers(CimSession session,CimInstance current,CimOperationOptions options) {
        var numbers=new Dictionary<string,int>(StringComparer.OrdinalIgnoreCase);int ide=0,scsi=0,count=0;
        // The original API assigns independent zero-based indices in returned order, not RASD.Address.
        foreach(var resource in session.EnumerateAssociatedInstances(HypervNamespace,current,"Msvm_VirtualSystemSettingDataComponent","Msvm_ResourceAllocationSettingData",null,null,options))using(resource) {
            if(++count>50000)throw new InvalidOperationException("Hyper-V controller enumeration budget exceeded");var row=new StateRow(resource);
            HypervControllerNumber(numbers,Text(VmRequired(row,"InstanceID")),Convert.ToInt32(VmRequired(row,"ResourceType")),Text(VmRequired(row,"ResourceSubType")),ref ide,ref scsi);
        }
        return numbers;
    }
    static object HypervDiskItem(string vm,string name,int type,int number,int slot,string path,int? physical) {
        Guid id;if(!Guid.TryParse(vm,out id)||String.IsNullOrWhiteSpace(name)||type<0||type>1||number<0||slot<0||physical<0)throw new InvalidOperationException("Invalid Hyper-V disk attachment identity");
        string controller=type==0?"IDE":"SCSI";
        return Item(id.ToString()+"/"+controller+"/"+number.ToString(CultureInfo.InvariantCulture)+"/"+slot.ToString(CultureInfo.InvariantCulture),D("VMId",id.ToString(),"VMName",EvidenceText(name),"Path",EvidenceValue("Path",path),"ControllerType",type,"ControllerNumber",number,"ControllerLocation",slot,"DiskNumber",physical));
    }
    static void HypervVmDisks(CimSession session,CimInstance current,string vm,string name,CimOperationOptions options,List<object> rows) {
        var resources=new Dictionary<string,CimInstance>(StringComparer.OrdinalIgnoreCase);
        try {
            foreach(var resource in session.EnumerateAssociatedInstances(HypervNamespace,current,"Msvm_VirtualSystemSettingDataComponent","CIM_ResourceAllocationSettingData",null,null,options)) {
                try {string id=Text(VmRequired(new StateRow(resource),"InstanceID"));if(resources.Count>=50000||resources.ContainsKey(id))throw new InvalidOperationException("Hyper-V resource graph budget or identity conflict");resources.Add(id,resource);}catch{resource.Dispose();throw;}
            }
            var controllerNumbers=HypervControllerNumbers(session,current,options);
            foreach(var resource in resources.Values) {
                var drive=new StateRow(resource);if(Convert.ToInt32(VmRequired(drive,"ResourceType"))!=17)continue;
                string subtype=Text(VmRequired(drive,"ResourceSubType"));bool physical=subtype=="Microsoft:Hyper-V:Physical Disk Drive";
                if(!physical&&subtype!="Microsoft:Hyper-V:Synthetic Disk Drive")throw new InvalidOperationException("Unsupported Hyper-V disk drive subtype");
                string controllerId=HypervDiskParentId(Text(VmRequired(drive,"Parent")));CimInstance controller;
                if(!resources.TryGetValue(controllerId,out controller))throw new InvalidOperationException("Hyper-V disk controller is outside current VM settings");
                var control=new StateRow(controller);string controlType=Text(VmRequired(control,"ResourceSubType"));int type;
                if(controlType=="Microsoft:Hyper-V:Emulated IDE Controller")type=0;else if(controlType=="Microsoft:Hyper-V:Synthetic SCSI Controller")type=1;else throw new InvalidOperationException("Unsupported Hyper-V disk controller type");
                int number;if(!controllerNumbers.TryGetValue(controllerId,out number))throw new InvalidOperationException("Hyper-V controller number could not be confirmed");int slot=HypervDiskAddress(drive,"AddressOnParent");string path=null;int? diskNumber=null;
                if(physical) {
                    string host=HypervDiskResource(drive);if(!String.IsNullOrEmpty(host))using(var reference=HypervReference(host)) {
                        if(!String.Equals(reference.CimSystemProperties.ClassName,"Msvm_DiskDrive",StringComparison.OrdinalIgnoreCase)||!String.Equals(Text(reference.CimInstanceProperties["SystemName"].Value),Environment.MachineName,StringComparison.OrdinalIgnoreCase))throw new InvalidOperationException("Physical Hyper-V disk resource is outside host scope");
                        using(var device=session.GetInstance(HypervNamespace,reference,options)){var deviceRow=new StateRow(device);path=Text(VmRequired(deviceRow,"ElementName"));diskNumber=checked(Convert.ToInt32(VmRequired(deviceRow,"DriveNumber"),CultureInfo.InvariantCulture));}
                    }
                }else {
                    int mediaCount=0;string driveId=Text(VmRequired(drive,"InstanceID"));
                    foreach(var media in resources.Values){var medium=new StateRow(media);if(Text(medium["ResourceSubType"])!="Microsoft:Hyper-V:Virtual Hard Disk")continue;string parent=Text(medium["Parent"]);if(parent==null||!String.Equals(HypervDiskParentId(parent),driveId,StringComparison.OrdinalIgnoreCase))continue;if(++mediaCount>1)throw new InvalidOperationException("Hyper-V disk has ambiguous attached media");path=HypervDiskResource(medium);}
                }
                if(rows.Count>=50000)throw new InvalidOperationException("Hyper-V disk attachment budget exceeded");rows.Add(HypervDiskItem(vm,name,type,number,slot,path,diskNumber));
            }
        }finally{foreach(var resource in resources.Values)resource.Dispose();}
    }
    static List<object> HypervDisks() {
        RequireStateIntegrity();var rows=new List<object>();int vmCount=0;
        using(var options=new CimOperationOptions{Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions{Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            foreach(var vm in session.QueryInstances(HypervNamespace,"WQL","SELECT * FROM Msvm_ComputerSystem",options))using(vm) {
                var machine=new StateRow(vm);if(Text(machine["Description"])!="Microsoft Virtual Machine")continue;if(++vmCount>4096)throw new InvalidOperationException("Hyper-V VM budget exceeded");
                CimInstance current=null;
                try {
                    foreach(var candidate in session.EnumerateAssociatedInstances(HypervNamespace,vm,null,"Msvm_VirtualSystemSettingData",null,null,options)){if(Text(new StateRow(candidate)["VirtualSystemType"])!="Microsoft:Hyper-V:System:Realized"){candidate.Dispose();continue;}if(current!=null){candidate.Dispose();throw new InvalidOperationException("Ambiguous current Hyper-V settings");}current=candidate;}
                    if(current==null)throw new InvalidOperationException("Current Hyper-V settings unavailable");HypervVmDisks(session,current,Text(VmRequired(machine,"Name")),Text(VmRequired(machine,"ElementName")),options,rows);
                }finally{if(current!=null)current.Dispose();}
            }
        }
        return rows;
    }
    static void CollectHypervDisks(string unit){if(unit=="virtualization")Emit("pcscope.virtualization.hyperv-disks",HypervDisks);}
    static void HypervDiskContractTest() {
        string vm="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        object file=HypervDiskItem(vm,"样本 VM",1,2,3,"C:\\fixture.vhdx",null);if(Text(Get(file,"id"))!=vm+"/SCSI/2/3"||Get(Get(file,"data"),"DiskNumber")!=null)throw new Exception("Virtual disk identity/null physical number mismatch");
        object physical=HypervDiskItem(vm,"样本 VM",0,1,0,"Physical Drive 4",4);if(Text(Get(physical,"id"))!=vm+"/IDE/1/0"||Convert.ToInt32(Get(Get(physical,"data"),"DiskNumber"))!=4)throw new Exception("Pass-through disk field mapping mismatch");
        using(var reference=HypervReference("Msvm_ResourceAllocationSettingData.InstanceID=\"Microsoft:fixture\\\\controller\"")){if(Text(reference.CimInstanceProperties["InstanceID"].Value)!="Microsoft:fixture\\controller")throw new Exception("Controller reference escape mismatch");}
        bool rejected=false;try{HypervDiskItem(vm,"VM",1,-1,0,null,null);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Missing controller number accepted");
        using(var resource=new CimInstance("Msvm_StorageAllocationSettingData")){resource.CimInstanceProperties.Add(CimProperty.Create("HostResource",new[]{"a","b"},CimType.StringArray,CimFlags.None));rejected=false;try{HypervDiskResource(new StateRow(resource));}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Ambiguous disk backing resource accepted");}
        var numbers=new Dictionary<string,int>();int ide=0,scsi=0;
        HypervControllerNumber(numbers,"scsi-b",6,"Microsoft:Hyper-V:Synthetic SCSI Controller",ref ide,ref scsi);HypervControllerNumber(numbers,"ide-z",5,"Microsoft:Hyper-V:Emulated IDE Controller",ref ide,ref scsi);HypervControllerNumber(numbers,"scsi-a",6,"Microsoft:Hyper-V:Synthetic SCSI Controller",ref ide,ref scsi);HypervControllerNumber(numbers,"ide-a",5,"Microsoft:Hyper-V:Emulated IDE Controller",ref ide,ref scsi);
        if(numbers["scsi-b"]!=0||numbers["scsi-a"]!=1||numbers["ide-z"]!=0||numbers["ide-a"]!=1)throw new Exception("Controller numbering changed returned order or mixed types");
        rejected=false;try{HypervControllerNumber(numbers,"scsi-a",6,"Microsoft:Hyper-V:Synthetic SCSI Controller",ref ide,ref scsi);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Duplicate controller identity accepted");
        Console.WriteLine(Json.Serialize(D("controller_identity",true,"controller_returned_order",true,"separate_type_indices",true,"duplicate_controller_rejected",true,"virtual_path",true,"physical_path_and_number",true,"reference_escaping",true,"missing_number_rejected",true,"ambiguous_resource_rejected",true,"disk_content_read",false,"system_changes",false)));
    }
}
