---
format: aep.planning-md/3
id: story:the-scan-misses-renamed-and-macro-made-includes
kind: story
status: draft
title: The confinement scan misses a renamed include and a macro-made path attribute
summary: 'include! renamed by use, and a #[path] whose name or meta a macro supplies, bring a file the scan never reads into a non-test build'
tags:
- finding
- priority:medium
- rsi
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# The confinement scan misses a renamed include and a macro-made path attribute

## What is wrong

The confinement guard (`crates/takt/tests/confinement.rs`, `no_module_but_the_seam_names_the_process_command`)
reads every source under `crates/*/src` as text and flags each way a file can bring in code the
scan does not read, among them `include!` and `#[path = …]`. Wave 44's adversarial pass on the
guard found two spellings it passes over, both older than wave 44:

- `use std::include as inc;` then `inc!("../../elsewhere.rs");`: the renamed macro is not flagged.
- a `#[path]` attribute whose name or whole meta a macro supplies: `#[$a = "…"] mod x;` with
  `attr!(path)`, and `#[$m] mod x;` with `meta!(path = "…")`.

The pass compiled each with rustc 1.99.1: each brings a file the scan never reads into a non-test
build, so a process started there is not found. Nothing in the tree writes either form today.

Seen in the cases `w44_adv_a_renamed_include_is_flagged` and
`w44_adv_a_path_attribute_a_macro_makes_is_flagged` (`crates/takt/tests/confinement.rs`, ignored
as this finding).

## Check

Not runnable by `scripts/check-finding`: it runs a library test, not `$TAKT`.

```expected_before
command: cargo test -p b10x-takt --test confinement -- --include-ignored w44_adv_a_renamed_include_is_flagged w44_adv_a_path_attribute_a_macro_makes_is_flagged
exit: 101
healthy_exit: 0
output: test result: FAILED
source: crates/takt/tests/confinement.rs
```

## Outcome

The scan flags a `use` that renames `include`, `asm`, `global_asm` or `naked_asm`, and any
attribute whose name or meta is a macro metavariable (`$…`). Both cases pass with their
`#[ignore]` removed, and every other case of `confinement.rs` still passes.
