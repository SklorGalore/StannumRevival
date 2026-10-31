# Helpers shared by the release scripts. Dot-source this file.

# The app version as written in each file that carries it.
function Get-AppVersions([string] $Root) {
    $package = Get-Content (Join-Path $Root 'package.json') -Raw | ConvertFrom-Json
    $tauri = Get-Content (Join-Path $Root 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
    $versions = [ordered]@{
        'package.json'              = $package.version
        'src-tauri/tauri.conf.json' = $tauri.version
    }
    # The CLI and the Python package ship with the app, so they share its version.
    foreach ($manifest in $CrateManifests) {
        $cargo = Get-Content (Join-Path $Root $manifest) -Raw
        if ($cargo -notmatch '(?ms)^\[package\][^\[]*?^version\s*=\s*"([^"]+)"') {
            throw "No [package] version in $manifest"
        }
        $versions[$manifest] = $Matches[1]
    }
    $versions
}

# Cargo.toml files whose [package] version is the app version.
$CrateManifests = @(
    'src-tauri/Cargo.toml'
    'crates/stannum-cli/Cargo.toml'
    'crates/stannum-py/Cargo.toml'
)

# Their package names, as Cargo.lock lists them.
$CratePackages = @('stannum', 'stannum-cli', 'stannum-py')

# The body of the "## <Heading>" section of a changelog, or $null if there is
# none. A heading may carry a date after the name: "## 0.3.0 - 2026-10-27".
function Get-ChangelogSection([string] $Path, [string] $Heading) {
    $lines = Get-Content -Path $Path
    $pattern = '^##\s+\[?' + [regex]::Escape($Heading) + '\]?(\s|$)'
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $pattern) { $start = $i + 1; break }
    }
    if ($start -lt 0) { return $null }
    $end = $lines.Count
    for ($i = $start; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^##\s') { $end = $i; break }
    }
    if ($end -le $start) { return '' }
    ($lines[$start..($end - 1)] -join "`n")
}

# Git flow branch names. Releases are cut from development as release/vX.Y.Z;
# urgent fixes to the released code are cut from main as hotfix/vX.Y.Z. Both
# are squashed into main, where that commit is tagged vX.Y.Z, and main is
# merged back into development with a merge commit.
$script:SemVerPattern = '\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?'
$script:DevelopBranch = 'development'
$script:MainBranch = 'main'

# The version a release or hotfix branch is for ("release/v0.3.0" -> "0.3.0"),
# or $null for any other branch. Accepts "refs/heads/..." too.
function Get-BranchVersion([string] $Branch) {
    $name = $Branch -replace '^refs/heads/', ''
    if ($name -match "^(release|hotfix)/v($script:SemVerPattern)$") { return $Matches[2] }
    $null
}

# Run git, failing on a non-zero exit. Returns its output lines.
function Invoke-Git {
    $out = & git @args
    if ($LASTEXITCODE -ne 0) { throw "git $($args -join ' ') failed" }
    $out
}

# The changelog for a back-merge after a release or hotfix was squashed into
# main: main's file (which has the new version's section), with every entry
# that development added under Unreleased since the release was cut, and that
# main doesn't hold, put back under main's Unreleased. Both are text.
function Merge-BackChangelog([string] $Main, [string] $Development) {
    $lines = $Development -split "`r?`n"
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^##\s+\[?Unreleased\]?(\s|$)') { $start = $i + 1; break }
    }
    $groups = [ordered]@{}
    if ($start -ge 0) {
        $heading = ''
        $entry = $null
        $flush = {
            if ($null -ne $entry) {
                if (-not $groups.Contains($heading)) { $groups[$heading] = [System.Collections.Generic.List[string]]::new() }
                $groups[$heading].Add(($entry -join "`n").TrimEnd())
            }
        }
        for ($i = $start; $i -lt $lines.Count; $i++) {
            $line = $lines[$i]
            if ($line -match '^##\s') { break }
            if ($line -match '^###\s') { . $flush; $entry = $null; $heading = $line.Trim(); continue }
            if ($line -match '^[-*]\s') { . $flush; $entry = [System.Collections.Generic.List[string]]::new(); $entry.Add($line); continue }
            if ($null -ne $entry -and ($line -match '^\s+\S' -or $line.Trim() -eq '')) { $entry.Add($line); continue }
        }
        . $flush
    }
    $mainNormal = ($Main -replace '\s+', ' ')
    $kept = [System.Collections.Generic.List[string]]::new()
    foreach ($heading in $groups.Keys) {
        $new = @($groups[$heading] | Where-Object { -not $mainNormal.Contains(($_ -replace '\s+', ' ').Trim()) })
        if ($new.Count -eq 0) { continue }
        if ($heading) { $kept.Add($heading); $kept.Add('') }
        foreach ($e in $new) { $kept.Add($e); $kept.Add('') }
    }
    if ($kept.Count -eq 0) { return $Main }
    $pattern = '(?m)^(##\s+\[?Unreleased\]?[^\r\n]*\r?\n)(\r?\n)?'
    if ($Main -notmatch $pattern) { throw "main's CHANGELOG.md has no '## Unreleased' heading." }
    $block = "`n" + ($kept -join "`n") + "`n"
    ([regex]$pattern).Replace($Main, { param($m) $m.Groups[1].Value + $block }, 1)
}
