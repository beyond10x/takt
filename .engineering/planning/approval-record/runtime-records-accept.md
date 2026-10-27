---
format: aep.planning-md/3
id: approval-record:runtime-records-accept
kind: approval-record
status: accepted
title: Accepts the five runtime-records stories for integration/takt
tags:
- wave-approval
relations:
- decides: story:cases-live-under-dot-takt
- decides: story:the-state-directory-holds-every-host-record
- decides: story:a-run-keeps-every-decision-only-on-a-trial
- decides: story:python-modules-are-kept-only-when-a-python-answers
- decides: story:the-pull-request-and-the-store-carry-the-evidence
revision: 2
transitions:
- {from: "draft", to: "accepted", at: "2026-10-27T01:44:46Z", actor: "human:timo", revision: 2}
---
Decider: operator

Accepts the five runtime-records stories for implementation on integration/takt:
story:cases-live-under-dot-takt, story:the-state-directory-holds-every-host-record,
story:a-run-keeps-every-decision-only-on-a-trial, story:python-modules-are-kept-only-when-a-python-answers and
story:the-pull-request-and-the-store-carry-the-evidence.

Quoted operator words, 2026-10-27, relayed by the orchestrator:

- On a trial's case files: "we need some more control over where this is actually stored. So currently it's stored
  under .engineering/cases. It should probably be under .takt. So all the runtime data should be probably there, and
  most likely by default we do not want this to be going into version control unless we need it. But this is
  difficult for me to judge whether we need this stuff in version control or not. So this is for you to verify
  first. Then in general, maybe what we do want, I could imagine some of that store, some of the things is being
  created through Loom and Canon. So that probably needs some base tier configuration or something like that, where
  we tell it where to store the stuff."
- "the state files are probably that verbose because we need it on that verbosity level because of the evals and the
  trials. So maybe we need just some better defaults and then only enable this high verbosity during the trials."
- "why do I now there also have a file called Python modules? I'm not using Python and I don't want to."
- 01:39Z: "re-read my last reviews/feedback - tell, reconcile, and make sure its tracked in the takt planning store,
  and efficiently implemented and integrated"

Verification asked for first: no reader of a case reads it from version control (each story's Context cites the
readers), so the stories keep cases out of it by default and move what a reviewer needs into the store record and the
pull request.

Scope of the acceptance: the acceptance items of the five stories, built in waves of at most four units, merged into
integration/takt. Not merged to the base branch; no pull request; no release. Loom's half of the base-tier key
(`loom-agent serve` reading `runtime.state_dir`) is Loom's and is not accepted here.
