//! story:replay-offers-granted-actions: `takt replay` rebuilds a step's offer with the grants the
//! step had. A step offered an action only through a grant in the case's `authority.yaml`
//! replays to the prompt it was given, byte for byte, and the replay runs (what `takt replay`
//! needs to exit 0). Replay grants nothing the run was not granted: with the grant removed, or
//! given at another revision of its subject, the offer is rebuilt without the action and the
//! prompt differs from the recorded one, which `takt replay` refuses (exit 3); and a step the
//! run took before the grant was given, at the same revisions, is rebuilt without the action,
//! as the run offered it. No live `claude` runs: the harness program is a script.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{AUTHORITY_FILE, Authority, CASES_DIR, grant};
use takt::harness::{ClaudeCode, Harness as _};
use takt::replay::{
    AS_RECORDED, Start, rebuilt_prompt_line, recorded_prompt_line, replay_once, starting_state,
};
use takt::run::{Offer, judgement_offer};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// A protocol whose one gated action is the harness's: `release.ship` needs a grant of
/// `release`, bound to the `implementation` revision the tests passed at; `notes.write` is a
/// judgement step that needs none, so a step is taken at the gate before the grant too.
const PROTOCOL: &str = "format: protocol/1

protocol:
  id: ship.granted
  revision: 1
  description: >-
    Run the tests, write notes, and ship the release once the tests pass and release is granted.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision
  note_record:
    description: the notes written about an implementation revision
  ship_record:
    description: the release shipped from an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation
  noted:
    description: the notes are written for the current implementation revision
    true_when:
      evidence:
        kind: note_record
        result: written
        subject: implementation
  shipped:
    description: the release shipped from the current implementation revision
    true_when:
      evidence:
        kind: ship_record
        result: shipped
        subject: implementation

actions:
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result
  notes.write:
    description: write the notes
    effect: write
    may_produce:
      - evidence: note_record
  release.ship:
    description: ship the release
    effect: write
    precondition:
      claim: tests.pass
    requires:
      - capability: release
    may_produce:
      - evidence: ship_record

outcomes:
  accepted:
    description: the release shipped
    requires:
      claim: shipped
";

/// The gated action, the run's approval gate.
const GATE: &str = "release.ship";
/// The capability it requires.
const CAPABILITY: &str = "release";
/// The case's id.
const CASE: &str = "ship-granted";

/// A scratch directory under this crate's build directory, removed when dropped.
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
            let message = format!("remove the scratch directory {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

/// An empty scratch directory named for this process and `name`, its path resolved.
fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("replay-after-grant-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(fs::canonicalize(&dir).expect("the scratch directory resolves"))
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A committed repository on `main` with a README.
fn repository(name: &str) -> Scratch {
    let tree = directory(name);
    fs::write(tree.join("README.md"), "a release\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

/// A fake `claude` in `dir` that changes nothing and reports `action`. Each call counts itself
/// in `dir/calls` and keeps the prompt it was given (the argument after `-p`) as
/// `dir/prompt-<n>`.
fn fake_claude(dir: &Path, action: &str) -> ClaudeCode {
    let script = r#"#!/bin/sh
d='@DIR@'
n=$(cat "$d/calls" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$d/calls"
printf '%s' "$2" > "$d/prompt-$n"
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"@ACTION@\", \"summary\": \"took it\"}"}'
"#
    .replace("@DIR@", &dir.display().to_string())
    .replace("@ACTION@", action);
    let program = dir.join("claude");
    fs::write(&program, script).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    }
}

/// The run's options: the protocol at `protocol`, the gate [`GATE`].
fn options(protocol: &Path) -> RunOptions {
    RunOptions {
        protocol: protocol.display().to_string(),
        intent: "ship the release".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some(CASE.to_owned()),
        gate: Some(GATE.to_owned()),
    }
}

/// Runs the case on `tree`, opened (`resume` false) or resumed, with `harness` and the tests
/// validator, its transcripts kept under `transcripts`; how it stopped, and its output.
fn run_case(
    tree: &Path,
    protocol: &Path,
    harness: &ClaudeCode,
    transcripts: &Path,
    resume: bool,
) -> (Stop, String) {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let mut run = if resume {
        Run::resume(tree, options(protocol), CASE, harness, &validators).expect("the case resumes")
    } else {
        Run::open(tree, options(protocol), harness, &validators).expect("the case opens")
    };
    run.transcripts_in(transcripts.to_path_buf())
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let stop = stop.unwrap_or_else(|error| panic!("the run runs: {error:#}\n{text}"));
    (stop, text)
}

/// Each harness step the case's record holds: its run's ordinal in the case, from 1, its
/// number, and the action it took.
fn harness_steps(case_dir: &Path) -> Vec<(u64, u64, String)> {
    let text = fs::read_to_string(case_dir.join("events.jsonl")).expect("the record");
    let mut runs = 0;
    let mut steps = Vec::new();
    for line in text.lines() {
        let value: serde_json::Value = serde_json::from_str(line).expect("a record line");
        if value["event"] == "opened" {
            runs += 1;
        }
        if value["event"] == "step" && value["by"] == "harness" {
            steps.push((
                runs,
                value["step"].as_u64().expect("a step number"),
                value["action"].as_str().unwrap_or_default().to_owned(),
            ));
        }
    }
    steps
}

/// A recorded case, granted: a first run tests the implementation, takes one judgement step at
/// the gate (`notes.write`, with `release.ship` approval-required) and stops; `release` is
/// granted; a second run takes `release.ship`, offered only through the grant.
struct Granted {
    tree: Scratch,
    transcripts: Scratch,
    scripts: Scratch,
    _protocol: Scratch,
    case_dir: PathBuf,
    /// The step taken before the grant, `(run, step)`.
    before: (u64, u64),
    /// The step taken through the grant, `(run, step)`.
    after: (u64, u64),
}

fn granted(name: &str) -> Granted {
    let tree = repository(&format!("{name}-tree"));
    let transcripts = directory(&format!("{name}-transcripts"));
    let protocol_dir = directory(&format!("{name}-protocol"));
    let protocol = protocol_dir.join("ship-granted.yaml");
    fs::write(&protocol, PROTOCOL).expect("a protocol");
    let scripts = directory(&format!("{name}-scripts"));
    let noting = directory(&format!("{name}-noting"));
    let shipping = directory(&format!("{name}-shipping"));

    let (stop, text) = run_case(
        &tree,
        &protocol,
        &fake_claude(&noting, "notes.write"),
        &transcripts,
        false,
    );
    assert_eq!(stop, Stop::NoProgress, "{text}");
    let case_dir = tree.join(CASES_DIR).join(CASE);
    grant(&tree, CASE, CAPABILITY, "human:tester").expect("release is granted at the gate");
    let (stop, text) = run_case(
        &tree,
        &protocol,
        &fake_claude(&shipping, GATE),
        &transcripts,
        true,
    );
    assert_eq!(stop, Stop::NoProgress, "{text}");

    let steps = harness_steps(&case_dir);
    let [(r1, s1, a1), (r2, s2, a2)] = steps.as_slice() else {
        panic!("two harness steps, one in each run: {steps:?}");
    };
    assert_eq!((*r1, a1.as_str()), (1, "notes.write"), "{steps:?}");
    assert_eq!((*r2, a2.as_str()), (2, GATE), "{steps:?}");
    drop(noting);
    drop(shipping);
    Granted {
        tree,
        transcripts,
        scripts,
        _protocol: protocol_dir,
        case_dir,
        before: (*r1, *s1),
        after: (*r2, *s2),
    }
}

impl Granted {
    /// The state the step `(run, step)` started from, the prompt line its transcript recorded,
    /// and the one replay rebuilds for it.
    fn prompts(&self, (run, step): (u64, u64)) -> (Start, Vec<u8>, Vec<u8>) {
        let start = starting_state(&self.case_dir, step, Some(run)).expect("the starting state");
        let (_, recorded) = recorded_prompt_line(&start, &self.transcripts, &self.tree)
            .expect("the transcripts read")
            .expect("the step's transcript is kept");
        let probe = fake_claude(&self.scripts, GATE);
        let rebuilt =
            rebuilt_prompt_line(&probe, &start, Some(GATE), &self.tree).expect("a prompt rebuilt");
        (start, recorded, rebuilt)
    }
}

/// The offer replay rebuilds for the step `start` describes, under the gate.
fn offer(start: &Start) -> Offer {
    judgement_offer(
        &start.protocol,
        &start.inputs,
        &start.decision,
        ClaudeCode::default().keeps_state(),
        Some(GATE),
    )
    .expect("an offer")
}

/// What the run recorded for its decision `file`: its line's frontier.
fn recorded_frontier(case_dir: &Path, file: &str) -> String {
    let text = fs::read_to_string(case_dir.join("events.jsonl")).expect("the record");
    text.lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| value["event"] == "decision" && value["file"] == file)
        .find_map(|value| value["frontier"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("the record names the decision {file}"))
}

/// Acceptance 1: the step taken through the grant replays to the prompt it was given, byte for
/// byte, with the gated action offered, and the replay runs with its prompt as recorded: what
/// `takt replay` exits 0 on.
#[test]
fn a_step_taken_through_a_grant_replays_as_recorded() {
    let case = granted("as-recorded");
    let (start, recorded, rebuilt) = case.prompts(case.after);
    assert!(
        recorded_frontier(&case.case_dir, &start.decision_file)
            .contains(&format!("{GATE} (admissible)")),
        "the run was offered {GATE} through the grant"
    );
    assert_eq!(
        offer(&start).offered,
        vec!["notes.write".to_owned(), GATE.to_owned()],
        "the offer rebuilt with the grant the step had"
    );
    assert!(
        recorded == rebuilt,
        "the prompt rebuilt for the step taken through the grant is the recorded one\nrecorded: {}\nrebuilt:  {}",
        String::from_utf8_lossy(&recorded),
        String::from_utf8_lossy(&rebuilt)
    );

    let replays = directory("as-recorded-replays");
    let harness = fake_claude(&case.scripts, GATE);
    let replayed = replay_once(
        &start,
        &case.tree,
        &harness,
        Some(GATE),
        &replays.join("replay-1"),
        Some(&recorded),
    )
    .expect("the step replays");
    assert_eq!(replayed.prompt, AS_RECORDED);
    assert_eq!(replayed.result, Ok(GATE.to_owned()));
    let record: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(replays.join("replay-1").join("replay.json")).expect("replay.json"),
    )
    .expect("a replay record");
    assert_eq!(
        record["offered"],
        serde_json::json!(["notes.write", GATE]),
        "{record:#}"
    );
    assert_eq!(record["approvals"], serde_json::json!([]), "{record:#}");
}

/// Acceptance 2: a grant the run did not have offers nothing. With the grant removed from
/// `authority.yaml`, or given at another revision of its subject, the step taken through it is
/// rebuilt without the action, which is approval-required again, and its prompt differs from
/// the recorded one: `takt replay` refuses it (exit 3) and replays nothing.
#[test]
fn a_grant_the_run_did_not_have_offers_nothing() {
    let case = granted("not-had");
    let authority_file = case.case_dir.join(AUTHORITY_FILE);
    let on_file = fs::read_to_string(&authority_file).expect("the grant is on file");

    let mut moved = Authority::read(&case.case_dir)
        .expect("the authority reads")
        .expect("the authority is on file");
    for decision in &mut moved.decisions {
        decision.revision = Some("0123456789abcdef0123456789abcdef01234567".to_owned());
    }
    let variants: [(&str, Option<Authority>); 2] = [
        ("given at another subject revision", Some(moved)),
        ("removed", None),
    ];
    for (what, authority) in variants {
        match &authority {
            Some(authority) => authority.write(&case.case_dir).expect("authority written"),
            None => fs::remove_file(&authority_file).expect("the grant removed"),
        }
        let (start, recorded, rebuilt) = case.prompts(case.after);
        let offer = offer(&start);
        assert!(
            !offer.offered.contains(&GATE.to_owned()),
            "the grant {what}: {GATE} is not offered: {offer:?}"
        );
        assert_eq!(
            offer.approvals,
            vec![GATE.to_owned()],
            "the grant {what}: {GATE} needs an approval again"
        );
        assert!(
            recorded != rebuilt,
            "the grant {what}: the prompt rebuilt differs from the recorded one, and replay refuses it"
        );
    }
    fs::write(&authority_file, on_file).expect("the grant restored");
    let (_, recorded, rebuilt) = case.prompts(case.after);
    assert!(recorded == rebuilt, "the grant restored replays again");
}

/// Acceptance 2, the grant given at another point of the case than the step: the step the run
/// took at the gate before the grant was given, at the same case and subject revisions, is
/// rebuilt without the action, which needed an approval there, as the run offered it. The grant
/// on file now applies at those revisions; replay offers what the run had, and the run had no
/// grant then.
#[test]
fn a_step_taken_before_the_grant_replays_without_it() {
    let case = granted("before");
    let (before, recorded, rebuilt) = case.prompts(case.before);
    let (after, _, _) = case.prompts(case.after);
    let at = |start: &Start| -> serde_json::Value {
        let doc: serde_json::Value =
            serde_json::from_str(&start.decision.json).expect("a decision document");
        doc["explanation"]["computed_from"]["case"].clone()
    };
    assert_eq!(
        (
            &at(&before)["revision"],
            &at(&before)["artifacts"]["implementation"]["revision"]
        ),
        (
            &at(&after)["revision"],
            &at(&after)["artifacts"]["implementation"]["revision"]
        ),
        "both steps were decided at the same case and implementation revisions"
    );
    assert!(
        recorded_frontier(&case.case_dir, &before.decision_file)
            .contains(&format!("{GATE} (approval-required)")),
        "the run had no grant before it was given"
    );
    let offer = offer(&before);
    assert_eq!(offer.offered, vec!["notes.write".to_owned()], "{offer:?}");
    assert_eq!(offer.approvals, vec![GATE.to_owned()], "{offer:?}");
    assert!(
        recorded == rebuilt,
        "the step before the grant is rebuilt as the run offered it\nrecorded: {}\nrebuilt:  {}",
        String::from_utf8_lossy(&recorded),
        String::from_utf8_lossy(&rebuilt)
    );
}
