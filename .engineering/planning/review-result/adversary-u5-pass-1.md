---
format: aep.planning-md/3
id: review-result:adversary-u5-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 on U5: run configuration keys'
relations:
- reviews: story:run-configuration-names-the-story-and-token-variable
- reviews: story:run-state-directory-is-configurable
- reviews: story:scenario-run-takes-config
revision: 1
---
# Adversary pass 1 on U5 (run configuration keys, --state-dir, scenario run --config)

Tree: `wave/38` at 417f4b33 plus the uncommitted unit edits. Test file added:
`crates/takt-cli/tests/endpoint_token_env_cli_adv1.rs`: 1 passed, 1 failed
(`a_command_line_endpoint_files_credential_wins_over_the_configurations_variable`).
run_state_dir_cli 4 passed; run_story_cli 10 passed; scenario_run_config_cli 5 passed.

| file:line | verdict | origin | finding |
|---|---|---|---|
| crates/takt-cli/src/main.rs:720 | warning | introduced | the file's `endpoint_token_env` overrides the credential of an `--endpoint-file` given on the command line |
| README.md:135 | note | introduced | `state_dir` documented as replacing `$XDG_STATE_HOME/takt`; only transcripts move |
| crates/takt-cli/src/main.rs:2159 | note | introduced | an explicitly named state directory that cannot be used only warns; the run keeps no transcripts |
| crates/takt-cli/src/main.rs:678 | note | introduced | the file's `story` applies to a `self.improvement/1` turn, and refusals name a `--story` flag nobody gave |

```findings
- file: crates/takt-cli/src/main.rs
  line: 720
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the configuration file's endpoint_token_env overrides the credential of an --endpoint-file given on the command line, so the file beats the flag
- file: README.md
  line: 135
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: state_dir is documented as replacing $XDG_STATE_HOME/takt, but only transcripts move; set-aside, judges and replays stay under the state home
- file: crates/takt-cli/src/main.rs
  line: 2159
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: an explicitly named state directory that cannot be used only warns on stderr and the run keeps no transcripts, the loss the story exists to prevent
- file: crates/takt-cli/src/main.rs
  line: 678
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the file's story is applied to a self.improvement/1 turn and to refusals that name a --story flag the operator never gave
```
