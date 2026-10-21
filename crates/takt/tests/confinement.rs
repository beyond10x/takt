//! story:confinement-seam: every child process a run starts goes through one seam,
//! `crates/takt/src/confinement.rs`, and the test command runs under the `SubstrateRunner` of
//! loom-sdk when the run is given `--confinement substrate`.
//!
//! - No module of `crates/takt/src` but the seam, and no library source of the session-executor
//!   crate (its test module's fixtures excepted), names `std::process::Command`.
//! - An adapter that refuses every program: the run starts no process, writes no evidence
//!   record and stops with the refusal named; the harness's program is refused at its own spawn.
//! - Under Substrate the test command of a scratch crate writes the record it writes under
//!   `none`; a test that writes outside the build directory (M19) reaches no host path outside
//!   the tree and is refused the write in it, one that writes over the object store (M20) is
//!   refused the write; and a Python test command is refused by its toolchain's name with no
//!   child started unconfined.
//! - The first line a run prints and its `opened` event name the adapter.
//!
//! The Substrate cases run against the real backend in a delegated cgroup v2 scope, as Loom's
//! own do: the test binary starts itself again under `systemd-run --user --scope -p
//! Delegate=yes` and runs the case there. Where bubblewrap, a user systemd manager or the
//! Substrate driver is absent, the case prints `skipped: missing prerequisite: …` and returns.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use takt::confinement::{self, Confinement, Substrate, TestRun, Unconfined};
use takt::forge::Finished;
use takt::harness::{ClaudeCode, Harness, Kind, SessionHarness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// One validator-owned action, `tests.run`; accepted once the tests pass at the current
/// revision.
const TEST_ONLY: &str = "format: protocol/1

protocol:
  id: test.only
  revision: 1
  description: >-
    Run the tests against the implementation; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

/// A judgement action, `implementation.edit`, before `tests.run`.
const EDIT_AND_TEST: &str = "format: protocol/1

protocol:
  id: edit.and.test
  revision: 1
  description: >-
    Edit the implementation and run the tests against it; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  implementation.edit:
    description: change the implementation
    effect: write
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

/// The adapter is the process's: a case that installs one, or relies on the default, holds
/// this while it runs, and the default is put back when it lets go ([`Held`]).
static ADAPTER: Mutex<()> = Mutex::new(());

/// The process's adapter held for one case; dropping it installs [`Unconfined`] again.
struct Held(#[allow(dead_code)] MutexGuard<'static, ()>);

fn held() -> Held {
    let guard = ADAPTER.lock().unwrap_or_else(PoisonError::into_inner);
    confinement::install(Arc::new(Unconfined));
    Held(guard)
}

impl Held {
    fn install(&self, adapter: Arc<dyn Confinement>) {
        confinement::install(adapter);
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        confinement::install(Arc::new(Unconfined));
    }
}

/// Refuses every program, counting what it was asked.
#[derive(Default)]
struct RefusesAll {
    asked: AtomicUsize,
}

impl Confinement for RefusesAll {
    fn name(&self) -> &str {
        "refuses-all"
    }

    fn admit(&self, program: &OsStr) -> std::io::Result<()> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Err(confinement::refused(
            self.name(),
            program,
            "this test adapter refuses every program",
        ))
    }
}

/// Refuses the one program at `refused`, by path, and admits every other.
struct RefusesOne {
    refused: PathBuf,
    asked: AtomicUsize,
}

impl Confinement for RefusesOne {
    fn name(&self) -> &str {
        "refuses-one"
    }

    fn admit(&self, program: &OsStr) -> std::io::Result<()> {
        if Path::new(program) == self.refused {
            self.asked.fetch_add(1, Ordering::SeqCst);
            return Err(confinement::refused(
                self.name(),
                program,
                "this test adapter refuses the harness's program",
            ));
        }
        Ok(())
    }
}

/// Substrate, recording every program it admitted.
struct Watching {
    inner: Arc<dyn Confinement>,
    admitted: Mutex<Vec<String>>,
}

impl Watching {
    fn new(inner: Arc<dyn Confinement>) -> Self {
        Self {
            inner,
            admitted: Mutex::new(Vec::new()),
        }
    }

    fn admitted(&self) -> Vec<String> {
        self.admitted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn noted(&self, program: &OsStr, admitted: std::io::Result<()>) -> std::io::Result<()> {
        if admitted.is_ok() {
            self.admitted
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(program.to_string_lossy().into_owned());
        }
        admitted
    }
}

impl Confinement for Watching {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn admit(&self, program: &OsStr) -> std::io::Result<()> {
        self.noted(program, self.inner.admit(program))
    }

    fn admit_delegated(&self, program: &OsStr) -> std::io::Result<()> {
        self.noted(program, self.inner.admit_delegated(program))
    }

    fn run_test(&self, test: &TestRun<'_>) -> anyhow::Result<Finished> {
        self.inner.run_test(test)
    }
}

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
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

/// A scratch directory under this crate's build directory: the protocol file at its root and
/// the repository in `repo/`. Dropping it removes both.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.repo
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Scratch {
    fn protocol(&self) -> String {
        self.root.join("protocol.yaml").display().to_string()
    }
}

/// A directory of this case's own under the build directory, empty.
fn directory(name: &str) -> PathBuf {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("conf-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a directory");
    dir
}

/// A repository on `main` holding `files` (path, text, executable) in its first commit, beside
/// a `.gitignore` that ignores `target/`, with `protocol` beside it; a crate's lock file is
/// generated offline and committed with it.
fn repository(name: &str, protocol: &str, files: &[(&str, &str, bool)]) -> Scratch {
    let root = directory(name);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), protocol).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n").expect("a file");
    for (path, text, executable) in files {
        let at = repo.join(path);
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, text).expect("a file");
        if *executable {
            fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("a mode");
        }
    }
    if repo.join("Cargo.toml").is_file() {
        let cargo = std::env::var("CARGO").expect("cargo runs the tests");
        let locked = Command::new(cargo)
            .args(["generate-lockfile", "--offline", "--quiet"])
            .current_dir(&repo)
            .status()
            .expect("cargo runs");
        assert!(locked.success(), "the lock file is generated");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

/// A crate whose one test is `test`, a function body.
fn scratch_crate(name: &str, protocol: &str, test: &str) -> Scratch {
    let lib = format!("#[test]\nfn the_test() {{\n{test}\n}}\n");
    repository(
        name,
        protocol,
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
                false,
            ),
            ("src/lib.rs", &lib, false),
        ],
    )
}

fn options(protocol: String, iterations: u64) -> RunOptions {
    RunOptions {
        protocol,
        intent: "make the check pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    }
}

/// A harness that is never stepped: reaching it is the failure.
struct Never;

impl Harness for Never {
    fn name(&self) -> &str {
        "never"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        anyhow::bail!("the harness was stepped")
    }
}

/// What one run left: its stop, what it printed, the case's evidence records, its record.
struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    records: Vec<serde_json::Value>,
    events: String,
}

impl Ran {
    /// The stop or the error, then what the run printed.
    fn said(&self) -> String {
        match &self.stop {
            Ok(stop) => format!("{stop:?}\n{}", self.text),
            Err(error) => format!("{error:#}\n{}", self.text),
        }
    }
}

/// Every evidence record under `tree`'s cases, whichever case wrote it.
fn evidence(tree: &Path) -> Vec<serde_json::Value> {
    let Ok(cases) = fs::read_dir(tree.join(".engineering/cases")) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for case in cases {
        let Ok(entries) = fs::read_dir(case.expect("an entry").path().join("evidence")) else {
            continue;
        };
        paths.extend(
            entries
                .map(|e| e.expect("an entry").path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .filter(|p| !p.to_string_lossy().ends_with(".bundle.json")),
        );
    }
    paths.sort();
    paths
        .iter()
        .map(|path| serde_json::from_slice(&fs::read(path).expect("a record")).expect("json"))
        .collect()
}

/// Opens a case on `tree` with `harness` and the test command `command`, and runs it.
fn run(tree: &Scratch, command: &str, harness: &dyn Harness) -> Ran {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let mut run =
        Run::open(tree, options(tree.protocol(), 4), harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    Ran {
        stop,
        text: String::from_utf8_lossy(&out).into_owned(),
        records: evidence(tree),
        events: fs::read_to_string(run.case.dir.join("events.jsonl")).unwrap_or_default(),
    }
}

// --- Acceptance 1: the seam is the one place that names `std::process::Command`. ---

/// Whether `c` continues an identifier.
fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `text` with its white space dropped where a token does not need it, and the line each byte
/// came from.
fn squeezed(text: &str) -> (String, Vec<usize>) {
    let punct = |c: char| ":{},;()<>[]*&=".contains(c);
    let mut flat = String::new();
    let mut lines = Vec::new();
    let mut line = 1;
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if c == '\n' {
                line += 1;
            }
            space = true;
            continue;
        }
        if space && !flat.is_empty() && !punct(c) && !flat.ends_with(punct) {
            flat.push(' ');
            lines.push(line);
        }
        space = false;
        flat.push(c);
        lines.extend(std::iter::repeat_n(line, c.len_utf8()));
    }
    (flat, lines)
}

/// The lines of `text` that name `std::process::Command`: by its path, by a group or a glob
/// that imports it, or through the module under another name.
fn names_process_command(text: &str) -> Vec<usize> {
    let (flat, lines) = squeezed(text);
    let mut found = Vec::new();
    for (at, _) in flat.match_indices("process::") {
        if flat[..at].chars().next_back().is_some_and(ident) {
            continue;
        }
        let rest = &flat[at + "process::".len()..];
        let named = if let Some(after) = rest.strip_prefix("Command") {
            !after.chars().next().is_some_and(ident)
        } else if rest.starts_with('*') {
            true
        } else if let Some(group) = rest.strip_prefix('{') {
            let mut depth = 1;
            let mut end = group.len();
            for (i, c) in group.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            group[..end]
                .split([',', '{', '}'])
                .map(str::trim)
                .any(|item| item == "Command" || item.starts_with("Command as ") || item == "*")
        } else {
            false
        };
        if named {
            found.push(lines[at]);
        }
    }
    for (at, _) in flat.match_indices("std::process as ") {
        found.push(lines[at]);
    }
    found.sort_unstable();
    found.dedup();
    found
}

/// Every `.rs` file under `dir`, at any depth.
fn sources(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a source directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            sources(&path, into);
        } else if path.extension().is_some_and(|x| x == "rs") {
            into.push(path);
        }
    }
}

#[test]
fn the_scan_finds_every_way_of_naming_the_process_command() {
    for line in [
        "use std::process::Command;",
        "use std::process::{Child, Command, Stdio};",
        "use std::process::{\n    Child,\n    Command,\n};",
        "let c = std::process::Command::new(\"x\");",
        "let c = std :: process :: Command :: new(\"x\");",
        "use std::process::*;",
        "use std::process as p;",
        "use std::process::{Command as C};",
        "use std::{process::{self, Command}};",
        "fn f(c: &mut process::Command) {}",
    ] {
        assert!(
            !names_process_command(line).is_empty(),
            "the scan misses {line:?}"
        );
    }
    for line in [
        "use std::os::unix::process::CommandExt as _;",
        "use std::process::{Child, Stdio, ExitStatus};",
        "let c = crate::confinement::Command::new(\"x\");",
        "subprocess::Command::new(\"x\")",
        "use metaharness::protocol::{Command, Decision};",
        "std::process::id()",
    ] {
        assert!(
            names_process_command(line).is_empty(),
            "the scan flags {line:?}"
        );
    }
    assert_eq!(
        names_process_command("// a\n\nuse std::process::{\n    Stdio,\n    Command,\n};\n"),
        vec![3],
        "the line a name is on is the line its path starts on"
    );
}

#[test]
fn no_module_but_the_seam_names_the_process_command() {
    let takt = Path::new(env!("CARGO_MANIFEST_DIR"));
    let seam = takt.join("src/confinement.rs");
    let seam_text = fs::read_to_string(&seam).expect("the seam's source");
    assert!(
        !names_process_command(&seam_text).is_empty(),
        "the scan sees the seam's own use of std::process::Command"
    );
    let mut files = Vec::new();
    sources(&takt.join("src"), &mut files);
    sources(&takt.join("../session-executor"), &mut files);
    files.sort();
    assert!(
        files.iter().any(|f| f.ends_with("src/run.rs"))
            && files
                .iter()
                .any(|f| f.ends_with("session-executor/src/lib.rs")),
        "the scan reads takt's and the session executor's sources: {files:?}"
    );
    // The session executor's test module is no session's: its fixtures start `git` and the
    // test binary itself, and the crate depends on no crate of this repository, its tests
    // included (`harness_parity.rs`), so they cannot take takt's seam. Its library, everything
    // above that module, is read.
    let fixtures = "\n#[cfg(test)]\nmod tests {\n";
    let mut named = Vec::new();
    for file in files.iter().filter(|f| **f != seam) {
        let mut text = fs::read_to_string(file).expect("a source");
        if file.ends_with("session-executor/src/lib.rs") {
            let at = text
                .find(fixtures)
                .expect("the session executor's test module is where its library ends");
            assert_eq!(
                text.matches(fixtures).count(),
                1,
                "one test module in the session executor"
            );
            text.truncate(at);
        }
        for line in names_process_command(&text) {
            named.push(format!("{}:{line}", file.display()));
        }
    }
    assert!(
        named.is_empty(),
        "std::process::Command is named outside the seam:\n{}",
        named.join("\n")
    );
}

// --- Acceptance 2: an adapter that refuses every program. ---

/// The check the repositories here run: it writes `ran` beside the repository, so a run that
/// started it shows.
fn check(root: &Path) -> String {
    format!(
        "#!/bin/sh\ntouch '{}'\nexit 0\n",
        root.join("ran").display()
    )
}

#[test]
fn an_adapter_refusing_every_program_refuses_the_case_before_it_opens() {
    let held = held();
    let root = directory("refuses-open");
    let tree = {
        let tree = repository("refuses-open-tree", TEST_ONLY, &[]);
        let check = check(&root);
        fs::write(tree.join("check.sh"), check).expect("a check");
        fs::set_permissions(tree.join("check.sh"), fs::Permissions::from_mode(0o755))
            .expect("a mode");
        git(&tree, &["add", "check.sh"]);
        git(&tree, &["commit", "-q", "-m", "the check"]);
        tree
    };
    let refusing = Arc::new(RefusesAll::default());
    held.install(refusing.clone());
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let opened = Run::open(&tree, options(tree.protocol(), 4), &Never, &validators);
    drop(held);
    let error = match opened {
        Ok(_) => panic!("the case opened with every program refused"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        error.contains("confinement refuses-all refused"),
        "the refusal is named: {error}"
    );
    assert!(
        refusing.asked.load(Ordering::SeqCst) > 0,
        "the adapter was asked"
    );
    assert!(evidence(&tree).is_empty(), "{:?}", evidence(&tree));
    assert!(!root.join("ran").exists(), "the check ran");
}

#[test]
fn an_adapter_refusing_every_program_stops_the_run_with_the_refusal_named() {
    let held = held();
    let root = directory("refuses-run");
    let tree = repository(
        "refuses-run-tree",
        TEST_ONLY,
        &[("check.sh", &check(&root), true)],
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let mut run = Run::open(&tree, options(tree.protocol(), 4), &Never, &validators)
        .expect("the case opens under none");
    let refusing = Arc::new(RefusesAll::default());
    held.install(refusing.clone());
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    drop(held);
    let text = String::from_utf8_lossy(&out);
    let said = match &stop {
        Ok(stop) => panic!("the run stopped {stop:?} with every program refused\n{text}"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        said.contains("confinement refuses-all refused"),
        "the refusal is named: {said}\n{text}"
    );
    assert!(
        refusing.asked.load(Ordering::SeqCst) > 0,
        "the adapter was asked"
    );
    assert!(evidence(&tree).is_empty(), "{:?}\n{text}", evidence(&tree));
    assert!(!root.join("ran").exists(), "the check ran\n{text}");
}

#[test]
fn the_test_command_itself_goes_through_the_adapter() {
    let held = held();
    let root = directory("refuses-test");
    let tree = repository(
        "refuses-test-tree",
        TEST_ONLY,
        &[("check.sh", &check(&root), true)],
    );
    let program = tree.join("check.sh");
    let refusing = Arc::new(RefusesOne {
        refused: PathBuf::from("./check.sh"),
        asked: AtomicUsize::new(0),
    });
    held.install(refusing.clone());
    let ran = run(&tree, "./check.sh", &Never);
    drop(held);
    assert!(
        ran.said()
            .contains("confinement refuses-one refused ./check.sh"),
        "the refusal names the test command: {}",
        ran.said()
    );
    assert_eq!(refusing.asked.load(Ordering::SeqCst), 1, "{}", ran.said());
    assert!(ran.records.is_empty(), "{:?}\n{}", ran.records, ran.said());
    assert!(
        !root.join("ran").exists(),
        "{} ran\n{}",
        program.display(),
        ran.said()
    );
}

/// A harness program that writes `started` beside the repository when it runs.
fn double(root: &Path) -> PathBuf {
    let dir = root.join("double");
    fs::create_dir_all(&dir).expect("a directory");
    let program = dir.join("claude");
    fs::write(
        &program,
        format!(
            "#!/bin/sh\ntouch '{}'\nexit 1\n",
            root.join("started").display()
        ),
    )
    .expect("the double");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("a mode");
    program
}

/// Runs the edit-and-test protocol with `harness`, whose program is refused by path.
fn harness_refused(name: &str, harness: impl FnOnce(&Path) -> Box<dyn Harness>) {
    let held = held();
    let root = directory(name);
    let program = double(&root);
    let tree = repository(
        &format!("{name}-tree"),
        EDIT_AND_TEST,
        &[("check.sh", "#!/bin/sh\nexit 1\n", true)],
    );
    let harness = harness(&program);
    let refusing = Arc::new(RefusesOne {
        refused: program.clone(),
        asked: AtomicUsize::new(0),
    });
    held.install(refusing.clone());
    let ran = run(&tree, "./check.sh", harness.as_ref());
    drop(held);
    assert!(
        ran.said().contains("confinement refuses-one refused")
            && ran.said().contains(&program.display().to_string()),
        "the refusal names the harness's program: {}",
        ran.said()
    );
    assert!(
        refusing.asked.load(Ordering::SeqCst) > 0,
        "the harness's program was asked for: {}",
        ran.said()
    );
    assert!(
        !root.join("started").exists(),
        "the harness's program ran: {}",
        ran.said()
    );
}

#[test]
fn the_session_harness_program_goes_through_the_adapter() {
    harness_refused("session-harness", |program| {
        Box::new(SessionHarness {
            program: program.display().to_string(),
            ..SessionHarness::of(Kind::Claude)
        })
    });
}

#[test]
fn the_print_mode_harness_program_goes_through_the_adapter() {
    harness_refused("print-harness", |program| {
        Box::new(ClaudeCode {
            program: program.display().to_string(),
            ..ClaudeCode::default()
        })
    });
}

// --- Acceptance 6: the first line and the events name the adapter. ---

#[test]
fn the_first_line_and_the_opened_event_name_the_adapter() {
    let held = held();
    let tree = repository(
        "named",
        TEST_ONLY,
        &[("check.sh", "#!/bin/sh\nexit 0\n", true)],
    );
    let ran = run(&tree, "./check.sh", &Never);
    drop(held);
    let first = ran.text.lines().next().unwrap_or_default();
    assert!(
        first.starts_with("case ") && first.ends_with("; confinement none"),
        "the first line names the adapter: {first}\n{}",
        ran.said()
    );
    let opened: serde_json::Value = serde_json::from_str(
        ran.events
            .lines()
            .find(|line| line.contains("\"event\":\"opened\""))
            .expect("an opened line"),
    )
    .expect("json");
    assert_eq!(opened["confinement"], "none", "{opened}");
    assert!(
        matches!(ran.stop, Ok(Stop::Completed(_))),
        "the run completes under none: {}",
        ran.said()
    );
}

// --- Substrate, in a delegated scope. ---

/// Set in the delegated child of a case: it runs the case.
const DELEGATED_CASE: &str = "TAKT_CONFINEMENT_TEST_DELEGATED";
/// The file the delegated child writes what it decided into.
const REPORT: &str = "TAKT_CONFINEMENT_TEST_REPORT";
const BUBBLEWRAP: &str = "/usr/bin/bwrap";

/// Runs `case` in a delegated cgroup scope with Substrate open, or skips naming what is missing.
fn delegated(name: &str, case: impl FnOnce()) {
    if std::env::var_os(DELEGATED_CASE).is_some() {
        let report = PathBuf::from(std::env::var_os(REPORT).expect("the report path is given"));
        fs::write(&report, "started").expect("the report");
        let probe = directory(&format!("{name}-probe")).join("workspace");
        fs::create_dir_all(&probe).expect("a workspace");
        match Substrate::open(&probe) {
            Ok(_) => {
                fs::write(&report, "ran").expect("the report");
                case();
            }
            Err(error) => {
                fs::write(&report, format!("the Substrate driver ({error})")).expect("the report");
            }
        }
        return;
    }
    if !Path::new(BUBBLEWRAP).is_file() {
        eprintln!("skipped: missing prerequisite: executable {BUBBLEWRAP}");
        return;
    }
    let state = directory(&format!("{name}-state"));
    let report = state.join("report");
    let started = Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"])
        .arg(std::env::current_exe().expect("this binary"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(DELEGATED_CASE, "1")
        .env(REPORT, &report)
        .env(confinement::DELEGATED, "1")
        .output();
    let output = match started {
        Ok(output) => output,
        Err(error) => {
            eprintln!("skipped: missing prerequisite: systemd-run ({error})");
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match fs::read_to_string(&report).ok().as_deref() {
        None => eprintln!(
            "skipped: missing prerequisite: a user systemd manager that starts a delegated scope (systemd-run {}: {})",
            output.status,
            stderr.trim()
        ),
        Some("started") => panic!(
            "the delegated case stopped before it decided its prerequisites: {}\n{stdout}{stderr}",
            output.status
        ),
        Some("ran") => {
            assert!(
                output.status.success(),
                "the delegated case failed: {}\n{stdout}{stderr}",
                output.status
            );
            assert!(
                stdout.contains("1 passed"),
                "the delegated binary ran the case: {stdout}{stderr}"
            );
            eprintln!("ran delegated: {name}");
        }
        Some(missing) => eprintln!("skipped: missing prerequisite: {missing}"),
    }
}

/// The Substrate adapter, opened on `tree`.
fn substrate(tree: &Path) -> Arc<dyn Confinement> {
    Arc::new(Substrate::open(tree).expect("Substrate opens on the tree"))
}

/// The test a scratch crate runs to show its `PATH`: written to `target/seen-path`.
const SEES_ITS_PATH: &str = r#"    let path = std::env::var("PATH").unwrap_or_default();
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/target/seen-path"), path).unwrap();"#;

#[test]
fn under_substrate_the_test_command_writes_the_record_it_writes_under_none() {
    delegated(
        "under_substrate_the_test_command_writes_the_record_it_writes_under_none",
        || {
            let held = held();
            let none = scratch_crate("record-none", TEST_ONLY, SEES_ITS_PATH);
            let under_none = run(&none, "cargo test --quiet --offline", &Never);
            let confined = scratch_crate("record-substrate", TEST_ONLY, SEES_ITS_PATH);
            held.install(substrate(&confined));
            let under_substrate = run(&confined, "cargo test --quiet --offline", &Never);
            drop(held);
            let [unconfined] = under_none.records.as_slice() else {
                panic!("one record under none: {}", under_none.said());
            };
            let [record] = under_substrate.records.as_slice() else {
                panic!("one record under substrate: {}", under_substrate.said());
            };
            for field in ["format", "id", "kind", "result", "subject", "producer"] {
                assert_eq!(
                    record[field], unconfined[field],
                    "{field}: {record} against {unconfined}"
                );
            }
            assert_eq!(record["kind"], "test_result");
            assert_eq!(record["result"], "pass", "{}", under_substrate.said());
            assert!(
                matches!(under_substrate.stop, Ok(Stop::Completed(_))),
                "{}",
                under_substrate.said()
            );
            let seen = fs::read_to_string(confined.join("target/seen-path"))
                .expect("the confined test wrote where it was told");
            assert_eq!(
                seen, "/toolchain/rust/bin:/usr/bin:/bin",
                "the test ran on Substrate's fixed PATH"
            );
            assert_ne!(
                fs::read_to_string(none.join("target/seen-path")).expect("the unconfined test"),
                seen,
                "the unconfined test ran on the caller's PATH"
            );
            let first = under_substrate.text.lines().next().unwrap_or_default();
            assert!(
                first.ends_with("; confinement substrate"),
                "the first line names the adapter: {first}"
            );
            assert!(
                under_substrate
                    .events
                    .lines()
                    .any(|line| line.contains("\"event\":\"opened\"")
                        && line.contains("\"confinement\":\"substrate\"")),
                "the opened event names the adapter: {}",
                under_substrate.events
            );
        },
    );
}

/// What a probe test recorded in `target/probe`.
fn probed(tree: &Path) -> String {
    fs::read_to_string(tree.join("target/probe")).expect("the probe recorded what it did")
}

/// A probe that writes a promotion record outside the build directory twice, as a test with
/// `XDG_STATE_HOME` would (M19): at the host path `root/state/…` and in the crate's own tree
/// beside `target`; it records what each write returned, `outside <result>` and `tree <result>`
/// on two lines.
fn writes_outside(root: &Path) -> String {
    let dir = root.join("state/takt/promotions");
    format!(
        r#"    let outside = std::fs::create_dir_all("{dir}")
        .and_then(|()| std::fs::write("{dir}/probe.json", "{{}}"));
    let tree = std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/promotion.json"), "{{}}");
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/target/probe"), format!("outside {{outside:?}}\ntree {{tree:?}}")).unwrap();"#,
        dir = dir.display()
    )
}

/// The result the probe of [`writes_outside`] recorded for `which`, `outside` or `tree`.
fn probe_line(tree: &Path, which: &str) -> String {
    probed(tree)
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{which} ")).map(str::to_owned))
        .unwrap_or_else(|| panic!("the probe recorded no `{which}` line: {}", probed(tree)))
}

/// Observes the test command once on `tree`'s `HEAD`, outside any case.
fn observe(tree: &Path) -> anyhow::Result<takt::validator::Observation> {
    let revision = git(tree, &["rev-parse", "HEAD"]);
    TestsRun::new("cargo test --quiet --offline")
        .expect("a command")
        .observe(tree, &revision, 1)
}

#[test]
fn under_substrate_a_test_cannot_write_outside_the_build_directory() {
    delegated(
        "under_substrate_a_test_cannot_write_outside_the_build_directory",
        || {
            let held = held();
            let none_root = directory("m19-none-root");
            let none = scratch_crate("m19-none", TEST_ONLY, &writes_outside(&none_root));
            let observed = observe(&none).expect("the test command runs under none");
            assert!(
                none_root.join("state/takt/promotions/probe.json").is_file()
                    && none.join("promotion.json").is_file(),
                "the premise: under none the test writes outside the tree and in it: {} {}",
                observed.summary,
                probed(&none)
            );
            let root = directory("m19-substrate-root");
            let confined = scratch_crate("m19-substrate", TEST_ONLY, &writes_outside(&root));
            held.install(substrate(&confined));
            let observed = observe(&confined).expect("the test command runs under substrate");
            drop(held);
            // A host path outside the tree is not in the child's file system at all: the
            // directories it makes there are in the private root of its own mount namespace,
            // gone when it ends, so nothing reaches the host's.
            assert!(
                !root.join("state").exists(),
                "the confined test wrote outside the tree: {} {}",
                observed.summary,
                probed(&confined)
            );
            // The tree is in it, and the write beside `target` is refused.
            let tree = probe_line(&confined, "tree");
            assert!(
                tree.starts_with("Err(") && tree.contains("ReadOnlyFilesystem"),
                "the write in the tree outside `target` was refused: {tree}"
            );
            assert!(
                !confined.join("promotion.json").exists(),
                "the confined test wrote into the tree outside `target`"
            );
        },
    );
}

/// A probe that removes the first loose object of its repository and writes another in its
/// place (M20), and records what that returned.
const PLANTS_AN_OBJECT: &str = r#"    let objects = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".git/objects");
    let planted = (|| -> std::io::Result<String> {
        let mut dirs: Vec<_> = std::fs::read_dir(&objects)?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<_, _>>()?;
        dirs.sort();
        for dir in dirs {
            if dir.file_name().is_some_and(|n| n.len() == 2) {
                if let Some(object) = std::fs::read_dir(&dir)?.next() {
                    let object = object?.path();
                    std::fs::remove_file(&object)?;
                    std::fs::write(&object, b"planted")?;
                    return Ok(object.display().to_string());
                }
            }
        }
        Err(std::io::Error::other("no loose object"))
    })();
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/target/probe"), format!("{planted:?}")).unwrap();"#;

/// Every loose object of `tree` and its bytes.
fn loose_objects(tree: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    for dir in fs::read_dir(tree.join(".git/objects")).expect("an object store") {
        let dir = dir.expect("an entry").path();
        if !dir.file_name().is_some_and(|n| n.len() == 2) {
            continue;
        }
        for object in fs::read_dir(&dir).expect("an object directory") {
            let object = object.expect("an entry").path();
            let bytes = fs::read(&object).expect("an object");
            found.push((object, bytes));
        }
    }
    found.sort();
    found
}

#[test]
fn under_substrate_a_test_cannot_write_the_object_store() {
    delegated(
        "under_substrate_a_test_cannot_write_the_object_store",
        || {
            let held = held();
            let none = scratch_crate("m20-none", TEST_ONLY, PLANTS_AN_OBJECT);
            let before = loose_objects(&none);
            observe(&none).expect("the test command runs under none");
            assert_ne!(
                loose_objects(&none),
                before,
                "the premise: under none the test rewrites an object: {}",
                probed(&none)
            );
            let confined = scratch_crate("m20-substrate", TEST_ONLY, PLANTS_AN_OBJECT);
            let before = loose_objects(&confined);
            held.install(substrate(&confined));
            let observed = observe(&confined).expect("the test command runs under substrate");
            drop(held);
            assert_eq!(
                loose_objects(&confined),
                before,
                "the confined test rewrote an object: {}",
                observed.summary
            );
            assert!(
                probed(&confined).starts_with("Err("),
                "the write was refused: {}",
                probed(&confined)
            );
        },
    );
}

#[test]
fn under_substrate_a_python_test_command_is_refused_by_its_toolchain() {
    delegated(
        "under_substrate_a_python_test_command_is_refused_by_its_toolchain",
        || {
            let held = held();
            let root = directory("python-root");
            let tree = repository(
                "python",
                TEST_ONLY,
                &[(
                    "test.py",
                    &format!(
                        "open({:?}, 'w').close()\n",
                        root.join("ran").display().to_string()
                    ),
                    false,
                )],
            );
            let watching = Arc::new(Watching::new(substrate(&tree)));
            held.install(watching.clone());
            let ran = run(&tree, "python3 -I test.py", &Never);
            drop(held);
            let said = ran.said();
            assert!(ran.stop.is_err(), "the run ends with the refusal: {said}");
            assert!(
                said.contains("confinement substrate refused") && said.contains("Python"),
                "the refusal names the adapter and the toolchain: {said}"
            );
            assert!(ran.records.is_empty(), "{:?}\n{said}", ran.records);
            assert!(!root.join("ran").exists(), "the test command ran: {said}");
            let pythons: Vec<String> = watching
                .admitted()
                .into_iter()
                .filter(|program| {
                    Path::new(program)
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("python"))
                })
                .collect();
            assert!(
                pythons.is_empty(),
                "a Python was started unconfined: {pythons:?}\n{said}"
            );
        },
    );
}
