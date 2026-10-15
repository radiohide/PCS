param([string]$Bundle,[switch]$SetRuntimeIntegrity)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
if(-not $Bundle){$Bundle=Join-Path $project 'dist/dev'}
$source=(Resolve-Path -LiteralPath $Bundle).Path
$destination=(Resolve-Path -LiteralPath (Join-Path $project 'apps/desktop/src-tauri/target/debug')).Path
$desktop=Join-Path $destination 'pcscope-desktop.exe'
if(-not(Test-Path -LiteralPath $desktop -PathType Leaf)){throw '请先编译桌面程序。'}
# Keep the just-built desktop binary. Copy only its packaged runtime companions; no target
# directory recursion, deletion, database writes or backend replacement.
$files=@(Get-ChildItem -LiteralPath $source -File -Recurse | Sort-Object FullName)
foreach($file in $files){
    $relative=$file.FullName.Substring($source.Length+1)
    if($relative -in @('pcscope-desktop.exe','pcscope-launch.json')){continue}
    $target=Join-Path $destination $relative
    New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
    if((Test-Path -LiteralPath $target -PathType Leaf) -and
       (Get-FileHash -LiteralPath $target).Hash -eq (Get-FileHash -LiteralPath $file.FullName).Hash){continue}
    Copy-Item -LiteralPath $file.FullName -Destination $target -Force
    if((Get-FileHash -LiteralPath $target).Hash -ne (Get-FileHash -LiteralPath $file.FullName).Hash){throw "运行文件校验失败：$relative"}
}
$data=(Resolve-Path -LiteralPath (Join-Path $project 'target/desktop-preview-data')).Path
$cache=(Resolve-Path -LiteralPath (Join-Path $project 'target/desktop-preview-webview')).Path
$configPath=Join-Path $destination 'pcscope-launch.json'
if(Test-Path -LiteralPath $configPath -PathType Leaf){
    $config=Get-Content -LiteralPath $configPath -Raw -Encoding utf8|ConvertFrom-Json
    if($config.version -ne 1 -or -not $config.data_dir){throw '已有启动配置无法识别，已保留原文件。'}
    if(-not $config.PSObject.Properties['runtime_dir']){
        $config|Add-Member -NotePropertyName runtime_dir -NotePropertyValue $destination
        [IO.File]::WriteAllText($configPath,($config|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
    }
}else{
    $config=[ordered]@{version=1;data_dir=$data;webview_dir=$cache;runtime_dir=$destination}
    [IO.File]::WriteAllText($configPath,($config|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
}
$runtimeNames=@('pcscope-desktop.exe','pcscope-service.exe','pcscope-software.exe','pcscope-powershell-adapter.exe','pcscope-eventlog.exe','pcscope-cli.exe','pcscope-usn.exe','pcscope-eventlog-native.exe','pcscope-platform-native.exe','pcscope-probe.exe','WebView2Loader.dll')
$runtime=@($runtimeNames|ForEach-Object {$path=Join-Path $destination $_;if(-not(Test-Path -LiteralPath $path -PathType Leaf)){throw "缺少运行文件：$_"};[ordered]@{path=$path;sha256=(Get-FileHash -LiteralPath $path).Hash}})
if($SetRuntimeIntegrity){
    # Detach the public executable from Cargo's low-integrity compiler cache before assigning
    # its runtime label. Otherwise changing this file also changes the deps hard-link target.
    $detached=Join-Path $destination ('pcscope-desktop-'+[guid]::NewGuid().ToString('N')+'.pending')
    Copy-Item -LiteralPath $desktop -Destination $detached
    if((Get-FileHash -LiteralPath $detached).Hash -ne (Get-FileHash -LiteralPath $desktop).Hash){throw '桌面副本校验失败。'}
    Move-Item -LiteralPath $detached -Destination $desktop -Force
    foreach($file in $runtime){
        $item=Get-Item -LiteralPath $file.path
        if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){throw '运行文件不能是重解析链接。'}
        & icacls.exe $file.path /setintegritylevel M
        if($LASTEXITCODE -ne 0){throw ('无法设置运行文件的完整性级别：'+$file.path)}
    }
}
$report=[ordered]@{directory=$destination;data_dir=$config.data_dir;webview_dir=$config.webview_dir;runtime_files=$runtime;prepared_at=[DateTimeOffset]::Now.ToString('o')}
$report|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $project 'target/debug-runtime-prepared.json') -Encoding utf8
$report|ConvertTo-Json -Depth 5
