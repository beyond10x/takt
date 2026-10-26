//! story:no-inspect-for-stateless-harness: a harness that keeps nothing between steps is not
//! offered an action that only reads, because what it read is gone at the next step and the
//! step that changed nothing ends the run with `NoProgress`. The Claude Code adapter runs each
//! step with `--no-session-persistence`: it is driven here through a fake `claude` program that
//! keeps every prompt it is given and picks `repository.inspect` whenever the prompt lists it.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Forge, Memory};
use takt::harness::ClaudeCode;
use takt::validator::{CiRead, Observation, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";

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

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("no-inspect-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// A git tree with one commit under `root`.
fn tree(root: &Path) -> PathBuf {
    let tree = root.join("tree");
    fs::create_dir_all(&tree).expect("a tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

/// A fake `claude`: appends the prompt it was given (its second argument, after `-p`) to
/// `prompts`, each followed by a line `=====`; answers `repository.inspect`, changing nothing,
/// when the prompt lists it, and otherwise appends a line to `src/work.txt` in its working
/// directory and answers `implementation.edit`.
fn fake_claude(at: &Path, prompts: &Path) -> PathBuf {
    let script = format!(
        "#!/bin/sh\n\
         printf '%s\\n=====\\n' \"$2\" >> '{prompts}'\n\
         case \"$2\" in\n\
         *'- repository.inspect:'*)\n\
         printf '%s' '{{\"result\": \"{{\\\"action\\\": \\\"repository.inspect\\\", \\\"summary\\\": \\\"read the code\\\"}}\"}}'\n\
         ;;\n\
         *)\n\
         mkdir -p src\n\
         echo \"$$\" >> src/work.txt\n\
         printf '%s' '{{\"result\": \"{{\\\"action\\\": \\\"implementation.edit\\\", \\\"summary\\\": \\\"edited\\\"}}\"}}'\n\
         ;;\n\
         esac\n",
        prompts = prompts.display()
    );
    fs::write(at, script).expect("a script");
    fs::set_permissions(at, fs::Permissions::from_mode(0o755)).expect("exec bit");
    at.to_path_buf()
}

/// A validator that observes `pass` for its action at every revision.
struct Green {
    action: &'static str,
    kind: &'static str,
}

impl Validator for Green {
    fn name(&self) -> &str {
        self.action
    }

    fn action(&self) -> &str {
        self.action
    }

    fn kind(&self) -> &str {
        self.kind
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "pass".to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("{}-{seq}", self.kind),
                self.kind,
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
        })
    }
}

/// The story's Expected before: on a fresh story case the Claude Code adapter was offered
/// `repository.inspect`, the fake answered it changing nothing, and the run stopped
/// `NoProgress` after that one judgement step. Now the prompt does not offer it and the run
/// never stops `NoProgress` from an inspect step.
#[test]
fn the_claude_code_harness_is_not_offered_repository_inspect() {
    let root = scratch("claude");
    let tree = tree(&root);
    let prompts = root.join("prompts.txt");
    let program = fake_claude(&root.join("claude.sh"), &prompts);
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Green {
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(Green {
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "remove a todo by its number".to_owned(),
        inputs: Vec::new(),
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: Some("fresh".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8(out).expect("utf-8");
    let given = fs::read_to_string(&prompts).unwrap_or_default();
    let first = given.split("=====\n").next().unwrap_or_default();
    assert!(!first.is_empty(), "the fake was never called: {text}");
    assert!(
        !first
            .lines()
            .any(|line| line.starts_with("- repository.inspect")),
        "the prompt offers repository.inspect to a stateless harness:\n{first}"
    );
    assert!(
        !text.contains("repository.inspect (claude-code)"),
        "an inspect step was taken: {text}"
    );
    assert_ne!(stop, Ok(Stop::NoProgress), "{text}");
}

/// Keeps what it is offered at its one step, says whether it keeps state as it was built to,
/// and ends the run.
struct Offered {
    keeps: bool,
    seen: std::cell::RefCell<Vec<String>>,
}

impl takt::harness::Harness for Offered {
    fn name(&self) -> &str {
        "offered"
    }

    fn keeps_state(&self) -> bool {
        self.keeps
    }

    fn step(
        &self,
        context: &takt::harness::StepContext<'_>,
    ) -> anyhow::Result<takt::harness::StepReport> {
        *self.seen.borrow_mut() = context.offered.to_vec();
        anyhow::bail!("seen")
    }
}

/// What a harness that keeps state, or not, is offered at the first judgement step of a fresh
/// case on the delivery protocol with no validator: the actions no validator owns.
fn offered_to(keeps: bool, name: &str) -> Vec<String> {
    let root = scratch(name);
    let tree = tree(&root);
    let harness = Offered {
        keeps,
        seen: std::cell::RefCell::new(Vec::new()),
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "remove a todo by its number".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let _ = run.run(&mut out);
    harness.seen.into_inner()
}

/// The marker decides it, and only for the actions the protocol declares `effect: read` that
/// record no choice: a harness that keeps state is offered `repository.inspect` beside the
/// edits and `change.handover`; one that does not, the default, the edits and
/// `change.handover`, a read whose `handover` record moves the decision. Neither is offered
/// `tests.run` or `gate.run` with no validator for them: only a validator writes their kinds.
#[test]
fn only_a_harness_that_keeps_state_is_offered_the_actions_that_only_read() {
    assert_eq!(
        offered_to(true, "stateful"),
        [
            "change.handover",
            "implementation.edit",
            "repository.inspect",
            "tests.write"
        ]
    );
    assert_eq!(
        offered_to(false, "stateless"),
        ["change.handover", "implementation.edit", "tests.write"]
    );
}

/// The Claude Code adapter is the stateless kind.
#[test]
fn the_claude_code_adapter_keeps_no_state() {
    use takt::harness::Harness as _;
    assert!(!ClaudeCode::default().keeps_state());
}

/// A stateless harness on a protocol whose only admissible action reads is not called at all:
/// the run stops `NothingAdmissible` before a step that could only end it with `NoProgress`.
#[test]
fn with_only_a_read_action_admissible_a_stateless_harness_is_not_called() {
    let root = scratch("read-only");
    let tree = tree(&root);
    let protocol = root.join("read-only.yaml");
    fs::write(
        &protocol,
        "format: protocol/1\n\
         protocol:\n  id: read.only\n  revision: 1\n\
         artifacts:\n  implementation:\n    description: the code\n\
         actions:\n  repository.inspect:\n    description: read the repository\n    effect: read\n\
         outcomes:\n  abandoned:\n    description: given up\n    requires:\n      decision: explicitly_abandoned\n",
    )
    .expect("a protocol");
    let harness = Offered {
        keeps: false,
        seen: std::cell::RefCell::new(vec!["never called".to_owned()]),
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "read".to_owned(),
        inputs: Vec::new(),
        iterations: Some(3),
        hours: None,
        tokens: None,
        case_id: Some("read-only".to_owned()),
        gate: None,
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(stop, Ok(Stop::NothingAdmissible), "{text}");
    assert_eq!(*harness.seen.borrow(), ["never called"], "{text}");
}
