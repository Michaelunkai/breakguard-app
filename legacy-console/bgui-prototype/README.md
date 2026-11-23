# Breakguard

A small Windows desktop app (Rust, egui) for break reminders, timed sleep, "night mode" and screen controls.
It never closes, suspends or changes the priority of your programs.

> Status: early. Built and launched on one Windows 11 PC. Power features have **not** been exercised yet. See "Tested vs untested".

## Features
- **Breaks**: work/break timer with a ring, optional bring-to-front reminder, and a before/after process-name comparison.
  The comparison only checks process names. It is not a health check.
- **Sleep (S3)**: arms a wake timer, then asks Windows to sleep, with an 8-second cancel countdown.
  Sleep pauses all work. Waking on time depends on firmware, drivers and Windows and is not guaranteed.
- **Night mode**: keeps the PC awake (prevents idle sleep) and can turn screens off. Programs, installs and AI jobs keep running.
  Prevent-sleep is released when you stop night mode or exit. This is not hibernation.
- **Screens off for N minutes**: asks Windows to switch all displays off and sends a turn-on request at the end. Mouse or key turns them on early.
- **Blackout overlay per monitor**: a black always-on-top window. It does **not** power the monitor off or save power. Click or Esc closes it.
- **DDC/CI monitor power** (off by default): only offered if the monitor register is readable, and needs an explicit acknowledgement. Readable does not mean the monitor will wake again.
- **Watch list**: configurable process names to compare.

## Sleep vs night mode
| | Sleep (S3) | Night mode |
|---|---|---|
| Programs running | paused | keep running |
| Power use | lowest | normal, screens can be off |
| Wake | timer or key, not guaranteed | already awake |
| Remote access while active | offline | stays online |

Windows reports which sleep states a PC supports (`powercfg /a`). The development PC supports only S3: no Modern Standby, no hibernate, no hybrid sleep.
Use night mode when background work must continue.

## Build
Needs the Rust toolchain. On Windows:

    cargo build --release

The `stub-webbrowser` folder replaces the `webbrowser` crate so the GNU toolchain builds without `dlltool`. Links in the UI will not open in a browser.

## Run
`breakguard.exe` (or `start-breakguard.bat`). There is no autostart.

## Tested vs untested
Tested: release build, process starts in the interactive session.
Untested: window rendering check on the PC, real sleep and wake, screens off, blackout overlay, DDC, reminder popup over a fullscreen game, multi-monitor DPI placement.
No CI: GitHub Actions is disabled on the owning account.
