$ErrorActionPreference='Stop'
# Read-only AST inventory; never invokes collector code.
$base=Split-Path $PSScriptRoot -Parent
$files=@(Join-Path $base 'plugins/powershell-adapter/collect-system.ps1')+@(Get-ChildItem (Join-Path $base 'plugins/powershell-adapter/domains') -Filter '*.ps1' | ForEach-Object FullName)
$all=@()
foreach($file in $files){
 $tokens=$null;$errors=$null;$ast=[System.Management.Automation.Language.Parser]::ParseFile($file,[ref]$tokens,[ref]$errors)
 if($errors.Count){throw "Cannot parse $file"}
 $functions=@{};foreach($fn in $ast.FindAll({param($n)$n -is [System.Management.Automation.Language.FunctionDefinitionAst]},$true)){$functions[$fn.Name]=$fn.Body}
 $emits=$ast.FindAll({param($n)$n -is [System.Management.Automation.Language.CommandAst] -and $n.GetCommandName() -in @('Emit','Emit-Collection')},$true)
 foreach($emit in $emits){
  $source=$emit.CommandElements[1].Value;if(!$source){$source=$emit.CommandElements[1].Extent.Text.Trim('"')}
  if(!$source.StartsWith('pcscope.')){$source='pcscope.'+$source}
  $fields=[Collections.Generic.HashSet[string]]::new();$helpers=[Collections.Generic.HashSet[string]]::new();$blocks=[Collections.Generic.List[object]]::new();$blocks.Add($emit)
  for($i=0;$i -lt $blocks.Count;$i++){
   $block=$blocks[$i]
   # Payloads built into a variable before being handed to Item (`$data=@{...}` … `Item $key $data`)
   # carry no hashtable inside the Item call itself, so resolving the variable is the only way to see
   # their fields. Nested hashtables are collected too: a payload may embed its own sub-record.
   # Look inside the right-hand side rather than testing it for a hashtable: `$data=@{...}` parses as
   # a CommandExpressionAst wrapping the hashtable, so a direct type test matches nothing.
   $assigned=@{}
   foreach($a in $block.FindAll({param($n)$n -is [System.Management.Automation.Language.AssignmentStatementAst]},$true)){if($a.Left -is [System.Management.Automation.Language.VariableExpressionAst]){$hashes=@($a.Right.FindAll({param($n)$n -is [System.Management.Automation.Language.HashtableAst]},$true));if($hashes.Count){$key=$a.Left.VariablePath.UserPath;$assigned[$key]=@($assigned[$key])+$hashes}}}
   foreach($cmd in $block.FindAll({param($n)$n -is [System.Management.Automation.Language.CommandAst]},$true)){
    $name=$cmd.GetCommandName()
    if($name -in @('Fields','Cim-Items')){foreach($arr in $cmd.FindAll({param($n)$n -is [System.Management.Automation.Language.ArrayExpressionAst]},$true)){foreach($str in $arr.FindAll({param($n)$n -is [System.Management.Automation.Language.StringConstantExpressionAst]},$true)){[void]$fields.Add($str.Value)}};[void]$helpers.Add('Fields')}
    if($name -eq 'Registry'){[void]$helpers.Add('Registry');foreach($field in @('path','hive','view','values','basis','collection')){[void]$fields.Add($field)}}
    if($name -eq 'Item'){foreach($hash in $cmd.FindAll({param($n)$n -is [System.Management.Automation.Language.HashtableAst]},$true)){foreach($pair in $hash.KeyValuePairs){if($pair.Item1 -is [System.Management.Automation.Language.StringConstantExpressionAst]){[void]$fields.Add($pair.Item1.Value)}}}
     foreach($arg in $cmd.CommandElements){if($arg -is [System.Management.Automation.Language.VariableExpressionAst]){foreach($hash in $assigned[$arg.VariablePath.UserPath]){foreach($pair in $hash.KeyValuePairs){if($pair.Item1 -is [System.Management.Automation.Language.StringConstantExpressionAst]){[void]$fields.Add($pair.Item1.Value)}}}}}}
    if($name -and $functions.ContainsKey($name) -and $helpers.Add($name)){$blocks.Add($functions[$name])}
   }
  }
  $all+=@{source=$source;file=$file.Substring($base.Length+1).Replace('\','/');line=$emit.Extent.StartLineNumber;code=$emit.Extent.Text;fields=@($fields|Sort-Object);helpers=@($helpers|Sort-Object)}
 }
}
$all|ConvertTo-Json -Depth 8|Set-Content (Join-Path $base 'target/collector-schema.json') -Encoding utf8
Write-Output "Extracted $($all.Count) source declarations"
