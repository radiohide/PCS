using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Management.Infrastructure;
using Microsoft.Management.Infrastructure.Options;

partial class Platform {
    static object VpnItem(CimInstance instance) {
        var row=new StateRow(instance);string name=Text(row["Name"]);
        if(String.IsNullOrWhiteSpace(name))throw new InvalidOperationException("VPN profile has no identity");
        if(!(row["AllUserConnection"] is bool)||((bool)row["AllUserConnection"]))throw new InvalidOperationException("VPN provider returned a profile outside the current-user collection scope");
        return Item(name,WmiFields(row,new[]{"Name","TunnelType","ConnectionStatus","SplitTunneling","AllUserConnection","AuthenticationMethod"}));
    }
    static void AddVpnResult(List<object> rows,CimMethodParameter returned,IEnumerable<CimMethodParameter> parameters) {
        if(returned!=null&&returned.Value!=null&&Convert.ToUInt32(returned.Value)!=0)throw new InvalidOperationException("VPN profile query failed");
        CimMethodParameter output=null;foreach(var parameter in parameters){if(String.Equals(parameter.Name,"cmdletOutput",StringComparison.OrdinalIgnoreCase))output=parameter;else throw new InvalidOperationException("VPN provider returned an unsupported output field");}
        // This provider returns no output parameters for a successful empty phonebook.
        if(output==null||output.Value==null)return;
        var instances=output.Value as CimInstance[];if(instances==null||instances.Length>50000)throw new InvalidOperationException("Invalid VPN profile collection");
        try {foreach(var instance in instances){if(instance==null)throw new InvalidOperationException("VPN provider returned an empty profile");rows.Add(VpnItem(instance));}}
        finally{foreach(var instance in instances)if(instance!=null)instance.Dispose();}
    }
    static List<object> RemoteAccess() {
        RequireStateIntegrity();
        var rows=Configurations("CurrentUser","Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",0,"LocalMachine","SYSTEM\\CurrentControlSet\\Control\\Terminal Server",0);
        const string ns="root/Microsoft/Windows/RemoteAccess/Client";
        using(var options=new CimOperationOptions{Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions{Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            using(var schema=session.GetClass(ns,"PS_VpnConnection",options)) {
                var method=schema.CimClassMethods["Get"];
                if(method==null||method.Parameters["cmdletOutput"]==null||method.Parameters["cmdletOutput"].CimType!=CimType.InstanceArray)throw new InvalidOperationException("Unsupported VPN provider result schema");
            }
            var parameters=new CimMethodParametersCollection();
            parameters.Add(CimMethodParameter.Create("Name",null,CimType.StringArray,CimFlags.In));
            parameters.Add(CimMethodParameter.Create("AllUserConnection",false,CimFlags.In));
            // The system provider's fixed Get method reads profile metadata only.
            using(var result=session.InvokeMethod(ns,"PS_VpnConnection","Get",parameters,options)) {
                AddVpnResult(rows,result.ReturnValue,result.OutParameters);
            }
        }
        return rows;
    }
    static void CollectRemoteAccess(string unit){if(unit=="network")Emit("pcscope.network.access",RemoteAccess);}
    static void VpnContractTest() {
        using(var instance=new CimInstance("VpnConnection")) {
            instance.CimInstanceProperties.Add(CimProperty.Create("Name","隔离样本",CimFlags.None));
            instance.CimInstanceProperties.Add(CimProperty.Create("TunnelType","Ikev2",CimFlags.None));
            instance.CimInstanceProperties.Add(CimProperty.Create("ConnectionStatus","Disconnected",CimFlags.None));
            instance.CimInstanceProperties.Add(CimProperty.Create("SplitTunneling",true,CimFlags.None));
            instance.CimInstanceProperties.Add(CimProperty.Create("AllUserConnection",false,CimFlags.None));
            instance.CimInstanceProperties.Add(CimProperty.Create("AuthenticationMethod",new[]{"Eap","MsChapv2"},CimType.StringArray,CimFlags.None));
            instance.CimInstanceProperties.Add(CimProperty.Create("Password","fixture-secret",CimFlags.None));
            object item=VpnItem(instance),data=Get(item,"data");
            if(Text(Get(item,"id"))!="隔离样本"||Text(Get(data,"TunnelType"))!="Ikev2"||Text(Get(data,"ConnectionStatus"))!="Disconnected"||!(bool)Get(data,"SplitTunneling"))throw new Exception("VPN field mapping mismatch");
            var methods=Get(data,"AuthenticationMethod") as object[];if(methods==null||methods.Length!=2||Text(methods[0])!="Eap"||Text(methods[1])!="MsChapv2")throw new Exception("VPN authentication array mismatch");
            if(!(bool)Get(Get(Get(data,"source_properties"),"Password"),"sensitive"))throw new Exception("VPN source field protection mismatch");
            var results=new List<object>();
            AddVpnResult(results,null,new CimMethodParametersCollection());if(results.Count!=0)throw new Exception("Empty VPN method result mapping mismatch");
            var parameters=new CimMethodParametersCollection();parameters.Add(CimMethodParameter.Create("cmdletOutput",new[]{new CimInstance(instance)},CimType.InstanceArray,CimFlags.Out));
            AddVpnResult(results,CimMethodParameter.Create("ReturnValue",(uint)0,CimFlags.Out),parameters);if(results.Count!=1||Text(Get(results[0],"id"))!="隔离样本")throw new Exception("VPN provider result array mapping mismatch");
            bool failed=false;try{AddVpnResult(new List<object>(),CimMethodParameter.Create("ReturnValue",(uint)5,CimFlags.Out),new CimMethodParametersCollection());}catch(InvalidOperationException){failed=true;}if(!failed)throw new Exception("VPN failure code accepted");
            instance.CimInstanceProperties["AllUserConnection"].Value=true;bool rejected=false;try{VpnItem(instance);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("VPN scope expansion was accepted");
            Console.WriteLine(Json.Serialize(D("metadata_fields",true,"authentication_array",true,"source_protection",true,"provider_result_array",true,"empty_result",true,"failure_code_rejected",true,"unexpected_global_scope_rejected",true,"no_profile_created",true)));
        }
    }
}
