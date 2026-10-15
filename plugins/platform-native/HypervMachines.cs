using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Management.Infrastructure;
using Microsoft.Management.Infrastructure.Options;

partial class Platform {
    static object VmRequired(StateRow row,string field){object value=row[field];if(value==null)throw new InvalidOperationException("Hyper-V metadata unavailable: "+field);return value;}
    static long VmMemory(StateRow row,string field,string units) {
        string name=Text(VmRequired(row,units)).Replace(" ","");long multiplier;
        if(name=="byte*2^20")multiplier=1048576;else if(name=="byte")multiplier=1;else throw new InvalidOperationException("Unsupported Hyper-V memory units");
        long amount=Convert.ToInt64(VmRequired(row,field),CultureInfo.InvariantCulture);if(amount<0)throw new InvalidOperationException("Invalid Hyper-V memory quantity");return checked(amount*multiplier);
    }
    static int VmState(int state,string other,int health) {
        if(state==1){var values=new Dictionary<string,int>(StringComparer.OrdinalIgnoreCase){{"Quiescing",32776},{"Resuming",32777},{"Saving",32773},{"FastSaving",32780},{"ComponentServicing",32784}};int decoded;if(other!=null&&values.TryGetValue(other,out decoded))state=decoded;}
        if(health!=5){var critical=new Dictionary<int,int>{{2,32785},{3,32786},{4,32787},{6,32788},{9,32789},{10,32790},{11,32791},{32773,32792},{32776,32793},{32777,32794},{32779,32795},{32780,32796}};int converted;if(critical.TryGetValue(state,out converted))state=converted;}
        return state;
    }
    static int VmCheckpoint(int value){if(value>=2&&value<=5)return value;throw new InvalidOperationException("Unsupported Hyper-V checkpoint configuration");}
    static object VmItem(StateRow vm,StateRow settings,StateRow memory,StateRow processor,int health) {
        Guid id,configuration;if(!Guid.TryParse(Text(VmRequired(vm,"Name")),out id)||id==Guid.Empty)throw new InvalidOperationException("Hyper-V VM has no GUID identity");
        if(!Guid.TryParse(Text(VmRequired(settings,"ConfigurationID")),out configuration)||configuration!=id)throw new InvalidOperationException("Hyper-V VM configuration identity mismatch");
        string subtype=Text(VmRequired(settings,"VirtualSystemSubType"));int generation;if(subtype=="Microsoft:Hyper-V:SubType:1")generation=1;else if(subtype=="Microsoft:Hyper-V:SubType:2")generation=2;else throw new InvalidOperationException("Unsupported Hyper-V VM generation");
        long processors=Convert.ToInt64(VmRequired(processor,"VirtualQuantity"),CultureInfo.InvariantCulture);if(processors<1)throw new InvalidOperationException("Invalid Hyper-V processor count");
        return Item(id.ToString(),D("Id",id.ToString(),"Name",EvidenceValue("Name",VmRequired(vm,"ElementName")),"State",VmState(Convert.ToInt32(VmRequired(vm,"EnabledState")),Text(vm["OtherEnabledState"]),health),"Generation",generation,"Version",EvidenceValue("Version",VmRequired(settings,"Version")),"ProcessorCount",processors,"MemoryStartup",VmMemory(memory,"VirtualQuantity","VirtualQuantityUnits"),"DynamicMemoryEnabled",VmRequired(memory,"DynamicMemoryEnabled"),"MemoryMinimum",VmMemory(memory,"Reservation","AllocationUnits"),"MemoryMaximum",VmMemory(memory,"Limit","AllocationUnits"),"AutomaticStartAction",VmRequired(settings,"AutomaticStartupAction"),"AutomaticStopAction",VmRequired(settings,"AutomaticShutdownAction"),"ConfigurationLocation",EvidenceValue("ConfigurationLocation",VmRequired(settings,"ConfigurationDataRoot")),"CheckpointType",VmCheckpoint(Convert.ToInt32(VmRequired(settings,"UserSnapshotType")))));
    }
    static CimInstance VmSingleResource(CimSession session,CimInstance settings,string name,CimOperationOptions options) {
        CimInstance result=null;try{foreach(var instance in session.EnumerateAssociatedInstances(HypervNamespace,settings,"Msvm_VirtualSystemSettingDataComponent",name,null,null,options)){if(result!=null){instance.Dispose();throw new InvalidOperationException("Ambiguous Hyper-V resource metadata");}result=instance;}if(result==null)throw new InvalidOperationException("Missing Hyper-V resource metadata: "+name);return result;}catch{if(result!=null)result.Dispose();throw;}
    }
    static int VmHealth(CimSession session,CimInstance service,CimInstance settings,CimOperationOptions options) {
        var parameters=new CimMethodParametersCollection();parameters.Add(CimMethodParameter.Create("SettingData",new[]{settings},CimType.ReferenceArray,CimFlags.In));parameters.Add(CimMethodParameter.Create("RequestedInformation",new uint[]{109},CimType.UInt32Array,CimFlags.In));
        // Fixed query only: no virtual machine state/configuration operations.
        using(var result=session.InvokeMethod(HypervNamespace,service,"GetSummaryInformation",parameters,options)) {
            if(result.ReturnValue==null||result.ReturnValue.Value==null||Convert.ToUInt32(result.ReturnValue.Value)!=0)throw new InvalidOperationException("Hyper-V summary read failed");
            CimInstance[] summaries=null;foreach(var output in result.OutParameters)if(String.Equals(output.Name,"SummaryInformation",StringComparison.OrdinalIgnoreCase))summaries=output.Value as CimInstance[];
            if(summaries==null||summaries.Length!=1)throw new InvalidOperationException("Hyper-V summary result requires one matching VM");
            try{return Convert.ToInt32(VmRequired(new StateRow(summaries[0]),"HealthState"));}finally{foreach(var summary in summaries)if(summary!=null)summary.Dispose();}
        }
    }
    static List<object> HypervMachines() {
        RequireStateIntegrity();var rows=new List<object>();
        using(var options=new CimOperationOptions{Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions{Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            CimInstance service=null;
            try {
                foreach(var vm in session.QueryInstances(HypervNamespace,"WQL","SELECT * FROM Msvm_ComputerSystem",options))using(vm) {
                    var row=new StateRow(vm);if(!String.Equals(Text(row["Description"]),"Microsoft Virtual Machine",StringComparison.Ordinal))continue;if(rows.Count>=4096)throw new InvalidOperationException("Hyper-V VM count exceeds budget");
                    if(service==null){foreach(var instance in session.QueryInstances(HypervNamespace,"WQL","SELECT * FROM Msvm_VirtualSystemManagementService",options)){if(service!=null){instance.Dispose();throw new InvalidOperationException("Hyper-V management service is ambiguous");}service=instance;}if(service==null)throw new InvalidOperationException("Hyper-V management service missing");}
                    CimInstance current=null;
                    try {
                        foreach(var candidate in session.EnumerateAssociatedInstances(HypervNamespace,vm,null,"Msvm_VirtualSystemSettingData",null,null,options)){if(Text(new StateRow(candidate)["VirtualSystemType"])!="Microsoft:Hyper-V:System:Realized"){candidate.Dispose();continue;}if(current!=null){candidate.Dispose();throw new InvalidOperationException("Hyper-V current VM settings are ambiguous");}current=candidate;}
                        if(current==null)throw new InvalidOperationException("Hyper-V current VM settings unavailable");
                        using(var memory=VmSingleResource(session,current,"Msvm_MemorySettingData",options))using(var processor=VmSingleResource(session,current,"Msvm_ProcessorSettingData",options))rows.Add(VmItem(row,new StateRow(current),new StateRow(memory),new StateRow(processor),VmHealth(session,service,current,options)));
                    }finally{if(current!=null)current.Dispose();}
                }
            }finally{if(service!=null)service.Dispose();}
        }
        return rows;
    }
    static void CollectHypervMachines(string unit){if(unit=="virtualization")Emit("pcscope.virtualization.hyperv-vms",HypervMachines);}
    static void HypervVmContractTest() {
        if(VmState(2,null,20)!=32785||VmState(1,"Saving",25)!=32792||VmState(3,null,5)!=3||VmState(65500,null,5)!=65500||VmState(9,null,10)!=32789)throw new Exception("VM state/health mapping mismatch");
        if(VmState(1,"Quiescing",5)!=32776||VmState(1,"quiescing",20)!=32793||VmState(1,"ComponentServicing",5)!=32784||VmState(1,"ComponentServicing",20)!=32784||VmState(1,"Running",5)!=1)throw new Exception("VM provider OtherEnabledState conversion mismatch");
        for(int special=32781;special<=32784;special++)if(VmState(special,null,5)!=special)throw new Exception("VM special enabled state changed");
        if(VmCheckpoint(3)!=3||VmCheckpoint(4)!=4)throw new Exception("VM checkpoint enum representation mismatch");
        int[] normalStates={2,3,4,6,9,10,11,32773,32776,32777,32779,32780};
        for(int index=0;index<normalStates.Length;index++){if(VmState(normalStates[index],null,20)!=32785+index||VmState(normalStates[index],null,5)!=normalStates[index])throw new Exception("VM complete critical-state conversion mismatch");}
        for(int checkpoint=2;checkpoint<=5;checkpoint++)if(VmCheckpoint(checkpoint)!=checkpoint)throw new Exception("Checkpoint enum number changed");
        using(var memory=new CimInstance("Msvm_MemorySettingData")){memory.CimInstanceProperties.Add(CimProperty.Create("VirtualQuantity",(ulong)4096,CimFlags.None));memory.CimInstanceProperties.Add(CimProperty.Create("VirtualQuantityUnits","byte * 2^20",CimFlags.None));if(VmMemory(new StateRow(memory),"VirtualQuantity","VirtualQuantityUnits")!=4294967296L)throw new Exception("VM memory byte conversion mismatch");memory.CimInstanceProperties["VirtualQuantityUnits"].Value="unknown";bool rejected=false;try{VmMemory(new StateRow(memory),"VirtualQuantity","VirtualQuantityUnits");}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Unknown memory units accepted");}
        HypervVmFieldContractTest();
        Console.WriteLine(Json.Serialize(D("critical_state",true,"non_ok_health_conversion",true,"provider_other_state",true,"special_numeric_states_preserved",true,"unknown_state_preserved",true,"memory_bytes",true,"unknown_units_rejected",true,"checkpoint_numeric_enum",true,"full_fields",14,"configuration_identity",true,"negative_memory_rejected",true,"memory_overflow_rejected",true,"state_changes",false)));
    }
    static void HypervVmFieldContractTest() {
        string id="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        using(var vm=new CimInstance("Msvm_ComputerSystem"))using(var settings=new CimInstance("Msvm_VirtualSystemSettingData"))using(var memory=new CimInstance("Msvm_MemorySettingData"))using(var processor=new CimInstance("Msvm_ProcessorSettingData")) {
            foreach(var pair in D("Name",id,"ElementName","开发虚拟机","EnabledState",2))vm.CimInstanceProperties.Add(CimProperty.Create(pair.Key,pair.Value,CimFlags.None));
            foreach(var pair in D("ConfigurationID",id,"VirtualSystemSubType","Microsoft:Hyper-V:SubType:2","Version","11.0","AutomaticStartupAction",2,"AutomaticShutdownAction",3,"ConfigurationDataRoot",@"C:\Fixture\VM","UserSnapshotType",4))settings.CimInstanceProperties.Add(CimProperty.Create(pair.Key,pair.Value,CimFlags.None));
            foreach(var pair in D("VirtualQuantity",4096L,"VirtualQuantityUnits","byte * 2^20","DynamicMemoryEnabled",true,"Reservation",512L,"Limit",8192L,"AllocationUnits","byte * 2^20"))memory.CimInstanceProperties.Add(CimProperty.Create(pair.Key,pair.Value,CimFlags.None));
            processor.CimInstanceProperties.Add(CimProperty.Create("VirtualQuantity",4L,CimFlags.None));
            object item=VmItem(new StateRow(vm),new StateRow(settings),new StateRow(memory),new StateRow(processor),5);
            var expected=D("Id",id,"Name","开发虚拟机","State",2,"Generation",2,"Version","11.0","ProcessorCount",4L,"MemoryStartup",4294967296L,"DynamicMemoryEnabled",true,"MemoryMinimum",536870912L,"MemoryMaximum",8589934592L,"AutomaticStartAction",2,"AutomaticStopAction",3,"ConfigurationLocation",@"C:\Fixture\VM","CheckpointType",4);
            if(Text(Get(item,"id"))!=id||Json.Serialize(Get(item,"data"))!=Json.Serialize(expected))throw new Exception("VM full field serialization mismatch");
            settings.CimInstanceProperties["ConfigurationID"].Value="11111111-2222-3333-4444-555555555555";
            bool rejected=false;try{VmItem(new StateRow(vm),new StateRow(settings),new StateRow(memory),new StateRow(processor),5);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Foreign VM configuration accepted");
            settings.CimInstanceProperties["ConfigurationID"].Value=id;memory.CimInstanceProperties["VirtualQuantity"].Value=-1L;
            rejected=false;try{VmMemory(new StateRow(memory),"VirtualQuantity","VirtualQuantityUnits");}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Negative VM memory accepted");
            memory.CimInstanceProperties["VirtualQuantity"].Value=Int64.MaxValue;
            rejected=false;try{VmMemory(new StateRow(memory),"VirtualQuantity","VirtualQuantityUnits");}catch(OverflowException){rejected=true;}if(!rejected)throw new Exception("VM memory overflow accepted");
        }
    }
}
