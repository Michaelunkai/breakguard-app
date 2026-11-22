# One-command health check: build, tests, QA, idle CPU, installed exe. Everything runs in mock mode.
# Usage: pwsh scripts\check-all.ps1
# Never touches the installed breakguard.exe in the repo root (that is the user's live app).
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$env:CARGO_TARGET_DIR = 'F:\bgbuild\a15\QAShots'
$exe = 'F:\bgbuild\a15\QAShots\x86_64-pc-windows-gnu\release\breakguard.exe'
$fail = 0
function Step($name, [scriptblock]$b) {
  Write-Host "== $name"
  $prev = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
  $o = & $b 2>&1 | Out-String
  $code = $LASTEXITCODE
  $ErrorActionPreference = $prev
  if ($code -ne 0) { Write-Host $o; Write-Host "FAIL: $name"; $script:fail++ } else { Write-Host "ok: $name" }
}
Push-Location $root
try {
  Step 'build release' { cargo build --release --offline }
  Step 'clippy' { cargo clippy --release --offline -- -D warnings }
  Step 'cargo test' { cargo test --release --offline }
  Step 'qa.ps1' { pwsh -NoProfile -File scripts\qa.ps1 }
  Step 'idle cpu' { pwsh -NoProfile -File scripts\idle-cpu.ps1 -Exe $exe }
} finally { Pop-Location }
if ($fail -eq 0) { 'ALL OK' } else { "FAILED STEPS: $fail"; exit 1 }
