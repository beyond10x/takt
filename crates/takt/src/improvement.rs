//! The validators of an improvement turn, a `self.improvement/1` case
//! (`protocols/self-improvement/1.yaml`, design:recursive-self-improvement § The loops as
//! protocols): [`FindingFile`] (`finding.file`), [`ScenarioReproduce`] (`scenario.reproduce`),
//! [`ScenarioVerify`] (`scenario.verify`), [`ScorecardCompare`] (`scorecard.compare`),
//! [`CandidateMerged`] (`candidate.merge` and `candidate.merge_judge_surface`) and
//! [`BinaryPromote`] (`binary.promote`). A turn opens with its finding, its candidate, the
//! promoted baseline and its judge surface ([`crate::case::turn_opening`],
//! [`crate::case::CaseDir::open_turn`]), which declares each of them independent of the run
//! that delivered the candidate ([`PRODUCERS`]): a validator is no agent run.
//!
//! These validators run in the takt that drives the case, the installed promoted binary, which
//! judges; the candidate's binary runs only inside `scripts/check-finding`'s run step. The
//! scripts they run (`scripts/check-finding`, and `scripts/file-finding` it calls) are read at the
//! turn's baseline, the promoted revision, checked against their ids, and run from a directory
//! of their own, never from the tree a step edits nor from the commit the case forks from, which
//! the operator's `HEAD` decides; a candidate that changes them does not open a turn
//! ([`crate::case::turn_opening`]). The finding is read on the mainline ([`Mainline`]), at the
//! commit the case forks from, and checked out there for the check. The candidate is delivered
//! outside the case and given at open.
//!
//! The merge and the promotion are observed, never made: the candidate is merged in its own
//! delivery case, or by the owner, and promoted by `takt promote`; the turn records each once the
//! case holds its grant and the mainline holds the candidate, or the promotion store's latest
//! record installs it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::evidence::Record;
use crate::forge::{Forge, Limits};
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
pub const CONFINED_CHECK_FINDING_SHA256S: [&str; 1] = [
    // The script of story:check-finding-run-step-confined (its last commit f2a20e9), its build's
    // checkout a repository made with no template, by a git under a cleared environment, that
    // fetches the revision through git's transport, every object hashed and checked
    // (story:self-improvement-protocol, part 1), and no replace ref of a repository it reads
    // honoured (part 2).
    "7878f0f27ad4618280967d4674de65a3ae86fe4b54a5a2b28fde02b7ef36a2cf",
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
pub const PRODUCERS: [&str; 7] = [
    FindingFile::NAME,
    ScenarioReproduce::NAME,
    ScenarioVerify::NAME,
    ScorecardCompare::NAME,
    CandidateMerged::NAME,
    CandidateMerged::JUDGE_SURFACE_NAME,
    BinaryPromote::NAME,
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

/// The turn the tree's case is: the finding its `story.yaml` names, its `baseline` revision,
/// the promoted one the finding scripts are read at, never the commit the case forks from
/// (which the operator's `HEAD` decides, the candidate's included), and its `finding`
/// revision, the commit the case forks from, where the finding is read; or what to say when it
/// has one not.
struct Turn {
    finding: String,
    baseline: String,
    filed: String,
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
    let Some(filed) = case_revision(&dir, "finding")? else {
        return Ok(Err(
            "the case names no finding revision to read the finding at".to_owned(),
        ));
    };
    Ok(Ok(Turn {
        finding,
        baseline,
        filed,
    }))
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

/// The cache root `<cache>/takt`, made when there is none, opened once and followed when it is
/// a link (the operator's choice of where takt's cache lives); everything below it is reached
/// relative to that descriptor and holds no link.
fn cache_root(cache: &Path) -> Result<at::Dir> {
    let root = cache.join("takt");
    std::fs::create_dir_all(&root).with_context(|| format!("make {}", root.display()))?;
    at::Dir::open(&root)
}

/// The finding scripts as the baseline holds them, each read through git with every object on
/// the way checked against its id, written executable into a directory of their own under the
/// cache root's `improvement/` (a link there refuses the check), made and, when this is dropped,
/// removed through directory descriptors, nothing through a link.
struct Scripts {
    dir: PathBuf,
    /// The cache root's `improvement/`, held open.
    parent: at::Dir,
    /// The scripts' own directory, held open.
    held: at::Dir,
    name: std::ffi::OsString,
}

impl Scripts {
    /// The scripts at `base` (the baseline) in `tree`, under `cache`; or what to say when it
    /// holds one not.
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
        let name = std::ffi::OsString::from(format!(
            "scripts-{}-{}-{nanos}",
            std::process::id(),
            MADE.fetch_add(1, Ordering::Relaxed)
        ));
        let parent = cache_root(cache)?.child_made("improvement".as_ref())?;
        let held = parent.child_made(&name)?;
        let scripts = Self {
            dir: held.path.clone(),
            parent,
            held,
            name,
        };
        for path in [CHECK_FINDING, FILE_FINDING] {
            let Some(bytes) =
                crate::loop_scorecard::verified_file_within(tree, base, path, deadline)?
            else {
                return Ok(Err(format!("the baseline {} holds no {path}", short(base))));
            };
            scripts.create(
                Path::new(path).file_name().unwrap_or(path.as_ref()),
                &bytes,
                0o700,
            )?;
        }
        Ok(Ok(scripts))
    }

    /// Writes `bytes` as the new file `name` in the scripts' directory, through its descriptor.
    fn create(&self, name: &std::ffi::OsStr, bytes: &[u8], mode: u32) -> Result<()> {
        self.held.create(name, bytes, mode)
    }

    /// Where the script `path` (one of [`CHECK_FINDING`], [`FILE_FINDING`]) is.
    fn path(&self, path: &str) -> PathBuf {
        self.dir
            .join(Path::new(path).file_name().unwrap_or(path.as_ref()))
    }

    /// A detached checkout of `filed`, the turn's finding revision, beside the scripts, outside
    /// the case's tree, so `check-finding` reads the finding as that commit of the mainline holds
    /// it, never from the case tree's working copy (which `git status` may not show whole: an
    /// index entry marked skip-worktree). It is a repository of its own that fetches `filed`,
    /// `baseline` and `revision` by their ids from `tree`'s through git's transport, never a
    /// local copy of the object files: every object is hashed as it is received and checked
    /// (`transfer.fsckObjects`), so one overwritten in the case repository's store (which no
    /// step's guard covers) is an error naming it, and nothing is checked on other content than
    /// the commits hold. The finding's file there is checked against its bytes at `filed`, read
    /// with every object checked; one that differs is what to say. It holds a branch at
    /// `baseline` and one at `revision`, the revision checked, so a script that clones the
    /// checkout rather than fetching by id reaches both.
    fn checkout(
        &self,
        tree: &Path,
        turn: &Turn,
        revision: &str,
        deadline: Option<std::time::Instant>,
    ) -> Result<std::result::Result<PathBuf, String>> {
        let (baseline, filed, finding) = (
            turn.baseline.as_str(),
            turn.filed.as_str(),
            turn.finding.as_str(),
        );
        let repo = self.dir.join("baseline");
        let source = std::path::absolute(tree)
            .with_context(|| format!("make {} absolute", tree.display()))?;
        let source = source
            .to_str()
            .ok_or_else(|| anyhow!("the tree {} is no UTF-8 path", tree.display()))?;
        crate::git::run(
            &self.dir,
            &["init", "--quiet", "--template=", "--", "baseline"],
        )?;
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
                filed,
                baseline,
                revision,
            ],
        )
        .with_context(|| {
            format!(
                "fetch {}, {} and {} from the case repository, every object hashed and checked; the turn is not judged on objects that do not hash to their ids",
                short(filed),
                short(baseline),
                short(revision)
            )
        })?;
        crate::git::run(&repo, &["checkout", "--quiet", "--detach", filed])?;
        for (branch, at) in [("turn-baseline", baseline), ("turn-checked", revision)] {
            crate::git::run(&repo, &["update-ref", &format!("refs/heads/{branch}"), at])?;
        }
        let Some(path) = finding_path(finding) else {
            return Ok(Err(format!(
                "the finding {finding} is no story of the store"
            )));
        };
        let held = crate::loop_scorecard::verified_file_within(tree, filed, &path, deadline)?;
        let checked_out = std::fs::read(repo.join(&path)).ok();
        if held.is_none() || checked_out != held {
            return Ok(Err(format!(
                "the checkout of the finding's revision {} does not hold {path} as that commit does",
                short(filed)
            )));
        }
        Ok(Ok(repo))
    }
}

/// The directory of `revision`'s kept build, `check-finding/builds/<revision>` under the cache
/// root, held open and reached through descriptors: none while a component is missing. A link,
/// or anything but a directory, at `check-finding` or `builds` is an error naming it, nothing
/// removed. An entry at `builds/<revision>` that is a link or a file is no build: it is unlinked
/// as itself (never what it names), and none answered, so `check-finding` builds the revision
/// again.
fn build_dir(cache: &Path, revision: &str) -> Result<Option<at::Dir>> {
    let root = cache_root(cache)?;
    let Some(checks) = root.child("check-finding".as_ref())? else {
        return Ok(None);
    };
    let Some(builds) = checks.child("builds".as_ref())? else {
        return Ok(None);
    };
    match builds.entry(revision.as_ref())? {
        at::Entry::Missing => Ok(None),
        at::Entry::Dir => builds.child(revision.as_ref()),
        at::Entry::File | at::Entry::Link | at::Entry::Other => {
            builds.unlink(revision.as_ref())?;
            Ok(None)
        }
    }
}

/// The sha256 of `revision`'s kept build when it is a regular file reached through descriptors
/// without a link ([`build_dir`]); none for any other.
fn kept_build(cache: &Path, revision: &str) -> Result<Option<String>> {
    use sha2::{Digest as _, Sha256};
    let Some(dir) = build_dir(cache, revision)? else {
        return Ok(None);
    };
    Ok(dir
        .regular("takt".as_ref())?
        .map(|bytes| format!("{:x}", Sha256::digest(&bytes))))
}

/// Whether `revision`'s kept build may run: only a regular file, reached through descriptors
/// without a link ([`build_dir`]), whose sha256 is the one the turn recorded when it built it
/// (`artifacts/`[`build_record`] under `case_dir`). Any other is removed with the `revision` file
/// beside it, each a file or a link unlinked as itself relative to the build's directory, so
/// `check-finding` builds the revision again; there is no removal by a path resolved again. A
/// directory in either place is none a build makes: it is an error naming it, nothing removed.
fn trust_build(cache: &Path, case_dir: &Path, revision: &str) -> Result<()> {
    let Some(dir) = build_dir(cache, revision)? else {
        return Ok(());
    };
    for name in ["takt", "revision"] {
        if matches!(dir.entry(name.as_ref())?, at::Entry::Dir | at::Entry::Other) {
            bail!(
                "{} is no file a build makes; the check is refused, and nothing is removed",
                dir.path.join(name).display()
            );
        }
    }
    let recorded = std::fs::read_to_string(case_dir.join("artifacts").join(build_record(revision)))
        .ok()
        .map(|text| text.trim().to_owned());
    let kept = {
        use sha2::{Digest as _, Sha256};
        dir.regular("takt".as_ref())?
            .map(|bytes| format!("{:x}", Sha256::digest(&bytes)))
    };
    if kept.is_some() && kept == recorded {
        return Ok(());
    }
    for name in ["takt", "revision"] {
        dir.unlink(name.as_ref())?;
    }
    Ok(())
}

impl Drop for Scripts {
    fn drop(&mut self) {
        let _ = self.parent.remove(&self.name);
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
        scripts.create("finding.md".as_ref(), &text, 0o600)?;
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
    let repo = match scripts.checkout(tree, &turn, revision, deadline)? {
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
        state_dir(self.state.as_deref())
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

/// The state directory: `given`, else the one the promotion store lies in (`$XDG_STATE_HOME`,
/// `~/.local/state` without it).
fn state_dir(given: Option<&Path>) -> Result<PathBuf> {
    if let Some(state) = given {
        return Ok(state.to_path_buf());
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

    /// It waits on the candidate's scorecard file under the state directory, which only the
    /// operator saves.
    fn waits_locally(&self) -> bool {
        true
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

/// Where a turn reads the mainline, the branch [`crate::promotion::MAINLINE`] (`main`) a merged
/// candidate is in: its tip as the forge reports it ([`Forge::target_tip`]), or, with no forge,
/// as the case's own repository holds it. Under `--forge none` the operator's local `main` is
/// trusted, as everything else under no forge is: whoever moves it decides what is merged.
#[derive(Clone)]
pub enum Mainline {
    /// The forge's `main`, read through [`Forge::target_tip`] when the turn reads it.
    Forge(Rc<dyn Forge>),
    /// The case repository's `refs/heads/main`.
    Local,
}

impl std::fmt::Debug for Mainline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mainline({})", self.forge())
    }
}

impl Mainline {
    /// The forge's name, or `none` for the local `main`.
    #[must_use]
    pub fn forge(&self) -> String {
        match self {
            Self::Forge(forge) => forge.name().to_owned(),
            Self::Local => "none".to_owned(),
        }
    }

    /// The commit `main` is at now, an object id `tree`'s repository holds.
    pub fn tip(&self, tree: &Path) -> Result<String> {
        let main = crate::promotion::MAINLINE;
        let tip = match self {
            Self::Forge(forge) => forge.target_tip(tree, main).with_context(|| {
                format!(
                    "read the tip of {main}, the mainline, through the forge {}",
                    forge.name()
                )
            })?,
            Self::Local => crate::git::run(
                tree,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "--end-of-options",
                    &format!("refs/heads/{main}^{{commit}}"),
                ],
            )
            .with_context(|| {
                format!(
                    "read {main}, the mainline with no forge, in {}",
                    tree.display()
                )
            })?,
        };
        if !crate::forge::is_object_id(&tip) {
            bail!(
                "{main}, the mainline (forge {}), is at `{}`, which is no object id",
                self.forge(),
                tip.escape_debug()
            );
        }
        Ok(tip)
    }

    /// Whether the mainline at `tip` holds `revision` (it is `tip` or an ancestor of it), read
    /// from a copy of both made through git's transport, every object hashed as it arrives; the
    /// inner error when they do not arrive as their ids.
    pub fn holds(
        tree: &Path,
        tip: &str,
        revision: &str,
    ) -> Result<std::result::Result<bool, String>> {
        let mirror = match crate::git::Mirror::of(tree, &[tip, revision])? {
            Ok(mirror) => mirror,
            Err(why) => return Ok(Err(why)),
        };
        Ok(Ok(mirror
            .output(&[
                "merge-base",
                "--is-ancestor",
                "--end-of-options",
                revision,
                tip,
            ])?
            .status
            .success()))
    }
}

/// The judge surfaces the candidate's change touches, in the order they are found: every path
/// that [`crate::validator::JUDGE_ALLOW`] at `baseline` does not admit, that is one of
/// [`crate::validator::OWNER_ONLY`], or whose new mode is no regular file's, changed from
/// `baseline` to `candidate` (the trees' difference, which promoting the candidate installs, so
/// a candidate branched before a judge change that would roll it back is judge) or from any point
/// where `candidate` meets `baseline` (`git merge-base --all`) to `candidate` (the candidate's
/// own change, which a baseline holding the same change would hide). Read from a copy of both
/// made through git's transport, every object hashed as it arrives. An allow-list that does not
/// read admits nothing; a candidate with no history in common with the baseline, or commits that
/// do not arrive as their ids, are an error.
pub fn judge_surfaces(tree: &Path, baseline: &str, candidate: &str) -> Result<Vec<String>> {
    let mirror = match crate::git::Mirror::of(tree, &[baseline, candidate])? {
        Ok(mirror) => mirror,
        Err(why) => bail!(
            "the baseline {} and the candidate {} do not arrive from this repository as their ids, every object hashed again ({why})",
            short(baseline),
            short(candidate)
        ),
    };
    let globs = crate::validator::allow_list_at(&mirror, baseline)?.unwrap_or_default();
    let meets: Vec<String> = mirror
        .run(&[
            "merge-base",
            "--all",
            "--end-of-options",
            baseline,
            candidate,
        ])
        .unwrap_or_default()
        .lines()
        .filter(|meet| crate::forge::is_object_id(meet))
        .map(str::to_owned)
        .collect();
    if meets.is_empty() {
        bail!(
            "the candidate {} has no history in common with the baseline {}, so its change is not known",
            short(candidate),
            short(baseline)
        );
    }
    let mut surfaces: Vec<String> = Vec::new();
    for from in std::iter::once(baseline).chain(meets.iter().map(String::as_str)) {
        let diff = mirror.bytes(&[
            "diff-tree",
            "-r",
            "-z",
            "--no-renames",
            "--raw",
            from,
            candidate,
            "--",
        ])?;
        for (mode, path) in crate::validator::raw_changes(&diff)? {
            let judge = !crate::validator::NAMED_BY_PATH.contains(&mode)
                || crate::validator::OWNER_ONLY.contains(&path)
                || !globs.iter().any(|glob| crate::run::glob_admits(glob, path));
            if judge && !surfaces.iter().any(|known| known == path) {
                surfaces.push(path.to_owned());
            }
        }
    }
    Ok(surfaces)
}

/// Why the case at `dir`'s grant of [`crate::validator::JUDGE_SURFACE_CAPABILITY`] does not
/// authorise merging the candidate `revision`, a judge-surface change, by story:judge-guard's
/// rule ([`crate::validator::approver_refusal`]): the owner's grant does, an agent role's only
/// under an unrevoked delegation in [`crate::validator::DELEGATIONS`] at the mainline's `tip`,
/// read from a copy made through git's transport, and never for a change to a file of
/// [`crate::validator::OWNER_ONLY`]; or none when it does.
fn judge_surface_refusal(
    dir: &Path,
    tree: &Path,
    tip: &str,
    revision: &str,
) -> Result<Option<String>> {
    let capability = crate::validator::JUDGE_SURFACE_CAPABILITY;
    let authority = crate::case::Authority::read(dir)?.unwrap_or_default();
    let approver = authority
        .decision(capability)
        .map_or("", |decision| decision.approver.as_str());
    let Some(baseline) = case_revision(dir, "baseline")? else {
        return Ok(Some(
            "the case names no baseline revision to judge the candidate's change against"
                .to_owned(),
        ));
    };
    let surfaces = judge_surfaces(tree, &baseline, revision)?;
    let owner_only: Vec<&str> = surfaces
        .iter()
        .map(String::as_str)
        .filter(|path| crate::validator::OWNER_ONLY.contains(path))
        .collect();
    let mirror = match crate::git::Mirror::of(tree, &[tip])? {
        Ok(mirror) => mirror,
        Err(why) => {
            return Ok(Some(format!(
                "the mainline's tip {} does not arrive from this repository as its id ({why})",
                short(tip)
            )));
        }
    };
    crate::validator::approver_refusal(
        &mirror,
        tip,
        crate::promotion::MAINLINE,
        approver,
        &owner_only,
    )
}

/// `candidate.merge` and `candidate.merge_judge_surface`, one type for both: observes, never
/// merges. Writes one `merge_observation` about `candidate`, `merged`, when the case holds the
/// grant its protocol requires for the action at the candidate revision
/// ([`crate::case::holds_grant`]) and the mainline's tip ([`Mainline::tip`]) is the candidate or
/// descends from it; the mainline it read (the forge, `none` for the local `main`) and the tip
/// are retained beside the record as `merge-observation-<seq>.txt`. While the mainline does not
/// hold the candidate it records nothing and waits, naming the tip; without the grant it records
/// nothing. `candidate.merge_judge_surface` holds its grant to story:judge-guard's rule besides
/// ([`judge_surface_refusal`]): the owner's, or an agent role's under an unrevoked delegation on
/// record at the mainline's tip; any other grant records nothing. The merge itself happens in
/// the candidate's own delivery case, under the judge guard, or by the owner.
#[derive(Clone)]
pub struct CandidateMerged {
    action: &'static str,
    name: &'static str,
    mainline: Mainline,
}

impl std::fmt::Debug for CandidateMerged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CandidateMerged")
            .field("action", &self.action)
            .field("mainline", &self.mainline)
            .finish()
    }
}

impl CandidateMerged {
    /// The action of a candidate that touches no judge surface.
    pub const ACTION: &str = "candidate.merge";
    /// The action of a candidate that touches a judge surface.
    pub const JUDGE_SURFACE_ACTION: &str = "candidate.merge_judge_surface";
    /// The evidence kind it writes.
    pub const KIND: &str = "merge_observation";
    /// The producer of `candidate.merge`'s records.
    pub const NAME: &str = "takt.candidate-merge";
    /// The producer of `candidate.merge_judge_surface`'s records.
    pub const JUDGE_SURFACE_NAME: &str = "takt.candidate-merge-judge-surface";

    /// `candidate.merge`, reading `mainline`.
    #[must_use]
    pub fn new(mainline: Mainline) -> Self {
        Self {
            action: Self::ACTION,
            name: Self::NAME,
            mainline,
        }
    }

    /// `candidate.merge_judge_surface`, reading `mainline`.
    #[must_use]
    pub fn judge_surface(mainline: Mainline) -> Self {
        Self {
            action: Self::JUDGE_SURFACE_ACTION,
            name: Self::JUDGE_SURFACE_NAME,
            mainline,
        }
    }

    fn observe_by(&self, tree: &Path, revision: &str, seq: usize) -> Result<(Observation, bool)> {
        let dir = crate::case::current(tree)?;
        if !crate::case::holds_grant(&dir, self.action)? {
            return Ok((
                nothing(format!(
                    "{} is not granted at the candidate {}; nothing recorded",
                    self.action,
                    short(revision)
                )),
                false,
            ));
        }
        let main = crate::promotion::MAINLINE;
        let forge = self.mainline.forge();
        // The mainline as a summary names it: the forge's, or with none the local one, its
        // summaries then saying nothing of a forge.
        let named = match self.mainline {
            Mainline::Forge(_) => format!("{main} (forge: {forge})"),
            Mainline::Local => format!("the local {main}"),
        };
        let tip = self.mainline.tip(tree)?;
        if self.action == Self::JUDGE_SURFACE_ACTION
            && let Some(why) = judge_surface_refusal(&dir, tree, &tip, revision)?
        {
            return Ok((
                nothing(format!(
                    "{} at {} is granted, but {why}; nothing recorded",
                    self.action,
                    short(revision)
                )),
                false,
            ));
        }
        let holds = match Mainline::holds(tree, &tip, revision)? {
            Ok(holds) => holds,
            Err(why) => {
                return Ok((
                    nothing(format!(
                        "{main} at {} and the candidate {} do not arrive from this repository as their ids, every object hashed again ({why}); nothing recorded",
                        short(&tip),
                        short(revision)
                    )),
                    false,
                ));
            }
        };
        if !holds {
            return Ok((
                nothing(format!(
                    "{named} is at {}, which does not hold the candidate {}; waiting for its merge",
                    short(&tip),
                    short(revision)
                )),
                true,
            ));
        }
        let artifact = format!("merge-observation-{seq}.txt");
        Ok((
            Observation {
                summary: format!(
                    "{named} at {} holds the candidate {}: merged; retained as {artifact}",
                    short(&tip),
                    short(revision)
                ),
                records: vec![Record::new(
                    format!("merge-observation-{seq}"),
                    Self::KIND,
                    Some("merged".to_owned()),
                    "candidate",
                    revision,
                    self.name,
                )],
                artifact: Some(Artifact {
                    name: artifact,
                    bytes: format!(
                        "forge: {forge}\nmainline: {main}\ntip: {tip}\ncandidate: {revision}\naction: {}\n",
                        self.action
                    )
                    .into_bytes(),
                }),
            },
            false,
        ))
    }
}

impl Validator for CandidateMerged {
    fn name(&self) -> &str {
        self.name
    }

    fn action(&self) -> &str {
        self.action
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

    /// With no forge it waits on the local `main`, which only the operator moves; through a
    /// forge it waits on the forge.
    fn waits_locally(&self) -> bool {
        matches!(self.mainline, Mainline::Local)
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
        let observed = held_to(deadline, self.action, observed.map(|(o, _)| o))?;
        Ok((observed, waiting))
    }
}

/// `binary.promote`: observes, never promotes. Writes one `promotion_record` about `promotion`
/// (the candidate's revision, as the turn opens it), `promoted`, when the case holds the
/// protocol's grant for the action at that revision and the promotion store's latest record that
/// is no refusal ([`crate::promotion::baseline`]) is a promotion (no baseline, no rebaseline)
/// of that revision whose `binary_sha256` is the sha256 of the binary installed now at
/// `<install root>/bin/takt`: `takt promote` installed the binary built from it. While the
/// latest record is another, or names another binary than the one installed, it records nothing
/// and waits, naming why; without the grant it records nothing.
#[derive(Debug, Clone)]
pub struct BinaryPromote {
    /// The state directory; none, and it is the one the promotion store lies in, read when the
    /// validator observes, so no other case needs one.
    pub state: Option<PathBuf>,
    /// The install root `takt promote` installs into; none, and it is the one the turn recorded
    /// when it opened ([`crate::case::install_root_of`]), else
    /// [`crate::promotion::install_root`], read when the validator observes.
    pub install_root: Option<PathBuf>,
}

impl BinaryPromote {
    /// The action it performs.
    pub const ACTION: &str = "binary.promote";
    /// The evidence kind it writes.
    pub const KIND: &str = "promotion_record";
    /// Its name, the producer of its records.
    pub const NAME: &str = "takt.binary-promote";

    /// Reading the state directory `state`.
    #[must_use]
    pub fn in_state(state: &Path) -> Self {
        Self {
            state: Some(state.to_path_buf()),
            install_root: None,
        }
    }

    /// Reading the installed binary under the install root `root`.
    #[must_use]
    pub fn with_install_root(mut self, root: &Path) -> Self {
        self.install_root = Some(root.to_path_buf());
        self
    }

    /// Reading the state directory the promotion store lies in and the install root
    /// `takt promote` installs into, when it observes.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            state: None,
            install_root: None,
        }
    }

    /// Why the store's latest record, `latest`, does not show `revision` promoted and
    /// installed now, or none when it does.
    fn unpromoted(
        latest: Option<&crate::promotion::Promotion>,
        revision: &str,
        installed: &Path,
    ) -> Result<Option<String>> {
        let Some(record) = latest.filter(|record| record.revision == revision) else {
            return Ok(Some(format!(
                "its latest record installs {}, not {}",
                latest.map_or("nothing", |record| short(&record.revision)),
                short(revision)
            )));
        };
        if record.kind != Kind::Promotion {
            return Ok(Some(format!(
                "its latest record of {} is a {:?} record, which installs no binary",
                short(revision),
                record.kind
            )));
        }
        let Some(recorded) = record.binary_sha256.as_deref() else {
            return Ok(Some(format!(
                "its promotion of {} names no binary's sha256",
                short(revision)
            )));
        };
        let digest = match std::fs::read(installed) {
            Ok(bytes) => {
                use sha2::{Digest as _, Sha256};
                format!("{:x}", Sha256::digest(&bytes))
            }
            Err(error) => {
                return Ok(Some(format!(
                    "no binary is installed at {} ({error})",
                    installed.display()
                )));
            }
        };
        if digest != recorded {
            return Ok(Some(format!(
                "{} hashes to {digest}, not to {recorded}, the binary its promotion of {} installed",
                installed.display(),
                short(revision)
            )));
        }
        Ok(None)
    }

    fn observe_by(&self, tree: &Path, revision: &str, seq: usize) -> Result<(Observation, bool)> {
        let dir = crate::case::current(tree)?;
        if !crate::case::holds_grant(&dir, Self::ACTION)? {
            return Ok((
                nothing(format!(
                    "{} is not granted at {}; nothing recorded",
                    Self::ACTION,
                    short(revision)
                )),
                false,
            ));
        }
        let store = state_dir(self.state.as_deref())?.join(crate::promotion::STORE);
        let latest = crate::promotion::baseline(&store, &crate::now())
            .with_context(|| format!("read the promotion store {}", store.display()))?;
        // The install root given, else the one the turn recorded when it opened, else the one
        // `takt promote` takes from the environment.
        let root = match &self.install_root {
            Some(root) => root.clone(),
            None => match crate::case::install_root_of(&dir)? {
                Some(root) => root,
                None => crate::promotion::install_root()?,
            },
        };
        let installed = root.join("bin/takt");
        match Self::unpromoted(latest.as_ref(), revision, &installed)? {
            None => Ok((
                Observation {
                    summary: format!(
                        "the promotion store {}'s latest record installs {}, the binary installed now at {}: promoted",
                        store.display(),
                        short(revision),
                        installed.display()
                    ),
                    records: vec![Record::new(
                        format!("promotion-record-{seq}"),
                        Self::KIND,
                        Some("promoted".to_owned()),
                        "promotion",
                        revision,
                        Self::NAME,
                    )],
                    artifact: None,
                },
                false,
            )),
            Some(why) => Ok((
                nothing(format!(
                    "the promotion store {}, the installed binary read at {}: {why}; promote {} with `takt promote` into that install root; waiting",
                    store.display(),
                    installed.display(),
                    short(revision)
                )),
                true,
            )),
        }
    }
}

impl Validator for BinaryPromote {
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
        "promotion"
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

    /// It waits on the promotion store and the binary installed, which only `takt promote`
    /// run by the operator moves.
    fn waits_locally(&self) -> bool {
        true
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

/// Directories held open by descriptor: every entry below one is read, opened and removed
/// relative to it (`fstatat`, `openat` with `O_NOFOLLOW`, `unlinkat`), never by a path that is
/// resolved again, so a link swapped in after a check is never followed and nothing is removed
/// through one.
#[cfg(unix)]
mod at {
    use std::ffi::{CString, OsStr, OsString};
    use std::io::{Read as _, Write as _};
    use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
    use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
    use std::path::{Path, PathBuf};

    use anyhow::{Context as _, Result, anyhow, bail};

    /// What an entry is, not followed.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum Entry {
        Missing,
        Dir,
        File,
        Link,
        Other,
    }

    /// A directory held open, and the path it was reached by, for messages.
    pub(super) struct Dir {
        fd: OwnedFd,
        pub(super) path: PathBuf,
    }

    /// `name` as one component for a call relative to a directory: no `/`, not `.` or `..`.
    fn component(name: &OsStr) -> Result<CString> {
        let bytes = name.as_bytes();
        if bytes.is_empty() || bytes.contains(&b'/') || bytes == b"." || bytes == b".." {
            bail!("{} is no single path component", name.to_string_lossy());
        }
        CString::new(bytes).map_err(|_| anyhow!("{} holds a NUL", name.to_string_lossy()))
    }

    fn errno() -> std::io::Error {
        std::io::Error::last_os_error()
    }

    impl Dir {
        /// The directory at `path`, following links: the operator's cache root.
        pub(super) fn open(path: &Path) -> Result<Self> {
            let c = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| anyhow!("{} holds a NUL", path.display()))?;
            // SAFETY: `c` is a valid NUL-terminated string for the duration of the call.
            let fd = unsafe {
                libc::open(
                    c.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(errno()).with_context(|| format!("open {}", path.display()));
            }
            // SAFETY: `fd` was just opened and is owned by nothing else.
            Ok(Self {
                fd: unsafe { OwnedFd::from_raw_fd(fd) },
                path: path.to_path_buf(),
            })
        }

        /// What the entry `name` is, not followed.
        pub(super) fn entry(&self, name: &OsStr) -> Result<Entry> {
            let c = component(name)?;
            // SAFETY: a zeroed `stat` is a valid value for `fstatat` to fill.
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: the descriptor is open, `c` is valid and `st` writable.
            let done = unsafe {
                libc::fstatat(
                    self.fd.as_raw_fd(),
                    c.as_ptr(),
                    &raw mut st,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if done != 0 {
                let error = errno();
                if error.kind() == std::io::ErrorKind::NotFound {
                    return Ok(Entry::Missing);
                }
                return Err(error)
                    .with_context(|| format!("read {}", self.path.join(name).display()));
            }
            Ok(match st.st_mode & libc::S_IFMT {
                libc::S_IFDIR => Entry::Dir,
                libc::S_IFREG => Entry::File,
                libc::S_IFLNK => Entry::Link,
                _ => Entry::Other,
            })
        }

        /// The directory `name` under this one, opened without following a link; none when there
        /// is none. A link, or anything but a directory, there is an error naming it: what is
        /// below the cache root is takt's own and holds no link.
        pub(super) fn child(&self, name: &OsStr) -> Result<Option<Self>> {
            let c = component(name)?;
            let path = self.path.join(name);
            // SAFETY: the descriptor is open and `c` is valid.
            let fd = unsafe {
                libc::openat(
                    self.fd.as_raw_fd(),
                    c.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                let error = errno();
                return match error.raw_os_error() {
                    Some(libc::ENOENT) => Ok(None),
                    // A link to a directory fails `O_DIRECTORY | O_NOFOLLOW` with `ENOTDIR`, not
                    // `ELOOP`: what stands there, not followed, names it.
                    Some(libc::ELOOP | libc::ENOTDIR) if self.entry(name)? == Entry::Link => bail!(
                        "{} is a link; a link below the cache root refuses the check, and nothing is removed",
                        path.display()
                    ),
                    Some(libc::ELOOP | libc::ENOTDIR) => bail!(
                        "{} is no directory; the check is refused, and nothing is removed",
                        path.display()
                    ),
                    _ => Err(error).with_context(|| format!("open {}", path.display())),
                };
            }
            // SAFETY: `fd` was just opened and is owned by nothing else.
            Ok(Some(Self {
                fd: unsafe { OwnedFd::from_raw_fd(fd) },
                path,
            }))
        }

        /// [`Self::child`], the directory made (owner only) when there is none.
        pub(super) fn child_made(&self, name: &OsStr) -> Result<Self> {
            let c = component(name)?;
            // SAFETY: the descriptor is open and `c` is valid.
            let made = unsafe { libc::mkdirat(self.fd.as_raw_fd(), c.as_ptr(), 0o700) };
            if made != 0 {
                let error = errno();
                if error.raw_os_error() != Some(libc::EEXIST) {
                    return Err(error)
                        .with_context(|| format!("make {}", self.path.join(name).display()));
                }
            }
            self.child(name)?
                .ok_or_else(|| anyhow!("{} is gone", self.path.join(name).display()))
        }

        /// The bytes of the regular file `name`, opened without following a link; none when it is
        /// missing, a link, or anything but a regular file.
        pub(super) fn regular(&self, name: &OsStr) -> Result<Option<Vec<u8>>> {
            let c = component(name)?;
            // SAFETY: the descriptor is open and `c` is valid.
            let fd = unsafe {
                libc::openat(
                    self.fd.as_raw_fd(),
                    c.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                let error = errno();
                return match error.raw_os_error() {
                    Some(libc::ENOENT | libc::ELOOP) => Ok(None),
                    _ => Err(error)
                        .with_context(|| format!("open {}", self.path.join(name).display())),
                };
            }
            // SAFETY: `fd` was just opened and is owned by nothing else.
            let mut file = std::fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) });
            if !file.metadata()?.is_file() {
                return Ok(None);
            }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .with_context(|| format!("read {}", self.path.join(name).display()))?;
            Ok(Some(bytes))
        }

        /// Writes `bytes` as the new file `name` (mode `mode`), made here and never through a
        /// link: one already there is an error.
        pub(super) fn create(&self, name: &OsStr, bytes: &[u8], mode: u32) -> Result<()> {
            let c = component(name)?;
            // SAFETY: the descriptor is open and `c` is valid.
            let fd = unsafe {
                libc::openat(
                    self.fd.as_raw_fd(),
                    c.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    mode,
                )
            };
            if fd < 0 {
                return Err(errno())
                    .with_context(|| format!("make {}", self.path.join(name).display()));
            }
            // SAFETY: `fd` was just opened and is owned by nothing else.
            let mut file = std::fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) });
            file.write_all(bytes)
                .with_context(|| format!("write {}", self.path.join(name).display()))
        }

        /// Unlinks the entry `name` as itself, a file or a link, never what a link names; none
        /// there is no error. A directory is an error: it is removed only by [`Self::remove`].
        pub(super) fn unlink(&self, name: &OsStr) -> Result<()> {
            let c = component(name)?;
            // SAFETY: the descriptor is open and `c` is valid.
            if unsafe { libc::unlinkat(self.fd.as_raw_fd(), c.as_ptr(), 0) } != 0 {
                let error = errno();
                if error.kind() != std::io::ErrorKind::NotFound {
                    return Err(error)
                        .with_context(|| format!("remove {}", self.path.join(name).display()));
                }
            }
            Ok(())
        }

        /// The names in the directory, `.` and `..` left out.
        pub(super) fn names(&self) -> Result<Vec<OsString>> {
            // SAFETY: duplicating an open descriptor; the duplicate goes to `fdopendir`, which
            // owns it and closes it with `closedir`.
            let dup = unsafe { libc::fcntl(self.fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
            if dup < 0 {
                return Err(errno()).with_context(|| format!("list {}", self.path.display()));
            }
            // SAFETY: `dup` is an open directory descriptor this call now owns.
            let stream = unsafe { libc::fdopendir(dup) };
            if stream.is_null() {
                let error = errno();
                // SAFETY: `fdopendir` failed, so `dup` is still ours to close.
                unsafe { libc::close(dup) };
                return Err(error).with_context(|| format!("list {}", self.path.display()));
            }
            // SAFETY: the stream starts where the shared offset is; rewind it to read it whole.
            unsafe { libc::rewinddir(stream) };
            let mut names = Vec::new();
            loop {
                // SAFETY: `stream` is an open directory stream.
                let entry = unsafe { libc::readdir(stream) };
                if entry.is_null() {
                    break;
                }
                // SAFETY: `readdir` answers an entry whose `d_name` is NUL-terminated.
                let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
                let bytes = name.to_bytes();
                if bytes != b"." && bytes != b".." {
                    names.push(OsString::from_vec(bytes.to_vec()));
                }
            }
            // SAFETY: closing the stream this call opened, which closes `dup`.
            unsafe { libc::closedir(stream) };
            Ok(names)
        }

        /// Removes the entry `name` as itself: a file or a link unlinked, a directory opened
        /// without following a link, emptied the same way and removed. Nothing is removed through
        /// a link, however the entries change meanwhile; none there is no error.
        pub(super) fn remove(&self, name: &OsStr) -> Result<()> {
            match self.child(name) {
                Ok(Some(dir)) => {
                    for entry in dir.names()? {
                        dir.remove(&entry)?;
                    }
                    let c = component(name)?;
                    // SAFETY: the descriptor is open and `c` is valid.
                    if unsafe {
                        libc::unlinkat(self.fd.as_raw_fd(), c.as_ptr(), libc::AT_REMOVEDIR)
                    } != 0
                    {
                        let error = errno();
                        match error.raw_os_error() {
                            Some(libc::ENOENT) => {}
                            Some(libc::ENOTDIR) => self.unlink(name)?,
                            _ => {
                                return Err(error).with_context(|| {
                                    format!("remove {}", self.path.join(name).display())
                                });
                            }
                        }
                    }
                    Ok(())
                }
                Ok(None) => Ok(()),
                Err(_) => self.unlink(name),
            }
        }
    }
}

/// Elsewhere than on unix, nothing is reached through descriptors, and no check runs.
#[cfg(not(unix))]
mod at {
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};

    use anyhow::{Result, bail};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum Entry {
        Missing,
        Dir,
        File,
        Link,
        Other,
    }

    pub(super) struct Dir {
        pub(super) path: PathBuf,
    }

    impl Dir {
        pub(super) fn open(path: &Path) -> Result<Self> {
            bail!(
                "{}: a turn's checks run on a unix system only",
                path.display()
            )
        }
        pub(super) fn entry(&self, _: &OsStr) -> Result<Entry> {
            bail!("unix only")
        }
        pub(super) fn child(&self, _: &OsStr) -> Result<Option<Self>> {
            bail!("unix only")
        }
        pub(super) fn child_made(&self, _: &OsStr) -> Result<Self> {
            bail!("unix only")
        }
        pub(super) fn regular(&self, _: &OsStr) -> Result<Option<Vec<u8>>> {
            bail!("unix only")
        }
        pub(super) fn create(&self, _: &OsStr, _: &[u8], _: u32) -> Result<()> {
            bail!("unix only")
        }
        pub(super) fn unlink(&self, _: &OsStr) -> Result<()> {
            bail!("unix only")
        }
        pub(super) fn names(&self) -> Result<Vec<OsString>> {
            bail!("unix only")
        }
        pub(super) fn remove(&self, _: &OsStr) -> Result<()> {
            bail!("unix only")
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{Scripts, at};

    /// A scratch directory of this test's own, removed on drop.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("takt-improvement-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        Scratch(dir)
    }

    fn git(tree: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(args)
            .current_dir(tree)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// The scripts' directory is removed through the descriptor of the `improvement/` it was
    /// made in: with `improvement` replaced meanwhile by a link to an outside directory that
    /// holds a directory of the same name, nothing behind the link is removed.
    #[test]
    fn the_scripts_directory_is_removed_through_its_parents_descriptor() {
        let dir = scratch("drop");
        let tree = dir.0.join("tree");
        for script in ["check-finding", "file-finding"] {
            let path = tree.join("scripts").join(script);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("scripts");
            std::fs::write(&path, "#!/bin/sh\n").expect("a script");
        }
        git(&tree, &["init", "-q", "-b", "main"]);
        git(&tree, &["add", "-A"]);
        git(&tree, &["commit", "-q", "-m", "scripts"]);
        let head = git(&tree, &["rev-parse", "HEAD"]);
        let cache = dir.0.join("cache");
        let scripts = Scripts::at(&tree, &head, &cache, None)
            .expect("the scripts are written")
            .expect("the base holds them");
        let improvement = cache.join("takt/improvement");
        std::fs::rename(&improvement, cache.join("takt/improvement-moved")).expect("moved");
        let outside = dir.0.join("outside");
        let precious = outside.join(&scripts.name).join("precious.txt");
        std::fs::create_dir_all(precious.parent().expect("a parent")).expect("outside");
        std::fs::write(&precious, "keep me\n").expect("precious");
        std::os::unix::fs::symlink(&outside, &improvement).expect("the link");
        let held = cache.join("takt/improvement-moved").join(&scripts.name);
        drop(scripts);
        assert!(precious.is_file(), "removed through the link");
        assert!(!held.exists(), "the scripts' own directory is removed");
    }

    /// A kept build that is a link is read as none: its target's bytes never stand for it.
    #[test]
    fn a_link_reads_as_no_regular_file() {
        let dir = scratch("regular");
        std::fs::write(dir.0.join("target"), "a binary\n").expect("a target");
        std::os::unix::fs::symlink(dir.0.join("target"), dir.0.join("takt")).expect("the link");
        std::fs::write(dir.0.join("file"), "a binary\n").expect("a file");
        let held = at::Dir::open(&dir.0).expect("the directory");
        assert_eq!(held.regular("takt".as_ref()).expect("read"), None);
        assert_eq!(
            held.regular("file".as_ref()).expect("read"),
            Some(b"a binary\n".to_vec())
        );
    }
}
