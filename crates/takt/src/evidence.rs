//! `canon-evidence/1` records, written by validators only, and the case's evidence port they
//! reach a case through ([`CaseEvidence`]).

use std::cell::RefCell;
use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use loom_sdk::commission::model::json as cjson;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, Observation, ObservationData, ObservationId,
};
use loom_sdk::commission::ports::evidence::{
    AttributedEvidence, EvidencePort, ObservationPort as _, submit_evidence,
};
use loom_sdk::governor::{CanonGovernor, FallibleCaseStore as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::case::{CaseDir, CaseFiles};

/// One evidence record, as Canon reads it.
#[derive(Debug, Clone, Serialize)]
pub struct Record {
    pub format: &'static str,
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub subject: String,
    pub subject_revision: String,
    pub observed_at: String,
    pub producer: String,
}

/// The format every record names.
const FORMAT: &str = "canon-evidence/1";

/// A record as [`Record::from_facts`] reads it back: the members [`Record`] writes and no other.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Read {
    format: String,
    id: String,
    kind: String,
    #[serde(default)]
    result: Option<String>,
    subject: String,
    subject_revision: String,
    observed_at: String,
    producer: String,
}

impl Record {
    /// A record of `kind` with `result` about `subject` at `revision`, produced by `producer` now.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        result: Option<String>,
        subject: impl Into<String>,
        revision: impl Into<String>,
        producer: impl Into<String>,
    ) -> Self {
        Self {
            format: FORMAT,
            id: id.into(),
            kind: kind.into(),
            result,
            subject: subject.into(),
            subject_revision: revision.into(),
            observed_at: crate::now(),
            producer: producer.into(),
        }
    }

    /// The record as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a record serializes")
    }

    /// The record as the facts of the evidence it is submitted as: its JSON, member for member.
    pub fn facts(&self) -> Result<cjson::Value> {
        cjson::parse(&self.to_json()).map_err(|error| anyhow!("the record {}: {error}", self.id))
    }

    /// The record `facts` are, when they are exactly the facts [`Record::facts`] gives for it;
    /// `None` for any other facts.
    #[must_use]
    pub fn from_facts(facts: &cjson::Value) -> Option<Self> {
        let mut text = String::new();
        cjson::push_value(&mut text, facts);
        let read: Read = serde_json::from_str(&text).ok()?;
        if read.format != FORMAT {
            return None;
        }
        let record = Self {
            format: FORMAT,
            id: read.id,
            kind: read.kind,
            result: read.result,
            subject: read.subject,
            subject_revision: read.subject_revision,
            observed_at: read.observed_at,
            producer: read.producer,
        };
        (record.facts().ok()? == *facts).then_some(record)
    }
}

/// A UUID-shaped id derived from `seed`: the first 16 bytes of its SHA-256, marked version 8
/// (custom) and the RFC 9562 variant, as the case store derives its records' ids.
pub(crate) fn uuid_from(seed: &str) -> String {
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

/// The one way a record reaches a case: the case's evidence port, an [`EvidencePort`] of
/// loom-sdk. A record is submitted with [`CaseEvidence::submit`], which reports the producer's
/// observation of it to the case's governor and submits it through loom-sdk's
/// `submit_evidence`, attributed to the producer the caller names. The port takes it only from a
/// producer of the run (`producers`), only citing an observation the case received with the
/// record's `observed_at` (the governor's `check_evidence`), and only as a record whose facts
/// are a `canon-evidence/1` record and nothing else; it writes it under the case's `evidence/`
/// with the attributed producer, whatever producer the record named. Anything else is refused
/// with nothing written.
pub struct CaseEvidence<'c> {
    case: RefCell<&'c mut CaseDir>,
    governor: CanonGovernor<CaseFiles>,
    producers: Vec<String>,
    written: RefCell<Option<PathBuf>>,
    refused: RefCell<Option<String>>,
}

impl<'c> CaseEvidence<'c> {
    /// The evidence port of `case`, taking records from `producers` alone.
    pub fn new(case: &'c mut CaseDir, producers: Vec<String>) -> Result<Self> {
        let files = CaseFiles::new(&case.dir, &case.id, &case.protocol.id);
        Ok(Self {
            case: RefCell::new(case),
            governor: CanonGovernor::new(files),
            producers,
            written: RefCell::new(None),
            refused: RefCell::new(None),
        })
    }

    /// Submits `record` as `producer`'s, for the action request `request` when an effect
    /// performing one wrote it: the producer's observation of it first, then the record citing
    /// that observation. Answers the file it was written to.
    ///
    /// # Errors
    /// The port's refusal, naming the producer and why.
    pub fn submit(
        &self,
        producer: &str,
        record: &Record,
        request: Option<&str>,
    ) -> Result<PathBuf> {
        let (id, files) = {
            let case = self.case.borrow();
            (
                case.id.clone(),
                CaseFiles::new(&case.dir, &case.id, &case.protocol.id),
            )
        };
        let case_id = CaseId(id.clone());
        let revision = files
            .get(&case_id)?
            .ok_or_else(|| anyhow!("no case {id}"))?
            .revision;
        let request = request.map_or(cjson::Value::Null, |request| {
            cjson::Value::Text(request.to_owned())
        });
        let observation = ObservationId(Uuid(uuid_from(&format!(
            "{id}/observation/{}/{}",
            record.id, record.observed_at
        ))));
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: producer.to_owned(),
                subject: format!("{}@{}", record.subject, record.subject_revision),
                observed_at: Timestamp(record.observed_at.clone()),
                payload: cjson::Value::Object(vec![
                    ("record".to_owned(), cjson::Value::Text(record.id.clone())),
                    ("kind".to_owned(), cjson::Value::Text(record.kind.clone())),
                    ("action_request".to_owned(), request.clone()),
                ]),
            }))
            .map_err(|error| {
                anyhow!(
                    "the case {id} did not receive {producer}'s observation of {}: {error:?}",
                    record.id
                )
            })?;
        let evidence = EvidenceData {
            evidence_id: EvidenceId(Uuid(uuid_from(&format!("{id}/evidence/{}", record.id)))),
            case_id,
            kind: record.kind.clone(),
            subject_revision: revision,
            producer: producer.to_owned(),
            observation_ids: vec![observation],
            facts: record.facts()?,
            provenance: cjson::Value::Object(vec![
                (
                    "producer".to_owned(),
                    cjson::Value::Text(producer.to_owned()),
                ),
                ("action_request".to_owned(), request),
            ]),
        };
        self.written.replace(None);
        self.refused.replace(None);
        if let Err(error) = submit_evidence(self, producer, evidence) {
            let why = self.refused.take().unwrap_or_else(|| format!("{error}"));
            bail!(
                "the case {id} refused the record {} from {producer}: {why}; nothing was written",
                record.id
            );
        }
        self.written
            .take()
            .ok_or_else(|| anyhow!("the record {} from {producer} was not written", record.id))
    }

    /// Refuses with `why`, kept for [`CaseEvidence::submit`] to report.
    fn refuse(&self, why: String) -> GovernorError {
        self.refused.replace(Some(why));
        GovernorError::GovernorUnavailable
    }
}

impl EvidencePort for CaseEvidence<'_> {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        let data = evidence.evidence().data();
        let id = self.case.borrow().id.clone();
        if data.case_id.0 != id {
            return Err(self.refuse(format!(
                "the port of the case {id} takes no record for {}",
                data.case_id.0
            )));
        }
        if !self.producers.contains(&data.producer) {
            return Err(self.refuse(format!(
                "{} is not a validator of the run, nor {}",
                data.producer,
                crate::run::CHOICE_PRODUCER
            )));
        }
        if data.observation_ids.is_empty() {
            return Err(self.refuse("it cites no observation".to_owned()));
        }
        if let Err(refusal) = self.governor.check_evidence(data) {
            return Err(self.refuse(format!("{refusal}")));
        }
        let Some(mut record) = Record::from_facts(&data.facts) else {
            return Err(self.refuse("its facts are not a canon-evidence/1 record".to_owned()));
        };
        record.producer.clone_from(&data.producer);
        match self.case.borrow_mut().add_evidence(&record) {
            Ok(path) => {
                self.written.replace(Some(path));
                Ok(())
            }
            Err(error) => Err(self.refuse(format!("{error:#}"))),
        }
    }
}
