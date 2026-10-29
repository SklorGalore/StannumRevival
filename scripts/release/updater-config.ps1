<#
.SYNOPSIS
Writes the Tauri config overlay that turns the app's updater on.

.DESCRIPTION
Release builds pass this file to "tauri build --config". It makes the build
sign its installers for the updater (createUpdaterArtifacts, which needs
TAURI_SIGNING_PRIVATE_KEY in the environment) and gives the app the public
key it checks those signatures with and the address of the update manifest
(latest.json on the latest published GitHub release).

Builds without the overlay, such as "npm run tauri dev", have no updater.

.PARAMETER PubKey
The public key, as "tauri signer generate" writes it to the .pub file
(one line of base64). Release builds use src-tauri/updater-key.pub.

.PARAMETER Out
Where to write the overlay.

.PARAMETER Repo
The GitHub repository whose releases carry the updates.

.EXAMPLE
pwsh scripts/release/updater-config.ps1 -PubKey (Get-Content src-tauri/updater-key.pub -Raw) -Out src-tauri/tauri.updater.conf.json
#>
param(
    [Parameter(Mandatory)]
    [string] $PubKey,
    [Parameter(Mandatory)]
    [string] $Out,
    [string] $Repo = 'SklorGalore/StannumRevival'
)

$ErrorActionPreference = 'Stop'

$key = $PubKey.Trim()
# The .pub file is the base64 of a minisign public key, whose text starts
# "untrusted comment:". Catch a pasted private key or decoded text early.
try {
    $text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($key))
} catch {
    throw 'The updater public key is not base64. Use the whole .pub file that "tauri signer generate" wrote.'
}
if ($text -notmatch '^untrusted comment: minisign public key') {
    throw 'The updater public key is not a minisign public key. Use the .pub file, not the private key.'
}

$config = [ordered]@{
    bundle  = [ordered]@{ createUpdaterArtifacts = $true }
    plugins = [ordered]@{
        updater = [ordered]@{
            pubkey               = $key
            endpoints            = @("https://github.com/$Repo/releases/latest/download/latest.json")
            # The signature carries the version it was signed for, so an
            # old (validly signed) installer can't be offered as a new one.
            requireSignedVersion = $true
            # Windows: show the installer's progress but ask nothing.
            windows              = [ordered]@{ installMode = 'passive' }
        }
    }
}
$config | ConvertTo-Json -Depth 6 | Set-Content -Path $Out -Encoding utf8NoBOM
Write-Host "Wrote the updater config to $Out"
