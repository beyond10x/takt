//! A Canon case on disk: `<tree>/.takt/cases/<id>/` (or the legacy `<tree>/.engineering/cases/<id>/`,
//! where a case already is) holds `case.yaml`, `protocol.ir.json`,
//! `evidence/*.json`, `decisions/*.json`, `branch.yaml`, the case's branch and its pull
//! request, `story.yaml`, the story the case was opened for, when it was, and `authority.yaml`,
//! the authority the operator granted the case, once granted, `commit-identity`, the identity
//! its commits carry when it opened with one, and `riders/<revision>/`, what the
//! run's store writes on the story committed, which its riders must equal byte for byte
//! ([`approvals_only`]), and `observations.json`, the observations the Commission runtime
//! reported to the case ([`OBSERVATIONS_FILE`]). Every evaluation is the governor of Loom's, over
//! the case's files as its case store ([`CaseFiles`]); a grant reaches it as the answer of the
//! case's authority provider ([`CaseAuthority`]). A case is opened once and resumed by every
//! later run on it.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};
use loom_sdk::commission::model::json as cjson;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestData, ActionStatus, AuthorityVerdict, AuthorityVerdictApprovalRequired,
    AuthorityVerdictDeny, CaseId, CommissionData, EvidenceData, EvidenceId, GovernorError,
    ObservationData, ObservationId, Unit,
};
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::commission::ports::governor::Governor as _;
use loom_sdk::governor::{
    CanonGovernor, CaseIndependence, CaseInputs, CaseState, FallibleCaseStore, HeldEvidence,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::evidence::Record;
use crate::protocol::LoadedProtocol;

/// The directory under the tree that holds the cases a run opens: each case's record, evidence
/// and files, under [`TAKT_DIR`], which takt keeps out of version control (its `.gitignore`).
/// Excluded from the tree's change detection.
pub const CASES_DIR: &str = ".takt/cases";

/// The directory under the tree that held the cases before [`CASES_DIR`]. A case already there
/// is read and written where it is; no new case opens here. Excluded from the tree's change
/// detection like [`CASES_DIR`], and takt's own repository keeps its improvement turns here.
pub const LEGACY_CASES_DIR: &str = ".engineering/cases";

/// Both roots a case may be under, the one a new case opens under first.
pub const CASE_ROOTS: [&str; 2] = [CASES_DIR, LEGACY_CASES_DIR];

/// The directory under the tree that holds takt's runtime data, cases among it.
pub const TAKT_DIR: &str = ".takt";

/// The file in [`TAKT_DIR`] that ignores everything under it, written when it is absent. A
/// repository that wants its cases in version control commits its own, and takt leaves it alone.
pub const TAKT_GITIGNORE: &str = ".gitignore";

/// The text of [`TAKT_GITIGNORE`] that takt writes: every path under [`TAKT_DIR`] ignored.
const TAKT_GITIGNORE_TEXT: &str = "*\n";

/// The prefix of a case's branch: `takt/<case id>`.
pub const BRANCH_PREFIX: &str = "takt/";

/// The file under the case directory that records the branch and its pull request.
pub const BRANCH_FILE: &str = "branch.yaml";

/// The file under the case directory that holds the authority the operator granted the case.
pub const AUTHORITY_FILE: &str = "authority.yaml";

/// The file under the case directory that names the story the case was opened for.
pub const STORY_FILE: &str = "story.yaml";

/// The file under the case directory that keeps the observations reported to the case, in the
/// order received: a JSON array of objects with `observation_id`, `source`, `subject`,
/// `observed_at` and `payload`, the shape Loom's `FileCaseStore` keeps them in. Written as `[]`
/// when the case opens; a case opened before has none, which reads as no observation.
pub const OBSERVATIONS_FILE: &str = "observations.json";

/// The member of `case.yaml` holding the case revision, `r<n>`: 1 when the case opens (and when
/// the member is absent, as in a case opened before it was kept), one more each time an
/// artifact moves to another revision. Canon reads it as the snapshot's `revision`.
const CASE_REVISION: &str = "revision";

/// The file under the case directory whoever holds the case creates, holding the instant it
/// took hold: a run while it runs, `resume` while it brings the case to its branch, a grant
/// while it is checked, recorded and written. Created with `create_new`, so of two that try at
/// once exactly one holds; no grant lands and no second run starts meanwhile.
pub const LOCK_FILE: &str = "run.lock";

/// The file under the case directory that names the identity every commit of the case carries
/// (`<name> <email>` on one line), written when the case opens; absent when it opened with
/// none, and then its commits are takt's own ([`CaseDir::committer`]).
pub const COMMIT_IDENTITY_RECORD: &str = "commit-identity";

/// The identity a case's commits carry, as the case is opened with it ([`CaseDir::open_as`]),
/// recorded in the case so every resume and approval commits with the same one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CommitAs {
    /// None recorded: the case commits as [`crate::git::CommitIdentity::takt`]. A library run
    /// given no identity opens its case so.
    #[default]
    Takt,
    /// This identity, whatever the repository configures: the operator's `--commit-identity`,
    /// or one the caller read elsewhere (a scenario's declaring revision).
    Given(crate::git::CommitIdentity),
    /// The identity [`crate::git::COMMIT_IDENTITY_FILE`] names at the commit the case forks
    /// from, read when it opens; with none there, as [`CommitAs::Takt`], unless `required` (a run
    /// whose commits leave the machine): then the case is refused before anything is written.
    Configured {
        /// Whether a case finding none configured is refused.
        required: bool,
    },
}

/// The identity the case at `case_dir` was opened with ([`COMMIT_IDENTITY_RECORD`]); none when
/// it recorded none. A record that does not read as an identity is an error.
pub fn commit_identity_of(case_dir: &Path) -> Result<Option<crate::git::CommitIdentity>> {
    let path = case_dir.join(COMMIT_IDENTITY_RECORD);
    match std::fs::read_to_string(&path) {
        Ok(text) => crate::git::CommitIdentity::parse(text.trim_end_matches('\n'))
            .with_context(|| format!("read {}", path.display()))
            .map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

/// The protocol whose cases open with a baseline the run is given: `loop.improvement/1`.
pub const LOOP_PROTOCOL: &str = "loop.improvement/1";

/// The loop case's baseline artifact, and the name `takt run` reads it from:
/// `--input loop_baseline=<full commit id>`, which the protocol declares as no case input.
pub const LOOP_BASELINE: &str = "loop_baseline";

/// What a case opened on `tree` now forks from: the branch the tree is on (`left`), the case's
/// base branch (that branch, or, when the tree is on another case's branch, that case's base),
/// and the branch its own branch is created at when that differs from `HEAD` (the other case's
/// base). A tree on a case's branch with no base to fork from is an error.
fn opening_base(tree: &Path) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let left = crate::git::current_branch(tree)?;
    let (base, from) = match left
        .as_deref()
        .map(|name| (name, name.strip_prefix(BRANCH_PREFIX)))
    {
        Some((name, Some(other))) => {
            // The name is joined to the cases directory only as a case id.
            check_id(other).with_context(|| format!("the branch {} names no case", shown(name)))?;
            let Some(base) = Branch::read(&CaseDir::locate(tree, other))
                .ok()
                .and_then(|branch| branch.base)
            else {
                bail!(
                    "the tree {} is on {name}, a case's branch with no base to fork from (that case was opened from a detached HEAD, or its directory is gone); check out the branch the new case is to fork from",
                    tree.display()
                );
            };
            (Some(base.clone()), Some(base))
        }
        Some((name, None)) => (Some(name.to_owned()), None),
        None => (None, None),
    };
    Ok((left, base, from))
}

/// The commit a loop case opens with as its baseline, from `value`: a full commit id (40 or 64
/// lower-case hexadecimal digits) of a commit `tree` holds, on the base branch the case opens
/// from, its metric definitions readable with every object on the way checked. A missing
/// value, or any other (a short id, a name, a tree's id, an id the repository does not hold, a
/// side branch's or an unreachable commit, a baseline whose objects do not hash to their ids,
/// one whose definitions are absent or do not read),
/// is refused, naming why, before anything is written. Until promotions are recorded the
/// operator gives it; then the latest promotion's revision.
pub fn loop_baseline(tree: &Path, value: Option<&str>) -> Result<String> {
    let Some(value) = value else {
        bail!(
            "{LOOP_PROTOCOL} opens with --input {LOOP_BASELINE}=<full commit id>, the promoted revision its baseline is; none was given; nothing was written"
        );
    };
    let full = matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !full {
        bail!(
            "--input {LOOP_BASELINE}={value}: a loop case's baseline is a full commit id, 40 or 64 lower-case hexadecimal digits; nothing was written"
        );
    }
    // Every commit is read through one checked history: each against its id, capped, never
    // parsed by git first.
    let mut history = crate::loop_scorecard::History::open(tree, None)
        .with_context(|| format!("--input {LOOP_BASELINE}={value}; nothing was written"))?;
    let resolved = history
        .resolve(value)
        .with_context(|| format!("--input {LOOP_BASELINE}={value}; nothing was written"))?;
    if resolved.as_deref() != Some(value) {
        bail!(
            "--input {LOOP_BASELINE}={value}: no commit of {} has that id; nothing was written",
            tree.display()
        );
    }
    // The baseline is a commit of the base branch the case opens from: a side branch's commit,
    // or one no branch reaches, is no promoted revision.
    let (_, base, _) = opening_base(tree)?;
    let Some(base) = base else {
        bail!(
            "--input {LOOP_BASELINE}={value}: the case would open from a detached HEAD, with no base branch its baseline is on; check out the base branch; nothing was written"
        );
    };
    let tip = crate::git::run(
        tree,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{base}"),
        ],
    )
    .with_context(|| format!("--input {LOOP_BASELINE}={value}: the base branch {base}"))?;
    let on_base = history
        .descends(value, &tip)
        .with_context(|| format!("--input {LOOP_BASELINE}={value}; nothing was written"))?;
    if on_base == crate::loop_scorecard::Ancestry::Unknown {
        bail!(
            "--input {LOOP_BASELINE}={value}: the shallow history of {base} ends before it could say whether the commit is on it; nothing was written"
        );
    }
    if on_base == crate::loop_scorecard::Ancestry::No {
        bail!(
            "--input {LOOP_BASELINE}={value}: the commit is not on {base}, the branch the case opens from; nothing was written"
        );
    }
    // Its metric definitions are read, every object on the way checked, before anything is
    // written: one planted under another's id is refused here, naming it, and so are
    // definitions that are absent or do not read, which would open a case that never records.
    let found = crate::loop_scorecard::Definitions::at(tree, value).with_context(|| {
        format!("--input {LOOP_BASELINE}={value}: its metric definitions; nothing was written")
    })?;
    match found {
        crate::loop_scorecard::Found::Definitions(_) => Ok(value.to_owned()),
        crate::loop_scorecard::Found::Absent => bail!(
            "--input {LOOP_BASELINE}={value}: the commit holds no {}; a loop case could record nothing; nothing was written",
            crate::loop_scorecard::DEFINITIONS
        ),
        crate::loop_scorecard::Found::Malformed(why) => bail!(
            "--input {LOOP_BASELINE}={value}: its {} do not read: {why}; nothing was written",
            crate::loop_scorecard::DEFINITIONS
        ),
        crate::loop_scorecard::Found::NoCommit => bail!(
            "--input {LOOP_BASELINE}={value}: no commit of {} has that id; nothing was written",
            tree.display()
        ),
    }
}

/// Whether `path`, relative to the tree, is a cases root ([`CASE_ROOTS`]) or lies under one:
/// what the kernel's change detection, commit and clean leave out, the new root and the legacy
/// one alike. The directory and its contents only, never a path that merely starts with the
/// same characters.
#[must_use]
pub fn is_under_cases(path: &str) -> bool {
    CASE_ROOTS.iter().any(|root| {
        path == *root
            || path
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// Whether `path`, relative to the tree, is a directory that holds a cases root, such as
/// `.takt`: git names an ignored directory as one entry, which would take its cases with it.
#[must_use]
pub fn holds_cases_root(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    CASE_ROOTS.iter().any(|root| {
        root.strip_prefix(path)
            .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// Every case directory under `tree`, under either root, by case id in name order: a case under
/// both roots is listed once, under the new one. Neither root present is none; a root that is
/// not a directory is an error naming it.
pub fn case_dirs(tree: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();
    for root in CASE_ROOTS {
        let dir = tree.join(root);
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.with_context(|| format!("read {}", dir.display()))?;
                    if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                        let id = entry.file_name().to_string_lossy().into_owned();
                        found.entry(id).or_insert_with(|| entry.path());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("read {}", dir.display())),
        }
    }
    Ok(found.into_iter().collect())
}

/// The Unicode format characters, general category Cf (Unicode 15.1), as inclusive ranges:
/// invisible, or changing the direction of the text around them.
const FORMAT_CHARACTERS: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

/// The default-ignorable code points, `Default_Ignorable_Code_Point` of Unicode 16.0's
/// DerivedCoreProperties.txt, as inclusive ranges: invisible wherever they stand, so a name
/// holding one reads as the name without it (U+034F COMBINING GRAPHEME JOINER, the variation
/// selectors, the Hangul fillers, the reserved tag and filler planes).
const DEFAULT_IGNORABLE: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x034F, 0x034F),
    (0x061C, 0x061C),
    (0x115F, 0x1160),
    (0x17B4, 0x17B5),
    (0x180B, 0x180F),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x206F),
    (0x3164, 0x3164),
    (0xFE00, 0xFE0F),
    (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0),
    (0xFFF0, 0xFFF8),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0000, 0xE0FFF),
];

/// Whether `c` has no business in a name that is shown to people: a control character, a
/// whitespace or separator character (spaces, U+2028, U+2029), a format character or a
/// default-ignorable code point.
fn is_unshowable(c: char) -> bool {
    let point = u32::from(c);
    c.is_control()
        || c.is_whitespace()
        || FORMAT_CHARACTERS
            .iter()
            .chain(DEFAULT_IGNORABLE)
            .any(|&(first, last)| (first..=last).contains(&point))
}

/// `id` as it was typed, but for each character that does not show ([`is_unshowable`]),
/// written as its code point (`\u{200b}`): a quote or a backslash shows as itself.
pub(crate) fn shown(id: &str) -> String {
    id.chars()
        .map(|c| {
            if is_unshowable(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// The longest case id, in bytes: git keeps the branch `takt/<id>` as the file `<id>` and locks
/// it as `<id>.lock`, and a file name holds 255 bytes.
const MAX_ID_BYTES: usize = 250;

/// The characters git refuses anywhere in a branch name, besides `\`, controls and spaces.
const REFUSED_IN_A_BRANCH: &[char] = &['~', '^', ':', '?', '*', '['];

/// Why git would refuse `takt/<id>` as a branch name, for an id [`check_id`] has already found
/// one name with no `/`, `\`, `..`, control or whitespace character: git's `check-ref-format`
/// rules for one component of a ref (no leading `.`, no trailing `.` or `.lock`, no `@{`, none
/// of `~ ^ : ? * [`), and a name of at most [`MAX_ID_BYTES`] bytes, so the ref's lock file can
/// be made. `@` alone is refused too: git reads a lone `@` as `HEAD`, though it accepts
/// `takt/@`.
fn branch_refusal(id: &str) -> Option<String> {
    if id.len() > MAX_ID_BYTES {
        return Some(format!(
            "it is {} bytes long, past the limit of {MAX_ID_BYTES}: git cannot make the lock file of the branch {BRANCH_PREFIX}<id>",
            id.len()
        ));
    }
    if id == "@" {
        return Some("it is `@` alone, which git reads as `HEAD`".to_owned());
    }
    let why = if id.starts_with('.') {
        "it starts with `.`".to_owned()
    } else if id.ends_with(".lock") {
        "it ends with `.lock`".to_owned()
    } else if id.ends_with('.') {
        "it ends with `.`".to_owned()
    } else if id.contains("@{") {
        "it contains `@{`".to_owned()
    } else {
        let c = id.chars().find(|c| REFUSED_IN_A_BRANCH.contains(c))?;
        format!("it contains `{c}`")
    };
    Some(format!(
        "{why}, and git refuses {BRANCH_PREFIX}{id} as a branch name"
    ))
}

/// Refuses a case id that is not one plain name under the cases directory: empty, `.`, a `..`
/// anywhere, a leading `/`, a path separator, or a character that does not show as itself (a
/// control, whitespace, separator or Unicode format character, or a default-ignorable code
/// point, named by its code point); and an id whose branch `takt/<id>` git would not accept,
/// past 250 bytes among them ([`branch_refusal`]). Every id an operator gives (`takt approve`,
/// `takt run --case`, `RunOptions.case_id`) passes here before it is joined to a path, so no
/// id reaches outside a cases root ([`CASE_ROOTS`]), nests a case inside another, holds a character
/// that does not show in a message or a `takt/<id>` branch, or opens a case whose branch git
/// cannot make. The check does not normalise: `café` precomposed (NFC) and decomposed (NFD)
/// are two ids, two case directories and two branches. The refusal names the id as typed, a
/// character that does not show written as its code point.
pub fn check_id(id: &str) -> Result<()> {
    let why = if id.is_empty() {
        Some("it is empty".to_owned())
    } else if id == "." {
        Some("it names the cases directory itself".to_owned())
    } else if id.contains("..") {
        Some(format!(
            "it contains `..`, which a path reads as the directory above, and git refuses {BRANCH_PREFIX}{} as a branch name",
            shown(id)
        ))
    } else if id.starts_with('/') {
        Some("it starts with `/`".to_owned())
    } else if id.contains(['/', '\\']) {
        Some("it contains a path separator".to_owned())
    } else if let Some(c) = id.chars().find(|c| is_unshowable(*c)) {
        Some(format!(
            "it contains U+{:04X}, a control, whitespace, separator, format or default-ignorable character",
            u32::from(c)
        ))
    } else {
        branch_refusal(id)
    };
    match why {
        Some(why) => bail!(
            "the case id `{}` is refused: {why}; a case id is one name under {CASES_DIR}",
            shown(id)
        ),
        None => Ok(()),
    }
}

/// The instant whoever holds the case at `case_dir` took hold, from its `run.lock`; `None`
/// when nobody holds it.
pub fn held_since(case_dir: &Path) -> Result<Option<String>> {
    let path = case_dir.join(LOCK_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    Ok(Some(text.trim().to_owned()))
}

/// Takes hold of the case `id` at `case_dir`: creates `run.lock` with `create_new`, so the
/// check and the take are one step and of two holders exactly one gets the file, then writes
/// the instant into it. Refused while another holds it, naming the lock; a holder that crashed
/// leaves its lock behind, and the refusal says to remove the file by hand.
fn take_hold(case_dir: &Path, id: &str) -> Result<()> {
    let path = case_dir.join(LOCK_FILE);
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let since = match held_since(case_dir).ok().flatten() {
                Some(since) if !since.is_empty() => format!("since {since}"),
                _ => "since a moment ago".to_owned(),
            };
            bail!(
                "the case {id} is held {since} ({}) by a run or an approval; if neither is in progress, remove the file",
                case_dir.join(LOCK_FILE).display()
            );
        }
        Err(error) => {
            return Err(error).with_context(|| format!("create {}", path.display()));
        }
    };
    if let Err(error) = writeln!(file, "{}", crate::now()) {
        let _ = std::fs::remove_file(&path);
        return Err(error).with_context(|| format!("write {}", path.display()));
    }
    Ok(())
}

/// A hold taken for the length of one operation, `resume`'s or a grant's: let go when dropped,
/// whichever way the operation ends.
#[derive(Debug)]
struct Held(PathBuf);

impl Held {
    fn take(case_dir: &Path, id: &str) -> Result<Self> {
        take_hold(case_dir, id)?;
        Ok(Self(case_dir.join(LOCK_FILE)))
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The case's branch, recorded as `branch.yaml` under the case directory: created when the
/// case opens, at the tip of its base, pushed and given its one pull request by the `pr.open`
/// validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// `takt/<case id>`.
    pub name: String,
    /// The pull request's base, the branch the case forked from: the branch the tree was on
    /// when the case opened, or, when that was another case's branch, that case's base; absent
    /// when `HEAD` was detached or the other case was gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// The commit the case's branch forked from, pinned when the case opened: what the gate
    /// validator reads the gates document and the project file from, whatever the base
    /// branch names later. Absent for a case opened before it was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_commit: Option<String>,
    /// The branch the tree was on when the case opened, the one a discarded case returns the
    /// tree to; absent when `HEAD` was detached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
    /// The implementation revision last pushed, the pull request's head; absent until the
    /// first push.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed: Option<String>,
    /// The pull request's number; absent until it is opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<u64>,
}

impl Branch {
    /// The branch name of the case `id`.
    #[must_use]
    pub fn name_of(id: &str) -> String {
        format!("{BRANCH_PREFIX}{id}")
    }

    /// Reads the case's `branch.yaml`.
    pub fn read(case_dir: &Path) -> Result<Self> {
        let path = case_dir.join(BRANCH_FILE);
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))
    }

    /// Writes the case's `branch.yaml`.
    pub fn write(&self, case_dir: &Path) -> Result<()> {
        let path = case_dir.join(BRANCH_FILE);
        let text = serde_yaml_ng::to_string(self).context("write the branch record")?;
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
    }
}

/// An authority decision on one capability: the two values Canon's `canon-authority/1` knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grant {
    /// The capability is granted.
    Granted,
    /// The capability is denied.
    Denied,
}

impl Grant {
    /// The decision as `canon-authority/1` writes it: `granted` or `denied`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
        }
    }
}

impl std::fmt::Display for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One entry of the case's `authority.yaml`: the `capability` and `decision` a
/// `canon-authority/1` entry has, and beside them who decided it, when, and at which revision
/// of the artifact the capability's action is about. Canon is handed the two keys it declares ([`Authority::canon`])
/// and would refuse the others, which are the case's own record of the operator's decision;
/// the revision is enforced by takt, not by Canon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityDecision {
    /// The capability decided, `repository.merge`.
    pub capability: String,
    /// Whether it is granted.
    pub decision: Grant,
    /// Who decided it, as given to `takt approve --by`.
    pub approver: String,
    /// When, as [`crate::now`] writes an instant.
    pub granted_at: String,
    /// The revision of the artifact the capability's action is about
    /// ([`LoadedProtocol::grant_subject`]: `implementation` for a delivery, `candidate` for an
    /// improvement turn) the case was at the gate at when the grant was given: the one revision
    /// it authorises. A grant without one (written before grants were bound to a
    /// revision) authorises none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// The commit the mainline was at when the grant was given, read by `takt approve` through
    /// the forge the improvement turn recorded at open ([`FORGE_FILE`]): what
    /// `candidate.merge_judge_surface` decides a merge made before the grant by, never a date.
    /// Recorded for a turn's grant of [`crate::validator::JUDGE_SURFACE_CAPABILITY`] only; a
    /// grant without one authorises no judge-surface merge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mainline_tip: Option<String>,
}

impl AuthorityDecision {
    /// Whether the decision applies at the revision `at`: a denial everywhere,
    /// a grant only at the revision it was given at.
    #[must_use]
    pub fn applies_at(&self, at: Option<&str>) -> bool {
        match self.decision {
            Grant::Denied => true,
            Grant::Granted => self.revision.is_some() && self.revision.as_deref() == at,
        }
    }
}

/// The authority the operator granted the case, `authority.yaml`: a list of decisions, one per
/// capability as `takt approve` writes it (Canon refuses a capability decided twice; a file that
/// decides one more than once is read by [`Authority::decision`], a denial first), answered into
/// every evaluation of the case by its authority provider ([`CaseAuthority`]), each grant only at
/// the revision it was given at. Never read or written through a link.
/// Absent until the first grant: a case with none stops at the gate with `ApprovalRequired`,
/// and a connection to the forge is never authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Authority {
    /// The decisions, in the order they were given.
    pub decisions: Vec<AuthorityDecision>,
}

impl Authority {
    /// Reads the case's `authority.yaml`; `None` when the case has none. It is read without
    /// following a link at it: a link there, or anything but a regular file, is an error naming
    /// it, so a grant the case does not hold is never read from elsewhere.
    pub fn read(case_dir: &Path) -> Result<Option<Self>> {
        let path = case_dir.join(AUTHORITY_FILE);
        let Some(bytes) = crate::replay::read_plain(&path)? else {
            return Ok(None);
        };
        let text = String::from_utf8(bytes).with_context(|| format!("read {}", path.display()))?;
        let authority =
            serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
        Ok(Some(authority))
    }

    /// Writes the case's `authority.yaml`, without following a link at it (`O_NOFOLLOW`): a link
    /// there is refused, naming it.
    pub fn write(&self, case_dir: &Path) -> Result<()> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let path = case_dir.join(AUTHORITY_FILE);
        let text = serde_yaml_ng::to_string(self).context("write the authority")?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .with_context(|| {
                format!(
                    "write {}; a case is never read or written through a link",
                    path.display()
                )
            })?;
        file.write_all(text.as_bytes())
            .with_context(|| format!("write {}", path.display()))
    }

    /// The decision on `capability`, when the case decides it: a denial of it wherever the file
    /// has one, in whatever order, since a denial applies at every revision and a capability
    /// decided more than once fails closed; else the first decision on it. Every reader of a
    /// grant reads this one ([`Authority::grants_at`], [`CaseAuthority`], `takt approve`).
    #[must_use]
    pub fn decision(&self, capability: &str) -> Option<&AuthorityDecision> {
        let mut on = self
            .decisions
            .iter()
            .filter(|decision| decision.capability == capability);
        let first = on.clone().next()?;
        Some(
            on.find(|decision| decision.decision == Grant::Denied)
                .unwrap_or(first),
        )
    }

    /// Whether the case holds a grant of `capability` at `revision`, a revision of the artifact
    /// the capability's action is about.
    #[must_use]
    pub fn grants_at(&self, capability: &str, revision: &str) -> bool {
        self.decision(capability).is_some_and(|decision| {
            decision.decision == Grant::Granted && decision.applies_at(Some(revision))
        })
    }

    /// The document as Canon reads it with every grant's subject at the revision `at`: a
    /// `canon-authority/1` list of `capability` and `decision` entries, nothing else, holding
    /// the decisions that apply there ([`AuthorityDecision::applies_at`]). A grant given at
    /// another revision is left out, so Canon finds the capability undecided and the action it
    /// guards approval-required again: a moved subject needs a new grant.
    pub fn canon(&self, at: Option<&str>) -> Result<String> {
        self.canon_by(|_| at.map(str::to_owned))
    }

    /// [`Authority::canon`], each decision read at the revision `revision_of` answers for its
    /// capability: the case's revision of the artifact the capability's action is about
    /// ([`LoadedProtocol::grant_subject`]); a grant whose capability has none authorises
    /// nothing.
    pub fn canon_by(&self, revision_of: impl Fn(&str) -> Option<String>) -> Result<String> {
        #[derive(Serialize)]
        struct Entry<'a> {
            capability: &'a str,
            decision: Grant,
        }
        let entries: Vec<Entry<'_>> = self
            .decisions
            .iter()
            .filter(|decision| decision.applies_at(revision_of(&decision.capability).as_deref()))
            .map(|decision| Entry {
                capability: &decision.capability,
                decision: decision.decision,
            })
            .collect();
        serde_yaml_ng::to_string(&entries).context("render the authority for Canon")
    }
}

/// A grant `takt approve` made: what the planning store records on the story, beside the
/// grant in the case's `authority.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    /// The case the capability was granted to.
    pub case: String,
    /// The story the case was opened for; none for a case opened on an intent, which has
    /// nothing in the store to record on, and for an improvement turn, whose grant is the
    /// case's alone.
    pub story: Option<String>,
    /// The capability granted.
    pub capability: String,
    /// Who granted it.
    pub approver: String,
    /// When, as [`crate::now`] writes an instant.
    pub granted_at: String,
    /// The revision the grant authorises: the one of the artifact its action is about that the
    /// case reached the gate at.
    pub revision: String,
}

impl Approval {
    /// Where to go and look: the case's `authority.yaml`, as `<case>/authority.yaml`.
    #[must_use]
    pub fn reference(&self) -> String {
        format!("{}/{AUTHORITY_FILE}", self.case)
    }

    /// Where the store's record of the approval says it came from: `takt approve <capability>
    /// --by <approver> at <revision>`.
    #[must_use]
    pub fn source(&self) -> String {
        format!(
            "takt approve {} --by {} at {}",
            self.capability, self.approver, self.revision
        )
    }
}

/// The planning store's record of an approval, as `aep plan artifact evidence --kind approval
/// --source … --ref … --at …` writes it for [`crate::frontier::approve`]: these fields and no
/// other, at either level. A field the store would also read (a `review` and its `outcome`,
/// which it takes as that review's outcome whatever the kind) makes the file no such record.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreRecord {
    at: String,
    actor: String,
    artifact: String,
    kind: String,
    revision: u64,
    change: StoreChange,
}

/// What a [`StoreRecord`] records: evidence, with its kind, source and reference, nothing else.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreChange {
    change: String,
    kind: String,
    source: String,
    reference: String,
}

/// The kinds of actor the store writes a record as, each followed by `:` and a name.
const ACTOR_KINDS: [&str; 3] = ["human", "agent", "service"];

/// The kinds of actor that may grant a story's capability: a person or an agent role.
const APPROVER_KINDS: [&str; 2] = ["human", "agent"];

/// Whether `approver` may grant a case's capability: `human:<name>` or `agent:<role>`, an
/// actor the store writes ([`is_store_actor`]). The store writes an approval's record as its
/// approver ([`crate::frontier::Aep`] sets `AEP_ACTOR`); `system` and `service:<name>` are the
/// store's and tools' own actors, not who grants, and takt's own actor
/// ([`crate::frontier::ACTOR`]), which writes the story's `test_result` and move, never grants
/// its own merge.
#[must_use]
pub fn is_approver(approver: &str) -> bool {
    approver != crate::frontier::ACTOR
        && is_store_actor(approver)
        && approver
            .split_once(':')
            .is_some_and(|(kind, _)| APPROVER_KINDS.contains(&kind))
}

/// Whether `actor` is an actor the store writes: `system`, or `human`, `agent` or `service`, a
/// `:`, and a name of ASCII letters, digits, `.`, `_`, `@` and `-`.
fn is_store_actor(actor: &str) -> bool {
    actor == "system"
        || actor.split_once(':').is_some_and(|(kind, name)| {
            ACTOR_KINDS.contains(&kind)
                && !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._@-".contains(&byte))
        })
}

impl StoreRecord {
    /// Whether this is the store's record of `approval` on its story, exactly as
    /// [`crate::frontier::approve`] has the store write it, the story at `story_revision` in the
    /// store: evidence of kind `approval` on the story, the story's kind and revision, the
    /// approval's own source, reference and instant, and its approver as the actor.
    fn records(&self, approval: &Approval, story_revision: u64) -> bool {
        let Some(story) = approval.story.as_deref() else {
            return false;
        };
        let Some((kind, _)) = story.split_once(':') else {
            return false;
        };
        self.artifact == story
            && self.kind == kind
            && self.revision == story_revision
            && self.at == approval.granted_at
            && is_store_actor(&self.actor)
            && self.actor == approval.approver
            && self.change.change == "evidence"
            && self.change.kind == "approval"
            && self.change.source == approval.source()
            && self.change.reference == approval.reference()
    }
}

/// The revision the planning store holds `story` at in the commit `revision`: the `revision`
/// of the front matter of `.engineering/planning/<kind>/<name>.md` there; `None` when the
/// commit holds no such file or it states none. A record of the store carries the revision the
/// artifact was at when it was written, and an approval's record adds evidence only, so every
/// approval's record above `revision` carries this one.
fn story_revision_at(tree: &Path, revision: &str, story: &str) -> Result<Option<u64>> {
    Ok(story_front_at(tree, revision, story)?.and_then(|doc| doc["revision"].as_u64()))
}

/// The status the planning store holds `story` at in the commit `revision`: the `status` of the
/// front matter of its file there; `None` when the commit holds no such file or it states none.
pub fn story_status_at(tree: &Path, revision: &str, story: &str) -> Result<Option<String>> {
    Ok(story_front_at(tree, revision, story)?
        .and_then(|doc| doc["status"].as_str().map(str::to_owned)))
}

/// The front matter of `.engineering/planning/<kind>/<name>.md` for `story` in the commit
/// `revision`, read as the store reads it ([`front_matter`]); `None` when the commit holds no
/// such file or its front matter does not read.
fn story_front_at(
    tree: &Path,
    revision: &str,
    story: &str,
) -> Result<Option<serde_yaml_ng::Value>> {
    let Some((kind, name)) = story.split_once(':') else {
        return Ok(None);
    };
    let blob = format!("{revision}:.engineering/planning/{kind}/{name}.md");
    if crate::git::run(tree, &["cat-file", "-e", &blob]).is_err() {
        return Ok(None);
    }
    let Ok(text) = String::from_utf8(crate::git::blob(tree, &blob)?) else {
        return Ok(None);
    };
    let Some(front) = front_matter(&text) else {
        return Ok(None);
    };
    Ok(serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&front).ok())
}

/// The front matter of a planning document, as the store reads it: the lines between a first
/// line `---` and the next line `---`, each line read without its line end, LF or CRLF. `None`
/// when the text does not open with that line (whitespace or anything else before it) or the
/// front matter is never closed.
fn front_matter(text: &str) -> Option<String> {
    let mut lines = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line));
    if lines.next()? != "---" {
        return None;
    }
    let mut front = Vec::new();
    for line in lines {
        if line == "---" {
            return Some(front.join("\n"));
        }
        front.push(line);
    }
    None
}

/// The actions of the case's protocol that require a capability, each with the capabilities
/// it requires, read from the case's own `protocol.yaml` (each action's `requires` list of
/// `capability` entries), with the protocol's id: what a grant may name, and which actions
/// show whether the case is at the gate for it.
fn required_capabilities(case_dir: &Path) -> Result<(String, BTreeMap<String, Vec<String>>)> {
    let path = case_dir.join("protocol.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    let id = format!(
        "{}/{}",
        doc["protocol"]["id"].as_str().unwrap_or("?"),
        doc["protocol"]["revision"].as_u64().unwrap_or(0)
    );
    let actions: BTreeMap<String, Vec<String>> = doc["actions"]
        .as_mapping()
        .map(|actions| {
            actions
                .iter()
                .filter_map(|(name, action)| {
                    let capabilities: Vec<String> = action["requires"]
                        .as_sequence()?
                        .iter()
                        .filter_map(|requirement| {
                            requirement["capability"].as_str().map(str::to_owned)
                        })
                        .collect();
                    (!capabilities.is_empty())
                        .then(|| Some((name.as_str()?.to_owned(), capabilities)))?
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((id, actions))
}

/// The revision of `artifact` a decision was computed at, from what its explanation says it
/// was computed from; `None` when it does not say.
fn decided_at(decision: &Decision, artifact: &str) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_str(&decision.json).ok()?;
    doc["explanation"]["computed_from"]["case"]["artifacts"][artifact]["revision"]
        .as_str()
        .map(str::to_owned)
}

/// The revision of `artifact` the case at `case_dir` is at, from its `case.yaml`.
fn revision_of(case_dir: &Path, artifact: &str) -> Result<Option<String>> {
    let path = case_dir.join("case.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    Ok(doc["artifacts"][artifact]["revision"]
        .as_str()
        .map(str::to_owned))
}

/// Whether the case at `case_dir` holds a grant for `action`, read from its own files, so a
/// validator observing while a run holds the case can ask: its `protocol.yaml` has the action
/// require at least one capability, and its `authority.yaml` grants each at the case's current
/// revision (`case.yaml`) of the artifact that capability's action is about. What
/// [`CaseDir::grants`] answers from the case in memory.
pub fn holds_grant(case_dir: &Path, action: &str) -> Result<bool> {
    let (_, actions) = required_capabilities(case_dir)?;
    let Some(capabilities) = actions.get(action) else {
        return Ok(false);
    };
    let protocol = LoadedProtocol::from_file(&case_dir.join("protocol.yaml"))?;
    let authority = Authority::read(case_dir)?.unwrap_or_default();
    for capability in capabilities {
        let Ok(subject) = protocol.grant_subject(capability) else {
            return Ok(false);
        };
        let Some(revision) = revision_of(case_dir, &subject)? else {
            return Ok(false);
        };
        if !authority.grants_at(capability, &revision) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A grant checked and ready to be written, the case held until it is written or dropped: what
/// `takt approve` records on the story before the grant lands, so a record the store refuses
/// leaves nothing written and the same approval can be given again.
#[derive(Debug)]
pub struct PendingGrant {
    dir: PathBuf,
    authority: Authority,
    approval: Approval,
    _held: Held,
}

impl PendingGrant {
    /// What the grant will grant, for the store to record.
    #[must_use]
    pub fn approval(&self) -> &Approval {
        &self.approval
    }

    /// Writes the grant into the case's `authority.yaml` and lets go of the case; answers what
    /// was granted.
    pub fn write(self) -> Result<Approval> {
        self.authority.write(&self.dir)?;
        Ok(self.approval)
    }
}

/// Grants `capability` to the case `id` under `tree`, decided by `approver` now, at the
/// revision of the artifact its action is about that the case reached the gate at: the decision goes into the case's
/// `authority.yaml`, and what was granted is answered for the store to record. Shorthand for
/// [`prepare_grant`] and [`PendingGrant::write`].
pub fn grant(tree: &Path, id: &str, capability: &str, approver: &str) -> Result<Approval> {
    prepare_grant(tree, id, capability, approver)?.write()
}

/// Checks a grant of `capability` to the case `id` under `tree` by `approver` and holds the case
/// for it, writing nothing. The case is held first (`run.lock`, taken atomically), so a grant
/// never lands while a run holds the case, and no run starts until the grant is written or
/// dropped. Refused: an id that is not a case id ([`check_id`]), a case that does not exist, a
/// case another holds, a capability that is not an identifier, an empty approver, an approver
/// that is not `human:<name>` or `agent:<role>` as the store writes them ([`is_approver`]), a
/// capability no action of the case's protocol requires (a grant nothing asks for is a typo, told at the
/// command line rather than by a run that stops at the gate again), a capability whose actions
/// are about no single artifact ([`LoadedProtocol::grant_subject`]), a case that is not at the
/// gate for the capability (its latest decision does not find an action requiring it
/// approval-required, or was computed at another revision of that artifact than the case is at
/// now), and a capability the case already decides at that revision. A grant given at another
/// revision is replaced: it authorises nothing here, and Canon refuses a capability decided
/// twice. An improvement turn's grant of [`crate::validator::JUDGE_SURFACE_CAPABILITY`] records
/// the mainline's tip read once the grant's instant is taken ([`mainline_at_grant`]), none for a
/// turn that recorded no forge; a tip that cannot be read is refused with nothing written.
pub fn prepare_grant(
    tree: &Path,
    id: &str,
    capability: &str,
    approver: &str,
) -> Result<PendingGrant> {
    check_id(id)?;
    let dir = CaseDir::locate(tree, id);
    if !dir.join("case.yaml").is_file() {
        bail!("no case {id} at {}", dir.display());
    }
    // A run holds its case while it runs: a grant given meanwhile is refused, not written into
    // a case a judgement step is in flight on; and the grant holds the case until it is written.
    let held = Held::take(&dir, id)?;
    if capability.is_empty()
        || capability
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        bail!("the capability `{capability}` is not an identifier");
    }
    if approver.trim().is_empty() {
        bail!("the approver is empty; say who grants {capability}");
    }
    // Every grant is typed: a person or an agent role the store writes as, for a story's case
    // and an intent's alike. A story's grant is recorded in the store as its approver, and its
    // record rides to the base only with the approver as its actor; and the merge of a
    // judge-surface change reads the kind to decide whose grant it is.
    // An improvement turn's `story.yaml` names the finding it verifies, a story it does not
    // deliver: its grant is the case's alone, recorded nowhere in the planning store.
    let story = if required_capabilities(&dir)?.0 == TURN_PROTOCOL {
        None
    } else {
        story_of(&dir)?
    };
    if !is_approver(approver) {
        bail!(
            "the approver `{}` is not an actor the planning store records as granting; say who grants {capability} as human:<name> or agent:<role>, not takt's own {}",
            approver.escape_debug(),
            crate::frontier::ACTOR
        );
    }
    let (protocol, actions) = required_capabilities(&dir)?;
    let requiring: Vec<&String> = actions
        .iter()
        .filter(|(_, capabilities)| capabilities.iter().any(|c| c == capability))
        .map(|(action, _)| action)
        .collect();
    if requiring.is_empty() {
        let mut required: Vec<&String> = actions.values().flatten().collect();
        required.sort();
        required.dedup();
        if required.is_empty() {
            bail!(
                "no action of {protocol} requires a capability; {capability} grants the case {id} nothing"
            );
        }
        let required: Vec<&str> = required.into_iter().map(String::as_str).collect();
        bail!(
            "no action of {protocol} requires {capability}; the capabilities its actions require: {}",
            required.join(", ")
        );
    }
    // The artifact the gated action is about, read from the protocol: the grant binds to its
    // revision, `implementation` for a delivery, `candidate` for an improvement turn.
    let subject =
        LoadedProtocol::from_file(&dir.join("protocol.yaml"))?.grant_subject(capability)?;
    // At the gate: the latest decision finds an action requiring the capability
    // approval-required, and was computed at the revision of that artifact the case is at now.
    // That revision is the one the grant authorises.
    let Some(latest) = latest_decision(&dir)? else {
        bail!(
            "the case {id} is not at the gate for {capability}: it has not been evaluated yet; run it until it stops ApprovalRequired"
        );
    };
    if !requiring
        .iter()
        .any(|action| latest.actions.get(*action).map(String::as_str) == Some("approval-required"))
    {
        let statuses: Vec<String> = requiring
            .iter()
            .map(|action| {
                format!(
                    "{action} {}",
                    latest.actions.get(*action).map_or("absent", String::as_str)
                )
            })
            .collect();
        bail!(
            "the case {id} is not at the gate for {capability}: its latest decision finds {}, not approval-required; run it until it stops ApprovalRequired",
            statuses.join(", ")
        );
    }
    let Some(revision) = revision_of(&dir, &subject)? else {
        bail!("the case {id} has no {subject} revision for a grant of {capability} to authorise");
    };
    let gate = decided_at(&latest, &subject);
    if gate.as_deref() != Some(revision.as_str()) {
        bail!(
            "the case {id} is not at the gate for {capability}: its latest decision was computed at {}, and the case is at {revision} now; run it until it stops ApprovalRequired at {revision}",
            gate.as_deref().unwrap_or("an unknown revision")
        );
    }
    let mut authority = Authority::read(&dir)?.unwrap_or_default();
    if let Some(decided) = authority.decision(capability)
        && decided.applies_at(Some(&revision))
    {
        bail!(
            "the case {id} already decides {capability} at {revision}: {} by {} at {}",
            decided.decision,
            decided.approver,
            decided.granted_at
        );
    }
    // A decision on the capability at another revision authorises nothing at this one; the
    // new grant takes its place, one decision per capability.
    authority
        .decisions
        .retain(|decision| decision.capability != capability);
    let granted_at = crate::now();
    // A turn's judge-surface grant records the mainline's tip, read after its instant, so a
    // merge made before the grant is found by ancestry, never by a date.
    let mainline_tip =
        if protocol == TURN_PROTOCOL && capability == crate::validator::JUDGE_SURFACE_CAPABILITY {
            mainline_at_grant(tree, &dir, id)?
        } else {
            None
        };
    authority.decisions.push(AuthorityDecision {
        capability: capability.to_owned(),
        decision: Grant::Granted,
        approver: approver.to_owned(),
        granted_at: granted_at.clone(),
        revision: Some(revision.clone()),
        mainline_tip,
    });
    let approval = Approval {
        case: id.to_owned(),
        story,
        capability: capability.to_owned(),
        approver: approver.to_owned(),
        granted_at,
        revision,
    };
    Ok(PendingGrant {
        dir,
        authority,
        approval,
        _held: held,
    })
}

/// The commit the mainline is at now for the turn `id` at `dir` under `tree`, read through the
/// forge the turn recorded when it opened ([`turn_forge`],
/// [`crate::improvement::Mainline::recorded`]): what its judge-surface grant records. None for
/// a turn that recorded no forge, whose grant then authorises no judge-surface merge; a tip that
/// cannot be read is an error, and nothing is written then.
fn mainline_at_grant(tree: &Path, dir: &Path, id: &str) -> Result<Option<String>> {
    let main = crate::promotion::MAINLINE;
    let Some(forge) = turn_forge(dir)? else {
        return Ok(None);
    };
    let mainline = crate::improvement::Mainline::recorded(tree, &forge)?;
    mainline.tip(tree).map(Some).with_context(|| {
        format!("the tip of {main} at this grant to the turn {id} could not be read; nothing was written")
    })
}

/// What `story.yaml` holds: the story the case was opened for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoryRef {
    id: String,
}

/// The story the case at `case_dir` was opened for; `None` for a case opened on an intent.
pub fn story_of(case_dir: &Path) -> Result<Option<String>> {
    let path = case_dir.join(STORY_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    // An anchor or an alias is refused, naming the file and its line, before it deserializes.
    let story: StoryRef = crate::loop_scorecard::read_yaml(&path, &text)?;
    Ok(Some(story.id))
}

/// The inputs of the case at `case_dir` as its `case.yaml` holds them, read without taking hold
/// of the case or touching its tree, as [`CaseDir::inputs`] answers them: `name=value` pairs, a
/// boolean as `true` or `false`; none for a protocol that declares no input. For `takt replay`
/// ([`crate::replay`]), which only reads the case.
pub fn inputs_of(case_dir: &Path) -> Result<Vec<(String, String)>> {
    let path = case_dir.join("case.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    Ok(inputs_in(&doc))
}

/// The inputs a `case.yaml` document holds, `name=value` pairs, a boolean as `true` or `false`.
fn inputs_in(doc: &serde_yaml_ng::Value) -> Vec<(String, String)> {
    doc["inputs"]
        .as_mapping()
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    let value = match value {
                        serde_yaml_ng::Value::Bool(flag) => flag.to_string(),
                        serde_yaml_ng::Value::String(text) => text.clone(),
                        other => serde_yaml_ng::to_string(other).ok()?.trim().to_owned(),
                    };
                    Some((key.as_str()?.to_owned(), value))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The case revision a `case.yaml` document holds ([`CASE_REVISION`]): 1 when it holds none; one
/// that is not `r<n>` with `n` from 1 is an error.
fn case_revision(doc: &serde_yaml_ng::Value) -> Result<i64> {
    let Some(value) = doc.get(CASE_REVISION) else {
        return Ok(1);
    };
    value
        .as_str()
        .and_then(case_revision_number)
        .ok_or_else(|| anyhow!("the case revision {value:?} is not r<n>"))
}

/// The number of the case revision `text`, `r<n>` with `n` from 1; none for text of another
/// form.
fn case_revision_number(text: &str) -> Option<i64> {
    let n = text.strip_prefix('r')?.parse::<i64>().ok()?;
    (n >= 1 && text == format!("r{n}")).then_some(n)
}

/// The evidence records under `case_dir`, `evidence/*.json` and `*.yaml`, in name order, each
/// with its path.
fn evidence_files(case_dir: &Path) -> Result<Vec<(PathBuf, String)>> {
    let evidence = case_dir.join("evidence");
    let mut names: Vec<_> = std::fs::read_dir(&evidence)
        .with_context(|| format!("read {}", evidence.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort();
    names
        .into_iter()
        .filter(|path| path.extension().is_some_and(|e| e == "json" || e == "yaml"))
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            Ok((path, text))
        })
        .collect()
}

/// Writes `text` to `path` through a file beside it renamed over it, so a reader finds the old
/// text or the new one, never a part.
fn write_atomically(path: &Path, text: &str) -> Result<()> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let partial = path.with_file_name(format!(".{name}.{}.partial", std::process::id()));
    std::fs::write(&partial, text).with_context(|| format!("write {}", partial.display()))?;
    std::fs::rename(&partial, path).with_context(|| format!("write {}", path.display()))
}

/// A UUID-shaped id derived from `seed`: the first 16 bytes of its SHA-256, marked version 8
/// (custom) and the RFC 9562 variant, so one record is given the same id at every read.
fn uuid_from(seed: &str) -> String {
    let digest = Sha256::digest(seed.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// The record in `text`, read from the file `name` under the case `case`'s `evidence/`, as the
/// governor holds a received record: its facts the record itself (JSON, or YAML read as JSON),
/// its kind and producer the record's own, its provenance the file. takt's records bind to the
/// revision of the artifact they are about in their facts (`subject_revision`), which Canon
/// reads; the case revision the governor's record carries is 0, a record received before any.
/// Every one applies: [`CaseDir::evaluate`] has Canon read each before the governor does.
fn held_evidence(case: &str, name: &str, text: &str) -> Result<HeldEvidence> {
    let facts = match cjson::parse(text) {
        Ok(facts) => facts,
        Err(_) => {
            let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(text)
                .with_context(|| format!("read the evidence record {name}"))?;
            let json = serde_json::to_string(&yaml)
                .with_context(|| format!("read the evidence record {name} as JSON"))?;
            cjson::parse(&json)
                .map_err(|error| anyhow!("{error}"))
                .with_context(|| format!("read the evidence record {name} as JSON"))?
        }
    };
    let text_of = |member: &str| match facts.member(member) {
        Some(cjson::Value::Text(text)) => text.clone(),
        _ => String::new(),
    };
    let kind = text_of("kind");
    let producer = text_of("producer");
    Ok(HeldEvidence {
        data: EvidenceData {
            evidence_id: EvidenceId(Uuid(uuid_from(&format!("{case}/evidence/{name}")))),
            case_id: CaseId(case.to_owned()),
            kind,
            subject_revision: 0,
            producer,
            observation_ids: Vec::new(),
            facts,
            provenance: cjson::Value::Object(vec![(
                "file".to_owned(),
                cjson::Value::Text(format!("evidence/{name}")),
            )]),
        },
        applies: true,
    })
}

/// The governor of Loom's case store over one case's directory, `.takt/cases/<id>/`:
/// the case is read from the files takt writes (`case.yaml`, `evidence/`), and observations are
/// kept in [`OBSERVATIONS_FILE`] beside them. It holds one case, the one whose directory it is,
/// under the name its protocol is registered with the governor by (`<name>/<major>`, the
/// protocol's [`LoadedProtocol::id`]). takt opens a case ([`CaseDir::open`]) and writes its
/// evidence; the store takes no new case and changes nothing of a case but its artifacts'
/// revisions and its case revision, and a change to anything else is refused with nothing
/// written. It holds no explicit decision.
#[derive(Debug, Clone)]
pub struct CaseFiles {
    dir: PathBuf,
    id: String,
    protocol: String,
}

impl CaseFiles {
    /// The store over the case `id` at `case_dir`, its protocol registered as `protocol`.
    #[must_use]
    pub fn new(case_dir: &Path, id: &str, protocol: &str) -> Self {
        Self {
            dir: case_dir.to_path_buf(),
            id: id.to_owned(),
            protocol: protocol.to_owned(),
        }
    }

    /// The store over the case at `case_dir`: its id the directory's name, its protocol the one
    /// the case's `protocol.yaml` declares.
    pub fn at(case_dir: &Path) -> Result<Self> {
        let id = case_dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| anyhow!("the case directory {} has no name", case_dir.display()))?;
        let (protocol, _) = required_capabilities(case_dir)?;
        Ok(Self::new(case_dir, &id, &protocol))
    }

    /// The name the case's protocol is registered with the governor by, `story.delivery/1`.
    #[must_use]
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    /// The case as its files hold it now; `None` when it has no `case.yaml`.
    fn read(&self) -> Result<Option<CaseState>> {
        let path = self.dir.join("case.yaml");
        if !path.is_file() {
            return Ok(None);
        }
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
        let mut artifacts = BTreeMap::new();
        for (artifact, entry) in doc["artifacts"].as_mapping().into_iter().flatten() {
            let artifact = artifact
                .as_str()
                .ok_or_else(|| anyhow!("{}: an artifact is not named", path.display()))?;
            let revision = entry["revision"].as_str().ok_or_else(|| {
                anyhow!(
                    "{}: the artifact {artifact} has no revision",
                    path.display()
                )
            })?;
            artifacts.insert(artifact.to_owned(), revision.to_owned());
        }
        let revision = case_revision(&doc).with_context(|| format!("read {}", path.display()))?;
        let inputs = CaseInputs::try_from_pairs(inputs_in(&doc))
            .map_err(|error| anyhow!("{}: {error}", path.display()))?;
        let mut independence = CaseIndependence::none();
        for (producer, dimensions) in doc["independence"].as_mapping().into_iter().flatten() {
            let producer = producer
                .as_str()
                .ok_or_else(|| anyhow!("{}: a producer is not named", path.display()))?;
            let dimensions: Vec<&str> = dimensions
                .as_sequence()
                .into_iter()
                .flatten()
                .map(|dimension| {
                    dimension.as_str().ok_or_else(|| {
                        anyhow!("{}: a dimension of {producer} is not named", path.display())
                    })
                })
                .collect::<Result<_>>()?;
            independence = independence
                .with(producer, dimensions)
                .map_err(|error| anyhow!("{}: {error}", path.display()))?;
        }
        let evidence = evidence_files(&self.dir)?
            .iter()
            .map(|(path, text)| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                held_evidence(&self.id, &name, text)
            })
            .collect::<Result<_>>()?;
        Ok(Some(CaseState {
            id: CaseId(self.id.clone()),
            protocol: self.protocol.clone(),
            revision,
            artifacts,
            inputs,
            independence,
            evidence,
            decisions: Vec::new(),
        }))
    }

    /// Writes `state`'s artifact revisions and case revision into `case.yaml`, the rest of the
    /// file as it is; an artifact the file does not declare is refused with nothing written.
    fn write_artifacts(&self, state: &CaseState) -> Result<()> {
        let path = self.dir.join("case.yaml");
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let mut doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
        for (artifact, revision) in &state.artifacts {
            let entry = doc
                .get_mut("artifacts")
                .and_then(|artifacts| artifacts.get_mut(artifact.as_str()))
                .ok_or_else(|| anyhow!("the case declares no artifact {artifact}"))?;
            entry["revision"] = revision.as_str().into();
        }
        doc[CASE_REVISION] = format!("r{}", state.revision).into();
        let yaml = serde_yaml_ng::to_string(&doc).context("write the case snapshot")?;
        write_atomically(&path, &yaml)
    }

    /// The observations [`OBSERVATIONS_FILE`] keeps, in the order received; none when there is
    /// no file.
    fn read_observations(&self) -> Result<Vec<ObservationData>> {
        let path = self.dir.join(OBSERVATIONS_FILE);
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let value = cjson::parse(&text)
            .map_err(|error| anyhow!("{error}"))
            .with_context(|| format!("read {}", path.display()))?;
        let read = |at: &str, item: &cjson::Value| -> Result<ObservationData, cjson::DecodeError> {
            let text = |name: &str| -> Result<String, cjson::DecodeError> {
                cjson::text_at(
                    cjson::member_at(item, at, name)?,
                    &cjson::nested(at, name),
                    "a string",
                )
                .map(str::to_owned)
            };
            Ok(ObservationData {
                observation_id: ObservationId(Uuid(text("observation_id")?)),
                source: text("source")?,
                subject: text("subject")?,
                observed_at: Timestamp(text("observed_at")?),
                payload: cjson::member_at(item, at, "payload")?.clone(),
            })
        };
        cjson::items_at(&value, "$", "an array")
            .and_then(|items| {
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| read(&format!("$[{index}]"), item))
                    .collect()
            })
            .map_err(|error| anyhow!("{error}"))
            .with_context(|| format!("read {}", path.display()))
    }
}

/// `observations` as [`OBSERVATIONS_FILE`] keeps them.
fn render_observations(observations: &[ObservationData]) -> String {
    let mut out = String::from("[");
    for (index, observation) in observations.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        cjson::member(&mut out, "observation_id");
        cjson::push_text(&mut out, &observation.observation_id.0.0);
        cjson::member(&mut out, "source");
        cjson::push_text(&mut out, &observation.source);
        cjson::member(&mut out, "subject");
        cjson::push_text(&mut out, &observation.subject);
        cjson::member(&mut out, "observed_at");
        cjson::push_text(&mut out, &observation.observed_at.0);
        cjson::member(&mut out, "payload");
        cjson::push_value(&mut out, &observation.payload);
        out.push('}');
    }
    out.push_str("]\n");
    out
}

/// `document` written as Canon writes a `canon-decision/1` document: two-space indentation,
/// members in the order the document holds them, and a newline at the end; for the document
/// the governor rendered, the bytes Canon rendered.
fn render_canonical(document: &loom_sdk::governor::model::json::Value) -> String {
    use loom_sdk::governor::model::json::Value;
    fn text(text: &str, out: &mut String) {
        out.push('"');
        for c in text.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\u{8}' => out.push_str("\\b"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\u{c}' => out.push_str("\\f"),
                '\r' => out.push_str("\\r"),
                c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    fn line(indent: usize, out: &mut String) {
        out.push('\n');
        out.extend(std::iter::repeat_n(' ', indent));
    }
    fn write(value: &Value, indent: usize, out: &mut String) {
        match value {
            Value::Null => out.push_str("null"),
            Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Value::Number(spelling) => out.push_str(spelling),
            Value::Text(value) => text(value, out),
            Value::Array(items) if items.is_empty() => out.push_str("[]"),
            Value::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    line(indent + 2, out);
                    write(item, indent + 2, out);
                }
                line(indent, out);
                out.push(']');
            }
            Value::Object(members) if members.is_empty() => out.push_str("{}"),
            Value::Object(members) => {
                out.push('{');
                for (index, (name, item)) in members.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    line(indent + 2, out);
                    text(name, out);
                    out.push_str(": ");
                    write(item, indent + 2, out);
                }
                line(indent, out);
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    write(document, 0, &mut out);
    out.push('\n');
    out
}

impl FallibleCaseStore for CaseFiles {
    type Error = anyhow::Error;

    /// takt opens its cases itself ([`CaseDir::open`]): the case this store holds is already
    /// held (`false`), and any other is refused.
    fn insert(&self, state: CaseState) -> Result<bool> {
        if state.id.0 == self.id && self.dir.join("case.yaml").is_file() {
            return Ok(false);
        }
        bail!(
            "the case store at {} holds the case {} alone, opened by takt; {} is not opened here",
            self.dir.display(),
            self.id,
            state.id.0
        )
    }

    fn get(&self, case: &CaseId) -> Result<Option<CaseState>> {
        if case.0 != self.id {
            return Ok(None);
        }
        self.read()
    }

    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>> {
        let Some(state) = self.get(case)? else {
            return Ok(None);
        };
        let mut changed = state.clone();
        let answer = change(&mut changed);
        if changed.id != state.id
            || changed.protocol != state.protocol
            || changed.inputs != state.inputs
            || changed.independence != state.independence
            || changed.evidence != state.evidence
            || changed.decisions != state.decisions
        {
            bail!(
                "the case {} changes only its artifacts' revisions here: its evidence is written under evidence/ by takt, and it holds no explicit decision; nothing was written",
                self.id
            );
        }
        if changed.artifacts != state.artifacts || changed.revision != state.revision {
            self.write_artifacts(&changed)?;
        }
        Ok(Some(answer))
    }

    fn observe(&self, observation: ObservationData) -> Result<()> {
        let mut observations = self.read_observations()?;
        observations.push(observation);
        write_atomically(
            &self.dir.join(OBSERVATIONS_FILE),
            &render_observations(&observations),
        )
    }

    fn observations(&self) -> Result<Vec<ObservationData>> {
        self.read_observations()
    }
}

/// The authority provider of a case: what Commission asks, at the moment an action that requires
/// a capability is requested, and what [`CaseDir::evaluate`] asks of every action the governor
/// finds approval-required. It answers from the case's `authority.yaml` and its files as they
/// are when asked. A denial on file is denied at every revision. A grant is allowed only while
/// the case is at the case revision the request expected, and its subject (the artifact the
/// capability's action is about, [`LoadedProtocol::grant_subject`]) at the revision the grant
/// was given at ([`Authority::grants_at`]); otherwise the capability is approval-required. The
/// request names no artifact revision, so the subject's revision is read from the case's files.
/// An action that does not require the capability asked about is denied it. The merge validator
/// still merges only under [`holds_grant`]. A decision on file is answered by the same rule
/// over the case as the decision was computed at, for `takt replay`
/// ([`CaseAuthority::answer_recorded`]).
#[derive(Debug, Clone)]
pub struct CaseAuthority {
    dir: PathBuf,
    id: String,
}

impl CaseAuthority {
    /// The provider of the case `id` at `case_dir`.
    #[must_use]
    pub fn new(case_dir: &Path, id: &str) -> Self {
        Self {
            dir: case_dir.to_path_buf(),
            id: id.to_owned(),
        }
    }

    /// Whether `action` is authorised `capability` with the case at `expected_case_revision`.
    pub fn verdict(
        &self,
        action: &str,
        capability: &str,
        expected_case_revision: i64,
    ) -> Result<AuthorityVerdict> {
        self.verdict_over(action, capability, expected_case_revision, || {
            let Some(state) = CaseFiles::at(&self.dir)?.get(&CaseId(self.id.clone()))? else {
                bail!("no case {} at {}", self.id, self.dir.display());
            };
            Ok((state.revision, state.artifacts))
        })
    }

    /// The decision `recorded`, a `canon-decision/1` document the governor rendered for the
    /// case, with each action it finds approval-required answered as [`CaseDir::evaluate`]
    /// answers it (`admissible` where the capability is allowed, `blocked` where it is denied),
    /// by the rule of [`CaseAuthority::verdict`] over the case as the document says it was
    /// computed from (`explanation.computed_from.case`): its case revision, both the one
    /// expected and the one held, and its artifacts' revisions, in place of the case's files
    /// now; `authority.yaml` is read as it is now. The capability asked about is the first the
    /// action requires in the case's protocol, the one the governor names. A document that
    /// names no case revision it was computed at is answered as it is, granting nothing.
    pub fn answer_recorded(&self, recorded: &Decision) -> Result<Decision> {
        let doc: serde_json::Value =
            serde_json::from_str(&recorded.json).context("read the decision document")?;
        let case = &doc["explanation"]["computed_from"]["case"];
        let mut answered = recorded.clone();
        let Some(revision) = case[CASE_REVISION].as_str() else {
            return Ok(answered);
        };
        let revision = case_revision_number(revision).ok_or_else(|| {
            anyhow!("the decision was computed at the case revision {revision:?}, not r<n>")
        })?;
        let artifacts: BTreeMap<String, String> = case["artifacts"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(artifact, entry)| {
                Some((artifact.clone(), entry["revision"].as_str()?.to_owned()))
            })
            .collect();
        let (_, actions) = required_capabilities(&self.dir)?;
        for (action, status) in &recorded.actions {
            if status != "approval-required" {
                continue;
            }
            let Some(capability) = actions.get(action).and_then(|required| required.first()) else {
                continue;
            };
            let verdict = self.verdict_over(action, capability, revision, || {
                Ok((revision, artifacts.clone()))
            })?;
            if let Some(status) = answered_status(&verdict) {
                answered.actions.insert(action.clone(), status.to_owned());
            }
        }
        Ok(answered)
    }

    /// [`CaseAuthority::verdict`] with the case held as `held` answers it, its case revision
    /// and its artifacts' revisions, read only once a denial on file has not decided it.
    fn verdict_over(
        &self,
        action: &str,
        capability: &str,
        expected_case_revision: i64,
        held: impl FnOnce() -> Result<(i64, BTreeMap<String, String>)>,
    ) -> Result<AuthorityVerdict> {
        let approval = |request: String| {
            AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired { request })
        };
        let (_, actions) = required_capabilities(&self.dir)?;
        if !actions
            .get(action)
            .is_some_and(|required| required.iter().any(|c| c == capability))
        {
            return Ok(AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: format!(
                    "{action} does not require {capability} in the protocol of the case {}",
                    self.id
                ),
            }));
        }
        let authority = Authority::read(&self.dir)?.unwrap_or_default();
        if let Some(decided) = authority.decision(capability)
            && decided.decision == Grant::Denied
        {
            return Ok(AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: format!(
                    "{capability} is denied to the case {} by {} at {}",
                    self.id, decided.approver, decided.granted_at
                ),
            }));
        }
        let (held_revision, artifacts) = held()?;
        if held_revision != expected_case_revision {
            return Ok(approval(format!(
                "{capability} for {action}: the case {} is at case revision {held_revision}, not {expected_case_revision}",
                self.id
            )));
        }
        let protocol = LoadedProtocol::from_file(&self.dir.join("protocol.yaml"))?;
        let Ok(subject) = protocol.grant_subject(capability) else {
            return Ok(approval(format!(
                "{capability} for {action}: its actions are about no single artifact"
            )));
        };
        let Some(revision) = artifacts.get(&subject) else {
            return Ok(approval(format!(
                "{capability} for {action}: the case has no {subject} revision"
            )));
        };
        if authority.grants_at(capability, revision) {
            Ok(AuthorityVerdict::Allow(Unit(true)))
        } else {
            Ok(approval(format!(
                "{capability} for {action} at {subject} {revision}"
            )))
        }
    }
}

/// The status an evaluation gives an action the governor finds approval-required once the
/// case's authority has answered for it: `admissible` where it allows the capability, `blocked`
/// where it denies it; none where an approval is still required, and the action keeps its status.
fn answered_status(verdict: &AuthorityVerdict) -> Option<&'static str> {
    match verdict {
        AuthorityVerdict::Allow(_) => Some("admissible"),
        AuthorityVerdict::Deny(_) => Some("blocked"),
        AuthorityVerdict::ApprovalRequired(_) => None,
    }
}

impl AuthorityProvider for CaseAuthority {
    fn decide(
        &self,
        commission: &CommissionData,
        request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        if request.case_id.0 != self.id || commission.case_id.0 != self.id {
            return Err(AuthorityProviderError::new(format!(
                "the authority of the case {} answers for it alone, not for {} under the commission of {}",
                self.id, request.case_id.0, commission.case_id.0
            )));
        }
        self.verdict(&request.action, capability, request.expected_case_revision)
            .map_err(|error| AuthorityProviderError::new(format!("{error:#}")))
    }
}

/// The case's `case.yaml` at `case_dir` with its `implementation` artifact at `revision`, the
/// rest as it is: the snapshot a step that started at `revision` was taken against, for
/// `takt replay` ([`crate::replay`]) to write into its scratch tree. The case's file is only
/// read. A case whose protocol declares no `implementation` artifact is answered as it is.
pub fn snapshot_at(case_dir: &Path, revision: &str) -> Result<String> {
    let path = case_dir.join("case.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    match doc
        .get_mut("artifacts")
        .and_then(|artifacts| artifacts.get_mut("implementation"))
    {
        Some(entry) => {
            entry["revision"] = revision.into();
            serde_yaml_ng::to_string(&doc).context("write the case snapshot")
        }
        None => Ok(text),
    }
}

/// The latest decision written under the case, the `decisions/<n>.json` with the highest
/// sequence number `n`, read as a number, so `10000.json` comes after `9999.json`; a file whose
/// name is not a number is no decision. `None` before the first evaluation.
pub fn latest_decision(case_dir: &Path) -> Result<Option<Decision>> {
    let decisions = case_dir.join("decisions");
    if !decisions.is_dir() {
        return Ok(None);
    }
    let mut latest: Option<(u64, PathBuf)> = None;
    for entry in
        std::fs::read_dir(&decisions).with_context(|| format!("read {}", decisions.display()))?
    {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let Some(n) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.parse::<u64>().ok())
        else {
            continue;
        };
        if latest.as_ref().is_none_or(|(highest, _)| n > *highest) {
            latest = Some((n, path));
        }
    }
    let Some((_, latest)) = latest else {
        return Ok(None);
    };
    let json =
        std::fs::read_to_string(&latest).with_context(|| format!("read {}", latest.display()))?;
    Decision::from_json(&json)
        .with_context(|| format!("read {}", latest.display()))
        .map(Some)
}

/// Whether the case at `case_dir` is finished: its latest decision has a legitimate outcome.
/// A finished case is not an open case; it stays on disk, and `takt run --case <id>` still
/// resumes it, but the frontier does not.
pub fn is_finished(case_dir: &Path) -> Result<bool> {
    Ok(latest_decision(case_dir)?.is_some_and(|decision| !decision.legitimate().is_empty()))
}

/// The open case to resume for one of the stories `active`: the first case directory under
/// `tree`, in name order, that records a story among them and is not finished, as `(case id,
/// story id)`. A story has at most one open case, so the first found is the one; a finished
/// case is skipped, so a story whose case completed and which the store still lists `active`
/// (until the record story moves it) holds up no other story. A case whose latest decision
/// does not read (a write cut short) counts as open, and `out` says so: one case's trouble is
/// not the frontier's.
pub fn resumable(
    tree: &Path,
    active: &[String],
    out: &mut dyn Write,
) -> Result<Option<(String, String)>> {
    if active.is_empty() {
        return Ok(None);
    }
    let dirs: Vec<PathBuf> = case_dirs(tree)?.into_iter().map(|(_, dir)| dir).collect();
    for dir in dirs {
        if !dir.join("case.yaml").is_file() {
            continue;
        }
        if let Some(story) = story_of(&dir)?
            && active.contains(&story)
        {
            let id = dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            match is_finished(&dir) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => writeln!(
                    out,
                    "frontier: the case {id} has a decision that does not read ({error:#}); taken as open"
                )?,
            }
            return Ok(Some((id, story)));
        }
    }
    Ok(None)
}

/// The most approval commits [`approvals_only`] walks back through.
const APPROVALS_AT_MOST: usize = 64;

/// The largest file [`approvals_only`] reads as a record; the store's record of an approval is
/// a few hundred bytes.
const RECORD_AT_MOST: u64 = 64 << 10;

/// The directory under the case directory where the run keeps what each of its store writes on
/// the case's story committed, one directory per implementation revision the case was at:
/// `riders/<revision>/`[`KEPT_TEST_RESULT`] and `riders/<revision>/`[`KEPT_MOVE`]. Kept by the
/// run only ([`keep_rider`]), in the case directory no step may change; what the rider rule
/// ([`approvals_only`]) compares a riding file with, byte for byte.
pub const RIDERS_DIR: &str = "riders";

/// The kept `test_result` record the run had the store write on the story.
pub const KEPT_TEST_RESULT: &str = "test_result.json";

/// The kept story file as the run's move to `implemented` left it.
pub const KEPT_MOVE: &str = "implemented.md";

/// Where the case at `case_dir` keeps the file `name` of its store writes at `revision`; `None`
/// for a revision that is not a commit id, which names no directory.
fn kept_path(case_dir: &Path, revision: &str, name: &str) -> Option<PathBuf> {
    (!revision.is_empty() && revision.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| case_dir.join(RIDERS_DIR).join(revision).join(name))
}

/// The suffix of the file beside a kept record that holds the path, relative to the tree, the
/// store's write committed it at.
const KEPT_PATH_SUFFIX: &str = ".path";

/// Whether the case at `case_dir` keeps the file `name` of a store write at `revision`
/// ([`keep_rider`]): the record and the path it was committed at, both regular files.
#[must_use]
pub fn is_kept_at(case_dir: &Path, revision: &str, name: &str) -> bool {
    let regular = |path: Option<PathBuf>| {
        path.and_then(|path| std::fs::symlink_metadata(path).ok())
            .is_some_and(|meta| meta.is_file())
    };
    regular(kept_path(case_dir, revision, name))
        && regular(kept_path(
            case_dir,
            revision,
            &format!("{name}{KEPT_PATH_SUFFIX}"),
        ))
}

/// Keeps in the case at `case_dir`, as `name` at `revision`, the one file the store's write
/// `commit` on the case's branch in `tree` added or changed against its one parent, read from
/// the commit, and beside it, as `<name>.path`, the path it was committed at: what
/// [`approvals_only`] lets ride above `revision`, at that path and byte for byte. A commit
/// with another number of parents, or that changes no file or more than one, keeps nothing and
/// is the error. Answers the paths kept.
pub fn keep_rider(
    tree: &Path,
    case_dir: &Path,
    revision: &str,
    commit: &str,
    name: &str,
) -> Result<[PathBuf; 2]> {
    let (Some(path), Some(at)) = (
        kept_path(case_dir, revision, name),
        kept_path(case_dir, revision, &format!("{name}{KEPT_PATH_SUFFIX}")),
    ) else {
        bail!("the revision {revision} is not a commit id; nothing kept");
    };
    let read = crate::git::commit(tree, commit)?;
    let [parent] = read.parents.as_slice() else {
        bail!("the store's write {commit} has no one parent; nothing kept");
    };
    let Some(changes) = changes(tree, parent, commit)? else {
        bail!("the changes of the store's write {commit} do not read; nothing kept");
    };
    let [change] = changes.as_slice() else {
        bail!(
            "the store's write {commit} changed {} files, not one; nothing kept",
            changes.len()
        );
    };
    let bytes = crate::git::blob(tree, &change.blob)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    std::fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    std::fs::write(&at, change.path.as_bytes())
        .with_context(|| format!("write {}", at.display()))?;
    Ok([path, at])
}

/// Whether `change` is, at its path and byte for byte, the file `name` the case at `case_dir`
/// keeps at `revision` ([`keep_rider`]): the record and its path regular files, never followed
/// through a link, the blob of the record's size; a file not kept matches nothing.
fn is_kept(
    tree: &Path,
    case_dir: &Path,
    revision: &str,
    name: &str,
    change: &Change,
) -> Result<bool> {
    if !is_kept_at(case_dir, revision, name) {
        return Ok(false);
    }
    let (Some(path), Some(at)) = (
        kept_path(case_dir, revision, name),
        kept_path(case_dir, revision, &format!("{name}{KEPT_PATH_SUFFIX}")),
    ) else {
        return Ok(false);
    };
    let kept_at = std::fs::read(&at).with_context(|| format!("read {}", at.display()))?;
    if kept_at != change.path.as_bytes() {
        return Ok(false);
    }
    let blob = change.blob.as_str();
    let meta =
        std::fs::symlink_metadata(&path).with_context(|| format!("read {}", path.display()))?;
    let size = crate::git::run(tree, &["cat-file", "-s", blob])?;
    if size.parse::<u64>().ok() != Some(meta.len()) {
        return Ok(false);
    }
    let kept = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    Ok(crate::git::blob(tree, blob)? == kept)
}

/// Whether `head` reaches `revision` only through the riders of the story of the case at
/// `case_dir`: commits takt makes for the story that carry its records to the base with the
/// pull request, each with one parent, read from the commit (never from the tree), and
/// nothing else. Three kinds ride, told by their subjects:
///
/// - an approval's, [`crate::frontier::approve_subject`] of the story and one capability: every
///   change the addition of a new regular file directly in the story's evidence directory
///   (`.engineering/evidence/story/<name>/`), each exactly the store's record, no field more,
///   of a grant of that capability the case holds at `revision` by the one rule that counts a
///   grant ([`Authority::grants_at`]), by its approver as its actor ([`StoreRecord::records`]);
///   no grant is recorded twice, so there are no more such commits than grants;
/// - one `test_result`, [`crate::frontier::test_result_subject`]: one change, the addition of a
///   new regular file directly in the story's evidence directory, at the path and byte for byte
///   the record the case keeps at `revision` as [`KEPT_TEST_RESULT`];
/// - one move to `implemented`, [`crate::frontier::implemented_subject`], only as the last,
///   `head` itself, and only with the `test_result` below it: one change, the story's own file
///   in the planning store changed, byte for byte the file the case keeps at `revision` as
///   [`KEPT_MOVE`].
///
/// The case stays at `revision`, the one the grant was given at and every claim is about;
/// `pr.open` and `repository.merge` push `HEAD` with the riders above it and merge that head,
/// so the store's records of the story reach the base with the pull request and nothing else
/// rides along. A commit that does anything else, a file that is any other record or not
/// byte-equal to the one kept at its path, a grant recorded twice, a second `test_result`, a
/// move that is not last, without the `test_result` below it or a second one, or a case with
/// no story, rides on nothing.
pub fn approvals_only(tree: &Path, revision: &str, head: &str, case_dir: &Path) -> Result<bool> {
    let Some(story) = story_of(case_dir)? else {
        return Ok(false);
    };
    let case = case_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // The grants at `revision` by the rule `grants_at` reads, the one decision on each
    // capability `decision` answers, not yet found recorded; each record found takes its grant
    // out.
    let authority = Authority::read(case_dir)?.unwrap_or_default();
    let mut unrecorded: Vec<Approval> = authority
        .decisions
        .iter()
        .filter(|decision| {
            authority
                .decision(&decision.capability)
                .is_some_and(|first| std::ptr::eq(first, *decision))
                && authority.grants_at(&decision.capability, revision)
        })
        .map(|decision| Approval {
            case: case.clone(),
            story: Some(story.clone()),
            capability: decision.capability.clone(),
            approver: decision.approver.clone(),
            granted_at: decision.granted_at.clone(),
            revision: revision.to_owned(),
        })
        .collect();
    let grants = unrecorded.len();
    let prefix = crate::frontier::approve_subject(&story, "");
    let test_result = crate::frontier::test_result_subject(&story);
    let moved = crate::frontier::implemented_subject(&story);
    let path = story.replacen(':', "/", 1);
    let evidence = format!(".engineering/evidence/{path}/");
    let story_file = format!(".engineering/planning/{path}.md");
    if head == revision {
        return Ok(true);
    }
    let Some(story_revision) = story_revision_at(tree, revision, &story)? else {
        return Ok(false);
    };
    let mut test_result_rode = false;
    // Whether the head is the move, which rides only with the `test_result` below it.
    let mut moved_last = false;
    let mut at = head.to_owned();
    // One approval's commit per grant, one `test_result` and one move, then `revision`.
    for depth in 0..=grants.min(APPROVALS_AT_MOST) + 2 {
        if at == revision {
            return Ok(!moved_last || test_result_rode);
        }
        let commit = crate::git::commit(tree, &at)?;
        let [parent] = commit.parents.as_slice() else {
            return Ok(false);
        };
        let Some(changes) = changes(tree, parent, &at)? else {
            return Ok(false);
        };
        let rides = if commit.subject == moved {
            moved_last = true;
            depth == 0
                && match changes.as_slice() {
                    [change] => {
                        change.status == "M"
                            && change.from_mode == "100644"
                            && change.to_mode == "100644"
                            && change.path == story_file
                            && is_kept(tree, case_dir, revision, KEPT_MOVE, change)?
                    }
                    _ => false,
                }
        } else if commit.subject == test_result {
            !std::mem::replace(&mut test_result_rode, true)
                && match changes.as_slice() {
                    [change] => {
                        change.adds_a_record_in(&evidence)
                            && is_kept(tree, case_dir, revision, KEPT_TEST_RESULT, change)?
                    }
                    _ => false,
                }
        } else if let Some(capability) = commit.subject.strip_prefix(&prefix).filter(|capability| {
            !capability.is_empty() && !capability.contains(char::is_whitespace)
        }) {
            records_grants(
                tree,
                &changes,
                &evidence,
                capability,
                story_revision,
                &mut unrecorded,
            )?
        } else {
            false
        };
        if !rides {
            return Ok(false);
        }
        at.clone_from(parent);
    }
    Ok(false)
}

/// Whether `changes`, those of an approval's commit naming `capability`, are at least one and
/// each adds a new regular file directly in `evidence` that is the store's record of a grant of
/// that capability among `unrecorded` ([`StoreRecord::records`]); each grant found recorded is
/// taken out of `unrecorded`.
fn records_grants(
    tree: &Path,
    changes: &[Change],
    evidence: &str,
    capability: &str,
    story_revision: u64,
    unrecorded: &mut Vec<Approval>,
) -> Result<bool> {
    if changes.is_empty()
        || !changes
            .iter()
            .all(|change| change.adds_a_record_in(evidence))
    {
        return Ok(false);
    }
    for change in changes {
        let Some(record) = record_at(tree, &change.blob)? else {
            return Ok(false);
        };
        let Some(granted) = unrecorded.iter().position(|approval| {
            approval.capability == capability && record.records(approval, story_revision)
        }) else {
            return Ok(false);
        };
        unrecorded.swap_remove(granted);
    }
    Ok(true)
}

/// The blob `blob` read as a [`StoreRecord`], its bytes as git holds them: `None` when it is
/// larger than [`RECORD_AT_MOST`] or is no record.
fn record_at(tree: &Path, blob: &str) -> Result<Option<StoreRecord>> {
    let size = crate::git::run(tree, &["cat-file", "-s", blob])?;
    if size
        .parse::<u64>()
        .ok()
        .is_none_or(|size| size > RECORD_AT_MOST)
    {
        return Ok(None);
    }
    let bytes = crate::git::blob(tree, blob)?;
    Ok(serde_json::from_slice(&bytes).ok())
}

/// One change of a commit against its parent, as `git diff-tree --raw` reports it.
struct Change {
    from_mode: String,
    to_mode: String,
    blob: String,
    status: String,
    path: String,
}

impl Change {
    /// Whether the change adds a new regular file directly in the directory `evidence` (ending
    /// in `/`): the shape of a record the store writes.
    fn adds_a_record_in(&self, evidence: &str) -> bool {
        let name = self.path.strip_prefix(evidence).unwrap_or_default();
        self.status == "A"
            && self.from_mode == "000000"
            && self.to_mode == "100644"
            && !name.is_empty()
            && !name.contains('/')
    }
}

/// Every change from `parent` to `commit`, read from `git diff-tree --raw -z` byte for byte
/// ([`crate::git::bytes`]), not from quoted, trimmed or decoded text; `None` when the output
/// does not read, a path that is not UTF-8 among it.
fn changes(tree: &Path, parent: &str, commit: &str) -> Result<Option<Vec<Change>>> {
    let raw = crate::git::bytes(
        tree,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--no-renames",
            "--raw",
            parent,
            commit,
            "--",
        ],
    )?;
    let mut fields = raw
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let mut changes = Vec::new();
    while let Some(change) = fields.next() {
        // A field that is not UTF-8, a path above all, does not read: never a decoded stand-in.
        let (Ok(change), Some(Ok(path))) = (
            std::str::from_utf8(change),
            fields.next().map(std::str::from_utf8),
        ) else {
            return Ok(None);
        };
        let parts: Vec<&str> = change.trim_start_matches(':').split(' ').collect();
        let [from_mode, to_mode, _, blob, status] = parts.as_slice() else {
            return Ok(None);
        };
        changes.push(Change {
            from_mode: (*from_mode).to_owned(),
            to_mode: (*to_mode).to_owned(),
            blob: (*blob).to_owned(),
            status: (*status).to_owned(),
            path: path.to_owned(),
        });
    }
    Ok(Some(changes))
}

/// The directory of the case whose branch `tree` is on: `takt/<id>` names the case `<id>`. A
/// refusal shows the branch as [`check_id`] shows an id, a character that does not show written
/// as its code point.
pub fn current(tree: &Path) -> Result<PathBuf> {
    let branch = crate::git::current_branch(tree)?
        .ok_or_else(|| anyhow!("the tree {} is not on a branch", tree.display()))?;
    let id = branch.strip_prefix(BRANCH_PREFIX).ok_or_else(|| {
        anyhow!(
            "the tree {} is on {}, not on a case's branch {BRANCH_PREFIX}<id>",
            tree.display(),
            shown(&branch)
        )
    })?;
    check_id(id).with_context(|| format!("the branch {} names no case", shown(&branch)))?;
    let dir = CaseDir::locate(tree, id);
    if !dir.join("case.yaml").is_file() {
        bail!(
            "the branch {} names no case at {}",
            shown(&branch),
            dir.display()
        );
    }
    Ok(dir)
}

/// Refuses the case `id` under `tree` when a link stands on the way to its directory under
/// either root: `.takt` or `.engineering`, the cases directory or the case's own directory.
/// What a link points at could be anywhere, in the tree or out of it, and a run snapshots the
/// cases directories without following one, so a case reached through a link would be read and
/// written where no snapshot sees it. `what` says what the case is not: `opened`, `resumed`.
pub(crate) fn refuse_link_on_the_way(tree: &Path, id: &str, what: &str) -> Result<()> {
    for root in CASE_ROOTS {
        let mut way = PathBuf::new();
        for part in Path::new(root).join(id).components() {
            way.push(part);
            let at = tree.join(&way);
            if std::fs::symlink_metadata(&at).is_ok_and(|meta| meta.file_type().is_symlink()) {
                let target = std::fs::read_link(&at)
                    .with_context(|| format!("read the link {}", at.display()))?;
                bail!(
                    "the case {id} is not {what}: {} is a link to {}; a case is never read or written through a link",
                    way.display(),
                    target.display()
                );
            }
        }
    }
    Ok(())
}

/// Writes [`TAKT_GITIGNORE`] under `tree`'s [`TAKT_DIR`] when there is none, creating the
/// directory as needed. A file already there is left as it is, byte for byte.
fn write_takt_gitignore(tree: &Path) -> Result<()> {
    let takt = tree.join(TAKT_DIR);
    let path = takt.join(TAKT_GITIGNORE);
    if std::fs::symlink_metadata(&path).is_ok() {
        return Ok(());
    }
    std::fs::create_dir_all(&takt).with_context(|| format!("create {}", takt.display()))?;
    std::fs::write(&path, TAKT_GITIGNORE_TEXT).with_context(|| format!("write {}", path.display()))
}

/// The records profile a run keeps its case's decisions under: `default` keeps the latest
/// `decisions/NNNN.json` alone, `trial` every one. Named by `--records` and the run
/// configuration's `records` (story:a-run-keeps-every-decision-only-on-a-trial).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Records {
    /// Only the latest decision is kept; the sequence still continues from it.
    #[default]
    Default,
    /// Every decision is kept, as every run kept them before the profile.
    Trial,
}

impl Records {
    /// The profile `name` names, `default` or `trial`; any other name is none.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::Default),
            "trial" => Some(Self::Trial),
            _ => None,
        }
    }

    /// The profile's name as the flag and the run configuration spell it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Trial => "trial",
        }
    }

    /// The profile a case of protocol `protocol` keeps when `self` is asked for: an improvement
    /// case (`self.improvement/1`, `loop.improvement/1`) keeps every decision whatever is asked,
    /// since its scorecard reads every one of them; any other keeps what is asked. A scenario
    /// run is asked for `trial` by its caller.
    #[must_use]
    pub fn for_protocol(self, protocol: &str) -> Self {
        if protocol == TURN_PROTOCOL || protocol == LOOP_PROTOCOL {
            Self::Trial
        } else {
            self
        }
    }

    /// The profile a case `id` of protocol `protocol` keeps when `self` is asked for: every
    /// decision of a scenario case ([`is_scenario_case`]) whatever is asked, and of an improvement
    /// case ([`Records::for_protocol`]); what is asked otherwise. The one rule a run keeps its
    /// decisions by, a new run and a resumed one alike.
    #[must_use]
    pub fn for_case(self, protocol: &str, id: &str) -> Self {
        if is_scenario_case(id) {
            Self::Trial
        } else {
            self.for_protocol(protocol)
        }
    }
}

/// Whether the case `id` is a scenario run's: `scenario-<scenario>-trial-<n>`, the case a
/// `takt scenario run` opens for each trial of a scenario, whose scorecard reads every decision
/// it made.
#[must_use]
pub fn is_scenario_case(id: &str) -> bool {
    id.strip_prefix("scenario-")
        .and_then(|rest| rest.rsplit_once("-trial-"))
        .is_some_and(|(scenario, n)| !scenario.is_empty() && n.parse::<u128>().is_ok())
}

/// The parts of the `canon-decision/1` document the governor of Loom rendered for a case that
/// the loop acts on: claim values, obligation, action and outcome statuses
/// ([`CaseDir::evaluate`]).
#[derive(Debug, Clone)]
pub struct Decision {
    /// Each claim's value: `true`, `false` or `unknown`.
    pub claims: BTreeMap<String, String>,
    /// Each obligation's status: `open`, `discharged` or `not_applicable`.
    pub obligations: BTreeMap<String, String>,
    /// Each action's status: `admissible`, `approval-required` or `blocked`.
    pub actions: BTreeMap<String, String>,
    /// Each outcome's status: `legitimate` or `blocked`.
    pub outcomes: BTreeMap<String, String>,
    /// The whole document, canonical JSON.
    pub json: String,
}

impl Decision {
    /// Reads the rendered decision document.
    pub fn from_json(json: &str) -> Result<Self> {
        let doc: serde_json::Value =
            serde_json::from_str(json).context("read the decision document")?;
        // A section is a map from id to entry, or a list of entries carrying their `id`.
        let statuses = |section: &str, key: &str| -> BTreeMap<String, String> {
            let value = |entry: &serde_json::Value| {
                entry[key]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| entry.to_string())
            };
            match &doc[section] {
                serde_json::Value::Object(map) => map
                    .iter()
                    .map(|(id, entry)| (id.clone(), value(entry)))
                    .collect(),
                serde_json::Value::Array(list) => list
                    .iter()
                    .filter_map(|entry| {
                        entry["id"].as_str().map(|id| (id.to_owned(), value(entry)))
                    })
                    .collect(),
                _ => BTreeMap::new(),
            }
        };
        Ok(Self {
            claims: statuses("claims", "value"),
            obligations: statuses("obligations", "status"),
            actions: statuses("actions", "status"),
            outcomes: statuses("outcomes", "status"),
            json: json.to_owned(),
        })
    }

    /// The admissible actions, in id order.
    #[must_use]
    pub fn admissible(&self) -> Vec<String> {
        self.actions
            .iter()
            .filter(|(_, status)| status.as_str() == "admissible")
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// The actions that need an approval, in id order.
    #[must_use]
    pub fn approval_required(&self) -> Vec<String> {
        self.actions
            .iter()
            .filter(|(_, status)| status.as_str() == "approval-required")
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// The legitimate outcomes, in id order.
    #[must_use]
    pub fn legitimate(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|(_, status)| status.as_str() == "legitimate")
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Whether `claim` is `true`.
    #[must_use]
    pub fn holds(&self, claim: &str) -> bool {
        self.claims.get(claim).is_some_and(|value| value == "true")
    }

    /// One line: each action with its status.
    #[must_use]
    pub fn frontier_line(&self) -> String {
        let parts: Vec<String> = self
            .actions
            .iter()
            .map(|(id, status)| format!("{id} ({status})"))
            .collect();
        format!("frontier: {}", parts.join(", "))
    }
}

/// One case's files.
#[derive(Debug, Clone)]
pub struct CaseDir {
    /// The case id.
    pub id: String,
    /// `<tree>/.takt/cases/<id>`, or `<tree>/.engineering/cases/<id>` for a case that is there
    /// ([`CaseDir::locate`]).
    pub dir: PathBuf,
    /// The case's branch, `takt/<id>`.
    pub branch: String,
    /// The worktree.
    tree: PathBuf,
    /// The protocol the case runs under.
    pub protocol: LoadedProtocol,
    /// The case snapshot as a YAML document tree.
    doc: serde_yaml_ng::Value,
    /// Evidence records written so far.
    written: usize,
}

impl CaseDir {
    /// The directory of case `id` under `tree`: under [`CASES_DIR`] when it is there, else under
    /// [`LEGACY_CASES_DIR`] when it is there, so a case already under the legacy root is read and
    /// written where it is. A case in neither is one a new open makes, so the directory under
    /// [`CASES_DIR`] is named. Nothing is created or read beyond the two existence checks.
    #[must_use]
    pub fn locate(tree: &Path, id: &str) -> PathBuf {
        let new = tree.join(CASES_DIR).join(id);
        if std::fs::symlink_metadata(&new).is_ok() {
            return new;
        }
        let legacy = tree.join(LEGACY_CASES_DIR).join(id);
        if std::fs::symlink_metadata(&legacy).is_ok() {
            legacy
        } else {
            new
        }
    }

    /// Opens a new case under `tree` on `protocol` for `intent`: the `intent` artifact at the
    /// SHA-256 of the intent text, `implementation` at `HEAD`, every other artifact at `r0`, and
    /// the inputs given; a declared input that is not given is refused, naming its values: the
    /// kernel takes no default, because the inputs select the obligations the case owes. The
    /// case's branch `takt/<id>` is created and checked out at the tip of its base (the branch
    /// the tree is on, or that case's base when the tree is on another case's branch, so a
    /// case never forks from another case's commits; a case branch whose base is unknown
    /// refuses the case before anything is written), and recorded with its base and the
    /// branch it left as `branch.yaml`; a branch that exists refuses the case. A tree with
    /// uncommitted work outside the cases directory (staged, unstaged or untracked; ignored
    /// files are not work) refuses the case before anything is touched: everything a run
    /// commits or discards afterwards is then its own. Nothing is written until the tree,
    /// `HEAD`, the inputs and the branch are in order, and an open that fails after the branch
    /// was made takes the directory, the cases directory when it was the last, and the branch
    /// with it, the tree back where it was, so a case directory exists only with its
    /// `case.yaml` and a `takt/<id>` branch only with its case. Its commits are takt's own
    /// ([`CommitAs::Takt`]); [`CaseDir::open_as`] opens one with another identity.
    pub fn open(
        tree: &Path,
        protocol: LoadedProtocol,
        id: &str,
        intent: &str,
        inputs: &[(String, String)],
    ) -> Result<Self> {
        Self::open_as(tree, protocol, id, intent, inputs, &CommitAs::Takt)
    }

    /// [`CaseDir::open`], the case's commits carrying the identity `commit_as` says, recorded
    /// in the case ([`COMMIT_IDENTITY_RECORD`]). [`CommitAs::Configured`] reads it at the commit
    /// the case's branch forks from, once the tree is found clean and before the branch is made;
    /// one that must be there and is not refuses the case with nothing written.
    pub fn open_as(
        tree: &Path,
        protocol: LoadedProtocol,
        id: &str,
        intent: &str,
        inputs: &[(String, String)],
        commit_as: &CommitAs,
    ) -> Result<Self> {
        check_id(id)?;
        refuse_link_on_the_way(tree, id, "opened")?;
        let dir = tree.join(CASES_DIR).join(id);
        let existing = CaseDir::locate(tree, id);
        if std::fs::symlink_metadata(&existing).is_ok() {
            return Err(anyhow!(
                "the case {id} already exists at {}",
                existing.display()
            ));
        }
        let uncommitted = crate::git::uncommitted(tree)?;
        if !uncommitted.is_empty() {
            const SHOWN: usize = 10;
            let mut named = uncommitted
                .iter()
                .take(SHOWN)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            if uncommitted.len() > SHOWN {
                named.push_str(&format!(" and {} more", uncommitted.len() - SHOWN));
            }
            bail!(
                "the tree {} has uncommitted work: {named}; a case opens only on a clean tree, so that nothing a run commits or discards is anyone else's: commit it, or move it out of the tree, first",
                tree.display()
            );
        }
        let head = crate::git::head(tree)?;
        let mut artifacts = serde_yaml_ng::Mapping::new();
        for artifact in &protocol.artifacts {
            let revision = match artifact.as_str() {
                "intent" => format!("{:x}", Sha256::digest(intent.as_bytes())),
                "implementation" => head.clone(),
                _ => "r0".to_owned(),
            };
            let mut entry = serde_yaml_ng::Mapping::new();
            entry.insert("revision".into(), revision.into());
            artifacts.insert(artifact.as_str().into(), entry.into());
        }
        let mut doc = serde_yaml_ng::Mapping::new();
        doc.insert("format".into(), "canon-case/1".into());
        doc.insert("id".into(), id.into());
        doc.insert("protocol".into(), protocol.name.as_str().into());
        doc.insert("artifacts".into(), artifacts.into());
        if !protocol.inputs.is_empty() {
            let mut values = serde_yaml_ng::Mapping::new();
            for (input, allowed) in &protocol.inputs {
                let given = inputs
                    .iter()
                    .find(|(k, _)| k == input)
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| {
                        anyhow!("the case input {input} is not given; it takes one of {allowed:?}")
                    })?;
                if !allowed.is_empty() && !allowed.contains(&given) {
                    return Err(anyhow!(
                        "the input {input} takes one of {allowed:?}, not `{given}`"
                    ));
                }
                let value: serde_yaml_ng::Value = match given.as_str() {
                    "true" => true.into(),
                    "false" => false.into(),
                    other => other.into(),
                };
                values.insert(input.as_str().into(), value);
            }
            doc.insert("inputs".into(), values.into());
        }
        // The case's branch forks from its base: the branch the tree is on, or, when the tree
        // is on another case's branch, that case's base, at its tip, so a case never forks from
        // another case's commits. `left` is the branch the tree was on, which a discarded case
        // returns to.
        let (left, base, from) = opening_base(tree)?;
        let base_commit = crate::git::run(
            tree,
            &["rev-parse", "--verify", from.as_deref().unwrap_or("HEAD")],
        )?;
        let identity = match commit_as {
            CommitAs::Takt => None,
            CommitAs::Given(identity) => Some(identity.clone()),
            CommitAs::Configured { required } => {
                match crate::git::configured_identity(tree, &base_commit)? {
                    Some(identity) => Some(identity),
                    None if *required => {
                        return Err(crate::git::no_identity(
                            &base_commit[..12.min(base_commit.len())],
                        ));
                    }
                    None => None,
                }
            }
        };
        let branch = Branch {
            name: Branch::name_of(id),
            base,
            base_commit: Some(base_commit),
            left,
            pushed: None,
            pull_request: None,
        };
        crate::git::create_branch(tree, &branch.name, from.as_deref())?;
        let takt_dir = tree.join(TAKT_DIR);
        let takt_was = std::fs::symlink_metadata(&takt_dir).is_ok();
        let ignore_was = std::fs::symlink_metadata(takt_dir.join(TAKT_GITIGNORE)).is_ok();
        let opened = (|| {
            // The branch's tip is the implementation revision: not the HEAD the case was asked
            // on when the branch forked from a base elsewhere.
            let tip = crate::git::head(tree)?;
            let mut case = Self {
                id: id.to_owned(),
                dir: dir.clone(),
                branch: branch.name.clone(),
                tree: tree.to_path_buf(),
                protocol,
                doc: doc.into(),
                written: 0,
            };
            if case
                .protocol
                .artifacts
                .iter()
                .any(|artifact| artifact == "implementation")
            {
                case.doc["artifacts"]["implementation"]["revision"] = tip.into();
            }
            write_takt_gitignore(tree)?;
            case.create(intent, &branch)?;
            if let Some(identity) = &identity {
                let path = case.dir.join(COMMIT_IDENTITY_RECORD);
                std::fs::write(&path, format!("{identity}\n"))
                    .with_context(|| format!("write {}", path.display()))?;
            }
            Ok(case)
        })();
        opened.map_err(|error: anyhow::Error| {
            let existed = (takt_was, ignore_was);
            match Self::undo_open(
                tree,
                &dir,
                &branch.name,
                branch.left.as_deref(),
                &head,
                existed,
            ) {
                Ok(()) => error,
                Err(undo) => error.context(format!(
                    "and the half-opened case {id} could not be taken back: {undo:#}"
                )),
            }
        })
    }

    /// Takes back an open that failed after its branch was made: the case directory if any of
    /// it was written, the cases directory when it is empty, the `.takt/.gitignore` and the
    /// `.takt/` directory the open wrote where they were absent before (`existed`: whether
    /// `.takt/` and its ignore file were there), and the branch, the tree back on `left`, or
    /// detached at `start` when it was detached.
    fn undo_open(
        tree: &Path,
        dir: &Path,
        branch: &str,
        left: Option<&str>,
        start: &str,
        existed: (bool, bool),
    ) -> Result<()> {
        if std::fs::symlink_metadata(dir).is_ok() {
            std::fs::remove_dir_all(dir)
                .with_context(|| format!("remove the half-written case {}", dir.display()))?;
        }
        // The cases directory stays only while a case is in it.
        if let Some(cases) = dir.parent() {
            let _ = std::fs::remove_dir(cases);
        }
        let (takt_was, ignore_was) = existed;
        let takt_dir = tree.join(TAKT_DIR);
        if !ignore_was {
            let _ = std::fs::remove_file(takt_dir.join(TAKT_GITIGNORE));
        }
        if !takt_was {
            let _ = std::fs::remove_dir(&takt_dir);
        }
        match left {
            Some(left) => crate::git::run(tree, &["checkout", "-q", left])?,
            None => crate::git::run(tree, &["checkout", "-q", "--detach", start])?,
        };
        crate::git::run(tree, &["branch", "-q", "-D", branch])?;
        Ok(())
    }

    /// Opens the case `id` that exists under `tree` again, for a run that resumes it: its files
    /// as they are, the records written so far counted so the next is numbered after them, the
    /// protocol from the case's own `protocol.yaml`, and the tree put on the case's branch,
    /// which the validators resolve the case from. The implementation revision is the branch's
    /// `HEAD`: a commit made on the branch between two runs moves it, and with it every claim
    /// returns to unknown and the validators run again, so nothing goes through on records
    /// about the commit before. The one exception is takt's own records for the story above it:
    /// a `HEAD` that reaches the implementation revision only through its riders (the
    /// approval's commits, the `test_result` and the move to `implemented`, by the one rule the
    /// push and the merge read, [`approvals_only`]) leaves it where it is, the revision the
    /// grant authorises, so an accepted case resumed by id has nothing due again. Nothing else
    /// is written. A directory without a `case.yaml` is
    /// no case, and an id that is not a case id ([`check_id`]) is refused before it is joined
    /// to a path. The case is held (`run.lock`, taken atomically) before the checkout and the
    /// revision are written and let go once they are: a case another holds is refused with
    /// nothing written, and of two resuming at once only one writes. The run takes its own
    /// hold when it starts ([`CaseDir::hold`]).
    pub fn resume(tree: &Path, id: &str) -> Result<Self> {
        check_id(id)?;
        refuse_link_on_the_way(tree, id, "resumed")?;
        let dir = Self::locate(tree, id);
        let snapshot = dir.join("case.yaml");
        if !snapshot.is_file() {
            bail!("no case {id} at {}", dir.display());
        }
        // One holder at a time: a case another holds is not resumed, and nothing below is
        // written while another holds it.
        let _held = Held::take(&dir, id)?;
        let protocol = LoadedProtocol::from_file(&dir.join("protocol.yaml"))?;
        let text = std::fs::read_to_string(&snapshot)
            .with_context(|| format!("read {}", snapshot.display()))?;
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text)
            .with_context(|| format!("read {}", snapshot.display()))?;
        let branch = Branch::name_of(id);
        if crate::git::current_branch(tree)?.as_deref() != Some(branch.as_str()) {
            crate::git::run(tree, &["checkout", "-q", &branch])
                .with_context(|| format!("check out the branch of the case {id}"))?;
        }
        let mut case = Self {
            id: id.to_owned(),
            dir,
            branch,
            tree: tree.to_path_buf(),
            protocol,
            doc,
            written: 0,
        };
        case.written = case.evidence_texts()?.len();
        let head = crate::git::head(tree)?;
        if let Some(revision) = case.revision("implementation")
            && revision != head
            && !approvals_only(tree, &revision, &head, &case.dir)?
        {
            case.set_revision("implementation", &head)?;
        }
        Ok(case)
    }

    /// Takes hold of the case for a run: creates `run.lock` atomically (`create_new`) with the
    /// instant now, so of two runs taking hold at once exactly one does. Refused while another
    /// run, a `resume` or a grant holds it.
    pub fn hold(&self) -> Result<()> {
        take_hold(&self.dir, &self.id)
    }

    /// Lets go of the case: removes `run.lock`; one already gone is no error.
    pub fn release(&self) -> Result<()> {
        let path = self.dir.join(LOCK_FILE);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| format!("remove {}", path.display())),
        }
    }

    /// Checks that every part of the way from the tree down to the case's directory, the
    /// directory itself included, is a real directory, each read with `symlink_metadata` and
    /// never followed: a link on it (one a step left at `.engineering` when its undo failed)
    /// would take a write, an append or a removal under the case wherever it points. A run
    /// writes, appends to and removes nothing under the case directory, its event log and its
    /// hold among them, until this holds ([`crate::Run::run`]).
    pub fn check_way(&self) -> Result<()> {
        let mut way = PathBuf::new();
        let relative = self.dir.strip_prefix(&self.tree).unwrap_or(&self.dir);
        for part in relative.components() {
            way.push(part);
            let at = self.tree.join(&way);
            let meta =
                std::fs::symlink_metadata(&at).with_context(|| format!("read {}", at.display()))?;
            if meta.file_type().is_symlink() {
                let target = std::fs::read_link(&at)
                    .with_context(|| format!("read the link {}", at.display()))?;
                bail!(
                    "{} is a link to {}; nothing under the case {} is written through it",
                    way.display(),
                    target.display(),
                    self.id
                );
            }
            if !meta.is_dir() {
                bail!(
                    "{} is not a directory; nothing under the case {} is written there",
                    way.display(),
                    self.id
                );
            }
        }
        Ok(())
    }

    /// The records profile the case's first `opened` line names, the profile the case was opened
    /// under: a resume without `--records` runs under it. A first line with no `records` field
    /// reads as `trial`, as does a case whose record holds no `opened` line yet: each keeps every
    /// decision it may hold (story:a-run-keeps-every-decision-only-on-a-trial).
    pub fn opened_profile(&self) -> Result<Records> {
        use std::io::Read as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let path = self.dir.join(crate::events::EVENTS_FILE);
        let mut bytes = Vec::new();
        match std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
        {
            Ok(mut file) => {
                file.read_to_end(&mut bytes)
                    .with_context(|| format!("read {}", path.display()))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Records::Trial);
            }
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", path.display()));
            }
        }
        // A line that does not read as an event is skipped here, as the run itself goes on past one.
        Ok(bytes
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
            .find(|line| line["event"] == "opened")
            .and_then(|line| line["records"].as_str().and_then(Records::named))
            .unwrap_or(Records::Trial))
    }

    /// The number of the latest evaluation written under the case, the highest `n` of its
    /// `decisions/<n>.json`; 0 before the first. One sequence per case, continued across its
    /// runs: a run numbers its evaluations, and the records its validators write, from here.
    pub fn evaluations(&self) -> Result<usize> {
        let decisions = self.dir.join("decisions");
        if !decisions.is_dir() {
            return Ok(0);
        }
        let mut highest = 0;
        for entry in std::fs::read_dir(&decisions)
            .with_context(|| format!("read {}", decisions.display()))?
        {
            let path = entry?.path();
            if let Some(n) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<usize>().ok())
            {
                highest = highest.max(n);
            }
        }
        Ok(highest)
    }

    /// Removes the decisions this run wrote before its latest one, `latest`: the
    /// `decisions/<n>.json` numbered from `first`, the run's first sequence number, up to
    /// `latest`, so that decision `latest` is the only one of the run the case keeps. An earlier
    /// run's decisions are never removed (story:a-run-keeps-every-decision-only-on-a-trial). The
    /// `default` records profile does this after it writes each decision. Only regular `.json`
    /// files whose name is a number in that range go; the sequence is unchanged, since
    /// [`CaseDir::evaluations`] still reads `latest` as the highest. Returns how many were removed.
    pub fn keep_only_decision(&self, first: usize, latest: usize) -> Result<usize> {
        let decisions = self.dir.join("decisions");
        let mut removed = 0;
        for entry in std::fs::read_dir(&decisions)
            .with_context(|| format!("read {}", decisions.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let this_run = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<usize>().ok())
                .is_some_and(|n| first <= n && n < latest);
            let json = path
                .extension()
                .is_some_and(|extension| extension == "json");
            if this_run && json && entry.file_type()?.is_file() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("remove {}", path.display()))?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// The story the case was opened for; `None` for a case opened on an intent.
    pub fn story(&self) -> Result<Option<String>> {
        story_of(&self.dir)
    }

    /// Records the story the case was opened for, as `story.yaml`.
    pub fn set_story(&self, id: &str) -> Result<()> {
        let path = self.dir.join(STORY_FILE);
        let text = serde_yaml_ng::to_string(&StoryRef { id: id.to_owned() })
            .context("write the story record")?;
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
    }

    /// The intent the case was opened for, `intent.md`.
    pub fn intent(&self) -> Result<String> {
        let path = self.dir.join("intent.md");
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))
    }

    /// The case inputs as `case.yaml` holds them, `name=value` pairs, a boolean as `true` or
    /// `false`; none for a protocol that declares no input.
    #[must_use]
    pub fn inputs(&self) -> Vec<(String, String)> {
        inputs_in(&self.doc)
    }

    /// Creates the directory and writes the case's files, `case.yaml` last.
    fn create(&self, intent: &str, branch: &Branch) -> Result<()> {
        std::fs::create_dir_all(self.dir.join("evidence"))
            .with_context(|| format!("create {}", self.dir.display()))?;
        std::fs::create_dir_all(self.dir.join("decisions"))?;
        std::fs::write(self.dir.join("protocol.ir.json"), &self.protocol.ir_json)?;
        std::fs::write(self.dir.join("protocol.yaml"), &self.protocol.yaml)?;
        std::fs::write(self.dir.join("intent.md"), intent)?;
        std::fs::write(self.dir.join(OBSERVATIONS_FILE), render_observations(&[]))?;
        branch.write(&self.dir)?;
        self.write()
    }

    /// Removes a case just opened, for one the store would not take or whose take failed: what
    /// was left uncommitted on its branch since it opened (the case opened on a clean tree, so
    /// that is the take's, the index included), its directory, the cases directory when this was
    /// the last case in it, and its branch, the tree returning to the branch the case left
    /// (detached when it left none). Nothing of the case remains. Not for a case with commits
    /// of its own on its branch.
    pub fn discard(self) -> Result<()> {
        let branch = Branch::read(&self.dir)?;
        if crate::git::current_branch(&self.tree)?.as_deref() == Some(branch.name.as_str()) {
            crate::git::discard_uncommitted(&self.tree)?;
        }
        std::fs::remove_dir_all(&self.dir)
            .with_context(|| format!("remove the case {}", self.dir.display()))?;
        if let Some(cases) = self.dir.parent() {
            let _ = std::fs::remove_dir(cases);
        }
        crate::git::leave_branch(&self.tree, &branch.name, branch.left.as_deref())
    }

    /// Writes `case.yaml`.
    fn write(&self) -> Result<()> {
        let yaml = serde_yaml_ng::to_string(&self.doc).context("write the case snapshot")?;
        std::fs::write(self.dir.join("case.yaml"), yaml)?;
        Ok(())
    }

    /// The current revision of `artifact`.
    #[must_use]
    pub fn revision(&self, artifact: &str) -> Option<String> {
        self.doc["artifacts"][artifact]["revision"]
            .as_str()
            .map(str::to_owned)
    }

    /// Whether the case holds a grant for `action`: the case's protocol has it require at least
    /// one capability, and the case holds each granted ([`Authority::grants_at`]) at its
    /// current revision of the artifact that capability's action is about
    /// ([`LoadedProtocol::grant_subject`]). The step such a grant authorises, the merge for a
    /// delivery.
    pub fn grants(&self, action: &str) -> Result<bool> {
        let (_, actions) = required_capabilities(&self.dir)?;
        let Some(capabilities) = actions.get(action) else {
            return Ok(false);
        };
        let authority = Authority::read(&self.dir)?.unwrap_or_default();
        for capability in capabilities {
            // A capability whose actions are about no single artifact is granted nothing.
            let Ok(subject) = self.protocol.grant_subject(capability) else {
                return Ok(false);
            };
            let Some(revision) = self.revision(&subject) else {
                return Ok(false);
            };
            if !authority.grants_at(capability, &revision) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Opens a loop case's baseline, before any step: `loop_baseline` at `revision`, a commit
    /// [`loop_baseline`] answered, and the baseline's metric definitions snapshotted into the
    /// case ([`crate::loop_scorecard::snapshot_baseline`]), which every loop validator reads the
    /// baseline from. A snapshot that cannot be taken is an error.
    pub fn open_loop_baseline(&mut self, tree: &Path, revision: &str) -> Result<()> {
        self.set_revision(LOOP_BASELINE, revision)?;
        if !crate::loop_scorecard::snapshot_baseline(tree, self)? {
            bail!(
                "the loop case {} could not snapshot its baseline {revision}",
                self.id
            );
        }
        Ok(())
    }

    /// Opens an improvement turn, before any step: `finding` at the commit the case's branch
    /// forked from (its `base_commit`), `baseline` at the promoted baseline (the installed
    /// binary's revision when the turn opens), `candidate` at the candidate given, `promotion` at
    /// the candidate too (the revision `binary.promote` observes installed),
    /// `story.yaml` naming the finding, and `independence` in `case.yaml` declaring each turn
    /// validator ([`crate::improvement::PRODUCERS`]) independent on
    /// [`crate::improvement::INDEPENDENT_ON`]: a validator is no agent run, so its record is
    /// another run's than the one that delivered the candidate. Nothing else declares
    /// independence. The sha256 of the binary that judges the turn goes into [`JUDGE_FILE`], and
    /// the install root given, an existing directory named canonically, into
    /// [`INSTALL_ROOT_FILE`]; any other root is refused before anything is written. The forge
    /// given goes into [`FORGE_FILE`]; a name that is empty or holds white space is refused
    /// before anything is written.
    pub fn open_turn(&mut self, opening: &TurnOpening) -> Result<()> {
        if let Some(forge) = &opening.forge
            && (forge.is_empty() || forge.chars().any(|c| c.is_whitespace() || c.is_control()))
        {
            bail!(
                "the forge `{}` is no forge's name; a turn records the forge it reads the mainline through",
                forge.escape_debug()
            );
        }
        // The install root is checked before anything is written: an existing directory named
        // canonically, an absolute UTF-8 path on one line, so no `..` or link recorded could
        // later lead elsewhere.
        let install_root = match &opening.install_root {
            None => None,
            Some(root) => {
                let canonical = std::fs::canonicalize(root)
                    .with_context(|| format!("the install root {}", root.display()))?;
                if canonical != *root || !canonical.is_dir() {
                    bail!(
                        "the install root {} is no directory named canonically ({}); a turn records the directory itself",
                        root.display(),
                        canonical.display()
                    );
                }
                Some(
                    root.to_str()
                        .filter(|root| root.starts_with('/') && !root.contains('\n'))
                        .ok_or_else(|| {
                            anyhow!(
                                "the install root {} is no absolute UTF-8 path on one line",
                                root.display()
                            )
                        })?
                        .to_owned(),
                )
            }
        };
        let Some(base) = Branch::read(&self.dir)?.base_commit else {
            bail!(
                "the turn {} pins no base commit, which its finding artifact is",
                self.id
            );
        };
        for (artifact, revision) in [
            ("finding", base.as_str()),
            ("baseline", opening.baseline.as_str()),
            ("candidate", opening.candidate.as_str()),
            ("promotion", opening.candidate.as_str()),
        ] {
            let entry = self
                .doc
                .get_mut("artifacts")
                .and_then(|a| a.get_mut(artifact))
                .ok_or_else(|| anyhow!("the case declares no artifact {artifact}"))?;
            entry["revision"] = revision.into();
        }
        let mut independence = serde_yaml_ng::Mapping::new();
        for producer in crate::improvement::PRODUCERS {
            independence.insert(
                producer.into(),
                vec![crate::improvement::INDEPENDENT_ON].into(),
            );
        }
        self.doc["independence"] = independence.into();
        self.write()?;
        let judge = self.dir.join(JUDGE_FILE);
        std::fs::write(&judge, format!("{}\n", opening.judge_sha256))
            .with_context(|| format!("write {}", judge.display()))?;
        if let Some(root) = &install_root {
            let path = self.dir.join(INSTALL_ROOT_FILE);
            std::fs::write(&path, format!("{root}\n"))
                .with_context(|| format!("write {}", path.display()))?;
        }
        if let Some(forge) = &opening.forge {
            let path = self.dir.join(FORGE_FILE);
            std::fs::write(&path, format!("{forge}\n"))
                .with_context(|| format!("write {}", path.display()))?;
        }
        self.set_story(&opening.finding)
    }

    /// The identity the case was opened with ([`commit_identity_of`]); none when it recorded
    /// none.
    pub fn commit_identity(&self) -> Result<Option<crate::git::CommitIdentity>> {
        commit_identity_of(&self.dir)
    }

    /// The identity every commit of the case carries: the one it was opened with, else takt's
    /// own ([`crate::git::CommitIdentity::takt`]).
    pub fn committer(&self) -> Result<crate::git::CommitIdentity> {
        Ok(self
            .commit_identity()?
            .unwrap_or_else(crate::git::CommitIdentity::takt))
    }

    /// Moves `artifact` to `revision`; evidence about the old revision no longer applies. A move
    /// to another revision is a new revision of the case ([`CASE_REVISION`]), the one an
    /// authority answer was asked at no longer.
    pub fn set_revision(&mut self, artifact: &str, revision: &str) -> Result<()> {
        let next = case_revision(&self.doc)? + 1;
        let entry = self
            .doc
            .get_mut("artifacts")
            .and_then(|a| a.get_mut(artifact))
            .ok_or_else(|| anyhow!("the case declares no artifact {artifact}"))?;
        if entry["revision"].as_str() != Some(revision) {
            entry["revision"] = revision.into();
            self.doc[CASE_REVISION] = format!("r{next}").into();
        }
        self.write()
    }

    /// Adds an evidence record, written as `evidence/<n>-<id>.json`. In takt's own code only the
    /// case's evidence port calls it ([`crate::evidence::CaseEvidence`]); it is public for the
    /// tests that seed a case with records.
    pub fn add_evidence(&mut self, record: &Record) -> Result<PathBuf> {
        self.written += 1;
        let path = self
            .dir
            .join("evidence")
            .join(format!("{:04}-{}.json", self.written, record.id));
        std::fs::write(&path, record.to_json())
            .with_context(|| format!("write {}", path.display()))?;
        Ok(path)
    }

    /// Whether a record of `kind` exists about `subject` at its current revision. Each record
    /// is read as a `canon-evidence/1` document, as [`CaseDir::evaluate`] reads it (YAML, of
    /// which a JSON record is one); a record that does not read is an error naming its file,
    /// never an answer either way: every record is read before the answer is given.
    pub fn has_evidence(&self, kind: &str, subject: &str) -> Result<bool> {
        let records = self.evidence_records()?;
        let Some(revision) = self.revision(subject) else {
            return Ok(false);
        };
        Ok(records.iter().any(|record| {
            record.kind.as_str() == kind
                && record.subject.as_str() == subject
                && record.subject_revision.as_str() == revision
        }))
    }

    /// Every evidence record under the case, each read as Canon's `EvidenceRecord` from YAML,
    /// in name order; the first that does not read (an empty file, a bare scalar, a missing
    /// field) is an error naming its file, and nothing is answered. What [`CaseDir::evaluate`]
    /// checks before it asks the governor, which would drop such a record, and what
    /// [`CaseDir::has_evidence`] answers from: one reader, all records or none.
    fn evidence_records(&self) -> Result<Vec<b10x_canon::model::EvidenceRecord>> {
        self.evidence_texts()?
            .iter()
            .map(|(path, text)| {
                serde_yaml_ng::from_str::<b10x_canon::model::EvidenceRecord>(text)
                    .context("evidence is not a canon-evidence/1 document")
                    .with_context(|| format!("read the evidence record {}", path.display()))
            })
            .collect()
    }

    /// The evidence records under the case, `evidence/*.json` and `*.yaml`, in name order, each
    /// with its path.
    fn evidence_texts(&self) -> Result<Vec<(PathBuf, String)>> {
        evidence_files(&self.dir)
    }

    /// Asks the governor of Loom for the decision over the case as its files hold it now
    /// ([`CaseFiles`]), its evidence and the time now; writes the `canon-decision/1` document the
    /// governor rendered for it under `decisions/`, and answers it with each action the governor
    /// finds approval-required answered by the case's authority ([`CaseAuthority`], reading
    /// `authority.yaml` now, so a grant written since the last evaluation is seen, and only at
    /// the revision of its subject it was given at): `admissible` where it allows the
    /// capability, `blocked` where it denies it. The governor supplies no authority, so the
    /// document on file finds such an action approval-required, granted or not. Every
    /// evidence record is read first, as [`CaseDir::has_evidence`] reads it, so one that does
    /// not read is an error naming its file; the governor would drop it. A case the governor
    /// refuses to decide is an error carrying the governor's refusal.
    ///
    /// The loop's one call is held by approval-record:loop-pre-checks-ahead-of-loom: the two
    /// checks takt runs ahead of each Loom step (`Shape::only_behind`, `Shape::unperformable`)
    /// read the decision it answers.
    pub fn evaluate(&self, seq: usize) -> Result<Decision> {
        self.evidence_records()?;
        let files = CaseFiles::new(&self.dir, &self.id, &self.protocol.id);
        let case = CaseId(self.id.clone());
        if files.get(&case)?.is_none() {
            bail!("no case {} at {}", self.id, self.dir.display());
        }
        let at = crate::now();
        let governor = CanonGovernor::new(files)
            .with_protocol_yaml(&self.protocol.id, &self.protocol.yaml)
            .map_err(|error| anyhow!("register the protocol {}: {error}", self.protocol.id))?
            .with_evaluation_time(move |_: &CaseId| Ok(Some(at.clone())));
        let refused = |error: GovernorError| {
            anyhow!(
                "the governor of Loom does not decide the case {}: {error:?}",
                self.id
            )
        };
        let frontier = governor.frontier(&case).map_err(refused)?;
        let frontier = frontier.data();
        let document = governor
            .decision_document(&case, &frontier.frontier_id)
            .map_err(refused)?
            .ok_or_else(|| {
                anyhow!(
                    "the case {} changed while it was decided; nothing was written",
                    self.id
                )
            })?;
        let json = render_canonical(&document);
        std::fs::write(
            self.dir.join("decisions").join(format!("{seq:04}.json")),
            &json,
        )?;
        let mut decision = Decision::from_json(&json)?;
        let authority = CaseAuthority::new(&self.dir, &self.id);
        for action in &frontier.actions {
            let (ActionStatus::ApprovalRequired, Some(capability)) =
                (&action.status, &action.capability)
            else {
                continue;
            };
            let verdict = authority.verdict(&action.action, capability, frontier.case_revision)?;
            let Some(status) = answered_status(&verdict) else {
                continue;
            };
            decision
                .actions
                .insert(action.action.clone(), status.to_owned());
        }
        Ok(decision)
    }
}

/// The protocol of an improvement turn, which opens with its finding, its candidate and the
/// promoted baseline: `self.improvement/1`.
pub const TURN_PROTOCOL: &str = "self.improvement/1";

/// The name `takt run` reads a turn's finding from, `--input finding=<story id>`, which the
/// protocol declares as no case input.
pub const TURN_FINDING: &str = "finding";

/// The name `takt run` reads a turn's candidate from, `--input candidate=<full commit id>`,
/// which the protocol declares as no case input.
pub const TURN_CANDIDATE: &str = "candidate";

/// What an improvement turn opens with, each checked by [`turn_opening`] before anything is
/// written: the finding it verifies (`story:<key>`), the candidate revision delivered outside
/// the case, the baseline, the revision of the latest promotion record, the judge surface the
/// candidate's change touches or not, and the finding's title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnOpening {
    /// The finding, `story:<key>`.
    pub finding: String,
    /// The candidate, a full commit id.
    pub candidate: String,
    /// The baseline, the latest promoted revision.
    pub baseline: String,
    /// The case input `surface`: `judge` when the candidate's change touches a judge surface
    /// ([`crate::improvement::judge_surfaces`]), else `non_judge`.
    pub surface: String,
    /// The finding's title, the turn's intent when none is given: its frontmatter's `title`,
    /// else its first `# ` heading, else the finding's id.
    pub title: String,
    /// The sha256 of the binary that judges the turn, the promoted one that opens it: every
    /// later run of the turn is refused unless it is that binary ([`turn_judge_refusal`]).
    pub judge_sha256: String,
    /// The install root `takt promote` installs into, which `binary.promote` reads the
    /// installed binary under, recorded at open ([`INSTALL_ROOT_FILE`]); none, and it is read
    /// from the environment when the validator observes.
    pub install_root: Option<PathBuf>,
    /// The forge whose mainline the turn reads ([`crate::improvement::Mainline::forge`]: `bot`,
    /// or `none` for the local `main`), recorded at open ([`FORGE_FILE`]) for `takt approve` to
    /// read the mainline's tip at a judge-surface grant through; none records nothing, and such
    /// a grant records no tip and authorises no judge-surface merge.
    pub forge: Option<String>,
}

/// The file of an improvement turn's case that holds the install root it was opened with, an
/// absolute path, on one line.
pub const INSTALL_ROOT_FILE: &str = "install-root";

/// The install root the turn at `case_dir` was opened with ([`INSTALL_ROOT_FILE`]); none when it
/// recorded none.
pub fn install_root_of(case_dir: &Path) -> Result<Option<PathBuf>> {
    let path = case_dir.join(INSTALL_ROOT_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let root = PathBuf::from(text.trim_end_matches('\n'));
            if !root.is_absolute() {
                bail!(
                    "{} names {}, which is no absolute path",
                    path.display(),
                    root.display()
                );
            }
            Ok(Some(root))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

/// The file of an improvement turn's case that holds the forge it was opened with, its name on
/// one line ([`TurnOpening::forge`]).
pub const FORGE_FILE: &str = "forge";

/// The forge the turn at `case_dir` was opened with ([`FORGE_FILE`]); none when it recorded none.
pub fn turn_forge(case_dir: &Path) -> Result<Option<String>> {
    let path = case_dir.join(FORGE_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let forge = text.strip_suffix('\n').unwrap_or(&text);
            if forge.is_empty() || forge.chars().any(|c| c.is_whitespace() || c.is_control()) {
                bail!(
                    "{} names `{}`, which is no forge's name",
                    path.display(),
                    forge.escape_debug()
                );
            }
            Ok(Some(forge.to_owned()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

/// Why a resume of the case `id` at `case_dir` by a run whose forge is `run_forge` (`bot`, or
/// `none` for the local `main`, [`crate::improvement::Mainline::forge`]) is refused before
/// anything is written, or none: a turn reads the mainline through the forge it recorded when it
/// opened ([`turn_forge`]), `takt approve` at a judge-surface grant and the merge validators
/// alike, so a run through another forge would read another `main`. A case that recorded no
/// forge, any case but a turn and a turn opened before turns recorded one, is not refused here.
pub fn turn_forge_refusal(case_dir: &Path, id: &str, run_forge: &str) -> Result<Option<String>> {
    let Some(recorded) = turn_forge(case_dir)? else {
        return Ok(None);
    };
    if recorded == run_forge {
        return Ok(None);
    }
    Ok(Some(format!(
        "{TURN_PROTOCOL}: the turn {id} recorded the forge `{recorded}` when it opened, and this run's forge is `{run_forge}`; a turn reads the mainline through the forge it opened with, `takt approve` and the merge validators alike, so resume it with `--forge {recorded}`; nothing was resumed"
    )))
}

/// The case input a turn's judge surface is, `surface`.
pub const TURN_SURFACE: &str = "surface";

/// The title of the finding `finding` whose file holds `text`: its frontmatter's `title`, else
/// its first `# ` heading (a leading `Story: ` dropped), else `finding` itself.
fn finding_title(finding: &str, text: &[u8]) -> String {
    let text = String::from_utf8_lossy(text);
    if let Some(front) = front_matter(&text)
        && let Ok(doc) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&front)
        && let Some(title) = doc["title"]
            .as_str()
            .map(str::trim)
            .filter(|t| !t.is_empty())
    {
        return title.to_owned();
    }
    text.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(|heading| {
            heading
                .trim()
                .trim_start_matches("Story: ")
                .trim()
                .to_owned()
        })
        .filter(|heading| !heading.is_empty())
        .unwrap_or_else(|| finding.to_owned())
}

/// A story file's body, below its frontmatter, or the whole text when it has none.
fn finding_body(text: Option<&[u8]>) -> Option<&[u8]> {
    text.map(|text| crate::validator::story_body(text).unwrap_or(text))
}

/// Whether `value` is a full commit id: 40 or 64 lower-case hexadecimal digits.
fn is_full_commit_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The binary that would judge a turn: the revision it was built from, as `takt --version`
/// names it, and its file.
#[derive(Debug, Clone, Copy)]
pub struct Judge<'a> {
    /// The revision, `<sha>`, `<sha>-dirty` or `unknown revision`.
    pub revision: &'a str,
    /// The binary's file.
    pub binary: &'a Path,
}

/// What a turn on `tree` opens with, from `--input finding=<story id>`, `--input
/// candidate=<full commit id>` and, when given, `--input surface=<judge|non_judge>`, the
/// promotion store `store` and the mainline `mainline`: the finding a story the planning store
/// holds at the commit the case forks from (`.engineering/planning/story/<key>.md`), the
/// candidate a full commit id of a commit `tree` holds, the baseline the revision of the
/// store's latest record that is no refusal ([`crate::promotion::baseline`]), a commit `tree`
/// holds, and the judge surface computed from the candidate's change against the baseline
/// ([`crate::improvement::judge_surfaces`]). A missing or wrong value, a store with no such
/// record, or a given `surface` that is not the computed one (naming the first judge-surface
/// path) is refused, naming why, before anything is written. The candidate is delivered outside
/// the case, so it is given here. A candidate `mainline`'s tip already holds (the tip or an
/// ancestor of it) was merged before a turn could grant it and is refused, naming both full ids
/// and the remedy, a fresh candidate. A tip that cannot be read (the forge's route or the
/// network down, no `main`), or one whose objects do not arrive to say whether it holds the
/// candidate, is refused too, naming the forge, why the read failed and the remedy, to open
/// again once the tip can be read. Only an opening reads this: a turn whose candidate reaches
/// the mainline after it opened, or whose mainline cannot be read since, resumes.
///
/// What judges the turn is never the candidate's, nor the tree's: the finding scripts are read
/// at the baseline ([`crate::improvement`]), and a candidate whose tree differs from the
/// baseline's at `scripts/check-finding` or `scripts/file-finding` is refused, naming the path.
/// The finding is read on the mainline: when the commit the case forks from is `mainline`'s tip
/// or an ancestor of it ([`crate::improvement::Mainline`]; the local `main` with no forge, which
/// the operator is trusted with), the finding is read there and its file is no part of the
/// candidate's comparison, so a finding filed after the last promotion opens a turn. Else the
/// finding's body (below its frontmatter, which a delivery's take changes) must be the
/// baseline's, at the commit the case forks from and in the candidate, or the turn is refused
/// naming its file. The turn is refused too unless `judge` is the promoted binary: its
/// sha256 is the record's `binary_sha256` whenever the record has one, and its revision is the
/// promoted one unless the operator stated the record; and unless the baseline's
/// `scripts/check-finding` is a reviewed confined one
/// ([`crate::improvement::CONFINED_CHECK_FINDING_SHA256S`]).
pub fn turn_opening(
    tree: &Path,
    finding: Option<&str>,
    candidate: Option<&str>,
    surface: Option<&str>,
    store: &Path,
    judge: &Judge<'_>,
    mainline: &crate::improvement::Mainline,
) -> Result<TurnOpening> {
    let Some(finding) = finding else {
        bail!(
            "{TURN_PROTOCOL} opens with --input {TURN_FINDING}=<story id>, the finding the turn verifies; none was given; nothing was written"
        );
    };
    let Some(path) = crate::improvement::finding_path(finding) else {
        bail!(
            "--input {TURN_FINDING}={finding}: a finding is a story of the planning store, `story:<key>`; nothing was written"
        );
    };
    let (_, _, from) = opening_base(tree)?;
    let at = crate::git::run(
        tree,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{}^{{commit}}", from.as_deref().unwrap_or("HEAD")),
        ],
    )
    .with_context(|| format!("--input {TURN_FINDING}={finding}: the commit the case forks from"))?;
    let filed = crate::loop_scorecard::verified_file(tree, &at, &path)
        .with_context(|| format!("--input {TURN_FINDING}={finding}; nothing was written"))?;
    if filed.is_none() {
        bail!(
            "--input {TURN_FINDING}={finding}: the planning store at {} holds no {path}; file the finding before the turn opens; nothing was written",
            &at[..8.min(at.len())]
        );
    }
    let Some(candidate) = candidate else {
        bail!(
            "{TURN_PROTOCOL} opens with --input {TURN_CANDIDATE}=<full commit id>, the candidate delivered outside the case; none was given; nothing was written"
        );
    };
    let mut history = crate::loop_scorecard::History::open(tree, None)
        .with_context(|| format!("--input {TURN_CANDIDATE}={candidate}; nothing was written"))?;
    if !is_full_commit_id(candidate)
        || history
            .resolve(candidate)
            .with_context(|| format!("--input {TURN_CANDIDATE}={candidate}; nothing was written"))?
            .as_deref()
            != Some(candidate)
    {
        bail!(
            "--input {TURN_CANDIDATE}={candidate}: a turn's candidate is the full commit id (40 or 64 lower-case hexadecimal digits) of a commit of {}; nothing was written",
            tree.display()
        );
    }
    // A turn opens only on a candidate the mainline's tip was read and found not to hold: one it
    // holds was merged before the turn could grant it, and with no tip read (the route or the
    // network down, no `main`) or no answer from it, whether it does is not known.
    let main = crate::promotion::MAINLINE;
    let tip = match mainline.tip(tree) {
        Ok(tip) => tip,
        Err(error) => bail!(
            "--input {TURN_CANDIDATE}={candidate}: the tip of {main}, the mainline (forge: {}), cannot be read ({error:#}), so whether it holds the candidate, merged before a turn could grant it, is not known; open the turn again once the tip can be read; nothing was written",
            mainline.forge()
        ),
    };
    match crate::improvement::Mainline::holds(tree, &tip, candidate)? {
        Ok(false) => {}
        Ok(true) => bail!(
            "--input {TURN_CANDIDATE}={candidate}: {main}, the mainline (forge: {}), is at {tip}, which holds the candidate already, so it was merged before a turn could grant it; open the turn on a fresh candidate, one {main} does not hold; nothing was written",
            mainline.forge()
        ),
        Err(why) => bail!(
            "--input {TURN_CANDIDATE}={candidate}: whether {main}, the mainline (forge: {}), at {tip} holds the candidate cannot be read ({why}), so whether it was merged before a turn could grant it is not known; open the turn again once the tip can be read; nothing was written",
            mainline.forge()
        ),
    }
    let Some(promoted) = crate::promotion::baseline(store, &crate::now())
        .with_context(|| format!("the promotion store {}", store.display()))?
    else {
        bail!(
            "{TURN_PROTOCOL}: the promotion store {} holds no baseline or promotion record, the turn's baseline; record the installed revision with `takt promote --baseline-only`; nothing was written",
            store.display()
        );
    };
    let baseline = promoted.revision;
    if history
        .resolve(&baseline)
        .with_context(|| format!("the promoted baseline {baseline}; nothing was written"))?
        .as_deref()
        != Some(baseline.as_str())
    {
        bail!(
            "{TURN_PROTOCOL}: the promoted baseline {baseline} ({}) is no commit of {}; nothing was written",
            store.display(),
            tree.display()
        );
    }
    let short = |revision: &str| revision[..8.min(revision.len())].to_owned();
    // What judges the candidate is never the candidate's: the scripts are read at the baseline,
    // so a candidate that changes either is no turn.
    for judged in [
        crate::improvement::CHECK_FINDING,
        crate::improvement::FILE_FINDING,
    ] {
        if history.file(candidate, judged)? != history.file(&baseline, judged)? {
            bail!(
                "--input {TURN_CANDIDATE}={candidate}: the candidate changes {judged} against the baseline {}; what judges a turn is never the candidate's, so the turn is not opened; nothing was written",
                short(&baseline)
            );
        }
    }
    // The finding is read on the mainline, where the case forks from, and checked out there for
    // the check: what the candidate holds at its path judges nothing. Off the mainline, the
    // finding's body must be the baseline's, where the case forks from and in the candidate
    // alike.
    let on_mainline = matches!(
        crate::improvement::Mainline::holds(tree, &tip, &at)?,
        Ok(true)
    );
    if !on_mainline {
        let at_baseline = history.file(&baseline, &path)?;
        if finding_body(history.file(candidate, &path)?.as_deref())
            != finding_body(at_baseline.as_deref())
        {
            bail!(
                "--input {TURN_CANDIDATE}={candidate}: the candidate changes the body of {path} against the baseline {}, and the case forks from {}, no commit of {} (forge: {}); what judges a turn is never the candidate's, so the turn is not opened; nothing was written",
                short(&baseline),
                short(&at),
                crate::promotion::MAINLINE,
                mainline.forge()
            );
        }
        if finding_body(filed.as_deref()) != finding_body(at_baseline.as_deref()) {
            bail!(
                "--input {TURN_FINDING}={finding}: {path} at {}, where the case forks from, no commit of {} (forge: {}), does not hold the body the baseline {} holds; the turn's finding is read on the mainline or is the one filed at the baseline; nothing was written",
                short(&at),
                crate::promotion::MAINLINE,
                mainline.forge(),
                short(&baseline)
            );
        }
    }
    // The binary that judges is the promoted one: of the digest the record holds, whenever it
    // holds one, and of its revision unless the operator stated the record (a binary that names
    // no revision).
    let bytes = std::fs::read(judge.binary)
        .with_context(|| format!("read this binary, {}", judge.binary.display()))?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    if (promoted.revision_stated_by.is_some() || promoted.binary_sha256.is_some())
        && promoted.binary_sha256.as_deref() != Some(digest.as_str())
    {
        bail!(
            "{TURN_PROTOCOL}: this binary {} hashes to sha256 {digest}, not to {}, the sha256 the record of the promoted binary of {} holds; a turn is judged by the promoted binary only; nothing was written",
            judge.binary.display(),
            promoted.binary_sha256.as_deref().unwrap_or("none"),
            short(&baseline)
        );
    }
    if promoted.revision_stated_by.is_none() && judge.revision != baseline {
        bail!(
            "{TURN_PROTOCOL}: this binary was built from {}, not from {baseline}, the revision of the promoted binary ({}); a turn is judged by the promoted binary only; nothing was written",
            judge.revision,
            store.display()
        );
    }
    // The baseline runs a confined check: its script is one reviewed as confined, byte for byte.
    // The confinement lives in the script alone, so the baseline's history says nothing of it.
    let script = history
        .file(&baseline, crate::improvement::CHECK_FINDING)?
        .map(|bytes| format!("{:x}", Sha256::digest(&bytes)));
    if !script
        .as_deref()
        .is_some_and(|digest| crate::improvement::CONFINED_CHECK_FINDING_SHA256S.contains(&digest))
    {
        bail!(
            "{TURN_PROTOCOL}: the promoted baseline {} does not run a confined {}: its sha256 {} is none of the reviewed confined scripts' ({}); its run step would not be confined, so no turn opens on it until a revision holding a reviewed script is promoted; nothing was written",
            short(&baseline),
            crate::improvement::CHECK_FINDING,
            script.as_deref().unwrap_or("(no such file)"),
            crate::improvement::CONFINED_CHECK_FINDING_SHA256S.join(", ")
        );
    }
    // The judge surface is the candidate's change's, computed here; a given one that says
    // otherwise is refused, naming the first path that decides it.
    let surfaces = crate::improvement::judge_surfaces(tree, &baseline, candidate)
        .with_context(|| format!("--input {TURN_CANDIDATE}={candidate}; nothing was written"))?;
    let computed = if surfaces.is_empty() {
        "non_judge"
    } else {
        "judge"
    };
    if let Some(given) = surface
        && given != computed
    {
        match surfaces.first() {
            Some(first) => bail!(
                "--input {TURN_SURFACE}={given}: the candidate's change touches the judge surface {first}, a path {} at the baseline {} does not admit, so the turn's surface is judge; nothing was written",
                crate::validator::JUDGE_ALLOW,
                short(&baseline)
            ),
            None => bail!(
                "--input {TURN_SURFACE}={given}: the candidate's change touches no judge surface, every path admitted by {} at the baseline {}, so the turn's surface is non_judge; nothing was written",
                crate::validator::JUDGE_ALLOW,
                short(&baseline)
            ),
        }
    }
    Ok(TurnOpening {
        finding: finding.to_owned(),
        candidate: candidate.to_owned(),
        baseline,
        surface: computed.to_owned(),
        title: finding_title(finding, filed.as_deref().unwrap_or_default()),
        judge_sha256: digest,
        install_root: None,
        forge: Some(mainline.forge()),
    })
}

/// Why a case may not run on `protocol`, or none: an improvement turn (`self.improvement/1`)
/// runs on the document this binary embeds and on no other of that id, so no file can add an
/// action the harness would be offered inside a turn, nor take one away. Any other protocol is
/// not refused here.
pub fn turn_protocol_refusal(protocol: &LoadedProtocol) -> Result<Option<String>> {
    if protocol.id != TURN_PROTOCOL {
        return Ok(None);
    }
    let embedded = LoadedProtocol::builtin(TURN_PROTOCOL)?;
    if protocol.yaml == embedded.yaml {
        return Ok(None);
    }
    Ok(Some(format!(
        "{TURN_PROTOCOL}: a turn runs on the protocol this binary embeds (`--protocol {TURN_PROTOCOL}`) and on no other document of that id; this one differs from it; nothing was run"
    )))
}

/// The file of an improvement turn's case that holds the sha256 of the binary that judges it,
/// written when the turn opens ([`CaseDir::open_turn`]).
pub const JUDGE_FILE: &str = "judge.sha256";

/// Why a run of the case at `case_dir` by the binary `binary` (the running one: `/proc/self/exe`
/// on Linux, the file the kernel holds; elsewhere the path `std::env::current_exe` reports, read
/// as that path holds it now) is refused before any step, or none: a case of any protocol but an
/// improvement turn's is never refused here; a turn is run only by the binary whose sha256 it
/// recorded when it opened ([`JUDGE_FILE`]), so a resume by another binary, the candidate's that
/// `takt promote` just installed among them, judges nothing. `takt promote` keeps the binary it
/// replaces by its digest ([`crate::promotion::kept_binary`]), and the refusal names where.
pub fn turn_judge_refusal(case_dir: &Path, binary: &Path) -> Result<Option<String>> {
    if required_capabilities(case_dir)?.0 != TURN_PROTOCOL {
        return Ok(None);
    }
    let path = case_dir.join(JUDGE_FILE);
    let Ok(recorded) = std::fs::read_to_string(&path) else {
        return Ok(Some(format!(
            "{TURN_PROTOCOL}: the turn records no binary that judges it ({} is not there), so no binary may run it; open the turn again",
            path.display()
        )));
    };
    let recorded = recorded.trim();
    let bytes =
        std::fs::read(binary).with_context(|| format!("read this binary, {}", binary.display()))?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    if digest == recorded {
        return Ok(None);
    }
    let kept = crate::promotion::store()
        .map(|store| crate::promotion::kept_binary(&store, recorded))
        .map_or_else(
            |_| String::new(),
            |kept| {
                format!(
                    "; `takt promote` keeps a binary it replaces at {}: check that `sha256sum {}` prints {recorded} before you run it",
                    kept.display(),
                    kept.display()
                )
            },
        );
    Ok(Some(format!(
        "{TURN_PROTOCOL}: this binary hashes to sha256 {digest}, not to {recorded}, the binary that judges the turn since it opened; a turn is judged by one binary only, so run it with that one{kept}; nothing was run"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// story:a-run-keeps-every-decision-only-on-a-trial: an improvement case keeps every decision
    /// whatever is asked, a story's case keeps what is asked, and the names are the flag's.
    #[test]
    fn improvement_cases_keep_every_decision_whatever_is_asked() {
        for asked in [Records::Default, Records::Trial] {
            assert_eq!(asked.for_protocol(TURN_PROTOCOL), Records::Trial);
            assert_eq!(asked.for_protocol(LOOP_PROTOCOL), Records::Trial);
            assert_eq!(asked.for_protocol("story.delivery/1"), asked);
            assert_eq!(asked.for_case(TURN_PROTOCOL, "turn-1"), Records::Trial);
            assert_eq!(
                asked.for_case("story.delivery/1", "scenario-1-trial-2"),
                Records::Trial
            );
            assert_eq!(asked.for_case("story.delivery/1", "issue-42"), asked);
        }
        assert!(is_scenario_case("scenario-1-trial-2"));
        assert!(is_scenario_case("scenario-a-b-trial-10"));
        assert!(!is_scenario_case("scenario-1-trial-"));
        assert!(!is_scenario_case("scenario--trial-2"));
        assert!(!is_scenario_case("scenario-1-trial-x"));
        assert!(!is_scenario_case("issue-42"));
        assert_eq!(Records::named("default"), Some(Records::Default));
        assert_eq!(Records::named("trial"), Some(Records::Trial));
        assert_eq!(Records::named("Trial"), None);
        assert_eq!(Records::named(""), None);
        assert_eq!(Records::default(), Records::Default);
        assert_eq!(Records::Trial.name(), "trial");
    }

    #[test]
    fn statuses_are_read_from_each_section() {
        let json = r#"{"claims":{"tests.pass":{"value":"true"}},
            "obligations":[{"id":"tests.required","status":"discharged"}],
            "actions":{"tests.run":{"status":"admissible"},"repository.merge":{"status":"approval-required"}},
            "outcomes":{"accepted":{"status":"blocked"}}}"#;
        let decision = Decision::from_json(json).expect("reads");
        assert!(decision.holds("tests.pass"));
        assert_eq!(
            decision
                .obligations
                .get("tests.required")
                .map(String::as_str),
            Some("discharged")
        );
        assert_eq!(decision.admissible(), vec!["tests.run".to_owned()]);
        assert_eq!(
            decision.approval_required(),
            vec!["repository.merge".to_owned()]
        );
        assert!(decision.legitimate().is_empty());
        assert_eq!(
            decision.frontier_line(),
            "frontier: repository.merge (approval-required), tests.run (admissible)"
        );
    }
}
