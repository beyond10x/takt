//! A Canon case on disk: `<tree>/.engineering/cases/<id>/` holds `case.yaml`, `protocol.ir.json`,
//! `evidence/*.json` and `decisions/*.json`. Every evaluation reads the files and asks Canon.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow};
use b10x_canon::eval;
use sha2::{Digest as _, Sha256};

use crate::decision::Decision;
use crate::evidence::Record;
use crate::protocol::LoadedProtocol;

/// The directory under the tree that holds cases; excluded from the tree's change detection.
pub const CASES_DIR: &str = ".engineering/cases";

/// One case's files.
#[derive(Debug, Clone)]
pub struct CaseDir {
    /// The case id.
    pub id: String,
    /// `<tree>/.engineering/cases/<id>`.
    pub dir: PathBuf,
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
    /// kernel takes no default, because the inputs select the obligations the case owes.
    /// Nothing is written until `HEAD` and the inputs are in order, and a write that fails takes
    /// the directory with it, so a case directory exists only with its `case.yaml`.
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
        let case = Self {
            id: id.to_owned(),
            dir,
            protocol,
            doc: doc.into(),
            written: 0,
        };
        if let Err(error) = case.create(intent) {
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
    fn create(&self, intent: &str) -> Result<()> {
        std::fs::create_dir_all(self.dir.join("evidence"))
            .with_context(|| format!("create {}", self.dir.display()))?;
        std::fs::create_dir_all(self.dir.join("decisions"))?;
        std::fs::write(self.dir.join("protocol.ir.json"), &self.protocol.ir_json)?;
        std::fs::write(self.dir.join("protocol.yaml"), &self.protocol.yaml)?;
        std::fs::write(self.dir.join("intent.md"), intent)?;
        self.write()
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
