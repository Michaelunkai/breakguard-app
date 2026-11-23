# Screenshot every page (or -Pages 1,4,gallery) of a COPY of the exe in mock mode, DPI-aware window capture.
# Output: <OutDir>\page<N>.png. Stops only the child PID it started.
# -Seed   settings with toggles on, 7 days of stats and a fixture Claude sessions folder (never live data)
# -Fresh  empty data dir (first-run onboarding card)
# -Scale  ui_scale percent written into the seeded settings (e.g. 125)
param([string[]]$Pages = @('1','2','3','4','5','6','7','8','9'), [switch]$Seed, [switch]$Fresh, [switch]$Break, [switch]$LogTab, [switch]$Picker, [switch]$PauseDemo, [switch]$Live, [switch]$Cpu, [int]$CpuSecs = 10, [int]$Monitor = 0, [switch]$Gallery, [switch]$Blackout, [int]$Many = 0, [int]$Width = 0, [int]$Height = 760, [string]$Hover = '', [int]$Scale = 100, [string]$Accent = '0', [string]$Press = '', [switch]$Tab, [ValidateSet('', 'dark', 'light')] [string]$Theme = '',
      [string]$OutDir = (Join-Path $env:TEMP 'bgshots'), [string]$Delay = '1200',
      [string]$Exe = 'F:\bgbuild\shared\x86_64-pc-windows-gnu\release\breakguard.exe')
# Drop LIB entries that do not exist: a stale LIB makes Add-Type fail with "Warning as Error" before any test runs.
$env:LIB = (@($env:LIB -split ';') | Where-Object { $_ -and (Test-Path $_) }) -join ';'
$Pages = @($Pages | ForEach-Object { $_ -split ',' })   # -File passes "a,b" as one string
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Text; using System.Runtime.InteropServices;
public class WS { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [StructLayout(LayoutKind.Sequential)] public struct R{public int L,T,Rt,B;}
 public delegate bool EP(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EP cb, IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
 [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out R r, int size);
 public static R Ext(IntPtr h){ R r; if (DwmGetWindowAttribute(h, 9, out r, 16) != 0) GetWindowRect(h, out r); return r; }
 public static string GetWindowTitle(IntPtr h){ var s=new StringBuilder(256); GetWindowText(h,s,256); return s.ToString(); }
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr e);
 public static IntPtr Find(uint pid){ IntPtr f=IntPtr.Zero; EnumWindows((h,x)=>{uint p; GetWindowThreadProcessId(h,out p); if(p==pid&&IsWindowVisible(h)){var s=new StringBuilder(256); GetWindowText(h,s,256); if(s.ToString().StartsWith("BreakGuard")) f=h;} return true;},IntPtr.Zero); return f; }
}
'@
[void][WS]::SetProcessDPIAware()
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System; using System.Drawing; using System.Runtime.InteropServices;
public static class Cap {
 [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
 // Renders only this window (never what is on top of it): PW_RENDERFULLCONTENT = 2.
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
Add-Type -AssemblyName System.Windows.Forms
# Monitor 2 only: the user's primary monitor must never show a test window or cursor move.
$second = @([Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary })
if ($second.Count -eq 0) { throw 'No second monitor found; refusing to run on the primary.' }
$mon = $second[[Math]::Min($Monitor, $second.Count - 1)].Bounds
$env:BREAKGUARD_ONLY_MONITOR = "$($mon.X),$($mon.Y),$($mon.Width),$($mon.Height)"
$dpiScale = ([Drawing.Graphics]::FromHwnd([IntPtr]::Zero)).DpiX / 96   # the app takes logical points, Windows reports physical pixels
$env:BREAKGUARD_WINDOW_POS = "$([int][Math]::Round(($mon.X + 100) / $dpiScale)),$([int][Math]::Round(($mon.Y + 100) / $dpiScale))"   # app opens on monitor 2 directly
New-Item -ItemType Directory -Force $OutDir | Out-Null
$rectLog = Join-Path $OutDir 'rects.log'; Set-Content $rectLog -Value "monitor2 X=$($mon.X) Y=$($mon.Y) W=$($mon.Width) H=$($mon.Height)" -Encoding ASCII
$d = Join-Path $env:TEMP "bgmock-shot-$PID"; New-Item -ItemType Directory -Force $d | Out-Null
$copy = Join-Path $d 'bg.exe'; Copy-Item $Exe $copy
$env:BREAKGUARD_DDC_MOCK = "$d\mock.log"; $env:BREAKGUARD_DATA_DIR = "$d\data"
if ($Width -gt 0) { New-Item -ItemType Directory -Force "$d\data" | Out-Null; [IO.File]::WriteAllText("$d\data\window.ini", "$Width $Height", [Text.Encoding]::ASCII) }
# Sessions always come from a fixture folder, never from the live Claude folders.
$sess = "$d\sess"; $env:BREAKGUARD_SESSIONS_DIR = $sess
if ($Live) {   # truth check only: junctions give the app a read-only view of the real session folders (mock mode otherwise never reads them)
  New-Item -ItemType Directory -Force $sess | Out-Null
  $h = $env:USERPROFILE
  New-Item -ItemType Junction -Path "$sess\claude-code-sessions" -Target "$env:APPDATA\Claude\claude-code-sessions" | Out-Null
  New-Item -ItemType Junction -Path "$sess\projects" -Target "$h\.claude\projects" | Out-Null
  New-Item -ItemType Junction -Path "$sess\sessions" -Target "$h\.claude\sessions" -ErrorAction SilentlyContinue | Out-Null
}
New-Item -ItemType Directory -Force "$d\data" | Out-Null
# -Theme dark|light writes settings key theme=<value> (read by the app at startup); without it the app default applies.
$themeLine = if ($Theme) { "light_theme=$(if ($Theme -eq 'light') { 1 } else { 0 })`n" } else { '' }
if (-not ($Seed -and -not $Fresh) -and $Theme) { [IO.File]::WriteAllText("$d\data\settings.ini", $themeLine, [Text.Encoding]::ASCII) }
if ($Seed -and -not $Fresh) {
  $ini = "break_enabled=1`nnight_enabled=1`nnight_level=40`nui_scale=$Scale`naccent=$Accent`nmicro_water=45`ndaily_goal=8`ncustom_tips=`nsched=1,9,30,25,1,Screen 1`nsched=2,14,0,15,0,Screen 1`n" + $themeLine
  [IO.File]::WriteAllText("$d\data\settings.ini", $ini, [Text.Encoding]::ASCII)
  $unix = [DateTimeOffset]::Now.ToUnixTimeSeconds() + [TimeZoneInfo]::Local.GetUtcOffset([DateTime]::Now).TotalSeconds
  $today = [int][Math]::Floor($unix / 86400)
  $rows = @(); $vals = @(5,7,8,3,8,6,4)
  for ($i = 0; $i -lt 7; $i++) { $rows += ("d={0},{1},{2},{3},{4}" -f ($today - 6 + $i), $vals[$i], ($i % 3), (($i + 1) % 2), ($vals[$i] * 20)) }
  [IO.File]::WriteAllText("$d\data\stats.ini", ($rows -join "`n") + "`n", [Text.Encoding]::ASCII)
  $sub = "$sess\claude-code-sessions\acct\org"; $proj = "$sess\projects\F--fixture"
  New-Item -ItemType Directory -Force $sub, $proj | Out-Null
  $now = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
  $he = -join ([char]0x5E9, [char]0x5DC, [char]0x5D5, [char]0x5DD)
  $defs = @(
    @{ id = 'local_a1'; cli = 'cli-a1'; title = "Relay run $he"; ago = 3; kind = 'run' },
    @{ id = 'local_a2'; cli = 'cli-a2'; title = 'Docs rewrite'; ago = 40; kind = 'wait' },
    @{ id = 'local_a3'; cli = 'cli-a3'; title = 'Old experiment'; ago = 90000; kind = 'wait' },
    @{ id = 'local_a4'; cli = ''; title = 'Cloud review'; ago = 600; kind = 'cloud' })
  for ($m = 1; $m -le $Many; $m++) { $defs += @{ id = "local_m$m"; cli = "cli-m$m"; title = "Session $m"; ago = 5 + $m; kind = 'run' } }
  foreach ($s in $defs) {
    $meta = [ordered]@{ sessionId = $s.id; cliSessionId = $s.cli; cwd = 'F:\fixture\project'; title = $s.title; createdAt = $now - 3600000; lastActivityAt = $now - $s.ago * 1000; isArchived = $false }
    if ($s.kind -eq 'cloud') { $meta.cloudSessionId = 'cse_fixture' }
    ($meta | ConvertTo-Json -Compress) | Set-Content "$sub\$($s.id).json" -Encoding UTF8
    if ($s.cli) {
      $plan = '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"TodoWrite","input":{"todos":[{"status":"completed"},{"status":"completed"},{"status":"pending"},{"status":"pending"}]}}],"stop_reason":"tool_use"}}'
      $last = if ($s.kind -eq 'run') { '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"description":"Run cargo test"}}],"stop_reason":"tool_use"}}' } else { '{"type":"assistant","message":{"content":[{"type":"text","text":"Finished the draft. What next?"}],"stop_reason":"end_turn"}}' }
      $iso = { param($ago) [DateTime]::UtcNow.AddSeconds(-$ago).ToString('yyyy-MM-ddTHH:mm:ss.fffZ') }
      $rich = @(
        ('{"type":"user","message":{"role":"user","content":"Continue the relay; do not restart."},"timestamp":"' + (& $iso 300) + '"}'),
        ('{"type":"assistant","message":{"content":[{"type":"text","text":"Reading the box list and picking the next open box."}]},"timestamp":"' + (& $iso 280) + '"}'),
        ('{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"F:/backup/ai/ralp/run/notes.md"}}]},"timestamp":"' + (& $iso 270) + '"}'),
        ('{"type":"user","message":{"content":[{"type":"tool_result","content":"- [x] 1. PLAN\n- [ ] 6. Custom break presets","is_error":false}]},"timestamp":"' + (& $iso 268) + '"}'),
        ('{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test --release --offline --bin breakguard presets","description":"Run the preset tests"}}]},"timestamp":"' + (& $iso 200) + '"}'),
        ('{"type":"user","message":{"content":[{"type":"tool_result","content":"error[E0308]: mismatched types at src/engine.rs:412:9, expected u32 found usize","is_error":true}]},"timestamp":"' + (& $iso 190) + '"}'),
        ('{"type":"assistant","message":{"content":[{"type":"text","text":"The compile failed on line 412 with 3 mismatched types. Fixing the cast and running the tests again. ' + ('This line is deliberately very long so the wrap and truncation rules can be seen. ' * 4) + '"}]},"timestamp":"' + (& $iso 180) + '"}'),
        ('{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"F:/study/Windows/Applications/Desktop/Utilities/TimeManager/BreakReminders/breakguard/src/engine.rs"}}]},"timestamp":"' + (& $iso 170) + '"}'),
        ('{"type":"user","message":{"content":[{"type":"tool_result","content":"The file src/engine.rs has been updated successfully.","is_error":false}]},"timestamp":"' + (& $iso 160) + '"}')
      )
      Set-Content "$proj\$($s.cli).jsonl" -Value (@($rich) + @($plan, $last)) -Encoding UTF8
      (Get-Item "$proj\$($s.cli).jsonl").LastWriteTime = (Get-Date).AddSeconds(-$s.ago)
    }
  }
}
$dummies = @()
if ($Seed -and -not $Fresh) {   # live dummy "agents" so the Agents card has real processes to list (pids are ours, killed at the end)
  New-Item -ItemType Directory -Force "$sess\sessions" | Out-Null
  $clis = @('cli-a1', 'cli-a2', 'cli-a3'); for ($m = 1; $m -le $Many; $m++) { $clis += "cli-m$m" }
  foreach ($cli in $clis) {
    $dp = Start-Process powershell -ArgumentList '-NoProfile', '-Command', 'Start-Sleep 300' -PassThru -WindowStyle Hidden
    $dummies += $dp
    $j = [ordered]@{ pid = $dp.Id; sessionId = $cli; cwd = 'F:\fixture\project'; startedAt = [DateTimeOffset]::Now.ToUnixTimeMilliseconds(); name = $cli }
    ($j | ConvertTo-Json -Compress) | Set-Content "$sess\sessions\$($dp.Id).json" -Encoding UTF8
  }
}
if ($PauseDemo) { $env:BREAKGUARD_PAUSE_DEMO = "1" }
if ($Break) { $env:BREAKGUARD_START_BREAK = "1" }
if ($LogTab) { $env:BREAKGUARD_LOG_TAB = "1" }
if ($Picker) { $env:BREAKGUARD_PICKER_DEMO = "1" }
if ($Blackout) {   # 10 s mock blackout on monitor 2 only (BlackChrome's OK); one screenshot of monitor 2 mid-run
  $bo = Start-Process $copy -ArgumentList 'blackout', '--monitor', 'all', '--secs', '10' -PassThru -WindowStyle Hidden
  Start-Sleep -Seconds 4
  $bmp = New-Object System.Drawing.Bitmap $mon.Width, $mon.Height
  $bg = [System.Drawing.Graphics]::FromImage($bmp); $bg.CopyFromScreen($mon.X, $mon.Y, 0, 0, $bmp.Size)
  $bmp.Save("$OutDir\page-blackout.png", [System.Drawing.Imaging.ImageFormat]::Png); "blackout captured monitor 2"
  Add-Content $rectLog -Value "blackout monitor2 X=$($mon.X) Y=$($mon.Y) W=$($mon.Width) H=$($mon.Height)" -Encoding ASCII
  if (-not $bo.WaitForExit(15000)) { Stop-Process -Id $bo.Id -ErrorAction SilentlyContinue }
  $Pages = @()
}
if ($Gallery) { $Pages = @('gallery') }   # Widgets: BREAKGUARD_START_PAGE=gallery opens the widget gallery
foreach ($n in $Pages) {
  $env:BREAKGUARD_START_PAGE = "$n"
  $p = Start-Process $copy -PassThru
  try {
    $h = [IntPtr]::Zero
    for ($i = 0; $i -lt 40 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 250; $h = [WS]::Find([uint32]$p.Id) }
    Start-Sleep -Milliseconds ([int]$Delay)
    if ($h -eq [IntPtr]::Zero) { "page $n no window"; continue }
    if ($Cpu) { $p.Refresh(); $c0 = $p.TotalProcessorTime.TotalSeconds; Start-Sleep $CpuSecs; $p.Refresh(); "CPU page ${n}: $([math]::Round($p.TotalProcessorTime.TotalSeconds - $c0, 3)) s over $CpuSecs s"; continue }
    # Never move or raise the window: BREAKGUARD_WINDOW_POS places it on monitor 2 before first show; the rect check below only verifies it.
    for ($k = 0; $k -lt 12; $k++) { $r0 = New-Object WS+R; [void][WS]::GetWindowRect($h, [ref]$r0); if (($r0.Rt - $r0.L) -ge 600) { break }; Start-Sleep -Milliseconds 500 }
    $rw = New-Object WS+R; [void][WS]::GetWindowRect($h, [ref]$rw)
    Add-Content $rectLog -Value "page $n L=$($rw.L) T=$($rw.T) R=$($rw.Rt) B=$($rw.B)" -Encoding ASCII
    if ($rw.L -lt $mon.X -or $rw.T -lt $mon.Y -or $rw.Rt -gt ($mon.X + $mon.Width) -or $rw.B -gt ($mon.Y + $mon.Height)) { throw "page $n window outside monitor 2 (see $rectLog)" }
    if ($Hover) { $xy = $Hover -split ','; $wr = New-Object WS+R; [void][WS]::GetWindowRect($h, [ref]$wr); [void][WS]::SetCursorPos($wr.L + [int]$xy[0], $wr.T + [int]$xy[1]); Start-Sleep -Milliseconds 300; [void][WS]::SetCursorPos($wr.L + [int]$xy[0] + 1, $wr.T + [int]$xy[1] + 1); Start-Sleep -Milliseconds 1500 }   # topmost so other windows never cover the capture
    if ($Press) { $xy = $Press -split ','; $wr = New-Object WS+R; [void][WS]::GetWindowRect($h, [ref]$wr); [void][WS]::SetCursorPos($wr.L + [int]$xy[0], $wr.T + [int]$xy[1]); Start-Sleep -Milliseconds 300; [WS]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 400 }   # left button held down on the point; released after capture
    if ($Tab) { [WS]::keybd_event(0x09, 0, 0, [IntPtr]::Zero); [WS]::keybd_event(0x09, 0, 2, [IntPtr]::Zero); Start-Sleep -Milliseconds 600 }   # Tab moves keyboard focus: focus ring
    $r = [WS]::Ext($h)   # visible frame, without the invisible DWM border
    $wr = New-Object WS+R; [void][WS]::GetWindowRect($h, [ref]$wr)
    if (-not ([WS]::GetWindowTitle($h)).StartsWith('BreakGuard')) { throw "page ${n}: captured handle is not BreakGuard" }
    $w = $r.Rt - $r.L; $hh = $r.B - $r.T
    $inner = New-Object System.Drawing.Rectangle $r.L, $r.T, $w, $hh
    $outer = New-Object System.Drawing.Rectangle $wr.L, $wr.T, ($wr.Rt - $wr.L), ($wr.B - $wr.T)
    $bmp = [Cap]::Grab($h, $inner, $outer)   # PrintWindow: only the BreakGuard window is drawn, never a window on top
    $bmp.Save("$OutDir\page$n$(if ($Break) {"-break"})$(if ($Hover) {"-hover"})$(if ($Press) {"-press"})$(if ($Tab) {"-focus"})$(if ($LogTab) {"-tab"}).png", [System.Drawing.Imaging.ImageFormat]::Png); "page $n ${w}x${hh}"
    if ($Press) { [WS]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero) }   # release the held button
  } finally { if (-not $p.HasExited) { Stop-Process -Id $p.Id }; $p.WaitForExit(3000) | Out-Null; Start-Sleep -Milliseconds 1500 }
}
foreach ($dp in $dummies) { if (-not $dp.HasExited) { Stop-Process -Id $dp.Id -ErrorAction SilentlyContinue } }
