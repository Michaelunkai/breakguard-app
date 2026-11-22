# QA report

Run: `pwsh scripts/qa.ps1` (or `cargo test --release --offline`). All runs use BREAKGUARD_DDC_MOCK + BREAKGUARD_DATA_DIR (fresh temp dirs).

## Results (tree as of 2026-11-19)
- Unit tests (src): 43 passed.
- tests/cli.rs: 17 passed, 0 failed (~15 s), with all builders' final code. `pwsh scripts/qa.ps1`: PASS=60 FAIL=0 (59 before selftest_refuses_without_mock was added).
- Added: `--secs abc/-5/0` must exit in <1 s (passes); selftest_refuses_without_mock (rc 2, mock file untouched).

## Tests (tests/cli.rs)
selftest_ok, version_prints, mock_proves_no_real_hardware_path, blackout_honors_deadline, blackout_all_and_options, blackout_bad_args_never_panic (15 arg cases), blackout_garbage_secs_does_not_run_forever, monitor_name_injection_rejected_safely (injection/unicode/5000 chars, blackout + helper-on), helper_reenables_after_deadline, helper_past_deadline_acts_immediately, helper_bad_args_never_panic, unknown_subcommand_no_panic, settings_corruption (11 corrupt files), settings_path_is_directory, log_rotation_bounded, gui_smoke_and_second_instance (--demo-tour exits 0, 2nd instance no crash).

## Bugs found
None in product code so far. Two test-premise errors fixed in tests (selftest writes no log; selftest correctly exits 1 when settings path unwritable).

## Not yet covered
Real overlay window (only mock), final exe, GUI pages after Frontend polish, Overlay `cancel`/pidfile paths at process level.

QA VERDICT: PASS (mock-mode scope above; no open bugs; real-hardware and real-window paths untested by rule)
