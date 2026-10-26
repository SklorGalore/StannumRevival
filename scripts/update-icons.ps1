#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Regenerates every Stannum icon from the two master SVGs in src/assets.

.DESCRIPTION
  src/assets/stannum-logo.svg        full mark (app icon, docs-site logo, 48 px and up)
  src/assets/stannum-logo-small.svg  heavier drawing for 16 to 32 px (favicons, module rail, docs header, small .ico sizes)

  Writes the Tauri icon set in src-tauri/icons (PNG, .icns, and an .ico whose 16/24/32 px
  entries come from the small drawing), and copies the SVGs to the app favicon and the docs site.
  Needs the npm dev dependencies (npx tauri) and Python 3 for assembling the .ico.

.EXAMPLE
  pwsh scripts/update-icons.ps1
#>
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$full = 'src/assets/stannum-logo.svg'
$small = 'src/assets/stannum-logo-small.svg'
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "stannum-icons-$([guid]::NewGuid())"
$tmpFull = Join-Path $tmp 'full'
$tmpSmall = Join-Path $tmp 'small'

try {
  npx tauri icon $full -o $tmpFull 2>&1 | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'tauri icon failed for the full mark' }
  npx tauri icon $small -o $tmpSmall 2>&1 | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'tauri icon failed for the small mark' }

  $icons = 'src-tauri/icons'
  foreach ($f in '64x64.png', '128x128.png', '128x128@2x.png', 'icon.png', 'icon.icns') {
    Copy-Item (Join-Path $tmpFull $f) (Join-Path $icons $f) -Force
  }
  Copy-Item (Join-Path $tmpSmall '32x32.png') (Join-Path $icons '32x32.png') -Force
  Copy-Item $full (Join-Path $icons 'icon.svg') -Force

  # .ico: small drawing for 16/24/32 px, full mark for 48/64/256 px. tauri writes PNG entries
  # in that size order, so take entries 0-2 from one file and 3-5 from the other.
  $py = @'
import struct, sys
def entries(path):
    d = open(path, 'rb').read()
    n = struct.unpack_from('<H', d, 4)[0]
    out = []
    for i in range(n):
        w, h, _, _, _, _, size, off = struct.unpack_from('<BBBBHHII', d, 6 + 16 * i)
        out.append((w or 256, d[off:off + size]))
    return out
small, full, dest = sys.argv[1:4]
pick = [e for e in entries(small) if e[0] <= 32] + [e for e in entries(full) if e[0] > 32]
head = struct.pack('<HHH', 0, 1, len(pick))
off = 6 + 16 * len(pick)
dirs = b''
body = b''
for w, png in pick:
    dirs += struct.pack('<BBBBHHII', w % 256, w % 256, 0, 0, 1, 32, len(png), off)
    off += len(png)
    body += png
open(dest, 'wb').write(head + dirs + body)
'@
  $pyFile = Join-Path $tmp 'ico.py'
  Set-Content -Path $pyFile -Value $py
  $python = if (Get-Command python3 -ErrorAction SilentlyContinue) { 'python3' } else { 'python' }
  & $python $pyFile (Join-Path $tmpSmall 'icon.ico') (Join-Path $tmpFull 'icon.ico') (Join-Path $icons 'icon.ico')
  if ($LASTEXITCODE -ne 0) { throw 'assembling icon.ico failed' }

  Copy-Item $small 'public/favicon.svg' -Force
  Copy-Item $full 'docs-site/src/assets/logo.svg' -Force
  Copy-Item $small 'docs-site/src/assets/logo-small.svg' -Force
  Copy-Item $small 'docs-site/public/favicon.svg' -Force
  Write-Host 'Icons updated.'
}
finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
