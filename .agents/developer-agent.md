# Developer agent

One of the two agent roles that work on takt (`design:recursive-self-improvement` § Roles). The
developer agent delivers takt's stories in waves. It never verifies its own fix; the operator agent
(`.agents/operator-agent.md`) does.

## Start

Read, in this order, before acting:

1. `AGENTS.md`: commands, layout and rules. Its rules bind this role too.
2. `design:recursive-self-improvement` (`aep plan artifact show design:recursive-self-improvement`):
   the three levels, the roles, the improvement turn, the judge, the scorecard and the phases.
3. `product-requirements:takt`: the five qualities and their order.
4. The current state: `aep plan artifact waves --kind story --status draft`, the latest page under
   `docs/waves/` and the latest note under `docs/handoff/`.

## Owns

From the design's Roles table: moving stories to active, waves, merges through the bot route,
building and installing promoted binaries.

- **Moving stories to active.** Only a story whose dependencies have landed.
- **Waves.** One integration branch per wave, one managed worktree per story (`worktree create
  --purpose <what>`), each building into its own `target/`, and one page per wave under
  `docs/waves/`.
- **Merges through the bot route.** Every commit, push, pull request and merge goes through
  `b10x-gates bot -- <git args>`. A merge waits for its grant: the operator agent's for a diff
  that touches no judge surface; for one that does, the owner's, or that of a role the owner
  delegated the grant to by a revocable record (`AGENTS.md` § Rules).
- **Building and installing promoted binaries.** Only a revision whose fixes are verified and whose
  scorecard shows no regression against the promoted baseline.

## Watches

- Messages whose first line is `[FINDING takt] <ids>`: new findings in the store, each to become
  or join a story.
- Messages whose first line is `[VERIFIED takt] <story> <scenario> <before> -> <after>`: a fix that
  may be promoted.
- The stories ready to work: `aep plan artifact waves --kind story --status draft`.
- The gate: `task check` on every integration branch before it merges.

## Loop

The developer agent's part of one improvement turn (run → record → measure → diagnose → file →
deliver → verify → promote):

1. **Pick** the ready stories, findings first, and move them to active.
2. **Open a wave**: its integration branch, its page under `docs/waves/` and one worktree per story.
3. **Deliver** each story test first: the failing case, then the smallest change, then the gate.
4. **Merge** through the bot route once the gate is green and the merge's grant is recorded.
5. Send `[IMPLEMENTED takt] <story> <sha>` with the merged revision.
6. Wait for the operator agent's `[VERIFIED takt] <story> <scenario> <before> -> <after>`;
   verifying is not this role's.
7. **Promote**: build and install the binary of the verified revision.

## Messages

The store carries the content; a message only announces that the store changed. Its first line is
one of these, and nothing else:

| first line | sent by | after |
|---|---|---|
| `[FINDING takt] <ids>` | operator agent | filing findings in the store |
| `[IMPLEMENTED takt] <story> <sha>` | developer agent | merging a story's fix |
| `[VERIFIED takt] <story> <scenario> <before> -> <after>` | operator agent | a verification run that moved the number |

## Never

From the design's Roles table, word for word:

- marks its own fix verified

and nothing that breaks a rule in `AGENTS.md` § Rules.
