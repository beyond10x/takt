//! `canon-evidence/1` records, written by validators only, and the case's evidence port they
//! reach a case through ([`CaseEvidence`]).

use std::cell::RefCell;
use std::collections::BTreeSet;
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

use crate::case::{CaseDir, CaseFiles, LatestStamp};

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
    /// The stamp [`Record::new`] took from the clock, never written: while `observed_at` is
    /// still it, the stamp is the clock's, and the case's evidence port stamps the record again
    /// when it is submitted ([`CaseEvidence::submit`]); a stamp a caller set is kept.
    #[serde(skip)]
    clock: Option<String>,
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
    /// A record of `kind` with `result` about `subject` at `revision`, produced by `producer` now,
    /// stamped to the microsecond.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        result: Option<String>,
        subject: impl Into<String>,
        revision: impl Into<String>,
        producer: impl Into<String>,
    ) -> Self {
        let observed_at = crate::instant_micros(crate::clock_micros());
        Self {
            format: FORMAT,
            id: id.into(),
            kind: kind.into(),
            result,
            subject: subject.into(),
            subject_revision: revision.into(),
            clock: Some(observed_at.clone()),
            observed_at,
            producer: producer.into(),
        }
    }

    /// Whether `observed_at` is the stamp [`Record::new`] took from the clock: no caller set it.
    #[must_use]
    pub fn clock_stamped(&self) -> bool {
        self.clock.as_deref() == Some(self.observed_at.as_str())
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
            clock: None,
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
    /// The kinds each order form of the case's protocol reads ([`LoadedProtocol::order_reads`]).
    ///
    /// [`LoadedProtocol::order_reads`]: crate::protocol::LoadedProtocol::order_reads
    orders: Vec<BTreeSet<String>>,
    written: RefCell<Option<PathBuf>>,
    refused: RefCell<Option<String>>,
}

impl<'c> CaseEvidence<'c> {
    /// The evidence port of `case`, taking records from `producers` alone.
    ///
    /// # Errors
    /// The case's protocol does not compile.
    pub fn new(case: &'c mut CaseDir, producers: Vec<String>) -> Result<Self> {
        let files = CaseFiles::new(&case.dir, &case.id, &case.protocol.id);
        let orders = case.protocol.order_reads()?;
        Ok(Self {
            case: RefCell::new(case),
            governor: CanonGovernor::new(files),
            producers,
            orders,
            written: RefCell::new(None),
            refused: RefCell::new(None),
        })
    }

    /// The instant, in microseconds since the epoch, a record of `kind` whose stamp is the clock's
    /// is stamped with when it is submitted, `latest` the case's latest stamp
    /// ([`CaseDir::latest_stamp`]): [`stamp_after`] the clock now, ordered when an order form of
    /// the protocol reads both `kind` and another kind observed in the latest tick.
    fn stamp_for(&self, kind: &str, latest: Option<&LatestStamp>) -> i128 {
        let ordered = latest.is_some_and(|latest| {
            latest.kinds.iter().any(|other| {
                other != kind
                    && self
                        .orders
                        .iter()
                        .any(|form| form.contains(other) && form.contains(kind))
            })
        });
        stamp_after(crate::clock_micros(), latest, ordered)
    }

    /// Submits `record` as `producer`'s, for the action request `request` when an effect
    /// performing one wrote it: the producer's observation of it first, then the record citing
    /// that observation. Answers the file it was written to.
    ///
    /// A record whose stamp is the clock's ([`Record::clock_stamped`]) is stamped again now, to the
    /// microsecond, as [`CaseEvidence::stamp_for`] says: never before the case's latest record,
    /// and in a later tick than it where an order form reads both kinds. Nothing waits for the
    /// clock. A stamp a caller set is kept.
    ///
    /// # Errors
    /// The port's refusal, naming the producer and why.
    pub fn submit(
        &self,
        producer: &str,
        record: &Record,
        request: Option<&str>,
    ) -> Result<PathBuf> {
        let restamped = if record.clock_stamped() {
            let latest = self.case.borrow().latest_stamp()?;
            let mut again = record.clone();
            again.observed_at =
                crate::instant_micros(self.stamp_for(&record.kind, latest.as_ref()));
            Some(again)
        } else {
            None
        };
        let record = restamped.as_ref().unwrap_or(record);
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

/// The stamp, in microseconds since the epoch, of a record whose stamp is the clock's, `clock`
/// the clock now and `latest` the case's latest stamp: `clock`, but never before the latest
/// record, so a case's stamps only grow; and when `ordered` (an order form of the protocol reads
/// the record's kind and a kind in the latest tick), in a later tick at the resolution Canon
/// compares the case in, the start of the next tick when the clock has not left the latest one.
/// Records all written to the microsecond are a microsecond apart at least; a case holding a
/// record in whole seconds is compared in whole seconds, and an ordered record is stamped in the
/// next second at once. Nothing waits for the clock. A floor past the last instant Canon writes
/// is held at [`LAST_MICROS`].
pub(crate) fn stamp_after(clock: i128, latest: Option<&LatestStamp>, ordered: bool) -> i128 {
    let Some(latest) = latest else {
        return clock;
    };
    let floor = if ordered {
        (latest.tick + 1) * latest.tick_micros()
    } else {
        latest.micros
    };
    clock.max(floor.min(LAST_MICROS))
}

/// 9999-12-31T23:59:59.999999Z in microseconds since the epoch: the last microsecond an instant
/// Canon reads can name.
pub(crate) const LAST_MICROS: i128 = 253_402_300_799_999_999;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::stamp_after;
    use crate::case::LatestStamp;

    /// 2026-10-29T12:00:00Z in microseconds since the epoch.
    const NOON: i128 = 1_761_739_200_000_000;

    fn latest(digits: u32, tick: i128, micros: i128) -> LatestStamp {
        LatestStamp {
            digits,
            tick,
            kinds: BTreeSet::from(["tests_change".to_owned()]),
            micros,
        }
    }

    /// With no record, the clock's microsecond.
    #[test]
    fn the_first_stamp_is_the_clock() {
        assert_eq!(stamp_after(NOON + 7, None, true), NOON + 7);
    }

    /// Records to the microsecond: an ordered record in the latest record's microsecond is
    /// stamped one microsecond later; a later clock is kept.
    #[test]
    fn an_ordered_stamp_in_the_latest_microsecond_is_one_microsecond_later() {
        let at = latest(6, NOON + 500_000, NOON + 500_000);
        assert_eq!(stamp_after(NOON + 500_000, Some(&at), true), NOON + 500_001);
        assert_eq!(stamp_after(NOON + 499_000, Some(&at), true), NOON + 500_001);
        assert_eq!(stamp_after(NOON + 900_000, Some(&at), true), NOON + 900_000);
    }

    /// A case holding a record in whole seconds is compared in whole seconds: an ordered record in
    /// the latest second is stamped at the start of the next one, at once.
    #[test]
    fn an_ordered_stamp_in_a_whole_second_case_is_the_next_second() {
        let at = latest(0, NOON / 1_000_000, NOON);
        assert_eq!(
            stamp_after(NOON + 400_000, Some(&at), true),
            NOON + 1_000_000
        );
        assert_eq!(
            stamp_after(NOON + 1_000_001, Some(&at), true),
            NOON + 1_000_001
        );
    }

    /// A record of a kind no order form reads with the latest is stamped no earlier than it, in
    /// its tick if the clock is behind.
    #[test]
    fn an_unordered_stamp_is_never_before_the_latest() {
        let at = latest(0, NOON / 1_000_000, NOON);
        assert_eq!(stamp_after(NOON - 5, Some(&at), false), NOON);
        assert_eq!(stamp_after(NOON + 3, Some(&at), false), NOON + 3);
    }

    /// Records in nanoseconds are compared to the microsecond once takt's stamp joins them.
    #[test]
    fn a_finer_latest_is_compared_at_the_microsecond() {
        let at = latest(6, NOON + 123_456, NOON + 123_457);
        assert_eq!(stamp_after(NOON + 123_456, Some(&at), true), NOON + 123_457);
        assert_eq!(
            stamp_after(NOON + 123_400, Some(&at), false),
            NOON + 123_457
        );
    }

    /// A floor past the last microsecond Canon writes (year 9999) is held at that microsecond, so
    /// a stamp never wraps to the epoch: the adversary's year-9999 cases.
    #[test]
    fn a_floor_past_the_last_instant_is_held_at_the_last_microsecond() {
        let last_second = super::LAST_MICROS / 1_000_000;
        let at = latest(0, last_second, last_second * 1_000_000);
        assert_eq!(stamp_after(NOON, Some(&at), true), super::LAST_MICROS);
        let at = latest(6, super::LAST_MICROS, super::LAST_MICROS + 1);
        assert_eq!(stamp_after(NOON, Some(&at), false), super::LAST_MICROS);
    }
}
