---
format: aep.planning-md/3
id: story:finding-on-any-repository
kind: story
status: implemented
title: A finding names the repository, seed and test of the task it was seen on
relations:
- informed_by: story:finding-intake
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T23:06:57Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-18T23:06:58Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-18T23:07:06Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A finding seen on any task can be filed and checked: an `expected_before` block may name the repository URL, the seed commit and the test command of the task it was seen on, instead of only a scenario id declared in the scratch repository's `.engineering/scenarios.conf`, and `scripts/check-finding` clones that repository at that seed.

## Context

The owner's direction of 2026-10-18 is to run many different tasks by hand (story-sized merged units of public beyond10x repositories, a realistic scenario corpus, a common benchmark) and turn each defect into an improvement turn. Today a predicate binds to `scenario: <id>`, which `scripts/check-finding` resolves only in the checkout given by `--scenarios` (`scripts/check-finding:5-14`, the `[scenario "<id>"]` block read with `git config --no-includes --blob`), and `scripts/file-finding` checks that key (`scripts/file-finding`, header: "scenario the scenario's id, as `.engineering/scenarios.conf` of the scenario repository declares it"). A defect seen on a task from another repository cannot be filed. All three improvement-turn findings so far came from code reviews; none came from a run (36 takt runs on 2026-10-18, scenario 1 passing every one).

## Scope

- `scripts/file-finding`: accept `repository`, `seed` and `test` keys as an alternative to `scenario` (cited: its header documents the keys)
- `scripts/check-finding`: clone `repository` at `seed` when the predicate names them; keep `--scenarios` for the `scenario` form (cited)
- `protocols/self-improvement/1.yaml`: `finding_record` and `scenario_result` descriptions, if they name the scenario form (inferred)
- `ess/domains/takt.yaml`: the Finding entity's predicate fields (inferred)

## Acceptance

A finding whose `expected_before` names `repository: https://github.com/beyond10x/canon`, a `seed` and a `command` is filed by `scripts/file-finding` and checked by `scripts/check-finding <finding> <revision>` without `--scenarios`, printing `reproduced` or `not_reproduced`; a predicate naming both `scenario` and `repository`, or neither, is refused as `malformed`; existing scenario-form findings still check unchanged.
