---
format: aep.planning-md/3
id: decision-blocker:live-trial-repository
kind: decision-blocker
status: open
title: Nobody has decided which repository the live trial of the forge runs against
relations:
- blocks: story:story-done-end-to-end
revision: 2
---
# Nobody has decided which repository the live trial of the forge runs against

## ## Decision

## Decision

The forge (push, pull request, checks, merge through the bot route) is tested against a double
and the adapter's command lines. Proving it live means pushing a branch, opening and merging a
pull request on a repository the bot route may push to. Of the repositories enrolled in the gates
policy none is a scratch repository, and a push to this repository before the proof is against
the operator's rule. Options: a new scratch repository under the organisation (the operator's
call: a new repository), the trial on this repository itself, or no live trial. Cleared when the
operator names the repository or declines; the proof then is one run to the gate, `takt approve`,
and one resumed run.
