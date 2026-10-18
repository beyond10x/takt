//! The run record: `events.jsonl` in the case directory, one JSON object per line, appended and
//! never rewritten. Each line carries the instant it was written (`at`, as [`instant`] writes
//! one, to the microsecond; a line written before had whole seconds), the step it belongs to
//! (`step`, 0 before a run's first) and the event (`event`) with its fields. A field added to an
//! event later is optional: a line written before it reads as it did. What a run prints is for
//! the person watching it; this file is what a person, or a program, reads back afterwards.
//! Every run on a case appends to the same file, so a case resumed after its gate continues its
//! record.
//!
//! The file lies in the case directory, which a judgement step and a validator's command may
//! not write: a line in it was written by the kernel.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

/// The file in the case directory that holds the run record.
pub const EVENTS_FILE: &str = "events.jsonl";

/// Who refused: the kernel, or the planning store through its own verb.
pub const BY_TAKT: &str = "takt";
/// The planning store refused a write, its words verbatim.
pub const BY_STORE: &str = "store";

/// The instant now, to the microsecond: `YYYY-MM-DDTHH:MM:SS.ffffffZ`.
#[must_use]
pub fn instant() -> String {
    let now = time::OffsetDateTime::now_utc();
    let format = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
    );
    now.format(&format)
        .unwrap_or_else(|_| "1970-01-01T00:00:00.000000Z".to_owned())
}

/// The tokens a harness reported for one step, by component; [`TokenUsage::total`] is the
/// step's `tokens`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
}

impl TokenUsage {
    /// The four components summed.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_creation
    }
}

/// One event of a run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A run opened on the case, or resumed it: the protocol, the story when the case works
    /// one, the inputs as `name=value`, and the commit the case's branch forked from when the
    /// case recorded one.
    Opened {
        case: String,
        protocol: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        story: Option<String>,
        inputs: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        base: Option<String>,
    },
    /// An evaluation: its sequence number, the decision file in the case directory it was
    /// written to, and the frontier as printed.
    Decision {
        seq: usize,
        file: String,
        frontier: String,
    },
    /// A step: by a `validator` or the `harness`, which one, the action, what it reported, the
    /// tokens the harness reported when it reported any, and their four components when it
    /// reported those; a validator's, the revision it observed.
    Step {
        by: String,
        name: String,
        action: String,
        summary: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<TokenUsage>,
        #[serde(skip_serializing_if = "Option::is_none")]
        revision: Option<String>,
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
    /// commit on the case's branch that holds it. The instant is `observed_at`, never `at`,
    /// which is the line's own.
    StoreEvidence {
        story: String,
        kind: String,
        reference: String,
        observed_at: String,
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
    /// An error, as it was reported: the one a run ended on, or one met while recording an
    /// accepted case in the store, after which the run keeps its stop.
    Error { error: String },
    /// The line `line`, from 1, was cut short, without its end, and is skipped on read; written
    /// with the next event, which ends the torn line first.
    Skipped { line: usize, reason: String },
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

    /// Appends `event` at step `step`, the instant now, as one line written whole. A file whose
    /// last line was cut short (a write that hit a full disk, a process killed mid-write) has no
    /// end to that line: it is ended first, and a [`Event::Skipped`] line naming it is written
    /// before the event, in the same write, so the event is never appended to a torn line and
    /// the reader knows the torn one was skipped.
    pub fn emit(&self, step: u64, event: &Event) -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open {}", self.path.display()))?;
        let mut text = String::new();
        if let Some(line) = self
            .torn(&mut file)
            .with_context(|| format!("read {}", self.path.display()))?
        {
            text.push('\n');
            text.push_str(&Self::line(
                step,
                &Event::Skipped {
                    line,
                    reason: "the line was cut short, without its end, and is skipped".to_owned(),
                },
            )?);
        }
        text.push_str(&Self::line(step, event)?);
        file.write_all(text.as_bytes())
            .with_context(|| format!("write {}", self.path.display()))
    }

    /// `event` at `step`, the instant now, as one line with its end.
    fn line(step: u64, event: &Event) -> Result<String> {
        let mut line = serde_json::to_string(&Line {
            at: instant(),
            step,
            event,
        })
        .context("write an event")?;
        line.push('\n');
        Ok(line)
    }

    /// The number, from 1, of the file's last line when it has no end: the line was cut short.
    /// `None` for an empty file and for one whose last line ends.
    fn torn(&self, file: &mut std::fs::File) -> std::io::Result<Option<usize>> {
        use std::io::{Read as _, Seek as _, SeekFrom};
        if file.metadata()?.len() == 0 {
            return Ok(None);
        }
        let mut last = [0_u8; 1];
        file.seek(SeekFrom::End(-1))?;
        file.read_exact(&mut last)?;
        if last[0] == b'\n' {
            return Ok(None);
        }
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(Some(
            bytes.iter().filter(|byte| **byte == b'\n').count() + 1,
        ))
    }

    /// Every line written so far, each read as JSON, in order; none before the first. Blank
    /// lines are passed over. A line that does not read is skipped when it was cut short: it
    /// is the last line and has no end (the next [`Events::emit`] ends it and records the
    /// skip), or the line after it is the [`Event::Skipped`] line naming it. Lines that do not
    /// read one after another (a repair that was cut short too) are skipped together when the
    /// last of them is such a line. Any other line that does not read is an error naming the
    /// file and the line.
    pub fn read(&self) -> Result<Vec<serde_json::Value>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", self.path.display()));
            }
        };
        let ended = text.ends_with('\n');
        let lines: Vec<&str> = text.split_terminator('\n').collect();
        let parsed: Vec<Option<serde_json::Value>> = lines
            .iter()
            .map(|line| serde_json::from_str(line).ok())
            .collect();
        let mut read = Vec::new();
        let mut n = 0;
        while n < lines.len() {
            if lines[n].trim().is_empty() {
                n += 1;
                continue;
            }
            if let Some(value) = &parsed[n] {
                read.push(value.clone());
                n += 1;
                continue;
            }
            // A repair cut short too is one more torn line: the lines that do not read, one
            // after another, are skipped together when the last of them was cut short.
            let mut last = n;
            while last + 1 < lines.len()
                && parsed[last + 1].is_none()
                && !lines[last + 1].trim().is_empty()
            {
                last += 1;
            }
            let last_unended = last + 1 == lines.len() && !ended;
            let named_skipped = parsed
                .get(last + 1)
                .and_then(Option::as_ref)
                .is_some_and(|next| {
                    next["event"] == "skipped" && next["line"].as_u64() == Some(last as u64 + 1)
                });
            if !(last_unended || named_skipped) {
                return Err(anyhow::anyhow!(
                    "line {} of {} does not read as an event, and was not cut short",
                    n + 1,
                    self.path.display()
                ));
            }
            n = last + 1;
        }
        Ok(read)
    }
}
