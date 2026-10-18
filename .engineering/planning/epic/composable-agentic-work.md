---
format: aep.planning-md/3
id: epic:composable-agentic-work
kind: epic
status: draft
title: 'Composable agentic work: one bounded, validated run at a time'
summary: A binary drives a repository through engineering protocols; Canon decides what is permitted, owed and earned
revision: 4
---
## Why

Safe agentic coding, applied. A YAML description of workflows made of engineering protocols and
building blocks (inner, outer and validation loops); a harness such as Claude Code drives a
workflow through the model; every step is validated by Canon over evidence, with AEP as the
record. One binary, pointed at a repository: it takes an intent or pulls the next unit from the
planning store and runs one iteration, three, or three hours. The design is `docs/design/takt.md`.
The nouns takt introduces (case, run, workflow, step, evidence producer) are declared in an ESS
domain document before stories build on them, the organization's spec-first rule.

## Acceptance

`takt run` on a repository with a planning store and ready stories finishes one story with no
manual step but the merge approval, and every claim in the final decision document was established
by a validator's evidence record, none by the harness's text.

## ## Findings (2026-10-18)

## Findings (2026-10-18)

Waves 01–05 landed on main (story-delivery protocol and domain model, the frontier, the gate
validator, the forge, the merge under authority): 159 cases, every wave with two adversary passes
and corrections, every finding fixed or recorded on the story. A live trial with the real harness
on this repository took a ready story, implemented it in one judgement step, re-validated the
tests and the gate at the new revision, and stopped at the pull-request gate because no forge was
configured. What the waves left open, in order of weight:

1. The forge path (push, pull request, checks, merge) is proven against an in-memory double and
   the adapter's command lines only; no run has pushed, opened or merged a live pull request.
   `decision-blocker:live-trial-repository` holds the decision.
2. Actions a validator owns (`gate.run`, `pr.open`, `ci.read`, `repository.merge`) are offered to
   the harness as judgement steps when no validator is registered for them; the harness cannot
   perform them and the run ends `NoProgress`. The loop needs a rule: a validator-owned action is
   never a judgement step; without its validator the run stops with a stop that names it.
3. With `--harness none`, a case whose only pending step is an approval stops `ApprovalRequired`
   (correction in wave 05); the kernel still relies on a harness to request the gate in every
   other setting. A kernel-side stop at the gate without a judgement step is not built.
4. Confinement: the harness runs in the tree with write access; judgement steps that touch
   `.engineering/cases` are refused and restored (wave 05), but `.git`, the planning store and
   every other path are still the harness's to write; the first harness commit carries whatever
   the tree held before (the frontier's own store move included). `story:confinement-seam` holds
   the evidence.
5. A run's result is not written back to the story: no `test_result` evidence, no move to
   `implemented` (`story:run-record-and-story-evidence`, which the live trial drafted on a local
   branch outside the wave process).
6. Cost: the gate validator hashes the executable and the tree on every observation (about 2 s in
   a debug build); the harness reported 2,848,613 tokens for two judgement steps in the trial
   (cache reads included). No comparison exists yet.
7. The lock a run holds on its case is a file, not an OS lock; two runs starting in the same
   instant are not told apart. The planning store records takt's own moves with the actor it
   finds in the environment.
8. The README and the CLI reference do not describe `--gates`, `--project`, `--forge`, `--case`
   or `takt approve`; `--help` does.
