using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Text.RegularExpressions;

partial class Platform {
    static object NullableNetworkValue(object value,bool array) {
        if(value==null)return "Any";
        if(!array)return String.IsNullOrEmpty(Text(value))?"Any":Text(value);
        var rows=new List<object>();foreach(object item in (IEnumerable)value)rows.Add(item);
        return rows.Count==0?"Any":rows.Count==1?rows[0]:(object)rows.ToArray();
    }
    static Dictionary<string,object> FirewallFields(StateRow row,string[] fields,string kind) {
        var data=WmiFields(row,fields);
        if(kind=="rule"||kind=="ipsec") {data["Name"]=row["InstanceID"];data["Profile"]=row["Profiles"]??(object)0;}
        if(kind=="rule")data["Status"]=Text(row["Status"])+" ("+Convert.ToInt64(row["StatusCode"])+")";
        if(kind=="profile"&&Text(row["LogMaxSizeKilobytes"])=="4294967296")data["LogMaxSizeKilobytes"]="NotConfigured";
        if(kind=="Port") {data["Protocol"]=NullableNetworkValue(row["Protocol"],false);data["DynamicTarget"]=row["DynamicTransport"]??(object)0;foreach(string field in new[]{"LocalPort","RemotePort","IcmpType"})data[field]=NullableNetworkValue(row[field],true);}
        if(kind=="Address")foreach(string field in new[]{"LocalAddress","RemoteAddress"})data[field]=NullableNetworkValue(row[field],true);
        if(kind=="Application")data["Program"]=NullableNetworkValue(row["AppPath"],false);
        if(kind=="Service")data["Service"]=NullableNetworkValue(row["ServiceName"],false);
        if(kind=="Interface")data["InterfaceAlias"]=NullableNetworkValue(row["InterfaceAlias"],true);
        if(kind=="Security")foreach(string field in new[]{"LocalUser","RemoteUser","RemoteMachine"})data[field]=NullableNetworkValue(row[field+"s"],false);
        return data;
    }
    static List<object> FirewallRules() {
        var clock=System.Diagnostics.Stopwatch.StartNew();var maps=new Dictionary<string,Dictionary<string,object>>();
        var classes=new Dictionary<string,string>{{"Port","MSFT_NetProtocolPortFilter"},{"Address","MSFT_NetAddressFilter"},{"Application","MSFT_NetApplicationFilter"},{"Service","MSFT_NetServiceFilter"},{"Interface","MSFT_NetInterfaceFilter"},{"InterfaceType","MSFT_NetInterfaceTypeFilter"},{"Security","MSFT_NetNetworkLayerSecurityFilter"}};
        var fields=new Dictionary<string,string[]>{{"Port",new[]{"Protocol","LocalPort","RemotePort","IcmpType","DynamicTarget"}},{"Address",new[]{"LocalAddress","RemoteAddress"}},{"Application",new[]{"Program","Package"}},{"Service",new[]{"Service"}},{"Interface",new[]{"InterfaceAlias"}},{"InterfaceType",new[]{"InterfaceType"}},{"Security",new[]{"Authentication","Encryption","OverrideBlockRules","LocalUser","RemoteUser","RemoteMachine"}}};
        var active=D("PolicyStore","ActiveStore");
        foreach(var entry in classes) {
            string kind=entry.Key;var map=new Dictionary<string,object>(StringComparer.OrdinalIgnoreCase);
            ReadWmi("root\\StandardCimv2",entry.Value,row=>{map[Text(row["InstanceID"])]=FirewallFields(row,fields[kind],kind);return D();},false,active);
            maps[kind]=map;if(clock.Elapsed>TimeSpan.FromSeconds(35))throw new Exception("Firewall source exceeds time budget; incomplete result discarded");
        }
        return ReadWmi("root\\StandardCimv2","MSFT_NetFirewallRule",row=>{
            if(clock.Elapsed>TimeSpan.FromSeconds(35))throw new Exception("Firewall source exceeds time budget; incomplete result discarded");
            string id=Text(row["InstanceID"]);var filters=new Dictionary<string,object>();
            foreach(string kind in classes.Keys){object filter;if(!maps[kind].TryGetValue(id,out filter))throw new Exception("Missing "+kind+" filter; rules changed during collection; previous state retained");filters[kind]=filter;}
            var data=FirewallFields(row,new[]{"Name","DisplayName","Enabled","Direction","Action","Profile","EdgeTraversalPolicy","PolicyStoreSource","PolicyStoreSourceType","PrimaryStatus","Status"},"rule");data["filters"]=filters;return Item(id,data);
        },false,active);
    }
    static List<object> DnsConfiguration() {
        var rows=ReadWmi("root\\StandardCimv2","MSFT_DNSClientServerAddress",row=>Item(Text(row["InterfaceIndex"])+"/"+Text(row["AddressFamily"]),WmiFields(row,new[]{"InterfaceIndex","InterfaceAlias","AddressFamily","ServerAddresses"})));
        rows.AddRange(Configuration("LocalMachine","SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters"));return rows;
    }
    static List<object> DnsCache() {
        return ReadWmi("root\\StandardCimv2","MSFT_DNSClientCache",DnsCacheItem);
    }
    static object DnsCacheItem(StateRow row) {
        string type=Text(row["Type"]);return Item(Text(row["Entry"])+"/"+Text(row["Name"])+"/"+type+"/"+Text(row["Data"]),D("entry",row["Entry"],"name",row["Name"],"type",type,"data",row["Data"],"status",Text(row["Status"]),"section",Text(row["Section"]),"basis","cache snapshot; not query time or complete DNS traffic"));
    }
    static List<object> Hosts() {
        RequireStateIntegrity();string path=Path.Combine(Environment.GetEnvironmentVariable("SystemRoot"),"System32\\drivers\\etc\\hosts");var file=new FileInfo(path);
        if(file.Length>1048576||(file.Attributes&FileAttributes.ReparsePoint)!=0)throw new Exception("Hosts file outside safe read policy");
        var rows=new List<object>();int number=0;foreach(string line in File.ReadAllLines(path,System.Text.Encoding.Default)){
            number++;string text=line.Split('#')[0].Trim();if(text.Length==0)continue;string[] parts=Regex.Split(text,@"\s+");IPAddress address;
            if(parts.Length<2||!IPAddress.TryParse(parts[0],out address))throw new Exception("Hosts syntax not recognized at line "+number);
            for(int i=1;i<parts.Length;i++)rows.Add(Item(number+"/"+parts[i],D("address",address.ToString(),"name",parts[i],"line",number,"basis","hosts configuration; comments excluded")));
        }
        return rows;
    }
    static void CollectNetworkConfiguration(string unit) {
        if(unit!="network")return;
        Emit("pcscope.network.firewall-profiles",()=>ReadWmi("root\\StandardCimv2","MSFT_NetFirewallProfile",row=>Item(Text(row["Name"]),FirewallFields(row,new[]{"Name","Enabled","DefaultInboundAction","DefaultOutboundAction","AllowInboundRules","AllowLocalFirewallRules","AllowLocalIPsecRules","LogAllowed","LogBlocked","LogFileName","LogMaxSizeKilobytes"},"profile"))));
        Emit("pcscope.network.firewall-rules",FirewallRules);
        Emit("pcscope.network.ipsec",()=>ReadWmi("root\\StandardCimv2","MSFT_NetConSecRule",row=>Item(Text(row["InstanceID"]),FirewallFields(row,new[]{"Name","DisplayName","Enabled","Profile","Mode","InboundSecurity","OutboundSecurity","PolicyStoreSource","PolicyStoreSourceType"},"ipsec")),false,D("PolicyStore","ActiveStore")));
        Emit("pcscope.network.dns-configuration",DnsConfiguration);
        Emit("pcscope.network.dns-cache",DnsCache);
        Emit("pcscope.network.hosts",Hosts);
    }
}
