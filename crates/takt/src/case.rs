//! A Canon case on disk: `<tree>/.engineering/cases/<id>/` holds `case.yaml`, `protocol.ir.json`,
//! `evidence/*.json`, `decisions/*.json`, `branch.yaml`, the case's branch and its pull
//! request, `story.yaml`, the story the case was opened for, when it was, and `authority.yaml`,
//! the authority the operator granted the case, once granted. Every evaluation reads the files
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

/// Whether `c` has no business in a name that is shown to people: a control character, a
/// whitespace or separator character (spaces, U+2028, U+2029), or a format character.
fn is_unshowable(c: char) -> bool {
    let point = u32::from(c);
    c.is_control()
        || c.is_whitespace()
        || FORMAT_CHARACTERS
            .iter()
            .any(|&(first, last)| (first..=last).contains(&point))
}

/// Refuses a case id that is not one plain name under the cases directory: empty, `.`, a `..`
/// anywhere, a leading `/`, a path separator, or a character that does not show as itself (a
/// control, whitespace, separator or Unicode format character, named by its code point). Every
/// id an operator gives (`takt approve`, `takt run --case`, `RunOptions.case_id`) passes here
/// before it is joined to a path, so no id reaches outside `.engineering/cases`, nests a case
/// inside another, or displays as another id in a message or a `takt/<id>` branch.
pub fn check_id(id: &str) -> Result<()> {
    let why = if id.is_empty() {
        Some("it is empty".to_owned())
    } else if id == "." {
        Some("it names the cases directory itself".to_owned())
    } else if id.contains("..") {
        Some("it contains `..`".to_owned())
    } else if id.starts_with('/') {
        Some("it starts with `/`".to_owned())
    } else if id.contains(['/', '\\']) {
        Some("it contains a path separator".to_owned())
    } else {
        id.chars().find(|c| is_unshowable(*c)).map(|c| {
            format!(
                "it contains U+{:04X}, a control, whitespace, separator or format character",
                u32::from(c)
            )
        })
    };
    match why {
        Some(why) => bail!(
            "the case id `{}` is refused: {why}; a case id is one name under {CASES_DIR}",
            id.escape_debug()
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
/// `canon-authority/1` entry has, and beside them who decided it, when, and at which
/// implementation revision. Canon is handed the two keys it declares ([`Authority::canon`])
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
    /// The implementation revision the case was at the gate at when the grant was given: the
    /// one revision it authorises. A grant without one (written before grants were bound to a
    /// revision) authorises none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

impl AuthorityDecision {
    /// Whether the decision applies at the implementation revision `at`: a denial everywhere,
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

    /// Whether the case holds a grant of `capability` at the implementation revision
    /// `revision`.
    #[must_use]
    pub fn grants_at(&self, capability: &str, revision: &str) -> bool {
        self.decision(capability).is_some_and(|decision| {
            decision.decision == Grant::Granted && decision.applies_at(Some(revision))
        })
    }

    /// The document as Canon reads it at the implementation revision `at`: a
    /// `canon-authority/1` list of `capability` and `decision` entries, nothing else, holding
    /// the decisions that apply there ([`AuthorityDecision::applies_at`]). A grant given at
    /// another revision is left out, so Canon finds the capability undecided and the action it
    /// guards approval-required again: a new commit needs a new grant.
    pub fn canon(&self, at: Option<&str>) -> Result<String> {
        #[derive(Serialize)]
        struct Entry<'a> {
            capability: &'a str,
            decision: Grant,
        }
        let entries: Vec<Entry<'_>> = self
            .decisions
            .iter()
            .filter(|decision| decision.applies_at(at))
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
    /// The implementation revision the grant authorises: the one the case reached the gate at.
    pub revision: String,
}

impl Approval {
    /// Where to go and look: the case's `authority.yaml`, as `<case>/authority.yaml`.
    #[must_use]
    pub fn reference(&self) -> String {
        format!("{}/{AUTHORITY_FILE}", self.case)
    }
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

/// The implementation revision a decision was computed at, from what its explanation says it
/// was computed from; `None` when it does not say.
fn decided_at(decision: &Decision) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_str(&decision.json).ok()?;
    doc["explanation"]["computed_from"]["case"]["artifacts"]["implementation"]["revision"]
        .as_str()
        .map(str::to_owned)
}

/// The implementation revision the case at `case_dir` is at, from its `case.yaml`.
fn implementation_of(case_dir: &Path) -> Result<Option<String>> {
    let path = case_dir.join("case.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    Ok(doc["artifacts"]["implementation"]["revision"]
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
/// implementation revision the case reached the gate at: the decision goes into the case's
/// `authority.yaml`, and what was granted is answered for the store to record. Shorthand for
/// [`prepare_grant`] and [`PendingGrant::write`].
pub fn grant(tree: &Path, id: &str, capability: &str, approver: &str) -> Result<Approval> {
    prepare_grant(tree, id, capability, approver)?.write()
}

/// Checks a grant of `capability` to the case `id` under `tree` by `approver` and holds the case
/// for it, writing nothing. The case is held first (`run.lock`, taken atomically), so a grant
/// never lands while a run holds the case, and no run starts until the grant is written or
/// dropped. Refused: an id that is not a case id ([`check_id`]), a case that does not exist, a
/// case another holds, a capability that is not an identifier, an empty approver, a capability
/// no action of the case's protocol requires (a grant nothing asks for is a typo, told at the
/// command line rather than by a run that stops at the gate again), a case that is not at the
/// gate for the capability (its latest decision does not find an action requiring it
/// approval-required, or was computed at another implementation revision than the case is at
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
    // At the gate: the latest decision finds an action requiring the capability
    // approval-required, and was computed at the revision the case is at now. That revision is
    // the one the grant authorises.
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
    let Some(revision) = implementation_of(&dir)? else {
        bail!(
            "the case {id} has no implementation revision for a grant of {capability} to authorise"
        );
    };
    let gate = decided_at(&latest);
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
        story: story_of(&dir)?,
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

/// The directory of the case whose branch `tree` is on: `takt/<id>` names the case `<id>`.
pub fn current(tree: &Path) -> Result<PathBuf> {
    let branch = crate::git::current_branch(tree)?
        .ok_or_else(|| anyhow!("the tree {} is not on a branch", tree.display()))?;
    let id = branch.strip_prefix(BRANCH_PREFIX).ok_or_else(|| {
        anyhow!(
            "the tree {} is on {branch}, not on a case's branch {BRANCH_PREFIX}<id>",
            tree.display()
        )
    })?;
    check_id(id).with_context(|| format!("the branch {branch} names no case"))?;
    let dir = tree.join(CASES_DIR).join(id);
    if !dir.join("case.yaml").is_file() {
        bail!("the branch {branch} names no case at {}", dir.display());
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
        let left = crate::git::current_branch(tree)?;
        let (base, from) = match left
            .as_deref()
            .map(|name| (name, name.strip_prefix(BRANCH_PREFIX)))
        {
            Some((name, Some(other))) => {
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
    /// about the commit before. Nothing else is written. A directory without a `case.yaml` is
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
        if case
            .revision("implementation")
            .is_some_and(|revision| revision != head)
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
    /// seen; a grant only at the implementation revision it was given at), now; writes it
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
        let implementation = snapshot["artifacts"]["implementation"]["revision"]
            .as_str()
            .map(str::to_owned);
        let authority = Authority::read(&self.dir)?
            .map(|authority| authority.canon(implementation.as_deref()))
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
