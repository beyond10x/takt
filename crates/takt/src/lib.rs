//! Takt: composable agentic work. The kernel keeps a Canon case on disk, evaluates it over the
//! evidence validators wrote, and runs the loop: evaluate, frontier, one step, effect,
//! observation, validator, evidence, evaluate. A judgement step is performed by a [`Harness`];
//! a deterministic step is a [`Validator`] or a command. The model never produces evidence.
//! Each run appends its events to the case's `events.jsonl` ([`events`]).
//!
//! The design is `docs/design/takt.md`.

pub mod case;
pub mod commit_style;
pub mod confinement;
pub mod console;
pub mod events;
pub mod evidence;
pub mod forge;
pub mod frontier;
pub mod git;
pub mod harness;
pub mod improvement;
pub mod loop_scorecard;
#[cfg(target_os = "macos")]
mod marked;
pub mod obligations;
pub mod promotion;
pub mod protocol;
pub mod replay;
pub mod report;
pub mod run;
pub mod run_config;
pub mod scan;
pub mod scorecard;
pub mod trace;
pub mod validator;
pub mod view;
pub mod workspace;

pub use case::{CaseDir, Decision};
pub use forge::Forge;
pub use frontier::{Frontier, Story, Unit};
pub use harness::{Harness, StepContext, StepReport};
pub use protocol::LoadedProtocol;
pub use run::{Run, RunOptions, Stop};
pub use validator::Validator;

/// The instant now, in whole seconds: `YYYY-MM-DDTHH:MM:SSZ`, as case ids and trial names carry
/// it. An evidence record is stamped to the microsecond instead (`instant_micros`).
#[must_use]
pub fn now() -> String {
    instant(time::OffsetDateTime::now_utc())
}

/// `at` as Canon writes an instant: `YYYY-MM-DDTHH:MM:SSZ`.
fn instant(at: time::OffsetDateTime) -> String {
    let format =
        time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    at.format(&format)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

/// The clock now, in microseconds since the epoch.
pub(crate) fn clock_micros() -> i128 {
    time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000
}

/// `micros` (microseconds since the epoch) as an instant to the microsecond,
/// `YYYY-MM-DDTHH:MM:SS.ffffffZ`: the one precision takt writes every evidence record's
/// `observed_at` at, so Canon, which compares the records of an evaluation at the coarsest
/// precision written among them, orders two of them within one second.
pub(crate) fn instant_micros(micros: i128) -> String {
    let format = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
    );
    time::OffsetDateTime::from_unix_timestamp_nanos(micros.saturating_mul(1_000))
        .ok()
        .and_then(|at| at.format(&format).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00.000000Z".to_owned())
}

#[cfg(test)]
mod tests {
    /// An instant is written with six fraction digits, leading zeros kept.
    #[test]
    fn an_instant_is_written_to_the_microsecond() {
        assert_eq!(
            super::instant_micros(32_503_680_000_000_001),
            "3000-01-01T00:00:00.000001Z"
        );
        assert_eq!(super::instant_micros(0), "1970-01-01T00:00:00.000000Z");
        assert_eq!(
            super::instant_micros(1_761_739_200_250_000),
            "2025-10-29T12:00:00.250000Z"
        );
    }
}
