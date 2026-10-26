<#
.SYNOPSIS
Installs the Linux packages Tauri needs to build, from a local cache of .deb
files when one is there.

.DESCRIPTION
CI restores -CacheDir with actions/cache before this runs. On a cache hit the
packages install straight from the cached .deb files, so a slow Ubuntu mirror
can't stall the job. On a miss (or if the cached files don't install) it falls
back to apt-get with retries and timeouts, then copies the downloaded .deb
files into -CacheDir for the next run to save.

.EXAMPLE
pwsh scripts/ci/install-tauri-deps.ps1 -CacheDir ~/.cache/tauri-apt
#>
param(
    [Parameter(Mandatory)]
    [string] $CacheDir
)

$ErrorActionPreference = 'Stop'

$packages = @(
    'libwebkit2gtk-4.1-dev'
    'libgtk-3-dev'
    'librsvg2-dev'
    'libayatana-appindicator3-dev'
)

# Retry flaky mirrors and give up on a stuck connection instead of hanging.
$aptOptions = @(
    '-o', 'Acquire::Retries=5'
    '-o', 'Acquire::http::Timeout=30'
    '-o', 'Acquire::https::Timeout=30'
)

New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
$cached = @(Get-ChildItem -Path $CacheDir -Filter '*.deb' -ErrorAction SilentlyContinue)

if ($cached.Count -gt 0) {
    Write-Host "Installing $($cached.Count) cached packages from $CacheDir"
    & sudo dpkg -i @($cached.FullName)
    if ($LASTEXITCODE -eq 0) {
        & dpkg -s @packages | Out-Null
        if ($LASTEXITCODE -eq 0) {
            Write-Host 'Tauri packages installed from the cache.'
            exit 0
        }
    }
    Write-Warning 'The cached packages did not install cleanly; falling back to apt-get.'
}

& sudo apt-get @aptOptions update
if ($LASTEXITCODE -ne 0) { throw 'apt-get update failed' }

# Clear the archive folder so only this install's downloads get cached.
& sudo apt-get clean
& sudo apt-get @aptOptions install -y --no-install-recommends @packages
if ($LASTEXITCODE -ne 0) { throw 'apt-get install failed' }

$downloaded = @(Get-ChildItem -Path '/var/cache/apt/archives' -Filter '*.deb' -ErrorAction SilentlyContinue)
Remove-Item -Path (Join-Path $CacheDir '*.deb') -ErrorAction SilentlyContinue
foreach ($deb in $downloaded) {
    Copy-Item -Path $deb.FullName -Destination $CacheDir
}
Write-Host "Saved $($downloaded.Count) packages to $CacheDir for the next run."
