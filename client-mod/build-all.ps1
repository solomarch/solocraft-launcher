# Builds SoloCraft Client for several Minecraft versions into .\dist
#
#   .\build-all.ps1                       ONE jar that works on every Minecraft 1.21.x (1.21 .. 1.21.11)
#   .\build-all.ps1 -Versions 1.20.1,1.20.4   separate jars for older versions (1.20 .. 1.20.6)
#   .\build-all.ps1 -All                  one 1.21.x jar plus every 1.20.x jar
#
# Needs JDK 21 active and internet. Mappings and Fabric API versions are looked up automatically.
param(
    [string[]]$Versions = @(),
    [switch]$All
)
$ErrorActionPreference = "Stop"
# "1.21.x" means the one universal jar (compiled once, works on every 1.21 version).
if ($All) {
    $Versions = "1.21.x", "1.20", "1.20.1", "1.20.2", "1.20.3", "1.20.4", "1.20.5", "1.20.6"
} elseif ($Versions.Count -eq 0) {
    $Versions = @("1.21.x")
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
    else { throw "Minecraft $v needs the universal build: run .\build-all.ps1 (no arguments) for every 1.21.x version." }
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
        if ($v -eq "1.21.x") {
            # compiled against 1.21.4; the jar accepts every 1.21.x (the mod finds version-specific
            # parts of the game at run time)
            Ensure-Props "1.21.4"
            .\gradlew.bat clean build "-Pmc=1.21.4" "-Ptag=1.21.x" "-Prange=~1.21" --console=plain
        } else {
            Ensure-Props $v
            .\gradlew.bat clean build "-Pmc=$v" --console=plain
        }
        if ($LASTEXITCODE -ne 0) { throw "gradle failed" }
        Get-ChildItem dist -Filter "solocraft-client-$v-*.jar" -ErrorAction SilentlyContinue | Remove-Item -Force
        Get-ChildItem build\libs -Filter "solocraft-client-$v-*.jar" |
            Where-Object { $_.Name -notmatch "sources|dev" } | Copy-Item -Destination dist -Force
        $ok += $v
        Get-ChildItem dist -Filter "solocraft-client-$v-*.jar" | ForEach-Object { Write-Host "  built: $($_.Name)  ($($_.LastWriteTime))" -ForegroundColor Green }
    } catch {
        Write-Host "FAILED $v : $_" -ForegroundColor Red
        $bad += $v
    }
}
Write-Host ""
Write-Host "Built:  $($ok -join ', ')" -ForegroundColor Green
if ($bad.Count -gt 0) { Write-Host "FAILED: $($bad -join ', ')  (scroll up for the error and send it to me)" -ForegroundColor Red }
if ($bad.Count -gt 0) { exit 1 }
