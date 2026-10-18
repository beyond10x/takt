//! A Canon case on disk: `<tree>/.engineering/cases/<id>/` holds `case.yaml`, `protocol.ir.json`,
//! `evidence/*.json`, `decisions/*.json` and `branch.yaml`, the case's branch and its pull
//! request. Every evaluation reads the files and asks Canon.

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
    /// case never forks from another case's commits), and recorded with its base and the
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
            Some((_, Some(other))) => {
                let base = Branch::read(&tree.join(CASES_DIR).join(other))
                    .ok()
                    .and_then(|branch| branch.base);
                (base.clone(), base)
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

    /// Asks Canon for the decision over the case and its evidence, now; writes it under
    /// `decisions/` and answers it.
    pub fn evaluate(&self, seq: usize) -> Result<Decision> {
        let ir = eval::read_ir(&self.protocol.ir_json).map_err(|r| anyhow!("{r}"))?;
        let case_yaml = std::fs::read_to_string(self.dir.join("case.yaml"))?;
        let case = eval::read_case(&case_yaml).map_err(|r| anyhow!("{r}"))?;
        let records = self
            .evidence_texts()?
            .iter()
            .map(|text| eval::read_evidence(text).map_err(|r| anyhow!("{r}")))
            .collect::<Result<Vec<_>>>()?;
        let now = crate::now();
        let supplied = eval::Supplied {
            authority: None,
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
