Emit 'apps.browser-policies' {foreach($hive in @('LocalMachine','CurrentUser')){foreach($path in @('Software\Policies\Google\Chrome','Software\Policies\Microsoft\Edge','Software\Policies\Mozilla\Firefox')){Registry $hive $path @('ExtensionInstallBlocklist','ExtensionInstallAllowlist','PasswordManagerEnabled','BrowserSignin','SyncDisabled') 2}}}
Emit 'apps.ide-extensions' {foreach($base in @((Join-Path $env:USERPROFILE '.vscode/extensions'),(Join-Path $env:USERPROFILE '.vscode-insiders/extensions'))){if(-not (Test-Path -LiteralPath $base)){continue};Get-ChildItem -LiteralPath $base -Directory | ForEach-Object {$manifest=Join-Path $_.FullName 'package.json';if(Test-Path -LiteralPath $manifest){$file=Get-Item -LiteralPath $manifest;if($file.Length -gt 1048576){throw 'Extension manifest too large'};$data=Get-Content -Raw -LiteralPath $manifest|ConvertFrom-Json;Item $_.FullName @{name=$data.name;publisher=$data.publisher;version=$data.version;engines=$data.engines;basis='installed manifest; does not establish activation'}}}}}
Emit 'apps.dotnet-runtimes' {Registry LocalMachine 'SOFTWARE\dotnet\Setup\InstalledVersions' @('InstallLocation','Version') 4;Registry LocalMachine 'SOFTWARE\Microsoft\NET Framework Setup\NDP' @('Version','Release','Install','InstallPath') 3}
Emit 'apps.npm-global-packages' {
    foreach($base in @((Join-Path $env:APPDATA 'npm/node_modules'),(Join-Path $env:ProgramFiles 'nodejs/node_modules'))){
        if(-not (Test-Path -LiteralPath $base)){continue}
        $directories=@(Get-ChildItem -LiteralPath $base -Directory|ForEach-Object {if($_.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Package reparse point not followed'};if($_.Name.StartsWith('@')){Get-ChildItem -LiteralPath $_.FullName -Directory}else{$_}})
        foreach($directory in $directories){if($directory.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Package directory reparse point not followed'};$path=Join-Path $directory.FullName 'package.json';if(Test-Path -LiteralPath $path){$file=Get-Item -LiteralPath $path;if($file.Length -gt 1048576 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Package manifest outside read policy'};$package=Get-Content -Raw -LiteralPath $path|ConvertFrom-Json;Item $directory.FullName @{name=$package.name;version=$package.version;path=$directory.FullName;basis='standard global npm directory; custom prefixes not scanned'}}}
    }
}
Emit 'apps.onedrive-registration' {Registry CurrentUser 'Software\Microsoft\OneDrive\Accounts' @('UserFolder','Business','DisplayName','AccountName','ConfiguredTenantId','TenantId','UserEmail') 1;Registry CurrentUser 'Software\SyncEngines\Providers\OneDrive' @('MountPoint','DisplayName','NamespaceID') 1;Registry LocalMachine 'Software\Policies\Microsoft\OneDrive' @('DisableFileSyncNGSC','FilesOnDemandEnabled','SilentAccountConfig') 0}
Emit 'apps.chromium-extensions' {
    foreach($browser in @(@{name='Chrome';path=(Join-Path $env:LOCALAPPDATA 'Google/Chrome/User Data')},@{name='Edge';path=(Join-Path $env:LOCALAPPDATA 'Microsoft/Edge/User Data')})){
        if(-not (Test-Path -LiteralPath $browser.path)){continue}
        foreach($profile in @(Get-ChildItem -LiteralPath $browser.path -Directory|Where-Object {$_.Name -eq 'Default' -or $_.Name -match '^Profile [0-9]+$'})){
            if($profile.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Browser profile is a reparse point; not followed'}
            $settings=@{};$settingsStatus='not_provided'
            foreach($settingsName in @('Preferences','Secure Preferences')){
                $settingsPath=Join-Path $profile.FullName $settingsName
                if(Test-Path -LiteralPath $settingsPath){
                    $settingsFile=Get-Item -LiteralPath $settingsPath
                    if($settingsFile.Length -gt 16777216 -or ($settingsFile.Attributes -band [IO.FileAttributes]::ReparsePoint)){$settingsStatus='preferences_outside_read_policy';continue}
                    try{
                        $preferences=Get-Content -Raw -LiteralPath $settingsPath|ConvertFrom-Json
                        foreach($property in $preferences.extensions.settings.PSObject.Properties){$settings[$property.Name]=$property.Value}
                        $settingsStatus='preferences_read'
                    }catch{$settingsStatus='preferences_unavailable'}
                }
            }
            $extensions=Join-Path $profile.FullName 'Extensions';if(-not (Test-Path -LiteralPath $extensions)){continue}
            foreach($extension in @(Get-ChildItem -LiteralPath $extensions -Directory)){
                if($extension.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Extension directory is a reparse point'}
                foreach($version in @(Get-ChildItem -LiteralPath $extension.FullName -Directory)){
                    if($version.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Extension version is a reparse point'}
                    $manifest=Join-Path $version.FullName 'manifest.json';if(-not (Test-Path -LiteralPath $manifest)){continue}
                    $file=Get-Item -LiteralPath $manifest;if($file.Length -gt 1048576 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Extension manifest not safely readable'}
                    $data=Get-Content -Raw -LiteralPath $manifest|ConvertFrom-Json
                    $configuration=$settings[$extension.Name]
                    Item "$($browser.name)/$($profile.Name)/$($extension.Name)/$($version.Name)" @{browser=$browser.name;profile=$profile.Name;extension_id=$extension.Name;name=$data.name;version=$data.version;manifest_version=$data.manifest_version;description=(Protect-Text $data.description);permissions=@($data.permissions);host_permissions=@($data.host_permissions);optional_permissions=@($data.optional_permissions);recorded_state=$configuration.state;disable_reasons=$configuration.disable_reasons;registered_path=$configuration.path;preferences_status=$settingsStatus;basis='on-disk manifest and recorded preferences; state code is not proof of current activation'}
                }
            }
        }
    }
}
Emit 'apps.firefox-extensions' {
    $base=Join-Path $env:APPDATA 'Mozilla/Firefox/Profiles';if(Test-Path -LiteralPath $base){foreach($profile in @(Get-ChildItem -LiteralPath $base -Directory)){
        if($profile.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Firefox profile reparse point not followed'}
        $path=Join-Path $profile.FullName 'extensions.json';if(Test-Path -LiteralPath $path){$file=Get-Item -LiteralPath $path;if($file.Length -gt 16777216 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Firefox extension index exceeds read policy'};$data=Get-Content -Raw -LiteralPath $path|ConvertFrom-Json;foreach($addon in $data.addons){Item "$($profile.Name)/$($addon.id)" @{profile=$profile.Name;id=$addon.id;version=$addon.version;type=$addon.type;active=$addon.active;user_disabled=$addon.userDisabled;app_disabled=$addon.appDisabled;signed_state=$addon.signedState;install_date=$addon.installDate;update_date=$addon.updateDate;basis='Firefox extension registry'}}}
    }}
}
