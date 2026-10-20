//! The judgement step's model works in a scratch copy of the case tree, never in the tree itself,
//! and returns its change as a proposal; takt's edit effect applies the proposal to the tree under
//! the step guard, so a step that does more than the frontier admits is refused before a byte of
//! it reaches the tree (story:harness-proposes-takt-performs). Each session here is metaharness's
//! scripted Claude Code: no vendor process, no credential, no network.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessPoll, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use session_executor::{CredentialSource, Proposal, ProposedFile, SessionExecutor};
use takt::harness::{ClaudeSession, Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The executor is loom-sdk's `AgentExecutor`.
#[test]
fn the_executor_is_an_agent_executor_of_loom_sdk() {
    fn implements<T: loom_sdk::commission::ports::executor::AgentExecutor>() {}
    implements::<SessionExecutor>();
}

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

/// A scratch directory under this crate's build directory, `tree/` in it the repository;
/// removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("proposes-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(tree.join("src")).expect("the tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(tree.join("src/lib.rs"), BROKEN).expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    scratch
}

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

/// Every file under `dir` but `.git/`, by path relative to it, a link as `-> <target>`.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            let relative = path.strip_prefix(root).expect("under").to_path_buf();
            if relative == Path::new(".git") {
                continue;
            }
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
    walk(dir, dir, &mut into);
    into
}

/// The tree's files outside `.engineering/`, where the run keeps its case.
fn work(tree: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    files(tree)
        .into_iter()
        .filter(|(path, _)| !path.starts_with(".engineering"))
        .collect()
}

/// What the scripted session saw and left, read after the run.
#[derive(Default)]
struct Seen {
    cwd: Vec<PathBuf>,
    env: Vec<BTreeMap<String, String>>,
    args: Vec<Vec<String>>,
    /// The case tree's work as it stood when the session started and when it ended.
    tree_at_start: Vec<BTreeMap<PathBuf, Vec<u8>>>,
    tree_at_end: Vec<BTreeMap<PathBuf, Vec<u8>>>,
}

/// A scripted Claude Code session: at its start it records the launch plan, writes `writes`
/// into its working directory as the model's file tools would, and then prints an opening
/// record naming that directory, one reply naming `action`, and a result.
struct Session {
    tree: PathBuf,
    writes: Vec<(String, String)>,
    action: String,
    seen: Rc<RefCell<Seen>>,
    /// The plugins its opening record names.
    plugins: serde_json::Value,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        {
            let mut seen = self.seen.borrow_mut();
            seen.cwd.push(plan.cwd.to_path_buf());
            seen.env.push(plan.env.clone());
            seen.args.push(plan.args.to_vec());
            seen.tree_at_start.push(work(&self.tree));
        }
        for (path, text) in &self.writes {
            let at = plan.cwd.join(path);
            fs::create_dir_all(at.parent().expect("a parent"))?;
            fs::write(at, text)?;
        }
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.259",
            "cwd": plan.cwd.display().to_string(), "tools": ["Read", "Edit", "Write", "Glob", "Grep"],
            "mcp_servers": [], "plugins": self.plugins, "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default", "session_id": "s-1",
        });
        let reply = format!(
            "Done.\n{{\"action\": \"{}\", \"summary\": \"scripted\"}}",
            self.action
        );
        let assistant = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": reply}],
                        "usage": {"input_tokens": 10, "output_tokens": 5}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "duration_ms": 5, "duration_api_ms": 4, "total_cost_usd": 0.01,
            "usage": {"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 0,
                      "cache_creation_input_tokens": 0},
            "modelUsage": {"scripted": {}},
        });
        let mut scripted = ScriptedRunner::of_lines(
            [init.to_string(), assistant.to_string(), result.to_string()],
            ScriptedLog::new(),
        );
        Ok(Box::new(Ending {
            inner: scripted.start(plan)?,
            tree: self.tree.clone(),
            seen: Rc::clone(&self.seen),
        }))
    }
}

/// The scripted child, reading the case tree once more when its stream ends.
struct Ending {
    inner: Box<dyn HarnessProcess>,
    tree: PathBuf,
    seen: Rc<RefCell<Seen>>,
}

impl HarnessProcess for Ending {
    fn next_line(&mut self) -> std::io::Result<Option<String>> {
        let line = self.inner.next_line()?;
        if line.is_none() {
            self.seen.borrow_mut().tree_at_end.push(work(&self.tree));
        }
        Ok(line)
    }

    fn poll_line(&mut self) -> std::io::Result<ProcessPoll> {
        let polled = self.inner.poll_line()?;
        if matches!(polled, ProcessPoll::Ended) {
            self.seen.borrow_mut().tree_at_end.push(work(&self.tree));
        }
        Ok(polled)
    }

    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.inner.write_line(line)
    }

    fn kill(&mut self) -> std::io::Result<()> {
        self.inner.kill()
    }

    fn wait(&mut self) -> std::io::Result<Option<i32>> {
        self.inner.wait()
    }
}

/// `--harness claude` over a scripted session that writes `writes` and names `action`.
fn harness(
    tree: &Path,
    writes: &[(&str, &str)],
    action: &str,
) -> (ClaudeSession, Rc<RefCell<Seen>>) {
    harness_with(tree, writes, action, serde_json::json!([]))
}

/// [`harness`], its opening record naming `plugins`.
fn harness_with(
    tree: &Path,
    writes: &[(&str, &str)],
    action: &str,
    plugins: serde_json::Value,
) -> (ClaudeSession, Rc<RefCell<Seen>>) {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let (tree, writes, action, shared) = (
        tree.to_path_buf(),
        writes
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect::<Vec<_>>(),
        action.to_owned(),
        Rc::clone(&seen),
    );
    let harness = ClaudeSession {
        credentials: CredentialSource::None,
        ..ClaudeSession::default()
    }
    .with_runner(move || {
        Box::new(Session {
            tree: tree.clone(),
            writes: writes.clone(),
            action: action.clone(),
            seen: Rc::clone(&shared),
            plugins: plugins.clone(),
        })
    });
    (harness, seen)
}

/// Strict's floor holds a proposal back when the session loaded a plugin metaharness did not
/// declare (H1a), unless every such plugin ships inside the vendor program (`@builtin`): a
/// builtin-only session's proposal is applied, a foreign plugin's is not, and the tree is left
/// as it was.
#[test]
fn a_session_with_a_foreign_plugin_proposes_nothing() {
    let builtin = serde_json::json!([{"name": "cc-plugin-sec-default", "path": "builtin",
                                      "source": "cc-plugin-sec-default@builtin"}]);
    let foreign = serde_json::json!([{"name": "planted", "path": "/somewhere/planted",
                                      "source": "planted@a-marketplace"}]);
    for (name, plugins, applied) in [("builtin", builtin, true), ("foreign", foreign, false)] {
        let scratch = repository(&format!("plugin-{name}"));
        let tree = scratch.tree();
        let before = work(&tree);
        let (harness, _) = harness_with(
            &tree,
            &[("src/lib.rs", FIXED)],
            "implementation.edit",
            plugins,
        );
        let validators = validators();
        let id = format!("proposes-plugin-{name}");
        let mut run = Run::open(&tree, options(&id), &harness, &validators).expect("opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        if applied {
            assert!(stop.is_ok(), "{name}: {stop:?}\n{text}");
            assert_eq!(
                fs::read_to_string(tree.join("src/lib.rs")).expect("the file"),
                FIXED,
                "{name}: applied\n{text}"
            );
        } else {
            let error = format!("{:#}", stop.expect_err("the harness fails"));
            assert!(
                error.contains("hermetic floor has a gating gap: H1a"),
                "{name}: {error}\n{text}"
            );
            assert_eq!(work(&tree), before, "{name}: the tree is unchanged\n{text}");
        }
    }
}

fn validators() -> Vec<Box<dyn Validator>> {
    vec![Box::new(TestsRun::new("true").expect("a command"))]
}

/// Acceptance 2: the session's working directory is a scratch copy outside the case tree; the
/// case tree is byte-identical when the session starts and when it ends, and changes only when
/// the edit effect applies the proposal, which the step then commits.
#[test]
fn the_session_works_in_a_copy_and_only_the_edit_effect_changes_the_tree() {
    let scratch = repository("copy");
    let tree = scratch.tree();
    let before = work(&tree);
    let (harness, seen) = harness(&tree, &[("src/lib.rs", FIXED)], "implementation.edit");
    let validators = validators();
    let mut run = Run::open(&tree, options("proposes-copy"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let seen = seen.borrow();
    assert_eq!(seen.cwd.len(), 1, "one session\n{text}");
    let cwd = &seen.cwd[0];
    let resolved = fs::canonicalize(&tree).expect("the tree resolves");
    assert!(
        !cwd.starts_with(&resolved) && !cwd.starts_with(&tree),
        "the session works outside the tree: {}\n{text}",
        cwd.display()
    );
    assert_eq!(
        seen.tree_at_start,
        vec![before.clone()],
        "untouched at the start\n{text}"
    );
    assert_eq!(
        seen.tree_at_end,
        vec![before.clone()],
        "untouched at the session's end\n{text}"
    );
    assert!(!cwd.exists(), "the copy is removed after the step\n{text}");
    assert!(
        !matches!(stop, Ok(Stop::StepRefused(_)) | Err(_)),
        "the step is accepted: {stop:?}\n{text}"
    );
    assert_eq!(
        fs::read_to_string(tree.join("src/lib.rs")).expect("the file"),
        FIXED,
        "the edit effect applied the proposal\n{text}"
    );
    assert_eq!(
        git(&tree, &["show", "HEAD:src/lib.rs"]),
        FIXED.trim_end(),
        "and the step committed it\n{text}"
    );
}

/// Acceptance 3, by the session: a proposal naming an action the frontier did not offer, or
/// changing the cases directory, ends the run `StepRefused` (exit 3) with the tree unchanged.
#[test]
fn a_session_proposal_the_guard_refuses_leaves_the_tree_unchanged() {
    for (name, writes, action) in [
        (
            "unoffered",
            vec![("src/lib.rs", FIXED)],
            "repository.merge.now",
        ),
        (
            "cases",
            vec![(".engineering/cases/planted.yaml", "granted: true\n")],
            "implementation.edit",
        ),
        ("inspect", vec![("src/lib.rs", FIXED)], "repository.inspect"),
    ] {
        let scratch = repository(name);
        let tree = scratch.tree();
        let before = work(&tree);
        let head = git(&tree, &["rev-parse", "HEAD"]);
        let (harness, _) = harness(&tree, &writes, action);
        let validators = validators();
        let id = format!("proposes-{name}");
        let mut run = Run::open(&tree, options(&id), &harness, &validators).expect("opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        let Ok(Stop::StepRefused(why)) = &stop else {
            panic!("{name}: refused, not {stop:?}\n{text}");
        };
        assert!(
            text.contains("before the edit effect"),
            "{name}: {why}\n{text}"
        );
        assert_eq!(work(&tree), before, "{name}: the tree is unchanged\n{text}");
        assert!(
            !tree.join(".engineering/cases/planted.yaml").exists(),
            "{name}: nothing planted\n{text}"
        );
        assert_eq!(
            git(&tree, &["rev-parse", "HEAD"]),
            head,
            "{name}: HEAD\n{text}"
        );
    }
}

/// A harness that proposes `proposal` as it stands.
struct Proposing(RefCell<Option<Proposal>>);

impl Harness for Proposing {
    fn name(&self) -> &str {
        "proposing"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let proposal = self.0.borrow().clone().expect("one step");
        Ok(StepReport {
            action: proposal.action.clone(),
            summary: proposal.summary.clone(),
            tokens: None,
        })
    }

    fn take_proposal(&self) -> Option<Proposal> {
        self.0.borrow_mut().take()
    }
}

/// Acceptance 3, by the proposal: a path that is not a plain path of the tree, one under `.git`,
/// on the judge surface or reached through a link is refused before a byte reaches the tree.
#[test]
fn a_scripted_proposal_of_a_refused_path_leaves_the_tree_unchanged() {
    for (name, path) in [
        ("parent", "../outside.txt"),
        ("absolute", "/proposes-absolute.txt"),
        ("git", ".git/hooks/pre-commit"),
        ("surface", ".cargo/config.toml"),
        ("link", "linked/x.txt"),
    ] {
        let scratch = repository(name);
        let tree = scratch.tree();
        let elsewhere = scratch.0.join("elsewhere");
        fs::create_dir_all(&elsewhere).expect("a directory");
        std::os::unix::fs::symlink(&elsewhere, tree.join("linked")).expect("a link");
        git(&tree, &["add", "-A"]);
        git(&tree, &["commit", "-q", "-m", "a link"]);
        let before = work(&tree);
        let git_before = files(&tree.join(".git"));
        let harness = Proposing(RefCell::new(Some(Proposal {
            action: "implementation.edit".to_owned(),
            summary: "scripted".to_owned(),
            files: vec![ProposedFile {
                path: path.to_owned(),
                content: Some("planted\n".to_owned()),
            }],
        })));
        let validators = validators();
        let id = format!("proposes-path-{name}");
        let mut run = Run::open(&tree, options(&id), &harness, &validators).expect("opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(
            matches!(&stop, Ok(Stop::StepRefused(_))),
            "{name}: refused, not {stop:?}\n{text}"
        );
        assert!(text.contains("before the edit effect"), "{name}\n{text}");
        assert_eq!(work(&tree), before, "{name}: the tree is unchanged\n{text}");
        assert!(
            files(&elsewhere).is_empty(),
            "{name}: nothing through the link\n{text}"
        );
        assert!(!scratch.0.join("outside.txt").exists(), "{name}\n{text}");
        assert!(
            !Path::new("/proposes-absolute.txt").exists(),
            "{name}\n{text}"
        );
        let git_after = files(&tree.join(".git"));
        assert_eq!(
            git_after.get(Path::new("hooks/pre-commit")),
            git_before.get(Path::new("hooks/pre-commit")),
            "{name}: no hook planted\n{text}"
        );
    }
}

/// The variables the run withholds from a harness: credentials by takt's rule, Claude Code's
/// own session variables and the API key.
const WITHHELD: [(&str, &str); 8] = [
    ("ANTHROPIC_API_KEY", "sk-withheld-anthropic"),
    ("CLAUDECODE", "withheld-nested"),
    ("CLAUDE_PID", "withheld-pid-4242"),
    ("CLAUDE_CODE_ENTRYPOINT", "withheld-entrypoint"),
    ("GITHUB_TOKEN", "ghp-withheld-token"),
    ("GITLAB_HOST", "withheld-gitlab-host"),
    ("SSH_AUTH_SOCK", "/withheld/agent.sock"),
    ("DEPLOY_SECRET", "withheld-deploy-secret"),
];

const CHILD: &str = "TAKT_HARNESS_PROPOSES_CHILD";

/// Acceptance 4: run in a child of this test binary with the withheld variables set, a session's
/// environment and prompt, as metaharness hands them to the child it starts, carry none of them,
/// by name or by value.
#[test]
fn the_session_is_handed_no_withheld_variable() {
    if std::env::var_os(CHILD).is_some() {
        let scratch = repository("withheld");
        let tree = scratch.tree();
        let (harness, seen) = harness(&tree, &[("src/lib.rs", FIXED)], "implementation.edit");
        let validators = validators();
        let mut run =
            Run::open(&tree, options("proposes-withheld"), &harness, &validators).expect("opens");
        let mut out = Vec::new();
        let _ = run.run(&mut out);
        let seen = seen.borrow();
        assert_eq!(seen.env.len(), 1, "{}", String::from_utf8_lossy(&out));
        for (name, value) in WITHHELD {
            assert_eq!(std::env::var(name).as_deref(), Ok(value), "set for the run");
            assert!(
                !seen.env[0].contains_key(name),
                "{name} reached the session"
            );
            for (key, held) in &seen.env[0] {
                assert!(!held.contains(value), "{name}'s value is in {key}");
                assert!(
                    !(takt::forge::is_credential(key)
                        || key == "CLAUDECODE"
                        || key == "CLAUDE_PID"
                        || key.starts_with("CLAUDE_CODE_")),
                    "{key} is withheld from a harness and reached the session"
                );
            }
            for arg in &seen.args[0] {
                assert!(
                    !arg.contains(value),
                    "{name}'s value is in the prompt or arguments"
                );
            }
        }
        return;
    }
    let mut child = Command::new(std::env::current_exe().expect("this test binary"));
    child
        .args([
            "--exact",
            "the_session_is_handed_no_withheld_variable",
            "--nocapture",
        ])
        .env(CHILD, "1");
    for (name, value) in WITHHELD {
        child.env(name, value);
    }
    let output = child.output().expect("the child runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{said}");
    assert!(said.contains("1 passed"), "the child ran the case: {said}");
}
