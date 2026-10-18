//! Adversary pass 2 on story:ci-pending-waits: `forge::refused_for_a_pending_check` after
//! correction 1, driven with the words `b10x-gates api` prints for a refused write (`b10x-gates:
//! GitHub operation failed (<status>): <GitHub's message>`, `ApiFailure::render` in the gates
//! source), as the bot route passes them on in a `forge::Failed`.

use std::fs;
use std::path::{Path, PathBuf};

use takt::forge::{BotRoute, Exit, Failed, Forge, Runner, refused_for_a_pending_check};

/// What `b10x-gates api` prints on stderr before GitHub's message for a 405.
const GATES_405: &str = "b10x-gates: GitHub operation failed (405 Method Not Allowed): ";

/// The merge's error as the bot route makes it after correction 1: a `Failed` whose stderr is
/// `words`, with the context `PrMerged` adds when it gives up.
fn failed_merge(words: &str) -> anyhow::Error {
    anyhow::Error::from(Failed {
        line: "b10x-gates api --method PUT --path /repos/o/r/pulls/1/merge".to_owned(),
        code: Some(1),
        stderr: words.to_owned(),
    })
    .context("merge pull request #1 through the forge bot")
}

/// Every line succeeds but the merge's API write, which exits 1 with `stderr`.
struct RefusingMerge {
    stderr: String,
}

impl Runner for RefusingMerge {
    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        let merge = argv.first().map(String::as_str) == Some("b10x-gates")
            && argv.iter().any(|arg| arg == "PUT");
        Ok(Exit {
            code: Some(i32::from(merge)),
            stdout: String::new(),
            stderr: if merge {
                format!("{}\n", self.stderr)
            } else {
                String::new()
            },
        })
    }
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The error `BotRoute::merge` returns for a merge the runner refuses with `stderr`.
fn bot_route_refusal(root: &Path, stderr: &str) -> anyhow::Error {
    let route = BotRoute::new(
        "o/app",
        None,
        Box::new(RefusingMerge {
            stderr: stderr.to_owned(),
        }),
    );
    let artifacts = root.join("artifacts");
    route
        .merge(root, &artifacts, 1, &"a".repeat(40))
        .expect_err("the forge refuses the merge")
}

/// The contract with the real adapter: `b10x-gates` 0.1.21 prints GitHub's message after its
/// own prefix. A required check that has not reported is waited on; one that is failing, and a
/// refusal whose message `b10x-gates` withheld, stay errors. Green: pins the wording the
/// classifier is read against.
#[test]
fn adv2_the_words_b10x_gates_prints_for_a_405_are_classified_as_the_forge_meant_them() {
    let root = Scratch(Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "ci-pending-waits-adv2-{}-gates-words",
        std::process::id()
    )));
    fs::create_dir_all(&root.0).expect("the scratch tree");
    let pending = format!("{GATES_405}Required status check \"takt / task check\" is expected.");
    let error = bot_route_refusal(&root.0, &pending);
    assert!(
        error.downcast_ref::<Failed>().is_some(),
        "the bot route's merge failure is a Failed: {error:#}"
    );
    assert!(
        refused_for_a_pending_check(&error),
        "a pending required check in the words b10x-gates prints is not waited on: {error:#}"
    );
    for refused in [
        format!("{GATES_405}Required status check \"takt / task check\" is failing."),
        "b10x-gates: GitHub operation failed (405 Method Not Allowed); response withheld"
            .to_owned(),
        "b10x-gates: GitHub operation failed (405 Method Not Allowed); message withheld: trusted policy unavailable"
            .to_owned(),
    ] {
        let error = bot_route_refusal(&root.0, &refused);
        assert!(
            !refused_for_a_pending_check(&error),
            "waited on, not an error: {error:#}"
        );
    }
}

/// The quoted check name is left out, so a pending check whose name holds a failing phrase is
/// still waited on. Green: kills the mutant that drops the quote stripping in
/// `pending_check_refusal` (the name's `has failed` would then read as the status).
#[test]
fn adv2_a_pending_check_whose_name_holds_a_failing_phrase_is_waited_on() {
    let words = format!("{GATES_405}Required status check \"e2e has failed before\" is expected.");
    assert!(
        refused_for_a_pending_check(&failed_merge(&words)),
        "not waited on: {words}"
    );
}

/// Mixed pending and failing checks are an error, not a wait. GitHub does not escape a quote in
/// a check's name, and `b10x-gates` joins GitHub's message and its `errors` with `; `. A
/// failing check named `lint"` (a name ending in a quote) leaves the quote toggle open, which
/// swallows its own `is failing`; the pending check before it is read, and the refusal is
/// waited on although a required check failed.
#[test]
fn adv2_a_failing_check_whose_name_ends_in_a_quote_does_not_turn_a_mixed_refusal_into_a_wait() {
    let words = format!(
        "{GATES_405}Required status check \"build\" is expected.; Required status check \"lint\"\" is failing."
    );
    assert!(
        !refused_for_a_pending_check(&failed_merge(&words)),
        "a refusal naming a failing required check is waited on, not an error: {words}"
    );
}

/// "Every other refusal" stays an error. A ruleset refusal lists each rule the merge breaks
/// (`b10x-gates` drops the newlines between them): a required review missing besides a check
/// that has not reported. Waiting on the check cannot make this merge go through; the run waits
/// out its whole budget and stops `Waiting` instead of naming the review.
#[test]
fn adv2_a_refusal_that_also_requires_a_review_is_not_waited_on() {
    let words = format!(
        "{GATES_405}Repository rule violations foundAt least 1 approving review is required by reviewers with write access.Required status check \"takt / task check\" is expected."
    );
    assert!(
        !refused_for_a_pending_check(&failed_merge(&words)),
        "a refusal that needs a review as well is waited on: {words}"
    );
}
