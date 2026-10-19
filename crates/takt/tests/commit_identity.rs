//! story:takt-commits-with-the-repositorys-identity: every commit takt makes (the take, a
//! judgement step's, the store's writes on the case's branch, the approval's record) carries the
//! identity the case was opened with as author and committer, never the operator's git
//! configuration (`user.*`, `author.*`, `committer.*`). The identity is the one
//! `.engineering/commit-identity.conf` names at the commit the case forks from, or the one the
//! caller gives, which wins; it is recorded in the case, so a resume and an approval commit with
//! it too. A case opened with none commits as takt's own `takt <takt@localhost>`, unless the
//! caller requires one (a run that pushes), and then it is refused before anything is written.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{CASES_DIR, CommitAs};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::frontier::{Aep, approve};
use takt::git::{COMMIT_IDENTITY_FILE, CommitIdentity, configured_identity};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The identity the repository configures in these cases.
const BOT: (&str, &str) = (
    "b10x-bot[bot]",
    "316511680+b10x-bot[bot]@users.noreply.github.com",
);

/// The operator's own identity, in every place git's configuration can give one.
const OPERATOR_CONFIG: [(&str, &str); 6] = [
    ("user.name", "Operator"),
    ("user.email", "operator@example.invalid"),
    ("author.name", "Operator Author"),
    ("author.email", "author@example.invalid"),
    ("committer.name", "Operator Committer"),
    ("committer.email", "committer@example.invalid"),
];

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

/// A scratch directory under this crate's build directory. Dropping it removes it; a removal
/// that fails fails the case, unless the case is already failing.
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

/// `[identity]` with `name` and `email`, as `.engineering/commit-identity.conf` holds it.
fn identity_file(name: &str, email: &str) -> String {
    format!("[identity]\n\tname = {name}\n\temail = {email}\n")
}

/// A scratch repository on `main` holding a committed copy of the `one-ready` store, with
/// `identity` (the file's text) committed as `.engineering/commit-identity.conf` when given, and
/// the operator's identity in its own configuration ([`OPERATOR_CONFIG`]).
fn repository(name: &str, identity: Option<&str>) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("commit-identity-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    if let Some(text) = identity {
        fs::write(dir.join(COMMIT_IDENTITY_FILE), text).expect("a file");
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    for (key, value) in OPERATOR_CONFIG {
        git(&dir, &["config", key, value]);
    }
    Scratch(dir)
}

/// Edits `check.txt` when an edit is offered and the tree has not been worked yet; requests the
/// merge gate when it is offered; hands the change over once worked.
struct Editor;

impl Harness for Editor {
    fn name(&self) -> &str {
        "editor"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        let report = |action: &str| StepReport {
            action: action.to_owned(),
            summary: "the double".to_owned(),
            tokens: None,
        };
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(report("repository.merge"));
        }
        let check = context.tree.join("check.txt");
        if fs::read_to_string(&check)? == "broken\n"
            && context.offered.contains(&"implementation.edit".to_owned())
        {
            fs::write(&check, "fixed\n")?;
            return Ok(report("implementation.edit"));
        }
        if context.choices.contains(&"change.handover".to_owned()) {
            return Ok(report("change.handover"));
        }
        Err(anyhow!(
            "nothing for the double among {}",
            context.offered.join(", ")
        ))
    }
}

fn options(iterations: u64) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
    }
}

/// The three forge validators over `forge`, in the order the binary registers them.
fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// [`validators`] after the test command `true`, whose pass record the store's `test_result`
/// is written from.
fn delivery_validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    let mut all: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a test command"))];
    all.extend(validators(forge));
    all
}

/// The forge double, its checks passing.
fn double() -> Rc<Memory> {
    Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })
}

fn record(n: usize, kind: &str, result: Option<&str>, revision: &str, producer: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        result.map(str::to_owned),
        "implementation",
        revision,
        producer,
    )
}

/// Each commit of `range`, newest first, as `<subject>|<author name> <author email>|<committer
/// name> <committer email>`.
fn identities(repo: &Path, range: &str) -> Vec<String> {
    git(
        repo,
        &["log", "--format=%s|%an <%ae>|%cn <%ce>", range, "--"],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// Asserts every commit of `range` names `who` as author and committer, and that there are
/// `count` of them; the commits for the message.
fn all_by(repo: &Path, range: &str, who: &str, count: usize) -> Vec<String> {
    let commits = identities(repo, range);
    assert_eq!(commits.len(), count, "{commits:#?}");
    for commit in &commits {
        let (_, sides) = commit.split_once('|').expect("a subject");
        assert_eq!(sides, format!("{who}|{who}"), "{commits:#?}");
    }
    commits
}

/// Acceptance 1 for a delivery: with `.engineering/commit-identity.conf` at the base naming the
/// bot, every commit of the case (the take, the judgement step's edit, the approval's record,
/// and the store's `test_result` and move to `implemented` before the merge) carries the bot as
/// author and committer, whatever the operator's `user.*`, `author.*` and `committer.*` say. Two
/// of the runs are resumes and one commit is `takt approve`'s: each commits with the identity
/// recorded when the case opened.
#[test]
fn every_commit_of_a_delivery_carries_the_configured_identity_over_the_operators_config() {
    let repo = repository("delivery", Some(&identity_file(BOT.0, BOT.1)));
    let start = git(&repo, &["rev-parse", "HEAD"]);
    let memory = double();
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = delivery_validators(forge);
    let store = Aep::default();

    let mut out = Vec::new();
    let taken = Run::take_next_as(
        &repo,
        options(1),
        &store,
        &CommitAs::Configured { required: true },
        &Editor,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = taken.case.id.clone();
    assert_eq!(
        taken.case.commit_identity().expect("reads"),
        Some(CommitIdentity::new(BOT.0, BOT.1).expect("an identity"))
    );
    drop(taken);

    // The tests, then one judgement step: the edit, committed by the run.
    let mut run = Run::resume(&repo, options(2), &id, &Editor, &validators).expect("resumes");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(stop, Stop::Iterations(2), "{text}");
    let worked = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(
        git(&repo, &["show", "-s", "--format=%s", &worked]),
        "implementation.edit: the double",
        "{text}"
    );
    drop(run);

    // Green and handed over at the edit, as the validators and the choice record them; the run
    // pushes, opens the pull request and stops at the gate.
    let mut run = Run::resume(&repo, options(8), &id, &Editor, &validators).expect("resumes");
    for (n, (kind, result, producer)) in [
        ("gate_result", Some("pass"), "commit-identity-test"),
        ("handover", None, takt::run::CHOICE_PRODUCER),
        ("change_record", Some("ahead"), "takt.change-read"),
    ]
    .into_iter()
    .enumerate()
    {
        run.case
            .add_evidence(&record(n + 1, kind, result, &worked, producer))
            .expect("the record is written");
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    drop(run);

    approve(&repo, &store, &id, "repository.merge", "human:operator").expect("approved");

    // The merge: the store's records are committed on the case's branch before it.
    let mut run = Run::resume(&repo, options(8), &id, &Editor, &validators).expect("resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    drop(run);

    let bot = format!("{} <{}>", BOT.0, BOT.1);
    let commits = all_by(&repo, &format!("{start}..HEAD"), &bot, 5);
    let subjects: Vec<&str> = commits
        .iter()
        .map(|commit| commit.split_once('|').expect("a subject").0)
        .collect();
    assert_eq!(
        subjects,
        [
            "takt: implemented story:ready-one",
            "takt: test_result story:ready-one",
            "takt: approve story:ready-one repository.merge",
            "implementation.edit: the double",
            "takt: take story:ready-one",
        ],
        "{text}"
    );
}

/// The identity the caller gives (`--commit-identity`) wins over the one the file names, is
/// recorded in the case, and a resume, given none, commits with the recorded one.
#[test]
fn the_identity_given_wins_over_the_file_and_a_resume_commits_with_the_recorded_one() {
    let repo = repository("given", Some(&identity_file(BOT.0, BOT.1)));
    let start = git(&repo, &["rev-parse", "HEAD"]);
    let given = CommitIdentity::parse("Release Bot <release@example.invalid>").expect("parses");
    let memory = double();
    let forge: Rc<dyn Forge> = memory;
    let validators = validators(forge);

    let mut out = Vec::new();
    let taken = Run::take_next_as(
        &repo,
        options(1),
        &Aep::default(),
        &CommitAs::Given(given.clone()),
        &Editor,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = taken.case.id.clone();
    drop(taken);
    assert_eq!(
        takt::case::commit_identity_of(&repo.join(CASES_DIR).join(&id)).expect("reads"),
        Some(given)
    );

    let mut run = Run::resume(&repo, options(1), &id, &Editor, &validators).expect("resumes");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(stop, Stop::Iterations(1), "{text}");
    drop(run);

    all_by(
        &repo,
        &format!("{start}..HEAD"),
        "Release Bot <release@example.invalid>",
        2,
    );
}

/// Acceptance 2, where commits leave the machine: a case that must have an identity (the CLI
/// asks so for a run that pushes) and finds none configured is refused, naming the file, its
/// keys and the flag, before anything is written: no case directory, no case branch, no commit,
/// the story not taken, the tree where it was.
#[test]
fn a_case_that_must_have_an_identity_and_finds_none_is_refused_with_nothing_written() {
    let repo = repository("required", None);
    let start = git(&repo, &["rev-parse", "HEAD"]);
    let memory = double();
    let forge: Rc<dyn Forge> = memory;
    let validators = validators(forge);

    let mut out = Vec::new();
    let refused = Run::take_next_as(
        &repo,
        options(1),
        &Aep::default(),
        &CommitAs::Configured { required: true },
        &Editor,
        &validators,
        &mut out,
    );
    let error = format!("{:#}", refused.err().expect("refused"));
    for named in [
        COMMIT_IDENTITY_FILE,
        "identity.name",
        "identity.email",
        "--commit-identity",
    ] {
        assert!(error.contains(named), "{named} in: {error}");
    }
    assert!(!repo.join(CASES_DIR).exists(), "{error}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), start);
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    assert_eq!(
        git(
            &repo,
            &["for-each-ref", "--format=%(refname)", "refs/heads/takt/"]
        ),
        ""
    );
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
}

/// With none configured and none required (a run that pushes nothing), and in a library run
/// given no identity, commits are takt's own on both sides: never the operator's `author.*` or
/// `committer.*`, which outrank `user.*` and so outranked takt's `-c user.*` before.
#[test]
fn with_none_configured_commits_are_takts_own_whatever_the_operators_config_says() {
    for (name, commit_as) in [
        ("unconfigured", CommitAs::Configured { required: false }),
        ("library", CommitAs::Takt),
    ] {
        let repo = repository(name, None);
        let start = git(&repo, &["rev-parse", "HEAD"]);
        let memory = double();
        let forge: Rc<dyn Forge> = memory;
        let validators = validators(forge);
        let mut out = Vec::new();
        let taken = Run::take_next_as(
            &repo,
            options(1),
            &Aep::default(),
            &commit_as,
            &Editor,
            &validators,
            &mut out,
        )
        .expect("the frontier answers")
        .expect("story:ready-one is taken");
        assert_eq!(taken.case.commit_identity().expect("reads"), None, "{name}");
        drop(taken);
        all_by(&repo, &format!("{start}..HEAD"), "takt <takt@localhost>", 1);
    }
}

/// The same through the library's plain entry point, on an intent with one edit: a run given
/// no identity commits as `takt <takt@localhost>` on both sides.
#[test]
fn a_library_run_given_no_identity_commits_as_takt_on_both_sides() {
    let repo = repository("plain", Some(&identity_file(BOT.0, BOT.1)));
    let start = git(&repo, &["rev-parse", "HEAD"]);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "fix the check".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("plain".to_owned()),
        gate: None,
    };
    let mut run = Run::open(&repo, options, &Editor, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(stop, Stop::Iterations(1), "{text}");
    drop(run);
    all_by(&repo, &format!("{start}..HEAD"), "takt <takt@localhost>", 1);
}

/// The file is read at a revision, without a checkout: the identity it names there, none where
/// it is absent, and a refusal naming what is wrong with one that gives a key twice, misses
/// one, gives one takt does not read, or names no identity git would record.
#[test]
fn the_configured_identity_is_read_at_a_revision_and_a_wrong_file_is_refused() {
    let repo = repository("read", None);
    let before = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(repo.join(COMMIT_IDENTITY_FILE), identity_file(BOT.0, BOT.1)).expect("a file");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "configure"]);
    assert_eq!(configured_identity(&repo, &before).expect("reads"), None);
    let configured = configured_identity(&repo, "HEAD")
        .expect("reads")
        .expect("configured");
    assert_eq!(configured.name, BOT.0);
    assert_eq!(configured.email, BOT.1);
    assert_eq!(configured.to_string(), format!("{} <{}>", BOT.0, BOT.1));

    for (text, named) in [
        ("[identity]\n\tname = a\n", "identity.email"),
        ("[identity]\n\temail = a@example.invalid\n", "identity.name"),
        (
            "[identity]\n\tname = a\n\tname = b\n\temail = a@example.invalid\n",
            "twice",
        ),
        (
            "[identity]\n\tname = a\n\temail = a@example.invalid\n\tsigningkey = x\n",
            "identity.signingkey",
        ),
        (
            "[identity]\n\tname = a <b>\n\temail = a@example.invalid\n",
            "identity.name",
        ),
        (
            "[identity]\n\tname = a\n\temail = a example\n",
            "identity.email",
        ),
    ] {
        fs::write(repo.join(COMMIT_IDENTITY_FILE), text).expect("a file");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "reconfigure"]);
        let error = format!("{:#}", configured_identity(&repo, "HEAD").expect_err(text));
        assert!(error.contains(COMMIT_IDENTITY_FILE), "{error}");
        assert!(error.contains(named), "{named} in: {error}");
    }

    assert_eq!(
        CommitIdentity::parse(&format!("{} <{}>", BOT.0, BOT.1)).expect("parses"),
        CommitIdentity::new(BOT.0, BOT.1).expect("an identity")
    );
    for wrong in [
        "",
        "name only",
        "<a@example.invalid>",
        "a <b@example.invalid",
        "a <b c>",
    ] {
        assert!(CommitIdentity::parse(wrong).is_err(), "{wrong:?}");
    }
}
