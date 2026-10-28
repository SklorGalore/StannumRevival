<#
.SYNOPSIS
Writes THIRD-PARTY-LICENSES.txt: the licence of every Rust crate and npm
package that is built into Stannum, with each licence text once.

.DESCRIPTION
Rust: the normal (not dev or build) dependencies of the app (stannum),
stannum-cli and the Python package (stannum-py), on the three release
platforms, as `cargo metadata` resolves them. npm: the packages package-lock.json does
not mark as dev-only, read from node_modules, so run `npm ci` first.

Licence files (LICENSE*, LICENCE*, COPYING*, NOTICE*, COPYRIGHT*) are read
from each package's own folder. A package offered under "MIT OR ..." is
used under MIT, so only its MIT text is kept. Identical texts (ignoring
spacing) are written once, under the list of packages that carry them. Packages that ship no licence file
are listed with the licence their metadata names.

The installers, the wheels and the stannum-cli archives ship this file
beside LICENSE and THIRD-PARTY-NOTICES.md. new-version.ps1 regenerates it
for each release, and the release workflow regenerates it before building.

.PARAMETER Out
File to write. Defaults to THIRD-PARTY-LICENSES.txt at the repository root.

.EXAMPLE
pwsh scripts/release/third-party-licenses.ps1
#>
param(
    [string] $Out
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $Out) { $Out = Join-Path $root 'THIRD-PARTY-LICENSES.txt' }

# The crates whose builds are shipped, and the platforms they are built for.
$shippedCrates = @('stannum', 'stannum-cli', 'stannum-py')
$platforms = @('x86_64-pc-windows-msvc', 'aarch64-apple-darwin', 'x86_64-unknown-linux-gnu')
$licenceFile = '^(licen[cs]e|copying|notice|copyright)([-_.].*)?$'

# Licence files in a package folder, as @{ Name; Text } in name order. When
# the package may be used under MIT, the files for its other choices
# (LICENSE-APACHE, LICENSE-ZLIB, ...) are left out.
function Get-LicenceTexts([string] $Dir, [string] $Licence) {
    if (-not (Test-Path $Dir)) { return @() }
    $files = @(Get-ChildItem -Path $Dir -File | Where-Object { $_.Name -match $licenceFile })
    if ($Licence -match '\bOR\b' -and $Licence -match '\bMIT\b') {
        $mitOnly = @($files | Where-Object { $_.Name -notmatch 'apache|zlib|unlicense|bsl|mpl|lgpl|gpl' })
        if ($mitOnly.Count) { $files = $mitOnly }
    }
    @($files |
        Sort-Object Name |
        ForEach-Object {
            $text = [IO.File]::ReadAllText($_.FullName).Replace("`r`n", "`n").Trim()
            if ($text) { [pscustomobject]@{ Name = $_.Name; Text = $text } }
        })
}

$packages = [Collections.Generic.List[object]]::new()

# --- Rust -------------------------------------------------------------------
$byId = @{}
$seen = [Collections.Generic.HashSet[string]]::new()
foreach ($platform in $platforms) {
    $meta = cargo metadata --format-version 1 --locked --filter-platform $platform `
        --manifest-path (Join-Path $root 'Cargo.toml') | ConvertFrom-Json -Depth 64
    foreach ($p in $meta.packages) { $byId[$p.id] = $p }
    $nodes = @{}
    foreach ($n in $meta.resolve.nodes) { $nodes[$n.id] = $n }
    $workspace = [Collections.Generic.HashSet[string]]::new([string[]] $meta.workspace_members)
    $queue = [Collections.Generic.Queue[string]]::new()
    foreach ($id in $meta.workspace_members) {
        if ($shippedCrates -contains $byId[$id].name) { $queue.Enqueue($id) }
    }
    $visited = [Collections.Generic.HashSet[string]]::new()
    while ($queue.Count) {
        $id = $queue.Dequeue()
        if (-not $visited.Add($id)) { continue }
        if (-not $workspace.Contains($id)) { [void] $seen.Add($id) }
        foreach ($dep in $nodes[$id].deps) {
            # Normal dependencies only: dev and build dependencies are not shipped.
            if ($dep.dep_kinds | Where-Object { $null -eq $_.kind }) { $queue.Enqueue($dep.pkg) }
        }
    }
}
foreach ($id in $seen) {
    $p = $byId[$id]
    $licence = $p.license ?? ($p.license_file ? "see $($p.license_file)" : 'not stated')
    $packages.Add([pscustomobject]@{
        Ecosystem = 'Rust crate'
        Name      = $p.name
        Version   = $p.version
        Licence   = $licence
        Source    = $p.repository ?? "https://crates.io/crates/$($p.name)"
        Texts     = Get-LicenceTexts (Split-Path $p.manifest_path) $licence
    })
}

# --- npm --------------------------------------------------------------------
$lock = Get-Content (Join-Path $root 'package-lock.json') -Raw | ConvertFrom-Json -AsHashtable
foreach ($entry in $lock.packages.GetEnumerator()) {
    $path = $entry.Key
    $info = $entry.Value
    if (-not $path -or $info.dev) { continue }
    $dir = Join-Path $root $path
    # Optional packages for other platforms are not installed here.
    if (-not (Test-Path $dir)) {
        if ($info.optional) { continue }
        throw "$path is not installed. Run npm ci first."
    }
    $pkg = Get-Content (Join-Path $dir 'package.json') -Raw | ConvertFrom-Json -AsHashtable
    $repo = $pkg.repository
    if ($repo -is [hashtable]) { $repo = $repo.url }
    $licence = $info.license ?? $pkg.license ?? 'not stated'
    $packages.Add([pscustomobject]@{
        Ecosystem = 'npm package'
        Name      = $pkg.name
        Version   = $pkg.version
        Licence   = $licence
        Source    = $repo ?? "https://www.npmjs.com/package/$($pkg.name)"
        Texts     = Get-LicenceTexts $dir $licence
    })
}

# --- Write ------------------------------------------------------------------
# Group packages by their exact licence texts, so each text is written once.
$groups = [ordered]@{}
$noText = [Collections.Generic.List[object]]::new()
foreach ($p in $packages | Sort-Object Ecosystem, Name, Version) {
    if (-not $p.Texts.Count) { $noText.Add($p); continue }
    $key = (($p.Texts | ForEach-Object Text) -join "`n`f`n") -replace '\s+', ' '
    if (-not $groups.Contains($key)) {
        $groups[$key] = [pscustomobject]@{ Texts = $p.Texts; Packages = [Collections.Generic.List[object]]::new() }
    }
    $groups[$key].Packages.Add($p)
}

$rule = '=' * 78
$thin = '-' * 78
$sb = [Text.StringBuilder]::new()
$line = { param($s) [void] $sb.Append($s).Append("`n") }
& $line 'Stannum: licences of third-party software'
& $line $rule
& $line ''
& $line 'Stannum is BSD-3-Clause (see LICENSE). It is built with the Rust crates and'
& $line 'npm packages below, which keep their own licences. Where a package offers a'
& $line 'choice of licences, Stannum uses it under any one of them. Notes on bundled'
& $line 'data and on code adapted from other projects are in THIRD-PARTY-NOTICES.md.'
& $line ''
& $line 'Generated by scripts/release/third-party-licenses.ps1; do not edit by hand.'
& $line ("{0} Rust crates and {1} npm packages." -f
    @($packages | Where-Object Ecosystem -eq 'Rust crate').Count,
    @($packages | Where-Object Ecosystem -eq 'npm package').Count)
& $line ''

$i = 0
foreach ($g in $groups.Values) {
    $i++
    & $line $rule
    & $line ("[{0}] Used by:" -f $i)
    foreach ($p in $g.Packages) {
        & $line ("  {0} {1} {2} ({3}) {4}" -f $p.Ecosystem, $p.Name, $p.Version, $p.Licence, $p.Source)
    }
    foreach ($t in $g.Texts) {
        & $line $thin
        & $line $t.Name
        & $line ''
        & $line $t.Text
    }
    & $line ''
}

if ($noText.Count) {
    & $line $rule
    & $line 'Packages that ship no licence file, with the licence their metadata names:'
    foreach ($p in $noText) {
        & $line ("  {0} {1} {2} ({3}) {4}" -f $p.Ecosystem, $p.Name, $p.Version, $p.Licence, $p.Source)
    }
}

[IO.File]::WriteAllText($Out, $sb.ToString())
Write-Host ("Wrote {0}: {1} packages, {2} distinct licence texts, {3:N0} KB." -f
    $Out, $packages.Count, $groups.Count, ((Get-Item $Out).Length / 1KB))
