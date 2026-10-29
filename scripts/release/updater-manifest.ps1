<#
.SYNOPSIS
Writes latest.json, the manifest the app's updater reads.

.DESCRIPTION
The release builds sign each installer for the updater and leave a .sig file
beside it. This script finds those .sig files under -Dir (the downloaded
workflow artifacts, one folder per platform named installers-<platform>) and
writes a Tauri updater manifest listing, for every platform and installer
kind, the download address on the GitHub release and the signature.

The app asks for the manifest on the latest published release, so a draft
release is not offered to anyone until it is published.

Fails if a platform has no signed installer, so a release can't silently
leave one platform without updates.

.EXAMPLE
pwsh scripts/release/updater-manifest.ps1 -Dir out -Version 0.5.0 -Tag v0.5.0 -NotesFile release-notes.md -Out latest.json
#>
param(
    [Parameter(Mandatory)]
    [string] $Dir,
    [Parameter(Mandatory)]
    [string] $Version,
    [Parameter(Mandatory)]
    [string] $Tag,
    [Parameter(Mandatory)]
    [string] $Out,
    [string] $NotesFile,
    [string] $Repo = 'SklorGalore/StannumRevival',
    # The workflow's artifact platforms, which must all be in the manifest.
    [string[]] $Platforms = @('windows-x64', 'macos-arm64', 'linux-x64')
)

$ErrorActionPreference = 'Stop'

# Workflow artifact platform -> the updater's {os}-{arch}.
$targets = @{
    'windows-x64' = 'windows-x86_64'
    'macos-arm64' = 'darwin-aarch64'
    'linux-x64'   = 'linux-x86_64'
}

# Signed file suffix -> the installer kind the updater asks for
# ({os}-{arch}-{kind}), and whether it is also the platform's default
# ({os}-{arch}), used when the running app doesn't know how it was installed.
$kinds = @(
    @{ Suffix = '-setup.exe'; Kind = 'nsis'; Default = $true }
    @{ Suffix = '.msi'; Kind = 'msi'; Default = $false }
    @{ Suffix = '.app.tar.gz'; Kind = 'app'; Default = $true }
    @{ Suffix = '.AppImage'; Kind = 'appimage'; Default = $true }
    @{ Suffix = '.deb'; Kind = 'deb'; Default = $false }
    @{ Suffix = '.rpm'; Kind = 'rpm'; Default = $false }
)

$platformsOut = [ordered]@{}
foreach ($platform in $Platforms) {
    $target = $targets[$platform]
    if (-not $target) { throw "Unknown platform $platform" }
    $folder = Join-Path $Dir "installers-$platform"
    $sigs = @(Get-ChildItem -Path $folder -File -Filter '*.sig' -ErrorAction SilentlyContinue)
    $found = $false
    foreach ($sig in $sigs) {
        $file = $sig.Name.Substring(0, $sig.Name.Length - 4)
        $kind = $kinds | Where-Object { $file.EndsWith($_.Suffix, [StringComparison]::Ordinal) } | Select-Object -First 1
        if (-not $kind) { continue }
        if (-not (Test-Path (Join-Path $folder $file))) { throw "$($sig.Name) has no $file beside it" }
        $entry = [ordered]@{
            signature = (Get-Content -Path $sig.FullName -Raw).Trim()
            url       = "https://github.com/$Repo/releases/download/$Tag/$([Uri]::EscapeDataString($file))"
        }
        $platformsOut["$target-$($kind.Kind)"] = $entry
        if ($kind.Default) { $platformsOut[$target] = $entry }
        Write-Host "$target-$($kind.Kind): $file"
        $found = $true
    }
    if (-not $found) { throw "No signed installer for $platform in $folder. Was TAURI_SIGNING_PRIVATE_KEY set for the build?" }
    if (-not $platformsOut.Contains($target)) { throw "No default installer for $platform (expected one of -setup.exe, .app.tar.gz, .AppImage)" }
}

$notes = if ($NotesFile -and (Test-Path $NotesFile)) { (Get-Content -Path $NotesFile -Raw).Trim() } else { '' }
$manifest = [ordered]@{
    version   = $Version
    notes     = $notes
    pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = $platformsOut
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -Path $Out -Encoding utf8NoBOM
Write-Host "Wrote $Out for $Version ($($platformsOut.Count) entries)"
