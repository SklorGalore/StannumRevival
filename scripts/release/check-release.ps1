<#
.SYNOPSIS
Checks that the app version is the same everywhere it is written down, and
that CHANGELOG.md has notes for it.

.DESCRIPTION
The version lives in package.json, src-tauri/tauri.conf.json and
src-tauri/Cargo.toml. All three must agree. With -Tag, the tag must be "v"
followed by that version. With -Branch set to a release or hotfix branch
(release/vX.Y.Z, hotfix/vX.Y.Z), the branch must be named for that version.
With -NotesFile, the version's section of CHANGELOG.md is written to that
file for the GitHub release.

On a tag or a release/hotfix branch the version must have its own changelog
section. Anywhere else an "Unreleased" section is accepted in its place, so
feature branches and development can be checked before a version is cut.

.EXAMPLE
pwsh scripts/release/check-release.ps1 -Tag v0.3.0 -NotesFile notes.md

.EXAMPLE
pwsh scripts/release/check-release.ps1 -Branch release/v0.3.0
#>
param(
    [string] $Tag,
    [string] $Branch,
    [string] $NotesFile,
    # Where the versioned files are. Defaults to this repository.
    [string] $Root = (Join-Path $PSScriptRoot '../..')
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path $Root
. (Join-Path $PSScriptRoot 'release-common.ps1')

$versions = Get-AppVersions -Root $root
$versions.GetEnumerator() | ForEach-Object { Write-Host ("{0,-28} {1}" -f $_.Key, $_.Value) }
$distinct = @($versions.Values | Sort-Object -Unique)
if ($distinct.Count -ne 1) {
    throw "The app version differs between files. Run scripts/release/new-version.ps1 to set them all."
}
$version = $distinct[0]
if ($version -notmatch "^$SemVerPattern$") {
    throw "Version '$version' is not a semantic version (x.y.z)."
}

if ($Tag) {
    if ($Tag -ne "v$version") {
        throw "Tag '$Tag' does not match the app version $version (expected v$version)."
    }
}

$branchVersion = if ($Branch) { Get-BranchVersion $Branch } else { $null }
if ($branchVersion -and $branchVersion -ne $version) {
    throw "Branch '$Branch' is for $branchVersion but the app version is $version. Run scripts/release/new-version.ps1 -Version $branchVersion on it."
}
$cut = $Tag -or $branchVersion

$changelog = Join-Path $root 'CHANGELOG.md'
$notes = Get-ChangelogSection -Path $changelog -Heading $version
if ($null -eq $notes -and -not $cut) {
    $notes = Get-ChangelogSection -Path $changelog -Heading 'Unreleased'
    if ($null -ne $notes) { Write-Host "No section for $version yet; using Unreleased." }
}
if ($null -eq $notes -or $notes.Trim().Length -eq 0) {
    throw "CHANGELOG.md has no notes for $version."
}

if ($NotesFile) {
    $notes.Trim() | Set-Content -Path $NotesFile -Encoding utf8NoBOM
    Write-Host "Wrote release notes to $NotesFile"
}
Write-Host "Release check passed for $version."
if ($env:GITHUB_OUTPUT) { "version=$version" | Add-Content -Path $env:GITHUB_OUTPUT }
