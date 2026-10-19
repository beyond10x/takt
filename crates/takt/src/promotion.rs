//! Promotion: the takt binary installed for runs is built from a revision whose saved scorecard
//! shows no regression against the promoted baseline (design:recursive-self-improvement § The
//! judge, § Scorecard). A promotion is a record, one JSON file per revision under the store
//! `$XDG_STATE_HOME/takt/promotions/` (`~/.local/state/takt/promotions/` without
//! `XDG_STATE_HOME`), outside every repository a candidate changes, beside a copy of the
//! scorecard it read and its digest; the latest record by `promoted_at` that is no refusal
//! names the baseline the next candidate is compared with. The first record is a `baseline`
//! one, written by `takt promote --baseline-only`, which installs nothing and requires the
//! installed binary to name the revision it records; for a binary built before `--version`
//! named its revision, the operator states both (`--revision <sha> --binary-sha256 <hex>`):
//! the installed binary must name no revision and hash to `<hex>`, every run of the scorecard
//! must state `<hex>` in a `binary.txt` beside its record, and the record marks the revision
//! and each run's digest as the operator's statement. Each record carries the digest of the
//! record before it (`previous_sha256`), so a record removed from between two is noticed; the
//! latest record's file name and sha256 are kept beside the store in its head ([`Head`],
//! `<state>/takt/promotions.head`), written with every record, so a chain that ends before the
//! head, its latest record removed or replaced, is refused naming that record, a refusal
//! included; and before anything is judged the installed binary's `--version` must name the
//! latest baseline or promotion. A store with records and no head is read on trust, again
//! whenever no head is found: the head is written from the chain as it stands and the run says
//! so on stderr. The head detects a record removed or replaced by accident or by a tool; a
//! process running as the same user can rewrite both, and removing the latest record and the
//! head together leaves a shorter chain that is read on trust.
//!
//! `takt promote` reads the candidate's saved scorecard (a file outside the candidate's
//! repository with one link, whose `revision` field equals its name and the checkout's `HEAD`,
//! a full commit id that is an ancestor of the checkout's [`MAINLINE`] branch, whose rule is
//! takt's own and whose metric definitions takt knows, every run of which was made with the
//! binary of that revision, as its `opened` line names it, and every scenario run of which
//! carries its held-out result), refuses a candidate with a scenario of fewer cases than the
//! rule's trials, compares it with the baseline's copy in the store under takt's [`Rule`], and
//! refuses a candidate that is worse, writing a `refused` record so the same revision, or the
//! same tree under another commit, is not judged again until the operator supplies a scorecard
//! of new runs, none of whose cases is in a scorecard it was refused on (`--rejudge`), whose
//! record names the refusal before it ([`Rejudged`]); a baseline saved under an older metric
//! definition is replaced, nothing built or installed, by a scorecard of the same revision and
//! cases under the newer one ([`rebaseline`], `--rebaseline`); a
//! candidate that holds is built from a fresh
//! clone of the exact commit, confined in bubblewrap ([`Confinement`]): only the clone, a
//! scratch `CARGO_HOME` with the host's registry cache bound read-only, the toolchain takt
//! resolves and the system directories are visible, the environment is cleared, the process and
//! network namespaces are the build's own, and `cargo build --locked --offline` is the
//! toolchain's own binary, never a rustup proxy; the store, the install root and the real home
//! are not mounted, so nothing the commit runs reads a setting outside it, writes outside the
//! build or outlives it. Every run of the candidate's binary during a promotion (`--version`)
//! is confined the same way. The binary is installed with `cargo install --locked --path
//! crates/takt-cli` into a fresh root, hashed, recorded first and then moved into place in one
//! rename, and checked again in place against the revision and the digest; a failure after the
//! move puts the binary before it back and removes the record. Promotions run one at a time,
//! under an exclusive lock on the store's [`LOCK`] from its reading to its writing. The
//! store is read fail-closed: a file that is no record, a record whose name, kind or instant
//! disagrees with its content, a scorecard copy whose digest differs from its record, a chain
//! of records with a gap, or one that ends before its head or runs past it, refuses every
//! promotion until someone looks. `--dry-run` prints the verdict and the record it would write,
//! checks the head as every read does, and writes nothing, a head read on trust included.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::scorecard::{
    BINARY_FILE, METRICS, Row, Rule, RunBinary, Scorecard, StatedBy, is_sha256, names_no_revision,
    rank, unnamed_in_a_mix,
};

/// The promotion record's format.
pub const FORMAT: &str = "takt-promotion/1";

/// The store's path below the state directory.
pub const STORE: &str = "takt/promotions";

/// The lock file in the store, held exclusively (`flock`) from the store's reading to its
/// writing so that two promotions at once record one; the only file in the store that is no
/// record and no scorecard copy.
pub const LOCK: &str = ".lock";

/// The mainline a promoted revision must be an ancestor of: the remote's branch
/// [`MAINLINE_REMOTE`]`/`[`MAINLINE`] as the candidate's checkout last fetched it, which a
/// local branch moved by hand does not move.
pub const MAINLINE: &str = "main";

/// The remote whose [`MAINLINE`] branch is the mainline.
pub const MAINLINE_REMOTE: &str = "origin";

/// The sandbox the candidate is built in and its binary run under: bubblewrap, found on `PATH`.
pub const BWRAP: &str = "bwrap";

/// Where the confined build sees the clone: writable, at a path no ancestor of which holds a
/// `.cargo` directory, so cargo reads configuration from the commit and nowhere else.
pub const INSIDE_WORK: &str = "/work";

/// Where the confined build sees its target directory.
pub const INSIDE_TARGET: &str = "/target";

/// Where the confined build sees the toolchain, read-only; its `bin/cargo` and `bin/rustc` are
/// the ones called.
pub const INSIDE_TOOLCHAIN: &str = "/toolchain";

/// Where the confined build sees its scratch `CARGO_HOME`: writable, holding no `config.toml`,
/// with the host's [`CACHED`] directories bound read-only under it.
pub const INSIDE_CARGO_HOME: &str = "/cargo-home";

/// Where the confined build sees the fresh install root `cargo install --root` fills.
pub const INSIDE_INSTALL: &str = "/install";

/// Where the binary moved into place is seen when it is run confined after the move.
pub const INSIDE_INSTALLED: &str = "/installed";

/// The home directory inside the sandbox, an empty tmpfs.
pub const INSIDE_HOME: &str = "/build-home";

/// The host `CARGO_HOME` directories bound read-only into the scratch one: the registry's cache
/// and index, and git dependencies' databases and checkouts. Nothing is fetched (`--offline`),
/// so a crate missing from them fails the build.
pub const CACHED: [&str; 4] = [
    "registry/cache",
    "registry/index",
    "git/db",
    "git/checkouts",
];

/// What a record is: the first baseline (`takt promote --baseline-only`, nothing installed), a
/// promotion that installed a binary, a candidate refused on its scorecard, kept so the same
/// revision is not judged again but on new runs, or the baseline's scorecard replaced by one of
/// the same revision and cases under newer metric definitions, or adding whole scenarios
/// (`--rebaseline`), which installs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Baseline,
    Promotion,
    Refused,
    Rebaseline,
}

/// One promotion record, `<store>/<revision>.json`, or `<store>/<revision>.refused.json` for a
/// refusal, `<store>/<revision>.refused-<scorecard_sha256>.json` for a refusal of a candidate
/// judged again, `<store>/<revision>.rebaseline-<scorecard_sha256>.json` for a rebaseline; a
/// baseline or a promotion has the scorecard it read copied beside it as
/// `<store>/<revision>.scorecard.json`, a rebaseline as
/// `<store>/<revision>.rebaseline-<scorecard_sha256>.scorecard.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promotion {
    /// [`FORMAT`].
    pub format: String,
    /// The revision the binary was built from, a full commit id; the candidate's for a refusal.
    pub revision: String,
    /// Who stated `revision` when the binary could not: the operator, by `--revision`, on a
    /// baseline of a binary whose `--version` names no revision; none when the binary named it.
    #[serde(default)]
    pub revision_stated_by: Option<StatedBy>,
    /// The tree of that commit, so the same tree under another commit is known.
    pub tree: String,
    /// When the record was written, `YYYY-MM-DDTHH:MM:SSZ`, read after the build; no record
    /// carries one before the record before it.
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
    /// The installed binary's sha256, lower-case hex; none unless the record is a promotion or
    /// a baseline the operator stated, whose binary was hashed against the statement.
    pub binary_sha256: Option<String>,
    /// On a baseline the operator stated, each run of the scorecard with the binary it states,
    /// every digest `binary_sha256`; empty on every other record.
    #[serde(default)]
    pub runs: Vec<RecordedRun>,
    /// The sha256 of the record file written before this one, lower-case hex, so a record
    /// removed from between two is noticed; none on the first record only.
    pub previous_sha256: Option<String>,
    /// On a candidate judged again (`--rejudge`), the latest refusal of its revision or tree;
    /// none on every other record, and on one written before it had the field.
    #[serde(default)]
    pub rejudges: Option<Rejudged>,
    /// On a rebaseline, the sha256 of the baseline's scorecard it replaces, lower-case hex; none
    /// on every other record, and on one written before it had the field.
    #[serde(default)]
    pub replaces_sha256: Option<String>,
    /// On a refusal, the case ids of the scorecard refused, so a candidate judged again is
    /// judged on new runs only; empty on every other record, and on one written before it had
    /// the field.
    #[serde(default)]
    pub cases: Vec<String>,
}

/// The head of the chain of records: the latest record's file name in the store and that file's
/// sha256, lower-case hex, kept beside the store at `<store>.head` ([`head_path`]), outside the
/// record directory; written with every record ([`write`], [`unwrite`]) under the store's lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Head {
    pub record: String,
    pub sha256: String,
}

/// The refusal a candidate judged again on new runs was refused by before: its record's file
/// name in the store and that file's sha256, lower-case hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejudged {
    pub record: String,
    pub sha256: String,
}

/// One run of the scorecard a stated baseline read, with the binary it states, copied from the
/// scorecard's row: the digest, who stated it and the operator's note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedRun {
    /// The case the run worked.
    pub case: String,
    /// The binary it states.
    pub binary: RunBinary,
}

/// The build or the install of a candidate that held failed, or built something that is not the
/// candidate, or the candidate changed once in place and was rolled back: the CLI tells this
/// apart from a refusal. Nothing is recorded and the binary installed before is in place.
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
    /// With `baseline_only`: the revision the operator states the installed binary was built
    /// from, for a binary whose `--version` names none; a full commit id, the scorecard's.
    pub revision: Option<&'a str>,
    /// With `baseline_only` and `revision`: the sha256 the operator states for the installed
    /// binary, lower-case hex. The binary is hashed and must match, and every run of the
    /// scorecard must state the same digest, before anything is written.
    pub binary_sha256: Option<&'a str>,
    /// Print the verdict and the record that would be written; write and install nothing.
    pub dry_run: bool,
    /// Judge again a candidate whose revision or tree was refused, on a scorecard other than
    /// every one it was refused on; the record names the latest refusal.
    pub rejudge: bool,
    /// The clock: read once when the store is read, and again, after the build, for the instant
    /// the record carries.
    pub clock: fn() -> String,
}

/// What `takt promote --rebaseline` asks for.
pub struct Rebaseline<'a> {
    /// The scorecard that replaces the baseline's: of the baseline's revision, its cases exactly
    /// and whole scenarios it lacks, under metric definitions at or above the stored ones, one of
    /// them newer or a scenario added.
    pub scorecard: &'a Path,
    /// The store of promotion records.
    pub store: &'a Path,
    /// Where the binary is installed: its `bin/takt` must be the baseline's.
    pub install_root: &'a Path,
    /// Print the record that would be written; write nothing.
    pub dry_run: bool,
    /// The clock: read when the store is read and again for the instant the record carries.
    pub clock: fn() -> String,
}

/// How a request ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The rebaseline record is written beside the scorecard that replaces the baseline's.
    Rebaselined(Promotion),
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

/// The head of `store`'s chain ([`Head`]): the file `<store>.head` beside it,
/// `$XDG_STATE_HOME/takt/promotions.head` for the store [`store`] names.
pub fn head_path(store: &Path) -> PathBuf {
    let mut name = store.file_name().unwrap_or_default().to_os_string();
    name.push(".head");
    store.with_file_name(name)
}

/// The head of `store`'s chain; none when there is no head file. A head that is no regular file
/// or does not read as a [`Head`] naming a record by its file name and a sha256 is an error
/// naming it.
fn read_head(store: &Path) -> Result<Option<Head>> {
    let path = head_path(store);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => bail!(
            "{}: the head of the chain of records is not a regular file; the store is not read past this",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let head: Head = serde_json::from_str(&text).with_context(|| {
        format!(
            "{} is not the head of a chain of records; the store is not read past this",
            path.display()
        )
    })?;
    let plain = Path::new(&head.record).file_name() == Some(std::ffi::OsStr::new(&head.record));
    if !plain || !head.record.ends_with(".json") || !is_sha256(&head.sha256) {
        bail!(
            "{}: names {:?} with the digest {:?}, not a record's file name and a sha256; the store is not read past this",
            path.display(),
            head.record,
            head.sha256
        );
    }
    Ok(Some(head))
}

/// Points the head of `store`'s chain at `latest`'s record file, or removes it when `latest` is
/// none: written and synced to a temporary file beside it, then renamed into place.
fn set_head(store: &Path, latest: Option<&Promotion>) -> Result<()> {
    let path = head_path(store);
    let Some(latest) = latest else {
        return match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(error).with_context(|| format!("remove {}", path.display()))
            }
            _ => Ok(()),
        };
    };
    let record = record_path(store, latest);
    let head = Head {
        record: record
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        sha256: sha256_of(&record)?,
    };
    let mut text = serde_json::to_string_pretty(&head).context("write the head")?;
    text.push('\n');
    let temporary = path.with_file_name(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, &path)
    })()
    .with_context(|| format!("write {}", path.display()));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

/// The chain `records` against its `head`: the latest record must be the file the head names,
/// with its digest. A chain that ends before the head, the record it names missing, or runs past
/// it is an error naming the record.
fn ends_at_head(store: &Path, head: &Head, records: &[Promotion]) -> Result<()> {
    let shown = head_path(store);
    let named = store.join(&head.record);
    let Some(latest) = records.last() else {
        bail!(
            "{} names {} ({}) as the latest record, but the store holds none; the record is missing: a record is missing or was removed, and the store is not read past this",
            shown.display(),
            head.record,
            head.sha256
        );
    };
    let last = record_path(store, latest);
    if last == named {
        return Ok(());
    }
    if records
        .iter()
        .any(|record| record_path(store, record) == named)
    {
        bail!(
            "{} names {} as the latest record, but {} follows it in the chain; a record was written the head does not name, and the store is not read past this",
            shown.display(),
            named.display(),
            last.display()
        );
    }
    bail!(
        "{} names {} ({}) as the latest record, but the chain ends before it, at {}; the record is missing: a record is missing or was removed, and the store is not read past this",
        shown.display(),
        head.record,
        head.sha256,
        last.display()
    )
}

/// A store with records and no head is read on trust, again whenever no head is found: the head
/// is written from the chain `records` as it stands, and stderr says so; a dry run says so and
/// writes nothing.
fn trust(store: &Path, records: &[Promotion], head: Option<&Head>, dry_run: bool) -> Result<()> {
    let Some(latest) = records.last().filter(|_| head.is_none()) else {
        return Ok(());
    };
    let shown = head_path(store);
    let last = record_path(store, latest);
    if dry_run {
        eprintln!(
            "takt: {} does not exist: the chain of records is read on trust as it stands, ending at {}; a dry run writes no head",
            shown.display(),
            last.display()
        );
        return Ok(());
    }
    set_head(store, Some(latest))?;
    eprintln!(
        "takt: {} did not exist: the chain of records is read on trust as it stands, and the head is written naming {}; from now on a chain that ends before it is refused",
        shown.display(),
        last.display()
    );
    Ok(())
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

pub(crate) fn state_dir() -> Result<PathBuf> {
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

/// The host's `CARGO_HOME`, whose [`CACHED`] directories the confined build reads:
/// `CARGO_HOME`, else `~/.cargo`; none when the directory does not exist. Its `config.toml` is
/// never read.
fn cargo_home() -> Option<PathBuf> {
    let dir = match std::env::var_os("CARGO_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => home().ok()?.join(".cargo"),
    };
    dir.is_dir().then_some(dir)
}

/// The record's file in `store`: by its revision and kind, and for a refusal of a candidate
/// judged again also by the digest of the scorecard it read, which differs from every earlier
/// refusal's of that candidate.
fn record_path(store: &Path, record: &Promotion) -> PathBuf {
    let revision = &record.revision;
    match (record.kind, &record.rejudges) {
        (Kind::Refused, None) => store.join(format!("{revision}.refused.json")),
        (Kind::Refused, Some(_)) => store.join(format!(
            "{revision}.refused-{}.json",
            record.scorecard_sha256
        )),
        (Kind::Rebaseline, _) => store.join(format!(
            "{revision}.rebaseline-{}.json",
            record.scorecard_sha256
        )),
        (Kind::Baseline | Kind::Promotion, _) => store.join(format!("{revision}.json")),
    }
}

/// The store's copy of the scorecard a baseline, a promotion or a rebaseline read, beside its
/// record.
fn copy_path(store: &Path, record: &Promotion) -> PathBuf {
    let revision = &record.revision;
    match record.kind {
        Kind::Rebaseline => store.join(format!(
            "{revision}.rebaseline-{}.scorecard.json",
            record.scorecard_sha256
        )),
        _ => store.join(format!("{revision}.scorecard.json")),
    }
}

/// Whether `stem` names a scorecard copy: `<revision>` or `<revision>.rebaseline-<sha256>`.
fn is_copy_stem(stem: &str) -> bool {
    match stem.split_once(".rebaseline-") {
        Some((revision, digest)) => is_full_revision(revision) && is_sha256(digest),
        None => is_full_revision(stem),
    }
}

/// Whether `candidate` is a full commit id, 40 or 64 lower-case hex digits.
fn is_full_revision(candidate: &str) -> bool {
    matches!(candidate.len(), 40 | 64)
        && candidate
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Every record in `store`, oldest first in the order of their chain ([`chained`]); none when the
/// store does not exist. Read fail-closed: a file that is no record or no scorecard copy (the
/// [`LOCK`] aside), a record whose name does not carry its revision and kind, one whose instant
/// does not parse or lies after `now`, a scorecard copy without its record or with a digest
/// other than the record's, or a record whose `previous_sha256` is not the digest of the record
/// before it, is an error naming it; so is a chain that ends before its head ([`Head`]) or runs
/// past it, and a record the head names whose digest is not the head's, before the record is
/// read. A store with no head is read as it stands ([`trust`]).
pub fn records(store: &Path, now: &str) -> Result<Vec<Promotion>> {
    Ok(read(store, now)?.0)
}

/// [`records`], with the head they were checked against; none when there is no head file.
fn read(store: &Path, now: &str) -> Result<(Vec<Promotion>, Option<Head>)> {
    let head = read_head(store)?;
    if let Some(head) = &head {
        let named = store.join(&head.record);
        if named.is_file() {
            let digest = sha256_of(&named)?;
            if digest != head.sha256 {
                bail!(
                    "{} names {} with the digest {} as the latest record, but the file hashes to {digest}; the record was replaced, and the store is not read past this",
                    head_path(store).display(),
                    named.display(),
                    head.sha256
                );
            }
        }
    }
    let records = chain(store, now)?;
    if let Some(head) = &head {
        ends_at_head(store, head, &records)?;
    }
    Ok((records, head))
}

/// The chain of records in `store`, read as [`records`] says, the head aside.
fn chain(store: &Path, now: &str) -> Result<Vec<Promotion>> {
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
        if name == LOCK {
            continue;
        }
        if let Some(stem) = name.strip_suffix(".scorecard.json") {
            if !is_copy_stem(stem) {
                bail!(
                    "{}: a scorecard copy is named by a full commit id, a rebaseline's by `<revision>.rebaseline-<sha256>`; the store is not read past this",
                    path.display()
                );
            }
            copies.insert(name.into_owned());
            continue;
        }
        let Some(stem) = name.strip_suffix(".json") else {
            bail!(
                "{}: not a record (`<revision>.json`, `<revision>.refused.json`) nor a scorecard copy (`<revision>.scorecard.json`); the store is not read past this",
                path.display()
            );
        };
        let (named_revision, named_kind) = match (
            stem.strip_suffix(".refused"),
            stem.split_once(".refused-"),
            stem.split_once(".rebaseline-"),
        ) {
            (Some(revision), _, _) | (None, Some((revision, _)), _) => (revision, Kind::Refused),
            (None, None, Some((revision, _))) => (revision, Kind::Rebaseline),
            (None, None, None) => (stem, Kind::Promotion),
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
            Kind::Refused | Kind::Rebaseline => record.kind == named_kind,
            Kind::Baseline | Kind::Promotion => {
                matches!(record.kind, Kind::Baseline | Kind::Promotion)
            }
        };
        if !kind_agrees {
            bail!(
                "{}: the file name says {} and the record says {:?}; the store is not read past this",
                path.display(),
                match named_kind {
                    Kind::Refused => "refused",
                    Kind::Rebaseline => "a rebaseline",
                    Kind::Baseline | Kind::Promotion => "a baseline or a promotion",
                },
                record.kind
            );
        }
        let named = record_path(store, &record);
        if named != path {
            bail!(
                "{}: the record names its file {}; the store is not read past this",
                path.display(),
                named.display()
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
        let copy = copy_path(store, record);
        let copy_name = copy
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !copies.remove(&copy_name) {
            bail!(
                "{}: the record's scorecard copy {} is missing; the store is not read past this",
                record_path(store, record).display(),
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
            store.join(orphan).display()
        );
    }
    records.sort_by(|a, b| {
        a.promoted_at
            .cmp(&b.promoted_at)
            .then_with(|| a.revision.cmp(&b.revision))
    });
    let ordered = chained(store, records)?;
    rejudges_in_chain(store, &ordered)?;
    rebaselines_in_chain(store, &ordered)?;
    Ok(ordered)
}

/// Every rebaseline in the chain `records` replaces the baseline before it: its
/// `replaces_sha256` is the scorecard digest of the latest record before it that is no refusal,
/// of its revision, and the case ids of its scorecard copy are that record's copy's, but for the
/// cases of whole scenarios that copy lacks ([`added_scenarios`]). One that replaces anything
/// else is an error naming it.
fn rebaselines_in_chain(store: &Path, records: &[Promotion]) -> Result<()> {
    for (n, record) in records.iter().enumerate() {
        if record.kind != Kind::Rebaseline {
            continue;
        }
        let path = record_path(store, record);
        let replaced = records[..n]
            .iter()
            .rfind(|earlier| earlier.kind != Kind::Refused)
            .filter(|earlier| {
                Some(&earlier.scorecard_sha256) == record.replaces_sha256.as_ref()
                    && earlier.revision == record.revision
            });
        let Some(replaced) = replaced else {
            bail!(
                "{}: its replaces_sha256 {} is not the scorecard of a baseline of {} before it in the chain; the store is not read past this",
                path.display(),
                record.replaces_sha256.as_deref().unwrap_or("(none)"),
                record.revision
            );
        };
        let (copy, replaced_copy) = (copy_path(store, record), copy_path(store, replaced));
        if let Err(why) =
            added_scenarios(&Scorecard::load(&copy)?, &Scorecard::load(&replaced_copy)?)
        {
            bail!(
                "{}: the case ids of its scorecard {} are not those of the scorecard it replaces, {}, with whole scenarios added: {why}; the store is not read past this",
                path.display(),
                copy.display(),
                replaced_copy.display()
            );
        }
    }
    Ok(())
}

/// The scenarios a rebaseline's `card` adds to the `stored` scorecard it replaces: every case of
/// `stored` is one of `card`'s, and every case `card` adds is of a scenario `stored` has no case
/// of, so the stored cases are re-scored exactly and a new scenario joins the corpus whole,
/// never a case slipped into a scenario the baseline judges already. Otherwise the first case
/// that is not, named with why.
fn added_scenarios(
    card: &Scorecard,
    stored: &Scorecard,
) -> std::result::Result<BTreeSet<String>, String> {
    let (cases, stored_cases) = (case_ids(card), case_ids(stored));
    if let Some(case) = stored_cases.difference(&cases).next() {
        return Err(format!(
            "case {case} is among the baseline's and not this scorecard's"
        ));
    }
    let stored_scenarios: BTreeSet<&str> = stored
        .rows
        .iter()
        .filter_map(|row| row.scenario.as_deref())
        .collect();
    let mut added = BTreeSet::new();
    for row in card
        .rows
        .iter()
        .filter(|row| !stored_cases.contains(&row.case))
    {
        match row.scenario.as_deref() {
            None => {
                return Err(format!(
                    "case {} is in this scorecard and not among the baseline's, and is of no scenario",
                    row.case
                ));
            }
            Some(scenario) if stored_scenarios.contains(scenario) => {
                return Err(format!(
                    "case {} is in this scorecard and not among the baseline's, and its scenario {scenario} is the baseline's",
                    row.case
                ));
            }
            Some(scenario) => {
                added.insert(scenario.to_owned());
            }
        }
    }
    Ok(added)
}

/// Every run of `card` is the run of `stored` of the same case at the same position, re-derived
/// under newer definitions and nothing else: each case has as many runs as stored, and every
/// field whose definition did not change is equal: the steps, tokens, refusals, `NoProgress`,
/// errors and wall time under unchanged metric versions, the outcome when its definition is
/// unchanged, and always the stop (the stored outcome under the outcome definition of version 1,
/// which was the stop) and the revision the run's `opened` line names. A stored run saved
/// without that revision accepts a run naming exactly the revision of `base`, the record whose
/// scorecard is replaced, and, when `base` states its revision (a binary that names none), a
/// run naming none; anything else is refused. No run's outcome may rank below the stored one's,
/// so a rebaseline keeps or raises the bar and never lowers it. A stored scorecard with no
/// outcome metric is refused. The first run that is not bound is the refusal, named with the
/// field and both figures. A case of a scenario in `added`, one the rebaseline adds whole
/// ([`added_scenarios`]), has no stored run to be bound to; its runs are checked as a
/// baseline's are, and nothing here.
fn runs_bound(
    card: &Scorecard,
    stored: &Scorecard,
    added: &BTreeSet<String>,
    base: &Promotion,
    shown: &Path,
    stored_path: &Path,
) -> Result<()> {
    if !stored.metrics.contains_key("outcome") {
        bail!(
            "{}: the stored scorecard has no outcome metric, so its outcomes cannot be bound to a re-read; --rebaseline is refused",
            stored_path.display()
        );
    }
    let runs_of = |card: &'_ Scorecard| {
        let mut runs: BTreeMap<String, Vec<Row>> = BTreeMap::new();
        for row in &card.rows {
            runs.entry(row.case.clone()).or_default().push(row.clone());
        }
        runs
    };
    let (runs, stored_runs) = (runs_of(card), runs_of(stored));
    let unchanged = |metric: &str| card.metrics.get(metric) == stored.metrics.get(metric);
    for (case, now) in &runs {
        if crate::scorecard::scenario_of(case).is_some_and(|scenario| added.contains(scenario)) {
            continue;
        }
        let then = stored_runs.get(case).map_or(&[][..], Vec::as_slice);
        if now.len() != then.len() {
            bail!(
                "{}: case {case} has {} runs, {} stored ({}); --rebaseline re-scores the stored runs and no other",
                shown.display(),
                now.len(),
                then.len(),
                stored_path.display()
            );
        }
        for (n, (run, was)) in now.iter().zip(then).enumerate() {
            let stored_stop = if stored.metrics.get("outcome") == Some(&1) {
                &was.outcome
            } else {
                &was.stop
            };
            let mut fields: Vec<(&str, String, String)> =
                vec![("stop", run.stop.clone(), stored_stop.clone())];
            if unchanged("outcome") {
                fields.push(("outcome", run.outcome.clone(), was.outcome.clone()));
            }
            for (metric, field, figure, stored_figure) in [
                ("steps", "steps", run.steps, was.steps),
                ("tokens", "tokens", run.tokens, was.tokens),
                ("refusals", "refusals", run.refusals, was.refusals),
                (
                    "no_progress",
                    "no_progress",
                    run.no_progress,
                    was.no_progress,
                ),
                ("errors", "errors", run.errors, was.errors),
                ("wall", "wall_ms", run.wall_ms, was.wall_ms),
            ] {
                if unchanged(metric) {
                    fields.push((field, figure.to_string(), stored_figure.to_string()));
                }
            }
            let named = run
                .opened_binary
                .clone()
                .unwrap_or_else(|| "(none)".to_owned());
            match (&run.opened_binary, &was.opened_binary) {
                (_, Some(then)) => fields.push(("opened_binary", named, then.clone())),
                (Some(now), None) if *now == base.revision => {}
                (None, None) if base.revision_stated_by.is_some() => {}
                (_, None) => fields.push((
                    "opened_binary",
                    named,
                    format!("(none, which accepts {} only)", base.revision),
                )),
            }
            if let Some((field, figure, stored_figure)) = fields
                .into_iter()
                .find(|(_, figure, stored)| figure != stored)
            {
                bail!(
                    "{}: run {case}#{} gives {field} {figure:?}, the stored run {stored_figure:?} ({}); --rebaseline re-derives what a newer definition changes and nothing else",
                    shown.display(),
                    n + 1,
                    stored_path.display()
                );
            }
            if rank(&run.outcome) < rank(&was.outcome) {
                bail!(
                    "{}: run {case}#{} has the outcome {}, which ranks below the stored {} ({}); a rebaseline keeps or raises the bar and never lowers it",
                    shown.display(),
                    n + 1,
                    run.outcome,
                    was.outcome,
                    stored_path.display()
                );
            }
        }
    }
    Ok(())
}

/// Every record that judges a candidate again names, in `rejudges`, a refusal before it in the
/// chain `records`, of its revision or its tree, by that record's file name and the file's
/// sha256; one that names anything else is an error naming both.
fn rejudges_in_chain(store: &Path, records: &[Promotion]) -> Result<()> {
    for (n, record) in records.iter().enumerate() {
        let Some(rejudged) = &record.rejudges else {
            continue;
        };
        let mut found = false;
        for earlier in records[..n].iter().filter(|earlier| {
            earlier.kind == Kind::Refused
                && (earlier.revision == record.revision || earlier.tree == record.tree)
        }) {
            let path = record_path(store, earlier);
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy() == rejudged.record)
                && sha256_of(&path)? == rejudged.sha256
            {
                found = true;
                break;
            }
        }
        if !found {
            bail!(
                "{}: its rejudges names {} with the digest {}, which is no refusal of its revision or tree before it in the chain; the store is not read past this",
                record_path(store, record).display(),
                rejudged.record,
                rejudged.sha256
            );
        }
    }
    Ok(())
}

/// `records` in the order of their chain: the one record that carries no `previous_sha256`
/// first, then each record whose `previous_sha256` is the digest of the record file before it,
/// so two records of the same second keep the order they were written in. A second first
/// record, two records naming the same one before them, a record whose `previous_sha256` is the
/// digest of no record in the store, or a record whose instant lies before the one before it, is
/// an error naming it.
fn chained(store: &Path, records: Vec<Promotion>) -> Result<Vec<Promotion>> {
    let total = records.len();
    let mut first: Option<Promotion> = None;
    let mut after: std::collections::BTreeMap<String, Promotion> =
        std::collections::BTreeMap::new();
    for record in records {
        let path = record_path(store, &record);
        match record.previous_sha256.clone() {
            None => {
                if let Some(other) = &first {
                    bail!(
                        "{}: carries no previous_sha256, but {} is the first record already; the chain of records is broken, and the store is not read past this",
                        path.display(),
                        record_path(store, other).display()
                    );
                }
                first = Some(record);
            }
            Some(previous) => {
                if let Some(other) = after.get(&previous) {
                    bail!(
                        "{}: its previous_sha256 {previous} is the one {} carries too; two records follow one, and the store is not read past this",
                        path.display(),
                        record_path(store, other).display()
                    );
                }
                after.insert(previous, record);
            }
        }
    }
    let mut ordered = Vec::with_capacity(total);
    let mut cursor = first;
    while let Some(record) = cursor {
        let path = record_path(store, &record);
        if let Some(before) = ordered
            .last()
            .map(|earlier: &Promotion| earlier.promoted_at.clone())
            && record.promoted_at < before
        {
            bail!(
                "{}: promoted_at {} lies before the record before it ({before}); the store is not read past this",
                path.display(),
                record.promoted_at
            );
        }
        cursor = after.remove(&sha256_of(&path)?);
        ordered.push(record);
    }
    if let Some((previous, record)) = after.into_iter().next() {
        bail!(
            "{}: its previous_sha256 {previous} is the digest of no record in the chain; the record before it is missing or changed, and the store is not read past this",
            record_path(store, &record).display()
        );
    }
    Ok(ordered)
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

/// The revision in the parentheses of a `takt <version> (<revision>)` line.
fn parse_version(line: &str) -> Option<String> {
    line.rsplit_once(" (")
        .and_then(|(_, rest)| rest.strip_suffix(')'))
        .map(str::to_owned)
}

/// The revision `takt --version` prints, the binary run as it is: for the binary installed
/// before a promotion, which is the one runs use anyway; a candidate's binary is run confined
/// ([`Confinement::printed_revision`]) instead.
fn printed_revision(binary: &Path) -> Result<String> {
    let line = printed_line(binary)?;
    parse_version(&line).ok_or_else(|| {
        anyhow!(
            "{} --version printed {line:?}, not `takt <version> (<revision>)`",
            binary.display()
        )
    })
}

/// The first line `binary --version` prints, run unconfined.
fn printed_line(binary: &Path) -> Result<String> {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .with_context(|| format!("run {} --version", binary.display()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().next().unwrap_or("").to_owned())
}

/// The sha256 of the file at `path`, lower-case hex.
fn sha256_of(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

/// The last five lines of `bytes`, joined by ` | `.
fn last_lines(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let from = lines.len().saturating_sub(5);
    lines[from..].join(" | ")
}

/// Whether a regular file at `path` has an execute bit.
fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
}

/// Publishes `text` at `path` in the store: written and synced to a temporary file beside it,
/// then linked into place, so a reader sees the whole file or none; a file there already is not
/// overwritten.
fn publish(path: &Path, text: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no directory", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("{} has no UTF-8 name", path.display()))?;
    let temporary = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(text)?;
        file.sync_all()
    })()
    .with_context(|| format!("write {}", temporary.display()));
    let linked = written.and_then(|()| match std::fs::hard_link(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(anyhow!(
            "{} exists: a store file is not overwritten",
            path.display()
        )),
        Err(error) => Err(error).with_context(|| format!("write {}", path.display())),
    });
    let _ = std::fs::remove_file(&temporary);
    linked
}

/// Writes `record` to the store, with the scorecard it read copied beside it unless it is a
/// refusal, the copy first, then the head pointed at it ([`set_head`]); a record that cannot be
/// written leaves no copy, and one whose head cannot be written is removed again, the head as it
/// was. The record's path.
fn write(store: &Path, record: &Promotion, scorecard_text: &[u8]) -> Result<PathBuf> {
    std::fs::create_dir_all(store).with_context(|| format!("create {}", store.display()))?;
    let path = record_path(store, record);
    let mut text = serde_json::to_string_pretty(record).context("write the record")?;
    text.push('\n');
    let copy = (record.kind != Kind::Refused).then(|| copy_path(store, record));
    if let Some(copy) = &copy {
        publish(copy, scorecard_text)?;
    }
    let written = publish(&path, text.as_bytes()).and_then(|()| {
        set_head(store, Some(record)).inspect_err(|_| {
            let _ = std::fs::remove_file(&path);
        })
    });
    if let Err(error) = written {
        if let Some(copy) = &copy {
            let _ = std::fs::remove_file(copy);
        }
        return Err(error);
    }
    Ok(path)
}

/// Removes `record` and its scorecard copy from the store, after the install that followed the
/// record failed, and points the head back at `before`, the record before it; a record that
/// could not be removed stays the head. What could not be removed or pointed back is the error.
fn unwrite(store: &Path, record: &Promotion, before: Option<&Promotion>) -> Result<()> {
    let file = record_path(store, record);
    let mut paths = vec![file.clone()];
    if record.kind != Kind::Refused {
        paths.push(copy_path(store, record));
    }
    let mut failed = Vec::new();
    for path in paths {
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failed.push(format!("{}: {error}", path.display()));
        }
    }
    if !file.exists()
        && let Err(error) = set_head(store, before)
    {
        failed.push(format!("{error:#}"));
    }
    if failed.is_empty() {
        Ok(())
    } else {
        bail!("remove the record: {}", failed.join("; "))
    }
}

/// `error` with `note` appended, a [`BuildFailed`] staying one.
fn with_note(error: anyhow::Error, note: &str) -> anyhow::Error {
    match error.downcast::<BuildFailed>() {
        Ok(BuildFailed(text)) => BuildFailed(format!("{text}; {note}")).into(),
        Err(error) => anyhow!("{error:#}; {note}"),
    }
}

/// The store's lock, held while the store is read and written; released when dropped.
struct StoreLock(#[allow(dead_code)] std::fs::File);

/// Takes the store's exclusive lock ([`LOCK`], `flock(LOCK_EX)`), creating the store; one held
/// by another promotion is a refusal naming it, not a wait.
fn lock(store: &Path) -> Result<StoreLock> {
    std::fs::create_dir_all(store).with_context(|| format!("create {}", store.display()))?;
    let path = store.join(LOCK);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("open the store lock {}", path.display()))?;
    // SAFETY: `flock` on a descriptor this process owns and keeps open for as long as the lock.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::WouldBlock {
            bail!(
                "another promotion holds the store lock {}; one promotion at a time",
                path.display()
            );
        }
        return Err(error).with_context(|| format!("lock {}", path.display()));
    }
    Ok(StoreLock(file))
}

/// Where the candidate is built and its binary run: bubblewrap, the toolchain and the host's
/// cargo caches, resolved by takt before the candidate is judged, never from the commit.
#[derive(Debug, Clone)]
pub struct Confinement {
    /// The `bwrap` binary, found on `PATH`.
    pub bwrap: PathBuf,
    /// The toolchain's directory: rustup's default toolchain, read from its settings file
    /// ([`Confinement::toolchain`]), whose `bin/cargo` and sibling `bin/rustc` are the ones
    /// called; never one found on `PATH`, a `rust-toolchain.toml` or `RUSTUP_TOOLCHAIN` picks.
    pub toolchain: PathBuf,
    /// The host's `CARGO_HOME`, whose [`CACHED`] directories are bound read-only; none when it
    /// does not exist.
    pub cargo_home: Option<PathBuf>,
}

impl Confinement {
    /// Finds `bwrap` on `PATH` and the toolchain; either missing is an error naming it, and a
    /// candidate is never built unconfined.
    pub fn resolve() -> Result<Self> {
        let bwrap = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join(BWRAP))
                .find(|candidate| is_executable(candidate))
        });
        let Some(bwrap) = bwrap else {
            bail!(
                "bubblewrap (`{BWRAP}`) is not on PATH; a promotion builds the candidate confined in it and never unconfined, so install bubblewrap first"
            );
        };
        Ok(Self {
            bwrap,
            toolchain: Self::toolchain()?,
            cargo_home: cargo_home(),
        })
    }

    /// The toolchain a promotion builds with, resolved once and without running anything or
    /// searching `PATH`: rustup's home (`RUSTUP_HOME`, else `~/.rustup`), the
    /// `default_toolchain` its `settings.toml` names, and that toolchain's directory under
    /// `toolchains/` (by its name, or the one directory named by it and a target triple). Its
    /// `bin/cargo` and the sibling `bin/rustc` must be executable files and `lib/rustlib` must
    /// exist; anything missing is an error naming it.
    pub fn toolchain() -> Result<PathBuf> {
        let rustup_home = match std::env::var_os("RUSTUP_HOME").filter(|dir| !dir.is_empty()) {
            Some(dir) if Path::new(&dir).is_absolute() => PathBuf::from(dir),
            _ => home()?.join(".rustup"),
        };
        let settings = rustup_home.join("settings.toml");
        let text = std::fs::read_to_string(&settings).with_context(|| {
            format!(
                "read {}, rustup's settings, which name the toolchain a promotion builds with; a promotion finds no toolchain on PATH",
                settings.display()
            )
        })?;
        let name = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("default_toolchain"))
            .filter_map(|rest| rest.trim_start().strip_prefix('='))
            .map(|value| value.trim().trim_matches('"').to_owned())
            .find(|value| !value.is_empty());
        let Some(name) = name else {
            bail!(
                "{} names no default_toolchain; a promotion builds with rustup's default toolchain and finds none on PATH",
                settings.display()
            );
        };
        if name.contains('/') || name.starts_with('.') {
            bail!(
                "{}: default_toolchain {name:?} is no toolchain name",
                settings.display()
            );
        }
        let toolchains = rustup_home.join("toolchains");
        let exact = toolchains.join(&name);
        let toolchain = if exact.is_dir() {
            exact
        } else {
            let prefix = format!("{name}-");
            let mut found: Vec<PathBuf> = std::fs::read_dir(&toolchains)
                .with_context(|| format!("read {}", toolchains.display()))?
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();
            match found.len() {
                1 => found.remove(0),
                0 => bail!(
                    "rustup's default toolchain {name} ({}) is not installed under {}",
                    settings.display(),
                    toolchains.display()
                ),
                n => bail!(
                    "rustup's default toolchain {name} ({}) matches {n} directories under {}; name it in full",
                    settings.display(),
                    toolchains.display()
                ),
            }
        };
        for binary in ["bin/cargo", "bin/rustc"] {
            if !is_executable(&toolchain.join(binary)) {
                bail!(
                    "the toolchain {} (rustup's default, {}) has no executable {binary}; a promotion calls the toolchain's own cargo and its sibling rustc, never a rustc on PATH",
                    toolchain.display(),
                    settings.display()
                );
            }
        }
        if !toolchain.join("lib/rustlib").is_dir() {
            bail!(
                "the toolchain {} (rustup's default, {}) has no lib/rustlib",
                toolchain.display(),
                settings.display()
            );
        }
        Ok(toolchain)
    }

    /// A bwrap command with the system directories read-only, its own process and network
    /// namespaces, dying with takt, the environment cleared but for `HOME` and `TMPDIR`, and an
    /// empty `/tmp` and home directory; the caller binds what else is seen and sets `PATH`.
    fn sandbox(&self) -> Command {
        let mut command = Command::new(&self.bwrap);
        command.env_clear();
        command.args([
            "--unshare-pid",
            "--unshare-net",
            "--die-with-parent",
            "--clearenv",
            "--setenv",
            "HOME",
            INSIDE_HOME,
            "--setenv",
            "TMPDIR",
            "/tmp",
            "--ro-bind",
            "/usr",
            "/usr",
        ]);
        for dir in ["/lib", "/lib64", "/bin", "/sbin"] {
            match std::fs::read_link(dir) {
                Ok(target) => {
                    command.arg("--symlink").arg(target).arg(dir);
                }
                Err(_) if Path::new(dir).is_dir() => {
                    command.args(["--ro-bind", dir, dir]);
                }
                Err(_) => {}
            }
        }
        command.args([
            "--ro-bind",
            "/etc",
            "/etc",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--tmpfs",
            INSIDE_HOME,
        ]);
        command
    }

    /// Runs the toolchain's `cargo <args>` confined, in the clone at [`INSIDE_WORK`], with the
    /// target directory, the scratch `CARGO_HOME` (the host's [`CACHED`] directories read-only
    /// under it) and the fresh install root bound, the command shown on `out`; a failure is a
    /// [`BuildFailed`].
    fn cargo(&self, work: &Work, args: &[&str], out: &mut dyn Write) -> Result<()> {
        writeln!(out, "cargo {} (confined in {BWRAP})", args.join(" "))?;
        let mut command = self.sandbox();
        command.args([
            "--setenv",
            "PATH",
            &format!("{INSIDE_TOOLCHAIN}/bin:/usr/bin:/bin"),
            "--setenv",
            "CARGO_HOME",
            INSIDE_CARGO_HOME,
            "--setenv",
            "RUSTC",
            &format!("{INSIDE_TOOLCHAIN}/bin/rustc"),
        ]);
        command
            .arg("--ro-bind")
            .arg(&self.toolchain)
            .arg(INSIDE_TOOLCHAIN);
        command.arg("--bind").arg(&work.source).arg(INSIDE_WORK);
        command.arg("--bind").arg(&work.target).arg(INSIDE_TARGET);
        command
            .arg("--bind")
            .arg(&work.cargo_home)
            .arg(INSIDE_CARGO_HOME);
        if let Some(host) = &self.cargo_home {
            for sub in CACHED {
                let dir = host.join(sub);
                if dir.is_dir() {
                    command
                        .arg("--ro-bind")
                        .arg(&dir)
                        .arg(format!("{INSIDE_CARGO_HOME}/{sub}"));
                }
            }
        }
        command.arg("--bind").arg(&work.install).arg(INSIDE_INSTALL);
        command.args([
            "--chdir",
            INSIDE_WORK,
            "--",
            &format!("{INSIDE_TOOLCHAIN}/bin/cargo"),
        ]);
        command.args(args);
        let output = command
            .output()
            .with_context(|| format!("run {BWRAP} for cargo {}", args.join(" ")))?;
        if !output.status.success() {
            return Err(BuildFailed(format!(
                "cargo {} confined in {BWRAP} exited {}: {}",
                args.join(" "),
                output.status,
                last_lines(&output.stderr)
            ))
            .into());
        }
        Ok(())
    }

    /// The revision the binary at `binary` prints for `--version`, run confined with its
    /// directory bound read-only at `at` and nothing of the host visible but the system
    /// directories.
    fn printed_revision(&self, binary: &Path, at: &str) -> Result<String> {
        let dir = binary
            .parent()
            .ok_or_else(|| anyhow!("{} has no directory", binary.display()))?;
        let name = binary
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("{} has no UTF-8 name", binary.display()))?;
        let mut command = self.sandbox();
        command.args(["--setenv", "PATH", "/usr/bin:/bin"]);
        command.arg("--ro-bind").arg(dir).arg(at);
        command.args([
            "--chdir",
            INSIDE_HOME,
            "--",
            &format!("{at}/{name}"),
            "--version",
        ]);
        let output = command
            .output()
            .with_context(|| format!("run {} --version confined in {BWRAP}", binary.display()))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let line = text.lines().next().unwrap_or("");
        parse_version(line).ok_or_else(|| {
            let stderr = last_lines(&output.stderr);
            anyhow!(
                "{} --version, confined at {at}, printed {line:?}, not `takt <version> (<revision>)`{}",
                binary.display(),
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!("; stderr: {stderr}")
                }
            )
        })
    }
}

/// The build's directories on the host: under the build directory, the clone, its target
/// directory and the scratch `CARGO_HOME`; and the fresh install root beside the real one, on
/// its file system, so the binary moves into place in one rename. All removed when dropped.
struct Work {
    dir: PathBuf,
    source: PathBuf,
    target: PathBuf,
    cargo_home: PathBuf,
    install: PathBuf,
}

impl Drop for Work {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.install);
    }
}

/// A candidate built and installed into the fresh root, not yet in place.
struct Staged {
    /// The directories the build used, kept until the binary is in place.
    #[allow(dead_code)]
    work: Work,
    /// The binary in the fresh root.
    binary: PathBuf,
    /// Its sha256, lower-case hex.
    digest: String,
}

/// Checks the candidate scorecard and the checkout agree: the scorecard, the store, the install
/// root and the build directory lie outside the repository, the scorecard has one link, its
/// `revision` is a full commit id equal to its file name and to `HEAD` and an ancestor of the
/// [`MAINLINE`] branch, its rule is takt's and
/// its metric definitions are the ones takt knows, the checkout is clean (no tracked file
/// differs from `HEAD`, no untracked file, no entry marked assume-unchanged or skip-worktree),
/// and the commit names no toolchain of its own (a `rust-toolchain.toml` with a `path`). The
/// scorecard, its text, the revision and its tree.
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
    merged(repo, &revision)?;
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
    no_own_toolchain(repo)?;
    Ok((card, text, revision, tree))
}

/// `revision` is an ancestor of the tip of `repo`'s remote-tracking branch
/// [`MAINLINE_REMOTE`]`/`[`MAINLINE`], or that tip: a promotion installs a revision merged
/// into the remote's mainline, as the checkout last fetched it, and never a branch's tip that is
/// not; a local `main` moved onto that tip moves nothing here. A checkout without the
/// remote-tracking branch is refused naming it.
fn merged(repo: &Path, revision: &str) -> Result<()> {
    let tip = git(
        repo,
        &[
            "rev-parse",
            "--verify",
            "-q",
            &format!("refs/remotes/{MAINLINE_REMOTE}/{MAINLINE}^{{commit}}"),
        ],
    )
    .map_err(|_| {
        anyhow!(
            "{} has no {MAINLINE_REMOTE}/{MAINLINE}: a promotion installs a revision merged into the remote's {MAINLINE}, so fetch it first",
            repo.display()
        )
    })?;
    let status = Command::new("git")
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "merge-base",
            "--is-ancestor",
            revision,
            &tip,
        ])
        .current_dir(repo)
        .status()
        .with_context(|| format!("run git merge-base in {}", repo.display()))?;
    match status.code() {
        Some(0) => Ok(()),
        Some(1) => bail!(
            "{revision} is not an ancestor of {MAINLINE_REMOTE}/{MAINLINE} ({tip}) in {}: a promotion installs a revision merged into the remote's {MAINLINE}, so merge it there and fetch first",
            repo.display()
        ),
        _ => bail!(
            "git merge-base --is-ancestor {revision} {tip} in {} exited {status}",
            repo.display()
        ),
    }
}

/// Every run of `card` was made with the binary of `revision` when one is given, as its
/// `opened` line names it (`<sha>-dirty` and no revision are other binaries), and every run of a
/// scenario case carries its held-out result: a promotion judges a revision on runs of its own
/// binary, judged by the held-out check. The first run that is not is the refusal, named
/// `<case>#<n>`.
fn runs_judged(card: &Scorecard, revision: Option<&str>, shown: &Path) -> Result<()> {
    let mut counted: BTreeMap<&str, u64> = BTreeMap::new();
    for row in &card.rows {
        let n = counted.entry(row.case.as_str()).or_insert(0);
        *n += 1;
        let run = format!("{}#{n}", row.case);
        if let Some(revision) = revision
            && row.opened_binary.as_deref() != Some(revision)
        {
            let made = match &row.opened_binary {
                Some(binary) => format!("the binary of {binary}"),
                None => "a binary that recorded no revision".to_owned(),
            };
            bail!(
                "{}: run {run} was made with {made}, not with the binary of {revision}; a promotion judges a revision on runs of its own binary, read from each run's `opened` line, so nothing is judged",
                shown.display()
            );
        }
        if row.scenario.is_some() && row.held_out.is_none() {
            bail!(
                "{}: run {run} carries no held-out result; a scenario run's outcome comes from the held-out check, so nothing is judged",
                shown.display()
            );
        }
    }
    Ok(())
}

/// The commit at `HEAD` of `repo` carries no `rust-toolchain.toml` with a `path`: a promotion
/// builds with the toolchain takt resolves ([`Confinement::resolve`]), never one the commit
/// names, and a commit that names one is refused rather than built with another.
fn no_own_toolchain(repo: &Path) -> Result<()> {
    const FILE: &str = "rust-toolchain.toml";
    let listed = git(repo, &["ls-tree", "--name-only", "HEAD", "--", FILE])?;
    if listed.is_empty() {
        return Ok(());
    }
    let text = git(repo, &["show", &format!("HEAD:{FILE}")])?;
    let names_path = text.lines().any(|line| {
        let key = line.trim_start().trim_start_matches(['"', '\'']);
        key.strip_prefix("path").is_some_and(|rest| {
            rest.trim_start_matches(['"', '\''])
                .trim_start()
                .starts_with('=')
        })
    });
    if names_path {
        bail!(
            "{}: HEAD commits {FILE} with a toolchain `path`, naming a compiler outside the commit; a promotion builds with the toolchain takt resolves (rustup's default, from its settings file), never one the commit names, so the candidate is refused",
            repo.display()
        );
    }
    Ok(())
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

/// The binary at `installed` names `latest.revision`, the latest baseline or promotion in the
/// store: a candidate is compared with the revision that is installed, so a record removed from
/// the end of the store, or a binary replaced by hand, is a refusal naming both. A record whose
/// revision the operator stated (`revision_stated_by`) is checked by its `binary_sha256` alone,
/// whatever the binary prints, since a printed revision is what a replacement can copy: a
/// mismatch is refused naming both digests, and such a record without a digest is refused
/// naming it. Any other record is checked by the revision the binary names; a binary whose
/// `--version` names none is checked by its sha256 against the record's `binary_sha256` when
/// the record carries one, a mismatch refused naming both digests, and without one refused as
/// a binary naming no revision.
fn installed_names_latest(installed: &Path, store: &Path, latest: &Promotion) -> Result<()> {
    let path = record_path(store, latest);
    if !installed.exists() {
        bail!(
            "{}: no binary is installed, but the store records {} as the latest baseline or promotion ({}); a candidate is compared with the revision that is installed, so nothing is judged until they agree",
            installed.display(),
            latest.revision,
            path.display()
        );
    }
    let line = printed_line(installed)?;
    if latest.revision_stated_by.is_some() {
        let Some(recorded) = latest.binary_sha256.as_deref() else {
            bail!(
                "the latest baseline or promotion recorded, {} ({}), states its revision but names no binary digest to check {} by; nothing is judged until it does",
                latest.revision,
                path.display(),
                installed.display()
            );
        };
        let digest = sha256_of(installed)?;
        if digest != recorded {
            bail!(
                "{} prints {line:?} and hashes to {digest}, but the latest baseline or promotion recorded is {} ({}), its revision stated by the operator, whose binary hashes to {recorded}; a record is missing or the binary was replaced, and nothing is judged until they agree",
                installed.display(),
                latest.revision,
                path.display()
            );
        }
        return Ok(());
    }
    let printed = match (parse_version(&line), latest.binary_sha256.as_deref()) {
        (Some(printed), _) => printed,
        (None, Some(recorded)) => {
            let digest = sha256_of(installed)?;
            if digest != recorded {
                bail!(
                    "{} names no revision and hashes to {digest}, but the latest baseline or promotion recorded is {} ({}), whose binary hashes to {recorded}; a record is missing or the binary was replaced, and nothing is judged until they agree",
                    installed.display(),
                    latest.revision,
                    path.display()
                );
            }
            return Ok(());
        }
        (None, None) => bail!(
            "{} --version printed {line:?}, not `takt <version> (<revision>)`, and the latest baseline or promotion recorded, {} ({}), names no binary digest to check it by; nothing is judged until the binary names its revision",
            installed.display(),
            latest.revision,
            path.display()
        ),
    };
    if printed != latest.revision {
        bail!(
            "{} names {printed}, but the latest baseline or promotion recorded is {} ({}); a record is missing or the binary was replaced, and nothing is judged until they agree",
            installed.display(),
            latest.revision,
            path.display()
        );
    }
    Ok(())
}

/// The binary at `installed` names `revision`: `--baseline-only` records the scorecard of the
/// installed revision and no other.
fn installed_names_revision(installed: &Path, revision: &str) -> Result<()> {
    if !installed.exists() {
        bail!(
            "{}: no binary is installed; --baseline-only records the scorecard of the installed revision, so install {revision} there first",
            installed.display()
        );
    }
    let printed = printed_revision(installed)?;
    if printed != revision {
        bail!(
            "{} names {printed}, not {revision}; --baseline-only records the scorecard of the installed revision",
            installed.display()
        );
    }
    Ok(())
}

/// What the operator states with `--baseline-only` for a binary whose `--version` names no
/// revision: `--revision`, which must be the scorecard's `revision`, and `--binary-sha256`,
/// the digest returned. Both are refused without `--baseline-only`, the digest without the
/// revision, and either malformed. `--revision` alone states nothing the binary is not asked
/// to name itself, so none is returned and the binary is checked as without it.
fn stated_digest(request: &Request<'_>, revision: &str) -> Result<Option<String>> {
    let (stated, digest) = (request.revision, request.binary_sha256);
    if stated.is_none() && digest.is_none() {
        return Ok(None);
    }
    if !request.baseline_only {
        bail!(
            "--revision and --binary-sha256 are accepted with --baseline-only only: they record a baseline for a binary whose --version names no revision, and a candidate is named by its own binary"
        );
    }
    let Some(stated) = stated else {
        bail!(
            "--binary-sha256 needs --revision <sha>: the binary names no revision, so the operator states the one it was built from"
        );
    };
    if !is_full_revision(stated) {
        bail!("--revision {stated:?} is no full commit id (40 or 64 lower-case hex digits)");
    }
    if stated != revision {
        bail!(
            "--revision {stated} is not the scorecard's revision {revision}; a baseline records the scorecard of the revision it names"
        );
    }
    let Some(digest) = digest else {
        return Ok(None);
    };
    if !is_sha256(digest) {
        bail!("--binary-sha256 {digest:?} is no sha256 (64 lower-case hex digits)");
    }
    Ok(Some(digest.to_owned()))
}

/// The binary at `installed` names no revision and hashes to `stated`: a baseline the operator
/// states records the binary at that digest and no other, and a binary whose `--version` names
/// a revision is recorded by it, never by a statement.
fn installed_hashes_to(installed: &Path, revision: &str, stated: &str) -> Result<()> {
    if !installed.exists() {
        bail!(
            "{}: no binary is installed; --baseline-only records the scorecard of the installed binary, so install the one of {revision} there first",
            installed.display()
        );
    }
    let line = printed_line(installed)?;
    if let Some(printed) = parse_version(&line) {
        bail!(
            "{} names {printed}; a binary that names its revision is recorded by it, so --revision and --binary-sha256 are refused: drop them",
            installed.display()
        );
    }
    let digest = sha256_of(installed)?;
    if digest != stated {
        bail!(
            "{} hashes to {digest}, not to the stated {stated}; nothing is recorded until they agree",
            installed.display()
        );
    }
    Ok(())
}

/// Every run of `card` states the binary at `stated` (a [`BINARY_FILE`] beside its record), by
/// the operator: a run that states none, states it by anything but the operator, or states
/// another digest, is a refusal naming it, so the scorecard recorded is the stated binary's and
/// every statement the record copies is the operator's. A run whose `opened` line names a
/// revision must name `revision`; only a run that names none ([`names_no_revision`]: no
/// `binary`, or `unknown revision`) is taken on its statement alone, and a run that names none
/// beside one that names one ([`unnamed_in_a_mix`]) is refused, as a save refuses it. The runs
/// with their statements, for the record.
fn runs_stating(
    card: &Scorecard,
    stated: &str,
    revision: &str,
    shown: &Path,
) -> Result<Vec<RecordedRun>> {
    if let Some((run, _)) = unnamed_in_a_mix(&card.rows) {
        bail!(
            "{}: run {run} names no revision, beside runs that name one; a scorecard's runs all name {revision} or all name none, so nothing is recorded",
            shown.display()
        );
    }
    let mut runs = Vec::new();
    let mut counted: BTreeMap<&str, u64> = BTreeMap::new();
    for row in &card.rows {
        let n = counted.entry(row.case.as_str()).or_insert(0);
        *n += 1;
        let run = format!("{}#{n}", row.case);
        if let Some(named) = row.opened_binary.as_deref()
            && !names_no_revision(Some(named))
            && named != revision
        {
            bail!(
                "{}: run {run} was made with the binary of {named}, not of the stated revision {revision}; a run whose `opened` line names a revision is taken on it, so nothing is recorded",
                shown.display()
            );
        }
        match &row.binary {
            None => bail!(
                "{}: run {run} states no binary (no {BINARY_FILE} beside its record); with --binary-sha256 every run states the binary measured, so nothing is recorded",
                shown.display()
            ),
            Some(binary) if binary.stated_by != StatedBy::Operator => bail!(
                "{}: run {run} states its binary by the {}, not by the operator; with --binary-sha256 every run's binary is the operator's statement in a {BINARY_FILE} beside its record, so nothing is recorded",
                shown.display(),
                stated_by_name(binary.stated_by)
            ),
            Some(binary) if binary.sha256 != stated => bail!(
                "{}: run {run} states the binary {}, not the stated {stated}; the scorecard is not the stated binary's, so nothing is recorded",
                shown.display(),
                binary.sha256
            ),
            Some(binary) => runs.push(RecordedRun {
                case: row.case.clone(),
                binary: binary.clone(),
            }),
        }
    }
    Ok(runs)
}

/// Who stated a binary, as a refusal names it.
fn stated_by_name(by: StatedBy) -> &'static str {
    match by {
        StatedBy::Operator => "operator",
        StatedBy::Record => "record",
    }
}

/// The instant a record written now carries: the clock read again, after the build, which must
/// not lie before the latest record's instant; a clock set back writes nothing.
fn write_instant(clock: fn() -> String, existing: &[Promotion]) -> Result<String> {
    let now = clock();
    if let Some(latest) = existing.last()
        && now.as_str() < latest.promoted_at.as_str()
    {
        bail!(
            "the clock reads {now}, before the latest record's {} ({}); a record carries no instant before a record before it, so nothing is written",
            latest.promoted_at,
            latest.revision
        );
    }
    Ok(now)
}

/// Builds the candidate from a fresh clone of `repo` at `revision` in an empty directory under
/// `build_dir`, confined, checks the built binary names `revision`, installs it into a fresh
/// root beside `root`, checks that one, and hashes it: the staged binary, nothing yet in place.
fn stage(
    confinement: &Confinement,
    repo: &Path,
    revision: &str,
    root: &Path,
    build_dir: &Path,
    out: &mut dyn Write,
) -> Result<Staged> {
    let dir = build_dir.join(format!("{revision}-{}", std::process::id()));
    let install = root.join(format!(".takt-promote-{}", std::process::id()));
    for stale in [&dir, &install] {
        if stale.exists() {
            std::fs::remove_dir_all(stale).with_context(|| format!("clear {}", stale.display()))?;
        }
    }
    let work = Work {
        source: dir.join("source"),
        target: dir.join("target"),
        cargo_home: dir.join("cargo-home"),
        dir,
        install,
    };
    for made in [&work.target, &work.cargo_home, &work.install] {
        std::fs::create_dir_all(made).with_context(|| format!("create {}", made.display()))?;
    }
    let repo_text = repo
        .to_str()
        .ok_or_else(|| anyhow!("{} is not UTF-8", repo.display()))?;
    let clone_text = work
        .source
        .to_str()
        .ok_or_else(|| anyhow!("{} is not UTF-8", work.source.display()))?;
    writeln!(out, "cloning {revision} into {}", work.source.display())?;
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
    git(&work.source, &["checkout", "--quiet", "--detach", revision])
        .map_err(|error| BuildFailed(format!("check out {revision}: {error:#}")))?;
    confinement.cargo(
        &work,
        &[
            "build",
            "--locked",
            "--offline",
            "--release",
            "-p",
            "b10x-takt-cli",
            "--bin",
            "takt",
            "--target-dir",
            INSIDE_TARGET,
        ],
        out,
    )?;
    let built = work.target.join("release/takt");
    let printed = confinement
        .printed_revision(&built, &format!("{INSIDE_TARGET}/release"))
        .map_err(|error| BuildFailed(format!("{error:#}")))?;
    if printed != revision {
        return Err(BuildFailed(format!(
            "the built binary names {printed}, not {revision}: a promotion installs a clean build of the exact revision"
        ))
        .into());
    }
    confinement.cargo(
        &work,
        &[
            "install",
            "--locked",
            "--offline",
            "--force",
            "--path",
            &format!("{INSIDE_WORK}/crates/takt-cli"),
            "--root",
            INSIDE_INSTALL,
            "--target-dir",
            INSIDE_TARGET,
        ],
        out,
    )?;
    let binary = work.install.join("bin/takt");
    let printed = confinement
        .printed_revision(&binary, &format!("{INSIDE_INSTALL}/bin"))
        .map_err(|error| BuildFailed(format!("{error:#}")))?;
    if printed != revision {
        return Err(BuildFailed(format!(
            "the installed binary {} names {printed}, not {revision}",
            binary.display()
        ))
        .into());
    }
    let digest = sha256_of(&binary)?;
    Ok(Staged {
        work,
        binary,
        digest,
    })
}

/// Where `takt promote` keeps a binary it replaced, by its sha256, beside the store `store`:
/// `<state>/takt/judges/<sha256>`. An improvement turn that judged the promotion records its
/// judge's sha256 when it opens and finishes only under that binary
/// ([`crate::case::turn_judge_refusal`]), which the install has just replaced.
#[must_use]
pub fn kept_binary(store: &Path, sha256: &str) -> PathBuf {
    store.parent().unwrap_or(store).join("judges").join(sha256)
}

/// Copies the binary `before` to [`kept_binary`] under `store`, executable, written beside its
/// place and renamed into it; a regular file already kept there by that digest is kept as it
/// is, and anything else there (a link above all) is replaced. Its path.
fn keep_replaced(before: &Path, store: &Path) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    let digest = sha256_of(before)?;
    let kept = kept_binary(store, &digest);
    // Kept already only as a regular file of its own, never through a link: a link planted there
    // could be pointed elsewhere after this check. Anything else is replaced by the rename below.
    if std::fs::symlink_metadata(&kept).is_ok_and(|meta| meta.file_type().is_file())
        && sha256_of(&kept).is_ok_and(|at| at == digest)
    {
        return Ok(kept);
    }
    let dir = kept
        .parent()
        .ok_or_else(|| anyhow!("{} has no directory", kept.display()))?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let temporary = dir.join(format!(".{digest}.{}", std::process::id()));
    std::fs::copy(before, &temporary)
        .with_context(|| format!("copy {} to {}", before.display(), temporary.display()))?;
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("make {} executable", temporary.display()))?;
    std::fs::rename(&temporary, &kept)
        .with_context(|| format!("move {} to {}", temporary.display(), kept.display()))?;
    Ok(kept)
}

/// Moves the staged binary into `installed` in one rename, keeping the binary before it as a
/// hard link beside it until the one in place, run confined, names `revision` and hashes to the
/// staged digest; on any failure after the move the binary before it is back in place, or none,
/// as before. Once the binary is in place, the one it replaced is kept under the store `store`
/// by its digest ([`kept_binary`]): the answer is its path, or why it was not kept (which
/// undoes nothing), or none when nothing was installed before.
fn install(
    confinement: &Confinement,
    staged: &Staged,
    installed: &Path,
    revision: &str,
    store: &Path,
) -> Result<Option<std::result::Result<PathBuf, String>>> {
    let bin = installed
        .parent()
        .ok_or_else(|| anyhow!("{} has no directory", installed.display()))?;
    std::fs::create_dir_all(bin).with_context(|| format!("create {}", bin.display()))?;
    let before = bin.join(format!(".takt-before-{}", std::process::id()));
    let _ = std::fs::remove_file(&before);
    let had = match std::fs::hard_link(installed, &before) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("keep {} as {}", installed.display(), before.display()));
        }
    };
    let moved = std::fs::rename(&staged.binary, installed).with_context(|| {
        format!(
            "move {} into {}",
            staged.binary.display(),
            installed.display()
        )
    });
    let checked = moved.and_then(|()| {
        let printed = confinement
            .printed_revision(installed, INSIDE_INSTALLED)
            .map_err(|error| BuildFailed(format!("{error:#}")))?;
        if printed != revision {
            return Err(BuildFailed(format!(
                "{} names {printed} once in place, not {revision}",
                installed.display()
            ))
            .into());
        }
        let digest = sha256_of(installed)?;
        if digest != staged.digest {
            return Err(BuildFailed(format!(
                "{} hashes to {digest} once in place, not to the staged {}",
                installed.display(),
                staged.digest
            ))
            .into());
        }
        Ok(())
    });
    match checked {
        Ok(()) => {
            if !had {
                return Ok(None);
            }
            let kept = keep_replaced(&before, store).map_err(|error| format!("{error:#}"));
            let _ = std::fs::remove_file(&before);
            Ok(Some(kept))
        }
        Err(error) => {
            let restored = if had {
                std::fs::rename(&before, installed)
            } else {
                match std::fs::remove_file(installed) {
                    Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                    _ => Ok(()),
                }
            };
            Err(match restored {
                Ok(()) if had => with_note(
                    error,
                    &format!("the binary before it is back at {}", installed.display()),
                ),
                Ok(()) => with_note(error, "nothing is installed, as before"),
                Err(failure) => with_note(
                    error,
                    &format!("and putting the binary before it back failed: {failure}"),
                ),
            })
        }
    }
}

/// `takt promote`: see the module's documentation. Prints the comparison and the verdict on
/// `out`.
pub fn promote(request: &Request<'_>, out: &mut dyn Write) -> Result<Outcome> {
    let (card, text, revision, tree) = candidate(request)?;
    let stated = stated_digest(request, &revision)?;
    runs_judged(
        &card,
        stated.is_none().then_some(revision.as_str()),
        request.scorecard,
    )?;
    if request.rejudge && request.baseline_only {
        bail!(
            "--rejudge judges a refused candidate again and --baseline-only records a baseline, which is never refused; give one of them"
        );
    }
    let store = request.store;
    let installed = request.install_root.join("bin/takt");
    let confinement = if request.dry_run || request.baseline_only {
        None
    } else {
        Some(Confinement::resolve()?)
    };
    let _lock = if request.dry_run {
        None
    } else {
        Some(lock(store)?)
    };
    let now = (request.clock)();
    let (existing, head) = read(store, &now)?;
    trust(store, &existing, head.as_ref(), request.dry_run)?;
    let previous_sha256 = match existing.last() {
        Some(latest) => Some(sha256_of(&record_path(store, latest))?),
        None => None,
    };
    let mut record = Promotion {
        format: FORMAT.to_owned(),
        revision: revision.clone(),
        revision_stated_by: None,
        tree: tree.clone(),
        promoted_at: now.clone(),
        kind: Kind::Baseline,
        scorecard: revision.clone(),
        scorecard_file: resolved(request.scorecard),
        scorecard_sha256: format!("{:x}", Sha256::digest(&text)),
        against: None,
        rule: card.rule,
        worse: 0,
        binary: None,
        binary_sha256: None,
        runs: Vec::new(),
        previous_sha256,
        rejudges: None,
        replaces_sha256: None,
        cases: Vec::new(),
    };
    let mut refusals: Vec<&Promotion> = Vec::new();
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
        if earlier.kind == Kind::Refused && request.rejudge {
            refusals.push(earlier);
            continue;
        }
        let kind = match earlier.kind {
            Kind::Refused => {
                "refused; a refused candidate is not judged again until it changes, or on a scorecard of new runs with --rejudge"
            }
            Kind::Baseline | Kind::Rebaseline => "as a baseline",
            Kind::Promotion => "as a promotion",
        };
        bail!(
            "{same}: {kind}, at {} ({})",
            earlier.promoted_at,
            record_path(store, earlier).display()
        );
    }
    if request.rejudge {
        let Some(latest) = refusals.last() else {
            bail!(
                "--rejudge: {revision} was never refused, nor its tree; there is nothing to judge again"
            );
        };
        let cases = case_ids(&card);
        for earlier in &refusals {
            if earlier.cases.is_empty() {
                bail!(
                    "--rejudge judges a refused candidate on new runs, and the refusal at {} ({}) records no case ids, so its runs cannot be told from new ones",
                    earlier.promoted_at,
                    record_path(store, earlier).display()
                );
            }
            if let Some(case) = earlier.cases.iter().find(|case| cases.contains(*case)) {
                bail!(
                    "--rejudge judges a refused candidate on new runs, and case {case} of {} is in the scorecard refused at {} ({}); run the candidate again on new cases and save their scorecard",
                    request.scorecard.display(),
                    earlier.promoted_at,
                    record_path(store, earlier).display()
                );
            }
        }
        let path = record_path(store, latest);
        record.rejudges = Some(Rejudged {
            record: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            sha256: sha256_of(&path)?,
        });
        writeln!(
            out,
            "judged again: refused at {} ({})",
            latest.promoted_at,
            path.display()
        )?;
    }
    if request.baseline_only {
        if let Some(base) = existing.iter().rfind(|record| record.kind != Kind::Refused) {
            bail!(
                "the store {} holds a baseline already: {} at {} ({})",
                store.display(),
                base.revision,
                base.promoted_at,
                record_path(store, base).display()
            );
        }
        match &stated {
            Some(digest) => {
                installed_hashes_to(&installed, &revision, digest)?;
                record.runs = runs_stating(&card, digest, &revision, request.scorecard)?;
                record.binary = Some(installed.clone());
                record.binary_sha256 = Some(digest.clone());
                record.revision_stated_by = Some(StatedBy::Operator);
            }
            None => installed_names_revision(&installed, &revision)?,
        }
        enough_trials(&card, &card, request.scorecard)?;
        if card.rows.iter().all(|row| row.scenario.is_none()) {
            bail!(
                "{}: no scenario case, so a baseline of it would judge nothing; a comparison covers every scenario either side has, and a baseline needs at least one",
                request.scorecard.display()
            );
        }
        if let Some(digest) = &stated {
            writeln!(
                out,
                "baseline {revision}, as the operator states it: {} names no revision and hashes to {digest}, which every run of the scorecard states; nothing compared, nothing installed",
                installed.display()
            )?;
        } else {
            writeln!(
                out,
                "baseline {revision}: the scorecard of the installed revision, nothing compared, nothing installed"
            )?;
        }
        if request.dry_run {
            writeln!(
                out,
                "dry run: would write {} and {}",
                record_path(store, &record).display(),
                copy_path(store, &record).display()
            )?;
            writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
            return Ok(Outcome::WouldWrite(record));
        }
        record.promoted_at = write_instant(request.clock, &existing)?;
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
    installed_names_latest(&installed, store, &base)?;
    let base_copy = copy_path(store, &base);
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
        record.cases = case_ids(&card).into_iter().collect();
        writeln!(
            out,
            "verdict: refused, {worse} scenario{} worse than the baseline; nothing installed",
            if worse == 1 { "" } else { "s" }
        )?;
        if request.dry_run {
            writeln!(
                out,
                "dry run: would write {}",
                record_path(store, &record).display()
            )?;
            writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
            return Ok(Outcome::WouldRefuse(record));
        }
        record.promoted_at = write_instant(request.clock, &existing)?;
        let path = write(store, &record, &text)?;
        writeln!(out, "recorded: {}", path.display())?;
        return Ok(Outcome::Regression(record));
    }
    record.kind = Kind::Promotion;
    writeln!(out, "verdict: no scenario worse than the baseline")?;
    if request.dry_run {
        writeln!(
            out,
            "dry run: would clone {} at {revision} under {}, build it there confined in {BWRAP}, install under {} and write {} and {}",
            request.repo.display(),
            request.build_dir.display(),
            request.install_root.display(),
            record_path(store, &record).display(),
            copy_path(store, &record).display()
        )?;
        writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
        return Ok(Outcome::WouldWrite(record));
    }
    let confinement = confinement
        .as_ref()
        .ok_or_else(|| anyhow!("the confinement is resolved before a build"))?;
    let staged = stage(
        confinement,
        request.repo,
        &revision,
        request.install_root,
        request.build_dir,
        out,
    )?;
    record.promoted_at = write_instant(request.clock, &existing)?;
    record.binary = Some(installed.clone());
    record.binary_sha256 = Some(staged.digest.clone());
    let path = write(store, &record, &text)?;
    let kept = match install(confinement, &staged, &installed, &revision, store) {
        Ok(kept) => kept,
        Err(error) => {
            return Err(match unwrite(store, &record, existing.last()) {
                Ok(()) => with_note(error, "nothing recorded"),
                Err(removal) => with_note(
                    error,
                    &format!("and removing the record failed: {removal:#}"),
                ),
            });
        }
    };
    writeln!(
        out,
        "promoted: {} at {revision}, sha256 {}; recorded: {}",
        installed.display(),
        staged.digest,
        path.display()
    )?;
    match kept {
        Some(Ok(kept)) => writeln!(
            out,
            "kept: the binary it replaced at {}, for a turn that judged this promotion to finish under",
            kept.display()
        )?,
        Some(Err(why)) => writeln!(out, "not kept: the binary it replaced ({why})")?,
        None => {}
    }
    Ok(Outcome::Promoted(record))
}

/// The case ids of `card`'s runs.
fn case_ids(card: &Scorecard) -> BTreeSet<String> {
    card.rows.iter().map(|row| row.case.clone()).collect()
}

/// `takt promote --rebaseline <scorecard>`: replaces the scorecard of the baseline, the latest
/// record that is no refusal, with `scorecard`, which a promotion then compares candidates
/// with. Accepted only when the scorecard's revision is the baseline's, its case ids are the
/// stored scorecard's exactly but for the cases of whole scenarios it adds ([`added_scenarios`]:
/// a new scenario joins the corpus only here), and every metric definition is at or above the
/// stored one's version with one of them higher or a scenario added; its runs are checked as a
/// baseline's are (made with the baseline revision's binary, or stating the baseline's digest
/// when its revision is stated, each scenario run with its held-out result) and the installed
/// binary must be the baseline's. The record appended to the chain carries the baseline's
/// revision, tree and binary, and names the scorecard it replaces by its sha256
/// (`replaces_sha256`); nothing is built or installed. Anything else is refused naming the
/// reason, nothing written.
pub fn rebaseline(request: &Rebaseline<'_>, out: &mut dyn Write) -> Result<Outcome> {
    let shown = request.scorecard;
    let metadata =
        std::fs::metadata(shown).with_context(|| format!("--rebaseline {}", shown.display()))?;
    if !metadata.is_file() {
        bail!("--rebaseline {}: not a regular file", shown.display());
    }
    if metadata.nlink() > 1 {
        bail!(
            "--rebaseline {}: the file has {} links; a scorecard that may also lie elsewhere under another name is not read",
            shown.display(),
            metadata.nlink()
        );
    }
    let text = std::fs::read(shown).with_context(|| format!("read {}", shown.display()))?;
    let card = Scorecard::load(shown)?;
    if card.rule != Rule::default() {
        bail!(
            "{}: its rule is trials {} margin {}; this takt judges under trials {} margin {}",
            shown.display(),
            card.rule.trials,
            card.rule.margin,
            Rule::default().trials,
            Rule::default().margin
        );
    }
    known_metrics(&card, shown)?;
    let Some(revision) = card.revision.clone() else {
        bail!(
            "{}: the scorecard names no revision; save it with `takt scorecard --save --revision`",
            shown.display()
        );
    };
    let named = shown
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("scorecard-"))
        .and_then(|name| name.strip_suffix(".json"));
    if named != Some(revision.as_str()) {
        bail!(
            "{}: the file is not named by its revision {revision} (`scorecard-{revision}.json`)",
            shown.display()
        );
    }
    let store = request.store;
    let _lock = if request.dry_run {
        None
    } else {
        Some(lock(store)?)
    };
    let now = (request.clock)();
    let (existing, head) = read(store, &now)?;
    trust(store, &existing, head.as_ref(), request.dry_run)?;
    let Some(base) = existing
        .iter()
        .rfind(|record| record.kind != Kind::Refused)
        .cloned()
    else {
        bail!(
            "the store {} holds no baseline, so there is nothing to rebaseline; record one with `takt promote --baseline-only`",
            store.display()
        );
    };
    if revision != base.revision {
        bail!(
            "{}: its revision {revision} is not the baseline's {} ({}); --rebaseline replaces the scorecard of the baseline revision and no other",
            shown.display(),
            base.revision,
            record_path(store, &base).display()
        );
    }
    let base_copy = copy_path(store, &base);
    let stored = Scorecard::load(&base_copy)?;
    let added = added_scenarios(&card, &stored).map_err(|why| {
        anyhow!(
            "{}: {why} ({}); --rebaseline re-scores the baseline's cases exactly and adds a whole scenario the baseline lacks, nothing else",
            shown.display(),
            base_copy.display()
        )
    })?;
    let mut newer = false;
    for (metric, stored_version) in &stored.metrics {
        match card.metrics.get(metric) {
            Some(version) if version < stored_version => bail!(
                "{}: metric {metric} is at definition version {version}, below the stored {stored_version} ({}); --rebaseline takes every definition at or above the stored one",
                shown.display(),
                base_copy.display()
            ),
            Some(version) if version > stored_version => newer = true,
            Some(_) => {}
            None => bail!(
                "{}: metric {metric} has no definition version, and the stored scorecard ({}) has version {stored_version}",
                shown.display(),
                base_copy.display()
            ),
        }
    }
    newer |= card
        .metrics
        .keys()
        .any(|metric| !stored.metrics.contains_key(metric));
    if !newer && added.is_empty() {
        bail!(
            "{}: no metric definition is newer than the stored scorecard's ({}), and no scenario is added; --rebaseline replaces a baseline's scorecard only under a newer definition or to add a whole scenario",
            shown.display(),
            base_copy.display()
        );
    }
    runs_bound(&card, &stored, &added, &base, shown, &base_copy)?;
    let runs = match (base.revision_stated_by, &base.binary_sha256) {
        (Some(_), Some(digest)) => {
            runs_judged(&card, None, shown)?;
            runs_stating(&card, digest, &revision, shown)?
        }
        _ => {
            runs_judged(&card, Some(&revision), shown)?;
            Vec::new()
        }
    };
    enough_trials(&card, &card, shown)?;
    installed_names_latest(&request.install_root.join("bin/takt"), store, &base)?;
    let previous_sha256 = match existing.last() {
        Some(latest) => Some(sha256_of(&record_path(store, latest))?),
        None => None,
    };
    let mut record = Promotion {
        format: FORMAT.to_owned(),
        revision: revision.clone(),
        revision_stated_by: base.revision_stated_by,
        tree: base.tree.clone(),
        promoted_at: now,
        kind: Kind::Rebaseline,
        scorecard: revision.clone(),
        scorecard_file: resolved(shown),
        scorecard_sha256: format!("{:x}", Sha256::digest(&text)),
        against: None,
        rule: card.rule,
        worse: 0,
        binary: base.binary.clone(),
        binary_sha256: base.binary_sha256.clone(),
        runs,
        previous_sha256,
        rejudges: None,
        replaces_sha256: Some(base.scorecard_sha256.clone()),
        cases: Vec::new(),
    };
    let mut why: Vec<String> = Vec::new();
    if newer {
        why.push("under newer metric definitions".to_owned());
    }
    if !added.is_empty() {
        why.push(format!(
            "and adds {}",
            added
                .iter()
                .map(|scenario| format!("scenario {scenario}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    writeln!(
        out,
        "rebaseline {revision}: {} replaces the scorecard {} ({}) {}; nothing compared, nothing installed",
        shown.display(),
        base.scorecard_sha256,
        base_copy.display(),
        why.join(" ")
    )?;
    if request.dry_run {
        writeln!(
            out,
            "dry run: would write {} and {}",
            record_path(store, &record).display(),
            copy_path(store, &record).display()
        )?;
        writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
        return Ok(Outcome::WouldWrite(record));
    }
    record.promoted_at = write_instant(request.clock, &existing)?;
    let path = write(store, &record, &text)?;
    writeln!(out, "recorded: {}", path.display())?;
    Ok(Outcome::Rebaselined(record))
}

/// Every scenario the baseline has holds at least the rule's trials on the baseline's own side,
/// and every scenario either side has holds at least the rule's trials on the candidate's side,
/// so a candidate never becomes a baseline with a scenario that judges nothing; fewer is a
/// refusal, not a verdict.
fn enough_trials(card: &Scorecard, base_card: &Scorecard, shown: &Path) -> Result<()> {
    let trials = usize::try_from(card.rule.trials).unwrap_or(usize::MAX);
    let count = |card: &Scorecard, scenario: &str| {
        card.cases()
            .iter()
            .filter(|case| case.scenario.as_deref() == Some(scenario))
            .count()
    };
    let of = |card: &Scorecard| -> BTreeSet<String> {
        card.cases()
            .iter()
            .filter_map(|case| case.scenario.clone())
            .collect()
    };
    let (base_scenarios, scenarios) = (of(base_card), of(card));
    for scenario in base_scenarios.union(&scenarios) {
        let n = count(base_card, scenario);
        if base_scenarios.contains(scenario) && n < trials {
            bail!(
                "scenario {scenario}: the baseline has {n} case{}, the rule needs {trials}; a baseline with fewer judges nothing",
                if n == 1 { "" } else { "s" }
            );
        }
        let n = count(card, scenario);
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
