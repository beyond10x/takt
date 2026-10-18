//! Promotion: the takt binary installed for runs is built from a revision whose saved scorecard
//! shows no regression against the promoted baseline (design:recursive-self-improvement § The
//! judge, § Scorecard). A promotion is a record, one JSON file per revision under the store
//! `$XDG_STATE_HOME/takt/promotions/` (`~/.local/state/takt/promotions/` without
//! `XDG_STATE_HOME`), outside every repository a candidate changes; the latest record by
//! `promoted_at` names the baseline the next candidate is compared with. The first record is a
//! `baseline` one, written by `takt promote --baseline-only`, which installs nothing.
//!
//! `takt promote` reads the candidate's saved scorecard (a file outside the candidate's
//! repository whose `revision` field equals its name and the checkout's `HEAD`, a full commit
//! id), compares it with the baseline's under the candidate scorecard's [`Rule`], and refuses
//! a candidate that is worse, writing a `refused` record so the same revision is not judged
//! again until it changes; a candidate that holds is built in its clean checkout, its binary's
//! `--version` checked to name the revision (no `-dirty`, no `unknown revision`), installed with
//! `cargo install --locked --path crates/takt-cli`, and recorded with the installed file's
//! sha256. One promotion a UTC day. `--dry-run` prints the verdict and the record it would write
//! and writes nothing.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::scorecard::{Rule, Scorecard};

/// The promotion record's format.
pub const FORMAT: &str = "takt-promotion/1";

/// The store's path below the state directory.
pub const STORE: &str = "takt/promotions";

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
/// refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promotion {
    /// [`FORMAT`].
    pub format: String,
    /// The revision the binary was built from, a full commit id; the candidate's for a refusal.
    pub revision: String,
    /// When the record was written, `YYYY-MM-DDTHH:MM:SSZ`; the one-a-UTC-day rule reads it.
    pub promoted_at: String,
    /// What the record is.
    pub kind: Kind,
    /// The saved scorecard read, by its id: the revision that names it, equal to `revision`.
    pub scorecard: String,
    /// The saved scorecard's file, absolute.
    pub scorecard_file: PathBuf,
    /// The baseline revision the scorecard was compared with; none on a baseline record.
    pub against: Option<String>,
    /// The rule the comparison applied.
    pub rule: Rule,
    /// The scenarios the comparison marked worse; 0 on a baseline or a promotion.
    pub worse: u32,
    /// The installed binary; none unless the record is a promotion.
    pub binary: Option<PathBuf>,
    /// The installed binary's sha256, lower-case hex; none unless the record is a promotion.
    pub binary_sha256: Option<String>,
}

/// The build or the install of a candidate that held failed: the CLI tells this apart from a
/// refusal.
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
    let state = match std::env::var_os("XDG_STATE_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) if Path::new(&dir).is_absolute() => PathBuf::from(dir),
        _ => {
            let home = std::env::var_os("HOME").filter(|home| !home.is_empty());
            let Some(home) = home else {
                bail!("neither XDG_STATE_HOME nor HOME is set: the promotion store has no place");
            };
            PathBuf::from(home).join(".local/state")
        }
    };
    Ok(state.join(STORE))
}

/// Where `cargo install` puts the binary when `--root` is not given: `CARGO_INSTALL_ROOT`,
/// else `CARGO_HOME`, else `~/.cargo`; promote passes it as `--root` so both agree.
pub fn install_root() -> Result<PathBuf> {
    for name in ["CARGO_INSTALL_ROOT", "CARGO_HOME"] {
        if let Some(dir) = std::env::var_os(name).filter(|dir| !dir.is_empty()) {
            return Ok(PathBuf::from(dir));
        }
    }
    let home = std::env::var_os("HOME").filter(|home| !home.is_empty());
    let Some(home) = home else {
        bail!("none of CARGO_INSTALL_ROOT, CARGO_HOME and HOME is set: nowhere to install");
    };
    Ok(PathBuf::from(home).join(".cargo"))
}

/// Every record in `store`, oldest first by `promoted_at`, then by revision; none when the
/// store does not exist. A file that is no record is an error naming it.
pub fn records(store: &Path) -> Result<Vec<Promotion>> {
    let mut records = Vec::new();
    let entries = match std::fs::read_dir(store) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(records),
        Err(error) => return Err(error).with_context(|| format!("read {}", store.display())),
    };
    for entry in entries {
        let path = entry
            .with_context(|| format!("read {}", store.display()))?
            .path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            records.push(load(&path)?);
        }
    }
    records.sort_by(|a, b| {
        a.promoted_at
            .cmp(&b.promoted_at)
            .then_with(|| a.revision.cmp(&b.revision))
    });
    Ok(records)
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
pub fn baseline(store: &Path) -> Result<Option<Promotion>> {
    Ok(records(store)?
        .into_iter()
        .rfind(|record| record.kind != Kind::Refused))
}

/// Whether `candidate` is a full commit id, 40 or 64 lower-case hex digits.
fn is_full_revision(candidate: &str) -> bool {
    matches!(candidate.len(), 40 | 64)
        && candidate
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether `path` lies inside `repo`, both resolved.
fn inside(repo: &Path, path: &Path) -> bool {
    let repo = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    path.starts_with(&repo)
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

/// The record's file in `store`.
fn record_path(store: &Path, revision: &str, kind: Kind) -> PathBuf {
    match kind {
        Kind::Refused => store.join(format!("{revision}.refused.json")),
        Kind::Baseline | Kind::Promotion => store.join(format!("{revision}.json")),
    }
}

/// Writes `record` to the store; a file there already is not overwritten.
fn write(store: &Path, record: &Promotion) -> Result<PathBuf> {
    std::fs::create_dir_all(store).with_context(|| format!("create {}", store.display()))?;
    let path = record_path(store, &record.revision, record.kind);
    let mut text = serde_json::to_string_pretty(record).context("write the record")?;
    text.push('\n');
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!(
                "{} exists: a promotion record is not overwritten",
                path.display()
            );
        }
        Err(error) => return Err(error).with_context(|| format!("write {}", path.display())),
    };
    file.write_all(text.as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// Runs `cargo <args>` in `repo`, the output shown on `out`; a failure is a [`BuildFailed`].
fn cargo(repo: &Path, args: &[&str], out: &mut dyn Write) -> Result<()> {
    writeln!(out, "cargo {}", args.join(" "))?;
    let output = Command::new("cargo")
        .args(args)
        .current_dir(repo)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET_DIR")
        .output()
        .with_context(|| format!("run cargo {} in {}", args.join(" "), repo.display()))?;
    if !output.status.success() {
        return Err(BuildFailed(format!(
            "cargo {} in {} exited {}: {}",
            args.join(" "),
            repo.display(),
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

/// Checks the candidate scorecard and the checkout agree: the file lies outside the repository,
/// its `revision` is a full commit id equal to its file name and to `HEAD`, and no tracked file
/// differs from `HEAD`. The scorecard and the revision.
fn candidate(request: &Request<'_>) -> Result<(Scorecard, String)> {
    let Request {
        repo, scorecard, ..
    } = request;
    if inside(repo, scorecard) {
        bail!(
            "--scorecard {}: inside the repository {}; a candidate's scorecard is read from a place the candidate does not write",
            scorecard.display(),
            repo.display()
        );
    }
    if inside(repo, request.store) {
        bail!(
            "the promotion store {} is inside the repository {}; set XDG_STATE_HOME outside it",
            request.store.display(),
            repo.display()
        );
    }
    let card = Scorecard::load(scorecard)?;
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
    let dirty = git(repo, &["status", "--porcelain", "--untracked-files=no"])?;
    if !dirty.is_empty() {
        bail!(
            "{} has uncommitted changes to tracked files; a promotion builds a clean checkout:\n{dirty}",
            repo.display()
        );
    }
    Ok((card, revision))
}

/// Builds the candidate in its checkout, checks the built binary names `revision`, installs
/// it under `root` and checks the installed one again. The installed binary's path and sha256.
fn install(
    repo: &Path,
    revision: &str,
    root: &Path,
    out: &mut dyn Write,
) -> Result<(PathBuf, String)> {
    cargo(
        repo,
        &[
            "build",
            "--locked",
            "--release",
            "-p",
            "b10x-takt-cli",
            "--bin",
            "takt",
        ],
        out,
    )?;
    let built = repo.join("target/release/takt");
    let printed = printed_revision(&built)?;
    if printed != revision {
        bail!(
            "the built binary names {printed}, not {revision}: a promotion installs a clean build of the exact revision"
        );
    }
    let root_text = root
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{} is not UTF-8", root.display()))?;
    let path_text = repo
        .join("crates/takt-cli")
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{} is not UTF-8", repo.display()))?
        .to_owned();
    let target_text = repo
        .join("target")
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{} is not UTF-8", repo.display()))?
        .to_owned();
    cargo(
        repo,
        &[
            "install",
            "--locked",
            "--force",
            "--path",
            &path_text,
            "--root",
            root_text,
            "--target-dir",
            &target_text,
        ],
        out,
    )?;
    let installed = root.join("bin/takt");
    let printed = printed_revision(&installed)?;
    if printed != revision {
        bail!(
            "the installed binary {} names {printed}, not {revision}",
            installed.display()
        );
    }
    let digest = sha256_of(&installed)?;
    Ok((installed, digest))
}

/// `takt promote`: see the module's documentation. Prints the comparison and the verdict on
/// `out`.
pub fn promote(request: &Request<'_>, out: &mut dyn Write) -> Result<Outcome> {
    let (card, revision) = candidate(request)?;
    let store = request.store;
    let mut record = Promotion {
        format: FORMAT.to_owned(),
        revision: revision.clone(),
        promoted_at: request.now.clone(),
        kind: Kind::Baseline,
        scorecard: revision.clone(),
        scorecard_file: request
            .scorecard
            .canonicalize()
            .unwrap_or_else(|_| request.scorecard.to_path_buf()),
        against: None,
        rule: card.rule,
        worse: 0,
        binary: None,
        binary_sha256: None,
    };
    if request.baseline_only {
        if let Some(existing) = baseline(store)? {
            bail!(
                "the store {} holds a baseline already: {} at {} ({})",
                store.display(),
                existing.revision,
                existing.promoted_at,
                record_path(store, &existing.revision, existing.kind).display()
            );
        }
        writeln!(
            out,
            "baseline {revision}: the scorecard of the installed revision, nothing compared, nothing installed"
        )?;
        if request.dry_run {
            writeln!(
                out,
                "dry run: would write {}",
                record_path(store, &revision, Kind::Baseline).display()
            )?;
            writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
            return Ok(Outcome::WouldWrite(record));
        }
        let path = write(store, &record)?;
        writeln!(out, "recorded: {}", path.display())?;
        return Ok(Outcome::BaselineRecorded(record));
    }
    let Some(base) = baseline(store)? else {
        bail!(
            "the store {} holds no baseline; record the installed revision's scorecard first with `takt promote --baseline-only`",
            store.display()
        );
    };
    for existing in records(store)? {
        if existing.revision == revision {
            match existing.kind {
                Kind::Refused => bail!(
                    "{revision} was refused at {} ({}); a refused candidate is not judged again until it changes",
                    existing.promoted_at,
                    record_path(store, &revision, Kind::Refused).display()
                ),
                Kind::Baseline | Kind::Promotion => bail!(
                    "{revision} is recorded already, as a {} at {} ({})",
                    match existing.kind {
                        Kind::Baseline => "baseline",
                        _ => "promotion",
                    },
                    existing.promoted_at,
                    record_path(store, &revision, existing.kind).display()
                ),
            }
        }
    }
    let today = request.now.get(..10).unwrap_or("");
    if let Some(earlier) = records(store)?.iter().find(|record| {
        record.kind == Kind::Promotion && record.promoted_at.get(..10) == Some(today)
    }) {
        bail!(
            "a promotion was made this UTC day already: {} at {} ({}); one promotion a day",
            earlier.revision,
            earlier.promoted_at,
            record_path(store, &earlier.revision, earlier.kind).display()
        );
    }
    let base_card = Scorecard::load(&base.scorecard_file)?;
    if base_card.revision.as_deref() != Some(base.revision.as_str()) {
        bail!(
            "the baseline's scorecard {} names {}, not the baseline revision {}",
            base.scorecard_file.display(),
            base_card.revision.as_deref().unwrap_or("no revision"),
            base.revision
        );
    }
    let trials = u64::from(card.rule.trials);
    for scenario in base_card
        .cases()
        .iter()
        .filter_map(|case| case.scenario.clone())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let n = card
            .cases()
            .iter()
            .filter(|case| case.scenario.as_deref() == Some(scenario.as_str()))
            .count() as u64;
        if n < trials {
            bail!(
                "scenario {scenario}: the candidate has {n} case{}, the rule needs {trials}; run it {} more time{} before judging",
                if n == 1 { "" } else { "s" },
                trials - n,
                if trials - n == 1 { "" } else { "s" }
            );
        }
    }
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
        let path = write(store, &record)?;
        writeln!(out, "recorded: {}", path.display())?;
        return Ok(Outcome::Regression(record));
    }
    record.kind = Kind::Promotion;
    writeln!(out, "verdict: no scenario worse than the baseline")?;
    if request.dry_run {
        writeln!(
            out,
            "dry run: would build {} at {revision}, install under {} and write {}",
            request.repo.display(),
            request.install_root.display(),
            record_path(store, &revision, Kind::Promotion).display()
        )?;
        writeln!(out, "{}", serde_json::to_string_pretty(&record)?)?;
        return Ok(Outcome::WouldWrite(record));
    }
    let (binary, digest) = install(request.repo, &revision, request.install_root, out)?;
    record.binary = Some(binary.clone());
    record.binary_sha256 = Some(digest.clone());
    let path = write(store, &record)?;
    writeln!(
        out,
        "promoted: {} at {revision}, sha256 {digest}; recorded: {}",
        binary.display(),
        path.display()
    )?;
    Ok(Outcome::Promoted(record))
}
