<#
.SYNOPSIS
Fetch the large test cases listed in tests/large/cases.json.

.DESCRIPTION
Copies (from a folder) or downloads (from a base URL) each case, unzipping it
when the source is a .zip, into tests/large/, then checks its SHA-256 against
the manifest. Cases already present with the right hash are skipped.

The cases are not committed and have no public download here, because their
license does not state that they may be redistributed. Point -Source at a
folder that holds them, such as a synced copy of the project's Google Drive
folder, or at a private base URL.

.PARAMETER Source
A folder or an https:// base URL. Defaults to $env:STANNUM_TEST_DATA_SOURCE.

.PARAMETER Check
Only verify the files already in tests/large/; fetch nothing.

.EXAMPLE
pwsh scripts/fetch-test-cases.ps1 -Source "~/Google Drive/Stannum cases"

.EXAMPLE
pwsh scripts/fetch-test-cases.ps1 -Check
#>
[CmdletBinding()]
param(
    [string]$Source = $env:STANNUM_TEST_DATA_SOURCE,
    [switch]$Check
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dir = Join-Path $root 'tests/large'
$manifest = Get-Content -Raw (Join-Path $dir 'cases.json') | ConvertFrom-Json

function Test-Hash([string]$Path, [string]$Expected) {
    (Test-Path $Path) -and ((Get-FileHash -Algorithm SHA256 $Path).Hash -eq $Expected.ToUpperInvariant())
}

function Get-FromSource([string]$Name, [string]$Into) {
    if ($Source -match '^https://') {
        $url = $Source.TrimEnd('/') + '/' + [uri]::EscapeDataString($Name)
        try {
            Invoke-WebRequest -Uri $url -OutFile $Into -UseBasicParsing
            return $true
        } catch {
            return $false
        }
    }
    $path = Join-Path ([Environment]::ExpandEnvironmentVariables($Source.Replace('~', $HOME))) $Name
    if (Test-Path $path) {
        Copy-Item $path $Into
        return $true
    }
    return $false
}

$failed = 0
foreach ($case in $manifest.cases) {
    $target = Join-Path $dir $case.file
    if (Test-Hash $target $case.sha256) {
        Write-Host "ok       $($case.file)"
        continue
    }
    if ($Check) {
        $state = if (Test-Path $target) { 'MISMATCH' } else { 'missing ' }
        Write-Host "$state $($case.file)"
        $failed++
        continue
    }
    if (-not $Source) {
        throw 'Give -Source (a folder or https:// base URL) or set STANNUM_TEST_DATA_SOURCE.'
    }
    $got = $false
    foreach ($name in $case.sources) {
        $tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
        if (-not (Get-FromSource $name $tmp)) { continue }
        try {
            if ($name -like '*.zip') {
                $out = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
                Expand-Archive -Path $tmp -DestinationPath $out
                $member = Get-ChildItem -Recurse -File $out |
                    Where-Object { $_.Name -like $case.member -and $_.FullName -notmatch '__MACOSX' } |
                    Select-Object -First 1
                if ($member) { Move-Item -Force $member.FullName $target }
                Remove-Item -Recurse -Force $out
            } else {
                Move-Item -Force $tmp $target
            }
        } finally {
            if (Test-Path $tmp) { Remove-Item -Force $tmp }
        }
        if (Test-Hash $target $case.sha256) {
            Write-Host "fetched  $($case.file) from $name"
            $got = $true
            break
        }
        Write-Warning "$name did not give a $($case.file) with the expected SHA-256"
    }
    if (-not $got) {
        Write-Host "missing  $($case.file) (looked for: $($case.sources -join ', '))"
        $failed++
    }
}
if ($failed) { exit 1 }
