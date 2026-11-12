---
format: aep.planning-md/3
id: decision-blocker:connector-writes-unattended
kind: decision-blocker
status: open
title: 'Owner: may takt grant itself a connector write under a scoped delegation?'
relations:
- blocks: story:connector-writes-under-a-delegation
revision: 1
---
## Question

May takt grant itself a connector write under an owner's delegation, so that an unattended reconcile edits its status
message whenever it changes? takt's grant binds to a revision, so without this every changed tick waits for a person's
`takt approve` (exit 4).

Options (design `docs/design/scheduled-runs-and-connector-actions.md` § 5):

- A. Keep the rule: every write waits for `takt approve`. Unattended reconcile cannot post. No rule changes.
- B. A delegation of `connector.write` in `.engineering/delegations.yaml`, scoped to one action id and an expiry,
  naming a role; takt grants itself per case under it, as for `repository.merge`. Amends AGENTS.md § Rules; widens
  authority.
- C. No takt capability on the write; only the Connectors standing grant limits it.

Recommendation: B. Until answered, A is built (story:a-connector-write-under-a-grant).

## Who answers

The owner.

Decider: operator

## Answer

Open.
