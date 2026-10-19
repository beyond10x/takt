//! A Canon case on disk: `<tree>/.engineering/cases/<id>/` holds `case.yaml`, `protocol.ir.json`,
//! `evidence/*.json`, `decisions/*.json`, `branch.yaml`, the case's branch and its pull
//! request, `story.yaml`, the story the case was opened for, when it was, and `authority.yaml`,
//! the authority the operator granted the case, once granted, and `riders/<revision>/`, what the
//! run's store writes on the story committed, which its riders must equal byte for byte
//! ([`approvals_only`]). Every evaluation reads the files
//! and asks Canon. A case is opened once and resumed by every later run on it.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};
use b10x_canon::eval;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::decision::Decision;
use crate::evidence::Record;
use crate::protocol::LoadedProtocol;

/// The directory under the tree that holds cases; excluded from the tree's change detection.
pub const CASES_DIR: &str = ".engineering/cases";

/// The prefix of a case's branch: `takt/<case id>`.
pub const BRANCH_PREFIX: &str = "takt/";

/// The file under the case directory that records the branch and its pull request.
pub const BRANCH_FILE: &str = "branch.yaml";

/// The file under the case directory that holds the authority the operator granted the case.
pub const AUTHORITY_FILE: &str = "authority.yaml";

/// The file under the case directory that names the story the case was opened for.
pub const STORY_FILE: &str = "story.yaml";

/// The file under the case directory whoever holds the case creates, holding the instant it
/// took hold: a run while it runs, `resume` while it brings the case to its branch, a grant
/// while it is checked, recorded and written. Created with `create_new`, so of two that try at
/// once exactly one holds; no grant lands and no second run starts meanwhile.
pub const LOCK_FILE: &str = "run.lock";

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
            let Some(base) = Branch::read(&tree.join(CASES_DIR).join(other))
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

/// Whether `path`, relative to the tree, is the cases directory or lies under it: what the
/// kernel's change detection, commit and clean leave out. The directory and its contents only,
/// never a path that merely starts with the same characters.
#[must_use]
pub fn is_under_cases(path: &str) -> bool {
    path == CASES_DIR
        || path
            .strip_prefix(CASES_DIR)
            .is_some_and(|rest| rest.starts_with('/'))
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
/// id reaches outside `.engineering/cases`, nests a case inside another, holds a character
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
                "the case {id} is held {since} ({CASES_DIR}/{id}/{LOCK_FILE}) by a run or an approval; if neither is in progress, remove the file"
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

/// The authority the operator granted the case, `authority.yaml`: a list of decisions, at most
/// one per capability (Canon refuses a capability decided twice), read into every evaluation
/// of the case as the supplied authority, each grant only at the revision it was given at.
/// Absent until the first grant: a case with none stops at the gate with `ApprovalRequired`,
/// and a connection to the forge is never authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Authority {
    /// The decisions, in the order they were given.
    pub decisions: Vec<AuthorityDecision>,
}

impl Authority {
    /// Reads the case's `authority.yaml`; `None` when the case has none.
    pub fn read(case_dir: &Path) -> Result<Option<Self>> {
        let path = case_dir.join(AUTHORITY_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let authority =
            serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
        Ok(Some(authority))
    }

    /// Writes the case's `authority.yaml`.
    pub fn write(&self, case_dir: &Path) -> Result<()> {
        let path = case_dir.join(AUTHORITY_FILE);
        let text = serde_yaml_ng::to_string(self).context("write the authority")?;
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
    }

    /// The decision on `capability`, when the case decides it.
    #[must_use]
    pub fn decision(&self, capability: &str) -> Option<&AuthorityDecision> {
        self.decisions
            .iter()
            .find(|decision| decision.capability == capability)
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
    /// nothing in the store to record on.
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

/// Whether `approver` may grant a story's capability: `human:<name>` or `agent:<role>`, an
/// actor the store writes ([`is_store_actor`]). The store writes an approval's record as its
/// approver ([`crate::frontier::Aep`] sets `AEP_ACTOR`); `system` and `service:<name>` are the
/// store's and tools' own actors, not who grants, and takt's own actor
/// ([`crate::frontier::ACTOR`]), which writes the story's `test_result` and move, never grants
/// its own merge.
fn is_approver(approver: &str) -> bool {
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
/// case another holds, a capability that is not an identifier, an empty approver, for a story's
/// case an approver that is not `human:<name>` or `agent:<role>` as the store writes them, a
/// capability no action of the case's protocol requires (a grant nothing asks for is a typo, told at the
/// command line rather than by a run that stops at the gate again), a capability whose actions
/// are about no single artifact ([`LoadedProtocol::grant_subject`]), a case that is not at the
/// gate for the capability (its latest decision does not find an action requiring it
/// approval-required, or was computed at another revision of that artifact than the case is at
/// now), and a capability the case already decides at that revision. A grant given at another
/// revision is replaced: it authorises nothing here, and Canon refuses a capability decided
/// twice.
pub fn prepare_grant(
    tree: &Path,
    id: &str,
    capability: &str,
    approver: &str,
) -> Result<PendingGrant> {
    check_id(id)?;
    let dir = tree.join(CASES_DIR).join(id);
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
    // A story's grant is recorded in the store as its approver, and its record rides to the
    // base only with the approver as its actor: the approver is a person or an agent role the
    // store writes as.
    let story = story_of(&dir)?;
    if story.is_some() && !is_approver(approver) {
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
    authority.decisions.push(AuthorityDecision {
        capability: capability.to_owned(),
        decision: Grant::Granted,
        approver: approver.to_owned(),
        granted_at: granted_at.clone(),
        revision: Some(revision.clone()),
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
    let story: StoryRef =
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    Ok(Some(story.id))
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
    let cases = tree.join(CASES_DIR);
    if active.is_empty() || !cases.is_dir() {
        return Ok(None);
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&cases)
        .with_context(|| format!("read {}", cases.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    dirs.sort();
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
    // The grants at `revision` by the rule `grants_at` reads, the first decision on each
    // capability, not yet found recorded; each record found takes its grant out.
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
    let dir = tree.join(CASES_DIR).join(id);
    if !dir.join("case.yaml").is_file() {
        bail!(
            "the branch {} names no case at {}",
            shown(&branch),
            dir.display()
        );
    }
    Ok(dir)
}

/// One case's files.
#[derive(Debug, Clone)]
pub struct CaseDir {
    /// The case id.
    pub id: String,
    /// `<tree>/.engineering/cases/<id>`.
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
    /// `case.yaml` and a `takt/<id>` branch only with its case.
    pub fn open(
        tree: &Path,
        protocol: LoadedProtocol,
        id: &str,
        intent: &str,
        inputs: &[(String, String)],
    ) -> Result<Self> {
        check_id(id)?;
        let dir = tree.join(CASES_DIR).join(id);
        if dir.exists() {
            return Err(anyhow!("the case {id} already exists at {}", dir.display()));
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
        let branch = Branch {
            name: Branch::name_of(id),
            base,
            base_commit: Some(crate::git::run(
                tree,
                &["rev-parse", "--verify", from.as_deref().unwrap_or("HEAD")],
            )?),
            left,
            pushed: None,
            pull_request: None,
        };
        crate::git::create_branch(tree, &branch.name, from.as_deref())?;
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
            case.create(intent, &branch)?;
            Ok(case)
        })();
        opened.map_err(|error: anyhow::Error| {
            match Self::undo_open(tree, &dir, &branch.name, branch.left.as_deref(), &head) {
                Ok(()) => error,
                Err(undo) => error.context(format!(
                    "and the half-opened case {id} could not be taken back: {undo:#}"
                )),
            }
        })
    }

    /// Takes back an open that failed after its branch was made: the case directory if any of
    /// it was written, the cases directory when it is empty, and the branch, the tree back on
    /// `left`, or detached at `start` when it was detached.
    fn undo_open(
        tree: &Path,
        dir: &Path,
        branch: &str,
        left: Option<&str>,
        start: &str,
    ) -> Result<()> {
        if std::fs::symlink_metadata(dir).is_ok() {
            std::fs::remove_dir_all(dir)
                .with_context(|| format!("remove the half-written case {}", dir.display()))?;
        }
        // The cases directory stays only while a case is in it.
        if let Some(cases) = dir.parent() {
            let _ = std::fs::remove_dir(cases);
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
        let dir = tree.join(CASES_DIR).join(id);
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
        self.doc["inputs"]
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

    /// Creates the directory and writes the case's files, `case.yaml` last.
    fn create(&self, intent: &str, branch: &Branch) -> Result<()> {
        std::fs::create_dir_all(self.dir.join("evidence"))
            .with_context(|| format!("create {}", self.dir.display()))?;
        std::fs::create_dir_all(self.dir.join("decisions"))?;
        std::fs::write(self.dir.join("protocol.ir.json"), &self.protocol.ir_json)?;
        std::fs::write(self.dir.join("protocol.yaml"), &self.protocol.yaml)?;
        std::fs::write(self.dir.join("intent.md"), intent)?;
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

    /// Moves `artifact` to `revision`; evidence about the old revision no longer applies.
    pub fn set_revision(&mut self, artifact: &str, revision: &str) -> Result<()> {
        let entry = self
            .doc
            .get_mut("artifacts")
            .and_then(|a| a.get_mut(artifact))
            .ok_or_else(|| anyhow!("the case declares no artifact {artifact}"))?;
        entry["revision"] = revision.into();
        self.write()
    }

    /// Adds an evidence record, written as `evidence/<n>-<id>.json`.
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
    /// is read by Canon's reader, as [`CaseDir::evaluate`] reads it (YAML, of which a JSON
    /// record is one); a record that does not read is an error naming its file, never an
    /// answer either way: every record is read before the answer is given.
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

    /// Every evidence record under the case, read by Canon's own reader, in name order; the
    /// first that does not read (an empty file, a bare scalar, a missing field) is an error
    /// naming its file, and nothing is answered. What [`CaseDir::evaluate`] hands Canon and
    /// what [`CaseDir::has_evidence`] answers from: one reader, all records or none.
    fn evidence_records(&self) -> Result<Vec<b10x_canon::model::EvidenceRecord>> {
        self.evidence_texts()?
            .iter()
            .map(|(path, text)| {
                eval::read_evidence(text)
                    .map_err(|r| anyhow!("{r}"))
                    .with_context(|| format!("read the evidence record {}", path.display()))
            })
            .collect()
    }

    /// The evidence records under the case, `evidence/*.json` and `*.yaml`, in name order, each
    /// with its path.
    fn evidence_texts(&self) -> Result<Vec<(PathBuf, String)>> {
        let evidence = self.dir.join("evidence");
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

    /// Asks Canon for the decision over the case, its evidence and the authority the operator
    /// granted it (`authority.yaml`, read now, so a grant written since the last evaluation is
    /// seen; a grant only at the revision it was given at of the artifact its action is about),
    /// now; writes it
    /// under `decisions/` and answers it. An evidence record that does not read is an error
    /// naming its file.
    pub fn evaluate(&self, seq: usize) -> Result<Decision> {
        let ir = eval::read_ir(&self.protocol.ir_json).map_err(|r| anyhow!("{r}"))?;
        let case_yaml = std::fs::read_to_string(self.dir.join("case.yaml"))?;
        let case = eval::read_case(&case_yaml).map_err(|r| anyhow!("{r}"))?;
        let records = self.evidence_records()?;
        // The revision a grant is filtered by is the one in the `case.yaml` Canon is handed,
        // read from the same bytes: never this `CaseDir`'s in-memory copy, which another
        // holder may have moved on since it was read.
        let snapshot: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&case_yaml).context("read the case snapshot")?;
        let revision_of = |capability: &str| {
            let subject = self.protocol.grant_subject(capability).ok()?;
            snapshot["artifacts"][subject.as_str()]["revision"]
                .as_str()
                .map(str::to_owned)
        };
        let authority = Authority::read(&self.dir)?
            .map(|authority| authority.canon_by(revision_of))
            .transpose()?;
        let now = crate::now();
        let supplied = eval::Supplied {
            authority: authority.as_deref(),
            at: Some(&now),
            decisions: None,
        };
        let decision =
            eval::evaluate_with(&ir, &case, &records, supplied).map_err(|r| anyhow!("{r}"))?;
        let json = eval::render(&decision);
        std::fs::write(
            self.dir.join("decisions").join(format!("{seq:04}.json")),
            &json,
        )?;
        Decision::from_json(&json)
    }
}
