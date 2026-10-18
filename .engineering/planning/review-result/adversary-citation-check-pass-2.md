---
format: aep.planning-md/3
id: review-result:adversary-citation-check-pass-2
kind: review-result
status: active
title: Adversary, citation-check-in-task-check, pass 2
relations:
- reviews: story:citation-check-in-task-check
revision: 1
---
unit: story:citation-check-in-task-check, worktree w9-citation-check at d6d558e (base 8ce2019) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 387→403, red 10
origin: introduced 10 / pre-existing 0 / undecided 0
wrote-outside-worktree: 5 paths, all in the unit's scratch `adv2/`
needs-coordinator: none

**Cases** (`crates/takt-cli/tests/citation_check_adv2.rs`; alone: `6 passed; 10 failed`)

| case | expected | actual |
|---|---|---|
| `a_line_citation_after_read_from_the_is_not_silently_skipped` | exit 1 | exit 0 |
| `a_range_citation_after_read_from_the_file_is_not_silently_skipped` | exit 1 | exit 0 |
| `a_read_from_wrapped_across_two_comment_lines_is_not_silently_skipped` | exit 1 | exit 0 |
| `a_word_ending_in_read_before_from_is_not_a_marker` | exit 0 | exit 1, `thread from` reported as stray; summary counts 6 in-repository for 4 |
| `a_shell_comment_in_a_nested_fence_does_not_end_the_section` | exit 0 | exit 1 |
| `a_tilde_fence_holding_a_backtick_line_does_not_hide_the_next_heading` | exit 1 | exit 0 |
| `a_setext_heading_ends_the_section_above_it` | exit 1 | exit 0 |
| `a_heading_text_two_sections_share_does_not_resolve_by_the_other_one` | exit 1 | exit 0 |
| `a_section_whose_heading_is_the_last_line_holds_only_that_line` | exit 1 | green |
| `a_range_whose_last_needle_also_occurs_earlier_still_resolves` | exit 0 | exit 1 (earliest match taken) |
| `fix_rewrites_the_citation_not_the_same_text_earlier_on_its_line` | the citation rewritten | the prose rewritten, the citation stale |
| `fix_twice_changes_nothing_the_second_time` | no change | green |
| `fix_refuses_a_span_longer_than_the_cited_range` | refused | green |
| three non-UTF-8 / missing-table cases | exit 1, file named, no traceback | green |

**Gate.** `task check` exit 201: ess 0, check-citations 0 (319 of 319), fmt 0, clippy 0, cargo test 101 at `citation_check_adv2`. Without fail-fast: 403 executed, 393 passed, 10 failed.

**Attacked, not broken.** The range-end rule cannot be met with a drifted first needle (`found[0] == a-1` strict); CRLF kept; pass 1's 11 cases against 564d6d8's script give the same 9 red and 2 green, each red at its own assertion. Reachability today: the 5 prose comments in `takt.yaml` hold no path; the 2 section-cited documents have unique headings and no fences or setext headings.

```findings
[
  {"file": "scripts/check-citations", "line": 122, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the prose exception skips any comment line whose only marker is read from the, so a line or range citation written after it passes silently"},
  {"file": "scripts/check-citations", "line": 52, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the marker has no word boundary and the scan is per line, so thread from fails as a stray while a read / # from: path wrap passes silently"},
  {"file": "scripts/check-citations", "line": 205, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "any fence line toggles the fence state, so a nested fence ends a section early and a tilde fence holding a backtick line hides the next heading"},
  {"file": "scripts/check-citations", "line": 61, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "setext headings are not headings to the checker, so a section citation resolves by text under the setext heading that follows it"},
  {"file": "scripts/check-citations", "line": 303, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a heading text two sections share resolves if either section holds the needles"},
  {"file": "scripts/check-citations", "line": 181, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "in_order takes the earliest occurrence of the last needle, so the range-end rule refuses a correct range whose last needle also occurs earlier in it"},
  {"file": "scripts/check-citations", "line": 348, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "fix replaces the first occurrence of the cited text on the line, rewriting prose that contains it and leaving the citation stale"},
  {"file": "scripts/check-citations", "line": 309, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "stray and unknown lines are counted as in-repository citations, so the summary reported 6 in-repository for a document with 4"}
]
```
