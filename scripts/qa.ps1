# Runs all QA tests in mock mode; prints pass/fail table. Exit 1 on any failure.
$ErrorActionPreference = 'Continue'
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = 'F:\bgbuild\a15\AnimPerf' }
$root = Split-Path $PSScriptRoot -Parent
$mockDir = Join-Path ([IO.Path]::GetTempPath()) ("bgqa-ps-" + [guid]::NewGuid())
New-Item -ItemType Directory $mockDir | Out-Null
$env:BREAKGUARD_DDC_MOCK = Join-Path $mockDir 'mock.log'
$env:BREAKGUARD_DATA_DIR = $mockDir
Push-Location $root
$out = & cargo test -j 2 --release --offline 2>&1 | Out-String
Pop-Location
$rows = [regex]::Matches($out, 'test (\S+) \.\.\. (ok|FAILED|ignored)') | ForEach-Object { [pscustomobject]@{ Test = $_.Groups[1].Value; Result = $_.Groups[2].Value } }
$rows | Format-Table -AutoSize
$fail = @($rows | Where-Object Result -eq 'FAILED').Count
$pass = @($rows | Where-Object Result -eq 'ok').Count
Write-Host "PASS=$pass FAIL=$fail"
if ($rows.Count -eq 0) { Write-Host $out; $fail = 1 }
Remove-Item -Recurse -Force $mockDir -ErrorAction SilentlyContinue
if ($fail -gt 0) { Write-Host 'QA RESULT: FAIL'; exit 1 } else { Write-Host 'QA RESULT: PASS'; exit 0 }
