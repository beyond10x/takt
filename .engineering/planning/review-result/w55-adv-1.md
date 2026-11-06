---
format: aep.planning-md/3
id: review-result:w55-adv-1
kind: review-result
status: active
title: Wave 55 units a, b, c and d, adversary pass and security review
summary: eight in-acceptance findings fixed by the correction, three escalated as drafts
relations:
- reviews: story:the-gate-reads-paths-inside-arguments
- reviews: story:the-gate-compares-taskfile-includes
- reviews: story:the-gate-holds-against-lint-allows-in-source
- reviews: story:the-gate-does-not-trust-a-step-accepted-citation
revision: 1
---
# Adversary and security review, wave 55 units a, b, c and d

```
unit: story:the-gate-reads-paths-inside-arguments + story:the-gate-compares-taskfile-includes + story:the-gate-holds-against-lint-allows-in-source + story:the-gate-does-not-trust-a-step-accepted-citation
verdict: needs change
cases: adversary 11, red 8; security 8, red 8 when run against d0e8fba54
```

The cases are `crates/takt/tests/w55_adversary.rs` and `crates/takt/tests/w55_security.rs`, reviewed at d0e8fba54.

| finding | unit | case | outcome |
|---|---|---|---|
| `cd` under `CDPATH` set by the line or the binding's `env` entered a directory never compared | a | `a_path_after_a_cd_through_cdpath_is_compared_or_refused` | fixed |
| fish's `--command <line>` was skipped as a valueless option | a | `a_path_in_a_fish_command_line_given_by_its_long_option_is_compared_or_refused` | fixed |
| a path after `=` in a word that is no option (`dd if=`, `BASH_ENV=`) was not compared | a | `a_file_named_after_an_equals_sign_of_an_operand_in_a_shell_line_is_compared_at_the_base` | fixed |
| a group allow moved from a module to the crate root of one file was not named | c | `an_allow_of_clippy_all_moved_from_a_module_to_the_crate_root_...`, `a_group_allow_moved_from_a_module_...` | fixed |
| a group allow in a file pulled in through `include!` or `#[path]`, or behind a linked `.rs`, was not seen | c | `an_allow_of_clippy_all_in_a_file_include_pulls_in_...` and three security cases | fixed |
| `cfg_attr` nesting recursed without a bound (stack overflow); an unclosed `#[` built a quadratic summary | c | `a_deeply_nested_cfg_attr_is_refused_without_exhausting_the_stack` | fixed |
| the moved-key match of citation rows was not one to one, so a base digest justified any number of rows | d | `a_base_rows_digest_copied_onto_a_new_citation_...` and two security cases | fixed |
| a Taskfile named by `task -t` had its includes unread | b | removed from the file, kept with its draft | escalated: story:the-gate-follows-the-includes-of-a-taskfile-named-by-argument |
| a crate-level allow of one denied lint turned `-D warnings` green | c | removed from the file, kept with its draft | escalated: story:a-crate-level-allow-of-a-denied-lint-is-refused |
| a citation re-pointed to a copy after the cited line changed kept its digest | d | removed from the file, kept with its draft | escalated: story:a-citation-re-pointed-to-a-copy-is-not-trusted |

Out of the acceptance, filed as drafts: story:the-gate-compares-a-taskfiles-dotenv-and-scripts,
story:the-gate-reads-command-strings-other-programs-split, story:an-unrelated-taskfile-does-not-refuse-every-gate,
story:the-gate-compares-the-scalar-specification-inputs, story:the-step-guard-reads-key-path-words-as-the-gate-does
and story:an-include-the-gate-cannot-resolve-fails-closed.

Attacked and held: process starts stay inside the confinement seam; no slicing panic in the new readers; include cycles,
`..`, absolute and templated includes, anchors and linked Taskfiles are refused; `eval`, extended globs and a glued
`-Cdir` tail fail closed.

Decided by approval-record:wave-55-review-correction; the correction is dbd7bba5f.
