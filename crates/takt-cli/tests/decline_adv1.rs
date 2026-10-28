//! Adversarial cases for story:a-run-can-decline through the binary. A harness double on the
//! program path stands in for Claude Code: on its n-th call it runs `steps/<n>.sh` when there is
//! one (else `steps/last.sh`) in its scratch copy and prints `steps/<n>.json` (else
//! `steps/last.json`) as its reply, so each case scripts a run step by step; nothing reaches a
//! network or a model.

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

/// The reason the double gives when it declines, unless a case says otherwise.
const REASON: &str = "AGENTS.md forbids weakening check.sh";

fn git_status(tree: &Path, args: &[&str]) -> (bool, String) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    )
}

fn git(tree: &Path, args: &[&str]) -> String {
    let (ok, out) = git_status(tree, args);
    assert!(ok, "git {args:?}: {out}");
    out
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

/// The double's reply naming `inner`'s action: the one JSON document Claude Code prints.
fn reply(inner: &serde_json::Value) -> String {
    serde_json::json!({"result": inner.to_string(), "usage": {"input_tokens": 1}}).to_string()
}

/// A reply naming `decline` with `summary`.
fn decline(summary: &str) -> String {
    reply(&serde_json::json!({"action": "decline", "summary": summary}))
}

/// A reply naming `action` with `summary`.
fn action(action: &str, summary: &str) -> String {
    reply(&serde_json::json!({"action": action, "summary": summary}))
}

/// The scratch world: a repository with the fixture store (its story `story:ready-one`, and
/// with `second` a copy of it, `story:ready-two`), scenario 1's code and tests, a manifest and a
/// gates document, scenario 1 declared, no origin; a `claude` double in `bin` scripted by
/// `steps/`, declining with [`REASON`] by default.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    calls: PathBuf,
    steps: PathBuf,
    state: PathBuf,
}

fn world(name: &str, second: bool) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("decline-adv1-{}-{name}", std::process::id()));
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
    if second {
        let one = repo.join(".engineering/planning/story/ready-one.md");
        let text = fs::read_to_string(&one).expect("the ready story");
        fs::write(
            repo.join(".engineering/planning/story/ready-two.md"),
            text.replace("story:ready-one", "story:ready-two")
                .replace("# Story: ready-one", "# Story: ready-two")
                .replace("The one ready story", "The second ready story"),
        )
        .expect("a second ready story");
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
    let steps = outer.join("steps");
    fs::create_dir_all(&steps).expect("a steps directory");
    fs::write(steps.join("last.json"), decline(REASON)).expect("the default reply");
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
n=$(wc -l < '{calls}' | tr -d ' ')
s='{steps}'/$n.sh
[ -e "$s" ] || s='{steps}'/last.sh
[ -e "$s" ] && . "$s"
r='{steps}'/$n.json
[ -e "$r" ] || r='{steps}'/last.json
cat "$r"
"#,
            calls = calls.display(),
            arguments = arguments.display(),
            steps = steps.display(),
        ),
    )
    .expect("the double is written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        calls,
        steps,
        state,
    }
}

impl World {
    /// The flags every run here is given after its unit, as decline_cli gives them.
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

    /// The double's reply at its call `call` (`"1"`, `"2"`, ... or `"last"`).
    fn reply_at(&self, call: &str, reply: &str) {
        fs::write(self.steps.join(format!("{call}.json")), reply).expect("a reply");
    }

    /// The shell the double runs at its call `call`, in its scratch copy, before it replies.
    fn script_at(&self, call: &str, script: &str) {
        fs::write(self.steps.join(format!("{call}.sh")), script).expect("a script");
    }

    fn scenario(&self) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["scenario", "run", "--repo", repo, "1"];
        args.extend(Self::LOOP);
        self.takt(&args)
    }

    fn resume(&self, case: &str) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["run", "--repo", repo, "--case", case];
        args.extend(Self::LOOP);
        self.takt(&args)
    }

    fn story_run(&self, story: &str) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["run", "--repo", repo, "--story", story];
        args.extend(Self::LOOP);
        self.takt(&args)
    }

    fn frontier_run(&self) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["run", "--repo", repo];
        args.extend(Self::LOOP);
        self.takt(&args)
    }

    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env_remove("B10X_GATES_POLICY")
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

    fn calls(&self) -> usize {
        fs::read_to_string(&self.calls)
            .unwrap_or_default()
            .lines()
            .count()
    }
}

fn stop_line(out: &str, err: &str) -> String {
    out.lines()
        .find(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("no stop line: stdout {out} stderr {err}"))
        .to_owned()
}

fn case_of(stop: &str) -> PathBuf {
    PathBuf::from(
        stop.rsplit_once(", case ")
            .unwrap_or_else(|| panic!("the stop line names no case: {stop}"))
            .1,
    )
}

fn id_of(case: &Path) -> String {
    case.file_name()
        .expect("a case directory")
        .to_string_lossy()
        .into_owned()
}

fn record(case: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case.join("events.jsonl"))
        .expect("the case's record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

fn of_event<'r>(record: &'r [serde_json::Value], event: &str) -> Vec<&'r serde_json::Value> {
    record
        .iter()
        .filter(|line| line["event"] == event)
        .collect()
}

/// Every file under `dir`, read as text where it reads.
fn texts_under(dir: &Path, into: &mut Vec<(PathBuf, String)>) {
    let Ok(listed) = fs::read_dir(dir) else {
        return;
    };
    for entry in listed.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            texts_under(&path, into);
        } else if let Ok(bytes) = fs::read(&path) {
            into.push((path, String::from_utf8_lossy(&bytes).into_owned()));
        }
    }
}

/// A decline whose step changed a file of the tree anywhere (the planning store's tracked story
/// file, an untracked file under `.engineering/` or `.takt/`, a file in the case's own
/// directory, a removed tracked file, a mode-only change) is refused, `StepRefused`, exit 3,
/// never `Declined`, with no `decline` line, and the change is put back.
#[test]
fn a_decline_that_changed_any_file_of_the_tree_is_refused() {
    let cases: [(&str, &str); 6] = [
        ("store", "echo 'an edit' >> '{repo}/.engineering/planning/story/ready-one.md'\n"),
        ("engineering", "echo 'a note' > '{repo}/.engineering/note.txt'\n"),
        ("dot-takt", "mkdir -p '{repo}/.takt' && echo 'a note' > '{repo}/.takt/note.txt'\n"),
        (
            "case-dir",
            "for d in '{repo}'/.takt/cases/*/; do echo 'forged' > \"$d/forged.txt\"; done\n",
        ),
        ("removed", "rm -f '{repo}/todo.py'\n"),
        ("mode", "chmod -x '{repo}/check.sh'\n"),
    ];
    for (name, script) in cases {
        let world = world(&format!("changed-{name}"), false);
        world.script_at(
            "last",
            &script.replace("{repo}", world.repo.to_str().expect("utf-8")),
        );
        let (code, out, err) = world.scenario();
        let stop = stop_line(&out, &err);
        assert!(
            stop.starts_with("stopped: StepRefused ("),
            "{name}: a decline that changed a file is refused: {stop}\n{out}\n{err}"
        );
        assert_eq!(code, 3, "{name}: stdout {out} stderr {err}");
        let case = case_of(&stop);
        assert!(
            of_event(&record(&case), "decline").is_empty(),
            "{name}: a decline line for a refused step"
        );
        let repo = &world.repo;
        let put_back = match name {
            "store" => git_status(
                repo,
                &["diff", "--quiet", "--", ".engineering/planning/story/ready-one.md"],
            )
            .0,
            "engineering" => !repo.join(".engineering/note.txt").exists(),
            "dot-takt" => !repo.join(".takt/note.txt").exists(),
            "case-dir" => !case.join("forged.txt").exists(),
            "removed" => repo.join("todo.py").is_file(),
            _ => {
                fs::metadata(repo.join("check.sh"))
                    .expect("check.sh")
                    .permissions()
                    .mode()
                    & 0o111
                    != 0
            }
        };
        assert!(put_back, "{name}: the change is put back: {out}");
    }
}

/// A decline after an edit the run kept: step 1 fixes `todo.py` (kept and committed on the case's
/// branch), a later step declines. The run stops `Declined`, exit 9, the case's branch holding
/// the edit; a resume at that revision stops `Declined` again with 0 steps and no model call,
/// as a decline at step 1 does.
#[test]
fn a_decline_after_a_kept_edit_holds_on_resume() {
    let world = world("after-edit", false);
    let fix = Path::new(FIXTURES).join("scenario-1/fix/todo.py");
    world.script_at("1", &format!("cp '{}' todo.py\n", fix.display()));
    world.reply_at("1", &action("implementation.edit", "fixed the done command"));
    let (code, out, err) = world.scenario();
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with(&format!("stopped: Declined ({REASON}); ")),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let calls = world.calls();
    assert!(calls >= 2, "the edit and the decline were steps: {out}");
    let case = case_of(&stop);
    let id = id_of(&case);
    let (same, _) = git_status(
        &world.repo,
        &["diff", "--quiet", "main", &format!("takt/{id}"), "--", "todo.py"],
    );
    assert!(!same, "the case's branch holds the edit: {out}");
    let (code, out, err) = world.resume(&id);
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with(&format!(
            "stopped: Declined ({REASON}); steps 0, tokens 0, "
        )),
        "a resume at the declined revision takes no step: {stop}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    assert_eq!(world.calls(), calls, "a model call on the resume: {out}");
}

/// A reason with a line end, a tab and ANSI colour escapes: the stop line, on the output and in
/// the record, names it with each control character escaped and stays one line; the `decline`
/// line keeps the reason verbatim; a resume's stop names it as the first stop did.
#[test]
fn a_reason_with_control_characters_is_escaped_on_the_stop_line() {
    let world = world("control", false);
    let raw = "first line\nsecond line \u{1b}[31mred\u{1b}[0m\tend";
    let escaped = "first line\\u{a}second line \\u{1b}[31mred\\u{1b}[0m\\u{9}end";
    world.reply_at("last", &decline(raw));
    let (code, out, err) = world.scenario();
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with(&format!("stopped: Declined ({escaped}); ")),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let case = case_of(&stop);
    let record = record(&case);
    let declines = of_event(&record, "decline");
    assert_eq!(declines.len(), 1, "{record:#?}");
    assert_eq!(declines[0]["reason"], raw, "the reason verbatim");
    let stops = of_event(&record, "stop");
    assert_eq!(
        stops.last().expect("a stop line")["stop"],
        format!("Declined ({escaped})"),
        "{record:#?}"
    );
    let (code, out, err) = world.resume(&id_of(&case));
    let again = stop_line(&out, &err);
    assert!(
        again.starts_with(&format!("stopped: Declined ({escaped}); steps 0, ")),
        "{again}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");
}

/// No line a declining run prints carries a raw control character of the reason: the reason
/// shapes no line of the output (`second line` starts none) and sends no ANSI escape to the
/// operator's terminal.
#[test]
fn no_output_of_a_declining_run_carries_a_raw_control_character_of_the_reason() {
    let world = world("control-output", false);
    let raw = "first line\nsecond line \u{1b}[31mred\u{1b}[0m";
    world.reply_at("last", &decline(raw));
    let (code, out, err) = world.scenario();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let shaped: Vec<&str> = out
        .lines()
        .chain(err.lines())
        .filter(|line| line.starts_with("second line") || line.contains('\u{1b}'))
        .collect();
    assert!(
        shaped.is_empty(),
        "lines the reason shaped or coloured: {shaped:?}\nstdout {out}\nstderr {err}"
    );
}

/// A blank reason stops `Declined (no reason given)`; a 64 KiB reason stops `Declined` with it
/// whole, and a resume stops on it again.
#[test]
fn a_blank_and_a_long_reason_stop_declined() {
    let world = world("blank", false);
    world.reply_at("last", &decline("  \t "));
    let (code, out, err) = world.scenario();
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with("stopped: Declined (no reason given); "),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");

    let world = world_long();
    let long = "a reason the run keeps whole. ".repeat(2200);
    world.reply_at("last", &decline(&long));
    let (code, out, err) = world.scenario();
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with(&format!("stopped: Declined ({}); ", long.trim())),
        "a long reason, {} bytes: {}…\n{err}",
        long.len(),
        &stop[..stop.len().min(200)]
    );
    assert_eq!(code, 9, "stderr {err}");
    let (code, out, err) = world.resume(&id_of(&case_of(&stop)));
    let again = stop_line(&out, &err);
    assert!(
        again.starts_with(&format!("stopped: Declined ({}); steps 0, ", long.trim())),
        "{}…\n{err}",
        &again[..again.len().min(200)]
    );
    assert_eq!(code, 9, "stderr {err}");
}

fn world_long() -> World {
    world("long", false)
}

/// A reason holding text of the shape of a credential never reaches the case's files, the run's
/// output or a stop line: the step is refused before anything of it is recorded.
#[test]
fn a_credential_shaped_reason_never_reaches_the_record_or_the_output() {
    let world = world("credential", false);
    // Built at run time, so this file holds no text of the shape.
    let secret = format!("{}{}", "gh", format_args!("p_{}", "Ab1Cd2Ef3".repeat(5)));
    world.reply_at(
        "last",
        &decline(&format!("refused, the token is {secret} as AGENTS.md says")),
    );
    let (code, out, err) = world.scenario();
    assert_ne!(code, 9, "a stop naming the reason: stdout {out} stderr {err}");
    assert!(
        !out.contains(&secret) && !err.contains(&secret),
        "the output holds the credential: stdout {out} stderr {err}"
    );
    let mut files = Vec::new();
    texts_under(&world.repo.join(".takt"), &mut files);
    texts_under(&world.repo.join(".engineering"), &mut files);
    let holding: Vec<&PathBuf> = files
        .iter()
        .filter(|(_, text)| text.contains(&secret))
        .map(|(path, _)| path)
        .collect();
    assert!(holding.is_empty(), "files holding the credential: {holding:?}");
}

/// A decline whose reply carries a JSON `commit` member stops `Declined`, exit 9, and commits
/// nothing: the case's branch is where the run opened it.
#[test]
fn a_decline_with_a_commit_member_stops_declined_and_commits_nothing() {
    let world = world("commit-member", false);
    world.reply_at(
        "last",
        &reply(&serde_json::json!({
            "action": "decline",
            "summary": REASON,
            "commit": {"title": "Weaken check.sh", "body": ["- drop the tests"]},
        })),
    );
    let (code, out, err) = world.scenario();
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with(&format!("stopped: Declined ({REASON}); ")),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let id = id_of(&case_of(&stop));
    let log = git(
        &world.repo,
        &["log", "--format=%s", &format!("main..takt/{id}")],
    );
    assert!(
        !log.contains("Weaken check.sh") && !log.contains("decline"),
        "a commit for the decline: {log}"
    );
}

/// `takt run --story` naming the story of a declined case resumes that case, which stops
/// `Declined` again, exit 9, with no model call and no second case.
#[test]
fn a_story_run_on_a_declined_case_stops_declined_without_a_step() {
    let world = world("story", false);
    let (code, out, err) = world.frontier_run();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let case = case_of(&stop_line(&out, &err));
    let (code, out, err) = world.story_run("story:ready-one");
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with(&format!(
            "stopped: Declined ({REASON}); steps 0, tokens 0, "
        )),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    assert_eq!(case_of(&stop), case, "the declined case, not a new one");
    assert_eq!(world.calls(), 1, "a model call: {out}");
}

/// Two stories: the first argument-less run declines one case; the second passes it over, naming
/// it once, and opens the other story's case, which declines; the third passes over both, each
/// named once, calls no harness and ends with nothing ready, exit 0.
#[test]
fn an_argument_less_run_passes_over_two_declined_cases_and_takes_a_ready_story() {
    let world = world("two", true);
    let (code, out, err) = world.frontier_run();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let first = id_of(&case_of(&stop_line(&out, &err)));
    let passed_over = |out: &str, id: &str| {
        out.lines()
            .filter(|line| line.starts_with("frontier: passed over ") && line.contains(id))
            .count()
    };
    let (code, out, err) = world.frontier_run();
    assert_eq!(passed_over(&out, &first), 1, "the first case named once: {out}\n{err}");
    let stop = stop_line(&out, &err);
    let second = id_of(&case_of(&stop));
    assert_ne!(second, first, "the ready story's case was opened: {out}");
    assert_eq!(world.calls(), 2, "the ready story's case took a step: {out}");
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let (code, out, err) = world.frontier_run();
    assert_eq!(passed_over(&out, &first), 1, "{out}\n{err}");
    assert_eq!(passed_over(&out, &second), 1, "{out}\n{err}");
    assert_eq!(world.calls(), 2, "a harness call: {out}");
    assert_eq!(code, 0, "stdout {out} stderr {err}");
}

/// A declined case whose story is edited on the base, a commit on `main` that moves neither the
/// case's branch nor its case revision, stays passed over: the decline is keyed on the case
/// revision, as the story's design says.
#[test]
fn a_declined_case_stays_passed_over_when_its_story_is_edited_on_the_base() {
    let world = world("story-edited", false);
    let (code, out, err) = world.frontier_run();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let id = id_of(&case_of(&stop_line(&out, &err)));
    git(&world.repo, &["checkout", "-q", "main"]);
    let story = world.repo.join(".engineering/planning/story/ready-one.md");
    let mut text = fs::read_to_string(&story).expect("the story");
    text.push_str("\nThe check may not be weakened.\n");
    fs::write(&story, text).expect("the story is edited");
    git(&world.repo, &["add", ".engineering/planning/story/ready-one.md"]);
    git(&world.repo, &["commit", "-q", "-m", "clarify the story"]);
    let (code, out, err) = world.frontier_run();
    assert!(
        out.lines()
            .any(|line| line.starts_with("frontier: passed over ") && line.contains(&id)),
        "{out}\n{err}"
    );
    assert_eq!(world.calls(), 1, "a harness call: {out}");
    assert_eq!(code, 0, "stdout {out} stderr {err}");
}

/// `takt scorecard` over a repository whose one case (a frontier run's, so no held-out check is
/// needed) declined reads its run, exit 0, and names the stop `Declined`, not `NoProgress`.
#[test]
fn the_scorecard_names_a_declined_run() {
    let world = world("scorecard", false);
    let (code, out, err) = world.frontier_run();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let repo = world.repo.to_str().expect("utf-8");
    let (code, card, err) = world.takt(&["scorecard", "--repo", repo]);
    assert_eq!(code, 0, "{card}\n{err}");
    assert!(card.contains("Declined"), "{card}");
    assert!(!card.contains("NoProgress"), "{card}");
}

/// After a resume stopped `Declined` with no step, `takt report` (one entry per case record)
/// still gives the case's stop as `Declined` with the reason, and `takt trace` writes a stop
/// operation for each of the two declined runs, exit 0.
#[test]
fn report_and_trace_read_a_stepless_declined_run() {
    let world = world("report-resume", false);
    let (code, out, err) = world.frontier_run();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let case = case_of(&stop_line(&out, &err));
    let (code, out, err) = world.resume(&id_of(&case));
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let case = case.to_str().expect("utf-8");
    let (code, json, err) = world.takt(&["report", "--format", "json", "--case", case]);
    assert_eq!(code, 0, "{json}\n{err}");
    let report: serde_json::Value = serde_json::from_str(&json).expect("one JSON document");
    let runs = report["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 1, "{report}");
    assert_eq!(runs[0]["stop"], format!("Declined ({REASON})"), "{report}");
    assert_eq!(runs[0]["outcome"], "Declined", "{report}");
    let (code, trace, err) = world.takt(&["trace", "--case", case]);
    assert_eq!(code, 0, "{trace}\n{err}");
    let declined = trace
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|line| line["type"] == "op" && line["outcome"] == "Declined")
        .count();
    assert_eq!(declined, 2, "a stop operation per declined run: {trace}");
}

/// Two open cases of one story, the first in case order declined, the second not (two scenario
/// trials: one declined, one stopped `NoProgress`). An argument-less run passes over the
/// declined case only, as the story's design says ("a declined case leaves the argument-less
/// frontier"), and resumes the other: it is not declined, so it is not passed over with it.
#[test]
fn passing_over_a_declined_case_does_not_pass_over_its_storys_other_open_case() {
    let world = world("two-of-one-story", false);
    let (code, out, err) = world.scenario();
    assert_eq!(code, 9, "stdout {out} stderr {err}");
    let declined = id_of(&case_of(&stop_line(&out, &err)));
    world.reply_at("last", &action("implementation.edit", "nothing to change"));
    let (code, out, err) = world.scenario();
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let open = id_of(&case_of(&stop_line(&out, &err)));
    assert!(declined < open, "the declined case is first in case order");
    let (_, out, err) = world.frontier_run();
    assert!(
        out.lines()
            .any(|line| line.starts_with("frontier: passed over ") && line.contains(&declined)),
        "{out}\n{err}"
    );
    assert!(
        out.lines()
            .any(|line| line.starts_with("frontier: resumed ") && line.contains(&open)),
        "the open case that is not declined was not resumed: {out}\n{err}"
    );
}
