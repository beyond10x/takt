//! story:takt-view-renders-runs through the binary: `takt view <dir> --output <file>` reads
//! every `takt-trace/1` file and step transcript under `<dir>` and writes one self-contained
//! HTML file, one lane per run, each operation a mark naming its step, action and outcome, a
//! harness step's mark carrying its transcript; a directory with no trace is refused.
//!
//! The fixtures are written here by hand in the formats takt writes: two traces, one beside
//! its case's `events.jsonl` (whose step line names the transcript, relative to the
//! transcripts directory), one alone (its transcript found by the layout
//! `<repo-key>/<case>/run-<r>/step-<n>.jsonl`). Nothing runs a model or reaches a network.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("view-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("the parent is made");
    fs::write(path, text).expect("a fixture file");
}

const TRACE_A: &str = r#"{"type":"header","format":"takt-trace/1","run":"trial-a","case":"case-a","arm":"takt","harness":"claude-code","model":"m-1","outcome":"Completed","invocations":1,"resumes":0,"steps":3,"tokens":{},"t0":"2026-10-20T10:00:00Z","t_end":"2026-10-20T10:01:40Z","wall_s":100.0,"stop":"Completed","sources":["events.jsonl"],"notes":[]}
{"type":"op","seq":1,"step":1,"actor":"validator","intent":"verify","kind":"test","action":"tests.run","name":"takt.tests-run","t_start":0.0,"t_end":10.0,"outcome":"fail","evidence":"observed"}
{"type":"op","seq":2,"step":2,"actor":"harness","intent":"change","kind":"edit","action":"implementation.edit","name":"claude-code","t_start":10.0,"t_end":70.0,"tokens_sum":1200,"outcome":"changed","paths":["src/lib.rs"],"evidence":"declared"}
{"type":"op","seq":3,"step":3,"actor":"validator","intent":"verify","kind":"test","action":"tests.run","name":"takt.tests-run","t_start":70.0,"t_end":95.0,"outcome":"pass","evidence":"observed"}
{"type":"op","seq":4,"step":3,"actor":"takt","intent":"coordinate","kind":"other","t_start":100.0,"t_end":100.0,"outcome":"Completed","evidence":"observed"}
"#;

const EVENTS_A: &str = r#"{"at":"2026-10-20T10:00:00Z","step":0,"event":"opened","case":"case-a","protocol":"software.change/1"}
{"at":"2026-10-20T10:00:10Z","step":1,"event":"step","by":"validator","name":"takt.tests-run","action":"tests.run","summary":"the test command exited 101: fail"}
{"at":"2026-10-20T10:01:10Z","step":2,"event":"step","by":"harness","name":"claude-code","action":"implementation.edit","summary":"edited","tokens":1200,"transcript":"0123456789abcdef/case-a/run-1/step-2.jsonl"}
{"at":"2026-10-20T10:01:35Z","step":3,"event":"step","by":"validator","name":"takt.tests-run","action":"tests.run","summary":"the test command exited 0: pass"}
{"at":"2026-10-20T10:01:40Z","step":3,"event":"stop","stop":"Completed"}
"#;

const TRACE_B: &str = r#"{"type":"header","format":"takt-trace/1","run":"trial-b","case":"case-b","arm":"takt","harness":"claude-code","outcome":"NoProgress","invocations":1,"resumes":0,"steps":1,"tokens":{},"t0":"2026-10-20T11:00:00Z","t_end":"2026-10-20T11:00:50Z","wall_s":50.0,"stop":"NoProgress","sources":["events.jsonl"],"notes":[]}
{"type":"op","seq":1,"step":1,"actor":"harness","intent":"inspect","kind":"read","action":"repository.inspect","name":"claude-code","t_start":0.0,"t_end":45.0,"outcome":"unchanged","evidence":"declared"}
{"type":"op","seq":2,"step":1,"actor":"takt","intent":"coordinate","kind":"no_progress","t_start":50.0,"t_end":50.0,"outcome":"NoProgress","evidence":"observed"}
"#;

const TRANSCRIPT_A: &str = "{\"type\":\"takt.prompt\",\"text\":\"edit the implementation\"}\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"TRANSCRIPT-A-MARKER edited src/lib.rs\"}]}}\n";

const TRANSCRIPT_B: &str = "{\"type\":\"takt.prompt\",\"text\":\"inspect\"}\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"TRANSCRIPT-B-MARKER read the tree\"}]}}\n";

/// Two runs: case A's trace beside its record, whose harness step names its transcript; case
/// B's trace alone, its transcript in the transcripts layout.
fn fixture(root: &Path) -> PathBuf {
    let runs = root.join("runs");
    write(&runs.join("cases/case-a/trace.jsonl"), TRACE_A);
    write(&runs.join("cases/case-a/events.jsonl"), EVENTS_A);
    write(&runs.join("traces/case-b.trace.jsonl"), TRACE_B);
    write(
        &runs.join("transcripts/0123456789abcdef/case-a/run-1/step-2.jsonl"),
        TRANSCRIPT_A,
    );
    write(
        &runs.join("transcripts/fedcba9876543210/case-b/run-1/step-1.jsonl"),
        TRANSCRIPT_B,
    );
    runs
}

/// The data the page embeds: the JSON in its `takt-view-data` script element.
fn data(html: &str) -> Value {
    let start_tag = "<script id=\"takt-view-data\" type=\"application/json\">";
    let start = html.find(start_tag).expect("the page embeds its data") + start_tag.len();
    let end = start
        + html[start..]
            .find("</script>")
            .expect("the data element ends");
    serde_json::from_str(&html[start..end]).expect("the embedded data is JSON")
}

fn render(root: &Path) -> (String, PathBuf) {
    let runs = fixture(root);
    let output = root.join("out/view.html");
    let (code, stdout, stderr) = takt(&[
        "view",
        &runs.display().to_string(),
        "--output",
        &output.display().to_string(),
    ]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("view.html"), "{stdout}");
    (
        fs::read_to_string(&output).expect("the page is written"),
        output,
    )
}

/// Acceptance 1: one HTML file, self-contained: no script or stylesheet is loaded from
/// anywhere, so nothing leaves the page.
#[test]
fn view_writes_one_self_contained_html_file() {
    let root = scratch("offline");
    let (html, output) = render(&root);
    assert!(
        html.starts_with("<!doctype html>"),
        "{}",
        &html[..80.min(html.len())]
    );
    let lower = html.to_ascii_lowercase();
    for tag in lower.split('<').skip(1) {
        let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
        if tag.starts_with("script") {
            assert!(!tag.contains("src="), "a script with a source: <{tag}>");
        }
        if tag.starts_with("link") {
            assert!(
                !tag.contains("stylesheet")
                    && !tag.contains("http://")
                    && !tag.contains("https://"),
                "a linked resource: <{tag}>"
            );
        }
    }
    assert!(!lower.contains("@import"), "a stylesheet import");
    assert!(
        !lower.contains("url(http"),
        "a stylesheet fetches a resource"
    );
    assert!(lower.contains("<style>"), "the stylesheet is inline");
    assert!(html.contains("var Vue="), "Vue is inline");
    let entries: Vec<_> = fs::read_dir(output.parent().expect("a parent"))
        .expect("the output directory reads")
        .collect();
    assert_eq!(entries.len(), 1, "one file is written");
}

/// Acceptance 2: one lane per run on one time axis; each operation is a mark at its time,
/// naming its step, action and outcome.
#[test]
fn view_has_one_lane_per_run_and_a_named_mark_per_operation() {
    let root = scratch("lanes");
    let (html, _) = render(&root);
    let data = data(&html);
    let runs = data["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 2, "{data}");
    let a = runs
        .iter()
        .find(|run| run["run"] == "trial-a")
        .expect("trial-a has a lane");
    let b = runs
        .iter()
        .find(|run| run["run"] == "trial-b")
        .expect("trial-b has a lane");
    assert_eq!(a["ops"].as_array().expect("ops").len(), 4);
    assert_eq!(b["ops"].as_array().expect("ops").len(), 2);
    let edit = &a["ops"][1];
    assert_eq!(edit["step"], 2);
    assert_eq!(edit["action"], "implementation.edit");
    assert_eq!(edit["outcome"], "changed");
    assert_eq!(edit["t_start"], 10.0);
    assert_eq!(edit["t_end"], 70.0);
    assert_eq!(edit["mark"], "step 2 · implementation.edit · changed");
    assert_eq!(a["ops"][3]["mark"], "step 3 · other · Completed");
    assert_eq!(b["t0"], "2026-10-20T11:00:00Z");
}

/// Acceptance 3: a click on a mark opens its step's transcript: each harness step's mark
/// carries the text of the transcript file its step line names.
#[test]
fn view_carries_each_harness_steps_transcript() {
    let root = scratch("transcripts");
    let (html, _) = render(&root);
    let data = data(&html);
    let transcripts = data["transcripts"].as_array().expect("transcripts");
    let text_of = |op: &Value| -> String {
        let index = op["transcript"]
            .as_u64()
            .unwrap_or_else(|| panic!("a transcript for {op}"));
        transcripts[usize::try_from(index).expect("an index")]["text"]
            .as_str()
            .expect("its text")
            .to_owned()
    };
    let runs = data["runs"].as_array().expect("runs");
    let a = runs.iter().find(|run| run["run"] == "trial-a").expect("a");
    let b = runs.iter().find(|run| run["run"] == "trial-b").expect("b");
    assert!(
        text_of(&a["ops"][1]).contains("TRANSCRIPT-A-MARKER"),
        "{data}"
    );
    assert!(
        text_of(&b["ops"][0]).contains("TRANSCRIPT-B-MARKER"),
        "{data}"
    );
    assert!(
        a["ops"][0]["transcript"].is_null(),
        "a validator step has none"
    );
    assert!(html.contains("TRANSCRIPT-A-MARKER"));
    assert!(html.contains("TRANSCRIPT-B-MARKER"));
}

/// The transcript is the file the case's step line names: a second repository's transcript
/// of a case with the same id, at the same run and step, is not taken for it.
#[test]
fn view_takes_the_transcript_the_step_line_names() {
    let root = scratch("named");
    let runs = fixture(&root);
    write(
        &runs.join("transcripts/ffff000000000000/case-a/run-1/step-2.jsonl"),
        "{\"type\":\"takt.prompt\",\"text\":\"DECOY-MARKER\"}\n",
    );
    let output = root.join("view.html");
    let (code, _, stderr) = takt(&[
        "view",
        &runs.display().to_string(),
        "--output",
        &output.display().to_string(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    let data = data(&fs::read_to_string(&output).expect("the page"));
    let a = data["runs"]
        .as_array()
        .expect("runs")
        .iter()
        .find(|run| run["run"] == "trial-a")
        .expect("a");
    let index =
        usize::try_from(a["ops"][1]["transcript"].as_u64().expect("an index")).expect("an index");
    let transcript = &data["transcripts"][index];
    assert!(
        transcript["text"]
            .as_str()
            .is_some_and(|text| text.contains("TRANSCRIPT-A-MARKER")),
        "{transcript}"
    );
    assert_eq!(
        transcript["path"],
        "transcripts/0123456789abcdef/case-a/run-1/step-2.jsonl"
    );
}

/// A transcript past the cap is embedded as its head and its tail, the cut marked.
#[test]
fn view_embeds_the_head_and_tail_of_a_large_transcript() {
    let root = scratch("large");
    let runs = fixture(&root);
    let mut big = String::from("{\"type\":\"takt.prompt\",\"text\":\"HEAD-MARKER\"}\n");
    let filler = format!(
        "{{\"type\":\"filler\",\"text\":\"{}\"}}\n",
        "x".repeat(1000)
    );
    while big.len() < 2 << 20 {
        big.push_str(&filler);
    }
    big.push_str("{\"type\":\"result\",\"result\":\"TAIL-MARKER\"}\n");
    write(
        &runs.join("transcripts/fedcba9876543210/case-b/run-1/step-1.jsonl"),
        &big,
    );
    let output = root.join("view.html");
    let (code, _, stderr) = takt(&[
        "view",
        &runs.display().to_string(),
        "--output",
        &output.display().to_string(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    let html = fs::read_to_string(&output).expect("the page");
    assert!(
        html.len() < big.len(),
        "the page holds less than the whole transcript"
    );
    let data = data(&html);
    let cut = data["transcripts"]
        .as_array()
        .expect("transcripts")
        .iter()
        .find(|t| {
            t["text"]
                .as_str()
                .is_some_and(|text| text.contains("HEAD-MARKER"))
        })
        .expect("the large transcript");
    let text = cut["text"].as_str().expect("text");
    assert!(text.contains("TAIL-MARKER"), "the tail is kept");
    assert!(
        cut["omitted_bytes"].as_u64().is_some_and(|n| n > 0),
        "{cut}"
    );
}

/// Acceptance 4: a directory with no trace is refused, exit 1, naming the directory, and
/// nothing is written.
#[test]
fn view_refuses_a_directory_with_no_trace() {
    let root = scratch("empty");
    let dir = root.join("nothing-here");
    write(&dir.join("notes.jsonl"), "{\"type\":\"other\"}\n");
    write(&dir.join("events.jsonl"), EVENTS_A);
    let output = root.join("view.html");
    let (code, stdout, stderr) = takt(&[
        "view",
        &dir.display().to_string(),
        "--output",
        &output.display().to_string(),
    ]);
    assert_eq!(code, 1, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stderr.contains(&dir.display().to_string()), "{stderr}");
    assert!(stderr.contains("takt-trace/1"), "{stderr}");
    assert!(!output.exists(), "nothing is written");
}

/// A directory that is not there is refused the same way.
#[test]
fn view_refuses_a_missing_directory() {
    let root = scratch("missing");
    let dir = root.join("absent");
    let output = root.join("view.html");
    let (code, _, stderr) = takt(&[
        "view",
        &dir.display().to_string(),
        "--output",
        &output.display().to_string(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains(&dir.display().to_string()), "{stderr}");
    assert!(!output.exists());
}
