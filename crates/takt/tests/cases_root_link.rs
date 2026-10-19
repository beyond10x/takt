//! A step or a test command that replaces the cases directory with a symbolic link is undone
//! like any other foreign write: the link is removed, the directory put back from the snapshot
//! taken before it, `HEAD` and every ref rolled back, nothing written through the link, and the
//! run stops naming the link. A case whose cases directory, or whose own directory, is a link is
//! never resumed: the run refuses before its first step.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// Where the link made in the cases directory's place points: `../x` as written beside the
/// copy, which from `.engineering/` names `x` in the tree, where nothing is; and `../../x`, the
/// copy itself, beside the tree.
const LINKS: [&str; 2] = ["../x", "../../x"];

/// What the step and the test command run: the cases directory copied beside the tree, removed,
/// and a link to `link` made in its place; then a commit and a branch, so `HEAD` and the refs
/// move with it.
fn swap(link: &str) -> String {
    format!(
        "cp -r .engineering/cases ../x && rm -rf .engineering/cases && ln -s {link} .engineering/cases && git -c user.name=t -c user.email=t@localhost commit -q --allow-empty -m swapped && git branch planted"
    )
}

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

/// `HEAD` and every ref, as git lists them.
fn refs(tree: &Path) -> String {
    format!(
        "{}\n{}",
        git(tree, &["rev-parse", "HEAD"]),
        git(tree, &["for-each-ref", "--format=%(refname) %(objectname)"])
    )
}

/// Runs `script` with `sh` in `tree`.
fn sh(tree: &Path, script: &str) {
    let status = Command::new("sh")
        .args(["-c", script])
        .current_dir(tree)
        .status()
        .expect("sh runs");
    assert!(status.success(), "{script}");
}

/// A scratch directory under this crate's build directory holding the tree at `tree/`, so what
/// the step writes at `../x` stays inside it; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn copy(&self) -> PathBuf {
        self.0.join("x")
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

/// A committed repository on `main` with a README and `files`, executable when their name ends
/// in `.sh`, at `tree/` in a scratch directory named for this process and `name`.
fn repository(name: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("cases-swap-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("the tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        let path = tree.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
        fs::write(&path, text).expect("a file");
        if path.extension().is_some_and(|e| e == "sh") {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("exec bit");
        }
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    scratch
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
        gate: None,
    }
}

/// Every file and link under `dir`, by path relative to it, a link as `-> <target>`; a link is
/// never followed. Empty when `dir` is not a directory.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .to_path_buf();
            let meta = fs::symlink_metadata(&path).expect("metadata");
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&path).expect("a link");
                into.insert(relative, format!("-> {}", target.display()).into_bytes());
            } else if meta.is_dir() {
                walk(root, &path, into);
            } else {
                into.insert(relative, fs::read(&path).expect("a file"));
            }
        }
    }
    let mut into = BTreeMap::new();
    if fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        walk(dir, dir, &mut into);
    }
    into
}

/// The cases directory after the run is the one the step began on: every file of it there with
/// its bytes, the event log only grown by the lines the run wrote after putting it back, and
/// nothing else added; the run's own hold, `run.lock`, is let go when the run ends.
fn assert_put_back(
    before: &BTreeMap<PathBuf, Vec<u8>>,
    after: &BTreeMap<PathBuf, Vec<u8>>,
    text: &str,
) {
    assert!(
        !before.is_empty(),
        "the step saw the cases directory\n{text}"
    );
    for (path, bytes) in before {
        if path.ends_with("run.lock") {
            continue;
        }
        let now = after
            .get(path)
            .unwrap_or_else(|| panic!("{} is put back\n{text}", path.display()));
        if path.ends_with("events.jsonl") {
            assert!(
                now.starts_with(bytes),
                "{} keeps what it held\n{text}",
                path.display()
            );
        } else {
            assert_eq!(now, bytes, "{} as it was\n{text}", path.display());
        }
    }
    for path in after.keys() {
        assert!(
            before.contains_key(path),
            "{} was not there before the step\n{text}",
            path.display()
        );
    }
}

/// What every refused swap leaves: a stop naming the cases directory as a link, the directory a
/// directory again as it was in `before`, `HEAD` and the refs as `refs_before`, the copy beside
/// the tree as the swap made it, and nothing at the link's dangling target in the tree.
fn assert_undone(
    scratch: &Scratch,
    stop: &anyhow::Result<Stop>,
    before: &BTreeMap<PathBuf, Vec<u8>>,
    refs_before: &str,
    text: &str,
) {
    let tree = scratch.tree();
    let Ok(Stop::StepRefused(why)) = stop else {
        panic!("the step is refused, not {stop:?}\n{text}");
    };
    assert!(
        why.contains(".engineering/cases") && why.contains("a link"),
        "the refusal names the link: {why}\n{text}"
    );
    let cases = tree.join(".engineering/cases");
    let meta = fs::symlink_metadata(&cases).expect("the cases directory");
    assert!(meta.is_dir(), "a directory, not a link\n{text}");
    assert_put_back(before, &files(&cases), text);
    assert_eq!(refs(&tree), refs_before, "HEAD and refs\n{text}");
    assert_eq!(
        files(&scratch.copy()),
        *before,
        "nothing was written through the link\n{text}"
    );
    assert!(
        fs::symlink_metadata(tree.join("x")).is_err(),
        "nothing at the dangling target\n{text}"
    );
}

/// A harness whose first step records the tree as it finds it, then runs its script ([`swap`]
/// to `link` for [`Swaps::new`]); any later step ends the run with an error.
struct Swaps {
    script: String,
    calls: Cell<usize>,
    cases: RefCell<BTreeMap<PathBuf, Vec<u8>>>,
    refs: RefCell<String>,
}

impl Swaps {
    fn new(link: &str) -> Self {
        Self::running(swap(link))
    }

    fn running(script: String) -> Self {
        Self {
            script,
            calls: Cell::new(0),
            cases: RefCell::default(),
            refs: RefCell::default(),
        }
    }
}

impl Harness for Swaps {
    fn name(&self) -> &str {
        "swaps"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        if self.calls.get() > 1 {
            return Err(anyhow!("no further step"));
        }
        *self.cases.borrow_mut() = files(&context.tree.join(".engineering/cases"));
        *self.refs.borrow_mut() = refs(context.tree);
        sh(context.tree, &self.script);
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: None,
        })
    }
}

/// A harness step that swaps the cases directory for a link, dangling or to a copy of it, is
/// refused naming the link, and undone: the directory is a directory again, as the step found
/// it, `HEAD` and the refs as before the step, and nothing the run wrote afterwards reached the
/// copy.
#[test]
fn a_step_that_replaces_the_cases_directory_with_a_link_is_undone() {
    for (n, link) in LINKS.into_iter().enumerate() {
        let id = format!("step-{n}");
        let scratch = repository(&id, &[]);
        let tree = scratch.tree();
        let validators: Vec<Box<dyn Validator>> =
            vec![Box::new(TestsRun::new("true").expect("a command"))];
        let harness = Swaps::new(link);
        let mut run =
            Run::open(&tree, options(&id, 4), &harness, &validators).expect("the case opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let text = format!("{link}\n{}", String::from_utf8(out).expect("utf-8"));
        assert_eq!(harness.calls.get(), 1, "{stop:?}\n{text}");
        assert_undone(
            &scratch,
            &stop,
            &harness.cases.borrow(),
            &harness.refs.borrow(),
            &text,
        );
    }
}

/// A harness that writes `tests/case.sh` on its first step, so the test command the validator
/// runs next is the model's: it copies the cases directory to `../seen` and the refs to
/// `../refs` as it finds them, then runs [`swap`] to `link`. Any later step ends the run with an
/// error.
struct WritesTest {
    link: &'static str,
    calls: Cell<usize>,
}

impl Harness for WritesTest {
    fn name(&self) -> &str {
        "writes-test"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        if self.calls.get() > 1 {
            return Err(anyhow!("no further step"));
        }
        let script = context.tree.join("tests/case.sh");
        fs::create_dir_all(script.parent().expect("a parent"))?;
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nset -e\ncp -r .engineering/cases ../seen\ngit rev-parse HEAD > ../refs\ngit for-each-ref --format='%(refname) %(objectname)' >> ../refs\n{}\n",
                swap(self.link)
            ),
        )?;
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: None,
        })
    }
}

/// A test command that swaps the cases directory for a link, dangling or to a copy of it, is
/// refused naming the link, and undone: the directory is a directory again, as the command found
/// it, `HEAD` and the refs as before it, and nothing was recorded or written through the link.
#[test]
fn a_test_command_that_replaces_the_cases_directory_with_a_link_is_undone() {
    for (n, link) in LINKS.into_iter().enumerate() {
        let id = format!("validator-{n}");
        let scratch = repository(
            &id,
            &[(
                "tests/run.sh",
                "#!/bin/sh\nexec sh \"$(dirname \"$0\")/case.sh\" \"$@\"\n",
            )],
        );
        let tree = scratch.tree();
        let validators: Vec<Box<dyn Validator>> = vec![Box::new(
            TestsRun::new("sh tests/run.sh").expect("a command"),
        )];
        let harness = WritesTest {
            link,
            calls: Cell::new(0),
        };
        let mut run =
            Run::open(&tree, options(&id, 8), &harness, &validators).expect("the case opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let text = format!("{link}\n{}", String::from_utf8(out).expect("utf-8"));
        let seen = files(&scratch.0.join("seen"));
        let refs_before = fs::read_to_string(scratch.0.join("refs"))
            .unwrap_or_else(|_| panic!("the test command ran\n{text}"));
        assert_undone(&scratch, &stop, &seen, refs_before.trim_end(), &text);
    }
}

/// A case opened and left, its run over, the tree back on `main`.
fn opened(scratch: &Scratch, id: &str) {
    let tree = scratch.tree();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Swaps::new("../x");
    drop(Run::open(&tree, options(id, 1), &harness, &validators).expect("the case opens"));
    git(&tree, &["checkout", "-q", "main"]);
}

/// `Run::resume` of the case `id` is refused naming `named` as a link, before anything: no
/// step, the tree still on `main`, and the copy beside it as it was.
fn assert_not_resumed(scratch: &Scratch, id: &str, named: &str) {
    let tree = scratch.tree();
    let copied = files(&scratch.copy());
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Swaps::new("../x");
    let resumed = Run::resume(&tree, options(id, 1), id, &harness, &validators);
    let Err(error) = resumed else {
        panic!("{named}: the case is resumed through a link");
    };
    let error = format!("{error:#}");
    assert!(
        error.contains(named) && error.contains("a link"),
        "the refusal names {named} as a link: {error}"
    );
    assert_eq!(harness.calls.get(), 0, "no step was taken");
    assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(files(&scratch.copy()), copied, "nothing written through it");
}

/// A case whose cases directory is a link, dangling or to a copy of it beside the tree, is not
/// resumed.
#[test]
fn a_case_under_a_linked_cases_directory_is_not_resumed() {
    for (n, link) in LINKS.into_iter().enumerate() {
        let id = format!("root-{n}");
        let scratch = repository(&id, &[]);
        opened(&scratch, &id);
        sh(
            &scratch.tree(),
            &format!(
                "cp -r .engineering/cases ../x && rm -rf .engineering/cases && ln -s {link} .engineering/cases"
            ),
        );
        assert_not_resumed(&scratch, &id, ".engineering/cases");
    }
}

/// A case whose own directory is a link, dangling or to a copy of it beside the tree, is not
/// resumed either.
#[test]
fn a_case_whose_directory_is_a_link_is_not_resumed() {
    for (n, link) in ["../x", "../../../x"].into_iter().enumerate() {
        let id = format!("case-{n}");
        let scratch = repository(&id, &[]);
        opened(&scratch, &id);
        sh(
            &scratch.tree(),
            &format!(
                "cp -r .engineering/cases/{id} ../x && rm -rf .engineering/cases/{id} && ln -s {link} .engineering/cases/{id}"
            ),
        );
        assert_not_resumed(&scratch, &id, &format!(".engineering/cases/{id}"));
    }
}

/// The step is reset before the cases directory is put back, and whatever putting it back
/// says: a step that commits, writes into its case and makes the case's directory read-only, so
/// that what it wrote cannot be removed, still leaves `HEAD` and the refs as they were.
#[test]
fn a_step_whose_cases_change_cannot_be_put_back_is_still_reset() {
    let scratch = repository("read-only", &[]);
    let tree = scratch.tree();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Swaps::running(
        "git -c user.name=t -c user.email=t@localhost commit -q --allow-empty -m stepped && git branch planted && echo forged > .engineering/cases/read-only/authority.yaml && chmod 555 .engineering/cases/read-only".to_owned(),
    );
    let mut run =
        Run::open(&tree, options("read-only", 4), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    let case = tree.join(".engineering/cases/read-only");
    fs::set_permissions(&case, fs::Permissions::from_mode(0o755)).expect("writable again");
    assert_eq!(harness.calls.get(), 1, "{stop:?}\n{text}");
    assert_eq!(
        refs(&tree),
        *harness.refs.borrow(),
        "HEAD and refs\n{stop:?}\n{text}"
    );
}

/// No case is opened under a cases directory that is a link either: the open is refused naming
/// it, with no branch made, the tree on `main` and nothing written through the link.
#[test]
fn a_case_is_not_opened_under_a_linked_cases_directory() {
    let scratch = repository("open", &[]);
    let tree = scratch.tree();
    fs::create_dir_all(scratch.copy()).expect("the directory the link leads to");
    fs::create_dir_all(tree.join(".engineering")).expect("the engineering directory");
    sh(&tree, "ln -s ../../x .engineering/cases");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Swaps::new("../x");
    let opened = Run::open(&tree, options("open", 1), &harness, &validators);
    let Err(error) = opened else {
        panic!("a case is opened through a link");
    };
    let error = format!("{error:#}");
    assert!(
        error.contains(".engineering/cases") && error.contains("a link"),
        "the refusal names the link: {error}"
    );
    assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert!(git(&tree, &["branch", "--list", "takt/*"]).is_empty());
    assert!(
        files(&scratch.copy()).is_empty(),
        "nothing written through it"
    );
}
