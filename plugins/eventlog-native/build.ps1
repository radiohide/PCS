$ErrorActionPreference='Stop'
$out=Join-Path $PSScriptRoot '../../target/debug/pcscope-eventlog-native.exe'
New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null
$csc=Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
& $csc /nologo /optimize+ /target:exe /reference:System.Web.Extensions.dll /reference:System.Security.dll "/out:$out" (Join-Path $PSScriptRoot 'Program.cs')
if($LASTEXITCODE -ne 0){throw 'Native event reader build failed'}
Copy-Item (Join-Path $PSScriptRoot '../eventlog/channels.json') (Join-Path (Split-Path $out) 'channels.json') -Force
