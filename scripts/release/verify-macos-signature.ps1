<#
.SYNOPSIS
Checks that the Stannum.app inside a .dmg carries a whole-bundle signature.

.DESCRIPTION
An app whose bundle is not signed as a whole (only its main binary, as the
Rust linker signs it) is reported by macOS as "damaged and can't be opened"
once it has been downloaded. Tauri signs the bundle ad hoc because
src-tauri/tauri.macos.conf.json sets bundle.macOS.signingIdentity to "-", or
with a real identity when APPLE_SIGNING_IDENTITY is set.

This mounts each .dmg read-only, runs "codesign --verify --deep --strict" on
the app, and fails unless the signature seals the bundle's resources. macOS
only.

.EXAMPLE
pwsh scripts/release/verify-macos-signature.ps1 -Path installers
#>
param(
    # A .dmg, or a folder holding one or more.
    [Parameter(Mandatory)]
    [string] $Path
)

$ErrorActionPreference = 'Stop'
if (-not $IsMacOS) { throw 'verify-macos-signature.ps1 runs on macOS only.' }

$dmgs = @(if (Test-Path $Path -PathType Container) {
        Get-ChildItem -Path $Path -File -Filter '*.dmg'
    } else {
        Get-Item $Path
    })
if ($dmgs.Count -eq 0) { throw "No .dmg found at $Path" }

foreach ($dmg in $dmgs) {
    $mount = Join-Path ([IO.Path]::GetTempPath()) ("stannum-dmg-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $mount | Out-Null
    hdiutil attach $dmg.FullName -readonly -nobrowse -mountpoint $mount | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Could not mount $($dmg.Name)" }
    try {
        $app = Get-ChildItem -Path $mount -Directory -Filter '*.app' | Select-Object -First 1
        if (-not $app) { throw "No .app inside $($dmg.Name)" }

        $details = (codesign --display --verbose=2 $app.FullName 2>&1) -join "`n"
        Write-Host "$($dmg.Name): $($app.Name)"
        Write-Host $details

        codesign --verify --deep --strict --verbose=2 $app.FullName
        if ($LASTEXITCODE -ne 0) { throw "$($app.Name) in $($dmg.Name) fails codesign --verify." }
        if ($details -notmatch 'Sealed Resources') {
            throw "$($app.Name) in $($dmg.Name) is not signed as a bundle (no sealed resources). macOS will call it damaged."
        }
        if ($details -notmatch 'Info\.plist entries=') {
            throw "$($app.Name) in $($dmg.Name) has no Info.plist bound to its signature."
        }
        Write-Host "$($app.Name) in $($dmg.Name): bundle signature OK"
    } finally {
        # The disk image can still be busy right after codesign reads it, so
        # force the detach. Remove the mount folder only once it is empty: a
        # recursive delete of a still-mounted image would reach into it.
        hdiutil detach $mount -force -quiet | Out-Null
        if (-not (Get-ChildItem -LiteralPath $mount -Force -ErrorAction SilentlyContinue)) {
            Remove-Item -LiteralPath $mount -Force -ErrorAction SilentlyContinue
        }
    }
}
