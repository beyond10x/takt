---
format: aep.planning-md/3
id: story:main-history-names-no-private-repository
kind: story
status: implemented
title: Local main's history names no private repository or home path before its first push
tags:
- priority:high
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: story:docs-site-with-sidebar
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T01:43:34Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-19T01:43:34Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-19T04:22:06Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Before local `main` is first pushed (site publishing option B deploys from `main`), its history names no private repository and no home directory: every commit from the first one that did (4f7182b, 2026-10-18T19:40+02:00) onward is rewritten, every commit sha cited in tracked text is mapped to its rewritten sha, and a commit map from old to new is kept so records outside the repository can be mapped too.

## Context

Owner decision of 2026-10-19: "A rewrite main", on the question whether to rewrite local history, publish it as it is, or deploy the site from an orphan branch. The private name entered history at 4f7182b and home-directory paths at e593f33; today's tree has neither (main f7e0616 scrubbed the store; wave 12's 7518a71 scrubs `ess/`). Both entry points descend from a0b8576, so a0b8576 and its 235 ancestors keep their shas, and with them the promotion baseline record (revision a0b8576). Of 468 commits on main at f7e0616, 233 come after 4f7182b; every revision cited since then changes, among them the turn candidates 98719c3 and 9fb14cc, the promotion merge 9cc158b and every sha in review-results and story bodies after that point.

Timing, decided by the planner: at the close of wave 12, after its merge into `main` and before any push of `main`, so the fewest branches are open.

## Method

1. A bundle of every ref before the rewrite (`git bundle create <cache>/pre-rewrite.bundle --all`), which undoes it.
2. `git filter-repo` over all refs of a fresh clone: `--replace-text` for the private name ("the organization's benchmark instrument") and for home-directory paths (`<home>`), keeping its `commit-map`.
3. A second pass maps every abbreviated or full sha in tracked text (store, `ess/`, `docs/`) through the commit map; a sha that does not resolve after the pass is listed, not guessed.
4. Records outside the repository that name rewritten shas (the planner's R0 turn cases, measurement summaries, the check-finding build cache) are mapped with the same file; the promotion store needs nothing (its revision predates the rewrite).
5. Branches open at that moment are re-pointed through the map; their trees are finished first where they can be.

## Scope

- every ref of the repository (inferred)
- `.engineering/**`, `ess/**`, `docs/**`: the sha citations (inferred)

## Acceptance

On the rewritten `main`, `git log -p --all | grep -i -E 'beyond10x/[b]ench\b|/home/[a-z]+' | grep -v -c '/home/[s]omeone/'` prints 0 (the excluded path is a test fixture's made-up one, not a home directory); `python3 -I scripts/check-citations --require-digests` and `aep plan artifact validate` pass; every 7-to-40-hex token in `.engineering/` that named a commit before the rewrite names a commit after it; `task check` passes at the rewritten tip; the pre-rewrite bundle restores the old history.
