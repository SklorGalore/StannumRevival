# Helpers shared by the release scripts. Dot-source this file.

# The app version as written in each file that carries it.
function Get-AppVersions([string] $Root) {
    $package = Get-Content (Join-Path $Root 'package.json') -Raw | ConvertFrom-Json
    $tauri = Get-Content (Join-Path $Root 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
    $cargo = Get-Content (Join-Path $Root 'src-tauri/Cargo.toml') -Raw
    if ($cargo -notmatch '(?ms)^\[package\][^\[]*?^version\s*=\s*"([^"]+)"') {
        throw 'No [package] version in src-tauri/Cargo.toml'
    }
    [ordered]@{
        'package.json'              = $package.version
        'src-tauri/tauri.conf.json' = $tauri.version
        'src-tauri/Cargo.toml'      = $Matches[1]
    }
}

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
