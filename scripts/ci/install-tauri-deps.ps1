<#
.SYNOPSIS
Installs the Linux packages Tauri needs to build, from a local cache of .deb
files when one is there.

.DESCRIPTION
CI restores -CacheDir with actions/cache before this runs. The cached .deb
files are copied into apt's archive folder, so apt-get only downloads what
the cache doesn't already hold and a slow Ubuntu mirror can't stall the job.
apt still resolves the install itself, so packages the runner image already
has at newer versions are never downgraded. Afterwards the .deb files are
copied back into -CacheDir for the next run to save.

.PARAMETER ExtraPackages
More packages to install alongside Tauri's own, such as patchelf, which
the release workflow needs to bundle an AppImage.

.EXAMPLE
pwsh scripts/ci/install-tauri-deps.ps1 -CacheDir ~/.cache/tauri-apt
#>
param(
    [Parameter(Mandatory)]
    [string] $CacheDir,
    [string[]] $ExtraPackages = @()
)

$ErrorActionPreference = 'Stop'

$packages = @(
    'libwebkit2gtk-4.1-dev'
    'libgtk-3-dev'
    'librsvg2-dev'
    'libayatana-appindicator3-dev'
) + $ExtraPackages

# Retry flaky mirrors and give up on a stuck connection instead of hanging.
$aptOptions = @(
    '-o', 'Acquire::Retries=5'
    '-o', 'Acquire::http::Timeout=30'
    '-o', 'Acquire::https::Timeout=30'
)

New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
$cached = @(Get-ChildItem -Path $CacheDir -Filter '*.deb' -ErrorAction SilentlyContinue)

# Start from an empty archive folder, seeded with the cached downloads.
& sudo apt-get clean
if ($cached.Count -gt 0) {
    Write-Host "Seeding apt with $($cached.Count) cached packages from $CacheDir"
    & sudo cp @($cached.FullName) /var/cache/apt/archives/
    if ($LASTEXITCODE -ne 0) { throw 'Could not copy the cached packages' }
}

& sudo apt-get @aptOptions update
if ($LASTEXITCODE -ne 0) { throw 'apt-get update failed' }

& sudo apt-get @aptOptions install -y --no-install-recommends @packages
if ($LASTEXITCODE -ne 0) { throw 'apt-get install failed' }

# Drop versions the mirror no longer offers, then keep the rest for next time.
& sudo apt-get autoclean
$downloaded = @(Get-ChildItem -Path '/var/cache/apt/archives' -Filter '*.deb' -ErrorAction SilentlyContinue)
Remove-Item -Path (Join-Path $CacheDir '*.deb') -ErrorAction SilentlyContinue
foreach ($deb in $downloaded) {
    Copy-Item -Path $deb.FullName -Destination $CacheDir
}
Write-Host "Saved $($downloaded.Count) packages to $CacheDir for the next run."
