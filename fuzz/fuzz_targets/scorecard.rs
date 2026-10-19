//! The scorecard's rule over any two scorecards of one scenario: each side's median of tokens
//! lies within its minimum and maximum, a scorecard is never worse than itself, and adding a
//! case no better than any of the candidate's never makes its verdict better.
//!
//! The input: a byte whose value modulo 3 is the margin, then one case each 9 bytes, the first
//! naming the side (its high bit: the candidate) and the outcome (its low bits), the next eight
//! the tokens, little-endian.

#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use takt::scorecard::{FORMAT, METRICS, Row, Rule, Scorecard, rank};

const OUTCOMES: [&str; 12] = [
    "accepted",
    "merged",
    "ApprovalRequired",
    "Waiting",
    "Iterations",
    "Hours",
    "Tokens",
    "NothingAdmissible",
    "StepRefused",
    "NoProgress",
    "error",
    "unfinished",
];

fn card(cases: &[(&str, u64)], margin: u32) -> Scorecard {
    Scorecard {
        format: FORMAT.to_owned(),
        revision: None,
        named_by: None,
        metrics: METRICS
            .iter()
            .map(|(metric, version)| ((*metric).to_owned(), *version))
            .collect(),
        rule: Rule { trials: 1, margin },
        rows: cases
            .iter()
            .enumerate()
            .map(|(n, (outcome, tokens))| Row {
                case: format!("scenario-s-trial-{n}"),
                scenario: Some("s".to_owned()),
                outcome: (*outcome).to_owned(),
                steps: 0,
                tokens: *tokens,
                refusals: 0,
                no_progress: 0,
                errors: 0,
                wall_ms: 0,
                saturated: false,
                binary: None,
                stop: (*outcome).to_owned(),
                held_out: None,
                opened_binary: None,
            })
            .collect(),
        no_record: Vec::new(),
        unstated: BTreeMap::new(),
    }
}

fn against(candidate: &Scorecard, saved: &Scorecard) -> (usize, String) {
    let mut out = Vec::new();
    let worse = candidate.against(saved, &mut out).expect("compare");
    (worse, String::from_utf8(out).expect("printed text"))
}

/// Twice a figure as printed: `12` is 24, `12.5` is 25.
fn twice(figure: &str) -> u128 {
    match figure.split_once('.') {
        Some((whole, "5")) => 2 * whole.parse::<u128>().expect("a figure") + 1,
        _ => 2 * figure.parse::<u128>().expect("a figure"),
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((first, rest)) = data.split_first() else {
        return;
    };
    let margin = u32::from(*first % 3);
    let (mut saved, mut candidate) = (Vec::new(), Vec::new());
    for chunk in rest.as_chunks::<9>().0.iter().take(16) {
        let outcome = OUTCOMES[usize::from(chunk[0] & 0x7f) % OUTCOMES.len()];
        let tokens = u64::from_le_bytes(chunk[1..9].try_into().expect("eight bytes"));
        if chunk[0] & 0x80 == 0 {
            saved.push((outcome, tokens));
        } else {
            candidate.push((outcome, tokens));
        }
    }
    let (saved, candidate) = (card(&saved, margin), card(&candidate, margin));
    let (worse, printed) = against(&candidate, &saved);
    for line in printed.lines().filter(|line| line.contains(" tokens min ")) {
        let words: Vec<&str> = line.split_whitespace().collect();
        let after = |word: &str| {
            let at = words.iter().position(|w| *w == word).expect(word);
            twice(words[at + 1])
        };
        let (min, median, max) = (after("min"), after("median"), after("max"));
        assert!(min <= median && median <= max, "{line}");
    }
    assert_eq!(against(&candidate, &candidate).0, 0);
    let cases: Vec<(&str, u64)> = candidate
        .rows
        .iter()
        .map(|row| {
            let outcome = OUTCOMES
                .iter()
                .find(|outcome| **outcome == row.outcome)
                .expect("an outcome of the list");
            (*outcome, row.tokens)
        })
        .collect();
    if let Some(lowest) = cases.iter().map(|(outcome, _)| rank(outcome)).min() {
        let most = cases.iter().map(|(_, tokens)| *tokens).max().unwrap_or(0);
        let outcome = OUTCOMES
            .iter()
            .find(|outcome| rank(outcome) <= lowest)
            .expect("some outcome ranks lowest");
        let mut more = cases.clone();
        more.push((outcome, most.saturating_add(u64::from(*first))));
        let (after, printed) = against(&card(&more, margin), &saved);
        assert!(after >= worse, "{printed}");
    }
});
