//! Reviewer probes, round 3 (57c6462). Not part of the change.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

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

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("rev-probe-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

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

/// One scripted step: the action named, files written (path, content), the summary.
#[derive(Clone)]
struct Step {
    action: String,
    writes: Vec<(String, String)>,
    summary: String,
}

struct Script {
    keeps: bool,
    steps: RefCell<VecDeque<Step>>,
    offers: RefCell<Vec<Vec<String>>>,
}

impl Script {
    fn new(keeps: bool, steps: Vec<Step>) -> Self {
        Self {
            keeps,
            steps: RefCell::new(steps.into()),
            offers: RefCell::new(Vec::new()),
        }
    }
}

impl Harness for Script {
    fn name(&self) -> &str {
        "script"
    }
    fn keeps_state(&self) -> bool {
        self.keeps
    }
    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.offers.borrow_mut().push(context.offered.to_vec());
        let Some(step) = self.steps.borrow_mut().pop_front() else {
            anyhow::bail!("script exhausted; offered {:?}", context.offered);
        };
        for (path, content) in &step.writes {
            let path = path.replace("$TREE", &context.tree.display().to_string());
            let at = if Path::new(&path).is_absolute() {
                PathBuf::from(path)
            } else {
                context.tree.join(path)
            };
            fs::create_dir_all(at.parent().expect("parent")).expect("dir");
            fs::write(&at, content).expect("write");
        }
        Ok(StepReport {
            action: step.action,
            summary: step.summary,
            tokens: Some(1),
        })
    }
}

fn step(action: &str, writes: &[(&str, &str)], summary: &str) -> Step {
    Step {
        action: action.to_owned(),
        writes: writes
            .iter()
            .map(|(p, c)| ((*p).to_owned(), (*c).to_owned()))
            .collect(),
        summary: summary.to_owned(),
    }
}

fn records(case_dir: &Path, kind: &str) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| {
            let p = e.ok()?.path();
            serde_json::from_str::<serde_json::Value>(&fs::read_to_string(p).ok()?).ok()
        })
        .filter(|r| r["kind"] == kind)
        .collect()
}

fn delivery_validators(memory: &Rc<Memory>) -> Vec<Box<dyn Validator>> {
    let forge: Rc<dyn Forge> = memory.clone();
    vec![
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
        Box::new(ChangeRead),
    ]
}

fn options(protocol: &str, name: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: protocol.to_owned(),
        intent: "probe".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}
fn run_custom(
    name: &str,
    yaml: &str,
    harness: &dyn Harness,
) -> (Result<Stop, String>, String, PathBuf, String) {
    let tree = scratch(name);
    let protocol = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("rev-probe-{}-{name}.yaml", std::process::id()));
    fs::write(&protocol, yaml).unwrap();
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut opts = options(&protocol.display().to_string(), name, 6);
    opts.gate = None;
    let opened = Run::open(&tree, opts, harness, &validators);
    let mut run = match opened {
        Ok(run) => run,
        Err(e) => {
            let branch = git(&tree, &["branch", "--show-current"]);
            return (
                Err(format!("open: {e:#}")),
                String::new(),
                tree.join(CASES_DIR).join(name),
                branch,
            );
        }
    };
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|e| format!("run: {e:#}"));
    let dir = run.case.dir.clone();
    drop(run);
    let branch = git(&tree, &["branch", "--show-current"]);
    (stop, String::from_utf8(out).unwrap(), dir, branch)
}

const HEAD: &str = r#"format: protocol/1
protocol:
  id: PID
  revision: 1
artifacts:
  implementation:
    description: the code
"#;

fn proto(id: &str, rest: &str) -> String {
    format!("{}{}", HEAD.replace("PID", id), rest)
}

fn accepted(stop: &Result<Stop, String>) -> bool {
    *stop == Ok(Stop::Completed("accepted".to_owned()))
}

fn choice_records(dir: &Path, kinds: &[&str]) -> usize {
    kinds
        .iter()
        .map(|k| {
            records(dir, k)
                .into_iter()
                .filter(|r| r["producer"] == "takt.choice")
                .count()
        })
        .sum()
}

/// F1 re-check: the round-2 probe protocol is refused at load, naming the protocol.
#[test]
fn probe3_f1_original_refused_at_load() {
    let yaml = proto(
        "tests.said",
        r#"evidence_kinds:
  tests_said:
    description: the model said the tests pass
claims:
  tests.pass:
    description: the tests pass
    true_when:
      evidence:
        kind: tests_said
        subject: implementation
actions:
  implementation.edit:
    description: change the code
    effect: write
  tests.say:
    description: say the tests pass
    effect: read
    may_produce:
      - evidence: tests_said
outcomes:
  accepted:
    description: the tests pass
    requires:
      claim: tests.pass
"#,
    );
    let harness = Script::new(false, vec![step("tests.say", &[], "they pass")]);
    let (stop, text, _dir, _branch) = run_custom("p3-f1", &yaml, &harness);
    eprintln!("P3-F1 {stop:?}\n{text}");
    let err = stop.clone().err().unwrap_or_default();
    assert!(
        err.starts_with("open:") && err.contains("tests.said"),
        "{stop:?}"
    );
}

/// Around under_choices: an `is: unknown` test on a second choice claim. With every choice
/// record present the outcome is false; with only one of them it holds.
#[test]
fn probe3_f1_is_unknown_on_second_choice() {
    let yaml = proto(
        "unknown.gap",
        r#"evidence_kinds:
  said_a:
    description: a
  said_b:
    description: b
claims:
  a.said:
    description: a
    true_when:
      evidence:
        kind: said_a
        subject: implementation
  b.said:
    description: b
    true_when:
      evidence:
        kind: said_b
        subject: implementation
actions:
  implementation.edit:
    description: change the code
    effect: write
  a.say:
    description: say a
    effect: read
    may_produce:
      - evidence: said_a
  b.say:
    description: say b
    effect: read
    may_produce:
      - evidence: said_b
outcomes:
  accepted:
    description: a said and b not
    requires:
      all:
        - claim: a.said
        - claim: b.said
          is: unknown
"#,
    );
    let harness = Script::new(false, vec![step("a.say", &[], "a")]);
    let (stop, text, dir, _branch) = run_custom("p3-unknown", &yaml, &harness);
    let n = choice_records(&dir, &["said_a", "said_b"]);
    eprintln!("P3-UNKNOWN {stop:?} choice records {n}\n{text}");
    assert!(
        !accepted(&stop),
        "accepted on one choice record: {n} choice records\n{text}"
    );
}

/// Around under_choices: an order form. `before {first: reviewed, then: said}` is false with a
/// record of `said` and none of `reviewed`; under `not` it is true. under_choices calls every
/// order form unknown.
#[test]
fn probe3_f1_not_before() {
    let yaml = proto(
        "order.gap",
        r#"evidence_kinds:
  said:
    description: the model said it is reviewed
  reviewed:
    description: a review
actions:
  implementation.edit:
    description: change the code
    effect: write
  review.write:
    description: write a review
    effect: write
    may_produce:
      - evidence: reviewed
  it.say:
    description: say it
    effect: read
    may_produce:
      - evidence: said
outcomes:
  accepted:
    description: said, and no review came first
    requires:
      not:
        before:
          first: reviewed
          then: said
          subject: implementation
"#,
    );
    let harness = Script::new(false, vec![step("it.say", &[], "said")]);
    let (stop, text, dir, _branch) = run_custom("p3-before", &yaml, &harness);
    let n = choice_records(&dir, &["said"]);
    eprintln!("P3-BEFORE {stop:?} choice records {n}\n{text}");
    assert!(
        !accepted(&stop),
        "accepted on one choice record: {n} choice records\n{text}"
    );
}

/// The same through a claim tested `is: false`.
#[test]
fn probe3_f1_claim_false_before() {
    let yaml = proto(
        "order.gap2",
        r#"evidence_kinds:
  said:
    description: the model said it is reviewed
  reviewed:
    description: a review
claims:
  review.first:
    description: a review came before the saying
    true_when:
      before:
        first: reviewed
        then: said
        subject: implementation
actions:
  implementation.edit:
    description: change the code
    effect: write
  review.write:
    description: write a review
    effect: write
    may_produce:
      - evidence: reviewed
  it.say:
    description: say it
    effect: read
    may_produce:
      - evidence: said
outcomes:
  accepted:
    description: said, and no review came first
    requires:
      claim: review.first
      is: false
"#,
    );
    let harness = Script::new(false, vec![step("it.say", &[], "said")]);
    let (stop, text, dir, _branch) = run_custom("p3-before2", &yaml, &harness);
    let n = choice_records(&dir, &["said"]);
    eprintln!("P3-BEFORE2 {stop:?} choice records {n}\n{text}");
    assert!(
        !accepted(&stop),
        "accepted on one choice record: {n} choice records\n{text}"
    );
}

fn refused_at_load(name: &str, yaml: &str, script: Vec<Step>) -> String {
    let harness = Script::new(false, script);
    let (stop, text, dir, _b) = run_custom(name, yaml, &harness);
    eprintln!("{name} {stop:?}\n{text}");
    assert!(!accepted(&stop), "{name} accepted\n{text}");
    let _ = dir;
    stop.err().unwrap_or_default()
}

/// Expected refused: an obligation discharged by a choice claim alone.
#[test]
fn probe3_f1_obligation_refused() {
    let yaml = proto(
        "obl.choice",
        r#"evidence_kinds:
  said_a:
    description: a
  gate_result:
    description: the gate
claims:
  a.said:
    description: a
    true_when:
      evidence:
        kind: said_a
        subject: implementation
  gate.green:
    description: gate
    true_when:
      evidence:
        kind: gate_result
        result: pass
        subject: implementation
obligations:
  o.said:
    description: a said
    discharged_when:
      claim: a.said
actions:
  implementation.edit:
    description: change the code
    effect: write
  a.say:
    description: say a
    effect: read
    may_produce:
      - evidence: said_a
  gate.run:
    description: run the gate
    effect: read
    may_produce:
      - evidence: gate_result
outcomes:
  accepted:
    description: gate
    requires:
      claim: gate.green
"#,
    );
    let err = refused_at_load("p3-obl", &yaml, vec![step("a.say", &[], "a")]);
    assert!(err.starts_with("open:") && err.contains("o.said"), "{err}");
}

/// Expected refused: `any` of a choice claim and a validator claim; `not` over a validator.
#[test]
fn probe3_f1_any_and_not_refused() {
    let base = r#"evidence_kinds:
  said_a:
    description: a
  gate_result:
    description: the gate
claims:
  a.said:
    description: a
    true_when:
      evidence:
        kind: said_a
        subject: implementation
  gate.green:
    description: gate
    true_when:
      evidence:
        kind: gate_result
        result: pass
        subject: implementation
actions:
  implementation.edit:
    description: change the code
    effect: write
  a.say:
    description: say a
    effect: read
    may_produce:
      - evidence: said_a
  gate.run:
    description: run the gate
    effect: read
    may_produce:
      - evidence: gate_result
outcomes:
  accepted:
    description: x
    requires:
"#;
    let anyp = format!("{base}      any:\n        - claim: a.said\n        - claim: gate.green\n");
    let err = refused_at_load(
        "p3-any",
        &proto("any.choice", &anyp),
        vec![step("a.say", &[], "a")],
    );
    assert!(
        err.starts_with("open:") && err.contains("accepted"),
        "{err}"
    );
    let notp = format!(
        "{base}      all:\n        - claim: a.said\n        - not:\n            claim: gate.green\n            is: true\n"
    );
    let harness = Script::new(false, vec![step("a.say", &[], "a")]);
    let (stop, text, _d, _b) = run_custom("p3-not", &proto("not.choice", &notp), &harness);
    eprintln!("p3-not {stop:?}\n{text}");
    assert!(!accepted(&stop), "{text}");
}

/// Expected refused: a choice kind gated behind another choice.
#[test]
fn probe3_f1_gated_choice_refused() {
    let yaml = proto(
        "gated.choice",
        r#"evidence_kinds:
  said_a:
    description: a
  said_b:
    description: b
claims:
  a.said:
    description: a
    true_when:
      evidence:
        kind: said_a
        subject: implementation
  b.said:
    description: b
    true_when:
      evidence:
        kind: said_b
        subject: implementation
actions:
  implementation.edit:
    description: change the code
    effect: write
  a.say:
    description: say a
    effect: read
    may_produce:
      - evidence: said_a
  b.say:
    description: say b
    effect: read
    precondition:
      claim: a.said
    may_produce:
      - evidence: said_b
outcomes:
  accepted:
    description: b said
    requires:
      claim: b.said
"#,
    );
    let err = refused_at_load(
        "p3-gated",
        &yaml,
        vec![step("a.say", &[], "a"), step("b.say", &[], "b")],
    );
    assert!(
        err.starts_with("open:") && err.contains("accepted"),
        "{err}"
    );
}

const SHADOW: &str = r#"evidence_kinds:
  change_record:
    description: shadow
claims:
  change.made:
    description: made
    true_when:
      evidence:
        kind: change_record
        subject: implementation
actions:
  implementation.edit:
    description: change the code
    effect: write
  change.ack:
    description: ack
    effect: EFFECT
CAP    may_produce:
      - evidence: change_record
outcomes:
  accepted:
    description: made
    requires:
      claim: change.made
"#;

/// F3: change.ack (read) producing change_record is refused at load.
#[test]
fn probe3_f3_read_shadow_refused() {
    let yaml = proto(
        "shadow.read",
        &SHADOW.replace("EFFECT", "read").replace("CAP", ""),
    );
    let err = refused_at_load(
        "p3-shadow-read",
        &yaml,
        vec![
            step("change.ack", &[], "ack"),
            step("change.ack", &[], "ack"),
        ],
    );
    assert!(
        err.starts_with("open:") && err.contains("shadow.read") && err.contains("change.ack"),
        "{err}"
    );
}

/// F3 narrowing: change.ack with effect write, no capability, producing change_record. It
/// loads; the harness is never offered it; taking it anyway writes no change_record.
#[test]
fn probe3_f3_write_shadow_no_capability() {
    for (name, cap) in [
        ("p3-shadow-write", ""),
        (
            "p3-shadow-write-cap",
            "    requires:\n      - capability: repository.merge\n",
        ),
    ] {
        let yaml = proto(
            "shadow.write",
            &SHADOW.replace("EFFECT", "write").replace("CAP", cap),
        );
        for keeps in [false, true] {
            let harness = Script::new(
                keeps,
                vec![
                    step("change.ack", &[], "ack"),
                    step("change.ack", &[], "ack"),
                ],
            );
            let (stop, text, dir, _b) = run_custom(&format!("{name}-{keeps}"), &yaml, &harness);
            let offers = harness.offers.borrow().clone();
            let recs = records(&dir, "change_record");
            eprintln!("{name} keeps {keeps}: {stop:?} offers {offers:?} records {recs:?}\n{text}");
            assert!(recs.is_empty(), "{name}: {recs:?}");
            assert!(!accepted(&stop), "{name}: {text}");
            assert!(
                offers.iter().all(|o| !o.contains(&"change.ack".to_owned())),
                "{name}: {offers:?}"
            );
        }
    }
}

/// F2 variant: main gains the same fix and an unrelated change. change.read's tree differs from
/// both bases; does a pull request open whose merge into main changes nothing?
#[test]
fn probe3_f2_main_has_fix_and_more() {
    let tree = scratch("p3-base-more");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let validators = delivery_validators(&memory);
    let name = "p3-base-more";
    let harness = Script::new(
        false,
        vec![step(
            "implementation.edit",
            &[("README.md", "a story\nfix\n")],
            "fix",
        )],
    );
    {
        let mut run =
            Run::open(&tree, options(PROTOCOL, name, 3), &harness, &validators).expect("opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        eprintln!("phase1 {stop:?}\n{}", String::from_utf8(out).unwrap());
    }
    let wt = tree.with_extension("wt");
    let _ = fs::remove_dir_all(&wt);
    git(
        &tree,
        &["worktree", "add", "-q", wt.to_str().unwrap(), "main"],
    );
    fs::write(wt.join("README.md"), "a story\nfix\n").unwrap();
    fs::write(wt.join("OTHER.md"), "unrelated\n").unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "same fix and more on main"]);
    git(
        &tree,
        &["worktree", "remove", "--force", wt.to_str().unwrap()],
    );
    let h2 = Script::new(
        false,
        vec![
            step("change.handover", &[], "done"),
            step("change.handover", &[], "done"),
        ],
    );
    let mut run =
        Run::resume(&tree, options(PROTOCOL, name, 12), name, &h2, &validators).expect("resumes");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).unwrap();
    let rev = run.case.revision("implementation").unwrap();
    let merged = git(&tree, &["merge-tree", "--write-tree", "main", &rev]);
    let merged_tree = merged.lines().next().unwrap_or_default().to_owned();
    let main_tree = git(&tree, &["rev-parse", "main^{tree}"]);
    let net = git(
        &tree,
        &[
            "diff",
            "--stat",
            &main_tree,
            &merged_tree,
            "--",
            ".",
            ":(exclude).engineering",
        ],
    );
    eprintln!(
        "P3-F2 stop {stop:?} opened {} merge-into-main changes outside .engineering: {net:?}\n{text}",
        memory.opened.borrow().len()
    );
    assert!(
        memory.opened.borrow().is_empty() || !net.is_empty(),
        "a pull request opened whose merge into main changes nothing outside .engineering"
    );
}

/// The improvement protocols load through Run::open (the shape check).
#[test]
fn probe3_improvement_protocols_open() {
    for (protocol, inputs) in [
        (
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../protocols/self-improvement/1.yaml"
            ),
            vec![("surface", "non_judge")],
        ),
        (
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../protocols/loop-improvement/1.yaml"
            ),
            vec![],
        ),
    ] {
        let name = format!("p3-imp-{}", protocol.len());
        let tree = scratch(&name);
        let validators: Vec<Box<dyn Validator>> = Vec::new();
        let mut opts = options(protocol, &name, 1);
        opts.gate = None;
        opts.inputs = inputs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let harness = Script::new(false, vec![]);
        let opened = Run::open(&tree, opts, &harness, &validators);
        eprintln!("{protocol}: {:?}", opened.as_ref().err());
        let err = opened.err().map(|e| format!("{e:#}")).unwrap_or_default();
        assert!(
            !err.contains("choice") && !err.contains("built-in validator"),
            "{protocol}: {err}"
        );
    }
}
