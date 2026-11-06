---
format: aep.planning-md/3
id: approval-record:wave-55-review-correction
kind: approval-record
status: approved
title: 'Wave 55: one correction for the adversary and security findings'
summary: the in-acceptance review findings are fixed in the wave; seven rsi drafts take the rest
relations:
- decides: story:the-gate-reads-paths-inside-arguments
- decides: story:the-gate-holds-against-lint-allows-in-source
- decides: story:the-gate-does-not-trust-a-step-accepted-citation
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-06T08:24:01Z", actor: "human:timo", revision: 2}
---
# Decision: wave 55 review correction

## Decision

The orchestrator, under the owner's delegation, 2026-11-06, on the adversary pass (8 of 11 cases red) and the
security review (2 blockers, 4 warnings) of `wave/55` at d0e8fba54: "one correction in this wave, one implementor".

The correction fixes, in `crates/takt/src/validator.rs`:

- a `-c` line that sets `CDPATH`, or a binding whose `env` sets it, is refused (story:the-gate-reads-paths-inside-arguments);
- fish's `--command <line>` and `--command=<line>` are read, or refused (the same story);
- a word `key=path` that is no option (`dd if=`, `BASH_ENV=`) has its path compared, or is refused (the same story);
- the moved-key match of citation rows is one to one, exact keys first (story:the-gate-does-not-trust-a-step-accepted-citation);
- lint attributes are keyed by their scope, the crate root apart from each `mod` path
  (story:the-gate-holds-against-lint-allows-in-source);
- a file a changed `.rs` file pulls in through `include!` or `#[path]` is scanned, or the gate fails closed, and a
  linked `.rs` file fails closed (the same story);
- `cfg_attr` nesting is capped and refused past the cap, and an unclosed `#[` refuses once at the first (the same story).

The review's test targets `crates/takt/tests/w55_adversary.rs` and `crates/takt/tests/w55_security.rs` are committed
with the correction. The adversary's red cases outside the wave's acceptance are not committed and carry no
`#[ignore]`: their bodies are kept in the backlog drafts filed at the wave's close (a Taskfile named by `task -t` and
its includes, a crate-level allow of a single lint, a citation re-pointed to a copy after the cited line changed).

Backlog drafts, tagged `rsi`, filed at the wave's close: `task -t` includes; a citation re-pointed to a copy after the
cited line changed; a crate-level allow of a single lint; a Taskfile's `dotenv:` files and the scripts its `cmds:` run;
`env -S` and other programs that split a command string; the include walk refusing every gate over 256 Taskfiles or one
unresolvable fixture include; scalar fields of `ess-inputs.yaml` and `system.yaml`.
