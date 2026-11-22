# Clean reproducible release build. Usage: scripts\build.ps1 [-StageDir <path>]
param([string]$StageDir = "F:\bgbuild\stage")
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR = 'F:\bgbuild\release'
Set-Location $root
$ErrorActionPreference = 'Continue'
cargo clean --release --offline 2>&1 | Out-Null
cargo build --release --offline
if ($LASTEXITCODE -ne 0) { throw "build failed" }
New-Item -ItemType Directory -Force $StageDir | Out-Null
$exe = @('release\breakguard.exe','x86_64-pc-windows-gnu\release\breakguard.exe','x86_64-pc-windows-msvc\release\breakguard.exe') |
  ForEach-Object { Join-Path $env:CARGO_TARGET_DIR $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $exe) { throw "exe not found" }
Copy-Item $exe (Join-Path $StageDir 'breakguard.exe') -Force
$h = Get-FileHash (Join-Path $StageDir 'breakguard.exe') -Algorithm SHA256
"{0}  {1} bytes  sha256 {2}" -f $StageDir, (Get-Item $h.Path).Length, $h.Hash
