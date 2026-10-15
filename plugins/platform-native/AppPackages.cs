using System;
using System.Collections.Generic;
using System.Globalization;
using System.Runtime.InteropServices;
using Windows.ApplicationModel;
using Windows.Management.Deployment;

partial class Platform {
    [DllImport("appxalluserstore.dll",CharSet=CharSet.Unicode)]static extern int IsPackageFamilyInUninstallBlocklist(string family,[MarshalAs(UnmanagedType.Bool)]out bool blocked);
    static uint AppStatusBits(bool[] flags){if(flags==null||flags.Length!=12)throw new InvalidOperationException("Package status field layout mismatch");uint value=0;for(int i=0;i<flags.Length;i++)if(flags[i])value|=1u<<i;return value;}
    static uint AppPackageStatus(Windows.ApplicationModel.PackageStatus status) {
        if(status==null)throw new InvalidOperationException("Package status unavailable");
        return AppStatusBits(new[]{status.LicenseIssue,status.Modified,status.Tampered,status.Disabled,status.PackageOffline,status.DeploymentInProgress,status.DependencyIssue,status.DataOffline,status.IsPartiallyStaged,status.NotAvailable,status.Servicing,status.NeedsRemediation});
    }
    static List<object> AppUserPackages() {
        RequireStateIntegrity();var manager=new PackageManager();var rows=new List<object>();
        foreach(Package package in manager.FindPackagesForUserWithPackageTypes(UserSid(),PackageTypes.Main|PackageTypes.Framework)) {
            if(rows.Count>=50000)throw new InvalidOperationException("Installed package enumeration exceeds budget");
            var id=package.Id;string full=id.FullName,family=id.FamilyName;if(String.IsNullOrWhiteSpace(full)||String.IsNullOrWhiteSpace(family))throw new InvalidOperationException("Installed package has no identity");
            bool blocked=package.SignatureKind==PackageSignatureKind.System;if(!blocked)HResult(IsPackageFamilyInUninstallBlocklist(family,out blocked));
            var version=id.Version;string versionText=String.Format(CultureInfo.InvariantCulture,"{0}.{1}.{2}.{3}",version.Major,version.Minor,version.Build,version.Revision);
            var location=package.InstalledLocation;
            rows.Add(Item(full,D("Name",EvidenceText(id.Name),"PackageFullName",EvidenceText(full),"PackageFamilyName",EvidenceText(family),"Version",versionText,"Architecture",(uint)id.Architecture,"Publisher",EvidenceText(id.Publisher),"InstallLocation",location==null?"":EvidenceText(location.Path),"IsFramework",package.IsFramework,"IsBundle",package.IsBundle,"NonRemovable",blocked,"SignatureKind",(uint)package.SignatureKind,"Status",AppPackageStatus(package.Status))));
        }
        return rows;
    }
    static void CollectAppPackages(string unit){if(unit=="system")Emit("pcscope.software.appx-user",AppUserPackages);}
    static void AppPackageContractTest() {
        for(int i=0;i<12;i++){var flags=new bool[12];flags[i]=true;if(AppStatusBits(flags)!=(1u<<i))throw new Exception("App package status bit mismatch");}
        if(AppStatusBits(new bool[12])!=0||AppStatusBits(new[]{true,true,true,false,false,false,false,false,false,false,false,true})!=2055)throw new Exception("App package combined status mismatch");
        bool rejected=false;try{AppStatusBits(new bool[11]);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Incomplete package status accepted");
        Console.WriteLine(Json.Serialize(D("individual_status_flags",12,"combined_flags",true,"normal_flags",true,"incomplete_status_rejected",true,"no_app_changes",true)));
    }
}
