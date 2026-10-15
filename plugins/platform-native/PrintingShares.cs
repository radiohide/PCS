using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Management.Infrastructure;
using Microsoft.Management.Infrastructure.Options;

partial class Platform {
    [Flags]enum PrintStatus:uint {Normal=0,Paused=1,Error=2,Deleting=4,Spooling=8,Printing=16,Offline=32,PaperOut=64,Printed=128,Deleted=256,Blocked=512,UserIntervention=1024,Restarted=2048,Complete=4096,Retained=8192,RenderingLocally=16384}
    static object PrintJobItem(string printer,StateRow job) {
        object submitted=WmiProperty(job,"SubmittedTime");uint code=Convert.ToUInt32(job["JobStatus"]);string status=job["JobStatus"]==null||(code&~32767u)!=0?"":((PrintStatus)code).ToString();
        return Item(printer+"/"+Text(job["ID"])+"/"+Text(submitted),D("printer",printer,"id",job["ID"],"submitted_at",submitted,"status",status,"size",job["Size"],"total_pages",job["TotalPages"],"pages_printed",job["PagesPrinted"],"document_name",EvidenceText(Text(job["DocumentName"])),"document_content_collected",false,"content_exclusion_reason","print_metadata_only_no_document_content"));
    }
    static void MethodSuccess(CimMethodResult result) {
        if(Convert.ToInt64(result.ReturnValue.Value)!=0)throw new Exception("Native read-only provider operation failed: "+Text(result.ReturnValue.Value));
    }
    static List<object> PrintJobs() {
        RequireStateIntegrity();var rows=new List<object>();var clock=System.Diagnostics.Stopwatch.StartNew();const string ns="root/StandardCimv2";
        using(var options=new CimOperationOptions {Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions {Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            foreach(var printer in session.QueryInstances(ns,"WQL","SELECT * FROM MSFT_Printer",options))using(printer) {
                if(clock.Elapsed>TimeSpan.FromSeconds(35))throw new Exception("Print queue read exceeds source budget; incomplete source discarded");
                string name=Text(new StateRow(printer)["Name"]);var parameters=new CimMethodParametersCollection();parameters.Add(CimMethodParameter.Create("PrinterName",name,CimFlags.In));
                using(var result=session.InvokeMethod(ns,"MSFT_PrintJob","GetByName",parameters,options)) {
                    MethodSuccess(result);var jobs=result.OutParameters["CmdletOutput"].Value as IEnumerable;if(jobs==null)continue;
                    foreach(CimInstance job in jobs)using(job){if(rows.Count>=50000||clock.Elapsed>TimeSpan.FromSeconds(35))throw new Exception("Print queue read exceeds source budget");rows.Add(PrintJobItem(name,new StateRow(job)));}
                }
            }
        }
        return rows;
    }
    static List<object> Shares() {
        RequireStateIntegrity();var rows=new List<object>();var clock=System.Diagnostics.Stopwatch.StartNew();const string ns="root/Microsoft/Windows/SMB";
        using(var options=new CimOperationOptions {Timeout=TimeSpan.FromSeconds(15)})using(var connection=new DComSessionOptions {Culture=CultureInfo.GetCultureInfo("en-US"),UICulture=CultureInfo.GetCultureInfo("en-US")})using(var session=CimSession.Create(null,connection)) {
            foreach(var share in session.QueryInstances(ns,"WQL","SELECT * FROM MSFT_SmbShare",options))using(share) {
                if(rows.Count>=10000||clock.Elapsed>TimeSpan.FromSeconds(35))throw new Exception("Share read exceeds source budget; incomplete source discarded");var row=new StateRow(share);var access=new List<object>();
                using(var result=session.InvokeMethod(ns,share,"GetAccessControlEntries",new CimMethodParametersCollection(),options)) {
                    MethodSuccess(result);var entries=result.OutParameters["Output"].Value as IEnumerable;
                    if(entries!=null)foreach(CimInstance entry in entries)using(entry){if(access.Count>=50000)throw new Exception("Share ACL exceeds source budget");access.Add(WmiFields(new StateRow(entry),new[]{"AccountName","AccessControlType","AccessRight"}));}
                }
                rows.Add(Item(Text(row["Name"]),D("name",row["Name"],"path",row["Path"],"description",row["Description"],"encrypt_data",row["EncryptData"],"special",row["Special"],"access",access.ToArray())));
            }
        }
        return rows;
    }
    static void CollectPrintingShares(string unit) {
        if(unit=="desktop") {
            Emit("pcscope.desktop.printing",()=>WmiItems("MSFT_Printer","Name",new[]{"Name","Type","DriverName","PortName","Shared","ShareName","Published","PrinterStatus","RenderingMode"},"root\\StandardCimv2"));
            Emit("pcscope.desktop.printer-drivers",()=>ReadWmi("root\\StandardCimv2","MSFT_PrinterDriver",row=>Item(Text(row["Name"])+"/"+Text(row["PrinterEnvironment"]),WmiFields(row,new[]{"Name","Manufacturer","PrinterEnvironment","MajorVersion","DriverPath","InfPath","ConfigFile","DataFile"}))));
            Emit("pcscope.desktop.printer-ports",()=>WmiItems("MSFT_PrinterPort","Name",new[]{"Name","Description","PortMonitor","PrinterHostAddress","PortNumber","Protocol","SNMPEnabled"},"root\\StandardCimv2"));
            Emit("pcscope.desktop.print-jobs",PrintJobs);
        }
        if(unit=="network")Emit("pcscope.network.shares",Shares);
    }
}
