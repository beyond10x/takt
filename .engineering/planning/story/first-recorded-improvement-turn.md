---
format: aep.planning-md/3
id: story:first-recorded-improvement-turn
kind: story
status: draft
title: The first improvement turn recorded as a self.improvement/1 case
summary: the default-protocol defect, scenario 1 before and after
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:scenario-corpus
- depends_on: story:self-improvement-protocol
- depends_on: story:cli-protocol-defaults
- depends_on: story:grant-binds-to-action-subject
scope:
- confidence: inferred
  path: .engineering/cases
- confidence: inferred
  path: .engineering/evidence/story
revision: 5
---
# Story: first-recorded-improvement-turn

## Outcome

The first improvement turn is recorded end to end as a `self.improvement/1` case and ends in a
promotion: the finding "a story taken from the store runs under `software.change/1`", its
`expected_before` predicate run on scenario 1 at the baseline (reproduced) and at the candidate
(not reproduced), the fix merged, the scorecard compared, and the candidate's binary promoted.

## Context

design:recursive-self-improvement § One improvement turn; epic:recursive-self-improvement's
acceptance asks for one promotion decided by the scorecard, and this turn is where it happens. The
defect was found by the first live trial on 2026-10-18 (`takt run` with no `--protocol` took the
story under the default `software.change/1`, `crates/takt-cli/src/main.rs:66`); its fix is filed as
story:cli-protocol-defaults (epic:composable-agentic-work, on a branch not yet on main on
2026-10-18). The other trial defect, the pull request opened before any commit, was fixed by hand
before a case could record it, so it has no baseline record and is not this turn.

## Scope

Derived 2026-10-18 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** records, not code. The turn's case directory `.engineering/cases/<id>/` in the takt tree that `takt run --repo` is given. `crates/takt/src/case.rs:1` and `:21` put a case at `<tree>/.engineering/cases/<id>/` — inferred: the story names the directory but not which tree
- **Files:** `.engineering/evidence/story/<finding-story>/`, the evidence record whose `--ref` names the case directory (story Checks). The layout is the one at `.engineering/evidence/story/cli-protocol-defaults/`, and the evidence call is like `crates/takt/src/frontier.rs:365-373` — inferred: the finding's story id does not exist yet
- **Files:** scenario 1 of the scratch repository (`story:remove-a-todo`), run once at the baseline and once at the candidate, each on its own `trial/<n>` — cited (design § Scenario corpus; story § Scope)
- **Symbols:** `self.improvement/1`, i.e. `protocols/self-improvement/1.yaml`. The `accepted` outcome requires `scenario.passes_after`, `scorecard.no_regression` and `promoted` (`protocols/self-improvement/1.yaml:196-204`). The story reads this file and does not change it — cited
- **Symbols:** the defect site. On main it is `crates/takt-cli/src/main.rs:66` (`default_value = "software.change/1"`). On `fix/2026-10-18-review` the candidate is already merged (`316fbfc`), with `STORY_DELIVERY`/`SOFTWARE_CHANGE` at `crates/takt-cli/src/main.rs:478-480` and the doc text at `:66-70` — cited
- **Also likely:** none in code. The fix is story:cli-protocol-defaults (Out of Scope) and the validators belong to story:self-improvement-protocol — cited (story § Out of Scope; self-improvement-protocol § Scope)
- **Documents:** none — cited (the Acceptance is a case directory and its last decision)
- **Confidence:** medium — no code lands, which the story says (cited). The case's tree and the finding's story id are inferred, and the scratch repository's path is not named anywhere read
- **Would collide with:** any other scenario-1 run on the scratch repository at the same time (`trial/<n>` and the store on it), and any other promotion of the installed takt binary (design § Scorecard allows one a day). It touches no source file, so it is disjoint from every code unit in the wave — inferred
- **Safety fact:** the case directory is left out of the kernel's change detection, commit and clean (`crates/takt/src/case.rs:40-49`, `is_under_cases`), and case ids cannot leave `.engineering/cases` (`crates/takt/src/case.rs:90-96`). So recording the turn changes no tracked source. Proof level 2 (pointed at file:line), unproven

Not established: which tree holds the case (takt or the scratch repository); the finding's story id (story:finding-intake files it); the scratch repository's local path; whether the case directory gets committed; the edge to story:cli-protocol-defaults can be added once wave 06 is in main.

Typed entries: `.engineering/cases` (inferred), `.engineering/evidence/story` (inferred).

## Ordering

- depends_on story:scenario-corpus: the before and after runs need a fresh base each.
- depends_on story:self-improvement-protocol: the case runs under it.
- story:cli-protocol-defaults is the candidate; its edge is added once its branch is on main.

## Acceptance

A case directory whose `case.yaml` names protocol `self.improvement` holds a last decision in which
outcome `accepted` is legitimate and claim `promoted` is true.

## Checks

- The baseline run's case shows the story taken under `software.change/1`; the candidate run's under `story.delivery/1`.
- The finding's story carries an evidence record whose `--ref` names that case directory.

## Out of Scope

Fixing the defect itself (story:cli-protocol-defaults).
