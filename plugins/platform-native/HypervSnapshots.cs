using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Management.Infrastructure;
using Microsoft.Management.Infrastructure.Options;

partial class Platform {
    static int HypervSnapshotType(string value) {
        var types=new Dictionary<string,int>(StringComparer.OrdinalIgnoreCase){{"Microsoft:Hyper-V:Snapshot:Realized",0},{"Microsoft:Hyper-V:Snapshot:Recovery",1},{"Microsoft:Hyper-V:Snapshot:Planned",2},{"Microsoft:Hyper-V:Snapshot:Missing",5},{"Microsoft:Hyper-V:Snapshot:Replica:Standard",6},{"Microsoft:Hyper-V:Snapshot:Replica:ApplicationConsistent",7},{"Microsoft:Hyper-V:Snapshot:Replica:PlannedFailover",8}};
        int type;if(value==null||!types.TryGetValue(value,out type))throw new InvalidOperationException("Unsupported Hyper-V snapshot type");return type;
    }
    static string HypervSnapshotId(StateRow row) {Guid id;if(!Guid.TryParse(Text(VmRequired(row,"ConfigurationID")),out id)||id==Guid.Empty)throw new InvalidOperationException("Hyper-V snapshot has no configuration identity");return id.ToString();}
    static object HypervSnapshotItem(StateRow snapshot,string expectedVm,string parent) {
        Guid vm,expected;if(!Guid.TryParse(Text(VmRequired(snapshot,"VirtualSystemIdentifier")),out vm)||!Guid.TryParse(expectedVm,out expected)||vm!=expected)throw new InvalidOperationException("Hyper-V snapshot VM association mismatch");
        string id=HypervSnapshotId(snapshot);if(parent!=null){Guid parsed;if(!Guid.TryParse(parent,out parsed)||parsed==Guid.Empty||parsed.ToString()==id)throw new InvalidOperationException("Invalid Hyper-V parent snapshot identity");parent=parsed.ToString();}
        object created=WmiProperty(snapshot,"CreationTime");if(created==null)throw new InvalidOperationException("Hyper-V snapshot creation time unavailable");
        return Item(id,D("Id",id,"VMId",vm.ToString(),"Name",EvidenceValue("Name",VmRequired(snapshot,"ElementName")),"CreationTime",created,"SnapshotType",HypervSnapshotType(Text(VmRequired(snapshot,"VirtualSystemType"))),"ParentSnapshotId",parent));
    }
    static string HypervSnapshotParent(CimSession session,CimInstance snapshot,string expectedVm,CimOperationOptions options) {
        string parent=null;int count=0;
        foreach(var instance in session.EnumerateAssociatedInstances(HypervNamespace,snapshot,"Msvm_ParentChildSettingData","Msvm_VirtualSystemSettingData","Dependent","Antecedent",options))using(instance) {
            if(++count>1)throw new InvalidOperationException("Hyper-V snapshot parent is ambiguous");var row=new StateRow(instance);
            Guid parentVm,expected;if(!Guid.TryParse(Text(VmRequired(row,"VirtualSystemIdentifier")),out parentVm)||!Guid.TryParse(expectedVm,out expected)||parentVm!=expected)throw new InvalidOperationException("Hyper-V parent snapshot belongs to another VM");
            HypervSnapshotType(Text(VmRequired(row,"VirtualSystemType")));parent=HypervSnapshotId(row);
        }
        if(parent==null&&!String.IsNullOrEmpty(Text(new StateRow(snapshot)["Parent"])))throw new InvalidOperationException("Hyper-V parent snapshot reference could not be confirmed");
        return parent;
    }
    static List<object> HypervSnapshots() {
        RequireStateIntegrity();var rows=new List<object>();int vmCount=0;
        using(var options=new CimOperationOptions{Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions{Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            foreach(var vm in session.QueryInstances(HypervNamespace,"WQL","SELECT * FROM Msvm_ComputerSystem",options))using(vm) {
                var machine=new StateRow(vm);if(Text(machine["Description"])!="Microsoft Virtual Machine")continue;if(++vmCount>4096)throw new InvalidOperationException("Hyper-V VM budget exceeded");
                string id=Text(VmRequired(machine,"Name"));
                foreach(var snapshot in session.EnumerateAssociatedInstances(HypervNamespace,vm,"Msvm_SnapshotOfVirtualSystem","Msvm_VirtualSystemSettingData","Antecedent","Dependent",options))using(snapshot) {
                    if(rows.Count>=50000)throw new InvalidOperationException("Hyper-V snapshot budget exceeded");rows.Add(HypervSnapshotItem(new StateRow(snapshot),id,HypervSnapshotParent(session,snapshot,id,options)));
                }
            }
        }
        return rows;
    }
    static void CollectHypervSnapshots(string unit){if(unit=="virtualization")Emit("pcscope.virtualization.hyperv-checkpoints",HypervSnapshots);}
    static void HypervSnapshotContractTest() {
        string vm="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",id="11111111-2222-3333-4444-555555555555",parent="66666666-7777-8888-9999-aaaaaaaaaaaa";
        using(var instance=new CimInstance("Msvm_VirtualSystemSettingData")) {
            instance.CimInstanceProperties.Add(CimProperty.Create("ConfigurationID",id,CimFlags.None));instance.CimInstanceProperties.Add(CimProperty.Create("VirtualSystemIdentifier",vm,CimFlags.None));instance.CimInstanceProperties.Add(CimProperty.Create("ElementName","更新前",CimFlags.None));instance.CimInstanceProperties.Add(CimProperty.Create("VirtualSystemType","Microsoft:Hyper-V:Snapshot:Realized",CimFlags.None));instance.CimInstanceProperties.Add(CimProperty.Create("CreationTime",new DateTime(2026,10,14,1,2,3,DateTimeKind.Utc),CimFlags.None));
            object item=HypervSnapshotItem(new StateRow(instance),vm,parent),data=Get(item,"data");if(Text(Get(item,"id"))!=id||Text(Get(data,"VMId"))!=vm||Text(Get(data,"ParentSnapshotId"))!=parent||Text(Get(data,"CreationTime"))!="2026-10-14T01:02:03.0000000Z")throw new Exception("Snapshot identity/time/parent mapping mismatch");
            if(Get(Get(HypervSnapshotItem(new StateRow(instance),vm,null),"data"),"ParentSnapshotId")!=null)throw new Exception("Root snapshot parent was invented");
            bool rejected=false;try{HypervSnapshotItem(new StateRow(instance),parent,null);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Wrong VM association accepted");
            rejected=false;try{HypervSnapshotItem(new StateRow(instance),vm,id);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Self-parent snapshot accepted");
        }
        string[] suffixes={"Realized","Recovery","Planned","Missing","Replica:Standard","Replica:ApplicationConsistent","Replica:PlannedFailover"};int[] expected={0,1,2,5,6,7,8};for(int i=0;i<expected.Length;i++)if(HypervSnapshotType("Microsoft:Hyper-V:Snapshot:"+suffixes[i])!=expected[i])throw new Exception("Snapshot enum mapping mismatch");
        bool unknown=false;try{HypervSnapshotType("Microsoft:Hyper-V:System:Realized");}catch(InvalidOperationException){unknown=true;}if(!unknown)throw new Exception("Current VM configuration accepted as a snapshot");
        Console.WriteLine(Json.Serialize(D("configuration_identity",true,"vm_association",true,"parent_mapping",true,"utc_time",true,"snapshot_types",7,"non_snapshot_rejected",true,"system_changes",false)));
    }
}
