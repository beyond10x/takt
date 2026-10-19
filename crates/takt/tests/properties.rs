//! story:property-and-fuzz-tests: properties of the parsers that read input takt does not
//! control, each run with a fixed seed so a run is the same run every time: the run record
//! (`Events::read`) after any bytes and cut at any byte, the harness's reply, a case id, and the
//! scorecard's rule and median. A failure proptest finds is kept as a unit test below.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use proptest::prelude::*;
use proptest::test_runner::{RngAlgorithm, RngSeed};
use takt::case::{CASES_DIR, check_id};
use takt::events::{EVENTS_FILE, Event, Events};
use takt::harness::ClaudeCode;
use takt::scorecard::{FORMAT, METRICS, Row, Rule, Scorecard, rank};

/// The seed every property runs with, under ChaCha, pinned so a proptest upgrade that changes
/// its default generator does not change the cases.
const SEED: u64 = 0x7a6b_7400_0013;
/// The cases each property runs.
const CASES: u32 = 1024;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: CASES,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(SEED),
        ..ProptestConfig::default()
    }
}

/// A directory of its own under the test target's scratch, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "properties-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `bytes` as a case's run record, read back.
fn read_bytes(bytes: &[u8]) -> anyhow::Result<Vec<serde_json::Value>> {
    let dir = Scratch::new();
    fs::write(dir.0.join(EVENTS_FILE), bytes).expect("write the record");
    Events::new(&dir.0).read()
}

fn event() -> impl Strategy<Value = Event> {
    let text = any::<String>;
    prop_oneof![
        (
            text(),
            text(),
            text(),
            text(),
            proptest::option::of(any::<u64>())
        )
            .prop_map(|(by, name, action, summary, tokens)| Event::Step {
                by,
                name,
                action,
                summary,
                tokens,
                usage: None,
                revision: None,
                model: None,
                cost_usd: None,
                duration_ms: None,
                api_ms: None,
                turns: None,
                started_at: None,
            }),
        (text(), text()).prop_map(|(by, reason)| Event::Refusal { by, reason }),
        text().prop_map(|error| Event::Error { error }),
        (text(), any::<u64>(), any::<u64>(), any::<Option<bool>>()).prop_map(
            |(stop, steps, tokens, completed)| Event::Stop {
                stop,
                steps,
                tokens,
                completed,
            }
        ),
    ]
}

/// A record as `Events::emit` writes it, its last event naming a multi-byte character, so a
/// cut can land inside one; the bytes and the values it reads as.
fn record(events: &[(u64, Event)]) -> (Vec<u8>, Vec<serde_json::Value>) {
    let dir = Scratch::new();
    let record = Events::new(&dir.0);
    for (step, event) in events {
        record.emit(*step, event).expect("emit");
    }
    record
        .emit(
            0,
            &Event::Error {
                error: "grün ✓".to_owned(),
            },
        )
        .expect("emit");
    let bytes = fs::read(record.path()).expect("read the record's bytes");
    let read = record.read().expect("a record emit wrote reads");
    (bytes, read)
}

fn events() -> impl Strategy<Value = Vec<(u64, Event)>> {
    proptest::collection::vec((0..50_u64, event()), 0..6)
}

/// A string that names an action line's JSON value: any JSON, nested.
fn json() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::from),
        any::<i64>().prop_map(serde_json::Value::from),
        any::<String>().prop_map(serde_json::Value::from),
    ];
    leaf.prop_recursive(3, 16, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::from),
            proptest::collection::btree_map(any::<String>(), inner, 0..4)
                .prop_map(|map| { serde_json::Value::Object(map.into_iter().collect()) }),
        ]
    })
}

/// A line of a reply: any text without a line end.
fn line() -> impl Strategy<Value = String> {
    "[^\n]{0,24}"
}

/// Whether the reply parser reads `line` as an action line.
fn is_action_line(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('{') && line.contains("\"action\"")
}

fn action_line(action: &str, summary: &str) -> String {
    serde_json::json!({ "action": action, "summary": summary }).to_string()
}

/// An id built to probe the cases directory's edges: dots, separators, characters that do not
/// show, a drive-like colon, a home tilde.
fn edgy_id() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just("."),
            Just(".."),
            Just("/"),
            Just("\\"),
            Just("a"),
            Just("ü"),
            Just("~"),
            Just(":"),
            Just("\u{0}"),
            Just(" "),
            Just("\u{200b}"),
            Just(".lock"),
            Just("@{"),
        ],
        0..8,
    )
    .prop_map(|parts| parts.concat())
}

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

/// A scorecard of one scenario, `s`, a case a `(outcome, tokens)`, compared under `margin` with a
/// rule of one trial, so the 1 to 7 cases the properties draw compare; the trials rule itself is
/// the scorecard tests' to check.
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
                completed: None,
            })
            .collect(),
        no_record: Vec::new(),
        unstated: BTreeMap::new(),
    }
}

fn cases() -> impl Strategy<Value = Vec<(&'static str, u64)>> {
    proptest::collection::vec(
        (
            proptest::sample::select(&OUTCOMES[..]),
            prop_oneof![0..100_u64, any::<u64>()],
        ),
        1..8,
    )
}

/// The scenarios marked worse comparing `candidate` with `saved`, and what was printed.
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

/// The events fuzz target's first crash against a reader that took the whole file as UTF-8: a
/// line of one byte that is not UTF-8, ended, and named by no `skipped` line is refused,
/// naming it, never a panic.
#[test]
fn an_ended_line_that_is_not_utf8_is_refused() {
    let error = read_bytes(b"\n\x82\n").expect_err("a line that does not read, not cut short");
    assert!(error.to_string().starts_with("line 2 of "), "{error:#}");
}

proptest! {
    #![proptest_config(config())]

    /// Every event `emit` writes reads back, in order.
    #[test]
    fn a_record_emit_wrote_reads_back_whole(events in events()) {
        let (_, read) = record(&events);
        prop_assert_eq!(read.len(), events.len() + 1);
    }

    /// A record cut at any byte, a multi-byte character's among them, reads as the lines it
    /// holds whole: a prefix of the record's, and none of its ended lines lost.
    #[test]
    fn a_record_cut_at_any_byte_reads_its_whole_lines(
        events in events(),
        cut in any::<proptest::sample::Index>(),
    ) {
        let (bytes, full) = record(&events);
        let cut = cut.index(bytes.len() + 1);
        let read = read_bytes(&bytes[..cut]);
        prop_assert!(read.is_ok(), "cut at {cut}: {read:?}");
        let read = read.unwrap();
        let ended = bytes[..cut].iter().filter(|byte| **byte == b'\n').count();
        prop_assert!(read.len() >= ended && read.len() <= full.len());
        prop_assert_eq!(&read[..], &full[..read.len()]);
        // Cut just before a line's end, the unended last line is whole and reads; cut at a
        // line's end, nothing follows the ended lines.
        if cut > 0 && cut < bytes.len() && bytes[cut] == b'\n' && bytes[cut - 1] != b'\n' {
            prop_assert_eq!(read.len(), ended + 1, "cut at {} before a line's end", cut);
        } else if cut == 0 || bytes[cut - 1] == b'\n' {
            prop_assert_eq!(read.len(), ended, "cut at {} on a line's end", cut);
        }
    }

    /// A record cut inside a multi-byte character reads as the lines before it
    /// (story:run-record-survives-a-torn-character).
    #[test]
    fn a_record_torn_inside_a_character_reads(
        events in events(),
        at in any::<proptest::sample::Index>(),
    ) {
        let (bytes, full) = record(&events);
        let last = bytes[..bytes.len() - 1]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |end| end + 1);
        let inside: Vec<usize> = (last..bytes.len())
            .filter(|n| (0x80..0xc0).contains(&bytes[*n]))
            .collect();
        let cut = inside[at.index(inside.len())];
        let read = read_bytes(&bytes[..cut]);
        prop_assert!(read.is_ok(), "cut at {cut}: {read:?}");
        prop_assert_eq!(read.unwrap(), full[..full.len() - 1].to_vec());
    }

    /// Any bytes after a valid record never lose a line of it: on a last line without its end
    /// the record reads, and whatever the bytes, a record that reads starts with it.
    #[test]
    fn any_bytes_after_a_record_keep_its_lines(
        events in events(),
        after in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let (mut bytes, full) = record(&events);
        bytes.extend_from_slice(&after);
        let read = read_bytes(&bytes);
        if !after.contains(&b'\n') {
            prop_assert!(read.is_ok(), "{read:?}");
        }
        if let Ok(read) = read {
            prop_assert!(read.len() >= full.len() && read.len() <= full.len() + after.len() + 1);
            prop_assert_eq!(&read[..full.len()], &full[..]);
        }
    }

    /// Any bytes at all are a record that reads or an error; never a panic.
    #[test]
    fn any_bytes_read_or_refuse(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let _ = read_bytes(&bytes);
    }

    /// Any text is a reply that names an action or an error; never a panic.
    #[test]
    fn any_reply_parses_or_refuses(reply in any::<String>()) {
        let _ = ClaudeCode::parse(&reply);
    }

    /// The last action line wins, whatever came before it and whatever text follows it.
    #[test]
    fn the_last_action_line_wins(
        before in proptest::collection::vec(line(), 0..6),
        earlier in proptest::option::of((any::<String>(), any::<String>())),
        action in any::<String>(),
        summary in any::<String>(),
        after in proptest::collection::vec(line(), 0..6),
    ) {
        let mut lines = before;
        if let Some((action, summary)) = earlier {
            lines.push(action_line(&action, &summary));
        }
        lines.push(action_line(&action, &summary));
        lines.extend(after.into_iter().filter(|line| !is_action_line(line)));
        let parsed = ClaudeCode::parse(&lines.join("\n"));
        prop_assert!(parsed.is_ok(), "{parsed:?}");
        prop_assert_eq!(parsed.unwrap(), (action, summary));
    }

    /// An action line's action is read when it is a string and refused when it is any other
    /// JSON.
    #[test]
    fn an_action_that_is_not_a_string_is_refused(
        value in json(),
        before in proptest::collection::vec(line(), 0..4),
    ) {
        let mut lines = before;
        lines.push(serde_json::json!({ "action": value }).to_string());
        let parsed = ClaudeCode::parse(&lines.join("\n"));
        match value.as_str() {
            Some(action) => prop_assert_eq!(parsed.unwrap(), (action.to_owned(), String::new())),
            None => prop_assert!(parsed.is_err(), "{parsed:?}"),
        }
    }

    /// No id `check_id` accepts leaves the cases directory: it is one plain name under it.
    #[test]
    fn an_accepted_id_stays_under_the_cases_directory(
        id in prop_oneof![any::<String>(), edgy_id()],
    ) {
        if check_id(&id).is_ok() {
            let components: Vec<Component<'_>> = Path::new(&id).components().collect();
            prop_assert_eq!(
                components,
                vec![Component::Normal(id.as_ref())],
                "{:?}",
                id
            );
            let cases = Path::new("/srv/repo").join(CASES_DIR);
            let dir = cases.join(&id);
            prop_assert_eq!(dir.parent(), Some(cases.as_path()));
            prop_assert!(!id.contains('\0'));
        }
    }

    /// Each side's median of tokens lies within its minimum and maximum, as printed.
    #[test]
    fn the_median_lies_within_min_and_max(
        saved in cases(),
        candidate in cases(),
        margin in 0..3_u32,
    ) {
        let (_, printed) = against(&card(&candidate, margin), &card(&saved, margin));
        let mut sides = 0;
        for line in printed.lines().filter(|line| line.contains(" tokens min ")) {
            let words: Vec<&str> = line.split_whitespace().collect();
            let after = |word: &str| {
                let at = words.iter().position(|w| *w == word).expect(word);
                words[at + 1]
            };
            let (min, median, max) = (twice(after("min")), twice(after("median")), twice(after("max")));
            prop_assert!(min <= median && median <= max, "{line}");
            sides += 1;
        }
        prop_assert_eq!(sides, 2, "{}", printed);
    }

    /// A scorecard is never worse than itself.
    #[test]
    fn a_scorecard_is_not_worse_than_itself(cases in cases(), margin in 0..3_u32) {
        let card = card(&cases, margin);
        prop_assert_eq!(against(&card, &card).0, 0);
    }

    /// Adding a case no better than any of the candidate's, in outcome and in tokens, never
    /// makes its verdict better.
    #[test]
    fn a_worse_case_never_makes_a_verdict_better(
        saved in cases(),
        candidate in cases(),
        margin in 0..3_u32,
        outcome in any::<proptest::sample::Index>(),
        more in prop_oneof![0..100_u64, any::<u64>()],
    ) {
        let lowest = candidate.iter().map(|(outcome, _)| rank(outcome)).min().unwrap_or(0);
        let no_better: Vec<&str> = OUTCOMES
            .iter()
            .copied()
            .filter(|outcome| rank(outcome) <= lowest)
            .collect();
        let most = candidate.iter().map(|(_, tokens)| *tokens).max().unwrap_or(0);
        let mut worse = candidate.clone();
        worse.push((no_better[outcome.index(no_better.len())], most.saturating_add(more)));
        let saved = card(&saved, margin);
        let (before, _) = against(&card(&candidate, margin), &saved);
        let (after, printed) = against(&card(&worse, margin), &saved);
        prop_assert!(after >= before, "{}", printed);
    }
}
