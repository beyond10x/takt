//! story:a-step-replays-from-its-recorded-state: `takt replay --case <dir> --step <n>
//! [--times <k>]` runs one judgement step of a recorded case again from the state that step
//! started from, in a scratch worktree at the revision it started at, the case's evidence,
//! decisions and record up to the step rebuilt there, and records each replay outside the case.
//! The `claude` is a double that records the prompt it was given; nothing reaches a model.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("replay-cli-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        fs::create_dir_all(&outer).expect("a scratch tree");
        Self(outer)
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

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// A `claude` that records what each call was given under `calls`, numbered from 1: the
/// prompt after `-p`, byte for byte (`<n>.prompt`), the directory it ran in (`<n>.pwd`), what
/// `tests/work.txt` held when it started (`<n>.work`) and the evidence files the case
/// directory in its tree held (`<n>.evidence`). It then appends a line to `tests/work.txt` and
/// prints a result naming `tests.write`.
fn fake_claude(at: &Path, calls: &Path) {
    let script = r#"#!/bin/sh
prompt=''
prev=''
for a in "$@"; do
  if [ "$prev" = "-p" ]; then prompt="$a"; fi
  prev="$a"
done
n=$(ls '@CALLS@' | grep -c '\.prompt$')
n=$((n + 1))
printf '%s' "$prompt" > '@CALLS@'/$n.prompt
pwd > '@CALLS@'/$n.pwd
cat tests/work.txt > '@CALLS@'/$n.work 2>/dev/null
ls .engineering/cases/*/evidence > '@CALLS@'/$n.evidence 2>/dev/null
mkdir -p tests
echo work >> tests/work.txt
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}","usage":{"input_tokens":3,"output_tokens":4}}'
"#
    .replace("@CALLS@", &calls.display().to_string());
    executable(at, &script);
}

/// Every file under `dir`, by its path relative to `dir`, with its bytes.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(at) = todo.pop() {
        for entry in fs::read_dir(&at).expect("a directory") {
            let path = entry.expect("an entry").path();
            let meta = fs::symlink_metadata(&path).expect("metadata");
            if meta.is_dir() {
                todo.push(path);
            } else {
                found.insert(
                    path.strip_prefix(dir).expect("under dir").to_path_buf(),
                    fs::read(&path).expect("a file"),
                );
            }
        }
    }
    found
}

/// A repository whose test command fails, a recorded case on it from `takt run`: step 1 the
/// test command, step 2 the harness (it writes `tests/work.txt`, committed), step 3 the test
/// command at that commit, step 4 the harness again. Answers the case directory.
struct Recorded {
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
    calls: PathBuf,
    case: PathBuf,
}

fn recorded(outer: &Path) -> Recorded {
    recorded_with(outer, "probe the replay", &[])
}

/// [`recorded`], the run opened for `intent` on a repository that also holds `files`.
fn recorded_with(outer: &Path, intent: &str, files: &[(&str, &str)]) -> Recorded {
    let repo = outer.join("repo");
    let bin = outer.join("bin");
    let state = outer.join("state");
    let calls = outer.join("calls");
    for dir in [&repo, &bin, &state, &calls] {
        fs::create_dir_all(dir).expect("a directory");
    }
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        fs::write(repo.join(path), text).expect("a file");
    }
    executable(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    fake_claude(&bin.join("claude"), &calls);
    // story:a-run-keeps-every-decision-only-on-a-trial: a replay of an earlier step reads the
    // decisions before it, which only a `--records trial` run keeps.
    let output = takt(&bin, &state)
        .args([
            "run",
            "--continue-past-red-base",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            intent,
            "--harness",
            "claude",
            "--forge",
            "none",
            "--iterations",
            "4",
            "--records",
            "trial",
            "--test-cmd",
            "./check.sh",
        ])
        .output()
        .expect("takt runs");
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("the cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(cases.len(), 1, "{said}");
    let case = cases[0].clone();
    let events = fs::read_to_string(case.join("events.jsonl")).expect("the run record");
    let harness_steps: Vec<&str> = events
        .lines()
        .filter(|line| line.contains("\"event\":\"step\"") && line.contains("\"by\":\"harness\""))
        .collect();
    assert_eq!(
        harness_steps.len(),
        2,
        "two harness steps\n{events}\n{said}"
    );
    assert!(
        harness_steps[1].contains("\"step\":4,"),
        "step 4 is the harness's\n{events}"
    );
    Recorded {
        repo,
        bin,
        state,
        calls,
        case,
    }
}

fn takt(bin: &Path, state: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
    command
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("XDG_STATE_HOME", state)
        .env_remove("B10X_GATES_POLICY");
    command
}

/// The first line of step `step`'s transcript of run 1, as `takt run` kept it.
fn recorded_first_line(rec: &Recorded, step: u64) -> String {
    let case = rec.case.file_name().expect("a name");
    let root = fs::canonicalize(&rec.state)
        .expect("the state home")
        .join("takt/transcripts");
    let keys: Vec<PathBuf> = fs::read_dir(&root)
        .expect("the transcripts directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(keys.len(), 1, "one repository's transcripts");
    let path = keys[0]
        .join(case)
        .join("run-1")
        .join(format!("step-{step}.jsonl"));
    let kept =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    kept.lines().next().expect("a first line").to_owned()
}

/// A replay of step 4, twice: each call of the harness is given the prompt the recorded step
/// was given, byte for byte (the recorded transcript's first line holds it), in a scratch tree
/// at the revision step 4 started at (`tests/work.txt` as step 2 left it) with the case's
/// evidence up to step 4 and no later; each replay's transcript opens with the recorded first
/// line, byte for byte; the case's files, the repository's branch and status, its worktrees and
/// the recorded transcripts are as they were.
#[test]
fn a_replayed_step_is_given_the_recorded_prompt_and_leaves_the_case_as_it_was() {
    let scratch = Scratch::new("prompt");
    let rec = recorded(&scratch.0);
    let first = recorded_first_line(&rec, 4);
    let line: serde_json::Value = serde_json::from_str(&first).expect("the prompt line reads");
    assert_eq!(line["type"], "takt.prompt");
    let recorded_prompt = line["prompt"].as_str().expect("a prompt").to_owned();
    // Two calls so far: the run's steps 2 and 4.
    let before_calls = fs::read_dir(&rec.calls).expect("calls").count();
    let case_before = files(&rec.case);
    let transcripts = fs::canonicalize(&rec.state)
        .expect("the state home")
        .join("takt/transcripts");
    let transcripts_before = files(&transcripts);
    let branch_before = git(&rec.repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let head_before = git(&rec.repo, &["rev-parse", "HEAD"]);
    let status_before = git(&rec.repo, &["status", "--porcelain", "--ignored"]);
    let worktrees_before = git(&rec.repo, &["worktree", "list", "--porcelain"]);
    let out = scratch.0.join("replays");

    let output = takt(&rec.bin, &rec.state)
        .args([
            "replay",
            "--case",
            rec.case.to_str().expect("utf-8"),
            "--step",
            "4",
            "--times",
            "2",
            "--out",
            out.to_str().expect("utf-8"),
        ])
        .output()
        .expect("takt replay runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let said = format!("{stdout}\n{stderr}");
    assert_eq!(output.status.code(), Some(0), "{said}");
    assert!(stdout.contains("prompt: as recorded"), "{said}");

    // Two calls more, each given the recorded prompt byte for byte.
    let calls_after = fs::read_dir(&rec.calls).expect("calls").count();
    assert_eq!(calls_after - before_calls, 2 * 4, "two calls more\n{said}");
    let step2_work = fs::read(rec.calls.join("2.work")).expect("what step 4 found");
    for call in [3, 4] {
        let given = fs::read(rec.calls.join(format!("{call}.prompt"))).expect("the prompt");
        assert_eq!(
            given,
            recorded_prompt.as_bytes(),
            "call {call} is given the recorded prompt byte for byte\n{said}"
        );
        // The tree the replay ran in holds what step 4's did: `tests/work.txt` as step 2 left
        // it, and the case's evidence up to step 4.
        assert_eq!(
            fs::read(rec.calls.join(format!("{call}.work"))).expect("what the replay found"),
            step2_work,
            "call {call} starts from the revision step 4 started at"
        );
        assert_eq!(
            fs::read_to_string(rec.calls.join(format!("{call}.evidence"))).expect("evidence"),
            fs::read_to_string(rec.calls.join("2.evidence")).expect("evidence"),
            "call {call} sees the case's evidence as step 4 did"
        );
        let pwd = fs::read_to_string(rec.calls.join(format!("{call}.pwd"))).expect("pwd");
        let ran_in = fs::canonicalize(pwd.trim()).unwrap_or_else(|_| PathBuf::from(pwd.trim()));
        assert!(
            !ran_in.starts_with(fs::canonicalize(&rec.repo).expect("the repository")),
            "call {call} ran outside the repository's tree: {}",
            ran_in.display()
        );
    }

    // Each replay is recorded under --out, its transcript opening with the recorded line.
    for k in [1, 2] {
        let dir = out.join(format!("replay-{k}"));
        let transcript =
            fs::read_to_string(dir.join("transcript.jsonl")).expect("the replay's transcript");
        assert_eq!(
            transcript.lines().next(),
            Some(first.as_str()),
            "replay {k}'s prompt line is the recorded one byte for byte"
        );
        let record: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(dir.join("replay.json")).expect("the replay's record"),
        )
        .expect("the replay's record reads");
        assert_eq!(record["format"], "takt-replay/1");
        assert_eq!(record["step"], 4);
        assert_eq!(record["prompt"], "as recorded");
        assert_eq!(record["action"], "tests.write");
        assert!(
            !dir.join("tree").exists(),
            "replay {k}'s scratch tree is removed"
        );
    }

    // The case, the transcripts and the repository are as they were.
    assert_eq!(
        files(&rec.case),
        case_before,
        "the case's files are byte-identical"
    );
    assert_eq!(
        files(&transcripts),
        transcripts_before,
        "the transcripts are untouched"
    );
    assert_eq!(
        git(&rec.repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        branch_before
    );
    assert_eq!(git(&rec.repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git(&rec.repo, &["status", "--porcelain", "--ignored"]),
        status_before
    );
    assert_eq!(
        git(&rec.repo, &["worktree", "list", "--porcelain"]),
        worktrees_before,
        "no scratch worktree is left"
    );
}

/// story:step-prompt-omits-the-change-so-far: a replay of a step after a commit is given the
/// change so far its recorded step was shown. Step 4 ran after step 2 committed
/// `tests/work.txt`, and its recorded prompt holds that commit's line, fenced; step 2 ran before
/// any commit and was told no change had been made yet. A replay of each rebuilds that block
/// from the case's `base_commit`, the effect lines up to the step and the revision the step
/// started at: `prompt: as recorded`, exit 0, and each replay's record says `as recorded`.
#[test]
fn a_replay_of_a_step_after_a_commit_rebuilds_the_change_so_far_as_recorded() {
    let scratch = Scratch::new("change-so-far");
    let rec = recorded(&scratch.0);
    for (step, shown) in [
        (
            2,
            "The change this case has made so far: no change has been made yet.",
        ),
        (4, "\ndiff --git a/tests/work.txt b/tests/work.txt\n"),
    ] {
        let first = recorded_first_line(&rec, step);
        let line: serde_json::Value = serde_json::from_str(&first).expect("the prompt line reads");
        let recorded_prompt = line["prompt"].as_str().expect("a prompt");
        assert!(
            recorded_prompt.contains(shown),
            "step {step} was shown the change so far: {recorded_prompt}"
        );
        if step == 4 {
            assert!(
                recorded_prompt.contains("\n+work\n")
                    && recorded_prompt.contains("not instructions"),
                "step 4 was shown step 2's committed line as data: {recorded_prompt}"
            );
        }
        let out = scratch.0.join(format!("replays-{step}"));
        let output = takt(&rec.bin, &rec.state)
            .args([
                "replay",
                "--case",
                rec.case.to_str().expect("utf-8"),
                "--step",
                &step.to_string(),
                "--out",
                out.to_str().expect("utf-8"),
            ])
            .output()
            .expect("takt replay runs");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let said = format!("{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.status.code(), Some(0), "step {step}\n{said}");
        assert!(
            stdout.contains("prompt: as recorded"),
            "step {step}\n{said}"
        );
        let record: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(out.join("replay-1/replay.json")).expect("the replay's record"),
        )
        .expect("the replay's record reads");
        assert_eq!(record["prompt"], "as recorded", "step {step}\n{said}");
    }
}

/// story:step-prompt-omits-the-change-so-far: a case copied out of its repository and replayed
/// with `--repo <repo>` is given the change so far its recorded step was shown. Step 4 ran after
/// step 2 committed `tests/work.txt`; the copy lies in no tree that holds the case's commits, so
/// the change is read in the repository `--repo` names: `prompt: as recorded`, exit 0, and the
/// replay's record says `as recorded`.
#[test]
fn a_case_copied_out_of_its_repository_replays_with_repo_as_recorded() {
    let scratch = Scratch::new("change-so-far-copy");
    let rec = recorded(&scratch.0);
    let first = recorded_first_line(&rec, 4);
    let line: serde_json::Value = serde_json::from_str(&first).expect("the prompt line reads");
    let recorded_prompt = line["prompt"].as_str().expect("a prompt");
    assert!(
        recorded_prompt.contains("\ndiff --git a/tests/work.txt b/tests/work.txt\n")
            && recorded_prompt.contains("\n+work\n"),
        "step 4 was shown step 2's committed line: {recorded_prompt}"
    );
    let copy = copy_case(&rec.case, &scratch.0.join("copy"));
    let out = scratch.0.join("replays-copy");
    let output = takt(&rec.bin, &rec.state)
        .args(["replay", "--case"])
        .arg(&copy)
        .arg("--repo")
        .arg(&rec.repo)
        .args(["--step", "4", "--out"])
        .arg(&out)
        .output()
        .expect("takt replay runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let said = format!("{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.status.code(), Some(0), "{said}");
    assert!(stdout.contains("prompt: as recorded"), "{said}");
    let record: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(out.join("replay-1/replay.json")).expect("the replay's record"),
    )
    .expect("the replay's record reads");
    assert_eq!(record["prompt"], "as recorded", "{said}");
}

/// story:step-spends-a-request-reading-its-scope-files: a replay of a step whose intent names
/// scope files is given the files its recorded step was given, read in the tree as it was at
/// the revision the step started at. Step 2 was given `notes.py`, step 4 `notes.py` and the
/// `tests/work.txt` step 2 committed; `later.py` came after the run. A commit after the run
/// removes `notes.py` and adds `later.py`, and a replay of each step is `as recorded`, exit 0,
/// its record saying so.
#[test]
fn a_replay_after_a_commit_attaches_the_scope_files_as_recorded() {
    let scratch = Scratch::new("scope-files");
    let intent = "probe the replay\n\n## Scope\n\n- `notes.py` (the notes)\n- `tests/work.txt`\n- `later.py`\n";
    let rec = recorded_with(&scratch.0, intent, &[("notes.py", "notes\n")]);
    for (step, attached) in [
        (2, "\n\nFiles in scope, attached: @notes.py\n\n"),
        (
            4,
            "\n\nFiles in scope, attached: @notes.py @tests/work.txt\n\n",
        ),
    ] {
        let first = recorded_first_line(&rec, step);
        let line: serde_json::Value = serde_json::from_str(&first).expect("the prompt line reads");
        let recorded_prompt = line["prompt"].as_str().expect("a prompt");
        assert!(
            recorded_prompt.contains(attached),
            "step {step} was given its scope files: {recorded_prompt}"
        );
    }
    git(&rec.repo, &["rm", "-q", "notes.py"]);
    fs::write(rec.repo.join("later.py"), "later\n").expect("a file");
    git(&rec.repo, &["add", "later.py"]);
    git(&rec.repo, &["commit", "-q", "-m", "after the run"]);
    for step in [2, 4] {
        let out = scratch.0.join(format!("replays-scope-{step}"));
        let output = takt(&rec.bin, &rec.state)
            .args(["replay", "--case"])
            .arg(&rec.case)
            .args(["--step", &step.to_string(), "--out"])
            .arg(&out)
            .output()
            .expect("takt replay runs");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let said = format!("{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.status.code(), Some(0), "step {step}\n{said}");
        assert!(
            stdout.contains("prompt: as recorded"),
            "step {step}\n{said}"
        );
        let record: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(out.join("replay-1/replay.json")).expect("the replay's record"),
        )
        .expect("the replay's record reads");
        assert_eq!(record["prompt"], "as recorded", "step {step}\n{said}");
    }
}

/// A step that is a validator's (step 3, the test command) is no judgement step: refused, exit
/// 1, nothing run and nothing written.
#[test]
fn a_validator_step_is_refused() {
    let scratch = Scratch::new("validator");
    let rec = recorded(&scratch.0);
    let before_calls = fs::read_dir(&rec.calls).expect("calls").count();
    let out = scratch.0.join("replays");
    let output = takt(&rec.bin, &rec.state)
        .args([
            "replay",
            "--case",
            rec.case.to_str().expect("utf-8"),
            "--step",
            "3",
            "--out",
            out.to_str().expect("utf-8"),
        ])
        .output()
        .expect("takt replay runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("not a judgement step"), "{stderr}");
    assert_eq!(
        fs::read_dir(&rec.calls).expect("calls").count(),
        before_calls
    );
    assert!(!out.exists(), "nothing written");
}

/// A record directory inside the case is refused: a replay is never recorded in the case.
#[test]
fn a_record_directory_inside_the_case_is_refused() {
    let scratch = Scratch::new("inside");
    let rec = recorded(&scratch.0);
    let case_before = files(&rec.case);
    let output = takt(&rec.bin, &rec.state)
        .args([
            "replay",
            "--case",
            rec.case.to_str().expect("utf-8"),
            "--step",
            "4",
            "--out",
            rec.case.join("replays").to_str().expect("utf-8"),
        ])
        .output()
        .expect("takt replay runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(files(&rec.case), case_before);
}

/// A recorded prompt the rebuilt one does not equal (the recorded transcript's first line
/// changed by one word) replays nothing: exit 3, the harness not called, nothing written.
#[test]
fn a_prompt_that_differs_from_the_recorded_one_replays_nothing() {
    let scratch = Scratch::new("differs");
    let rec = recorded(&scratch.0);
    let case = rec.case.file_name().expect("a name");
    let root = fs::canonicalize(&rec.state)
        .expect("the state home")
        .join("takt/transcripts");
    let key = fs::read_dir(&root)
        .expect("the transcripts directory")
        .next()
        .expect("one repository")
        .expect("an entry")
        .path();
    let path = key.join(case).join("run-1").join("step-4.jsonl");
    let kept = fs::read_to_string(&path).expect("the transcript");
    fs::write(
        &path,
        kept.replacen("probe the replay", "probe the replays", 1),
    )
    .expect("the transcript rewritten");
    let before_calls = fs::read_dir(&rec.calls).expect("calls").count();
    let out = scratch.0.join("replays");
    let output = takt(&rec.bin, &rec.state)
        .args([
            "replay",
            "--case",
            rec.case.to_str().expect("utf-8"),
            "--step",
            "4",
            "--out",
            out.to_str().expect("utf-8"),
        ])
        .output()
        .expect("takt replay runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    assert!(stderr.contains("differs from the recorded one"), "{stderr}");
    assert_eq!(
        fs::read_dir(&rec.calls).expect("calls").count(),
        before_calls
    );
    assert!(!out.exists(), "nothing written");
}

/// A copy of the case `case` at `<outer>/.engineering/cases/<id>`, made with `cp -a`.
fn copy_case(case: &Path, outer: &Path) -> PathBuf {
    let cases = outer.join(".engineering/cases");
    fs::create_dir_all(&cases).expect("a cases directory");
    let copy = cases.join(case.file_name().expect("a name"));
    let status = Command::new("cp")
        .arg("-a")
        .arg(case)
        .arg(&copy)
        .status()
        .expect("cp runs");
    assert!(status.success(), "the case is copied");
    copy
}

/// The revision the record of `case` names last before step `step`'s first line: the one a
/// replay of that step starts at.
fn start_revision(case: &Path, step: u64) -> String {
    let events = fs::read_to_string(case.join("events.jsonl")).expect("the run record");
    let lines: Vec<serde_json::Value> = events
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let index = lines
        .iter()
        .position(|line| line["step"].as_u64() == Some(step))
        .expect("the step's line");
    lines[..index]
        .iter()
        .rev()
        .find_map(|line| match line["event"].as_str() {
            Some("step" | "effect") => line["revision"].as_str(),
            Some("store_evidence" | "store_move") => line["commit"].as_str(),
            _ => None,
        })
        .expect("a revision before the step")
        .to_owned()
}

/// A commit fetched into `repo` as `refs/heads/<name>` whose tree holds nothing but a link at
/// `link` to `target` (and the directories on the way to it): made in a clone at `<outer>`.
fn side_commit(repo: &Path, outer: &Path, name: &str, link: &str, target: &Path) -> String {
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(repo)
        .arg(outer)
        .status()
        .expect("git clone runs");
    assert!(status.success(), "the clone");
    git(outer, &["rm", "-q", "-r", "--cached", "."]);
    for entry in fs::read_dir(outer).expect("the clone") {
        let path = entry.expect("an entry").path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if fs::symlink_metadata(&path).expect("metadata").is_dir() {
            fs::remove_dir_all(&path).expect("removed");
        } else {
            fs::remove_file(&path).expect("removed");
        }
    }
    let at = outer.join(link);
    fs::create_dir_all(at.parent().expect("a parent")).expect("the way to the link");
    std::os::unix::fs::symlink(target, &at).expect("the link");
    git(outer, &["add", "-A"]);
    git(outer, &["commit", "-q", "-m", "side"]);
    let side = git(outer, &["rev-parse", "HEAD"]).trim().to_owned();
    git(
        repo,
        &[
            "fetch",
            "-q",
            outer.to_str().expect("utf-8"),
            &format!("HEAD:refs/heads/{name}"),
        ],
    );
    side
}

/// A record naming a revision whose tree holds a link on the way to the rebuilt case (at
/// `.engineering`) or inside it (at the case's `evidence`), each to the real case's: refused,
/// exit 1, and the real case's files are byte-identical. The crafted copy's evidence differs
/// from the real case's, so that a write through the link would show.
#[test]
fn a_link_in_the_revision_s_tree_is_refused_and_the_case_is_not_written() {
    let scratch = Scratch::new("link-tree");
    let rec = recorded(&scratch.0);
    let id = rec
        .case
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    let case_before = files(&rec.case);
    let revision = start_revision(&rec.case, 4);
    let ways = [
        (".engineering".to_owned(), rec.repo.join(".engineering")),
        (
            format!(".engineering/cases/{id}/evidence"),
            rec.case.join("evidence"),
        ),
    ];
    for (k, (link, target)) in ways.iter().enumerate() {
        let side = side_commit(
            &rec.repo,
            &scratch.0.join(format!("side-{k}")),
            &format!("side-{k}"),
            link,
            target,
        );
        let crafted = copy_case(&rec.case, &scratch.0.join(format!("crafted-{k}")));
        let events = fs::read_to_string(crafted.join("events.jsonl")).expect("the copy's record");
        assert!(events.contains(&revision), "the record names {revision}");
        fs::write(
            crafted.join("events.jsonl"),
            events.replace(&revision, &side),
        )
        .expect("the copy's record rewritten");
        for entry in fs::read_dir(crafted.join("evidence")).expect("the copy's evidence") {
            let path = entry.expect("an entry").path();
            let mut bytes = fs::read(&path).expect("evidence");
            bytes.extend_from_slice(b"crafted\n");
            fs::write(&path, bytes).expect("evidence rewritten");
        }
        let output = takt(&rec.bin, &rec.state)
            .args(["replay", "--case"])
            .arg(&crafted)
            .arg("--repo")
            .arg(&rec.repo)
            .args(["--step", "4", "--out"])
            .arg(scratch.0.join(format!("out-{k}")))
            .arg("--transcripts")
            .arg(scratch.0.join("none"))
            .output()
            .expect("takt replay runs");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert_eq!(output.status.code(), Some(1), "link at {link}\n{stderr}");
        assert!(stderr.contains("link"), "link at {link}\n{stderr}");
        assert_eq!(
            files(&rec.case),
            case_before,
            "the real case is byte-identical after a link at {link}"
        );
    }
}

/// With --repo, a record directory inside the tree the case directory lies in is refused too:
/// exit 1, nothing run, nothing written there.
#[test]
fn a_record_directory_inside_the_case_s_tree_is_refused_with_repo() {
    let scratch = Scratch::new("inside-tree");
    let rec = recorded(&scratch.0);
    let tree = scratch.0.join("copy");
    let copy = copy_case(&rec.case, &tree);
    let before_calls = fs::read_dir(&rec.calls).expect("calls").count();
    let out = tree.join("inside-case-tree");
    let output = takt(&rec.bin, &rec.state)
        .args(["replay", "--case"])
        .arg(&copy)
        .arg("--repo")
        .arg(&rec.repo)
        .args(["--step", "4", "--out"])
        .arg(&out)
        .arg("--transcripts")
        .arg(scratch.0.join("none"))
        .output()
        .expect("takt replay runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("case's tree"), "{stderr}");
    assert_eq!(
        fs::read_dir(&rec.calls).expect("calls").count(),
        before_calls
    );
    assert!(!out.exists(), "nothing written");
}

/// An evidence record or a decision of the case that is a link (to a copy of itself outside
/// the case) is refused, not read through: exit 1, nothing run.
#[test]
fn a_link_among_the_case_s_evidence_or_decisions_is_refused() {
    let scratch = Scratch::new("link-source");
    let rec = recorded(&scratch.0);
    let before_calls = fs::read_dir(&rec.calls).expect("calls").count();
    for (k, sub) in ["evidence", "decisions"].iter().enumerate() {
        let copy = copy_case(&rec.case, &scratch.0.join(format!("copy-{k}")));
        let outside = scratch.0.join(format!("outside-{k}"));
        fs::create_dir_all(&outside).expect("a directory");
        for entry in fs::read_dir(copy.join(sub)).expect("the copy's files") {
            let path = entry.expect("an entry").path();
            let aside = outside.join(path.file_name().expect("a name"));
            fs::rename(&path, &aside).expect("moved aside");
            std::os::unix::fs::symlink(&aside, &path).expect("the link");
        }
        let output = takt(&rec.bin, &rec.state)
            .args(["replay", "--case"])
            .arg(&copy)
            .arg("--repo")
            .arg(&rec.repo)
            .args(["--step", "4", "--out"])
            .arg(scratch.0.join(format!("out-{k}")))
            .arg("--transcripts")
            .arg(scratch.0.join("none"))
            .output()
            .expect("takt replay runs");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert_eq!(output.status.code(), Some(1), "links in {sub}\n{stderr}");
        assert!(stderr.contains("link"), "links in {sub}\n{stderr}");
        assert_eq!(
            fs::read_dir(&rec.calls).expect("calls").count(),
            before_calls,
            "nothing run with links in {sub}"
        );
    }
}

/// `takt replay --help` prints its usage, exit 0: the story's predicate.
#[test]
fn replay_help_exits_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["replay", "--help"])
        .output()
        .expect("takt runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{stdout}");
    assert!(stdout.contains("--step"), "{stdout}");
    assert!(stdout.contains("--times"), "{stdout}");
}

/// A relative `--out` is taken where takt checked it, the working directory takt runs in, and
/// never inside the repository where git would resolve it: the replay runs at the revision step 4
/// started at and leaves nothing in the repository; run from inside the repository's tree, the
/// same relative `--out` is refused, exit 1, nothing left there.
#[test]
fn a_relative_record_directory_lands_where_takt_checked_it() {
    let scratch = Scratch::new("relative-out");
    let rec = recorded(&scratch.0);
    let elsewhere = scratch.0.join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("a working directory outside the repository");
    let status_before = git(&rec.repo, &["status", "--porcelain", "--ignored"]);
    let worktrees_before = git(&rec.repo, &["worktree", "list", "--porcelain"]);
    let case_before = files(&rec.case);
    let step2_work = fs::read(rec.calls.join("2.work")).expect("what step 4 found");
    let before_calls = fs::read_dir(&rec.calls).expect("calls").count();

    let output = takt(&rec.bin, &rec.state)
        .current_dir(&elsewhere)
        .args([
            "replay",
            "--case",
            rec.case.to_str().expect("utf-8"),
            "--step",
            "4",
            "--out",
            "rel-out",
        ])
        .output()
        .expect("takt replay runs");
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0), "{said}");
    assert!(
        elsewhere.join("rel-out/replay-1/replay.json").is_file(),
        "the record is under the working directory takt ran in\n{said}"
    );
    let call = before_calls / 4 + 1;
    assert_eq!(
        fs::read(rec.calls.join(format!("{call}.work"))).expect("what the replay found"),
        step2_work,
        "the replay ran at the revision step 4 started at\n{said}"
    );
    assert!(
        !rec.repo.join("rel-out").exists(),
        "nothing in the repository\n{said}"
    );
    assert_eq!(
        git(&rec.repo, &["status", "--porcelain", "--ignored"]),
        status_before
    );
    assert_eq!(
        git(&rec.repo, &["worktree", "list", "--porcelain"]),
        worktrees_before
    );
    assert_eq!(files(&rec.case), case_before);

    let inside = takt(&rec.bin, &rec.state)
        .current_dir(&rec.repo)
        .args([
            "replay",
            "--case",
            rec.case.to_str().expect("utf-8"),
            "--step",
            "4",
            "--out",
            "rel-in",
        ])
        .output()
        .expect("takt replay runs");
    let stderr = String::from_utf8_lossy(&inside.stderr).into_owned();
    assert_eq!(inside.status.code(), Some(1), "{stderr}");
    assert!(
        !rec.repo.join("rel-in").exists(),
        "nothing left in the repository: {stderr}"
    );
    assert_eq!(
        git(&rec.repo, &["status", "--porcelain", "--ignored"]),
        status_before
    );
}
