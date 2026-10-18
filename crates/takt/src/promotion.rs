//! Promotion: the takt binary installed for runs is built from a revision whose saved scorecard
//! shows no regression against the promoted baseline (design:recursive-self-improvement § The
//! judge, § Scorecard). A promotion is a record, one JSON file per revision under the store
//! `$XDG_STATE_HOME/takt/promotions/` (`~/.local/state/takt/promotions/` without
//! `XDG_STATE_HOME`), outside every repository a candidate changes, beside a copy of the
//! scorecard it read and its digest; the latest record by `promoted_at` that is no refusal
//! names the baseline the next candidate is compared with. The first record is a `baseline`
//! one, written by `takt promote --baseline-only`, which installs nothing.
//!
//! `takt promote` reads the candidate's saved scorecard (a file outside the candidate's
//! repository with one link, whose `revision` field equals its name and the checkout's `HEAD`,
//! a full commit id, whose rule is takt's own and whose metric definitions takt knows),
//! compares it with the baseline's copy in the store under takt's [`Rule`], and refuses a
//! candidate that is worse, writing a `refused` record so the same revision, or the same tree
//! under another commit, is not judged again; a candidate that holds is built from a fresh
//! clone of the exact commit in an empty build directory outside every repository, with the
//! build's environment stripped of what could change what is compiled, its binary's `--version`
//! checked to name the revision, installed with `cargo install --locked --path crates/takt-cli`
//! into a fresh root and moved into place in one rename, checked again and recorded with the
//! installed file's sha256. One promotion a UTC day. The store is read fail-closed: a file that
//! is no record, a record whose name, kind or instant disagrees with its content, or a scorecard
//! copy whose digest differs from its record, refuses every promotion until someone looks.
//! `--dry-run` prints the verdict and the record it would write and writes nothing.

use std::collections::BTreeSet;
use std::io::Write;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::scorecard::{METRICS, Rule, Scorecard};

/// The promotion record's format.
pub const FORMAT: &str = "takt-promotion/1";

/// The store's path below the state directory.
pub const STORE: &str = "takt/promotions";

/// The build's environment variables that change what is compiled or where, removed before
/// cargo runs so the promoted binary is the revision's and nothing else; a prefix ends in `_`
/// and removes every variable that starts with it.
pub const UNTRUSTED_BUILD_ENV: [&str; 8] = [
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_BUILD_",
    "CARGO_TARGET_",
];

/// What a record is: the first baseline (`takt promote --baseline-only`, nothing installed), a
/// promotion that installed a binary, or a candidate refused on its scorecard, kept so the same
/// revision is not judged again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Baseline,
    Promotion,
    Refused,
}

/// One promotion record, `<store>/<revision>.json`, or `<store>/<revision>.refused.json` for a
/// refusal; a baseline or a promotion has the scorecard it read copied beside it as
/// `<store>/<revision>.scorecard.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promotion {
    /// [`FORMAT`].
    pub format: String,
    /// The revision the binary was built from, a full commit id; the candidate's for a refusal.
    pub revision: String,
    /// The tree of that commit, so the same tree under another commit is known.
    pub tree: String,
    /// When the record was written, `YYYY-MM-DDTHH:MM:SSZ`; the one-a-UTC-day rule reads it.
    pub promoted_at: String,
    /// What the record is.
    pub kind: Kind,
    /// The saved scorecard read, by its id: the revision that names it, equal to `revision`.
    pub scorecard: String,
    /// The saved scorecard's file as given, absolute.
    pub scorecard_file: PathBuf,
    /// The sha256 of the scorecard read, lower-case hex; the copy in the store must match it.
    pub scorecard_sha256: String,
    /// The baseline revision the scorecard was compared with; none on a baseline record.
    pub against: Option<String>,
    /// The rule the comparison applied, takt's own.
    pub rule: Rule,
    /// The scenarios the comparison marked worse; 0 on a baseline or a promotion.
    pub worse: u32,
    /// The installed binary; none unless the record is a promotion.
    pub binary: Option<PathBuf>,
    /// The installed binary's sha256, lower-case hex; none unless the record is a promotion.
    pub binary_sha256: Option<String>,
}

/// The build or the install of a candidate that held failed, or built something that is not the
/// candidate: the CLI tells this apart from a refusal.
#[derive(Debug)]
pub struct BuildFailed(pub String);

impl std::fmt::Display for BuildFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BuildFailed {}

/// What `takt promote` asks for.
pub struct Request<'a> {
    /// The takt checkout at the candidate revision, clean.
    pub repo: &'a Path,
    /// The candidate's saved scorecard.
    pub scorecard: &'a Path,
    /// The store of promotion records.
    pub store: &'a Path,
    /// Where the binary is installed: `cargo install --root`.
    pub install_root: &'a Path,
    /// Where the candidate is cloned and built, a fresh directory of its own under it.
    pub build_dir: &'a Path,
    /// Record the baseline only; compare with nothing, install nothing.
    pub baseline_only: bool,
    /// Print the verdict and the record that would be written; write and install nothing.
    pub dry_run: bool,
    /// The instant the record carries.
    pub now: String,
}

/// How a request ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The binary is installed and the record written.
    Promoted(Promotion),
    /// The baseline record is written.
    BaselineRecorded(Promotion),
    /// The candidate is worse; the refusal record is written, nothing installed.
    Regression(Promotion),
    /// A dry run: the record that would be written, a promotion or a baseline.
    WouldWrite(Promotion),
    /// A dry run: the refusal that would be written.
    WouldRefuse(Promotion),
}

/// The store: `$XDG_STATE_HOME/takt/promotions`, or `~/.local/state/takt/promotions`.
pub fn store() -> Result<PathBuf> {
    Ok(state_dir()?.join(STORE))
}

/// Where candidates are built: `$XDG_CACHE_HOME/takt/promote`, or `~/.cache/takt/promote`.
pub fn build_dir() -> Result<PathBuf> {
    let cache = match std::env::var_os("XDG_CACHE_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) if Path::new(&dir).is_absolute() => PathBuf::from(dir),
        _ => home()?.join(".cache"),
    };
    Ok(cache.join("takt/promote"))
}

fn home() -> Result<PathBuf> {
    match std::env::var_os("HOME").filter(|home| !home.is_empty()) {
        Some(home) => Ok(PathBuf::from(home)),
        None => bail!("HOME is not set"),
    }
}

fn state_dir() -> Result<PathBuf> {
    match std::env::var_os("XDG_STATE_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) if Path::new(&dir).is_absolute() => Ok(PathBuf::from(dir)),
        _ => Ok(home()?.join(".local/state")),
    }
}

/// Where `cargo install` puts the binary when `--root` is not given: `CARGO_INSTALL_ROOT`,
/// else `CARGO_HOME`, else `~/.cargo`; promote passes it as `--root` so both agree.
pub fn install_root() -> Result<PathBuf> {
    for name in ["CARGO_INSTALL_ROOT", "CARGO_HOME"] {
        if let Some(dir) = std::env::var_os(name).filter(|dir| !dir.is_empty()) {
            return Ok(PathBuf::from(dir));
        }
    }
    Ok(home()?.join(".cargo"))
}

/// The record's file in `store`.
fn record_path(store: &Path, revision: &str, kind: Kind) -> PathBuf {
    match kind {
        Kind::Refused => store.join(format!("{revision}.refused.json")),
        Kind::Baseline | Kind::Promotion => store.join(format!("{revision}.json")),
    }
}

/// The store's copy of the scorecard a baseline or a promotion read.
fn copy_path(store: &Path, revision: &str) -> PathBuf {
    store.join(format!("{revision}.scorecard.json"))
}

/// Whether `candidate` is a full commit id, 40 or 64 lower-case hex digits.
fn is_full_revision(candidate: &str) -> bool {
    matches!(candidate.len(), 40 | 64)
        && candidate
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Every record in `store`, oldest first by `promoted_at`, then by revision; none when the
/// store does not exist. Read fail-closed: a file that is no record or no scorecard copy, a
/// record whose name does not carry its revision and kind, one whose instant does not parse or
/// lies after `now`, or a scorecard copy without its record or with a digest other than the
/// record's, is an error naming it.
pub fn records(store: &Path, now: &str) -> Result<Vec<Promotion>> {
    let mut records = Vec::new();
    let mut copies: BTreeSet<String> = BTreeSet::new();
    let entries = match std::fs::read_dir(store) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(records),
        Err(error) => return Err(error).with_context(|| format!("read {}", store.display())),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("read {}", store.display()))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            bail!(
                "{}: not a regular file; the store holds records and scorecard copies only, so it is not read past this",
                path.display()
            );
        }
        if let Some(revision) = name.strip_suffix(".scorecard.json") {
            if !is_full_revision(revision) {
                bail!(
                    "{}: a scorecard copy is named by a full commit id; the store is not read past this",
                    path.display()
                );
            }
            copies.insert(revision.to_owned());
            continue;
        }
        let Some(stem) = name.strip_suffix(".json") else {
            bail!(
                "{}: not a record (`<revision>.json`, `<revision>.refused.json`) nor a scorecard copy (`<revision>.scorecard.json`); the store is not read past this",
                path.display()
            );
        };
        let (named_revision, named_kind) = match stem.strip_suffix(".refused") {
            Some(revision) => (revision, Kind::Refused),
            None => (stem, Kind::Promotion),
        };
        let record = load(&path)?;
        if record.revision != named_revision {
            bail!(
                "{}: the file is named by {named_revision} but the record says {}; the store is not read past this",
                path.display(),
                record.revision
            );
        }
        let kind_agrees = match named_kind {
            Kind::Refused => record.kind == Kind::Refused,
            _ => record.kind != Kind::Refused,
        };
        if !kind_agrees {
            bail!(
                "{}: the file name says {} and the record says {:?}; the store is not read past this",
                path.display(),
                if named_kind == Kind::Refused {
                    "refused"
                } else {
                    "a baseline or a promotion"
                },
                record.kind
            );
        }
        if !is_instant(&record.promoted_at) {
            bail!(
                "{}: promoted_at {:?} is no `YYYY-MM-DDTHH:MM:SSZ` instant; the store is not read past this",
                path.display(),
                record.promoted_at
            );
        }
        if record.promoted_at.as_str() > now {
            bail!(
                "{}: promoted_at {} lies after now ({now}); the store is not read past this",
                path.display(),
                record.promoted_at
            );
        }
        records.push(record);
    }
    for record in &records {
        if record.kind == Kind::Refused {
            continue;
        }
        let copy = copy_path(store, &record.revision);
        if !copies.remove(&record.revision) {
            bail!(
                "{}: the record's scorecard copy {} is missing; the store is not read past this",
                record_path(store, &record.revision, record.kind).display(),
                copy.display()
            );
        }
        let digest = sha256_of(&copy)?;
        if digest != record.scorecard_sha256 {
            bail!(
                "{}: its digest {digest} is not the one its record holds ({}); the store is not read past this",
                copy.display(),
                record.scorecard_sha256
            );
        }
    }
    if let Some(orphan) = copies.iter().next() {
        bail!(
            "{}: a scorecard copy without its record; the store is not read past this",
            copy_path(store, orphan).display()
        );
    }
    records.sort_by(|a, b| {
        a.promoted_at
            .cmp(&b.promoted_at)
            .then_with(|| a.revision.cmp(&b.revision))
    });
    Ok(records)
}

/// Whether `text` is `YYYY-MM-DDTHH:MM:SSZ`.
fn is_instant(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'Z'),
        ]
        .iter()
        .all(|(at, byte)| bytes[*at] == *byte)
        && bytes
            .iter()
            .enumerate()
            .all(|(n, b)| matches!(n, 4 | 7 | 10 | 13 | 16 | 19) || b.is_ascii_digit())
}

/// A record file; one that does not read, or of another format, is an error naming it.
pub fn load(path: &Path) -> Result<Promotion> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let record: Promotion = serde_json::from_str(&text)
        .with_context(|| format!("{} is not a promotion record", path.display()))?;
    if record.format != FORMAT {
        bail!(
            "{} is a promotion record of format {}, not {FORMAT}",
            path.display(),
            record.format
        );
    }
    Ok(record)
}

/// The baseline: the latest record by `promoted_at` that installed a binary or recorded the
/// first baseline; a refusal is none.
pub fn baseline(store: &Path, now: &str) -> Result<Option<Promotion>> {
    Ok(records(store, now)?
        .into_iter()
        .rfind(|record| record.kind != Kind::Refused))
}

/// `path` resolved through its nearest existing ancestor, so a path that does not exist yet,
/// one through a symbolic link and one with `..` compare by where they would land.
fn resolved(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|dir| dir.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut rest = Vec::new();
    let mut cursor = absolute.as_path();
    loop {
        if let Ok(real) = cursor.canonicalize() {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (cursor.file_name(), cursor.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name.to_owned());
                cursor = parent;
            }
            _ => return absolute,
        }
    }
}

/// Whether `path` lies inside `repo`, both resolved.
fn inside(repo: &Path, path: &Path) -> bool {
    resolved(path).starts_with(resolved(repo))
}

/// `git <args>` in `repo`; its trimmed standard output, or the error.
fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(repo)
        .output()
        .with_context(|| format!("run git {} in {}", args.join(" "), repo.display()))?;
    if !output.status.success() {
        bail!(
            "git {} in {}: {}",
            args.join(" "),
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The revision `takt --version` prints: the text in the parentheses of `takt <version>
/// (<revision>)`.
fn printed_revision(binary: &Path) -> Result<String> {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .with_context(|| format!("run {} --version", binary.display()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().unwrap_or("");
    let revision = line
        .rsplit_once(" (")
        .and_then(|(_, rest)| rest.strip_suffix(')'))
        .map(str::to_owned);
    revision.ok_or_else(|| {
        anyhow::anyhow!(
            "{} --version printed {line:?}, not `takt <version> (<revision>)`",
            binary.display()
        )
    })
}

/// The sha256 of the file at `path`, lower-case hex.
fn sha256_of(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

/// Writes `text` to `path`; a file there already is not overwritten.
fn write_new(path: &Path, text: &[u8]) -> Result<()> {
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!("{} exists: a store file is not overwritten", path.display());
        }
        Err(error) => return Err(error).with_context(|| format!("write {}", path.display())),
    };
    file.write_all(text)
        .with_context(|| format!("write {}", path.display()))
}

/// Writes `record` to the store, with the scorecard it read copied beside it unless it is a
/// refusal; the record's path.
fn write(store: &Path, record: &Promotion, scorecard_text: &[u8]) -> Result<PathBuf> {
    std::fs::create_dir_all(store).with_context(|| format!("create {}", store.display()))?;
    if record.kind != Kind::Refused {
        write_new(&copy_path(store, &record.revision), scorecard_text)?;
    }
    let path = record_path(store, &record.revision, record.kind);
    let mut text = serde_json::to_string_pretty(record).context("write the record")?;
    text.push('\n');
    write_new(&path, text.as_bytes())?;
    Ok(path)
}

/// Runs `cargo <args>` in `dir` with the build's untrusted environment removed, the command
/// shown on `out`; a failure is a [`BuildFailed`].
fn cargo(dir: &Path, args: &[&str], out: &mut dyn Write) -> Result<()> {
    writeln!(out, "cargo {}", args.join(" "))?;
    let mut command = Command::new("cargo");
    command.args(args).current_dir(dir);
    for (name, _) in std::env::vars_os() {
        let name_text = name.to_string_lossy();
        let untrusted = UNTRUSTED_BUILD_ENV.iter().any(|pattern| {
            if let Some(prefix) = pattern.strip_suffix('_') {
                name_text.starts_with(prefix) && name_text.len() > prefix.len()
            } else {
                name_text == *pattern
            }
        });
        if untrusted {
            command.env_remove(&name);
        }
    }
    let output = command
        .output()
        .with_context(|| format!("run cargo {} in {}", args.join(" "), dir.display()))?;
    if !output.status.success() {
        return Err(BuildFailed(format!(
            "cargo {} in {} exited {}: {}",
            args.join(" "),
            dir.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" | ")
        ))
        .into());
    }
    Ok(())
}

/// Checks the candidate scorecard and the checkout agree: the scorecard, the store, the install
/// root and the build directory lie outside the repository, the scorecard has one link, its
/// `revision` is a full commit id equal to its file name and to `HEAD`, its rule is takt's and
/// its metric definitions are the ones takt knows, and the checkout is clean: no tracked file
/// differs from `HEAD`, no untracked file, no entry marked assume-unchanged or skip-worktree.
/// The scorecard, its text, the revision and its tree.
fn candidate(request: &Request<'_>) -> Result<(Scorecard, Vec<u8>, String, String)> {
    let Request {
        repo, scorecard, ..
    } = request;
    for (what, path) in [
        ("--scorecard", *scorecard),
        ("the promotion store", request.store),
        ("--install-root", request.install_root),
        ("the build directory", request.build_dir),
    ] {
        if inside(repo, path) {
            bail!(
                "{what} {}: inside the repository {}; nothing a promotion reads or writes lies in the repository the candidate changes",
                path.display(),
                repo.display()
            );
        }
    }
    let metadata = std::fs::metadata(scorecard)
        .with_context(|| format!("--scorecard {}", scorecard.display()))?;
    if !metadata.is_file() {
        bail!("--scorecard {}: not a regular file", scorecard.display());
    }
    if metadata.nlink() > 1 {
        bail!(
            "--scorecard {}: the file has {} links; a scorecard that may also lie inside the repository under another name is not read",
            scorecard.display(),
            metadata.nlink()
        );
    }
    let text = std::fs::read(scorecard).with_context(|| format!("read {}", scorecard.display()))?;
    let card = Scorecard::load(scorecard)?;
    if card.rule != Rule::default() {
        bail!(
            "{}: its rule is trials {} margin {}; this takt judges under trials {} margin {}, and a scorecard saved under another rule is not compared",
            scorecard.display(),
            card.rule.trials,
            card.rule.margin,
            Rule::default().trials,
            Rule::default().margin
        );
    }
    known_metrics(&card, scorecard)?;
    let Some(revision) = card.revision.clone() else {
        bail!(
            "{}: the scorecard names no revision; save it with `takt scorecard --save --revision`",
            scorecard.display()
        );
    };
    if !is_full_revision(&revision) {
        bail!(
            "{}: its revision {revision} is no full commit id (40 or 64 hex digits); a promotion compares the full sha",
            scorecard.display()
        );
    }
    let named = scorecard
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("scorecard-"))
        .and_then(|name| name.strip_suffix(".json"));
    if named != Some(revision.as_str()) {
        bail!(
            "{}: the file is not named by its revision {revision} (`scorecard-{revision}.json`)",
            scorecard.display()
        );
    }
    let head = git(repo, &["rev-parse", "--verify", "-q", "HEAD"])?;
    if head != revision {
        bail!(
            "{} is at {head}, not at the scorecard's revision {revision}; check that revision out",
            repo.display()
        );
    }
    let tree = git(repo, &["rev-parse", "--verify", "-q", "HEAD^{tree}"])?;
    let dirty = git(repo, &["status", "--porcelain", "--untracked-files=normal"])?;
    if !dirty.is_empty() {
        bail!(
            "{} has uncommitted or untracked files; a promotion is made from a clean checkout:\n{dirty}",
            repo.display()
        );
    }
    let marked: Vec<&str> = Vec::new();
    let listing = git(repo, &["ls-files", "-v"])?;
    let marked: Vec<&str> = listing
        .lines()
        .filter(|line| {
            line.as_bytes()
                .first()
                .is_some_and(|tag| tag.is_ascii_lowercase() || *tag == b'S')
        })
        .chain(marked)
        .collect();
    if !marked.is_empty() {
        bail!(
            "{} has {} index entr{} marked assume-unchanged or skip-worktree, which `git status` does not show; a promotion is made from a checkout with none:\n{}",
            repo.display(),
            marked.len(),
            if marked.len() == 1 { "y" } else { "ies" },
            marked.join("\n")
        );
    }
    Ok((card, text, revision, tree))
}

/// Every metric definition of `card` is one this takt knows, at the version it knows; another
/// is refused, never skipped.
fn known_metrics(card: &Scorecard, shown: &Path) -> Result<()> {
    for (metric, version) in METRICS {
        match card.metrics.get(metric) {
            Some(saved) if *saved == version => {}
            Some(saved) => bail!(
                "{}: metric {metric} is at definition version {saved}; this takt knows version {version}, and a scorecard under another definition is not compared",
                shown.display()
            ),
            None => bail!(
                "{}: metric {metric} has no definition version; a scorecard without one is not compared",
                shown.display()
            ),
        }
    }
    for metric in card.metrics.keys() {
        if !METRICS.iter().any(|(known, _)| known == metric) {
            bail!(
                "{}: metric {metric} is one this takt does not know; a scorecard with it is not compared",
                shown.display()
            );
        }
    }
    Ok(())
}

/// Builds the candidate from a fresh clone of `repo` at `revision` in an empty directory under
/// `build_dir`, checks the built binary names `revision`, installs it into a fresh root beside
/// `root`, checks the installed one, and moves it into `root/bin/takt` in one rename; a failure
/// before the rename leaves what was installed before. The installed binary's path and sha256.
fn install(
    repo: &Path,
    revision: &str,
    root: &Path,
    build_dir: &Path,
    out: &mut dyn Write,
) -> Result<(PathBuf, String)> {
    let work = build_dir.join(format!("{revision}-{}", std::process::id()));
    if work.exists() {
        std::fs::remove_dir_all(&work).with_context(|| format!("clear {}", work.display()))?;
    }
    std::fs::create_dir_all(&work).with_context(|| format!("create {}", work.display()))?;
    let result = install_in(repo, revision, root, &work, out);
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn install_in(
    repo: &Path,
    revision: &str,
    root: &Path,
    work: &Path,
    out: &mut dyn Write,
) -> Result<(PathBuf, String)> {
    let clone = work.join("source");
    let repo_text = repo
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{} is not UTF-8", repo.display()))?;
    let clone_text = clone
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{} is not UTF-8", clone.display()))?;
    writeln!(out, "cloning {revision} into {}", clone.display())?;
    let output = Command::new("git")
        .args([
            "clone",
            "--quiet",
            "--no-local",
            "--no-hardlinks",
            "--no-checkout",
            "--",
            repo_text,
            clone_text,
        ])
        .output()
        .context("run git clone")?;
    if !output.status.success() {
        return Err(BuildFailed(format!(
            "clone {} for the build: {}",
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .into());
    }
    git(&clone, &["checkout", "--quiet", "--detach", revision])
        .map_err(|error| BuildFailed(format!("check out {revision}: {error:#}")))?;
    let target = work.join("target");
    let target_text = target
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{} is not UTF-8", target.display()))?
        .to_owned();
    cargo(
        &clone,
        &[
            "build",
            "--locked",
            "--release",
            "-p",
            "b10x-takt-cli",
            "--bin",
            "takt",
            "--target-dir",
            &target_text,
        ],
        out,
    )?;
    let built = target.join("release/takt");
    let printed = printed_revision(&built).map_err(|error| BuildFailed(format!("{error:#}")))?;
    if printed != revision {
        return Err(BuildFailed(format!(
            "the built binary names {printed}, not {revision}: a promotion installs a clean build of the exact revision"
        ))
        .into());
    }
    let fresh_root = root.join(format!(".takt-promote-{}", std::process::id()));
    if fresh_root.exists() {
        std::fs::remove_dir_all(&fresh_root)
            .with_context(|| format!("clear {}", fresh_root.display()))?;
    }
    std::fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    let installed = install_into(&clone, revision, &target_text, root, &fresh_root, out);
    let _ = std::fs::remove_dir_all(&fresh_root);
    installed
}

fn install_into(
    clone: &Path,
    revision: &str,
    target_text: &str,
    root: &Path,
    fresh_root: &Path,
    out: &mut dyn Write,
) -> Result<(PathBuf, String)> {
    let fresh_text = fresh_root
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{} is not UTF-8", fresh_root.display()))?;
    let path_text = clone
        .join("crates/takt-cli")
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{} is not UTF-8", clone.display()))?
        .to_owned();
    cargo(
        clone,
        &[
            "install",
            "--locked",
            "--force",
            "--path",
            &path_text,
            "--root",
            fresh_text,
            "--target-dir",
            target_text,
        ],
        out,
    )?;
    let staged = fresh_root.join("bin/takt");
    let printed = printed_revision(&staged).map_err(|error| BuildFailed(format!("{error:#}")))?;
    if printed != revision {
        return Err(BuildFailed(format!(
            "the installed binary {} names {printed}, not {revision}",
            staged.display()
        ))
        .into());
    }
    let digest = sha256_of(&staged)?;
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).with_context(|| format!("create {}", bin.display()))?;
    let installed = bin.join("takt");
    std::fs::rename(&staged, &installed)
        .with_context(|| format!("move {} into {}", staged.display(), installed.display()))?;
    let printed = printed_revision(&installed)?;
    if printed != revision {
        bail!(
            "{} names {printed} after the move, not {revision}",
            installed.display()
        );
    }
    Ok((installed, digest))
}

/// `takt promote`: see the module's documentation. Prints the comparison and the verdict on
/// `out`.
pub fn promote(request: &Request<'_>, out: &mut dyn Write) -> Result<Outcome> {
    let (card, text, revision, tree) = candidate(request)?;
    let store = request.store;
    let now = request.now.as_str();
    let mut record = Promotion {
        format: FORMAT.to_owned(),
        revision: revision.clone(),
        tree: tree.clone(),
        promoted_at: request.now.clone(),
        kind: Kind::Baseline,
        scorecard: revision.clone(),
        scorecard_file: resolved(request.scorecard),
        scorecard_sha256: format!("{:x}", Sha256::digest(&text)),
        against: None,
        rule: card.rule,
        worse: 0,
        binary: None,
        binary_sha256: None,
    };
    let existing = records(store, now)?;
    for earlier in &existing {
        let same = if earlier.revision == revision {
            format!("{revision} is recorded already")
        } else if earlier.tree == tree {
            format!(
                "the tree of {revision} is the tree of {}, recorded already",
                earlier.revision
            )
        } else {
            continue;
        };
        let kind = match earlier.kind {
            Kind::Refused => "refused; a refused candidate is not judged again until it changes",
            Kind::Baseline => "as a baseline",
            Kind::Promotion => "as a promotion",
        };
        bail!(
            "{same}: {kind}, at {} ({})",
            earlier.promoted_at,
            record_path(store, &earlier.revision, earlier.kind).display()
        );
    }
    if request.baseline_only {
        if let Some(base) = existing.iter().rfind(|record| record.kind != Kind::Refused) {
            bail!(
                "the store {} holds a baseline already: {} at {} ({})",
                store.display(),
                base.revision,
                base.promoted_at,
                record_path(store, &base.revision, base.kind).display()
            );
        }
        enough_trials(&card, &card, request.scorecard)?;
        writeln!(
            out,
            "baseline {revision}: the scorecard of the installed revision, nothing compared, nothing installed"
        )?;
        if request.dry_run {
            writeln!(
                out,
                "dry run: would write {} and {}",
                record_path(store, &revision, Kind::Baseline).display(),
                copy_path(store, &revision).display()
            )?;
            writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
            return Ok(Outcome::WouldWrite(record));
        }
        let path = write(store, &record, &text)?;
        writeln!(out, "recorded: {}", path.display())?;
        return Ok(Outcome::BaselineRecorded(record));
    }
    let Some(base) = existing
        .iter()
        .rfind(|record| record.kind != Kind::Refused)
        .cloned()
    else {
        bail!(
            "the store {} holds no baseline; record the installed revision's scorecard first with `takt promote --baseline-only`",
            store.display()
        );
    };
    let today = now.get(..10).unwrap_or("");
    if let Some(earlier) = existing.iter().find(|record| {
        record.kind == Kind::Promotion && record.promoted_at.get(..10) == Some(today)
    }) {
        bail!(
            "a promotion was made this UTC day already: {} at {} ({}); one promotion a day",
            earlier.revision,
            earlier.promoted_at,
            record_path(store, &earlier.revision, earlier.kind).display()
        );
    }
    let base_copy = copy_path(store, &base.revision);
    let base_card = Scorecard::load(&base_copy)?;
    if base_card.revision.as_deref() != Some(base.revision.as_str()) {
        bail!(
            "the baseline's scorecard copy {} names {}, not the baseline revision {}",
            base_copy.display(),
            base_card.revision.as_deref().unwrap_or("no revision"),
            base.revision
        );
    }
    if base_card.rule != Rule::default() {
        bail!(
            "the baseline's scorecard copy {} was saved under trials {} margin {}; this takt judges under trials {} margin {}",
            base_copy.display(),
            base_card.rule.trials,
            base_card.rule.margin,
            Rule::default().trials,
            Rule::default().margin
        );
    }
    known_metrics(&base_card, &base_copy)?;
    enough_trials(&card, &base_card, request.scorecard)?;
    writeln!(
        out,
        "candidate {revision} against baseline {}",
        base.revision
    )?;
    let worse = card.against(&base_card, out)?;
    record.against = Some(base.revision.clone());
    record.worse = u32::try_from(worse).unwrap_or(u32::MAX);
    if worse > 0 {
        record.kind = Kind::Refused;
        writeln!(
            out,
            "verdict: refused, {worse} scenario{} worse than the baseline; nothing installed",
            if worse == 1 { "" } else { "s" }
        )?;
        if request.dry_run {
            writeln!(
                out,
                "dry run: would write {}",
                record_path(store, &revision, Kind::Refused).display()
            )?;
            writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
            return Ok(Outcome::WouldRefuse(record));
        }
        let path = write(store, &record, &text)?;
        writeln!(out, "recorded: {}", path.display())?;
        return Ok(Outcome::Regression(record));
    }
    record.kind = Kind::Promotion;
    writeln!(out, "verdict: no scenario worse than the baseline")?;
    if request.dry_run {
        writeln!(
            out,
            "dry run: would clone {} at {revision} under {}, build it there, install under {} and write {} and {}",
            request.repo.display(),
            request.build_dir.display(),
            request.install_root.display(),
            record_path(store, &revision, Kind::Promotion).display(),
            copy_path(store, &revision).display()
        )?;
        writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
        return Ok(Outcome::WouldWrite(record));
    }
    let (binary, digest) = install(
        request.repo,
        &revision,
        request.install_root,
        request.build_dir,
        out,
    )?;
    record.binary = Some(binary.clone());
    record.binary_sha256 = Some(digest.clone());
    let path = write(store, &record, &text)?;
    writeln!(
        out,
        "promoted: {} at {revision}, sha256 {digest}; recorded: {}",
        binary.display(),
        path.display()
    )?;
    Ok(Outcome::Promoted(record))
}

/// Every scenario the baseline has holds at least the rule's trials on the candidate's side,
/// and at least the rule's trials on the baseline's own side; fewer is a refusal, not a verdict.
fn enough_trials(card: &Scorecard, base_card: &Scorecard, shown: &Path) -> Result<()> {
    let trials = usize::try_from(card.rule.trials).unwrap_or(usize::MAX);
    let count = |card: &Scorecard, scenario: &str| {
        card.cases()
            .iter()
            .filter(|case| case.scenario.as_deref() == Some(scenario))
            .count()
    };
    let scenarios: BTreeSet<String> = base_card
        .cases()
        .iter()
        .filter_map(|case| case.scenario.clone())
        .collect();
    for scenario in scenarios {
        let n = count(base_card, &scenario);
        if n < trials {
            bail!(
                "scenario {scenario}: the baseline has {n} case{}, the rule needs {trials}; a baseline with fewer judges nothing",
                if n == 1 { "" } else { "s" }
            );
        }
        let n = count(card, &scenario);
        if n < trials {
            bail!(
                "scenario {scenario}: the candidate {} has {n} case{}, the rule needs {trials}; run it {} more time{} before judging",
                shown.display(),
                if n == 1 { "" } else { "s" },
                trials - n,
                if trials - n == 1 { "" } else { "s" }
            );
        }
    }
    Ok(())
}
