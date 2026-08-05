# Portable Windows build for Blow Your Phase Off.
#
#   powershell -ExecutionPolicy Bypass -File tools\package.ps1 [-Smoke]
#
# Stages dist\blow-your-phase-off-v<version>\ and zips it. Assets are copied
# from an EXPLICIT include list, never by blanket copy: the working tree
# deliberately carries untracked, unlicensed source material (loose .jpgs,
# docs/) that must not reach a shipped archive — the same discipline that
# made Xilla a history rewrite applies to everything here.
#
# -Smoke launches the staged exe for three seconds and fails the build if it
# dies early or falls back on missing fonts (the binary prints
# "fonts not found" to stderr when a face is absent).

param([switch]$Smoke)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path "crates\fibonacci-gui\Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$name = "blow-your-phase-off-v$version"
$stage = Join-Path $root "dist\$name"

cargo build --release -p fibonacci-gui
if ($LASTEXITCODE -ne 0) { throw "build failed" }

if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force "$stage\assets" | Out-Null

# The binary.
Copy-Item "target\release\blow-your-phase-off-gui.exe" "$stage\"

# The include list. Everything here is tracked in git and licence-vetted.
Copy-Item -Recurse "crates\fibonacci-gui\assets\fonts" "$stage\assets\fonts"
Copy-Item "crates\fibonacci-gui\assets\relic_log.json" "$stage\assets\"
Copy-Item "crates\fibonacci-gui\assets\RELIC_LOG.md" "$stage\assets\"
Copy-Item "README.md" "$stage\"
if (Test-Path "LICENSE") { Copy-Item "LICENSE" "$stage\" } else { Write-Warning "no LICENSE at repo root yet - the zip ships without one" }

# The starter preset bank, when it exists: every tracked .json in the preset
# dir except state.json (user data, never shipped).
$bank = Get-ChildItem "crates\fibonacci-gui\presets\*.json" -ErrorAction SilentlyContinue | Where-Object { $_.Name -ne "state.json" }
if ($bank) {
    New-Item -ItemType Directory -Force "$stage\presets" | Out-Null
    $bank | Copy-Item -Destination "$stage\presets\"
    Write-Host "bank: $($bank.Count) preset(s)"
} else {
    Write-Warning "no presets found - the zip ships without a starter bank"
}

if ($Smoke) {
    $p = Start-Process -FilePath "$stage\blow-your-phase-off-gui.exe" -WorkingDirectory $env:TEMP -RedirectStandardError "$stage\smoke-stderr.txt" -PassThru
    Start-Sleep -Seconds 3
    $alive = -not $p.HasExited
    if ($alive) { Stop-Process -Id $p.Id -Force }
    $stderr = Get-Content "$stage\smoke-stderr.txt" -Raw -ErrorAction SilentlyContinue
    Remove-Item "$stage\smoke-stderr.txt" -ErrorAction SilentlyContinue
    if (-not $alive) { throw "smoke: the staged exe exited within 3 s`n$stderr" }
    if ($stderr -match "fonts not found") { throw "smoke: staged exe fell back on missing fonts`n$stderr" }
    Write-Host "smoke: staged exe ran from a foreign cwd, all fonts resolved"
    Write-Host ($stderr.Trim())
}

$zip = "dist\$name-windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "wrote $zip ($([math]::Round((Get-Item $zip).Length / 1MB, 1)) MB)"
