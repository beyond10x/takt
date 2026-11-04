---
format: aep.planning-md/3
id: review-result:w51-sec-a-1
kind: review-result
status: active
title: Wave 51 unit a, security review pass 1
summary: 'one introduced finding: a credential-shaped context file reaches the session'
relations:
- reviews: story:harness-section-is-metaharness-session-request
revision: 1
---
# Security review, wave 51 unit a, pass 1

unit: story:harness-section-is-metaharness-session-request at 8bbfe3567 (tree takt-w51-a)
verdict: red
cases: executed 0→6, red 2
origin: introduced 1, pre-existing 0, undecided 0
wrote-outside-worktree: 1 path (the reviewer's run log in the wave's scratch directory)
needs-coordinator: no

## Cases added

All in `crates/takt-cli/tests/w51_a_security.rs`:

| case | asserts | now |
|---|---|---|
| `every_reserved_session_key_is_refused_before_any_case_opens` | Each of metaharness 0.23.0's 35 document keys outside the six the mapping admits (credentials, cwd, scratch-cwd, hermetic, decisions, model-endpoint, api-key-secret, subscription-token-*, retain-dir, prompt, …) is refused under `claude`, `codex` and `b10x`. The run exits 1, opens no case, starts no harness, names the key and never prints the value. | green |
| `a_context_path_through_a_tracked_link_is_refused` | A tracked file link to a file outside the repository, and paths through a tracked directory link, are refused from the mapping and from `--context`, naming the path, before any case opens. | green |
| `an_untracked_or_git_internal_context_path_is_refused` | An untracked file, `.git/config` and `README.md/` are refused from the mapping and from `--context`. | green |
| `a_context_file_of_a_credentials_shape_from_the_mapping_reaches_no_session` | A tracked file holding a token-shaped value, named in `harness: {kind: claude, context: [...]}`, does not reach the harness's argv, and the run is refused before any case opens. | **red** |
| `a_context_file_of_a_credentials_shape_from_the_flag_reaches_no_session` | The same file named with `--context` gives the same result. | **red** |
| `no_case_record_holds_a_credential_shaped_context_files_text` | No file under `.takt/cases` holds that token. | green |

## Run

`cargo test -p b10x-takt-cli --test w51_a_security` (warm target, 3 s wall), exit 101:

```
thread 'a_context_file_of_a_credentials_shape_from_the_flag_reaches_no_session' panicked at crates/takt-cli/tests/w51_a_security.rs:348:5:
the credential-shaped context file's text was handed to the session's argv
thread 'a_context_file_of_a_credentials_shape_from_the_mapping_reaches_no_session' panicked at crates/takt-cli/tests/w51_a_security.rs:327:5:
the credential-shaped context file's text was handed to the session's argv
test result: FAILED. 4 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s
```

## Finding

`crates/takt/src/run_config.rs:628-648`: NEEDS-CHANGE, introduced.

- What was measured: the context check covers only path spelling, the index and links. A tracked file whose text has
  a credential's shape passes it. Its text then goes into the session as a `<context-file-…>` block in the Claude Code
  argv, so it reaches the vendor and is readable in the process's `/proc/<pid>/cmdline`. The two red cases above show
  it, through the mapping (test file line 327) and through `--context` (line 348).
- What reaches it: the documented workflow (`configuration.md`: "`context` lists files of the repository its index
  tracks"). The operator names a committed file that holds a token, such as a fixture or a `.env`.
- Why it counts: takt's other two ways of handing files to a session already guard against this. Instruction files of
  a credential's shape are refused (`harness.rs:2315`), and scope context files of that shape are left out
  (`harness.rs` around line 1875). The new `context` key has neither guard.
- Origin: at d661e73b1, `context` was refused outright (`HARNESS_KEYS_NOT_CARRIED`). This diff opened the path.
- Correction (not applied): in that check, read each named file and refuse the run when `crate::run::credential_shape`
  matches. Name the path and the shape, never the text, as `instruction_files` does.

## Checked and found nothing wrong

- Loom's reserved members cannot be reached. `SessionOverrides` has fields only for effort, max-budget-micro-usd,
  strict-version and context (loom 0.52.0 `loom-session-executor/src/lib.rs:355-371`), and these map only onto the
  matching `RunSpec` fields (`:1127-1133`). The mapping accepts only the six admitted keys, and the first case above
  confirms the rest are refused.
- No new field reaches the harness's environment.
- `effort` is passed to Claude Code as its own argv element. In the Codex config it is TOML-escaped (metaharness-codex
  `launch.rs:1432-1452`). The story's acceptance item 4 leaves its vocabulary unchecked on purpose.
- `max-budget-usd` refuses a sign, an exponent, more than six decimal places and overflow (`checked_mul`).
- No case record holds the token: it goes only to the session itself.
- Not proven: a context file that an earlier step of the same case deletes would be refused by Loom's own check at the
  next step, after the case has opened. Not tested.

```findings
[
  {
    "file": "crates/takt/src/run_config.rs",
    "line": 628,
    "category": "integrity",
    "severity": "warning",
    "verdict": "NEEDS-CHANGE",
    "origin": "introduced",
    "message": "a tracked context file holding text of a credential's shape is handed to the session's argv instead of being refused before any case opens, unlike instruction and scope files (a_context_file_of_a_credentials_shape_from_the_mapping_reaches_no_session, a_context_file_of_a_credentials_shape_from_the_flag_reaches_no_session)"
  }
]
```
