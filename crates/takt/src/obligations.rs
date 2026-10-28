//! The repository's obligations: each gate of its gates document, read from the case's base
//! revision, with its last status at the case's implementation revision, the revision the gate
//! judges: takt's store commits above it (the `test_result` record, the move, an approval) do not
//! move it, a code commit does. The prompt lists them ([`block`]), the run's last line names the
//! ones not met ([`summary`]) and `takt report` prints the table. A status comes from a gate
//! observation ([`crate::validator::GateRun`]) that the record holds as an `obligations` line; a
//! gate no observation has judged at the implementation revision is `not yet run`, never a
//! status an earlier revision earned.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

/// Where a gate stands at a revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Observed, and neither true nor false: an assertion or the gate's `applies_when` could not
    /// be answered.
    Open,
    /// Every assertion true.
    Pass,
    /// An assertion false.
    Fail,
    /// The gate's `applies_when` is false for this change.
    NotApplicable,
    /// No observation of the gate at this revision.
    NotYetRun,
}

impl Status {
    /// The status as the prompt, the last line and the report print it.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Pass => "pass",
            Status::Fail => "fail",
            Status::NotApplicable => "not applicable",
            Status::NotYetRun => "not yet run",
        }
    }

    /// Whether the obligation needs nothing more: it passed or does not apply.
    #[must_use]
    pub fn met(self) -> bool {
        matches!(self, Status::Pass | Status::NotApplicable)
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.text())
    }
}

/// One gate of the repository's gates document, with its status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Obligation {
    /// The gate's id.
    pub id: String,
    /// The gate's text, one line.
    pub text: String,
    /// Where the gate stands.
    pub status: Status,
}

/// The `event` the record names an observation's table by.
pub const EVENT: &str = "obligations";

/// The gates of the gates document `gates` as the commit `base` holds it: id and text, in
/// document order. Nothing is read from the tree, so a change that edits the document cannot
/// remove an obligation. A base without the document, or one that is not a gates document,
/// holds none.
///
/// # Errors
/// A git read that fails.
pub fn gates_at(tree: &Path, base: &str, gates: &Path) -> Result<Vec<(String, String)>> {
    let Some(text) = crate::validator::file_at(tree, base, gates)? else {
        return Ok(Vec::new());
    };
    let document: serde_yaml_ng::Value = match serde_yaml_ng::from_str(&text) {
        Ok(document) => document,
        Err(_) => return Ok(Vec::new()),
    };
    let one_line = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(document["gates"]
        .as_sequence()
        .map(|gates| {
            gates
                .iter()
                .filter_map(|gate| {
                    Some((
                        gate["id"].as_str()?.to_owned(),
                        one_line(gate["text"].as_str().unwrap_or_default()),
                    ))
                })
                .collect()
        })
        .unwrap_or_default())
}

/// The last table `record` (the case's record so far) holds: its revision and its rows; none
/// when no observation wrote one.
#[must_use]
pub fn recorded(record: &[serde_json::Value]) -> Option<(String, Vec<Obligation>)> {
    let line = record.iter().rev().find(|line| line["event"] == EVENT)?;
    let revision = line["revision"].as_str()?.to_owned();
    let rows = serde_json::from_value(line["obligations"].clone()).ok()?;
    Some((revision, rows))
}

/// The last table `record` holds at `revision`; none when no observation wrote one there. A
/// table recorded later at another revision (a stop's, at takt's store commits above it) does not
/// hide it.
#[must_use]
pub fn recorded_at(record: &[serde_json::Value], revision: &str) -> Option<Vec<Obligation>> {
    let line = record
        .iter()
        .rev()
        .find(|line| line["event"] == EVENT && line["revision"] == revision)?;
    serde_json::from_value(line["obligations"].clone()).ok()
}

/// The obligations of the case at `revision`, the case's implementation revision: each gate of
/// the document `gates` at `base`, with the status the last table recorded at `revision` gave
/// it, else `not yet run`.
///
/// # Errors
/// A git read that fails.
pub fn current(
    tree: &Path,
    base: &str,
    gates: &Path,
    record: &[serde_json::Value],
    revision: &str,
) -> Result<Vec<Obligation>> {
    let seen = recorded_at(record, revision);
    Ok(gates_at(tree, base, gates)
        .with_context(|| format!("read the gates document {} at {base}", gates.display()))?
        .into_iter()
        .map(|(id, text)| {
            let status = seen
                .as_ref()
                .and_then(|rows| rows.iter().find(|row| row.id == id))
                .map_or(Status::NotYetRun, |row| row.status);
            Obligation { id, text, status }
        })
        .collect())
}

/// The obligations of `run`'s case, given the case's record so far (`record`), and the revision
/// they are read at: the case's revision of the artifact the gate judges, its implementation
/// revision, which the gate's observation records its table at; the tree's `HEAD` for a case
/// that holds no revision of it. None for a run with no gates document, or a case that pins no
/// base commit.
///
/// # Errors
/// A git read that fails.
pub fn of_run(
    run: &crate::Run<'_>,
    record: &[serde_json::Value],
) -> Result<Option<(String, Vec<Obligation>)>> {
    let Some((gates, subject)) = run
        .validators
        .iter()
        .find_map(|v| v.gates_document().map(|gates| (gates, v.subject())))
    else {
        return Ok(None);
    };
    let Some(base) = crate::case::Branch::read(&run.case.dir)?.base_commit else {
        return Ok(None);
    };
    let revision = match run.case.revision(subject) {
        Some(revision) => revision,
        None => crate::git::head(&run.tree)?,
    };
    let obligations = current(&run.tree, &base, gates, record, &revision)?;
    Ok(Some((revision, obligations)))
}

/// The prompt's block of repository obligations: a heading, then a line per gate with its text
/// and status; nothing for a repository with none.
#[must_use]
pub fn block(obligations: &[Obligation]) -> String {
    if obligations.is_empty() {
        return String::new();
    }
    let mut text = String::from(
        "\nRepository obligations (each gate of the repository's gates document, its status at this revision):\n",
    );
    for obligation in obligations {
        text.push_str(&format!(
            "- {}: {} [{}]\n",
            obligation.id, obligation.text, obligation.status
        ));
    }
    text
}

/// The run's last line about the obligations: how many are not met and which, each with its
/// status; `handed over` first when the run hands its change over as a candidate. None when every
/// obligation is met, or there are none.
#[must_use]
pub fn summary(obligations: &[Obligation], handed_over: bool) -> Option<String> {
    let unmet: Vec<&Obligation> = obligations.iter().filter(|o| !o.status.met()).collect();
    if unmet.is_empty() {
        return None;
    }
    let named: Vec<String> = unmet
        .iter()
        .map(|obligation| format!("{} {}", obligation.id, obligation.status))
        .collect();
    Some(format!(
        "{}{} obligation{} open: {}",
        if handed_over { "handed over, " } else { "" },
        unmet.len(),
        if unmet.len() == 1 { "" } else { "s" },
        named.join(", ")
    ))
}

/// The table `takt report` prints for one run: a markdown table of id, status and text.
#[must_use]
pub fn table(obligations: &[Obligation]) -> String {
    let mut text = String::from("| obligation | status | text |\n|---|---|---|\n");
    for obligation in obligations {
        text.push_str(&format!(
            "| {} | {} | {} |\n",
            crate::trace::escape(&obligation.id),
            obligation.status,
            crate::trace::escape(&obligation.text).replace('|', "\\|")
        ));
    }
    text
}

/// The status a gate's result in the assertions library maps to.
///
/// # Errors
/// A result this takt does not know.
pub fn status_of(result: &str) -> Result<Status> {
    match result {
        "pass" => Ok(Status::Pass),
        "fail" => Ok(Status::Fail),
        "unknown" => Ok(Status::Open),
        "not_applicable" => Ok(Status::NotApplicable),
        other => Err(anyhow!("a gate result takt does not know: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, status: Status) -> Obligation {
        Obligation {
            id: id.to_owned(),
            text: format!("The gate {id} holds."),
            status,
        }
    }

    #[test]
    fn a_status_prints_as_its_words() {
        let words: Vec<&str> = [
            Status::Open,
            Status::Pass,
            Status::Fail,
            Status::NotApplicable,
            Status::NotYetRun,
        ]
        .iter()
        .map(|status| status.text())
        .collect();
        assert_eq!(
            words,
            ["open", "pass", "fail", "not applicable", "not yet run"]
        );
    }

    #[test]
    fn only_pass_and_not_applicable_are_met() {
        assert!(Status::Pass.met());
        assert!(Status::NotApplicable.met());
        assert!(!Status::Open.met());
        assert!(!Status::Fail.met());
        assert!(!Status::NotYetRun.met());
    }

    #[test]
    fn the_summary_counts_and_names_what_is_not_met() {
        let rows = [
            row("manifest", Status::Pass),
            row("release-notes", Status::Fail),
            row("docs", Status::NotApplicable),
        ];
        assert_eq!(
            summary(&rows, true).as_deref(),
            Some("handed over, 1 obligation open: release-notes fail")
        );
        assert_eq!(
            summary(&rows, false).as_deref(),
            Some("1 obligation open: release-notes fail")
        );
        let two = [row("a", Status::NotYetRun), row("b", Status::Open)];
        assert_eq!(
            summary(&two, false).as_deref(),
            Some("2 obligations open: a not yet run, b open")
        );
        assert_eq!(summary(&[row("a", Status::Pass)], true), None);
        assert_eq!(summary(&[], true), None);
    }

    #[test]
    fn the_block_lists_each_gate_with_its_text_and_status() {
        let rows = [row("a", Status::Fail)];
        let text = block(&rows);
        assert!(text.contains("- a: The gate a holds. [fail]"), "{text}");
        assert_eq!(block(&[]), "");
    }

    #[test]
    fn a_table_of_another_revision_is_not_yet_run() {
        let record = vec![serde_json::json!({
            "event": "obligations",
            "revision": "r1",
            "obligations": [{"id": "a", "text": "A.", "status": "pass"}]
        })];
        let (at, rows) = recorded(&record).expect("a table");
        assert_eq!((at.as_str(), rows.len()), ("r1", 1));
        assert_eq!(rows[0].status, Status::Pass);
    }

    #[test]
    fn a_later_table_at_another_revision_does_not_hide_the_gates_table() {
        let table = |revision: &str, status: &str| {
            serde_json::json!({
                "event": "obligations",
                "revision": revision,
                "obligations": [{"id": "a", "text": "A.", "status": status}]
            })
        };
        // The gate's table at the implementation revision, then a stop's at takt's store
        // commits above it, as takt before this story recorded it.
        let record = vec![table("r1", "pass"), table("r2", "not_yet_run")];
        let rows = recorded_at(&record, "r1").expect("the gate's table");
        assert_eq!(rows[0].status, Status::Pass);
        assert_eq!(
            recorded_at(&record, "r2").expect("the stop's table")[0].status,
            Status::NotYetRun
        );
        assert_eq!(recorded_at(&record, "r3"), None);
        let later = vec![table("r1", "pass"), table("r1", "fail")];
        assert_eq!(
            recorded_at(&later, "r1").expect("the last table")[0].status,
            Status::Fail
        );
    }

    #[test]
    fn a_gate_result_maps_to_a_status() {
        assert_eq!(status_of("unknown").expect("known"), Status::Open);
        assert_eq!(
            status_of("not_applicable").expect("known"),
            Status::NotApplicable
        );
        assert!(status_of("maybe").is_err());
    }
}
