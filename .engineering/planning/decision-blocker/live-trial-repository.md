---
format: aep.planning-md/3
id: decision-blocker:live-trial-repository
kind: decision-blocker
status: cleared
title: Nobody has decided which repository the live trial of the forge runs against
relations:
- blocks: story:story-done-end-to-end
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-18T09:25:49Z", actor: "human:timo", revision: 4}
---
# Nobody has decided which repository the live trial of the forge runs against

## Decision

The forge (push, pull request, checks, merge through the bot route) is tested against a double
and the adapter's command lines. Proving it live means pushing a branch, opening and merging a
pull request on a repository the bot route may push to. Of the repositories enrolled in the gates
policy none is a scratch repository, and a push to this repository before the proof is against
the operator's rule. Options: a new scratch repository under the organisation (the operator's
call: a new repository), the trial on this repository itself, or no live trial. Cleared when the
operator names the repository or declines; the proof then is one run to the gate, `takt approve`,
and one resumed run.

## Answer (2026-10-18)

The operator chose a new scratch repository: `beyond10x/todo-app-example`
(https://github.com/beyond10x/todo-app-example), public, created through the bot API route.

- Seed commit `865eaa8`: a todo list in one Python file (`todo.py`, standard library only), five
  unit tests run by `check.sh`, `.engineering/gates.yaml` running `check.sh` through
  `.engineering/assertions-project.yaml`, and `.github/workflows/check.yml` running `check.sh` on
  every pull request and on main. The workflow's run on the seed commit passed.
- The repository's planning store (`aep.project/5`) holds one ready story,
  `story:remove-a-todo`, `proposed`, scope `todo.py` and `tests/test_todo.py`.
- The bot route pushed `main` without the repository being enrolled in the common gates policy.

The trial for story:story-done-end-to-end: `takt run --repo <checkout> --test-cmd ./check.sh`,
`takt approve <case> repository.merge --by <approver>`, then `takt run` again.
