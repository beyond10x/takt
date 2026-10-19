---
format: aep.planning-md/3
id: review-result:adversary-case-id-pass-1
kind: review-result
status: active
title: Adversary, case-id-is-a-valid-branch-name, pass 1
relations:
- reviews: story:case-id-is-a-valid-branch-name
revision: 1
---
# Adversary pass 1: story:case-id-is-a-valid-branch-name

Unit at 8d49743 (impl/case-id, from fc0ee65). Verdict: NEEDS-CHANGE, 3 red cases in
`crates/takt/tests/case_id_branch_name_adv1.rs`; 439 → 442 in the b10x-takt package. All three are
pre-existing (red at the base too).

Attacked without a break: git's ASCII branch-name rules (`..`, `@{`, `.lock`, a leading `.`,
`~^:?*[\`, control characters, DEL); a `-` prefix; `HEAD`, `FETCH_HEAD`, `main` as ids; `@` alone;
zero-width and invisible format characters; every open, resume, approve and current-case path
through check_id; nothing written before a refusal; the ESS citations. Case-insensitive
filesystems were not testable on this machine.

```findings
[
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "check_id accepts ids of 251-255 bytes whose takt/<id> ref lock file git cannot create: takt run --case-id with a 251-byte id passes check_id and fails inside git checkout -b with File name too long",
    "line": 97
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "note",
    "message": "refusal names an id holding a quote with a backslash the operator never typed: check_id(\"it's~\") says the case id `it\\'s~` is refused",
    "line": 148
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "note",
    "message": "check_id doc claims no id displays as another, yet NFC/NFD and CGJ pairs both pass: ids caf\\u{e9} and cafe\\u{301} both open separate cases and branches that render identically",
    "line": 124
  }
]
```
