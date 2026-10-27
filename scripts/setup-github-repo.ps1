<#
.SYNOPSIS
Apply Stannum's GitHub security settings and branch rulesets.

.DESCRIPTION
Uses the GitHub CLI (gh) to set the repository up the way
CONTRIBUTING.md describes:

- Merge settings: merge commits (needed for subtree history) and squash;
  rebase merging off; head branches deleted after merge; auto-merge on.
- Security: Dependabot alerts and security updates, secret scanning with
  push protection, private vulnerability reporting, CodeQL default setup.
- Actions: GITHUB_TOKEN read-only by default, workflows may not approve
  PRs, only GitHub-owned, verified and listed actions may run, and runs
  from outside contributors wait for approval.
- Pages: the documentation site is published by the Docs workflow
  (build type "workflow"), not from a branch.
- Rulesets: every .github/rulesets/*.json file is created, or updated in
  place when a ruleset with the same name already exists.

Every step can be re-run safely. Run it from a clone of the repository,
signed in as an admin (gh auth login). Use -WhatIf to see what it would
do, and -Audit to print the current settings without changing anything.

.PARAMETER Repo
owner/name. Defaults to SklorGalore/StannumRevival.

.PARAMETER Audit
Print the current settings and rulesets, change nothing.

.PARAMETER SkipCodeScanning
Leave CodeQL default setup alone.

.EXAMPLE
pwsh scripts/setup-github-repo.ps1 -WhatIf

.EXAMPLE
pwsh scripts/setup-github-repo.ps1

.EXAMPLE
pwsh scripts/setup-github-repo.ps1 -Audit
#>
[CmdletBinding(SupportsShouldProcess)]
param(
    [string]$Repo = 'SklorGalore/StannumRevival',
    [switch]$Audit,
    [switch]$SkipCodeScanning
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$rulesetDir = Join-Path $root '.github/rulesets'
$script:failures = @()

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'The GitHub CLI (gh) is not installed. See https://cli.github.com.'
}

# Calls gh api and returns parsed JSON (or $null for an empty body).
# Body objects are sent as JSON on stdin.
function Invoke-GhApi {
    param(
        [Parameter(Mandatory)][string]$Path,
        [string]$Method = 'GET',
        $Body
    )
    $ghArgs = @('api', '--method', $Method, '-H', 'Accept: application/vnd.github+json', $Path)
    if ($null -ne $Body) {
        $json = if ($Body -is [string]) { $Body } else { $Body | ConvertTo-Json -Depth 20 -Compress }
        $out = $json | gh @ghArgs --input - 2>&1
    } else {
        $out = gh @ghArgs 2>&1
    }
    if ($LASTEXITCODE -ne 0) { throw "gh api $Method $Path failed: $out" }
    $text = ($out | Out-String).Trim()
    if ($text) { $text | ConvertFrom-Json } else { $null }
}

# Runs one named step, honouring -WhatIf, and keeps going if it fails so a
# single unsupported setting does not stop the rest.
function Invoke-Step {
    param([string]$StepName, [scriptblock]$StepAction)
    if (-not $PSCmdlet.ShouldProcess($Repo, $StepName)) { return }
    try {
        & $StepAction
        Write-Host "  ok    $StepName" -ForegroundColor Green
    } catch {
        Write-Host "  FAIL  $StepName" -ForegroundColor Red
        Write-Host "        $($_.Exception.Message)" -ForegroundColor DarkGray
        $script:failures += $StepName
    }
}

function Show-Audit {
    $r = Invoke-GhApi "repos/$Repo"
    Write-Host "Repository $($r.full_name) ($($r.visibility)), default branch $($r.default_branch)"
    Write-Host ("  merge commit {0}, squash {1}, rebase {2}, delete branch on merge {3}, auto-merge {4}" -f `
        $r.allow_merge_commit, $r.allow_squash_merge, $r.allow_rebase_merge, $r.delete_branch_on_merge, $r.allow_auto_merge)
    if ($r.security_and_analysis) {
        $r.security_and_analysis.PSObject.Properties | ForEach-Object {
            Write-Host ("  {0}: {1}" -f $_.Name, $_.Value.status)
        }
    } else {
        Write-Host '  security_and_analysis: not visible (needs admin)'
    }
    try { Invoke-GhApi "repos/$Repo/vulnerability-alerts" | Out-Null; Write-Host '  dependabot alerts: enabled' }
    catch { Write-Host '  dependabot alerts: disabled or not visible' }
    try { $p = Invoke-GhApi "repos/$Repo/private-vulnerability-reporting"; Write-Host "  private vulnerability reporting: $($p.enabled)" }
    catch { Write-Host '  private vulnerability reporting: not visible' }
    try { $c = Invoke-GhApi "repos/$Repo/code-scanning/default-setup"; Write-Host "  code scanning default setup: $($c.state) ($($c.languages -join ', '))" }
    catch { Write-Host '  code scanning default setup: not visible' }
    try {
        $a = Invoke-GhApi "repos/$Repo/actions/permissions"
        $w = Invoke-GhApi "repos/$Repo/actions/permissions/workflow"
        Write-Host "  actions allowed: $($a.allowed_actions); token default: $($w.default_workflow_permissions); can approve PRs: $($w.can_approve_pull_request_reviews)"
    } catch { Write-Host '  actions permissions: not visible' }
    try { $pg = Invoke-GhApi "repos/$Repo/pages"; Write-Host "  pages: $($pg.build_type) at $($pg.html_url)" }
    catch { Write-Host '  pages: not enabled' }
    $rulesets = @(Invoke-GhApi "repos/$Repo/rulesets")
    Write-Host "Rulesets: $($rulesets.Count)"
    foreach ($rs in $rulesets) { Write-Host "  $($rs.name) [$($rs.target), $($rs.enforcement)]" }
}

if ($Audit) { Show-Audit; return }

Write-Host "Configuring $Repo"

Invoke-Step 'Merge settings (merge + squash, no rebase, delete branches, auto-merge)' {
    Invoke-GhApi "repos/$Repo" -Method PATCH -Body @{
        allow_merge_commit          = $true
        allow_squash_merge          = $true
        allow_rebase_merge          = $false
        allow_auto_merge            = $true
        allow_update_branch         = $true
        delete_branch_on_merge      = $true
        merge_commit_title          = 'PR_TITLE'
        merge_commit_message        = 'PR_BODY'
        squash_merge_commit_title   = 'PR_TITLE'
        squash_merge_commit_message = 'COMMIT_MESSAGES'
    } | Out-Null
}

Invoke-Step 'Dependabot alerts' {
    Invoke-GhApi "repos/$Repo/vulnerability-alerts" -Method PUT | Out-Null
}

Invoke-Step 'Secret scanning, push protection and Dependabot security updates' {
    Invoke-GhApi "repos/$Repo" -Method PATCH -Body @{
        security_and_analysis = @{
            secret_scanning                 = @{ status = 'enabled' }
            secret_scanning_push_protection = @{ status = 'enabled' }
            dependabot_security_updates     = @{ status = 'enabled' }
        }
    } | Out-Null
}

Invoke-Step 'Private vulnerability reporting' {
    Invoke-GhApi "repos/$Repo/private-vulnerability-reporting" -Method PUT | Out-Null
}

if (-not $SkipCodeScanning) {
    Invoke-Step 'CodeQL default setup (all detected languages)' {
        Invoke-GhApi "repos/$Repo/code-scanning/default-setup" -Method PATCH -Body @{
            state       = 'configured'
            query_suite = 'default'
        } | Out-Null
    }
}

Invoke-Step 'Actions: only GitHub-owned, verified and listed actions' {
    Invoke-GhApi "repos/$Repo/actions/permissions" -Method PUT -Body @{
        enabled         = $true
        allowed_actions = 'selected'
    } | Out-Null
    Invoke-GhApi "repos/$Repo/actions/permissions/selected-actions" -Method PUT -Body @{
        github_owned_allowed = $true
        verified_allowed     = $true
        patterns_allowed     = @('dtolnay/rust-toolchain@*', 'Swatinem/rust-cache@*')
    } | Out-Null
}

Invoke-Step 'Actions: read-only GITHUB_TOKEN, workflows cannot approve PRs' {
    Invoke-GhApi "repos/$Repo/actions/permissions/workflow" -Method PUT -Body @{
        default_workflow_permissions     = 'read'
        can_approve_pull_request_reviews = $false
    } | Out-Null
}

Invoke-Step 'Actions: outside contributors need approval before workflows run' {
    Invoke-GhApi "repos/$Repo/actions/permissions/fork-pr-contributor-approval" -Method PUT -Body @{
        approval_policy = 'all_external_contributors'
    } | Out-Null
}

Invoke-Step 'GitHub Pages: publish from the Docs workflow' {
    $pages = $null
    try { $pages = Invoke-GhApi "repos/$Repo/pages" } catch { }
    if ($pages) {
        Invoke-GhApi "repos/$Repo/pages" -Method PUT -Body @{ build_type = 'workflow' } | Out-Null
    } else {
        Invoke-GhApi "repos/$Repo/pages" -Method POST -Body @{ build_type = 'workflow' } | Out-Null
    }
}

$existing = @{}
foreach ($rs in @(Invoke-GhApi "repos/$Repo/rulesets")) { $existing[$rs.name] = $rs.id }
foreach ($file in Get-ChildItem $rulesetDir -Filter *.json | Sort-Object Name) {
    $body = Get-Content -Raw $file.FullName
    $rulesetName = ($body | ConvertFrom-Json).name
    # Build the path out here. Step blocks run inside Invoke-Step, so a
    # variable there that shares a name with its parameters gets theirs.
    $rulesetPath = if ($existing.ContainsKey($rulesetName)) { "repos/$Repo/rulesets/$($existing[$rulesetName])" }
    if ($rulesetPath) {
        Invoke-Step "Ruleset '$rulesetName' (update)" {
            Invoke-GhApi $rulesetPath -Method PUT -Body $body | Out-Null
        }
    } else {
        Invoke-Step "Ruleset '$rulesetName' (create)" {
            Invoke-GhApi "repos/$Repo/rulesets" -Method POST -Body $body | Out-Null
        }
    }
}

if ($WhatIfPreference) { return }

Write-Host ''
Show-Audit
if ($script:failures.Count) {
    Write-Host ''
    Write-Warning ("{0} step(s) failed: {1}. The rest were applied; see CONTRIBUTING.md for doing these by hand." -f `
        $script:failures.Count, ($script:failures -join '; '))
    exit 1
}
