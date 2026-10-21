param([string]$Destination,[switch]$SetRuntimeIntegrity,[ValidateSet('debug','release')][string]$RuntimeProfile='debug')
$ErrorActionPreference='Stop'
# Keep identical runtime files in place: a running collector may hold them open.
function Copy-StageFile {
    param([string]$LiteralPath,[string]$Destination,[switch]$Force)
    $target=if(Test-Path -LiteralPath $Destination -PathType Container){Join-Path $Destination (Split-Path $LiteralPath -Leaf)}else{$Destination}
    if((Test-Path -LiteralPath $target -PathType Leaf) -and
       (Get-FileHash -LiteralPath $LiteralPath).Hash -eq (Get-FileHash -LiteralPath $target).Hash){return}
    Copy-Item -LiteralPath $LiteralPath -Destination $Destination -Force:$Force
}
$project=Split-Path $PSScriptRoot -Parent
if(-not $Destination){$Destination=Join-Path $project 'dist/dev'}
$runtimeOutput=Join-Path $project ('target/'+$RuntimeProfile)
foreach($name in @('pcscope-service.exe','pcscope-software.exe','pcscope-powershell-adapter.exe','pcscope-eventlog.exe','pcscope-cli.exe','pcscope-usn.exe','examples/probe.exe')) {
    if(-not(Test-Path -LiteralPath (Join-Path $runtimeOutput $name) -PathType Leaf)){throw "缺少 $RuntimeProfile 运行程序：$name；请先使用相同构建模式编译。"}
}
& (Join-Path $project 'plugins/eventlog-native/build.ps1')
& (Join-Path $project 'plugins/platform-native/build.ps1')
New-Item -ItemType Directory -Path $destination -Force | Out-Null
foreach($file in @('pcscope-service.exe','pcscope-software.exe','pcscope-powershell-adapter.exe','pcscope-eventlog.exe','pcscope-cli.exe')){Copy-StageFile -LiteralPath (Join-Path $runtimeOutput $file) -Destination $destination -Force}
Copy-StageFile -LiteralPath (Join-Path $runtimeOutput 'pcscope-usn.exe') -Destination $destination -Force
Copy-StageFile -LiteralPath (Join-Path $project 'target/debug/pcscope-eventlog-native.exe') -Destination $destination -Force
Copy-StageFile -LiteralPath (Join-Path $project 'target/debug/pcscope-platform-native.exe') -Destination $destination -Force
Copy-StageFile -LiteralPath (Join-Path $runtimeOutput 'examples/probe.exe') -Destination (Join-Path $destination 'pcscope-probe.exe') -Force
foreach($file in @('pcscope-desktop.exe','WebView2Loader.dll')){Copy-StageFile -LiteralPath (Join-Path $project "apps/desktop/src-tauri/target/debug/$file") -Destination $destination -Force}
Copy-StageFile -LiteralPath (Join-Path $project 'plugins/eventlog/channels.json') -Destination $destination -Force
$taskMigration=Get-Content -LiteralPath (Join-Path $project 'model/native-migration.json') -Raw | ConvertFrom-Json
$taskCompatibility=@($taskMigration.sources | Where-Object backend -eq 'powershell_compatibility')
$taskCompatibilityUnits=@($taskCompatibility | ForEach-Object {$_.execution_units} | Sort-Object -Unique)
if(@($taskCompatibilityUnits | Where-Object {$_ -ne 'virtualization'}).Count){throw 'Review staging dependencies for newly introduced compatibility units'}
if($taskCompatibilityUnits.Count){
    foreach($name in @('collect-domain.ps1','evidence.ps1')){Copy-StageFile -LiteralPath (Join-Path $project ('plugins/powershell-adapter/'+$name)) -Destination $destination -Force}
    $taskDomains=Join-Path $destination 'domains';New-Item -ItemType Directory -Path $taskDomains -Force | Out-Null
    Copy-StageFile -LiteralPath (Join-Path $project 'plugins/powershell-adapter/domains/virtualization.ps1') -Destination $taskDomains -Force
}
if(-not (Test-Path -LiteralPath (Join-Path $destination 'collector-options.json'))){Copy-StageFile -LiteralPath (Join-Path $project 'plugins/powershell-adapter/collector-options.json') -Destination $destination}
if($SetRuntimeIntegrity){
    # Assign ordinary application integrity only to the detached runtime copies we just staged.
    # Never relabel Cargo, compiler caches, user data, or the workspace recursively.
    foreach($name in @('pcscope-service.exe','pcscope-software.exe','pcscope-powershell-adapter.exe','pcscope-eventlog.exe','pcscope-cli.exe','pcscope-usn.exe','pcscope-eventlog-native.exe','pcscope-platform-native.exe','pcscope-probe.exe','pcscope-desktop.exe')){
        $taskRuntimeFile=Join-Path $destination $name
        & icacls.exe $taskRuntimeFile /setintegritylevel M | Out-Null
        if($LASTEXITCODE -ne 0){throw "Runtime integrity setup failed: $name"}
    }
}
Write-Output "Development bundle staged at $destination (Rust runtime: $RuntimeProfile)"


