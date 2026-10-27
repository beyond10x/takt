//! story:promotion through the binary: `takt promote` installs a takt built from a revision only
//! when its saved scorecard shows no scenario worse than the promoted baseline's under the
//! scorecard's rule (a margin of one case at any outcome rank, the median of tokens within the
//! baseline's maximum), and writes a record under `$XDG_STATE_HOME/takt/promotions/` naming
//! the revision the installed `takt --version` prints, as many a day as hold, one at a time;
//! a worse candidate is refused, recorded and not judged again; `--baseline-only`
//! records the first baseline and installs nothing; `--dry-run` writes and installs nothing.
//! The build runs confined through a Substrate adapter, so nothing the commit runs reads a
//! setting outside it, reaches the store or outlives the build; the record is written before the
//! binary moves
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
        if exe.starts_with("__TARGET__") || exe.starts_with("__INSTALL__") {
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

/// The held-out result the runner writes beside a case's record, and what it writes for a pass.
const HELD_OUT: &str = takt::scorecard::HELD_OUT_FILE;
const PASS: &str = "PASS\n";

/// The instant every line of a run [`run`] writes carries, so its wall time is 0 however long
/// the writing took, and two directories of the same runs read the same figures.
const AT: &str = "2026-10-19T12:00:00.000000Z";

/// One run of `case` in `repo`: opened by the binary of `binary` (none: a binary that recorded
/// no revision), a judgement step that reported `tokens`, then `stop`; the held-out check passed.
/// Every line of the record carries the instant [`AT`].
fn run(repo: &Path, case: &str, tokens: u64, stop: &str, binary: Option<&str>) {
    emit_run(repo, case, tokens, stop, binary);
    let events = Events::new(&repo.join(CASES_DIR).join(case));
    let mut text = String::new();
    for mut line in events.read().expect("the record reads") {
        line["at"] = AT.into();
        text.push_str(&line.to_string());
        text.push('\n');
    }
    fs::write(events.path(), text).expect("the record at fixed instants");
}

/// What [`run`] writes, each line at the instant it was written.
fn emit_run(repo: &Path, case: &str, tokens: u64, stop: &str, binary: Option<&str>) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    fs::write(dir.join(HELD_OUT), PASS).expect("the held-out result");
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
                binary: binary.map(str::to_owned),
                layers: None,
                confinement: None,
                substrate: None,
                endpoint: None,
                records: None,
                scan: None,
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
                version: None,
                cost_usd: None,
                duration_ms: None,
                api_ms: None,
                turns: None,
                started_at: None,
                transcript: None,
                transcript_refused: None,
            },
        ),
        (
            1,
            Event::Stop {
                stop: stop.to_owned(),
                steps: 1,
                tokens,
                completed: None,
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

/// The process ids whose command line holds `marker`, from `ps`, which Linux and macOS both
/// have; macOS has no `/proc` to read them from.
fn processes_holding(marker: &str) -> Vec<String> {
    let output = Command::new("ps")
        .args(["-A", "-ww", "-o", "pid=,args="])
        .output()
        .expect("ps runs");
    assert!(
        output.status.success(),
        "ps -A: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim_start().split_once(' '))
        .filter(|(_, args)| args.contains(marker))
        .map(|(pid, _)| pid.to_owned())
        .collect()
}

impl World {
    fn new(name: &str, baseline_tokens: [u64; 5], baseline_stops: [&str; 5]) -> Self {
        let outer = scratch(name);
        let repo = outer.join("candidate");
        fs::create_dir_all(&repo).expect("a directory");
        let revision = candidate(&repo);
        // The mainline is the remote's: a bare repository standing in for it, `main` pushed.
        let origin = outer.join("origin.git");
        git(
            &outer,
            &[
                "init",
                "-q",
                "-b",
                "main",
                "--bare",
                origin.to_str().expect("utf-8"),
            ],
        );
        git(
            &repo,
            &["remote", "add", "origin", origin.to_str().expect("utf-8")],
        );
        git(&repo, &["push", "-q", "origin", "main"]);
        let saves = outer.join("saves");
        let baseline_cases = outer.join("baseline-cases");
        fs::create_dir_all(&baseline_cases).expect("a directory");
        for (n, (tokens, stop)) in baseline_tokens.iter().zip(baseline_stops).enumerate() {
            run(
                &baseline_cases,
                &format!("scenario-1-trial-{}", n + 1),
                *tokens,
                stop,
                Some(BASELINE_REVISION),
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

    /// As [`World::save`] over runs under `cases` that name no revision, then each run's
    /// `opened_binary` in the saved file set to `binaries[n]`, in row order: a scorecard of runs
    /// of other binaries than `revision`'s, as a takt before story:scorecard-save-checks-binary
    /// saved one or a hand edit leaves one, since `takt scorecard --save` refuses to. The file.
    fn forged(cases: &Path, saves: &Path, revision: &str, binaries: &[Option<&str>]) -> PathBuf {
        const NONE: &str = "\"opened_binary\": null";
        let path = Self::save(cases, saves, revision);
        let text = fs::read_to_string(&path).expect("the saved file");
        let mut parts = text.split(NONE);
        let mut forged = parts
            .next()
            .expect("the text before the first run")
            .to_owned();
        let mut runs = 0;
        for part in parts {
            let binary = binaries
                .get(runs)
                .copied()
                .flatten()
                .map_or_else(|| "null".to_owned(), |binary| format!("{binary:?}"));
            forged.push_str(&format!("\"opened_binary\": {binary}{part}"));
            runs += 1;
        }
        assert_eq!(runs, binaries.len(), "a binary a run: {text}");
        fs::write(&path, forged).expect("the forged file");
        path
    }

    /// A scorecard of five scenario 1 cases with these tokens and stops, each run made with the
    /// binary of `revision`, saved under `revision` in a directory of its own named by `label`,
    /// since a saved scorecard is never overwritten.
    fn card_for(&self, label: &str, revision: &str, tokens: [u64; 5], stops: [&str; 5]) -> PathBuf {
        self.card_made_by(label, revision, tokens, stops, [Some(revision); 5])
    }

    /// As [`World::card_for`], run `n` made with the binary `binaries[n]` names.
    fn card_made_by(
        &self,
        label: &str,
        revision: &str,
        tokens: [u64; 5],
        stops: [&str; 5],
        binaries: [Option<&str>; 5],
    ) -> PathBuf {
        self.card_of_trials(label, revision, 11, tokens, stops, binaries)
    }

    /// The candidate's scorecard of five scenario 1 cases, trials `first` to `first + 4`, with
    /// these tokens and stops, each run made with the candidate's binary.
    fn trials_card(
        &self,
        label: &str,
        first: usize,
        tokens: [u64; 5],
        stops: [&str; 5],
    ) -> PathBuf {
        let own = self.revision.as_str();
        self.card_of_trials(label, own, first, tokens, stops, [Some(own); 5])
    }

    /// As [`World::card_made_by`], the cases trials `first` to `first + 4`; a card holding a run
    /// of another binary than `revision`'s is [`World::forged`], as the save refuses it.
    fn card_of_trials(
        &self,
        label: &str,
        revision: &str,
        first: usize,
        tokens: [u64; 5],
        stops: [&str; 5],
        binaries: [Option<&str>; 5],
    ) -> PathBuf {
        let cases = self.outer.join(format!("cases-{label}"));
        fs::create_dir_all(&cases).expect("a directory");
        let own = binaries.iter().all(|binary| *binary == Some(revision));
        for (n, (tokens, stop)) in tokens.iter().zip(stops).enumerate() {
            run(
                &cases,
                &format!("scenario-1-trial-{}", n + first),
                *tokens,
                stop,
                binaries[n].filter(|_| own),
            );
        }
        let saves = self.outer.join(format!("saves-{label}"));
        if own {
            Self::save(&cases, &saves, revision)
        } else {
            Self::forged(&cases, &saves, revision, &binaries)
        }
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
    /// Commits everything in the checkout and pushes `main` to the origin, as a merge into
    /// the mainline does; a commit on another branch leaves the origin's `main` as it was. The
    /// new HEAD.
    fn commit(&self, message: &str) -> String {
        git(&self.repo, &["add", "-A"]);
        git(&self.repo, &["commit", "-q", "-m", message]);
        git(&self.repo, &["push", "-q", "origin", "main"]);
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
        self.stated_card_made_by(label, stated, None)
    }

    /// As [`World::stated_card`], each run opened by the binary of `binary` (none: a binary
    /// that recorded no revision, as one built before `--version` named its revision); a card
    /// of another binary than the candidate revision's is [`World::forged`].
    fn stated_card_made_by(
        &self,
        label: &str,
        stated: [Option<(&str, &str)>; 5],
        binary: Option<&str>,
    ) -> PathBuf {
        self.stated_card_of_binaries(label, stated, [binary; 5])
    }

    /// As [`World::stated_card`], run `n` opened by the binary `binaries[n]` names; a card the
    /// save refuses (a run of another binary than the candidate revision's, or a mix of runs
    /// naming it and naming none) is [`World::forged`].
    fn stated_card_of_binaries(
        &self,
        label: &str,
        stated: [Option<(&str, &str)>; 5],
        binaries: [Option<&str>; 5],
    ) -> PathBuf {
        let cases = self.outer.join(format!("cases-{label}"));
        fs::create_dir_all(&cases).expect("a directory");
        let own = binaries.iter().all(Option::is_none)
            || binaries
                .iter()
                .all(|binary| *binary == Some(self.revision.as_str()));
        for (n, (tokens, stop)) in BASELINE_TOKENS.iter().zip(FIVE_APPROVALS).enumerate() {
            let case = format!("scenario-1-trial-{}", n + 11);
            run(&cases, &case, *tokens, stop, binaries[n].filter(|_| own));
            if let Some((digest, note)) = stated[n] {
                write(
                    &cases.join(CASES_DIR).join(&case).join("binary.txt"),
                    &format!("~/.cargo/bin/takt\t{digest}\n{note}"),
                );
            }
        }
        let saves = self.outer.join(format!("saves-{label}"));
        if own {
            Self::save(&cases, &saves, &self.revision)
        } else {
            Self::forged(&cases, &saves, &self.revision, &binaries)
        }
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

    /// Points the head beside the store at the record `name` as its file now is, as a process
    /// running as the same user can (story:promotion-chain-head-anchored's stated limit): a
    /// store edited by hand to reach a check behind the head.
    fn rehead(&self, name: &str) {
        write(
            &head(self),
            &format!(
                "{{\n  \"record\": \"{name}\",\n  \"sha256\": \"{}\"\n}}\n",
                sha256(&self.store.join(name))
            ),
        );
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
    // The installed binary names its revision, so its runs name it on their `opened` lines.
    let card = world.stated_card_made_by(
        "named",
        [Some((digest.as_str(), "")); 5],
        Some(&world.revision),
    );
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
    let replaced = fs::read(world.installed()).expect("the binary before it");
    let replaced_digest = sha256(&world.installed());
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    // The binary it replaced is kept by its digest under the state directory, for a turn
    // that judges the promotion to finish with (story:self-improvement-protocol).
    let kept = world
        .store
        .parent()
        .expect("the state's takt directory")
        .join("judges")
        .join(&replaced_digest);
    assert_eq!(
        fs::read(&kept).ok(),
        Some(replaced),
        "{}: {out}{err}",
        kept.display()
    );
    assert!(
        fs::metadata(&kept)
            .is_ok_and(
                |meta| std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111 != 0
            ),
        "the kept binary runs"
    );
    assert!(
        out.contains(&kept.display().to_string()),
        "the kept binary is named: {out}"
    );
    assert!(
        out.contains("rule  trials 5 a scenario  margin 1 case"),
        "the rule applied is printed: {out}"
    );
    assert!(
        out.contains("verdict: no scenario worse than the baseline"),
        "{out}"
    );
    assert!(
        out.contains("(confined by Substrate "),
        "story:confinement-through-substrate-on-linux-and-macos: each cargo step names the Substrate adapter it ran through: {out}"
    );
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
    // The same revision promoted again is refused: it is recorded already.
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("{} is recorded already", world.revision)),
        "the revision is named as recorded: {err}"
    );
}

/// story:promotion-without-daily-cap: two promotions of different revisions, one right after
/// the other, each judged against the baseline before it, both hold: nothing between them
/// refuses on time; each is recorded after the one before it and the head names the second;
/// the second revision promoted again is refused as recorded already.
#[test]
fn two_revisions_promoted_one_after_the_other_both_hold_and_chain() {
    let world = World::new("successive", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let baseline_digest = sha256(&world.store.join(format!("{BASELINE_REVISION}.json")));
    let card = world.candidate_card(
        "first",
        WITHIN_TOKENS,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, ERROR],
    );
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(&format!(
            "candidate {} against baseline {BASELINE_REVISION}",
            world.revision
        )),
        "the recorded baseline is the first's: {out}"
    );
    let first_name = format!("{}.json", world.revision);
    let first = world.record(&first_name);
    for line in [
        format!("\"against\": \"{BASELINE_REVISION}\""),
        format!("\"previous_sha256\": \"{baseline_digest}\""),
    ] {
        assert!(first.contains(&line), "{line} in {first}");
    }
    let first_digest = sha256(&world.store.join(&first_name));
    write(&world.repo.join("second.txt"), "second\n");
    let second = world.commit("second");
    let second_card = world.passing_card("second", &second);
    let (code, out, err) = world.promote(&second_card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(&format!(
            "candidate {second} against baseline {}",
            world.revision
        )),
        "the first promotion is the baseline: {out}"
    );
    assert!(
        out.contains("verdict: no scenario worse than the baseline"),
        "{out}"
    );
    assert_eq!(
        world.version(),
        format!("takt 0.1.0 ({second})"),
        "the installed binary names the second revision"
    );
    let second_name = format!("{second}.json");
    let record = world.record(&second_name);
    for line in [
        "\"kind\": \"promotion\"".to_owned(),
        format!("\"revision\": \"{second}\""),
        format!("\"against\": \"{}\"", world.revision),
        format!("\"previous_sha256\": \"{first_digest}\""),
    ] {
        assert!(record.contains(&line), "{line} in {record}");
    }
    assert_eq!(field(&record, "binary_sha256"), sha256(&world.installed()));
    let text = fs::read_to_string(head(&world)).expect("the head");
    assert!(
        text.contains(&format!("\"{second_name}\""))
            && text.contains(&sha256(&world.store.join(&second_name))),
        "the head names the second promotion: {text}"
    );
    let (code, out, err) = world.promote(&second_card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("{second} is recorded already")),
        "the second revision is named as recorded: {err}"
    );
    assert_eq!(
        world.version(),
        format!("takt 0.1.0 ({second})"),
        "nothing is installed by the refusal"
    );
}

const ITERATIONS: &str = "Iterations";
const NOTHING_ADMISSIBLE: &str = "NothingAdmissible";

/// A scorecard of scenario 1's trials 11 to 15 stopping at `stops`, and with `scenario_2` also
/// scenario 2's trials 11 to 15 at the approval, every run made with the binary of `revision`,
/// tokens the baseline's; saved under `revision` in a directory of its own named by `label`.
fn series_card(
    world: &World,
    label: &str,
    revision: &str,
    stops: [&str; 5],
    scenario_2: bool,
) -> PathBuf {
    let cases = world.outer.join(format!("cases-{label}"));
    for (n, (tokens, stop)) in BASELINE_TOKENS.iter().zip(stops).enumerate() {
        let trial = n + 11;
        run(
            &cases,
            &format!("scenario-1-trial-{trial}"),
            *tokens,
            stop,
            Some(revision),
        );
        if scenario_2 {
            run(
                &cases,
                &format!("scenario-2-trial-{trial}"),
                *tokens,
                APPROVAL,
                Some(revision),
            );
        }
    }
    World::save(
        &cases,
        &world.outer.join(format!("saves-{label}")),
        revision,
    )
}

/// Two promotions back to back over the world's baseline (five cases at the approval), each one
/// case one outcome rank worse than the one before on scenario 1 and within the rule against
/// it and against the baseline: the approval to a budget stop, then that budget stop to
/// another stop. The second revision and what its promotion printed.
fn two_steps_down(world: &World) -> (String, String) {
    let first_card = series_card(
        world,
        "first",
        &world.revision,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, ITERATIONS],
        false,
    );
    let (code, out, err) = world.promote(&first_card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    write(&world.repo.join("second.txt"), "second\n");
    let second = world.commit("second");
    let second_card = series_card(
        world,
        "second",
        &second,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, NOTHING_ADMISSIBLE],
        false,
    );
    let (code, out, err) = world.promote(&second_card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    (second, out)
}

/// story:promotion-judged-against-the-series-baseline: three promotions back to back, each one
/// case worse than the one before on scenario 1 and within the rule against it; the third is
/// two cases short at the approval rank against the baseline that starts the series, so it is
/// refused naming that record and the scenario, the refusal recorded, nothing installed.
#[test]
fn a_third_step_one_case_worse_is_refused_against_the_series_start() {
    let world = World::new("series", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let start = format!("{BASELINE_REVISION}.json");
    let (second, second_out) = two_steps_down(&world);
    write(&world.repo.join("third.txt"), "third\n");
    let third = world.commit("third");
    let card = series_card(
        &world,
        "third",
        &third,
        [APPROVAL, APPROVAL, APPROVAL, ITERATIONS, NOTHING_ADMISSIBLE],
        false,
    );
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 3, "{out}{err}");
    for line in [
        format!("candidate {second} against baseline {}", world.revision),
        format!("candidate {second} against series start {BASELINE_REVISION}"),
        "verdict: no scenario worse than the baseline".to_owned(),
    ] {
        assert!(second_out.contains(&line), "{line} in {second_out}");
    }
    for name in [format!("{}.json", world.revision), format!("{second}.json")] {
        let record = world.record(&name);
        assert!(
            record.contains(&format!("\"series_start\": \"{start}\"")),
            "{name} names the baseline as its series start: {record}"
        );
    }
    for line in [
        format!("candidate {third} against baseline {second}"),
        "scenario 1  outcomes by rank held  tokens held  not worse".to_owned(),
        format!("candidate {third} against series start {BASELINE_REVISION}"),
        "rank 3 or better (ApprovalRequired or Waiting): candidate 3/5 against saved 5/5, short by more than 1".to_owned(),
        format!(
            "verdict: refused, 1 scenario worse than the series start {BASELINE_REVISION} ({}): scenario 1; nothing installed",
            world.store.join(&start).display()
        ),
    ] {
        assert!(out.contains(&line), "{line} in {out}");
    }
    let record = world.record(&format!("{third}.refused.json"));
    for line in [
        "\"kind\": \"refused\"".to_owned(),
        format!("\"against\": \"{second}\""),
        format!("\"series_start\": \"{start}\""),
        "\"worse\": 1".to_owned(),
    ] {
        assert!(record.contains(&line), "{line} in {record}");
    }
    assert_eq!(
        world.version(),
        format!("takt 0.1.0 ({second})"),
        "nothing is installed by the refusal"
    );
}

/// A scorecard of trials 11 to 15 of each scenario in `stops`, scenario `id`'s cases stopping at
/// its five stops, every run made with the binary of `revision`, tokens the baseline's; saved
/// under `revision` in a directory of its own named by `label`.
fn scenarios_card(
    world: &World,
    label: &str,
    revision: &str,
    stops: &[(u32, [&str; 5])],
) -> PathBuf {
    let cases = world.outer.join(format!("cases-{label}"));
    for (n, tokens) in BASELINE_TOKENS.iter().enumerate() {
        for (id, scenario_stops) in stops {
            run(
                &cases,
                &format!("scenario-{id}-trial-{}", n + 11),
                *tokens,
                scenario_stops[n],
                Some(revision),
            );
        }
    }
    World::save(
        &cases,
        &world.outer.join(format!("saves-{label}")),
        revision,
    )
}

/// The one rebaseline record in the store of `revision`.
fn rebaseline_record(world: &World, revision: &str) -> String {
    world
        .records()
        .into_iter()
        .find(|name| {
            name.starts_with(&format!("{revision}.rebaseline-"))
                && !name.ends_with(".scorecard.json")
        })
        .expect("the rebaseline record")
}

/// Puts the scorecard copy of each record `names`, in chain order, back to outcome definition 1
/// and re-points each record's digests and the head, as a takt one definition older would have
/// left the store.
fn stale_chain(world: &World, names: &[String]) {
    let mut previous: Option<String> = None;
    for name in names {
        let record = world.store.join(name);
        let copy = world.store.join(name.replace(".json", ".scorecard.json"));
        let v1 = outcome_v1(world, &copy, &format!("stale-{name}"), &[]);
        fs::copy(&v1, &copy).expect("downgraded");
        patch(&record, "scorecard_sha256", &sha256(&copy));
        if let Some(previous) = &previous {
            patch(&record, "previous_sha256", previous);
        }
        previous = Some(sha256(&record));
    }
    world.rehead(names.last().expect("a record"));
}

/// story:promotion-judged-against-the-series-baseline, correction 1: which rebaselines start a
/// series. One that only adds a scenario keeps the series start, so after two steps down and a
/// rebaseline of the second adding scenario 2, the third step one case worse is judged against
/// the baseline too, two cases short of it on scenario 1, and refused naming it. One under a
/// newer metric definition starts a new series: the same two steps judged under outcome
/// definition 1, the second rebaselined under definition 2, and the same third step is judged
/// against the rebaseline alone and holds, its record naming the rebaseline as its series start.
#[test]
fn a_rebaseline_under_newer_definitions_starts_a_new_series_and_an_add_only_one_does_not() {
    let third_step = [APPROVAL, APPROVAL, APPROVAL, ITERATIONS, NOTHING_ADMISSIBLE];
    let second_step = [APPROVAL, APPROVAL, APPROVAL, APPROVAL, NOTHING_ADMISSIBLE];

    let world = World::new("series-add-only", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let start = format!("{BASELINE_REVISION}.json");
    let (second, _) = two_steps_down(&world);
    let joined = scenarios_card(
        &world,
        "joined",
        &second,
        &[(1, second_step), (2, FIVE_APPROVALS)],
    );
    let (code, out, err) = rebaseline(&world, &joined);
    assert_eq!(code, 0, "{out}{err}");
    write(&world.repo.join("third.txt"), "third\n");
    let third = world.commit("third");
    let card = scenarios_card(
        &world,
        "third",
        &third,
        &[(1, third_step), (2, FIVE_APPROVALS)],
    );
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(
        code, 3,
        "an add-only rebaseline keeps the series start: {out}{err}"
    );
    for line in [
        format!("candidate {third} against baseline {second}"),
        format!("candidate {third} against series start {BASELINE_REVISION}"),
        format!(
            "verdict: refused, 1 scenario worse than the series start {BASELINE_REVISION} ({}): scenario 1; nothing installed",
            world.store.join(&start).display()
        ),
        format!("\"series_start\": \"{start}\""),
    ] {
        assert!(out.contains(&line), "{line} in {out}");
    }

    let world = World::new("series-newer", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let (second, _) = two_steps_down(&world);
    stale_chain(
        &world,
        &[
            start.clone(),
            format!("{}.json", world.revision),
            format!("{second}.json"),
        ],
    );
    let rescored = world
        .outer
        .join("saves-second")
        .join(format!("scorecard-{second}.json"));
    let (code, out, err) = rebaseline(&world, &rescored);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("under newer metric definitions"), "{out}");
    let name = rebaseline_record(&world, &second);
    write(&world.repo.join("third.txt"), "third\n");
    let third = world.commit("third");
    let card = series_card(&world, "third", &third, third_step, false);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    for line in [
        format!("candidate {third} against baseline {second}"),
        "verdict: no scenario worse than the baseline".to_owned(),
        format!("\"series_start\": \"{name}\""),
    ] {
        assert!(out.contains(&line), "{line} in {out}");
    }
    assert!(
        !out.contains("against series start"),
        "the rebaseline is the latest record and the series start, compared once: {out}"
    );
}

/// story:promotion-judged-against-the-series-baseline, correction 1 (review F1): two rebaselines
/// that only add a scenario reset nothing. After two steps down, a rebaseline adding scenario 2,
/// a third step that holds against the latest record and the baseline, and a rebaseline adding
/// scenario 3, a fourth step one case worse than the third on scenario 1 is two cases short of
/// the baseline there and refused naming the baseline as its series start, the refusal recorded,
/// nothing installed; every record judged names the baseline as its series start.
#[test]
fn two_add_only_rebaselines_keep_the_series_start_and_a_fourth_step_is_refused_naming_it() {
    let world = World::new("series-add-only-twice", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let start = format!("{BASELINE_REVISION}.json");
    let step = [APPROVAL, APPROVAL, APPROVAL, APPROVAL, NOTHING_ADMISSIBLE];
    let (second, _) = two_steps_down(&world);
    let joined = scenarios_card(&world, "joined", &second, &[(1, step), (2, FIVE_APPROVALS)]);
    let (code, out, err) = rebaseline(&world, &joined);
    assert_eq!(code, 0, "first rebaseline: {out}{err}");
    write(&world.repo.join("third.txt"), "third\n");
    let third = world.commit("third");
    let card = scenarios_card(&world, "third", &third, &[(1, step), (2, FIVE_APPROVALS)]);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "third promoted: {out}{err}");
    assert!(
        out.contains(&format!(
            "candidate {third} against series start {BASELINE_REVISION}"
        )),
        "{out}"
    );
    let joined = scenarios_card(
        &world,
        "joined-2",
        &third,
        &[(1, step), (2, FIVE_APPROVALS), (3, FIVE_APPROVALS)],
    );
    let (code, out, err) = rebaseline(&world, &joined);
    assert_eq!(code, 0, "second rebaseline: {out}{err}");
    write(&world.repo.join("fourth.txt"), "fourth\n");
    let fourth = world.commit("fourth");
    let card = scenarios_card(
        &world,
        "fourth",
        &fourth,
        &[
            (
                1,
                [APPROVAL, APPROVAL, APPROVAL, ITERATIONS, NOTHING_ADMISSIBLE],
            ),
            (2, FIVE_APPROVALS),
            (3, FIVE_APPROVALS),
        ],
    );
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(
        code, 3,
        "fourth, two cases short of the baseline: {out}{err}"
    );
    for line in [
        format!("candidate {fourth} against baseline {third}"),
        format!("candidate {fourth} against series start {BASELINE_REVISION}"),
        "rank 3 or better (ApprovalRequired or Waiting): candidate 3/5 against saved 5/5, short by more than 1".to_owned(),
        format!(
            "verdict: refused, 1 scenario worse than the series start {BASELINE_REVISION} ({}): scenario 1; nothing installed",
            world.store.join(&start).display()
        ),
    ] {
        assert!(out.contains(&line), "{line} in {out}");
    }
    for name in [
        format!("{}.json", world.revision),
        format!("{second}.json"),
        format!("{third}.json"),
        format!("{fourth}.refused.json"),
    ] {
        let record = world.record(&name);
        assert!(
            record.contains(&format!("\"series_start\": \"{start}\"")),
            "{name} names the baseline as its series start: {record}"
        );
    }
    assert_eq!(
        world.version(),
        format!("takt 0.1.0 ({third})"),
        "nothing is installed by the refusal"
    );
}

/// story:promotion-judged-against-the-series-baseline, correction 1: a scenario a rebaseline adds
/// is judged against that rebaseline, the record that starts its series. After the baseline's
/// rebaseline adds scenario 2, three steps each one case worse on scenario 2 alone: the third is
/// two cases short of the rebaseline there and refused naming it and the scenario, while the
/// record names the baseline as the series start.
#[test]
fn a_scenario_an_add_only_rebaseline_added_is_judged_against_that_rebaseline() {
    let world = World::new("series-added-scenario", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let start = format!("{BASELINE_REVISION}.json");
    let cases = world.outer.join("cases-joined");
    two_scenarios(&cases, 1, APPROVAL, APPROVAL, BASELINE_REVISION);
    let joined = World::save(&cases, &world.outer.join("saves-joined"), BASELINE_REVISION);
    let (code, out, err) = rebaseline(&world, &joined);
    assert_eq!(code, 0, "{out}{err}");
    let added = rebaseline_record(&world, BASELINE_REVISION);
    let first = scenarios_card(
        &world,
        "first",
        &world.revision,
        &[
            (1, FIVE_APPROVALS),
            (2, [APPROVAL, APPROVAL, APPROVAL, APPROVAL, ITERATIONS]),
        ],
    );
    let (code, out, err) = world.promote(&first, &[]);
    assert_eq!(code, 0, "{out}{err}");
    write(&world.repo.join("second.txt"), "second\n");
    let second = world.commit("second");
    let card = scenarios_card(
        &world,
        "second",
        &second,
        &[
            (1, FIVE_APPROVALS),
            (
                2,
                [APPROVAL, APPROVAL, APPROVAL, APPROVAL, NOTHING_ADMISSIBLE],
            ),
        ],
    );
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    write(&world.repo.join("third.txt"), "third\n");
    let third = world.commit("third");
    let card = scenarios_card(
        &world,
        "third",
        &third,
        &[
            (1, FIVE_APPROVALS),
            (
                2,
                [APPROVAL, APPROVAL, APPROVAL, ITERATIONS, NOTHING_ADMISSIBLE],
            ),
        ],
    );
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 3, "{out}{err}");
    for line in [
        format!("candidate {third} against baseline {second}"),
        format!("candidate {third} against series start {BASELINE_REVISION}"),
        format!(
            "candidate {third} against the rebaseline {BASELINE_REVISION} ({}) that added scenario 2",
            world.store.join(&added).display()
        ),
        format!(
            "verdict: refused, 1 scenario worse than the rebaseline {BASELINE_REVISION} ({}) that added scenario 2: scenario 2; nothing installed",
            world.store.join(&added).display()
        ),
        format!("\"series_start\": \"{start}\""),
        "\"worse\": 1".to_owned(),
    ] {
        assert!(out.contains(&line), "{line} in {out}");
    }
}

/// story:promotion-judged-against-the-series-baseline, correction 1 (review F2): a record's
/// `series_start` is checked on read as `rejudges` is. A record without it is read; one naming a
/// record that starts no series, the promotion before it, refuses the store naming both.
#[test]
fn a_series_start_naming_a_record_that_starts_no_series_refuses_the_store() {
    let world = World::new("series-start-edited", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let (second, _) = two_steps_down(&world);
    let name = format!("{second}.json");
    let path = world.store.join(&name);
    let named = format!("\"series_start\": \"{BASELINE_REVISION}.json\"");
    let text = fs::read_to_string(&path).expect("the record");
    assert_eq!(text.matches(&named).count(), 1, "{text}");
    fs::write(&path, text.replace(&named, "\"series_start\": null")).expect("edited");
    world.rehead(&name);
    write(&world.repo.join("third.txt"), "third\n");
    let third = world.commit("third");
    let card = series_card(
        &world,
        "third",
        &third,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, NOTHING_ADMISSIBLE],
        false,
    );
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "a record without the field is read: {out}{err}");
    let wrong = format!("{}.json", world.revision);
    fs::write(
        &path,
        text.replace(&named, &format!("\"series_start\": \"{wrong}\"")),
    )
    .expect("edited");
    world.rehead(&name);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&name)
            && err.contains(&format!("series_start names {wrong}"))
            && err.contains("the store is not read past this"),
        "the record and the start it names are named: {err}"
    );
}

/// story:promotion-judged-against-the-series-baseline, correction 1 (review M8): a scenario worse
/// than both the latest record and the series start is one scenario worse, counted once.
#[test]
fn a_scenario_worse_than_the_baseline_and_the_series_start_is_counted_once() {
    let world = World::new("series-worse-twice", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let first = series_card(
        &world,
        "first",
        &world.revision,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, ITERATIONS],
        false,
    );
    let (code, out, err) = world.promote(&first, &[]);
    assert_eq!(code, 0, "{out}{err}");
    write(&world.repo.join("second.txt"), "second\n");
    let second = world.commit("second");
    let card = series_card(
        &world,
        "second",
        &second,
        [
            APPROVAL,
            APPROVAL,
            APPROVAL,
            NOTHING_ADMISSIBLE,
            NOTHING_ADMISSIBLE,
        ],
        false,
    );
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 3, "{out}{err}");
    for line in [
        format!(
            "worse than the series start {BASELINE_REVISION} ({}): scenario 1",
            world
                .store
                .join(format!("{BASELINE_REVISION}.json"))
                .display()
        ),
        "verdict: refused, 1 scenario worse than the baseline; nothing installed".to_owned(),
    ] {
        assert!(out.contains(&line), "{line} in {out}");
    }
    let record = world.record(&format!("{second}.refused.json"));
    assert!(record.contains("\"worse\": 1"), "{record}");
}

/// story:promotion-judged-against-the-series-baseline, correction 1 (review M10): the series
/// start's scorecard copy is checked as the baseline's is, so one naming another revision, saved
/// under another rule or under an older metric definition, its digests re-pointed by hand, is
/// refused naming the copy.
#[test]
fn a_series_start_copy_of_another_revision_rule_or_definition_is_refused_naming_it() {
    for (label, from, to, said) in [
        (
            "revision",
            format!("\"revision\": \"{BASELINE_REVISION}\""),
            format!("\"revision\": \"{}\"", "b".repeat(40)),
            format!(
                "names {}, not the series start revision {BASELINE_REVISION}",
                "b".repeat(40)
            ),
        ),
        (
            "rule",
            "\"margin\": 1".to_owned(),
            "\"margin\": 2".to_owned(),
            "the series start's scorecard copy".to_owned(),
        ),
        (
            "definition",
            "\"outcome\": 3,".to_owned(),
            "\"outcome\": 1,".to_owned(),
            "metric outcome is at definition version 1".to_owned(),
        ),
    ] {
        let world = World::new(
            &format!("series-start-copy-{label}"),
            BASELINE_TOKENS,
            FIVE_APPROVALS,
        );
        world.record_baseline();
        let first = series_card(
            &world,
            "first",
            &world.revision,
            [APPROVAL, APPROVAL, APPROVAL, APPROVAL, ITERATIONS],
            false,
        );
        let (code, out, err) = world.promote(&first, &[]);
        assert_eq!(code, 0, "{out}{err}");
        let copy = world
            .store
            .join(format!("{BASELINE_REVISION}.scorecard.json"));
        let text = fs::read_to_string(&copy).expect("the copy");
        assert_eq!(text.matches(&from).count(), 1, "{from} in {text}");
        fs::write(&copy, text.replace(&from, &to)).expect("edited");
        let base = world.store.join(format!("{BASELINE_REVISION}.json"));
        patch(&base, "scorecard_sha256", &sha256(&copy));
        let latest = format!("{}.json", world.revision);
        patch(
            &world.store.join(&latest),
            "previous_sha256",
            &sha256(&base),
        );
        world.rehead(&latest);
        write(&world.repo.join("second.txt"), "second\n");
        let second = world.commit("second");
        let card = series_card(&world, "second", &second, FIVE_APPROVALS, false);
        let (code, out, err) = world.promote(&card, &["--dry-run"]);
        assert_eq!(code, 1, "{label}: {out}{err}");
        assert!(
            err.contains(&copy.display().to_string()) && err.contains(&said),
            "{label}: {said} in {err}"
        );
    }
}

/// story:promotion-judged-against-the-series-baseline: `takt promote --help` says what starts a
/// series, what keeps it, and names the refusal.
#[test]
fn help_names_the_series_rule() {
    let (code, help, _) = takt(&["promote", "--help"], &[]);
    assert_eq!(code, 0);
    for text in [
        "series start",
        "the latest --baseline-only record or --rebaseline under a newer metric definition",
        "a --rebaseline that only adds a scenario keeps the series start",
        "judged against the --rebaseline that added it",
    ] {
        assert!(help.contains(text), "{text} in {help}");
    }
}

/// A link planted where `takt promote` keeps the binary it replaces, to a file holding that
/// binary's bytes (which a link could later point elsewhere), is not kept: it is replaced by a
/// regular file of its own.
#[test]
fn a_link_planted_where_the_replaced_binary_is_kept_is_replaced() {
    let world = World::new("kept-link", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card(
        "within",
        WITHIN_TOKENS,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, ERROR],
    );
    let replaced = fs::read(world.installed()).expect("the binary before it");
    let digest = sha256(&world.installed());
    let judges = world
        .store
        .parent()
        .expect("the state's takt directory")
        .join("judges");
    fs::create_dir_all(&judges).expect("the judges directory");
    let elsewhere = world
        .root
        .parent()
        .expect("a parent")
        .join("elsewhere-takt");
    fs::write(&elsewhere, &replaced).expect("a file elsewhere");
    std::os::unix::fs::symlink(&elsewhere, judges.join(&digest)).expect("the link");
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 0, "{out}{err}");
    let kept = judges.join(&digest);
    let meta = fs::symlink_metadata(&kept).expect("kept");
    assert!(meta.file_type().is_file(), "{}: {out}{err}", kept.display());
    assert_eq!(fs::read(&kept).ok(), Some(replaced));
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
    // A short revision: the save refuses one, so the file is renamed from a save under the
    // candidate revision, as a takt before decision D3 of story:scorecard-save-checks-binary
    // or a hand edit leaves one.
    let short_cases = world.outer.join("short-cases");
    fs::create_dir_all(&short_cases).expect("a directory");
    run(&short_cases, "scenario-1-trial-1", 1, APPROVAL, None);
    let short_saves = world.outer.join("short-saves");
    let full = World::save(&short_cases, &short_saves, &world.revision);
    let short = short_saves.join("scorecard-abcdef0.json");
    let text = fs::read_to_string(&full).expect("the saved file");
    let named = format!("\"revision\": \"{}\"", world.revision);
    assert_eq!(text.matches(&named).count(), 1, "{text}");
    fs::write(&short, text.replace(&named, "\"revision\": \"abcdef0\"")).expect("renamed");
    fs::remove_file(&full).expect("the full-id file removed");
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
            Some(&world.revision),
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
        "no Substrate adapter on the host serving the build",
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
    assert!(text.contains("\"tokens\": 2"), "{text}");
    fs::write(&card, text.replace("\"tokens\": 2", "\"tokens\": 9")).expect("rewritten");
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
    git(&world.repo, &["push", "-q", "origin", "main"]);
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
            Some(BASELINE_REVISION),
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

/// A stop line a run stopped for want of free disk writes.
const DISK_LOW: &str =
    "DiskLow (/srv/tree: 1073741824 bytes (1.00 GiB) free, under 10737418240 bytes (10.00 GiB))";

/// A baseline with a `DiskLow` trial judges nothing, though it holds the rule's five cases: the
/// promotion is refused naming the trial, before anything is built.
#[test]
fn a_baseline_with_a_disk_low_trial_judges_nothing() {
    let world = World::new(
        "disk-low-baseline",
        BASELINE_TOKENS,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, DISK_LOW],
    );
    world.record_baseline_of(&world.baseline);
    let card = world.candidate_card("disk-low-baseline", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("the baseline has DiskLow trials (scenario-1-trial-5)"),
        "{err}"
    );
}

/// A candidate set with a `DiskLow` trial is not recorded as a baseline, though it holds the
/// rule's five cases.
#[test]
fn a_set_with_a_disk_low_trial_is_not_recorded_as_a_baseline() {
    let world = World::new("disk-low-only", BASELINE_TOKENS, FIVE_APPROVALS);
    world.install_stand_in(&world.revision);
    let card = world.candidate_card(
        "disk-low-only",
        BASELINE_TOKENS,
        [APPROVAL, APPROVAL, APPROVAL, APPROVAL, DISK_LOW],
    );
    let (code, out, err) = world.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("has DiskLow trials (scenario-1-trial-15)"),
        "{err}"
    );
    assert!(world.records().is_empty(), "{:?}", world.records());
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

/// What a probing candidate's `build.rs` does first: fails the build when it can read a host
/// file outside it, write beside that file, or reach a listener on the loopback.
const PROBE: &str = r#"
fn probe() {
    if std::fs::read_to_string("__SECRET__").is_ok() {
        panic!("the build read a host file outside it");
    }
    if std::fs::write("__OUTSIDE__", "planted by the build\n").is_ok() {
        panic!("the build wrote outside it");
    }
    if std::net::TcpStream::connect(("127.0.0.1", __PORT__)).is_ok() {
        panic!("the build reached the network");
    }
}
"#;

/// story:confinement-through-substrate-on-linux-and-macos: a promotion's build goes through
/// the Substrate adapter a run's `--confinement substrate` chooses, each cargo step naming it;
/// the candidate's `build.rs` reads no host file outside the build, writes none and reaches no
/// listener, and the binary it built is installed and recorded. Where no adapter of this host
/// serves the build, the case prints `skipped: missing prerequisite: …` and returns.
#[test]
fn confinement_promotion_build_confined() {
    let world = World::new("substrate-build", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let secret = world.outer.join("secret.txt");
    write(&secret, "operator-secret\n");
    let outside = world.outer.join("planted-by-the-build");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback listener");
    listener.set_nonblocking(true).expect("non-blocking");
    let port = listener.local_addr().expect("an address").port();
    let probe = PROBE
        .replace("__SECRET__", secret.to_str().expect("utf-8"))
        .replace("__OUTSIDE__", outside.to_str().expect("utf-8"))
        .replace("__PORT__", &port.to_string());
    write(
        &world.repo.join("crates/takt-cli/build.rs"),
        &format!(
            "{}{probe}",
            TAKT_BUILD.replacen("fn main() {\n", "fn main() {\n    probe();\n", 1)
        ),
    );
    let revision = world.commit("probe");
    let card = world.passing_card("probe", &revision);
    let (code, out, err) = world.promote(&card, &[]);
    if err.contains("no Substrate adapter on this host serves") {
        eprintln!("skipped: missing prerequisite: {}", err.trim());
        return;
    }
    assert_eq!(
        code, 0,
        "story:confinement-through-substrate-on-linux-and-macos: the confined build holds: {out}{err}"
    );
    for step in ["cargo build ", "cargo install "] {
        assert!(
            out.lines()
                .any(|line| line.starts_with(step) && line.contains("(confined by Substrate ")),
            "story:confinement-through-substrate-on-linux-and-macos: {step}names the adapter: {out}"
        );
    }
    assert!(
        !outside.exists(),
        "story:confinement-through-substrate-on-linux-and-macos: nothing written outside the build"
    );
    assert!(
        listener.accept().is_err(),
        "story:confinement-through-substrate-on-linux-and-macos: the build reached no listener"
    );
    assert_eq!(
        world.version(),
        format!("takt 0.1.0 ({revision})"),
        "story:confinement-through-substrate-on-linux-and-macos: the confined build is installed"
    );
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
    // Where the build and the fresh root are as the candidate sees them: inside a Linux
    // adapter's mounts on Linux, at their host paths on macOS (the build under the cache, the
    // fresh root beside the install root).
    let (target, install) = if cfg!(target_os = "macos") {
        (
            format!("{}/", world.outer.join("cache/takt/promote").display()),
            format!("{}/.takt-promote-", world.root.display()),
        )
    } else {
        (format!("{INSIDE_TARGET}/"), format!("{INSIDE_INSTALL}/"))
    };
    write(
        &world.repo.join("crates/takt-cli/src/main.rs"),
        &FLIPPING_MAIN
            .replace("__TARGET__", &target)
            .replace("__INSTALL__", &install),
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
    let text = fs::read_to_string(head(&world)).expect("the head");
    assert!(
        text.contains(&format!("\"{BASELINE_REVISION}.json\"")),
        "the head points back at the record before it: {text}"
    );
    assert!(
        fs::read_dir(world.root.join("bin"))
            .expect("bin")
            .flatten()
            .all(|entry| entry.file_name() == "takt"),
        "no backup is left beside the binary"
    );
}

/// What a held candidate's `build.rs` does first: marks its clone `building`, then waits for
/// the test to put `go` beside it (a minute at most, so a test that never does cannot hang).
const HOLD: &str = r#"
fn hold() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let root = std::path::Path::new(&dir).join("../..");
    let _ = std::fs::write(root.join("building"), "");
    for _ in 0..600 {
        if root.join("go").exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
"#;

/// The clones under `world`'s build directory whose held `build.rs` has marked them `building`.
fn building(world: &World) -> Vec<PathBuf> {
    fs::read_dir(world.outer.join("cache/takt/promote"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path().join("source"))
                .filter(|source| source.join("building").exists())
                .collect()
        })
        .unwrap_or_default()
}

/// Polls `done` every 20 ms for a minute; panics naming `what` when it never holds.
fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..3000 {
        if done() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("waited a minute for {what}");
}

/// Two promotions at once run one at a time (probe N12): the store's lock is held from the
/// reading through the build, the record and the install. The first candidate's `build.rs`
/// holds its build until the test releases it; the second promotion starts only once that
/// build has begun, and the first is released only once the second has exited or begun a
/// build of its own. The second refuses naming the store's lock; the records form one chain
/// from the baseline, the head names the last, and the installed binary is the one its record
/// names.
#[test]
fn two_promotions_at_once_run_one_at_a_time_and_chain() {
    let world = World::new("two-at-once", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    write(
        &world.repo.join("crates/takt-cli/build.rs"),
        &format!(
            "{}{HOLD}",
            TAKT_BUILD.replacen("fn main() {\n", "fn main() {\n    hold();\n", 1)
        ),
    );
    let held = world.commit("hold");
    let card = world.passing_card("one", &held);
    let second = world.outer.join("candidate-two");
    git(
        &world.outer,
        &[
            "clone",
            "-q",
            "--no-hardlinks",
            world.outer.join("origin.git").to_str().expect("utf-8"),
            second.to_str().expect("utf-8"),
        ],
    );
    write(&second.join("two.txt"), "two\n");
    git(&second, &["add", "-A"]);
    git(&second, &["commit", "-q", "-m", "two"]);
    git(&second, &["push", "-q", "origin", "main"]);
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
    wait_until("the first build to begin", || !building(&world).is_empty());
    let mut second_run = spawn(&second, &other_card);
    wait_until("the second promotion to exit or begin a build", || {
        second_run
            .try_wait()
            .expect("the second's status")
            .is_some()
            || building(&world).len() > 1
    });
    for source in building(&world) {
        write(&source.join("go"), "");
    }
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
    let (code, out, err) = &outputs[0];
    assert_eq!(*code, 0, "the first promotes: {out}{err}");
    let (code, out, err) = &outputs[1];
    assert_eq!(*code, 1, "the second is refused: {out}{err}");
    assert!(
        err.contains("holds the store lock"),
        "the second's refusal names the store's lock: {err}"
    );
    let promoted = [&outputs[0].1];
    // The records form one chain from the baseline: each names the digest of the one before
    // it, and every record in the store is on it.
    let names: Vec<String> = world
        .records()
        .into_iter()
        .filter(|name| !name.ends_with(".scorecard.json"))
        .collect();
    let mut chain = vec![format!("{BASELINE_REVISION}.json")];
    while let Some(next) = names.iter().find(|name| {
        let previous = sha256(&world.store.join(chain.last().expect("a record")));
        world
            .record(name)
            .contains(&format!("\"previous_sha256\": \"{previous}\""))
    }) {
        chain.push(next.clone());
    }
    let mut sorted = chain.clone();
    sorted.sort();
    assert_eq!(sorted, names, "every record is on one chain: {chain:?}");
    assert_eq!(
        chain.len(),
        1 + promoted.len(),
        "a record a promotion, nothing else: {chain:?} {outputs:?}"
    );
    // Each promotion's installed binary is the one its record names.
    for out in &promoted {
        let line = out
            .lines()
            .find_map(|line| line.strip_prefix("promoted: "))
            .unwrap_or_else(|| panic!("a promoted line: {out}"));
        let (_, rest) = line.split_once(" at ").expect("the revision");
        let (revision, rest) = rest.split_once(", sha256 ").expect("the digest");
        let (digest, path) = rest.split_once("; recorded: ").expect("the record");
        let record = fs::read_to_string(path).expect("the promotion's record");
        assert_eq!(field(&record, "revision"), revision, "{record}");
        assert_eq!(field(&record, "binary_sha256"), digest, "{record}");
    }
    let last = chain.last().expect("the latest record");
    let record = world.record(last);
    assert_eq!(
        world.version(),
        format!("takt 0.1.0 ({})", field(&record, "revision")),
        "the installed binary is the latest record's"
    );
    assert_eq!(field(&record, "binary_sha256"), sha256(&world.installed()));
    let text = fs::read_to_string(head(&world)).expect("the head");
    assert!(
        text.contains(&format!("\"{last}\"")) && text.contains(&sha256(&world.store.join(last))),
        "the head names the last record: {text}"
    );
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

/// The chain's head, beside the store: `<state>/takt/promotions.head`.
fn head(world: &World) -> PathBuf {
    world.outer.join("state/takt/promotions.head")
}

/// story:promotion-chain-head-anchored: after a refusal, the refusal deleted and the same
/// revision promoted again without `--rejudge` exits 1 naming the missing record, `--dry-run`
/// too; nothing is written or installed and the head is left as it was.
#[test]
fn a_removed_latest_refusal_is_refused_naming_it() {
    let world = World::new("head-removed", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let worse = world.candidate_card("worse", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&worse, &[]);
    assert_eq!(code, 3, "{out}{err}");
    let refusal = format!("{}.refused.json", world.revision);
    fs::remove_file(world.store.join(&refusal)).expect("removed");
    let passing = world.candidate_card("passing", BASELINE_TOKENS, FIVE_APPROVALS);
    for extra in [&["--dry-run"][..], &[]] {
        let (code, out, err) = world.promote(&passing, extra);
        assert_eq!(code, 1, "{extra:?}: {out}{err}");
        assert!(
            err.contains(&refusal) && err.contains("promotions.head") && err.contains("missing"),
            "{extra:?}: the missing record is named: {err}"
        );
    }
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// story:promotion-chain-head-anchored: after a refusal, the refusal's file replaced with an
/// older record's bytes, the same revision promoted again exits 1 naming the record the head
/// names, `--dry-run` too; nothing is written or installed.
#[test]
fn a_latest_refusal_replaced_by_an_older_records_bytes_is_refused_naming_it() {
    let world = World::new("head-replaced", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let worse = world.candidate_card("worse", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&worse, &[]);
    assert_eq!(code, 3, "{out}{err}");
    let refusal = format!("{}.refused.json", world.revision);
    fs::copy(
        world.store.join(format!("{BASELINE_REVISION}.json")),
        world.store.join(&refusal),
    )
    .expect("replaced");
    let passing = world.candidate_card("passing", BASELINE_TOKENS, FIVE_APPROVALS);
    for extra in [&["--dry-run"][..], &[]] {
        let (code, out, err) = world.promote(&passing, extra);
        assert_eq!(code, 1, "{extra:?}: {out}{err}");
        assert!(
            err.contains(&refusal) && err.contains("promotions.head") && err.contains("replaced"),
            "{extra:?}: the replaced record is named: {err}"
        );
    }
    world.assert_untouched();
    let mut expected = baseline_files();
    expected.push(refusal);
    expected.sort();
    assert_eq!(world.records(), expected, "nothing recorded");
}

/// story:promotion-chain-head-anchored D2, D3: a store with records and no head is read on
/// trust whenever no head is found, said on stderr; a dry run writes no head, a promotion writes
/// it from the chain as it stands and then names each record it writes; a later read, the head
/// in place, says nothing of trust.
#[test]
fn a_store_without_a_head_is_read_on_trust_whenever_none_is_found() {
    let world = World::new("head-trust", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = world.candidate_card("trust", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 3, "{out}{err}");
    assert!(err.contains("on trust"), "{err}");
    assert!(!head(&world).exists(), "a dry run writes no head");
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 3, "{out}{err}");
    assert!(
        err.contains("on trust") && err.contains(&format!("{BASELINE_REVISION}.json")),
        "{err}"
    );
    let refused = world.store.join(format!("{}.refused.json", world.revision));
    let text = fs::read_to_string(head(&world)).expect("the head");
    assert!(
        text.contains(&format!("{}.refused.json", world.revision))
            && text.contains(&sha256(&refused)),
        "the head names the latest record: {text}"
    );
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(!err.contains("on trust"), "read on trust once: {err}");
}

/// story:promote-binds-scorecard-and-merge: a clean branch tip that is not an ancestor of
/// `main` is refused naming `main`, nothing written or installed; once merged into `main`, the
/// same revision and scorecard proceed.
#[test]
fn a_revision_not_merged_into_main_is_refused_naming_main() {
    let world = World::new("unmerged", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    git(&world.repo, &["checkout", "-q", "-b", "feature"]);
    write(&world.repo.join("feature.txt"), "feature\n");
    let tip = world.commit("feature");
    let card = world.passing_card("unmerged", &tip);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("{tip} is not an ancestor of origin/main")),
        "{err}"
    );
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
    let (code, out, err) = world.promote(&card, &["--baseline-only", "--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("not an ancestor of origin/main"), "{err}");
    git(&world.repo, &["checkout", "-q", "main"]);
    git(&world.repo, &["merge", "-q", "--ff-only", "feature"]);
    git(&world.repo, &["push", "-q", "origin", "main"]);
    git(&world.repo, &["checkout", "-q", "feature"]);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "merged, it proceeds: {out}{err}");
}

/// story:promote-binds-scorecard-and-merge: a scorecard whose runs were not all made with the
/// binary of the revision promoted (the run record's `opened` line names it) is refused naming
/// the first run that was not, whether it names another revision, the revision dirty, or none;
/// `--baseline-only` holds to the same check; matching runs proceed.
#[test]
fn a_scorecard_of_another_binarys_runs_is_refused_naming_the_row() {
    let world = World::new("other-binary", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let own = world.revision.as_str();
    let dirty = format!("{own}-dirty");
    for (label, binaries, row, named) in [
        (
            "another",
            [
                Some(own),
                Some(own),
                Some(BASELINE_REVISION),
                Some(own),
                None,
            ],
            "scenario-1-trial-13#1",
            BASELINE_REVISION,
        ),
        (
            "dirty",
            [
                Some(own),
                Some(dirty.as_str()),
                Some(own),
                Some(own),
                Some(own),
            ],
            "scenario-1-trial-12#1",
            dirty.as_str(),
        ),
        (
            "none",
            [None, Some(own), Some(own), Some(own), Some(own)],
            "scenario-1-trial-11#1",
            "no revision",
        ),
    ] {
        let card = world.card_made_by(label, own, BASELINE_TOKENS, FIVE_APPROVALS, binaries);
        let (code, out, err) = world.promote(&card, &["--dry-run"]);
        assert_eq!(code, 1, "{label}: {out}{err}");
        assert!(
            err.contains(&format!("run {row} ")) && err.contains(named) && err.contains(own),
            "{label}: the first such run is named: {err}"
        );
        assert!(!err.contains("scenario-1-trial-15"), "{label}: {err}");
    }
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
    let card = world.candidate_card("own", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "matching runs proceed: {out}{err}");

    let fresh = World::new("other-binary-baseline", BASELINE_TOKENS, FIVE_APPROVALS);
    fresh.install_stand_in(&fresh.revision);
    let card = fresh.card_made_by(
        "base",
        &fresh.revision,
        BASELINE_TOKENS,
        FIVE_APPROVALS,
        [Some(BASELINE_REVISION); 5],
    );
    let (code, out, err) = fresh.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("run scenario-1-trial-11#1 "), "{err}");
    assert_eq!(fresh.records(), Vec::<String>::new());
    let card = fresh.candidate_card("base-own", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = fresh.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 0, "{out}{err}");
}

/// story:promotion-refusal-is-not-final: a refused revision is judged again with `--rejudge` on
/// a scorecard of new runs, each record naming the refusal before it; the scorecard refused is
/// no new runs, and `--rejudge` without a refusal, or with `--baseline-only`, is refused.
#[test]
fn a_refused_revision_is_judged_again_with_rejudge_on_new_runs() {
    let world = World::new("rejudge", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let worse = world.candidate_card("worse", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&worse, &["--rejudge", "--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("was never refused"), "{err}");
    let (code, out, err) = world.promote(&worse, &[]);
    assert_eq!(code, 3, "{out}{err}");
    let first = format!("{}.refused.json", world.revision);
    assert_eq!(world.records().len(), 3, "{:?}", world.records());
    let (code, out, err) = world.promote(&worse, &["--rejudge"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("new runs") && err.contains(&first),
        "the scorecard refused is no new runs: {err}"
    );
    let (code, out, err) = world.promote(&worse, &["--rejudge", "--baseline-only"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("--rejudge"), "{err}");

    let again = world.trials_card("worse-again", 21, BASELINE_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&again, &["--rejudge"]);
    assert_eq!(code, 3, "judged again, and refused again: {out}{err}");
    let records = world.records();
    assert_eq!(
        records.len(),
        4,
        "a second refusal beside the first: {records:?}"
    );
    let second = records
        .iter()
        .find(|name| name.contains(".refused") && **name != first)
        .expect("the second refusal")
        .clone();
    let text = world.record(&second);
    assert!(
        text.contains("\"rejudges\"")
            && text.contains(&first)
            && text.contains(&sha256(&world.store.join(&first))),
        "the second refusal names the first: {text}"
    );

    let fresh = world.trials_card("new-runs", 31, WITHIN_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&fresh, &["--rejudge"]);
    assert_eq!(code, 0, "{out}{err}");
    let record = world.record(&format!("{}.json", world.revision));
    assert!(
        record.contains("\"kind\": \"promotion\"")
            && record.contains("\"rejudges\"")
            && record.contains(&second)
            && record.contains(&sha256(&world.store.join(&second))),
        "the promotion names the refusal before it: {record}"
    );
    assert_eq!(world.version(), format!("takt 0.1.0 ({})", world.revision));
}

/// story:promotion-refusal-is-not-final: a candidate whose scorecard holds a scenario the
/// baseline lacks, with fewer cases than the rule's trials, is refused naming the scenario and
/// the trials, so its copy can never become a baseline every later promotion fails against.
#[test]
fn a_candidate_scenario_with_fewer_cases_than_the_trials_is_refused() {
    let world = World::new("new-scenario", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let cases = world.outer.join("cases-new-scenario");
    for (n, tokens) in BASELINE_TOKENS.iter().enumerate() {
        run(
            &cases,
            &format!("scenario-1-trial-{}", n + 11),
            *tokens,
            APPROVAL,
            Some(&world.revision),
        );
    }
    run(
        &cases,
        "scenario-2-trial-1",
        100_000,
        APPROVAL,
        Some(&world.revision),
    );
    let card = World::save(
        &cases,
        &world.outer.join("saves-new-scenario"),
        &world.revision,
    );
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("scenario 2") && err.contains("has 1 case,") && err.contains("needs 5"),
        "{err}"
    );
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// The scorecard of no case at all, saved under `revision` in a directory of its own named by
/// `label`; the file.
fn empty_card(world: &World, label: &str, revision: &str) -> PathBuf {
    let cases = world.outer.join(format!("cases-{label}"));
    fs::create_dir_all(&cases).expect("a directory");
    World::save(
        &cases,
        &world.outer.join(format!("saves-{label}")),
        revision,
    )
}

/// story:the-judge-requires-the-whole-corpus: a promotion of a candidate with no scenario case
/// against a baseline with none is refused with exit 1 saying there is no scenario case,
/// `--dry-run` too; nothing is recorded or installed.
#[test]
fn empty_against_empty_is_refused() {
    let world = World::new("corpus-empty", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&empty_card(&world, "empty-baseline", BASELINE_REVISION));
    let empty = empty_card(&world, "empty-candidate", &world.revision);
    for extra in [&["--dry-run"][..], &[]] {
        let (code, out, err) = world.promote(&empty, extra);
        assert_eq!(code, 1, "empty against empty {extra:?}: {out}{err}");
        assert!(err.contains("no scenario case"), "{err}");
    }
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// story:the-judge-requires-the-whole-corpus: a scenario only the candidate has, with the rule's
/// five cases, is refused with exit 1 naming it, `--dry-run` too, and gets no verdict; nothing is
/// recorded or installed.
#[test]
fn a_scenario_only_the_candidate_has_is_refused_naming_it() {
    let world = World::new("corpus-candidate-only", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let cases = world.outer.join("cases-two");
    two_scenarios(&cases, 11, APPROVAL, APPROVAL, &world.revision);
    let two = World::save(&cases, &world.outer.join("saves-two"), &world.revision);
    for extra in [&["--dry-run"][..], &[]] {
        let (code, out, err) = world.promote(&two, extra);
        assert_eq!(
            code, 1,
            "a scenario only the candidate has {extra:?}: {out}{err}"
        );
        assert!(
            err.contains("scenario 2") && err.contains("not in the saved scorecard"),
            "{err}"
        );
        assert!(!out.contains("verdict:"), "{out}");
    }
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// story:the-judge-requires-the-whole-corpus: a scenario only the baseline has, with the rule's
/// five cases there, is refused with exit 1 naming it, `--dry-run` too; nothing is recorded or
/// installed.
#[test]
fn a_scenario_only_the_baseline_has_is_refused_naming_it() {
    let world = World::new("corpus-baseline-only", BASELINE_TOKENS, FIVE_APPROVALS);
    let cases = world.outer.join("cases-two-baseline");
    two_scenarios(&cases, 1, APPROVAL, APPROVAL, BASELINE_REVISION);
    world.record_baseline_of(&World::save(
        &cases,
        &world.outer.join("saves-two-baseline"),
        BASELINE_REVISION,
    ));
    let one = world.candidate_card("one", BASELINE_TOKENS, FIVE_APPROVALS);
    for extra in [&["--dry-run"][..], &[]] {
        let (code, out, err) = world.promote(&one, extra);
        assert_eq!(
            code, 1,
            "a scenario only the baseline has {extra:?}: {out}{err}"
        );
        assert!(err.contains("scenario 2"), "{err}");
    }
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// story:the-judge-requires-the-whole-corpus: `takt promote --help` names the corpus refusals and
/// how a scenario joins.
#[test]
fn help_names_the_corpus_refusals() {
    let (code, help, _) = takt(&["promote", "--help"], &[]);
    assert_eq!(code, 0);
    for refusal in [
        "no scenario case",
        "not in the saved scorecard",
        "a scenario it adds whole",
        "--baseline-only with a scorecard holding no scenario case",
    ] {
        assert!(help.contains(refusal), "{refusal} in {help}");
    }
}

/// story:the-judge-requires-the-whole-corpus: a new scenario joins the corpus only through
/// `--rebaseline`, with a scorecard of the baseline revision holding the stored cases exactly and
/// every case of the new scenario, under the same metric definitions; the record chains, and a
/// candidate with both scenarios, refused before it, is then judged on both.
#[test]
fn a_rebaseline_adds_a_scenario_and_a_candidate_is_judged_on_it() {
    let world = World::new("rebaseline-joins", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let candidate_cases = world.outer.join("cases-candidate");
    two_scenarios(&candidate_cases, 11, APPROVAL, APPROVAL, &world.revision);
    let candidate = World::save(
        &candidate_cases,
        &world.outer.join("saves-candidate"),
        &world.revision,
    );
    let (code, out, err) = world.promote(&candidate, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("scenario 2"), "{err}");
    let cases = world.outer.join("cases-joined");
    two_scenarios(&cases, 1, APPROVAL, APPROVAL, BASELINE_REVISION);
    let joined = World::save(&cases, &world.outer.join("saves-joined"), BASELINE_REVISION);
    let (code, out, err) = rebaseline(&world, &joined);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("adds scenario 2"), "{out}");
    let records = world.records();
    assert_eq!(records.len(), 4, "{records:?}");
    let name = records
        .iter()
        .find(|name| name.contains(".rebaseline-") && !name.ends_with(".scorecard.json"))
        .expect("the rebaseline record");
    let record = world.record(name);
    for line in [
        "\"kind\": \"rebaseline\"".to_owned(),
        format!("\"replaces_sha256\": \"{}\"", sha256(&world.baseline)),
        format!("\"scorecard_sha256\": \"{}\"", sha256(&joined)),
    ] {
        assert!(record.contains(&line), "{line} in {record}");
    }
    world.assert_untouched();
    let (code, out, err) = world.promote(&candidate, &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    for scenario in ["1", "2"] {
        assert!(
            out.contains(&format!(
                "scenario {scenario}  outcomes by rank held  tokens held  not worse"
            )),
            "scenario {scenario} in {out}"
        );
    }
}

/// story:the-judge-requires-the-whole-corpus: `--rebaseline` adds whole scenarios and nothing
/// else, so a scorecard adding a case to a scenario the baseline has, beside a new scenario, is
/// refused naming the case, nothing written.
#[test]
fn a_rebaseline_adding_a_case_to_a_stored_scenario_is_refused_naming_it() {
    let world = World::new("rebaseline-extra-case", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let cases = world.outer.join("cases-extra");
    two_scenarios(&cases, 1, APPROVAL, APPROVAL, BASELINE_REVISION);
    run(
        &cases,
        "scenario-1-trial-6",
        1,
        ERROR,
        Some(BASELINE_REVISION),
    );
    let extra = World::save(&cases, &world.outer.join("saves-extra"), BASELINE_REVISION);
    let (code, out, err) = rebaseline(&world, &extra);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("case scenario-1-trial-6 ") && err.contains("scenario 1"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// story:the-judge-requires-the-whole-corpus, correction 1: a scenario a rebaseline adds needs
/// the rule's trials as every baseline scenario does, so one added with a single case is
/// refused naming it and the trials; nothing written.
#[test]
fn a_rebaseline_adding_a_scenario_with_fewer_cases_than_the_trials_is_refused() {
    let world = World::new("rebaseline-one-case", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let cases = world.outer.join("cases-one");
    for (n, tokens) in BASELINE_TOKENS.iter().enumerate() {
        run(
            &cases,
            &format!("scenario-1-trial-{}", n + 1),
            *tokens,
            APPROVAL,
            Some(BASELINE_REVISION),
        );
    }
    run(
        &cases,
        "scenario-2-trial-1",
        100,
        APPROVAL,
        Some(BASELINE_REVISION),
    );
    let card = World::save(&cases, &world.outer.join("saves-one"), BASELINE_REVISION);
    let (code, out, err) = rebaseline(&world, &card);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("scenario 2") && err.contains("needs 5"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// story:the-judge-requires-the-whole-corpus, correction 1: the store is read under the same rule
/// a rebaseline is written under, so a rebaseline copy edited to hold a case added to a stored
/// scenario, its record and the head re-pointed to it, refuses the next promotion naming the
/// case.
#[test]
fn a_rebaseline_copy_holding_a_case_added_to_a_stored_scenario_refuses_the_store() {
    let world = World::new("chain-extra-case", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let cases = world.outer.join("cases-joined");
    two_scenarios(&cases, 1, APPROVAL, APPROVAL, BASELINE_REVISION);
    let joined = World::save(&cases, &world.outer.join("saves-joined"), BASELINE_REVISION);
    let (code, out, err) = rebaseline(&world, &joined);
    assert_eq!(code, 0, "{out}{err}");
    // The same rebaseline with scenario-1-trial-6 added, put in its place by hand.
    let extra = world.outer.join("cases-extra");
    two_scenarios(&extra, 1, APPROVAL, APPROVAL, BASELINE_REVISION);
    run(
        &extra,
        "scenario-1-trial-6",
        1,
        APPROVAL,
        Some(BASELINE_REVISION),
    );
    let forged = World::save(&extra, &world.outer.join("saves-extra"), BASELINE_REVISION);
    let name = world
        .records()
        .into_iter()
        .find(|name| name.contains(".rebaseline-") && !name.ends_with(".scorecard.json"))
        .expect("the rebaseline record");
    let (old_digest, new_digest) = (sha256(&joined), sha256(&forged));
    let copy = |digest: &str| {
        world.store.join(format!(
            "{BASELINE_REVISION}.rebaseline-{digest}.scorecard.json"
        ))
    };
    fs::remove_file(copy(&old_digest)).expect("the copy removed");
    fs::copy(&forged, copy(&new_digest)).expect("the forged copy");
    let text = world.record(&name).replace(&old_digest, &new_digest);
    let renamed = name.replace(&old_digest, &new_digest);
    fs::remove_file(world.store.join(&name)).expect("the record removed");
    write(&world.store.join(&renamed), &text);
    world.rehead(&renamed);
    let candidate_cases = world.outer.join("cases-candidate");
    two_scenarios(&candidate_cases, 11, APPROVAL, APPROVAL, &world.revision);
    let candidate = World::save(
        &candidate_cases,
        &world.outer.join("saves-candidate"),
        &world.revision,
    );
    let (code, out, err) = world.promote(&candidate, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("scenario-1-trial-6") && err.contains("the store is not read past this"),
        "{err}"
    );
}

/// story:the-judge-requires-the-whole-corpus, correction 1: `--baseline-only` with a scorecard
/// holding no scenario case would record a baseline that judges nothing, so it is refused saying
/// so, `--dry-run` too; nothing is recorded and the installed binary is untouched.
#[test]
fn baseline_only_with_no_scenario_case_is_refused() {
    let world = World::new("baseline-empty", BASELINE_TOKENS, FIVE_APPROVALS);
    world.install_stand_in(&world.revision);
    let empty = empty_card(&world, "empty", &world.revision);
    for extra in [&["--baseline-only", "--dry-run"][..], &["--baseline-only"]] {
        let (code, out, err) = world.promote(&empty, extra);
        assert_eq!(code, 1, "an empty baseline {extra:?}: {out}{err}");
        assert!(err.contains("no scenario case"), "{err}");
    }
    assert!(world.records().is_empty(), "{:?}", world.records());
    assert_eq!(
        fs::read_to_string(world.installed()).expect("the installed binary"),
        stand_in(&world.revision),
        "the installed binary is untouched"
    );
}

/// story:promotion-refusal-is-not-final, correction 1: `--rejudge` judges new runs only, so a
/// scorecard sharing a case with a scorecard the revision was refused on is refused naming the
/// case, whatever its digest; one of new cases only is judged.
#[test]
fn a_rejudge_sharing_a_case_with_a_refused_scorecard_is_refused_naming_the_case() {
    let world = World::new("rejudge-cases", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let worse = world.candidate_card("worse", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&worse, &[]);
    assert_eq!(code, 3, "{out}{err}");
    let refusal = format!("{}.refused.json", world.revision);
    let overlap = world.trials_card("overlap", 15, BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&overlap, &["--rejudge", "--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("case scenario-1-trial-15 ")
            && err.contains("new runs")
            && err.contains(&refusal),
        "the shared case and the refusal are named: {err}"
    );
    let fresh = world.trials_card("fresh", 21, BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&fresh, &["--rejudge", "--dry-run"]);
    assert_eq!(code, 0, "new cases only are judged: {out}{err}");
}

/// `scorecard` with its outcome metric at definition version 1, as a takt before the held-out
/// check saved it, and with `edits` applied, written in a directory of its own named by
/// `label`; the file.
fn outcome_v1(world: &World, scorecard: &Path, label: &str, edits: &[(&str, &str)]) -> PathBuf {
    outcome_at(world, scorecard, label, 1, edits)
}

/// `scorecard` with its outcome metric at definition `version`, as a takt knowing that version
/// saved it, and with `edits` applied, written in a directory of its own named by `label`; the
/// file.
fn outcome_at(
    world: &World,
    scorecard: &Path,
    label: &str,
    version: u32,
    edits: &[(&str, &str)],
) -> PathBuf {
    let mut text = fs::read_to_string(scorecard).expect("the scorecard");
    let to = format!("\"outcome\": {version},");
    for (from, to) in [("\"outcome\": 3,", to.as_str())].iter().chain(edits) {
        assert_eq!(text.matches(from).count(), 1, "{from} in {text}");
        text = text.replace(from, to);
    }
    let path = world
        .outer
        .join(format!("saves-{label}"))
        .join(scorecard.file_name().expect("a file name"));
    write(&path, &text);
    path
}

/// `takt promote --rebaseline <scorecard>` with the world's store and install root.
fn rebaseline(world: &World, scorecard: &Path) -> (i32, String, String) {
    let env = world.env();
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    takt(
        &[
            "promote",
            "--rebaseline",
            scorecard.to_str().expect("utf-8"),
            "--install-root",
            world.root.to_str().expect("utf-8"),
        ],
        &env,
    )
}

/// story:scorecard-outcome-from-held-out-check, correction 1: a baseline saved under the outcome
/// definition of version 1 judges nothing under version 2; `--rebaseline` replaces its scorecard
/// with one of the same revision and the same cases under the newer definitions, appending a
/// record to the chain that names the scorecard replaced by its digest, and a candidate is then
/// judged against it.
#[test]
fn a_v1_outcome_baseline_is_rebaselined_and_a_candidate_judged_against_it() {
    let world = World::new("rebaseline", BASELINE_TOKENS, FIVE_APPROVALS);
    let v1 = outcome_v1(&world, &world.baseline, "v1", &[]);
    world.record_baseline_of(&v1);
    let card = world.candidate_card("candidate", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("metric outcome is at definition version 1"),
        "{err}"
    );
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 0, "{out}{err}");
    let records = world.records();
    assert_eq!(records.len(), 4, "{records:?}");
    let name = records
        .iter()
        .find(|name| name.contains(".rebaseline-") && !name.ends_with(".scorecard.json"))
        .expect("the rebaseline record");
    let record = world.record(name);
    for line in [
        "\"kind\": \"rebaseline\"".to_owned(),
        format!("\"revision\": \"{BASELINE_REVISION}\""),
        format!("\"replaces_sha256\": \"{}\"", sha256(&v1)),
        format!("\"scorecard_sha256\": \"{}\"", sha256(&world.baseline)),
        format!(
            "\"previous_sha256\": \"{}\"",
            sha256(&world.store.join(format!("{BASELINE_REVISION}.json")))
        ),
    ] {
        assert!(record.contains(&line), "{line} in {record}");
    }
    world.assert_untouched();
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(&format!("against baseline {BASELINE_REVISION}")) && out.contains("not worse"),
        "{out}"
    );
}

/// story:outcome-metric-takes-version-3, acceptance 3: a baseline saved under the outcome
/// definition of version 2, before `Unperformable` and `DiskLow` changed what the ranks mean,
/// judges no candidate under version 3; after `--rebaseline` with the baseline revision's
/// scorecard under version 3 a candidate is judged against it.
#[test]
fn a_v2_outcome_baseline_is_compared_with_a_v3_candidate_only_after_a_rebaseline() {
    let world = World::new("rebaseline-v2", BASELINE_TOKENS, FIVE_APPROVALS);
    let v2 = outcome_at(&world, &world.baseline, "v2", 2, &[]);
    world.record_baseline_of(&v2);
    let card = world.candidate_card("candidate", BASELINE_TOKENS, FIVE_APPROVALS);
    let text = fs::read_to_string(&card).expect("the candidate");
    assert_eq!(text.matches("\"outcome\": 3,").count(), 1, "{text}");
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("metric outcome is at definition version 2; this takt knows version 3"),
        "{err}"
    );
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 0, "{out}{err}");
    let name = world
        .records()
        .into_iter()
        .find(|name| name.contains(".rebaseline-") && !name.ends_with(".scorecard.json"))
        .expect("the rebaseline record");
    let record = world.record(&name);
    for line in [
        format!("\"replaces_sha256\": \"{}\"", sha256(&v2)),
        format!("\"scorecard_sha256\": \"{}\"", sha256(&world.baseline)),
    ] {
        assert!(record.contains(&line), "{line} in {record}");
    }
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(&format!("against baseline {BASELINE_REVISION}")) && out.contains("not worse"),
        "{out}"
    );
}

/// `--rebaseline` with a scorecard of another revision than the baseline's is refused naming
/// both, nothing written.
#[test]
fn a_rebaseline_of_another_revision_is_refused() {
    let world = World::new("rebaseline-revision", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&outcome_v1(&world, &world.baseline, "v1", &[]));
    let other = world.candidate_card("other", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = rebaseline(&world, &other);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!(
            "its revision {} is not the baseline's {BASELINE_REVISION}",
            world.revision
        )),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// `--rebaseline` with a scorecard whose cases are not the baseline's exactly is refused naming
/// the first case only one side has, nothing written.
#[test]
fn a_rebaseline_on_other_cases_is_refused_naming_the_case() {
    let world = World::new("rebaseline-cases", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&outcome_v1(&world, &world.baseline, "v1", &[]));
    let cases = world.outer.join("cases-other");
    for (n, tokens) in [1, 2, 3, 4, 6].iter().zip(BASELINE_TOKENS) {
        run(
            &cases,
            &format!("scenario-1-trial-{n}"),
            tokens,
            APPROVAL,
            Some(BASELINE_REVISION),
        );
    }
    let other = World::save(&cases, &world.outer.join("saves-other"), BASELINE_REVISION);
    let (code, out, err) = rebaseline(&world, &other);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("case scenario-1-trial-5 ") && err.contains("cases exactly"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// `--rebaseline` with a scorecard under no newer metric definition than the stored one's is
/// refused, nothing written.
#[test]
fn a_rebaseline_with_no_newer_metric_definition_is_refused() {
    let world = World::new("rebaseline-same", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("no metric definition is newer"), "{err}");
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// `--rebaseline` with a scorecard under an older definition of some metric than the stored
/// one's is refused naming the metric and both versions, nothing written.
#[test]
fn a_rebaseline_with_an_older_metric_definition_is_refused_naming_it() {
    let world = World::new("rebaseline-older", BASELINE_TOKENS, FIVE_APPROVALS);
    let stored = outcome_v1(
        &world,
        &world.baseline,
        "v1",
        &[("\"tokens\": 2,", "\"tokens\": 3,")],
    );
    world.record_baseline_of(&stored);
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("metric tokens is at definition version 2, below the stored 3"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// story:promote-binds-scorecard-and-merge, review W2: the mainline is the remote's `main`, so a
/// local `main` forced onto an unmerged branch tip does not make the tip merged.
#[test]
fn a_forced_local_main_does_not_make_a_branch_tip_merged() {
    let world = World::new("forced-main", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    git(&world.repo, &["checkout", "-q", "-b", "feature"]);
    write(&world.repo.join("feature.txt"), "feature\n");
    let tip = world.commit("feature");
    git(&world.repo, &["branch", "-f", "main", &tip]);
    let card = world.passing_card("forced-main", &tip);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("{tip} is not an ancestor of origin/main")),
        "{err}"
    );
}

/// story:promote-binds-scorecard-and-merge, review W3: with `--baseline-only --binary-sha256`,
/// a run whose `opened` line names a revision must name the stated one; only a run that names
/// none is taken on its `binary.txt` statement alone.
#[test]
fn a_stated_baseline_refuses_runs_naming_another_revision() {
    let world = World::new("stated-other", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let stated = Some((digest.as_str(), ""));
    let card = world.stated_card_made_by("stated-other", [stated; 5], Some(BASELINE_REVISION));
    let flags = [
        "--baseline-only",
        "--revision",
        world.revision.as_str(),
        "--binary-sha256",
        digest.as_str(),
        "--dry-run",
    ];
    let (code, out, err) = world.promote(&card, &flags);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("run scenario-1-trial-11#1 ") && err.contains(BASELINE_REVISION),
        "{err}"
    );
    let card = world.stated_card_made_by("stated-own", [stated; 5], Some(&world.revision));
    let (code, out, err) = world.promote(&card, &flags);
    assert_eq!(code, 0, "runs naming the stated revision: {out}{err}");
}

/// The fixture's runs carry no clock: every line of a record [`run`] writes is at [`AT`], so a
/// run's wall time does not depend on how long the writing took, and a check that compares the
/// rows of two directories (`--rebaseline`) never refuses on a wall time the fixture made up.
#[test]
fn the_fixture_writes_every_run_at_a_fixed_instant() {
    let outer = scratch("fixed-instants");
    for n in 1..=3 {
        run(&outer, "scenario-1-trial-1", n, APPROVAL, None);
    }
    let lines = Events::new(&outer.join(CASES_DIR).join("scenario-1-trial-1"))
        .read()
        .expect("the record reads");
    assert_eq!(lines.len(), 9, "{lines:?}");
    for line in &lines {
        assert_eq!(line["at"], AT, "{line}");
    }
}

/// story:scorecard-save-checks-binary, decision D2 as a stated baseline applies it: runs that
/// name no revision (no `binary`, or `unknown revision`) beside runs that name the stated one
/// are a mix, refused naming the first run that names none, as a save refuses it; runs that all
/// name none are taken on their statements.
#[test]
fn a_stated_baseline_refuses_a_mix_of_runs_naming_none_and_naming_the_revision() {
    let world = World::new("stated-mix", BASELINE_TOKENS, FIVE_APPROVALS);
    let digest = world.install_pre_revision();
    let stated = Some((digest.as_str(), ""));
    let own = Some(world.revision.as_str());
    let unknown = Some("unknown revision");
    let flags = [
        "--baseline-only",
        "--revision",
        world.revision.as_str(),
        "--binary-sha256",
        digest.as_str(),
        "--dry-run",
    ];
    for (label, binaries, named) in [
        ("none", [own, own, None, own, own], "scenario-1-trial-13#1"),
        (
            "unknown",
            [own, unknown, own, own, None],
            "scenario-1-trial-12#1",
        ),
    ] {
        let card =
            world.stated_card_of_binaries(&format!("stated-mix-{label}"), [stated; 5], binaries);
        let (code, out, err) = world.promote(&card, &flags);
        assert_eq!(code, 1, "{label}: {out}{err}");
        assert!(
            err.contains(&format!("run {named} ")) && err.contains("no revision"),
            "{label}: {err}"
        );
    }
    assert_eq!(world.records(), Vec::<String>::new(), "nothing recorded");
    let card = world.stated_card_of_binaries(
        "stated-unnamed",
        [stated; 5],
        [unknown, None, unknown, unknown, None],
    );
    let (code, out, err) = world.promote(&card, &flags);
    assert_eq!(code, 0, "runs that all name none: {out}{err}");
}

/// story:promotion-refusal-is-not-final, review W5: a record's `rejudges` is verified on read,
/// so one naming a refusal that is not in the chain with that digest refuses the store.
#[test]
fn a_rejudges_naming_no_refusal_in_the_chain_refuses_the_store() {
    let world = World::new("rejudges-edited", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let worse = world.candidate_card("worse", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&worse, &[]);
    assert_eq!(code, 3, "{out}{err}");
    let again = world.trials_card("again", 21, BASELINE_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&again, &["--rejudge"]);
    assert_eq!(code, 3, "{out}{err}");
    let second = world
        .records()
        .into_iter()
        .find(|name| name.contains(".refused-"))
        .expect("the second refusal");
    let path = world.store.join(&second);
    let text = fs::read_to_string(&path).expect("the record");
    let first = format!("{}.refused.json", world.revision);
    let named = format!("\"record\": \"{first}\"");
    assert_eq!(text.matches(&named).count(), 1, "{text}");
    fs::write(
        &path,
        text.replace(&named, "\"record\": \"nothing.refused.json\""),
    )
    .expect("edited");
    world.rehead(&second);
    let fresh = world.trials_card("fresh", 31, WITHIN_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&fresh, &["--rejudge", "--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&second) && err.contains("nothing.refused.json"),
        "the record and the refusal it names are named: {err}"
    );
}

/// story:scorecard-outcome-from-held-out-check, review N6: a saved scorecard whose row gives an
/// outcome its held-out result disagrees with, edited by hand, is refused on load, naming the
/// run.
#[test]
fn a_saved_outcome_disagreeing_with_its_held_out_result_is_refused() {
    let world = World::new("edited-outcome", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let cases = world.outer.join("cases-edited-outcome");
    for (n, tokens) in BASELINE_TOKENS.iter().enumerate() {
        let case = format!("scenario-1-trial-{}", n + 11);
        run(&cases, &case, *tokens, APPROVAL, Some(&world.revision));
        write(
            &cases.join(CASES_DIR).join(&case).join(HELD_OUT),
            "FAIL: x\n",
        );
    }
    let card = World::save(
        &cases,
        &world.outer.join("saves-edited-outcome"),
        &world.revision,
    );
    let text = fs::read_to_string(&card).expect("the scorecard");
    let edited = text.replace(
        "\"outcome\": \"failed\"",
        "\"outcome\": \"ApprovalRequired\"",
    );
    assert_ne!(edited, text);
    fs::write(&card, edited).expect("edited");
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("scenario-1-trial-11#1") && err.contains("held-out"),
        "{err}"
    );
}

/// Replaces `"<key>": "<old>"` by `"<key>": "<new>"` in the file at `path`, exactly once.
fn patch(path: &Path, key: &str, new: &str) {
    let text = fs::read_to_string(path).expect("read");
    let prefix = format!("\"{key}\": \"");
    let mut done = 0;
    let out: Vec<String> = text
        .lines()
        .map(|line| {
            if line.trim_start().starts_with(&prefix) {
                done += 1;
                let indent = &line[..line.len() - line.trim_start().len()];
                let comma = if line.trim_end().ends_with(',') {
                    ","
                } else {
                    ""
                };
                format!("{indent}\"{key}\": \"{new}\"{comma}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    assert_eq!(done, 1, "{key} in {}: {text}", path.display());
    fs::write(path, out.join("\n") + "\n").expect("patched");
}

/// The scorecard of five scenario 1 cases with these trial numbers, tokens and stops, each run
/// opened by `binary`, saved under `revision` in a directory of its own; the file.
fn card_of_ids(
    world: &World,
    label: &str,
    revision: &str,
    ids: [usize; 5],
    tokens: [u64; 5],
    stops: [&str; 5],
    binary: &str,
) -> PathBuf {
    let cases = world.outer.join(format!("cases-{label}"));
    for n in 0..5 {
        run(
            &cases,
            &format!("scenario-1-trial-{}", ids[n]),
            tokens[n],
            stops[n],
            Some(binary),
        );
    }
    World::save(
        &cases,
        &world.outer.join(format!("saves-{label}")),
        revision,
    )
}

/// A world whose baseline is current, a candidate refused on it (cases 11 to 15), and then the
/// stored copy put back to outcome definition 1 with the chain's digests re-pointed, as a takt
/// one definition older would have left the store; the world and the refusal's file name.
fn refused_then_stale(name: &str) -> (World, String) {
    let world = World::new(name, BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let worse = world.candidate_card("worse", WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&worse, &[]);
    assert_eq!(code, 3, "{out}{err}");
    let refusal = format!("{}.refused.json", world.revision);
    let copy = world
        .store
        .join(format!("{BASELINE_REVISION}.scorecard.json"));
    let v1 = outcome_v1(&world, &copy, "stale", &[]);
    fs::copy(&v1, &copy).expect("downgraded");
    let base = world.store.join(format!("{BASELINE_REVISION}.json"));
    patch(&base, "scorecard_sha256", &sha256(&copy));
    patch(
        &world.store.join(&refusal),
        "previous_sha256",
        &sha256(&base),
    );
    world.rehead(&refusal);
    (world, refusal)
}

/// Correction 2, item 1: `--rebaseline` binds each run to the stored one, so a scorecard with
/// the baseline's case ids but other runs (five errors at ten times the tokens) is refused,
/// naming the run and the field; nothing is written.
#[test]
fn a_rebaseline_of_other_runs_under_the_same_ids_is_refused() {
    let world = World::new("rebaseline-other-runs", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&outcome_v1(&world, &world.baseline, "v1", &[]));
    let lowered = card_of_ids(
        &world,
        "lowered",
        BASELINE_REVISION,
        [1, 2, 3, 4, 5],
        [1_500_000; 5],
        [ERROR; 5],
        BASELINE_REVISION,
    );
    let (code, out, err) = rebaseline(&world, &lowered);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("run scenario-1-trial-1#1 ") && err.contains("the stored run"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// Correction 2, item 1: the same case ids with another number of runs each are refused, naming
/// the case and both counts.
#[test]
fn a_rebaseline_with_more_runs_a_case_is_refused() {
    let world = World::new("rebaseline-more-runs", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&outcome_v1(&world, &world.baseline, "v1", &[]));
    let cases = world.outer.join("cases-multi");
    for n in 1..=5 {
        let case = format!("scenario-1-trial-{n}");
        run(
            &cases,
            &case,
            BASELINE_TOKENS[n - 1],
            APPROVAL,
            Some(BASELINE_REVISION),
        );
        run(&cases, &case, 2_000_000, ERROR, Some(BASELINE_REVISION));
    }
    let card = World::save(&cases, &world.outer.join("saves-multi"), BASELINE_REVISION);
    let (code, out, err) = rebaseline(&world, &card);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("case scenario-1-trial-1 has 2 runs") && err.contains("1 stored"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// Correction 2, item 1: a lowered rebaseline after a refusal is refused, so the refused
/// candidate's new runs, as worse as before, stay refused; an honest one is accepted, chains onto
/// the refusal and judges them the same.
#[test]
fn a_rebaseline_cannot_turn_a_refused_candidate_into_a_pass() {
    let (world, refusal) = refused_then_stale("rebaseline-flip");
    let lowered = card_of_ids(
        &world,
        "lowered",
        BASELINE_REVISION,
        [1, 2, 3, 4, 5],
        BASELINE_TOKENS,
        TWO_WORSE,
        BASELINE_REVISION,
    );
    let (code, out, err) = rebaseline(&world, &lowered);
    assert_eq!(code, 1, "the lowered rebaseline is refused: {out}{err}");
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 0, "the honest one is accepted: {out}{err}");
    let name = world
        .records()
        .into_iter()
        .find(|name| name.contains(".rebaseline-") && !name.ends_with(".scorecard.json"))
        .expect("the rebaseline record");
    assert!(
        world.record(&name).contains(&format!(
            "\"previous_sha256\": \"{}\"",
            sha256(&world.store.join(&refusal))
        )),
        "it chains onto the refusal"
    );
    let again = world.trials_card("again", 21, WITHIN_TOKENS, TWO_WORSE);
    let (code, out, err) = world.promote(&again, &["--rejudge", "--dry-run"]);
    assert_eq!(code, 3, "the refused candidate stays refused: {out}{err}");
}

/// Correction 2, item 2: a rebaseline record is checked on read: one whose `replaces_sha256`
/// names no scorecard of the record it replaces refuses the store, naming it.
#[test]
fn a_rebaseline_record_replacing_nothing_refuses_the_store() {
    let world = World::new("rebaseline-replaces", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&outcome_v1(&world, &world.baseline, "v1", &[]));
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 0, "{out}{err}");
    let name = world
        .records()
        .into_iter()
        .find(|name| name.contains(".rebaseline-") && !name.ends_with(".scorecard.json"))
        .expect("the rebaseline record");
    patch(&world.store.join(&name), "replaces_sha256", &"0".repeat(64));
    world.rehead(&name);
    let card = world.candidate_card("candidate", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&name) && err.contains(&"0".repeat(64)),
        "{err}"
    );
}

/// Correction 2, item 3: `--rebaseline` refuses a file not named `scorecard-<revision>.json`.
#[test]
fn a_rebaseline_file_not_named_by_its_revision_is_refused() {
    let world = World::new("rebaseline-name", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&outcome_v1(&world, &world.baseline, "v1", &[]));
    let odd = world.outer.join("saves-odd").join("whatever.json");
    write(&odd, &fs::read_to_string(&world.baseline).expect("read"));
    let (code, out, err) = rebaseline(&world, &odd);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(&format!("scorecard-{BASELINE_REVISION}.json")),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// Correction 2, item 4: a held-out result's `passed` is derived from its `result` on load, so a
/// saved scorecard with `passed` flipped over a failing result is refused, naming the run.
#[test]
fn a_held_out_passed_flag_contradicting_its_result_is_refused() {
    let world = World::new("passed-flipped", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let cases = world.outer.join("cases-passed-flipped");
    for (n, tokens) in BASELINE_TOKENS.iter().enumerate() {
        let case = format!("scenario-1-trial-{}", n + 11);
        run(&cases, &case, *tokens, APPROVAL, Some(&world.revision));
        write(
            &cases.join(CASES_DIR).join(&case).join(HELD_OUT),
            "FAIL: x\n",
        );
    }
    let card = World::save(
        &cases,
        &world.outer.join("saves-passed-flipped"),
        &world.revision,
    );
    let text = fs::read_to_string(&card).expect("read");
    let edited = text
        .replace(
            "\"outcome\": \"failed\"",
            "\"outcome\": \"ApprovalRequired\"",
        )
        .replace("\"passed\": false", "\"passed\": true");
    assert_ne!(edited, text);
    fs::write(&card, edited).expect("edited");
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("scenario-1-trial-11#1"), "{err}");
}

/// Five cases each of scenarios 1 and 2, trials `from` to `from + 4`: scenario 1's runs stop
/// `s1`, scenario 2's `s2`, each run opened by `binary`.
fn two_scenarios(cases: &Path, from: usize, s1: &str, s2: &str, binary: &str) {
    for (n, tokens) in BASELINE_TOKENS.iter().enumerate() {
        run(
            cases,
            &format!("scenario-1-trial-{}", n + from),
            *tokens,
            s1,
            Some(binary),
        );
        run(
            cases,
            &format!("scenario-2-trial-{}", n + from),
            *tokens,
            s2,
            Some(binary),
        );
    }
}

/// The scorecard at `card` with the scenario labels 1 and 2 swapped in its rows, nothing else,
/// written in a directory of its own named by `label`; the file.
fn swap_labels(world: &World, card: &Path, label: &str) -> PathBuf {
    let text = fs::read_to_string(card).expect("the scorecard");
    let swapped = text
        .replace("\"scenario\": \"1\"", "\"scenario\": \"X\"")
        .replace("\"scenario\": \"2\"", "\"scenario\": \"1\"")
        .replace("\"scenario\": \"X\"", "\"scenario\": \"2\"");
    assert_ne!(text, swapped);
    let path = world
        .outer
        .join(format!("saves-{label}"))
        .join(card.file_name().expect("a file name"));
    write(&path, &swapped);
    path
}

/// Correction 3, item 1: a scenario label is derived from the case id on load, so a rebaseline
/// whose rows carry swapped labels is refused, naming the run; the honest one is accepted and a
/// candidate that regressed on scenario 1 is worse against it.
#[test]
fn a_rebaseline_relabelling_scenarios_is_refused() {
    let world = World::new("relabel-rebaseline", BASELINE_TOKENS, FIVE_APPROVALS);
    let cases = world.outer.join("two-scenario-cases");
    two_scenarios(&cases, 1, APPROVAL, ERROR, BASELINE_REVISION);
    let v2 = World::save(&cases, &world.outer.join("saves-two"), BASELINE_REVISION);
    world.record_baseline_of(&outcome_v1(&world, &v2, "v1", &[]));
    let forged = swap_labels(&world, &v2, "forged");
    let (code, out, err) = rebaseline(&world, &forged);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("run scenario-1-trial-1#1 ") && err.contains("scenario"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
    let (code, out, err) = rebaseline(&world, &v2);
    assert_eq!(code, 0, "{out}{err}");
    let candidate_cases = world.outer.join("cases-candidate");
    two_scenarios(&candidate_cases, 11, ERROR, APPROVAL, &world.revision);
    let candidate = World::save(
        &candidate_cases,
        &world.outer.join("saves-candidate"),
        &world.revision,
    );
    let (code, out, err) = world.promote(&candidate, &["--dry-run"]);
    assert_eq!(code, 3, "{out}{err}");
}

/// Correction 3, item 1: the candidate's own scorecard with its scenario labels swapped is
/// refused on load, naming the run, where the honest one is worse (exit 3).
#[test]
fn a_candidate_relabelling_scenarios_is_refused() {
    let world = World::new("relabel-candidate", BASELINE_TOKENS, FIVE_APPROVALS);
    let cases = world.outer.join("two-scenario-cases");
    two_scenarios(&cases, 1, APPROVAL, ERROR, BASELINE_REVISION);
    let v2 = World::save(&cases, &world.outer.join("saves-two"), BASELINE_REVISION);
    world.record_baseline_of(&v2);
    let candidate_cases = world.outer.join("cases-candidate");
    two_scenarios(&candidate_cases, 11, ERROR, APPROVAL, &world.revision);
    let candidate = World::save(
        &candidate_cases,
        &world.outer.join("saves-candidate"),
        &world.revision,
    );
    let (code, out, err) = world.promote(&candidate, &["--dry-run"]);
    assert_eq!(code, 3, "{out}{err}");
    let forged = swap_labels(&world, &candidate, "forged");
    let (code, out, err) = world.promote(&forged, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("run scenario-1-trial-11#1 ") && err.contains("scenario"),
        "{err}"
    );
}

/// Correction 3, item 3: in a rebaseline from outcome definition 1 to 2 the held-out results
/// are new input; one that lowers a run's outcome below the stored one is refused, naming the
/// run, so a rebaseline only keeps or raises the bar.
#[test]
fn a_rebaseline_cannot_lower_an_outcome() {
    let world = World::new("rebaseline-lower", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&outcome_v1(&world, &world.baseline, "v1", &[]));
    let cases = world.outer.join("baseline-cases");
    for n in 1..=5 {
        write(
            &cases
                .join(CASES_DIR)
                .join(format!("scenario-1-trial-{n}"))
                .join(HELD_OUT),
            "FAIL: written by hand\n",
        );
    }
    let card = World::save(&cases, &world.outer.join("saves-failed"), BASELINE_REVISION);
    let (code, out, err) = rebaseline(&world, &card);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("run scenario-1-trial-1#1 ") && err.contains("ranks below"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// Correction 3, item 4: a stored scorecard with no outcome metric is refused by name.
#[test]
fn a_rebaseline_over_a_scorecard_without_an_outcome_metric_is_refused() {
    let world = World::new("rebaseline-no-outcome", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&outcome_v1(
        &world,
        &world.baseline,
        "v1",
        &[("\"outcome\": 1,\n", "")],
    ));
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("no outcome metric"), "{err}");
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// `scorecard` as a takt before the held-out check saved it: outcome definition 1, and no
/// `stop`, `held_out` or `opened_binary` in its rows, written in a directory of its own named
/// by `label`; the file.
fn as_saved_before(world: &World, scorecard: &Path, label: &str) -> PathBuf {
    let text = fs::read_to_string(scorecard).expect("the scorecard");
    let mut kept: Vec<String> = Vec::new();
    let mut in_held = false;
    for line in text.lines() {
        let key = line.trim_start();
        if in_held {
            in_held = !key.starts_with('}');
            continue;
        }
        if key.starts_with("\"stop\":") || key.starts_with("\"opened_binary\":") {
            continue;
        }
        if key.starts_with("\"held_out\":") {
            in_held = key.ends_with('{');
            continue;
        }
        kept.push(line.replace("\"outcome\": 3,", "\"outcome\": 1,"));
    }
    let mut joined = kept.join("\n") + "\n";
    while joined.contains(",\n    }") {
        joined = joined.replace(",\n    }", "\n    }");
    }
    assert!(
        !joined.contains("opened_binary") && !joined.contains("held_out"),
        "{joined}"
    );
    let path = world
        .outer
        .join(format!("saves-{label}"))
        .join(scorecard.file_name().expect("a file name"));
    write(&path, &joined);
    path
}

/// Correction 3, item 2, the shape the first rebaseline after the merge meets: a stored
/// scorecard saved before the held-out check (outcome definition 1, no `opened_binary` in its
/// rows) whose runs' `opened` lines name the stored record's revision. The re-read is accepted,
/// and a candidate under the current definitions is judged against it; a re-read naming another
/// revision is refused.
#[test]
fn a_rebaseline_of_rows_saved_without_opened_binary_is_accepted() {
    let world = World::new("rebaseline-real-shape", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline_of(&as_saved_before(&world, &world.baseline, "before"));
    let others = world.outer.join("cases-others");
    for (n, tokens) in BASELINE_TOKENS.iter().enumerate() {
        run(
            &others,
            &format!("scenario-1-trial-{}", n + 1),
            *tokens,
            APPROVAL,
            None,
        );
    }
    let other = World::forged(
        &others,
        &world.outer.join("saves-others"),
        BASELINE_REVISION,
        &[Some(world.revision.as_str()); 5],
    );
    let (code, out, err) = rebaseline(&world, &other);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("opened_binary"), "{err}");
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 0, "{out}{err}");
    let card = world.candidate_card("candidate", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(&format!("against baseline {BASELINE_REVISION}")),
        "{out}"
    );
}

/// Removes `tokens` and `usage` from every harness step line of `case`'s record under `cases`:
/// a run whose harness reported no tokens.
fn untoken(cases: &Path, case: &str) {
    let path = cases.join(CASES_DIR).join(case).join("events.jsonl");
    let text = fs::read_to_string(&path).expect("the record");
    let mut stripped = 0;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).expect("a line");
            if value["event"] == "step" && value["by"] == "harness" {
                let object = value.as_object_mut().expect("an object");
                object.remove("tokens");
                object.remove("usage");
                stripped += 1;
            }
            value.to_string()
        })
        .collect();
    assert!(stripped > 0, "{case} has a harness step");
    fs::write(&path, lines.join("\n") + "\n").expect("written");
}

/// Five scenario 1 cases, trials 11 to 15, at the approval with [`BASELINE_TOKENS`], made with
/// the binary of `revision`, case `scenario-1-trial-<unknown>`'s harness step recording no
/// tokens; saved under `revision` in a directory of its own named by `label`.
fn untokened_card(world: &World, label: &str, revision: &str, unknown: usize) -> PathBuf {
    let cases = world.outer.join(format!("cases-{label}"));
    for (n, tokens) in BASELINE_TOKENS.iter().enumerate() {
        run(
            &cases,
            &format!("scenario-1-trial-{}", n + 11),
            *tokens,
            APPROVAL,
            Some(revision),
        );
    }
    untoken(&cases, &format!("scenario-1-trial-{unknown}"));
    World::save(
        &cases,
        &world.outer.join(format!("saves-{label}")),
        revision,
    )
}

/// story:run-tokens-unknown-is-not-zero, acceptance 2: a candidate with a case whose harness
/// step recorded no tokens is refused, exit 1, before anything is judged, naming the case and
/// the step; nothing recorded or installed, a dry run alike. Its tokens would otherwise read as
/// a saving of the whole step.
#[test]
fn a_candidate_with_unknown_run_tokens_is_refused_before_judging() {
    let world = World::new("unknown-candidate", BASELINE_TOKENS, FIVE_APPROVALS);
    world.record_baseline();
    let card = untokened_card(&world, "unknown", &world.revision, 13);
    for extra in [&["--dry-run"][..], &[]] {
        let (code, out, err) = world.promote(&card, extra);
        assert_eq!(code, 1, "{extra:?}: {out}{err}");
        assert!(
            err.contains("scenario-1-trial-13") && err.contains("implementation.edit"),
            "{extra:?}: the case and the step named: {err}"
        );
        assert!(err.contains("unknown"), "{err}");
        assert!(!out.contains("verdict"), "nothing judged: {out}");
        assert!(!out.contains("median tokens"), "nothing judged: {out}");
    }
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// Acceptance 2, the stored side: a baseline whose scorecard holds a run of unknown tokens
/// judges no candidate; refused, exit 1, naming the baseline's case and step, nothing recorded.
/// And `--baseline-only` does not record such a baseline.
#[test]
fn a_baseline_with_unknown_run_tokens_judges_nothing_and_is_not_recorded() {
    let world = World::new("unknown-baseline", BASELINE_TOKENS, FIVE_APPROVALS);
    let stored = untokened_card(&world, "stored", BASELINE_REVISION, 12);
    world.record_baseline_of(&stored);
    let card = world.candidate_card("candidate", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &[]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("scenario-1-trial-12") && err.contains("implementation.edit"),
        "{err}"
    );
    assert!(!out.contains("verdict"), "nothing judged: {out}");
    world.assert_untouched();
    assert_eq!(world.records(), baseline_files(), "nothing recorded");

    let world = World::new("unknown-baseline-only", BASELINE_TOKENS, FIVE_APPROVALS);
    world.install_stand_in(&world.revision);
    let card = untokened_card(&world, "base", &world.revision, 14);
    let (code, out, err) = world.promote(&card, &["--baseline-only"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("scenario-1-trial-14") && err.contains("implementation.edit"),
        "{err}"
    );
    assert_eq!(world.records(), Vec::<String>::new(), "nothing recorded");
}

/// Acceptance 3: a baseline saved under tokens definition 1, which read a harness step that
/// recorded no tokens as 0, is compared only after `--rebaseline`; the rebaseline re-reads its
/// runs, and one whose run tokens are now unknown is refused, exit 1, naming the case and the
/// step, nothing recorded: the stored figure counted the step 0 and is not carried over.
#[test]
fn a_tokens_v1_baseline_is_compared_only_after_a_rebaseline_which_refuses_an_unknown() {
    let world = World::new("tokens-v1", BASELINE_TOKENS, FIVE_APPROVALS);
    let v1 = tokens_v1(&world, &world.baseline, "v1");
    world.record_baseline_of(&v1);
    let card = world.candidate_card("candidate", BASELINE_TOKENS, FIVE_APPROVALS);
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("metric tokens is at definition version 1"),
        "{err}"
    );
    let (code, out, err) = rebaseline(&world, &world.baseline);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = world.promote(&card, &["--dry-run"]);
    assert_eq!(code, 0, "judged after the rebaseline: {out}{err}");

    let world = World::new("tokens-v1-unknown", BASELINE_TOKENS, FIVE_APPROVALS);
    let unknown = untokened_card(&world, "stored", BASELINE_REVISION, 12);
    let v1 = tokens_v1(&world, &unknown, "v1");
    world.record_baseline_of(&v1);
    let (code, out, err) = rebaseline(&world, &unknown);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("scenario-1-trial-12") && err.contains("implementation.edit"),
        "{err}"
    );
    assert_eq!(world.records(), baseline_files(), "nothing recorded");
}

/// `scorecard` as a takt knowing tokens definition 1 saved it: the definition at 1, every run's
/// unknown tokens read as the sum of the steps that recorded some (none here: 0), written in a
/// directory of its own named by `label`; the file.
fn tokens_v1(world: &World, scorecard: &Path, label: &str) -> PathBuf {
    let mut file: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(scorecard).expect("the scorecard")).expect("json");
    file["metrics"]["tokens"] = 1.into();
    for row in file["rows"].as_array_mut().expect("rows") {
        if row["tokens"].is_null() {
            row["tokens"] = 0.into();
        }
    }
    let path = world
        .outer
        .join(format!("saves-{label}"))
        .join(scorecard.file_name().expect("a file name"));
    write(
        &path,
        &(serde_json::to_string_pretty(&file).expect("json") + "\n"),
    );
    path
}
