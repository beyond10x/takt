//! Second adversarial pass of wave 40's fix round (U3): the replacement of a judge-surface grant
//! that records no repository (`prepare_grant`, crates/takt/src/case.rs). The helpers below are
//! copied from `self_improvement_cli.rs`; the case is at the end of the file. The cases whose
//! grant read a named repository's `main` went with the forge that read one: no forge takt has
//! names a repository, and a grant recording one is read in the library's own tests.

#![allow(dead_code)]

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The self-improvement protocol, the engineering protocols' built-in.
const PROTOCOL: &str = "self.improvement/1";

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
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// `takt` run in `world` with `args` alone, the run's state and cache given.
fn takt_alone(world: &World, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env("CARGO_INSTALL_ROOT", decoy(world))
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The turn's `authority.yaml`.
fn authority_of(world: &World) -> PathBuf {
    world.repo.join(".takt/cases/turn-1/authority.yaml")
}

/// `takt approve` of `capability` on the turn by `by`.
fn approve_as(world: &World, capability: &str, by: &str) -> (i32, String, String) {
    takt_alone(
        world,
        &[
            "approve",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "turn-1",
            capability,
            "--by",
            by,
        ],
    )
}

/// Writes `written` as the turn's `authority.yaml`; the next approve of `capability` must be
/// refused with "already decides" and leave the file as written.
fn refused_and_untouched(world: &World, capability: &str, written: &str) {
    fs::write(authority_of(world), written).expect("written");
    let (code, out, err) = approve_as(world, capability, "human:owner");
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("already decides"), "{out}\n{err}");
    assert_eq!(
        fs::read_to_string(authority_of(world)).expect("the authority"),
        written
    );
}

/// A turn's grant of another capability (`repository.merge`, a candidate that touches no judge
/// surface) is not replaced, even with a tip and no repository written beside it.
#[test]
fn w40_adv2_a_non_judge_surface_grant_is_not_replaced() {
    let world = world("adv2-non-judge", true, false);
    let (code, out, err) = run(&world, &opening(&world));
    assert_eq!(code, 4, "{out}\n{err}");
    assert!(out.contains("candidate.merge"), "{out}\n{err}");
    let (code, out, err) = approve_as(&world, "repository.merge", "human:owner");
    assert_eq!(code, 0, "{out}\n{err}");
    let granted = fs::read_to_string(authority_of(&world)).expect("the authority");
    assert!(!granted.contains("mainline_tip"), "{granted}");
    let tipped = format!("{granted}  mainline_tip: {}\n", world.baseline);
    refused_and_untouched(&world, "repository.merge", &tipped);
}
