//! A step or a test command that replaces the cases directory with a symbolic link is undone
//! like any other foreign write: the link is removed, the directory put back from the snapshot
//! taken before it, `HEAD` and every ref rolled back, nothing written through the link, and the
//! run stops naming the link. A case whose cases directory, or whose own directory, is a link is
//! never resumed: the run refuses before its first step. Every part of a discarded step or
//! observation is put back whatever another part's error, and nothing is put back through a link
//! on the way from the tree down.

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
    /// Gives every directory and file back its owner's permissions first: a step below makes
    /// some read-only or unreadable.
    fn drop(&mut self) {
        let _ = Command::new("chmod")
            .args(["-R", "u+rwx"])
            .arg(&self.0)
            .status();
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
/// its bytes, the event log only grown by the lines the run wrote after putting it back, the
/// observations only grown by the outcome Commission's runtime recorded of the refused step
/// after it (a refused effect's, or the suspended executor step of a judgement step takt
/// refused), and nothing else added; the run's own hold, `run.lock`, is let go when the run ends.
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
        } else if path.ends_with("observations.json") {
            let held = observations(bytes);
            let now = observations(now);
            assert!(
                now.starts_with(&held),
                "{} keeps what it held\n{text}",
                path.display()
            );
            for added in &now[held.len()..] {
                let outcome = (&added["source"], &added["payload"]["outcome"]);
                assert!(
                    outcome.0 == "effect" || (outcome.0 == "executor" && outcome.1 == "Suspended"),
                    "{} grows only by the outcome of the refused step: {added}\n{text}",
                    path.display()
                );
            }
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

/// The observations a case's `observations.json` holds, in the order kept.
fn observations(bytes: &[u8]) -> Vec<serde_json::Value> {
    serde_json::from_slice(bytes).expect("observations.json is a JSON array")
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

/// A harness for runs whose first step is the observation: no judgement step is expected, and
/// one asked for ends the run with an error.
#[derive(Default)]
struct NoStep {
    calls: Cell<usize>,
}

impl Harness for NoStep {
    fn name(&self) -> &str {
        "no-step"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        Err(anyhow!(
            "no judgement step is expected before the observation"
        ))
    }
}

/// A test command that swaps the cases directory for a link, dangling or to a copy of it, is
/// refused naming the link, and undone: the directory is a directory again, as the command found
/// it, `HEAD` and the refs as before it, and nothing was recorded or written through the link.
#[test]
fn a_test_command_that_replaces_the_cases_directory_with_a_link_is_undone() {
    for (n, link) in LINKS.into_iter().enumerate() {
        let id = format!("validator-{n}");
        let scratch = repository_with_test(&id, &swap(link), &[]);
        let (stop, text) = observe(&scratch, &id);
        let text = format!("{link}\n{text}");
        let seen = files(&scratch.0.join("seen"));
        assert_undone(&scratch, &stop, &seen, &refs_seen(&scratch, &text), &text);
    }
}

/// A committed repository with `files` whose test command, `sh tests/run.sh`, runs
/// `tests/case.sh`: it copies the cases directory to `../seen` and the refs to `../refs` as it
/// finds them, then runs `body`. Both scripts are committed before the case opens: the scripts
/// the test command runs are on the judge surface, so no step may write them.
fn repository_with_test(name: &str, body: &str, files: &[(&str, &str)]) -> Scratch {
    let case = format!(
        "#!/bin/sh\nset -e\ncp -r .engineering/cases ../seen\ngit rev-parse HEAD > ../refs\ngit for-each-ref --format='%(refname) %(objectname)' >> ../refs\n{body}\n"
    );
    let mut all = vec![
        (
            "tests/run.sh",
            "#!/bin/sh\nexec sh \"$(dirname \"$0\")/case.sh\" \"$@\"\n",
        ),
        ("tests/case.sh", case.as_str()),
    ];
    all.extend_from_slice(files);
    repository(name, &all)
}

/// The run of the case `id` in `scratch`, whose test command is `sh tests/run.sh`; its stop and
/// its output. The observation is its first step: a judgement step asked for fails the test.
fn observe(scratch: &Scratch, id: &str) -> (anyhow::Result<Stop>, String) {
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("sh tests/run.sh").expect("a command"),
    )];
    let harness = NoStep::default();
    let mut run =
        Run::open(&scratch.tree(), options(id, 8), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    drop(run);
    let text = format!("{stop:?}\n{}", String::from_utf8_lossy(&out));
    assert_eq!(
        harness.calls.get(),
        0,
        "a judgement step was asked for\n{text}"
    );
    (stop, text)
}

/// `HEAD` and the refs as the test command found them, from `../refs`.
fn refs_seen(scratch: &Scratch, text: &str) -> String {
    fs::read_to_string(scratch.0.join("refs"))
        .unwrap_or_else(|_| panic!("the test command ran\n{text}"))
        .trim_end()
        .to_owned()
}

/// The run of the case `id` in `scratch` whose one judgement step runs `script`, with `true` as
/// the test command; its stop, its output and the harness, which holds what the step found.
fn step(scratch: &Scratch, id: &str, script: String) -> (anyhow::Result<Stop>, String, Swaps) {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Swaps::running(script);
    let mut run =
        Run::open(&scratch.tree(), options(id, 4), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    drop(run);
    let text = format!("{stop:?}\n{}", String::from_utf8_lossy(&out));
    (stop, text, harness)
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

/// The script line that commits on the case's branch and makes the branch `planted`, so `HEAD`
/// and the refs move.
const COMMITS: &str = "git -c user.name=t -c user.email=t@localhost commit -q --allow-empty -m planted && git branch planted";

/// The script line that makes the reset fail: a tracked file changed in a directory made
/// read-only, which `git reset --hard` cannot write.
const RESET_FAILS: &str = "echo b > src/a && chmod 555 src";

/// A step that puts a link to a copy of `.engineering` beside the tree in its place, the copy
/// differing from the tree in one case file, and makes the reset fail writes nothing outside the
/// tree through the link: no case file is put back there.
#[test]
fn an_engineering_link_with_a_failing_reset_writes_nothing_outside() {
    let id = "eng-reset";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (_, text, _) = step(
        &scratch,
        id,
        format!(
            "cp -r .engineering ../eng && rm ../eng/cases/{id}/case.yaml && rm -rf .engineering && ln -s ../eng .engineering && {RESET_FAILS}"
        ),
    );
    let outside = scratch.0.join("eng/cases").join(id).join("case.yaml");
    assert!(
        fs::symlink_metadata(&outside).is_err(),
        "{} was written through the link\n{text}",
        outside.display()
    );
    // The judge surface is put back before the cases directory, so the link at `.engineering`
    // is gone, as itself, by the time the case is written again, in the tree.
    let tree = scratch.tree();
    assert!(
        fs::symlink_metadata(tree.join(".engineering")).is_ok_and(|meta| meta.is_dir()),
        "{text}"
    );
    assert!(
        tree.join(".engineering/cases")
            .join(id)
            .join("case.yaml")
            .is_file(),
        "{text}"
    );
}

/// The same link with a reset that succeeds: `.engineering` is a directory again with the case in
/// it, nothing is written outside the tree, and `HEAD` and the refs are as before the step.
#[test]
fn an_engineering_link_is_undone() {
    let id = "eng-ok";
    let scratch = repository(id, &[]);
    let (_, text, harness) = step(
        &scratch,
        id,
        format!(
            "cp -r .engineering ../eng && rm ../eng/cases/{id}/case.yaml && rm -rf .engineering && ln -s ../eng .engineering"
        ),
    );
    let tree = scratch.tree();
    assert!(
        fs::symlink_metadata(tree.join(".engineering")).is_ok_and(|meta| meta.is_dir()),
        "{text}"
    );
    assert!(
        tree.join(".engineering/cases")
            .join(id)
            .join("case.yaml")
            .is_file(),
        "{text}"
    );
    assert!(
        fs::symlink_metadata(scratch.0.join("eng/cases").join(id).join("case.yaml")).is_err(),
        "{text}"
    );
    assert_eq!(refs(&tree), *harness.refs.borrow(), "{text}");
}

/// `.engineering` replaced by a link to an identical copy beside the tree, git tracking a file in
/// it or ignoring it: refused, and `.engineering` a directory again with the case in it.
#[test]
fn an_engineering_tracked_or_ignored_replaced_by_a_link_to_a_copy_is_undone() {
    for (id, file) in [
        ("eng-tracked", (".engineering/planning/x.md", "x\n")),
        ("eng-ignored", (".gitignore", ".engineering\n")),
    ] {
        let scratch = repository(id, &[file]);
        let (stop, text, _) = step(
            &scratch,
            id,
            "cp -r .engineering ../eng && rm -rf .engineering && ln -s ../eng .engineering"
                .to_owned(),
        );
        let tree = scratch.tree();
        assert!(matches!(stop, Ok(Stop::StepRefused(_))), "{text}");
        assert!(
            fs::symlink_metadata(tree.join(".engineering")).is_ok_and(|meta| meta.is_dir()),
            "{text}"
        );
        assert!(
            tree.join(".engineering/cases")
                .join(id)
                .join("case.yaml")
                .is_file(),
            "{text}"
        );
    }
}

/// The cases directory replaced by a file, a FIFO, a dangling link or a link to itself, or
/// `.engineering` by a link to itself: each refused, the cases directory a directory again with
/// the case in it, and `HEAD` and the refs as before the step.
#[test]
fn a_cases_directory_replaced_by_another_kind_is_undone() {
    for (n, swap) in [
        "rm -rf .engineering/cases && echo f > .engineering/cases",
        "rm -rf .engineering/cases && mkfifo .engineering/cases",
        "rm -rf .engineering/cases && ln -s /nonexistent/nowhere .engineering/cases",
        "rm -rf .engineering/cases && ln -s cases .engineering/cases",
        "cp -r .engineering ../eng && rm -rf .engineering && ln -s .engineering .engineering",
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("kind-{n}");
        let scratch = repository(&id, &[]);
        let (stop, text, harness) = step(&scratch, &id, swap.to_owned());
        let tree = scratch.tree();
        assert!(matches!(stop, Ok(Stop::StepRefused(_))), "{swap}\n{text}");
        assert!(
            fs::symlink_metadata(tree.join(".engineering/cases")).is_ok_and(|meta| meta.is_dir()),
            "{swap}\n{text}"
        );
        assert!(
            tree.join(".engineering/cases")
                .join(&id)
                .join("case.yaml")
                .is_file(),
            "{swap}\n{text}"
        );
        assert_eq!(refs(&tree), *harness.refs.borrow(), "{swap}\n{text}");
    }
}

/// A case file replaced by a link to a file beside the tree: refused, the case's files there
/// again, and the file the link leads to not written.
#[test]
fn a_case_file_replaced_by_a_link_is_undone() {
    let id = "file-link";
    let scratch = repository(id, &[]);
    let (stop, text, _) = step(
        &scratch,
        id,
        format!(
            "cp .engineering/cases/{id}/events.jsonl ../outside && cp ../outside ../outside.before && rm .engineering/cases/{id}/events.jsonl && ln -s ../../../../outside .engineering/cases/{id}/events.jsonl"
        ),
    );
    let tree = scratch.tree();
    assert!(matches!(stop, Ok(Stop::StepRefused(_))), "{text}");
    assert!(
        fs::symlink_metadata(tree.join(".engineering/cases").join(id).join("case.yaml"))
            .is_ok_and(|meta| meta.is_file()),
        "{text}"
    );
    assert_eq!(
        fs::read(scratch.0.join("outside")).expect("the file outside"),
        fs::read(scratch.0.join("outside.before")).expect("its copy"),
        "{text}"
    );
}

/// A case file replaced by a hard link to an identical file beside the tree: whatever the run
/// does next, the file beside the tree is not written.
#[test]
fn a_case_file_replaced_by_a_hard_link_is_not_written_through() {
    let id = "hard-link";
    let scratch = repository(id, &[]);
    let (_, text, _) = step(
        &scratch,
        id,
        format!(
            "cp -p .engineering/cases/{id}/case.yaml ../outside && cp -p ../outside ../outside.before && rm .engineering/cases/{id}/case.yaml && ln ../outside .engineering/cases/{id}/case.yaml"
        ),
    );
    assert_eq!(
        fs::read_to_string(scratch.0.join("outside")).unwrap_or_default(),
        fs::read_to_string(scratch.0.join("outside.before")).unwrap_or_default(),
        "a case file was written through a hard link\n{text}"
    );
}

/// Every error after the observation goes through the full undo first: a test command that
/// commits, makes a branch and leaves an unreadable file in its case, so reading the cases
/// directory fails, still leaves `HEAD` and the refs as it found them.
#[test]
fn a_test_command_that_leaves_an_unreadable_case_file_is_still_reset() {
    let id = "unreadable";
    let scratch = repository_with_test(
        id,
        &format!(
            "{COMMITS}\necho z > .engineering/cases/{id}/secret && chmod 000 .engineering/cases/{id}/secret"
        ),
        &[],
    );
    let (_, text) = observe(&scratch, id);
    assert_eq!(refs(&scratch.tree()), refs_seen(&scratch, &text), "{text}");
}

/// The same on the judgement path: a step that commits, makes a branch and leaves an unreadable
/// file in its case leaves `HEAD` and the refs as it found them.
#[test]
fn a_step_that_leaves_an_unreadable_case_file_is_still_reset() {
    let id = "unreadable-step";
    let scratch = repository(id, &[]);
    let (_, text, harness) = step(
        &scratch,
        id,
        format!(
            "{COMMITS} && echo z > .engineering/cases/{id}/secret && chmod 000 .engineering/cases/{id}/secret"
        ),
    );
    assert_eq!(refs(&scratch.tree()), *harness.refs.borrow(), "{text}");
}

/// A case whose event log is a link to a file beside the tree: a run resumed on it, refused or
/// not, writes nothing to that file.
#[test]
fn a_resumed_run_writes_nothing_through_a_linked_case_file() {
    let id = "resume-file";
    let scratch = repository(id, &[]);
    let tree = scratch.tree();
    drop(step(&scratch, id, "true".to_owned()));
    git(&tree, &["checkout", "-q", "main"]);
    sh(
        &tree,
        &format!(
            "cp .engineering/cases/{id}/events.jsonl ../outside && rm .engineering/cases/{id}/events.jsonl && ln -s ../../../../outside .engineering/cases/{id}/events.jsonl"
        ),
    );
    let before = fs::read(scratch.0.join("outside")).expect("the file outside");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Swaps::running("true".to_owned());
    let text = match Run::resume(&tree, options(id, 1), id, &harness, &validators) {
        Ok(mut run) => {
            let mut out = Vec::new();
            let stop = run.run(&mut out);
            format!("resumed: {stop:?}\n{}", String::from_utf8_lossy(&out))
        }
        Err(error) => format!("refused: {error:#}"),
    };
    assert_eq!(
        fs::read(scratch.0.join("outside")).expect("the file outside"),
        before,
        "{text}"
    );
}

/// A case under an `.engineering` that is a link is not resumed either.
#[test]
fn a_case_under_a_linked_engineering_directory_is_not_resumed() {
    let id = "eng-resume";
    let scratch = repository(id, &[]);
    opened(&scratch, id);
    sh(
        &scratch.tree(),
        "mv .engineering ../eng && ln -s ../eng .engineering",
    );
    assert_not_resumed(&scratch, id, ".engineering");
}

/// The cases directory replaced by a link to a copy beside the tree, and the reset made to fail:
/// the link is removed as itself, nothing is written through it, and the cases directory is put
/// back in the tree.
#[test]
fn a_cases_link_with_a_failing_reset_writes_nothing_outside() {
    let id = "cases-reset";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (_, text, _) = step(
        &scratch,
        id,
        format!(
            "cp -r .engineering/cases ../c && rm ../c/{id}/case.yaml && rm -rf .engineering/cases && ln -s ../../c .engineering/cases && {RESET_FAILS}"
        ),
    );
    assert!(
        fs::symlink_metadata(scratch.0.join("c").join(id).join("case.yaml")).is_err(),
        "written through the link\n{text}"
    );
    assert!(
        scratch
            .tree()
            .join(".engineering/cases")
            .join(id)
            .join("case.yaml")
            .is_file(),
        "{text}"
    );
}

/// A step that rewrites its case's file and makes the reset fail: the case's file is put back
/// all the same.
#[test]
fn a_case_file_is_put_back_when_the_reset_fails() {
    let id = "putback-reset";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (_, text, _) = step(
        &scratch,
        id,
        format!(
            "cp .engineering/cases/{id}/case.yaml ../case.before && echo forged > .engineering/cases/{id}/case.yaml && {RESET_FAILS}"
        ),
    );
    assert_eq!(
        fs::read(
            scratch
                .tree()
                .join(".engineering/cases")
                .join(id)
                .join("case.yaml")
        )
        .expect("case.yaml"),
        fs::read(scratch.0.join("case.before")).expect("its copy"),
        "{text}"
    );
}

/// What the step or the observation below does besides making the reset fail: a file on the
/// judge surface, a file in its case and a cargo configuration beside the tree, which cargo reads
/// for it.
fn writes_everywhere(id: &str) -> String {
    format!(
        "mkdir -p .github && echo x > .github/planted.yml && echo forged > .engineering/cases/{id}/authority.yaml && mkdir -p ../.cargo && echo '[build]' > ../.cargo/config.toml && {RESET_FAILS}"
    )
}

/// After a reset that failed, every other part was put back all the same.
fn assert_every_part_put_back(scratch: &Scratch, id: &str, text: &str) {
    let tree = scratch.tree();
    assert!(
        fs::symlink_metadata(tree.join(".github/planted.yml")).is_err(),
        "the judge surface is put back\n{text}"
    );
    assert!(
        fs::symlink_metadata(
            tree.join(".engineering/cases")
                .join(id)
                .join("authority.yaml")
        )
        .is_err(),
        "the cases directory is put back\n{text}"
    );
    assert!(
        fs::symlink_metadata(scratch.0.join(".cargo/config.toml")).is_err(),
        "what cargo reads outside the tree is put back\n{text}"
    );
}

/// A discarded step whose reset fails still has its judge surface, its cases directory and what
/// lies outside the tree put back: no part of the undo is skipped for another's error.
#[test]
fn every_part_of_a_discarded_step_is_put_back_when_the_reset_fails() {
    let id = "every-part-step";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (_, text, _) = step(&scratch, id, writes_everywhere(id));
    assert_every_part_put_back(&scratch, id, &text);
}

/// The same for a discarded observation: a test command that makes the reset fail still has the
/// rest of what it did put back.
#[test]
fn every_part_of_a_discarded_observation_is_put_back_when_the_reset_fails() {
    let id = "every-part-observation";
    let scratch = repository_with_test(id, &writes_everywhere(id), &[("src/a", "a\n")]);
    let (_, text) = observe(&scratch, id);
    refs_seen(&scratch, &text);
    assert_every_part_put_back(&scratch, id, &text);
}

/// An observation that leaves a file on the judge surface the run cannot read ends the run on
/// that error, but only after the full undo: `HEAD` and the refs as the test command found them,
/// its case file removed and what cargo reads beside the tree put back.
#[test]
fn an_observation_whose_judge_surface_cannot_be_read_is_undone_whole() {
    let id = "surface-unreadable";
    let scratch = repository_with_test(
        id,
        &format!(
            "{COMMITS}\necho forged > .engineering/cases/{id}/authority.yaml\nmkdir -p ../.cargo && echo '[build]' > ../.cargo/config.toml\nmkdir -p .github && echo z > .github/secret && chmod 000 .github/secret"
        ),
        &[],
    );
    let (_, text) = observe(&scratch, id);
    let tree = scratch.tree();
    assert_eq!(refs(&tree), refs_seen(&scratch, &text), "{text}");
    assert!(
        fs::symlink_metadata(
            tree.join(".engineering/cases")
                .join(id)
                .join("authority.yaml")
        )
        .is_err(),
        "the cases directory is put back\n{text}"
    );
    assert!(
        fs::symlink_metadata(scratch.0.join(".cargo/config.toml")).is_err(),
        "what cargo reads outside the tree is put back\n{text}"
    );
}

/// The script line that leaves a file on the judge surface the run cannot read, so putting the
/// surface back fails (with [`RESET_FAILS`], `git clean` never removes it).
const SURFACE_UNREADABLE: &str =
    "mkdir -p .github && echo z > .github/secret && chmod 000 .github/secret";

/// A discarded step whose reset fails and whose judge surface cannot be put back still has its
/// cases directory and what lies outside the tree put back: the undo goes on past the surface's
/// error.
#[test]
fn the_undo_goes_on_past_a_judge_surface_that_cannot_be_put_back() {
    let id = "surface-fails";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (_, text, _) = step(
        &scratch,
        id,
        format!(
            "echo forged > .engineering/cases/{id}/authority.yaml && mkdir -p ../.cargo && echo '[build]' > ../.cargo/config.toml && {SURFACE_UNREADABLE} && {RESET_FAILS}"
        ),
    );
    let tree = scratch.tree();
    assert!(
        fs::symlink_metadata(
            tree.join(".engineering/cases")
                .join(id)
                .join("authority.yaml")
        )
        .is_err(),
        "the cases directory is put back\n{text}"
    );
    assert!(
        fs::symlink_metadata(scratch.0.join(".cargo/config.toml")).is_err(),
        "what cargo reads outside the tree is put back\n{text}"
    );
}

/// A link at `.engineering` that the judge surface's put-back cannot remove (the reset fails and
/// the surface cannot be read): putting the cases directory back is refused naming the link, and
/// nothing is written through it.
#[test]
fn nothing_is_put_back_through_a_link_on_the_way_to_the_cases_directory() {
    let id = "way-link";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (stop, text, _) = step(
        &scratch,
        id,
        format!(
            "cp -r .engineering ../eng && rm ../eng/cases/{id}/case.yaml && rm -rf .engineering && ln -s ../eng .engineering && {SURFACE_UNREADABLE} && {RESET_FAILS}"
        ),
    );
    let outside = scratch.0.join("eng/cases").join(id).join("case.yaml");
    assert!(
        fs::symlink_metadata(&outside).is_err(),
        "{} was written through the link\n{text}",
        outside.display()
    );
    let Err(error) = stop else {
        panic!("the run ends on the undo's error\n{text}");
    };
    assert!(
        format!("{error:#}").contains(".engineering is a link on the way to .engineering/cases"),
        "the refusal names the link\n{text}"
    );
}

/// A step that leaves `.engineering` a link to a copy beside the tree and makes its undo fail
/// (the reset fails, the judge surface cannot be read, so the link stays): the run ends on the
/// error and records nothing through the link. The copy's event log gets no error line, its
/// hold is not removed, and the error says nothing was recorded, naming the link.
#[test]
fn a_failed_undo_records_nothing_through_an_engineering_link() {
    let id = "err-link";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (stop, text, _) = step(
        &scratch,
        id,
        format!(
            "cp -r .engineering ../eng && rm ../eng/cases/{id}/case.yaml && rm -rf .engineering && ln -s ../eng .engineering && {SURFACE_UNREADABLE} && {RESET_FAILS}"
        ),
    );
    let copy = scratch.0.join("eng/cases").join(id);
    let events = fs::read_to_string(copy.join("events.jsonl")).unwrap_or_default();
    assert!(
        !events.contains("\"error\""),
        "the run's error line was written through the link to {}\n{text}",
        copy.join("events.jsonl").display()
    );
    assert!(
        copy.join("run.lock").exists(),
        "the hold beside the tree, {}, was removed through the link\n{text}",
        copy.join("run.lock").display()
    );
    let Err(error) = stop else {
        panic!("the run ends on the error\n{text}");
    };
    let error = format!("{error:#}");
    assert!(
        error.contains("nothing of the run was recorded")
            && error.contains(".engineering is a link to ../eng"),
        "the error says nothing was recorded, naming the link: {error}"
    );
}

/// After such a half-undone run the tree still has `.engineering` a link: no case is opened
/// through it, the half-undone case is not resumed, and nothing is written beside the tree.
#[test]
fn a_half_undone_tree_opens_and_resumes_no_case_through_the_link() {
    let id = "half";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (_, text, _) = step(
        &scratch,
        id,
        format!(
            "cp -r .engineering ../eng && rm -rf .engineering && ln -s ../eng .engineering && {SURFACE_UNREADABLE} && {RESET_FAILS}"
        ),
    );
    let tree = scratch.tree();
    assert!(
        fs::symlink_metadata(tree.join(".engineering"))
            .is_ok_and(|meta| meta.file_type().is_symlink()),
        "precondition: the link stays after the failed undo\n{text}"
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Swaps::running("true".to_owned());
    assert!(
        Run::open(&tree, options("next", 1), &harness, &validators).is_err(),
        "a case was opened under the linked .engineering\n{text}"
    );
    assert!(
        Run::resume(&tree, options(id, 1), id, &harness, &validators).is_err(),
        "the half-undone case was resumed\n{text}"
    );
    assert_eq!(harness.calls.get(), 0, "no step was taken");
    assert!(
        fs::symlink_metadata(scratch.0.join("eng/cases/next")).is_err(),
        "a case was written beside the tree\n{text}"
    );
}

/// The undo's error joins every part's: a step that leaves a file on the judge surface and a
/// cargo configuration beside the tree, each in a directory it made read-only so neither can be
/// removed, ends on an error that names both removals.
#[test]
fn a_failed_undo_names_the_surface_and_the_outside_errors() {
    let id = "joined";
    let scratch = repository(id, &[]);
    let (stop, text, _) = step(
        &scratch,
        id,
        "mkdir -p .github && echo x > .github/planted.yml && chmod 555 .github && mkdir -p ../.cargo && echo '[build]' > ../.cargo/config.toml && chmod 555 ../.cargo".to_owned(),
    );
    let Err(error) = stop else {
        panic!("the run ends on the undo's error\n{text}");
    };
    let error = format!("{error:#}");
    let real = fs::canonicalize(scratch.tree()).expect("the tree's real path");
    let surface = format!("remove {}", real.join(".github/planted.yml").display());
    assert!(
        error.contains(&surface),
        "the surface's error, {surface}, is named: {error}"
    );
    // The step made `.cargo` beside the tree, so putting it back removes that directory whole.
    let outside = format!("remove {}:", scratch.0.join(".cargo").display());
    assert!(
        error.contains(&outside),
        "the outside error, {outside}, is named: {error}"
    );
}

/// A case opened in the tree whose `.engineering` is then moved beside it and replaced by a link
/// before the run starts: the run takes no hold and no step, and writes nothing beside the tree.
#[test]
fn a_run_takes_no_hold_through_a_link_on_the_way_to_its_case() {
    let id = "hold-link";
    let scratch = repository(id, &[]);
    let tree = scratch.tree();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Swaps::running("true".to_owned());
    let mut run = Run::open(&tree, options(id, 1), &harness, &validators).expect("the case opens");
    sh(&tree, "mv .engineering ../eng && ln -s ../eng .engineering");
    let before = files(&scratch.0.join("eng"));
    let mut out = Vec::new();
    let Err(error) = run.run(&mut out) else {
        panic!("the run went on through the link");
    };
    assert!(
        format!("{error:#}").contains(".engineering is a link to ../eng"),
        "{error:#}"
    );
    assert_eq!(harness.calls.get(), 0, "no step was taken");
    assert_eq!(
        files(&scratch.0.join("eng")),
        before,
        "nothing written beside the tree"
    );
}

/// A harness whose first step runs `script` and then panics.
struct PanicsAfter(String);

impl Harness for PanicsAfter {
    fn name(&self) -> &str {
        "panics-after"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        sh(context.tree, &self.0);
        panic!("the harness broke");
    }
}

/// A harness that leaves `.engineering` a link to a copy beside the tree, makes its undo fail,
/// then panics: the panic goes on, and the run records nothing through the link on its way out,
/// no error line in the copy's event log and its hold not removed.
#[test]
fn a_panic_after_a_failed_undo_records_nothing_through_an_engineering_link() {
    let id = "panic-link";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let tree = scratch.tree();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = PanicsAfter(format!(
        "cp -r .engineering ../eng && rm ../eng/cases/{id}/case.yaml && rm -rf .engineering && ln -s ../eng .engineering && {SURFACE_UNREADABLE} && {RESET_FAILS}"
    ));
    let mut run = Run::open(&tree, options(id, 4), &harness, &validators).expect("the case opens");
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut out = Vec::new();
        let _ = run.run(&mut out);
    }));
    assert!(panicked.is_err(), "the harness's panic goes on");
    let copy = scratch.0.join("eng/cases").join(id);
    let events = fs::read_to_string(copy.join("events.jsonl")).unwrap_or_default();
    assert!(
        !events.contains("\"error\""),
        "the panic's error line was written through the link: {events}"
    );
    assert!(
        copy.join("run.lock").exists(),
        "the hold beside the tree was removed through the link"
    );
}

/// The copy of a case beside the tree is as the step left it: no `error` and no `stop` line
/// appended to its event log, its hold still there.
fn assert_copy_untouched(copy: &Path, text: &str) {
    let events = fs::read_to_string(copy.join("events.jsonl")).unwrap_or_default();
    assert!(
        !events.contains("\"error\"") && !events.contains("\"stop\""),
        "a line was written through the link into {}:\n{events}\n{text}",
        copy.join("events.jsonl").display()
    );
    assert!(
        copy.join("run.lock").exists(),
        "the hold in the copy {} was removed through the link\n{text}",
        copy.display()
    );
}

/// What a step runs to leave its case's own directory a link to a copy beside the tree that the
/// undo cannot remove (`.engineering/cases` made read-only, the reset failing).
fn case_dir_link(id: &str) -> String {
    format!(
        "cp -r .engineering/cases/{id} ../c && rm ../c/case.yaml && rm -rf .engineering/cases/{id} && ln -s ../../../c .engineering/cases/{id} && chmod 555 .engineering/cases && {RESET_FAILS}"
    )
}

/// `.engineering` real, the case's own directory left a link to a copy beside the tree by a
/// failed undo: nothing is recorded or let go through the link, and the error names it.
#[test]
fn a_case_directory_link_after_a_failed_undo_records_nothing() {
    let id = "case-dir-link";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (stop, text, _) = step(&scratch, id, case_dir_link(id));
    assert!(
        fs::symlink_metadata(scratch.tree().join(".engineering/cases").join(id))
            .is_ok_and(|meta| meta.file_type().is_symlink()),
        "precondition: the case directory is still a link after the failed undo\n{text}"
    );
    assert_copy_untouched(&scratch.0.join("c"), &text);
    let Err(error) = stop else {
        panic!("the run ends on the error\n{text}");
    };
    let error = format!("{error:#}");
    assert!(
        error.contains("nothing of the run was recorded")
            && error.contains(&format!(".engineering/cases/{id} is a link to ../../../c")),
        "the error names the case directory's link: {error}"
    );
}

/// `.engineering` real, `.engineering/cases` left a link to a copy beside the tree by a failed
/// undo (`.engineering` made read-only, the reset failing): nothing is recorded or let go
/// through the link, and the error names it.
#[test]
fn a_cases_directory_link_after_a_failed_undo_records_nothing() {
    let id = "cases-dir-link";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let (stop, text, _) = step(
        &scratch,
        id,
        format!(
            "cp -r .engineering/cases ../x && rm ../x/{id}/case.yaml && rm -rf .engineering/cases && ln -s ../../x .engineering/cases && chmod 555 .engineering && {RESET_FAILS}"
        ),
    );
    assert!(
        fs::symlink_metadata(scratch.tree().join(".engineering/cases"))
            .is_ok_and(|meta| meta.file_type().is_symlink()),
        "precondition: the cases directory is still a link after the failed undo\n{text}"
    );
    assert_copy_untouched(&scratch.0.join("x").join(id), &text);
    let Err(error) = stop else {
        panic!("the run ends on the error\n{text}");
    };
    let error = format!("{error:#}");
    assert!(
        error.contains("nothing of the run was recorded")
            && error.contains(".engineering/cases is a link to ../../x"),
        "the error names the cases link: {error}"
    );
}

/// The panic path with the case's own directory left a link by a failed undo: the panic goes
/// on and nothing is recorded or let go through the link.
#[test]
fn a_panic_with_a_case_directory_link_records_nothing() {
    let id = "panic-case-dir";
    let scratch = repository(id, &[("src/a", "a\n")]);
    let harness = PanicsAfter(case_dir_link(id));
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let mut run =
        Run::open(&scratch.tree(), options(id, 4), &harness, &validators).expect("the case opens");
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut out = Vec::new();
        let _ = run.run(&mut out);
    }));
    assert!(panicked.is_err(), "the harness's panic goes on");
    assert_copy_untouched(&scratch.0.join("c"), "the run panicked");
}
