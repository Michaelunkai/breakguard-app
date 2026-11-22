# UX review: live lane output (UXDesigner)

Target: default Claude app style. Calm rows, short plain sentences, grouped summaries, no raw tool names, no raw command echo, generous line height.

Status: NOT APPROVED. Only screenshot so far: `QA-Shots/team/before-1600.png`. It shows the Sessions tab (`Activity log` > `Sessions`), not the Live tab, so it does not show lane rows. Live-lane visual review pending the Live-tab shot from QAEng. Verdict below is from code read (`src/logs.rs` plain_* builders, `src/ui/pages/lanes.rs` draw_row L201-258, kind_tag L148).

## BLOCKING

1. [BLOCKING] `src/ui/pages/lanes.rs` draw_row (~L226-229): tool rows draw the raw tool name as a chip (`tool_chips::chip(ui, &l.tool, ...)`). Raw names like `Bash`, `mcp__multiagents-peer__send_message` show on every row. Fix: show the plain verb as the row lead, drop the chip for plain rows (or show a small icon only). Owner: LayoutEng.
2. [BLOCKING] `src/ui/pages/lanes.rs` draw_row (~L240-244) and `row_full` (L196-199): raw text (`row_text`, i.e. the command or path) is appended after `  ·  ` in dim mono on every tool row. That is a raw command echo. Fix: default row = plain sentence only. Keep raw text behind click/hover or in the wrap/expand view. Owner: LayoutEng (layout), TextEng (row_full contract).
3. [BLOCKING] `src/ui/pages/lanes.rs` kind_tag L148-153: ToolCall tag = `l.tool.chars().take(8)`, a cut raw tool name (e.g. `mcp__mu`). Fix: remove for tool rows or use a fixed glyph/word ("tool"). Owner: LayoutEng.
4. [BLOCKING] `src/logs.rs` plain_tool fallback (`n => format!("Uses the {n} tool")`) and mcp branch (`Uses the <name spaced> tool`) expose raw tool ids. Fix: unknown/mcp tools → "Used a helper tool" (or "Used a tool"). Keep known names mapped. Owner: TextEng. Test to update: plain_english_for_every_tool_and_result.
5. [BLOCKING] No grouped summaries anywhere. Each tool call is its own row ("Reads the file x", "Reads the file y", ...). Target: "Ran 5 commands, read 3 files" / "Edited 2 files +72 -1". Fix: a summary row for runs of consecutive tool calls (count per verb, plain words, no raw names). Owner: LayoutEng (grouping in lanes.rs show), TextEng (summary sentence builder in logs.rs, pure fn + test).

## SUGGESTION

6. [SUGGESTION] `plain_result` `Done, got 1 line back` / `Done, got N lines back`: fine, but a calm row would read "Done." for N>0 and move the count to the summary. Owner: TextEng.
7. [SUGGESTION] `Runs a command: <desc>` has a colon. Prefer `Runs: <desc>`, or drop the colon form and show `<desc>` alone as the lead. Owner: TextEng.
8. [SUGGESTION] Line height: confirm live rows use >= 1.5 line height in `row_height` (wrap mode). Verify on the Live-tab shot. Owner: LayoutEng.
9. [SUGGESTION] Dim raw/meta text (`theme::pal().text_dim`) must meet 4.5:1 on `surface_lo` in light theme (accent_text does this; text_dim is not checked). Owner: LayoutEng.
10. [SUGGESTION] Sessions tab (before-1600.png): project cards show the label `PROJECT` with no project name, and "No activity recorded yet." while the status bar says `2 Claude running`. Not the live lane, but confusing. Owner: orchestrator to route (not a lane-owner item).

## Screenshot review (after-1600.png, Live tab)

- Side-by-side lanes: OK. Two lanes, gap between, no overlap, headers readable.
- BLOCKING 11. [BLOCKING] Shot shows NO feed rows: both lanes are empty below the tool chips. Wording and line height cannot be judged. QAEng must capture a shot with rows (mock feed running) for UX DONE.
- SUGGESTION 12. [SUGGESTION] Toolbar wraps at 1600px: `A+` drops to a second line (toolbar row, Live tab). Owner: LayoutEng. Fit or shorten labels (`A-`, `A+`, `comfortable`, `time: ago`).
- Still BLOCKING from items 1, 3, 4, 5 above (chip, kind_tag name, unknown-tool wording, grouped summary). Not visible in these shots; verified by code read, not by screen.

## Next
- Need Live-tab screenshot (QAEng) to review rows, line height, and grouped summaries visually.
- Re-review after LayoutEng and TextEng fixes. DONE = zero BLOCKING.
