# Builds SoloCraft Client for several Minecraft versions into .\dist
#
#   .\build-all.ps1                       the popular ones: 1.20.1, 1.20.4, 1.21.1, 1.21.4
#   .\build-all.ps1 -Versions 1.20.2,1.21.3
#   .\build-all.ps1 -All                  every supported version 1.20 .. 1.21.4
#
# Needs JDK 21 active and internet. Mappings and Fabric API versions are looked up automatically.
param(
    [string[]]$Versions = @("1.20.1", "1.20.4", "1.21.1", "1.21.4"),
    [switch]$All
)
$ErrorActionPreference = "Stop"
if ($All) {
    $Versions = "1.20", "1.20.1", "1.20.2", "1.20.3", "1.20.4", "1.20.5", "1.20.6",
                "1.21", "1.21.1", "1.21.2", "1.21.3", "1.21.4"
}

function Ensure-Props([string]$v) {
    $file = "versions\$v.properties"
    if (Test-Path $file) { return }
    $vv = $v
    if (($v -split '\.').Count -eq 2) { $vv = "$v.0" }
    $p = [version]$vv
    if     ($p -le [version]"1.20.1") { $compat = "mc1201"; $java = 17 }
    elseif ($p -le [version]"1.20.4") { $compat = "mc1204"; $java = 17 }
    elseif ($p -le [version]"1.20.6") { $compat = "mc121";  $java = 21 }
    elseif ($p -le [version]"1.21.4") { $compat = "mc121";  $java = 21 }
    else { throw "Minecraft $v is not supported yet (this mod covers 1.20 to 1.21.4)." }
    Write-Host "  looking up mappings and Fabric API for $v ..."
    $yarn = (Invoke-RestMethod "https://meta.fabricmc.net/v2/versions/yarn/$v")[0].version
    $q = [uri]::EscapeDataString('["' + $v + '"]')
    $fab = (Invoke-RestMethod "https://api.modrinth.com/v2/project/fabric-api/version?game_versions=$q&loaders=%5B%22fabric%22%5D")[0].version_number
    if (-not $yarn -or -not $fab) { throw "Could not find mappings or Fabric API for $v" }
    @("yarn=$yarn", "fabric=$fab", "loader=0.16.10", "compat=$compat", "java=$java", "mcrange=$v") |
        Set-Content -Path $file -Encoding ascii
}

New-Item -ItemType Directory -Force -Path dist | Out-Null
$ok = @(); $bad = @()
foreach ($v in $Versions) {
    Write-Host "=== Minecraft $v ===" -ForegroundColor Cyan
    try {
        Ensure-Props $v
        .\gradlew.bat clean build "-Pmc=$v" --console=plain
        if ($LASTEXITCODE -ne 0) { throw "gradle failed" }
        Get-ChildItem build\libs -Filter "solocraft-client-$v-*.jar" |
            Where-Object { $_.Name -notmatch "sources|dev" } | Copy-Item -Destination dist -Force
        $ok += $v
    } catch {
        Write-Host "FAILED $v : $_" -ForegroundColor Red
        $bad += $v
    }
}
Write-Host ""
Write-Host "Built:  $($ok -join ', ')" -ForegroundColor Green
if ($bad.Count -gt 0) { Write-Host "FAILED: $($bad -join ', ')  (scroll up for the error and send it to me)" -ForegroundColor Red }
