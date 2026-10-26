---
format: aep.planning-md/3
id: review-result:run-configuration-file-adv1
kind: review-result
status: active
title: Adversarial review, story run-configuration-file, before its commit
relations:
- reviews: story:run-configuration-file
revision: 1
---
needs-fix
Adversarial review of story:run-configuration-file in the release tree before its commit (base 30f177d): `takt run --config`, `crates/takt/src/run_config.rs`, `RunArgs::configured`, the `Dockerfile` and README § Container. One red case written (`crates/takt-cli/tests/run_config_adv1.rs`), green after the fix.

```findings
[
{"file":"crates/takt-cli/src/main.rs","line":643,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a command-line --harness other than harness was refused because the file's endpoint_file stayed set; fixed, the file's endpoint_file applies only under the harness harness"},
{"file":".dockerignore","line":1,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the image's takt recorded unknown revision because .git and git were absent from the build; fixed for a build from a clone"},
{"file":"README.md","line":122,"category":"judgement","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"the test command's user and mount namespaces are refused under default container seccomp and AppArmor; the contract now requires both unconfined"},
{"file":"crates/takt-cli/src/main.rs","line":1396,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"refusals of file values name command-line flags and repeat the value written in the file; left, no credential is among those values"},
{"file":"crates/takt-cli/tests/run_config_cli.rs","line":1,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"removing the forge, hours or test_cmd assignment left the suite green; fixed by three cases"},
{"file":"README.md","line":129,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the image lacks claude, codex, b10x-gates and gh, so harness and forge must be set in run.toml; fixed in the contract"}
]
```
