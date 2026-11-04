//! The one-cap allowance `ci_wait_backoff.rs`'s `ci_result_follows_any_completion_within_one_cap`
//! holds a `ci_result` record to (story:ci-wait-watches-the-check,
//! story:a-slow-runner-is-no-late-ci-result), in one place, so the cases that measure the
//! allowance itself read the rule the case ships and not a copy of it.
//!
//! A run is measured in two parts, each against its own bound, because only the first holds a
//! wait the run chose:
//!
//! - Detection: from the instant the checks turned green to the read that saw them pass. The
//!   read before it saw them pending, so detection is at most the one wait after that read and
//!   the run's own time around it: `cap + own`, `own` the longest gap between two reads less the
//!   wait between them. A run that waits a whole interval too long is late here, whatever its
//!   runner's speed.
//! - The write: from that read to the record's modification time, at most [`WRITE_BOUND`], one
//!   second, whatever the run's own time. Between the read and the write the run does no wait:
//!   only what follows every observation (what it left compared with what the step began with,
//!   the tree's status, the record's file). The bound is not the run's own time, a maximum over
//!   two or three gaps that one load spike during the write outlasts (280 ms written against
//!   136 ms measured on a host at load 14). One second is past every write a runner was seen to
//!   take (the darwin record came 863 ms after the green, so after the read that saw it) and
//!   half the default first interval (2 s), so a write that held a wait would still show. No
//!   slack of the write reaches detection, so a slow write cannot hide a late read.
//!
//! On the darwin runner of story:a-slow-runner-is-no-late-ci-result (own 524 ms, the record
//! 863 ms after the green) a run on time passes by construction: its detection is within one gap
//! (at most `cap + own`, 749 ms), and its write within 863 ms.

use std::time::Duration;

/// The longest the write of the record may take after the read that saw the checks pass.
pub const WRITE_BOUND: Duration = Duration::from_secs(1);

/// One run, as the case measures it.
#[derive(Clone, Copy, Debug)]
pub struct Measured {
    /// The backoff's cap.
    pub cap: Duration,
    /// The run's own time between two reads: the longest gap less the wait between them.
    pub own: Duration,
    /// From the checks turning green to the read that saw them pass.
    pub detected: Duration,
    /// From that read to the record's modification time.
    pub written: Duration,
}

impl Measured {
    /// The bound on [`Self::detected`]: the cap and the run's own time.
    #[must_use]
    pub fn detection_bound(&self) -> Duration {
        self.cap + self.own
    }

    /// Whether the record came late: detection past its bound, or the write past [`WRITE_BOUND`].
    #[must_use]
    pub fn late(&self) -> bool {
        self.detected > self.detection_bound() || self.written > WRITE_BOUND
    }
}
