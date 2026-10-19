//! The viewer: the runs under a directory as one offline HTML page (`takt view`). Every
//! `takt-trace/1` file under the directory is a run and a lane on one time axis; each of its
//! operations is a mark at its time naming its step, action and outcome; a harness step's mark
//! opens that step's transcript. The page holds everything it shows: the stylesheet, the
//! script, Vue and the data are inline, so it opens with no server and no request leaves it.
//!
//! A step's transcript is the file `takt run` kept as
//! `<transcripts>/<repo-key>/<case>/run-<r>/step-<n>.jsonl` ([`crate::harness::keep_transcript`]).
//! It is found, under the directory, by the first of: the operation's own `transcript` field,
//! when the trace line carries one; the `transcript` the case's step line names in the
//! `events.jsonl` beside the trace or of the same case under the directory (a path relative to
//! the transcripts directory); the layout `<case>/run-<r>/step-<n>.jsonl`, `r` counted from the
//! runs the trace shows, when one file alone matches. A transcript past [`TRANSCRIPT_EMBED_CAP`]
//! is embedded as its head and its tail, the bytes left out counted. Every text the page
//! carries passes [`crate::trace::escape`], so it names the home directory as `~`.
//!
//! The assets live in `viewer/` at the repository's root and are compiled into the binary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::events::EVENTS_FILE;
use crate::trace::{FORMAT, Trace, escape};

const TEMPLATE: &str = include_str!("../../../viewer/index.html");
const STYLE: &str = include_str!("../../../viewer/style.css");
const APP: &str = include_str!("../../../viewer/app.js");
const VUE: &str = include_str!("../../../viewer/vendor/vue.global.prod.js");

/// The most bytes of one transcript the page embeds: past it, the first and the last half.
pub const TRANSCRIPT_EMBED_CAP: usize = 256 << 10;

/// Directories never walked: build output and version control.
const SKIPPED_DIRS: [&str; 3] = [".git", "target", "node_modules"];

/// What `render` read and wrote into the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// The page.
    pub html: String,
    /// The runs, one per trace.
    pub runs: usize,
    /// The operations across the runs.
    pub ops: usize,
    /// The harness steps whose transcript the page carries, and those it found none for.
    pub transcripts: usize,
    pub missing: usize,
}

/// One trace found under the directory.
struct Found {
    path: PathBuf,
    trace: Trace,
    /// The op lines as written, for fields a later trace may carry ([`Trace`] drops them).
    raw_ops: Vec<Value>,
}

/// What a walk under the directory found.
#[derive(Default)]
struct Walk {
    traces: Vec<PathBuf>,
    events: Vec<PathBuf>,
    transcripts: Vec<PathBuf>,
}

/// Whether the file at `path` begins with a `takt-trace/1` header line.
fn is_trace(path: &Path) -> bool {
    use std::io::Read as _;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = Vec::with_capacity(512);
    if file.take(512).read_to_end(&mut head).is_err() {
        return false;
    }
    let head = String::from_utf8_lossy(&head);
    let first = head.lines().next().unwrap_or("");
    first.starts_with("{\"type\":\"header\"") && first.contains(&format!("\"format\":\"{FORMAT}\""))
}

/// Whether `path` is laid out as a step transcript: `run-<r>/step-<n>.jsonl`.
fn transcript_layout(path: &Path) -> Option<(String, u64, u64)> {
    let file = path.file_name()?.to_str()?;
    let step = file
        .strip_prefix("step-")?
        .strip_suffix(".jsonl")?
        .parse()
        .ok()?;
    let run_dir = path.parent()?;
    let run = run_dir
        .file_name()?
        .to_str()?
        .strip_prefix("run-")?
        .parse()
        .ok()?;
    let case = run_dir.parent()?.file_name()?.to_str()?.to_owned();
    Some((case, run, step))
}

/// Walks `dir`, never through a link and never into [`SKIPPED_DIRS`], in name order.
fn walk(dir: &Path, found: &mut Walk) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .filter_map(std::result::Result::ok)
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            let name = entry.file_name();
            if !SKIPPED_DIRS.iter().any(|skip| name == *skip) {
                walk(&path, found)?;
            }
        } else if kind.is_file() {
            if transcript_layout(&path).is_some() {
                found.transcripts.push(path);
            } else if entry.file_name() == EVENTS_FILE {
                found.events.push(path);
            } else if is_trace(&path) {
                found.traces.push(path);
            }
        }
    }
    Ok(())
}

/// The step lines of a record: per line, its run (from 1, one per `opened`), its step, its
/// action and the transcript it names.
struct RecordStep {
    run: u64,
    step: u64,
    action: String,
    transcript: Option<String>,
}

/// A record's case id (its first `opened` line's, else its directory's name) and its steps.
fn read_record(path: &Path) -> Option<(String, Vec<RecordStep>)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut case = None;
    let mut run = 0;
    let mut steps = Vec::new();
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match line["event"].as_str() {
            Some("opened") => {
                run += 1;
                if case.is_none() {
                    case = line["case"].as_str().map(str::to_owned);
                }
            }
            Some("step") => steps.push(RecordStep {
                run: run.max(1),
                step: line["step"].as_u64().unwrap_or(0),
                action: line["action"].as_str().unwrap_or("").to_owned(),
                transcript: line["transcript"].as_str().map(str::to_owned),
            }),
            _ => {}
        }
    }
    let case = case.or_else(|| {
        path.parent()?
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    })?;
    Some((case, steps))
}

/// The transcript files embedded so far, each once.
#[derive(Default)]
struct Transcripts {
    index: BTreeMap<PathBuf, usize>,
    entries: Vec<Value>,
}

impl Transcripts {
    /// The index of the transcript at `path` in the page, reading it the first time.
    fn add(&mut self, dir: &Path, path: &Path) -> Option<usize> {
        if let Some(index) = self.index.get(path) {
            return Some(*index);
        }
        let bytes = std::fs::read(path).ok()?;
        let (text, omitted) = head_and_tail(&bytes, TRANSCRIPT_EMBED_CAP);
        let shown = path.strip_prefix(dir).unwrap_or(path);
        let index = self.entries.len();
        self.entries.push(json!({
            "path": escape(&shown.display().to_string()),
            "bytes": bytes.len(),
            "omitted_bytes": omitted,
            "text": escape_text(&text),
        }));
        self.index.insert(path.to_path_buf(), index);
        Some(index)
    }
}

/// `text` with the home directory as `~` ([`escape`]), line by line so the lines stay.
fn escape_text(text: &str) -> String {
    text.split('\n').map(escape).collect::<Vec<_>>().join("\n")
}

/// `bytes` as text, whole when within `cap`; past it, its first and its last `cap / 2` bytes,
/// each cut at a line's end, with a line naming the bytes left out between them. The bytes
/// left out, 0 when none.
fn head_and_tail(bytes: &[u8], cap: usize) -> (String, usize) {
    if bytes.len() <= cap {
        return (String::from_utf8_lossy(bytes).into_owned(), 0);
    }
    let half = cap / 2;
    let head_end = bytes[..half]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(half, |at| at + 1);
    let tail_from = bytes.len() - half;
    let tail_start = bytes[tail_from..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(tail_from, |at| tail_from + at + 1);
    let omitted = tail_start - head_end;
    let mut text = String::from_utf8_lossy(&bytes[..head_end]).into_owned();
    text.push_str(&format!(
        "{{\"type\":\"takt.view.cut\",\"omitted_bytes\":{omitted}}}\n"
    ));
    text.push_str(&String::from_utf8_lossy(&bytes[tail_start..]));
    (text, omitted)
}

/// A mark's name: `step <n> · <action> · <outcome>`, the kind standing in for an action an
/// operation has none of, and `-` for an outcome it has none of.
fn mark(op: &crate::trace::Op) -> String {
    format!(
        "step {} · {} · {}",
        op.step,
        op.action.as_deref().unwrap_or(op.kind.name()),
        op.outcome.as_deref().unwrap_or("-")
    )
}

/// The run each operation of `trace` belongs to, from 1: a new run begins at a `resume`
/// operation, or where a step's number falls below the last step's of the run before it.
fn runs_of(trace: &Trace) -> Vec<u64> {
    let mut run = 1;
    let mut last_step = 0;
    trace
        .ops
        .iter()
        .map(|op| {
            if op.kind == crate::trace::Kind::Resume {
                run += 1;
                last_step = 0;
            } else if op.is_step() {
                if op.step < last_step {
                    run += 1;
                }
                last_step = op.step;
            }
            run
        })
        .collect()
}

/// Reads every `takt-trace/1` file under `dir` and the step transcripts their steps name, and
/// renders the page. A directory that does not read, or holds no trace, is refused naming it;
/// a file that begins as a trace and does not read as one is refused naming the file.
pub fn render(dir: &Path) -> Result<Rendered> {
    if !dir.is_dir() {
        bail!(
            "{} is not a directory: no {FORMAT} trace to view",
            dir.display()
        );
    }
    let mut walked = Walk::default();
    walk(dir, &mut walked)?;
    if walked.traces.is_empty() {
        bail!(
            "{} holds no {FORMAT} trace (a file whose first line is its header): nothing to view, nothing written",
            dir.display()
        );
    }
    let mut found = Vec::new();
    for path in &walked.traces {
        let trace = Trace::load(path)?;
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let raw_ops = text
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|line| line["type"] == "op")
            .collect();
        found.push(Found {
            path: path.clone(),
            trace,
            raw_ops,
        });
    }
    let records: Vec<(PathBuf, String, Vec<RecordStep>)> = walked
        .events
        .iter()
        .filter_map(|path| read_record(path).map(|(case, steps)| (path.clone(), case, steps)))
        .collect();
    let mut layout: BTreeMap<(String, u64, u64), Vec<&PathBuf>> = BTreeMap::new();
    for path in &walked.transcripts {
        if let Some(key) = transcript_layout(path) {
            layout.entry(key).or_default().push(path);
        }
    }
    let named = |relative: &str| -> Option<&PathBuf> {
        let relative = Path::new(relative);
        walked
            .transcripts
            .iter()
            .find(|path| path.ends_with(relative))
    };

    let mut transcripts = Transcripts::default();
    let mut runs = Vec::new();
    let (mut ops_total, mut carried, mut missing) = (0, 0, 0);
    for found in &found {
        let header = &found.trace.header;
        let beside = found.path.parent().map(|parent| parent.join(EVENTS_FILE));
        let record = records
            .iter()
            .find(|(path, _, _)| Some(path) == beside.as_ref())
            .or_else(|| records.iter().find(|(_, case, _)| *case == header.case))
            .map(|(_, _, steps)| steps);
        let run_of = runs_of(&found.trace);
        let mut step_index = 0;
        let mut ops = Vec::new();
        for (n, op) in found.trace.ops.iter().enumerate() {
            let mut transcript = None;
            if op.is_step() {
                let line = record.and_then(|steps| steps.get(step_index));
                step_index += 1;
                let own = found
                    .raw_ops
                    .get(n)
                    .and_then(|raw| raw["transcript"].as_str())
                    .and_then(named);
                let from_record = line
                    .filter(|line| {
                        line.step == op.step
                            && op.action.as_deref() == Some(escape(&line.action).as_str())
                    })
                    .and_then(|line| line.transcript.as_deref())
                    .and_then(named);
                let run = line.map_or(run_of[n], |line| line.run);
                let by_layout = || {
                    layout
                        .get(&(header.case.clone(), run, op.step))
                        .filter(|paths| paths.len() == 1)
                        .map(|paths| paths[0])
                };
                let path = own.or(from_record).or_else(|| {
                    (op.actor == crate::trace::Actor::Harness)
                        .then(by_layout)
                        .flatten()
                });
                transcript = path.and_then(|path| transcripts.add(dir, path));
                if op.actor == crate::trace::Actor::Harness {
                    if transcript.is_some() {
                        carried += 1;
                    } else {
                        missing += 1;
                    }
                }
            }
            let mut value = serde_json::to_value(op).context("an operation as JSON")?;
            value["mark"] = json!(mark(op));
            value["run"] = json!(run_of[n]);
            value["transcript"] = json!(transcript);
            ops.push(value);
        }
        ops_total += ops.len();
        let shown = found.path.strip_prefix(dir).unwrap_or(&found.path);
        let mut run = serde_json::to_value(header).context("a header as JSON")?;
        run["source"] = json!(escape(&shown.display().to_string()));
        run["ops"] = json!(ops);
        runs.push(run);
    }
    let data = json!({
        "format": "takt-view/1",
        "trace_format": FORMAT,
        "dir": escape(&dir.display().to_string()),
        "runs": runs,
        "transcripts": transcripts.entries,
    });
    let html = page(&data)?;
    Ok(Rendered {
        html,
        runs: found.len(),
        ops: ops_total,
        transcripts: carried,
        missing,
    })
}

/// `data` as JSON safe inside a `<script>` element: no `<`, `>` or `&` as such.
fn script_json(data: &Value) -> Result<String> {
    let text = serde_json::to_string(data).context("the page's data as JSON")?;
    Ok(text
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026"))
}

/// The page: the template with the stylesheet, the data, Vue and the script put in place, each
/// placeholder replaced once, in the template's order, so nothing put in is read again.
fn page(data: &Value) -> Result<String> {
    let data = script_json(data)?;
    let parts: [(&str, &str); 4] = [
        ("__STYLE__", STYLE),
        ("__DATA__", &data),
        ("__VUE__", VUE),
        ("__APP__", APP),
    ];
    let mut out = String::with_capacity(TEMPLATE.len() + VUE.len() + data.len() + STYLE.len());
    let mut rest = TEMPLATE;
    for (marker, text) in parts {
        let Some(at) = rest.find(marker) else {
            bail!("the viewer's template has no {marker}");
        };
        if text.contains("</script") || text.contains("</style") {
            bail!("the viewer's {marker} would end its element early");
        }
        out.push_str(&rest[..at]);
        out.push_str(text);
        rest = &rest[at + marker.len()..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Renders the runs under `dir` and writes the page to `output`, readable by its owner only
/// (`0600`): a transcript carries what the model read. Nothing is written when rendering is
/// refused.
pub fn write(dir: &Path, output: &Path) -> Result<Rendered> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let rendered = render(dir)?;
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(output)
        .with_context(|| format!("write {}", output.display()))?;
    file.write_all(rendered.html.as_bytes())
        .with_context(|| format!("write {}", output.display()))?;
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transcript_within_the_cap_is_whole() {
        assert_eq!(head_and_tail(b"a\nb\n", 10), ("a\nb\n".to_owned(), 0));
    }

    #[test]
    fn a_transcript_past_the_cap_keeps_whole_lines_of_its_head_and_tail() {
        let (text, omitted) = head_and_tail(b"aa\nbb\ncc\ndd\nee\n", 8);
        assert_eq!(
            text,
            "aa\n{\"type\":\"takt.view.cut\",\"omitted_bytes\":9}\nee\n"
        );
        assert_eq!(omitted, 9);
    }

    #[test]
    fn the_data_cannot_end_its_script_element() {
        let text = script_json(&json!({"t": "</script><b>&"})).expect("json");
        assert!(!text.contains('<') && !text.contains('>') && !text.contains('&'));
        let back: Value = serde_json::from_str(&text).expect("still JSON");
        assert_eq!(back["t"], "</script><b>&");
    }

    #[test]
    fn the_layout_names_case_run_and_step() {
        assert_eq!(
            transcript_layout(Path::new("/x/key/case-a/run-2/step-7.jsonl")),
            Some(("case-a".to_owned(), 2, 7))
        );
        assert_eq!(transcript_layout(Path::new("/x/case-a/step-7.jsonl")), None);
    }
}
