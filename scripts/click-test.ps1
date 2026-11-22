# GUI click test (mock mode only, own PID only): open a page of a COPY of the exe, click at window-relative
# pixel coordinates (taken from a shot.ps1 screenshot), then print the mock log.
# Usage: pwsh scripts/click-test.ps1 -Page 2 -Clicks '467,406' [-Seed] [-Exe path] [-WaitMs 1500] [-Shot out.png]
param([string]$Page = '2', [string[]]$Clicks = @(), [int]$WaitMs = 1500, [string]$Shot = '', [string]$Ini = '', [string]$Win = '',
      [string]$Exe = 'F:\bgbuild\a15\QAShots\x86_64-pc-windows-gnu\debug\breakguard.exe', [int]$Monitor = 0)
# Drop LIB entries that do not exist: a stale LIB makes Add-Type fail with "Warning as Error".
$env:LIB = (@($env:LIB -split ';') | Where-Object { $_ -and (Test-Path $_) }) -join ';'
$Clicks = @($Clicks | ForEach-Object { $_ -split ';' } | Where-Object { $_ })
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Text; using System.Runtime.InteropServices;
public class CT { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [StructLayout(LayoutKind.Sequential)] public struct R{public int L,T,Rt,B;}
 public delegate bool EP(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EP cb, IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
 public static void Click(int x,int y){ SetCursorPos(x,y); System.Threading.Thread.Sleep(150); mouse_event(2,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(80); mouse_event(4,0,0,0,UIntPtr.Zero); }
 public static IntPtr Find(uint pid){ IntPtr f=IntPtr.Zero; EnumWindows((h,x)=>{uint p; GetWindowThreadProcessId(h,out p); if(p==pid&&IsWindowVisible(h)){var s=new StringBuilder(256); GetWindowText(h,s,256); if(s.ToString().StartsWith("BreakGuard")) f=h;} return true;},IntPtr.Zero); return f; }
}
'@
[void][CT]::SetProcessDPIAware()
Add-Type -AssemblyName System.Drawing
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
# Second monitor only: the user's primary monitor must never show a test window or a cursor move.
$second = @([Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary })
if ($second.Count -eq 0) { throw 'No second monitor found; refusing to run on the primary.' }
$mon = $second[[Math]::Min($Monitor, $second.Count - 1)].Bounds
$dpiScale = ([Drawing.Graphics]::FromHwnd([IntPtr]::Zero)).DpiX / 96
$d = Join-Path $env:TEMP "bgmock-click-$PID"; New-Item -ItemType Directory -Force "$d\data","$d\sess" | Out-Null
$copy = Join-Path $d 'bg.exe'; Copy-Item $Exe $copy
$env:BREAKGUARD_DDC_MOCK = "$d\mock.log"; $env:BREAKGUARD_DATA_DIR = "$d\data"; $env:BREAKGUARD_SESSIONS_DIR = "$d\sess"
$env:BREAKGUARD_START_PAGE = $Page
$env:BREAKGUARD_ONLY_MONITOR = "$($mon.X),$($mon.Y),$($mon.Width),$($mon.Height)"
$env:BREAKGUARD_WINDOW_POS = "$([int][Math]::Round(($mon.X + 100) / $dpiScale)),$([int][Math]::Round(($mon.Y + 100) / $dpiScale))"   # logical points
if ($Ini) { [IO.File]::WriteAllText("$d\data\settings.ini", $Ini.Replace('\n', "`n"), [Text.Encoding]::ASCII) }
if ($Win) { [IO.File]::WriteAllText("$d\data\window.ini", $Win.Replace(',', ' '), [Text.Encoding]::ASCII) }   # -Win '1020,1250' = window size in points (same file shot.ps1 -Width/-Height writes)
$p = Start-Process $copy -PassThru
try {
  $h = [IntPtr]::Zero
  for ($i = 0; $i -lt 50 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 200; $h = [CT]::Find([uint32]$p.Id) }
  if ($h -eq [IntPtr]::Zero) { 'NO WINDOW'; exit 1 }
  # Never move or raise: BREAKGUARD_WINDOW_POS placed it on monitor 2 before first show. Verify only.
  Start-Sleep -Milliseconds 1200
  $r = New-Object CT+R; [void][CT]::GetWindowRect($h, [ref]$r)
  if ($r.L -lt $mon.X -or $r.T -lt $mon.Y -or $r.Rt -gt ($mon.X + $mon.Width) -or $r.B -gt ($mon.Y + $mon.Height)) { throw "window outside monitor 2: L=$($r.L) T=$($r.T)" }
  foreach ($c in $Clicks) { $xy = $c -split ','; [CT]::Click($r.L + [int]$xy[0], $r.T + [int]$xy[1]); Start-Sleep -Milliseconds $WaitMs }
  if ($Shot) {
    $r2 = New-Object CT+R; [void][CT]::GetWindowRect($h, [ref]$r2)
    $bmp = New-Object Drawing.Bitmap ($r2.Rt - $r2.L), ($r2.B - $r2.T)
    $bmp = [Cap]::Grab($h, (New-Object Drawing.Rectangle $r2.L, $r2.T, ($r2.Rt - $r2.L), ($r2.B - $r2.T)), (New-Object Drawing.Rectangle $r2.L, $r2.T, ($r2.Rt - $r2.L), ($r2.B - $r2.T)))
    $bmp.Save($Shot); $bmp.Dispose()
  }
  'MOCKLOG:'; if (Test-Path "$d\mock.log") { Get-Content "$d\mock.log" }
  'SETTINGS:'; if (Test-Path "$d\data\settings.ini") { Get-Content "$d\data\settings.ini" }
} finally {
  if (-not $p.HasExited) { $null = $p.CloseMainWindow(); Start-Sleep -Milliseconds 800; if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force } }
}
