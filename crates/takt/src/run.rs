//! The loop: evaluate, frontier, one step, effect, observation, validator, evidence, evaluate;
//! until an outcome is legitimate, an approval is needed, nothing is admissible, a step is
//! refused, or the stop criteria are met. A run opens a case, or resumes one a run before it
//! opened: a case stopped at the gate is resumed once the operator has granted it, and the
//! merge is then a deterministic step like any other.
//!
//! Beside each printed line the run appends an event to the case's `events.jsonl`
//! ([`crate::events`]); a line about a judgement step is written once the step's change is
//! kept or put back, so putting the case directory back never takes a line with it. A story's
//! run given the planning store ([`Run::record_on`]) records its test result and its move to
//! `implemented` there: just before the step a grant authorises (the merge), so both ride to the
//! base with it above the granted revision, the move last ([`crate::case::approvals_only`]); or
//! once its case terminates `accepted`, when there was no such step.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};

use crate::case::{CASES_DIR, CaseDir, KEPT_MOVE, KEPT_TEST_RESULT};
use crate::events::{BY_STORE, BY_TAKT, Event, Events};
use crate::frontier::{Frontier, IMPLEMENTED, Story, TEST_RESULT, Take, Unit};
use crate::harness::{Harness, StepContext, is_test_path};
use crate::protocol::LoadedProtocol;
use crate::validator::Validator;

/// What a run is given.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// The protocol id, `software.change/1`, or a `protocol/1` file path.
    pub protocol: String,
    /// The intent.
    pub intent: String,
    /// Case inputs, `name=value`.
    pub inputs: Vec<(String, String)>,
    /// Stop after this many steps.
    pub iterations: Option<u64>,
    /// Stop after this many hours.
    pub hours: Option<f64>,
    /// Stop once the harness reported this many tokens.
    pub tokens: Option<u64>,
    /// The case id; generated from the time when absent.
    pub case_id: Option<String>,
    /// The approval gate: the one approval-required action the harness may request to end the
    /// run, `repository.merge` for a software change. Other approval-required actions are not
    /// offered.
    pub gate: Option<String>,
}

/// Why a run stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    /// An outcome is legitimate: the case is complete through it.
    Completed(String),
    /// Only actions that need an approval are left.
    ApprovalRequired(Vec<String>),
    /// No action is admissible and none needs an approval.
    NothingAdmissible,
    /// The step budget is spent.
    Iterations(u64),
    /// The time budget is spent.
    Hours(f64),
    /// The token budget is spent.
    Tokens(u64),
    /// The harness did something the frontier did not admit; its change was discarded.
    StepRefused(String),
    /// A judgement step changed no file and the decision did not move: another step would not either.
    NoProgress,
    /// The run waited on the forge (checks still running, a pull request's head behind the head
    /// pushed) and may wait no longer: its step or hour budget is spent, or it has neither to
    /// wait in. Nothing is recorded for what is awaited; the case stays open, and the next run
    /// resumes it and observes again.
    Waiting {
        /// The actions whose observation waits on the forge.
        on: Vec<String>,
        /// The budget spent; `None` for a run with no step or hour budget, which does not wait.
        budget: Option<Box<Stop>>,
    },
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stop::Completed(outcome) => write!(f, "Completed ({outcome})"),
            Stop::ApprovalRequired(actions) => {
                write!(f, "ApprovalRequired ({})", actions.join(", "))
            }
            Stop::NothingAdmissible => write!(f, "NothingAdmissible"),
            Stop::Iterations(n) => write!(f, "Iterations ({n})"),
            Stop::Hours(h) => write!(f, "Hours ({h})"),
            Stop::Tokens(t) => write!(f, "Tokens ({t})"),
            Stop::StepRefused(why) => write!(f, "StepRefused ({why})"),
            Stop::NoProgress => write!(f, "NoProgress"),
            Stop::Waiting { on, budget } => match budget {
                Some(budget) => write!(f, "Waiting ({}; {budget})", on.join(", ")),
                None => write!(f, "Waiting ({}; no step or hour budget)", on.join(", ")),
            },
        }
    }
}

/// The outcome of a case whose story is recorded and moved to `implemented` in the store.
pub const ACCEPTED: &str = "accepted";

/// The first eight characters of a revision, as the loop prints one.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// Who took a step, with what its event records of it: a validator, and the revision it
/// observed when it observed; or the harness, the tokens it reported and their components.
enum SteppedBy<'r> {
    Validator(Option<&'r str>),
    Harness(Option<u64>, Option<crate::events::TokenUsage>),
}

/// Whether `validator` is due at a decision: its action is admissible, its evidence is missing
/// at its subject's current revision, and it has not already observed there without a record.
/// An evidence record that does not read is an error naming its file, never an answer.
fn is_due(
    case: &CaseDir,
    admissible: &[String],
    observed_without_record: &BTreeSet<(String, String)>,
    validator: &dyn Validator,
) -> Result<bool> {
    if !admissible.iter().any(|a| a == validator.action()) {
        return Ok(false);
    }
    if case.has_evidence(validator.kind(), validator.subject())? {
        return Ok(false);
    }
    Ok(!case.revision(validator.subject()).is_some_and(|revision| {
        observed_without_record.contains(&(validator.name().to_owned(), revision))
    }))
}

/// One entry of a snapshot: a regular file with its bytes and whether it is executable, a
/// symbolic link with its target, or anything else (a FIFO, a socket, a device) by its kind
/// alone. A link is an entry by itself and is never followed: what it points at may lie outside
/// the tree, and nothing the kernel does for a case reaches outside the tree. Anything else is
/// never opened: reading a FIFO no one writes would block the run for good.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    File { bytes: Vec<u8>, executable: bool },
    Link(PathBuf),
    Other(&'static str),
}

/// What a file that is not a regular file, a directory or a link is.
fn other_kind(kind: std::fs::FileType) -> &'static str {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt as _;
        if kind.is_fifo() {
            return "fifo";
        }
        if kind.is_socket() {
            return "socket";
        }
        if kind.is_block_device() {
            return "block device";
        }
        if kind.is_char_device() {
            return "character device";
        }
    }
    let _ = kind;
    "other"
}

/// Whether `meta` gives its file an execute bit.
#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode() & 0o111 != 0
}

/// Whether `meta` gives its file an execute bit: never on this platform.
#[cfg(not(unix))]
fn is_executable(_: &std::fs::Metadata) -> bool {
    false
}

/// Every file, link and other entry under `dir`, by path relative to it: the cases directory
/// as it is before a judgement step, which the step may not touch.
fn snapshot(dir: &Path) -> Result<BTreeMap<PathBuf, Entry>> {
    snapshot_without(dir, None)
}

/// [`snapshot`] of `dir` without what lies under `without`, a path relative to it.
fn snapshot_without(dir: &Path, without: Option<&Path>) -> Result<BTreeMap<PathBuf, Entry>> {
    fn walk(
        root: &Path,
        dir: &Path,
        without: Option<&Path>,
        entries: &mut BTreeMap<PathBuf, Entry>,
    ) -> Result<()> {
        for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
            let path = entry?.path();
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if without.is_some_and(|without| relative.starts_with(without)) {
                continue;
            }
            let meta = std::fs::symlink_metadata(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let kind = meta.file_type();
            if kind.is_symlink() {
                let target = std::fs::read_link(&path)
                    .with_context(|| format!("read the link {}", path.display()))?;
                entries.insert(relative, Entry::Link(target));
            } else if kind.is_dir() {
                walk(root, &path, without, entries)?;
            } else if kind.is_file() {
                let bytes =
                    std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
                entries.insert(
                    relative,
                    Entry::File {
                        bytes,
                        executable: is_executable(&meta),
                    },
                );
            } else {
                entries.insert(relative, Entry::Other(other_kind(kind)));
            }
        }
        Ok(())
    }
    let mut entries = BTreeMap::new();
    if std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        walk(dir, dir, without, &mut entries)?;
    }
    Ok(entries)
}

/// The paths at which two snapshots differ: added, removed or changed, in path order.
fn differing(before: &BTreeMap<PathBuf, Entry>, after: &BTreeMap<PathBuf, Entry>) -> Vec<PathBuf> {
    before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Makes the link `at` pointing at `target`.
#[cfg(unix)]
fn make_link(target: &Path, at: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, at)
        .with_context(|| format!("put back the link {}", at.display()))
}

/// Makes the link `at` pointing at `target`: not on this platform.
#[cfg(not(unix))]
fn make_link(target: &Path, at: &Path) -> Result<()> {
    bail!(
        "the link {} to {} cannot be put back on this platform",
        at.display(),
        target.display()
    )
}

/// Writes `bytes` to `at` and gives it an execute bit or takes it away, as `executable` says.
fn put_back_file(at: &Path, bytes: &[u8], executable: bool) -> Result<()> {
    std::fs::write(at, bytes).with_context(|| format!("put back {}", at.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = std::fs::metadata(at)
            .with_context(|| format!("read {}", at.display()))?
            .permissions();
        let mode = permissions.mode();
        permissions.set_mode(if executable {
            mode | ((mode & 0o444) >> 2)
        } else {
            mode & !0o111
        });
        std::fs::set_permissions(at, permissions)
            .with_context(|| format!("put back the mode of {}", at.display()))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

/// Puts `dir` back to `before` at the paths `changed`: whatever is at the path now is removed
/// (a link as the link itself, never what it points at; a directory the step made in a file's
/// place with everything in it), then the file is written again as it was, its mode with it,
/// or the link made again; a path the step added stays removed, and an entry that was neither
/// a file nor a link (a FIFO) cannot be made again and stays removed too. The paths go in
/// order, so a link or file the step put in place of a directory is gone before anything is
/// written below it.
fn restore(dir: &Path, before: &BTreeMap<PathBuf, Entry>, changed: &[PathBuf]) -> Result<()> {
    if std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!(
            "the directory {} was replaced by a link; nothing is put back through it",
            dir.display()
        );
    }
    for path in changed {
        let full = dir.join(path);
        if let Ok(meta) = std::fs::symlink_metadata(&full) {
            if meta.is_dir() {
                std::fs::remove_dir_all(&full)
            } else {
                std::fs::remove_file(&full)
            }
            .with_context(|| format!("remove {}", full.display()))?;
        }
        if let Some(parent) = full.parent()
            && before.contains_key(path)
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        match before.get(path) {
            Some(Entry::File { bytes, executable }) => put_back_file(&full, bytes, *executable)?,
            Some(Entry::Link(target)) => make_link(target, &full)?,
            Some(Entry::Other(_)) | None => {}
        }
    }
    Ok(())
}

/// The directory under the tree that holds the planning store, the project file that says
/// where the store is kept, and the cases: everything in it but the cases is the store's, which
/// no judgement step writes.
const ENGINEERING: &str = ".engineering";

/// Whether `path`, relative to the tree as git names it, is the store's: under [`ENGINEERING`]
/// (or that directory itself) and not under the cases directory.
fn is_guarded_store_path(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    (path == ENGINEERING || path.starts_with(&format!("{ENGINEERING}/")))
        && !crate::case::is_under_cases(path)
}

/// Everything under [`ENGINEERING`] but the cases directory, as [`snapshot`] takes it: what a
/// step that wrote there is put back to.
fn snapshot_store(tree: &Path) -> Result<BTreeMap<PathBuf, Entry>> {
    let cases = Path::new(CASES_DIR)
        .strip_prefix(ENGINEERING)
        .unwrap_or(Path::new(CASES_DIR));
    snapshot_without(&tree.join(ENGINEERING), Some(cases))
}

/// Puts everything under [`ENGINEERING`] but the cases directory back to `kept`, where it still
/// differs.
fn put_back_store(tree: &Path, kept: &BTreeMap<PathBuf, Entry>) -> Result<()> {
    let again = differing(kept, &snapshot_store(tree)?);
    if !again.is_empty() {
        restore(&tree.join(ENGINEERING), kept, &again)?;
    }
    Ok(())
}

/// Discards everything a judgement step did, back to `checkpoint`, and puts the cases directory
/// back to `kept` once more: a reset can remove a case file the step had staged.
fn undo_step(
    tree: &Path,
    checkpoint: &crate::git::Checkpoint,
    cases: &Path,
    kept: &BTreeMap<PathBuf, Entry>,
) -> Result<()> {
    checkpoint
        .restore(tree)
        .context("discard the step's change")?;
    let again = differing(kept, &snapshot(cases)?);
    if !again.is_empty() {
        restore(cases, kept, &again)?;
    }
    Ok(())
}

/// The subject of the commit that records a take on the case's branch: `takt: take <story>`.
#[must_use]
pub fn take_subject(story: &str) -> String {
    format!("takt: take {story}")
}

/// The stories whose case is under `tree` and whose take is committed on that case's branch,
/// in case order: the stories a case is being worked for, wherever the store's own copy says
/// otherwise (the base's copy lists a story `proposed` until its case's branch is merged).
fn taken_on_case_branches(tree: &Path) -> Result<Vec<String>> {
    let mut taken = Vec::new();
    for (case, story) in cases_with_story(tree)? {
        let branch = format!("refs/heads/{}", crate::case::Branch::name_of(&case));
        if crate::git::has_subject(tree, &branch, &take_subject(&story))? && !taken.contains(&story)
        {
            taken.push(story);
        }
    }
    Ok(taken)
}

/// Every case under `tree` that records a story, as `(case id, story id)` in case order.
fn cases_with_story(tree: &Path) -> Result<Vec<(String, String)>> {
    let cases = tree.join(CASES_DIR);
    if !cases.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&cases)
        .with_context(|| format!("read {}", cases.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    dirs.sort();
    let mut found = Vec::new();
    for dir in dirs {
        if !dir.join("case.yaml").is_file() {
            continue;
        }
        if let Some(story) = crate::case::story_of(&dir)? {
            let id = dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            found.push((id, story));
        }
    }
    Ok(found)
}

/// Whether the tree is on a case's branch, `takt/<id>`: where [`to_base`] moves it, or refuses.
fn on_a_case_branch(tree: &Path) -> Result<bool> {
    Ok(crate::git::current_branch(tree)?
        .is_some_and(|branch| branch.starts_with(crate::case::BRANCH_PREFIX)))
}

/// Puts the tree on the base of the case whose branch it is on, so that the store is read as
/// the base has it, never as a case's branch, which carries a model's commits, has it. A tree
/// on any other branch, or detached, stays where it is. Refused when the case's base is not
/// known, or when the tree has uncommitted work, which the checkout would carry along.
fn to_base(tree: &Path) -> Result<()> {
    let Some(branch) = crate::git::current_branch(tree)? else {
        return Ok(());
    };
    let Some(case) = branch.strip_prefix(crate::case::BRANCH_PREFIX) else {
        return Ok(());
    };
    let Some(base) = crate::case::Branch::read(&tree.join(CASES_DIR).join(case))
        .ok()
        .and_then(|recorded| recorded.base)
    else {
        bail!(
            "the tree {} is on {branch}, a case's branch with no base recorded; the store is read from the base, never from a case's branch: check out the branch to take the next story from",
            tree.display()
        );
    };
    let uncommitted = crate::git::uncommitted(tree)?;
    if !uncommitted.is_empty() {
        bail!(
            "the tree {} is on {branch} with uncommitted work: {}; the next story is read from {base}, and checking it out would carry the work along: commit it, or move it out of the tree, first",
            tree.display(),
            uncommitted.join(", ")
        );
    }
    crate::git::run(tree, &["checkout", "-q", &base])
        .with_context(|| format!("check out {base} to read the store from"))?;
    Ok(())
}

/// A run over one case.
pub struct Run<'a> {
    /// The worktree.
    pub tree: PathBuf,
    /// The case.
    pub case: CaseDir,
    /// What the run works on: the intent given, or the story the frontier took.
    pub unit: Unit,
    /// The judgement actor.
    pub harness: &'a dyn Harness,
    /// The validators, one per deterministic action.
    pub validators: &'a [Box<dyn Validator>],
    /// Steps taken.
    pub steps: u64,
    /// Tokens the harness reported.
    pub tokens: u64,
    /// How long the run waits on the forge before it observes again: [`Run::POLL`] unless set.
    pub poll: Duration,
    /// The planning store a story's run records its test result and its move in; none until
    /// [`Run::record_on`] gives one, and only a story's run writes to it.
    store: Option<&'a dyn Frontier>,
    started: Instant,
    options: RunOptions,
}

impl<'a> Run<'a> {
    /// How long a run waits on the forge before it observes again, by default: thirty seconds.
    pub const POLL: Duration = Duration::from_secs(30);

    /// Opens the case on the intent in `options` and prepares the run.
    pub fn open(
        tree: &Path,
        options: RunOptions,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        let unit = Unit::Intent(options.intent.clone());
        Self::open_unit(tree, options, unit, harness, validators)
    }

    /// Opens the case on `unit` and prepares the run. For a story, `intent.md` is the story's
    /// title followed by its body, and the case id, unless `options` gives one, is derived from
    /// the story id and the instant.
    pub fn open_unit(
        tree: &Path,
        mut options: RunOptions,
        unit: Unit,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        match &unit {
            Unit::Intent(intent) => options.intent = intent.clone(),
            Unit::Story(story) => {
                options.intent = story.intent();
                if options.case_id.is_none() {
                    options.case_id = Some(story.case_id(&crate::now()));
                }
            }
        }
        let protocol = if Path::new(&options.protocol).is_file() {
            LoadedProtocol::from_file(Path::new(&options.protocol))?
        } else {
            LoadedProtocol::builtin(&options.protocol)?
        };
        let id = options
            .case_id
            .clone()
            .unwrap_or_else(|| format!("takt-{}", crate::now().replace([':', '-'], "")));
        let case = CaseDir::open(tree, protocol, &id, &options.intent, &options.inputs)?;
        // A case with no base cannot open its pull request. With a validator for `pr.open`
        // registered that is known now, so the run is refused before its first step rather
        // than after the tests, the gate and the harness have run; the case is discarded.
        if validators.iter().any(|v| v.action() == "pr.open")
            && crate::case::Branch::read(&case.dir)?.base.is_none()
        {
            case.discard()?;
            bail!(
                "the case {id} has no base branch to open its pull request against: the tree is on a detached HEAD; check out the branch the pull request is against, or run without a forge"
            );
        }
        // A case opened for a story records it: the next run on the repository finds the story
        // `active` with this case open and resumes it, never opens a second.
        if let Unit::Story(story) = &unit
            && let Err(error) = case.set_story(&story.id)
        {
            case.discard()?;
            return Err(error);
        }
        Ok(Self {
            tree: tree.to_path_buf(),
            case,
            unit,
            harness,
            validators,
            steps: 0,
            tokens: 0,
            poll: Self::POLL,
            store: None,
            started: Instant::now(),
            options,
        })
    }

    /// Gives the run the planning store `store`: a story's run records there the `test_result`
    /// pass record a validator of the case wrote, at the record's own `observed_at`, and moves
    /// the story to `implemented`, each a commit on the case's branch
    /// ([`crate::frontier::write_on_case_branch`]), before the step the case's grant authorises
    /// or, without one, once the case terminates `accepted`; a refusal is relayed and the stop
    /// is unchanged. A run on an intent has no story and writes nothing there.
    pub fn record_on(&mut self, store: &'a dyn Frontier) {
        self.store = Some(store);
    }

    /// Prepares a run on the case `id` a run before this one opened under `tree`: the case's
    /// files as they are, its `authority.yaml` read into every evaluation, its intent and inputs
    /// the run's. The unit is the story the case records, or the intent it was opened on.
    /// Nothing is opened and nothing discarded; the protocol, inputs and case id in `options`
    /// are not read: the case's own rule.
    pub fn resume(
        tree: &Path,
        mut options: RunOptions,
        id: &str,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        let case = CaseDir::resume(tree, id)?;
        let intent = case.intent()?;
        let unit = match case.story()? {
            Some(story) => {
                // `intent.md` of a story's case is its title, a blank line, its body.
                let (title, body) = intent.split_once("\n\n").unwrap_or((intent.as_str(), ""));
                Unit::Story(Story {
                    id: story,
                    title: title.to_owned(),
                    body: body.to_owned(),
                })
            }
            None => Unit::Intent(intent.clone()),
        };
        options.intent = intent;
        options.inputs = case.inputs();
        options.case_id = Some(id.to_owned());
        options.protocol = case.protocol.id.clone();
        Ok(Self {
            tree: tree.to_path_buf(),
            case,
            unit,
            harness,
            validators,
            steps: 0,
            tokens: 0,
            poll: Self::POLL,
            store: None,
            started: Instant::now(),
            options,
        })
    }

    /// Resumes the open case [`Run::next_to_resume`] names, if it names one; else opens the case
    /// on the next ready story, the store told last ([`Run::take_next`]). One case directory per
    /// taken story, none for a refused one, never a second for a story with one.
    pub fn open_next(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Option<Self>> {
        if let Some((case_id, story_id)) = Self::next_to_resume(tree, frontier, out)? {
            return Self::resumed(tree, options, &case_id, &story_id, harness, validators, out)
                .map(Some);
        }
        Self::take_next(tree, options, frontier, harness, validators, out)
    }

    /// The open case [`Run::open_next`] resumes, as `(case id, story id)`, answered with nothing
    /// changed: no checkout, no write, so a caller can refuse what was given for that case
    /// before anything is resumed, and then resume exactly it. The case of a story being
    /// worked: first one whose take is committed on its case's branch, whatever branch the
    /// tree is on; then one whose story the store lists `active`, read only from a tree that is
    /// not on a case's branch, so a status never comes from a branch a model committed on (a
    /// tree on a case's branch is put on its base by [`Run::take_next`], and the next run reads
    /// the base's list there). Among those, the first in case order that is not finished; a
    /// decision that does not read is said on `out` and the case taken as open. `None` when
    /// there is nothing to resume.
    pub fn next_to_resume(
        tree: &Path,
        frontier: &dyn Frontier,
        out: &mut dyn Write,
    ) -> Result<Option<(String, String)>> {
        let taken = taken_on_case_branches(tree)?;
        if let Some(found) = crate::case::resumable(tree, &taken, out)? {
            return Ok(Some(found));
        }
        if on_a_case_branch(tree)? {
            return Ok(None);
        }
        let active = frontier.active(tree)?;
        crate::case::resumable(tree, &active, out)
    }

    /// Opens the case on the next ready story, the store told last, and never resumes one: for
    /// a caller that asked [`Run::next_to_resume`] first and was told there is nothing to
    /// resume. The store is read from the base: a tree on a case's branch is put on that case's
    /// base first, so neither a story's text nor its status ever comes from a branch a model
    /// committed on. For each candidate in order, the case is opened first (on a clean tree,
    /// the protocol loaded, the inputs checked), then the story is taken, and the take
    /// committed on the case's branch, exactly the paths the store changed, as `takt: take
    /// <story>`: a refused step that discards the tree's change keeps it, and the case's
    /// implementation revision is that commit, the one its validators observe. A refusal
    /// removes the case just opened, is printed on `out` verbatim, and the next candidate is
    /// tried; a take or its commit that fails removes it too and is the error. A candidate a
    /// case already records is not taken again: it is skipped with a line on `out`. `None`,
    /// with no case opened, when nothing else is ready, such a skipped case included.
    pub fn take_next(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Option<Self>> {
        to_base(tree)?;
        let cased = cases_with_story(tree)?;
        for id in frontier.candidates(tree)? {
            if let Some((case, _)) = cased.iter().find(|(_, story)| *story == id) {
                writeln!(
                    out,
                    "frontier: {id} not taken: the case {case} was opened for it already"
                )?;
                continue;
            }
            let story = frontier.show(tree, &id)?;
            let title = story.title.clone();
            let mut run = Self::open_unit(
                tree,
                options.clone(),
                Unit::Story(story),
                harness,
                validators,
            )?;
            let taken = match frontier.take(tree, &id) {
                Ok(taken) => taken,
                Err(error) => return Err(Self::discarded(run, error)),
            };
            match taken {
                Take::Taken => {
                    let changed = match crate::git::uncommitted(tree) {
                        Ok(changed) => changed,
                        Err(error) => return Err(Self::discarded(run, error)),
                    };
                    let head = match crate::git::commit_paths(tree, &changed, &take_subject(&id)) {
                        Ok(head) => head,
                        Err(error) => return Err(Self::discarded(run, error)),
                    };
                    if run.case.revision("implementation").is_some() {
                        run.case.set_revision("implementation", &head)?;
                    }
                    writeln!(out, "frontier: took {id}: {title}")?;
                    return Ok(Some(run));
                }
                Take::Refused(refusal) => {
                    // The case goes whole: its directory, the cases directory when it was the
                    // last, and its branch, the tree back on the branch the case left.
                    let dir = run.case.dir.clone();
                    run.case.discard().with_context(|| {
                        format!("remove the case {} the store would not take", dir.display())
                    })?;
                    writeln!(out, "frontier: {id} not taken: {refusal}")?;
                }
            }
        }
        Ok(None)
    }

    /// Resumes the case `case_id` of the story `story_id` and says so on `out`.
    fn resumed(
        tree: &Path,
        options: RunOptions,
        case_id: &str,
        story_id: &str,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Self> {
        let run = Self::resume(tree, options, case_id, harness, validators)?;
        let title = match &run.unit {
            Unit::Story(story) => story.title.clone(),
            Unit::Intent(intent) => intent.clone(),
        };
        writeln!(
            out,
            "frontier: resumed {story_id}: {title} (case {case_id})"
        )?;
        Ok(run)
    }

    /// Removes the case `run` just opened, whose take failed with `error`, and answers the
    /// error; a removal that fails too is added to it.
    fn discarded(run: Self, error: anyhow::Error) -> anyhow::Error {
        let dir = run.case.dir.clone();
        match run.case.discard() {
            Ok(()) => error,
            Err(removal) => error.context(format!(
                "and the case {} could not be removed: {removal:#}",
                dir.display()
            )),
        }
    }

    /// The branches a checkpoint guards even when another worktree has them checked out: the
    /// case's own branch and its base, where `take_next` reads the store from and the next case
    /// forks from.
    fn kept_branches(&self) -> Vec<String> {
        let mut kept = vec![self.case.branch.clone()];
        kept.extend(
            crate::case::Branch::read(&self.case.dir)
                .ok()
                .and_then(|branch| branch.base),
        );
        kept
    }

    fn budget_spent(&self) -> Option<Stop> {
        if let Some(n) = self.options.iterations
            && self.steps >= n
        {
            return Some(Stop::Iterations(n));
        }
        // An hour budget that is not a number is spent: no comparison would ever end it.
        if let Some(h) = self.options.hours
            && (h.is_nan() || self.started.elapsed().as_secs_f64() >= h * 3600.0)
        {
            return Some(Stop::Hours(h));
        }
        if let Some(t) = self.options.tokens
            && self.tokens >= t
        {
            return Some(Stop::Tokens(t));
        }
        None
    }

    /// How long the run waits on the forge before it observes again: [`Run::poll`], cut to
    /// what is left of the hour budget. `None` for a run with neither a step nor an hour
    /// budget: it has nothing to wait in, and a wait without a bound would never end. What is
    /// left of an hour budget too large for a [`Duration`], infinite among them, saturates and
    /// leaves the poll as it is; nothing here panics.
    fn pause(&self) -> Option<Duration> {
        if self.options.iterations.is_none() && self.options.hours.is_none() {
            return None;
        }
        Some(match self.options.hours {
            Some(h) => {
                let left = (h * 3600.0 - self.started.elapsed().as_secs_f64()).max(0.0);
                self.poll
                    .min(Duration::try_from_secs_f64(left).unwrap_or(Duration::MAX))
            }
            None => self.poll,
        })
    }

    /// Runs until a stop; prints one line per frontier, step and evidence to `out`, and appends
    /// one event per such fact to the case's `events.jsonl`, the stop, or the error the run
    /// ended on, last. At an `accepted` stop a story's run given the store records there first
    /// ([`Run::record_on`]); what goes wrong there is recorded as an `error` line and the stop
    /// is kept. Every run's record ends on its `stop` line, or on an `error` line when the run
    /// ends on an error, a failing `out` among them. The run holds the case while it runs
    /// (`run.lock` under the case directory), so no grant lands and no second run starts on it
    /// meanwhile; it lets go at every stop and on error.
    pub fn run(&mut self, out: &mut dyn Write) -> Result<Stop> {
        self.case.hold()?;
        let events = Events::new(&self.case.dir);
        // A panic in the loop (a harness or a validator that broke) is recorded as the run's
        // `error` line and the case let go before the panic goes on: the record ends, and the
        // case is not left held.
        let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_held(&events, out)
                .and_then(|stop| self.close(&events, &stop, out).map(|()| stop))
        })) {
            Ok(outcome) => outcome,
            Err(panic) => {
                let what = panic
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "a panic with no message".to_owned());
                let _ = events.emit(
                    self.steps,
                    &Event::Error {
                        error: format!("the run panicked: {what}"),
                    },
                );
                let _ = self.case.release();
                std::panic::resume_unwind(panic);
            }
        };
        let outcome = match outcome {
            Ok(stop) => Ok(stop),
            Err(error) => Err(
                match events.emit(
                    self.steps,
                    &Event::Error {
                        error: format!("{error:#}"),
                    },
                ) {
                    Ok(()) => error,
                    Err(unrecorded) => error.context(format!(
                        "and the error could not be recorded: {unrecorded:#}"
                    )),
                },
            ),
        };
        let released = self.case.release();
        let stop = outcome?;
        released?;
        Ok(stop)
    }

    /// After the loop stopped: the store's writes of a story's case that terminated
    /// [`ACCEPTED`], then the stop recorded. Anything that goes wrong with the store's writes
    /// (an `events.jsonl` that does not read, an `out` that fails) is recorded as an `error`
    /// line before the stop, which stays the one the case reached.
    fn close(&self, events: &Events, stop: &Stop, out: &mut dyn Write) -> Result<()> {
        if let Stop::Completed(outcome) = stop
            && outcome == ACCEPTED
            && let (Unit::Story(story), Some(store)) = (&self.unit, self.store)
            && let Err(error) = self.record_accepted(events, store, &story.id, out)
        {
            events.emit(
                self.steps,
                &Event::Error {
                    error: format!(
                        "recording the accepted case on {} in the store: {error:#}",
                        story.id
                    ),
                },
            )?;
        }
        events.emit(
            self.steps,
            &Event::Stop {
                stop: stop.to_string(),
                steps: self.steps,
                tokens: self.tokens,
            },
        )
    }

    /// Records the case's story in `store`, before the step the grant authorises or once the
    /// case is accepted: the `test_result` pass record a validator of the case wrote
    /// ([`Run::observed_pass`]) as `test_result` evidence at its own `observed_at`, then the move
    /// to `implemented`, which the store judges, the last; each a commit on the case's branch
    /// with what it committed kept in the case directory ([`crate::case::keep_rider`]) and its
    /// line in the case's record, written with the commit: either that cannot be written takes
    /// the commit back ([`crate::frontier::write_on_case_branch`]). A write kept at the case's
    /// current revision and committed above it is not made again; one of an earlier revision
    /// is. A refusal, the store's verbatim or takt's own, is recorded, then printed, and nothing
    /// is retried. What is printed comes after what is recorded: an `out` that fails skips no
    /// line and no write, and is the error once both writes were tried.
    fn record_accepted(
        &self,
        events: &Events,
        store: &dyn Frontier,
        story: &str,
        out: &mut dyn Write,
    ) -> Result<()> {
        let recorded = events.read()?;
        let branch = &self.case.branch;
        let on_branch = format!("refs/heads/{branch}");
        // A write is done at the case's current revision only: kept under it and committed above
        // it. One made at an earlier revision is no record of this one and is made again.
        let revision = self.case.revision("implementation");
        let done = |name: &str, subject: &str| -> Result<bool> {
            let Some(revision) = revision.as_deref() else {
                return Ok(false);
            };
            Ok(crate::case::is_kept_at(&self.case.dir, revision, name)
                && crate::git::has_subject(
                    &self.tree,
                    &format!("{revision}..{on_branch}"),
                    subject,
                )?)
        };
        let committed = |commit: &Option<String>| match commit {
            Some(commit) => format!(" (committed {} on {branch})", short(commit)),
            None => String::new(),
        };
        let mut unprinted: Option<std::io::Error> = None;
        let mut say = |text: String| {
            if unprinted.is_none()
                && let Err(error) = writeln!(out, "{text}")
            {
                unprinted = Some(error);
            }
        };
        let evidence_subject = crate::frontier::test_result_subject(story);
        if !done(KEPT_TEST_RESULT, &evidence_subject)? {
            let written = self.observed_pass(&recorded).and_then(|observed| {
                let Some((reference, at)) = observed else {
                    return Err(anyhow::anyhow!(
                        "no {TEST_RESULT} pass record a validator of the case wrote is about the current revision; nothing was recorded"
                    ));
                };
                crate::frontier::write_on_case_branch(
                    &self.tree,
                    branch,
                    story,
                    &evidence_subject,
                    || store.record_test_result(&self.tree, story, &reference, &at),
                    |commit| {
                        self.kept_and_recorded(commit, KEPT_TEST_RESULT, || {
                            events.emit(
                                self.steps,
                                &Event::StoreEvidence {
                                    story: story.to_owned(),
                                    kind: TEST_RESULT.to_owned(),
                                    reference: reference.clone(),
                                    observed_at: at.clone(),
                                    commit: commit.map(str::to_owned),
                                },
                            )
                        })
                    },
                )
                .map(|written| written.map(|commit| (reference, at, commit)))
            });
            match written {
                Ok(Ok((reference, at, commit))) => say(format!(
                    "store: {TEST_RESULT} recorded on {story} at {at} from {reference}{}",
                    committed(&commit)
                )),
                Ok(Err(refusal)) => {
                    self.refused(events, BY_STORE, &refusal)?;
                    say(format!(
                        "store: {TEST_RESULT} not recorded on {story}: {refusal}"
                    ));
                }
                Err(error) => {
                    let why = format!("{error:#}");
                    self.refused(events, BY_TAKT, &why)?;
                    say(format!(
                        "store: {TEST_RESULT} not recorded on {story}: {why}"
                    ));
                }
            }
        }
        let move_subject = crate::frontier::implemented_subject(story);
        if !done(KEPT_MOVE, &move_subject)? {
            match crate::frontier::write_on_case_branch(
                &self.tree,
                branch,
                story,
                &move_subject,
                || store.implemented(&self.tree, story),
                |commit| {
                    self.kept_and_recorded(commit, KEPT_MOVE, || {
                        events.emit(
                            self.steps,
                            &Event::StoreMove {
                                story: story.to_owned(),
                                to: IMPLEMENTED.to_owned(),
                                commit: commit.map(str::to_owned),
                            },
                        )
                    })
                },
            ) {
                Ok(Ok(commit)) => say(format!(
                    "store: {story} moved to {IMPLEMENTED}{}",
                    committed(&commit)
                )),
                Ok(Err(refusal)) => {
                    self.refused(events, BY_STORE, &refusal)?;
                    say(format!(
                        "store: {story} not moved to {IMPLEMENTED}: {refusal}"
                    ));
                }
                Err(error) => {
                    let why = format!("{error:#}");
                    self.refused(events, BY_TAKT, &why)?;
                    say(format!("store: {story} not moved to {IMPLEMENTED}: {why}"));
                }
            }
        }
        match unprinted {
            Some(error) => Err(anyhow::Error::new(error).context("print the store's writes")),
            None => Ok(()),
        }
    }

    /// Keeps in the case directory, as `name` at the implementation revision, what the store's
    /// write `commit` committed ([`crate::case::keep_rider`]), then writes its line with
    /// `record`: a line that cannot be written takes the kept file back. A write that changed
    /// nothing keeps nothing.
    fn kept_and_recorded(
        &self,
        commit: Option<&str>,
        name: &str,
        record: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let kept = match (commit, self.case.revision("implementation")) {
            (Some(commit), Some(revision)) => Some(crate::case::keep_rider(
                &self.tree,
                &self.case.dir,
                &revision,
                commit,
                name,
            )?),
            _ => None,
        };
        let recorded = record();
        if recorded.is_err()
            && let Some(kept) = &kept
        {
            for kept in kept {
                let _ = std::fs::remove_file(kept);
            }
        }
        recorded
    }

    /// Before the step for `action`: when the case holds the grant that authorises it
    /// ([`CaseDir::grants`]), a story's run given the store records the story there
    /// ([`Run::record_accepted`]), so the `test_result` and the move, the last, ride to the base
    /// with the step's merge. What goes wrong is recorded as an `error` line, and the step is
    /// taken as it would have been. Answers whether it recorded.
    fn record_before(&self, events: &Events, action: &str, out: &mut dyn Write) -> Result<bool> {
        let (Unit::Story(story), Some(store)) = (&self.unit, self.store) else {
            return Ok(false);
        };
        if !self.case.grants(action)? {
            return Ok(false);
        }
        if let Err(error) = self.record_accepted(events, store, &story.id, out) {
            events.emit(
                self.steps,
                &Event::Error {
                    error: format!(
                        "recording {} in the store before {action}: {error:#}",
                        story.id
                    ),
                },
            )?;
        }
        Ok(true)
    }

    /// The latest `test_result` pass record a validator of the case wrote that is about the
    /// current revision of its subject: its path in the repository and its own `observed_at`.
    /// A record counts only when an `evidence` line of the case's record, which the kernel
    /// writes and a step may not, names it under the case's `evidence/`: a record put there by
    /// any other hand is never one the run observed. `None` when there is none.
    fn observed_pass(&self, recorded: &[serde_json::Value]) -> Result<Option<(String, String)>> {
        let mine = Path::new(CASES_DIR).join(&self.case.id).join("evidence");
        for line in recorded.iter().rev() {
            if line["event"] != "evidence"
                || line["kind"] != TEST_RESULT
                || line["result"] != "pass"
            {
                continue;
            }
            let Some(path) = line["path"].as_str() else {
                continue;
            };
            let relative = Path::new(path);
            if relative.parent() != Some(mine.as_path()) {
                continue;
            }
            let full = self.tree.join(relative);
            let text = std::fs::read_to_string(&full)
                .with_context(|| format!("read the record {}", full.display()))?;
            let record: serde_json::Value = serde_json::from_str(&text)
                .with_context(|| format!("read the record {}", full.display()))?;
            let current = record["subject"]
                .as_str()
                .and_then(|subject| self.case.revision(subject));
            if record["kind"] == TEST_RESULT
                && record["result"] == "pass"
                && current.is_some()
                && record["subject_revision"].as_str() == current.as_deref()
                && let Some(at) = record["observed_at"].as_str()
            {
                return Ok(Some((path.to_owned(), at.to_owned())));
            }
        }
        Ok(None)
    }

    /// Records a refusal by `by`, its reason verbatim.
    fn refused(&self, events: &Events, by: &str, reason: &str) -> Result<()> {
        events.emit(
            self.steps,
            &Event::Refusal {
                by: by.to_owned(),
                reason: reason.to_owned(),
            },
        )
    }

    /// Records a step: by a validator, with the revision it observed, or by the harness, with
    /// the tokens it reported and their components; which one, the action, what it reported.
    fn stepped(
        &self,
        events: &Events,
        by: SteppedBy<'_>,
        name: &str,
        action: &str,
        summary: &str,
    ) -> Result<()> {
        let (by, tokens, usage, revision) = match by {
            SteppedBy::Validator(revision) => ("validator", None, None, revision),
            SteppedBy::Harness(tokens, usage) => ("harness", tokens, usage, None),
        };
        events.emit(
            self.steps,
            &Event::Step {
                by: by.to_owned(),
                name: name.to_owned(),
                action: action.to_owned(),
                summary: summary.to_owned(),
                tokens,
                usage,
                revision: revision.map(str::to_owned),
            },
        )
    }

    /// The loop, the case held.
    fn run_held(&mut self, events: &Events, out: &mut dyn Write) -> Result<Stop> {
        let inputs: Vec<String> = self
            .options
            .inputs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        events.emit(
            self.steps,
            &Event::Opened {
                case: self.case.id.clone(),
                protocol: self.case.protocol.id.clone(),
                story: match &self.unit {
                    Unit::Story(story) => Some(story.id.clone()),
                    Unit::Intent(_) => None,
                },
                inputs: inputs.clone(),
                base: crate::case::Branch::read(&self.case.dir)
                    .ok()
                    .and_then(|branch| branch.base_commit),
            },
        )?;
        writeln!(
            out,
            "case {} on {} in {}; inputs {}",
            self.case.id,
            self.case.protocol.id,
            self.tree.display(),
            if inputs.is_empty() {
                "none".to_owned()
            } else {
                inputs.join(" ")
            }
        )?;
        // One sequence per case, continued across its runs: no decision file is overwritten,
        // so the latest by name is the latest written, and a record id, numbered by the same
        // sequence, never repeats.
        let mut seq = self.case.evaluations()?;
        let mut last_idle: Option<String> = None;
        // The validators that observed without writing a record, each with the revision it
        // observed at: not due again there, because the next observation would end the same
        // way; a commit moves the revision and makes them due again.
        let mut observed_without_record: BTreeSet<(String, String)> = BTreeSet::new();
        // Of those, the ones whose observation waits on the forge, as `(name, action,
        // revision)`: observed again after a pause, within the budget, and meanwhile no
        // judgement step is offered, because what is missing is not the harness's to make.
        let mut awaited: Vec<(String, String, String)> = Vec::new();
        // Whether the story's records were written before a granted step this run.
        let mut recorded_before = false;
        let waiting = |awaited: &[(String, String, String)], budget: Option<Stop>| {
            let mut on: Vec<String> = Vec::new();
            for (_, action, _) in awaited {
                if !on.contains(action) {
                    on.push(action.clone());
                }
            }
            Stop::Waiting {
                on,
                budget: budget.map(Box::new),
            }
        };
        loop {
            seq += 1;
            let decision = self.case.evaluate(seq)?;
            // Each fact is recorded before it is printed, here and below: an `out` that fails
            // ends the run on an error, never with a fact it printed missing from the record.
            events.emit(
                self.steps,
                &Event::Decision {
                    seq,
                    file: format!("decisions/{seq:04}.json"),
                    frontier: decision.frontier_line(),
                },
            )?;
            writeln!(out, "{}", decision.frontier_line())?;
            let state = format!(
                "{:?}{:?}{:?}",
                decision.claims, decision.obligations, decision.actions
            );
            if last_idle.as_deref() == Some(state.as_str()) {
                return Ok(Stop::NoProgress);
            }
            let legitimate = decision.legitimate();
            if let Some(outcome) = legitimate.first() {
                return Ok(Stop::Completed(outcome.clone()));
            }
            if let Some(stop) = self.budget_spent() {
                return Ok(if awaited.is_empty() {
                    stop
                } else {
                    waiting(&awaited, Some(stop))
                });
            }
            let admissible = decision.admissible();
            // The judgement actions: the admissible ones no validator owns.
            let offered: Vec<String> = admissible
                .iter()
                .filter(|a| !self.validators.iter().any(|v| v.action() == a.as_str()))
                .cloned()
                .collect();
            // A deterministic step first: a validator due at this decision.
            let mut due = None;
            for validator in self.validators {
                if is_due(
                    &self.case,
                    &admissible,
                    &observed_without_record,
                    validator.as_ref(),
                )? {
                    due = Some(validator);
                    break;
                }
            }
            if let Some(validator) = due {
                // The story's records in the store, once a run, just before the step its grant
                // authorises: they ride to the base with the merge.
                if !recorded_before {
                    recorded_before = self.record_before(events, validator.action(), out)?;
                }
                self.steps += 1;
                let revision = self
                    .case
                    .revision(validator.subject())
                    .context("the validator's subject has a revision")?;
                // A record is bound to the revision, `HEAD`: a tree with uncommitted changes
                // outside the cases directory would be observed on other content than the one
                // the record names. Nothing is observed, and the changes are left as they are.
                let uncommitted = crate::git::uncommitted(&self.tree)?;
                if !uncommitted.is_empty() {
                    let why = format!(
                        "{} would observe {} with uncommitted changes: {}",
                        validator.action(),
                        &revision[..8.min(revision.len())],
                        uncommitted.join(", ")
                    );
                    self.stepped(
                        events,
                        SteppedBy::Validator(None),
                        validator.name(),
                        validator.action(),
                        "nothing observed",
                    )?;
                    self.refused(events, BY_TAKT, &why)?;
                    writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
                    writeln!(out, "  refused: {why}; nothing observed")?;
                    return Ok(Stop::StepRefused(why));
                }
                // What a validator runs may be code a model wrote (the test command, a gate's
                // commands): the cases directory is guarded as around a judgement step, and the
                // tree must be as it was before the observation: the guarded part of `.git`
                // (hooks, config, `info/`), `HEAD`, every branch and tag, the index and every path outside the cases
                // directory; ignored files it made (a build's output) may stay. The validator's
                // own writes under its case are the only change admitted; anything else is put
                // back, and the observation refused with nothing recorded. `.git` is put back
                // before any git runs again: git runs what was planted there (a hook,
                // `core.fsmonitor`).
                let checkpoint =
                    crate::git::Checkpoint::take_guarding(&self.tree, &self.kept_branches())?;
                let cases = self.tree.join(CASES_DIR);
                let kept = snapshot(&cases)?;
                let observed = validator
                    .observe_waiting(&self.tree, &revision, seq)
                    .with_context(|| format!("{} ({})", validator.action(), validator.name()));
                let git_dir = checkpoint.put_back_git_dir(&self.tree);
                let mut planted = git_dir.touched;
                planted.extend(
                    git_dir
                        .failed
                        .into_iter()
                        .map(|path| format!("not put back: {path}")),
                );
                let touched = differing(&kept, &snapshot(&cases)?);
                let own: Vec<PathBuf> = match self.case.dir.file_name() {
                    Some(id) => validator
                        .writes()
                        .iter()
                        .map(|path| Path::new(id).join(path))
                        .collect(),
                    None => Vec::new(),
                };
                let mut foreign: Vec<String> = touched
                    .iter()
                    .filter(|path| !own.contains(path))
                    .map(|path| Path::new(CASES_DIR).join(path).display().to_string())
                    .collect();
                let change = checkpoint.change(&self.tree)?;
                let mut outside: Vec<String> = planted;
                outside.extend(change.paths.iter().cloned());
                if change.staged {
                    outside.push("the index".to_owned());
                }
                if let Some(moved) = &change.head {
                    outside.push(format!("HEAD, moved {moved}"));
                }
                outside.extend(change.refs.iter().cloned());
                if !foreign.is_empty() || !outside.is_empty() {
                    restore(&cases, &kept, &touched)?;
                    undo_step(&self.tree, &checkpoint, &cases, &kept)?;
                    foreign.extend(outside);
                    let mut why = format!(
                        "{} wrote what its validator does not: {}",
                        validator.action(),
                        foreign.join(", ")
                    );
                    if let Err(error) = &observed {
                        why.push_str(&format!("; the observation failed too: {error:#}"));
                    }
                    // Written after the cases directory was put back, which would take a
                    // line written before it along.
                    self.stepped(
                        events,
                        SteppedBy::Validator(Some(&revision)),
                        validator.name(),
                        validator.action(),
                        "nothing recorded",
                    )?;
                    self.refused(events, BY_TAKT, &why)?;
                    writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
                    writeln!(
                        out,
                        "  refused: {why}; the cases directory was put back, the change discarded and nothing recorded"
                    )?;
                    return Ok(Stop::StepRefused(why));
                }
                let (observation, waits) = observed?;
                self.stepped(
                    events,
                    SteppedBy::Validator(Some(&revision)),
                    validator.name(),
                    validator.action(),
                    &observation.summary,
                )?;
                writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
                writeln!(out, "  effect: {}", observation.summary)?;
                // What the validator retains lands before the records that refer to it: a
                // retention that fails ends the run with nothing written for the step.
                if let Some(artifact) = &observation.artifact {
                    let dir = self.case.dir.join("artifacts");
                    std::fs::create_dir_all(&dir)
                        .with_context(|| format!("create {}", dir.display()))?;
                    let path = dir.join(&artifact.name);
                    std::fs::write(&path, &artifact.bytes)
                        .with_context(|| format!("write {}", path.display()))?;
                    writeln!(
                        out,
                        "  artifact: {}",
                        path.strip_prefix(&self.tree).unwrap_or(&path).display()
                    )?;
                }
                for record in &observation.records {
                    let written = self.case.add_evidence(record)?;
                    events.emit(
                        self.steps,
                        &Event::Evidence {
                            id: record.id.clone(),
                            kind: record.kind.clone(),
                            result: record.result.clone(),
                            path: written
                                .strip_prefix(&self.tree)
                                .unwrap_or(&written)
                                .display()
                                .to_string(),
                        },
                    )?;
                    writeln!(
                        out,
                        "  evidence: {} {}",
                        record.kind,
                        record.result.as_deref().unwrap_or("-")
                    )?;
                }
                // A validator that wrote no record (the gate was unknown) is not due again at
                // this revision. The pass counts as idle only when nothing else can move the
                // decision, no judgement step offered, no other validator due and none waiting
                // on the forge: then the next pass would see the same decision, and the run
                // stops there as no progress. One that waits on the forge is due again after
                // the pause below.
                awaited.retain(|(name, _, _)| name != validator.name());
                if observation.records.is_empty() {
                    observed_without_record.insert((validator.name().to_owned(), revision.clone()));
                    if waits {
                        awaited.push((
                            validator.name().to_owned(),
                            validator.action().to_owned(),
                            revision,
                        ));
                        last_idle = None;
                        continue;
                    }
                    let mut another_due = false;
                    for v in self.validators {
                        if is_due(
                            &self.case,
                            &admissible,
                            &observed_without_record,
                            v.as_ref(),
                        )? {
                            another_due = true;
                            break;
                        }
                    }
                    last_idle =
                        (offered.is_empty() && !another_due && awaited.is_empty()).then_some(state);
                } else {
                    last_idle = None;
                }
                continue;
            }
            // No validator is due, and one waits on the forge whose action is still admissible:
            // the run pauses and observes it again, offering no judgement step meanwhile. A run
            // with no step or hour budget has nothing to wait in and stops waiting at once; the
            // case stays open for the next run.
            awaited.retain(|(_, action, _)| admissible.contains(action));
            if !awaited.is_empty() {
                let Some(pause) = self.pause() else {
                    return Ok(waiting(&awaited, None));
                };
                let on: Vec<&str> = awaited
                    .iter()
                    .map(|(_, action, _)| action.as_str())
                    .collect();
                events.emit(
                    self.steps,
                    &Event::Waiting {
                        on: on.iter().map(|action| (*action).to_owned()).collect(),
                        seconds: pause.as_secs_f64(),
                    },
                )?;
                writeln!(
                    out,
                    "  waiting: {:.1} s on the forge, then {} again",
                    pause.as_secs_f64(),
                    on.join(", ")
                )?;
                std::thread::sleep(pause);
                for (name, _, revision) in &awaited {
                    observed_without_record.remove(&(name.clone(), revision.clone()));
                }
                continue;
            }
            // Then a judgement step among the offered actions.
            if offered.is_empty() {
                let approvals = decision.approval_required();
                return Ok(if approvals.is_empty() {
                    Stop::NothingAdmissible
                } else {
                    Stop::ApprovalRequired(approvals)
                });
            }
            self.steps += 1;
            let approvals: Vec<String> = decision
                .approval_required()
                .into_iter()
                .filter(|a| self.options.gate.as_deref() == Some(a.as_str()))
                .collect();
            let context = StepContext {
                tree: &self.tree,
                intent: &self.options.intent,
                offered: &offered,
                approvals: &approvals,
                decision: &decision,
                protocol: &self.case.protocol,
                step: self.steps as usize,
            };
            // The step is judged by what it did to the tree, against a checkpoint taken just
            // before it: `HEAD`, the index, every path already uncommitted (a validator's run
            // may leave one) with what is at it, the ignored paths, and the guarded part of the
            // git directory. An accepted step commits exactly the paths it changed; a refused
            // one, or one whose harness failed, is put back to the checkpoint whole.
            //
            // The cases directory is the kernel's, every case under it: its files are what
            // the evaluations read, and a harness that wrote into it (a grant, a record, a
            // branch number, the protocol, for this case or a sibling waiting for its grant)
            // would decide what only a validator or the operator decides. It is snapshotted
            // before the step and put back after, whatever the step reported and however it
            // ended; a step that touched it is refused.
            let cases = self.tree.join(CASES_DIR);
            let kept = snapshot(&cases)?;
            // The planning store likewise, and everything else under `.engineering/` but the
            // cases: the project file that says where the store is kept among it. Its
            // artifacts and evidence are what the store's lifecycle moves a story on, and
            // evidence comes from validators only. Only takt's own writes, made once a case is
            // accepted ([`Run::record_on`]), change it. A step whose change git sees there
            // (a file, a link, a mode, a rename), or that left anything there the snapshot sees
            // differ (a FIFO, which git does not list), is refused, and put back to the
            // snapshot taken here.
            let store_kept = snapshot_store(&self.tree)?;
            let store_dirty_before: BTreeSet<String> = crate::git::changed_paths(&self.tree)?
                .into_iter()
                .filter(|path| is_guarded_store_path(path))
                .collect();
            // Every branch and tag is guarded beside `HEAD` and the case's branch: the case's
            // base, and a sibling case's branch, which a step could otherwise move under it; a
            // branch another worktree has checked out is that session's, but never the case's
            // base or its own branch.
            let checkpoint =
                crate::git::Checkpoint::take_guarding(&self.tree, &self.kept_branches())?;
            // A harness that panics is put back like one that failed, then the panic goes on
            // to `run`, which records it and lets go of the case.
            let stepped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.harness.step(&context)
            }));
            // What git runs or reads as configuration first: every git call after this one
            // would read what the step planted there. Nothing in it ends early; a path it could
            // not put back is named in the refusal below.
            let git_dir = checkpoint.put_back_git_dir(&self.tree);
            let tree = self.tree.clone();
            let undo = || -> Result<()> {
                undo_step(&tree, &checkpoint, &cases, &kept)?;
                put_back_store(&tree, &store_kept)
            };
            let stepped = match stepped {
                Ok(stepped) => stepped,
                Err(panic) => {
                    if let Err(undo) = undo() {
                        eprintln!(
                            "step {}: the harness panicked, and the step's change could not be discarded: {undo:#}",
                            self.steps
                        );
                    }
                    std::panic::resume_unwind(panic);
                }
            };
            // What the step left, read now; a read that fails (a file the run may not read)
            // discards the step before the run ends on the error.
            let seen = (|| -> Result<(Vec<PathBuf>, Vec<String>)> {
                let touched = differing(&kept, &snapshot(&cases)?);
                let mut store_touched: BTreeSet<String> =
                    differing(&store_kept, &snapshot_store(&tree)?)
                        .iter()
                        .map(|path| Path::new(ENGINEERING).join(path).display().to_string())
                        .collect();
                store_touched.extend(crate::git::changed_paths(&tree)?.into_iter().filter(
                    |path| is_guarded_store_path(path) && !store_dirty_before.contains(path),
                ));
                Ok((touched, store_touched.into_iter().collect()))
            })();
            let (touched, store_touched) = match seen {
                Ok(seen) => seen,
                Err(error) => {
                    return Err(match undo() {
                        Ok(()) => error.context(format!(
                            "step {}: what the step left could not be read; everything it changed was discarded",
                            self.steps
                        )),
                        Err(undo) => error.context(format!(
                            "step {}: what the step left could not be read, and its change could not be discarded: {undo:#}",
                            self.steps
                        )),
                    });
                }
            };
            if !touched.is_empty() {
                restore(&cases, &kept, &touched)?;
            }
            let report = match stepped {
                Ok(report) => report,
                Err(error) => {
                    return Err(match undo() {
                        Ok(()) => error.context(format!(
                            "step {}: the harness failed; everything the step changed was discarded",
                            self.steps
                        )),
                        Err(undo) => error.context(format!(
                            "step {}: the harness failed, and the step's change could not be discarded: {undo:#}",
                            self.steps
                        )),
                    });
                }
            };
            if let Some(tokens) = report.tokens {
                self.tokens += tokens;
            }
            // The step's line is written once its change is kept or put back: putting the
            // cases directory back to `kept` would take a line written before it along.
            let harness = self.harness;
            let record_step = |run: &Self| {
                run.stepped(
                    events,
                    SteppedBy::Harness(report.tokens, harness.last_usage()),
                    harness.name(),
                    &report.action,
                    &report.summary,
                )
            };
            // A step that cannot be told on `out` is not judged: it is discarded whole, as
            // one whose harness failed, and the run ends on the error.
            let told = writeln!(
                out,
                "step {}: {} ({})",
                self.steps,
                report.action,
                self.harness.name()
            )
            .and_then(|()| writeln!(out, "  said: {}", report.summary));
            if let Err(error) = told {
                undo()?;
                record_step(self)?;
                return Err(anyhow::Error::new(error).context(format!(
                    "step {}: {} could not be printed; the change was discarded",
                    self.steps, report.action
                )));
            }
            let change = match checkpoint.change(&self.tree) {
                Ok(change) => change,
                Err(error) => {
                    undo()?;
                    record_step(self)?;
                    return Err(error.context(format!(
                        "step {}: what {} changed could not be read; the change was discarded",
                        self.steps, report.action
                    )));
                }
            };
            let changed = &change.paths;
            if !touched.is_empty() {
                undo()?;
                let paths: Vec<String> = touched
                    .iter()
                    .map(|p| Path::new(CASES_DIR).join(p).display().to_string())
                    .collect();
                let why = format!(
                    "{} wrote into the cases directory: {}",
                    report.action,
                    paths.join(", ")
                );
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(
                    out,
                    "  refused: {why}; the cases directory was put back and the change discarded"
                )?;
                return Ok(Stop::StepRefused(why));
            }
            if !store_touched.is_empty() {
                undo()?;
                let why = format!(
                    "{} wrote into the planning store, which only takt writes once a case is accepted: {}",
                    report.action,
                    store_touched.join(", ")
                );
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(
                    out,
                    "  refused: {why}; the planning store was put back and the change discarded"
                )?;
                return Ok(Stop::StepRefused(why));
            }
            // What only takt does, or what nobody may: `.git/`, `HEAD`, the index, ignored
            // paths, and a path that was uncommitted before the step, whose change cannot be
            // told apart from what was there.
            let action = &report.action;
            let overreach = if !git_dir.touched.is_empty() {
                let mut why = format!(
                    "{action} wrote under .git, where git runs or reads it: {}",
                    git_dir.touched.join(", ")
                );
                if !git_dir.failed.is_empty() {
                    why.push_str(&format!("; not put back: {}", git_dir.failed.join("; ")));
                }
                Some(why)
            } else if let Some(moved) = &change.head {
                Some(format!("{action} moved HEAD {moved}"))
            } else if !change.refs.is_empty() {
                Some(format!("{action} moved {}", change.refs.join(", ")))
            } else if change.staged {
                Some(format!(
                    "{action} changed the index; takt stages and commits a step's change, the step does not"
                ))
            } else if !change.ignored_added.is_empty()
                || !change.ignored_changed.is_empty()
                || !change.ignored_removed.is_empty()
            {
                let mut what = Vec::new();
                if !change.ignored_added.is_empty() {
                    what.push(format!("wrote {}", change.ignored_added.join(", ")));
                }
                if !change.ignored_changed.is_empty() {
                    what.push(format!("rewrote {}", change.ignored_changed.join(", ")));
                }
                if !change.ignored_removed.is_empty() {
                    what.push(format!("removed {}", change.ignored_removed.join(", ")));
                }
                let mut why = format!(
                    "{action} {} ignored by git, which no commit would carry",
                    what.join(" and ")
                );
                if !change.ignored_lost.is_empty() {
                    why.push_str(&format!(
                        "; nothing was kept to put back of {}",
                        change.ignored_lost.join(", ")
                    ));
                }
                Some(why)
            } else if !change.already_uncommitted.is_empty() {
                Some(format!(
                    "{action} changed {}, uncommitted before the step already",
                    change.already_uncommitted.join(", ")
                ))
            } else {
                None
            };
            if let Some(why) = overreach {
                undo()?;
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "  refused: {why}; the change was discarded")?;
                return Ok(Stop::StepRefused(why));
            }
            if approvals.contains(&report.action) {
                if !changed.is_empty() {
                    undo()?;
                    let why = format!("{} requested with changes", report.action);
                    record_step(self)?;
                    self.refused(events, BY_TAKT, &why)?;
                    writeln!(
                        out,
                        "  refused: {} was requested with {} changed path(s); the change was discarded",
                        report.action,
                        changed.len()
                    )?;
                    return Ok(Stop::StepRefused(why));
                }
                record_step(self)?;
                writeln!(out, "  effect: none; {} needs an approval", report.action)?;
                return Ok(Stop::ApprovalRequired(vec![report.action]));
            }
            let refusal = if !offered.contains(&report.action) {
                Some(format!("{} was not offered", report.action))
            } else if report.action == "repository.inspect" && !changed.is_empty() {
                Some(format!(
                    "repository.inspect changed {} path(s)",
                    changed.len()
                ))
            } else if report.action == "tests.write"
                && let Some(path) = changed.iter().find(|p| !is_test_path(p))
            {
                Some(format!("tests.write changed a non-test path: {path}"))
            } else {
                None
            };
            if let Some(why) = refusal {
                undo()?;
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "  refused: {why}; the change was discarded")?;
                return Ok(Stop::StepRefused(why));
            }
            if changed.is_empty() {
                record_step(self)?;
                events.emit(
                    self.steps,
                    &Event::Effect {
                        paths: Vec::new(),
                        revision: None,
                    },
                )?;
                writeln!(out, "  effect: no file changed")?;
                last_idle = Some(state);
                continue;
            }
            last_idle = None;
            // A commit that fails (a hook of the repository's own refused it) leaves nothing
            // of the step behind either.
            let head = match crate::git::commit_paths(
                &self.tree,
                changed,
                &format!("{}: {}", report.action, report.summary),
            ) {
                Ok(head) => head,
                Err(error) => {
                    undo()?;
                    record_step(self)?;
                    return Err(error.context(format!(
                        "step {}: the commit of {}'s change failed; the change was discarded",
                        self.steps, report.action
                    )));
                }
            };
            self.case.set_revision("implementation", &head)?;
            record_step(self)?;
            events.emit(
                self.steps,
                &Event::Effect {
                    paths: changed.clone(),
                    revision: Some(head.clone()),
                },
            )?;
            writeln!(
                out,
                "  effect: committed {} path(s); implementation is now {}",
                changed.len(),
                &head[..8.min(head.len())]
            )?;
        }
    }
}
