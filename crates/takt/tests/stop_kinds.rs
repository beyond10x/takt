//! `STOP_KINDS` names every stop but `Completed`: the scorecard counts an outcome as a
//! completion only when it is not in that list, so a stop missing from it would score as
//! completed. The match below is exhaustive and indexes the list: a variant added to `Stop`
//! without an arm fails to compile, and an arm whose index the list does not reach is a constant
//! index out of bounds, which fails to compile too.

use std::collections::BTreeSet;

use takt::Stop;
use takt::run::STOP_KINDS;

/// The name `STOP_KINDS` holds for `stop`; none for `Completed`, whose outcome is the one it
/// names.
fn kind(stop: &Stop) -> Option<&'static str> {
    match stop {
        Stop::Completed(_) => None,
        Stop::ApprovalRequired(_) => Some(STOP_KINDS[0]),
        Stop::NothingAdmissible => Some(STOP_KINDS[1]),
        Stop::Iterations(_) => Some(STOP_KINDS[2]),
        Stop::Hours(_) => Some(STOP_KINDS[3]),
        Stop::Tokens(_) => Some(STOP_KINDS[4]),
        Stop::StepRefused(_) => Some(STOP_KINDS[5]),
        Stop::NoProgress => Some(STOP_KINDS[6]),
        Stop::Waiting { .. } => Some(STOP_KINDS[7]),
        Stop::Unperformable { .. } => Some(STOP_KINDS[8]),
        Stop::DiskLow { .. } => Some(STOP_KINDS[9]),
    }
}

/// One stop of each variant.
fn every_stop() -> Vec<Stop> {
    vec![
        Stop::Completed("accepted".to_owned()),
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        Stop::NothingAdmissible,
        Stop::Iterations(3),
        Stop::Hours(1.5),
        Stop::Tokens(10),
        Stop::StepRefused("why".to_owned()),
        Stop::NoProgress,
        Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: None,
            local: Vec::new(),
        },
        Stop::Unperformable {
            claim: "pr.opened".to_owned(),
            performer: "the forge".to_owned(),
        },
        Stop::DiskLow {
            path: "/srv/tree".into(),
            free_bytes: 64 << 20,
            min_bytes: 10 << 30,
        },
    ]
}

/// Each stop's display opens with its name in the list, every name in the list belongs to
/// exactly one variant, and `Completed` is not in it.
#[test]
fn stop_kinds_names_every_stop_but_completed_by_the_word_its_display_opens_with() {
    let mut named = BTreeSet::new();
    for stop in every_stop() {
        let shown = stop.to_string();
        match kind(&stop) {
            None => assert!(shown.starts_with("Completed ("), "{shown}"),
            Some(kind) => {
                assert_eq!(
                    shown.split([' ', '(']).next(),
                    Some(kind),
                    "{shown} is listed as {kind}"
                );
                assert!(named.insert(kind), "{kind} twice");
            }
        }
    }
    let listed: BTreeSet<&str> = STOP_KINDS.iter().copied().collect();
    assert_eq!(listed.len(), STOP_KINDS.len(), "a name listed twice");
    assert_eq!(
        named, listed,
        "a stop of every variant names the whole list"
    );
    assert!(!listed.contains("Completed"));
}
