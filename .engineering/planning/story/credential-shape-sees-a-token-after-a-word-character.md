---
format: aep.planning-md/3
id: story:credential-shape-sees-a-token-after-a-word-character
kind: story
status: draft
title: A credential after a word character is never withheld
tags:
- finding
- priority:high
- rsi
refs:
- provider: finding
  reference: credential-shape-sees-a-token-after-a-word-character
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/tests/failed_gate_lines.rs
revision: 2
---
# A credential after a word character is never withheld

## What is wrong

`credential_shape` (`crates/takt/src/run.rs:8581` at 7c9e7129) looks for a token's prefix only
where the byte before it is not a word character: `if at > 0 && word(bytes[at - 1]) { continue; }`
(`crates/takt/src/run.rs:8613`), with `_` and `-` counted as word characters. So a GitHub token
after `test_` (`test_ghp_` and 36 letters) or an Anthropic token after `test_`
(`test_sk-ant-` and 30 letters) is not seen: in a test's name, a path, an identifier or any text
where the token is joined to the word before it.

Every caller trusts that answer: the failed gate's lines (`crates/takt/src/validator.rs:236`,
`:359`, `:775`, `:787`), a proposal's paths (`crates/takt/src/run.rs:8697`), the text checked at
`crates/takt/src/run.rs:8745`, and the harness's reads (`crates/takt/src/harness.rs:1577`,
`:1675`). A token so joined reaches a prompt, a case record or a refusal unwithheld. Wave 39's
reviewer showed it on the failed gate's lines: a pytest summary line
`FAILED tests/t.py::test_ghp_… - boom` was kept with the token in it (probe f3b and its round-1
predecessor). Secure ranks first among the qualities, so this finding ranks first.

## Check

Not runnable by `scripts/check-finding`: the command builds a probe test with cargo and does not
run `$TAKT`. Run it in a fresh checkout of the revision. It holds at 7c9e7129 by the code above;
it was not run there, because the wave's gate was building in that tree.

```expected_before
command: printf '%s\n' '#[test] fn probe() { for text in [format!("test_ghp_{}", "A".repeat(36)), format!("test_sk-ant-{}", "b".repeat(30)), format!("src/x_ghp_{}.txt", "C".repeat(36))] { assert!(takt::run::credential_shape(&text).is_some(), "no credential seen after a word character in {}", text.len()); } }' > crates/takt/tests/zz_finding_credential_after_word.rs && cargo test -p b10x-takt --test zz_finding_credential_after_word
exit: 101
healthy_exit: 0
output: no credential seen after a word character
source: docs/waves/wave-39.md
```

## Outcome

A token of every shape `credential_shape` knows is found wherever it starts, also right after a
word character, so every caller withholds it.

## Acceptance

1. `credential_shape` names a shape for `test_ghp_` plus 36 letters, `test_sk-ant-` plus 30,
   and a path holding `x_ghp_` plus 36; the predicate above passes (exit 0).
2. Each caller withholds such text: a failed gate's line naming `test_ghp_…` is withheld and
   counted, never kept (a case in `crates/takt/tests/failed_gate_lines.rs`).
3. The shapes found today are still found: every existing test of `credential_shape` and of its
   callers passes unchanged.
4. Ordinary text in which a prefix sits inside a lower-case word, such as
   `the-risk-assessment-of-the-quarterly-report-v2` (`sk-` inside `risk`), is decided by a test
   either way, and the commit names the choice; under the qualities' order a withheld false
   positive is preferred to a shown token.

## Scope

- `crates/takt/src/run.rs`: `credential_shape`, the word-boundary skip at :8613 (cited)
- `crates/takt/tests/failed_gate_lines.rs`: a failed test whose name holds a token joined to a
  word (cited)
