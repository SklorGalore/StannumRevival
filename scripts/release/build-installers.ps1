<#
.SYNOPSIS
Builds the installers, signed for the updater when the key is available.

.DESCRIPTION
Runs "tauri build" for -Bundles. When TAURI_SIGNING_PRIVATE_KEY (with
TAURI_SIGNING_PRIVATE_KEY_PASSWORD when the key has one) is set, it writes the
updater config overlay (updater-config.ps1) with the public key committed in
src-tauri/updater-key.pub and builds with it, so each installer gets a .sig
beside it and the app checks GitHub for updates. It then checks that the
signatures were made with the private half of that public key, so a wrong
secret fails the build instead of shipping updates nobody can install.

Without the key:
- a release build (-Release, a tag push) builds without the updater and
  warns, so a release can still be cut;
- any other build (pull requests, release branches, by hand) signs with a
  throwaway key made on the spot, so the updater part of the build is
  proven on every platform. Those installers can't install real updates,
  as the throwaway key doesn't match the real one.

A pre-release version (1.0.0-rc.1) gets an MSI version too: Windows
Installer takes only numbers (major.minor.patch.build), and Tauri refuses a
pre-release that isn't a bare number. The build field is the pre-release's
last number (rc.1 -> 1.0.0.1). Windows Installer ignores that field when it
compares versions, and Tauri's MSI replaces any installed copy, so 1.0.0
still installs over 1.0.0-rc.1.

.EXAMPLE
pwsh scripts/release/build-installers.ps1 -Bundles deb,appimage
#>
param(
    [Parameter(Mandatory)]
    [string] $Bundles,
    [switch] $Release
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
$overlay = Join-Path $root 'src-tauri/tauri.updater.conf.json'
$msiOverlay = Join-Path $root 'src-tauri/tauri.msi-version.conf.json'
$pubKey = (Get-Content -Path (Join-Path $root 'src-tauri/updater-key.pub') -Raw).Trim()

# The 8-byte key id inside a minisign public key or signature (base64 of the
# text form, whose second line is base64 of: 2-byte algorithm, 8-byte key id, ...).
function Get-KeyId([string] $Base64) {
    $text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Base64.Trim()))
    $line = ($text -split "`n")[1].Trim()
    $bytes = [Convert]::FromBase64String($line)
    # Written as minisign prints it (little-endian), e.g. AD2CD0DAC66B8C16.
    -join ($bytes[9..2] | ForEach-Object { $_.ToString('X2') })
}

# The numeric version Windows Installer needs: 1.0.0-rc.1 -> 1.0.0.1,
# 1.0.0-beta -> 1.0.0.0, 1.0.0 -> 1.0.0 (unchanged).
function Get-MsiVersion([string] $Version) {
    if ($Version -notmatch '^(\d+\.\d+\.\d+)(?:-([0-9A-Za-z.-]+))?(?:\+.*)?$') {
        throw "The app version $Version is not major.minor.patch with an optional suffix."
    }
    if (-not $Matches[2]) { return $Version }
    $core = $Matches[1]
    $numbers = @([regex]::Matches($Matches[2], '\d+') | ForEach-Object { [int64] $_.Value })
    $build = if ($numbers) { $numbers[-1] } else { 0 }
    if ($build -gt 65535) { throw "The pre-release number in $Version is over 65535, which an MSI version can't hold." }
    "$core.$build"
}

$updater = $true
$throwaway = $false
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    if ($Release) {
        Write-Host '::warning title=No updater key::TAURI_SIGNING_PRIVATE_KEY is not set, so this release is built without the updater and installed copies will not be offered it. See docs/releasing.md.'
        $updater = $false
    } else {
        Write-Host 'No updater key: signing with a throwaway key to test the updater build.'
        $dir = Join-Path ([IO.Path]::GetTempPath()) "stannum-updater-$([guid]::NewGuid().ToString('n'))"
        New-Item -ItemType Directory -Path $dir | Out-Null
        $keyFile = Join-Path $dir 'throwaway.key'
        $password = [guid]::NewGuid().ToString('n')
        Push-Location $root
        try {
            & npx tauri signer generate --ci -p $password -w $keyFile | Out-Null
            if ($LASTEXITCODE -ne 0) { throw 'tauri signer generate failed' }
        } finally {
            Pop-Location
        }
        $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Path $keyFile -Raw
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $password
        $pubKey = (Get-Content -Path "$keyFile.pub" -Raw).Trim()
        $throwaway = $true
    }
}

$tauriArgs = @('build', '--ci', '--bundles', $Bundles)
if ($updater) {
    & (Join-Path $PSScriptRoot 'updater-config.ps1') -PubKey $pubKey -Out $overlay
    $tauriArgs += @('--config', $overlay)
}

$version = (Get-Content -Path (Join-Path $root 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json).version
$msiVersion = Get-MsiVersion $version
if ($msiVersion -ne $version) {
    @{ bundle = @{ windows = @{ wix = @{ version = $msiVersion } } } } |
        ConvertTo-Json -Depth 5 | Set-Content -Path $msiOverlay -Encoding utf8NoBOM
    $tauriArgs += @('--config', $msiOverlay)
    Write-Host "MSI version for $version is $msiVersion."
}

Push-Location $root
try {
    & npm run tauri -- @tauriArgs
    if ($LASTEXITCODE -ne 0) { throw 'tauri build failed' }
} finally {
    Pop-Location
    Remove-Item -Path $overlay, $msiOverlay -ErrorAction SilentlyContinue
}

if ($updater) {
    $want = Get-KeyId $pubKey
    $sigs = @(Get-ChildItem -Path (Join-Path $root 'target') -Recurse -File -Filter '*.sig' |
        Where-Object { $_.FullName -match '[\\/]release[\\/]bundle[\\/]' })
    if (-not $sigs) { throw 'The build made no updater signatures (.sig files).' }
    foreach ($sig in $sigs) {
        $got = Get-KeyId (Get-Content -Path $sig.FullName -Raw)
        if ($got -ne $want) {
            throw "$($sig.Name) was signed with key $got, but the app trusts $want (src-tauri/updater-key.pub). TAURI_SIGNING_PRIVATE_KEY is not the private half of that public key."
        }
    }
    $kind = if ($throwaway) { 'a throwaway key' } else { "the updater key $want" }
    Write-Host "$($sigs.Count) updater signature(s), all made with $kind."
}
