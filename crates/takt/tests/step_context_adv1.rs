//! Adversarial cases for story:every-harness-reads-the-repository-instructions-from-the-base:
//! the instruction bytes are the base's, never the step's copy, never a link's target, never a
//! path outside the repository.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::metaharness::{ScriptedLog, ScriptedRunner};
use loom_sdk::session::{CredentialSource, Endpoint, Kind};
use takt::case::Decision;
use takt::harness::{ChangeSoFar, Harness, InstructionFilesRefused, SessionHarness};
use takt::{LoadedProtocol, StepContext};

const AGENTS_AT_BASE: &str = "# Agents\n\nThe base's rule: write release notes.\n";

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
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A repository whose first commit tracks `files`, the base the steps read from.
fn repository(name: &str, files: &[(&str, &[u8])]) -> (Scratch, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("step-context-adv1-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    git(&dir, &["init", "-q", "-b", "main"]);
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    for (path, bytes) in files {
        fs::write(dir.join(path), bytes).expect("a file");
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);
    (Scratch(dir), base)
}

fn harness(kind: Kind, log: &ScriptedLog) -> SessionHarness {
    let mut harness = SessionHarness::of(kind);
    harness.credentials = CredentialSource::None;
    if kind == Kind::B10x {
        harness.endpoint = Some(Endpoint {
            base_url: "https://endpoint.invalid/v1".to_owned(),
            wire: "anthropic-messages".to_owned(),
            model: "scripted".to_owned(),
            token_file: PathBuf::from("/nonexistent/token.json"),
            token_pointer: Some("/token".to_owned()),
        });
    }
    let kept = log.clone();
    harness
        .with_runner(move || Box::new(ScriptedRunner::of_lines(Vec::<String>::new(), kept.clone())))
}

/// One step of `harness` in `tree` for `intent`, its instruction files read at `base`.
fn step(
    harness: &SessionHarness,
    tree: &Path,
    base: Option<&str>,
    intent: &str,
) -> anyhow::Result<takt::StepReport> {
    let protocol = LoadedProtocol::builtin("story.delivery/1").expect("a built-in");
    let decision = Decision {
        claims: std::collections::BTreeMap::new(),
        obligations: std::collections::BTreeMap::new(),
        actions: std::collections::BTreeMap::new(),
        outcomes: std::collections::BTreeMap::new(),
        json: String::new(),
    };
    let offered = ["tests.write".to_owned()];
    harness.step(&StepContext {
        tree,
        intent,
        offered: &offered,
        approvals: &[],
        choices: &[],
        granted: &[],
        decision: &decision,
        protocol: &protocol,
        step: 1,
        time_left: None,
        change: &ChangeSoFar::NoneYet,
        failed: &[],
        scope_revision: None,
        base_revision: base,
        commit: None,
    })
}

/// What the session was started with and written: every argument of each child and every line
/// written to it, where a session's brief, and the context files ahead of it, are.
fn received(log: &ScriptedLog) -> String {
    let mut all: Vec<String> = log.launched().into_iter().flatten().collect();
    all.extend(log.written());
    all.join("\n")
}

/// A Claude step whose intent's `## Scope` names `AGENTS.md` is handed the base's bytes as a
/// context file, and also an `@AGENTS.md` mention, which Claude Code resolves in the step's copy
/// of the tree: the edited file reaches the step beside the base's. The story's rule is that each
/// instruction file is read at the base, never from the step's copy.
#[test]
fn a_claude_step_scoped_to_agents_md_is_not_pointed_at_its_copy() {
    let (tree, base) = repository("scoped", &[("AGENTS.md", AGENTS_AT_BASE.as_bytes())]);
    fs::write(tree.0.join("AGENTS.md"), "# Agents\n\nA step's edit.\n").expect("an edit");
    git(&tree.0, &["commit", "-q", "-am", "an accepted step's edit"]);
    let log = ScriptedLog::new();
    let harness = harness(Kind::Claude, &log);
    let intent = "Update the agent notes\n\n## Scope\n\n- `AGENTS.md`\n";
    let _ = step(&harness, &tree.0, Some(&base), intent);
    let text = received(&log);
    assert!(
        text.contains(AGENTS_AT_BASE.trim_end()),
        "the base's bytes: {text}"
    );
    assert!(
        !text.contains("@AGENTS.md"),
        "the step is also pointed at its own copy of AGENTS.md: {text}"
    );
}

/// A base that tracks `AGENTS.md` as a link to a file outside the repository: the target's bytes
/// are never given to any kind.
#[test]
fn a_tracked_link_is_not_followed() {
    let (tree, base) = repository("link", &[]);
    let outside = tree.0.with_extension("outside.md");
    fs::write(&outside, "OUTSIDE-THE-REPOSITORY-MARKER\n").expect("a file outside");
    std::os::unix::fs::symlink(&outside, tree.0.join("AGENTS.md")).expect("a link");
    git(&tree.0, &["add", "AGENTS.md"]);
    git(&tree.0, &["commit", "-q", "-m", "a linked AGENTS.md"]);
    let at = git(&tree.0, &["rev-parse", "HEAD"]);
    assert_ne!(at, base);
    for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
        let log = ScriptedLog::new();
        let harness = harness(kind, &log);
        let _ = step(&harness, &tree.0, Some(&at), "Remove a todo");
        let text = received(&log);
        assert!(
            !text.contains("OUTSIDE-THE-REPOSITORY-MARKER"),
            "{kind:?}: the link's target is given: {text}"
        );
    }
    let _ = fs::remove_file(&outside);
}

/// A configured path that climbs out of the repository is refused by name, and no session starts.
#[test]
fn a_configured_parent_path_is_refused() {
    let (tree, base) = repository("parent", &[("AGENTS.md", AGENTS_AT_BASE.as_bytes())]);
    for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
        let log = ScriptedLog::new();
        let mut harness = harness(kind, &log);
        harness.context = vec!["../outside.md".to_owned(), "/etc/hostname".to_owned()];
        let error = step(&harness, &tree.0, Some(&base), "Remove a todo")
            .expect_err("a path outside the repository is refused");
        let refused = error
            .downcast_ref::<InstructionFilesRefused>()
            .unwrap_or_else(|| panic!("{kind:?}: {error:#}"));
        assert!(
            refused
                .0
                .iter()
                .any(|why| why.starts_with("../outside.md:"))
                && refused
                    .0
                    .iter()
                    .any(|why| why.starts_with("/etc/hostname:")),
            "{kind:?}: {refused}"
        );
        assert_eq!(log.spawns(), 0, "{kind:?}: a session started");
    }
}
