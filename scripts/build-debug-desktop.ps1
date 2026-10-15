param([string]$ToolsRoot,[string]$Bundle)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$debug=Join-Path $project 'apps/desktop/src-tauri/target/debug'
$desktop=Join-Path $debug 'pcscope-desktop.exe'
$backup=$null
$loader=Join-Path $debug 'WebView2Loader.dll'
$loaderBackup=$null
if(Test-Path -LiteralPath $desktop -PathType Leaf){
    $active=@(Get-Process -Name pcscope-desktop -ErrorAction SilentlyContinue|Where-Object {$_.Path -eq $desktop})
    if($active.Count){throw '请先关闭从 debug 打开的界面再构建；后台可以继续运行。'}
    $hash=(Get-FileHash -LiteralPath $desktop).Hash
    # Cargo may hard-link its published exe to deps. Keep the previous executable recoverable,
    # and remove only matching compiler aliases so a low-integrity linker gets a fresh output.
    $links=@(& fsutil.exe hardlink list $desktop)
    if($LASTEXITCODE -ne 0){throw '无法读取构建文件的硬链接，已保留原程序。'}
    $aliases=@()
    foreach($link in $links){
        if(-not $link.StartsWith('\')){continue}
        $path=[IO.Path]::GetFullPath([IO.Path]::GetPathRoot($desktop).TrimEnd('\')+$link)
        if((Split-Path $path -Parent) -eq (Join-Path $debug 'deps') -and
            (Split-Path $path -Leaf) -match '^pcscope_desktop-[0-9a-f]+\.exe$' -and
            (Get-FileHash -LiteralPath $path).Hash -eq $hash){$aliases+=$path}
    }
    $backupDirectory=Join-Path $project 'target/debug-launch-backups'
    New-Item -ItemType Directory -Path $backupDirectory -Force|Out-Null
    $backup=Join-Path $backupDirectory ($hash.Substring(0,16)+'-'+[guid]::NewGuid().ToString('N')+'.exe')
    Move-Item -LiteralPath $desktop -Destination $backup
    foreach($alias in $aliases){Remove-Item -LiteralPath $alias}
}
try{
    # Tauri recopies the loader after a version/config change. The runnable copy may have
    # medium integrity while Cargo runs low; detach that one file before resource generation.
    if(Test-Path -LiteralPath $loader -PathType Leaf){
        $loaderDirectory=Join-Path $project 'target/debug-launch-backups'
        New-Item -ItemType Directory -Path $loaderDirectory -Force|Out-Null
        $loaderBackup=Join-Path $loaderDirectory ('loader-'+[guid]::NewGuid().ToString('N')+'.dll')
        Move-Item -LiteralPath $loader -Destination $loaderBackup
    }
    $arguments=@{Project='apps/desktop/src-tauri';CargoArgs=@('build')}
    if($ToolsRoot){$arguments.ToolsRoot=$ToolsRoot}
    & (Join-Path $project 'build.ps1') @arguments
    $prepare=@{SetRuntimeIntegrity=$true};if($Bundle){$prepare.Bundle=$Bundle}
    & (Join-Path $PSScriptRoot 'prepare-debug-runtime.ps1') @prepare
}catch{
    if($backup -and -not(Test-Path -LiteralPath $desktop)){Move-Item -LiteralPath $backup -Destination $desktop}
    if($loaderBackup -and -not(Test-Path -LiteralPath $loader)){Move-Item -LiteralPath $loaderBackup -Destination $loader}
    throw
}
