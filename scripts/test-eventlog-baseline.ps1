$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
& (Join-Path $project 'plugins/eventlog-native/build.ps1')
$csc=Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$test=Join-Path $project 'target/debug/eventlog-baseline-tests.exe'
& $csc /nologo /optimize+ /target:exe /main:BaselineTests /reference:System.Web.Extensions.dll /reference:System.Security.dll "/out:$test" (Join-Path $project 'plugins/eventlog-native/Program.cs') (Join-Path $project 'plugins/eventlog-native/BaselineTests.cs')
if($LASTEXITCODE -ne 0){throw 'Native baseline tests did not compile'}
& $test
if($LASTEXITCODE -ne 0){throw 'Native baseline query tests failed'}
$reader=Join-Path $project 'target/debug/pcscope-eventlog-native.exe'
$future=[DateTime]::UtcNow.AddDays(1).ToString('o')
# No system mutation: a future cutoff must exclude the existing Application log,
# including after an invalid checkpoint forces the reset branch.
foreach($resume in @($null,@{epoch='test';batch_seq=1;checkpoint=(@{creation='changed';record_id=1;fingerprint='invalid'}|ConvertTo-Json -Compress)})){
    $request=@{channel='Application';started_at=$future;resume=$resume}|ConvertTo-Json -Depth 5 -Compress
    $result=$request | & $reader | ConvertFrom-Json
    if($result.type -ne 'event_batch'){throw "Native log query failed: $($result.reason)"}
    if(@($result.events|Where-Object kind -ne 'coverage.log_reset').Count -ne 0){throw 'Pre-baseline system events leaked into fresh collection'}
}
Write-Output 'Live Application log: initial and reset baseline boundaries passed'
