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

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The self-improvement protocol, the engineering protocols' built-in.
const PROTOCOL: &str = "self.improvement/1";

/// A `b10x-gates` policy file the stand-ins never read: the bot route needs one named
/// (story:forge-preflight).
const GATES_POLICY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/gates-policy-stand-in.json"
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
    /// The install root `takt promote` installs into and `binary.promote` reads the installed
    /// binary from (`CARGO_INSTALL_ROOT`), never the operator's.
    install: PathBuf,
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
    let install = dir.join("install");
    // A turn opens only on an install root that exists: the one it is given, and the decoy the
    // environment names.
    fs::create_dir_all(&install).expect("the install root");
    fs::create_dir_all(dir.join("decoy-install")).expect("the decoy install root");
    World {
        _scratch: Scratch(dir),
        repo,
        baseline,
        candidate,
        scenarios,
        state,
        cache,
        install,
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
            "--install-root",
            world.install.to_str().expect("utf-8"),
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
        .env("CARGO_INSTALL_ROOT", decoy(world))
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

/// The install root the environment names (`CARGO_INSTALL_ROOT`) to every run, which is not
/// the one the turn opens with (`--install-root`): `binary.promote` reads the recorded one.
fn decoy(world: &World) -> PathBuf {
    world.repo.parent().expect("a parent").join("decoy-install")
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
    let dir = repo.join(".takt/cases/turn-1/evidence");
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
    let case = world.repo.join(".takt/cases/turn-1");
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
        world.repo.join(".takt/cases/change-1/case.yaml").is_file(),
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
            !world.repo.join(".takt/cases").exists(),
            "{inputs:?}: nothing was written"
        );
        assert_eq!(git(&world.repo, &["branch", "--list", "takt/*"]), "");
    }
    fs::remove_dir_all(world.state.join("takt/promotions")).expect("the store is emptied");
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("promotion"), "{err}");
    assert!(!world.repo.join(".takt/cases").exists());
}

/// Asserts the turn was refused before anything was written, naming `named`.
fn refused(world: &World, (code, out, err): &(i32, String, String), named: &str) {
    assert_eq!(*code, 1, "{out}\n{err}");
    assert!(err.contains(named), "{named}: {err}");
    assert!(
        !world.repo.join(".takt/cases").exists(),
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

/// story:a-turn-refuses-a-candidate-main-already-holds: a candidate the mainline's tip already
/// holds (here `main` moved onto it before the turn opens, as the first real turn's candidate was
/// merged before its grant) is refused before any step, naming the candidate, the tip and the
/// remedy; one `main` holds as an ancestor of its tip is refused the same.
#[test]
fn a_candidate_the_mainline_already_holds_is_refused_at_open() {
    let held = world("held-candidate", true, false);
    git(&held.repo, &["reset", "-q", "--hard", &held.candidate]);
    assert_eq!(git(&held.repo, &["rev-parse", "main"]), held.candidate);
    let ran = run(&held, &opening(&held));
    refused(&held, &ran, &held.candidate);
    refused(&held, &ran, "open the turn on a fresh candidate");

    let ancestor = world("held-ancestor", true, false);
    git(
        &ancestor.repo,
        &["reset", "-q", "--hard", &ancestor.candidate],
    );
    write(&ancestor.repo.join("after.txt"), "after the candidate\n");
    git(&ancestor.repo, &["add", "-A"]);
    git(&ancestor.repo, &["commit", "-q", "-m", "after"]);
    let tip = git(&ancestor.repo, &["rev-parse", "main"]);
    assert_ne!(tip, ancestor.candidate);
    let ran = run(&ancestor, &opening(&ancestor));
    refused(&ancestor, &ran, &ancestor.candidate);
    refused(&ancestor, &ran, &tip);
    refused(&ancestor, &ran, "open the turn on a fresh candidate");
}

/// story:a-turn-does-not-open-when-the-mainline-tip-cannot-be-read: with no local `main` under
/// `--forge none` the mainline's tip cannot be read, so whether it holds the candidate is not
/// known, and the turn is refused before anything is written, naming the forge, why the read
/// failed and the remedy.
#[test]
fn a_turn_whose_mainline_tip_cannot_be_read_is_refused_at_open() {
    let world = world("no-tip", true, false);
    git(&world.repo, &["branch", "-m", "main", "trunk"]);
    let ran = run(&world, &opening(&world));
    refused(&world, &ran, "(forge: none)");
    refused(&world, &ran, "read main, the mainline with no forge");
    refused(&world, &ran, "open the turn again once the tip can be read");
}

/// The finding's predicate: a candidate the local `main` holds, opened through the bot route
/// while the route's programs fail, is refused for the unreadable tip before the opening reads
/// anything after it (the promotion store), naming the forge and the route's own error.
#[test]
fn a_turn_whose_forge_cannot_report_the_tip_is_refused_at_open() {
    let world = world("route-down", true, false);
    let bin = world.repo.parent().expect("a parent").join("bin");
    executable(
        &bin.join("b10x-gates"),
        "#!/bin/sh\necho \"the route is down\" >&2\nexit 1\n",
    );
    git(
        &world.repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example/todo-app.git",
        ],
    );
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").expect("a PATH")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--protocol",
            PROTOCOL,
            "--install-root",
            world.install.to_str().expect("utf-8"),
            "--case-id",
            "turn-1",
            "--harness",
            "none",
            "--forge",
            "bot",
            "--test-cmd",
            "true",
            "--iterations",
            "0",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
            "--intent",
            "the probe no longer shows the defect",
            "--commit-identity",
            "t <t@localhost>",
        ])
        .args(opening(&world).iter().flat_map(|input| ["--input", input]))
        .env("PATH", path)
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env("CARGO_INSTALL_ROOT", decoy(&world))
        .env("B10X_GATES_POLICY", GATES_POLICY)
        .output()
        .expect("takt runs");
    let ran = (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    );
    refused(&world, &ran, "(forge: bot)");
    refused(&world, &ran, "the route is down");
    refused(&world, &ran, "open the turn again once the tip can be read");
}

/// Only an opening reads the tip this way: a turn opened while `main` could be read resumes
/// once it cannot, and its next step records.
#[test]
fn a_turn_resumes_when_the_mainline_tip_cannot_be_read() {
    let world = world("resume-no-tip", true, false);
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
    let inputs = opening(&world);
    for input in &inputs {
        opened.extend(["--input", input.as_str()]);
    }
    let (code, out, err) = takt_in(&world, &opened);
    assert_eq!(code, 3, "{out}\n{err}");
    assert_eq!(records(&world.repo).len(), 2, "{out}\n{err}");
    git(&world.repo, &["branch", "-m", "main", "trunk"]);
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--iterations", "1"]);
    assert!(!err.contains("once the tip can be read"), "{out}\n{err}");
    assert_eq!(
        shown(&records(&world.repo))[2],
        row(
            "scenario_result",
            "candidate",
            "not_reproduced",
            "takt.scenario-verify"
        ),
        "{out}\n{err}"
    );
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
        .env("CARGO_INSTALL_ROOT", decoy(world))
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
/// the kept build only while it hashes to that record. The repository's `main`, the mainline,
/// is another history ([`another_mainline`]): one that holds the baseline would refuse it as the
/// candidate, and with no `main` no turn opens.
#[test]
fn a_build_planted_between_the_build_and_verify_is_rebuilt() {
    let world = world("planted-between", true, true);
    another_mainline(&world);
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
/// `previous`, with its scorecard copy, naming the binary `installed` and its digest when some:
/// the record `takt promote` leaves once it installed the binary built from `revision`.
fn record(world: &World, revision: &str, previous: &str, installed: Option<&Path>) {
    let store = world.state.join("takt/promotions");
    let copy = store.join(format!("{revision}.scorecard.json"));
    write(&copy, &scorecard(revision, revision));
    let digest = sha256(&copy);
    let before = sha256(&store.join(format!("{previous}.json")));
    let tree = git(&world.repo, &["rev-parse", &format!("{revision}^{{tree}}")]);
    let binary = installed.map_or(
        "\"binary\":null,\"binary_sha256\":null".to_owned(),
        |path| {
            format!(
                "\"binary\":\"{}\",\"binary_sha256\":\"{}\"",
                path.display(),
                sha256(path)
            )
        },
    );
    write(
        &store.join(format!("{revision}.json")),
        &format!(
            "{{\"format\":\"takt-promotion/1\",\"revision\":\"{revision}\",\"tree\":\"{tree}\",\"promoted_at\":\"2026-01-02T00:00:00Z\",\"kind\":\"promotion\",\"scorecard\":\"{revision}\",\"scorecard_file\":\"/srv/scorecards/scorecard-{revision}.json\",\"scorecard_sha256\":\"{digest}\",\"against\":\"{previous}\",\"rule\":{{\"trials\":5,\"margin\":1}},\"worse\":0,{binary},\"previous_sha256\":\"{before}\"}}"
        ),
    );
}

/// Promotes `revision` as `takt promote` would leave it: a binary built from it installed at the
/// world's install root (a stand-in, never the binary that judges), and its promotion record.
fn promote(world: &World, revision: &str, previous: &str) {
    let installed = world.install.join("bin/takt");
    executable(
        &installed,
        &format!("#!/bin/sh\necho 'takt 0.1.0 ({revision})'\n"),
    );
    record(world, revision, previous, Some(&installed));
}

/// `takt approve` on the turn: grants `capability` by the owner, and leaves the planning store
/// and the case's branch as they were, a turn's grant being the case's alone.
fn approve(world: &World, capability: &str) {
    let head = git(&world.repo, &["rev-parse", "HEAD"]);
    let finding = fs::read(world.repo.join(FINDING_FILE)).ok();
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "approve",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "turn-1",
            capability,
            "--by",
            "human:owner",
        ])
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let shown = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0), "{shown}");
    assert_eq!(
        git(&world.repo, &["rev-parse", "HEAD"]),
        head,
        "a commit: {shown}"
    );
    assert_eq!(
        fs::read(world.repo.join(FINDING_FILE)).ok(),
        finding,
        "the finding's story was written: {shown}"
    );
    assert_eq!(
        git(
            &world.repo,
            &[
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--",
                ".",
                ":!.takt/cases"
            ]
        ),
        "",
        "{shown}"
    );
}

/// The turn resumed by `takt run --case turn-1`.
fn resume(world: &World) -> (i32, String, String) {
    takt_in(world, &["run", "--case", "turn-1", "--iterations", "6"])
}

/// The acceptance of story:self-improvement-protocol under `--forge none`, no `--gate` given: a
/// turn whose finding reproduces at the baseline and not at the candidate stops for the merge's
/// grant; granted with `takt approve` (which writes nothing to the planning store and commits
/// nothing), with the candidate merged into the local `main` (the mainline under `--forge
/// none`, which the operator owns) and its scorecard saved, it stops for the promotion's grant;
/// granted, with the candidate promoted and installed, it reaches `accepted` in a run by the
/// binary that judged it from the start, every record in the case written by a validator of the
/// turn. The merge's record retains what it read: no forge, and the tip of `main` it found the
/// candidate in.
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
    approve(&world, "repository.merge");
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
    approve(&world, "takt.promote");
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
    let artifacts = world.repo.join(".takt/cases/turn-1/artifacts");
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
    let case =
        fs::read_to_string(world.repo.join(".takt/cases/turn-1/case.yaml")).expect("case.yaml");
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
    let intent =
        fs::read_to_string(world.repo.join(".takt/cases/turn-1/intent.md")).expect("intent.md");
    assert_eq!(intent, "The probe shows the defect");
}

/// The turn up to `binary.promote`'s grant: opened, the merge approved, the candidate merged
/// into the local `main`, its scorecard saved, resumed, and the promotion approved.
fn up_to_the_promotion(world: &World) {
    let (code, out, err) = run(world, &opening(world));
    assert_eq!(code, 4, "{out}\n{err}");
    approve(world, "repository.merge");
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    write(
        &world.state.join(format!(
            "takt/scorecards/scorecard-{}.json",
            world.candidate
        )),
        &scorecard(&world.candidate, &world.candidate),
    );
    let (code, out, err) = resume(world);
    assert_eq!(code, 4, "{out}\n{err}");
    approve(world, "takt.promote");
}

/// The reviewer's `rv_a_step_planting_a_promotion_record_is_not_promoted`: a promotion record
/// of the candidate that names no installed binary (one anything with the operator's rights
/// could write; nobody ran `takt promote`) earns nothing: `binary.promote` waits, and the turn
/// is not accepted.
#[test]
fn a_promotion_record_naming_no_installed_binary_is_not_promoted() {
    let world = world("planted-promotion", true, false);
    up_to_the_promotion(&world);
    record(&world, &world.candidate, &world.baseline, None);
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert!(!out.contains("Completed (accepted)"), "{out}\n{err}");
    // It reads the install root the turn opened with, never the environment's, and says where.
    let read = world.install.join("bin/takt").display().to_string();
    assert!(out.contains(&read), "{read}: {out}\n{err}");
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "promotion_record"),
        "{out}\n{err}"
    );
}

/// The turn's decision files and its run record's `decision` and `waiting` lines, counted.
fn written(world: &World) -> (usize, usize, usize) {
    let case = world.repo.join(".takt/cases/turn-1");
    let files = fs::read_dir(case.join("decisions"))
        .map(Iterator::count)
        .unwrap_or_default();
    let events = fs::read_to_string(case.join("events.jsonl")).unwrap_or_default();
    let lines = |event: &str| {
        events
            .lines()
            .filter(|line| line.contains(&format!("\"event\":\"{event}\"")))
            .count()
    };
    (files, lines("decision"), lines("waiting"))
}

/// A resume of the turn with a step budget of three, which a wait polled every 30 s would spend
/// observing again: what it wrote, as [`written`] counts it, and its exit code and output.
fn resumed_on_a_local_wait(world: &World) -> ((usize, usize, usize), (i32, String, String)) {
    let before = written(world);
    let ran = takt_in(world, &["run", "--case", "turn-1", "--iterations", "3"]);
    let after = written(world);
    (
        (after.0 - before.0, after.1 - before.1, after.2 - before.2),
        ran,
    )
}

/// What `out` says besides the line a `--forge none` run opens with, which names the forge it
/// chose: no other line of it says "forge".
fn says_forge(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|line| !line.starts_with("forge: none ("))
        .filter(|line| line.contains("forge"))
        .collect()
}

/// story:a-local-wait-exits-waiting: a turn whose `scorecard.compare` finds no saved scorecard
/// of the candidate stops `Waiting` at once after that step, exit 6, naming the file it waits
/// for: no pause, no observation again and no decision after the step, so a run with a budget
/// writes one decision per step and no `waiting` line. A second look, the turn resumed, does
/// the same in one step. Under `--forge none` no line of it says "forge".
#[test]
fn a_turn_without_the_candidates_scorecard_exits_waiting_after_the_step_naming_the_file() {
    let world = world("local-wait", true, false);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 4, "{out}\n{err}");
    approve(&world, "repository.merge");
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    let file = world
        .state
        .join(format!(
            "takt/scorecards/scorecard-{}.json",
            world.candidate
        ))
        .display()
        .to_string();
    // The merge observed, then the comparison, which waits: two steps, two decisions.
    let (grew, (code, out, err)) = resumed_on_a_local_wait(&world);
    assert_eq!(code, 6, "{out}\n{err}");
    assert_eq!(grew, (2, 2, 0), "{out}\n{err}");
    let stopped = out
        .lines()
        .find(|line| line.starts_with("stopped: "))
        .unwrap_or_default();
    assert!(
        stopped.starts_with("stopped: Waiting (scorecard.compare; ") && stopped.contains(&file),
        "{out}\n{err}"
    );
    assert!(stopped.contains("steps 2,"), "{out}\n{err}");
    assert_eq!(says_forge(&out), Vec::<&str>::new(), "{out}\n{err}");
    // The second look: one step, one decision, the same stop.
    let (grew, (code, out, err)) = resumed_on_a_local_wait(&world);
    assert_eq!(code, 6, "{out}\n{err}");
    assert_eq!(grew, (1, 1, 0), "{out}\n{err}");
    assert!(out.contains("steps 1,"), "{out}\n{err}");
    assert!(out.contains("Waiting (scorecard.compare; "), "{out}\n{err}");
    assert!(out.contains(&file), "{out}\n{err}");
    assert_eq!(says_forge(&out), Vec::<&str>::new(), "{out}\n{err}");
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "scorecard_result"),
        "{out}\n{err}"
    );
}

/// Under `--forge none` the mainline is the local `main`, which only the operator moves: a
/// granted merge not yet there stops `Waiting` at once after `candidate.merge`, exit 6, naming
/// the tip it read, and no line says "forge".
#[test]
fn a_forge_none_turn_waiting_for_the_local_merge_exits_waiting_after_the_step() {
    let world = world("local-merge-wait", true, false);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 4, "{out}\n{err}");
    approve(&world, "repository.merge");
    let (grew, (code, out, err)) = resumed_on_a_local_wait(&world);
    assert_eq!(code, 6, "{out}\n{err}");
    assert_eq!(grew, (1, 1, 0), "{out}\n{err}");
    assert!(
        out.contains("stopped: Waiting (candidate.merge; ") && out.contains("steps 1,"),
        "{out}\n{err}"
    );
    assert!(out.contains(&world.baseline[..8]), "{out}\n{err}");
    assert_eq!(says_forge(&out), Vec::<&str>::new(), "{out}\n{err}");
}

/// `binary.promote` waits on the promotion store and the binary installed, both local: granted
/// and not yet promoted, the turn stops `Waiting` at once after the step, exit 6, naming the
/// binary it read, and no line says "forge".
#[test]
fn a_turn_waiting_for_its_promotion_exits_waiting_after_the_step() {
    let world = world("local-promote-wait", true, false);
    up_to_the_promotion(&world);
    let (grew, (code, out, err)) = resumed_on_a_local_wait(&world);
    assert_eq!(code, 6, "{out}\n{err}");
    assert_eq!(grew, (1, 1, 0), "{out}\n{err}");
    assert!(
        out.contains("stopped: Waiting (binary.promote; ") && out.contains("steps 1,"),
        "{out}\n{err}"
    );
    let read = world.install.join("bin/takt").display().to_string();
    assert!(out.contains(&read), "{read}: {out}\n{err}");
    assert_eq!(says_forge(&out), Vec::<&str>::new(), "{out}\n{err}");
}

/// The reviewer's `rv_a_resume_by_another_binary_than_the_promoted_is_refused`: the turn
/// recorded the binary that judges it when it opened; a resume by another binary (here a copy of
/// it with bytes appended, as the candidate's build `takt promote` installed would be) is
/// refused before any step, naming both digests, and nothing is recorded.
#[test]
fn a_resume_by_another_binary_than_the_judge_is_refused() {
    let world = world("other-judge", true, false);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 4, "{out}\n{err}");
    approve(&world, "repository.merge");
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    write(
        &world.state.join(format!(
            "takt/scorecards/scorecard-{}.json",
            world.candidate
        )),
        &scorecard(&world.candidate, &world.candidate),
    );
    let other = world.repo.parent().expect("a parent").join("other-takt");
    let mut bytes = fs::read(env!("CARGO_BIN_EXE_takt")).expect("the binary");
    bytes.extend_from_slice(b"\nanother binary\n");
    fs::write(&other, bytes).expect("a copy");
    fs::set_permissions(&other, fs::Permissions::from_mode(0o755)).expect("executable");
    let before = records(&world.repo).len();
    let output = Command::new(&other)
        .args([
            "run",
            "--case",
            "turn-1",
            "--iterations",
            "6",
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
        .env("CARGO_INSTALL_ROOT", &world.install)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("the other binary runs");
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{out}\n{err}");
    assert_eq!(records(&world.repo).len(), before, "{out}\n{err}");
    assert!(
        err.contains(&sha256(Path::new(env!("CARGO_BIN_EXE_takt")))),
        "the judge's digest is named: {err}"
    );
    assert!(
        err.contains("sha256sum"),
        "the kept binary is to be checked first: {err}"
    );
}

/// The reviewer's `rv_takt_approve_on_a_turn_with_a_planning_store`, inverted as ruled: in a
/// repository with a planning store, whose finding is a story with its frontmatter, `takt
/// approve` on a turn grants in the case alone: no store write, no commit, and the turn runs on.
#[test]
fn approving_a_turn_in_a_repository_with_a_planning_store_writes_nothing_there() {
    let world = world("approve-store", true, false);
    write(
        &world.repo.join(".engineering/project.yaml"),
        "version: aep.project/5\n\nprotocol: adp/1\nprofile: development.standard\nplanning_scope: \"scratch\"\nstore:\n  git: {}\n\nsummary: >-\n  scratch\n",
    );
    write(
        &world.repo.join(FINDING_FILE),
        &format!(
            "---\nformat: aep.planning-md/3\nid: {FINDING}\nkind: story\nstatus: proposed\ntitle: The probe shows the defect\nsummary: the probe\nrevision: 3\n---\n{}",
            finding_text()
        ),
    );
    git(&world.repo, &["add", "-A"]);
    git(&world.repo, &["commit", "-q", "-m", "store"]);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 4, "{out}\n{err}");
    approve(&world, "repository.merge");
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert!(
        records(&world.repo)
            .iter()
            .any(|record| field(record, "kind") == "merge_observation"),
        "{out}\n{err}"
    );
}

/// Moves the operator's branch, `HEAD`, from `main` to `trunk`, and points `main`, the
/// mainline, at a commit with no parent, which holds neither the baseline nor the candidate:
/// the tip is read, the case forks off the mainline, and the candidate is no merged one.
fn another_mainline(world: &World) {
    git(&world.repo, &["branch", "-m", "main", "trunk"]);
    let tree = git(&world.repo, &["rev-parse", "HEAD^{tree}"]);
    let other = git(
        &world.repo,
        &["commit-tree", "-m", "another mainline", tree.as_str()],
    );
    git(&world.repo, &["branch", "main", other.as_str()]);
}

/// With the case forking off the mainline (`main` another history), the finding is read off
/// the mainline: the part-1 rule holds, and a candidate that rewrites the finding's body is
/// refused, naming its file.
#[test]
fn off_the_mainline_a_candidate_rewriting_the_finding_is_refused() {
    let world = world_with(
        "no-main",
        &Setup {
            touches: Some((FINDING_FILE, "#!/bin/sh\necho changed by the candidate\n")),
            ..Setup::plain(true, true)
        },
    );
    another_mainline(&world);
    let inputs = [
        "surface=judge".to_owned(),
        format!("finding={FINDING}"),
        format!("candidate={}", world.candidate),
    ];
    let ran = run(&world, &inputs);
    refused(&world, &ran, FINDING_FILE);
}

/// `takt approve` on the turn by `approver` for `capability`, its exit code and output.
fn approve_by(world: &World, capability: &str, approver: &str) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "approve",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "turn-1",
            capability,
            "--by",
            approver,
        ])
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

/// The reviewer's `rv_an_undelegated_agent_cannot_grant_a_turns_judge_surface_merge`: a turn
/// whose candidate touches a judge surface, granted `repository.merge.judge_surface` by an agent
/// role the owner delegated nothing to (no `.engineering/delegations.yaml` at `main`), records no
/// merge though the candidate is in `main`: story:judge-guard's rule holds for the turn too, and
/// the run names the delegation record it read.
#[test]
fn an_undelegated_agent_cannot_grant_a_turns_judge_surface_merge() {
    let world = world_with(
        "agent-judge",
        &Setup {
            touches: Some(("tools/judge.sh", "#!/bin/sh\nexit 0\n")),
            ..Setup::plain(true, false)
        },
    );
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
    let (code, shown) = approve_by(&world, "repository.merge.judge_surface", "agent:developer");
    assert_eq!(code, 0, "{shown}");
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "merge_observation"),
        "an undelegated agent's grant merged a judge-surface candidate: {out}\n{err}"
    );
    assert!(
        out.contains(".engineering/delegations.yaml"),
        "{out}\n{err}"
    );
}

/// A turn whose candidate touches a judge surface, opened by `takt run` under `--forge none`, at
/// the gate of its judge-surface merge.
fn judge_turn_at_the_gate(name: &str) -> World {
    let world = world_with(
        name,
        &Setup {
            touches: Some(("tools/judge.sh", "#!/bin/sh\nexit 0\n")),
            ..Setup::plain(true, false)
        },
    );
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
    world
}

/// The reviewer's `rv_cli_owner_grant_then_merge_records_merged`: through the binary, the turn
/// `takt run` opened records its forge, the owner's `takt approve` records `main`'s tip with the
/// grant, and a merge after the grant records `merged`.
#[test]
fn a_judge_surface_merge_after_the_owners_approval_is_recorded() {
    let world = judge_turn_at_the_gate("judge-after-approve");
    let case = world.repo.join(".takt/cases/turn-1");
    let forge = fs::read_to_string(case.join("forge")).expect("the turn records its forge");
    assert_eq!(forge, "none\n");
    let main = git(&world.repo, &["rev-parse", "main"]);
    let (code, shown) = approve_by(&world, "repository.merge.judge_surface", "human:owner");
    assert_eq!(code, 0, "{shown}");
    let authority = fs::read_to_string(case.join("authority.yaml")).expect("the authority");
    assert!(
        authority.contains(&format!("mainline_tip: {main}")),
        "{authority}"
    );
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert!(
        records(&world.repo)
            .iter()
            .any(|record| field(record, "kind") == "merge_observation"),
        "{out}\n{err}"
    );
}

/// The reviewer's `rv_cli_fast_forward_before_approve_records_nothing`, the first real turn's
/// shape through the binary: `main` fast-forwarded to the candidate before the owner's `takt
/// approve`. No merge is recorded, and the run says why.
#[test]
fn a_judge_surface_merge_before_the_owners_approval_records_nothing() {
    let world = judge_turn_at_the_gate("judge-before-approve");
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    let (code, shown) = approve_by(&world, "repository.merge.judge_surface", "human:owner");
    assert_eq!(code, 0, "{shown}");
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "merge_observation"),
        "{out}\n{err}"
    );
    assert!(out.contains("held the candidate"), "{out}\n{err}");
}

/// The reviewer's `rv_a_turn_on_a_protocol_file_with_a_harness_action`, inverted as ruled: a
/// protocol document of id `self.improvement/1` other than the embedded one (here with one more
/// action, which no validator performs) opens no turn: refused before anything is written,
/// naming the embedded protocol, and the harness never runs.
#[test]
fn a_turn_on_another_protocol_document_of_its_id_is_refused() {
    let world = world("protocol-file", true, false);
    let dir = world.repo.parent().expect("a parent").to_path_buf();
    let text = canon_engineering::registry::get("self-improvement", 1)
        .expect("the protocol")
        .yaml;
    let extra = text.replacen(
        "\nactions:\n",
        "\nactions:\n  turn.note:\n    description: a note\n    effect: write\n",
        1,
    );
    assert_ne!(extra, text);
    let protocol = dir.join("self-improvement-extra.yaml");
    fs::write(&protocol, &extra).expect("the protocol file");
    let bin = dir.join("harness-bin");
    executable(
        &bin.join("claude"),
        "#!/bin/sh\necho called >> ../claude-called\nprintf '%s\\n' '{\"result\": \"{\\\"action\\\": \\\"turn.note\\\", \\\"summary\\\": \\\"note\\\"}\"}'\n",
    );
    let mut args: Vec<String> = [
        "run",
        "--repo",
        world.repo.to_str().expect("utf-8"),
        "--protocol",
        protocol.to_str().expect("utf-8"),
        "--intent",
        "a note",
        "--case-id",
        "turn-1",
        "--harness",
        "claude",
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
    .map(|arg| (*arg).to_owned())
    .collect();
    for input in opening(&world) {
        args.extend(["--input".to_owned(), input]);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env("CARGO_INSTALL_ROOT", decoy(&world))
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let ran = (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    );
    refused(&world, &ran, "embeds");
    assert!(
        !dir.join("claude-called").exists(),
        "the harness ran: {}\n{}",
        ran.1,
        ran.2
    );
}

/// `takt` run in `world` with `args` and the run's state and cache, the environment naming the
/// decoy install root; its exit code, standard output and standard error.
fn takt_plain(world: &World, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env("CARGO_INSTALL_ROOT", decoy(world))
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The arguments of `takt run` opening the turn `turn-1` with the install root `root`.
fn opening_with_root(world: &World, root: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "run",
        "--repo",
        world.repo.to_str().expect("utf-8"),
        "--protocol",
        "self.improvement/1",
        "--intent",
        "the probe no longer shows the defect",
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
        "--install-root",
        root,
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    for input in opening(world) {
        args.extend(["--input".to_owned(), input]);
    }
    args
}

/// The reviewer's `rv_b_install_root_recorded_and_fixed`: an install root given through `..`
/// and a link is recorded canonical, the directory itself; a resume given `--install-root` is
/// refused, naming the flag, since the case keeps the root it was opened with.
#[test]
fn the_install_root_is_recorded_canonical_and_fixed() {
    let world = world("root-canonical", true, false);
    let dir = world.repo.parent().expect("a parent").to_path_buf();
    let link = dir.join("root-link");
    std::os::unix::fs::symlink(&world.install, &link).expect("the link");
    let given = format!(
        "{}/../{}/root-link",
        dir.display(),
        dir.file_name().expect("a name").to_string_lossy()
    );
    let args = opening_with_root(&world, &given);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, out, err) = takt_plain(&world, &args);
    assert_eq!(code, 4, "{out}\n{err}");
    let recorded = fs::read_to_string(world.repo.join(".takt/cases/turn-1/install-root"))
        .expect("the recorded install root");
    let canonical = fs::canonicalize(&world.install).expect("the install root");
    assert_eq!(
        recorded,
        format!("{}\n", canonical.display()),
        "given {given}"
    );
    let other = dir.join("other-root");
    let (code, out, err) = takt_plain(
        &world,
        &[
            "run",
            "--case",
            "turn-1",
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
            "--install-root",
            other.to_str().expect("utf-8"),
        ],
    );
    assert_ne!(code, 0, "{out}\n{err}");
    assert!(err.contains("--install-root"), "{err}");
}

/// story:self-improvement-protocol, as trial 4 met it: a resume given `--install-root` naming the
/// root the turn recorded, by a path whose canonical form is that root (here a link), is
/// resumed like a resume given none; one naming another root that exists is refused before
/// any step, naming both roots, and the recorded root is left as it was. A turn that recorded
/// no root, and a case that is no turn, refuse `--install-root` as before.
#[test]
fn a_resume_given_the_recorded_install_root_resumes_and_another_is_refused_naming_both() {
    let world = world("root-resume", true, false);
    let dir = world.repo.parent().expect("a parent").to_path_buf();
    let args = opening_with_root(&world, world.install.to_str().expect("utf-8"));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, out, err) = takt_plain(&world, &args);
    assert_eq!(code, 4, "{out}\n{err}");
    let recorded_file = world.repo.join(".takt/cases/turn-1/install-root");
    let recorded = fs::read_to_string(&recorded_file).expect("the recorded install root");
    let canonical = fs::canonicalize(&world.install).expect("the install root");
    let link = dir.join("install-link");
    std::os::unix::fs::symlink(&world.install, &link).expect("the link");
    let resume = |root: &Path| {
        takt_plain(
            &world,
            &[
                "run",
                "--case",
                "turn-1",
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
                "--install-root",
                root.to_str().expect("utf-8"),
            ],
        )
    };

    let (code, out, err) = resume(&link);
    assert_eq!(
        code, 4,
        "the recorded root, given again, was refused: {out}\n{err}"
    );
    assert!(out.contains("stopped: "), "{out}\n{err}");
    assert!(!err.contains("--install-root"), "{err}");

    let other = dir.join("decoy-install");
    let (code, out, err) = resume(&other);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("--install-root"), "{err}");
    assert!(err.contains(other.to_str().expect("utf-8")), "{err}");
    assert!(err.contains(canonical.to_str().expect("utf-8")), "{err}");
    assert!(!out.contains("stopped: "), "a step was taken: {out}\n{err}");
    assert_eq!(
        fs::read_to_string(&recorded_file).expect("the recorded install root"),
        recorded
    );

    fs::remove_file(&recorded_file).expect("the recorded root removed");
    let (code, out, err) = resume(&link);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("--install-root"), "{err}");
    assert!(!out.contains("stopped: "), "a step was taken: {out}\n{err}");

    let plain = dir.join("plain");
    fs::create_dir_all(&plain).expect("a directory");
    fs::write(plain.join("Cargo.toml"), "[package]\nname = \"plain\"\n").expect("a file");
    git(&plain, &["init", "-q", "-b", "main"]);
    git(&plain, &["add", "-A"]);
    git(&plain, &["commit", "-q", "-m", "plain"]);
    let plain_run = |extra: &[&str]| {
        let mut args = vec![
            "run",
            "--repo",
            plain.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--iterations",
            "1",
        ];
        args.extend_from_slice(extra);
        takt_plain(&world, &args)
    };
    let (_, out, err) = plain_run(&["--intent", "a plain change", "--case-id", "plain-1"]);
    assert!(out.contains("stopped: "), "{out}\n{err}");
    let (code, out, err) = plain_run(&[
        "--case",
        "plain-1",
        "--install-root",
        canonical.to_str().expect("utf-8"),
    ]);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("--install-root"), "{err}");
    assert!(!out.contains("stopped: "), "a step was taken: {out}\n{err}");
}

/// An install root that does not exist opens no turn: refused before anything is written,
/// naming it.
#[test]
fn a_missing_install_root_is_refused_at_open() {
    let world = world("root-missing", true, false);
    let missing = world.repo.parent().expect("a parent").join("no-such-root");
    let args = opening_with_root(&world, missing.to_str().expect("utf-8"));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let ran = takt_plain(&world, &args);
    refused(&world, &ran, missing.to_str().expect("utf-8"));
}

/// The reviewer's `rv_c_resumed_case_on_another_document_is_refused`: a turn whose
/// `protocol.yaml` is no longer the embedded document (one action more, as a case opened by an
/// older binary on a protocol file could hold) is refused before any step when resumed, naming
/// the embedded protocol.
#[test]
fn a_resumed_turn_on_another_protocol_document_is_refused() {
    let world = world("resume-document", true, false);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 4, "{out}\n{err}");
    let path = world.repo.join(".takt/cases/turn-1/protocol.yaml");
    let text = fs::read_to_string(&path).expect("protocol.yaml");
    fs::write(
        &path,
        text.replacen(
            "\nactions:\n",
            "\nactions:\n  turn.note:\n    description: a note\n    effect: write\n",
            1,
        ),
    )
    .expect("written");
    let before = records(&world.repo).len();
    let (code, out, err) = resume(&world);
    assert_ne!(code, 0, "{out}\n{err}");
    assert!(err.contains("embeds"), "{err}");
    assert_eq!(records(&world.repo).len(), before, "{out}\n{err}");
}

/// `takt` run in `world` with `args` alone, the run's state and cache given, and `PATH` set to
/// `path` when some, with a `b10x-gates` policy file the stand-ins never read
/// ([`GATES_POLICY`]); no policy otherwise.
fn takt_on_path(world: &World, args: &[&str], path: Option<&Path>) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
    command
        .args(args)
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env("CARGO_INSTALL_ROOT", decoy(world))
        .env_remove("B10X_GATES_POLICY");
    if let Some(path) = path {
        command
            .env("PATH", path)
            .env("B10X_GATES_POLICY", GATES_POLICY);
    }
    let output = command.output().expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A turn opened by `takt run` under `--forge none` with a commit identity, so that a resume
/// through the bot route is not refused for want of one, stopped at its merge's grant.
fn turn_opened_under_forge_none(name: &str) -> World {
    let world = world(name, true, false);
    let mut args: Vec<String> = [
        "run",
        "--gate",
        "repository.merge",
        "--intent",
        "the probe no longer shows the defect",
        "--repo",
        world.repo.to_str().expect("utf-8"),
        "--protocol",
        PROTOCOL,
        "--install-root",
        world.install.to_str().expect("utf-8"),
        "--case-id",
        "turn-1",
        "--harness",
        "none",
        "--forge",
        "none",
        "--commit-identity",
        "Probe <probe@example.com>",
        "--test-cmd",
        "true",
        "--iterations",
        "6",
        "--scenarios",
        world.scenarios.to_str().expect("utf-8"),
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    for input in opening(&world) {
        args.extend(["--input".to_owned(), input]);
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, out, err) = takt_on_path(&world, &args, None);
    assert_eq!(code, 4, "{out}\n{err}");
    let forge = fs::read_to_string(world.repo.join(".takt/cases/turn-1/forge"))
        .expect("the turn records its forge");
    assert_eq!(forge, "none\n");
    world
}

/// Every file under `dir` with its bytes, by path, and the repository's refs and `HEAD`: what a
/// refusal before anything is written leaves as it was.
fn state_of(repo: &Path) -> (Vec<(PathBuf, Vec<u8>)>, String, String) {
    fn walk(dir: &Path, into: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(dir).expect("the directory reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, into);
            } else {
                let bytes = fs::read(&path).expect("the file reads");
                into.push((path, bytes));
            }
        }
    }
    let mut files = Vec::new();
    walk(&repo.join(".takt/cases"), &mut files);
    files.sort();
    (
        files,
        git(repo, &["for-each-ref"]),
        git(repo, &["rev-parse", "--symbolic-full-name", "HEAD"]),
    )
}

/// A `PATH` holding `git` and a `b10x-gates` that refuses every line, so the bot route passes
/// its check before the case opens (story:forge-preflight) and a run that went on through it
/// could reach no forge.
fn git_only_path(world: &World) -> PathBuf {
    let bin = world.repo.parent().expect("a parent").join("git-only-bin");
    fs::create_dir_all(&bin).expect("the directory");
    executable(
        &bin.join("b10x-gates"),
        "#!/bin/sh\necho \"the route is down\" >&2\nexit 1\n",
    );
    let git = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("sh runs");
    let git = String::from_utf8_lossy(&git.stdout).trim().to_owned();
    std::os::unix::fs::symlink(&git, bin.join("git")).expect("the link");
    bin
}

/// Review finding N1 of wave 16: a turn opened under `--forge none` records `none`, and `takt
/// approve` reads the mainline through it; resumed by default through the bot route (`origin` on
/// github.com), the merge validators would read another `main`. The resume is refused before
/// anything is written, naming both forges and the way past, `--forge none`.
#[test]
fn a_turn_resumed_through_another_forge_than_it_opened_with_is_refused() {
    let world = turn_opened_under_forge_none("forge-switch");
    git(
        &world.repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example/turn.git",
        ],
    );
    let before = state_of(&world.repo);
    let path = git_only_path(&world);
    let repo = world.repo.to_str().expect("utf-8");
    let scenarios = world.scenarios.to_str().expect("utf-8");
    let (code, out, err) = takt_on_path(
        &world,
        &[
            "run",
            "--case",
            "turn-1",
            "--repo",
            repo,
            "--harness",
            "none",
            "--test-cmd",
            "true",
            "--scenarios",
            scenarios,
            "--hours",
            "0.001",
        ],
        Some(&path),
    );
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("`none`"), "{err}");
    assert!(err.contains("`bot`"), "{err}");
    assert!(err.contains("--forge none"), "{err}");
    assert!(err.contains("nothing was resumed"), "{err}");
    assert!(!out.contains("stopped: "), "a step was taken: {out}\n{err}");
    assert!(state_of(&world.repo) == before, "{out}\n{err}");
}

/// The reverse of N1: a turn that recorded the bot route, resumed under `--forge none`, would
/// read the local `main` in its merge validators while `takt approve` reads the forge's. Refused
/// before anything is written, naming the way past, `--forge bot`.
#[test]
fn a_turn_recorded_with_the_bot_route_resumed_under_forge_none_is_refused() {
    let world = turn_opened_under_forge_none("forge-switch-back");
    fs::write(world.repo.join(".takt/cases/turn-1/forge"), "bot\n").expect("written");
    let before = state_of(&world.repo);
    let (code, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("`bot`"), "{err}");
    assert!(err.contains("`none`"), "{err}");
    assert!(err.contains("--forge bot"), "{err}");
    assert!(!out.contains("stopped: "), "a step was taken: {out}\n{err}");
    assert!(state_of(&world.repo) == before, "{out}\n{err}");
}

/// A turn that recorded no forge, opened before turns recorded one, resumes as it did: no
/// refusal for its forge.
#[test]
fn a_turn_that_recorded_no_forge_resumes_under_any() {
    let world = turn_opened_under_forge_none("forge-unrecorded");
    fs::remove_file(world.repo.join(".takt/cases/turn-1/forge")).expect("removed");
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert!(!err.contains("recorded the forge"), "{err}");
    assert!(out.contains("stopped: "), "{out}\n{err}");
}

/// A stand-in for the bot route, `b10x-gates` on `PATH`: its dry-run fetch of `main` answers,
/// per repository (`acme/one`, `acme/two`), the commit named by `tip-<repo>` beside it, and every
/// other line (the object fetch, the identity's scan) succeeds, the commits being already local.
/// The `PATH` that finds it first.
fn bot_stand_in(world: &World) -> PathBuf {
    let bin = world.repo.parent().expect("a parent").join("bot-bin");
    executable(
        &bin.join("b10x-gates"),
        "#!/bin/sh\nu=$(printf '%s\\n' \"$@\" | grep '^https://' | head -n 1)\ncase \"$*\" in *--dry-run*) printf '* %s %s refs/remotes/origin/main\\n' 0000000000000000000000000000000000000000 \"$(cat \"$(dirname \"$0\")/tip-$(basename \"$u\" .git)\")\";; esac\nexit 0\n",
    );
    write(&bin.join("tip-one"), &format!("{}\n", world.baseline));
    PathBuf::from(format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").expect("a PATH")
    ))
}

/// Points the stand-in's `acme/two` at `tip`.
fn tip_of_two(path: &Path, tip: &str) {
    let path = path.to_str().expect("utf-8");
    let bin = Path::new(path.split(':').next().expect("the stand-in's directory"));
    write(&bin.join("tip-two"), &format!("{tip}\n"));
}

/// A turn whose candidate touches a judge surface, opened by `takt run` through the bot route on
/// `origin` `https://github.com/acme/one.git`, at the gate of its judge-surface merge; and the
/// `PATH` of the bot route's stand-in ([`bot_stand_in`]), `acme/one`'s `main` at the baseline.
fn bot_turn_at_the_gate(name: &str) -> (World, PathBuf) {
    let world = world_with(
        name,
        &Setup {
            touches: Some(("tools/judge.sh", "#!/bin/sh\nexit 0\n")),
            ..Setup::plain(true, false)
        },
    );
    git(
        &world.repo,
        &["remote", "add", "origin", "https://github.com/acme/one.git"],
    );
    let path = bot_stand_in(&world);
    let finding = format!("finding={FINDING}");
    let candidate = format!("candidate={}", world.candidate);
    let (code, out, err) = takt_on_path(
        &world,
        &[
            "run",
            "--intent",
            "the probe no longer shows the defect",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--protocol",
            PROTOCOL,
            "--install-root",
            world.install.to_str().expect("utf-8"),
            "--case-id",
            "turn-1",
            "--harness",
            "none",
            "--forge",
            "bot",
            "--commit-identity",
            "Probe <probe@example.com>",
            "--test-cmd",
            "true",
            "--iterations",
            "6",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
            "--input",
            &finding,
            "--input",
            &candidate,
        ],
        Some(&path),
    );
    assert_eq!(code, 4, "{out}\n{err}");
    assert!(
        out.contains("candidate.merge_judge_surface"),
        "{out}\n{err}"
    );
    (world, path)
}

/// `takt run --case turn-1` through the bot route, on `PATH` `path`.
fn bot_resume(world: &World, path: &Path) -> (i32, String, String) {
    takt_on_path(
        world,
        &[
            "run",
            "--case",
            "turn-1",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--forge",
            "bot",
            "--test-cmd",
            "true",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
            "--hours",
            "0.001",
        ],
        Some(path),
    )
}

/// The owner's `takt approve` of the turn's judge-surface merge, on `PATH` `path`.
fn bot_approve(world: &World, path: &Path) -> (i32, String, String) {
    takt_on_path(
        world,
        &[
            "approve",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "turn-1",
            "repository.merge.judge_surface",
            "--by",
            "human:owner",
        ],
        Some(path),
    )
}

/// The names of the files in the turn's evidence directory, sorted.
fn evidence_files(world: &World) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(world.repo.join(".takt/cases/turn-1/evidence"))
        .expect("the evidence directory reads")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// story:a-turn-pins-the-repository-of-its-forge, acceptance 1: a turn opened through the bot
/// route on `origin` `acme/one` records `acme/one` beside `bot` in its `forge` file, and the
/// owner's judge-surface grant records `acme/one` beside the tip it read in `authority.yaml`.
#[test]
fn a_turn_through_the_bot_route_records_its_repository_and_its_grant_the_same() {
    let (world, path) = bot_turn_at_the_gate("pin-recorded");
    let case = world.repo.join(".takt/cases/turn-1");
    let forge = fs::read_to_string(case.join("forge")).expect("the turn records its forge");
    assert_eq!(forge, "bot acme/one\n");
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 0, "{out}\n{err}");
    let authority = fs::read_to_string(case.join("authority.yaml")).expect("the authority");
    assert!(
        authority.contains(&format!("mainline_tip: {}", world.baseline)),
        "{authority}"
    );
    assert!(
        authority.contains("mainline_repository: acme/one"),
        "{authority}"
    );
}

/// Acceptance 2, the resume: with `origin` moved to `acme/two` after the open, `takt run --case`
/// through the bot route exits 1 naming both repositories, and leaves the cases directory, the
/// refs and `HEAD` as they were.
#[test]
fn a_turn_resumed_on_another_repository_than_it_opened_on_is_refused() {
    let (world, path) = bot_turn_at_the_gate("pin-resume");
    git(
        &world.repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/two.git",
        ],
    );
    tip_of_two(&path, &world.baseline);
    let before = state_of(&world.repo);
    let (code, out, err) = bot_resume(&world, &path);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("acme/one"), "{err}");
    assert!(err.contains("acme/two"), "{err}");
    assert!(!out.contains("stopped: "), "a step was taken: {out}\n{err}");
    assert!(state_of(&world.repo) == before, "{out}\n{err}");
}

/// Acceptance 2, the grant: with `origin` moved to `acme/two` after the open, `takt approve` of
/// the judge-surface capability exits 1 naming both repositories, and leaves the cases
/// directory, the refs and `HEAD` as they were.
#[test]
fn a_grant_on_another_repository_than_the_turn_opened_on_is_refused() {
    let (world, path) = bot_turn_at_the_gate("pin-grant");
    git(
        &world.repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/two.git",
        ],
    );
    tip_of_two(&path, &world.baseline);
    let before = state_of(&world.repo);
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("acme/one"), "{err}");
    assert!(err.contains("acme/two"), "{err}");
    assert!(state_of(&world.repo) == before, "{out}\n{err}");
}

/// Acceptance 3 through the bot route: the grant given on `acme/one`, the `forge` file then
/// edited to `bot acme/two` and `origin` moved there, whose `main` holds the candidate by a merge
/// made after the grant. The next run records no `merged`, its summary names both
/// repositories, and the evidence directory holds no new file.
#[test]
fn a_merge_read_from_another_repository_than_the_grants_records_nothing() {
    let (world, path) = bot_turn_at_the_gate("pin-merge-bot");
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 0, "{out}\n{err}");
    let case = world.repo.join(".takt/cases/turn-1");
    fs::write(case.join("forge"), "bot acme/two\n").expect("written");
    git(
        &world.repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/two.git",
        ],
    );
    let tree = git(
        &world.repo,
        &["rev-parse", &format!("{}^{{tree}}", world.candidate)],
    );
    let merge = git(
        &world.repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            &world.baseline,
            "-p",
            &world.candidate,
            "-m",
            "the candidate, merged on acme/two after the grant",
        ],
    );
    tip_of_two(&path, &merge);
    let before = evidence_files(&world);
    let (_, out, err) = bot_resume(&world, &path);
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "merge_observation"),
        "merged recorded from acme/two under a grant read from acme/one: {out}\n{err}"
    );
    assert!(out.contains("acme/one"), "{out}\n{err}");
    assert!(out.contains("acme/two"), "{out}\n{err}");
    assert_eq!(evidence_files(&world), before, "{out}\n{err}");
}

/// Acceptance 3 under no forge: the grant given on `acme/one`, the `forge` file then edited to
/// `none` and the local `main` moved to the candidate. The next run, under `--forge none`,
/// records no `merged`, its summary names the grant's repository and the local `main`, and the
/// evidence directory holds no new file.
#[test]
fn a_merge_read_from_the_local_main_under_a_grant_read_from_the_bot_route_records_nothing() {
    let (world, path) = bot_turn_at_the_gate("pin-merge-none");
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 0, "{out}\n{err}");
    let case = world.repo.join(".takt/cases/turn-1");
    fs::write(case.join("forge"), "none\n").expect("written");
    git(&world.repo, &["branch", "-f", "main", &world.candidate]);
    let before = evidence_files(&world);
    let (_, out, err) = takt_in(&world, &["run", "--case", "turn-1", "--hours", "0.001"]);
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "merge_observation"),
        "merged recorded from the local main under a grant read from acme/one: {out}\n{err}"
    );
    assert!(out.contains("acme/one"), "{out}\n{err}");
    assert!(out.contains("the local main"), "{out}\n{err}");
    assert_eq!(evidence_files(&world), before, "{out}\n{err}");
}

/// A judge-surface grant that records its repository is not given again at the same revision:
/// the second `takt approve` exits 1 saying the case already decides it, and `authority.yaml`
/// is left as it was. Only a grant that records no repository is replaced, by one that does.
#[test]
fn a_grant_that_records_its_repository_is_not_given_again_at_its_revision() {
    let (world, path) = bot_turn_at_the_gate("pin-regrant-pinned");
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 0, "{out}\n{err}");
    let authority = world.repo.join(".takt/cases/turn-1/authority.yaml");
    let granted = fs::read_to_string(&authority).expect("the authority");
    assert!(
        granted.contains("mainline_repository: acme/one"),
        "{granted}"
    );
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(
        err.contains("already decides repository.merge.judge_surface"),
        "{out}\n{err}"
    );
    assert_eq!(
        fs::read_to_string(&authority).expect("the authority"),
        granted
    );
}

/// A judge-surface grant that records no repository is replaced only by a grant that reads one:
/// with the turn's `forge` file then `none`, the local `main` and no repository, the second
/// `takt approve` exits 1 saying the case already decides it, and `authority.yaml` is left as
/// it was.
#[test]
fn a_grant_that_records_no_repository_is_not_replaced_by_one_that_reads_none() {
    let (world, path) = bot_turn_at_the_gate("pin-regrant-none");
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 0, "{out}\n{err}");
    let case = world.repo.join(".takt/cases/turn-1");
    let authority = case.join("authority.yaml");
    let legacy: String = fs::read_to_string(&authority)
        .expect("the authority")
        .lines()
        .filter(|line| !line.trim_start().starts_with("mainline_repository:"))
        .map(|line| format!("{line}\n"))
        .collect();
    fs::write(&authority, &legacy).expect("written");
    fs::write(case.join("forge"), "none\n").expect("written");
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(
        err.contains("already decides repository.merge.judge_surface"),
        "{out}\n{err}"
    );
    assert_eq!(
        fs::read_to_string(&authority).expect("the authority"),
        legacy
    );
}
