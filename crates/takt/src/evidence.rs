//! `canon-evidence/1` records, written by validators only.

use serde::Serialize;

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
            format: "canon-evidence/1",
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
}
