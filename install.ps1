# SM Craft - install or update (Windows PowerShell 5.1+ and PowerShell 7)
#   irm https://raw.githubusercontent.com/solomarch/sm-craft/main/install.ps1 | iex
# Running it again updates to the newest release. Set SOLOCRAFT_FORCE=1 to reinstall.
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 } catch {}

$Repo = $env:SOLOCRAFT_REPO
if (-not $Repo) { $Repo = "solomarch/sm-craft" }
$Headers = @{ "User-Agent" = "solocraft-installer" }

Write-Host "SM Craft - looking for the latest version ..." -ForegroundColor Cyan
try {
    $rel = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers $Headers
} catch {
    throw "Could not read the latest release of $Repo. Is the repository public, and has a release been published yet? ($_)"
}
$tag = $rel.tag_name

$stateDir = Join-Path $env:LOCALAPPDATA "SoloCraft-installer"
New-Item -ItemType Directory -Force -Path $stateDir | Out-Null
$stamp = Join-Path $stateDir "version.txt"
$have = ""
if (Test-Path $stamp) { $have = (Get-Content $stamp -Raw).Trim() }

if ($have -eq $tag -and -not $env:SOLOCRAFT_FORCE) {
    Write-Host "SM Craft is already up to date ($tag)." -ForegroundColor Green
} else {
    if (Get-Process -Name "solocraft-launcher" -ErrorAction SilentlyContinue) {
        throw "SM Craft is running. Close it (and any game started from it), then run this command again."
    }
    $asset = $rel.assets | Where-Object { $_.name -like "*-setup.exe" } | Select-Object -First 1
    if (-not $asset) {
        throw "Release $tag has no Windows installer yet (its build may still be running). Try again in a few minutes."
    }
    $exe = Join-Path $env:TEMP $asset.name
    Write-Host "Downloading $($asset.name) ($tag) ..."
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exe -Headers $Headers -UseBasicParsing
    Write-Host "Installing ..."
    $p = Start-Process -FilePath $exe -ArgumentList "/S" -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "The installer exited with code $($p.ExitCode)." }
    Remove-Item $exe -Force -ErrorAction SilentlyContinue
    Set-Content -Path $stamp -Value $tag
    Write-Host "Installed SM Craft $tag." -ForegroundColor Green
}

# In-game menu mod: one small jar per Minecraft version, picked automatically by the launcher.
$clientDir = Join-Path $env:APPDATA "SoloCraft\client"
New-Item -ItemType Directory -Force -Path $clientDir | Out-Null
$jars = @($rel.assets | Where-Object { $_.name -like "solocraft-client-*.jar" })
$updated = 0
foreach ($j in $jars) {
    $dest = Join-Path $clientDir $j.name
    if (-not (Test-Path $dest) -or (Get-Item $dest).Length -ne $j.size) {
        Invoke-WebRequest -Uri $j.browser_download_url -OutFile $dest -Headers $Headers -UseBasicParsing
        $updated++
    }
}
if ($jars.Count -gt 0) { Write-Host "SoloCraft Client mod: $($jars.Count) Minecraft versions available ($updated new or updated)." }
Write-Host "Done. Open 'SM Craft' from the Start menu." -ForegroundColor Green
