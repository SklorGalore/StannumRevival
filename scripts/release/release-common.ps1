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
