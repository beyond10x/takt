//! story:credential-shape-sees-a-token-after-a-word-character, adversarial pass: what reading
//! every shape at every byte costs and finds. Every prefix is assembled from pieces, so this file
//! holds no credential's shape.

use std::time::{Duration, Instant};

use takt::run::credential_shape;
use takt::validator::{FAILED_LINES_PER_CHECK, failing_lines};

/// The text these cases read: 16 KiB, a quarter of what a harness's declared files may take
/// together (`harness::SCOPE_BYTES`, 64 KiB), each file of them read whole by `credential_shape`
/// (`harness.rs`, the declared context), as is a proposed new file's content, unbounded
/// (`run.rs`, `proposal_scan_refusal`).
const TEXT: usize = 16 << 10;

/// How long one read of [`TEXT`] bytes may take. A linear read of 16 KiB takes a few
/// milliseconds unoptimised; this bound is a hundred times that.
const BOUND: Duration = Duration::from_millis(500);

fn timed(text: &str) -> (Option<&'static str>, Duration) {
    let start = Instant::now();
    let shape = credential_shape(text);
    (shape, start.elapsed())
}

/// A word run that repeats a JWT's first three characters (`ey` `J`) and never holds the dots a
/// JWT needs: before the change only the run's first byte was read; now every one of its
/// positions measures the rest of the run, so the read grows with the square of the run's length.
#[test]
fn a_run_of_jwt_openings_is_read_in_linear_time() {
    let opening = ["ey", "J"].concat();
    let text = opening.repeat(TEXT / opening.len());
    let (shape, took) = timed(&text);
    assert_eq!(shape, None);
    assert!(
        took < BOUND,
        "{} bytes of repeated JWT openings took {took:?}",
        text.len()
    );
}

/// A word run of kebab words ending in `sk` (`a` `sk-`), all lower case: every `sk-` measures the
/// rest of the run and then looks through it for an upper-case letter that is not there.
#[test]
fn a_run_of_words_ending_in_sk_is_read_in_linear_time() {
    let word = ["a", "sk", "-"].concat();
    let text = word.repeat(TEXT / word.len());
    let (shape, took) = timed(&text);
    assert_eq!(shape, None);
    assert!(
        took < BOUND,
        "{} bytes of repeated words ending in sk took {took:?}",
        text.len()
    );
}

/// A bare `sk-` and 32 lower-case hexadecimal characters (the form several OpenAI-compatible
/// providers issue) is the shape `openai-key` at the start of the text and after `_` or `-`.
/// Right after a letter or a digit it is not found, for it holds no upper-case letter: a decision
/// of the wave's coordinator, the price of words ending in `sk-` (`risk-…`) staying shown, and
/// already so before the story, when nothing after a letter was found.
#[test]
fn a_hexadecimal_key_is_found_unless_after_a_letter() {
    let hex = "0123456789abcdef".repeat(2);
    let key = ["sk", "-", &hex].concat();
    assert_eq!(credential_shape(&key), Some("openai-key"), "alone");
    assert_eq!(
        credential_shape(&["test_", &key].concat()),
        Some("openai-key"),
        "after _"
    );
    assert_eq!(
        credential_shape(&["x", &key].concat()),
        None,
        "after a letter, not found by decision"
    );
}

/// The rule after a letter is an upper-case letter *and* a digit: text after `sk-` with
/// upper-case letters and no digit is shown. `a_word_ending_in_sk_is_no_key` holds no
/// upper-case letter, so it stays green when the digit's half of the rule is dropped.
#[test]
fn a_word_ending_in_sk_with_capitals_and_no_digit_is_no_key() {
    let text = "the-risk-Assessment-of-the-Quarterly-Report-for-the-Board";
    assert_eq!(credential_shape(text), None, "{text}");
}

/// A key printed in colour: the SGR sequence ends in `m` right before `sk-`, a letter. Taken out
/// before the check, the key's `sk-` follows a space, and a key of lower-case letters is withheld.
/// `failed_gate_lines_adv1::a_token_printed_in_colour_is_withheld` uses a GitHub token, which is
/// now found after `m` whether the sequence is taken out or not, so it no longer guards that.
#[test]
fn a_lower_case_key_printed_in_colour_is_withheld() {
    let key = ["sk", "-", &"q".repeat(40)].concat();
    let printed = format!(
        "error: a secret in config/ci.env:3\n  found: \u{1b}[31m{key}\u{1b}[0m\n1 secret found\n"
    );
    let (lines, _) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert!(
        !lines.iter().any(|line| line.contains(&key)),
        "a credential's shape is kept: {} lines",
        lines.len()
    );
}

/// The process's peak resident memory, in bytes (`VmHWM` in `/proc/self/status`).
#[cfg(target_os = "linux")]
fn peak() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    let line = status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .expect("a VmHWM line");
    let kib: u64 = line
        .split_whitespace()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .expect("VmHWM in kB");
    kib << 10
}

/// A proposal's new file is read whole (`run.rs`, `proposed_addition`), and the session executor
/// puts no bound on its size: whatever the session wrote into its copy. Reading it for a
/// credential's shape costs a few times its size at most, not a table of a `usize` per byte for
/// each of six classes of byte.
#[cfg(target_os = "linux")]
#[test]
fn reading_a_large_text_takes_memory_in_proportion() {
    const SIZE: usize = 4 << 20;
    const AT_MOST: u64 = 8 * SIZE as u64;
    let text = "a".repeat(SIZE);
    let before = peak();
    assert_eq!(credential_shape(&text), None);
    let grew = peak().saturating_sub(before);
    assert!(
        grew <= AT_MOST,
        "reading {SIZE} bytes raised the peak by {grew} bytes, past {AT_MOST}"
    );
}

/// A token field's gap is at most four bytes; the gap read stops after five, so a sixth byte
/// of the gap's characters (`=` is also a value's character) is never taken as the value's
/// first. Five `=` between the name and the value is no assignment, as before the cap.
#[test]
fn a_token_field_with_a_gap_of_five_is_no_assignment() {
    let text = ["access", "_token", "=====", "abc123def456ghi789jkl"].concat();
    assert_eq!(credential_shape(&text), None);
}
