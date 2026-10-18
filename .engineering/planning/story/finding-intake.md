---
format: aep.planning-md/3
id: story:finding-intake
kind: story
status: implemented
title: Every finding is filed with a machine-checkable expected_before predicate
summary: story or blocker, predicate, source path, --ref dedupe
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:agent-profiles
- depends_on: story:scenario-corpus
scope:
- confidence: cited
  path: .agents/operator-agent.md
- confidence: inferred
  path: ess/domains/takt.yaml
- confidence: cited
  path: scripts/check-finding
- confidence: cited
  path: scripts/file-finding
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T13:07:06Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":7}}}
- {from: "proposed", to: "active", at: "2026-10-18T13:07:06Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":7}}}
- {from: "active", to: "implemented", at: "2026-10-18T13:58:13Z", actor: "human:timo", revision: 11, decided_on: {"recorded":{"test_result":1,"review_outcome":9}}}
---
# Story: finding-intake

## Outcome

`scripts/file-finding` files a finding as a draft story or blocker in this store carrying an
`expected_before` predicate (a command, a named scenario, the expected exit status or output), the
path of the run record or review that produced it, and the reference `finding:<key>`; it refuses a
second filing of a key already in the store; and `scripts/check-finding <id>` runs the predicate on
its scenario at a given takt revision and prints `reproduced` or `not_reproduced`.

## Context

design:recursive-self-improvement § One improvement turn. On 2026-10-18 thirty review findings
and two trial defects reached the developer agent as chat messages, some never delivered, and two
sessions shared one name. `aep plan artifact new` accepts a second artifact with the same `--ref`,
so the refusal is the script's: it runs `aep plan artifact list --ref finding:<key>` first. The
intake procedure is written into `.agents/operator-agent.md`, which story:agent-profiles creates.

## Domain relations

- Finding → Story: a finding becomes one story or one blocker. Inferred from the design.
- Finding → Scenario: the predicate names one scenario. UNMAPPED: whether a finding may need several.

## Scope

Derived 2026-10-18 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `scripts/` (new directory; absent at 768db1c and on main) — cited, the story's Outcome names both scripts
- **Files:** new `scripts/file-finding` — cited, story Outcome
- **Files:** new `scripts/check-finding` — cited, story Outcome
- **Files:** `.agents/operator-agent.md` (intake procedure appended to the file story:agent-profiles creates; absent at 768db1c) — cited, story Context
- **Symbols:** none in Rust; the scripts shell out to `aep plan artifact list --ref finding:<key>` and `aep plan artifact new --ref` (both flags exist in the installed aep's `--help`) — cited
- **Also likely:** `ess/domains/takt.yaml` — inferred, AGENTS.md:37-39 says a story that introduces a noun must declare it first; `takt.work.Scenario` is declared (ess/domains/takt.yaml:1007) but no Finding entity is
- **Documents:** `.agents/operator-agent.md` (above); filing also writes draft artifacts into `.engineering/planning/` at runtime, which is store content and not a scope file — cited
- **Confidence:** high — the story names every file it creates or edits; the only open item is the ESS declaration
- **Would collide with:** any unit editing `.agents/operator-agent.md` (story:agent-profiles creates it; story:loop-scorecard edits it, loop-scorecard.md:43, with no edge to this story), and any unit editing `ess/domains/takt.yaml` if the Finding noun is declared there — cited for the profile, inferred for ess
- **Collides with nothing in the Rust crates:** no `crates/` path is in scope — inferred, from the story naming only scripts and the profile
- **Safety fact:** `scripts/check-finding` must print exactly `reproduced` / `not_reproduced`, the result literals of the `scenario_result` evidence kind in protocols/self-improvement/1.yaml:45-48 (and the filer's results must be `filed` / `malformed`, :41-44); story:self-improvement-protocol runs this script for `scenario.reproduce`/`scenario.verify` (self-improvement-protocol.md:48), so this output is a wire format another reader parses — step 2, unproven

Not established: whether `aep plan artifact new blocker` resolves to an accepted kind; how `check-finding` builds takt at a given revision (worktree, promoted binary or `Taskfile.yml` build); whether `check-finding` reads a scenario file in the scratch repository; whether the ESS entity is needed for shell scripts.

Typed entries: `scripts/file-finding` (cited), `scripts/check-finding` (cited), `.agents/operator-agent.md` (cited), `ess/domains/takt.yaml` (inferred).

## Ordering

- depends_on story:agent-profiles: the procedure goes into the profile it creates.
- depends_on story:scenario-corpus: predicates run on its scenarios.

## Acceptance

`scripts/check-finding` on the default-protocol finding (a story taken from the store runs under
`software.change/1`), filed through `scripts/file-finding` against scenario 1, prints `reproduced`
with a takt built from revision `b71cb85`, where the defect is present.

## Checks

- The same finding prints `not_reproduced` with a takt built from the revision that fixes it (story:cli-protocol-defaults, epic:composable-agentic-work).
- A second `scripts/file-finding` with the same key exits non-zero and `aep plan artifact list --ref finding:<key>` returns one artifact.
- A finding without an `expected_before` block is refused at filing.
- `scripts/check-finding` takes the takt revision as an argument; the promoted revision is passed once story:promotion records one.

## Out of Scope

Verifying fixes inside a case (story:self-improvement-protocol); the review panel itself.

## Landed (wave 08)

Read from `git diff --stat 4cbc096 eb4542a` at the close of wave 08 (wave branch head c5d9ae3). `scripts/file-finding` and `scripts/check-finding`; the operator profile's Intake section; `takt.work.Finding` declared in ESS. The acceptance ran end to end without network: takt at b71cb85 `reproduced`, at 4cbc096 `not_reproduced`.

- `.agents/operator-agent.md`
- `crates/takt-cli/tests/finding_scripts.rs`
- `crates/takt-cli/tests/finding_scripts_adv1.rs`
- `crates/takt-cli/tests/finding_scripts_adv2.rs`
- `ess/domains/takt.yaml`
- `scripts/check-finding`
- `scripts/file-finding`
