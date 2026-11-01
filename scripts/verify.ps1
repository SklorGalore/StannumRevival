<#
.SYNOPSIS
Run the reference tests and write the Verification page's table.

.DESCRIPTION
Runs every test that compares Stannum with a reference (MATPOWER, saved
solutions, the Beerten 2012 paper, MatACDC, DC solves, hand calculations)
with STANNUM_VERIFY set, so each one records its largest difference from the
reference and its tolerance. The collect tool then groups the records into
the JSON file the documentation's Verification page reads.

A test over its tolerance fails, and so does this script. The docs workflow
runs it before every build of the site, so the published numbers are the
ones from the commit the site was built from.

.PARAMETER Out
Where to write the table. Defaults to docs-site/src/data/verification.json.

.EXAMPLE
pwsh scripts/verify.ps1
#>
[CmdletBinding()]
param(
    [string]$Out = (Join-Path $PSScriptRoot '../docs-site/src/data/verification.json')
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$records = Join-Path ([IO.Path]::GetTempPath()) "stannum-verify-$PID.jsonl"
Remove-Item $records -ErrorAction SilentlyContinue
$env:STANNUM_VERIFY = $records

# Each group: the crate, then the test targets or the library test filter.
$runs = @(
    @('-p', 'rs-power', '--test', 'reference', '--test', 'raw_reference', '--test', 'transformer_detail'),
    @('-p', 'rs-power', '--lib', '--', 'vsc_grid::tests::reproduces_the_papers_converter_results'),
    @('-p', 'stannum-core', '--test', 'saved_solutions', '--test', 'matacdc_benchmark', '--test', 'contingency'),
    @('-p', 'stannum-core', '--lib', '--', 'transfer::tests::factors_match_dc_solves', 'pv::tests::matches_an_arclength_continuation')
)

Push-Location $root
try {
    foreach ($run in $runs) {
        & cargo test --locked --release @run
        if ($LASTEXITCODE -ne 0) { throw "cargo test $($run -join ' ') failed" }
    }
    $commit = (& git rev-parse HEAD).Trim()
    $date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    $dir = Split-Path -Parent $Out
    if ($dir) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    & cargo run --locked --release -q -p stannum-verify --bin collect -- $records $Out $commit $date
    if ($LASTEXITCODE -ne 0) { throw 'collect failed' }
}
finally {
    Pop-Location
    Remove-Item Env:STANNUM_VERIFY
    Remove-Item $records -ErrorAction SilentlyContinue
}
