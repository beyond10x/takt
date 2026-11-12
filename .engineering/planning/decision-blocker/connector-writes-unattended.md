---
format: aep.planning-md/3
id: decision-blocker:connector-writes-unattended
kind: decision-blocker
status: cleared
title: 'Owner: may takt grant itself a connector write under a scoped delegation?'
relations:
- blocks: story:connector-writes-under-a-delegation
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-11-12T19:12:18Z", actor: "human:timo", revision: 3}
---
## Question

May takt grant itself a connector write, so that an unattended reconcile edits its status message whenever it
changes? takt's grant binds to a revision, so without this every changed tick waits for a person's `takt approve`
(exit 4).

Options put to the owner (design `docs/design/scheduled-runs-and-connector-actions.md` § 5, first draft): A, every
write waits for `takt approve`; B, a scoped delegation of `connector.write` in `.engineering/delegations.yaml`; C, only
the Connectors standing grant limits writes.

## Who answers

The owner.

Decider: operator

## Answer

2026-11-12, the owner: "in our specific case, there should be no approval, ideally this behavior/permissions can be
configured via config file".

So: no per-case `takt approve` for a declared connector write. The repository declares the writes it allows in its
`.engineering/delegations.yaml` (connector and instance, operations, target scope, an optional rate); takt grants a
write inside the declaration itself, records the grant in the case, and refuses any write outside it.
`AGENTS.md` § Rules names this as the second path by which takt grants itself, with these words and this date.
Built by story:connector-writes-under-a-delegation. Status: cleared.
