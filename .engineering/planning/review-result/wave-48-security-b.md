---
format: aep.planning-md/3
id: review-result:wave-48-security-b
kind: review-result
status: active
title: Security review, wave 48 unit b
relations:
- reviews: story:connectors-forge-at-a-served-instance
revision: 1
---
needs-revision, resolved

Security review of unit b: I1 (at most once), I4 and I5 hold; 4 guards did not. Fixed on rv/48-sec-b: the refusal record is read and written without following links, read up to 64 KiB from a regular file, and a malformed record is an error; keying turns off only on a whole v1alpha2 error envelope with code unsupported. 4 of 4 green; merged at 2b09c607. The same link-following write remains in case.rs, forge/local.rs and promotion.rs, recorded as a draft story.

```findings
[
{"file":"crates/takt/src/forge/connectors.rs","line":253,"category":"integrity","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"the atomic record write follows a link planted at its pid-named partial file and overwrites a file outside the case"},
{"file":"crates/takt/src/forge/connectors.rs","line":209,"category":"integrity","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"the refusal record is read through a link with no size limit, so a FIFO or device blocks or exhausts the run"},
{"file":"crates/takt/src/forge/connectors.rs","line":223,"category":"contract-drift","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a record whose refused member is not an object reads as zero refusals despite the reader's doc comment"},
{"file":"crates/takt/src/forge/connectors.rs","line":539,"category":"integrity","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"any 400 body with a top-level code unsupported switches the forge to unkeyed writes without validating the v1alpha2 envelope"},
{"file":"crates/takt/src/forge/connectors.rs","line":1027,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a count raised by hand gives a fresh key the host dispatches again, and only provider-side checks stop a second open or merge"}
]
```
