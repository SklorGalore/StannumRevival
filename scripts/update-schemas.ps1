#!/usr/bin/env pwsh
# Write the core's command schemas again after a command or a type it
# returns changes:
#   crates/stannum-core/schema/commands.schema.json  (JSON Schema)
#   src/lib/analysis/commands.gen.ts                 (TypeScript types)
# and the case and lists file reference pages after a change to
# crates/stannum-format/schema:
#   docs-site/src/content/docs/reference/case-file-schema.md
#   docs-site/src/content/docs/reference/lists-file-schema.md
# CI fails while any is out of date.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$env:STANNUM_UPDATE_SCHEMAS = '1'
try {
    cargo test -p stannum-core --test schemas
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo test -p stannum-format --lib reference
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Remove-Item Env:STANNUM_UPDATE_SCHEMAS
}
Write-Host 'Schemas written: commands.schema.json, commands.gen.ts and the file reference pages'
