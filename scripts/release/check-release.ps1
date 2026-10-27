<#
.SYNOPSIS
Checks that the app version is the same everywhere it is written down, and
that CHANGELOG.md has notes for it.

.DESCRIPTION
The version lives in package.json, src-tauri/tauri.conf.json and
src-tauri/Cargo.toml. All three must agree. With -Tag, the tag must be "v"
followed by that version. With -NotesFile, the version's section of
CHANGELOG.md is written to that file for the GitHub release. Without -Tag,
an "Unreleased" section is accepted in place of the version's own section,
so pull requests can be checked before a version is cut.

.EXAMPLE
pwsh scripts/release/check-release.ps1 -Tag v0.3.0 -NotesFile notes.md
#>
param(
    [string] $Tag,
    [string] $NotesFile
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
. (Join-Path $PSScriptRoot 'release-common.ps1')

$versions = Get-AppVersions -Root $root
$versions.GetEnumerator() | ForEach-Object { Write-Host ("{0,-28} {1}" -f $_.Key, $_.Value) }
$distinct = @($versions.Values | Sort-Object -Unique)
if ($distinct.Count -ne 1) {
    throw "The app version differs between files. Run scripts/release/new-version.ps1 to set them all."
}
$version = $distinct[0]
if ($version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    throw "Version '$version' is not a semantic version (x.y.z)."
}

if ($Tag) {
    if ($Tag -ne "v$version") {
        throw "Tag '$Tag' does not match the app version $version (expected v$version)."
    }
}

$changelog = Join-Path $root 'CHANGELOG.md'
$notes = Get-ChangelogSection -Path $changelog -Heading $version
if ($null -eq $notes -and -not $Tag) {
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
