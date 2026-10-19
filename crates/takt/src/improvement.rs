//! The validators of an improvement turn, a `self.improvement/1` case
//! (`protocols/self-improvement/1.yaml`, design:recursive-self-improvement § The loops as
//! protocols): [`FindingFile`] (`finding.file`), [`ScenarioReproduce`] (`scenario.reproduce`),
//! [`ScenarioVerify`] (`scenario.verify`) and [`ScorecardCompare`] (`scorecard.compare`). A turn
//! opens with its finding, its candidate and the promoted baseline
//! ([`crate::case::CaseDir::open_turn`]), which declares each of them independent of the run
//! that delivered the candidate ([`PRODUCERS`]): a validator is no agent run.
//!
//! These validators run in the takt that drives the case, the installed promoted binary, which
//! judges; the candidate's binary runs only inside `scripts/check-finding`'s run step. The
//! scripts they run (`scripts/check-finding`, and `scripts/file-finding` it calls) are read at the
//! turn's baseline, the promoted revision, checked against their ids, and run from a directory
//! of their own, never from the tree a step edits nor from the commit the case forks from, which
//! the operator's `HEAD` decides; a candidate that changes them, or the finding, does not open a
//! turn ([`crate::case::turn_opening`]). The candidate is delivered outside the case and given at
//! open.
//!
//! Not here yet: the merge validators (`candidate.merge`, `candidate.merge_judge_surface`), the
//! judge-surface input computed at open, and `binary.promote`; until they are, no turn reaches
//! `accepted` on validators' records.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::evidence::Record;
use crate::forge::Limits;
use crate::promotion::Kind;
use crate::scorecard::{Rule, Scorecard};
use crate::validator::{Artifact, Observation, Validator};

/// The script that runs a finding's predicate with a takt built from a revision, read at the
/// turn's baseline, the promoted revision.
pub const CHECK_FINDING: &str = "scripts/check-finding";

/// The script that files a finding and parses its predicate (`--predicate`), read at the turn's
/// baseline; `scripts/check-finding` calls it from its own directory.
pub const FILE_FINDING: &str = "scripts/file-finding";

/// Where a candidate's saved scorecard is read, under the state directory
/// (`$XDG_STATE_HOME`, `~/.local/state` without it): `scorecard-<candidate>.json`, written
/// beforehand with `takt scorecard --save`.
pub const SCORECARDS: &str = "takt/scorecards";

/// The sha256 of every `scripts/check-finding` reviewed as confined (story:check-finding-run-
/// step-confined): its run step has no network, no host socket, its own IPC and PID namespaces,
/// the run directory read-only and the homes hidden. A turn opens only on a promoted baseline
/// whose script hashes to one of them, since the baseline's script is the one the turn runs; the
/// confinement lives in that script alone, so no commit's ancestry stands in for it. A change to
/// the script adds its digest here once it is reviewed (`tests/self_improvement.rs` holds this
/// tree's script to the list).
pub const CONFINED_CHECK_FINDING_SHA256S: [&str; 2] = [
    // f2a20e9, the last commit of story:check-finding-run-step-confined: descriptors above 2
    // closed before bubblewrap, over the confinement of 7c8d831 and 1c905c7.
    "d87db09ee62e424e721bba54199927411ee523397c7b09c6bcc18080bc7def8e",
    // That script with its build's checkout fetched through git's transport, every object hashed
    // and checked (`transfer.fsckObjects`), instead of a local clone (story:self-improvement-
    // protocol, part 1): the run step's confinement unchanged.
    "7fd6e3195dbf11d5ae4bc6f6264751f6ede047241d7584a9154829fedbcbeeb7",
];

/// The file under a turn's `artifacts/` holding the sha256 of the takt `scripts/check-finding`
/// built for `revision` during the turn, as [`check_finding`] read it after the build.
#[must_use]
pub fn build_record(revision: &str) -> String {
    format!("takt-build-{revision}.sha256")
}

/// The independence dimension the kernel declares every turn validator on: a validator is no
/// agent run, so it is another than the one that delivered the candidate.
pub const INDEPENDENT_ON: &str = "different_agent_run";

/// The producers of a turn's records, each declared independent on [`INDEPENDENT_ON`] when the
/// turn opens.
pub const PRODUCERS: [&str; 4] = [
    FindingFile::NAME,
    ScenarioReproduce::NAME,
    ScenarioVerify::NAME,
    ScorecardCompare::NAME,
];

/// What the turn validators that run the finding scripts are given: the cache directory
/// (`$XDG_CACHE_HOME`, `~/.cache` without it) handed to `scripts/check-finding`, which keeps its
/// builds and runs under `takt/check-finding` there, and under which the scripts read at the baseline
/// are put while they run; the checkout of the scenario repository a predicate naming a
/// `scenario` runs on (`--scenarios`); and the limits the script is held to.
#[derive(Debug, Clone)]
pub struct Checking {
    /// The cache directory; none, and it is the one `scripts/check-finding` itself takes, read
    /// from the environment when a turn's validator observes ([`Checking::cache`]), so no other
    /// case needs one.
    pub cache: Option<PathBuf>,
    /// The scenario repository's checkout; none, and a predicate that names a scenario is not
    /// run.
    pub scenarios: Option<PathBuf>,
    /// What the script is held to, besides the run's hour budget.
    pub limits: Limits,
}

impl Checking {
    /// The limits by default: an hour (a build of the revision and the predicate's own 900 s),
    /// 64 KiB of output.
    pub const LIMITS: Limits = Limits {
        timeout: std::time::Duration::from_secs(3600),
        output: 64 << 10,
    };

    /// With the cache directory `cache`, no scenario checkout and [`Self::LIMITS`].
    #[must_use]
    pub fn new(cache: PathBuf) -> Self {
        Self {
            cache: Some(cache),
            scenarios: None,
            limits: Self::LIMITS,
        }
    }

    /// With the cache directory `scripts/check-finding` itself takes, read when a validator
    /// observes.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            cache: None,
            scenarios: None,
            limits: Self::LIMITS,
        }
    }

    /// The cache directory: the one given, else `$XDG_CACHE_HOME` when it is absolute, else
    /// `$HOME/.cache`, as `scripts/check-finding` takes it.
    pub fn cache(&self) -> Result<PathBuf> {
        if let Some(cache) = &self.cache {
            return Ok(cache.clone());
        }
        match std::env::var_os("XDG_CACHE_HOME").filter(|dir| !dir.is_empty()) {
            Some(dir) if Path::new(&dir).is_absolute() => Ok(PathBuf::from(dir)),
            _ => std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(|home| PathBuf::from(home).join(".cache"))
                .ok_or_else(|| {
                    anyhow!(
                        "neither XDG_CACHE_HOME nor HOME is set, so there is no cache directory"
                    )
                }),
        }
    }

    /// With the scenario repository's checkout `scenarios`.
    #[must_use]
    pub fn with_scenarios(mut self, scenarios: Option<PathBuf>) -> Self {
        self.scenarios = scenarios;
        self
    }
}

/// The first eight characters of a revision.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// An observation that recorded nothing.
fn nothing(summary: String) -> Observation {
    Observation {
        summary,
        records: Vec::new(),
        artifact: None,
    }
}

/// What an observation held to `deadline` answers: an error once it returned at or after it,
/// whatever it observed, so the budget that ended it records nothing; else `observed`.
fn held_to(
    deadline: Option<std::time::Instant>,
    action: &str,
    observed: Result<Observation>,
) -> Result<Observation> {
    if !deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
        return observed;
    }
    match observed {
        Err(error) => Err(error),
        Ok(_) => {
            bail!("{action} returned at or after the run's end; what it observed is not recorded")
        }
    }
}

/// The turn the tree's case is: the finding its `story.yaml` names and its `baseline`
/// revision, the promoted one the finding scripts are read at, never the commit the case forks
/// from (which the operator's `HEAD` decides, the candidate's included); or what to say when it
/// has either not.
struct Turn {
    finding: String,
    baseline: String,
}

fn turn_of(tree: &Path) -> Result<std::result::Result<Turn, String>> {
    let dir = crate::case::current(tree)?;
    let Some(finding) = crate::case::story_of(&dir)? else {
        return Ok(Err(format!(
            "the case names no finding ({})",
            crate::case::STORY_FILE
        )));
    };
    let Some(baseline) = case_revision(&dir, "baseline")? else {
        return Ok(Err(
            "the case names no baseline revision to read the finding scripts at".to_owned(),
        ));
    };
    Ok(Ok(Turn { finding, baseline }))
}

/// The finding's file in the planning store, for `story:<key>`, the key lower-case letters and
/// digits in words joined by `-`; none for anything else.
#[must_use]
pub fn finding_path(finding: &str) -> Option<String> {
    let key = finding.strip_prefix("story:")?;
    let is_key = key.split('-').all(|word| {
        !word.is_empty()
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    });
    is_key.then(|| format!(".engineering/planning/story/{key}.md"))
}

/// The finding scripts as the case's base holds them, each read through git with every object
/// on the way checked against its id, written executable into a directory of their own under
/// the cache, which is removed when this is dropped.
struct Scripts {
    dir: PathBuf,
}

impl Scripts {
    /// The scripts at `base` (the baseline) in `tree`, under `cache`; or what to say when it holds one
    /// not.
    fn at(
        tree: &Path,
        base: &str,
        cache: &Path,
        deadline: Option<std::time::Instant>,
    ) -> Result<std::result::Result<Self, String>> {
        static MADE: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let dir = cache.join("takt/improvement").join(format!(
            "scripts-{}-{}-{nanos}",
            std::process::id(),
            MADE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let scripts = Self { dir };
        for path in [CHECK_FINDING, FILE_FINDING] {
            let Some(bytes) =
                crate::loop_scorecard::verified_file_within(tree, base, path, deadline)?
            else {
                return Ok(Err(format!("the baseline {} holds no {path}", short(base))));
            };
            let to = scripts.path(path);
            std::fs::write(&to, bytes).with_context(|| format!("write {}", to.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&to, std::fs::Permissions::from_mode(0o700))
                    .with_context(|| format!("make {} executable", to.display()))?;
            }
        }
        Ok(Ok(scripts))
    }

    /// Where the script `path` (one of [`CHECK_FINDING`], [`FILE_FINDING`]) is.
    fn path(&self, path: &str) -> PathBuf {
        self.dir
            .join(Path::new(path).file_name().unwrap_or(path.as_ref()))
    }

    /// A detached checkout of `baseline` beside the scripts, outside the case's tree, so
    /// `check-finding` reads the finding as the baseline holds it, never from the case tree's
    /// working copy (which `git status` may not show whole: an index entry marked skip-worktree).
    /// It is a repository of its own that fetches `baseline` and `revision` by their ids from
    /// `tree`'s through git's transport, never a local copy of the object files: every object is
    /// hashed as it is received and checked (`transfer.fsckObjects`), so one overwritten in the
    /// case repository's store (which no step's guard covers) is an error naming it, and nothing
    /// is checked on other content than the commits hold. The finding's file there is checked
    /// against its bytes at `baseline`, read with every object checked; one that differs is what
    /// to say. It holds a branch at `baseline` and one at `revision`, the revision checked: the
    /// older reviewed `check-finding` builds from a local clone of the checkout, which takes what
    /// branches reach; the newer fetches the revision by its id.
    fn checkout(
        &self,
        tree: &Path,
        baseline: &str,
        revision: &str,
        finding: &str,
        deadline: Option<std::time::Instant>,
    ) -> Result<std::result::Result<PathBuf, String>> {
        let repo = self.dir.join("baseline");
        let source = std::path::absolute(tree)
            .with_context(|| format!("make {} absolute", tree.display()))?;
        let source = source
            .to_str()
            .ok_or_else(|| anyhow!("the tree {} is no UTF-8 path", tree.display()))?;
        crate::git::run(&self.dir, &["init", "--quiet", "--", "baseline"])?;
        crate::git::run(
            &repo,
            &[
                "-c",
                "transfer.fsckObjects=true",
                "fetch",
                "--quiet",
                "--no-tags",
                "--",
                source,
                baseline,
                revision,
            ],
        )
        .with_context(|| {
            format!(
                "fetch {} and {} from the case repository, every object hashed and checked; the turn is not judged on objects that do not hash to their ids",
                short(baseline),
                short(revision)
            )
        })?;
        crate::git::run(&repo, &["checkout", "--quiet", "--detach", baseline])?;
        for (branch, at) in [("turn-baseline", baseline), ("turn-checked", revision)] {
            crate::git::run(&repo, &["update-ref", &format!("refs/heads/{branch}"), at])?;
        }
        let Some(path) = finding_path(finding) else {
            return Ok(Err(format!(
                "the finding {finding} is no story of the store"
            )));
        };
        let held = crate::loop_scorecard::verified_file_within(tree, baseline, &path, deadline)?;
        let checked_out = std::fs::read(repo.join(&path)).ok();
        if held.is_none() || checked_out != held {
            return Ok(Err(format!(
                "the checkout of the baseline {} does not hold {path} as the baseline does",
                short(baseline)
            )));
        }
        Ok(Ok(repo))
    }
}

/// The directory of `revision`'s kept build under `check-finding`'s cache root `cache`, each
/// component from `cache` down read with `lstat` and never followed: the directory when every
/// component is a directory itself; else none, after removing the first component that is not
/// (a link is unlinked as itself, a file removed), so `check-finding` builds the revision again
/// on directories it makes. Nothing is removed through a link. None too when a component is
/// missing.
fn build_dir(cache: &Path, revision: &str) -> Result<Option<PathBuf>> {
    let mut at = cache.to_path_buf();
    for part in ["takt", "check-finding", "builds", revision] {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                std::fs::remove_file(&at)
                    .with_context(|| format!("unlink the planted {}", at.display()))?;
                return Ok(None);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).with_context(|| format!("read {}", at.display())),
        }
    }
    Ok(Some(at))
}

/// The sha256 of `revision`'s kept build when it is a regular file reached without a link
/// ([`build_dir`]); none for any other.
fn kept_build(cache: &Path, revision: &str) -> Result<Option<String>> {
    let Some(dir) = build_dir(cache, revision)? else {
        return Ok(None);
    };
    let binary = dir.join("takt");
    match std::fs::symlink_metadata(&binary) {
        Ok(metadata) if metadata.is_file() => Ok(Some(sha256_of(&binary)?)),
        _ => Ok(None),
    }
}

/// Whether `revision`'s kept build under `check-finding`'s cache may run: only a regular file,
/// reached from the cache root without a link ([`build_dir`]), whose sha256 is the one the turn
/// recorded when it built it (`artifacts/`[`build_record`] under `case_dir`). Any other (none
/// recorded, another digest, a link or a directory planted there) is removed with the `revision`
/// file beside it, each a link unlinked as itself and a directory removed only when it is one
/// itself, so `check-finding` builds the revision again.
fn trust_build(cache: &Path, case_dir: &Path, revision: &str) -> Result<()> {
    let Some(dir) = build_dir(cache, revision)? else {
        return Ok(());
    };
    let recorded = std::fs::read_to_string(case_dir.join("artifacts").join(build_record(revision)))
        .ok()
        .map(|text| text.trim().to_owned());
    let kept = kept_build(cache, revision)?;
    if kept.is_some() && kept == recorded {
        return Ok(());
    }
    for path in [dir.join("takt"), dir.join("revision")] {
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(&path),
            Ok(_) => std::fs::remove_file(&path),
            Err(_) => continue,
        }
        .with_context(|| format!("remove the untrusted build {}", path.display()))?;
    }
    Ok(())
}

/// The sha256 of the file at `path`, lower-case hex.
fn sha256_of(path: &Path) -> Result<String> {
    use sha2::{Digest as _, Sha256};
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

impl Drop for Scripts {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Runs `command` held to `checking`'s limits and to what is left before `deadline`, with no
/// credential of takt's environment and `XDG_CACHE_HOME` the cache `cache`.
fn run_script(
    command: &mut Command,
    checking: &Checking,
    cache: &Path,
    deadline: Option<std::time::Instant>,
) -> Result<crate::forge::Finished> {
    command.env("XDG_CACHE_HOME", cache);
    crate::forge::without_credentials(command, |_| false);
    let mut limits = checking.limits;
    if let Some(deadline) = deadline {
        limits.timeout = limits
            .timeout
            .min(deadline.saturating_duration_since(std::time::Instant::now()));
    }
    let program = command.get_program().to_string_lossy().into_owned();
    crate::forge::run_child(command, limits).with_context(|| format!("run {program}"))
}

/// The last non-empty line of `bytes`, at most 300 characters.
fn last_line(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let line = text
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    line.chars().take(300).collect()
}

/// `finding.file`: the finding the case was opened for (its `story.yaml`), as the planning store
/// holds it at the `finding` revision (the case's base commit), its predicate parsed by the
/// baseline's `scripts/file-finding --predicate`; writes one `finding_record` about `finding`:
/// `filed` when the predicate parses and the source it cites is a file at that revision,
/// `malformed` when either is not. It writes nothing to the planning store: the finding is filed
/// before the turn opens. A finding the store does not hold there, or a case that names none,
/// records nothing.
#[derive(Debug, Clone)]
pub struct FindingFile(pub Checking);

impl FindingFile {
    /// The action it performs.
    pub const ACTION: &str = "finding.file";
    /// The evidence kind it writes.
    pub const KIND: &str = "finding_record";
    /// Its name, the producer of its records.
    pub const NAME: &str = "takt.finding-file";

    fn observe_by(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<Observation> {
        let turn = match turn_of(tree)? {
            Ok(turn) => turn,
            Err(why) => return Ok(nothing(format!("{why}; nothing recorded"))),
        };
        let Some(path) = finding_path(&turn.finding) else {
            return Ok(nothing(format!(
                "the finding {} is no story of the planning store; nothing recorded",
                turn.finding
            )));
        };
        let Some(text) =
            crate::loop_scorecard::verified_file_within(tree, revision, &path, deadline)?
        else {
            return Ok(nothing(format!(
                "the finding {} is not filed at {}: no {path}; nothing recorded",
                turn.finding,
                short(revision)
            )));
        };
        let cache = self.0.cache()?;
        let scripts = match Scripts::at(tree, &turn.baseline, &cache, deadline)? {
            Ok(scripts) => scripts,
            Err(why) => return Ok(nothing(format!("{why}; nothing recorded"))),
        };
        let body = scripts.dir.join("finding.md");
        std::fs::write(&body, &text).with_context(|| format!("write {}", body.display()))?;
        let mut command = Command::new(scripts.path(FILE_FINDING));
        command
            .arg("--predicate")
            .arg(&body)
            .current_dir(&scripts.dir);
        let output = run_script(&mut command, &self.0, &cache, deadline)?;
        let record = |result: &str| {
            Record::new(
                format!("finding-record-{seq}"),
                Self::KIND,
                Some(result.to_owned()),
                "finding",
                revision,
                Self::NAME,
            )
        };
        let shown = format!("the finding {} at {}", turn.finding, short(revision));
        match output.code {
            Some(0) => {}
            Some(2) => {
                return Ok(Observation {
                    summary: format!("{shown}: malformed; {}", last_line(&output.stderr)),
                    records: vec![record("malformed")],
                    artifact: None,
                });
            }
            code => {
                return Ok(nothing(format!(
                    "{shown}: {FILE_FINDING} --predicate exited {}: {}; nothing recorded",
                    code.map_or("on a signal".to_owned(), |code| code.to_string()),
                    last_line(&output.stderr)
                )));
            }
        }
        let parsed = String::from_utf8_lossy(&output.stdout);
        let source = parsed
            .lines()
            .find_map(|line| line.strip_prefix("source\t"))
            .unwrap_or("");
        let relative = !source.is_empty()
            && !source.starts_with('/')
            && source
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..");
        let present = relative
            && crate::loop_scorecard::verified_file_within(tree, revision, source, deadline)?
                .is_some();
        let (result, why) = if present {
            ("filed", format!("its predicate parses and cites {source}"))
        } else {
            (
                "malformed",
                format!(
                    "its predicate cites the source `{source}`, which is no file at {}",
                    short(revision)
                ),
            )
        };
        Ok(Observation {
            summary: format!("{shown}: {result}; {why}"),
            records: vec![record(result)],
            artifact: None,
        })
    }
}

impl Validator for FindingFile {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "finding"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_within(tree, revision, seq, None)
            .map(|(observation, _)| observation)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = self.observe_by(tree, revision, seq, deadline);
        Ok((held_to(deadline, Self::ACTION, observed)?, false))
    }
}

/// Runs the baseline's `scripts/check-finding` on the case's finding at `revision` and writes one
/// `scenario_result` about `validator`'s subject from the line it prints: `reproduced`,
/// `not_reproduced` or `error` (which makes no claim true). A check that gives no result (exit
/// 1: no build, a scenario not declared, a predicate past its time) or is refused (exit 64: no
/// scenario checkout for a predicate naming a scenario) records nothing, the summary naming its
/// last line.
fn check_finding(
    checking: &Checking,
    validator: &dyn Validator,
    tree: &Path,
    revision: &str,
    seq: usize,
    deadline: Option<std::time::Instant>,
) -> Result<Observation> {
    let (action, subject) = (validator.action(), validator.subject());
    let turn = match turn_of(tree)? {
        Ok(turn) => turn,
        Err(why) => return Ok(nothing(format!("{action}: {why}; nothing recorded"))),
    };
    let cache = checking.cache()?;
    let scripts = match Scripts::at(tree, &turn.baseline, &cache, deadline)? {
        Ok(scripts) => scripts,
        Err(why) => return Ok(nothing(format!("{action}: {why}; nothing recorded"))),
    };
    let repo = match scripts.checkout(tree, &turn.baseline, revision, &turn.finding, deadline)? {
        Ok(repo) => repo,
        Err(why) => return Ok(nothing(format!("{action}: {why}; nothing recorded"))),
    };
    // A kept build runs only while it is the one this turn built; else it is built again, and
    // what was built is recorded in the case, which every step's guard puts back.
    let case_dir = crate::case::current(tree)?;
    trust_build(&cache, &case_dir, revision)?;
    let mut command = Command::new(scripts.path(CHECK_FINDING));
    command.arg("--repo").arg(&repo);
    if let Some(scenarios) = &checking.scenarios {
        command.arg("--scenarios").arg(scenarios);
    }
    command
        .arg("--")
        .arg(&turn.finding)
        .arg(revision)
        .current_dir(&repo);
    let output = run_script(&mut command, checking, &cache, deadline)?;
    let built = kept_build(&cache, revision)?.map(|digest| Artifact {
        name: build_record(revision),
        bytes: format!("{digest}\n").into_bytes(),
    });
    let shown = format!(
        "the finding {}'s predicate with takt at {} ({subject})",
        turn.finding,
        short(revision)
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    let result = printed.trim();
    if !output.success() || !matches!(result, "reproduced" | "not_reproduced" | "error") {
        return Ok(Observation {
            artifact: built,
            ..nothing(format!(
                "{shown}: {CHECK_FINDING} gave no result (exit {}): {}; nothing recorded",
                output
                    .code
                    .map_or("on a signal".to_owned(), |code| code.to_string()),
                last_line(&output.stderr)
            ))
        });
    }
    Ok(Observation {
        summary: format!("{shown}: {result}; {}", last_line(&output.stderr)),
        records: vec![Record::new(
            format!("scenario-result-{seq}"),
            "scenario_result",
            Some(result.to_owned()),
            subject,
            revision,
            validator.name(),
        )],
        artifact: built,
    })
}

/// `scenario.reproduce`: the finding's predicate run with a takt built from the `baseline`
/// revision, through the baseline's `scripts/check-finding`; one `scenario_result` about
/// `baseline`.
#[derive(Debug, Clone)]
pub struct ScenarioReproduce(pub Checking);

impl ScenarioReproduce {
    /// The action it performs.
    pub const ACTION: &str = "scenario.reproduce";
    /// The evidence kind it writes.
    pub const KIND: &str = "scenario_result";
    /// Its name, the producer of its records.
    pub const NAME: &str = "takt.scenario-reproduce";
}

impl Validator for ScenarioReproduce {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "baseline"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_within(tree, revision, seq, None)
            .map(|(observation, _)| observation)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = check_finding(&self.0, self, tree, revision, seq, deadline);
        Ok((held_to(deadline, Self::ACTION, observed)?, false))
    }
}

/// `scenario.verify`: the finding's predicate run with a takt built from the `candidate`
/// revision, through the baseline's `scripts/check-finding`; one `scenario_result` about
/// `candidate`. It is no run of the agent that delivered the candidate, which the kernel
/// declares when the turn opens, so its `not_reproduced` earns `scenario.passes_after`.
#[derive(Debug, Clone)]
pub struct ScenarioVerify(pub Checking);

impl ScenarioVerify {
    /// The action it performs.
    pub const ACTION: &str = "scenario.verify";
    /// The evidence kind it writes.
    pub const KIND: &str = "scenario_result";
    /// Its name, the producer of its records.
    pub const NAME: &str = "takt.scenario-verify";
}

impl Validator for ScenarioVerify {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "candidate"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_within(tree, revision, seq, None)
            .map(|(observation, _)| observation)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = check_finding(&self.0, self, tree, revision, seq, deadline);
        Ok((held_to(deadline, Self::ACTION, observed)?, false))
    }
}

/// `scorecard.compare`: the candidate's saved scorecard, read from
/// `<state>/`[`SCORECARDS`]`/scorecard-<candidate>.json`, compared with the scorecard copy of the
/// case's baseline in the promotion store ([`crate::promotion::store`]) under the promotion rule
/// ([`Scorecard::against`]); writes one `scorecard_result` about `candidate`: `regression` when a
/// scenario is worse, else `no_regression`, the comparison retained as
/// `scorecard-compare-<seq>.txt`. While the file is missing it records nothing and waits, naming
/// the path. A scorecard named by another revision, saved under another rule, with a run made
/// with another binary than the candidate's or a scenario run without its held-out result, a
/// baseline the store holds no record of, or a comparison with too few cases, records nothing.
#[derive(Debug, Clone)]
pub struct ScorecardCompare {
    /// The state directory; none, and it is the one the promotion store lies in
    /// (`$XDG_STATE_HOME`, `~/.local/state` without it), read when the validator observes, so no
    /// other case needs one.
    pub state: Option<PathBuf>,
}

impl ScorecardCompare {
    /// The action it performs.
    pub const ACTION: &str = "scorecard.compare";
    /// The evidence kind it writes.
    pub const KIND: &str = "scorecard_result";
    /// Its name, the producer of its records.
    pub const NAME: &str = "takt.scorecard-compare";

    /// Reading the state directory `state`.
    #[must_use]
    pub fn in_state(state: &Path) -> Self {
        Self {
            state: Some(state.to_path_buf()),
        }
    }

    /// Reading the state directory the promotion store lies in, when it observes.
    #[must_use]
    pub fn from_env() -> Self {
        Self { state: None }
    }

    /// The state directory: the one given, else the one the promotion store lies in.
    pub fn state(&self) -> Result<PathBuf> {
        if let Some(state) = &self.state {
            return Ok(state.clone());
        }
        let store = crate::promotion::store()?;
        store
            .ancestors()
            .nth(Path::new(crate::promotion::STORE).components().count())
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                anyhow!(
                    "the promotion store {} has no state directory",
                    store.display()
                )
            })
    }

    /// Where the saved scorecard of `revision` is read.
    pub fn saved(&self, revision: &str) -> Result<PathBuf> {
        Ok(self
            .state()?
            .join(SCORECARDS)
            .join(format!("scorecard-{revision}.json")))
    }

    fn observe_by(&self, tree: &Path, revision: &str, seq: usize) -> Result<(Observation, bool)> {
        let state = self.state()?;
        let path = self.saved(revision)?;
        if std::fs::symlink_metadata(&path).is_err() {
            return Ok((
                nothing(format!(
                    "no saved scorecard of the candidate {} at {}; save it there with `takt scorecard --save`; waiting",
                    short(revision),
                    path.display()
                )),
                true,
            ));
        }
        let card = match Scorecard::load(&path) {
            Ok(card) => card,
            Err(error) => return Ok((nothing(format!("{error:#}; nothing recorded")), false)),
        };
        if let Some(why) = unjudged(&card, revision) {
            return Ok((
                nothing(format!("{}: {why}; nothing recorded", path.display())),
                false,
            ));
        }
        let dir = crate::case::current(tree)?;
        let Some(baseline) = case_revision(&dir, "baseline")? else {
            return Ok((
                nothing("the case names no baseline revision; nothing recorded".to_owned()),
                false,
            ));
        };
        let store = state.join(crate::promotion::STORE);
        let records = crate::promotion::records(&store, &crate::now())?;
        let Some(base) = records
            .iter()
            .rfind(|record| record.kind != Kind::Refused && record.revision == baseline)
        else {
            return Ok((
                nothing(format!(
                    "the promotion store {} holds no baseline or promotion of the case's baseline {}; nothing recorded",
                    store.display(),
                    short(&baseline)
                )),
                false,
            ));
        };
        // The copy beside the record, as the promotion store names it.
        let copy = match base.kind {
            Kind::Rebaseline => store.join(format!(
                "{}.rebaseline-{}.scorecard.json",
                base.revision, base.scorecard_sha256
            )),
            _ => store.join(format!("{}.scorecard.json", base.revision)),
        };
        let base_card = Scorecard::load(&copy)?;
        if base_card.revision.as_deref() != Some(baseline.as_str()) {
            return Ok((
                nothing(format!(
                    "the baseline's scorecard copy {} is not of {}; nothing recorded",
                    copy.display(),
                    short(&baseline)
                )),
                false,
            ));
        }
        let mut compared = Vec::new();
        let worse = match card.against(&base_card, &mut compared) {
            Ok(worse) => worse,
            Err(error) => {
                return Ok((
                    nothing(format!(
                        "the candidate {} against the baseline {}: {error:#}; nothing recorded",
                        short(revision),
                        short(&baseline)
                    )),
                    false,
                ));
            }
        };
        let result = if worse > 0 {
            "regression"
        } else {
            "no_regression"
        };
        let artifact = format!("scorecard-compare-{seq}.txt");
        Ok((
            Observation {
                summary: format!(
                    "the candidate {} against the baseline {}: {worse} scenario{} worse, {result}; retained as {artifact}",
                    short(revision),
                    short(&baseline),
                    if worse == 1 { "" } else { "s" }
                ),
                records: vec![Record::new(
                    format!("scorecard-result-{seq}"),
                    Self::KIND,
                    Some(result.to_owned()),
                    "candidate",
                    revision,
                    Self::NAME,
                )],
                artifact: Some(Artifact {
                    name: artifact,
                    bytes: compared,
                }),
            },
            false,
        ))
    }
}

/// The revision of `artifact` in the case at `dir`, as its `case.yaml` holds it.
fn case_revision(dir: &Path, artifact: &str) -> Result<Option<String>> {
    let path = dir.join("case.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let doc: serde_yaml_ng::Value = crate::loop_scorecard::read_yaml(&path, &text)?;
    Ok(doc["artifacts"][artifact]["revision"]
        .as_str()
        .map(str::to_owned))
}

/// Why `card` judges nothing about `revision`, or none: it is named by another revision, saved
/// under another rule than takt's, has a run made with another binary than the revision's, as
/// the run's `opened` line names it, or a scenario run without its held-out result.
fn unjudged(card: &Scorecard, revision: &str) -> Option<String> {
    if card.revision.as_deref() != Some(revision) {
        return Some(format!(
            "the scorecard is named by {}, not by the candidate {}",
            card.revision.as_deref().unwrap_or("no revision"),
            short(revision)
        ));
    }
    if card.rule != Rule::default() {
        return Some(format!(
            "its rule is trials {} margin {}, not takt's trials {} margin {}",
            card.rule.trials,
            card.rule.margin,
            Rule::default().trials,
            Rule::default().margin
        ));
    }
    let mut counted: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    for row in &card.rows {
        let n = counted.entry(row.case.as_str()).or_insert(0);
        *n += 1;
        if row.opened_binary.as_deref() != Some(revision) {
            return Some(format!(
                "run {}#{n} was made with {}, not with the binary of the candidate {}",
                row.case,
                row.opened_binary
                    .as_deref()
                    .map_or("a binary that recorded no revision".to_owned(), |binary| {
                        format!("the binary of {}", short(binary))
                    }),
                short(revision)
            ));
        }
        if row.scenario.is_some() && row.held_out.is_none() {
            return Some(format!("run {}#{n} carries no held-out result", row.case));
        }
    }
    None
}

impl Validator for ScorecardCompare {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "candidate"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_waiting(tree, revision, seq)
            .map(|(observation, _)| observation)
    }

    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        self.observe_by(tree, revision, seq)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = self.observe_by(tree, revision, seq);
        let waiting = matches!(&observed, Ok((_, true)));
        let observed = held_to(deadline, Self::ACTION, observed.map(|(o, _)| o))?;
        Ok((observed, waiting))
    }
}
