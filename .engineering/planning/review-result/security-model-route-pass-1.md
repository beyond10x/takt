---
format: aep.planning-md/3
id: review-result:security-model-route-pass-1
kind: review-result
status: active
title: 'Security review of the model-route unit: APPROVE, one warning to fix at the wave end'
relations:
- decides: story:model-names-an-llm-route
- reviews: story:model-names-an-llm-route
revision: 1
---
Opus security review of the model-route unit (commit 97d5f111; integration head at the time a9a9f0e1).

verdict: APPROVE, no blocker. needs-coordinator: no.

Invariants (by reading; no runtime check): 1 literal secret refused, no value echoed: HOLDS. 2 scrub of every env variable holding the resolved value, staged token 0600 under 0700, no value in argv, child env, case file or record: HOLDS by reading. 3 only the target endpoint credential is read: HOLDS. 4 protocol and base-URL checks cannot be bypassed by alias or direct name: HOLDS. 5 no fallback to the environment or default endpoint: HOLDS. 6 ESS declarations match code: HOLDS with a note. 7 new dependencies add no spawn or network beyond the route: HOLDS.

Findings:
- warning, acceptance 4 untested: no case covers an env:VAR route, so nothing checks that the scrub removes the variable or that no child sees it.
- note: an empty secret (env:VAR set to empty) removes every empty variable before StagedToken refuses the run.
- note: StagedToken refusals say credential variable <endpoint name> for a route; the wording is misleading, no value exposed.
- note: RouteCredential and RouteSecretKind carry no read-from citations; EndpointCredential prose still lists three sources; RouteName is declared and read by no code.
