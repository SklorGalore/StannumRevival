<#
.SYNOPSIS
Copies the installers that "tauri build" made into one folder.

.DESCRIPTION
Looks under target/release/bundle and target/<triple>/release/bundle for the
installer formats Stannum ships (.msi, setup .exe, .dmg, .deb, .rpm,
.AppImage) and copies them to -OutDir. Fails if it finds none, so a build
that silently skipped bundling doesn't produce an empty release.

.EXAMPLE
pwsh scripts/release/collect-bundles.ps1 -OutDir dist-installers
#>
param(
    [Parameter(Mandatory)]
    [string] $OutDir
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
$target = Join-Path $root 'target'

$patterns = @{
    'msi'      = '*.msi'
    'nsis'     = '*-setup.exe'
    'dmg'      = '*.dmg'
    'deb'      = '*.deb'
    'rpm'      = '*.rpm'
    'appimage' = '*.AppImage'
}

$bundleDirs = @(Get-ChildItem -Path $target -Directory -Recurse -Filter 'bundle' -ErrorAction SilentlyContinue |
    Where-Object { $_.Parent.Name -eq 'release' })

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$copied = 0
foreach ($dir in $bundleDirs) {
    foreach ($kind in $patterns.Keys) {
        $kindDir = Join-Path $dir.FullName $kind
        if (-not (Test-Path $kindDir)) { continue }
        foreach ($file in Get-ChildItem -Path $kindDir -File -Filter $patterns[$kind]) {
            Copy-Item -Path $file.FullName -Destination $OutDir -Force
            $size = '{0:N1} MB' -f ($file.Length / 1MB)
            Write-Host "$($file.Name)  ($size)"
            $copied++
        }
    }
}
if ($copied -eq 0) { throw "No installers found under $target" }
Write-Host "Collected $copied installer(s) in $OutDir"
