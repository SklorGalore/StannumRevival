<#
.SYNOPSIS
Tags a merged release and merges main back into development.

.DESCRIPTION
Git flow, step two, after the release/v<version> (or hotfix/v<version>)
pull request has been squashed into main. The script finds that commit on
origin/main, checks the version and changelog there, tags it v<version>
and pushes the tag, which starts the Release workflow.

Then it merges that commit into development on a backmerge/v<version>
branch and opens its pull request into development, so the version bump
and any fixes made on the release branch reach development, and the next
release squashes cleanly onto main. Git can't tell which Unreleased lines
were released, so CHANGELOG.md comes out as main's, with any lines that
development added under Unreleased since the cut kept there. Merge that
pull request with a merge commit, not squash.

Tags matching v* can only be created by an admin.

.EXAMPLE
pwsh scripts/release/finish-release.ps1 -Version 1.0.0

.EXAMPLE
pwsh scripts/release/finish-release.ps1 -Version 1.0.0 -NoBackMerge

.EXAMPLE
pwsh scripts/release/finish-release.ps1 -Version 1.0.0 -NoTag   # back-merge only, tag already pushed
#>
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$')]
    [string] $Version,
    [switch] $NoBackMerge,
    [switch] $NoTag,
    [string] $Remote = 'origin'
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
. (Join-Path $PSScriptRoot 'release-common.ps1')
Push-Location $root
try {
    $tag = "v$Version"
    if (-not $NoBackMerge -and (Invoke-Git status --porcelain)) {
        throw 'The working tree has uncommitted changes. Commit or stash them first.'
    }
    Invoke-Git fetch $Remote --tags --prune | Out-Null

    # The release PR's squashed commit on main. Find it from the PR, falling
    # back to main's tip, and check that the version there is right.
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

    if ($NoTag) {
        $tagged = Invoke-Git tag --list $tag
        if (-not $tagged) { throw "-NoTag was given, but $tag doesn't exist yet." }
        $commit = (Invoke-Git rev-parse "$tag^{commit}").Trim()
    }
    else {
        if (Invoke-Git tag --list $tag) { throw "Tag $tag already exists. Run with -NoTag to only merge it back." }

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
    }

    if ($NoBackMerge) { return }
    $ahead = [int](Invoke-Git rev-list --count "$Remote/$DevelopBranch..$commit")
    if ($ahead -eq 0) {
        Write-Host "$DevelopBranch already has $tag; nothing to merge back."
        return
    }

    $branch = "backmerge/$tag"
    Write-Host "Merging $tag into $DevelopBranch on $branch"
    Invoke-Git switch --quiet --force-create $branch --no-track "$Remote/$DevelopBranch" | Out-Null
    & git merge --no-ff --no-commit $commit 2>&1 | Out-Null
    $conflicts = @(Invoke-Git diff --name-only --diff-filter=U | Where-Object { $_ -ne 'CHANGELOG.md' })
    if ($conflicts) {
        throw "Merging $tag into $DevelopBranch conflicts in $($conflicts -join ', '). On $branch, resolve them, commit, push and open the pull request into $DevelopBranch."
    }
    $merged = Merge-BackChangelog `
        -Main ((Invoke-Git show "${commit}:CHANGELOG.md") -join "`n") `
        -Development ((Invoke-Git show "$Remote/${DevelopBranch}:CHANGELOG.md") -join "`n")
    Set-Content -Path (Join-Path $root 'CHANGELOG.md') -Value $merged -Encoding utf8NoBOM
    Invoke-Git add CHANGELOG.md | Out-Null
    Invoke-Git commit --quiet --message "Merge $tag back into $DevelopBranch" | Out-Null
    Invoke-Git push --set-upstream $Remote $branch | Out-Null

    $body = @"
Brings the $tag release commit from ``$MainBranch`` into ``$DevelopBranch``, so
the version bump and any release fixes reach ``$DevelopBranch`` and the next
release squashes cleanly onto ``$MainBranch``.

``CHANGELOG.md`` is ``$MainBranch``'s, with any lines added under Unreleased on
``$DevelopBranch`` since the release was cut kept there.

Merge with **Create a merge commit**, not squash: a squash would leave
``$DevelopBranch`` without ``$MainBranch``'s commit.
"@
    gh pr create --base $DevelopBranch --head $branch --title "Merge $tag back into $DevelopBranch" --body $body
    if ($LASTEXITCODE -ne 0) { throw 'gh pr create failed' }
}
finally {
    Pop-Location
}
