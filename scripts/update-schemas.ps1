#!/usr/bin/env pwsh
# Write the core's command schemas again after a command or a type it
# returns changes:
#   crates/stannum-core/schema/commands.schema.json  (JSON Schema)
#   src/lib/analysis/commands.gen.ts                 (TypeScript types)
# CI fails while either is out of date (stannum-core test "schemas").
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$env:STANNUM_UPDATE_SCHEMAS = '1'
try {
    cargo test -p stannum-core --test schemas
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Remove-Item Env:STANNUM_UPDATE_SCHEMAS
}
Write-Host 'Schemas written: commands.schema.json and commands.gen.ts'
