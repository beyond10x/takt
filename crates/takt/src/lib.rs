//! Takt: composable agentic work. The kernel keeps a Canon case on disk, evaluates it over the
//! evidence validators wrote, and runs the loop: evaluate, frontier, one step, effect,
//! observation, validator, evidence, evaluate. A judgement step is performed by a [`Harness`];
//! a deterministic step is a [`Validator`] or a command. The model never produces evidence.
//! Each run appends its events to the case's `events.jsonl` ([`events`]).
//!
//! The design is `docs/design/takt.md`.

pub mod case;
pub mod confinement;
pub mod decision;
pub mod events;
pub mod evidence;
pub mod forge;
pub mod frontier;
pub mod git;
pub mod harness;
pub mod improvement;
pub mod loop_scorecard;
pub mod promotion;
pub mod protocol;
pub mod replay;
pub mod report;
pub mod run;
pub mod scorecard;
pub mod trace;
pub mod validator;
pub mod view;

pub use case::CaseDir;
pub use decision::Decision;
pub use forge::Forge;
pub use frontier::{Frontier, Story, Unit};
pub use harness::{Harness, StepContext, StepReport};
pub use protocol::LoadedProtocol;
pub use run::{Run, RunOptions, Stop};
pub use validator::Validator;

/// The instant now, as Canon writes one: `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn now() -> String {
    let now = time::OffsetDateTime::now_utc();
    let format =
        time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    now.format(&format)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
