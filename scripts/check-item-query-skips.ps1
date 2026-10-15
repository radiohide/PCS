$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot
foreach($spec in @(@{File='collect-system.ps1';Function='Emit-Collection';Source='pcscope.system.version'},@{File='collect-domain.ps1';Function='Emit';Source='identity.logon-sessions'})){
    $tokens=$null;$errors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $root "plugins/powershell-adapter/$($spec.File)"),[ref]$tokens,[ref]$errors)
    if($errors){throw 'PowerShell parse failed'}
    $name=$spec.Function
    $function=$ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name},$true)
    Invoke-Expression $function.Extent.Text
    $selection=[pscustomobject]@{disabled_sources=@('pcscope.system.version','pcscope.identity.logon-sessions')}
    $script:queried=$false
    $result=@(& $name $spec.Source {$script:queried=$true;throw 'Disabled query executed'})
    if($script:queried -or $result.Count){throw 'Disabled source was queried or emitted output'}
}
'PASS: disabled PowerShell sources skip query scriptblocks and emit no empty snapshots'
$request='{"channel":"Application","selection":{"event_rules":[{"rule":{},"enabled":false}]}}'
$lines=@($request | & (Join-Path $root 'target/debug/pcscope-eventlog-native.exe'))
if($LASTEXITCODE -ne 0){throw 'Native collector failed'}
$batch=$lines | ForEach-Object {ConvertFrom-Json $_} | Where-Object type -eq 'event_batch'
if(!$batch -or @($batch.events).Count -ne 0){throw 'Native reader did not exclude disabled events'}
$cursor=ConvertFrom-Json $batch.checkpoint
if($cursor.record_id -le 0){throw 'Expected Application log records to advance checkpoint'}
'PASS: native reader skips disabled payloads and advances the source checkpoint'
