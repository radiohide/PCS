using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.IO.Pipes;
using System.Text;
using System.Text.RegularExpressions;

partial class Platform {
    static string DockerFixturePipe;
    sealed class DockerReader {
        readonly Stream stream;readonly Stopwatch clock;readonly byte[] buffer=new byte[8192];int offset,available;
        internal DockerReader(Stream value,Stopwatch budget){stream=value;clock=budget;}
        internal int Byte() {
            if(offset==available){
                int remaining=5000-(int)clock.ElapsedMilliseconds;if(remaining<=0)throw new TimeoutException("Docker read exceeded time budget");
                IAsyncResult read=stream.BeginRead(buffer,0,buffer.Length,null,null);
                using(var wait=read.AsyncWaitHandle){if(!wait.WaitOne(remaining)){stream.Dispose();throw new TimeoutException("Docker read exceeded time budget");}available=stream.EndRead(read);}
                offset=0;if(available==0)return -1;
            }
            return buffer[offset++];
        }
        internal string Line() {
            var bytes=new List<byte>();int previous=-1;
            for(;;){int value=Byte();if(value<0)throw new EndOfStreamException("Incomplete Docker HTTP header");if(previous==13&&value==10){bytes.RemoveAt(bytes.Count-1);return Encoding.ASCII.GetString(bytes.ToArray());}bytes.Add((byte)value);previous=value;if(bytes.Count>8192)throw new InvalidDataException("Docker HTTP header line exceeds limit");}
        }
        internal void Body(MemoryStream result,int count) {
            if(count<0||result.Length+count>8388608)throw new InvalidDataException("Docker response exceeds bounded read policy");
            for(int i=0;i<count;i++){int value=Byte();if(value<0)throw new EndOfStreamException("Incomplete Docker response");result.WriteByte((byte)value);}
        }
    }
    static object DockerRead(string pipe,string route) {
        var clock=Stopwatch.StartNew();
        using(var client=new NamedPipeClientStream(".",pipe,PipeDirection.InOut,PipeOptions.Asynchronous,System.Security.Principal.TokenImpersonationLevel.Identification)) {
            client.Connect(3000);
            byte[] request=Encoding.ASCII.GetBytes("GET "+route+" HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nAccept: application/json\r\n\r\n");
            IAsyncResult write=client.BeginWrite(request,0,request.Length,null,null);using(var wait=write.AsyncWaitHandle){if(!wait.WaitOne(Math.Max(1,5000-(int)clock.ElapsedMilliseconds))){client.Dispose();throw new TimeoutException("Docker request exceeded time budget");}client.EndWrite(write);}
            var reader=new DockerReader(client,clock);string status=reader.Line();
            if(!Regex.IsMatch(status,@"^HTTP/1\.[01] 200(?: |$)"))throw new InvalidDataException("Local Docker API rejected read");
            var headers=new Dictionary<string,string>(StringComparer.OrdinalIgnoreCase);int total=0;
            for(;;){string line=reader.Line();total+=line.Length+2;if(total>32768)throw new InvalidDataException("Docker HTTP headers exceed limit");if(line.Length==0)break;int colon=line.IndexOf(':');if(colon<=0)throw new InvalidDataException("Malformed Docker HTTP header");string key=line.Substring(0,colon).Trim();if(headers.ContainsKey(key))throw new InvalidDataException("Duplicate Docker HTTP header");headers.Add(key,line.Substring(colon+1).Trim());}
            using(var body=new MemoryStream()) {
                string transfer,length;
                if(headers.TryGetValue("Transfer-Encoding",out transfer)){
                    if(!String.Equals(transfer,"chunked",StringComparison.OrdinalIgnoreCase)||headers.ContainsKey("Content-Length"))throw new InvalidDataException("Unsupported Docker HTTP framing");
                    for(;;){string size=reader.Line();int semicolon=size.IndexOf(';');if(semicolon>=0)size=size.Substring(0,semicolon);uint count;if(!UInt32.TryParse(size,NumberStyles.AllowHexSpecifier,CultureInfo.InvariantCulture,out count)||count>8388608)throw new InvalidDataException("Invalid Docker chunk length");if(count==0){int trailers=0;while(reader.Line().Length>0)if(++trailers>32)throw new InvalidDataException("Docker trailers exceed limit");break;}reader.Body(body,(int)count);if(reader.Byte()!=13||reader.Byte()!=10)throw new InvalidDataException("Malformed Docker chunk terminator");}
                }else if(headers.TryGetValue("Content-Length",out length)){
                    int count;if(!Int32.TryParse(length,NumberStyles.None,CultureInfo.InvariantCulture,out count))throw new InvalidDataException("Invalid Docker content length");reader.Body(body,count);
                }else{int nextByte;while((nextByte=reader.Byte())>=0){if(body.Length>=8388608)throw new InvalidDataException("Docker response exceeds bounded read policy");body.WriteByte((byte)nextByte);}}
                try {object value=Json.DeserializeObject(new UTF8Encoding(false,true).GetString(body.ToArray()));CheckJsonKeys(value);return value;}catch(Exception){throw new InvalidDataException("Invalid Docker JSON response");}
            }
        }
    }
    static object[] DockerArray(object value) {var array=value as object[];if(array==null||array.Length>50000)throw new InvalidDataException("Invalid Docker collection result");return array;}
    static string DockerText(object row,string field) {object value=JsonField(row,field);if(!(value is string))throw new InvalidDataException("Missing Docker metadata field: "+field);return (string)value;}
    static string DockerRepository(string repository) {
        if(repository.StartsWith("docker.io/",StringComparison.Ordinal))repository=repository.Substring(10);
        else if(repository.StartsWith("index.docker.io/",StringComparison.Ordinal))repository=repository.Substring(16);
        if(repository.StartsWith("library/",StringComparison.Ordinal)&&repository.IndexOf('/',8)<0)repository=repository.Substring(8);
        return repository;
    }
    static List<object> DockerItems(string pipe) {
        RequireStateIntegrity();object version=DockerRead(pipe,"/version");Version server,minimum;
        if(!Version.TryParse(DockerText(version,"ApiVersion"),out server)||server.Major!=1)throw new InvalidDataException("Unsupported Docker API version");
        string min=Text(JsonField(version,"MinAPIVersion"));minimum=String.IsNullOrEmpty(min)?new Version(1,24):new Version(min);
        Version selected=server<new Version(1,55)?server:new Version(1,55);if(selected<minimum||selected<new Version(1,24))throw new InvalidDataException("No compatible Docker API version");
        string prefix="/v"+selected.ToString(2);var rows=new List<object>();
        Action<string,string,object> add=(kind,id,data)=>{if(String.IsNullOrWhiteSpace(id))throw new InvalidDataException("Docker object has no identity");if(rows.Count>=50000)throw new InvalidDataException("Docker object budget exceeded");rows.Add(Item(kind+"/"+id,D("kind",kind,"data",data,"basis","local Docker API metadata; no logs, arguments, environment or container contents")));};
        foreach(object row in DockerArray(DockerRead(pipe,prefix+"/containers/json?all=1"))) {
            string id=DockerText(row,"Id");var names=new List<string>();foreach(object name in DockerArray(JsonField(row,"Names"))){if(!(name is string))throw new InvalidDataException("Invalid Docker container name");string text=(string)name;names.Add(text.StartsWith("/",StringComparison.Ordinal)?text.Substring(1):text);}
            string image=DockerText(row,"Image");add("container",id,D("id",id,"names",String.Join(",",names),"image",image.Length==0?"<no image>":image,"state",DockerText(row,"State"),"status",DockerText(row,"Status")));
        }
        foreach(object row in DockerArray(DockerRead(pipe,prefix+"/images/json"))) {
            string id=DockerText(row,"Id");object tagsValue=JsonField(row,"RepoTags"),digestsValue=JsonField(row,"RepoDigests");object[] tags=tagsValue==null?new object[0]:DockerArray(tagsValue),digests=digestsValue==null?new object[0]:DockerArray(digestsValue);
            var references=new Dictionary<string,HashSet<string>>(StringComparer.Ordinal);var digestRepositories=new HashSet<string>(StringComparer.Ordinal);
            foreach(object tag in tags){string text=tag as string;if(text=="<none>:<none>")continue;if(text==null)throw new InvalidDataException("Invalid Docker image tag");int colon=text.LastIndexOf(':');if(colon<=text.LastIndexOf('/')||colon<=0)throw new InvalidDataException("Invalid Docker image tag");string repo=DockerRepository(text.Substring(0,colon)),label=text.Substring(colon+1);HashSet<string> labels;if(!references.TryGetValue(repo,out labels)){labels=new HashSet<string>(StringComparer.Ordinal);references.Add(repo,labels);}labels.Add(label);}
            foreach(object digest in digests){string text=digest as string;if(text=="<none>@<none>")continue;if(text==null||text.IndexOf('@')<=0)throw new InvalidDataException("Invalid Docker image digest");digestRepositories.Add(DockerRepository(text.Substring(0,text.IndexOf('@'))));}
            foreach(var pair in references){digestRepositories.Remove(pair.Key);foreach(string tag in pair.Value)add("image",id+"/"+pair.Key+"/"+tag,D("id",id,"repository",pair.Key,"tag",tag));}
            foreach(string repo in digestRepositories)add("image",id+"/"+repo+"/<none>",D("id",id,"repository",repo,"tag","<none>"));
            if(references.Count==0&&digestRepositories.Count==0)add("image",id+"/<none>/<none>",D("id",id,"repository","<none>","tag","<none>"));
        }
        object volumes=DockerRead(pipe,prefix+"/volumes");object volumeRows=JsonField(volumes,"Volumes");if(!(volumes is Dictionary<string,object>)||!((Dictionary<string,object>)volumes).ContainsKey("Volumes"))throw new InvalidDataException("Invalid Docker volume result");
        foreach(object row in volumeRows==null?new object[0]:DockerArray(volumeRows)){string id=DockerText(row,"Name");add("volume",id,D("id",id,"driver",DockerText(row,"Driver")));}
        foreach(object row in DockerArray(DockerRead(pipe,prefix+"/networks"))){string id=DockerText(row,"Id");add("network",id,D("id",id,"name",DockerText(row,"Name"),"driver",DockerText(row,"Driver"),"scope",DockerText(row,"Scope")));}
        return rows;
    }
    static void CollectDocker(string unit) {
        if(unit!="scoped"||!Enabled("pcscope.virtualization.docker"))return;object options=Options();if(Get(options,"docker_metadata") is bool&&(bool)Get(options,"docker_metadata"))Emit("pcscope.virtualization.docker",()=>DockerItems(DockerFixturePipe??"docker_engine"));
    }
}
