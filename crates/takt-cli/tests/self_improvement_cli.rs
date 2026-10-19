//! `takt run` on `self.improvement/1` (story:self-improvement-protocol, part 1): a turn opens
//! with `--intent` and the inputs `finding=<story id>` and `candidate=<full commit id>`, taken out
//! of the inputs as `loop_baseline` is; `story.yaml` names the finding, the `finding` artifact is
//! the case's base commit, the baseline is the latest promoted revision of the promotion store,
//! and every validator of the turn is declared independent in `case.yaml` by the kernel. The
//! turn's validators then drive the case on their own records: `finding.file`, then
//! `scenario.reproduce` at the baseline, then `scenario.verify` at the candidate, each through
//! `scripts/check-finding` as the baseline holds it, run on a checkout of the baseline. A finding
//! that never reproduced at the baseline ends `unreproducible`; a candidate that still reproduces
//! it ends `declined`. Part 2: the judge surface is computed at open; the finding is read on the
//! mainline, the local `main` under `--forge none`; the merge and the promotion are observed by
//! `candidate.merge` and `binary.promote` once granted, so a verified candidate merged into
//! `main` and promoted ends `accepted` on the validators' records alone.
//!
//! Each revision is a scratch takt, a cargo workspace with no dependency whose `takt probe`
//! shows the defect or not, which `check-finding` builds itself, confined, in seconds; the
//! baseline holds this tree's own `scripts/check-finding`, so the tests need nothing of this
//! repository's history. Every path the run reads or writes outside the repository is this
//! test's own: the promotion store under `XDG_STATE_HOME`, `check-finding`'s cache under
//! `XDG_CACHE_HOME`; the scenario repository is a local checkout.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The self-improvement protocol of this repository.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/self-improvement/1.yaml"
);

/// The finding scripts of this repository, committed into the scratch repository's base.
const SCRIPTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");

/// The finding the turns are opened for.
const FINDING: &str = "story:probe-shows-the-defect";

/// The output the defect prints, which the finding's predicate names.
const DEFECT: &str = "the defect is still here";

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
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

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// One turn's world: the repository, its baseline and candidate, the scenario checkout, and the
/// state and cache directories the run is given.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    baseline: String,
    candidate: String,
    scenarios: PathBuf,
    state: PathBuf,
    cache: PathBuf,
}

/// A binary planted in `check-finding`'s cache in place of a build: a script that shows the
/// defect, or that does not.
fn build(defect: bool) -> String {
    if defect {
        format!("#!/bin/sh\necho '{DEFECT}'\nexit 1\n")
    } else {
        "#!/bin/sh\necho 'all is well here'\nexit 0\n".to_owned()
    }
}

/// The `main.rs` of a scratch takt revision: `probe` prints the defect and exits 1 when
/// `defect`, else prints that all is well and exits 0. `check-finding` builds it with cargo,
/// confined, in seconds: the workspace has no dependency.
fn takt_main(defect: bool) -> String {
    if defect {
        format!("fn main() {{\n    println!(\"{DEFECT}\");\n    std::process::exit(1);\n}}\n")
    } else {
        "fn main() {\n    println!(\"all is well here\");\n}\n".to_owned()
    }
}

/// The scratch takt's main source, relative to its repository.
const TAKT_MAIN: &str = "crates/takt-cli/src/main.rs";

/// The finding's file in the store.
const FINDING_FILE: &str = ".engineering/planning/story/probe-shows-the-defect.md";

/// The reviewer's rigged `scripts/check-finding`: `not_reproduced` for the revision `HEAD` is
/// at, `reproduced` for any other, whatever the predicate shows.
const RIGGED_FOR_HEAD: &str = "#!/bin/sh\nrev=\"\"\nfor a in \"$@\"; do rev=$a; done\nif [ \"$rev\" = \"$(git rev-parse HEAD)\" ]; then echo not_reproduced; else echo reproduced; fi\n";

/// A rigged `scripts/check-finding` that says `not_reproduced` for every revision.
const RIGGED_ALWAYS: &str = "#!/bin/sh\necho not_reproduced\n";

/// Where the operator's `HEAD` is when the turn opens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Head {
    /// `main`, the baseline.
    Main,
    /// The candidate's branch.
    Fix,
    /// A branch off the baseline whose one commit writes `(path, text)`.
    Side(&'static str, &'static str),
}

/// The promotion store's baseline record: one the operator stated for the binary with this
/// sha256 (the test binary's when none is given), or one that names the baseline's revision as
/// the binary's own, with no digest or with the digest given.
#[derive(Clone, PartialEq, Eq)]
enum Promoted {
    Stated(Option<String>),
    Named,
    NamedWith(String),
}

/// How a world differs from the plain one.
struct Setup {
    at_baseline: bool,
    at_candidate: bool,
    head: Head,
    /// What the candidate's commit writes besides `fix.txt`.
    touches: Option<(&'static str, &'static str)>,
    /// Text appended to the baseline's `scripts/check-finding`, making it another script than
    /// the confined one.
    unconfined: bool,
    /// The baseline's `.gitattributes`, when it has one.
    attributes: Option<&'static str>,
    promoted: Promoted,
    /// The finding filed on `main` after the baseline, in a commit of its own, rather than in
    /// the baseline.
    filed_after: bool,
}

impl Setup {
    fn plain(at_baseline: bool, at_candidate: bool) -> Self {
        Self {
            at_baseline,
            at_candidate,
            head: Head::Main,
            touches: None,
            unconfined: false,
            attributes: None,
            promoted: Promoted::Stated(None),
            filed_after: false,
        }
    }
}

/// The baseline's judge allow-list: the candidate's own paths (`fix.txt`, the scratch takt's
/// sources) are no judge surface; every other path is one.
const JUDGE_ALLOW: &str = "paths:\n  - fix.txt\n  - crates/**\n";

/// The finding's text in the store.
fn finding_text() -> String {
    format!(
        "# The probe shows the defect\n\nThe probe prints the defect.\n\n```expected_before\nscenario: 1\ncommand: \"$TAKT\" probe\nexit: 1\noutput: {DEFECT}\nsource: docs/runs/trial-1.md\n```\n"
    )
}

/// A scorecard of `revision` as `takt scorecard --save` writes it: scenario 1 run five times,
/// each accepted with its held-out check passed, made with the binary of `binary`.
fn scorecard(revision: &str, binary: &str) -> String {
    let metrics: Vec<String> = takt::scorecard::METRICS
        .iter()
        .map(|(metric, version)| format!("\"{metric}\":{version}"))
        .collect();
    let rows: Vec<String> = (1..=5)
        .map(|n| {
            format!(
                "{{\"case\":\"scenario-1-trial-{n}\",\"scenario\":\"1\",\"outcome\":\"accepted\",\"stop\":\"accepted\",\"held_out\":{{\"passed\":true,\"result\":\"PASS\"}},\"steps\":4,\"tokens\":1000,\"refusals\":0,\"no_progress\":0,\"errors\":0,\"wall_ms\":1000,\"saturated\":false,\"binary\":null,\"opened_binary\":\"{binary}\"}}"
            )
        })
        .collect();
    format!(
        "{{\"format\":\"{}\",\"revision\":\"{revision}\",\"named_by\":\"build\",\"metrics\":{{{}}},\"rule\":{{\"trials\":5,\"margin\":1}},\"rows\":[{}],\"no_record\":[],\"unstated\":{{}}}}",
        takt::scorecard::FORMAT,
        metrics.join(","),
        rows.join(",")
    )
}

/// [`world_with`] with HEAD on `main`, a candidate that touches `fix.txt` alone and a baseline
/// record the operator stated for the test binary.
fn world(name: &str, at_baseline: bool, at_candidate: bool) -> World {
    world_with(name, &Setup::plain(at_baseline, at_candidate))
}

/// A repository whose base holds the finding scripts, the finding and the run record it cites,
/// and a candidate commit on a side branch; the scenario repository the predicate runs on; a
/// promotion store whose baseline is the base, stated by the operator for the test binary (its
/// sha256), as a turn needs the binary that judges to be the promoted one; and
/// `check-finding`'s cache holding a build of each revision, the baseline's showing the defect
/// when `at_baseline` and the candidate's when `at_candidate`. `setup` says where `HEAD` is,
/// what else the candidate writes, and how the baseline differs.
fn world_with(name: &str, setup: &Setup) -> World {
    let Setup {
        at_baseline,
        at_candidate,
        ..
    } = *setup;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "self-improvement-cli-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let repo = dir.join("takt");
    for script in ["check-finding", "file-finding"] {
        let mut text = fs::read_to_string(Path::new(SCRIPTS).join(script)).expect("the script");
        if setup.unconfined && script == "check-finding" {
            text.push_str("# an older check-finding than the confined one\n");
        }
        executable(&repo.join("scripts").join(script), &text);
    }
    write(
        &repo.join("docs/runs/trial-1.md"),
        "# Trial 1\n\nThe probe showed the defect.\n",
    );
    write(
        &repo.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n",
    );
    write(
        &repo.join("crates/takt-cli/Cargo.toml"),
        "[package]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"takt\"\npath = \"src/main.rs\"\n",
    );
    write(
        &repo.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\n",
    );
    write(&repo.join(TAKT_MAIN), &takt_main(at_baseline));
    if let Some(attributes) = setup.attributes {
        write(&repo.join(".gitattributes"), attributes);
    }
    write(&repo.join(".engineering/judge-allow.yaml"), JUDGE_ALLOW);
    if !setup.filed_after {
        write(&repo.join(FINDING_FILE), &finding_text());
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "base"]);
    let baseline = git(&repo, &["rev-parse", "HEAD"]);
    if setup.filed_after {
        write(&repo.join(FINDING_FILE), &finding_text());
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "file the finding"]);
    }
    git(&repo, &["checkout", "-q", "-b", "fix", &baseline]);
    write(&repo.join("fix.txt"), "the fix\n");
    write(&repo.join(TAKT_MAIN), &takt_main(at_candidate));
    if let Some((path, text)) = setup.touches {
        executable(&repo.join(path), text);
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "fix"]);
    let candidate = git(&repo, &["rev-parse", "HEAD"]);
    match setup.head {
        Head::Fix => {}
        Head::Main => {
            git(&repo, &["checkout", "-q", "main"]);
        }
        Head::Side(path, text) => {
            git(&repo, &["checkout", "-q", "-b", "side", &baseline]);
            executable(&repo.join(path), text);
            git(&repo, &["add", "-A"]);
            git(&repo, &["commit", "-q", "-m", "side"]);
        }
    }

    let scenarios = dir.join("scenarios");
    write(&scenarios.join("README.md"), "the scenario\n");
    git(&scenarios, &["init", "-q", "-b", "main"]);
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "seed"]);
    let seed = git(&scenarios, &["rev-parse", "HEAD"]);
    write(
        &scenarios.join(".engineering/scenarios.conf"),
        &format!("[scenario \"1\"]\n\tstory = {FINDING}\n\tseed = {seed}\n"),
    );
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "declare"]);

    let state = dir.join("state");
    let store = state.join("takt/promotions");
    write(
        &store.join(format!("{baseline}.scorecard.json")),
        &scorecard(&baseline, &baseline),
    );
    let digest = sha256(&store.join(format!("{baseline}.scorecard.json")));
    let tree = git(&repo, &["rev-parse", &format!("{baseline}^{{tree}}")]);
    let stated = match &setup.promoted {
        Promoted::Stated(digest) => format!(
            "\"binary\":null,\"binary_sha256\":\"{}\",\"revision_stated_by\":\"operator\",",
            digest
                .clone()
                .unwrap_or_else(|| sha256(Path::new(env!("CARGO_BIN_EXE_takt"))))
        ),
        Promoted::Named => "\"binary\":null,\"binary_sha256\":null,".to_owned(),
        Promoted::NamedWith(digest) => {
            format!("\"binary\":null,\"binary_sha256\":\"{digest}\",")
        }
    };
    write(
        &store.join(format!("{baseline}.json")),
        &format!(
            "{{\"format\":\"takt-promotion/1\",\"revision\":\"{baseline}\",\"tree\":\"{tree}\",\"promoted_at\":\"2026-01-01T00:00:00Z\",\"kind\":\"baseline\",\"scorecard\":\"{baseline}\",\"scorecard_file\":\"/srv/scorecards/scorecard-{baseline}.json\",\"scorecard_sha256\":\"{digest}\",\"against\":null,\"rule\":{{\"trials\":5,\"margin\":1}},\"worse\":0,{stated}\"previous_sha256\":null}}"
        ),
    );

    let cache = dir.join("cache");
    World {
        _scratch: Scratch(dir),
        repo,
        baseline,
        candidate,
        scenarios,
        state,
        cache,
    }
}

fn sha256(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum runs");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .expect("a digest")
        .to_owned()
}

/// `takt run` on the protocol in `world.repo` opening the case `turn-1` with `inputs`; its exit
/// code, standard output and standard error.
fn run(world: &World, inputs: &[String]) -> (i32, String, String) {
    run_with_gate(world, inputs, "repository.merge")
}

/// [`run`], with `gate` the approval the harness may request.
fn run_with_gate(world: &World, inputs: &[String], gate: &str) -> (i32, String, String) {
    run_with_env(world, inputs, gate, &[])
}

/// [`run_with_gate`], with `env` added to the run's environment.
fn run_with_env(
    world: &World,
    inputs: &[String],
    gate: &str,
    env: &[(&str, &Path)],
) -> (i32, String, String) {
    run_as(
        world,
        inputs,
        Some(gate),
        Some("the probe no longer shows the defect"),
        env,
    )
}

/// `takt run` opening the turn `turn-1` on the protocol with `inputs`, `--gate` and `--intent`
/// given only when they are some, and `env` added to the run's environment.
fn run_as(
    world: &World,
    inputs: &[String],
    gate: Option<&str>,
    intent: Option<&str>,
    env: &[(&str, &Path)],
) -> (i32, String, String) {
    let mut args: Vec<String> = Vec::new();
    args.push("run".to_owned());
    if let Some(gate) = gate {
        args.extend(["--gate".to_owned(), gate.to_owned()]);
    }
    if let Some(intent) = intent {
        args.extend(["--intent".to_owned(), intent.to_owned()]);
    }
    args.extend(
        [
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--protocol",
            PROTOCOL,
            "--case-id",
            "turn-1",
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--iterations",
            "6",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
        ]
        .iter()
        .map(|arg| (*arg).to_owned()),
    );
    for input in inputs {
        args.extend(["--input".to_owned(), input.clone()]);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .envs(env.iter().copied())
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The inputs a turn opens with.
fn opening(world: &World) -> Vec<String> {
    vec![
        "surface=non_judge".to_owned(),
        format!("finding={FINDING}"),
        format!("candidate={}", world.candidate),
    ]
}

/// The case's evidence records, each file's text, in the order they were written.
fn records(repo: &Path) -> Vec<String> {
    let dir = repo.join(".engineering/cases/turn-1/evidence");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|entries| {
            entries
                .map(|entry| entry.expect("an entry").path())
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
        .iter()
        .map(|path| fs::read_to_string(path).expect("a record"))
        .collect()
}

/// The string value of `key` in a record's JSON text, as `canon-evidence/1` writes it pretty;
/// empty when it has none.
fn field(record: &str, key: &str) -> String {
    let opening = format!("\"{key}\": \"");
    record
        .split_once(&opening)
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(value, _)| value.to_owned())
        .unwrap_or_default()
}

/// Each record's kind, subject, result and producer.
fn shown(records: &[String]) -> Vec<[String; 4]> {
    records
        .iter()
        .map(|record| ["kind", "subject", "result", "producer"].map(|key| field(record, key)))
        .collect()
}

fn row(kind: &str, subject: &str, result: &str, producer: &str) -> [String; 4] {
    [kind, subject, result, producer].map(str::to_owned)
}

/// The turn opens with the finding, the candidate and the promoted baseline, and the kernel
/// declares the turn's validators independent; a finding that never reproduced at the baseline
/// ends the turn `unreproducible` on the validators' records alone.
#[test]
fn a_turn_whose_finding_does_not_reproduce_at_the_baseline_is_unreproducible() {
    let world = world("unreproducible", false, false);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (unreproducible)"), "{out}\n{err}");
    let case = takt::CaseDir::resume(&world.repo, "turn-1").expect("the case reads");
    let base = git(&world.repo, &["rev-parse", "main"]);
    assert_eq!(case.revision("finding").as_deref(), Some(base.as_str()));
    assert_eq!(
        case.revision("baseline").as_deref(),
        Some(world.baseline.as_str())
    );
    assert_eq!(
        case.revision("candidate").as_deref(),
        Some(world.candidate.as_str())
    );
    assert_eq!(case.story().expect("story.yaml").as_deref(), Some(FINDING));
    assert_eq!(
        case.revision("promotion").as_deref(),
        Some(world.candidate.as_str())
    );
    assert_eq!(
        shown(&records(&world.repo)),
        [
            row("finding_record", "finding", "filed", "takt.finding-file"),
            row(
                "scenario_result",
                "baseline",
                "not_reproduced",
                "takt.scenario-reproduce"
            ),
        ],
        "{out}\n{err}"
    );
}

/// A candidate that still reproduces the finding ends the turn `declined`, every record written
/// by a validator of the turn, none by the harness.
#[test]
fn a_turn_whose_candidate_still_reproduces_the_finding_is_declined() {
    let world = world("declined", true, true);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
    let written = records(&world.repo);
    assert_eq!(
        shown(&written),
        [
            row("finding_record", "finding", "filed", "takt.finding-file"),
            row(
                "scenario_result",
                "baseline",
                "reproduced",
                "takt.scenario-reproduce"
            ),
            row(
                "scenario_result",
                "candidate",
                "reproduced",
                "takt.scenario-verify"
            ),
        ],
        "{out}\n{err}"
    );
    assert_eq!(field(&written[1], "subject_revision"), world.baseline);
    assert_eq!(field(&written[2], "subject_revision"), world.candidate);
}

/// A candidate that no longer reproduces the finding is verified by `scenario.verify`, which the
/// kernel declared independent of the run that delivered the candidate: `scenario.passes_after`
/// holds on the validator's record, and the turn stops at the merge's grant, which part 2 of the
/// story observes.
#[test]
fn a_candidate_verified_by_the_validator_holds_passes_after_and_waits_for_the_merge() {
    let world = world("verified", true, false);
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    assert_eq!(code, 4, "{out}\n{err}");
    let written = records(&world.repo);
    assert_eq!(
        shown(&written)[2],
        row(
            "scenario_result",
            "candidate",
            "not_reproduced",
            "takt.scenario-verify"
        ),
        "{out}\n{err}"
    );
    let case = world.repo.join(".engineering/cases/turn-1");
    let decision = takt::case::latest_decision(&case)
        .expect("the decision reads")
        .expect("a decision");
    assert_eq!(
        decision
            .claims
            .get("scenario.passes_after")
            .map(String::as_str),
        Some("true"),
        "{}",
        decision.json
    );
}

/// The turn's validators are registered with every run, and read their cache and state
/// directories only when a turn's step observes: a run on another protocol with neither `HOME`
/// nor `XDG_CACHE_HOME` and `XDG_STATE_HOME` set is not refused for them.
#[test]
fn a_run_on_another_protocol_needs_no_cache_or_state_directory() {
    let world = world("no-home", true, false);
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--intent",
            "a change",
            "--case-id",
            "change-1",
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--iterations",
            "1",
        ])
        .env_remove("HOME")
        .env_remove("XDG_CACHE_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        world
            .repo
            .join(".engineering/cases/change-1/case.yaml")
            .is_file(),
        "exit {:?}\n{}\n{err}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!err.contains("HOME"), "{err}");
}

/// A turn opened without its finding, without a full commit id for its candidate, or with no
/// promotion record to take its baseline from, is refused before anything is written.
#[test]
fn a_turn_without_its_opening_values_is_refused_before_anything_is_written() {
    let world = world("refused", true, false);
    let candidate = format!("candidate={}", world.candidate);
    let cases: Vec<(Vec<String>, &str)> = vec![
        (
            vec!["surface=non_judge".to_owned(), candidate.clone()],
            "finding",
        ),
        (
            vec![
                "surface=non_judge".to_owned(),
                "finding=story:no-such-finding".to_owned(),
                candidate.clone(),
            ],
            "story:no-such-finding",
        ),
        (
            vec!["surface=non_judge".to_owned(), format!("finding={FINDING}")],
            "candidate",
        ),
        (
            vec![
                "surface=non_judge".to_owned(),
                format!("finding={FINDING}"),
                format!("candidate={}", &world.candidate[..12]),
            ],
            "candidate",
        ),
    ];
    for (inputs, named) in cases {
        let (code, out, err) = run(&world, &inputs);
        assert_eq!(code, 1, "{inputs:?}: {out}\n{err}");
        assert!(err.contains(named), "{inputs:?}: {err}");
        assert!(
            !world.repo.join(".engineering/cases").exists(),
            "{inputs:?}: nothing was written"
        );
        assert_eq!(git(&world.repo, &["branch", "--list", "takt/*"]), "");
    }
    fs::remove_dir_all(world.state.join("takt/promotions")).expect("the store is emptied");
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("promotion"), "{err}");
    assert!(!world.repo.join(".engineering/cases").exists());
}

/// Asserts the turn was refused before anything was written, naming `named`.
fn refused(world: &World, (code, out, err): &(i32, String, String), named: &str) {
    assert_eq!(*code, 1, "{out}\n{err}");
    assert!(err.contains(named), "{named}: {err}");
    assert!(
        !world.repo.join(".engineering/cases").exists(),
        "nothing was written: {out}\n{err}"
    );
    assert_eq!(git(&world.repo, &["branch", "--list", "takt/*"]), "");
}

/// The reviewer's probe `head-on-fix`: the operator's `HEAD` is on the candidate, whose own
/// `scripts/check-finding` is rigged to say `not_reproduced` for the revision `HEAD` is at,
/// while the candidate's build still shows the defect. Read at the case's base commit, that
/// script would earn `scenario.passes_after`; the turn refuses to open instead, naming it, since
/// the candidate changes a script that judges it.
#[test]
fn a_candidate_that_rigs_check_finding_with_head_on_it_is_refused() {
    let world = world_with(
        "head-on-fix",
        &Setup {
            head: Head::Fix,
            touches: Some(("scripts/check-finding", RIGGED_FOR_HEAD)),
            ..Setup::plain(true, true)
        },
    );
    let ran = run_with_gate(&world, &opening(&world), "candidate.merge");
    refused(&world, &ran, "scripts/check-finding");
}

/// A candidate that changes `scripts/file-finding` against the baseline is refused before
/// anything is written, naming the path: what judges the candidate is never the candidate's.
#[test]
fn a_candidate_that_changes_the_finding_scripts_is_refused() {
    let path = "scripts/file-finding";
    let world = world_with(
        "touches-script",
        &Setup {
            touches: Some((path, "#!/bin/sh\necho changed by the candidate\n")),
            ..Setup::plain(true, true)
        },
    );
    let ran = run(&world, &opening(&world));
    refused(&world, &ran, path);
}

/// The finding is read on the mainline, local `main` under `--forge none`, at the commit the
/// case forks from, and its file is no part of the candidate's comparison with the baseline: a
/// candidate that rewrites the finding opens a turn (on a judge surface, the planning store)
/// that judges the finding as `main` holds it, whose defect both builds still show, so the
/// turn ends `declined`.
#[test]
fn a_candidate_that_rewrites_the_finding_is_judged_on_the_mainlines_finding() {
    let world = world_with(
        "touches-finding",
        &Setup {
            touches: Some((FINDING_FILE, "#!/bin/sh\necho changed by the candidate\n")),
            ..Setup::plain(true, true)
        },
    );
    let inputs = [
        "surface=judge".to_owned(),
        format!("finding={FINDING}"),
        format!("candidate={}", world.candidate),
    ];
    let (code, out, err) = run(&world, &inputs);
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
    assert_eq!(
        shown(&records(&world.repo))[0],
        row("finding_record", "finding", "filed", "takt.finding-file"),
        "{out}\n{err}"
    );
}

/// With `HEAD` on a branch whose `scripts/check-finding` is rigged to say `not_reproduced`, the
/// turn's validators still run the baseline's: the defect reproduces at both revisions and the
/// turn ends `declined`, not `unreproducible`.
#[test]
fn the_turn_runs_the_baselines_check_finding_wherever_head_is() {
    let world = world_with(
        "head-on-side",
        &Setup {
            head: Head::Side("scripts/check-finding", RIGGED_ALWAYS),
            ..Setup::plain(true, true)
        },
    );
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
    let written = records(&world.repo);
    assert_eq!(
        shown(&written)[1..],
        [
            row(
                "scenario_result",
                "baseline",
                "reproduced",
                "takt.scenario-reproduce"
            ),
            row(
                "scenario_result",
                "candidate",
                "reproduced",
                "takt.scenario-verify"
            ),
        ],
        "{out}\n{err}"
    );
}

/// The finding is read at the commit the case forks from: one that holds another text of it
/// than the baseline refuses the turn, naming the finding's file.
#[test]
fn a_finding_edited_where_head_is_is_refused() {
    let world = world_with(
        "finding-on-side",
        &Setup {
            head: Head::Side(
                FINDING_FILE,
                "# Another finding\n\n```expected_before\nscenario: 1\ncommand: \"$TAKT\" probe\nexit: 1\noutput: NEVER PRINTED AT ALL\nsource: docs/runs/trial-1.md\n```\n",
            ),
            ..Setup::plain(true, true)
        },
    );
    let ran = run(&world, &opening(&world));
    refused(&world, &ran, FINDING_FILE);
}

/// A promoted baseline whose `scripts/check-finding` is not the confined one (story
/// check-finding-run-step-confined), and that does not descend from the commit that confined
/// it, judges nothing: the turn refuses to open, naming the missing confinement.
#[test]
fn a_baseline_without_the_confined_check_finding_is_refused() {
    let world = world_with(
        "unconfined",
        &Setup {
            unconfined: true,
            ..Setup::plain(true, false)
        },
    );
    let ran = run(&world, &opening(&world));
    refused(&world, &ran, "confine");
}

/// The binary that judges a turn is the promoted one: a store whose latest record names a
/// revision this binary was not built from, or that the operator stated for a binary with
/// another sha256, refuses the turn before anything is written.
#[test]
fn a_turn_is_judged_only_by_the_promoted_binary() {
    let world = world_with(
        "named",
        &Setup {
            promoted: Promoted::Named,
            ..Setup::plain(true, false)
        },
    );
    let ran = run(&world, &opening(&world));
    refused(&world, &ran, "promoted binary");
    let world = world_with(
        "stated-other",
        &Setup {
            promoted: Promoted::Stated(Some("0".repeat(64))),
            ..Setup::plain(true, false)
        },
    );
    let ran = run(&world, &opening(&world));
    refused(&world, &ran, "sha256");
}

/// `takt` run in `world` with `args` and the run's state, cache and scenario checkout; its exit
/// code, standard output and standard error.
fn takt_in(world: &World, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .args([
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
        ])
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A finding whose predicate says the defect is there only for the baseline's build: it names
/// the revision's build directory, which holds the revision.
fn rigged_finding(baseline: &str) -> String {
    format!(
        "# The probe shows the defect\n\nThe probe prints the defect.\n\n```expected_before\nscenario: 1\ncommand: case \"$TAKT\" in */{baseline}/*) \"$TAKT\" probe;; *) exit 0;; esac\nexit: 1\noutput: {DEFECT}\nsource: docs/runs/trial-1.md\n```\n"
    )
}

/// The reviewer's RV-A2: the finding's file marked skip-worktree in the operator's index and
/// rewritten in the working tree to a predicate that holds at the baseline only, which neither
/// the open's clean-tree check nor the per-step check sees. `check-finding` runs on a checkout
/// of the baseline that takt makes, so the predicate it runs is the one filed: both builds show
/// the defect and the turn ends `declined`.
#[test]
fn a_finding_hidden_from_git_status_does_not_decide_the_check() {
    let world = world("skip-worktree", true, true);
    git(
        &world.repo,
        &["update-index", "--skip-worktree", FINDING_FILE],
    );
    fs::write(
        world.repo.join(FINDING_FILE),
        rigged_finding(&world.baseline),
    )
    .expect("the working tree's finding");
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
    assert_eq!(
        shown(&records(&world.repo))[2],
        row(
            "scenario_result",
            "candidate",
            "reproduced",
            "takt.scenario-verify"
        ),
        "{out}\n{err}"
    );
}

/// Plants, as a kept build of `revision` in `check-finding`'s cache, a binary that does not show
/// the defect.
fn plant(world: &World, revision: &str) {
    let built = world.cache.join("takt/check-finding/builds").join(revision);
    executable(&built.join("takt"), &build(false));
    write(&built.join("revision"), &format!("{revision}\n"));
}

/// The reviewer's RV-E: a binary planted as the candidate's kept build before the turn opens is
/// not what `scenario.verify` runs: no build of the turn recorded it, so the candidate is built
/// from its source, which still shows the defect.
#[test]
fn a_build_planted_before_the_turn_is_not_judged() {
    let world = world("planted-before", true, true);
    plant(&world, &world.candidate);
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
}

/// A binary planted over a build the turn made and recorded, between that build and the next
/// check of the same revision, is rebuilt: the turn recorded the build's sha256 in the case when
/// `scenario.reproduce` built it, and `scenario.verify` (the candidate is the baseline here) runs
/// the kept build only while it hashes to that record.
#[test]
fn a_build_planted_between_the_build_and_verify_is_rebuilt() {
    let world = world("planted-between", true, true);
    let inputs = [
        "surface=non_judge".to_owned(),
        format!("finding={FINDING}"),
        format!("candidate={}", world.baseline),
    ];
    let mut opened: Vec<&str> = vec![
        "run",
        "--protocol",
        PROTOCOL,
        "--intent",
        "the probe no longer shows the defect",
        "--case-id",
        "turn-1",
        "--iterations",
        "2",
    ];
    for input in &inputs {
        opened.extend(["--input", input.as_str()]);
    }
    let (code, out, err) = takt_in(&world, &opened);
    assert_eq!(code, 3, "{out}\n{err}");
    assert_eq!(
        shown(&records(&world.repo))[1],
        row(
            "scenario_result",
            "baseline",
            "reproduced",
            "takt.scenario-reproduce"
        ),
        "{out}\n{err}"
    );
    plant(&world, &world.baseline);
    let (code, out, err) = takt_in(
        &world,
        &[
            "run",
            "--case",
            "turn-1",
            "--gate",
            "candidate.merge",
            "--iterations",
            "4",
        ],
    );
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
    assert_eq!(
        shown(&records(&world.repo))[2],
        row(
            "scenario_result",
            "candidate",
            "reproduced",
            "takt.scenario-verify"
        ),
        "{out}\n{err}"
    );
}

/// A promotion record that names its revision and carries the sha256 of the binary it installed
/// is checked against both: a binary that hashes otherwise is refused, naming the sha256, before
/// the revision is compared.
#[test]
fn a_record_with_another_binary_digest_is_refused() {
    let world = world_with(
        "named-digest",
        &Setup {
            promoted: Promoted::NamedWith("0".repeat(64)),
            ..Setup::plain(true, false)
        },
    );
    let ran = run(&world, &opening(&world));
    refused(&world, &ran, "sha256");
}

/// The path of the loose object `id` in `repo`.
fn loose(repo: &Path, id: &str) -> PathBuf {
    repo.join(".git/objects").join(&id[..2]).join(&id[2..])
}

/// The reviewer's RV-L: `builds/<candidate>` in `check-finding`'s cache planted as a link to a
/// directory outside the cache that holds a directory `takt` and a file `revision`. The turn
/// unlinks the link itself and builds the candidate again; nothing is removed through the link,
/// and the turn ends on the candidate's own build.
#[test]
fn a_build_directory_planted_as_a_link_is_unlinked_never_followed() {
    let world = world("planted-link", true, true);
    let builds = world.cache.join("takt/check-finding/builds");
    let outside = world.repo.parent().expect("a parent").join("someone-elses");
    write(&outside.join("takt/precious.txt"), "keep me\n");
    write(&outside.join("revision"), "keep me too\n");
    fs::create_dir_all(&builds).expect("the builds directory");
    std::os::unix::fs::symlink(&outside, builds.join(&world.candidate)).expect("the link");
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    assert!(
        outside.join("takt/precious.txt").is_file() && outside.join("revision").is_file(),
        "the turn removed files outside the cache through a planted link:\n{out}\n{err}"
    );
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
}

/// The reviewer's RV-H: the loose object of the candidate's `main.rs` is overwritten in the
/// case repository's object store with the bytes of an object holding the fixed text, which no
/// per-step guard covers and git does not re-hash on a checkout or a local clone. Every clone
/// that feeds the judgement goes through git's transport, which hashes every object it receives:
/// the check is refused naming the object, and nothing says the defect is gone.
#[test]
fn a_poisoned_loose_object_of_the_candidate_is_refused() {
    let (world, blob) = poisoned("poisoned");
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    refused_poison(&world, &blob, &(code, out, err));
}

/// Asserts that a turn on a poisoned world recorded nothing that says the defect is gone, did
/// not stop for the merge, and named the poisoned object `blob`.
fn refused_poison(world: &World, blob: &str, (code, out, err): &(i32, String, String)) {
    assert!(
        !records(&world.repo)
            .iter()
            .any(|record| field(record, "result") == "not_reproduced"),
        "a candidate whose commit shows the defect was recorded not_reproduced:\n{out}\n{err}"
    );
    assert_ne!(*code, 4, "{out}\n{err}");
    assert!(
        out.contains(blob) || err.contains(blob),
        "the refusal names the object {blob}:\n{out}\n{err}"
    );
}

/// A world whose candidate keeps the defect in its own `main.rs`, the loose object of which is
/// overwritten in the case repository's store with the bytes of an object holding the fixed
/// text; the world and that object's id.
fn poisoned(name: &str) -> (World, String) {
    let mut world = world(name, true, true);
    git(&world.repo, &["checkout", "-q", "fix"]);
    let own = format!("{}// the candidate's own text\n", takt_main(true));
    fs::write(world.repo.join(TAKT_MAIN), &own).expect("the candidate's main.rs");
    git(
        &world.repo,
        &["commit", "-q", "-am", "the candidate keeps the defect"],
    );
    world.candidate = git(&world.repo, &["rev-parse", "HEAD"]);
    git(&world.repo, &["checkout", "-q", "main"]);
    let blob = git(
        &world.repo,
        &["rev-parse", &format!("{}:{TAKT_MAIN}", world.candidate)],
    );
    let other = world.repo.parent().expect("a parent").join("other");
    fs::create_dir_all(&other).expect("another repository");
    git(&other, &["init", "-q"]);
    fs::write(other.join("main.rs"), takt_main(false)).expect("the fixed text");
    let fixed = git(&other, &["hash-object", "-w", "main.rs"]);
    let target = loose(&world.repo, &blob);
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).expect("writable");
    fs::write(
        &target,
        fs::read(loose(&other, &fixed)).expect("its object"),
    )
    .expect("poisoned");
    (world, blob)
}

/// `GIT_TEMPLATE_DIR` names a template whose `objects/info/alternates` lends a new repository
/// the case repository's (poisoned) objects, so a fetch into one made from it would find every
/// object present and receive none. Takt and `check-finding` make their checkouts with no
/// template, so the objects still come through the transport and are hashed: the check is
/// refused naming the poisoned object.
#[test]
fn a_git_template_lending_objects_does_not_skip_the_rehash() {
    let (world, blob) = poisoned("template");
    let template = world.repo.parent().expect("a parent").join("template");
    write(
        &template.join("objects/info/alternates"),
        &format!("{}\n", world.repo.join(".git/objects").display()),
    );
    let ran = run_with_env(
        &world,
        &opening(&world),
        "candidate.merge",
        &[("GIT_TEMPLATE_DIR", &template)],
    );
    refused_poison(&world, &blob, &ran);
}

/// An outside directory holding what a kept build directory would: a directory `takt` with a
/// file in it, and a file `revision`.
fn outside_build(at: &Path) {
    write(&at.join("takt/precious.txt"), "keep me\n");
    write(&at.join("revision"), "keep me too\n");
}

/// Whether [`outside_build`]'s files at `at` are all still there.
fn outside_kept(at: &Path) -> bool {
    at.join("takt/precious.txt").is_file() && at.join("revision").is_file()
}

/// The reviewer's RV-M, its outcome as ruled: `check-finding/builds` itself a link to an
/// outside directory. A link below the cache root refuses the check, naming the path; the link
/// stays, and nothing behind it is removed.
#[test]
fn a_link_at_the_builds_directory_refuses_the_check() {
    let world = world("link-at-builds", true, true);
    let outside = world
        .repo
        .parent()
        .expect("a parent")
        .join("outside-builds");
    outside_build(&outside.join(&world.candidate));
    outside_build(&outside.join(&world.baseline));
    let builds = world.cache.join("takt/check-finding/builds");
    fs::create_dir_all(builds.parent().expect("a parent")).expect("check-finding's cache");
    std::os::unix::fs::symlink(&outside, &builds).expect("the link");
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    assert!(
        outside_kept(&outside.join(&world.candidate)),
        "{out}\n{err}"
    );
    assert!(outside_kept(&outside.join(&world.baseline)), "{out}\n{err}");
    assert!(
        fs::symlink_metadata(&builds)
            .expect("the link")
            .file_type()
            .is_symlink(),
        "{out}\n{err}"
    );
    assert_eq!(code, 5, "{out}\n{err}");
    let named = format!("{} is a link", builds.display());
    assert!(out.contains(&named) || err.contains(&named), "{out}\n{err}");
}

/// The reviewer's RV-N, inverted as ruled: the cache root `<cache>/takt` is the operator's link
/// to another directory, which takt follows; under it, the candidate's kept-build directory
/// holds a directory `takt`, which no build makes. The check is refused naming it: the
/// operator's link, its target and everything in it stay.
#[test]
fn an_operators_linked_cache_root_stays_and_a_foreign_build_is_refused() {
    let world = world("link-at-root", true, true);
    let outside = world.repo.parent().expect("a parent").join("outside-takt");
    let kept = outside.join("check-finding/builds").join(&world.candidate);
    outside_build(&kept);
    write(&outside.join("promote/keep.txt"), "another takt cache\n");
    fs::create_dir_all(&world.cache).expect("the cache");
    std::os::unix::fs::symlink(&outside, world.cache.join("takt")).expect("the link");
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    assert!(outside_kept(&kept), "{out}\n{err}");
    assert!(outside.join("promote/keep.txt").is_file(), "{out}\n{err}");
    assert!(
        fs::symlink_metadata(world.cache.join("takt"))
            .expect("the link")
            .file_type()
            .is_symlink(),
        "{out}\n{err}"
    );
    assert_eq!(code, 5, "{out}\n{err}");
    let named = world
        .cache
        .join("takt/check-finding/builds")
        .join(&world.candidate)
        .join("takt")
        .display()
        .to_string();
    assert!(out.contains(&named) || err.contains(&named), "{out}\n{err}");
}

/// The reviewer's RV-J: the candidate's kept build a link to a script that does not show the
/// defect. The link is unlinked as itself, the candidate built from its source, and the turn
/// ends `declined`.
#[test]
fn a_kept_build_that_is_a_link_is_rebuilt() {
    let world = world("kept-link", true, true);
    let built = world
        .cache
        .join("takt/check-finding/builds")
        .join(&world.candidate);
    let target = world.cache.join("planted-takt");
    executable(&target, &build(false));
    fs::create_dir_all(&built).expect("the build directory");
    std::os::unix::fs::symlink(&target, built.join("takt")).expect("the link");
    write(&built.join("revision"), &format!("{}\n", world.candidate));
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
    assert!(target.is_file(), "the link's target stays: {out}\n{err}");
}

/// A baseline whose `.gitattributes` has git rewrite the finding when it is checked out (CRLF
/// line ends) gives `check-finding` another text than the one filed: the checkout's finding is
/// compared with the baseline's bytes, and the check records nothing.
#[test]
fn a_finding_rewritten_by_the_baselines_checkout_records_nothing() {
    let world = world_with(
        "rewritten",
        &Setup {
            attributes: Some("*.md text eol=crlf\n"),
            ..Setup::plain(true, true)
        },
    );
    let (code, out, err) = run_with_gate(&world, &opening(&world), "candidate.merge");
    assert_ne!(code, 4, "{out}\n{err}");
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "scenario_result"),
        "{out}\n{err}"
    );
    assert!(out.contains("does not hold"), "{out}\n{err}");
}

/// Writes the promotion record of `revision` into the world's store, chained to the record of
/// `previous`, with its scorecard copy, as `takt promote` leaves one once it installed the
/// binary built from `revision`.
fn promote(world: &World, revision: &str, previous: &str) {
    let store = world.state.join("takt/promotions");
    let copy = store.join(format!("{revision}.scorecard.json"));
    write(&copy, &scorecard(revision, revision));
    let digest = sha256(&copy);
    let before = sha256(&store.join(format!("{previous}.json")));
    let tree = git(&world.repo, &["rev-parse", &format!("{revision}^{{tree}}")]);
    write(
        &store.join(format!("{revision}.json")),
        &format!(
            "{{\"format\":\"takt-promotion/1\",\"revision\":\"{revision}\",\"tree\":\"{tree}\",\"promoted_at\":\"2026-01-02T00:00:00Z\",\"kind\":\"promotion\",\"scorecard\":\"{revision}\",\"scorecard_file\":\"/srv/scorecards/scorecard-{revision}.json\",\"scorecard_sha256\":\"{digest}\",\"against\":\"{previous}\",\"rule\":{{\"trials\":5,\"margin\":1}},\"worse\":0,\"binary\":null,\"binary_sha256\":null,\"previous_sha256\":\"{before}\"}}"
        ),
    );
}

/// The turn resumed by `takt run --case turn-1`.
fn resume(world: &World) -> (i32, String, String) {
    takt_in(world, &["run", "--case", "turn-1", "--iterations", "6"])
}

/// The acceptance of story:self-improvement-protocol under `--forge none`, no `--gate` given: a
/// turn whose finding reproduces at the baseline and not at the candidate stops for the merge's
/// grant; granted, with the candidate merged into the local `main` (the mainline under `--forge
/// none`, which the operator owns) and its scorecard saved, it stops for the promotion's grant;
/// granted, with the candidate promoted, it reaches `accepted`, every record in the case written
/// by a validator of the turn. The merge's record retains what it read: no forge, and the tip of
/// `main` it found the candidate in.
#[test]
fn a_turn_under_forge_none_reaches_accepted_on_validators_records() {
    let world = world("accepted", true, false);
    let (code, out, err) = run_as(
        &world,
        &opening(&world),
        None,
        Some("the probe no longer shows the defect"),
        &[],
    );
    assert_eq!(code, 4, "{out}\n{err}");
    assert!(out.contains("candidate.merge"), "{out}\n{err}");
    takt::case::grant(&world.repo, "turn-1", "repository.merge", "human:owner")
        .expect("the merge is granted");
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    write(
        &world.state.join(format!(
            "takt/scorecards/scorecard-{}.json",
            world.candidate
        )),
        &scorecard(&world.candidate, &world.candidate),
    );
    let (code, out, err) = resume(&world);
    assert_eq!(code, 4, "{out}\n{err}");
    assert!(out.contains("binary.promote"), "{out}\n{err}");
    takt::case::grant(&world.repo, "turn-1", "takt.promote", "human:owner")
        .expect("the promotion is granted");
    promote(&world, &world.candidate, &world.baseline);
    let (code, out, err) = resume(&world);
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (accepted)"), "{out}\n{err}");
    let written = records(&world.repo);
    assert_eq!(
        shown(&written),
        [
            row("finding_record", "finding", "filed", "takt.finding-file"),
            row(
                "scenario_result",
                "baseline",
                "reproduced",
                "takt.scenario-reproduce"
            ),
            row(
                "scenario_result",
                "candidate",
                "not_reproduced",
                "takt.scenario-verify"
            ),
            row(
                "merge_observation",
                "candidate",
                "merged",
                "takt.candidate-merge"
            ),
            row(
                "scorecard_result",
                "candidate",
                "no_regression",
                "takt.scorecard-compare"
            ),
            row(
                "promotion_record",
                "promotion",
                "promoted",
                "takt.binary-promote"
            ),
        ],
        "{out}\n{err}"
    );
    for record in &written[2..] {
        assert_eq!(
            field(record, "subject_revision"),
            world.candidate,
            "{record}"
        );
    }
    let artifacts = world.repo.join(".engineering/cases/turn-1/artifacts");
    let retained: Vec<String> = fs::read_dir(&artifacts)
        .expect("the case's artifacts")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("merge-observation-"))
        })
        .map(|path| fs::read_to_string(path).expect("the merge's artifact"))
        .collect();
    assert_eq!(retained.len(), 1, "{retained:?}");
    assert!(retained[0].contains("forge: none"), "{}", retained[0]);
    assert!(
        retained[0].contains(&format!("tip: {}", world.candidate)),
        "{}",
        retained[0]
    );
}

/// Granted, the merge validator records nothing while the candidate is not in local `main`: it
/// waits, naming the tip it read (the baseline, where `main` still is), and the turn stays short
/// of `fix.merged`.
#[test]
fn a_granted_candidate_not_yet_in_main_is_not_merged() {
    let world = world("not-merged", true, false);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 4, "{out}\n{err}");
    takt::case::grant(&world.repo, "turn-1", "repository.merge", "human:owner")
        .expect("the merge is granted");
    // An hour budget of 3.6 s: the validator waits on the mainline, and the budget ends it.
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "merge_observation"),
        "{out}\n{err}"
    );
    assert!(out.contains(&world.baseline[..8]), "{out}\n{err}");
}

/// The judge surface is computed when the turn opens, from the baseline's allow-list over the
/// candidate's change: a given `surface` that disagrees is refused before anything is written,
/// naming the first judge-surface path; none given, the case opens with the computed one, and
/// the turn stops for the judge-surface merge's grant.
#[test]
fn the_surface_is_computed_at_open_and_a_disagreeing_one_is_refused() {
    let world = world_with(
        "surface",
        &Setup {
            touches: Some(("tools/judge.sh", "#!/bin/sh\nexit 0\n")),
            ..Setup::plain(true, false)
        },
    );
    let ran = run(&world, &opening(&world));
    refused(&world, &ran, "tools/judge.sh");
    let inputs = vec![
        format!("finding={FINDING}"),
        format!("candidate={}", world.candidate),
    ];
    let (code, out, err) = run_as(
        &world,
        &inputs,
        None,
        Some("the probe no longer shows the defect"),
        &[],
    );
    assert_eq!(code, 4, "{out}\n{err}");
    assert!(
        out.contains("candidate.merge_judge_surface"),
        "{out}\n{err}"
    );
    let case = fs::read_to_string(world.repo.join(".engineering/cases/turn-1/case.yaml"))
        .expect("case.yaml");
    assert!(case.contains("surface: judge"), "{case}");
}

/// A finding filed on `main` after the last promotion opens a turn: it is read where the case
/// forks from, a commit of the mainline, not at the baseline, which does not hold it.
#[test]
fn a_finding_filed_on_main_after_the_last_promotion_opens_a_turn() {
    let world = world_with(
        "filed-after",
        &Setup {
            filed_after: true,
            ..Setup::plain(true, true)
        },
    );
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
    let case = takt::CaseDir::resume(&world.repo, "turn-1").expect("the case reads");
    let main = git(&world.repo, &["rev-parse", "main"]);
    assert_ne!(main, world.baseline);
    assert_eq!(case.revision("finding").as_deref(), Some(main.as_str()));
}

/// A turn opened with no `--intent` opens on its finding's title, never the planning store's
/// frontier.
#[test]
fn a_turn_without_an_intent_opens_with_the_findings_title() {
    let world = world("no-intent", true, true);
    let (code, out, err) = run_as(&world, &opening(&world), Some("candidate.merge"), None, &[]);
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(!out.contains("frontier: nothing ready"), "{out}\n{err}");
    assert!(out.contains("Completed (declined)"), "{out}\n{err}");
    let intent = fs::read_to_string(world.repo.join(".engineering/cases/turn-1/intent.md"))
        .expect("intent.md");
    assert_eq!(intent, "The probe shows the defect");
}
