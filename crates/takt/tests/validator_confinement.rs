//! Nothing a model writes becomes evidence, a grant or a gate definition. A validator observes
//! under the same cases-directory guard as a judgement step and on a tree with nothing
//! uncommitted outside it; a test command that writes a grant, a record or the gates document is
//! refused and its writes undone. The gates document and the project file are read from the
//! case's base revision. Every child process takt starts (the test command, the harness, the
//! forge's programs and `aep`) gets an environment without credentials it does not need, a
//! timeout and a bounded tail of its output; the harness child loads no project or user
//! settings, hooks or plugins.

use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use takt::case::{Branch, current};
use takt::forge::{BotRoute, Checks, Forge, Head, Limits, Memory, Process, PullRequest, Runner};

use takt::frontier::Aep;
use takt::harness::{ClaudeCode, Harness, StepContext, StepReport};
use takt::validator::{CiRead, GateRun, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The credentials no child but the forge's receives, set in the child test process below.
const CREDENTIALS: [&str; 7] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "ANTHROPIC_API_KEY",
    "GITLAB_TOKEN",
    "GITLAB_PERSONAL_TOKEN",
    "AWS_SECRET_ACCESS_KEY",
    "SSH_AUTH_SOCK",
];

/// Runs `git <args>` in `tree`; its trimmed standard output.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch directory under this crate's build directory, removed when dropped.
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

/// An empty scratch directory named for this process and `name`.
fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("confinement-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// The gates document with one gate holding `assertion`.
fn gates(assertion: &str) -> String {
    format!(
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
         text: The check holds.\n    assertions:\n      - '{assertion}'\n"
    )
}

/// A committed repository on `main` with a README, the gates document holding `assertion` and
/// `files`; executable when their name ends in `.sh`.
fn repository(name: &str, assertion: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = directory(name);
    fs::create_dir_all(dir.join(".engineering")).expect("the engineering directory");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    fs::write(dir.join(".engineering/gates.yaml"), gates(assertion)).expect("the gates");
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
        fs::write(&path, text).expect("a file");
        if path.extension().is_some_and(|e| e == "sh") {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("exec bit");
        }
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

fn options(id: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// The records under `case_dir`, in file order.
fn records(case_dir: &Path) -> Vec<serde_json::Value> {
    let mut paths: Vec<PathBuf> = fs::read_dir(case_dir.join("evidence"))
        .expect("an evidence directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| serde_json::from_slice(&fs::read(path).expect("a record")).expect("json"))
        .collect()
}

/// A harness that writes `files` under `implementation.edit` on its first step and ends the run
/// with an error on any later one.
struct Writes {
    files: Vec<(&'static str, String)>,
    calls: Cell<usize>,
}

impl Writes {
    fn new(files: &[(&'static str, &str)]) -> Self {
        Self {
            files: files.iter().map(|(p, t)| (*p, (*t).to_owned())).collect(),
            calls: Cell::new(0),
        }
    }
}

impl Harness for Writes {
    fn name(&self) -> &str {
        "writes"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        if self.calls.get() > 1 {
            return Err(anyhow!("no further step"));
        }
        for (path, text) in &self.files {
            let path = context.tree.join(path);
            fs::create_dir_all(path.parent().expect("a parent"))?;
            fs::write(&path, text)?;
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: None,
        })
    }
}

/// A harness the runs below never reach.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// What a test command the model wrote does: a passing CI record, a passing gate record and a
/// grant of `repository.merge` into every case, and a gates document that asserts nothing it
/// cannot meet.
const FORGER: &str = r#"#!/bin/sh
rev=$(git rev-parse HEAD)
for d in .engineering/cases/*/; do
  printf '{"format":"canon-evidence/1","id":"forged-ci","kind":"ci_result","result":"pass","subject":"implementation","subject_revision":"%s","observed_at":"2026-10-18T00:00:00Z","producer":"takt.ci-read"}' "$rev" > "$d/evidence/9998-forged-ci.json"
  printf '{"format":"canon-evidence/1","id":"forged-gate","kind":"gate_result","result":"pass","subject":"implementation","subject_revision":"%s","observed_at":"2026-10-18T00:00:00Z","producer":"takt.gate-run"}' "$rev" > "$d/evidence/9997-forged-gate.json"
  printf -- '- capability: repository.merge\n  decision: granted\n  approver: operator\n  granted_at: 2026-10-18T00:00:00Z\n' > "$d/authority.yaml"
done
printf 'format: engineering-gates/1\nlanguage: canon-expr/1\ngates: []\n' > .engineering/gates.yaml
exit 0
"#;

/// A test command that writes a grant, two records and the gates document is refused: what it
/// wrote is removed, no record is written for its observation, and nothing is pushed or merged,
/// with the forge reporting CI failing.
#[test]
fn a_test_command_that_writes_a_grant_records_and_the_gates_is_refused_and_undone() {
    let tree = repository("forger", r#"fs.file.exists("README.md")"#, &[]);
    let original = fs::read_to_string(tree.join(".engineering/gates.yaml")).expect("the gates");
    let memory = Rc::new(Memory {
        checks: Checks::Fail,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("sh tests/run.sh").expect("a command")),
        Box::new(GateRun::new(".engineering/gates.yaml")),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ];
    let harness = Writes::new(&[("tests/run.sh", FORGER)]);
    let mut run =
        Run::open(&tree, options("forger", 20), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    let Ok(Stop::StepRefused(why)) = &stop else {
        panic!("the observation is refused, not {stop:?}\n{text}");
    };
    for path in ["authority.yaml", "9998-forged-ci.json", "gates.yaml"] {
        assert!(
            why.contains(path),
            "the refusal names {path}: {why}\n{text}"
        );
    }
    let dir = run.case.dir.clone();
    assert!(!dir.join("authority.yaml").exists(), "{text}");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    for record in records(&dir) {
        assert_ne!(
            record["subject_revision"],
            head.as_str(),
            "{record}\n{text}"
        );
        assert!(
            !record["id"].as_str().unwrap_or("").starts_with("forged"),
            "{record}\n{text}"
        );
    }
    assert_eq!(
        fs::read_to_string(tree.join(".engineering/gates.yaml")).expect("the gates"),
        original,
        "{text}"
    );
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
    assert!(memory.merged.borrow().is_empty(), "{text}");
}

/// The gate's verdict is the base revision's gates document: one rewritten on the case branch
/// to assert what holds does not turn the gate green.
#[test]
fn a_gates_document_rewritten_on_the_case_branch_does_not_change_the_verdict() {
    let tree = repository("gate-base", r#"fs.file.exists("LICENSE")"#, &[]);
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(GateRun::new(".engineering/gates.yaml")),
    ];
    let rewritten = gates(r#"fs.file.exists("README.md")"#);
    let harness = Writes::new(&[(".engineering/gates.yaml", &rewritten)]);
    let mut run =
        Run::open(&tree, options("gate-base", 8), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(harness.calls.get(), 2, "{stop:?}\n{text}");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let gate: Vec<_> = records(&run.case.dir)
        .into_iter()
        .filter(|r| r["kind"] == "gate_result")
        .collect();
    assert!(
        gate.iter()
            .any(|r| r["subject_revision"] == head.as_str() && r["result"] == "fail"),
        "{gate:?}\n{text}"
    );
    assert!(
        gate.iter().all(|r| r["result"] == "fail"),
        "{gate:?}\n{text}"
    );
}

/// The project file is the base revision's too: one rewritten on the case branch to admit
/// another command for the gate's binding does not change what the gate runs.
#[test]
fn a_project_file_rewritten_on_the_case_branch_does_not_change_the_verdict() {
    let project = |executable: &str| {
        format!(
            "format: engineering-assertions-project/1\ncommands:\n  check:\n    executable: {executable}\n"
        )
    };
    let tree = repository(
        "project-base",
        r#"process.succeeded("check")"#,
        &[
            (".engineering/assertions-project.yaml", &project("fails.sh")),
            ("fails.sh", "#!/bin/sh\nexit 1\n"),
            ("passes.sh", "#!/bin/sh\nexit 0\n"),
        ],
    );
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(GateRun::new(".engineering/gates.yaml")),
    ];
    let rewritten = project("passes.sh");
    let harness = Writes::new(&[(".engineering/assertions-project.yaml", &rewritten)]);
    let mut run = Run::open(&tree, options("project-base", 8), &harness, &validators)
        .expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(harness.calls.get(), 2, "{stop:?}\n{text}");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let gate: Vec<_> = records(&run.case.dir)
        .into_iter()
        .filter(|r| r["kind"] == "gate_result")
        .collect();
    assert!(
        gate.iter()
            .any(|r| r["subject_revision"] == head.as_str() && r["result"] == "fail"),
        "{gate:?}\n{text}"
    );
    assert!(
        gate.iter().all(|r| r["result"] == "fail"),
        "{gate:?}\n{text}"
    );
}

/// Evidence is bound to `HEAD`, so a validator never observes a tree with uncommitted changes
/// outside the cases directory: the run stops naming them, writes no record and leaves them.
#[test]
fn a_validator_on_a_tree_with_uncommitted_changes_is_refused_naming_them() {
    let tree = repository("dirty", r#"fs.file.exists("README.md")"#, &[]);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let mut run =
        Run::open(&tree, options("dirty", 4), &NoHarness, &validators).expect("the case opens");
    fs::write(tree.join("README.md"), "changed by hand\n").expect("a change");
    // Staged, so the change's porcelain line starts with no blank, which `git::changed_paths`
    // trims away from the first line: what is named here is the path itself.
    git(&tree, &["add", "README.md"]);
    fs::write(tree.join("notes.txt"), "untracked\n").expect("an untracked file");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    let Ok(Stop::StepRefused(why)) = &stop else {
        panic!("the observation is refused, not {stop:?}\n{text}");
    };
    assert!(
        why.contains("README.md") && why.contains("notes.txt"),
        "{why}"
    );
    assert!(records(&run.case.dir).is_empty(), "{text}");
    assert_eq!(
        fs::read_to_string(tree.join("README.md")).expect("the readme"),
        "changed by hand\n"
    );
    assert!(tree.join("notes.txt").is_file());
}

/// The variable names `path` lists, one per line, as the dumping scripts below write them.
fn names(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A script that writes the names of its environment's variables to `names` and its
/// arguments, one per line, to `argv`, then prints `stdout`.
fn dumper(at: &Path, names: &Path, argv: &Path, stdout: &str) -> PathBuf {
    let script = at.to_path_buf();
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nenv | sed 's/=.*//' > '{}'\nprintf '%s\\n' \"$@\" > '{}'\nprintf '%s' '{stdout}'\n",
            names.display(),
            argv.display()
        ),
    )
    .expect("a script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("exec bit");
    script
}

/// What the fake `claude` prints: a result whose last line names `repository.inspect`.
const CLAUDE_REPLY: &str =
    r#"{"result": "{\"action\": \"repository.inspect\", \"summary\": \"read\"}"}"#;

/// One judgement step by `harness` on the delivery protocol with no validator.
fn one_step(tree: &Path, id: &str, harness: &dyn Harness) -> (anyhow::Result<Stop>, String) {
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(tree, options(id, 1), harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8(out).expect("utf-8"))
}

/// Run in a child test process given exactly `CREDENTIALS` (fake values), `PATH` and `HOME`:
/// the test command, the harness and `aep` receive none of them; the forge's programs keep
/// the GitHub ones the bot route reads and lose the rest.
#[test]
fn children_receive_no_credential_they_do_not_need() {
    if std::env::var_os("TAKT_CONFINEMENT_CHILD").is_none() {
        let mut command = Command::new(std::env::current_exe().expect("the test binary"));
        command
            .args([
                "children_receive_no_credential_they_do_not_need",
                "--exact",
                "--test-threads",
                "1",
            ])
            .env_clear()
            .env("TAKT_CONFINEMENT_CHILD", "1");
        for name in ["PATH", "HOME"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for name in CREDENTIALS {
            command.env(name, "fake-not-a-credential");
        }
        let output = command.output().expect("the child test runs");
        assert!(
            output.status.success(),
            "the child test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let scripts = directory("env-scripts");
    let leaked = |what: &str, names: &[String], keep: &[&str]| {
        let leaked: Vec<&str> = CREDENTIALS
            .iter()
            .copied()
            .filter(|name| !keep.contains(name) && names.iter().any(|n| n == name))
            .collect();
        assert!(leaked.is_empty(), "{what} received {leaked:?}");
        for name in keep {
            assert!(names.iter().any(|n| n == name), "{what} lost {name}");
        }
    };

    let tree = repository("env-tests", r#"fs.file.exists("README.md")"#, &[]);
    let tests_names = scripts.join("tests.names");
    dumper(
        &scripts.join("tests.sh"),
        &tests_names,
        &scripts.join("tests.argv"),
        "",
    );
    let command = format!("sh {}", scripts.join("tests.sh").display());
    TestsRun::new(&command)
        .expect("a command")
        .observe(&tree, "r0", 1)
        .expect("observes");
    leaked("the test command", &names(&tests_names), &[]);

    let claude_names = scripts.join("claude.names");
    let program = dumper(
        &scripts.join("claude.sh"),
        &claude_names,
        &scripts.join("claude.argv"),
        CLAUDE_REPLY,
    );
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let tree = repository("env-claude", r#"fs.file.exists("README.md")"#, &[]);
    let (stop, text) = one_step(&tree, "env-claude", &harness);
    stop.unwrap_or_else(|error| panic!("the step runs: {error:#}\n{text}"));
    leaked("the harness", &names(&claude_names), &[]);

    let store = directory("env-aep");
    fs::create_dir_all(store.join(".engineering")).expect("a store");
    fs::write(store.join(".engineering/project.yaml"), "name: x\n").expect("a project");
    let aep_names = scripts.join("aep.names");
    let aep = dumper(
        &scripts.join("aep.sh"),
        &aep_names,
        &scripts.join("aep.argv"),
        "[]",
    );
    Aep {
        command: aep.display().to_string(),
        ..Aep::default()
    }
    .list(&store)
    .expect("lists");
    leaked("aep", &names(&aep_names), &[]);

    let forge_names = scripts.join("forge.names");
    let gh = dumper(
        &scripts.join("gh.sh"),
        &forge_names,
        &scripts.join("gh.argv"),
        "",
    );
    let exit = Process::default()
        .run(&store, &[gh.display().to_string()])
        .expect("runs");
    assert!(exit.success(), "{exit:?}");
    leaked(
        "the forge",
        &names(&forge_names),
        &["GH_TOKEN", "GITHUB_TOKEN"],
    );
}

/// The harness child loads no project or user settings, hooks, plugins or MCP servers: its
/// argument line restricts it, and no live `claude` runs.
#[test]
fn the_harness_child_loads_no_project_or_user_settings() {
    let scripts = directory("claude-argv");
    let argv = scripts.join("claude.argv");
    let program = dumper(
        &scripts.join("claude.sh"),
        &scripts.join("claude.names"),
        &argv,
        CLAUDE_REPLY,
    );
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let tree = repository("claude-argv-tree", r#"fs.file.exists("README.md")"#, &[]);
    let (stop, text) = one_step(&tree, "claude-argv-tree", &harness);
    stop.unwrap_or_else(|error| panic!("the step runs: {error:#}\n{text}"));
    let line = names(&argv);
    for flag in [
        "--restricted",
        "--strict-mcp-config",
        "--no-session-persistence",
    ] {
        assert!(line.iter().any(|a| a == flag), "{flag} in {line:?}");
    }
    let settings = line
        .iter()
        .position(|a| a == "--settings")
        .map(|i| line[i + 1].as_str());
    assert_eq!(settings, Some(r#"{"disableAllHooks":true}"#), "{line:?}");
}

/// A script that sleeps in a child of its shell, so the child holds the output pipes after
/// the shell is killed, then prints.
const SLOW: &str = "#!/bin/sh\nsleep 30\necho late\n";

/// Short limits for the timeout cases.
fn short() -> Limits {
    Limits {
        timeout: Duration::from_millis(300),
        output: 4096,
    }
}

/// The run stops on a test command that outlives its timeout, the error naming the action and
/// the timeout, in seconds rather than the command's own thirty.
#[test]
fn a_test_command_past_its_timeout_is_killed_and_the_run_stops_naming_it() {
    let tree = repository(
        "slow-tests",
        r#"fs.file.exists("README.md")"#,
        &[("slow.sh", SLOW)],
    );
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("sh slow.sh")
            .expect("a command")
            .with_limits(short()),
    )];
    let mut run = Run::open(&tree, options("slow-tests", 4), &NoHarness, &validators)
        .expect("the case opens");
    let started = Instant::now();
    let error = run
        .run(&mut Vec::new())
        .expect_err("the run stops on the timeout");
    let error = format!("{error:#}");
    assert!(started.elapsed() < Duration::from_secs(10), "{error}");
    assert!(error.contains("tests.run"), "{error}");
    assert!(error.contains("timed out"), "{error}");
}

/// The harness child is killed after its timeout and the run stops naming it.
#[test]
fn the_harness_child_past_its_timeout_is_killed() {
    let scripts = directory("slow-claude");
    let program = scripts.join("claude.sh");
    fs::write(&program, SLOW).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    let harness = ClaudeCode {
        program: program.display().to_string(),
        limits: short(),
        ..ClaudeCode::default()
    };
    let tree = repository("slow-claude-tree", r#"fs.file.exists("README.md")"#, &[]);
    let started = Instant::now();
    let (stop, text) = one_step(&tree, "slow-claude-tree", &harness);
    let error = format!("{:#}", stop.expect_err("the run stops on the timeout"));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{error}\n{text}"
    );
    assert!(error.contains("timed out"), "{error}");
}

/// `aep` is killed after its timeout.
#[test]
fn aep_past_its_timeout_is_killed() {
    let store = directory("slow-aep");
    fs::create_dir_all(store.join(".engineering")).expect("a store");
    fs::write(store.join(".engineering/project.yaml"), "name: x\n").expect("a project");
    let program = store.join("aep.sh");
    fs::write(&program, SLOW).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    let aep = Aep {
        command: program.display().to_string(),
        limits: short(),
    };
    let started = Instant::now();
    let error = format!("{:#}", aep.list(&store).expect_err("the timeout"));
    assert!(started.elapsed() < Duration::from_secs(10), "{error}");
    assert!(error.contains("timed out"), "{error}");
}

/// The forge's programs are killed after their timeout.
#[test]
fn a_forge_program_past_its_timeout_is_killed() {
    let dir = directory("slow-forge");
    let program = dir.join("gh.sh");
    fs::write(&program, SLOW).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    let started = Instant::now();
    let error = Process { limits: short() }
        .run(&dir, &[program.display().to_string()])
        .expect_err("the timeout");
    let error = format!("{error:#}");
    assert!(started.elapsed() < Duration::from_secs(10), "{error}");
    assert!(error.contains("timed out"), "{error}");
}

/// Of what a child prints, only the last bytes up to the limit are kept.
#[test]
fn a_child_keeps_a_bounded_tail_of_its_output() {
    let dir = directory("loud");
    let limits = Limits {
        timeout: Duration::from_secs(60),
        output: 1024,
    };
    let exit = Process { limits }
        .run(
            &dir,
            &[
                "sh".to_owned(),
                "-c".to_owned(),
                "yes x | head -n 500000; echo last".to_owned(),
            ],
        )
        .expect("runs");
    assert!(exit.success(), "{}", exit.stderr);
    assert!(exit.stdout.len() <= 1024, "{}", exit.stdout.len());
    assert!(exit.stdout.ends_with("x\nlast\n"), "{:?}", exit.stdout);
}

/// `gh pr view` and `gh pr checks` name the repository: what they read is never the one the
/// tree's remotes say.
#[test]
fn the_forge_reads_name_the_repository() {
    let forge = BotRoute::new("owner/repo", None, Box::new(Process::default()));
    for argv in [forge.view_argv(7), forge.checks_argv(7)] {
        let at = argv.iter().position(|a| a == "--repo");
        assert_eq!(
            at.map(|i| argv[i + 1].as_str()),
            Some("owner/repo"),
            "{argv:?}"
        );
    }
}

/// `ci.read` reads nothing when the case's branch was last pushed at another revision, even
/// when the forge reports the pull request's head at this one: the pushed revision is checked
/// before the forge is asked.
#[test]
fn ci_read_reads_nothing_when_the_pushed_revision_is_another() {
    let tree = repository("ci-pushed", r#"fs.file.exists("README.md")"#, &[]);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let run =
        Run::open(&tree, options("ci-pushed", 1), &NoHarness, &validators).expect("the case opens");
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    let dir = current(&tree).expect("the case");
    let mut branch = Branch::read(&dir).expect("the branch");
    branch.pull_request = Some(1);
    branch.pushed = Some("0".repeat(40));
    branch.write(&dir).expect("written");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        head: Some(Head {
            branch: branch.name.clone(),
            sha: revision.clone(),
        }),
        ..Memory::default()
    });
    memory.opened.borrow_mut().push(PullRequest {
        branch: branch.name.clone(),
        base: "main".to_owned(),
        title: "t".to_owned(),
        body: "b".to_owned(),
    });
    let observation = CiRead::new(memory.clone())
        .observe(&tree, &revision, 3)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
    drop(run);
}

/// One run of the delivery protocol whose test command is `script`, written by the model under
/// `implementation.edit`: what it stopped with and printed, and the case directory.
fn tested_by(name: &str, script: &str) -> (Scratch, anyhow::Result<Stop>, String, PathBuf) {
    let tree = repository(name, r#"fs.file.exists("README.md")"#, &[]);
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("sh tests/run.sh").expect("a command"),
    )];
    let harness = Writes::new(&[("tests/run.sh", script)]);
    let mut run = Run::open(&tree, options(name, 6), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let dir = run.case.dir.clone();
    drop(run);
    (tree, stop, String::from_utf8(out).expect("utf-8"), dir)
}

/// A test command that plants a hook under `.git` is refused, the hook named and gone, and it
/// never runs: no later git call of takt's sees it.
#[test]
fn a_test_command_that_plants_a_hook_is_refused_and_the_hook_removed() {
    let script = "#!/bin/sh\nprintf '#!/bin/sh\\ntouch planted-hook-ran\\n' > .git/hooks/post-commit\nchmod +x .git/hooks/post-commit\nexit 0\n";
    let (tree, stop, text, _) = tested_by("hook", script);
    let Ok(Stop::StepRefused(why)) = &stop else {
        panic!("the observation is refused, not {stop:?}\n{text}");
    };
    assert!(why.contains(".git/hooks/post-commit"), "{why}");
    assert!(!tree.join(".git/hooks/post-commit").exists(), "{text}");
    assert!(!tree.join("planted-hook-ran").exists(), "{text}");
}

/// A test command that commits (moving `HEAD` under the record's revision) is refused and
/// `HEAD` put back.
#[test]
fn a_test_command_that_commits_is_refused_and_head_put_back() {
    let script = "#!/bin/sh\necho forged > forged.txt\ngit add forged.txt\ngit -c user.name=m -c user.email=m@localhost commit -q -m forged\nexit 0\n";
    let (tree, stop, text, _) = tested_by("commits", script);
    let Ok(Stop::StepRefused(why)) = &stop else {
        panic!("the observation is refused, not {stop:?}\n{text}");
    };
    assert!(why.contains("HEAD"), "{why}");
    assert_ne!(
        git(&tree, &["log", "-1", "--format=%s"]),
        "forged",
        "{text}"
    );
    assert!(!tree.join("forged.txt").exists(), "{text}");
}

/// takt's own git runs no code the repository's configuration names: not its `core.fsmonitor`,
/// not its hooks, whether in `.git/hooks` or under a `core.hooksPath`.
#[test]
fn takt_git_runs_no_code_the_repository_configures() {
    let tree = repository("configured", r#"fs.file.exists("README.md")"#, &[]);
    let marker = |name: &str| tree.join(format!("ran-{name}"));
    let hook = format!("#!/bin/sh\ntouch '{}'\n", marker("hook").display());
    for dir in [".git/hooks", "hooks"] {
        let path = tree.join(dir).join("post-commit");
        fs::create_dir_all(path.parent().expect("a parent")).expect("hooks");
        fs::write(&path, &hook).expect("a hook");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("exec bit");
    }
    let monitor = tree.join("monitor.sh");
    fs::write(
        &monitor,
        format!(
            "#!/bin/sh\ntouch '{}'\nexit 1\n",
            marker("fsmonitor").display()
        ),
    )
    .expect("a monitor");
    fs::set_permissions(&monitor, fs::Permissions::from_mode(0o755)).expect("exec bit");
    git(
        &tree,
        &["config", "core.fsmonitor", &monitor.display().to_string()],
    );
    git(&tree, &["config", "core.hooksPath", "hooks"]);
    fs::write(tree.join("README.md"), "changed\n").expect("a change");
    takt::git::changed_paths(&tree).expect("status");
    takt::git::commit_paths(&tree, &["README.md".to_owned()], "a change").expect("commits");
    assert!(!marker("fsmonitor").exists(), "core.fsmonitor ran");
    assert!(!marker("hook").exists(), "a post-commit hook ran");
}

/// A case opened before the base commit was pinned gets no gate record: no branch name stands
/// in for the pin. The summary says so and names the way out.
#[test]
fn an_unpinned_case_gets_no_gate_record_and_is_told_the_way_out() {
    let tree = repository("unpinned", r#"fs.file.exists("README.md")"#, &[]);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut run = Run::open(&tree, options("unpinned", 2), &NoHarness, &validators).expect("opens");
    let dir = current(&tree).expect("the case");
    let mut branch = Branch::read(&dir).expect("the branch");
    assert!(
        branch.base_commit.is_some(),
        "the base commit is pinned at open"
    );
    branch.base_commit = None;
    branch.write(&dir).expect("written");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    assert!(
        text.contains("the case pins no base commit"),
        "{stop:?}\n{text}"
    );
    assert!(
        text.contains("abandon the case and open it again"),
        "{stop:?}\n{text}"
    );
    assert!(!text.contains("evidence: gate_result"), "{stop:?}\n{text}");
    assert!(
        records(&dir).iter().all(|r| r["kind"] != "gate_result"),
        "{stop:?}\n{text}"
    );
}
