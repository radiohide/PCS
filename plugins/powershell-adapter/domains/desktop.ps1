Emit 'desktop.printing' {Get-Printer | ForEach-Object {Item $_.Name (Fields $_ @('Name','Type','DriverName','PortName','Shared','ShareName','Published','PrinterStatus','RenderingMode'))}}
Emit 'desktop.printer-drivers' {Get-PrinterDriver | ForEach-Object {Item "$($_.Name)/$($_.PrinterEnvironment)" (Fields $_ @('Name','Manufacturer','PrinterEnvironment','MajorVersion','DriverPath','InfPath','ConfigFile','DataFile'))}}
Emit 'desktop.printer-ports' {Get-PrinterPort | ForEach-Object {Item $_.Name (Fields $_ @('Name','Description','PortMonitor','PrinterHostAddress','PortNumber','Protocol','SNMPEnabled'))}}
Emit 'desktop.print-jobs' {Get-Printer|ForEach-Object {$printer=$_;Get-PrintJob -PrinterName $printer.Name|ForEach-Object {Item "$($printer.Name)/$($_.ID)/$(Iso $_.SubmittedTime)" @{printer=$printer.Name;id=$_.ID;submitted_at=(Iso $_.SubmittedTime);status=[string]$_.JobStatus;size=$_.Size;total_pages=$_.TotalPages;pages_printed=$_.PagesPrinted;document_name=(Protect-Text $_.DocumentName);document_content_collected=$false;content_exclusion_reason='print_metadata_only_no_document_content'}}}}
Emit 'desktop.input' {Registry CurrentUser 'Keyboard Layout' @('','1','2','3','4','5') 2;Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Keyboard Layouts' @('Layout File','Layout Text','Layout Id','Ime File') 1;Registry LocalMachine 'SYSTEM\CurrentControlSet\Control\Class\{4d36e96b-e325-11ce-bfc1-08002be10318}' @('UpperFilters','LowerFilters') 1}
Emit 'desktop.clipboard' {Registry CurrentUser 'Software\Microsoft\Clipboard' @('EnableClipboardHistory','CloudClipboardAutomaticUpload','EnableCloudClipboard');Registry LocalMachine 'Software\Policies\Microsoft\Windows\System' @('AllowClipboardHistory','AllowCrossDeviceClipboard')}
Emit 'desktop.shell-settings' {Registry CurrentUser 'Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' @('Hidden','HideFileExt','ShowSuperHidden','LaunchTo','TaskbarAl','ShowTaskViewButton');Registry CurrentUser 'Control Panel\Desktop' @('Wallpaper','WallpaperStyle','TileWallpaper','ScreenSaveActive','ScreenSaveTimeOut');Registry CurrentUser 'Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders' @('Desktop','Personal','Start Menu','Startup','Programs','AppData','Local AppData')}
Emit 'desktop.shortcuts' {
    $shell=New-Object -ComObject WScript.Shell
    try{foreach($folder in @('DesktopDirectory','CommonDesktopDirectory','StartMenu','CommonStartMenu')){
        $path=[Environment]::GetFolderPath([Environment+SpecialFolder]::$folder);if(-not $path){continue};if($path.StartsWith('\\')){throw 'Network shell folder not followed'}
        $queue=[Collections.Generic.Stack[string]]::new();$queue.Push($path);$count=0
        while($queue.Count){$current=$queue.Pop();foreach($entry in @(Get-ChildItem -LiteralPath $current -Force)){
            if($entry.Attributes -band [IO.FileAttributes]::ReparsePoint){Item $entry.FullName.ToLowerInvariant() @{path=$entry.FullName;attributes=[string]$entry.Attributes;collection=@{reparse_target_followed=$false;reason='reparse_target_outside_scan_policy'}};continue}
            if($entry.PSIsContainer){$queue.Push($entry.FullName);continue};if($entry.Extension -ine '.lnk'){continue}
            $count++;if($count -gt 10000){throw 'Shortcut inventory limit reached'}
            $link=$shell.CreateShortcut($entry.FullName)
            try{Item $entry.FullName.ToLowerInvariant() @{path=$entry.FullName;target=$link.TargetPath;working_directory=$link.WorkingDirectory;arguments=(Protect-Text ([string]$link.Arguments));description=$link.Description;window_style=$link.WindowStyle;icon=$link.IconLocation;hotkey=$link.Hotkey;basis='existing shortcut metadata; target never executed'}}finally{[void][Runtime.InteropServices.Marshal]::ReleaseComObject($link)}
        }}
    }}finally{[void][Runtime.InteropServices.Marshal]::ReleaseComObject($shell)}
}
