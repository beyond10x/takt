//! story:cases-live-under-dot-takt, acceptance 1 to 6. A new case opens under `.takt/cases` and
//! takt writes `.takt/.gitignore` beside it, which keeps the root out of version control; a
//! `.takt/.gitignore` that exists is left alone; a case under the legacy `.engineering/cases`
//! resumes where it is; a step that writes under either root is refused; the records a refused
//! step leaves survive its undo; and the readers list a case under each root. The `claude` is a
//! double and the forge is `local`, its hub a bare repository: nothing reaches a model or a
//! network.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::events::{Event, Events, TokenUsage};
use takt::loop_scorecard::LoopScorecard;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A recorded `loop.improvement/1` turn, as the run record copied it.
const TURN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/runs/2026-10-18-improvement-turn-1/case"
);

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// `git <args>` in `tree`; its standard output, trimmed.
fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "cases-under-dot-takt-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A committed repository with one file, at `<scratch>/tree`; the scratch directory holds the
/// doubles beside it and is removed on drop.
fn plain_repository(name: &str) -> (Scratch, PathBuf) {
    let outer = fresh(name);
    let repo = outer.join("tree");
    fs::create_dir_all(&repo).expect("a repository");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

/// `takt run --repo <repo>` with `args` and the harness, forge and test command a single step
/// needs: its exit code, standard output and standard error.
fn takt_run(repo: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(repo)
        .args(args)
        .args([
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ])
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Runs the built `takt` binary with `args`: the exit code, stdout and stderr.
fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable directory") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// Writes `name` into `bin`, executable, with `text`.
fn executable(bin: &Path, name: &str, text: &str) {
    let path = bin.join(name);
    fs::write(&path, text).expect("the stub is written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// A `claude` double that writes each of `paths` into the tree and proposes them as the files
/// of its one `tests.write` step, so the step is one that writes under a cases root. Nothing
/// reaches a model.
fn planting_claude(at: &Path, paths: &[&str]) {
    let mut writes = String::new();
    let mut items = Vec::new();
    for path in paths {
        writes.push_str(&format!(
            "mkdir -p \"$(dirname '{path}')\"\nprintf 'planted\\n' > '{path}'\n"
        ));
        items.push(format!(
            "{{\\\"path\\\": \\\"{path}\\\", \\\"content\\\": \\\"planted\\\"}}"
        ));
    }
    let result = format!(
        "{{\\\"action\\\": \\\"tests.write\\\", \\\"summary\\\": \\\"planted\\\", \\\"files\\\": [{}]}}",
        items.join(", ")
    );
    let script = format!(
        "#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\n{writes}printf '%s\\n' '{{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"{result}\",\"usage\":{{\"input_tokens\":3,\"output_tokens\":4}}}}'\n"
    );
    executable(at.parent().expect("a program directory"), "claude", &script);
}

/// `takt run` on `repo` with `args` and `--harness claude`, the `claude` double first on the
/// program path and takt's state kept under `state`: the exit code, stdout and stderr.
fn takt_run_claude(repo: &Path, bin: &Path, state: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(repo)
        .args(args)
        .args([
            "--harness",
            "claude",
            "--forge",
            "none",
            "--test-cmd",
            "true",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("XDG_STATE_HOME", state)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A committed copy of the fixture store `one-ready` with a manifest, a gates document whose
/// one assertion is true and a commit identity, at `<scratch>/tree`, its `origin` the bare hub
/// `<scratch>/hub.git` holding its `main`, and `<scratch>/run.yaml`, a run file whose forge is
/// `local` with `true` as its checks. Nothing reaches a network.
fn delivery_repository(name: &str) -> (Scratch, PathBuf) {
    let outer = fresh(name);
    let repo = outer.join("tree");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    git(&outer, &["init", "-q", "--bare", "-b", "main", "hub.git"]);
    let hub = outer.join("hub.git");
    git(
        &repo,
        &["remote", "add", "origin", hub.to_str().expect("utf-8")],
    );
    git(&repo, &["push", "-q", "origin", "main"]);
    fs::write(
        outer.join("run.yaml"),
        "forge: local\nlocal_checks: [\"true\"]\n",
    )
    .expect("the run file");
    (Scratch(outer), repo)
}

/// Moves the case `id` from `.takt/cases` to `.engineering/cases`, its files rewritten where
/// they name the root they were in: the case as a takt before this story wrote it.
fn relocate_to_legacy(repo: &Path, id: &str) {
    let from = repo.join(".takt/cases").join(id);
    let to = repo.join(".engineering/cases").join(id);
    copy_rewritten(&from, &to);
    fs::remove_dir_all(&from).expect("the moved case leaves the new root");
}

fn copy_rewritten(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable case") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_rewritten(&entry.path(), &target);
        } else {
            let bytes = fs::read(entry.path()).expect("a file");
            let bytes = match String::from_utf8(bytes) {
                Ok(text) => text
                    .replace(".takt/cases/", ".engineering/cases/")
                    .into_bytes(),
                Err(error) => error.into_bytes(),
            };
            fs::write(&target, bytes).expect("a copied file");
        }
    }
}

/// A case directory `dir` holding one run that opened, took one step and stopped, written as
/// takt writes it.
fn write_opened_case(dir: &Path, case: &str) {
    fs::create_dir_all(dir).expect("a case directory");
    let events = Events::new(dir);
    let usage = TokenUsage {
        input: 5,
        output: 0,
        cache_read: 0,
        cache_creation: 0,
    };
    for (step, event) in [
        (
            0,
            Event::Opened {
                case: case.to_owned(),
                protocol: "story.delivery/1".to_owned(),
                story: None,
                inputs: Vec::new(),
                base: None,
                binary: None,
                layers: None,
                confinement: None,
                substrate: None,
                endpoint: None,
                records: None,
                scan: None,
                protocol_source: None,
                input_sources: None,
                harness_program: None,
                preflight: None,
            },
        ),
        (
            1,
            Event::Step {
                by: "harness".to_owned(),
                name: "claude-code".to_owned(),
                action: "implementation.edit".to_owned(),
                summary: "edited".to_owned(),
                tokens: Some(usage.total()),
                usage: Some(usage),
                revision: None,
                model: None,
                version: None,
                cost_usd: None,
                duration_ms: None,
                api_ms: None,
                turns: None,
                started_at: None,
                transcript: None,
                transcript_refused: None,
            },
        ),
        (
            1,
            Event::Stop {
                stop: "Completed (accepted)".to_owned(),
                steps: 1,
                tokens: 5,
                completed: None,
            },
        ),
    ] {
        events.emit(step, &event).expect("the event is written");
    }
}

/// Acceptance 1. `takt run` on a clean repository writes its case under `.takt/cases/<id>/`,
/// writes `.takt/.gitignore` as `*` and a newline, creates no legacy root, and leaves
/// `git status --porcelain` empty.
#[test]
fn a_new_case_opens_under_dot_takt() {
    let (_scratch, repo) = plain_repository("new-case");
    let (code, out, err) = takt_run(&repo, &["--intent", "probe", "--case-id", "fresh"]);
    assert!(
        repo.join(".takt/cases/fresh/case.yaml").is_file(),
        "the case opens under .takt/cases ({code})\nstdout {out}\nstderr {err}"
    );
    assert!(
        !repo.join(".engineering/cases").exists(),
        "a new case makes no legacy root\nstdout {out}\nstderr {err}"
    );
    assert_eq!(
        fs::read_to_string(repo.join(".takt/.gitignore")).expect("takt writes .takt/.gitignore"),
        "*\n",
        "stdout {out}\nstderr {err}"
    );
    assert_eq!(
        git_out(&repo, &["status", "--porcelain"]),
        "",
        "git status shows none of the run's records\nstdout {out}\nstderr {err}"
    );
}

/// Acceptance 2. With a committed `.takt/.gitignore` of `!cases/`, the run opens its case under
/// `.takt/cases` and leaves the file byte for byte.
#[test]
fn an_existing_gitignore_is_kept() {
    let (_scratch, repo) = plain_repository("kept");
    fs::create_dir_all(repo.join(".takt")).expect("the state directory");
    fs::write(repo.join(".takt/.gitignore"), "!cases/\n").expect("a file");
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &["commit", "-q", "-m", "the repository's own ignore file"],
    );
    let (code, out, err) = takt_run(&repo, &["--intent", "probe", "--case-id", "kept-case"]);
    assert!(
        repo.join(".takt/cases/kept-case/case.yaml").is_file(),
        "the case opens under .takt/cases ({code})\nstdout {out}\nstderr {err}"
    );
    assert_eq!(
        fs::read(repo.join(".takt/.gitignore")).expect("the file is still there"),
        b"!cases/\n",
        "the existing .takt/.gitignore is left byte for byte\nstdout {out}\nstderr {err}"
    );
}

/// Acceptance 3. A case under `.engineering/cases/<id>/` waiting on `repository.merge` is
/// approved with `takt approve`, resumes and merges, and no `.takt/cases/<id>` is created. The
/// case is written under the new root by the first run, then moved to the legacy root as a
/// takt before this story left it.
#[test]
fn a_legacy_case_resumes_where_it_is() {
    let (scratch, repo) = delivery_repository("legacy");
    let config = scratch.0.join("run.yaml");
    let path = repo.to_str().expect("utf-8");
    let run = [
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        "story.delivery/1",
        "--config",
        config.to_str().expect("utf-8"),
    ];
    let mut first = run.to_vec();
    first.extend(["--iterations", "0"]);
    let (code, out, err) = takt(&first);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let id = git_out(&repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();

    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);

    let (code, out, err) = takt(&run);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
    assert!(
        repo.join(".takt/cases")
            .join(&id)
            .join("case.yaml")
            .is_file(),
        "the new case opens under .takt/cases\nstdout {out} stderr {err}"
    );
    relocate_to_legacy(&repo, &id);

    let (code, out, err) = takt(&[
        "approve",
        "--repo",
        path,
        &id,
        "repository.merge",
        "--by",
        "human:operator",
    ]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let (code, out, err) = takt(&run);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("merged pull request #1"),
        "the legacy case merges: stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: Completed"),
        "stdout {out} stderr {err}"
    );
    assert!(
        !repo.join(".takt/cases").join(&id).exists(),
        "no .takt/cases/<id> is created for a legacy case"
    );
    assert!(
        repo.join(".engineering/cases")
            .join(&id)
            .join("case.yaml")
            .is_file(),
        "the legacy case stays where it is"
    );
}

/// Acceptance 4. A harness step that proposes a path under `.takt/cases/` and one under
/// `.engineering/cases/` is refused, and the files it wrote are put back.
#[test]
fn a_step_writing_under_either_root_is_refused() {
    let (scratch, repo) = plain_repository("refused");
    let bin = scratch.0.join("bin");
    let state = scratch.0.join("state");
    fs::create_dir_all(&bin).expect("a program directory");
    fs::create_dir_all(&state).expect("a state home");
    for (id, root) in [
        ("probe-new", ".takt/cases"),
        ("probe-legacy", ".engineering/cases"),
    ] {
        let planted = format!("{root}/{id}/planted.txt");
        planting_claude(&bin.join("claude"), &[planted.as_str()]);
        let (code, out, err) = takt_run_claude(
            &repo,
            &bin,
            &state,
            &["--intent", "probe", "--case-id", id, "--iterations", "2"],
        );
        let said = format!("{out}\n{err}");
        assert!(
            said.contains(&format!(
                "proposes a path in the cases directory: {planted}"
            )),
            "a step proposing {planted} is refused ({code}): {said}"
        );
        assert!(
            !repo.join(&planted).exists(),
            "the refused step's file {planted} is put back: {said}"
        );
        assert_eq!(
            git_out(&repo, &["status", "--porcelain"]),
            "",
            "the tree is clean after the undo of a step proposing {planted}: {said}"
        );
    }
}

/// Acceptance 5. A step refused and undone leaves the case's `events.jsonl` and its `evidence/`
/// as the run wrote them, with `.takt/` ignored: the run's opening line is there, and every
/// evidence line names a record that is still on disk.
#[test]
fn records_written_during_a_step_survive_its_undo() {
    let (scratch, repo) = plain_repository("survive");
    let bin = scratch.0.join("bin");
    let state = scratch.0.join("state");
    fs::create_dir_all(&bin).expect("a program directory");
    fs::create_dir_all(&state).expect("a state home");
    planting_claude(&bin.join("claude"), &[".takt/cases/probe/planted.txt"]);
    let (code, out, err) = takt_run_claude(
        &repo,
        &bin,
        &state,
        &[
            "--intent",
            "probe",
            "--case-id",
            "probe",
            "--iterations",
            "2",
        ],
    );
    let said = format!("{out}\n{err}");
    let events_path = repo.join(".takt/cases/probe/events.jsonl");
    let events = fs::read_to_string(&events_path).unwrap_or_else(|error| {
        panic!("the case's events.jsonl is kept ({code}): {error}\n{said}")
    });
    assert!(
        events
            .lines()
            .any(|line| line.contains("\"event\":\"opened\"")),
        "the run's opening line survives the undo: {events}\n{said}"
    );
    let mut evidence = 0;
    for line in events.lines() {
        let value: serde_json::Value = serde_json::from_str(line).expect("a JSON line");
        if value["event"] == "evidence" {
            let path = value["path"]
                .as_str()
                .expect("an evidence line names its path");
            assert!(
                repo.join(path).is_file(),
                "the record {path} the run wrote is gone after the undo: {said}"
            );
            evidence += 1;
        }
    }
    assert!(
        evidence > 0,
        "the run wrote evidence the undo must keep: {events}\n{said}"
    );
    assert!(
        !repo.join(".takt/cases/probe/planted.txt").exists(),
        "the refused step's file is put back: {said}"
    );
}

/// Acceptance 6. `takt scorecard` lists a case under each root, `takt trace` reads a case under
/// each root, and the loop scorecard lists a turn under each root.
#[test]
fn readers_find_both_roots() {
    let (_scratch, repo) = plain_repository("readers");
    write_opened_case(&repo.join(".takt/cases/fresh-root"), "fresh-root");
    write_opened_case(&repo.join(".engineering/cases/old-root"), "old-root");
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&["scorecard", "--repo", path]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("fresh-root") && out.contains("old-root"),
        "the scorecard lists a case under each root: stdout {out} stderr {err}"
    );
    for (root, case) in [
        (".takt/cases/fresh-root", "fresh-root"),
        (".engineering/cases/old-root", "old-root"),
    ] {
        let dir = repo.join(root).display().to_string();
        let (code, out, err) = takt(&["trace", "--case", &dir]);
        assert_eq!(code, 0, "stdout {out} stderr {err}");
        assert!(
            out.contains(case),
            "the trace reads {root}: stdout {out} stderr {err}"
        );
    }
    copy_dir(Path::new(TURN), &repo.join(".takt/cases/turn-new"));
    copy_dir(Path::new(TURN), &repo.join(".engineering/cases/turn-old"));
    let card = LoopScorecard::read(&repo).expect("the loop scorecard reads");
    assert_eq!(
        card.rows.len(),
        2,
        "the loop scorecard lists a turn under each root: {:?}",
        card.rows
            .iter()
            .map(|row| row.turn.as_str())
            .collect::<Vec<_>>()
    );
}
