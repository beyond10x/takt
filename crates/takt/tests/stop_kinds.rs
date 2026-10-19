//! `STOP_KINDS` names every stop but `Completed`: the scorecard counts an outcome as a
//! completion only when it is not in that list, so a stop missing from it would score as
//! completed. One table below gives both the exhaustive match into the list and a stop of each
//! variant, so neither can leave a variant out of the other: a variant added to `Stop` without a
//! row fails to compile, a row whose index the list does not reach is a constant index out of
//! bounds, which fails to compile too, and a row is one variant's match and that same variant's
//! stop, checked. A row that reuses another's index names that kind twice, and fails.

use std::collections::BTreeSet;

use takt::Stop;
use takt::run::STOP_KINDS;

/// From one row per variant, `pattern => its name in STOP_KINDS (none for Completed), a stop of
/// it`: [`kind`], the exhaustive match, and [`every_stop`], each stop with whether it matches
/// its own row's pattern.
macro_rules! stops {
    ($($pattern:pat => $kind:expr, $stop:expr;)*) => {
        /// The name `STOP_KINDS` holds for `stop`; none for `Completed`, whose outcome is the
        /// one it names.
        fn kind(stop: &Stop) -> Option<&'static str> {
            match stop {
                $($pattern => $kind,)*
            }
        }

        /// A stop of each row, with whether it is of the variant its row matches.
        fn every_stop() -> Vec<(Stop, bool)> {
            vec![$({
                let stop = $stop;
                let own = matches!(stop, $pattern);
                (stop, own)
            }),*]
        }
    };
}

stops! {
    Stop::Completed(_) => None, Stop::Completed("accepted".to_owned());
    Stop::ApprovalRequired(_) => Some(STOP_KINDS[0]),
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]);
    Stop::NothingAdmissible => Some(STOP_KINDS[1]), Stop::NothingAdmissible;
    Stop::Iterations(_) => Some(STOP_KINDS[2]), Stop::Iterations(3);
    Stop::Hours(_) => Some(STOP_KINDS[3]), Stop::Hours(1.5);
    Stop::Tokens(_) => Some(STOP_KINDS[4]), Stop::Tokens(10);
    Stop::StepRefused(_) => Some(STOP_KINDS[5]), Stop::StepRefused("why".to_owned());
    Stop::NoProgress => Some(STOP_KINDS[6]), Stop::NoProgress;
    Stop::Waiting { .. } => Some(STOP_KINDS[7]),
        Stop::Waiting { on: vec!["ci.read".to_owned()], budget: None, local: Vec::new() };
    Stop::Unperformable { .. } => Some(STOP_KINDS[8]),
        Stop::Unperformable { claim: "pr.opened".to_owned(), performer: "the forge".to_owned() };
    Stop::DiskLow { .. } => Some(STOP_KINDS[9]),
        Stop::DiskLow { path: "/srv/tree".into(), free_bytes: 64 << 20, min_bytes: 10 << 30 };
}

/// A stop of every variant, one per row and each of its own row's variant; each display opens
/// with its name in the list; the names are distinct, one per variant but `Completed`, and are
/// the whole list.
#[test]
fn stop_kinds_names_every_stop_but_completed_by_the_word_its_display_opens_with() {
    let stops = every_stop();
    assert_eq!(
        stops.len(),
        STOP_KINDS.len() + 1,
        "one row per variant: every name in the list and Completed"
    );
    let mut named = BTreeSet::new();
    for (stop, own) in &stops {
        let shown = stop.to_string();
        assert!(own, "{shown} is not of its own row's variant");
        match kind(stop) {
            None => assert!(shown.starts_with("Completed ("), "{shown}"),
            Some(kind) => {
                assert_eq!(
                    shown.split([' ', '(']).next(),
                    Some(kind),
                    "{shown} is listed as {kind}"
                );
                assert!(named.insert(kind), "{kind} twice: two rows share an index");
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
