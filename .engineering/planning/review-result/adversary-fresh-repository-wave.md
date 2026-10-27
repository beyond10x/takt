---
format: aep.planning-md/3
id: review-result:adversary-fresh-repository-wave
kind: review-result
status: active
title: Adversary pass on the fresh-repository wave U1-U4 on integration/takt
summary: 'needs-change: 4 red cases, 6 findings; 4 fixed, 2 escalated to stories'
relations:
- reviews: story:untracked-cargo-lock-remedy
- reviews: story:gate-commands-find-programs-on-a-system-path
- reviews: story:a-gate-script-with-a-colon-is-run
- reviews: story:harness-none-takes-a-granted-action
revision: 1
---
## Subject

The fresh-repository wave on integration/takt at 0e9690c: U1 story:untracked-cargo-lock-remedy
(merge bbd264b), U2 story:gate-commands-find-programs-on-a-system-path (263a747), U3
story:a-gate-script-with-a-colon-is-run (7c90bfc), U4 story:harness-none-takes-a-granted-action
(195e71a). One combined adversary pass; each finding names its unit.

## Verdict

NEEDS-CHANGE: 4 red cases, 6 findings (5 introduced, 1 pre-existing). Cases executed 1968 before,
1972 after.

## Routing

- 1 (U2, blocker): an ignored tree file kept a bare program name bare and ran from the tree. Fixed:
  a name stays bare only when it is a file the tree tracks (`tracked_at_root`).
- 2 (U2): the PATH lookup took a non-executable file. Fixed: `found_on` takes the first file with
  an execute bit.
- 3 (U1): `ls-files` read the path as a pathspec. Fixed: `--literal-pathspecs`.
- 4 (U4): the outcome does not hold under `--forge none`: the harness takes the granted merge and
  nothing performs it, NoProgress exit 3. Escalated to story:forge-none-merges-locally; the case is
  kept, pinned to today's stop under that story's id.
- 5 (U4): the acceptance case never reaches the new branch. Fixed: the kept adversary case reaches
  it and asserts its line.
- 6 (U2, U4, pre-existing): `ess/ess-inputs.yaml` lists no scenario. Escalated to
  story:story-acceptance-scenarios-run-in-task-check.

Not findings: active_time_budget and self_improvement_cli fail only under parallel load (timing,
ETXTBSY) and pass alone.

```findings
[
{"file":"crates/takt/src/validator.rs","line":860,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"U2: a bare gate program name matching an ignored tree file is left bare and run from the tree, outside the base comparison, so a planted ./sh makes `sh -c 'exit 1'` pass; red: an_ignored_tree_file_does_not_shadow_a_bare_program_name"},
{"file":"crates/takt/src/validator.rs","line":756,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"U2: found_on takes the first file of the name on the PATH without an exec bit check, so a non-executable file shadows the program a PATH lookup would run; red: a_bare_name_is_found_as_a_path_lookup_finds_it_skipping_a_non_executable_file"},
{"file":"crates/takt/src/run.rs","line":8027,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"U1: untracked_cargo_lock passes the path to git ls-files as a pathspec, so an untracked :/Cargo.lock is read as the tracked root lock and the remedy is dropped; red: an_untracked_cargo_lock_under_a_colon_directory_gets_the_remedy"},
{"file":"crates/takt-cli/src/main.rs","line":966,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"U4: the story's outcome does not hold: the new branch is reached only under --forge none, where it ends NoProgress exit 3 with nothing merged; with a forge the merge validator takes the merge and the harness is never asked"},
{"file":"crates/takt-cli/tests/harness_none_after_a_grant.rs","line":290,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"U4: the acceptance case harness_none_takes_the_granted_merge never reaches NoHarness's granted branch and passes without the change"},
{"file":"ess/ess-inputs.yaml","line":6,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"U2, U4: the acceptance requires conformance scenarios named in ess/ess-inputs.yaml and run by task check; the file holds scenarios: [] and names none of them"}
]
```
