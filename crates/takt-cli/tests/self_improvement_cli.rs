//! `takt run` on `self.improvement/1` (story:self-improvement-protocol, part 1): a turn opens
//! with `--intent` and the inputs `finding=<story id>` and `candidate=<full commit id>`, taken out
//! of the inputs as `loop_baseline` is; `story.yaml` names the finding, the `finding` artifact is
//! the case's base commit, the baseline is the latest promoted revision of the promotion store,
//! and every validator of the turn is declared independent in `case.yaml` by the kernel. The
//! turn's validators then drive the case on their own records: `finding.file`, then
//! `scenario.reproduce` at the baseline, then `scenario.verify` at the candidate, each through
//! `scripts/check-finding` as the base holds it. A finding that never reproduced at the baseline
//! ends `unreproducible`; a candidate that still reproduces it ends `declined`.
//!
//! Every path the run reads or writes outside the repository is this test's own: the promotion
//! store under `XDG_STATE_HOME`, `check-finding`'s cache under `XDG_CACHE_HOME`, in which each
//! revision's build is a stand-in script, so nothing is compiled; the scenario repository is a
//! local checkout.

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

/// The stand-in of a takt build that shows the defect, or that does not.
fn build(defect: bool) -> String {
    if defect {
        format!("#!/bin/sh\necho '{DEFECT}'\nexit 1\n")
    } else {
        "#!/bin/sh\necho 'all is well here'\nexit 0\n".to_owned()
    }
}

/// The confined `scripts/check-finding` a promoted baseline holds: the script at
/// `takt::improvement::CONFINED_CHECK_FINDING_COMMIT` when this repository's object store has
/// that commit, else this tree's own (a shallow checkout holds the commit's script in its tree
/// once that commit is merged). Read through git, nothing written.
fn confined_check_finding() -> String {
    let output = Command::new("git")
        .args([
            "show",
            &format!(
                "{}:scripts/check-finding",
                takt::improvement::CONFINED_CHECK_FINDING_COMMIT
            ),
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git runs");
    if output.status.success() {
        return String::from_utf8(output.stdout).expect("a UTF-8 script");
    }
    fs::read_to_string(Path::new(SCRIPTS).join("check-finding")).expect("the script")
}

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
/// the binary's own.
#[derive(Clone, PartialEq, Eq)]
enum Promoted {
    Stated(Option<String>),
    Named,
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
    promoted: Promoted,
}

impl Setup {
    fn plain(at_baseline: bool, at_candidate: bool) -> Self {
        Self {
            at_baseline,
            at_candidate,
            head: Head::Main,
            touches: None,
            unconfined: false,
            promoted: Promoted::Stated(None),
        }
    }
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
        let mut text = if script == "check-finding" {
            confined_check_finding()
        } else {
            fs::read_to_string(Path::new(SCRIPTS).join(script)).expect("the script")
        };
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
        &repo.join(".engineering/planning/story/probe-shows-the-defect.md"),
        &format!(
            "# The probe shows the defect\n\nThe probe prints the defect.\n\n```expected_before\nscenario: 1\ncommand: \"$TAKT\" probe\nexit: 1\noutput: {DEFECT}\nsource: docs/runs/trial-1.md\n```\n"
        ),
    );
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "base"]);
    let baseline = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "-q", "-b", "fix"]);
    write(&repo.join("fix.txt"), "the fix\n");
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
    let scorecard = format!(
        "{{\"format\":\"takt-scorecard/1\",\"revision\":\"{baseline}\",\"named_by\":\"build\",\"metrics\":{{}},\"rows\":[],\"no_record\":[]}}"
    );
    write(
        &store.join(format!("{baseline}.scorecard.json")),
        &scorecard,
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
    };
    write(
        &store.join(format!("{baseline}.json")),
        &format!(
            "{{\"format\":\"takt-promotion/1\",\"revision\":\"{baseline}\",\"tree\":\"{tree}\",\"promoted_at\":\"2026-01-01T00:00:00Z\",\"kind\":\"baseline\",\"scorecard\":\"{baseline}\",\"scorecard_file\":\"/srv/scorecards/scorecard-{baseline}.json\",\"scorecard_sha256\":\"{digest}\",\"against\":null,\"rule\":{{\"trials\":5,\"margin\":1}},\"worse\":0,{stated}\"previous_sha256\":null}}"
        ),
    );

    let cache = dir.join("cache");
    for (revision, defect) in [(&baseline, at_baseline), (&candidate, at_candidate)] {
        let built = cache.join("takt/check-finding/builds").join(revision);
        executable(&built.join("takt"), &build(defect));
        write(&built.join("revision"), &format!("{revision}\n"));
    }
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
    let mut args: Vec<String> = [
        "run",
        "--gate",
        gate,
        "--repo",
        world.repo.to_str().expect("utf-8"),
        "--protocol",
        PROTOCOL,
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
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    for input in inputs {
        args.extend(["--input".to_owned(), input.clone()]);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
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
        Some(world.baseline.as_str())
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

/// A candidate that changes the finding's file or `scripts/file-finding` against the baseline is
/// refused before anything is written, naming the path: what judges the candidate is never the
/// candidate's.
#[test]
fn a_candidate_that_changes_the_finding_or_its_scripts_is_refused() {
    for (n, path) in [FINDING_FILE, "scripts/file-finding"]
        .into_iter()
        .enumerate()
    {
        let world = world_with(
            &format!("touches-{n}"),
            &Setup {
                touches: Some((path, "#!/bin/sh\necho changed by the candidate\n")),
                ..Setup::plain(true, true)
            },
        );
        let ran = run(&world, &opening(&world));
        refused(&world, &ran, path);
    }
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
