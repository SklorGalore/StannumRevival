<#
.SYNOPSIS
    Fuzzes Stannum's file readers with cargo-fuzz.

.DESCRIPTION
    Runs the cargo-fuzz targets in fuzz/ (raw, matpower, run_file,
    coordinates, stn), each for the given time, seeded with the test fixtures of
    the same kind. The corpus each run grows is kept in fuzz/corpus/<target>,
    so the next run starts where this one stopped.

    A crash stops that target and leaves its input in
    fuzz/artifacts/<target>/. Reproduce it with
    `cargo +nightly fuzz run <target> <file>`, fix the reader so the file
    gives an error, and add the input as a test in
    crates/stannum-core/tests/bad_files.rs.

    Needs the nightly toolchain and cargo-fuzz:
        rustup toolchain install nightly
        cargo install cargo-fuzz --locked

.PARAMETER Target
    Targets to run. All five when left out.

.PARAMETER Seconds
    How long to fuzz each target.

.PARAMETER Jobs
    Fuzzing processes per target.

.EXAMPLE
    pwsh scripts/fuzz.ps1 -Target raw -Seconds 600
#>
[CmdletBinding()]
param(
    [ValidateSet('raw', 'matpower', 'run_file', 'coordinates', 'stn')]
    [string[]] $Target = @('raw', 'matpower', 'run_file', 'coordinates', 'stn'),
    [int] $Seconds = 60,
    [int] $Jobs = 1
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$fuzz = Join-Path $root 'fuzz'

# Fixtures of each kind seed the corpus. Large cases are left out: libFuzzer
# reads at most -max_len bytes of each input.
$seedPatterns = @{
    raw         = @('*.raw')   # wildcards ignore case, so .RAW too
    matpower    = @('*.m')
    run_file    = @('*.stnrun')
    coordinates = @('*.csv')
    stn         = @('*.stn', '*.stnb', '*.stnl')
}
$fixtureFolders = @(
    'crates/rs-power/tests/fixtures',
    'crates/stannum-core/tests/fixtures',
    'crates/stannum-format/conformance'
) | ForEach-Object { Join-Path $root $_ }

$failed = @()
foreach ($t in $Target) {
    $corpus = Join-Path $fuzz "corpus/$t"
    $seeds = Join-Path $fuzz "seeds/$t"
    New-Item -ItemType Directory -Force -Path $corpus, $seeds | Out-Null
    Get-ChildItem -Path $fixtureFolders -Recurse -File -Include $seedPatterns[$t] |
        Where-Object { $_.Length -le 65536 } |
        ForEach-Object { Copy-Item $_.FullName (Join-Path $seeds $_.Name) -Force }

    $fuzzArgs = @("-max_total_time=$Seconds", '-max_len=65536')
    $dict = Join-Path $fuzz "dicts/$t.dict"
    if (Test-Path $dict) { $fuzzArgs += "-dict=$dict" }
    if ($Jobs -gt 1) { $fuzzArgs += "-fork=$Jobs" }

    Write-Host "Fuzzing $t for $Seconds s"
    Push-Location $fuzz
    try {
        & cargo +nightly fuzz run $t $corpus $seeds -- @fuzzArgs
        if ($LASTEXITCODE -ne 0) { $failed += $t }
    }
    finally {
        Pop-Location
    }
}

if ($failed) {
    Write-Error "Crashes found in: $($failed -join ', '). Inputs are in fuzz/artifacts/."
}
Write-Host 'No crashes found.'
