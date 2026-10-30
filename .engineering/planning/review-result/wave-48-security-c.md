---
format: aep.planning-md/3
id: review-result:wave-48-security-c
kind: review-result
status: active
title: Security review, wave 48 unit c
relations:
- reviews: story:git-through-the-connectors-git-relay
revision: 1
---
needs-revision, resolved

Security review of unit c: I2-I4 hold; I1 did not. Fixed on rv/48-sec-c with forge/relay.rs: relay git lines read no global or system config and get an empty HOME of their own, and a repository config that sets a credential helper, a URL-scoped extraHeader, proxy or followRedirects, a cookie file, a curl resolve override or a userinfo rewrite refuses the line. 5 of 5 green; merged at 3ae1a408.

```findings
[
{"file":"crates/takt-cli/tests/w48_sec_c.rs","line":432,"category":"integrity","severity":"warning","verdict":"CONFIRMED","origin":"undecided","message":"a credential helper in the operator's global git config sends a Basic token to the loopback relay on both paths"},
{"file":"crates/takt-cli/tests/w48_sec_c.rs","line":459,"category":"integrity","severity":"warning","verdict":"CONFIRMED","origin":"undecided","message":"a ~/.netrc entry for 127.0.0.1 sends a Basic token to the relay, because relay git children keep HOME"},
{"file":"crates/takt-cli/tests/w48_sec_c.rs","line":489,"category":"integrity","severity":"warning","verdict":"CONFIRMED","origin":"undecided","message":"http.extraHeader from a repo-included config file reaches the relay's wire on both paths"},
{"file":"crates/takt-cli/tests/w48_sec_c.rs","line":541,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"undecided","message":"a URL-scoped http.<relay>.followRedirects outranks -c http.followRedirects=false, so a relay redirect carries the push and creates the branch at another endpoint"}
]
```
