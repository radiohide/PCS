using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.CompilerServices;
using Windows.Foundation.Metadata;
using Windows.Management.Deployment;

partial class Platform {
    // Layout and entry point match the installed DISM interop; private API availability is checked by loading it.
    [StructLayout(LayoutKind.Sequential,Pack=1)] struct ProvisionedPackage {
        public IntPtr name,display,publisher;public uint major,minor,build,revision,architecture;public IntPtr resource,location,regions;
    }
    [DllImport("dismapi.dll",EntryPoint="_DismGetProvisionedAppxPackages")]static extern int DismProvisionedPackages(uint session,out IntPtr packages,out uint count);
    static object ProvisionedItem(ProvisionedPackage package) {
        string name=Marshal.PtrToStringUni(package.name);if(String.IsNullOrWhiteSpace(name))throw new InvalidOperationException("Provisioned package has no identity");
        string version=String.Format(CultureInfo.InvariantCulture,"{0}.{1}.{2}.{3}",package.major,package.minor,package.build,package.revision);
        return Item(name,D("DisplayName",EvidenceValue("DisplayName",Marshal.PtrToStringUni(package.display)),"Version",version,"Architecture",package.architecture,"PackageName",EvidenceValue("PackageName",name),"InstallLocation",EvidenceValue("InstallLocation",Marshal.PtrToStringUni(package.location))));
    }
    static List<object> ProvisionedPackages() {
        RequireStateIntegrity();ProvisionedLayout();if(!Elevated())throw new UnauthorizedAccessException("Reading online provisioned applications requires administrator authorization");
        // The public provisioned-package API exists from build 19041. Keep the older DISM
        // backend for earlier LTSC systems; an API failure is not an empty inventory or PS fallback.
        if(ApiInformation.IsMethodPresent("Windows.Management.Deployment.PackageManager","FindProvisionedPackages"))return ProvisionedPackagesWinRt();
        return ProvisionedPackagesDism();
    }
    [MethodImpl(MethodImplOptions.NoInlining)]
    static List<object> ProvisionedPackagesWinRt() {
        var rows=new List<object>();var manager=new PackageManager();
        foreach(var package in manager.FindProvisionedPackages()) {
            if(rows.Count>=50000)throw new InvalidOperationException("Provisioned package enumeration exceeds budget");
            var id=package.Id;var version=id.Version;var location=package.InstalledLocation;
            rows.Add(ProvisionedWinRtItem(id.FullName,id.Name,version.Major,version.Minor,version.Build,version.Revision,(uint)id.Architecture,location==null?null:location.Path));
        }
        return rows;
    }
    static object ProvisionedWinRtItem(string fullName,string name,uint major,uint minor,uint build,uint revision,uint architecture,string location) {
        if(String.IsNullOrWhiteSpace(fullName)||String.IsNullOrWhiteSpace(name))throw new InvalidOperationException("Provisioned package has no identity or deployment name");
        // DISM DisplayName is the deployment identity name, not the localized package title.
        return Item(fullName,D("DisplayName",EvidenceValue("DisplayName",name),"Version",String.Format(CultureInfo.InvariantCulture,"{0}.{1}.{2}.{3}",major,minor,build,revision),"Architecture",architecture,"PackageName",EvidenceValue("PackageName",fullName),"InstallLocation",EvidenceValue("InstallLocation",location)));
    }
    static List<object> ProvisionedPackagesDism() {
        string log=Path.Combine(Path.GetTempPath(),"pcscope-native-provisioned-"+Guid.NewGuid().ToString("N")+".log");
        HResult(DismInitialize(0,log,null));uint session=0;bool opened=false;IntPtr buffer=IntPtr.Zero;
        try {
            HResult(DismOpenSession("DISM_{53BFAE52-B167-4E2F-A258-0A37B57FF845}",null,null,out session));opened=true;
            uint count;HResult(DismProvisionedPackages(session,out buffer,out count));if(count>50000||(count>0&&buffer==IntPtr.Zero))throw new InvalidOperationException("Invalid provisioned package result size");
            var rows=new List<object>();int size=Marshal.SizeOf(typeof(ProvisionedPackage));
            for(uint index=0;index<count;index++)rows.Add(ProvisionedItem((ProvisionedPackage)Marshal.PtrToStructure(IntPtr.Add(buffer,checked((int)index*size)),typeof(ProvisionedPackage))));
            return rows;
        }finally{if(buffer!=IntPtr.Zero)DismDelete(buffer);if(opened)DismCloseSession(session);DismShutdown();}
    }
    static void ProvisionedLayout(){if(Marshal.SizeOf(typeof(ProvisionedPackage))!=68||Marshal.OffsetOf(typeof(ProvisionedPackage),"architecture").ToInt64()!=40||Marshal.OffsetOf(typeof(ProvisionedPackage),"location").ToInt64()!=52)throw new InvalidOperationException("DISM provisioned package packed layout mismatch");}
    static void ProvisionedContractTest() {
        ProvisionedLayout();int size=Marshal.SizeOf(typeof(ProvisionedPackage));IntPtr buffer=Marshal.AllocHGlobal(size*2),name=Marshal.StringToHGlobalUni("Fixture_1.2.3.4_x64__fixture"),second=Marshal.StringToHGlobalUni("Second_1.2.3.5_neutral__fixture"),display=Marshal.StringToHGlobalUni("应用样本"),location=Marshal.StringToHGlobalUni(@"C:\Fixture\应用");
        try {
            Marshal.StructureToPtr(new ProvisionedPackage{name=name,display=display,major=1,minor=2,build=3,revision=4,architecture=9},buffer,false);
            Marshal.StructureToPtr(new ProvisionedPackage{name=second,major=1,minor=2,build=3,revision=5,architecture=11,location=location},IntPtr.Add(buffer,size),false);
            var identities=new HashSet<string>();
            for(int i=0;i<2;i++) {
                var row=(ProvisionedPackage)Marshal.PtrToStructure(IntPtr.Add(buffer,size*i),typeof(ProvisionedPackage));object item=ProvisionedItem(row),data=Get(item,"data");
                string expectedName=i==0?"Fixture_1.2.3.4_x64__fixture":"Second_1.2.3.5_neutral__fixture";
                var expected=D("DisplayName",i==0?(object)"应用样本":null,"Version","1.2.3."+(4+i),"Architecture",i==0?9U:11U,"PackageName",expectedName,"InstallLocation",i==0?null:(object)@"C:\Fixture\应用");
                if(Text(Get(item,"id"))!=expectedName||!identities.Add(expectedName)||Json.Serialize(data)!=Json.Serialize(expected))throw new Exception("Provisioned package complete field/identity array mismatch");
            }
            bool rejected=false;try{ProvisionedItem(new ProvisionedPackage());}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Provisioned package missing identity accepted");
            object modern=ProvisionedWinRtItem("Fixture_1.2.3.4_x64__fixture","Fixture",1,2,3,4,9,@"C:\Fixture\应用");
            var modernExpected=D("DisplayName","Fixture","Version","1.2.3.4","Architecture",9U,"PackageName","Fixture_1.2.3.4_x64__fixture","InstallLocation",@"C:\Fixture\应用");
            if(Json.Serialize(Get(modern,"data"))!=Json.Serialize(modernExpected))throw new Exception("Public provisioned package five-field contract mismatch");
            rejected=false;try{ProvisionedWinRtItem("","Fixture",1,2,3,4,9,null);}catch(InvalidOperationException){rejected=true;}if(!rejected)throw new Exception("Public provisioned package missing identity accepted");
            Console.WriteLine(Json.Serialize(D("packed_size",size,"packed_array",true,"complete_fields",5,"distinct_package_identity",true,"version_string",true,"numeric_architecture",true,"nullable_display_and_location",true,"unicode_location",true,"missing_identity_rejected",true,"no_package_changes",true)));
        }finally{Marshal.FreeHGlobal(location);Marshal.FreeHGlobal(display);Marshal.FreeHGlobal(second);Marshal.FreeHGlobal(name);Marshal.FreeHGlobal(buffer);}
    }
    static void CollectProvisionedPackages(string unit){if(unit=="system")Emit("pcscope.software.provisioned",ProvisionedPackages);}
}
