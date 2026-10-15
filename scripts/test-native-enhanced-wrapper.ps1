param([Parameter(Mandatory=$true)][string]$BinDirectory,[Parameter(Mandatory=$true)][string]$OutputDirectory,[Parameter(Mandatory=$true)][string]$ProjectRoot,[Parameter(Mandatory=$true)][string]$ExpectedSid,[ValidateSet('ordinary','enhanced')][string]$Mode='ordinary')
$ErrorActionPreference='Stop'
$taskIdentity=[Security.Principal.WindowsIdentity]::GetCurrent()
if($taskIdentity.User.Value -ne $ExpectedSid){throw 'Native wrapper acceptance requires the original account'}
$taskElevated=([Security.Principal.WindowsPrincipal]::new($taskIdentity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if($taskElevated -ne ($Mode -eq 'enhanced')){throw 'Native wrapper acceptance token mode mismatch'}
$taskBin=(Resolve-Path -LiteralPath $BinDirectory).Path
$taskOutput=(Resolve-Path -LiteralPath $OutputDirectory).Path
if((Test-Path -LiteralPath (Join-Path $taskBin 'collect-domain.ps1')) -or (Test-Path -LiteralPath (Join-Path $taskBin 'collect-system.ps1'))){throw 'Acceptance must use detached binaries without compatibility scripts'}
$env:TEMP=$taskOutput;$env:TMP=$taskOutput
$taskInventory=Get-Content -LiteralPath (Join-Path $ProjectRoot 'model/native-migration.json') -Raw | ConvertFrom-Json
$taskReports=@()
foreach($taskUnit in @('system','security')){
    $taskSelected=if($taskUnit -eq 'system'){@('pcscope.software.optional-features','pcscope.software.provisioned')}else{@('pcscope.security.secureboot','pcscope.security.encryption','pcscope.security.tpm')}
    $taskInput=@{selection=@{disabled_sources=@($taskInventory.sources | Where-Object {$_.source -notin $taskSelected} | ForEach-Object source)}} | ConvertTo-Json -Depth 4 -Compress
    $taskStart=[Diagnostics.ProcessStartInfo]::new((Join-Path $taskBin 'pcscope-powershell-adapter.exe'),$taskUnit)
    $taskStart.UseShellExecute=$false;$taskStart.CreateNoWindow=$true;$taskStart.WorkingDirectory=$taskBin
    $taskStart.RedirectStandardInput=$true;$taskStart.RedirectStandardOutput=$true;$taskStart.RedirectStandardError=$true
    $taskStart.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$taskStart.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
    $taskProcess=[Diagnostics.Process]::Start($taskStart)
    $taskStdout=$taskProcess.StandardOutput.ReadToEndAsync();$taskStderr=$taskProcess.StandardError.ReadToEndAsync()
    $taskProcess.StandardInput.Write($taskInput);$taskProcess.StandardInput.Close()
    if(-not $taskProcess.WaitForExit(70000)){$taskProcess.Kill();throw 'Bounded native wrapper acceptance timed out'}
    $taskText=$taskStdout.GetAwaiter().GetResult();$taskErrors=$taskStderr.GetAwaiter().GetResult()
    [IO.File]::WriteAllText((Join-Path $taskOutput ($taskUnit+'.jsonl')),$taskText,[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $taskOutput ($taskUnit+'.stderr.txt')),$taskErrors,[Text.UTF8Encoding]::new($false))
    if($taskProcess.ExitCode -ne 0){throw 'Native wrapper failed; retained diagnostic material'}
    $taskRows=@($taskText -split '\r?\n' | Where-Object {$_} | ForEach-Object {$_ | ConvertFrom-Json})
    if(@($taskRows | Where-Object {$_.source -notin $taskSelected}).Count){throw 'Wrapper emitted an unselected source'}
    foreach($taskSource in $taskSelected){
        $taskSourceRows=@($taskRows | Where-Object source -eq $taskSource)
        $taskGaps=@($taskSourceRows | Where-Object type -eq 'gap');$taskSnapshots=@($taskSourceRows | Where-Object type -eq 'snapshot')
        if($Mode -eq 'enhanced' -and $taskGaps.Count){throw ('Enhanced native read failed: '+$taskSource)}
        if($taskGaps.Count){
            if($taskGaps.Count -ne 1 -or $taskSnapshots.Count){throw 'Read failure must retain state without an empty snapshot'}
            if($taskGaps[0].reason -match 'below ordinary application|compatibility script|PowerShell compatibility'){throw 'Invalid token or unexpected script route'}
        }elseif($taskSnapshots.Count -ne 16 -or @($taskSnapshots | Where-Object {-not $_.complete}).Count){throw 'Native wrapper did not emit all complete shards'}
        $taskReports+=@{source=$taskSource;snapshots=$taskSnapshots.Count;gaps=$taskGaps.Count;items=@($taskSnapshots | ForEach-Object {$_.items}).Count}
    }
}
@{mode=$Mode;elevated=$taskElevated;original_account=$true;legacy_scripts_present=$false;sources=$taskReports;system_configuration_changes=$false;formal_database_accessed=$false} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $taskOutput 'summary.json') -Encoding UTF8
