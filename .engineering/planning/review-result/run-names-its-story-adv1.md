---
format: aep.planning-md/3
id: review-result:run-names-its-story-adv1
kind: review-result
status: active
title: Adversarial review, story run-names-its-story, after its commit
tags:
- loom-agent
relations:
- reviews: story:run-names-its-story
revision: 1
---
needs-fix
Adversarial review of story:run-names-its-story on wave/37 after its commit e9ae5449: `takt run --story`, `Named`, `story_not_taken`, `case_to_resume`. Four cases written (`crates/takt-cli/tests/run_story_adv1.rs`), one red, green after the fix round. Attacked without a break: id forms (prefixed, bare, empty, unicode, another kind), nothing written on every refusal from the base, two open cases both resumable, a `story` key in `--config` refused, budgets of one case, a finished case refused as a plain run refuses it.

```findings
[
{"file":"crates/takt-cli/src/main.rs","line":1405,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a refused takt run --story from a tree on a case's branch had already checked out the case's base inside take_next_as and left the tree there, though exit 1 means refused before the run"}
]
```
