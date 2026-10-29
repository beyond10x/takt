//! Wave 41 adversarial pass on `takt::console`'s header: the title and the summary are text a
//! person or an issue wrote (a story's `intent.md`, or `--intent`), and the header prints them to
//! the console. Every other piece of such text the run prints is escaped by
//! `takt::trace::escape` (the harness's `said:`, a validator's failed checks, a committed path);
//! these cases assert the header holds no control character either.

use takt::console;

/// Whether `line` holds a character a terminal acts on: a C0 or C1 control, or a bidi control.
fn has_control(line: &str) -> bool {
    line.chars()
        .any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
}

#[test]
fn w41_adv_the_header_title_carries_no_escape_byte() {
    let intent = "Fix the \x1b[2J\x1b[Hlist\n\nThe list loses the todo it names.\n";
    let lines = console::header("0.18.0", None, intent);
    assert!(
        !lines.iter().any(|line| has_control(line)),
        "a title's control characters reach the console: {lines:?}"
    );
}

#[test]
fn w41_adv_a_carriage_return_in_the_title_forges_no_stop_line() {
    let intent = "Fix it\rstopped: Completed; steps 0, tokens 0, case /nowhere\n";
    let lines = console::header("0.18.0", Some("story:x"), intent);
    assert!(
        !lines.iter().any(|line| line.contains('\r')),
        "a lone carriage return in the title reaches the console: {lines:?}"
    );
}

#[test]
fn w41_adv_the_summary_carries_no_escape_byte() {
    let intent = "Make the check pass\n\n# Story: x\n\n## Outcome\n\nThe \x1b[31mcheck\x1b[0m passes \
                  \u{7}for whoever runs it.\n";
    let lines = console::header("0.18.0", Some("story:x"), intent);
    assert!(
        !lines.iter().any(|line| has_control(line)),
        "an Outcome's control characters reach the console: {lines:?}"
    );
}

#[test]
fn w41_adv_a_bidi_override_in_the_title_is_escaped() {
    let intent = "Fix \u{202e}txt.exe\n";
    let lines = console::header("0.18.0", None, intent);
    assert!(
        !lines.iter().any(|line| has_control(line)),
        "a bidi override in the title reaches the console: {lines:?}"
    );
}

/// Green probe: `may now:` names exactly the symmetric difference, added in `now`'s order then
/// removed in `before`'s, over every pair of subsets of four actions.
#[test]
fn w41_adv_may_now_is_the_symmetric_difference() {
    let all = ["a.one", "b.two", "c.three", "d.four"];
    let subset = |mask: u32| -> Vec<String> {
        all.iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, a)| (*a).to_owned())
            .collect()
    };
    for b in 0..16u32 {
        for n in 0..16u32 {
            let before = subset(b);
            let now = subset(n);
            let got = console::may_now(&before, &now);
            if b == n {
                assert_eq!(got, None, "{before:?} {now:?}");
                continue;
            }
            let mut want: Vec<String> = now
                .iter()
                .filter(|a| !before.contains(a))
                .map(|a| format!("+{a}"))
                .collect();
            want.extend(
                before
                    .iter()
                    .filter(|a| !now.contains(a))
                    .map(|a| format!("-{a}")),
            );
            assert_eq!(
                got,
                Some(format!("  may now: {}", want.join(" "))),
                "{before:?} {now:?}"
            );
        }
    }
}
