//! story:a-run-can-decline through the binary: a judgement step names `decline` instead of an
//! offered action, changing no file, its reason the reply's summary; the run records a `decline`
//! line and stops `Declined (<reason>)`, exit 9, apart from a stuck run's `NoProgress`, exit 3. A
//! harness double on the program path stands in for Claude Code: it counts its calls, keeps the
//! arguments of its last call and replies as its mode file says; nothing reaches a network or a
//! model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The reason the double gives when it declines.
const REASON: &str = "AGENTS.md forbids weakening check.sh";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// How the double replies, read from its mode file at each call.
#[derive(Clone, Copy)]
enum Mode {
    /// Names `decline` with [`REASON`], changing no file.
    Decline,
    /// Writes `work.txt` in its copy, which it proposes, and names `decline`.
    DeclineEditing,
    /// Writes `work.txt` in the repository itself, outside its copy, so it proposes no file, and
    /// names `decline`: the step changed a file of the tree.
    DeclineInTheTree,
    /// Names `implementation.edit`, changing no file.
    Idle,
}

impl Mode {
    fn word(self) -> &'static str {
        match self {
            Mode::Decline => "decline",
            Mode::DeclineEditing => "decline-editing",
            Mode::DeclineInTheTree => "decline-in-the-tree",
            Mode::Idle => "idle",
        }
    }
}

/// The scratch world: a repository with the fixture store, its one story `proposed`, scenario 1's
/// code and tests, a manifest and a gates document, scenario 1 declared on the commit that holds
/// them, no origin; a `claude` double in `bin` that appends a line to `calls`, writes its
/// arguments to `arguments` and replies as `mode` says.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    calls: PathBuf,
    arguments: PathBuf,
    mode: PathBuf,
    state: PathBuf,
}

fn world(name: &str, mode: Mode) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("decline-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    copy_dir(&Path::new(FIXTURES).join("scenario-1/base"), &repo);
    for script in ["check.sh", "todo.py"] {
        fs::set_permissions(repo.join(script), fs::Permissions::from_mode(0o755))
            .expect("executable");
    }
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(repo.join(".gitignore"), "__pycache__/\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration is written");
    git(&repo, &["add", ".engineering/scenarios.conf"]);
    git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let calls = outer.join("calls");
    let arguments = outer.join("arguments");
    let mode_file = outer.join("mode");
    fs::write(&mode_file, mode.word()).expect("the mode is written");
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    let path = bin.join("claude");
    fs::write(
        &path,
        format!(
            r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
echo call >> '{calls}'
printf '%s\n' "$@" > '{arguments}'
action=decline
summary='{REASON}'
case $(cat '{mode}') in
  decline-editing) echo "the work" >> work.txt ;;
  decline-in-the-tree) echo "the work" >> '{repo}/work.txt' ;;
  idle) action=implementation.edit; summary='nothing to change' ;;
esac
printf '{{"result":"{{\\"action\\": \\"%s\\", \\"summary\\": \\"%s\\"}}","usage":{{"input_tokens":1}}}}' "$action" "$summary"
"#,
            calls = calls.display(),
            arguments = arguments.display(),
            mode = mode_file.display(),
            repo = repo.display(),
        ),
    )
    .expect("the double is written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        calls,
        arguments,
        mode: mode_file,
        state,
    }
}

impl World {
    /// The flags every run here is given after its unit: scenario 1's check, no forge, a step
    /// commit named by its action, at most 8 steps.
    const LOOP: [&'static str; 8] = [
        "--test-cmd",
        "./check.sh",
        "--forge",
        "none",
        "--commit-style",
        "action",
        "--iterations",
        "8",
    ];

    /// `takt scenario run` of scenario 1.
    fn scenario(&self) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["scenario", "run", "--repo", repo, "1"];
        args.extend(Self::LOOP);
        self.takt(&args)
    }

    /// `takt run --case <case>`.
    fn resume(&self, case: &str) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["run", "--repo", repo, "--case", case];
        args.extend(Self::LOOP);
        self.takt(&args)
    }

    /// `takt run` with neither `--case`, `--intent` nor `--story`: the frontier's run.
    fn frontier_run(&self) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["run", "--repo", repo];
        args.extend(Self::LOOP);
        self.takt(&args)
    }

    /// Runs takt with `args`: the double first on the program path, the state home in the
    /// scratch world. The exit code, stdout and stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env("XDG_STATE_HOME", self.state.join("xdg-state"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// The double's calls so far.
    fn calls(&self) -> usize {
        fs::read_to_string(&self.calls)
            .unwrap_or_default()
            .lines()
            .count()
    }

    /// Sets how the double replies from its next call on.
    fn set_mode(&self, mode: Mode) {
        fs::write(&self.mode, mode.word()).expect("the mode is written");
    }
}

/// The run's `stopped:` line.
fn stop_line(out: &str, err: &str) -> String {
    out.lines()
        .find(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("no stop line: stdout {out} stderr {err}"))
        .to_owned()
}

/// The case directory a stop line names, after its last `, case `.
fn case_of(stop: &str) -> PathBuf {
    PathBuf::from(
        stop.rsplit_once(", case ")
            .unwrap_or_else(|| panic!("the stop line names no case: {stop}"))
            .1,
    )
}

/// The case's id, its directory's name.
fn id_of(case: &Path) -> String {
    case.file_name()
        .expect("a case directory")
        .to_string_lossy()
        .into_owned()
}

/// The lines of the case's `events.jsonl`.
fn record(case: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case.join("events.jsonl"))
        .expect("the case's record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

/// The record's lines of `event`.
fn of_event<'r>(record: &'r [serde_json::Value], event: &str) -> Vec<&'r serde_json::Value> {
    record
        .iter()
        .filter(|line| line["event"] == event)
        .collect()
}

/// The case revision the case's `case.yaml` holds, `r<n>`.
fn case_revision(case: &Path) -> String {
    let text = fs::read_to_string(case.join("case.yaml")).expect("the case's case.yaml");
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).expect("YAML");
    doc["revision"]
        .as_str()
        .unwrap_or_else(|| panic!("case.yaml holds no case revision: {text}"))
        .to_owned()
}

/// Acceptance 1: the double names `decline` with a reason, offered in its prompt; the run stops
/// `Declined (<reason>)`, exit 9; the record holds one `decline` line with the reason and the case
/// revision `case.yaml` holds, and every file under the case's `evidence/` is one a validator's
/// `evidence` line names, none at the decline's step.
#[test]
fn a_declined_step_stops_declined_with_exit_9_and_its_reason() {
    let world = world("declined", Mode::Decline);
    let (code, out, err) = world.scenario();
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with(&format!("stopped: Declined ({REASON}); ")),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    assert_eq!(world.calls(), 1, "{out}");
    let arguments = fs::read_to_string(&world.arguments).expect("the double's arguments");
    assert!(
        arguments.contains("`decline`"),
        "the prompt offers the decline: {arguments}"
    );
    let case = case_of(&stop);
    let record = record(&case);
    let declines = of_event(&record, "decline");
    assert_eq!(declines.len(), 1, "one decline line: {record:#?}");
    let decline = declines[0];
    assert_eq!(decline["reason"], REASON, "{decline}");
    assert_eq!(
        decline["case_revision"].as_str(),
        Some(case_revision(&case).as_str()),
        "{decline}"
    );
    let evidence = of_event(&record, "evidence");
    assert!(
        evidence.iter().all(|line| line["step"] != decline["step"]),
        "evidence at the decline's step: {record:#?}"
    );
    let files = fs::read_dir(case.join("evidence"))
        .expect("the case's evidence directory")
        .count();
    assert_eq!(
        files,
        evidence.len(),
        "a file under evidence/ no evidence line names: {record:#?}"
    );
    let stops = of_event(&record, "stop");
    let last = stops.last().expect("a stop line");
    assert!(
        last["stop"]
            .as_str()
            .is_some_and(|stop| stop == format!("Declined ({REASON})")),
        "{last}"
    );
    assert_eq!(last["completed"], false, "{last}");
}

/// Acceptance 2: a decline whose step changed a file, proposed from the session's copy or written
/// in the tree itself, is refused and its change discarded: `StepRefused`, exit 3, never
/// `Declined`, no `decline` line, and `work.txt` is not in the tree.
#[test]
fn a_decline_that_changed_a_file_is_refused() {
    for mode in [Mode::DeclineEditing, Mode::DeclineInTheTree] {
        let world = world(mode.word(), mode);
        let (code, out, err) = world.scenario();
        let stop = stop_line(&out, &err);
        assert!(
            stop.starts_with("stopped: StepRefused (decline ") && stop.contains("changes none"),
            "{}: refused for the file it changed: {stop}\n{out}\n{err}",
            mode.word()
        );
        assert_eq!(code, 3, "{}: stdout {out} stderr {err}", mode.word());
        assert_eq!(world.calls(), 1, "{}: {out}", mode.word());
        assert!(
            !world.repo.join("work.txt").exists(),
            "{}: the change is reverted: {out}",
            mode.word()
        );
        let record = record(&case_of(&stop));
        assert!(
            of_event(&record, "decline").is_empty(),
            "{}: {record:#?}",
            mode.word()
        );
    }
}

/// Acceptance 3: `takt run --case` on the declined case, its revision where it was declined,
/// stops `Declined` again, exit 9, with 0 steps, 0 tokens and no call of the double.
#[test]
fn a_resume_at_the_same_revision_stops_declined_without_a_step() {
    let world = world("resume-same", Mode::Decline);
    let (code, out, err) = world.scenario();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let case = case_of(&stop_line(&out, &err));
    let (code, out, err) = world.resume(&id_of(&case));
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with(&format!(
            "stopped: Declined ({REASON}); steps 0, tokens 0, "
        )),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    assert_eq!(world.calls(), 1, "a model call on the resume: {out}");
}

/// Acceptance 4: a commit on the case's branch moves the case's revision, and `takt run --case`
/// steps as before, the double called again (idle now, so the run stops `NoProgress`), instead of
/// stopping `Declined` at once.
#[test]
fn a_resume_at_a_moved_revision_steps_again() {
    let world = world("resume-moved", Mode::Decline);
    let (code, out, err) = world.scenario();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let case = case_of(&stop_line(&out, &err));
    let id = id_of(&case);
    let declined_at = case_revision(&case);
    git(&world.repo, &["checkout", "-q", &format!("takt/{id}")]);
    fs::write(world.repo.join("notes.txt"), "a note\n").expect("a file");
    git(&world.repo, &["add", "notes.txt"]);
    git(
        &world.repo,
        &["commit", "-q", "-m", "a commit on the case's branch"],
    );
    world.set_mode(Mode::Idle);
    let (code, out, err) = world.resume(&id);
    let stop = stop_line(&out, &err);
    assert_ne!(
        case_revision(&case),
        declined_at,
        "the commit moved the case's revision: {out}"
    );
    assert_eq!(world.calls(), 2, "a judgement step was taken: {out}\n{err}");
    assert!(
        stop.starts_with("stopped: NoProgress; "),
        "the step was taken as before: {stop}\n{out}\n{err}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
}

/// Acceptance 5: a step that changes nothing and names an offered action still stops
/// `NoProgress`, exit 3, with no `decline` line.
#[test]
fn an_idle_step_still_stops_no_progress() {
    let world = world("idle", Mode::Idle);
    let (code, out, err) = world.scenario();
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with("stopped: NoProgress; "),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let record = record(&case_of(&stop));
    assert!(of_event(&record, "decline").is_empty(), "{record:#?}");
}

/// Acceptance 6: `takt report --format json` gives the run's stop as `Declined` with the reason,
/// and `takt trace` writes a stop operation whose outcome is `Declined`, the declining step an
/// operation of its own, `declined`, never an edit.
#[test]
fn takt_report_and_trace_name_the_decline() {
    let world = world("report", Mode::Decline);
    let (code, out, err) = world.scenario();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let case = case_of(&stop_line(&out, &err));
    let case = case.to_str().expect("utf-8");
    let (code, json, err) = world.takt(&["report", "--format", "json", "--case", case]);
    assert_eq!(code, 0, "{json}\n{err}");
    let report: serde_json::Value = serde_json::from_str(&json).expect("one JSON document");
    let run = &report["runs"][0];
    assert_eq!(run["stop"], format!("Declined ({REASON})"), "{report}");
    assert_eq!(run["outcome"], "Declined", "{report}");
    let (code, trace, err) = world.takt(&["trace", "--case", case]);
    assert_eq!(code, 0, "{trace}\n{err}");
    let ops: Vec<serde_json::Value> = trace
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .filter(|line: &serde_json::Value| line["type"] == "op")
        .collect();
    assert!(
        ops.iter().any(|op| op["outcome"] == "Declined"),
        "no stop operation names Declined: {trace}"
    );
    let step = ops
        .iter()
        .find(|op| op["action"] == "decline")
        .unwrap_or_else(|| panic!("no operation of the declining step: {trace}"));
    assert_eq!(step["outcome"], "declined", "{step}");
    assert_ne!(step["kind"], "edit", "{step}");
}

/// Acceptance 7: with the declined case open, a `takt run` given no unit does not resume it: it
/// prints the case it passed over, calls no harness and writes nothing to that case's record.
#[test]
fn an_argument_less_run_passes_over_a_declined_case() {
    let world = world("frontier", Mode::Decline);
    let (code, out, err) = world.frontier_run();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let case = case_of(&stop_line(&out, &err));
    let id = id_of(&case);
    let lines = record(&case).len();
    let (code, out, err) = world.frontier_run();
    let passed = out
        .lines()
        .find(|line| line.starts_with("frontier: passed over "))
        .unwrap_or_else(|| panic!("no line naming the case passed over: {out}\n{err}"));
    assert!(passed.contains(&id), "{passed}");
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(world.calls(), 1, "a harness call: {out}");
    assert_eq!(
        record(&case).len(),
        lines,
        "the declined case was resumed: {out}"
    );
}

/// Acceptance 8: `takt run --help` and `takt scenario run --help` list exit 9 for `Declined`, and
/// the exit-3 line does not.
#[test]
fn the_help_of_run_and_scenario_run_lists_exit_9() {
    let world = world("help", Mode::Decline);
    for verb in [&["run", "--help"][..], &["scenario", "run", "--help"][..]] {
        let (code, help, _) = world.takt(verb);
        assert_eq!(code, 0, "{help}");
        let codes: Vec<&str> = help
            .lines()
            .skip_while(|line| line.trim() != "Exit codes:")
            .collect();
        let nine = codes
            .iter()
            .find(|line| line.trim_start().starts_with("9  stopped: Declined ("))
            .unwrap_or_else(|| panic!("{verb:?}: no exit-9 line: {help}"));
        assert!(nine.contains("decline"), "{nine}");
        let three = codes
            .iter()
            .find(|line| line.trim_start().starts_with("3  stopped:"))
            .unwrap_or_else(|| panic!("{verb:?}: no exit-3 line: {help}"));
        assert!(!three.contains("Declined"), "{three}");
    }
}
