<#
.SYNOPSIS
Download MatACDC 1.0 for the optional comparison tests.

.DESCRIPTION
Downloads MatACDC 1.0 from KU Leuven and unpacks it into tests/matacdc/,
which git ignores. MatACDC may be used freely but not redistributed, so it is
never committed. Then run the comparison tests with

    $env:STANNUM_MATACDC = '<printed folder>'
    cargo test -p rs-power --test matacdc -- --ignored

.PARAMETER Url
Where to download the MatACDC 1.0 zip from.

.EXAMPLE
pwsh scripts/fetch-matacdc.ps1
#>
[CmdletBinding()]
param(
    [string]$Url = 'https://www.esat.kuleuven.be/electa/teaching/matacdc/MatACDC1_0'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dir = Join-Path $root 'tests/matacdc'
New-Item -ItemType Directory -Force -Path $dir | Out-Null

$found = Get-ChildItem -Path $dir -Recurse -Filter 'runacdcpf.m' -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $found) {
    $zip = Join-Path $dir 'MatACDC1_0.zip'
    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $dir -Force
    Remove-Item $zip
    $found = Get-ChildItem -Path $dir -Recurse -Filter 'runacdcpf.m' | Select-Object -First 1
    if (-not $found) { throw "The download did not contain runacdcpf.m." }
}

$folder = $found.Directory.FullName
Write-Host "MatACDC is in $folder"
Write-Host "Set STANNUM_MATACDC to that folder to run: cargo test -p rs-power --test matacdc -- --ignored"
