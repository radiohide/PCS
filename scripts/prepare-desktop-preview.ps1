param([string]$Bundle)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
if(-not $Bundle){$Bundle=Join-Path $project 'dist/history-notice-cleanup'}
$bundleRoot=(Resolve-Path -LiteralPath $Bundle).Path
foreach($name in @('pcscope-desktop.exe','pcscope-service.exe','WebView2Loader.dll','pcscope-platform-native.exe','pcscope-powershell-adapter.exe','collect-system.ps1')){
    if(-not(Test-Path -LiteralPath (Join-Path $bundleRoot $name) -PathType Leaf)){throw "开发包不完整：$name"}
}
$files=@(Get-ChildItem -LiteralPath $bundleRoot -Recurse -File | Sort-Object FullName | ForEach-Object {
    [pscustomobject]@{path=$_.FullName.Substring($bundleRoot.Length+1);source=$_.FullName;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash}
})
$hasher=[Security.Cryptography.SHA256]::Create()
try{$fingerprint=([BitConverter]::ToString($hasher.ComputeHash([Text.Encoding]::UTF8.GetBytes(($files|ForEach-Object {$_.path+':'+$_.sha256}) -join "`n")))).Replace('-','').Substring(0,16)}finally{$hasher.Dispose()}
# A runnable, versioned application directory: no changes to workspace ACLs or integrity labels.
$application=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) ('Programs\PCScopePreview\'+$fingerprint)
New-Item -ItemType Directory -Path $application -Force | Out-Null
foreach($file in $files){
    $destination=Join-Path $application $file.path
    if(-not(Test-Path -LiteralPath $destination -PathType Leaf)){
        New-Item -ItemType Directory -Path (Split-Path $destination -Parent) -Force | Out-Null
        Copy-Item -LiteralPath $file.source -Destination $destination
    }
    if((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $file.sha256){throw "程序文件校验失败：$($file.path)"}
}
$data=Join-Path $project 'target/desktop-preview-data'
$cache=Join-Path $project 'target/desktop-preview-webview'
New-Item -ItemType Directory -Path $data,$cache -Force | Out-Null
$configPath=Join-Path $application 'pcscope-launch.json'
if(-not(Test-Path -LiteralPath $configPath)){
    $config=[ordered]@{version=1;data_dir=$data;webview_dir=$cache;runtime_dir=$application}
    [IO.File]::WriteAllText($configPath,($config|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
}
$shell=New-Object -ComObject WScript.Shell
$links=@()
foreach($directory in @($project,(Join-Path $project 'apps/desktop/src-tauri/target/debug'))){
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $link=Join-Path $directory '打开 PCScope.lnk'
    # WSH can round-trip a non-ANSI filename as question marks on this machine. Save through
    # an ASCII temporary filename, then use the Unicode filesystem API for the visible name.
    $temporaryLink=Join-Path $directory ('PCScope-'+[guid]::NewGuid().ToString('N')+'.lnk')
    $shortcut=$shell.CreateShortcut($temporaryLink)
    $shortcut.TargetPath=Join-Path $application 'pcscope-desktop.exe'
    $shortcut.Arguments=''
    $shortcut.WorkingDirectory=$application
    $shortcut.IconLocation=(Join-Path $application 'pcscope-desktop.exe')+',0'
    $shortcut.Description='打开 PCScope，继续使用当前监控数据'
    $shortcut.WindowStyle=1
    $shortcut.Save()
    Move-Item -LiteralPath $temporaryLink -Destination $link -Force
    $links+=$link
}
$result=[ordered]@{application=$application;executable=(Join-Path $application 'pcscope-desktop.exe');arguments=$shortcut.Arguments;data_dir=$data;webview_dir=$cache;shortcuts=$links;files=$files|Select-Object path,sha256}
$result|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $project 'target/direct-launch-preview.json') -Encoding utf8
[pscustomobject]@{application=$application;shortcuts=$links;verified_files=$files.Count}|ConvertTo-Json
