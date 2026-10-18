//! `pr.open` scans everything it is about to publish before anything leaves the machine: the
//! commits the push would publish (their identities, messages and diffs, the diff's lines as the
//! files hold them) and, when it opens the pull request, the title and the body; so does
//! `repository.merge` before it pushes takt's riders. A scan that refuses stops the step with
//! nothing pushed, no pull request opened, nothing merged and no record written, and the refusal
//! carries what the policy said, never the text it refused.
//!
//! A recording runner stands in for `b10x-gates` and `gh`: `scan-text` refuses a file that holds
//! [`FORBIDDEN`], as a policy rule would, and says which file and line, never the text; every
//! line is kept with what each file `scan-text` names held. Nothing leaves the machine.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{anyhow, bail};
use takt::forge::{
    BotRoute, Checks, Exit, Forge, Memory, Outgoing, PullRequest, PullRequestView, Runner,
};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, TestsRun, Validator};
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
    assert!(all.lines().any(|line| line == "a line of work"), "{all}");
    assert!(
        !all.contains("+a line of work"),
        "the diff is scanned with its markers: {all}"
    );
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
/// system configuration. Any HTTPS it would open goes to a proxy on a closed local port, so
/// nothing leaves the machine. `scan-text` goes to [`Gates`].
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
                ("HTTPS_PROXY", "http://127.0.0.1:9"),
                ("https_proxy", "http://127.0.0.1:9"),
            ]);
            return takt::forge::Process::default().run_env(tree, &git, &env);
        }
        self.0.run_env(tree, argv, env)
    }
}

/// The push goes to the repository's HTTPS URL. A URL rewrite in the repository's own
/// configuration that would send it over SSH (`insteadOf`, `pushInsteadOf`): git refuses the
/// transport before it starts `ssh`, and `pr.open` stops naming the URL and the transport, with
/// nothing pushed, opened or recorded.
#[test]
fn a_rewrite_that_would_push_over_ssh_is_refused_naming_it() {
    for (n, key) in [
        "url.ssh://git@localhost/.insteadOf",
        "url.ssh://git@localhost/.pushInsteadOf",
    ]
    .into_iter()
    .enumerate()
    {
        let tree = scratch(&format!("ssh-{n}"));
        git(&tree, &["config", key, "https://github.com/"]);
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
        assert!(
            error.contains("the push to https://github.com/owner/repo.git would go over ssh"),
            "{error}"
        );
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

/// A push goes to the repository's URL, not to the remote's name, so it moves no tracking ref:
/// those are guarded from a validator as from a step, and the scan reads them as what the
/// remote holds.
#[test]
fn the_push_names_the_repository_url_not_the_remote() {
    let gates = Rc::new(Gates::default());
    let forge = bot(&gates);
    assert_eq!(
        forge.push_argv("takt/c").join(" "),
        "b10x-gates bot -- push https://github.com/owner/repo.git takt/c"
    );
    assert_eq!(
        forge.push_revision_argv("takt/c", "abc").join(" "),
        "b10x-gates bot -- push https://github.com/owner/repo.git abc:refs/heads/takt/c"
    );
}
/// Messages and files over what one scan is given are refused, not cut: nothing is scanned,
/// pushed, opened or recorded, and the refusal names the size and the limit.
#[test]
fn messages_and_files_over_the_limit_are_refused_not_cut() {
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
    assert!(error.contains("more than the 256 bytes"), "{error}");
    assert!(error.contains("refused, not cut"), "{error}");
    assert!(error.contains("nothing was pushed"), "{error}");
    assert!(gates.argvs().is_empty(), "{:#?}", gates.argvs());
    assert!(
        !kinds(&ran.case_dir).contains(&"pull_request".to_owned()),
        "{error}"
    );
}

/// What the one scan before the push read last: the files the commits add or change.
fn scanned_files(gates: &Gates) -> String {
    gates
        .lines()
        .into_iter()
        .find(|line| line.argv.starts_with("b10x-gates scan-text "))
        .and_then(|line| line.scanned.last().cloned())
        .expect("a scan before the push")
}

/// Every file a commit adds or changes is scanned whole, as the commit holds it, under a line
/// naming it: no diff markers, no removed line between two the file holds next to each other,
/// and no context bound, so a literal wrapped across those lines is next to itself for the
/// scanner's wrapped rule.
#[test]
fn every_file_a_commit_changes_is_scanned_whole() {
    let (head, tail) = FORBIDDEN.split_at(6);
    let tree = scratch("lines");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "lines", Rc::new(bot(&gates)), |tree| {
        commit(
            tree,
            "notes.md",
            &format!("about {head}\nmiddle\n{tail} tail\n"),
            "implementation.edit: notes",
        );
        commit(
            tree,
            "notes.md",
            &format!("about {head}\n{tail} tail\n"),
            "implementation.edit: fewer notes",
        );
    });
    assert_eq!(ran.stop, Ok(Stop::Iterations(1)), "{}", ran.text);
    let files = scanned_files(&gates);
    let second = git(&tree, &["rev-parse", "HEAD"]);
    assert!(
        files.contains(&format!(
            "file notes.md at {second}\nabout {head}\n{tail} tail\n"
        )),
        "{files}"
    );
    assert!(
        files.contains(&format!("about {head}\nmiddle\n{tail} tail\n")),
        "{files}"
    );
    assert!(!files.contains("+about"), "{files}");
}

/// A file name that is not ASCII is scanned as git holds it, not octal-escaped.
#[test]
fn a_file_name_is_scanned_as_it_is() {
    let tree = scratch("names");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "names", Rc::new(bot(&gates)), |tree| {
        commit(tree, "grüße.txt", "hallo\n", "implementation.edit: greet");
    });
    assert_eq!(ran.stop, Ok(Stop::Iterations(1)), "{}", ran.text);
    let files = scanned_files(&gates);
    assert!(files.contains("file grüße.txt at "), "{files}");
}
/// With no tracking ref of the base on the remote, every commit of the head is scanned: a local
/// commit on `main` the remote may not have is refused.
#[test]
fn with_no_tracking_ref_the_bases_own_commits_are_scanned() {
    let tree = scratch("untracked-base");
    commit(
        &tree,
        "local.txt",
        &format!("token {FORBIDDEN}\n"),
        "local work",
    );
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "untracked-base", Rc::new(bot(&gates)), |tree| {
        commit(tree, "work.txt", "clean\n", "implementation.edit: clean");
    });
    refused_before_anything_left(&ran, &gates);
}

/// With tracking refs of the remote, what they hold is not scanned again: a literal in a
/// commit `origin/main` holds does not stop the push, and the scanned messages hold only the
/// case's commit.
#[test]
fn a_commit_the_remote_holds_is_not_scanned_again() {
    let tree = scratch("tracked-base");
    commit(
        &tree,
        "old.txt",
        &format!("token {FORBIDDEN}\n"),
        "published long ago",
    );
    let main = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["update-ref", "refs/remotes/origin/main", &main]);
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "tracked-base", Rc::new(bot(&gates)), |tree| {
        commit(tree, "work.txt", "clean\n", "implementation.edit: clean");
    });
    assert_eq!(ran.stop, Ok(Stop::Iterations(1)), "{}", ran.text);
    let scanned = gates.lines()[0].scanned.join("\n");
    assert!(scanned.contains("implementation.edit: clean"), "{scanned}");
    assert!(!scanned.contains("published long ago"), "{scanned}");
    assert!(scanned.contains("author t <t@localhost>"), "{scanned}");
}

/// git's output is read no further than the limit: past it the answer is none.
#[test]
fn git_output_past_the_limit_is_not_read() {
    let tree = scratch("bounded");
    commit(&tree, "work.txt", &"x\n".repeat(100), "work");
    let all = takt::git::run_bounded(&tree, &["log", "--patch"], 1 << 20)
        .expect("git runs")
        .expect("under the limit");
    assert!(all.len() > 64, "{all}");
    assert_eq!(
        takt::git::run_bounded(&tree, &["log", "--patch"], 64).expect("git runs"),
        None
    );
}

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// A forge over [`Memory`] that logs `scan <branch>` for each scan and the pushes as the
/// double does, and refuses a scan while `refuse` is set.
#[derive(Default)]
struct Scanning {
    memory: Memory,
    refuse: Cell<bool>,
    log: RefCell<Vec<String>>,
}

impl Forge for Scanning {
    fn name(&self) -> &str {
        "scanning"
    }

    fn scan_outgoing(&self, _: &Path, _: &Path, outgoing: &Outgoing<'_>) -> anyhow::Result<()> {
        self.log
            .borrow_mut()
            .push(format!("scan {} {}", outgoing.branch, outgoing.head));
        if self.refuse.get() {
            bail!("scan-text: 1 private rule(s); private details remain local");
        }
        Ok(())
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.log.borrow_mut().push(format!("push {branch}"));
        self.memory.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.log
            .borrow_mut()
            .push(format!("push {branch} {revision}"));
        self.memory.push_revision(tree, branch, revision)
    }

    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &PullRequest,
    ) -> anyhow::Result<u64> {
        self.memory.open_pull_request(tree, dir, request)
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.memory.pull_request(tree, number)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.memory.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.log.borrow_mut().push(format!("merge #{number}"));
        self.memory.merge(tree, dir, number, head)
    }
}

/// Requests the merge gate as soon as it is offered, changing nothing.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!("the gate was not offered"))
    }
}

/// A delivery run's validators over `forge`: tests (`true`), the gate as `TestsRun` would not
/// do, so a gate that always passes, and the forge's three.
fn delivery(forge: &Rc<Scanning>) -> Vec<Box<dyn Validator>> {
    let forge: Rc<dyn Forge> = forge.clone();
    vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(GatePass),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// `gate.run` without a gate: one `gate_result` pass at the revision.
struct GatePass;

impl Validator for GatePass {
    fn name(&self) -> &str {
        "record.gate-pass"
    }

    fn action(&self) -> &str {
        "gate.run"
    }

    fn kind(&self) -> &str {
        "gate_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(
        &self,
        _: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<takt::validator::Observation, anyhow::Error> {
        Ok(takt::validator::Observation {
            summary: "pass".to_owned(),
            records: vec![takt::evidence::Record::new(
                format!("gate-result-{seq}"),
                self.kind(),
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
            artifact: None,
        })
    }
}

/// The options of a delivery run of the fixture store's story.
fn delivery_options() -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
    }
}

/// Copies the directory `from` to `to`.
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

/// The fixture store's story taken, worked, opened through `forge`, its checks passing, and
/// granted the merge: the tree, the case id. The next run pushes takt's riders before the merge.
fn granted(name: &str, forge: &Rc<Scanning>) -> (Scratch, String) {
    let tree = scratch(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &tree);
    fs::write(tree.join("check.txt"), "broken\n").expect("a file");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "the store"]);
    let validators = delivery(forge);
    let store = Aep::default();
    let mut out = Vec::new();
    let opened = Run::open_next(
        &tree,
        delivery_options(),
        &store,
        &Requester,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("the story is taken");
    let id = opened.case.id.clone();
    drop(opened);
    fs::write(tree.join("check.txt"), "fixed\n").expect("a file");
    git(&tree, &["commit", "-q", "-am", "the work"]);
    let mut run = Run::resume(&tree, delivery_options(), &id, &Requester, &validators)
        .expect("the case resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    drop(run);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    approve(&tree, &store, &id, "repository.merge", "human:operator")
        .expect("the approval is given");
    (tree, id)
}

/// Runs the granted case once more, past the gate: how it ended and what it printed.
fn merge_run(tree: &Path, id: &str, forge: &Rc<Scanning>) -> (Result<Stop, String>, String) {
    let validators = delivery(forge);
    let store = Aep::default();
    let mut run = Run::resume(tree, delivery_options(), id, &NoHarness, &validators)
        .expect("the case resumes");
    run.record_on(&store);
    run.poll = std::time::Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// `repository.merge` pushes takt's riders above the reviewed head before it merges: the forge
/// scans that push before it, and the merge follows.
#[test]
fn the_rider_push_before_the_merge_is_scanned_first() {
    let forge = Rc::new(Scanning {
        memory: Memory {
            checks: Checks::Pass,
            ..Memory::default()
        },
        ..Scanning::default()
    });
    let (tree, id) = granted("riders", &forge);
    let before = forge.log.borrow().len();
    let (stop, text) = merge_run(&tree, &id, &forge);
    assert_eq!(stop, Ok(Stop::Completed("accepted".to_owned())), "{text}");
    let log = forge.log.borrow()[before..].to_vec();
    let push = log
        .iter()
        .position(|line| line.starts_with("push "))
        .unwrap_or_else(|| panic!("the riders were pushed: {log:#?}"));
    let pushed = log[push].rsplit(' ').next().expect("a revision").to_owned();
    assert!(
        log[..push].contains(&format!("scan takt/{id} {pushed}")),
        "{log:#?}"
    );
    assert!(log.contains(&"merge #1".to_owned()), "{log:#?}");
}

/// A scan that refuses the riders' push: nothing is pushed, nothing merged, no record written.
#[test]
fn a_refused_scan_of_the_rider_push_pushes_and_merges_nothing() {
    let forge = Rc::new(Scanning {
        memory: Memory {
            checks: Checks::Pass,
            ..Memory::default()
        },
        ..Scanning::default()
    });
    let (tree, id) = granted("riders-refused", &forge);
    let before = forge.log.borrow().len();
    forge.refuse.set(true);
    let (stop, text) = merge_run(&tree, &id, &forge);
    let error = stop.expect_err("the scan refused");
    assert!(error.contains("private rule"), "{error}\n{text}");
    let log = forge.log.borrow()[before..].to_vec();
    assert!(
        !log.iter().any(|line| line.starts_with("push ")),
        "{log:#?}"
    );
    assert!(forge.memory.merged.borrow().is_empty(), "{log:#?}");
    let dir = tree.join(".engineering/cases").join(&id);
    assert!(
        !kinds(&dir).contains(&"merge_observation".to_owned()),
        "{:?}",
        kinds(&dir)
    );
}

/// A harness whose step writes a file and moves a tracking ref and makes another: what the scan
/// before a push reads as what the remote holds.
struct Retracking;

impl Harness for Retracking {
    fn name(&self) -> &str {
        "retracking"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::write(context.tree.join("work.txt"), "work\n")?;
        let head = git(context.tree, &["rev-parse", "HEAD"]);
        git(
            context.tree,
            &["update-ref", "refs/remotes/origin/main", &head],
        );
        git(
            context.tree,
            &["update-ref", "refs/remotes/origin/planted", &head],
        );
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "acted".to_owned(),
            tokens: None,
        })
    }
}

/// A step may not move or make a tracking ref: the checkpoint guards `refs/remotes/` as it
/// guards branches and tags, so the step is refused, the ref it moved is put back and the one it
/// made removed.
#[test]
fn a_step_that_moves_a_tracking_ref_is_refused_and_the_refs_put_back() {
    let tree = scratch("retracking");
    let start = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["update-ref", "refs/remotes/origin/main", &start]);
    let options = RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "make the change".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some("retracking".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let mut run = Run::open(&tree, options, &Retracking, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out);
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("refs/remotes/origin/")),
        "{stop:?}\n{text}"
    );
    assert_eq!(
        git(
            &tree,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/remotes/"
            ]
        ),
        format!("refs/remotes/origin/main {start}"),
        "{text}"
    );
}
