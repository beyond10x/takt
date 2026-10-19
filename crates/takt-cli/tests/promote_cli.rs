//! story:promotion through the binary: `takt promote` installs a takt built from a revision only
//! when its saved scorecard shows no scenario worse than the promoted baseline's under the
//! scorecard's rule (a margin of one case at any outcome rank, the median of tokens within the
//! baseline's maximum), one promotion a UTC day, and writes a record under
//! `$XDG_STATE_HOME/takt/promotions/` naming the revision the installed `takt --version`
//! prints; a worse candidate is refused, recorded and not judged again; `--baseline-only`
//! records the first baseline and installs nothing; `--dry-run` writes and installs nothing.
//! The build runs confined in bubblewrap, so nothing the commit runs reads a setting outside
//! it, reaches the store or outlives the build; the record is written before the binary moves
//! into place and a failure after the move puts the binary before it back; the store is locked
//! while read and written; each record carries the digest of the one before it; and the
//! installed binary must name the latest record before anything is judged.
//!
//! The candidate is a scratch takt workspace with no dependencies, whose `build.rs` embeds the
//! checkout's revision as the real one does, so a build takes seconds; its scorecards are made
//! with the real binary's `takt scorecard --save` over scratch case records. The binary
//! installed before a promotion is a stand-in script printing what `takt --version` would.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use takt::case::CASES_DIR;
use takt::events::{Event, Events, TokenUsage};
use takt::promotion::{INSIDE_INSTALL, INSIDE_TARGET, LOCK};

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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("promote-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

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

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

/// Writes an executable script.
fn script(path: &Path, text: &str) {
    write(path, text);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// A stand-in for the installed takt of `revision`: prints what `takt --version` would.
fn stand_in(revision: &str) -> String {
    format!("#!/bin/sh\necho \"takt 0.1.0 ({revision})\"\n")
}

/// A stand-in for a takt built before `--version` named its revision: prints `takt 0.1.0`.
fn pre_revision_stand_in() -> String {
    "#!/bin/sh\necho \"takt 0.1.0\"\n".to_owned()
}

/// A sha256 no file of a world hashes to.
const OTHER_DIGEST: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// The scratch takt's `build.rs`: the checkout's revision into `TAKT_REVISION`, `-dirty` when
/// a tracked file differs, `unknown revision` outside a checkout.
const TAKT_BUILD: &str = r#"use std::process::Command;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let revision = match git(&["rev-parse", "--verify", "-q", "HEAD"]) {
        Some(sha) if !sha.is_empty() => {
            let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
                .is_some_and(|status| !status.is_empty());
            if dirty { format!("{sha}-dirty") } else { sha }
        }
        _ => "unknown revision".to_owned(),
    };
    println!("cargo:rustc-env=TAKT_REVISION={revision}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
"#;

/// The scratch takt's `main.rs`: `--version` prints `takt 0.1.0 (<revision>)`; `--probe`
/// prints whether debug assertions were compiled in.
const TAKT_MAIN: &str = r#"fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("takt 0.1.0 ({})", env!("TAKT_REVISION"));
    }
    if args.iter().any(|arg| arg == "--probe") {
        println!("debug_assertions={}", cfg!(debug_assertions));
    }
}
"#;

/// A `main.rs` that names its revision while it runs from the build's or the fresh root's
/// directory and another once it sits anywhere else, the install root included.
const FLIPPING_MAIN: &str = r#"fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        let exe = std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        if exe.starts_with("__TARGET__/") || exe.starts_with("__INSTALL__/") {
            println!("takt 0.1.0 ({})", env!("TAKT_REVISION"));
        } else {
            println!("takt 0.1.0 (something-else)");
        }
    }
}
"#;

/// What a planting candidate does whenever its code runs during a promotion (its `build.rs`,
/// its `--version`): writes a record into the store by its host path and leaves a sleeper that
/// would write there later, every error ignored so the build itself succeeds.
const PLANT: &str = r#"
fn plant() {
    let store = std::path::Path::new("__STORE__");
    let _ = std::fs::create_dir_all(store);
    let _ = std::fs::write(
        store.join("ffffffffffffffffffffffffffffffffffffffff.json"),
        "planted by the candidate\n",
    );
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg("sleep 20; echo late > '__STORE__/late.txt' # __MARKER__")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
"#;

/// A committed scratch takt workspace; its HEAD is the candidate revision.
fn candidate(at: &Path) -> String {
    write(
        &at.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n",
    );
    write(
        &at.join("crates/takt-cli/Cargo.toml"),
        "[package]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"takt\"\npath = \"src/main.rs\"\n",
    );
    write(
        &at.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\n",
    );
    write(&at.join(".gitignore"), "/target\n");
    write(&at.join("crates/takt-cli/build.rs"), TAKT_BUILD);
    write(&at.join("crates/takt-cli/src/main.rs"), TAKT_MAIN);
    git(at, &["init", "-q", "-b", "main"]);
    git(at, &["add", "-A"]);
    git(at, &["commit", "-q", "-m", "candidate"]);
    git(at, &["rev-parse", "HEAD"])
}

/// One run of `case` in `repo`: a judgement step that reported `tokens`, then `stop`.
fn run(repo: &Path, case: &str, tokens: u64, stop: &str) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    let events = Events::new(&dir);
    let usage = TokenUsage {
        input: tokens,
        output: 0,
        cache_read: 0,
        cache_creation: 0,
    };
    for (step, event) in [
        (
            0,
            Event::Opened {
                case: case.to_owned(),
                protocol: "story.delivery/1".to_owned(),
                story: None,
                inputs: Vec::new(),
                base: None,
                binary: None,
            },
        ),
        (
            1,
            Event::Step {
                by: "harness".to_owned(),
                name: "claude-code".to_owned(),
                action: "implementation.edit".to_owned(),
                summary: "edited".to_owned(),
                tokens: Some(usage.total()),
                usage: Some(usage),
                revision: None,
                model: None,
                cost_usd: None,
                duration_ms: None,
                api_ms: None,
                turns: None,
                started_at: None,
            },
        ),
        (
            1,
            Event::Stop {
                stop: stop.to_owned(),
                steps: 1,
                tokens,
            },
        ),
    ] {
        events.emit(step, &event).expect("the event is written");
    }
}

/// Runs the built `takt` with `args` and `env`: the exit code, stdout and stderr.
fn takt(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .envs(env.iter().copied())
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

const APPROVAL: &str = "ApprovalRequired (repository.merge)";
const ERROR: &str = "error";

/// One world: the candidate checkout, a baseline scorecard saved under the baseline revision
/// (five scenario 1 cases at the approval with the given tokens) and a store and an install
/// root of its own.
struct World {
    outer: Scratch,
    repo: PathBuf,
    revision: String,
    saves: PathBuf,
    baseline: PathBuf,
    store: PathBuf,
    root: PathBuf,
}

const BASELINE_REVISION: &str = "a0b8576fe56aca1304e412eb62e5622762de198a";
const BASELINE_TREE: &str = "1111111111111111111111111111111111111111";

/// What the store holds once the baseline is recorded and nothing else: its record and its
/// scorecard copy, in name order.
fn baseline_files() -> Vec<String> {
    vec![
        format!("{BASELINE_REVISION}.json"),
        format!("{BASELINE_REVISION}.scorecard.json"),
    ]
}

/// The sha256 of the file at `path`, lower-case hex, by `sha256sum`.
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

/// The string value of `field` in a pretty-printed record.
fn field(record: &str, field: &str) -> String {
    let prefix = format!("\"{field}\": \"");
    record
        .lines()
        .find_map(|line| line.trim().strip_prefix(prefix.as_str()))
        .and_then(|rest| rest.strip_suffix('"').or_else(|| rest.strip_suffix("\",")))
        .unwrap_or_else(|| panic!("{field} in {record}"))
        .to_owned()
}

/// The process ids whose command line holds `marker`.
fn processes_holding(marker: &str) -> Vec<String> {
    let mut found = Vec::new();
    for entry in fs::read_dir("/proc").expect("/proc").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        if let Ok(bytes) = fs::read(entry.path().join("cmdline"))
            && String::from_utf8_lossy(&bytes).contains(marker)
        {
            found.push(name);
        }
    }
    found
}

impl World {
    fn new(name: &str, baseline_tokens: [u64; 5], baseline_stops: [&str; 5]) -> Self {
        let outer = scratch(name);
        let repo = outer.join("candidate");
        fs::create_dir_all(&repo).expect("a directory");
        let revision = candidate(&repo);
        let saves = outer.join("saves");
        let baseline_cases = outer.join("baseline-cases");
        fs::create_dir_all(&baseline_cases).expect("a directory");
        for (n, (tokens, stop)) in baseline_tokens.iter().zip(baseline_stops).enumerate() {
            run(
                &baseline_cases,
                &format!("scenario-1-trial-{}", n + 1),
                *tokens,
                stop,
            );
        }
        let baseline = Self::save(&baseline_cases, &saves, BASELINE_REVISION);
        let store = outer.join("state/takt/promotions");
        let root = outer.join("root");
        World {
            outer,
            repo,
            revision,
            saves,
            baseline,
            store,
            root,
        }
    }

    /// Saves the scorecard of `cases` under `revision` in `saves`; the file.
    fn save(cases: &Path, saves: &Path, revision: &str) -> PathBuf {
        let (code, out, err) = takt(
            &[
                "scorecard",
                "--repo",
                cases.to_str().expect("utf-8"),
                "--save",
                saves.to_str().expect("utf-8"),
                "--revision",
                revision,
            ],
            &[],
        );
        assert_eq!(code, 0, "{out}{err}");
        saves.join(format!("scorecard-{revision}.json"))
    }

    /// A scorecard of five scenario 1 cases with these tokens and stops, saved under `revision`
    /// in a directory of its own named by `label`, since a saved scorecard is never overwritten.
    fn card_for(&self, label: &str, revision: &str, tokens: [u64; 5], stops: [&str; 5]) -> PathBuf {
        let cases = self.outer.join(format!("cases-{label}"));
        fs::create_dir_all(&cases).expect("a directory");
        for (n, (tokens, stop)) in tokens.iter().zip(stops).enumerate() {
            run(
                &cases,
                &format!("scenario-1-trial-{}", n + 11),
                *tokens,
                stop,
            );
        }
        Self::save(&cases, &self.outer.join(format!("saves-{label}")), revision)
    }

    /// The candidate's scorecard: five scenario 1 cases with these tokens and stops, saved
    /// under the candidate revision.
    fn candidate_card(&self, label: &str, tokens: [u64; 5], stops: [&str; 5]) -> PathBuf {
        self.card_for(label, &self.revision, tokens, stops)
    }

    /// A scorecard the rule passes, saved under `revision`.
    fn passing_card(&self, label: &str, revision: &str) -> PathBuf {
        self.card_for(label, revision, BASELINE_TOKENS, FIVE_APPROVALS)
    }

    /// Commits everything in the checkout; the new HEAD.
    fn commit(&self, message: &str) -> String {
        git(&self.repo, &["add", "-A"]);
        git(&self.repo, &["commit", "-q", "-m", message]);
        git(&self.repo, &["rev-parse", "HEAD"])
    }

    /// The store and the build directory of this world, never the real ones.
    fn env(&self) -> Vec<(String, String)> {
        vec![
            (
                "XDG_STATE_HOME".to_owned(),
                self.outer.join("state").to_str().expect("utf-8").to_owned(),
            ),
            (
                "XDG_CACHE_HOME".to_owned(),
                self.outer.join("cache").to_str().expect("utf-8").to_owned(),
            ),
        ]
    }

    /// The arguments of `takt promote` for `repo` and `scorecard` in this world.
    fn args<'a>(&'a self, repo: &'a Path, scorecard: &'a Path) -> Vec<&'a str> {
        vec![
            "promote",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--scorecard",
            scorecard.to_str().expect("utf-8"),
            "--install-root",
            self.root.to_str().expect("utf-8"),
        ]
    }

    /// `takt promote` with the world's store and install root, `extra` arguments and
    /// environment added.
    fn promote_with(
        &self,
        scorecard: &Path,
        extra: &[&str],
        env: &[(&str, &str)],
    ) -> (i32, String, String) {
        let own = self.env();
        let mut full: Vec<(&str, &str)> =
            own.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        full.extend_from_slice(env);
        let mut args = self.args(&self.repo, scorecard);
        args.extend_from_slice(extra);
        takt(&args, &full)
    }

    /// `takt promote` with the world's store and install root, `extra` arguments added.
    fn promote(&self, scorecard: &Path, extra: &[&str]) -> (i32, String, String) {
        self.promote_with(scorecard, extra, &[])
    }

    /// Installs the stand-in for the takt of `revision` at `<root>/bin/takt`.
    fn install_stand_in(&self, revision: &str) {
        script(&self.installed(), &stand_in(revision));
    }

    /// Installs the stand-in for a binary that names no revision at `<root>/bin/takt`; its
    /// sha256.
    fn install_pre_revision(&self) -> String {
        script(&self.installed(), &pre_revision_stand_in());
        sha256(&self.installed())
    }

    /// A scorecard the rule passes, saved under the candidate revision, each of its five runs
    /// stating the binary `stated[n]` gives as a `binary.txt` beside its record: the first
    /// line `<path>\t<sha256>`, then the note; no file for `None`.
    fn stated_card(&self, label: &str, stated: [Option<(&str, &str)>; 5]) -> PathBuf {
        let cases = self.outer.join(format!("cases-{label}"));
        fs::create_dir_all(&cases).expect("a directory");
        for (n, (tokens, stop)) in BASELINE_TOKENS.iter().zip(FIVE_APPROVALS).enumerate() {
            let case = format!("scenario-1-trial-{}", n + 11);
            run(&cases, &case, *tokens, stop);
            if let Some((digest, note)) = stated[n] {
                write(
                    &cases.join(CASES_DIR).join(&case).join("binary.txt"),
                    &format!("~/.cargo/bin/takt\t{digest}\n{note}"),
                );
            }
        }
        Self::save(
            &cases,
            &self.outer.join(format!("saves-{label}")),
            &self.revision,
        )
    }

    /// Records the baseline: `--baseline-only` over the baseline scorecard, with the checkout
    /// standing in for the baseline revision's.
    fn record_baseline(&self) {
        self.record_baseline_of(&self.baseline);
    }

    /// Records `scorecard` as the baseline of [`BASELINE_REVISION`]: the record, written by
    /// hand in the record's own shape (the scratch checkout is at the candidate's revision, not
    /// the baseline's), the scorecard's copy beside it, and the stand-in for that revision's
    /// binary installed, as a promotion requires.
    fn record_baseline_of(&self, scorecard: &Path) {
        fs::create_dir_all(&self.store).expect("the store");
        let copy = self
            .store
            .join(format!("{BASELINE_REVISION}.scorecard.json"));
        fs::copy(scorecard, &copy).expect("the copy");
        let record = format!(
            "{{\n  \"format\": \"takt-promotion/1\",\n  \"revision\": \"{BASELINE_REVISION}\",\n  \"tree\": \"{BASELINE_TREE}\",\n  \"promoted_at\": \"2026-10-17T12:00:00Z\",\n  \"kind\": \"baseline\",\n  \"scorecard\": \"{BASELINE_REVISION}\",\n  \"scorecard_file\": \"{}\",\n  \"scorecard_sha256\": \"{}\",\n  \"against\": null,\n  \"rule\": {{\n    \"trials\": 5,\n    \"margin\": 1\n  }},\n  \"worse\": 0,\n  \"binary\": null,\n  \"binary_sha256\": null,\n  \"previous_sha256\": null\n}}\n",
            scorecard.display(),
            sha256(scorecard)
        );
        write(
            &self.store.join(format!("{BASELINE_REVISION}.json")),
            &record,
        );
        self.install_stand_in(BASELINE_REVISION);
    }

    /// The record of `revision` in the store, as text.
    fn record(&self, name: &str) -> String {
        fs::read_to_string(self.store.join(name)).expect("the record")
    }

    /// The store's files in name order, the lock aside.
    fn records(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.store)
            .map(|entries| {
                entries
                    .map(|entry| {
                        entry
                            .expect("an entry")
                            .file_name()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .filter(|name| name != LOCK)
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn installed(&self) -> PathBuf {
        self.root.join("bin/takt")
    }

    fn version(&self) -> String {
        let output = Command::new(self.installed())
            .arg("--version")
            .output()
            .expect("the installed takt runs");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// The installed binary is the baseline's stand-in, byte for byte.
    fn assert_untouched(&self) {
        assert_eq!(
            fs::read_to_string(self.installed()).expect("the installed binary"),
            stand_in(BASELINE_REVISION),
            "the installed binary is untouched"
        );
    }
}

const FIVE_APPROVALS: [&str; 5] = [APPROVAL; 5];
const BASELINE_TOKENS: [u64; 5] = [107_205, 122_192, 122_361, 122_811, 138_160];
const WITHIN_TOKENS: [u64; 5] = [110_000, 120_000, 125_000, 130_000, 135_000];
const TWO_WORSE: [&str; 5] = [APPROVAL, APPROVAL, APPROVAL, ERROR, ERROR];

/// `--baseline-only` writes a baseline record naming the scorecard and installs nothing; a
/// second one is refused.
#[test]
fn baseline_only_records_the_first_baseline_and_installs_nothing() {
    let world = World::new("baseline", BASELINE_TOKENS, FIVE_APPROVALS);
    // The candidate checkout is the "installed revision" here: its own scorecard is the
    // baseline's content under its own revision, and its stand-in is installed.
    world.install_stand_in(&world.revision);
    let card = world.candidate_card("base", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        world.records(),
        vec![
            format!("{}.json", world.revision),
            format!("{}.scorecard.json", world.revision)
        ]
    );
    let record = world.record(&format!("{}.json", world.revision));
    for line in [
        "\"kind\": \"baseline\"".to_owned(),
        format!("\"revision\": \"{}\"", world.revision),
        format!("\"scorecard\": \"{}\"", world.revision),
        "\"binary\": null".to_owned(),
        "\"previous_sha256\": null".to_owned(),
    ] {
        assert!(record.contains(&line), "{line} in {record}");
    }
    assert_eq!(
        fs::read_to_string(world.installed()).expect("the stand-in"),
        stand_in(&world.revision),
        "nothing is installed"
    );
    let (code, out, err) = world.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("recorded already") && err.contains("as a baseline"),
        "{err}"
    );
}

/// `--baseline-only` records the scorecard of the installed revision: with no binary at
/// `<root>/bin/takt`, or one naming another revision, it is refused naming them (probe N14).
#[test]
fn baseline_only_requires_the_installed_binary_to_name_the_revision() {
    let world = World::new("baseline-binary", BASELINE_TOKENS, FIVE_APPROVALS);
    let card = world.candidate_card("base", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("no binary is installed") && err.contains("bin/takt"),
        "{err}"
    );
    assert_eq!(world.records(), Vec::<String>::new());
    world.install_stand_in(BASELINE_REVISION);
    let (code, out, err) = world.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!(
            "names {BASELINE_REVISION}, not {}",
            world.revision
        )),
        "{err}"
    );
    assert_eq!(world.records(), Vec::<String>::new());
    world.install_stand_in(&world.revision);
    let (code, out, err) = world.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 0, "{out}{err}");
}

/// `--baseline-only --revision <sha> --binary-sha256 <hex>` records a baseline for a binary
/// built before `--version` named its revision: the installed binary names none and hashes to
/// `<hex>`, every run of the scorecard states `<hex>` in a `binary.txt` beside its record, and
/// the record names the revision and the digest, marks both as the operator's statement and
/// carries each run's statement with its note, verbatim, as the scorecard copy does; nothing
/// is installed. A dry run prints the record and writes nothing.
#[test]
fn a_stated_digest_records_a_baseline_for_a_binary_naming_no_revision() {
    let world = World::new("stated", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let note = "built from the clone of 2026-10-17\nkept beside the run\n";
    let stated = Some((digest.as_str(), ""));
    let card = world.stated_card(
        "stated",
        [
            stated,
            Some((digest.as_str(), note)),
            stated,
            stated,
            stated,
        ],
    );
    let flags = [
        "--baseline-only",
        "--revision",
        world.revision.as_str(),
        "--binary-sha256",
        digest.as_str(),
    ];
    let mut dry = flags.to_vec();
    dry.push("--dry-run");
    let (code, out, err) = world.promote(&card, &dry);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("\"revision_stated_by\": \"operator\""),
        "{out}"
    );
    assert_eq!(
        world.records(),
        Vec::<String>::new(),
        "a dry run writes nothing"
    );
    let (code, out, err) = world.promote(&card, &flags);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        world.records(),
        vec![
            format!("{}.json", world.revision),
            format!("{}.scorecard.json", world.revision)
        ]
    );
    let quoted_note = "\"note\": \"built from the clone of 2026-10-17\\nkept beside the run\\n\"";
    let record = world.record(&format!("{}.json", world.revision));
    for line in [
        "\"kind\": \"baseline\"".to_owned(),
        format!("\"revision\": \"{}\"", world.revision),
        "\"revision_stated_by\": \"operator\"".to_owned(),
        format!("\"binary\": \"{}\"", world.installed().display()),
        format!("\"binary_sha256\": \"{digest}\""),
        "\"case\": \"scenario-1-trial-12\"".to_owned(),
        quoted_note.to_owned(),
        "\"previous_sha256\": null".to_owned(),
    ] {
        assert!(record.contains(&line), "{line} in {record}");
    }
    assert_eq!(
        record.matches("\"stated_by\": \"operator\"").count(),
        5,
        "one statement a run: {record}"
    );
    let copy = world.record(&format!("{}.scorecard.json", world.revision));
    assert_eq!(
        copy.matches(&format!("\"sha256\": \"{digest}\"")).count(),
        5,
        "{copy}"
    );
    assert!(copy.contains(quoted_note), "{copy}");
    assert_eq!(
        fs::read_to_string(world.installed()).expect("the stand-in"),
        pre_revision_stand_in(),
        "nothing is installed"
    );
}

/// A stated digest the installed binary does not hash to is refused naming both digests, and
/// nothing is written.
#[test]
fn a_stated_digest_the_installed_binary_does_not_hash_to_is_refused() {
    let world = World::new("stated-mismatch", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let card = world.stated_card("stated", [Some((OTHER_DIGEST, "")); 5]);
    let (code, out, err) = world.promote(
        &card,
        &[
            "--baseline-only",
            "--revision",
            world.revision.as_str(),
            "--binary-sha256",
            OTHER_DIGEST,
        ],
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!(
            "hashes to {digest}, not to the stated {OTHER_DIGEST}"
        )),
        "{err}"
    );
    assert_eq!(world.records(), Vec::<String>::new(), "nothing is written");
    assert_eq!(
        fs::read_to_string(world.installed()).expect("the stand-in"),
        pre_revision_stand_in()
    );
}

/// Without `--binary-sha256`, a binary that names no revision is refused as before, with or
/// without `--revision`: `--revision` alone states nothing the binary is not asked to name.
#[test]
fn without_a_stated_digest_a_binary_naming_no_revision_is_refused_as_before() {
    let world = World::new("stated-none", BASELINE_TOKENS, FIVE_APPROVALS);
    world.install_pre_revision();
    let card = world.candidate_card("base", BASELINE_TOKENS, FIVE_APPROVALS);
    for extra in [
        vec!["--baseline-only"],
        vec!["--baseline-only", "--revision", world.revision.as_str()],
    ] {
        let (code, out, err) = world.promote(&card, &extra);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("--version printed \"takt 0.1.0\", not `takt <version> (<revision>)`"),
            "{err}"
        );
        assert_eq!(world.records(), Vec::<String>::new());
    }
}

/// With `--binary-sha256`, a run of the scorecard that states no binary (no `binary.txt`
/// beside its record), or another digest, is refused naming the run; nothing is written.
#[test]
fn a_run_stating_no_binary_or_another_is_refused_naming_it() {
    let world = World::new("stated-runs", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let flags = [
        "--baseline-only",
        "--revision",
        world.revision.as_str(),
        "--binary-sha256",
        digest.as_str(),
    ];
    let stated = Some((digest.as_str(), ""));
    let missing = world.stated_card("missing", [stated, stated, None, stated, stated]);
    let (code, out, err) = world.promote(&missing, &flags);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("run scenario-1-trial-13#1 states no binary") && err.contains("binary.txt"),
        "{err}"
    );
    assert_eq!(world.records(), Vec::<String>::new());
    let other = world.stated_card(
        "other",
        [
            stated,
            stated,
            stated,
            stated,
            Some((OTHER_DIGEST, "another build\n")),
        ],
    );
    let (code, out, err) = world.promote(&other, &flags);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!(
            "run scenario-1-trial-15#1 states the binary {OTHER_DIGEST}, not the stated {digest}"
        )),
        "{err}"
    );
    assert_eq!(world.records(), Vec::<String>::new());
}

/// `--binary-sha256` is refused when the installed binary's `--version` names a revision: such
/// a binary is recorded by what it names, as before, and its record marks no statement.
#[test]
fn a_stated_digest_is_refused_when_the_binary_names_a_revision() {
    let world = World::new("stated-named", BASELINE_TOKENS, FIVE_APPROVALS);
    world.install_stand_in(&world.revision);
    let digest = sha256(&world.installed());
    let card = world.stated_card("named", [Some((digest.as_str(), "")); 5]);
    let (code, out, err) = world.promote(
        &card,
        &[
            "--baseline-only",
            "--revision",
            world.revision.as_str(),
            "--binary-sha256",
            digest.as_str(),
        ],
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("names {}", world.revision))
            && err.contains("--binary-sha256 are refused"),
        "{err}"
    );
    assert_eq!(world.records(), Vec::<String>::new());
    let (code, out, err) = world.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 0, "{out}{err}");
    let record = world.record(&format!("{}.json", world.revision));
    for line in [
        "\"revision_stated_by\": null",
        "\"binary_sha256\": null",
        "\"runs\": []",
    ] {
        assert!(record.contains(line), "{line} in {record}");
    }
}

/// `--revision` and `--binary-sha256` are refused outside `--baseline-only`, `--binary-sha256`
/// without `--revision`, a `--revision` that is no full commit id or not the scorecard's, and a
/// `--binary-sha256` that is no sha256; each names what is wrong and writes nothing.
#[test]
fn the_stated_flags_are_refused_outside_baseline_only_or_malformed() {
    let world = World::new("stated-flags", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let card = world.stated_card("flags", [Some((digest.as_str(), "")); 5]);
    let revision = world.revision.as_str();
    for (extra, named) in [
        (
            vec!["--revision", revision, "--binary-sha256", digest.as_str()],
            "with --baseline-only only",
        ),
        (
            vec!["--baseline-only", "--binary-sha256", digest.as_str()],
            "--binary-sha256 needs --revision",
        ),
        (
            vec![
                "--baseline-only",
                "--revision",
                BASELINE_REVISION,
                "--binary-sha256",
                digest.as_str(),
            ],
            "is not the scorecard's revision",
        ),
        (
            vec![
                "--baseline-only",
                "--revision",
                "abc123",
                "--binary-sha256",
                digest.as_str(),
            ],
            "is no full commit id",
        ),
        (
            vec![
                "--baseline-only",
                "--revision",
                revision,
                "--binary-sha256",
                "xyz",
            ],
            "is no sha256",
        ),
    ] {
        let (code, out, err) = world.promote(&card, &extra);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains(named), "{named} in {err}");
        assert_eq!(world.records(), Vec::<String>::new());
    }
}

/// With a stated baseline recorded, a candidate promotion checks the installed binary by its
/// digest: one that names no revision and hashes to the record's `binary_sha256` passes (a dry
/// run reaches the verdict), and a replaced one with another digest is refused naming both
/// digests; nothing is written either way.
#[test]
fn a_candidate_against_a_stated_baseline_checks_the_installed_binary_by_digest() {
    let world = World::new("stated-next", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let card = world.stated_card("stated", [Some((digest.as_str(), "")); 5]);
    let (code, out, err) = world.promote(
        &card,
        &[
            "--baseline-only",
            "--revision",
            world.revision.as_str(),
            "--binary-sha256",
            digest.as_str(),
        ],
    );
    assert_eq!(code, 0, "{out}{err}");
    let recorded = world.records();
    write(&world.repo.join("f.txt"), "f\n");
    let next = world.commit("f");
    let next_card = world.passing_card("next", &next);
    let (code, out, err) = world.promote(&next_card, &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("verdict: no scenario worse than the baseline"),
        "{out}"
    );
    assert_eq!(world.records(), recorded, "a dry run writes nothing");
    script(
        &world.installed(),
        "#!/bin/sh\necho \"takt 0.1.0\"\n# replaced by hand\n",
    );
    let replaced = sha256(&world.installed());
    assert_ne!(replaced, digest);
    let (code, out, err) = world.promote(&next_card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("hashes to {replaced}"))
            && err.contains(&format!("whose binary hashes to {digest}"))
            && err.contains("the binary was replaced"),
        "{err}"
    );
    assert_eq!(world.records(), recorded);
}

/// With `--binary-sha256`, a run of the saved scorecard whose statement is not the operator's
/// (`"stated_by": "record"`) is refused naming the run, whichever rows say it, so no stated
/// baseline carries a statement the operator did not make; nothing is written.
#[test]
fn a_run_stated_by_other_than_the_operator_is_refused_naming_it() {
    let world = World::new("stated-by", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let flags = [
        "--baseline-only",
        "--revision",
        world.revision.as_str(),
        "--binary-sha256",
        digest.as_str(),
    ];
    let operator = "\"stated_by\": \"operator\"";
    for (label, rows, named) in [
        ("third", vec![2], "scenario-1-trial-13#1"),
        ("all", vec![0, 1, 2, 3, 4], "scenario-1-trial-11#1"),
    ] {
        let card = world.stated_card(label, [Some((digest.as_str(), "")); 5]);
        let text = fs::read_to_string(&card).expect("the saved scorecard");
        let at: Vec<usize> = text.match_indices(operator).map(|(at, _)| at).collect();
        assert_eq!(at.len(), 5, "{text}");
        let mut edited = text.clone();
        for row in rows.iter().rev() {
            edited.replace_range(
                at[*row]..at[*row] + operator.len(),
                "\"stated_by\": \"record\"",
            );
        }
        fs::write(&card, &edited).expect("the edited scorecard");
        let (code, out, err) = world.promote(&card, &flags);
        assert_eq!(code, 1, "{label}: {out}{err}");
        assert!(
            err.contains(&format!("run {named} states its binary by the record")),
            "{label}: {err}"
        );
        assert_eq!(world.records(), Vec::<String>::new(), "{label}");
    }
}

/// A binary replaced by one that prints the stated baseline's revision under another digest is
/// refused by digest, naming both digests: a stated baseline is checked by its `binary_sha256`
/// whatever the installed binary prints; nothing is written.
#[test]
fn a_replaced_binary_printing_the_stated_revision_is_refused_by_digest() {
    let world = World::new("stated-replaced", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let card = world.stated_card("stated", [Some((digest.as_str(), "")); 5]);
    let (code, out, err) = world.promote(
        &card,
        &[
            "--baseline-only",
            "--revision",
            world.revision.as_str(),
            "--binary-sha256",
            digest.as_str(),
        ],
    );
    assert_eq!(code, 0, "{out}{err}");
    let recorded = world.records();
    write(&world.repo.join("f.txt"), "f\n");
    let next = world.commit("f");
    let next_card = world.passing_card("next", &next);
    world.install_stand_in(&world.revision);
    let replaced = sha256(&world.installed());
    assert_ne!(replaced, digest);
    let (code, out, err) = world.promote(&next_card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("hashes to {replaced}"))
            && err.contains(&format!("whose binary hashes to {digest}"))
            && err.contains("the binary was replaced"),
        "{err}"
    );
    assert_eq!(world.records(), recorded, "nothing is written");
}

/// A candidate within the rule (one case worse, tokens within the maximum) is built, installed
/// and recorded; the record's revision is the one the installed binary prints, and it carries
/// the baseline record's digest.
#[test]
fn a_candidate_within_the_rule_is_installed_and_recorded() {
    let world = World::new("promoted", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let baseline_digest = sha256(&world.store.join(format!("{BASELINE_REVISION}.json")));
    let card = world.candidate_card(
        "within",
        WITHIN_TOKENS,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, ERROR],
    );
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("rule  trials 5 a scenario  margin 1 case"),
        "the rule applied is printed: {out}"
    );
    assert!(
        out.contains("verdict: no scenario worse than the baseline"),
        "{out}"
    );
    assert!(out.contains("confined in bwrap"), "{out}");
    assert_eq!(
        world.version(),
        format!("takt 0.1.0 ({})", world.revision),
        "the installed binary names the candidate revision"
    );
    let record = world.record(&format!("{}.json", world.revision));
    for line in [
        "\"kind\": \"promotion\"".to_owned(),
        format!("\"revision\": \"{}\"", world.revision),
        format!("\"against\": \"{BASELINE_REVISION}\""),
        "\"worse\": 0".to_owned(),
        format!("\"binary\": \"{}\"", world.installed().display()),
        format!("\"previous_sha256\": \"{baseline_digest}\""),
    ] {
        assert!(record.contains(&line), "{line} in {record}");
    }
    assert_eq!(
        field(&record, "binary_sha256"),
        sha256(&world.installed()),
        "the record's digest is the installed file's"
    );
    assert!(
        fs::read_dir(&world.root)
            .expect("the root")
            .flatten()
            .all(|entry| entry.file_name() == "bin"),
        "the fresh root is removed"
    );
    assert!(
        fs::read_dir(world.root.join("bin"))
            .expect("bin")
            .flatten()
            .all(|entry| entry.file_name() == "takt"),
        "the binary before it is not kept beside the new one"
    );
    // A second promotion the same UTC day is refused, naming the first.
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("recorded already") || err.contains("this UTC day already"),
        "the first promotion is named: {err}"
    );
}

/// Two cases worse at a rank, or a median of tokens over the baseline's maximum, refuse the
/// candidate: nothing installed, a refusal recorded, and the same revision not judged again.
#[test]
fn a_worse_candidate_is_refused_recorded_and_not_judged_again() {
    let world = World::new("refused", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let two_worse = world.candidate_card("two-worse", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&two_worse, &[]);
    assert_eq!(code, 3, "{out}{err}");
    assert!(
        out.contains("rank 3 or better (ApprovalRequired or Waiting): candidate 3/5 against saved 5/5, short by more than 1"),
        "{out}"
    );
    assert!(out.contains("verdict: refused, 1 scenario worse"), "{out}");
    world.assert_untouched();
    let mut expected = baseline_files();
    expected.push(format!("{}.refused.json", world.revision));
    expected.sort();
    assert_eq!(world.records(), expected);
    let (code, out, err) = world.promote(&two_worse, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("refused") && err.contains("not judged again until it changes"),
        "{err}"
    );

    let world = World::new("over-tokens", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let over = world.candidate_card(
        "over",
        [120_000, 139_000, 140_000, 141_000, 150_000],
        FIVE_APPROVALS,
    );
    let (code, out, err) = world.promote(&over, &[]);
    assert_eq!(code, 3, "{out}{err}");
    assert!(
        out.contains("median tokens 140000 exceeds the saved maximum 138160"),
        "{out}"
    );
    world.assert_untouched();
}

/// `--dry-run` prints the verdict and the record it would write, and writes and installs
/// nothing, for a candidate that holds and for one that is refused.
#[test]
fn dry_run_prints_the_verdict_and_the_record_and_writes_nothing() {
    let world = World::new("dry-run", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("same", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("verdict: no scenario worse than the baseline"),
        "{out}"
    );
    assert!(out.contains("dry run: would clone"), "{out}");
    assert!(
        out.contains("\"kind\": \"promotion\""),
        "the record it would write: {out}"
    );
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files());
    assert!(!world.store.join(LOCK).exists(), "a dry run takes no lock");
    let worse = world.candidate_card(
        "worse",
        WITHIN_TOKENS,
        [ERROR, ERROR, APPROVAL, APPROVAL, APPROVAL],
    );
    let (code, out, err) = world.promote(&worse, &["--dry-run"]);
    assert_eq!(code, 3, "{out}{err}");
    assert!(out.contains("\"kind\": \"refused\""), "{out}");
    assert_eq!(world.records(), baseline_files());
}

/// Refusals before any comparison: a scorecard inside the repository, one whose revision is not
/// the checkout's, a checkout with uncommitted changes, no baseline, and fewer cases than the
/// rule's trials. Each exits 1, writes nothing and installs nothing.
#[test]
fn a_scorecard_or_checkout_the_rule_cannot_trust_is_refused() {
    let world = World::new("untrusted", BASELINE_TOKENS, FIVE_APPROVALS);
    let card = world.candidate_card("untrusted", BASELINE_TOKENS, FIVE_APPROVALS);
    // No baseline yet.
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("holds no baseline"), "{err}");
    world.record_baseline();
    // Inside the repository.
    let inside = world
        .repo
        .join(format!("scorecard-{}.json", world.revision));
    fs::copy(&card, &inside).expect("copied");
    let (code, out, err) = world.promote(&inside, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("inside the repository"), "{err}");
    fs::remove_file(&inside).expect("removed");
    // Named by another revision than the checkout's.
    let other = world
        .saves
        .join(format!("scorecard-{BASELINE_REVISION}.json"));
    let (code, out, err) = world.promote(&other, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("not at the scorecard's revision"), "{err}");
    // A short revision.
    let short_cases = world.outer.join("short-cases");
    fs::create_dir_all(&short_cases).expect("a directory");
    run(&short_cases, "scenario-1-trial-1", 1, APPROVAL);
    let short = World::save(&short_cases, &world.saves, "abcdef0");
    let (code, out, err) = world.promote(&short, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("no full commit id"), "{err}");
    // Fewer cases than the rule's trials.
    let few_cases = world.outer.join("few-cases");
    fs::create_dir_all(&few_cases).expect("a directory");
    for n in 1..=3 {
        run(
            &few_cases,
            &format!("scenario-1-trial-{n}"),
            120_000,
            APPROVAL,
        );
    }
    let few_saves = world.outer.join("few-saves");
    let few = World::save(&few_cases, &few_saves, &world.revision);
    let (code, out, err) = world.promote(&few, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("the rule needs 5"), "{err}");
    // Uncommitted changes to a tracked file.
    write(
        &world.repo.join("crates/takt-cli/src/main.rs"),
        "fn main() {}\n",
    );
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("uncommitted or untracked"), "{err}");
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files());
}

/// `takt promote --help` lists the exit codes.
#[test]
fn help_lists_the_exit_codes() {
    let (code, out, _) = takt(&["promote", "--help"], &[]);
    assert_eq!(code, 0);
    for line in [
        "Exit codes:",
        "  0  promoted",
        "  3  refused on the scorecard",
        "  5  ",
        "bubblewrap",
        "--revision <SHA>",
        "--binary-sha256 <HEX>",
        "binary.txt",
        "--binary-sha256 without --revision",
    ] {
        assert!(out.contains(line), "{line} in {out}");
    }
}

/// A scorecard saved under another rule, or under a metric definition this takt does not know,
/// is refused before any comparison: the rule is takt's, not the file's.
#[test]
fn a_scorecard_under_another_rule_or_definition_is_refused() {
    let world = World::new("other-rule", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("rule", BASELINE_TOKENS, FIVE_APPROVALS);
    let text = fs::read_to_string(&card).expect("the card");
    assert!(text.contains("\"margin\": 1"), "{text}");
    fs::write(&card, text.replace("\"margin\": 1", "\"margin\": 2")).expect("rewritten");
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("trials 5 margin 2") && err.contains("trials 5 margin 1"),
        "both rules named: {err}"
    );
    let card = world.candidate_card("metric", BASELINE_TOKENS, FIVE_APPROVALS);
    let text = fs::read_to_string(&card).expect("the card");
    assert!(text.contains("\"tokens\": 1"), "{text}");
    fs::write(&card, text.replace("\"tokens\": 1", "\"tokens\": 9")).expect("rewritten");
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("metric tokens is at definition version 9"),
        "{err}"
    );
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files());
}

/// The store is read fail-closed: a file takt does not read, a record whose name does not
/// carry its revision, or a scorecard copy whose digest is not the record's refuses every
/// promotion, naming the file.
#[test]
fn a_store_takt_cannot_read_refuses_every_promotion() {
    let world = World::new("store-integrity", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("ok", BASELINE_TOKENS, FIVE_APPROVALS);
    let stray = world.store.join("notes.txt");
    write(&stray, "a note\n");
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("notes.txt"), "{err}");
    fs::remove_file(&stray).expect("removed");

    let record = world.store.join(format!("{BASELINE_REVISION}.json"));
    let misnamed = world.store.join(format!("{}.json", "b".repeat(40)));
    fs::rename(&record, &misnamed).expect("renamed");
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&"b".repeat(40)) && err.contains(BASELINE_REVISION),
        "{err}"
    );
    fs::rename(&misnamed, &record).expect("renamed back");

    let copy = world
        .store
        .join(format!("{BASELINE_REVISION}.scorecard.json"));
    let text = fs::read_to_string(&copy).expect("the copy");
    fs::write(&copy, format!("{text}\n")).expect("rewritten");
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("digest"), "{err}");
    fs::write(&copy, text).expect("restored");

    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "a clean store reads again: {out}{err}");
}

/// A promoted tree under another commit is recorded already: an empty commit on top of a
/// promoted revision is no new candidate.
#[test]
fn the_same_tree_under_another_commit_is_recorded_already() {
    let world = World::new("same-tree", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("first", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    git(
        &world.repo,
        &["commit", "-q", "--allow-empty", "-m", "the same tree again"],
    );
    let again = git(&world.repo, &["rev-parse", "HEAD"]);
    assert_ne!(again, world.revision);
    let card = world.passing_card("again", &again);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("the tree of") && err.contains(&world.revision),
        "{err}"
    );
}

/// Nothing a promotion reads or writes may lie in the repository the candidate changes: an
/// install root inside it, and a scorecard with a second link (which may be a name inside it),
/// are refused.
#[test]
fn paths_that_land_inside_the_repository_are_refused() {
    let world = World::new("inside", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("inside", BASELINE_TOKENS, FIVE_APPROVALS);
    let inside = world.repo.join("root");
    let env = world.env();
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (code, out, err) = takt(
        &[
            "promote",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--scorecard",
            card.to_str().expect("utf-8"),
            "--install-root",
            inside.to_str().expect("utf-8"),
        ],
        &env,
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("--install-root") && err.contains("inside the repository"),
        "{err}"
    );
    let link = world.repo.join("scorecard-link.json");
    fs::hard_link(&card, &link).expect("a hard link");
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("2 links"), "{err}");
    fs::remove_file(&link).expect("removed");
    world.assert_untouched();
}

/// A candidate that holds but does not build, or builds a binary naming another revision, is
/// exit 5; the binary installed before stays, and nothing is recorded.
#[test]
fn a_failed_or_foreign_build_leaves_the_installed_binary_and_records_nothing() {
    let world = World::new("build-fails", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    // A commit whose source does not compile.
    write(
        &world.repo.join("crates/takt-cli/src/main.rs"),
        "fn main() { this is no rust }\n",
    );
    let broken = world.commit("broken");
    let card = world.passing_card("broken", &broken);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 5, "{out}{err}");
    world.assert_untouched();
    // A commit whose build script embeds a revision that is not the commit's.
    write(&world.repo.join("crates/takt-cli/src/main.rs"), TAKT_MAIN);
    write(
        &world.repo.join("crates/takt-cli/build.rs"),
        "fn main() {\n    println!(\"cargo:rustc-env=TAKT_REVISION=deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\");\n}\n",
    );
    let foreign = world.commit("foreign revision");
    let card = world.passing_card("foreign", &foreign);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 5, "{out}{err}");
    assert!(err.contains("deadbeef") && err.contains(&foreign), "{err}");
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
    assert!(
        fs::read_dir(&world.root)
            .expect("the root")
            .flatten()
            .all(|entry| entry.file_name() == "bin"),
        "no fresh root is left"
    );
}

/// A baseline with fewer cases than the rule's trials judges nothing.
#[test]
fn a_baseline_with_fewer_cases_than_the_trials_judges_nothing() {
    let world = World::new("thin-baseline", BASELINE_TOKENS, FIVE_APPROVALS);
    let thin_cases = world.outer.join("thin-cases");
    fs::create_dir_all(&thin_cases).expect("a directory");
    for n in 1..=3 {
        run(
            &thin_cases,
            &format!("scenario-1-trial-{n}"),
            120_000,
            APPROVAL,
        );
    }
    let thin = World::save(
        &thin_cases,
        &world.outer.join("thin-saves"),
        BASELINE_REVISION,
    );
    world.record_baseline_of(&thin);
    let card = world.candidate_card("thin", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("the baseline has 3 cases"), "{err}");
}

/// The build reads nothing outside the commit (probes N1, N2, N3, N4, N6, N7, N9 and V2a): a
/// `RUSTC`, `RUSTC_WRAPPER`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_PROFILE_*` or `RUSTUP_TOOLCHAIN`
/// in the caller's environment, a `.cargo/config.toml` in an ancestor of the build directory or
/// in the caller's `CARGO_HOME`, and a `cargo` earlier on `PATH` change nothing: the wrapper and
/// the planted cargo never run, no linker map is written, debug assertions stay off, and the
/// build succeeds.
#[test]
fn the_build_reads_nothing_outside_the_commit() {
    let world = World::new("outside", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("outside", BASELINE_TOKENS, FIVE_APPROVALS);
    let log = world.outer.join("wrap.log");
    let wrapper = world.outer.join("wrap.sh");
    script(
        &wrapper,
        &format!(
            "#!/bin/sh\necho \"INJECTED $0 $*\" >> '{}'\nexit 1\n",
            log.display()
        ),
    );
    let config = format!("[build]\nrustc-wrapper = \"{}\"\n", wrapper.display());
    write(&world.outer.join("cache/.cargo/config.toml"), &config);
    let cargo_home = world.outer.join("cargo-home-with-config");
    write(&cargo_home.join("config.toml"), &config);
    let planted = world.outer.join("planted");
    script(
        &planted.join("cargo"),
        &format!(
            "#!/bin/sh\necho \"INJECTED planted cargo $*\" >> '{}'\nexit 1\n",
            log.display()
        ),
    );
    let path = format!(
        "{}:{}",
        planted.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let encoded = format!(
        "-Clink-args=-Wl,-Map={}",
        world.outer.join("encoded.map").display()
    );
    let rustflags = format!(
        "-Clink-args=-Wl,-Map={}",
        world.outer.join("rustflags.map").display()
    );
    let target_dir = world.outer.join("tdir");
    let wrapper_text = wrapper.to_str().expect("utf-8");
    let (code, out, err) = world.promote_with(
        &card,
        &[],
        &[
            ("PATH", path.as_str()),
            ("RUSTC", wrapper_text),
            ("RUSTC_WRAPPER", wrapper_text),
            ("RUSTC_WORKSPACE_WRAPPER", wrapper_text),
            ("CARGO_BUILD_RUSTC_WRAPPER", wrapper_text),
            ("CARGO_ENCODED_RUSTFLAGS", encoded.as_str()),
            ("RUSTFLAGS", rustflags.as_str()),
            ("CARGO_TARGET_DIR", target_dir.to_str().expect("utf-8")),
            ("CARGO_HOME", cargo_home.to_str().expect("utf-8")),
            ("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS", "true"),
            ("RUSTUP_TOOLCHAIN", "no-such-toolchain-for-takt"),
        ],
    );
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(world.version(), format!("takt 0.1.0 ({})", world.revision));
    assert!(
        !log.exists(),
        "neither the wrapper nor the planted cargo ran"
    );
    assert!(
        !world.outer.join("encoded.map").exists() && !world.outer.join("rustflags.map").exists(),
        "no linker map is written"
    );
    assert!(!target_dir.exists(), "CARGO_TARGET_DIR is not used");
    let probe = Command::new(world.installed())
        .arg("--probe")
        .output()
        .expect("the installed takt runs");
    assert_eq!(
        String::from_utf8_lossy(&probe.stdout).trim(),
        "debug_assertions=false",
        "the release profile is the commit's"
    );
}

/// A `rustc` first on `PATH` that prints a false sysroot, whose `bin/cargo` and `bin/rustc`
/// would inject, changes nothing: the toolchain is rustup's default as its settings file names
/// it, so neither the planted rustc nor anything under the false sysroot runs, and the
/// candidate is built and installed.
#[test]
fn a_planted_rustc_on_path_is_ignored() {
    let world = World::new("planted-rustc", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("planted-rustc", BASELINE_TOKENS, FIVE_APPROVALS);
    let log = world.outer.join("planted.log");
    let sysroot = world.outer.join("false-sysroot");
    fs::create_dir_all(sysroot.join("lib/rustlib")).expect("a directory");
    for binary in ["cargo", "rustc"] {
        script(
            &sysroot.join("bin").join(binary),
            &format!(
                "#!/bin/sh\necho \"INJECTED false sysroot {binary} $*\" >> '{}'\nexit 1\n",
                log.display()
            ),
        );
    }
    let planted = world.outer.join("planted-bin");
    script(
        &planted.join("rustc"),
        &format!(
            "#!/bin/sh\necho \"INJECTED planted rustc $*\" >> '{}'\nif [ \"$1 $2\" = \"--print sysroot\" ]; then echo '{}'; exit 0; fi\nexit 1\n",
            log.display(),
            sysroot.display()
        ),
    );
    let path = format!(
        "{}:{}",
        planted.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let (code, out, err) = world.promote_with(&card, &[], &[("PATH", path.as_str())]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        !log.exists(),
        "neither the planted rustc nor the false sysroot ran: {}",
        fs::read_to_string(&log).unwrap_or_default()
    );
    assert_eq!(world.version(), format!("takt 0.1.0 ({})", world.revision));
}

/// A committed `.cargo/config.toml` naming a wrapper outside the repository fails the build
/// (probe N5): the path does not exist inside the confinement, so exit 5, the wrapper never
/// runs, the old binary is untouched and nothing is recorded.
#[test]
fn a_committed_cargo_config_naming_an_outside_wrapper_fails_the_build() {
    let world = World::new("committed-config", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let log = world.outer.join("wrap.log");
    let wrapper = world.outer.join("wrap.sh");
    script(
        &wrapper,
        &format!(
            "#!/bin/sh\necho \"INJECTED $*\" >> '{}'\nshift\nexec \"$@\"\n",
            log.display()
        ),
    );
    write(
        &world.repo.join(".cargo/config.toml"),
        &format!("[build]\nrustc-wrapper = \"{}\"\n", wrapper.display()),
    );
    let revision = world.commit("a wrapper outside the repository");
    let card = world.passing_card("committed-config", &revision);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 5, "{out}{err}");
    assert!(!log.exists(), "the wrapper never ran: {err}");
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// A committed `rust-toolchain.toml` with a `path` is refused by name before anything is
/// judged (probe N8): exit 1, nothing written, the old binary untouched.
#[test]
fn a_committed_rust_toolchain_path_is_refused() {
    let world = World::new("toolchain-path", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let toolchain = world.outer.join("toolchain");
    fs::create_dir_all(toolchain.join("bin")).expect("a directory");
    write(
        &world.repo.join("rust-toolchain.toml"),
        &format!("[toolchain]\npath = \"{}\"\n", toolchain.display()),
    );
    let revision = world.commit("a toolchain outside the repository");
    let card = world.passing_card("toolchain-path", &revision);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("rust-toolchain.toml") && err.contains("`path`"),
        "{err}"
    );
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// The candidate's code, run during the promotion (its `build.rs`, its `--version`), cannot
/// reach the store and cannot outlive the build (probe N15): the record it plants is not in the
/// store, and the sleeper it leaves is gone when `takt promote` returns.
#[test]
fn the_candidate_cannot_reach_the_store_or_outlive_its_build() {
    let world = World::new("plant", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let marker = format!("takt-promote-n15-{}", std::process::id());
    let plant = PLANT
        .replace("__STORE__", world.store.to_str().expect("utf-8"))
        .replace("__MARKER__", &marker);
    write(
        &world.repo.join("crates/takt-cli/build.rs"),
        &format!(
            "{}{plant}",
            TAKT_BUILD.replacen("fn main() {\n", "fn main() {\n    plant();\n", 1)
        ),
    );
    write(
        &world.repo.join("crates/takt-cli/src/main.rs"),
        &format!(
            "{}{plant}",
            TAKT_MAIN.replacen("fn main() {\n", "fn main() {\n    plant();\n", 1)
        ),
    );
    let revision = world.commit("plant");
    let card = world.passing_card("plant", &revision);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        processes_holding(&marker),
        Vec::<String>::new(),
        "the sleeper died with the build"
    );
    let mut expected = baseline_files();
    expected.push(format!("{revision}.json"));
    expected.push(format!("{revision}.scorecard.json"));
    expected.sort();
    assert_eq!(world.records(), expected, "the store holds what takt wrote");
    let record = world.record(&format!("{revision}.json"));
    assert!(record.contains("\"kind\": \"promotion\""), "{record}");
    // The planting binary is not run on the host here: its `--version` would plant from
    // outside the confinement. The record's digest is the installed file's instead.
    assert_eq!(field(&record, "revision"), revision);
    assert_eq!(field(&record, "binary_sha256"), sha256(&world.installed()));
}

/// A record that cannot be written leaves the installed binary untouched (probe N10): the
/// record is written before the binary moves into place, so a store that refuses the write is
/// exit 1 with nothing installed and nothing recorded.
#[test]
fn a_record_that_cannot_be_written_leaves_the_binary_untouched() {
    let world = World::new("readonly-store", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("readonly", BASELINE_TOKENS, FIVE_APPROVALS);
    write(&world.store.join(LOCK), "");
    fs::set_permissions(&world.store, fs::Permissions::from_mode(0o555)).expect("read-only");
    let (code, out, err) = world.promote(&card, &[]);
    fs::set_permissions(&world.store, fs::Permissions::from_mode(0o755)).expect("writable");
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("Permission denied"), "{err}");
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// A binary that names another revision once it sits at `<root>/bin/takt` is rolled back
/// (probe N11): exit 5, the binary before it back in place, the record removed.
#[test]
fn a_binary_that_changes_once_installed_is_rolled_back() {
    let world = World::new("flip", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    write(
        &world.repo.join("crates/takt-cli/src/main.rs"),
        &FLIPPING_MAIN
            .replace("__TARGET__", INSIDE_TARGET)
            .replace("__INSTALL__", INSIDE_INSTALL),
    );
    let revision = world.commit("flip");
    let card = world.passing_card("flip", &revision);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 5, "{out}{err}");
    assert!(
        err.contains("something-else") && err.contains("back at"),
        "{err}"
    );
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "the record is removed");
    assert!(
        fs::read_dir(world.root.join("bin"))
            .expect("bin")
            .flatten()
            .all(|entry| entry.file_name() == "takt"),
        "no backup is left beside the binary"
    );
}

/// Two promotions started together record and install exactly one (probe N12): the other
/// refuses naming the store's lock, or sees the first's record.
#[test]
fn two_promotions_at_once_record_and_install_exactly_one() {
    let world = World::new("two-at-once", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("one", BASELINE_TOKENS, FIVE_APPROVALS);
    let second = world.outer.join("candidate-two");
    git(
        &world.outer,
        &[
            "clone",
            "-q",
            "--no-hardlinks",
            world.repo.to_str().expect("utf-8"),
            second.to_str().expect("utf-8"),
        ],
    );
    write(&second.join("two.txt"), "two\n");
    git(&second, &["add", "-A"]);
    git(&second, &["commit", "-q", "-m", "two"]);
    let other = git(&second, &["rev-parse", "HEAD"]);
    let other_card = world.passing_card("two", &other);
    let env = world.env();
    let spawn = |repo: &Path, scorecard: &Path| {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(world.args(repo, scorecard))
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("takt starts")
    };
    let first = spawn(&world.repo, &card);
    let second_run = spawn(&second, &other_card);
    let outputs: Vec<(i32, String, String)> = [first, second_run]
        .into_iter()
        .map(|child| {
            let output = child.wait_with_output().expect("takt ends");
            (
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stdout).into_owned(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )
        })
        .collect();
    let promoted: Vec<&(i32, String, String)> =
        outputs.iter().filter(|(code, _, _)| *code == 0).collect();
    assert_eq!(promoted.len(), 1, "exactly one promotes: {outputs:?}");
    let refused: Vec<&(i32, String, String)> =
        outputs.iter().filter(|(code, _, _)| *code != 0).collect();
    assert_eq!(refused.len(), 1, "{outputs:?}");
    let (code, _, err) = refused[0];
    assert_eq!(*code, 1, "{err}");
    assert!(
        err.contains("holds the store lock")
            || err.contains("this UTC day already")
            || err.contains("recorded already"),
        "{err}"
    );
    let records = world.records();
    assert_eq!(records.len(), 4, "one promotion recorded: {records:?}");
    let recorded = records
        .iter()
        .find(|name| {
            name.ends_with(".json")
                && !name.ends_with(".scorecard.json")
                && !name.starts_with(BASELINE_REVISION)
        })
        .expect("the promotion's record");
    let record = world.record(recorded);
    let revision = field(&record, "revision");
    assert_eq!(
        world.version(),
        format!("takt 0.1.0 ({revision})"),
        "the installed binary is the recorded one"
    );
    assert_eq!(field(&record, "binary_sha256"), sha256(&world.installed()));
}

/// A record removed from the store is noticed (probe N13): the installed binary must name the
/// latest record, so a promotion whose record was removed refuses naming both; and a record
/// removed from between two breaks the chain of digests, refusing and naming the gap.
#[test]
fn a_removed_record_is_noticed() {
    let world = World::new("removed-end", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("first", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    let promoted = world.revision.clone();
    fs::remove_file(world.store.join(format!("{promoted}.json"))).expect("removed");
    fs::remove_file(world.store.join(format!("{promoted}.scorecard.json"))).expect("removed");
    write(&world.repo.join("c.txt"), "c\n");
    let next = world.commit("c");
    let card = world.passing_card("next", &next);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("names {promoted}"))
            && err.contains(BASELINE_REVISION)
            && err.contains("a record is missing"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");

    let world = World::new("removed-between", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let worse = world.candidate_card("worse", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&worse, &[]);
    assert_eq!(code, 3, "{out}{err}");
    let refused = world.store.join(format!("{}.refused.json", world.revision));
    write(&world.repo.join("d.txt"), "d\n");
    let good = world.commit("d");
    let card = world.passing_card("good", &good);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    let record = world.record(&format!("{good}.json"));
    assert_eq!(
        field(&record, "previous_sha256"),
        sha256(&refused),
        "the promotion chains to the refusal before it"
    );
    fs::remove_file(&refused).expect("removed");
    write(&world.repo.join("e.txt"), "e\n");
    let later = world.commit("e");
    let card = world.passing_card("later", &later);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("previous_sha256")
            && err.contains(&format!("{good}.json"))
            && err.contains("missing"),
        "the gap is named: {err}"
    );
}
