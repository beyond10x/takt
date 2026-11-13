//! Wave 58's adversarial pass on story:a-command-action: the reference's sentence that lists the
//! run-file fields with no flag of their name counts them, and the count must be the list's.

use std::path::Path;

/// `website/docs/reference/configuration.md` § Every field: "Every field but `a`, ..., `y` and
/// `z` is the `takt run` flag of the same name ...; those <n> have no flag of that name". The
/// unit added `command_actions` to the list and left the count.
#[test]
fn the_reference_counts_the_fields_without_a_flag_as_it_lists_them() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/docs/reference/configuration.md");
    let text = std::fs::read_to_string(&path).expect("the reference");
    let start = text.find("Every field but ").expect("the sentence");
    let rest = &text[start + "Every field but ".len()..];
    let end = rest
        .find(" is the `takt run` flag")
        .expect("the list's end");
    let listed = rest[..end].matches('`').count() / 2;
    let after = &rest[end..];
    let those = after.find("those ").expect("the count");
    let word = after[those + "those ".len()..]
        .split_whitespace()
        .next()
        .expect("a number word");
    let words = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve", "thirteen",
    ];
    let counted = words
        .iter()
        .position(|w| *w == word)
        .unwrap_or_else(|| panic!("`{word}` is no number word"));
    assert_eq!(
        counted, listed,
        "the reference says \"those {word}\" of a list of {listed} fields"
    );
}
