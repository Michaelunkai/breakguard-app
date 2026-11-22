# Idle CPU check: launches a COPY of the exe in mock mode on MONITOR 2 ONLY (never the primary),
# measures CPU seconds over 10 s, logs the window rect, and stops only its own PID.
param([Parameter(Mandatory)][string]$Exe, [double]$Max = 1.0, [string]$Page = '', [string]$OutDir = '')   # -Page 8 = idle on the Log page (live hub polling)
Remove-Item Env:LIB -ErrorAction SilentlyContinue   # a stale LIB path makes Add-Type fail
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class Win {
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
}
'@
[void][System.Reflection.Assembly]::LoadWithPartialName('System.Drawing')
[void][Win]::SetProcessDPIAware()   # physical coords: unaware powershell.exe reports monitor 2 at X=2560 instead of 3840
$second = @([Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary })
if ($second.Count -eq 0) { throw 'No second monitor found; refusing to run on the primary.' }
$mon = $second[0].Bounds
$dpiScale = ([Drawing.Graphics]::FromHwnd([IntPtr]::Zero)).DpiX / 96
$env:BREAKGUARD_ONLY_MONITOR = "$($mon.X),$($mon.Y),$($mon.Width),$($mon.Height)"
$env:BREAKGUARD_WINDOW_POS = "$([int][Math]::Round(($mon.X + 100) / $dpiScale)),$([int][Math]::Round(($mon.Y + 100) / $dpiScale))"
$d = Join-Path $env:TEMP "bgmock-cpu-$PID"; New-Item -ItemType Directory -Force $d | Out-Null
$copy = Join-Path $d 'bg.exe'; Copy-Item $Exe $copy
$env:BREAKGUARD_DDC_MOCK = "$d\mock.log"; $env:BREAKGUARD_DATA_DIR = "$d\data"
$env:BREAKGUARD_SESSIONS_DIR = "$d\sess"; New-Item -ItemType Directory -Force "$d\sess" | Out-Null
if ($Page) { $env:BREAKGUARD_START_PAGE = $Page }
$p = Start-Process $copy -PassThru
try {
  Start-Sleep 3; $p.Refresh(); $c0 = $p.TotalProcessorTime.TotalSeconds
  Start-Sleep 10; $p.Refresh(); $c1 = $p.TotalProcessorTime.TotalSeconds
  $u = [math]::Round($c1 - $c0, 3)
  $rect = New-Object Win+RECT
  $h = $p.MainWindowHandle
  for ($i = 0; $i -lt 20 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep 1; $p.Refresh(); $h = $p.MainWindowHandle }
  # Under heavy CPU load the first MainWindowHandle can be a tiny placeholder at (0,0): wait (max 30 s) for the real window.
  for ($i = 0; $i -lt 30 -and $h -ne [IntPtr]::Zero; $i++) {
    if ([Win]::GetWindowRect($h, [ref]$rect) -and (($rect.R - $rect.L) -ge 200)) { break }
    Start-Sleep 1; $p.Refresh(); $h = $p.MainWindowHandle
  }
  $rectText = 'window rect: none'
  if ($h -ne [IntPtr]::Zero -and [Win]::GetWindowRect($h, [ref]$rect)) {
    $rectText = "window rect: X=$($rect.L) Y=$($rect.T) R=$($rect.R) B=$($rect.B)"
    $onPrimary = $rect.L -lt $mon.X
    if ($onPrimary) { $rectText += ' ON PRIMARY' }
  }
  $log = "monitor2 X=$($mon.X) Y=$($mon.Y) W=$($mon.Width) H=$($mon.Height)`r`n$rectText`r`nIDLE CPU over 10 s: $u s (max $Max)"
  if ($OutDir) { New-Item -ItemType Directory -Force $OutDir | Out-Null; Set-Content (Join-Path $OutDir 'idle-rects.log') -Value $log -Encoding ASCII }
  $log
  if ($u -le $Max -and $rectText -notmatch 'ON PRIMARY') { 'IDLE CPU OK' } else { 'IDLE CPU FAIL'; exit 1 }
} finally { if (-not $p.HasExited) { Stop-Process -Id $p.Id } }
