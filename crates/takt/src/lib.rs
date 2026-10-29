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

/// The instant now, as Canon writes one: `YYYY-MM-DDTHH:MM:SSZ`.
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

/// The whole second `second` (seconds since the epoch) as Canon writes an instant.
pub(crate) fn instant_at(second: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(second)
        .map_or_else(|_| "1970-01-01T00:00:00Z".to_owned(), instant)
}

/// The longest [`now_after`] waits, by the monotonic clock: past the next whole second, with room
/// for the sleep to overrun.
const NOW_AFTER_BOUND: std::time::Duration = std::time::Duration::from_millis(1_100);

/// The clock's whole second once it has left `second` (seconds since the epoch): at once when it
/// already has, else after sleeping to the next whole second. The wait is bounded by the monotonic
/// clock ([`NOW_AFTER_BOUND`]), whatever the system clock does; a system clock that has not left
/// `second` by then (stepped back, or `second` ahead of it) gets `second + 1`, the one second the
/// caller needs past `second`, though the clock does not read it.
pub(crate) fn now_after(second: i64) -> i64 {
    let started = std::time::Instant::now();
    loop {
        let clock = time::OffsetDateTime::now_utc();
        if clock.unix_timestamp() > second {
            return clock.unix_timestamp();
        }
        let left = NOW_AFTER_BOUND.saturating_sub(started.elapsed());
        if left.is_zero() {
            return second + 1;
        }
        let to_next =
            std::time::Duration::from_nanos((1_000_000_000 - u64::from(clock.nanosecond())).max(1));
        std::thread::sleep(to_next.min(left));
    }
}

/// The clock's whole second now, seconds since the epoch.
pub(crate) fn clock_second() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    /// A second the clock has left is answered at once with the clock's second.
    #[test]
    fn a_second_already_left_is_answered_at_once() {
        let started = Instant::now();
        let second = super::clock_second() - 5;
        assert!(super::now_after(second) > second);
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    /// A second the system clock does not leave within the bound, here one far ahead of it, is
    /// answered with the second after it once the monotonic bound is spent.
    #[test]
    fn the_wait_is_bounded_whatever_the_system_clock_reads() {
        let started = Instant::now();
        let ahead = super::clock_second() + 3_600;
        assert_eq!(super::now_after(ahead), ahead + 1);
        let waited = started.elapsed();
        assert!(waited >= super::NOW_AFTER_BOUND, "{waited:?}");
        assert!(waited < Duration::from_millis(1_500), "{waited:?}");
    }
}
