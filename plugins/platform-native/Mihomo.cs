using System;
using System.IO;
using System.Net;
using System.Text;
using System.Collections.Generic;

partial class Platform {
    static object MihomoRead(int port,string route) {
        var clock=System.Diagnostics.Stopwatch.StartNew();
        var request=(HttpWebRequest)WebRequest.Create("http://127.0.0.1:"+port+route);request.Method="GET";request.AllowAutoRedirect=false;request.Proxy=null;request.Timeout=5000;request.ReadWriteTimeout=5000;
        string token=Environment.GetEnvironmentVariable("PCSCOPE_MIHOMO_TOKEN");if(!String.IsNullOrEmpty(token))request.Headers[HttpRequestHeader.Authorization]="Bearer "+token;
        using(var response=(HttpWebResponse)request.GetResponse()){
            if(response.StatusCode!=HttpStatusCode.OK)throw new Exception("Local Mihomo API rejected read: "+(int)response.StatusCode);
            using(var stream=response.GetResponseStream())using(var buffer=new MemoryStream()){
                byte[] bytes=new byte[8192];int count;while((count=stream.Read(bytes,0,bytes.Length))>0){if(clock.Elapsed>TimeSpan.FromSeconds(5))throw new Exception("Mihomo response exceeded read time budget");if(buffer.Length+count>1048576)throw new Exception("Mihomo response exceeds bounded read policy");buffer.Write(bytes,0,count);}
                object value=Json.DeserializeObject(Encoding.UTF8.GetString(buffer.ToArray()));CheckJsonKeys(value);return value;
            }
        }
    }
    static List<object> Mihomo(int port) {
        RequireStateIntegrity();if(port<1||port>65535)throw new Exception("Invalid Mihomo loopback port");object version=MihomoRead(port,"/version"),config=MihomoRead(port,"/configs");
        return new List<object>{Item("runtime",D("version",JsonField(version,"version"),"meta",JsonField(version,"meta"),"mode",JsonField(config,"mode"),"log_level",JsonField(config,"log-level"),"ipv6",JsonField(config,"ipv6"),"allow_lan",JsonField(config,"allow-lan"),"port",JsonField(config,"port"),"socks_port",JsonField(config,"socks-port"),"mixed_port",JsonField(config,"mixed-port"),"tun_enabled",JsonField(JsonField(config,"tun"),"enable"),"basis","explicit local Mihomo API; no subscription, node credentials or traffic content"))};
    }
    static void CollectMihomo(string unit) {
        if(unit!="scoped"||!Enabled("pcscope.apps.mihomo"))return;object options=Options();if(Get(options,"mihomo_metadata") is bool&&(bool)Get(options,"mihomo_metadata"))Emit("pcscope.apps.mihomo",()=>Mihomo(Convert.ToInt32(Get(options,"mihomo_port"))));
    }
}
