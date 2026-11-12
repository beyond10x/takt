---
format: aep.planning-md/3
id: review-result:w57-adv-1
kind: review-result
status: active
title: 'Wave 57 adversarial pass at 2c1cb190: seven findings, all fixed in a68eb760'
tags:
- scheduled-runs
relations:
- reviews: story:a-run-on-a-schedule
- reviews: story:one-served-connectors-client
revision: 1
---
# Wave 57 adversarial pass at 2c1cb190: seven findings, all fixed in a68eb760

```findings
[
{"file":"crates/takt/src/schedule.rs","line":606,"category":"concurrency","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"two simultaneous starts of one schedule both pass running() and tick the same slot, sharing its tick directory (w57_adv_simultaneous_starts_run_one_schedule); fixed in a68eb760 by an exclusive lock held for the life of the resident"},
{"file":"crates/takt-cli/src/main.rs","line":884,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"--help promises the outcome file on every exit but clap's get_matches exits without writing it (w57_adv_outcome_file_on_a_refused_flag); fixed in a68eb760 with try_get_matches"},
{"file":"crates/takt/src/schedule.rs","line":491,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"an unbounded every overflows the window loop and above i64::MAX ends a plain schedule with no-slot before its first tick; fixed in a68eb760: every is 1s to 7d and the arithmetic is checked"},
{"file":"crates/takt/src/schedule.rs","line":610,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"any live pid, another user's included, blocks a restart after the resident was killed and its pid reused; fixed in a68eb760 by the lock"},
{"file":"crates/takt-cli/src/main.rs","line":1907,"category":"concurrency","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a signal between the slot wait's check and the spawn still starts one tick; fixed in a68eb760 by a second read before spawn"},
{"file":"crates/takt/src/schedule.rs","line":647,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"schedule.json, end.json and the outcome file are written in place, not through a temporary file and rename; fixed in a68eb760"},
{"file":"crates/takt/src/schedule.rs","line":968,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the test named for the deadline asserts only the weekly repeat; renamed and a deadline test added in a68eb760"}
]
```
