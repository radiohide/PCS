param([switch]$Repair)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$admin=([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$result=[ordered]@{checked_at=[DateTime]::UtcNow.ToString('o');elevated=$admin;user_sid=$identity.User.Value;audits=@();logs=@();changes=@();history_warning='只能补采 Windows 仍保留的日志。检查通过不代表历史完整；未开启审计、覆盖、清空及采集前的历史均可能缺失。'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PCScopeAudit {
 [StructLayout(LayoutKind.Sequential)] public struct Policy { public Guid sub; public uint flags; public Guid category; }
 [DllImport("advapi32.dll", SetLastError=true)] [return:MarshalAs(UnmanagedType.U1)] static extern bool AuditQuerySystemPolicy([In] Guid[] ids, uint count, out IntPtr buffer);
 [DllImport("advapi32.dll")] static extern void AuditFree(IntPtr buffer);
 public static uint Query(string id) { IntPtr p; if(!AuditQuerySystemPolicy(new[]{new Guid(id)},1,out p)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error()); try{return ((Policy)Marshal.PtrToStructure(p,typeof(Policy))).flags;}finally{AuditFree(p);} }
}
'@
$policies=@(
 @('计划任务修改审计','0cce9227-69ae-11d9-bed3-505054503030','4698–4702：任务创建、删除、启停和修改账户'),
 @('进程创建审计','0cce922b-69ae-11d9-bed3-505054503030','4688：进程路径、进程 ID、父进程，辅助识别修改来源'),
 @('登录审计','0cce9215-69ae-11d9-bed3-505054503030','4624：关联修改账户的登录会话')
)
foreach($policy in $policies){
 $item=[ordered]@{name=$policy[0];guid=$policy[1];purpose=$policy[2];source='AuditQuerySystemPolicy';success_enabled=$null;error=$null}
 try{
  $flags=[PCScopeAudit]::Query($policy[1]);$item.success_enabled=($flags -band 1) -ne 0
  if($Repair -and $admin -and -not $item.success_enabled){
   $output=& "$env:SystemRoot\System32\auditpol.exe" /set "/subcategory:{$($policy[1])}" /success:enable 2>&1
   if($LASTEXITCODE -ne 0){throw ($output -join ' ')}
   $item.success_enabled=([PCScopeAudit]::Query($policy[1]) -band 1) -ne 0
   $result.changes+=@{name=$policy[0];before=$flags;after=[PCScopeAudit]::Query($policy[1])}
  }
 }catch{$item.error=$_.Exception.Message}
 $result.audits+=$item
}
# Manifest injected by the service, so the check follows the actual collector set.
$channels=ConvertFrom-Json '__CHANNELS__'
foreach($channel in $channels){
 $entry=[ordered]@{name=$channel;source='EventLogConfiguration / EventLogReader';enabled=$null;readable=$false;mode=$null;maximum_bytes=$null;record_count=$null;oldest_record=$null;oldest_time=$null;newest_time=$null;error=$null}
 $config=$null;$reader=$null
 try{
  $config=[Diagnostics.Eventing.Reader.EventLogConfiguration]::new($channel)
  if($Repair -and $admin -and $channel -eq 'Microsoft-Windows-TaskScheduler/Operational' -and -not $config.IsEnabled){
   $config.IsEnabled=$true;$config.SaveChanges();$result.changes+=@{name=$channel;before='disabled';after='enabled'}
  }
  $entry.enabled=$config.IsEnabled;$entry.mode=[string]$config.LogMode;$entry.maximum_bytes=$config.MaximumSizeInBytes
  $info=[Diagnostics.Eventing.Reader.EventLogSession]::GlobalSession.GetLogInformation($channel,[Diagnostics.Eventing.Reader.PathType]::LogName)
  $entry.record_count=$info.RecordCount;$entry.oldest_record=$info.OldestRecordNumber
  $query=[Diagnostics.Eventing.Reader.EventLogQuery]::new($channel,[Diagnostics.Eventing.Reader.PathType]::LogName,'*')
  $reader=[Diagnostics.Eventing.Reader.EventLogReader]::new($query);$event=$reader.ReadEvent()
  if($event){try{if($event.TimeCreated){$entry.oldest_time=$event.TimeCreated.ToUniversalTime().ToString('o')}}finally{$event.Dispose()}}
  $reader.Dispose();$reader=$null;$query.ReverseDirection=$true
  $reader=[Diagnostics.Eventing.Reader.EventLogReader]::new($query);$event=$reader.ReadEvent()
  if($event){try{if($event.TimeCreated){$entry.newest_time=$event.TimeCreated.ToUniversalTime().ToString('o')}}finally{$event.Dispose()}}
  $entry.readable=$true
 }catch{$entry.error=$_.Exception.Message}finally{if($reader){$reader.Dispose()};if($config){$config.Dispose()}}
 $result.logs+=$entry
}
$result|ConvertTo-Json -Depth 8 -Compress
