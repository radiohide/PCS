param([string]$Bundle)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
if(-not $Bundle){$Bundle=Join-Path $projectRoot 'dist/history-notice-cleanup'}
& (Join-Path $PSScriptRoot 'prepare-desktop-preview.ps1') -Bundle $Bundle
Start-Process -FilePath (Join-Path $projectRoot '打开 PCScope.lnk') -WorkingDirectory $projectRoot -WindowStyle Normal
