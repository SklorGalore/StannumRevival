<#
.SYNOPSIS
Tags a merged release and merges main back into development.

.DESCRIPTION
Git flow, step two, after the release/v<version> (or hotfix/v<version>)
pull request has been merged into main. The script finds the merge commit
on origin/main, checks the version and changelog there, tags it v<version>
and pushes the tag, which starts the Release workflow. Then it opens a pull
request from main into development so the version bump and any fixes made
on the release branch reach development. Merge that one with a merge
commit, not squash.

Tags matching v* can only be created by an admin.

.EXAMPLE
pwsh scripts/release/finish-release.ps1 -Version 0.4.0

.EXAMPLE
pwsh scripts/release/finish-release.ps1 -Version 0.4.0 -NoBackMerge
#>
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$')]
    [string] $Version,
    [switch] $NoBackMerge,
    [string] $Remote = 'origin'
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
. (Join-Path $PSScriptRoot 'release-common.ps1')
Push-Location $root
try {
    $tag = "v$Version"
    Invoke-Git fetch $Remote --tags --prune | Out-Null
    if (Invoke-Git tag --list $tag) { throw "Tag $tag already exists." }

    # The release PR's merge commit is main's newest commit whose second parent
    # is the release or hotfix branch. Find it from the PR, falling back to
    # main's tip, and check that the version there is right before tagging.
    $commit = $null
    foreach ($kind in 'release', 'hotfix') {
        $sha = gh pr list --base $MainBranch --head "$kind/$tag" --state merged --json mergeCommit --jq '.[0].mergeCommit.oid' 2>$null
        if ($LASTEXITCODE -eq 0 -and $sha) { $commit = $sha.Trim(); Write-Host "Found merged pull request from $kind/$tag"; break }
    }
    if (-not $commit) {
        $commit = (Invoke-Git rev-parse "$Remote/$MainBranch").Trim()
        Write-Warning "No merged pull request from release/$tag or hotfix/$tag found; using the tip of $Remote/$MainBranch."
    }
    & git merge-base --is-ancestor $commit "$Remote/$MainBranch"
    if ($LASTEXITCODE -ne 0) { throw "$commit is not on $Remote/$MainBranch." }

    # Check the files as they are in that commit, without touching the working tree.
    $tree = Join-Path ([IO.Path]::GetTempPath()) "stannum-release-$Version"
    if (Test-Path $tree) { Remove-Item -Recurse -Force $tree }
    New-Item -ItemType Directory -Path (Join-Path $tree 'src-tauri') -Force | Out-Null
    foreach ($file in 'package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'CHANGELOG.md') {
        Invoke-Git show "${commit}:$file" | Set-Content -Path (Join-Path $tree $file) -Encoding utf8NoBOM
    }
    try { & (Join-Path $PSScriptRoot 'check-release.ps1') -Tag $tag -Root $tree }
    finally { Remove-Item -Recurse -Force $tree }

    Write-Host "Tagging $commit as $tag"
    Invoke-Git tag --annotate $tag $commit --message "Stannum $Version" | Out-Null
    Invoke-Git push $Remote $tag | Out-Null
    Write-Host "Pushed $tag. The Release workflow will build a draft release."

    if ($NoBackMerge) { return }
    $ahead = [int](Invoke-Git rev-list --count "$Remote/$DevelopBranch..$commit")
    if ($ahead -eq 0) {
        Write-Host "$DevelopBranch already has $tag; nothing to merge back."
        return
    }
    gh pr create --base $DevelopBranch --head $MainBranch --title "Merge $tag back into $DevelopBranch" `
        --body "Brings the $tag version bump and release fixes from ``$MainBranch`` into ``$DevelopBranch``. Merge with **Create a merge commit**, not squash."
    if ($LASTEXITCODE -ne 0) { throw 'gh pr create failed' }
}
finally {
    Pop-Location
}
