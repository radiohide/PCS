using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

// Read-only identities and CNG key names. No key is opened, exported, created or deleted.
public static class PCScopeMetadata {
    [StructLayout(LayoutKind.Sequential)] struct FileInfo {
        public uint Attributes, CreationLow, CreationHigh, AccessLow, AccessHigh, WriteLow, WriteHigh;
        public uint VolumeSerial, SizeHigh, SizeLow, Links, IndexHigh, IndexLow;
    }
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern SafeFileHandle CreateFile(string path, uint access, uint share, IntPtr security, uint creation, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetFileInformationByHandle(SafeFileHandle file, out FileInfo info);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool GetVolumePathName(string path, StringBuilder mount, uint size);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool GetVolumeNameForVolumeMountPoint(string mount, StringBuilder name, uint size);
    public static string[] FileIdentity(string path) {
        using (SafeFileHandle file = CreateFile(path, 0, 7, IntPtr.Zero, 3, 0x02200000, IntPtr.Zero)) {
            if (file.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
            FileInfo info;
            if (!GetFileInformationByHandle(file, out info)) throw new Win32Exception(Marshal.GetLastWin32Error());
            StringBuilder mount = new StringBuilder(1024), volume = new StringBuilder(1024);
            if (!GetVolumePathName(path, mount, 1024) || !GetVolumeNameForVolumeMountPoint(mount.ToString(), volume, 1024)) throw new Win32Exception(Marshal.GetLastWin32Error());
            return new string[] { volume.ToString().ToLowerInvariant(), (((ulong)info.IndexHigh << 32) | info.IndexLow).ToString("x16") };
        }
    }
    [StructLayout(LayoutKind.Sequential)] struct ProviderName { public IntPtr Name, Comment; }
    [StructLayout(LayoutKind.Sequential)] struct KeyName { public IntPtr Name, Algorithm; public uint LegacyKeySpec, Flags; }
    public sealed class KeyMetadata { public string Name, Algorithm; public uint LegacyKeySpec, Flags; }
    [DllImport("ncrypt.dll", CharSet=CharSet.Unicode)] static extern int NCryptEnumStorageProviders(out uint count, out IntPtr providers, uint flags);
    [DllImport("ncrypt.dll", CharSet=CharSet.Unicode)] static extern int NCryptOpenStorageProvider(out IntPtr handle, string provider, uint flags);
    [DllImport("ncrypt.dll", CharSet=CharSet.Unicode)] static extern int NCryptEnumKeys(IntPtr provider, string scope, out IntPtr name, ref IntPtr state, uint flags);
    [DllImport("ncrypt.dll")] static extern int NCryptFreeBuffer(IntPtr buffer);
    [DllImport("ncrypt.dll")] static extern int NCryptFreeObject(IntPtr handle);
    static void Check(int status) { if (status != 0) throw new InvalidOperationException("CNG enumeration failed: 0x" + ((uint)status).ToString("X8")); }
    public static string[] Providers() {
        uint count; IntPtr providers = IntPtr.Zero;
        try {
            Check(NCryptEnumStorageProviders(out count, out providers, 0x40));
            List<string> names = new List<string>();
            int size = Marshal.SizeOf(typeof(ProviderName));
            for (uint i=0; i<count; i++) {
                ProviderName row = (ProviderName)Marshal.PtrToStructure(IntPtr.Add(providers, checked((int)i * size)), typeof(ProviderName));
                names.Add(Marshal.PtrToStringUni(row.Name));
            }
            return names.ToArray();
        } finally { if (providers != IntPtr.Zero) NCryptFreeBuffer(providers); }
    }
    public static KeyMetadata[] Keys(string provider, bool machine) {
        IntPtr handle = IntPtr.Zero, state = IntPtr.Zero;
        try {
            Check(NCryptOpenStorageProvider(out handle, provider, 0));
            List<KeyMetadata> keys = new List<KeyMetadata>();
            while (true) {
                IntPtr name = IntPtr.Zero;
                int status = NCryptEnumKeys(handle, null, out name, ref state, 0x40u | (machine ? 0x20u : 0u));
                try {
                    if (unchecked((uint)status) == 0x8009002Au) break;
                    Check(status);
                    KeyName row = (KeyName)Marshal.PtrToStructure(name, typeof(KeyName));
                    keys.Add(new KeyMetadata { Name=Marshal.PtrToStringUni(row.Name), Algorithm=Marshal.PtrToStringUni(row.Algorithm), LegacyKeySpec=row.LegacyKeySpec, Flags=row.Flags });
                } finally { if (name != IntPtr.Zero) NCryptFreeBuffer(name); }
            }
            return keys.ToArray();
        } finally { if (state != IntPtr.Zero) NCryptFreeBuffer(state); if (handle != IntPtr.Zero) NCryptFreeObject(handle); }
    }
}
