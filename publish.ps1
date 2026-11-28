# One-click publish: uploads the project to GitHub and (optionally) starts a release.
#   Run from the solocraft folder:   .\publish.ps1            (upload code only)
#                                    .\publish.ps1 -Release   (upload + build installers + notify users)
param([switch]$Release, [string]$Repo = "solomarch/sm-craft")
$OldRepo = "solomarch/solocraft-launcher"
# git and gh print normal progress to stderr; "Stop" would treat that as a failure, so exit codes are checked instead
$ErrorActionPreference = "Continue"
Set-Location $PSScriptRoot

if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "Git is not installed. Get it from https://git-scm.com/download/win and run this again." }
if (-not (Test-Path .git)) { git init | Out-Null }
git branch -M main

# the version comes from package.json, so every release is numbered automatically
$ver = (Get-Content package.json -Raw | ConvertFrom-Json).version
Write-Host "SM Craft version $ver -> github.com/$Repo" -ForegroundColor Cyan

# create the repo automatically if the GitHub CLI is installed, otherwise ask once
$url = "https://github.com/$Repo.git"
if (Get-Command gh -ErrorAction SilentlyContinue) {
    gh repo view $Repo *> $null
    if ($LASTEXITCODE -ne 0) {
        # the old repo is renamed (not recreated) so its history, releases and old links keep working
        gh repo view $OldRepo *> $null
        if ($LASTEXITCODE -eq 0) { gh repo rename $Repo.Split('/')[1] -R $OldRepo --yes 2>$null | Out-Null }
        gh repo view $Repo *> $null
        if ($LASTEXITCODE -ne 0) { gh repo create $Repo --public 2>$null | Out-Null }
    }
}
if (git remote | Where-Object { $_ -eq "origin" }) { git remote remove origin }
git remote add origin $url

git add -A
git -c user.name="solomarch" -c user.email="solomarch@users.noreply.github.com" commit -m "SM Craft v$ver" 2>&1 | Out-Null
Write-Host "Uploading ... (a browser window may ask you to sign in to GitHub the first time)" -ForegroundColor Cyan
git push -u -f origin main
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nPush failed. If it says the repository was not found: on GitHub open your old repo > Settings > rename it to '$($Repo.Split('/')[1])' (or create an EMPTY public repo with that name), then run this again. Repo name: '$($Repo.Split('/')[1])' at https://github.com/new and run this again." -ForegroundColor Yellow
    exit 1
}

if ($Release) {
    $tag = "v$ver"
    git tag -f $tag | Out-Null
    git push -f origin $tag
    Write-Host "`nRelease $tag started. Installers are built on GitHub in about 15-25 minutes:" -ForegroundColor Green
    Write-Host "https://github.com/$Repo/actions"
} else {
    Write-Host "`nCode uploaded: https://github.com/$Repo" -ForegroundColor Green
    Write-Host "When you want users to get the update, run:  .\publish.ps1 -Release"
}
