---
format: aep.planning-md/3
id: review-result:adversary-citation-check-pass-1
kind: review-result
status: active
title: Adversary, citation-check-in-task-check, pass 1
relations:
- reviews: story:citation-check-in-task-check
revision: 1
---
unit: story:citation-check-in-task-check, worktree w9-citation-check at 564d6d8 (base 8ce2019) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 373→384, red 9
origin: introduced 11 / pre-existing 0 / undecided 0
wrote-outside-worktree: 7 paths, all in the unit's scratch `adv1/`
needs-coordinator: whether a `read from:` outside a full comment line must fail (decided: it must)

**Cases** (`crates/takt-cli/tests/citation_check_adv1.rs`; alone: `2 passed; 9 failed`)

| case | expected | actual |
|---|---|---|
| `a_citation_in_an_inline_yaml_comment_is_checked_not_skipped` | exit 1 | exit 0 |
| `a_capitalised_read_from_marker_is_not_skipped` | exit 1 | exit 0 |
| `a_read_from_in_a_block_scalar_is_not_skipped` | exit 1 | exit 0 |
| `needles_written_for_a_section_citation_are_checked` | exit 1 | exit 0: the row counts as used, never matched |
| `a_heading_renamed_to_a_longer_one_does_not_hold_the_cited_section` | `## Section one hundred` does not hold `(§ Section one)` | exit 0 |
| `a_range_that_a_shorter_function_leaves_swallowing_the_next_one_is_unresolved` | exit 1 | exit 0: the range covers `pub fn gamma` |
| `fix_does_not_re_point_a_citation_onto_another_function` | no `lib.rs:6-10` | fix wrote `lib.rs:6-10`, check then exits 0 |
| `fix_changes_only_the_citation_lines_of_a_crlf_document` | CRLF kept | every line converted to LF |
| `a_cited_file_that_is_not_utf8_fails_with_a_message_not_a_traceback` | no traceback | `UnicodeDecodeError` traceback, empty stdout |
| `a_second_needle_row_for_a_key_fails_the_check` | exit 1 | green; red against a mutant disabling the guard |
| `a_needle_row_without_a_tab_or_with_an_empty_needle_fails_the_check` | exit 1 | green; red against a mutant |

**Gate.** `task check`: ess 0, check-citations 0 (319 of 319), fmt 0, clippy 0, cargo test 101 at `citation_check_adv1` only. Without fail-fast: 384 executed, 375 passed, 9 failed.

**Attacked, not broken.** Shifting cited text by ±5 lines leaves 0 of 308 line citations resolving except the `steps: 0,` row; needles cannot match across the range boundary; `../` and absolute paths fail; a duplicate key is refused; a deleted file or missing TSV is unresolved; CRLF resolves in check mode; the citation step runs second in `task check`, `fix` never; both AGENTS.md lines byte-identical, one hunk inside Commands.

```findings
[
  {"file": "scripts/check-citations", "line": 80, "category": "boundary", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a read-from in an inline YAML comment or a block scalar is skipped silently, so a citation there is never checked"},
  {"file": "scripts/check-citations", "line": 43, "category": "boundary", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the marker match is case-sensitive, so a citation written as Read from: is skipped silently"},
  {"file": "scripts/check-citations", "line": 210, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "section and file citations carry no needles, against the brief's every-citation-has-needles decision, and a needle row written for one counts as used without ever being matched"},
  {"file": "scripts/check-citations", "line": 219, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "a section citation resolves on any heading line that contains the cited words as a substring, so a heading renamed by extension stays green"},
  {"file": "scripts/check-citations", "line": 138, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "resolves never bounds the last needle to the range end, so a range that a code deletion makes swallow the next function stays green"},
  {"file": "scripts/check-citations", "line": 152, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "fix follows later needles to the end of the file and re-points a stale citation onto a span covering another function, after which check passes"},
  {"file": "scripts/check-citations", "line": 260, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "fix rewrites the whole document with LF line endings, changing every line of a CRLF document"},
  {"file": "scripts/check-citations", "line": 109, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a non-UTF-8 cited file ends the run with a UnicodeDecodeError traceback, exit 1 and nothing on stdout naming the file"},
  {"file": "ess/domains/takt.citations.tsv", "line": 64, "category": "property", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the single needle steps: 0, used by rows 64, 68 and 229 also resolves 44 lines below run.rs:418, in the other constructor"},
  {"file": "crates/takt-cli/tests/citation_check.rs", "line": 1, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "no unit case covers the duplicate-row or malformed-row guards; disabling either leaves the unit suite green"}
]
```
