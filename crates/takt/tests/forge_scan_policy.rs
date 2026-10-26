//! `pr.open` through the bot route scans the pull request under the forge's own policy file
//! and repository, the same ones its API write carries; it scans the title and body as they
//! are published, beside the JSON request, once with the commits before the push and once
//! before the API write; and a scan that refuses after the case's branch was pushed says the
//! branch left, never that nothing did.
//!
//! What this proves about the policy: with one given to `BotRoute::new`, the scan and the API
//! write both carry it. From the command line the interface is `B10X_GATES_POLICY`: the forge
//! reads it and `b10x-gates` reads it again from the environment it inherits, so this change
//! moves nothing there, and a run with the variable unset and no default policy file still
//! stops at the scan.
//!
//! A recording runner stands in for `b10x-gates` and `gh` on a machine with no default policy
//! file: `scan-text` and `api` refuse unless the line names `--policy` or the environment the
//! fake is given names one, as the real program does. Nothing leaves the machine.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::forge::{BotRoute, Exit, Forge, POLICY_VAR, Runner};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The protocol under test, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

/// The policy file the forge is given; nothing is read from it, the runner only checks that a
/// line names it.
const POLICY: &str = "/policies/gates.json";

/// What `b10x-gates` says when it falls back to a default policy file that is not there.
const NO_DEFAULT_POLICY: &str =
    "b10x-gates: protected file <home>/.config/b10x/gates/policy.json unavailable: not found";

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

/// A scratch git tree with one commit on `main`, under this crate's build directory. Dropping
/// it removes the tree; a removal that fails fails the case, unless the case is already failing.
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
        .join(format!("forge-scan-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A harness the runs below never reach: every step they take is a validator's.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// `b10x-gates` on a machine with no default policy file. Like the real program it takes the
/// policy from `--policy`, else from `B10X_GATES_POLICY` in the environment it inherits
/// (`environment`; the forge keeps `B10X_*` in a child's environment), else from the default
/// file, which is not there. `bot -- push` passes; `scan-text` and `api` refuse without a
/// policy, and `scan-text` refuses anyway after a push when `refuse_scan` is set. `api` writes
/// pull request #7 to its `--output`. Every line is recorded, and what each file `scan-text`
/// names held.
struct Gates {
    lines: RefCell<Vec<String>>,
    scanned: RefCell<Vec<String>>,
    refuse_scan: bool,
    environment: Option<PathBuf>,
}

impl Gates {
    /// Under the environment this process would hand the program.
    fn new(refuse_scan: bool) -> Rc<Self> {
        Self::under(refuse_scan, std::env::var_os(POLICY_VAR).map(PathBuf::from))
    }

    /// Under an environment whose `B10X_GATES_POLICY` is `environment`.
    fn under(refuse_scan: bool, environment: Option<PathBuf>) -> Rc<Self> {
        Rc::new(Self {
            lines: RefCell::new(Vec::new()),
            scanned: RefCell::new(Vec::new()),
            refuse_scan,
            environment,
        })
    }

    fn lines(&self) -> Vec<String> {
        self.lines.borrow().clone()
    }

    fn scanned(&self) -> Vec<String> {
        self.scanned.borrow().clone()
    }
}

fn refused(stderr: &str) -> Exit {
    Exit {
        code: Some(1),
        stdout: String::new(),
        stderr: stderr.to_owned(),
    }
}

impl Runner for Gates {
    /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
    /// dropped.
    fn run_env(
        &self,
        tree: &std::path::Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.join(" "));
        let command = argv.get(1).map(String::as_str);
        let policy = argv.iter().any(|a| a == "--policy") || self.environment.is_some();
        if command == Some("scan-text") {
            // Every word after the subcommand that is neither an option nor its value.
            let mut words = argv.iter().skip(2);
            while let Some(word) = words.next() {
                if word.starts_with("--") {
                    words.next();
                    continue;
                }
                let text = fs::read_to_string(word)
                    .unwrap_or_else(|error| panic!("scan-text names {word}: {error}"));
                self.scanned.borrow_mut().push(text);
            }
        }
        match command {
            Some("scan-text" | "api") if !policy => return Ok(refused(NO_DEFAULT_POLICY)),
            Some("scan-text")
                if self.refuse_scan
                    && self
                        .lines
                        .borrow()
                        .iter()
                        .any(|line| line.contains(" push ")) =>
            {
                return Ok(refused("a private rule"));
            }
            Some("api") => {
                let output = argv
                    .iter()
                    .position(|a| a == "--output")
                    .map(|i| &argv[i + 1])
                    .expect("--output");
                fs::write(output, r#"{"number": 7, "state": "open"}"#)?;
            }
            _ => {}
        }
        Ok(Exit {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// Lends a shared [`Gates`] to a forge that owns its runner.
struct Shared(Rc<Gates>);

impl Runner for Shared {
    /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
    /// dropped.
    fn run_env(
        &self,
        tree: &std::path::Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }

    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.0.run(tree, argv)
    }
}

/// The pair `option value` in `argv`, if it is there.
fn option<'a>(argv: &'a [String], name: &str) -> Option<&'a str> {
    argv.iter()
        .position(|a| a == name)
        .and_then(|i| argv.get(i + 1))
        .map(String::as_str)
}

/// The story's acceptance on the argument line: with a policy file, `scan-text` names it and
/// the repository, exactly as the API write does, and the scanned file comes last.
#[test]
fn scan_argv_carries_the_forge_policy() {
    let forge = BotRoute::new(
        "owner/repo",
        Some(PathBuf::from(POLICY)),
        Box::new(Shared(Gates::new(false))),
    );
    let path = Path::new("artifacts/pull-request.request.json");
    assert_eq!(
        forge.scan_argv(path).join(" "),
        format!(
            "b10x-gates scan-text --repository owner/repo --policy {POLICY} artifacts/pull-request.request.json"
        )
    );
}

/// For a forge with a policy file and one without, every `--policy` and `--repository` the API
/// write carries, `scan-text` carries with the same value, and none the API write does not.
#[test]
fn scan_argv_carries_what_the_api_write_carries_with_or_without_a_policy() {
    for policy in [Some(PathBuf::from(POLICY)), None] {
        let forge = BotRoute::new(
            "owner/repo",
            policy.clone(),
            Box::new(Shared(Gates::new(false))),
        );
        let scan = forge.scan_argv(Path::new("in.json"));
        let open = forge.open_argv(Path::new("in.json"), Path::new("out.json"));
        let merge = forge.merge_argv(7, Path::new("in.json"), Path::new("out.json"));
        for name in ["--policy", "--repository"] {
            assert_eq!(
                option(&scan, name),
                option(&open, name),
                "{policy:?} {name}"
            );
            assert_eq!(
                option(&scan, name),
                option(&merge, name),
                "{policy:?} {name}"
            );
        }
        assert_eq!(scan.last().map(String::as_str), Some("in.json"), "{scan:?}");
    }
    let forge = BotRoute::new("owner/repo", None, Box::new(Shared(Gates::new(false))));
    assert_eq!(
        forge.scan_argv(Path::new("in.json")).join(" "),
        "b10x-gates scan-text --repository owner/repo in.json"
    );
}

/// A run on the delivery protocol over `tree` with `pr.open` over `forge` as the only validator,
/// the tests and the gate already green at an implementation commit, one step: `pr.open`. How
/// the run ended (or the error it ended with, in full), and what it printed.
fn pr_open(tree: &Path, id: &str, forge: Rc<dyn Forge>) -> (Result<Stop, String>, String) {
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story\n\nThe story's body.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let mut run = Run::open(tree, options, &NoHarness, &validators).expect("the case opens");
    fs::write(tree.join("implemented.txt"), "done\n").expect("a file");
    git(tree, &["add", "implemented.txt"]);
    git(tree, &["commit", "-q", "-m", "implemented by the test"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for (n, kind) in [(1, "test_result"), (2, "gate_result")] {
        run.case
            .add_evidence(&takt::evidence::Record::new(
                format!("e{n}"),
                kind,
                Some("pass".to_owned()),
                "implementation",
                &revision,
                "forge-test",
            ))
            .expect("the record is written");
    }
    // The change handed over at the revision, as takt records a `change.handover` step.
    run.case
        .add_evidence(&takt::evidence::Record::new(
            "handover",
            "handover",
            None,
            "implementation",
            &revision,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("the record is written");
    // A change to publish at the revision, as `change.read` records it.
    run.case
        .add_evidence(&takt::evidence::Record::new(
            "change-record",
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            &revision,
            "takt.change-read",
        ))
        .expect("the record is written");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8(out).expect("utf-8"))
}

/// The file the scan reads the pull request from as it is published, under the case's
/// artifacts directory.
const PUBLISHED: &str = "pull-request.published.txt";

/// With a policy given to `BotRoute::new` and none in the environment `b10x-gates` inherits,
/// the scans and the API write all carry it, and the pull request is opened. Before the push
/// one `scan-text` line reads the JSON request, the title and body as published, decoded, and
/// the commits' messages and diff; after it one reads the JSON request and, last, the title and
/// body as published.
#[test]
fn with_a_policy_given_to_the_forge_pr_open_opens_the_pull_request() {
    let tree = scratch("opens");
    let gates = Gates::under(false, None);
    let forge = Rc::new(BotRoute::new(
        "owner/repo",
        Some(PathBuf::from(POLICY)),
        Box::new(Shared(gates.clone())),
    ));
    let (stop, text) = pr_open(&tree, "opens", forge);
    assert_eq!(stop, Ok(Stop::Iterations(1)), "{text}");
    assert!(text.contains("step 1: pr.open {}"), "{text}");
    assert!(text.contains("opened pull request #7"), "{text}");
    assert!(text.contains("evidence: pull_request open"), "{text}");
    let lines = gates.lines();
    assert_eq!(lines.len(), 4, "{lines:?}");
    let artifacts = tree.join(".engineering/cases/opens/artifacts");
    assert_eq!(
        lines[0],
        format!(
            "b10x-gates scan-text --repository owner/repo --policy {POLICY} {} {} {} {}",
            artifacts.join("pull-request.request.json").display(),
            artifacts.join(PUBLISHED).display(),
            artifacts.join("push.messages.txt").display(),
            artifacts.join("push.files.txt").display()
        ),
        "{lines:?}"
    );
    assert_eq!(
        lines[1],
        format!(
            "b10x-gates bot -- push https://github.com/owner/repo.git {}:refs/heads/takt/opens --no-follow-tags --recurse-submodules=no",
            git(&tree, &["rev-parse", "refs/heads/takt/opens"])
        ),
        "{lines:?}"
    );
    assert_eq!(
        lines[2],
        format!(
            "b10x-gates scan-text --repository owner/repo --policy {POLICY} {} {}",
            artifacts.join("pull-request.request.json").display(),
            artifacts.join(PUBLISHED).display()
        ),
        "{lines:?}"
    );
    assert!(
        lines[3].starts_with("b10x-gates api --method POST "),
        "{lines:?}"
    );
    let scanned = gates.scanned();
    assert_eq!(scanned.len(), 6, "{scanned:?}");
    let published = &scanned[1];
    assert!(
        published.starts_with("deliver the story\n\n"),
        "{published}"
    );
    assert!(
        published.contains("deliver the story\n\nThe story's body."),
        "{published}"
    );
    assert!(!published.contains("\\n"), "{published}");
}

/// The command line's state with `B10X_GATES_POLICY` unset and no default policy file: the
/// forge has no policy, no line names one, and the run stops at the scan before the push, saying
/// nothing was pushed. The variable is the interface; this change does not replace it.
#[test]
fn without_the_variable_the_run_stops_at_the_scan_before_anything_is_pushed() {
    let tree = scratch("unset");
    let gates = Gates::under(false, None);
    let forge = Rc::new(BotRoute::new(
        "owner/repo",
        None,
        Box::new(Shared(gates.clone())),
    ));
    let (stop, text) = pr_open(&tree, "unset", forge);
    let error = stop.expect_err("no policy anywhere");
    assert!(error.contains("protected file"), "{error}\n{text}");
    assert!(error.contains("nothing was pushed"), "{error}");
    let lines = gates.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("b10x-gates scan-text "), "{lines:?}");
    assert!(
        !lines.iter().any(|line| line.contains("--policy")),
        "{lines:?}"
    );
}

/// The same state with `B10X_GATES_POLICY` set for `b10x-gates` and no policy given to the
/// forge: no line names `--policy`, the program reads the variable, and the pull request is
/// opened.
#[test]
fn with_the_variable_in_the_environment_the_program_reads_it_itself() {
    let tree = scratch("variable");
    let gates = Gates::under(false, Some(PathBuf::from(POLICY)));
    let forge = Rc::new(BotRoute::new(
        "owner/repo",
        None,
        Box::new(Shared(gates.clone())),
    ));
    let (stop, text) = pr_open(&tree, "variable", forge);
    assert_eq!(stop, Ok(Stop::Iterations(1)), "{text}");
    assert!(text.contains("opened pull request #7"), "{text}");
    let lines = gates.lines();
    assert!(
        !lines.iter().any(|line| line.contains("--policy")),
        "{lines:?}"
    );
}

/// A scan that refuses the body after the case's branch was pushed (the scan before the push
/// passed it) stops the run with an error that names the pushed branch, the published text it
/// scanned and says the pull request was not opened; it never says that nothing left the
/// machine. The API write is not run.
#[test]
fn a_scan_refusal_after_the_push_names_the_pushed_branch() {
    let tree = scratch("refused");
    let gates = Gates::under(true, None);
    let forge = Rc::new(BotRoute::new(
        "owner/repo",
        Some(PathBuf::from(POLICY)),
        Box::new(Shared(gates.clone())),
    ));
    let (stop, text) = pr_open(&tree, "refused", forge);
    let error = stop.expect_err("the scan refused");
    assert!(!error.contains("nothing left the machine"), "{error}");
    assert!(error.contains("scan-text"), "{error}");
    assert!(error.contains("a private rule"), "{error}");
    assert!(
        error.contains("the branch takt/refused was pushed"),
        "{error}"
    );
    assert!(error.contains("not opened"), "{error}");
    assert!(error.contains(PUBLISHED), "{error}");
    let lines = gates.lines();
    assert_eq!(lines.len(), 3, "{lines:?}\n{text}");
    assert!(lines[0].starts_with("b10x-gates scan-text "), "{lines:?}");
    assert_eq!(
        lines[1],
        format!(
            "b10x-gates bot -- push https://github.com/owner/repo.git {}:refs/heads/takt/refused --no-follow-tags --recurse-submodules=no",
            git(&tree, &["rev-parse", "refs/heads/takt/refused"])
        ),
        "{lines:?}"
    );
    assert!(lines[2].starts_with("b10x-gates scan-text "), "{lines:?}");
}
