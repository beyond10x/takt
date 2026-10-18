//! The run record: `events.jsonl` in the case directory, one JSON object per line, appended and
//! never rewritten. Each line carries the instant it was written (`at`, as [`crate::now`] writes
//! one), the step it belongs to (`step`, 0 before a run's first) and the event (`event`) with its
//! fields. What a run prints is for the person watching it; this file is what a person, or a
//! program, reads back afterwards. Every run on a case appends to the same file, so a case
//! resumed after its gate continues its record.
//!
//! The file lies in the case directory, which a judgement step and a validator's command may
//! not write: a line in it was written by the kernel.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::Serialize;

/// The file in the case directory that holds the run record.
pub const EVENTS_FILE: &str = "events.jsonl";

/// Who refused: the kernel, or the planning store through its own verb.
pub const BY_TAKT: &str = "takt";
/// The planning store refused a write, its words verbatim.
pub const BY_STORE: &str = "store";

/// One event of a run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A run opened on the case, or resumed it: the protocol, the story when the case works
    /// one, the inputs as `name=value`.
    Opened {
        case: String,
        protocol: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        story: Option<String>,
        inputs: Vec<String>,
    },
    /// An evaluation: its sequence number, the decision file in the case directory it was
    /// written to, and the frontier as printed.
    Decision {
        seq: usize,
        file: String,
        frontier: String,
    },
    /// A step: by a `validator` or the `harness`, which one, the action, what it reported, and
    /// the tokens the harness reported when it reported any.
    Step {
        by: String,
        name: String,
        action: String,
        summary: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tokens: Option<u64>,
    },
    /// What a judgement step changed: the paths committed and the implementation revision they
    /// were committed as; no path and no revision when it changed no file.
    Effect {
        paths: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        revision: Option<String>,
    },
    /// An evidence record a validator wrote under the case: its id, kind and result, and its
    /// path relative to the tree.
    Evidence {
        id: String,
        kind: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        path: String,
    },
    /// An evidence record written on the story in the planning store: the kind, the reference
    /// and the instant given to the store (the case's record's own `observed_at`), and the
    /// commit on the case's branch that holds it.
    StoreEvidence {
        story: String,
        kind: String,
        reference: String,
        at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        commit: Option<String>,
    },
    /// The story moved in the planning store to `to`, and the commit on the case's branch that
    /// holds the move.
    StoreMove {
        story: String,
        to: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        commit: Option<String>,
    },
    /// A refusal: by the kernel ([`BY_TAKT`]: a step it discarded, a write it would not make)
    /// or by the planning store ([`BY_STORE`]), the reason verbatim.
    Refusal { by: String, reason: String },
    /// The run pauses on the forge for `seconds`, then observes the actions `on` again.
    Waiting { on: Vec<String>, seconds: f64 },
    /// The run ended on an error, as it was reported.
    Error { error: String },
    /// The run stopped: why, with the steps taken and the tokens reported.
    Stop {
        stop: String,
        steps: u64,
        tokens: u64,
    },
}

/// One line of the file: the instant, the step, the event.
#[derive(Serialize)]
struct Line<'a> {
    at: String,
    step: u64,
    #[serde(flatten)]
    event: &'a Event,
}

/// The run record of one case.
#[derive(Debug, Clone)]
pub struct Events {
    path: PathBuf,
}

impl Events {
    /// The record of the case at `case_dir`, `<case_dir>/events.jsonl`; nothing is written yet.
    #[must_use]
    pub fn new(case_dir: &Path) -> Self {
        Self {
            path: case_dir.join(EVENTS_FILE),
        }
    }

    /// The file's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends `event` at step `step`, the instant now, as one line written whole.
    pub fn emit(&self, step: u64, event: &Event) -> Result<()> {
        let mut line = serde_json::to_string(&Line {
            at: crate::now(),
            step,
            event,
        })
        .context("write an event")?;
        line.push('\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open {}", self.path.display()))?;
        file.write_all(line.as_bytes())
            .with_context(|| format!("write {}", self.path.display()))
    }

    /// Every line written so far, each read as JSON; none before the first. A line that does
    /// not read is an error naming the file.
    pub fn read(&self) -> Result<Vec<serde_json::Value>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", self.path.display()));
            }
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(n, line)| {
                serde_json::from_str(line)
                    .with_context(|| format!("read line {} of {}", n + 1, self.path.display()))
            })
            .collect()
    }
}
