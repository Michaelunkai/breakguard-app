//! slice reviewer (owned by one team agent)
//!
//! Review log (reviewer). Broker (multiagents-peer) unreachable at time of writing
//! (ECONNREFUSED), so approvals and feedback below could NOT be sent; they are recorded here.
//!
//! Test evidence: `CARGO_TARGET_DIR=F:/bgbuild/team/reviewer cargo test --offline --release
//! --target x86_64-pc-windows-gnu ui::parts` finished exit 0 (about 150 tests listed, all ok in
//! the captured output except one line, see lane_header_stats below).
//!
//! Approvals (zero `[BLOCKING]` after source read + test run):
//! - button_states (cl-0lntm6): approved.
//! - card_elev (cl-adk60z): approved.
//! - dark_polish (cl-sixli5): approved.
//! - light_polish (cl-axajzs): approved.
//! - settings_groups (cl-cro1pp): approved.
//! - typography (cl-anfgsw): approved.
//! - inputs_styled (cl-zxoq66): no `[BLOCKING]`; `[SUGGESTION]` add headless tests for toggle,
//!   slider_styled, number_input (owner acknowledged).
//! - nav_indicator (cl-6v885m): no `[BLOCKING]`; `[SUGGESTION]` indicator_y test does not test easing
//!   (first call snaps); paint_indicator degenerate test asserts nothing.
//!
//! Open `[BLOCKING]`:
//! - lane_header_stats: test `activity_hours_branch` reported FAILED in the captured run while the
//!   process exit was 0. Owner must re-run and fix or confirm.
//!
//! Not yet reviewed by reviewer: tokens_doc, perf_bench, qa_breaker (reported green by owners),
//! icon_set, lane_wrap_row, lane_toolbar, creature_state_fx, lane_header_stats, lane_picker_ui,
//! lane_keys_ui, lane_retention_ui, lane_actions_ui, lane_persist_io, lane_density,
//! lane_scale_perf, log_color_tests, and the creature/fade/tool slices.
//!
//! Status: DONE is NOT reached. Zero-open-blocking is not satisfied.
#![allow(dead_code)]
