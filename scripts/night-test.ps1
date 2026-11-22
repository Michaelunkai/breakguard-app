# Night on/off button state: persisted night_force in settings.ini drives the (mock) overlay.
# Case A: force=1 outside any schedule -> "night overlay level" in mock log. Case B: force=2 inside the window -> none.
# Mock mode, temp data dir, kills only the PID it started.
param([string]$Exe = 'F:\bgbuild\a15\QAShots\x86_64-pc-windows-gnu\debug\breakguard.exe', [int]$Monitor = 0)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
# Second monitor only: the app window opens there from its first frame (logical point position).
$second = @([Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary })
if ($second.Count -eq 0) { throw 'No second monitor found; refusing to run on the primary.' }
$mon = $second[[Math]::Min($Monitor, $second.Count - 1)].Bounds
$dpiScale = ([Drawing.Graphics]::FromHwnd([IntPtr]::Zero)).DpiX / 96
$env:BREAKGUARD_ONLY_MONITOR = "$($mon.X),$($mon.Y),$($mon.Width),$($mon.Height)"
$env:BREAKGUARD_WINDOW_POS = "$([int][Math]::Round(($mon.X + 100) / $dpiScale)),$([int][Math]::Round(($mon.Y + 100) / $dpiScale))"
$fail = @()
foreach ($case in @(
  @{ name = 'forced on';  ini = "night_enabled=0`nnight_force=1`nnight_level=40`n"; want = $true },
  @{ name = 'forced off'; ini = "night_enabled=1`nnight_start=00:00`nnight_end=23:59`nnight_force=2`nnight_level=40`n"; want = $false },
  @{ name = 'schedule off, auto'; ini = "night_enabled=0`nnight_force=0`nnight_level=40`n"; want = $false })) {
  $d = Join-Path $env:TEMP "bgnight-$PID-$([guid]::NewGuid().ToString('N').Substring(0,6))"
  New-Item -ItemType Directory -Force "$d\data" | Out-Null
  [IO.File]::WriteAllText("$d\data\settings.ini", $case.ini, [Text.Encoding]::ASCII)
  Copy-Item $Exe "$d\bg.exe"
  $env:BREAKGUARD_DDC_MOCK = "$d\mock.log"; $env:BREAKGUARD_DATA_DIR = "$d\data"; $env:BREAKGUARD_SESSIONS_DIR = "$d\sess"
  $p = Start-Process "$d\bg.exe" -PassThru
  try { Start-Sleep -Seconds 4 } finally { if (-not $p.HasExited) { Stop-Process -Id $p.Id } }
  $log = if (Test-Path "$d\mock.log") { Get-Content "$d\mock.log" -Raw } else { '' }
  $got = $log -match 'night overlay level 40'
  $ini = Get-Content "$d\data\settings.ini" -Raw
  "{0}: overlay={1} (want {2}); settings.ini has night_force: {3}" -f $case.name, $got, $case.want, ($ini -match 'night_force=')
  if ($got -ne $case.want) { $fail += $case.name }
}
if ($fail.Count) { "NIGHT TEST FAILED: $($fail -join ', ')"; exit 1 } else { "NIGHT TEST OK" }
