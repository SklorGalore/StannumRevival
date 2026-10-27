<#
.SYNOPSIS
Builds the stannum Python wheel, installs it in a fresh virtual environment
and runs the package's tests against it.

.DESCRIPTION
The wheel comes from crates/stannum-py with maturin. It is abi3, so the one
wheel works on Python 3.10 and newer. The tests run against the installed
wheel, not the source folder, so they check what a user would get.

.PARAMETER Python
The Python to build for and test with. Defaults to python3, or python where
there is no python3 (Windows).

.PARAMETER Out
Folder the wheel is written to. Defaults to dist-py at the repository root.

.PARAMETER Venv
Folder for the virtual environment. Defaults to .venv-stannum at the
repository root; it is made again on every run.

.PARAMETER Manylinux
Linux only: link against an old glibc with zig, so the wheel is tagged
manylinux2014 and installs on any Linux from about 2014 on. The release
workflow builds its Linux wheel this way; without it the wheel only fits
Linux as new as the build machine.

.EXAMPLE
pwsh scripts/ci/test-python.ps1

.EXAMPLE
pwsh scripts/ci/test-python.ps1 -Python python3.12 -Out wheels/

.EXAMPLE
pwsh scripts/ci/test-python.ps1 -Manylinux -Out wheels/
#>
[CmdletBinding()]
param(
    [string] $Python,
    [string] $Out,
    [string] $Venv,
    [switch] $Manylinux
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
$crate = Join-Path $root 'crates/stannum-py'
if (-not $Out) { $Out = Join-Path $root 'dist-py' }
if (-not $Venv) { $Venv = Join-Path $root '.venv-stannum' }
if (-not $Python) {
    $Python = if (Get-Command python3 -ErrorAction SilentlyContinue) { 'python3' } else { 'python' }
}

Write-Host "Python: $(& $Python --version)"

if (Test-Path $Venv) { Remove-Item -Recurse -Force $Venv }
& $Python -m venv $Venv
$bin = if ($IsWindows) { Join-Path $Venv 'Scripts' } else { Join-Path $Venv 'bin' }
$py = Join-Path $bin ($IsWindows ? 'python.exe' : 'python')

& $py -m pip install --quiet --upgrade pip
& $py -m pip install --quiet 'maturin>=1.9,<2' 'pytest>=7'
# maturin finds zig, and anything else it runs, on the PATH.
$env:PATH = $bin + [IO.Path]::PathSeparator + $env:PATH

$extra = @()
if ($Manylinux) {
    if (-not $IsLinux) { throw '-Manylinux only applies on Linux.' }
    & $py -m pip install --quiet 'ziglang<1'
    $extra = @('--zig', '--compatibility', 'manylinux2014')
}

# A wheel left from an earlier build would be picked up below instead.
if (Test-Path $Out) {
    Get-ChildItem $Out -Filter 'stannum-*.whl' | Remove-Item -Force
}
& $py -m maturin build --release --locked --manifest-path (Join-Path $crate 'Cargo.toml') --out $Out @extra

$wheel = Get-ChildItem $Out -Filter 'stannum-*.whl' | Select-Object -First 1
if (-not $wheel) { throw "maturin wrote no wheel to $Out." }
Write-Host "Built $($wheel.Name)"
& $py -m pip install --quiet $wheel.FullName

# Run from the crate folder so pytest reads its settings, but import the
# installed package: the source folder isn't on the path.
Push-Location $crate
try {
    & $py -m pytest -q -p no:cacheprovider
} finally {
    Pop-Location
}
