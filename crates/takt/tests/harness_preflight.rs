//! story:an-unqualified-harness-stops-before-the-first-session: before its first session, on
//! open and on every resume, takt asks metaharness whether the harness binary it would run
//! qualifies under the run's hermetic mode, with no model call. The binary here is a double that
//! answers `--version` and, asked for a session, calls a model endpoint this test counts
//! ([`preflight_fakes`]); the run is the library's, and the exit code the stop gets is the
//! command line's (`crates/takt-cli/tests/harness_preflight_cli.rs`).

#[path = "support/own_home.rs"]
mod own_home;

mod preflight_fakes;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::{CredentialSource, Kind};
use takt::harness::SessionHarness;
use takt::scan::Scan;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";

fn git(tree: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed when dropped: `tree/` the
/// repository, `bin/` the vendor program's directory.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("preflight-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let scratch = Self(dir);
        let tree = scratch.tree();
        fs::create_dir_all(tree.join("src")).expect("the tree");
        fs::write(tree.join("README.md"), "a story\n").expect("a file");
        fs::write(tree.join("src/lib.rs"), BROKEN).expect("a file");
        git(&tree, &["init", "-q", "-b", "main"]);
        git(&tree, &["add", "-A"]);
        git(&tree, &["commit", "-q", "-m", "start"]);
        scratch
    }

    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn bin(&self) -> PathBuf {
        self.0.join("bin")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(3),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

/// A harness of `kind` that starts the program at `program`, on no login.
fn harness(kind: Kind, program: &Path) -> SessionHarness {
    let mut harness = SessionHarness::of(kind);
    harness.credentials = CredentialSource::None;
    harness.program = program.display().to_string();
    harness.timeout = std::time::Duration::from_secs(60);
    harness
}

/// One run of `id` on `scratch`'s tree, opening the case or resuming it: what it stopped on and
/// what it printed.
fn run(scratch: &Scratch, id: &str, harness: &SessionHarness) -> (anyhow::Result<Stop>, String) {
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("git grep -qE a.[+].b$").expect("a command"),
    )];
    let tree = scratch.tree();
    let mut run = if tree.join(".takt/cases").join(id).is_dir() {
        Run::resume(&tree, options(id), id, harness, &validators).expect("resumes")
    } else {
        Run::open(&tree, options(id), harness, &validators).expect("opens")
    };
    run.scan_with(Scan::None {
        declared_in: tree.with_file_name("run.toml"),
    });
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The `opened` lines of the case `id`, as the run recorded them.
fn opened(scratch: &Scratch, id: &str) -> Vec<serde_json::Value> {
    let events = scratch
        .tree()
        .join(".takt/cases")
        .join(id)
        .join("events.jsonl");
    fs::read_to_string(&events)
        .expect("the case's record")
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|line| line["event"] == "opened")
        .collect()
}

/// A stop that is the preflight's, or a failure that prints what the run said.
fn unqualified(
    stop: anyhow::Result<Stop>,
    said: &str,
) -> (String, Option<String>, Vec<String>, Vec<String>, String) {
    match stop {
        Ok(stop @ Stop::HarnessUnqualified { .. }) => {
            let line = stop.to_string();
            let Stop::HarnessUnqualified {
                kind,
                version,
                rows,
                qualified,
            } = stop
            else {
                unreachable!()
            };
            (kind, version, rows, qualified, line)
        }
        other => panic!("expected HarnessUnqualified, got {other:?}\n{said}"),
    }
}

/// A fake `claude` reporting 2.1.311, a version metaharness pins no memory canary for: the run
/// stops `HarnessUnqualified` naming H11 and the versions that qualify, the model endpoint was
/// sent no request, and the case is still there to resume.
#[test]
fn an_unqualified_claude_stops_before_a_model_call() {
    let scratch = Scratch::new("claude-unqualified");
    let claude = preflight_fakes::install(&scratch.bin(), "claude", "2.1.311");
    let endpoint = preflight_fakes::Endpoint::serving(&claude);
    let (stop, said) = run(
        &scratch,
        "preflight-claude",
        &harness(Kind::Claude, &claude),
    );
    let (kind, version, rows, qualified, line) = unqualified(stop, &said);
    assert_eq!(kind, "claude");
    assert_eq!(version.as_deref(), Some("2.1.311"));
    assert!(rows.iter().any(|row| row.starts_with("H11 ")), "{rows:?}");
    assert_eq!(qualified, ["2.1.304", "2.1.305", "2.1.309"]);
    for named in [
        "2.1.311",
        "H11",
        "2.1.304, 2.1.305, 2.1.309",
        "--harness-program",
    ] {
        assert!(line.contains(named), "the stop line names {named}: {line}");
    }
    assert_eq!(endpoint.requests(), 0, "no model call\n{said}");
    assert!(
        scratch.tree().join(".takt/cases/preflight-claude").is_dir(),
        "the case stays, to resume"
    );
}

/// A case opened on a qualified fake (2.1.309) and resumed after the fake reports 2.1.311 stops
/// before its next session: the endpoint is sent no request by the resume.
#[test]
fn a_resume_rechecks_the_binary() {
    let scratch = Scratch::new("resume");
    let claude = preflight_fakes::install(&scratch.bin(), "claude", "2.1.309");
    let endpoint = preflight_fakes::Endpoint::serving(&claude);
    let id = "preflight-resume";
    let (first, said) = run(&scratch, id, &harness(Kind::Claude, &claude));
    assert!(
        !matches!(first, Ok(Stop::HarnessUnqualified { .. })),
        "a qualified binary runs: {first:?}\n{said}"
    );
    let after_first = endpoint.requests();
    assert!(
        after_first >= 1,
        "the first run's session called the model\n{said}"
    );
    preflight_fakes::set_version(&claude, "2.1.311");
    let (resumed, said) = run(&scratch, id, &harness(Kind::Claude, &claude));
    let (_, version, rows, _, _) = unqualified(resumed, &said);
    assert_eq!(version.as_deref(), Some("2.1.311"));
    assert!(rows.iter().any(|row| row.starts_with("H11 ")), "{rows:?}");
    assert_eq!(
        endpoint.requests(),
        after_first,
        "the resume made no model call\n{said}"
    );
}

/// A Codex binary off the pin: its only row that is not `ok` is H9, which is advisory without
/// `strict-version`. The run goes on (the session was started and called the model) and the
/// opening record names the row as advisory.
#[test]
fn an_advisory_row_does_not_stop() {
    let scratch = Scratch::new("advisory");
    let codex = preflight_fakes::install(&scratch.bin(), "codex", "0.143.0");
    let endpoint = preflight_fakes::Endpoint::serving(&codex);
    let id = "preflight-advisory";
    let (stop, said) = run(&scratch, id, &harness(Kind::Codex, &codex));
    assert!(
        !matches!(stop, Ok(Stop::HarnessUnqualified { .. })),
        "H9 alone does not stop a run: {stop:?}\n{said}"
    );
    assert!(endpoint.requests() >= 1, "the session started\n{said}");
    let lines = opened(&scratch, id);
    let preflight = &lines.last().expect("an opened line")["preflight"];
    assert_eq!(preflight["kind"], "codex", "{preflight}");
    assert_eq!(preflight["version"], "0.143.0", "{preflight}");
    let advisory: Vec<&str> = preflight["advisory"]
        .as_array()
        .expect("the advisory rows")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        advisory.iter().any(|row| row.starts_with("H9 ")),
        "H9 is named as advisory: {advisory:?}"
    );
}

/// The same binary under `strict-version` stops before the first model call, naming H9.
#[test]
fn strict_version_makes_h9_stop_first() {
    let scratch = Scratch::new("strict");
    let codex = preflight_fakes::install(&scratch.bin(), "codex", "0.143.0");
    let endpoint = preflight_fakes::Endpoint::serving(&codex);
    let mut harness = harness(Kind::Codex, &codex);
    harness.strict_version = true;
    let (stop, said) = run(&scratch, "preflight-strict", &harness);
    let (kind, version, rows, _, line) = unqualified(stop, &said);
    assert_eq!(kind, "codex");
    assert_eq!(version.as_deref(), Some("0.143.0"));
    assert!(rows.iter().any(|row| row.starts_with("H9 ")), "{rows:?}");
    assert!(line.contains("H9"), "{line}");
    assert_eq!(endpoint.requests(), 0, "no model call\n{said}");
}
