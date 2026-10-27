<#
.SYNOPSIS
Build a synthetic test case of any size.

.DESCRIPTION
Runs the stannum-synth generator (the method of Birchfield et al., IEEE
Trans. Power Systems, 2017) and writes the case as MATPOWER, with bus
coordinates, a validation report and a map. The same seed always builds the
same case. See docs-site/src/content/docs/manual/synthetic-cases.md.

.PARAMETER Buses
About how many buses the case should have.

.PARAMETER Seed
Random seed; the same seed builds the same case.

.PARAMETER Config
A JSON file of settings (see stannum_synth::Config). -Buses and -Seed
override it.

.PARAMETER Name
File name for the outputs. Defaults to synthetic_<buses>.

.PARAMETER Out
Folder to write into. Defaults to the current folder.

.EXAMPLE
pwsh scripts/new-synthetic-case.ps1 -Buses 2000 -Seed 1 -Out cases/

.EXAMPLE
pwsh scripts/new-synthetic-case.ps1 -Buses 80000 -Out tests/large/
#>
[CmdletBinding()]
param(
    [int]$Buses = 2000,
    [long]$Seed = 1,
    [string]$Config,
    [string]$Name,
    [string]$Out = "."
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

# Paths are relative to where the script was run from.
$Out = [System.IO.Path]::GetFullPath($Out, (Get-Location).Path)
$arguments = @("--buses", $Buses, "--seed", $Seed, "--out", $Out)
if ($Config) { $arguments += @("--config", (Resolve-Path $Config).Path) }
if ($Name) { $arguments += @("--name", $Name) }

Push-Location $root
try {
    cargo run --release --locked -p stannum-synth --example synth_case -- @arguments
    if ($LASTEXITCODE -ne 0) { throw "The generator failed (exit code $LASTEXITCODE)." }
}
finally {
    Pop-Location
}
