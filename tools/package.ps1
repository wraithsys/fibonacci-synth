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

# The shipped preset banks: the folders under presets/, each one a bank whose
# folder name is its name in the app and its contributor's credit.
#
# Asked of git, not of the filesystem (2026-08-06). The old form globbed
# `presets\*.json` and would have swept up whatever the player had saved that
# afternoon — the root of presets/ is private user data and has no business in
# a zip. Banks are folders and every file in one is tracked, so "what git knows
# about" and "what ships" are now the same set by construction.
$rel = @(& git -c core.quotePath=false ls-files "crates/fibonacci-gui/presets/*/*.json")
if ($LASTEXITCODE -ne 0) { throw "package: could not ask git for the preset banks" }
if ($rel) {
    foreach ($f in $rel) {
        # crates/fibonacci-gui/presets/<bank>/<preset>.json
        $parts = $f -split "/"
        $dest = Join-Path $stage (Join-Path "presets" $parts[-2])
        New-Item -ItemType Directory -Force $dest | Out-Null
        Copy-Item ($f -replace "/", "\") -Destination $dest
    }
    $banks = $rel | ForEach-Object { ($_ -split "/")[-2] } | Sort-Object -Unique
    Write-Host "banks: $($banks -join ', ') - $($rel.Count) preset(s)"
} else {
    Write-Warning "no preset banks found - the zip ships without a starter bank"
}

if ($Smoke) {
    # The capture lives OUTSIDE $stage. It used to be written inside it and
    # deleted before zipping, which worked until it didn't: Stop-Process
    # -Force returns before Windows releases the redirect handle, so the
    # delete failed, -ErrorAction SilentlyContinue swallowed it, and
    # smoke-stderr.txt shipped inside the v1.0.1 zip. A transient file has no
    # business in the directory you are about to archive.
    $smokeLog = Join-Path ([IO.Path]::GetTempPath()) "bypo-smoke-stderr.txt"
    $p = Start-Process -FilePath "$stage\blow-your-phase-off-gui.exe" -WorkingDirectory $env:TEMP -RedirectStandardError $smokeLog -PassThru
    Start-Sleep -Seconds 3
    $alive = -not $p.HasExited
    if ($alive) { Stop-Process -Id $p.Id -Force }
    $stderr = Get-Content $smokeLog -Raw -ErrorAction SilentlyContinue
    Remove-Item $smokeLog -ErrorAction SilentlyContinue
    if (-not $alive) { throw "smoke: the staged exe exited within 3 s`n$stderr" }
    if ($stderr -match "fonts not found") { throw "smoke: staged exe fell back on missing fonts`n$stderr" }
    Write-Host "smoke: staged exe ran from a foreign cwd, all fonts resolved"
    Write-Host ($stderr.Trim())
}

# Nothing ships that nobody put there on purpose. The include list above is
# the whole contract, so anything else at the root of $stage is a leak - a
# stray log, an editor backup, a half-copied asset - and a public zip is the
# wrong place to discover it.
$expected = @("blow-your-phase-off-gui.exe", "README.md", "LICENSE", "assets", "presets")
$strays = Get-ChildItem $stage | Where-Object { $expected -notcontains $_.Name }
if ($strays) { throw "package: unexpected file(s) staged for the zip - $($strays.Name -join ', ')" }

$zip = "dist\$name-windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "wrote $zip ($([math]::Round((Get-Item $zip).Length / 1MB, 1)) MB)"

# Checksums, written every build rather than by hand.
#
# This instrument ships unsigned, and Windows Defender's ML classifier calls
# new unsigned binaries Trojan:Win32/Wacatac.C!ml on sight. Every other
# reassurance in the README asks the reader to trust a claim; a hash is the
# one thing they can check for themselves. Both files are listed: the zip is
# what people download, and the exe is what Defender objects to and what a
# false-positive submission to Microsoft is keyed on.
#
# Format is `<hash>  <name>` - two spaces, the coreutils convention - so
# `sha256sum -c` accepts the file unmodified. On Windows: Get-FileHash.
$sums = "dist\$name-SHA256SUMS.txt"
$lines = foreach ($f in @($zip, "$stage\blow-your-phase-off-gui.exe")) {
    $h = Get-FileHash $f -Algorithm SHA256
    "$($h.Hash.ToLower())  $(Split-Path $f -Leaf)"
}
Set-Content -Path $sums -Value $lines -Encoding ascii
Write-Host "wrote $sums"
$lines | ForEach-Object { Write-Host "  $_" }
