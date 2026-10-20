---
format: aep.planning-md/3
id: story:takt-models-its-inner-workings-in-ess
kind: story
status: draft
title: 'takt models its inner workings in ESS: the commands a trial, scorecard and promotion go through, a promotion naming every pinned layer revision, the wave 22 layer fields'
summary: The domain declares commands and views so ess synthesises a conformance suite task check runs; Promotion gains layers (ADR 0101 point 5); today CI only validates
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: takt-models-its-inner-workings-in-ess

## Outcome

takt's ESS domain models how takt works, not only what it records: the commands a trial, a
scorecard and a promotion go through, with their inputs, responses and outcomes (the refusals takt
prints as exit 1 among them), the views the records are read through, a promotion record that
names the full set of revisions the promoted takt was pinned to, one per layer of the stack it was
trialled on (ADR 0101 point 5), and the layer fields wave 22 added, each declared and validated
with the `ess:specifying` skill. From the domain `ess` synthesises a conformance suite that `task
check` runs against the takt binary; today CI validates the specification and tests nothing
against it.

## Context

- The orchestrator's ESS survey (10:50, 2026-10-20), relayed from the owner (10:55): file one
  story that models takt's inner workings in ESS with the `ess:specifying` skill, covering the
  three points below; propose it for wave 24.
- Promotion. ADR 0101 point 5: "an accepted candidate merges and releases through its own
  repository's process; takt then moves its pin, and the promotion record names the full set of
  pinned revisions" (`atlas/architecture/adr/0101-trials-measure-every-layer.md:35-37`).
  `takt.work.Promotion` (`ess/domains/takt.yaml:4134-4377` on `wave/22`) names takt's own
  `revision`, `tree`, `binary` and `binary_sha256` and nothing of any other layer, while the run
  record's `layers` map and the scorecard row's `layers` (wave 22) hold what a promotion would
  name. Declare it in ESS before building it.
- Commands. `ess/domains/takt.yaml` on `wave/22` holds 125 `takt.work.*` entities and no
  `commands:` or `views:` block; `Taskfile.yml:11` runs `ess specify validate --path ess` and
  nothing more of ESS, so no conformance suite can be built. The records already have a writer and
  a reader each in the CLI (`crates/takt-cli/src/main.rs:38-115`: `run`, `approve`, `scenario
  run`, `scorecard`, `trace`, `report`, `view`, `promote`, `replay`).
- Layers. Wave 22 declares `RunOpened.layers`, `RunStep.version` and `RunStep.model`,
  `ScorecardRow.layers` and `Trace.layers` (`ess/domains/takt.yaml:712,1287,4749` on `wave/22`;
  they validate with ess 0.60.0). Once wave 22 is merged, the commands of this story take them as
  their response fields; until then this story cites the wave branch.
- Comes from self-improvement work, so it carries `rsi`.

## Scope

- `ess/domains/takt.yaml` (`commands:` and `views:` blocks; `takt.work.Promotion` gains
  `layers`; the relations between a promotion, its scorecard and the scorecard's rows stated as
  `relations:` entries), its citations, `ess/system.yaml` where the suite needs a target declared.
- `crates/takt/src/promotion.rs` (the record written and read), `crates/takt-cli/src/main.rs`
  (`takt promote` and its listing), tests and a fixture promotion record.
- `Taskfile.yml` (the conformance task), `.github/workflows/check.yml` if it names the task.
- A conformance target: a thin adapter that answers `ExecuteCommand`/`QueryView` by running the
  takt binary on a scratch repository (`crates/takt-cli/tests/` or `scripts/`), and the fixtures
  its scenarios need.

## Acceptance

1. The domain declares at least: `takt.work.RunScenario` (writes a run record; outcomes: a
   completed run, a stopped run naming its stop, a refusal at open naming why),
   `takt.work.SaveScorecard` (writes a scorecard; outcomes: saved, refused on unknown run tokens
   naming the case and the step, refused on differing layers naming the layer and the runs),
   `takt.work.Promote` (writes a promotion or a refusal record; outcomes: promoted, refused on the
   rule naming the worse scenarios, refused before judging on a scorecard it cannot read),
   `takt.work.Approve` (writes a grant); and the views `takt.work.Trace`, `takt.work.Report` and
   `takt.work.View` over the records they read. Each command's input and response are the
   existing entities, the wave 22 layer fields among them; a noun no entity declares is declared
   before a command uses it, and a relation between two nouns is a `relations:` entry, never
   prose.
2. `takt.work.Promotion` declares `layers: Optional<Map<takt.work.LayerName, String>>`, the
   promoted takt's own revision among them under `takt`, its comment citing ADR 0101 point 5;
   `takt promote` writes it from the candidate scorecard's rows: a test on a scorecard whose runs
   all state the same `harness` and `model` layers records them; one whose runs disagree on a
   layer is refused before anything is installed, naming the layer and the runs; a record written
   before the field reads as "layers not recorded", never as a revision (a test on 0.5.0's
   fixture record); the CLI's promotion listing prints each promotion's layers, `not recorded`
   when absent.
3. `ess specify validate --path ess` passes, and every `read from` citation resolves with
   `python3 -I scripts/check-citations --require-digests`; the suite `ess` synthesises from the
   domain has at least one scenario per outcome of 1.
4. `task check` runs the suite against the built binary through the adapter and it passes with no
   skipped scenario, its counts held by a gate line; a mutant binary that writes a promotion with
   an empty `cases` list, and one that writes a promotion naming no `takt` layer, each fail a
   scenario.
5. The conformance report is recorded on this story as evidence (`aep plan artifact evidence
   <id> --from <report>`), and the story's ESS work is done through the `ess:specifying` skill
   (its interview for any noun the code does not settle).
