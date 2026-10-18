---
format: aep.planning-md/3
id: story:finding-intake
kind: story
status: draft
title: Every finding is filed with a machine-checkable expected_before predicate
summary: story or blocker, predicate, source path, --ref dedupe
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:agent-profiles
- depends_on: story:scenario-corpus
revision: 4
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

- new: `scripts/file-finding` (inferred)
- new: `scripts/check-finding` (inferred)
- `.agents/operator-agent.md` (the intake procedure, in the file story:agent-profiles creates) (inferred)

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
