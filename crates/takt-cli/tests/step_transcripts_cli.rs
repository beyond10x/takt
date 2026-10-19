//! story:step-transcripts-are-kept, where `takt run` keeps them: each harness step's transcript
//! under `$XDG_STATE_HOME/takt/transcripts/<case>/run-<r>/step-<n>.jsonl`, outside the
//! repository and its cases directory, its directories `0700` and the file `0600`, and the
//! step's line in the run record naming it. The test gives takt its own `XDG_STATE_HOME`. The
//! `claude` is a double; nothing reaches a model.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

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

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// A `claude` that prints a stream: an init event listing the tools it was offered (the
/// value after `--tools`, as Claude Code's own init event lists them), then, having written a
/// test file, the result event naming `tests.write`. What it printed is copied to `printed`.
fn fake_claude(at: &Path, printed: &Path) {
    let script = r#"#!/bin/sh
tools=''
prev=''
for a in "$@"; do
  if [ "$prev" = "--tools" ]; then tools="$a"; fi
  prev="$a"
done
list=$(printf '%s' "$tools" | sed 's/[^,][^,]*/"&"/g')
printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"tools\":[$list]}" > '@PRINTED@'
mkdir -p tests
echo work >> tests/work.txt
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}","usage":{"input_tokens":3,"output_tokens":4}}' >> '@PRINTED@'
cat '@PRINTED@'
"#
    .replace("@PRINTED@", &printed.display().to_string());
    executable(at, &script);
}

/// The mode bits of `path`, not following a link.
fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

/// `takt run --harness claude` with its own `XDG_STATE_HOME` keeps the harness's step (step 2,
/// after the test command's) at `<state>/takt/transcripts/<case>/run-1/step-2.jsonl`: the
/// case's and the run's directories `0700`, the file `0600`, the prompt first and then exactly
/// what the harness printed; the case's `artifacts/` holds no transcript; and the step's line
/// names the file. The init event the double printed lists the five file tools takt offered:
/// that proves takt passes `--tools Read,Edit,Write,Glob,Grep` and that the transcript keeps
/// the init event; it does not prove that Claude Code itself offers no other tool, which only a
/// live `claude` shows.
#[test]
fn takt_run_keeps_each_step_transcript_under_the_state_home() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("step-transcripts-cli-{}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(outer.clone());
    let repo = outer.join("repo");
    let bin = outer.join("bin");
    let state = outer.join("state");
    fs::create_dir_all(&repo).expect("a repository");
    fs::create_dir_all(&bin).expect("a program directory");
    fs::create_dir_all(&state).expect("a state home");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    executable(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let printed = outer.join("printed");
    fake_claude(&bin.join("claude"), &printed);

    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "probe",
            "--harness",
            "claude",
            "--forge",
            "none",
            "--iterations",
            "2",
            "--test-cmd",
            "./check.sh",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("XDG_STATE_HOME", &state)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let said = format!("{stdout}\n{stderr}");
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("the cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(cases.len(), 1, "{said}");
    let case = cases[0].file_name().expect("a name").to_owned();
    let real_state = fs::canonicalize(&state).expect("the state home resolves");
    let case_dir = real_state.join("takt/transcripts").join(&case);
    let path = case_dir.join("run-1").join("step-2.jsonl");
    assert!(
        path.is_file(),
        "{} holds step 2's transcript, the harness's (step 1 is the test command's)\n{said}",
        path.display()
    );
    assert_eq!(mode(&case_dir), 0o700, "{}", case_dir.display());
    assert_eq!(mode(&case_dir.join("run-1")), 0o700);
    assert_eq!(mode(&path), 0o600);
    let kept = fs::read_to_string(&path).expect("the transcript");
    let (first, rest) = kept.split_once('\n').expect("a first line");
    assert!(
        first.contains("\"type\":\"takt.prompt\"") && first.contains("Intent: probe"),
        "the prompt line: {first}"
    );
    assert_eq!(
        rest,
        fs::read_to_string(&printed).expect("what the double printed")
    );
    assert_eq!(
        rest.lines().next(),
        Some(r#"{"type":"system","subtype":"init","tools":["Read","Edit","Write","Glob","Grep"]}"#),
        "the tools the step was offered, as the transcript keeps them"
    );
    let artifacts = cases[0].join("artifacts");
    assert!(
        !artifacts.join("run-1").exists() && !artifacts.join("step-2.jsonl").exists(),
        "no transcript under the case's artifacts"
    );
    let events = fs::read_to_string(cases[0].join("events.jsonl")).expect("the run record");
    let step = events
        .lines()
        .find(|line| line.contains("\"event\":\"step\"") && line.contains("\"by\":\"harness\""))
        .unwrap_or_else(|| panic!("a harness step in\n{events}"));
    assert!(
        step.contains(&format!(
            "\"transcript\":\"{}\"",
            path.to_str().expect("utf-8")
        )),
        "the step's line names its transcript {}: {step}",
        path.display()
    );
    drop(scratch);
}
