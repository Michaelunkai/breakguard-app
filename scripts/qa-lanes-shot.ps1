# Captures the Log page lanes view with 2 mock sessions at a given window width.
# Usage: pwsh scripts/qa-lanes-shot.ps1 -Width 1600 -Out QA-Shots/team/before-1600.png
# Mock only: BREAKGUARD_DDC_MOCK + BREAKGUARD_DATA_DIR point at a fresh temp dir; sessions come from a fixture folder.
# Captures monitor 2 only (refuses to run if none). Window is rendered via PrintWindow, never what is on top of it.
param(
  [Parameter(Mandatory)][int]$Width,
  [Parameter(Mandatory)][string]$Out,
  [int]$Height = 760,
  [ValidateRange(1, 60)][int]$Sessions = 2,
  [switch]$Feed,
  [int]$Delay = 2000,
  [int[]]$ClickAt,   # optional window-relative physical px "x,y": left-click there before grabbing (expands a row)
  [string]$Exe = 'F:\bgbuild\release\x86_64-pc-windows-gnu\release\breakguard.exe'
)
$ErrorActionPreference = 'Stop'
if ($Width -lt 320 -or $Width -gt 4000) { throw "Width $Width out of range 320..4000" }
if (-not (Test-Path $Exe)) { throw "exe not found: $Exe" }
$env:LIB = (@($env:LIB -split ';') | Where-Object { $_ -and (Test-Path $_) }) -join ';'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -ReferencedAssemblies 'System.Drawing', 'System.Drawing.Primitives', 'System.Windows.Forms' -TypeDefinition @'
using System; using System.Text; using System.Drawing; using System.Drawing.Imaging; using System.Runtime.InteropServices;
public class QaWin {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  public delegate bool EP(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EP cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, IntPtr extra);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out R r, int size);
  public static IntPtr Find(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h) && Title(h).StartsWith("BreakGuard")) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  public static string Title(IntPtr h) { var sb = new StringBuilder(256); GetWindowText(h, sb, 256); return sb.ToString(); }
  public static R Ext(IntPtr h) { R r; if (DwmGetWindowAttribute(h, 9, out r, 16) != 0) GetWindowRect(h, out r); return r; }
  public static Bitmap Grab(IntPtr h, Rectangle inner, Rectangle outer) {
    var full = new Bitmap(outer.Width, outer.Height);
    using (var g = Graphics.FromImage(full)) {
      IntPtr hdc = g.GetHdc();
      bool ok;
      try { ok = PrintWindow(h, hdc, 2); } finally { g.ReleaseHdc(hdc); }
      if (!ok) throw new InvalidOperationException("PrintWindow failed; refusing to save a screen grab");
    }
    return full.Clone(new Rectangle(inner.X - outer.X, inner.Y - outer.Y, inner.Width, inner.Height), full.PixelFormat);
  }
}
'@
[void][QaWin]::SetProcessDPIAware()

$second = @([Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary })
if ($second.Count -eq 0) { throw 'No second monitor found; refusing to run on the primary.' }
$mon = $second[0].Bounds
$env:BREAKGUARD_ONLY_MONITOR = "$($mon.X),$($mon.Y),$($mon.Width),$($mon.Height)"
$dpiScale = ([Drawing.Graphics]::FromHwnd([IntPtr]::Zero)).DpiX / 96
$env:BREAKGUARD_WINDOW_POS = "$([int][Math]::Round(($mon.X + 100) / $dpiScale)),$([int][Math]::Round(($mon.Y + 100) / $dpiScale))"

$outFull = [IO.Path]::GetFullPath($Out)
New-Item -ItemType Directory -Force (Split-Path $outFull) | Out-Null

$d = Join-Path $env:TEMP "bgqa-lanes-$PID"
New-Item -ItemType Directory -Force "$d\data", "$d\sess\sessions" | Out-Null
$copy = Join-Path $d 'bg.exe'; Copy-Item $Exe $copy
$env:BREAKGUARD_DDC_MOCK = "$d\mock.log"
$env:BREAKGUARD_DATA_DIR = "$d\data"
$env:BREAKGUARD_SESSIONS_DIR = "$d\sess"
$env:BREAKGUARD_START_PAGE = '8'        # NAV index 8 = Log
# BREAKGUARD_LOG_TAB is NOT set: any value opens tab 2 (Sessions); unset opens tab 0 = Live (lanes).
# window.ini holds logical points; -Width is physical px, so divide by the DPI scale.
$logicalW = [int][Math]::Round($Width / $dpiScale)
[IO.File]::WriteAllText("$d\data\window.ini", "$logicalW $Height", [Text.Encoding]::ASCII)

# Two dummy "agents": real processes (killed at end) with fixture session files.
$dummies = @()
foreach ($cli in (1..$Sessions | ForEach-Object { "cli-a$_" })) {
  $dp = Start-Process powershell -ArgumentList '-NoProfile', '-Command', 'Start-Sleep 300' -PassThru -WindowStyle Hidden
  $dummies += $dp
  $j = [ordered]@{ pid = $dp.Id; sessionId = $cli; cwd = 'F:\fixture\project'; startedAt = [DateTimeOffset]::Now.ToUnixTimeMilliseconds(); name = $cli }
  ($j | ConvertTo-Json -Compress) | Set-Content "$d\sess\sessions\$($dp.Id).json" -Encoding UTF8
  if ($Feed) {   # fixture transcript: tool call, result, plain text, and one long line (clipping check)
    New-Item -ItemType Directory -Force "$d\sess\projects\F--fixture" | Out-Null
    $long = 'cargo test --release --offline -- ' + ('very_long_test_name_segment_' * 12)
    $lines = @(
      '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}}],"stop_reason":"tool_use"}}',
      '{"type":"user","message":{"content":[{"type":"tool_result","content":"test result: ok. 906 passed; 0 failed","is_error":false}]}}',
      '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"F:\\fixture\\project\\src\\main.rs"}}],"stop_reason":"tool_use"}}',
      '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"' + $long + '"}}],"stop_reason":"tool_use"}}',
      '{"type":"user","message":{"content":[{"type":"tool_result","content":"error: linker failed","is_error":true}]}}'
    )
    Set-Content "$d\sess\projects\F--fixture\$cli.jsonl" -Value $lines -Encoding UTF8
  }
}

$p = $null
try {
  $p = Start-Process $copy -PassThru
  $h = [IntPtr]::Zero
  for ($i = 0; $i -lt 60 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 250; $h = [QaWin]::Find([uint32]$p.Id) }
  if ($h -eq [IntPtr]::Zero) { throw 'no BreakGuard window appeared' }
  Start-Sleep -Milliseconds $Delay
  if (-not ([QaWin]::Title($h)).StartsWith('BreakGuard')) { throw "captured handle is not BreakGuard: title='$([QaWin]::Title($h))'" }
  $r = [QaWin]::Ext($h)
  if ($ClickAt -and $ClickAt.Count -eq 2) {
    [void][QaWin]::SetForegroundWindow($h); Start-Sleep -Milliseconds 400
    [QaWin]::SetCursorPos($r.L + $ClickAt[0], $r.T + $ClickAt[1]); Start-Sleep -Milliseconds 600   # hover first: egui needs a frame to register the pointer
    [QaWin]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 120; [QaWin]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 800
  }
  $wr = New-Object QaWin+R; [void][QaWin]::GetWindowRect($h, [ref]$wr)
  $w = $r.Rt - $r.L; $hh = $r.B - $r.T
  if ($w -ne $Width) { Write-Host "NOTE: requested width $Width, window is ${w}px (DPI/minimum clamp)" }
  $inner = New-Object System.Drawing.Rectangle $r.L, $r.T, $w, $hh
  $outer = New-Object System.Drawing.Rectangle $wr.L, $wr.T, ($wr.Rt - $wr.L), ($wr.B - $wr.T)
  $bmp = [QaWin]::Grab($h, $inner, $outer)
  $bmp.Save($outFull, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  "saved $outFull (${w}x${hh})"
} finally {
  if ($p -and -not $p.HasExited) { Stop-Process -Id $p.Id -ErrorAction SilentlyContinue }
  foreach ($dp in $dummies) { if (-not $dp.HasExited) { Stop-Process -Id $dp.Id -ErrorAction SilentlyContinue } }
}
