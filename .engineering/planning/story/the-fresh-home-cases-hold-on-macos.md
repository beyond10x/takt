---
format: aep.planning-md/3
id: story:the-fresh-home-cases-hold-on-macos
kind: story
status: draft
title: The fresh-home cases hold on macOS
summary: The five fresh-home step-guard cases gated to Linux after the 0.15.0 release run run on macOS, or name why they cannot
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
revision: 2
---
## Outcome

The fresh-home cases of story:fresh-home-first-test-run-is-not-refused run on macOS and hold
there, or say by name why the case cannot arise under Seatbelt.

## Evidence

The 0.15.0 release run's macOS jobs (Release workflow, tag 0.15.0) failed five cases that pass
on Linux:

- `fresh_home_outside_adv1`: `adv_a_crate_source_planted_in_a_fresh_cargo_home_is_accepted_today`
  (the run stopped NoProgress, the planted source not left), `adv_a_program_planted_in_a_fresh_cargo_bin_is_removed_with_cargo_home_today`
  (not refused: NoProgress), `adv_a_rustup_default_toolchain_linked_into_the_tree_is_accepted_today`
  (the script did not plant).
- `step_guard_sees_test_script_imports`: `a_fresh_homes_first_tests_run_is_not_refused_for_the_cargo_cache`
  (not refused and a test result recorded, but no `~/.cargo/registry/cache` under the run's
  `HOME`), `a_program_written_into_a_fresh_homes_cargo_bin_still_stops_the_run` (records written:
  the program did not stop the run).

The five are `#[cfg(target_os = "linux")]` until this story decides them.

## Acceptance

1. Each of the five cases runs on macOS, asserting what takt does under Seatbelt, or is replaced
   by a case that names why its planting cannot happen there.
2. The darwin job passes with them.
