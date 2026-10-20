//! story:step-transcripts-are-kept, where `takt run` keeps them: each harness step's transcript
//! under `$XDG_STATE_HOME/takt/transcripts/<repo-key>/<case>/run-<r>/step-<n>.jsonl`, outside
//! the repository and its cases directory, its directories `0700` and the file `0600`, and the
//! step's line in the run record naming it. The first test gives takt its own `XDG_STATE_HOME`;
//! the second an empty `HOME` and no `XDG_STATE_HOME`, as on a runner or a new machine. The
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
/// after the test command's) at `<state>/takt/transcripts/<repo-key>/<case>/run-1/step-2.jsonl`:
/// the repository's, the case's and the run's directories `0700`, the file `0600`, the prompt
/// first and then exactly what the harness printed; the case's `artifacts/` holds no
/// transcript; and the step's line names the file. The init event the double printed lists
/// the five file tools takt offered: that proves takt passes `--tools Read,Edit,Write,Glob,Grep`
/// and that the transcript keeps the init event; it does not prove that Claude Code itself
/// offers no other tool, which only a live `claude` shows.
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
    let real_repo = fs::canonicalize(&repo).expect("the repository resolves");
    let digest = Command::new("sh")
        .args(["-c", "printf '%s' \"$1\" | sha256sum", "sh"])
        .arg(&real_repo)
        .output()
        .expect("sha256sum runs");
    let repo_key = String::from_utf8_lossy(&digest.stdout)[..16].to_owned();
    let case_dir = real_state
        .join("takt/transcripts")
        .join(&repo_key)
        .join(&case);
    let path = case_dir.join("run-1").join("step-2.jsonl");
    assert!(
        path.is_file(),
        "{} holds step 2's transcript, the harness's (step 1 is the test command's)\n{said}",
        path.display()
    );
    assert_eq!(mode(&case_dir), 0o700, "{}", case_dir.display());
    assert_eq!(
        mode(case_dir.parent().expect("the repository's directory")),
        0o700
    );
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
    // Relative to the transcripts directory: the record carries no home directory.
    let named = format!(
        "{repo_key}/{}/run-1/step-2.jsonl",
        case.to_str().expect("utf-8")
    );
    assert!(
        step.contains(&format!("\"transcript\":\"{named}\"")),
        "the step's line names its transcript {named}: {step}"
    );
    assert!(
        !events.contains(real_state.to_str().expect("utf-8")),
        "no line of the run record names the state home:\n{events}"
    );
    drop(scratch);
}

/// `takt run --harness claude` on a home that holds nothing, `XDG_STATE_HOME` unset and
/// `~/.local/bin` on `PATH` (a runner's, a container's, a new machine's): the transcripts
/// directory `~/.local/state/takt/transcripts` is made when the run opens, each part takt made
/// `0700`, so the harness's step is not charged with the `~/.local` takt made for it. The step
/// is taken, not refused, and its transcript kept under `~/.local/state`.
#[test]
fn takt_run_on_an_empty_home_keeps_the_transcript_and_refuses_no_step() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("step-transcripts-home-{}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(outer.clone());
    let repo = outer.join("repo");
    let bin = outer.join("bin");
    let home = outer.join("home");
    fs::create_dir_all(&repo).expect("a repository");
    fs::create_dir_all(&bin).expect("a program directory");
    fs::create_dir_all(&home).expect("an empty home");
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
                "{}:{}:{}",
                bin.display(),
                home.join(".local/bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("HOME", &home)
        .env_remove("XDG_STATE_HOME")
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let said = format!("{stdout}\n{stderr}");
    assert!(
        !stdout.contains("refused:") && !stdout.contains("StepRefused"),
        "no step is refused on an empty home\n{said}"
    );
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("the cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(cases.len(), 1, "{said}");
    let case = cases[0].file_name().expect("a name").to_owned();
    let real_home = fs::canonicalize(&home).expect("the home resolves");
    let real_repo = fs::canonicalize(&repo).expect("the repository resolves");
    let digest = Command::new("sh")
        .args(["-c", "printf '%s' \"$1\" | sha256sum", "sh"])
        .arg(&real_repo)
        .output()
        .expect("sha256sum runs");
    let repo_key = String::from_utf8_lossy(&digest.stdout)[..16].to_owned();
    let root = real_home.join(".local/state/takt/transcripts");
    let path = root
        .join(&repo_key)
        .join(&case)
        .join("run-1")
        .join("step-2.jsonl");
    assert!(
        path.is_file(),
        "{} holds the harness's step 2 transcript\n{said}",
        path.display()
    );
    for made in [
        ".local",
        ".local/state",
        ".local/state/takt",
        ".local/state/takt/transcripts",
    ] {
        assert_eq!(mode(&real_home.join(made)), 0o700, "~/{made}");
    }
    let events = fs::read_to_string(cases[0].join("events.jsonl")).expect("the run record");
    assert!(
        events
            .lines()
            .any(|line| line.contains("\"event\":\"step\"") && line.contains("\"by\":\"harness\"")),
        "the harness's step is recorded\n{events}"
    );
    drop(scratch);
}

/// A repository whose test command fails, a program directory `bin` with a `claude` double in
/// it, and a home that holds nothing, under a scratch directory of its own named `name`.
struct Home {
    scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    home: PathBuf,
}

impl Home {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("step-transcripts-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let scratch = Scratch(outer.clone());
        let repo = outer.join("repo");
        let bin = outer.join("bin");
        let home = outer.join("home");
        fs::create_dir_all(&repo).expect("a repository");
        fs::create_dir_all(&bin).expect("a program directory");
        fs::create_dir_all(&home).expect("an empty home");
        fs::write(repo.join("README.md"), "a story\n").expect("a file");
        executable(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "start"]);
        fake_claude(&bin.join("claude"), &outer.join("printed"));
        let home = fs::canonicalize(&home).expect("the home resolves");
        Self {
            scratch,
            repo,
            bin,
            home,
        }
    }

    /// `takt run` on the repository with `harness` for `iterations` steps, `HOME` the empty
    /// home, `bin` and `~/.local/bin` first on `PATH`, no `XDG_STATE_HOME` and no variable of
    /// Python's but `env`: its standard output and standard error.
    fn run(&self, harness: &str, iterations: &str, env: &[(&str, String)]) -> (String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args([
                "run",
                "--repo",
                self.repo.to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--harness",
                harness,
                "--forge",
                "none",
                "--iterations",
                iterations,
                "--test-cmd",
                "./check.sh",
            ])
            .env(
                "PATH",
                format!(
                    "{}:{}:{}",
                    self.bin.display(),
                    self.home.join(".local/bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("HOME", &self.home)
            .env_remove("B10X_GATES_POLICY");
        for name in [
            "XDG_STATE_HOME",
            "PYTHONPATH",
            "PYTHONHOME",
            "PYTHONUSERBASE",
            "PYTHONPYCACHEPREFIX",
            "PYTHONSTARTUP",
        ] {
            command.env_remove(name);
        }
        for (name, value) in env {
            command.env(name, value);
        }
        let output = command.output().expect("takt runs");
        (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// The transcript of step `step` of the first run of the repository's one case, under
    /// `root`, the transcripts directory.
    fn transcript(&self, root: &Path, step: u64) -> PathBuf {
        let cases: Vec<PathBuf> = fs::read_dir(self.repo.join(".engineering/cases"))
            .expect("the cases directory")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        assert_eq!(cases.len(), 1, "one case");
        let real_repo = fs::canonicalize(&self.repo).expect("the repository resolves");
        let digest = Command::new("sh")
            .args(["-c", "printf '%s' \"$1\" | sha256sum", "sh"])
            .arg(&real_repo)
            .output()
            .expect("sha256sum runs");
        let repo_key = String::from_utf8_lossy(&digest.stdout)[..16].to_owned();
        root.join(repo_key)
            .join(cases[0].file_name().expect("a name"))
            .join("run-1")
            .join(format!("step-{step}.jsonl"))
    }
}

/// story:transcripts-root-failures-name-it-and-charge-no-step, acceptance 1: a transcripts
/// directory that cannot be made, a file or a dangling link at its path, is named on standard
/// error as the transcripts directory, never as the set-aside directory (`~/.local/state/takt/
/// set-aside` beside it, which is fine), and the run goes on.
#[test]
fn a_transcripts_directory_that_cannot_be_made_is_named_as_such() {
    for (name, plant) in [
        ("file", "a file at the transcripts directory's path"),
        (
            "link",
            "a dangling link at the transcripts directory's path",
        ),
    ] {
        let home = Home::new(&format!("unmakeable-{name}"));
        let takt = home.home.join(".local/state/takt");
        fs::create_dir_all(&takt).expect("the way");
        if name == "file" {
            fs::write(takt.join("transcripts"), "").expect("a file in the way");
        } else {
            std::os::unix::fs::symlink(home.home.join("nowhere"), takt.join("transcripts"))
                .expect("a link");
        }
        let (stdout, stderr) = home.run("none", "1", &[]);
        let said = format!("{plant}\n{stdout}\n{stderr}");
        assert!(
            stderr.contains(&format!(
                "no step transcript is kept: the transcripts directory {}",
                takt.join("transcripts").display()
            )),
            "{said}"
        );
        assert!(stderr.contains("cannot be made"), "{said}");
        assert!(!stderr.contains("set-aside"), "{said}");
        assert!(
            stdout.contains("step 1: tests.run"),
            "the run goes on\n{said}"
        );
        drop(home.scratch);
    }
}

/// story:transcripts-root-failures-name-it-and-charge-no-step, acceptances 1 and 2: on an
/// empty home with `~/.local/bin` on `PATH`, an `XDG_STATE_HOME` of `~/.local/` and a name too
/// long for the file system: takt makes `~/.local`, then fails. The failure names the
/// transcripts directory, and `~/.local`, which takt made, is taken as the run found it, so
/// neither the harness's step nor the test command's after it is refused for it.
#[test]
fn a_transcripts_directory_made_part_way_charges_no_step() {
    let home = Home::new("part-way");
    let long = "x".repeat(300);
    let state = home.home.join(".local").join(&long);
    let (stdout, stderr) = home.run(
        "claude",
        "3",
        &[("XDG_STATE_HOME", state.display().to_string())],
    );
    let said = format!("{stdout}\n{stderr}").replace(&long, "<300 x>");
    assert!(
        stderr.contains(&format!(
            "no step transcript is kept: the transcripts directory {}",
            state.join("takt/transcripts").display()
        )),
        "{said}"
    );
    assert!(!stderr.contains("set-aside"), "{said}");
    assert!(
        home.home.join(".local").is_dir(),
        "takt made ~/.local\n{said}"
    );
    assert!(
        !stdout.contains("refused:") && !stdout.contains("StepRefused"),
        "no step is charged with the ~/.local takt made\n{said}"
    );
    assert!(stdout.contains("step 3: tests.run"), "{said}");
    drop(home.scratch);
}

/// story:transcripts-under-a-python-dir-charge-no-step, acceptances 1 and 2: the transcripts
/// directory inside a directory Python imports from (`PYTHONUSERBASE` or a `PYTHONPATH` entry
/// at `~/.local`, or `XDG_STATE_HOME` inside a `PYTHONPATH` entry), on an empty home: the
/// directory takt made when the run opened, and the transcript it wrote after the harness's
/// step 2, charge neither that step nor the test command's step 3 after it; the transcript is
/// kept.
#[test]
fn transcripts_under_a_python_import_directory_charge_no_step() {
    for (name, guarded, state) in [
        ("userbase", "PYTHONUSERBASE", None),
        ("pythonpath", "PYTHONPATH", None),
        ("xdg", "PYTHONPATH", Some("py/state")),
    ] {
        let home = Home::new(&format!("python-{name}"));
        let mut env = Vec::new();
        let root = match state {
            None => {
                env.push((guarded, home.home.join(".local").display().to_string()));
                home.home.join(".local/state/takt/transcripts")
            }
            Some(state) => {
                env.push((guarded, home.home.join("py").display().to_string()));
                env.push((
                    "XDG_STATE_HOME",
                    home.home.join(state).display().to_string(),
                ));
                home.home.join(state).join("takt/transcripts")
            }
        };
        let (stdout, stderr) = home.run("claude", "3", &env);
        let said = format!("{name}\n{stdout}\n{stderr}");
        assert!(
            !stdout.contains("refused:") && !stdout.contains("StepRefused"),
            "no step is charged with the transcripts takt keeps\n{said}"
        );
        assert!(stdout.contains("step 3: tests.run"), "{said}");
        let path = home.transcript(&root, 2);
        assert!(path.is_file(), "{} is kept\n{said}", path.display());
        drop(home.scratch);
    }
}

/// story:transcripts-under-a-python-dir-charge-no-step, acceptance 3: with `PYTHONUSERBASE` at
/// `~/.local`, a harness step that writes a module into `~/.local`, beside the transcripts
/// directory, is still refused for it.
#[test]
fn a_module_written_beside_the_transcripts_is_still_refused() {
    let home = Home::new("python-module");
    let module = home.home.join(".local/planted.py");
    let script = format!(
        "#!/bin/sh\nprintf 'x = 1\\n' > '{}'\nmkdir -p tests\necho work >> tests/work.txt\nprintf '%s\\n' '{}'\n",
        module.display(),
        r#"{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}","usage":{"input_tokens":3,"output_tokens":4}}"#
    );
    executable(&home.bin.join("claude"), &script);
    let (stdout, stderr) = home.run(
        "claude",
        "3",
        &[(
            "PYTHONUSERBASE",
            home.home.join(".local").display().to_string(),
        )],
    );
    let said = format!("{stdout}\n{stderr}");
    let refused = stdout
        .lines()
        .find(|line| line.contains("refused:"))
        .unwrap_or_else(|| panic!("the step that planted a module is refused\n{said}"));
    assert!(refused.contains(&module.display().to_string()), "{said}");
    // The transcripts directory itself, not the bare `.local/state`: a checkout under
    // `~/.local/state` (a managed worktree) puts that substring in the module's own path.
    let transcripts = home.home.join(".local/state/takt/transcripts");
    assert!(
        !refused.contains(&transcripts.display().to_string()),
        "the refusal names the module, not the transcripts\n{said}"
    );
}
