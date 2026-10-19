---
format: aep.planning-md/3
id: review-result:adversary-case-id-pass-2
kind: review-result
status: active
title: Adversary, case-id-is-a-valid-branch-name, pass 2
relations:
- reviews: story:case-id-is-a-valid-branch-name
revision: 1
---
# Adversary pass 2: story:case-id-is-a-valid-branch-name

Unit at a158246 (impl/case-id; unit 8d49743, correction 1 a158246). Verdict: NEEDS-CHANGE on one
pre-existing defect; 4 red cases in `crates/takt/tests/case_id_branch_name_adv2.rs` and
`crates/takt-cli/tests/case_id_branch_name_cli_adv2.rs`, one green case holding the lone-`@` guard;
628 → 633 executed.

Attacked without a break: the 250-byte limit (no name built from a case id is longer than
`<id>.lock`); the default-ignorable and format-character tables (identical, range for range, to
Unicode 16.0.0's lists); git's byte-wise rules for non-ASCII; terminal escapes and bidi controls in
check_id's own refusal.

```findings
[
  {
    "file": "crates/takt-cli/src/main.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "takt run --case joins the id to a path and reads protocol.yaml before check_id refuses it: takt run --case /some/dir reads /some/dir/protocol.yaml and echoes its key `secret` in the error instead of check_id's refusal",
    "line": 1206
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "note",
    "message": "current() repeats the branch name raw around check_id's escaped refusal: a tree on branch takt/evil<U+202E>kcab gets an error with a raw right-to-left override from the validators' case lookup",
    "line": 1119
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "note",
    "message": "shown() is not one-to-one: two different ids get the same refusal: ids a\\<U+200B> and a\\\\u{200b} both refuse as the case id `a\\\\u{200b}` with the same reason",
    "line": 117
  },
  {
    "file": "crates/takt/src/frontier.rs",
    "category": "correctness",
    "severity": "note",
    "message": "ids takt mints from a story name or scenario id can pass 250 bytes and be refused: a 228-byte story name mints a 251-byte case id that check_id refuses; a scenario id of 235 or more bytes is refused only after its trial branch is cut and pushed",
    "line": 68
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "note",
    "message": "no test holds the lone @ refusal; deleting the guard leaves the suite green: the corpus skips every id git accepts, and git accepts takt/@, so removing the `@` branch goes unnoticed",
    "line": 149
  }
]
```
