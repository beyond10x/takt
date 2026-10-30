//! Adversary pass 2 on story:ci-pending-waits: `forge::refused_for_a_pending_check` after
//! correction 1, driven with the words a forge adapter that reports text passes on in a
//! `forge::Failed` for a refused merge: the HTTP status, then the host's message.

use takt::forge::{Failed, refused_for_a_pending_check};

/// The status an adapter puts before the host's message for a 405.
const FORGE_405: &str = "405 ";

/// The merge's error as an adapter makes it after correction 1: a `Failed` whose stderr is
/// `words`, with the context `PrMerged` adds when it gives up.
fn failed_merge(words: &str) -> anyhow::Error {
    anyhow::Error::from(Failed {
        line: "forge merge 1".to_owned(),
        code: Some(1),
        stderr: words.to_owned(),
    })
    .context("merge pull request #1 through the forge")
}

/// The quoted check name is left out, so a pending check whose name holds a failing phrase is
/// still waited on. Green: kills the mutant that drops the quote stripping in
/// `pending_check_refusal` (the name's `has failed` would then read as the status).
#[test]
fn adv2_a_pending_check_whose_name_holds_a_failing_phrase_is_waited_on() {
    let words = format!("{FORGE_405}Required status check \"e2e has failed before\" is expected.");
    assert!(
        refused_for_a_pending_check(&failed_merge(&words)),
        "not waited on: {words}"
    );
}

/// Mixed pending and failing checks are an error, not a wait. GitHub does not escape a quote in
/// a check's name, and its message and its `errors` are joined with `; `. A failing check named
/// `lint"` (a name ending in a quote) leaves the quote toggle open, which swallows its own
/// `is failing`; the pending check before it is read, and the refusal is waited on although a
/// required check failed.
#[test]
fn adv2_a_failing_check_whose_name_ends_in_a_quote_does_not_turn_a_mixed_refusal_into_a_wait() {
    let words = format!(
        "{FORGE_405}Required status check \"build\" is expected.; Required status check \"lint\"\" is failing."
    );
    assert!(
        !refused_for_a_pending_check(&failed_merge(&words)),
        "a refusal naming a failing required check is waited on, not an error: {words}"
    );
}

/// "Every other refusal" stays an error. A ruleset refusal lists each rule the merge breaks
/// (the newlines between them dropped): a required review missing besides a check that has not
/// reported. Waiting on the check cannot make this merge go through; the run waits out its whole
/// budget and stops `Waiting` instead of naming the review.
#[test]
fn adv2_a_refusal_that_also_requires_a_review_is_not_waited_on() {
    let words = format!(
        "{FORGE_405}Repository rule violations foundAt least 1 approving review is required by reviewers with write access.Required status check \"takt / task check\" is expected."
    );
    assert!(
        !refused_for_a_pending_check(&failed_merge(&words)),
        "a refusal that needs a review as well is waited on: {words}"
    );
}
