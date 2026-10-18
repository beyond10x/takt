//! A run never destroys or commits work it did not make. On a scratch repository: a case opens
//! only on a clean tree; a judgement step that edits a path already uncommitted, stages, writes
//! an ignored file, writes under `.git/`, moves `HEAD`, or turns a case file into a directory is
//! refused and leaves no trace; a harness error leaves none either; an accepted step's commit
//! holds exactly the paths the step changed; porcelain paths are read exactly; a refused or
//! failed open leaves no case, no branch and the tree where it was; a take is committed on the
//! case branch, survives a refusal and is never taken twice; the store is read from the base.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{Approval, Branch, CASES_DIR};
use takt::frontier::{Aep, Frontier, Story, Take, Unit};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{CaseDir, Run, RunOptions, Stop};

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

/// A scratch directory under this crate's build directory holding the repository `repo/` and,
/// beside it, whatever a case leaves outside the tree (a hook's marker). Dropping it removes
/// both.
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
        if let Err(error) = fs::remove_dir_all(&self.root) {
            let message = format!("remove the scratch tree {}: {error}", self.root.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

fn empty(name: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("hygiene-{}-{name}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("a stale scratch tree is removed");
    }
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("scratch dir");
    Scratch { root, repo }
}

/// A committed repository on `main`: a source file, a test, notes, a script that leaves
/// `stamp.txt` behind as a validator's run may, and `*.local` ignored.
fn repository(name: &str) -> Scratch {
    let scratch = empty(name);
    let dir = &scratch.repo;
    fs::create_dir_all(dir.join("src")).expect("src");
    fs::create_dir_all(dir.join("tests")).expect("tests");
    fs::write(dir.join("src/lib.rs"), "a\n").expect("a file");
    fs::write(dir.join("tests/a.rs"), "a\n").expect("a file");
    fs::write(dir.join("notes.md"), "committed\n").expect("a file");
    fs::write(dir.join(".gitignore"), "*.local\n").expect("a file");
    fs::write(
        dir.join("stamp.sh"),
        "#!/bin/sh\necho validator > stamp.txt\necho validator > notes.md\n",
    )
    .expect("a file");
    fs::set_permissions(dir.join("stamp.sh"), fs::Permissions::from_mode(0o755)).expect("exec bit");
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "start"]);
    scratch
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

/// A committed copy of the fixture store `fixture`.
fn store(fixture: &str, name: &str) -> Scratch {
    let scratch = empty(name);
    copy_dir(&Path::new(FIXTURES).join(fixture), &scratch.repo);
    fs::write(scratch.repo.join("check.txt"), "broken\n").expect("a file");
    git(&scratch.repo, &["init", "-q", "-b", "main"]);
    git(&scratch.repo, &["add", "-A"]);
    git(&scratch.repo, &["commit", "-q", "-m", "start"]);
    scratch
}

fn options(id: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "make the change".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

fn store_options() -> RunOptions {
    let mut options = options("unused", 2);
    options.intent = String::new();
    options.case_id = None;
    options
}

/// A harness whose every step does `act` in the tree and reports `action`.
struct Acting<F: Fn(&Path)> {
    action: &'static str,
    act: F,
}

impl<F: Fn(&Path)> Harness for Acting<F> {
    fn name(&self) -> &str {
        "acting"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        (self.act)(context.tree);
        Ok(StepReport {
            action: self.action.to_owned(),
            summary: "acted".to_owned(),
            tokens: None,
        })
    }
}

fn acting<F: Fn(&Path)>(action: &'static str, act: F) -> Acting<F> {
    Acting { action, act }
}

/// The validator every case here runs first: `command` as the test command.
fn tests_run(command: &str) -> Vec<Box<dyn Validator>> {
    vec![Box::new(TestsRun::new(command).expect("a command"))]
}

/// Opens the case `id` on `tree` and runs it with `harness` for `iterations` steps.
fn run(
    tree: &Path,
    id: &str,
    iterations: u64,
    harness: &dyn Harness,
    validators: &[Box<dyn Validator>],
) -> (anyhow::Result<Stop>, String) {
    let mut run = Run::open(tree, options(id, iterations), harness, validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The uncommitted paths outside the cases directory, as `git status` names them.
fn dirty(tree: &Path) -> Vec<String> {
    git(tree, &["status", "--porcelain", "--untracked-files=all"])
        .lines()
        .map(|line| line[3..].to_owned())
        .filter(|path| !path.starts_with(CASES_DIR) && !path.starts_with("\".engineering/cases"))
        .collect()
}

fn branches(tree: &Path) -> String {
    git(
        tree,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads/"],
    )
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn a_case_does_not_open_on_a_tree_with_uncommitted_work() {
    type Dirt = fn(&Path);
    let kinds: [(&str, Dirt, &str); 3] = [
        (
            "modified",
            |tree| fs::write(tree.join("notes.md"), "operator wip\n").expect("a write"),
            "notes.md",
        ),
        (
            "untracked",
            |tree| fs::write(tree.join("draft.md"), "operator draft\n").expect("a write"),
            "draft.md",
        ),
        (
            "staged",
            |tree| {
                fs::write(tree.join("staged.md"), "staged\n").expect("a write");
                git(tree, &["add", "staged.md"]);
            },
            "staged.md",
        ),
    ];
    for (kind, make, path) in kinds {
        let tree = repository(&format!("dirty-open-{kind}"));
        make(&tree);
        let before = dirty(&tree);
        let head = git(&tree, &["rev-parse", "HEAD"]);
        let validators = tests_run("true");
        let harness = acting("repository.inspect", |_| {});
        let error = match Run::open(&tree, options("dirty", 1), &harness, &validators) {
            Ok(_) => panic!("a case opened on a tree with a {kind} file"),
            Err(error) => format!("{error:#}"),
        };
        assert!(error.contains(path), "{kind}: {error}");
        assert_eq!(git(&tree, &["branch", "--show-current"]), "main", "{kind}");
        assert_eq!(branches(&tree), "main", "{kind}");
        assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head, "{kind}");
        assert!(!tree.join(CASES_DIR).exists(), "{kind}");
        assert_eq!(
            dirty(&tree),
            before,
            "{kind}: the operator's work is untouched"
        );
    }

    // An ignored file is no uncommitted work: the case opens beside it and leaves it be.
    let tree = repository("dirty-open-ignored");
    fs::write(tree.join("keep.local"), "operator's ignored file\n").expect("a write");
    let validators = tests_run("true");
    let harness = acting("repository.inspect", |_| {});
    Run::open(&tree, options("ignored", 1), &harness, &validators).expect("opens");
    assert_eq!(read(&tree.join("keep.local")), "operator's ignored file\n");
}

#[test]
fn a_step_that_edits_a_path_already_uncommitted_is_refused_and_undone() {
    // `stamp.sh`, the test command, leaves `stamp.txt` untracked and `notes.md` modified: what a
    // validator's run may leave. The step edits each in turn.
    for path in ["stamp.txt", "notes.md"] {
        let tree = repository(&format!("predirty-{}", path.replace('.', "-")));
        let validators = tests_run("./stamp.sh");
        let harness = acting("implementation.edit", |tree: &Path| {
            fs::write(tree.join(path), "model\n").expect("a write");
            fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
        });
        let id = "predirty";
        let (stop, text) = run(&tree, id, 4, &harness, &validators);
        let stop = stop.expect("the run stops");
        assert!(
            matches!(&stop, Stop::StepRefused(why) if why.contains(path)),
            "{path}: {stop:?}\n{text}"
        );
        assert_eq!(read(&tree.join(path)), "validator\n", "{path}\n{text}");
        assert_eq!(read(&tree.join("src/lib.rs")), "a\n", "{path}\n{text}");
        assert_eq!(
            git(&tree, &["rev-list", "--count", "main..HEAD"]),
            "0",
            "{path}: nothing was committed\n{text}"
        );
    }
}

#[test]
fn a_step_that_stages_its_change_is_refused_and_the_index_put_back() {
    let tree = repository("staged");
    let validators = tests_run("true");
    let harness = acting("implementation.edit", |tree: &Path| {
        fs::write(tree.join("src/new.rs"), "new\n").expect("a write");
        fs::write(tree.join("src/lib.rs"), "staged edit\n").expect("a write");
        git(tree, &["add", "src/new.rs", "src/lib.rs"]);
    });
    let (stop, text) = run(&tree, "staged", 4, &harness, &validators);
    let stop = stop.expect("the run stops");
    assert!(matches!(&stop, Stop::StepRefused(_)), "{stop:?}\n{text}");
    assert!(!tree.join("src/new.rs").exists(), "{text}");
    assert_eq!(read(&tree.join("src/lib.rs")), "a\n", "{text}");
    assert_eq!(
        git(&tree, &["diff", "--cached", "--name-only"]),
        "",
        "{text}"
    );
    assert_eq!(dirty(&tree), Vec::<String>::new(), "{text}");
    assert_eq!(git(&tree, &["rev-list", "--count", "main..HEAD"]), "0");
}

#[test]
fn a_step_that_writes_an_ignored_file_is_refused_and_the_file_removed() {
    let tree = repository("ignored");
    fs::write(tree.join("keep.local"), "operator's\n").expect("a write");
    let validators = tests_run("true");
    let harness = acting("implementation.edit", |tree: &Path| {
        fs::write(tree.join("override.local"), "PASS=1\n").expect("a write");
        fs::create_dir_all(tree.join("cfg")).expect("a dir");
        fs::write(tree.join("cfg/more.local"), "PASS=1\n").expect("a write");
        fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
    });
    let (stop, text) = run(&tree, "ignored", 4, &harness, &validators);
    let stop = stop.expect("the run stops");
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("override.local")),
        "{stop:?}\n{text}"
    );
    assert!(!tree.join("override.local").exists(), "{text}");
    assert!(!tree.join("cfg/more.local").exists(), "{text}");
    assert_eq!(read(&tree.join("keep.local")), "operator's\n", "{text}");
    assert_eq!(read(&tree.join("src/lib.rs")), "a\n", "{text}");
    assert_eq!(dirty(&tree), Vec::<String>::new(), "{text}");
}

#[test]
fn a_step_that_writes_under_git_is_refused_and_nothing_it_planted_runs() {
    let scratch = repository("dotgit");
    let tree: &Path = &scratch;
    let marker = scratch.root.join("planted-ran");
    let config = read(&tree.join(".git/config"));
    let exclude = read(&tree.join(".git/info/exclude"));
    let script = format!("#!/bin/sh\necho ran >> {}\nexit 1\n", marker.display());
    let harness = acting("implementation.edit", |tree: &Path| {
        for hook in ["pre-commit", "post-commit", "post-checkout"] {
            let path = tree.join(".git/hooks").join(hook);
            fs::write(&path, &script).expect("a hook");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("exec bit");
        }
        let monitor = tree.join(".git/monitor.sh");
        fs::write(&monitor, &script).expect("a monitor");
        fs::set_permissions(&monitor, fs::Permissions::from_mode(0o755)).expect("exec bit");
        let mut text = read(&tree.join(".git/config"));
        text.push_str(&format!("[core]\n\tfsmonitor = {}\n", monitor.display()));
        fs::write(tree.join(".git/config"), text).expect("config");
        let mut text = read(&tree.join(".git/info/exclude"));
        text.push_str("hidden.rs\n");
        fs::write(tree.join(".git/info/exclude"), text).expect("exclude");
        fs::write(tree.join("src/hidden.rs"), "hidden\n").expect("a write");
    });
    let validators = tests_run("true");
    let (stop, text) = run(tree, "dotgit", 4, &harness, &validators);
    let stop = stop.expect("the run stops");
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains(".git")),
        "{stop:?}\n{text}"
    );
    assert_eq!(read(&tree.join(".git/config")), config, "{text}");
    assert_eq!(read(&tree.join(".git/info/exclude")), exclude, "{text}");
    for hook in ["pre-commit", "post-commit", "post-checkout"] {
        assert!(
            !tree.join(".git/hooks").join(hook).exists(),
            "{hook}\n{text}"
        );
    }
    // `.git/monitor.sh` itself is no file git runs or reads as configuration, so it is left
    // where it is; the config key that would run it is put back, and the marker below shows it
    // never ran.
    assert!(!tree.join("src/hidden.rs").exists(), "{text}");

    // The next step is accepted and committed: nothing the refused step planted runs.
    let harness = acting("implementation.edit", |tree: &Path| {
        fs::write(tree.join("src/lib.rs"), "fixed\n").expect("a write");
    });
    let mut resumed =
        Run::resume(tree, options("dotgit", 1), "dotgit", &harness, &validators).expect("resumes");
    let mut out = Vec::new();
    let stop = resumed.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out);
    assert_eq!(stop, Stop::Iterations(1), "{text}");
    assert_eq!(
        git(tree, &["rev-list", "--count", "main..HEAD"]),
        "1",
        "{text}"
    );
    assert!(
        !marker.exists(),
        "a planted hook or monitor ran: {}",
        read(&marker)
    );
}

#[test]
fn a_step_that_commits_by_itself_is_refused_and_head_put_back() {
    let tree = repository("self-commit");
    let validators = tests_run("true");
    let harness = acting("implementation.edit", |tree: &Path| {
        fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
        git(tree, &["commit", "-q", "-am", "the model's own commit"]);
    });
    let mut opened =
        Run::open(&tree, options("self-commit", 4), &harness, &validators).expect("opens");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let mut out = Vec::new();
    let stop = opened.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out);
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("HEAD")),
        "{stop:?}\n{text}"
    );
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head, "{text}");
    assert_eq!(read(&tree.join("src/lib.rs")), "a\n", "{text}");
    assert_eq!(dirty(&tree), Vec::<String>::new(), "{text}");
}

#[test]
fn a_harness_error_leaves_nothing_of_the_step_behind() {
    struct Crasher;
    impl Harness for Crasher {
        fn name(&self) -> &str {
            "crasher"
        }
        fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
            fs::write(context.tree.join("src/lib.rs"), "half done\n").expect("a write");
            fs::write(context.tree.join("half.rs"), "half\n").expect("a write");
            fs::write(context.tree.join("half.local"), "half\n").expect("a write");
            anyhow::bail!("the harness died")
        }
    }
    let tree = repository("crash");
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "crash", 4, &Crasher, &validators);
    let error = format!("{:#}", stop.expect_err("the harness error ends the run"));
    assert!(error.contains("the harness died"), "{error}\n{text}");
    assert_eq!(read(&tree.join("src/lib.rs")), "a\n", "{text}");
    assert!(!tree.join("half.rs").exists(), "{text}");
    assert!(!tree.join("half.local").exists(), "{text}");
    assert_eq!(dirty(&tree), Vec::<String>::new(), "{text}");
}

#[test]
fn a_case_file_the_step_turned_into_a_directory_is_put_back() {
    let tree = repository("dir-over-file");
    let validators = tests_run("true");
    let harness = acting("repository.inspect", |tree: &Path| {
        let case = tree.join(CASES_DIR).join("dir-over-file/case.yaml");
        fs::remove_file(&case).expect("the case file goes");
        fs::create_dir_all(case.join("x")).expect("a directory in its place");
        fs::write(case.join("x/y"), "z").expect("a file in it");
    });
    let (stop, text) = run(&tree, "dir-over-file", 4, &harness, &validators);
    let stop = stop.expect("the run stops with a refusal, not an error");
    assert!(matches!(&stop, Stop::StepRefused(_)), "{stop:?}\n{text}");
    assert!(
        tree.join(CASES_DIR)
            .join("dir-over-file/case.yaml")
            .is_file(),
        "{text}"
    );
    CaseDir::resume(&tree, "dir-over-file").expect("the case reads back");
}

#[test]
fn an_accepted_step_commits_exactly_the_paths_it_changed() {
    let tree = repository("exact");
    // `stamp.sh` leaves `stamp.txt` and `notes.md` uncommitted: not the step's, not committed.
    let validators = tests_run("./stamp.sh");
    let harness = acting("implementation.edit", |tree: &Path| {
        fs::write(tree.join("src/lib.rs"), "b\n").expect("a write");
        fs::write(tree.join("src/new file ü.rs"), "new\n").expect("a write");
        fs::write(tree.join(" lead.rs"), "leading space\n").expect("a write");
        fs::remove_file(tree.join("tests/a.rs")).expect("a removal");
    });
    let mut opened = Run::open(&tree, options("exact", 2), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = opened.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out);
    assert_eq!(stop, Stop::Iterations(2), "{text}");
    let mut committed: Vec<String> = git(&tree, &["show", "--name-only", "--format=", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    committed.sort();
    assert_eq!(
        committed,
        [" lead.rs", "src/lib.rs", "src/new file ü.rs", "tests/a.rs"],
        "{text}"
    );
    assert_eq!(git(&tree, &["rev-list", "--count", "main..HEAD"]), "1");
    assert_eq!(
        opened.case.revision("implementation").as_deref(),
        Some(git(&tree, &["rev-parse", "HEAD"]).as_str())
    );
    let mut left = dirty(&tree);
    left.sort();
    assert_eq!(left, ["notes.md", "stamp.txt"], "{text}");
    assert_eq!(read(&tree.join("stamp.txt")), "validator\n");
}

#[test]
fn porcelain_paths_are_read_exactly() {
    let tree = repository("porcelain");
    fs::write(tree.join("src/lib.rs"), "b\n").expect("a write");
    assert_eq!(
        takt::git::changed_paths(&tree).expect("status"),
        ["src/lib.rs"],
        "the first line's leading space is kept"
    );
    git(&tree, &["checkout", "--", "."]);
    fs::write(tree.join("tests/a.rs"), "b\n").expect("a write");
    let paths = takt::git::changed_paths(&tree).expect("status");
    assert_eq!(paths, ["tests/a.rs"]);
    assert!(takt::harness::is_test_path(&paths[0]));
    git(&tree, &["checkout", "--", "."]);

    fs::write(tree.join("tests/a b.rs"), "x\n").expect("a write");
    fs::write(tree.join("tests/ü.rs"), "x\n").expect("a write");
    fs::write(tree.join(" lead.rs"), "x\n").expect("a write");
    let mut paths = takt::git::changed_paths(&tree).expect("status");
    paths.sort();
    assert_eq!(paths, [" lead.rs", "tests/a b.rs", "tests/ü.rs"]);
    assert!(takt::harness::is_test_path("tests/a b.rs"));
    assert!(takt::harness::is_test_path("tests/ü.rs"));
    for path in [" lead.rs", "tests/a b.rs", "tests/ü.rs"] {
        fs::remove_file(tree.join(path)).expect("a removal");
    }

    git(&tree, &["mv", "src/lib.rs", "src/mo ved.rs"]);
    let mut paths = takt::git::changed_paths(&tree).expect("status");
    paths.sort();
    assert_eq!(
        paths,
        ["src/lib.rs", "src/mo ved.rs"],
        "a rename names both sides"
    );
}

#[test]
fn a_failed_open_leaves_no_case_no_branch_and_the_tree_where_it_was() {
    let tree = repository("failed-open");
    fs::create_dir_all(tree.join(".engineering")).expect("a dir");
    fs::write(tree.join(CASES_DIR), "not a directory\n").expect("a file");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "the cases path is a file"]);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let harness = acting("repository.inspect", |_| {});
    let opened = Run::open(&tree, options("failed", 1), &harness, &validators);
    assert!(opened.is_err(), "the case opened over a file");
    assert_eq!(git(&tree, &["branch", "--show-current"]), "main");
    assert_eq!(branches(&tree), "main");
    assert_eq!(dirty(&tree), Vec::<String>::new());

    // From a detached HEAD the tree is left detached at the same commit.
    let head = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "--detach"]);
    let opened = Run::open(&tree, options("failed", 1), &harness, &validators);
    assert!(opened.is_err(), "the case opened over a file");
    assert_eq!(git(&tree, &["branch", "--show-current"]), "");
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head);
    assert_eq!(branches(&tree), "main");
}

/// A frontier whose take writes into the tree and then fails.
struct FailingTake;

impl Frontier for FailingTake {
    fn name(&self) -> &str {
        "failing-take"
    }
    fn candidates(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(vec!["story:x".to_owned()])
    }
    fn active(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }
    fn show(&self, _: &Path, id: &str) -> anyhow::Result<Story> {
        Ok(Story {
            id: id.to_owned(),
            title: "t".to_owned(),
            body: "b".to_owned(),
        })
    }
    fn take(&self, repo: &Path, _: &str) -> anyhow::Result<Take> {
        fs::write(repo.join("notes.md"), "half a move\n")?;
        anyhow::bail!("the store crashed")
    }
    fn record_approval(&self, _: &Path, _: &str, _: &Approval) -> anyhow::Result<()> {
        Ok(())
    }
}

#[test]
fn a_take_that_fails_leaves_no_case_and_the_tree_where_it_was() {
    let tree = repository("take-fails");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let harness = acting("repository.inspect", |_| {});
    let mut out = Vec::new();
    let opened = Run::open_next(
        &tree,
        store_options(),
        &FailingTake,
        &harness,
        &validators,
        &mut out,
    );
    let error = match opened {
        Ok(_) => panic!("the take failed, yet a run opened"),
        Err(error) => format!("{error:#}"),
    };
    assert!(error.contains("the store crashed"), "{error}");
    assert!(!tree.join(CASES_DIR).exists());
    assert_eq!(git(&tree, &["branch", "--show-current"]), "main");
    assert_eq!(branches(&tree), "main");
    assert_eq!(read(&tree.join("notes.md")), "committed\n");
    assert_eq!(dirty(&tree), Vec::<String>::new());
}

/// The case directories under `tree`, sorted.
fn case_names(tree: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(tree.join(CASES_DIR))
        .map(|entries| {
            entries
                .map(|entry| {
                    entry
                        .expect("an entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn status(tree: &Path, id: &str) -> String {
    Aep::default()
        .list(tree)
        .expect("the store lists")
        .into_iter()
        .find(|artifact| artifact.id == id)
        .map(|artifact| artifact.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

#[test]
fn a_take_is_committed_on_the_case_branch_and_survives_a_refusal() {
    let tree = store("one-ready", "take-committed");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let rogue = acting("not.offered", |tree: &Path| {
        fs::write(tree.join("check.txt"), "rogue\n").expect("a write");
    });
    let mut out = Vec::new();
    let mut first = Run::open_next(
        &tree,
        store_options(),
        &Aep::default(),
        &rogue,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let case_id = first.case.id.clone();
    assert_eq!(
        dirty(&tree),
        Vec::<String>::new(),
        "the take is not left uncommitted"
    );
    assert_eq!(
        git(&tree, &["branch", "--show-current"]),
        format!("takt/{case_id}")
    );
    assert_eq!(
        git(&tree, &["show", "--name-only", "--format=", "HEAD"]),
        ".engineering/planning/story/ready-one.md"
    );
    assert_eq!(git(&tree, &["rev-list", "--count", "main..HEAD"]), "1");
    assert_eq!(
        first.case.revision("implementation").as_deref(),
        Some(git(&tree, &["rev-parse", "HEAD"]).as_str()),
        "the case is about the commit its validators observe"
    );
    let stop = first.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(matches!(&stop, Stop::StepRefused(_)), "{stop:?}\n{text}");
    drop(first);
    assert_eq!(status(&tree, "story:ready-one"), "active", "{text}");
    assert_eq!(read(&tree.join("check.txt")), "broken\n", "{text}");

    std::thread::sleep(std::time::Duration::from_millis(1100));
    let mut out = Vec::new();
    let second = Run::open_next(
        &tree,
        store_options(),
        &Aep::default(),
        &rogue,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("the open case is resumed");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(second.case.id, case_id, "{text}");
    assert!(
        text.starts_with("frontier: resumed story:ready-one"),
        "{text}"
    );
    assert_eq!(case_names(&tree), [case_id], "{text}");
}

#[test]
fn the_next_case_reads_the_store_from_the_base_branch() {
    let tree = store("one-ready", "store-on-base");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let harness = acting("repository.inspect", |_| {});
    let mut on_intent = options("on-intent", 1);
    on_intent.intent = "an earlier case".to_owned();
    let earlier = Run::open_unit(
        &tree,
        on_intent,
        Unit::Intent("an earlier case".to_owned()),
        &harness,
        &validators,
    )
    .expect("the earlier case opens");
    drop(earlier);
    // A commit on the earlier case's branch rewrites the ready story's title.
    let story = tree.join(".engineering/planning/story/ready-one.md");
    let text = read(&story).replace("title: Make the check pass", "title: Injected title");
    fs::write(&story, text).expect("a write");
    git(
        &tree,
        &["commit", "-q", "-am", "a model's commit on its case branch"],
    );
    assert_eq!(git(&tree, &["branch", "--show-current"]), "takt/on-intent");

    let mut out = Vec::new();
    let next = Run::open_next(
        &tree,
        store_options(),
        &Aep::default(),
        &harness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(
        text, "frontier: took story:ready-one: Make the check pass\n",
        "the story as the base has it"
    );
    let Unit::Story(story) = &next.unit else {
        panic!("the unit is a story, not {:?}", next.unit);
    };
    assert_eq!(story.title, "Make the check pass");
    assert!(read(&next.case.dir.join("intent.md")).starts_with("Make the check pass\n"));
    let branch = Branch::read(&next.case.dir).expect("branch.yaml");
    assert_eq!(branch.base.as_deref(), Some("main"));
    let main = git(&tree, &["rev-parse", "main"]);
    assert_eq!(git(&tree, &["rev-parse", "HEAD^"]), main, "{text}");
}

#[test]
fn a_take_commit_runs_no_hook_of_the_repository() {
    let tree = store("one-ready", "take-no-hook");
    let hook = tree.join(".git/hooks/commit-msg");
    fs::write(
        &hook,
        "#!/bin/sh\nif grep -q \"^takt: take\" \"$1\"; then echo rejected >&2; exit 1; fi\n",
    )
    .expect("a hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("exec bit");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let harness = acting("repository.inspect", |_| {});
    let mut out = Vec::new();
    let run = Run::open_next(
        &tree,
        store_options(),
        &Aep::default(),
        &harness,
        &validators,
        &mut out,
    )
    .expect("the take is committed past a commit-msg hook that rejects it")
    .expect("story:ready-one is taken");
    assert_eq!(
        git(&tree, &["log", "-1", "--format=%s"]),
        "takt: take story:ready-one"
    );
    drop(run);
}

#[test]
fn a_step_that_moves_the_base_branch_is_refused_and_the_base_put_back() {
    let tree = repository("base-moved");
    let main = git(&tree, &["rev-parse", "main"]);
    let validators = tests_run("true");
    let harness = acting("implementation.edit", |tree: &Path| {
        let commit = git(
            tree,
            &["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "moved"],
        );
        git(tree, &["update-ref", "refs/heads/main", &commit]);
        fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
    });
    let (stop, text) = run(&tree, "base-moved", 4, &harness, &validators);
    let stop = stop.expect("the run stops");
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("refs/heads/main")),
        "{stop:?}\n{text}"
    );
    assert_eq!(git(&tree, &["rev-parse", "main"]), main, "{text}");
    assert_eq!(read(&tree.join("src/lib.rs")), "a\n", "{text}");
}
