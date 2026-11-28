# SM Craft - uninstall (Windows PowerShell 5.1+ and PowerShell 7)
#   irm https://raw.githubusercontent.com/solomarch/sm-craft/main/uninstall.ps1 | iex
# Your instances, worlds and accounts are KEPT. To delete them as well run first:  $env:SOLOCRAFT_PURGE=1
$ErrorActionPreference = "Continue"

Get-Process -Name "solocraft-launcher" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 700

$keys = @("HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
          "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
          "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*")
$apps = @(Get-ItemProperty $keys -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match "SM Craft|SoloCraft" -and $_.UninstallString })
if ($apps.Count -eq 0) {
    Write-Host "No installed SM Craft was found (nothing to remove)." -ForegroundColor Yellow
} else {
    foreach ($a in $apps) {
        Write-Host "Removing $($a.DisplayName) ..." -ForegroundColor Cyan
        $cmd = $a.UninstallString.Trim()
        if ($cmd -match '^"([^"]+)"') { $exe = $Matches[1] } else { $exe = $cmd -replace '\s+/.*$', '' }
        if (Test-Path $exe) { Start-Process -FilePath $exe -ArgumentList "/S" -Wait }
        Write-Host "Removed." -ForegroundColor Green
    }
}

# the installer's version stamp, so the install command works again later
Remove-Item -Recurse -Force (Join-Path $env:LOCALAPPDATA "SoloCraft-installer") -ErrorAction SilentlyContinue

if ($env:SOLOCRAFT_PURGE) {
    foreach ($d in @((Join-Path $env:APPDATA "SoloCraft"), (Join-Path $env:LOCALAPPDATA "SoloCraft"))) {
        if (Test-Path $d) { Remove-Item -Recurse -Force $d -ErrorAction SilentlyContinue; Write-Host "Deleted $d" }
    }
    Write-Host "All SM Craft data deleted." -ForegroundColor Yellow
} else {
    Write-Host "Your instances and accounts were kept. (Set `$env:SOLOCRAFT_PURGE=1 and run again to delete them too.)"
}
Write-Host "Done." -ForegroundColor Green
