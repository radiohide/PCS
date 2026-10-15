# Shared evidence policy. Never execute collected commands or export credential stores.
function Redact-Text([string]$text){
    $text=[regex]::Replace($text,'(?is)-----BEGIN [^-]*PRIVATE KEY-----.*?-----END [^-]*PRIVATE KEY-----','[REDACTED PRIVATE KEY]')
    $text=[regex]::Replace($text,'(?im)(authorization\s*[:=]\s*)(?:Basic|Bearer|Digest|Negotiate)\s+[^\r\n]+','$1[REDACTED]')
    $text=[regex]::Replace($text,'(?i)(\bBearer\s+)[A-Za-z0-9._~+/-]+=*','$1[REDACTED]')
    $text=[regex]::Replace($text,'(?i)((?:-|/)(?:enc|encodedcommand|encodedarguments)\s+)("[^"]*"|''[^'']*''|\S+)','$1[REDACTED]')
    $text=[regex]::Replace($text,'(?i)(\b(?:password|passwd|pwd|secret|token|access[_-]?token|refresh[_-]?token|api[_-]?key|authorization|client[_-]?secret)\b["'']?\s*(?:[:=]|\s)\s*)("[^"]*"|''[^'']*''|[^\s,;]+)','$1[REDACTED]')
    return [regex]::Replace($text,'(?i)(https?://)[^\s/:@]+:[^\s/@]+@','$1[REDACTED]@')
}
function Protect-Sensitive($value){
    Add-Type -AssemblyName System.Security
    $plain=[Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $value -Compress -Depth 30))
    try{$cipher=[Security.Cryptography.ProtectedData]::Protect($plain,[Text.Encoding]::UTF8.GetBytes('PCScope/evidence/v1'),[Security.Cryptography.DataProtectionScope]::CurrentUser)}finally{[Array]::Clear($plain,0,$plain.Length)}
    return [ordered]@{sensitive=$true;protection='dpapi-current-user-v1';ciphertext=([BitConverter]::ToString($cipher)).Replace('-','').ToLowerInvariant()}
}
function Protect-Text([string]$text){
    if((Redact-Text $text) -cne $text){return (Protect-Sensitive $text)}
    return $text
}
function Protect-Value([string]$name,$value){
    if($null -eq $value){return $null}
    if($name -notmatch '(?i)(required|enabled|lastset|expires|age|length|history|policy|type|status)$' -and $name -match '(?i)password|passwd|secret|credential|private.?key|recovery.?password|^(token|access.?token|refresh.?token|api.?key|authorization|ScriptBlockText|ScriptText|Script|Binary)$'){
        return (Protect-Sensitive $value)
    }
    if($value -is [string]){return (Protect-Text $value)}
    if($value -is [datetime]){return $value.ToUniversalTime().ToString('o')}
    if($value -is [byte[]]){return [ordered]@{encoding='base64';bytes=$value.Length;value=[Convert]::ToBase64String($value)}}
    if($value -is [System.Collections.IDictionary]){$result=[ordered]@{};foreach($key in $value.Keys){$result[$key]=Protect-Value ([string]$key) $value[$key]};return $result}
    if($value -is [System.Collections.IEnumerable]){return ,@($value|ForEach-Object {Protect-Value $name $_})}
    if($value -is [ValueType]){return $value}
    return (Protect-Text ([string]$value))
}
function Public-CimFields($object,[int]$depth=0){
    if($depth -gt 8){throw 'Nested structure exceeds depth limit; incomplete source is not committed'}
    $result=[ordered]@{}
    foreach($property in $object.CimInstanceProperties){
        $value=$property.Value
        if($value -is [Microsoft.Management.Infrastructure.CimInstance]){$value=Public-CimFields $value ($depth+1)}
        elseif($value -is [array]){$value=@($value|ForEach-Object {if($_ -is [Microsoft.Management.Infrastructure.CimInstance]){Public-CimFields $_ ($depth+1)}else{$_}})}
        $result[$property.Name]=Protect-Value $property.Name $value
    }
    return $result
}
