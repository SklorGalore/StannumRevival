<#
.SYNOPSIS
Cuts a release (or hotfix) branch, sets its version and opens its pull
request into main.

.DESCRIPTION
Git flow, step one. A release branch release/v<version> is made from the
latest origin/development; with -Hotfix, a hotfix branch hotfix/v<version>
is made from the latest origin/main instead. On the new branch the script
runs new-version.ps1, commits the result as "Release v<version>", pushes the
branch and opens a pull request into main.

A hotfix branch starts with no changelog notes, so the script stops after
cutting it. Commit the fix and its CHANGELOG.md line, then run the script
again on that branch to set the version, push and open the pull request.
The same goes for any release or hotfix branch: run on it, the script
carries on from there instead of cutting a new one.

Fixes found while the release is being tried go into the branch by pull
request. When the pull request into main is merged, run finish-release.ps1.

Needs a clean working tree, and for pushing and the pull request, the GitHub
CLI signed in.

.EXAMPLE
pwsh scripts/release/start-release.ps1 -Version 0.4.0

.EXAMPLE
pwsh scripts/release/start-release.ps1 -Version 0.3.1 -Hotfix

.EXAMPLE
pwsh scripts/release/start-release.ps1 -Version 0.4.0 -NoPush   # branch and commit only
#>
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$')]
    [string] $Version,
    [switch] $Hotfix,
    [switch] $NoPush,
    [string] $Remote = 'origin'
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
. (Join-Path $PSScriptRoot 'release-common.ps1')
Push-Location $root
try {
    if (Invoke-Git status --porcelain) { throw 'The working tree has uncommitted changes. Commit or stash them first.' }
    Invoke-Git fetch $Remote --tags --prune | Out-Null
    if (Invoke-Git tag --list "v$Version") { throw "Tag v$Version already exists." }

    # Run again on a branch this script cut, it carries on from there.
    $current = (Invoke-Git branch --show-current).Trim()
    $resume = $current -match '^(release|hotfix)/v' -and (Get-BranchVersion $current) -eq $Version
    if ($resume) {
        $branch = $current
        $Hotfix = $current.StartsWith('hotfix/')
    }
    $kind = if ($Hotfix) { 'hotfix' } else { 'release' }
    $base = if ($Hotfix) { $MainBranch } else { $DevelopBranch }
    if (-not $resume) {
        $branch = "$kind/v$Version"
        if (Invoke-Git branch --list --all $branch "$Remote/$branch") { throw "Branch $branch already exists. Switch to it and run this again to carry on." }
        $open = @(Invoke-Git branch --remotes --list "$Remote/release/v*" "$Remote/hotfix/v*") | ForEach-Object { $_.Trim() }
        if ($open) { Write-Warning "Other release branches are still open: $($open -join ', ')" }
        Write-Host "Cutting $branch from $Remote/$base"
        Invoke-Git switch --quiet --create $branch --no-track "$Remote/$base" | Out-Null
    }

    # The version's notes come from the Unreleased section. A hotfix branch
    # starts without any: stop so the fix and its changelog line go in first.
    $changelog = Join-Path $root 'CHANGELOG.md'
    $unreleased = Get-ChangelogSection -Path $changelog -Heading 'Unreleased'
    $hasNotes = ($null -ne (Get-ChangelogSection -Path $changelog -Heading $Version)) -or
        ($null -ne $unreleased -and $unreleased.Trim().Length -gt 0)
    if (-not $hasNotes) {
        if ($Hotfix) {
            Write-Host "On $branch. Commit the fix with a line for it under '## Unreleased' in CHANGELOG.md, then run this again."
            return
        }
        if (-not $resume) {
            Invoke-Git switch --quiet - | Out-Null
            Invoke-Git branch --quiet --delete $branch | Out-Null
        }
        throw "CHANGELOG.md on $base has nothing under '## Unreleased'. Add the release's notes there first."
    }

    & (Join-Path $PSScriptRoot 'new-version.ps1') -Version $Version
    if (Invoke-Git status --porcelain) {
        Invoke-Git add --all | Out-Null
        Invoke-Git commit --quiet --message "Release v$Version" | Out-Null
    }

    if ($NoPush) {
        Write-Host "Made $branch locally. Push it and open a pull request into $MainBranch when ready."
        return
    }

    Invoke-Git push --set-upstream $Remote $branch | Out-Null
    $existing = gh pr list --base $MainBranch --head $branch --state open --json url --jq '.[0].url' 2>$null
    if ($LASTEXITCODE -eq 0 -and $existing) {
        Write-Host "Pushed $branch. Its pull request is already open: $existing"
        return
    }
    $notes = Get-ChangelogSection -Path (Join-Path $root 'CHANGELOG.md') -Heading $Version
    $body = @"
Release v$Version, cut from ``$base``.

Merge with **Create a merge commit** (not squash), then run
``pwsh scripts/release/finish-release.ps1 -Version $Version`` to tag it and
merge ``$MainBranch`` back into ``$DevelopBranch``.

## Release notes

$($notes.Trim())
"@
    gh pr create --base $MainBranch --head $branch --title "Release v$Version" --body $body
    if ($LASTEXITCODE -ne 0) { throw 'gh pr create failed' }
}
finally {
    Pop-Location
}
