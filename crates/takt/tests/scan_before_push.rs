//! `pr.open` scans everything it is about to publish before anything leaves the machine: the
//! commit messages of `base..head`, the diff of every commit in that range, and, when it opens
//! the pull request, the title and the body. A scan that refuses stops `pr.open` with nothing
//! pushed, no pull request opened and no record written, and the refusal carries what the
//! policy said, never the text it refused.
//!
//! A recording runner stands in for `b10x-gates` and `gh`: `scan-text` refuses a file that holds
//! [`FORBIDDEN`], as a policy rule would, and says which file and line, never the text; every
//! line is kept with what each file `scan-text` names held. Nothing leaves the machine.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::forge::{BotRoute, Exit, Forge, Runner};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The protocol under test, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The policy file the forge is given; the runner reads nothing from it.
const POLICY: &str = "/policies/gates.json";

/// The literal the stand-in policy forbids.
const FORBIDDEN: &str = "FORBIDDEN-7Q2X";

/// Runs `git <args>` in `tree`; its trimmed standard output.
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
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch git tree with one commit on `main` and an `origin` on github.com, under this
/// crate's build directory. Dropping it removes the tree.
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
        .join(format!("scan-push-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    git(
        &dir,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/repo.git",
        ],
    );
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

/// One line the runner was given.
#[derive(Debug, Clone)]
struct Line {
    argv: String,
    /// What the line was started with beyond the runner's own environment.
    env: Vec<(String, String)>,
    /// What each file a `scan-text` line names held, in order; empty for other lines.
    scanned: Vec<String>,
}

/// `b10x-gates` and `gh`: `scan-text` refuses a file holding [`FORBIDDEN`] with the policy's
/// own words (the file, the line and the rule), `bot -- push` passes and keeps the commit it
/// pushed, or fails with `push_refusal` on stderr when that is set, `api` answers pull request
/// #7, `gh pr view` reports it open with the head last pushed for the case's branch.
#[derive(Default)]
struct Gates {
    lines: RefCell<Vec<Line>>,
    pushed: RefCell<Option<(String, String)>>,
    push_refusal: Option<String>,
}

impl Gates {
    fn lines(&self) -> Vec<Line> {
        self.lines.borrow().clone()
    }

    fn argvs(&self) -> Vec<String> {
        self.lines().into_iter().map(|line| line.argv).collect()
    }
}

fn exit(code: i32, stdout: &str, stderr: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

impl Runner for Gates {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let mut line = Line {
            argv: argv.join(" "),
            env: env
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            scanned: Vec::new(),
        };
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        let answer = match words.as_slice() {
            ["b10x-gates", "scan-text", rest @ ..] => {
                let mut refusal = None;
                let mut rest = rest.iter();
                while let Some(word) = rest.next() {
                    if word.starts_with("--") {
                        rest.next();
                        continue;
                    }
                    let text = fs::read_to_string(word)
                        .unwrap_or_else(|error| panic!("scan-text names {word}: {error}"));
                    if let Some(at) = text.lines().position(|l| l.contains(FORBIDDEN)) {
                        refusal.get_or_insert(format!(
                            "{word} breaks 1 private rule(s): line {} literal-7; private details remain local",
                            at + 1
                        ));
                    }
                    line.scanned.push(text);
                }
                match refusal {
                    Some(said) => exit(1, "", &said),
                    None => exit(0, "1 file(s) carry no private rule", ""),
                }
            }
            ["b10x-gates", "bot", "--", "push", ..] if self.push_refusal.is_some() => {
                exit(128, "", self.push_refusal.as_deref().unwrap_or_default())
            }
            ["b10x-gates", "bot", "--", "push", _, spec] => {
                let (revision, branch) = match spec.split_once(":refs/heads/") {
                    Some((revision, branch)) => (revision.to_owned(), branch.to_owned()),
                    None => (
                        git(tree, &["rev-parse", &format!("refs/heads/{spec}")]),
                        (*spec).to_owned(),
                    ),
                };
                *self.pushed.borrow_mut() = Some((branch, revision));
                exit(0, "", "")
            }
            ["b10x-gates", "api", ..] => {
                let output = argv
                    .iter()
                    .position(|a| a == "--output")
                    .map(|i| &argv[i + 1])
                    .expect("--output");
                fs::write(output, r#"{"number": 7}"#)?;
                exit(0, "", "")
            }
            ["gh", "pr", "view", ..] => {
                let (branch, sha) = self.pushed.borrow().clone().expect("a push before a view");
                let view = serde_json::json!({
                    "state": "OPEN",
                    "mergedAt": null,
                    "headRefName": branch,
                    "headRefOid": sha,
                });
                exit(0, &view.to_string(), "")
            }
            _ => exit(2, "", "the double does not know this line"),
        };
        self.lines.borrow_mut().push(line);
        Ok(answer)
    }
}

/// Lends a shared [`Gates`] to a forge that owns its runner.
struct Shared(Rc<Gates>);

impl Runner for Shared {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.0.run(tree, argv)
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.0.run_env(tree, argv, env)
    }
}

/// The bot route over `gates`, with a policy file.
fn bot(gates: &Rc<Gates>) -> BotRoute {
    BotRoute::new(
        "owner/repo",
        Some(PathBuf::from(POLICY)),
        Box::new(Shared(gates.clone())),
    )
}

/// Commits `path` holding `text` with the message `message`, as a step leaves it.
fn commit(tree: &Path, path: &str, text: &str, message: &str) {
    fs::write(tree.join(path), text).expect("a file");
    git(tree, &["add", "-A", "--", path]);
    git(tree, &["commit", "-q", "-m", message]);
}

/// What a run of `pr.open` left.
struct Ran {
    stop: Result<Stop, String>,
    text: String,
    case_dir: PathBuf,
    revision: String,
}

/// A run on the delivery protocol over `tree` with `pr.open` over `forge` as the only
/// validator: the case opened, `work` makes the implementation's commits, the tests and the
/// gate green at the last of them, one step: `pr.open`.
fn pr_open(tree: &Path, id: &str, forge: Rc<dyn Forge>, work: impl Fn(&Path)) -> Ran {
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
    work(tree);
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
                "scan-test",
            ))
            .expect("the record is written");
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    Ran {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
        case_dir: run.case.dir.clone(),
        revision,
    }
}

/// The kinds of the records the case holds.
fn kinds(case_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            let text = fs::read_to_string(entry.expect("an entry").path()).expect("a record");
            let record: serde_json::Value = serde_json::from_str(&text).expect("json");
            record["kind"].as_str().unwrap_or_default().to_owned()
        })
        .collect()
}

/// A refused scan: `pr.open` ran no line but the one scan, pushed nothing, opened nothing and
/// recorded nothing, and its refusal carries the policy's words, never the literal.
fn refused_before_anything_left(ran: &Ran, gates: &Gates) {
    let error = ran.stop.as_ref().expect_err("the scan refused");
    let argvs = gates.argvs();
    assert_eq!(argvs.len(), 1, "{argvs:#?}\n{error}\n{}", ran.text);
    assert!(argvs[0].starts_with("b10x-gates scan-text "), "{argvs:#?}");
    assert!(error.contains("scan-text"), "{error}");
    assert!(error.contains("private rule"), "{error}");
    assert!(
        !error.contains(FORBIDDEN),
        "the refusal carries the text: {error}"
    );
    assert!(!ran.text.contains(FORBIDDEN), "{}", ran.text);
    assert!(
        !kinds(&ran.case_dir).contains(&"pull_request".to_owned()),
        "{:?}",
        kinds(&ran.case_dir)
    );
    let branch = fs::read_to_string(ran.case_dir.join("branch.yaml")).expect("branch.yaml");
    assert!(!branch.contains("pushed"), "{branch}");
    assert!(!branch.contains("pull_request"), "{branch}");
}

/// The story's expected-before, turned: a step's commit subject carries a literal the policy
/// forbids. `pr.open` scans the commit messages of `base..head` before the push, the scan
/// refuses, and nothing is pushed, opened or recorded.
#[test]
fn a_commit_subject_the_policy_forbids_stops_pr_open_before_the_push() {
    let tree = scratch("subject");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "subject", Rc::new(bot(&gates)), |tree| {
        commit(
            tree,
            "work.txt",
            "clean\n",
            &format!("implementation.edit: notes on {FORBIDDEN}"),
        );
    });
    refused_before_anything_left(&ran, &gates);
    let scanned = &gates.lines()[0].scanned;
    assert!(
        scanned
            .iter()
            .any(|text| text.contains(&format!("implementation.edit: notes on {FORBIDDEN}"))),
        "{scanned:#?}"
    );
}

/// A literal the policy forbids in what a commit adds, under a clean message: the diff is
/// scanned before the push, and nothing leaves.
#[test]
fn a_diff_the_policy_forbids_stops_pr_open_before_the_push() {
    let tree = scratch("diff");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "diff", Rc::new(bot(&gates)), |tree| {
        commit(
            tree,
            "work.txt",
            &format!("token {FORBIDDEN}\n"),
            "implementation.edit: a clean summary",
        );
    });
    refused_before_anything_left(&ran, &gates);
}

/// A literal one commit adds and a later one removes is absent from the net diff of
/// `base..head`, but the push publishes both commits: the diff of every commit in the range is
/// scanned, and nothing leaves.
#[test]
fn a_literal_a_later_commit_removed_still_stops_the_push() {
    let tree = scratch("removed");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "removed", Rc::new(bot(&gates)), |tree| {
        commit(
            tree,
            "work.txt",
            &format!("token {FORBIDDEN}\n"),
            "implementation.edit: first",
        );
        commit(tree, "work.txt", "clean\n", "implementation.edit: second");
    });
    refused_before_anything_left(&ran, &gates);
}

/// A clean case: the one scan before the push reads the request, the title and body as
/// published, the commit messages of `base..head` and their diff; then the push, then the
/// pull request.
#[test]
fn a_clean_case_is_scanned_whole_before_the_push_and_then_opened() {
    let tree = scratch("clean");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "clean", Rc::new(bot(&gates)), |tree| {
        commit(
            tree,
            "work.txt",
            "a line of work\n",
            "implementation.edit: a clean summary",
        );
    });
    assert_eq!(ran.stop, Ok(Stop::Iterations(1)), "{}", ran.text);
    assert!(ran.text.contains("opened pull request #7"), "{}", ran.text);
    let lines = gates.lines();
    let argvs = gates.argvs();
    let push = argvs
        .iter()
        .position(|argv| argv.starts_with("b10x-gates bot -- push "))
        .expect("a push");
    assert!(push > 0, "{argvs:#?}");
    let first = &lines[0];
    assert!(
        first.argv.starts_with("b10x-gates scan-text "),
        "{argvs:#?}"
    );
    assert!(
        first.argv.contains(&format!("--policy {POLICY}")),
        "{}",
        first.argv
    );
    let all = first.scanned.join("\n");
    assert!(
        all.contains("implementation.edit: a clean summary"),
        "{all}"
    );
    assert!(all.contains("+a line of work"), "{all}");
    assert!(
        all.contains("deliver the story\n\nThe story's body."),
        "{all}"
    );
    assert!(
        argvs[push + 1..]
            .iter()
            .any(|argv| argv.starts_with("b10x-gates api --method POST "))
    );
    assert!(
        kinds(&ran.case_dir).contains(&"pull_request".to_owned()),
        "{}",
        ran.text
    );
}

/// At a later revision, with the pull request open, `pr.open` pushes the branch again: the new
/// commit is scanned before that push, and one the policy refuses is not pushed.
#[test]
fn a_later_revision_is_scanned_before_it_is_pushed_again() {
    let tree = scratch("later");
    let gates = Rc::new(Gates::default());
    let forge: Rc<dyn Forge> = Rc::new(bot(&gates));
    let ran = pr_open(&tree, "later", forge.clone(), |tree| {
        commit(tree, "work.txt", "first\n", "implementation.edit: first");
    });
    assert_eq!(ran.stop, Ok(Stop::Iterations(1)), "{}", ran.text);
    let before = gates.argvs().len();
    commit(
        &tree,
        "work.txt",
        "second\n",
        &format!("implementation.edit: second, {FORBIDDEN}"),
    );
    let second = git(&tree, &["rev-parse", "HEAD"]);
    assert_ne!(second, ran.revision);
    let error = PrOpen::new(forge)
        .observe(&tree, &second, 5)
        .map(|observation| observation.summary)
        .expect_err("the scan refuses the second revision");
    let error = format!("{error:#}");
    let after: Vec<String> = gates.argvs().split_off(before);
    assert_eq!(after.len(), 1, "{after:#?}\n{error}");
    assert!(after[0].starts_with("b10x-gates scan-text "), "{after:#?}");
    assert!(!error.contains(FORBIDDEN), "{error}");
    let branch = fs::read_to_string(ran.case_dir.join("branch.yaml")).expect("branch.yaml");
    assert!(branch.contains(&ran.revision), "{branch}");
    assert!(!branch.contains(&second), "{branch}");
}

/// The push line, and only it, is started with HTTPS as the one transport git may use.
#[test]
fn the_push_alone_runs_with_https_as_the_one_transport() {
    let tree = scratch("https");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "https", Rc::new(bot(&gates)), |tree| {
        commit(tree, "work.txt", "work\n", "implementation.edit: work");
    });
    assert_eq!(ran.stop, Ok(Stop::Iterations(1)), "{}", ran.text);
    let https = vec![("GIT_ALLOW_PROTOCOL".to_owned(), "https".to_owned())];
    for line in gates.lines() {
        let expected = if line.argv.starts_with("b10x-gates bot -- push ") {
            https.clone()
        } else {
            Vec::new()
        };
        assert_eq!(line.env, expected, "{}", line.argv);
    }
}

/// Real git behind the push: the bot route's words dropped, `git push` runs as a forge child
/// with what the forge starts the push with, and, as the bot route's git does, with no user or
/// system configuration. `scan-text` goes to [`Gates`].
struct RealPush(Rc<Gates>);

impl Runner for RealPush {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        if argv.starts_with(&["b10x-gates", "bot", "--", "push"].map(str::to_owned)) {
            let git: Vec<String> = std::iter::once("git".to_owned())
                .chain(argv[3..].iter().cloned())
                .collect();
            let mut env = env.to_vec();
            env.extend([
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
                ("GIT_CONFIG_NOSYSTEM", "1"),
            ]);
            return takt::forge::Process::default().run_env(tree, &git, &env);
        }
        self.0.run_env(tree, argv, env)
    }
}

/// A remote that would push over SSH, named so or rewritten so by the repository's own
/// configuration: git refuses the transport before it starts `ssh`, and `pr.open` stops naming
/// the remote, its push URL and the transport, with nothing pushed, opened or recorded.
#[test]
fn a_remote_that_would_push_over_ssh_is_refused_naming_it() {
    for (n, (origin, rewrite, shown)) in [
        (
            "ssh://git@localhost/owner/repo.git",
            None,
            "ssh://localhost/owner/repo.git",
        ),
        (
            "https://github.com/owner/repo.git",
            Some("url.ssh://git@localhost/.insteadOf"),
            "https://github.com/owner/repo.git",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let tree = scratch(&format!("ssh-{n}"));
        git(&tree, &["remote", "set-url", "origin", origin]);
        if let Some(key) = rewrite {
            git(&tree, &["config", key, "https://github.com/"]);
        }
        let gates = Rc::new(Gates::default());
        let forge = BotRoute::new(
            "owner/repo",
            Some(PathBuf::from(POLICY)),
            Box::new(RealPush(gates.clone())),
        );
        let id = format!("ssh-{n}");
        let ran = pr_open(&tree, &id, Rc::new(forge), |tree| {
            commit(tree, "work.txt", "work\n", "implementation.edit: work");
        });
        let error = ran.stop.as_ref().expect_err("the push is refused");
        assert!(error.contains("the remote origin"), "{error}");
        assert!(error.contains(shown), "{error}");
        assert!(error.contains("would push over ssh"), "{error}");
        assert!(error.contains("nothing was pushed"), "{error}");
        let argvs = gates.argvs();
        assert_eq!(argvs.len(), 1, "{argvs:#?}");
        assert!(argvs[0].starts_with("b10x-gates scan-text "), "{argvs:#?}");
        assert!(
            !kinds(&ran.case_dir).contains(&"pull_request".to_owned()),
            "{error}"
        );
    }
}

/// Messages and diff over what one scan is given are refused, not cut: nothing is scanned,
/// pushed, opened or recorded, and the refusal names the size and the limit.
#[test]
fn messages_and_diff_over_the_limit_are_refused_not_cut() {
    let tree = scratch("large");
    let gates = Rc::new(Gates::default());
    let mut forge = bot(&gates);
    forge.scan_limit = 256;
    let ran = pr_open(&tree, "large", Rc::new(forge), |tree| {
        commit(
            tree,
            "work.txt",
            &"a line of work\n".repeat(40),
            "implementation.edit: work",
        );
    });
    let error = ran.stop.as_ref().expect_err("too large to scan");
    assert!(error.contains("over the 256 bytes"), "{error}");
    assert!(error.contains("refused, not cut"), "{error}");
    assert!(error.contains("nothing was pushed"), "{error}");
    assert!(gates.argvs().is_empty(), "{:#?}", gates.argvs());
    assert!(
        !kinds(&ran.case_dir).contains(&"pull_request".to_owned()),
        "{error}"
    );
}
