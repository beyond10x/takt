//! story:the-run-shows-each-repository-obligation: the gate runner is told the change (the case's
//! base, the revision, the story and its tags), the prompt lists each gate of the repository's
//! gates document with its status, and the run's last line and `takt report` print the same
//! table. The gates document is read from the base, so a change that edits it cannot weaken it.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use loom_sdk::session::metaharness::{ScriptedLog, ScriptedRunner};
use loom_sdk::session::{CredentialSource, Kind};
use takt::forge::{Forge, Memory};
use takt::harness::{Harness, SessionHarness, StepContext, StepReport};
use takt::obligations::{self, Status};
use takt::trace::Trace;
use takt::validator::{GateRun, PrOpen, Validator};
use takt::{Decision, Run, RunOptions, Stop, Story, Unit};

const STORY: &str = "story:the-change";

const PROTOCOL: &str = "format: protocol/1

protocol:
  id: gate.pr
  revision: 1
  description: >-
    Edit the implementation until the repository's gate is green at its current revision, then
    open its pull request.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  gate_result:
    description: the result of the repository's gate against an implementation revision
  pull_request:
    description: a pull request opened for an implementation revision

claims:
  gate.green:
    description: the repository's gate is green for the current implementation revision
    true_when:
      evidence:
        kind: gate_result
        result: pass
        subject: implementation

obligations:
  gate.required:
    description: the repository's gate is green for the current implementation revision
    discharged_when:
      claim: gate.green

actions:
  implementation.edit:
    description: change the implementation in the repository's working revision
    effect: write
  gate.run:
    description: run the repository's gate against an implementation revision
    effect: read
    may_produce:
      - evidence: gate_result
  pr.open:
    description: open the pull request for the implementation revision
    effect: write
    precondition:
      claim: gate.green
    may_produce:
      - evidence: pull_request

outcomes:
  accepted:
    description: the repository's gate is green for the current implementation revision
    requires:
      claim: gate.green
";

const RELEASE_NOTES: &str = "format: engineering-gates/1
language: canon-expr/1
gates:
  - id: release-notes
    text: A consumer-visible change records a release note in CHANGELOG.md.
    applies_when: 'story.has_tag(\"consumer-visible\")'
    assertions:
      - 'git.changed(\"CHANGELOG.md\")'
";

const MANIFEST_ONLY: &str = "format: engineering-gates/1
language: canon-expr/1
gates:
  - id: manifest
    text: The repository has its manifest.
    assertions:
      - 'fs.file.exists(\"Cargo.toml\")'
";

const TWO_GATES: &str = "format: engineering-gates/1
language: canon-expr/1
gates:
  - id: release-notes
    text: A consumer-visible change records a release note in CHANGELOG.md.
    applies_when: 'story.has_tag(\"consumer-visible\")'
    assertions:
      - 'git.changed(\"CHANGELOG.md\")'
  - id: manifest
    text: The repository has its manifest.
    assertions:
      - 'fs.file.exists(\"Cargo.toml\")'
";

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

/// A committed repository holding a manifest, a changelog, the gates document `gates` at
/// `gates.yaml`, the protocol and the planning-store file of the story, tagged `tags`.
fn repository(name: &str, gates: &str, tags: &[&str]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("obligations-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join(".engineering/planning/story")).expect("scratch dir");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a manifest");
    fs::write(dir.join("CHANGELOG.md"), "# Changelog\n").expect("a changelog");
    fs::write(dir.join("gates.yaml"), gates).expect("a gates document");
    fs::write(dir.join("protocol.yaml"), PROTOCOL).expect("a protocol");
    let tags: String = tags.iter().map(|tag| format!("- {tag}\n")).collect();
    fs::write(
        dir.join(".engineering/planning/story/the-change.md"),
        format!(
            "---\nformat: aep.planning-md/3\nid: {STORY}\nkind: story\nstatus: active\ntitle: Change the thing\ntags:\n{tags}revision: 1\n---\n# Story\n"
        ),
    )
    .expect("a story");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A harness that, offered `implementation.edit`, writes each of `files` (path, content) and
/// names the action; it never touches any other file.
struct Edits {
    files: Vec<(&'static str, &'static str)>,
    calls: Cell<usize>,
    seen: RefCell<Vec<Vec<(String, Status)>>>,
}

impl Edits {
    fn new(files: &[(&'static str, &'static str)]) -> Self {
        Self {
            files: files.to_vec(),
            calls: Cell::new(0),
            seen: RefCell::new(Vec::new()),
        }
    }
}

impl Harness for Edits {
    fn name(&self) -> &str {
        "edits"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        self.calls.set(self.calls.get() + 1);
        self.seen.borrow_mut().push(
            context
                .obligations
                .iter()
                .map(|obligation| (obligation.id.clone(), obligation.status))
                .collect(),
        );
        for (path, content) in &self.files {
            fs::write(context.tree.join(path), content)?;
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: None,
        })
    }
}

struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    decision: Decision,
    case_dir: PathBuf,
    record: Vec<serde_json::Value>,
    forge_calls: usize,
}

fn options(iterations: u64) -> RunOptions {
    RunOptions {
        protocol: String::new(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some("obligations".to_owned()),
        gate: None,
    }
}

fn the_story() -> Unit {
    Unit::Story(Story {
        id: STORY.to_owned(),
        title: "Change the thing".to_owned(),
        body: "A consumer-visible change.".to_owned(),
    })
}

fn run_with(tree: &Path, harness: &dyn Harness, iterations: u64) -> Ran {
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(GateRun::new("gates.yaml")),
        Box::new(PrOpen::new(forge)),
    ];
    let mut options = options(iterations);
    options.protocol = tree.join("protocol.yaml").display().to_string();
    let mut run =
        Run::open_unit(tree, options, the_story(), harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let record = takt::events::Events::new(&run.case.dir)
        .read()
        .expect("the record reads");
    let decision = run.case.evaluate(99).expect("evaluates");
    Ran {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
        decision,
        case_dir: run.case.dir.clone(),
        record,
        forge_calls: memory.calls.borrow().len(),
    }
}

fn claim<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .claims
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

fn statuses(record: &[serde_json::Value]) -> Vec<(String, Status)> {
    obligations::recorded(record)
        .map(|(_, rows)| rows.into_iter().map(|row| (row.id, row.status)).collect())
        .unwrap_or_default()
}

#[test]
fn a_consumer_visible_story_without_a_note_is_not_ready() {
    let tree = repository("note", RELEASE_NOTES, &["consumer-visible"]);
    let harness = Edits::new(&[("work.txt", "the work\n")]);
    let ran = run_with(&tree, &harness, 4);
    ran.stop.as_ref().expect("the run stops without an error");
    assert_eq!(claim(&ran.decision, "gate.green"), "false", "{}", ran.text);
    assert!(
        harness.calls.get() >= 1,
        "the harness took a step: {}",
        ran.text
    );
    assert_eq!(
        ran.forge_calls, 0,
        "no pull request was opened: {}",
        ran.text
    );
    assert!(!ran.text.contains("step 5: pr.open"), "{}", ran.text);
    assert!(!ran.text.contains("pull_request"), "{}", ran.text);
    assert!(
        ran.text
            .lines()
            .any(|line| line == "obligations: 1 obligation open: release-notes fail"),
        "{}",
        ran.text
    );
    assert_eq!(
        statuses(&ran.record),
        vec![("release-notes".to_owned(), Status::Fail)]
    );
}

#[test]
fn not_applicable_is_not_unchecked() {
    let tree = repository("not-applicable", TWO_GATES, &["internal"]);
    let base = git(&tree, &["rev-parse", "HEAD"]);
    // Before any observation every gate is not yet run, whatever its `applies_when` would say.
    let unrun = obligations::current(&tree, &base, Path::new("gates.yaml"), &[], &base)
        .expect("the gates document reads at the base");
    assert_eq!(
        unrun
            .iter()
            .map(|row| (row.id.as_str(), row.status))
            .collect::<Vec<_>>(),
        vec![
            ("release-notes", Status::NotYetRun),
            ("manifest", Status::NotYetRun)
        ]
    );
    // After the gate ran against a story without the tag the gate does not apply, and the one
    // that does passes.
    let harness = Edits::new(&[]);
    let ran = run_with(&tree, &harness, 3);
    assert_eq!(
        ran.stop.as_ref().expect("the run stops without an error"),
        &Stop::Completed("accepted".to_owned()),
        "{}",
        ran.text
    );
    assert_eq!(
        statuses(&ran.record),
        vec![
            ("release-notes".to_owned(), Status::NotApplicable),
            ("manifest".to_owned(), Status::Pass)
        ],
        "{}",
        ran.text
    );
    assert!(!ran.text.contains("obligations:"), "{}", ran.text);
    // The same table at a revision no observation judged is not yet run again.
    let moved = obligations::current(
        &tree,
        &base,
        Path::new("gates.yaml"),
        &ran.record,
        "0000000000000000000000000000000000000000",
    )
    .expect("reads");
    assert!(
        moved.iter().all(|row| row.status == Status::NotYetRun),
        "{moved:?}"
    );
}

#[test]
fn a_change_to_the_gates_document_does_not_weaken_its_gate() {
    let tree = repository("weaken", RELEASE_NOTES, &["consumer-visible"]);
    let harness = Edits::new(&[("gates.yaml", MANIFEST_ONLY), ("work.txt", "the work\n")]);
    let ran = run_with(&tree, &harness, 4);
    ran.stop.as_ref().expect("the run stops without an error");
    assert!(
        fs::read_to_string(tree.join("gates.yaml"))
            .expect("the document")
            .contains("id: manifest"),
        "the harness did replace the gates document in the tree"
    );
    assert_eq!(claim(&ran.decision, "gate.green"), "false", "{}", ran.text);
    assert_eq!(ran.forge_calls, 0, "{}", ran.text);
    assert_eq!(
        statuses(&ran.record),
        vec![("release-notes".to_owned(), Status::Fail)],
        "the gate the candidate deleted still holds it: {}",
        ran.text
    );
    assert!(
        ran.text
            .contains("obligations: 1 obligation open: release-notes fail"),
        "{}",
        ran.text
    );
}

#[test]
fn the_prompt_names_the_repository_obligations() {
    let tree = repository("prompt", TWO_GATES, &["consumer-visible"]);
    let log = ScriptedLog::default();
    let mut harness = SessionHarness::of(Kind::Claude);
    harness.credentials = CredentialSource::None;
    let kept = log.clone();
    let harness = harness.with_runner(move || {
        Box::new(ScriptedRunner::of_lines(Vec::<String>::new(), kept.clone()))
    });
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory;
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(GateRun::new("gates.yaml")),
        Box::new(PrOpen::new(forge)),
    ];
    let mut options = options(2);
    options.protocol = tree.join("protocol.yaml").display().to_string();
    let mut run =
        Run::open_unit(&tree, options, the_story(), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    // The scripted session never names an action, so the run ends on an error: the prompt is
    // what is read.
    let _ = run.run(&mut out);
    let mut all: Vec<String> = log.launched().into_iter().flatten().collect();
    all.extend(log.written());
    let prompt = all.join("\n");
    assert!(
        prompt.contains(
            "- release-notes: A consumer-visible change records a release note in CHANGELOG.md. [fail]"
        ),
        "{prompt}"
    );
    assert!(
        prompt.contains("- manifest: The repository has its manifest. [pass]"),
        "{prompt}"
    );
    let open = prompt
        .find("Open obligations:")
        .expect("the open obligations");
    let repository = prompt
        .find("Repository obligations")
        .expect("the repository obligations");
    let actions = prompt
        .find("You may perform exactly ONE")
        .expect("the actions");
    assert!(open < repository && repository < actions, "{prompt}");
}

#[test]
fn the_report_prints_the_table() {
    let tree = repository("report", RELEASE_NOTES, &["consumer-visible"]);
    let harness = Edits::new(&[("work.txt", "the work\n")]);
    let ran = run_with(&tree, &harness, 4);
    ran.stop.as_ref().expect("the run stops without an error");
    let trace = Trace::from_case(&ran.case_dir, None, None).expect("the trace reads");
    let mut printed = Vec::new();
    takt::report::print(std::slice::from_ref(&trace), &mut printed).expect("prints");
    let printed = String::from_utf8(printed).expect("utf-8");
    assert!(
        printed
            .lines()
            .any(|line| line.starts_with("| release-notes | fail |")),
        "{printed}"
    );
    let mut json = Vec::new();
    takt::report::print_json(std::slice::from_ref(&trace), &mut json).expect("prints");
    let json: serde_json::Value = serde_json::from_slice(&json).expect("one JSON document");
    let rows = &json["runs"][0]["obligations"];
    assert_eq!(rows[0]["id"], "release-notes", "{json}");
    assert_eq!(rows[0]["status"], "fail", "{json}");
    assert_eq!(rows.as_array().map(Vec::len), Some(1), "{json}");
}
