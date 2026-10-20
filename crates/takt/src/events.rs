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

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
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
    /// The four components summed, at most `u64::MAX`.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_creation)
    }
}

/// One event of a run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A run opened on the case, or resumed it: the protocol, the story when the case works
    /// one, the inputs as `name=value`, the commit the case's branch forked from when the
    /// case recorded one, and the revision the running binary was built from when its caller
    /// gave one, as `takt --version` prints it: `<sha>`, `<sha>-dirty` or `unknown revision`.
    /// The layers (ADR 0101): layer name to revision, each read from the binary or library
    /// that ran, never typed: `takt`, the binary's revision as `binary` names it, when the
    /// caller gave one; `harness`, the harness program's version as it reports it
    /// ([`crate::harness::Harness::revision`]). The model layer is not known when the run
    /// opens: a reader takes it from the run's harness `step` lines, whose `model` is the model
    /// id the harness's stream reported. A layer the run cannot read is absent, never guessed;
    /// the map is written, empty or not, by every run of this takt, and absent on a line
    /// written before it, which a reader reads as "layers not recorded", never as a revision.
    Opened {
        case: String,
        protocol: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        story: Option<String>,
        inputs: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        base: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        binary: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        layers: Option<BTreeMap<String, String>>,
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
    /// reported those; a validator's, the revision it observed. A harness step's also carries
    /// what the harness reported about the call, each absent when it did not state it, never
    /// 0: the model, the cost in dollars, the call's duration and its time in the API in
    /// milliseconds, the turns, and the instant the call started, which precedes the line's
    /// `at`; and the path of its transcript relative to the transcripts directory,
    /// `<repo>/<case>/run-<r>/step-<n>.jsonl`, when takt kept one
    /// ([`crate::harness::keep_transcript`]), or why it kept none (`transcript_refused`, its
    /// paths relative too: a file already at that path, which is never overwritten, or a link
    /// on the way); so no record carries the directory's own path.
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
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cost_usd: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        api_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turns: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        started_at: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transcript: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transcript_refused: Option<String>,
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
    /// The run stopped: why, with the steps taken and the tokens reported, and whether the stop
    /// is a completion (`Completed`), as the binary that stopped knows its own stops: a reader
    /// that does not know the stop's kind takes this word for it. A run always writes it; `None`
    /// writes no field, as a line from before it had one, which a reader scores by the stops it
    /// knows.
    Stop {
        stop: String,
        steps: u64,
        tokens: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        completed: Option<bool>,
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
    /// the reader knows the torn one was skipped. A symbolic link at the path is refused, never
    /// written through ([`Events::open`]).
    pub fn emit(&self, step: u64, event: &Event) -> Result<()> {
        let mut file = self
            .open(
                std::fs::OpenOptions::new()
                    .create(true)
                    .read(true)
                    .append(true),
            )
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

    /// Opens the record with `options`, never through a symbolic link at its path
    /// (`O_NOFOLLOW`): the record is the plain file the kernel wrote, and a link there would
    /// read as, or append to, some other file. The open itself refuses the link, so no check
    /// races it; the refusal says so, and holds for a link that leads nowhere too.
    fn open(&self, options: &mut std::fs::OpenOptions) -> std::io::Result<std::fs::File> {
        use std::os::unix::fs::OpenOptionsExt as _;
        options
            .custom_flags(libc::O_NOFOLLOW)
            .open(&self.path)
            .map_err(|error| {
                if error.raw_os_error() == Some(libc::ELOOP) {
                    std::io::Error::other("a symbolic link, which a run record never is: refused")
                } else {
                    error
                }
            })
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
    /// file and the line. The file is read as bytes, line by line: a line that is not valid
    /// UTF-8 (a write cut inside a multi-byte character) is one more line that does not read,
    /// so one torn write never makes the whole record unreadable. A line's `at` is its first: a
    /// `store_evidence` line an earlier takt wrote carries the store's instant as a second
    /// `at`, which is read as `observed_at`. A symbolic link at the path is refused, never read through ([`Events::open`]).
    pub fn read(&self) -> Result<Vec<serde_json::Value>> {
        Ok(self
            .read_numbered()?
            .into_iter()
            .map(|(_, value)| value)
            .collect())
    }

    /// As [`Events::read`], each line with its number in the file, from 1, counting the
    /// blank and the skipped lines it passes over: the line a report names as the record a
    /// figure came from.
    pub fn read_numbered(&self) -> Result<Vec<(usize, serde_json::Value)>> {
        let mut bytes = Vec::new();
        match self.open(std::fs::OpenOptions::new().read(true)) {
            Ok(mut file) => {
                file.read_to_end(&mut bytes)
                    .with_context(|| format!("read {}", self.path.display()))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", self.path.display()));
            }
        }
        let ended = bytes.ends_with(b"\n");
        let mut lines: Vec<Option<&str>> = bytes
            .split(|byte| *byte == b'\n')
            .map(|line| std::str::from_utf8(line).ok())
            .collect();
        if ended {
            lines.pop();
        }
        let blank = |n: usize| lines[n].is_some_and(|line| line.trim().is_empty());
        let parsed: Vec<Option<serde_json::Value>> =
            lines.iter().map(|line| line.and_then(read_line)).collect();
        let mut read = Vec::new();
        let mut n = 0;
        while n < lines.len() {
            if blank(n) {
                n += 1;
                continue;
            }
            if let Some(value) = &parsed[n] {
                read.push((n + 1, value.clone()));
                n += 1;
                continue;
            }
            // A repair cut short too is one more torn line: the lines that do not read, one
            // after another, are skipped together when the last of them was cut short.
            let mut last = n;
            while last + 1 < lines.len() && parsed[last + 1].is_none() && !blank(last + 1) {
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

/// One line as JSON, its `at` the first the line holds and a later one, when the line holds
/// two, its `observed_at`; none when it does not read.
fn read_line(line: &str) -> Option<serde_json::Value> {
    let mut value: serde_json::Value = serde_json::from_str(line).ok()?;
    if let Ok(Instants(Some(first), Some(last))) = serde_json::from_str(line)
        && let Some(object) = value.as_object_mut()
    {
        object
            .entry("observed_at")
            .or_insert_with(|| last.clone().into());
        object.insert("at".to_owned(), first.into());
    }
    Some(value)
}

/// The first and the last `at` of a line's object, when it holds two or more.
struct Instants(Option<String>, Option<String>);

impl<'de> Deserialize<'de> for Instants {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Instants;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an event line")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Instants, A::Error> {
                let mut ats: Vec<serde_json::Value> = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    if key == "at" {
                        ats.push(map.next_value()?);
                    } else {
                        map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                let text = |value: &serde_json::Value| value.as_str().map(str::to_owned);
                Ok(match ats.as_slice() {
                    [first, .., last] => Instants(text(first), text(last)),
                    _ => Instants(None, None),
                })
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
