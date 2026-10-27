<#
.SYNOPSIS
Sets a new app version everywhere it is written down, and starts its
section in CHANGELOG.md.

.DESCRIPTION
Updates package.json, package-lock.json, src-tauri/tauri.conf.json, the
Cargo.toml of the app, stannum-cli and the Python package (stannum-py), and
their entries in Cargo.lock. In CHANGELOG.md the
notes under "## Unreleased" become the new version's section, dated today,
and an empty "## Unreleased" section is left above it.

Commit the result in a pull request. Once it is merged, tag the merge
commit with v<version> and push the tag to start the release workflow.

.EXAMPLE
pwsh scripts/release/new-version.ps1 -Version 0.3.0
#>
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$')]
    [string] $Version,
    [string] $Date = (Get-Date -Format 'yyyy-MM-dd')
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
. (Join-Path $PSScriptRoot 'release-common.ps1')

# Replace the first $Count matches of $Pattern in a file, keeping everything
# else (formatting, line endings) as it is.
function Set-InFile([string] $Path, [string] $Pattern, [string] $Replacement, [int] $Count = 1) {
    $text = [IO.File]::ReadAllText($Path)
    $regex = [regex]::new($Pattern, 'Multiline')
    $found = $regex.Matches($text).Count
    if ($found -lt $Count) { throw "Expected $Count match(es) of $Pattern in $Path, found $found" }
    $text = $regex.Replace($text, $Replacement, $Count)
    [IO.File]::WriteAllText($Path, $text)
}

$jsonVersion = '("version"\s*:\s*")[^"]+(")'
$jsonReplace = '${1}' + $Version + '${2}'
Set-InFile (Join-Path $root 'package.json') $jsonVersion $jsonReplace
# The lockfile carries the root version twice: at the top and under packages[""].
Set-InFile (Join-Path $root 'package-lock.json') $jsonVersion $jsonReplace 2
Set-InFile (Join-Path $root 'src-tauri/tauri.conf.json') $jsonVersion $jsonReplace
foreach ($manifest in $CrateManifests) {
    Set-InFile (Join-Path $root $manifest) '^(version\s*=\s*")[^"]+(")' $jsonReplace
}
foreach ($name in $CratePackages) {
    Set-InFile (Join-Path $root 'Cargo.lock') ('^(name = "' + [regex]::Escape($name) + '"\r?\nversion = ")[^"]+(")') $jsonReplace
}

$changelog = Join-Path $root 'CHANGELOG.md'
if ($null -ne (Get-ChangelogSection -Path $changelog -Heading $Version)) {
    Write-Host "CHANGELOG.md already has a $Version section; left it as it is."
} else {
    $nl = if ([IO.File]::ReadAllText($changelog).Contains("`r`n")) { "`r`n" } else { "`n" }
    Set-InFile $changelog '^## Unreleased[ \t]*$' ("## Unreleased$nl$nl## $Version - $Date")
}

Write-Host "Version set to $Version."
& (Join-Path $PSScriptRoot 'check-release.ps1') -Tag "v$Version"
