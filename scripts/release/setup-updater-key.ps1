<#
.SYNOPSIS
Stores the updater signing key as the repository secrets the release workflow reads.

.DESCRIPTION
Run this once, on your own machine, with the GitHub CLI logged in as a
repository admin ("gh auth login"). It:

1. Checks that the key's public half (-KeyPath.pub) is the one the app
   trusts, src-tauri/updater-key.pub.
2. Asks for the key's password and checks it by signing a scratch file.
3. Stores the private key and its password as the repository secrets
   TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD.

The key itself is made once with "npx tauri signer generate -w <file>".
Back up the key file and its password: every installed copy of Stannum only
accepts updates signed with it.

.PARAMETER KeyPath
The private key file. Its public key must be beside it, with .pub.

.PARAMETER Repo
The repository to store the secrets in.

.EXAMPLE
pwsh scripts/release/setup-updater-key.ps1 -KeyPath ~/.tauri/stannum.key
#>
param(
    [Parameter(Mandatory)]
    [string] $KeyPath,
    [string] $Repo = 'SklorGalore/StannumRevival'
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')

foreach ($tool in 'npx', 'gh') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "$tool is not installed" }
}
& gh auth status --hostname github.com *> $null
if ($LASTEXITCODE -ne 0) { throw 'gh is not logged in. Run "gh auth login" first.' }

$KeyPath = (Resolve-Path $KeyPath).Path
$pubPath = "$KeyPath.pub"
if (-not (Test-Path $pubPath)) { throw "No public key at $pubPath" }
$trusted = (Get-Content -Path (Join-Path $root 'src-tauri/updater-key.pub') -Raw).Trim()
if ((Get-Content -Path $pubPath -Raw).Trim() -ne $trusted) {
    throw "$pubPath is not the public key in src-tauri/updater-key.pub. Use the matching key, or (for a new key) commit its .pub there first; copies built with the old key then can't update to builds signed with the new one."
}

$secure = Read-Host -AsSecureString 'Key password (press Enter if it has none)'
$password = [Net.NetworkCredential]::new('', $secure).Password

# A wrong password only shows up at the next release, so check it now.
$scratch = New-TemporaryFile
try {
    Set-Content -Path $scratch -Value 'stannum updater key check'
    if ($password) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $password }
    Push-Location $root
    try {
        & npx tauri signer sign -f $KeyPath $scratch.FullName | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'That password does not open the key. Nothing was uploaded.' }
    } finally {
        Pop-Location
        Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
    }
} finally {
    Remove-Item -Path $scratch, "$scratch.sig" -ErrorAction SilentlyContinue
}

# gh reads the values from standard input, so they never appear on a command line.
Get-Content -Path $KeyPath -Raw | & gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $Repo
if ($LASTEXITCODE -ne 0) { throw 'Could not set TAURI_SIGNING_PRIVATE_KEY' }
if ($password) {
    $password | & gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo $Repo
    if ($LASTEXITCODE -ne 0) { throw 'Could not set TAURI_SIGNING_PRIVATE_KEY_PASSWORD' }
}

Write-Host ''
Write-Host "Done. $Repo now signs its release installers for the updater."
Write-Host "Back up $KeyPath and its password. Without them, installed copies can't be updated."
