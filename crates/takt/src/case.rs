//! A Canon case on disk: `<tree>/.engineering/cases/<id>/` holds `case.yaml`, `protocol.ir.json`,
//! `evidence/*.json`, `decisions/*.json`, `branch.yaml`, the case's branch and its pull
//! request, `story.yaml`, the story the case was opened for, when it was, and `authority.yaml`,
//! the authority the operator granted the case, once granted. Every evaluation reads the files
//! and asks Canon. A case is opened once and resumed by every later run on it.

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

/// The file under the case directory a run writes while it holds the case, holding the run's
/// start instant: no grant lands and no second run starts meanwhile.
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

/// The instant the run holding the case at `case_dir` started, from its `run.lock`; `None`
/// when no run holds it.
pub fn held_since(case_dir: &Path) -> Result<Option<String>> {
    let path = case_dir.join(LOCK_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    Ok(Some(text.trim().to_owned()))
}

/// Refuses while a run holds the case `id` at `case_dir`, naming the lock. A run that crashed
/// leaves its lock behind; the refusal says to remove the file by hand.
fn refuse_if_held(case_dir: &Path, id: &str) -> Result<()> {
    if let Some(since) = held_since(case_dir)? {
        bail!(
            "the case {id} is held by a run started at {since} ({CASES_DIR}/{id}/{LOCK_FILE}); if that run is gone, remove the file"
        );
    }
    Ok(())
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
/// `canon-authority/1` entry has, and beside them who decided it and when. Canon is handed the
/// two keys it declares ([`Authority::canon`]) and would refuse the other two, which are the
/// case's own record of the operator's decision.
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
}

/// The authority the operator granted the case, `authority.yaml`: a list of decisions, at most
/// one per capability (Canon refuses a capability decided twice), read into every evaluation
/// of the case as the supplied authority. Absent until the first grant: a case with none stops
/// at the gate with `ApprovalRequired`, and a connection to the forge is never authority.
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

    /// The document as Canon reads it: a `canon-authority/1` list of `capability` and
    /// `decision` entries, nothing else.
    pub fn canon(&self) -> Result<String> {
        #[derive(Serialize)]
        struct Entry<'a> {
            capability: &'a str,
            decision: Grant,
        }
        let entries: Vec<Entry<'_>> = self
            .decisions
            .iter()
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
}

impl Approval {
    /// Where to go and look: the case's `authority.yaml`, as `<case>/authority.yaml`.
    #[must_use]
    pub fn reference(&self) -> String {
        format!("{}/{AUTHORITY_FILE}", self.case)
    }
}

/// The capabilities the actions of the case's protocol require, read from the case's own
/// `protocol.yaml` (each action's `requires` list of `capability` entries), sorted, with the
/// protocol's id: what a grant may name.
fn required_capabilities(case_dir: &Path) -> Result<(String, Vec<String>)> {
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
    let mut capabilities: Vec<String> = doc["actions"]
        .as_mapping()
        .map(|actions| {
            actions
                .values()
                .filter_map(|action| action["requires"].as_sequence())
                .flatten()
                .filter_map(|requirement| requirement["capability"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    capabilities.sort();
    capabilities.dedup();
    Ok((id, capabilities))
}

/// Grants `capability` to the case `id` under `tree`, decided by `approver` now: the decision
/// is appended to the case's `authority.yaml`, and what was granted is answered for the store
/// to record. Refused with nothing written: a case that does not exist, a capability that is
/// not an identifier, a capability no action of the case's protocol requires (a grant nothing
/// asks for is a typo, told at the command line rather than by a run that stops at the gate
/// again), a capability the case already decides (Canon refuses a capability decided twice,
/// so a second grant would break every later evaluation), an empty approver.
pub fn grant(tree: &Path, id: &str, capability: &str, approver: &str) -> Result<Approval> {
    let dir = tree.join(CASES_DIR).join(id);
    if !dir.join("case.yaml").is_file() {
        bail!("no case {id} at {}", dir.display());
    }
    // A run holds its case while it runs: a grant given meanwhile is refused, not written into
    // a case a judgement step is in flight on.
    refuse_if_held(&dir, id)?;
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
    let (protocol, required) = required_capabilities(&dir)?;
    if !required.iter().any(|required| required == capability) {
        if required.is_empty() {
            bail!(
                "no action of {protocol} requires a capability; {capability} grants the case {id} nothing"
            );
        }
        bail!(
            "no action of {protocol} requires {capability}; the capabilities its actions require: {}",
            required.join(", ")
        );
    }
    let mut authority = Authority::read(&dir)?.unwrap_or_default();
    if let Some(decided) = authority.decision(capability) {
        bail!(
            "the case {id} already decides {capability}: {} by {} at {}",
            decided.decision,
            decided.approver,
            decided.granted_at
        );
    }
    let granted_at = crate::now();
    authority.decisions.push(AuthorityDecision {
        capability: capability.to_owned(),
        decision: Grant::Granted,
        approver: approver.to_owned(),
        granted_at: granted_at.clone(),
    });
    authority.write(&dir)?;
    Ok(Approval {
        case: id.to_owned(),
        story: story_of(&dir)?,
        capability: capability.to_owned(),
        approver: approver.to_owned(),
        granted_at,
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

/// The latest decision written under the case, the `decisions/*.json` with the highest name;
/// `None` before the first evaluation.
pub fn latest_decision(case_dir: &Path) -> Result<Option<Decision>> {
    let decisions = case_dir.join("decisions");
    if !decisions.is_dir() {
        return Ok(None);
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&decisions)
        .with_context(|| format!("read {}", decisions.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let Some(latest) = paths.last() else {
        return Ok(None);
    };
    let json =
        std::fs::read_to_string(latest).with_context(|| format!("read {}", latest.display()))?;
    Decision::from_json(&json).map(Some)
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
    /// branch it left as `branch.yaml`; a branch that exists refuses the case. Nothing is
    /// written until `HEAD`, the inputs and the branch are in order, and a write that fails
    /// takes the directory with it, so a case directory exists only with its `case.yaml`.
    pub fn open(
        tree: &Path,
        protocol: LoadedProtocol,
        id: &str,
        intent: &str,
        inputs: &[(String, String)],
    ) -> Result<Self> {
        let dir = tree.join(CASES_DIR).join(id);
        if dir.exists() {
            return Err(anyhow!("the case {id} already exists at {}", dir.display()));
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
            left,
            pushed: None,
            pull_request: None,
        };
        crate::git::create_branch(tree, &branch.name, from.as_deref())?;
        // The branch's tip is the implementation revision: not the HEAD the case was asked on
        // when the branch forked from a base elsewhere.
        let head = crate::git::head(tree)?;
        let mut case = Self {
            id: id.to_owned(),
            dir,
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
            case.doc["artifacts"]["implementation"]["revision"] = head.into();
        }
        if let Err(error) = case.create(intent, &branch) {
            if case.dir.exists()
                && let Err(removal) = std::fs::remove_dir_all(&case.dir)
            {
                return Err(error.context(format!(
                    "and the half-written case {} could not be removed: {removal}",
                    case.dir.display()
                )));
            }
            // The cases directory stays only while a case is in it.
            if let Some(cases) = case.dir.parent() {
                let _ = std::fs::remove_dir(cases);
            }
            return Err(error);
        }
        Ok(case)
    }

    /// Opens the case `id` that exists under `tree` again, for a run that resumes it: its files
    /// as they are, the records written so far counted so the next is numbered after them, the
    /// protocol from the case's own `protocol.yaml`, and the tree put on the case's branch,
    /// which the validators resolve the case from. The implementation revision is the branch's
    /// `HEAD`: a commit made on the branch between two runs moves it, and with it every claim
    /// returns to unknown and the validators run again, so nothing goes through on records
    /// about the commit before. Nothing else is written. A directory without a `case.yaml` is
    /// no case.
    pub fn resume(tree: &Path, id: &str) -> Result<Self> {
        let dir = tree.join(CASES_DIR).join(id);
        let snapshot = dir.join("case.yaml");
        if !snapshot.is_file() {
            bail!("no case {id} at {}", dir.display());
        }
        // One run at a time: a case a run holds is not resumed by a second.
        refuse_if_held(&dir, id)?;
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

    /// Takes hold of the case for a run: writes `run.lock` with the instant now. Refused while
    /// another run holds it.
    pub fn hold(&self) -> Result<()> {
        refuse_if_held(&self.dir, &self.id)?;
        let path = self.dir.join(LOCK_FILE);
        std::fs::write(&path, format!("{}\n", crate::now()))
            .with_context(|| format!("write {}", path.display()))
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

    /// Removes the case, for one the store would not take: its directory, the cases directory
    /// when this was the last case in it, and its branch, the tree returning to the branch the
    /// case left (detached when it left none). Nothing of the case remains.
    pub fn discard(self) -> Result<()> {
        let branch = Branch::read(&self.dir)?;
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

    /// Whether a record of `kind` exists about `subject` at its current revision.
    pub fn has_evidence(&self, kind: &str, subject: &str) -> Result<bool> {
        let Some(revision) = self.revision(subject) else {
            return Ok(false);
        };
        for text in self.evidence_texts()? {
            let doc: serde_json::Value = serde_json::from_str(&text)?;
            if doc["kind"] == kind
                && doc["subject"] == subject
                && doc["subject_revision"] == revision.as_str()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn evidence_texts(&self) -> Result<Vec<String>> {
        let mut names: Vec<_> = std::fs::read_dir(self.dir.join("evidence"))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        names.sort();
        names
            .iter()
            .filter(|path| path.extension().is_some_and(|e| e == "json" || e == "yaml"))
            .map(|path| {
                std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
            })
            .collect()
    }

    /// Asks Canon for the decision over the case, its evidence and the authority the operator
    /// granted it (`authority.yaml`, read now, so a grant written since the last evaluation is
    /// seen), now; writes it under `decisions/` and answers it.
    pub fn evaluate(&self, seq: usize) -> Result<Decision> {
        let ir = eval::read_ir(&self.protocol.ir_json).map_err(|r| anyhow!("{r}"))?;
        let case_yaml = std::fs::read_to_string(self.dir.join("case.yaml"))?;
        let case = eval::read_case(&case_yaml).map_err(|r| anyhow!("{r}"))?;
        let records = self
            .evidence_texts()?
            .iter()
            .map(|text| eval::read_evidence(text).map_err(|r| anyhow!("{r}")))
            .collect::<Result<Vec<_>>>()?;
        let authority = Authority::read(&self.dir)?
            .map(|authority| authority.canon())
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
