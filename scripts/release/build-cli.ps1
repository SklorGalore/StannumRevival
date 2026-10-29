<#
.SYNOPSIS
Builds stannum-cli for this machine and packs it for a release.

.DESCRIPTION
Builds the release binary, checks that it runs (prints its version and
checks one of the repository's run files), and packs it with a short
README, LICENSE and the third-party notices and licences into
stannum-cli-<version>-<platform>.zip on Windows or .tar.gz elsewhere, in
-OutDir. The version is stannum-cli's own, which
check-release.ps1 holds to the app's.

.PARAMETER OutDir
Folder the archive is written to.

.PARAMETER Platform
Name of the platform in the file name, such as windows-x64. Defaults to
this machine's OS and processor.

.EXAMPLE
pwsh scripts/release/build-cli.ps1 -OutDir cli
#>
param(
    [Parameter(Mandatory)]
    [string] $OutDir,
    [string] $Platform
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')

if (-not $Platform) {
    $os = if ($IsWindows) { 'windows' } elseif ($IsMacOS) { 'macos' } else { 'linux' }
    $arch = switch ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
        'X64' { 'x64' }
        'Arm64' { 'arm64' }
        default { "$_".ToLowerInvariant() }
    }
    $Platform = "$os-$arch"
}

$cargo = Get-Content (Join-Path $root 'crates/stannum-cli/Cargo.toml') -Raw
if ($cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw 'No version in crates/stannum-cli/Cargo.toml' }
$version = $Matches[1]

cargo build --release --locked -p stannum-cli --manifest-path (Join-Path $root 'Cargo.toml')
$exe = Join-Path $root ('target/release/stannum-cli' + ($IsWindows ? '.exe' : ''))
if (-not (Test-Path $exe)) { throw "cargo wrote no binary at $exe." }

# It runs, and reads a run file.
& $exe --version
& $exe check (Join-Path $root 'crates/stannum-core/tests/fixtures/run_files/case9_study.stnrun')

$name = "stannum-cli-$version-$Platform"
$stage = Join-Path ([IO.Path]::GetTempPath()) $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item $exe $stage
foreach ($file in 'LICENSE', 'THIRD-PARTY-NOTICES.md', 'THIRD-PARTY-LICENSES.txt') {
    Copy-Item (Join-Path $root $file) $stage
}
@"
stannum-cli $version ($Platform)

Runs Stannum run files (.stnrun) without the app:

  stannum-cli run study.stnrun [--keep-going] [--json] [--verbose]
  stannum-cli check study.stnrun
  stannum-cli commands

Guide: https://sklorgalore.github.io/StannumRevival/guide/run-files/

Stannum is BSD-3-Clause (LICENSE). THIRD-PARTY-NOTICES.md and
THIRD-PARTY-LICENSES.txt list the third-party work it includes.
"@ | Set-Content -Path (Join-Path $stage 'README.txt') -Encoding utf8NoBOM

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = Resolve-Path $OutDir
if ($IsWindows) {
    $archive = Join-Path $OutDir "$name.zip"
    if (Test-Path $archive) { Remove-Item $archive }
    Compress-Archive -Path $stage -DestinationPath $archive
} else {
    $archive = Join-Path $OutDir "$name.tar.gz"
    tar -czf $archive -C ([IO.Path]::GetTempPath()) $name
}
Remove-Item -Recurse -Force $stage
Write-Host ("{0}  ({1:N1} MB)" -f (Split-Path $archive -Leaf), ((Get-Item $archive).Length / 1MB))
