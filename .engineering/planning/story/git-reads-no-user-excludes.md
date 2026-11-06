---
format: aep.planning-md/3
id: story:git-reads-no-user-excludes
kind: story
status: draft
title: takt's git calls read no user excludes file
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-ae680d5-adversarial
- informed_by: decision-blocker:excluded-files-and-user-excludes
revision: 3
---
## Outcome

Every git call takt makes runs with `core.excludesFile` set to an empty value (and `$XDG_CONFIG_HOME/git/ignore` not read), so a file the user's or a test command's global excludes would hide is seen as untracked by takt's status, its `uncommitted` precondition and its step guard.

## Context

From the adversarial review of main at bd20f7b on 2026-10-19: ten adversarial reviewers, each finding fact-checked against the code by a second reviewer (review-result:main-ae680d5-adversarial), finding A5.3, partly confirmed (medium). `NO_CONFIGURED_CODE` (`crates/takt/src/git.rs:26-44@bd20f7b`) does not override `core.excludesFile`, `HOME` is passed through, and `Outside` omits git config. Replay: a global ignore entry `pkg/mod/` hides a shadow package (`git status -uall` prints nothing, import gives the shadow). From a harness step the hidden path appears as an ignored addition and is refused; from a test command the excludes file and the shadow survive, and the next `tests.run` passes the `uncommitted` check (`run.rs:3463`) while importing the shadow (inferred end to end).

## Scope

- `crates/takt/src/git.rs`: `command`, `NO_CONFIGURED_CODE` (cited)

## Acceptance

With `$HOME/.config/git/ignore` listing `pkg/mod/` and an untracked `pkg/mod/__init__.py`, takt's `uncommitted` reports `pkg/mod/__init__.py`.

## Decision

decision-blocker:excluded-files-and-user-excludes chose option 3: this story holds as written, and the repository's own
`.git/info/exclude` is not trusted by the step guard either. A deployment's own files beside the tree are named in the
run file instead (story:git-excluded-files-are-not-step-changes).

Added acceptance: with `.git/info/exclude` listing `pkg/mod/` and an untracked `pkg/mod/__init__.py`, the step guard
still counts `pkg/mod/__init__.py` as a change the step made.

## Wave 56 security review

The wave 56 security review (on `wave/56` at 9a67088ee, after `deployment_paths` landed) confirmed the finding again:
with `.git/info/exclude` listing `scratch.tmp` and a global excludes file listing `pkg/mod/`, `git::uncommitted` reports
neither file, so the `uncommitted` preconditions (case open, `tests.run`) do not see them, while the step guard still
counts them (`crates/takt/tests/deployment_paths.rs`). The reviewer's test, `uncommitted_reports_a_file_only_an_excludes_file_hides`,
left the wave because this story is still a draft; it is the acceptance test to start from.

The test, verbatim (it uses the helpers `repository`, `append` and `git` of `crates/takt/tests/work_and_deployment_w56_sec.rs`):

```rust
/// Invariant 5 (story:git-reads-no-user-excludes, acceptance): takt's git reads no excludes file,
/// so a file `.git/info/exclude` or a `core.excludesFile` lists is untracked to takt, and
/// `uncommitted` reports it.
#[test]
fn uncommitted_reports_a_file_only_an_excludes_file_hides() {
    let scratch = repository("excludes");
    let repo = &scratch.repo;
    append(&repo.join(".git/info/exclude"), "pkg/mod/\n");
    append(&repo.join("pkg/mod/__init__.py"), "shadow = True\n");
    let excludes = scratch.root.join("global-ignore");
    fs::write(&excludes, "scratch.tmp\n").expect("the excludes file");
    git(
        repo,
        &[
            "config",
            "core.excludesFile",
            &excludes.display().to_string(),
        ],
    );
    fs::write(repo.join("scratch.tmp"), "hidden\n").expect("a file");
    let uncommitted = takt::git::uncommitted(repo).expect("uncommitted");
    let missing: Vec<&str> = ["pkg/mod/__init__.py", "scratch.tmp"]
        .into_iter()
        .filter(|path| !uncommitted.iter().any(|named| named == path))
        .collect();
    assert!(
        missing.is_empty(),
        "takt's git still reads the excludes files: uncommitted is {uncommitted:?}, missing {missing:?}"
    );
}
```
