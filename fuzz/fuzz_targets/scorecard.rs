//! The scorecard's rule over any two scorecards of one scenario: each side's median of tokens
//! lies within its minimum and maximum, a scorecard is never worse than itself, and adding a
//! case no better than any of the candidate's never makes its verdict better.
//!
//! The input: a byte whose value modulo 3 is the margin, then one case each 9 bytes, the first
//! naming the side (its high bit: the candidate), what its stop line says of completion (the
//! next two bits: 1 not completed, 2 completed, else no word) and the outcome (its low five
//! bits), the next eight the tokens, little-endian.

#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use takt::run::STOP_KINDS;
use takt::scorecard::{DISK_LOW, FORMAT, METRICS, Row, Rule, Scorecard, rank};

/// Two outcomes a `Completed` stop names, every other stop's kind, an error and no stop.
const OUTCOMES: [&str; STOP_KINDS.len() + 4] = {
    let mut all = [""; STOP_KINDS.len() + 4];
    all[0] = "accepted";
    all[1] = "merged";
    let mut at = 0;
    while at < STOP_KINDS.len() {
        all[at + 2] = STOP_KINDS[at];
        at += 1;
    }
    all[at + 2] = "error";
    all[at + 3] = "unfinished";
    all
};

/// What the stop line of a case's run says of completion, from two bits: `completed`.
fn completed(bits: u8) -> Option<bool> {
    match bits & 0b11 {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}

fn card(cases: &[(&str, u64, Option<bool>)], margin: u32) -> Scorecard {
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
            .map(|(n, (outcome, tokens, completed))| Row {
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
                completed: *completed,
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
        let outcome = OUTCOMES[usize::from(chunk[0] & 0x1f) % OUTCOMES.len()];
        let said = completed(chunk[0] >> 5);
        let tokens = u64::from_le_bytes(chunk[1..9].try_into().expect("eight bytes"));
        if chunk[0] & 0x80 == 0 {
            saved.push((outcome, tokens, said));
        } else {
            candidate.push((outcome, tokens, said));
        }
    }
    let (saved, candidate) = (card(&saved, margin), card(&candidate, margin));
    // A `DiskLow` trial on either side gives the scenario no verdict, whatever the count; when
    // the other side has no case of it, the error names that gap too.
    if !saved.disk_low().is_empty() || !candidate.disk_low().is_empty() {
        let mut out = Vec::new();
        let error = candidate
            .against(&saved, &mut out)
            .expect_err("a scenario with a DiskLow trial gave a verdict")
            .to_string();
        assert!(error.contains(DISK_LOW), "{error}");
        if saved.rows.is_empty() {
            assert!(error.contains("not in the saved scorecard"), "{error}");
        }
        if candidate.rows.is_empty() {
            assert!(error.contains("too few cases"), "{error}");
        }
        return;
    }
    let short = saved.rows.is_empty() || candidate.rows.is_empty();
    // A side with no case gives no verdict, and says so: fewer cases than the rule's one trial
    // on this side, a scenario the saved side lacks, or no scenario case on either side.
    if short {
        let mut out = Vec::new();
        match candidate.against(&saved, &mut out) {
            Err(error) => {
                let error = error.to_string();
                assert!(
                    [
                        "too few cases",
                        "not in the saved scorecard",
                        "no scenario case"
                    ]
                    .iter()
                    .any(|refusal| error.contains(refusal)),
                    "{error}"
                );
            }
            Ok(worse) => panic!("a side with no case gave a verdict: {worse}"),
        }
        return;
    }
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
    // Turning any counted case into `DiskLow`, on either side, never improves the verdict: it
    // keeps it or gives none.
    let lowered = |card: &Scorecard| {
        let mut card = card.clone();
        let at = usize::from(*first) % card.rows.len();
        card.rows[at].stop = DISK_LOW.to_owned();
        card.rows[at].outcome = DISK_LOW.to_owned();
        card.rows[at].completed = Some(false);
        card
    };
    for (one, other) in [
        (lowered(&candidate), saved.clone()),
        (candidate.clone(), lowered(&saved)),
    ] {
        let mut out = Vec::new();
        if let Ok(after) = one.against(&other, &mut out) {
            assert!(after >= worse, "a DiskLow trial improved the verdict: {after} < {worse}");
        }
    }
    let cases: Vec<(&str, u64, Option<bool>)> = candidate
        .rows
        .iter()
        .map(|row| {
            let outcome = OUTCOMES
                .iter()
                .find(|outcome| **outcome == row.outcome)
                .expect("an outcome of the list");
            (*outcome, row.tokens, row.completed)
        })
        .collect();
    if let Some(lowest) = cases.iter().map(|(outcome, _, _)| rank(outcome)).min() {
        let most = cases.iter().map(|(_, tokens, _)| *tokens).max().unwrap_or(0);
        let outcome = OUTCOMES
            .iter()
            .find(|outcome| rank(outcome) <= lowest && **outcome != DISK_LOW)
            .expect("some outcome ranks lowest");
        let mut more = cases.clone();
        more.push((outcome, most.saturating_add(u64::from(*first)), completed(*first)));
        let (after, printed) = against(&card(&more, margin), &saved);
        assert!(after >= worse, "{printed}");
    }
});
