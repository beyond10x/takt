//! Validators: programs that turn an observation into evidence: `tests.run` and `gate.run`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, bail};

use crate::evidence::Record;

/// What a validator observed, and the records it wrote from it.
#[derive(Debug)]
pub struct Observation {
    /// One line on what was observed.
    pub summary: String,
    /// The evidence records the observation yields.
    pub records: Vec<Record>,
    /// What the validator retains beside its records, when it keeps something: the
    /// observation as its provider wrote it, kept under the case as `artifacts/<name>`.
    pub artifact: Option<Artifact>,
}

/// One file a validator retains with an observation.
#[derive(Clone)]
pub struct Artifact {
    /// The file name under the case's `artifacts/`.
    pub name: String,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Artifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Artifact")
            .field("name", &self.name)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// A program that performs one deterministic action and answers evidence.
pub trait Validator {
    /// The validator's name, the producer of its records.
    fn name(&self) -> &str;
    /// The protocol action it performs.
    fn action(&self) -> &str;
    /// The evidence kind it produces.
    fn kind(&self) -> &str;
    /// The artifact its records are about.
    fn subject(&self) -> &str;
    /// Performs the action in `tree` about `revision`; `seq` makes record ids unique.
    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation>;
}

/// `tests.run`: runs the test command without a shell; exit 0 is `pass`, anything else `fail`.
#[derive(Debug, Clone)]
pub struct TestsRun {
    /// The command and its arguments.
    pub command: Vec<String>,
}

impl TestsRun {
    /// Splits `command` at white space.
    pub fn new(command: &str) -> Result<Self> {
        let command: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        if command.is_empty() {
            bail!("the test command is empty");
        }
        Ok(Self { command })
    }
}

impl Validator for TestsRun {
    fn name(&self) -> &str {
        "takt.tests-run"
    }

    fn action(&self) -> &str {
        "tests.run"
    }

    fn kind(&self) -> &str {
        "test_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let output = Command::new(&self.command[0])
            .args(&self.command[1..])
            .current_dir(tree)
            .output()
            .with_context(|| format!("run {}", self.command.join(" ")))?;
        let result = if output.status.success() {
            "pass"
        } else {
            "fail"
        };
        let tail =
            String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
        let tail: String = tail
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        let record = Record::new(
            format!("test-result-{seq}"),
            self.kind(),
            Some(result.to_owned()),
            self.subject(),
            revision,
            self.name(),
        );
        Ok(Observation {
            summary: format!(
                "the test command exited {}: {result}; {}",
                output
                    .status
                    .code()
                    .map_or("signal".to_owned(), |c| c.to_string()),
                tail.chars().take(300).collect::<String>()
            ),
            records: vec![record],
            artifact: None,
        })
    }
}

/// The project file a gates document is evaluated with when the validator is given none and
/// the tree has it: `engineering-assertions-project/1`, at the path the assertions guide names.
pub const DEFAULT_PROJECT: &str = ".engineering/assertions-project.yaml";

/// `gate.run`: evaluates the repository's `engineering-gates/1` document through the
/// engineering assertions library, as `canon-engineering gates run` does. Every gate true is
/// `pass`, any gate false is `fail`; an unknown evaluation writes no record, because a claim
/// that could not be observed stays unknown; an invalid document or provider datum is an error.
/// The project file admits the commands and providers the gates bind. Every observation
/// retains the bundle the library evaluated, `engineering-gate-evidence/1`, as
/// `<record-id>.bundle.json`.
#[derive(Debug, Clone)]
pub struct GateRun {
    /// The gates document, relative to the tree.
    pub gates: PathBuf,
    /// The `engineering-assertions-project/1` file that admits the gates' commands and
    /// providers, relative to the tree; `None` takes [`DEFAULT_PROJECT`] when the tree has it.
    pub project: Option<PathBuf>,
}

impl GateRun {
    /// Over the gates document at `gates`, relative to the tree.
    #[must_use]
    pub fn new(gates: impl Into<PathBuf>) -> Self {
        Self {
            gates: gates.into(),
            project: None,
        }
    }

    /// With the project file at `project`, relative to the tree.
    #[must_use]
    pub fn with_project(mut self, project: impl Into<PathBuf>) -> Self {
        self.project = Some(project.into());
        self
    }
}

impl Validator for GateRun {
    fn name(&self) -> &str {
        "takt.gate-run"
    }

    fn action(&self) -> &str {
        "gate.run"
    }

    fn kind(&self) -> &str {
        "gate_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let document = self.gates.display();
        let project = self.project.clone().or_else(|| {
            tree.join(DEFAULT_PROJECT)
                .is_file()
                .then(|| PathBuf::from(DEFAULT_PROJECT))
        });
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        // The case directory, which the loop writes every pass, stays out of the bundle's
        // source identity: that identity is the repository tree's.
        let cases = Path::new(crate::case::CASES_DIR);
        let prepared = canon_engineering_assertions::prepare(
            tree,
            &self.gates,
            project.as_deref(),
            Some(cases),
        )
        .with_context(|| match &project {
            Some(project) => format!(
                "prepare the gates document {document} with the project file {}",
                project.display()
            ),
            None => format!("prepare the gates document {document}"),
        })?;
        let bundle = canon_engineering_assertions::collect(prepared, now)
            .with_context(|| format!("collect the observations of {document}"))?;
        let report = canon_engineering_assertions::evaluate(&bundle, now, None, None)
            .with_context(|| format!("evaluate {document}"))?;
        // The report holds the gates' assertions in declaration order; the gate that decided
        // is the first whose assertion shares the report's truth.
        let decided = bundle
            .document
            .gates
            .iter()
            .flat_map(|gate| std::iter::repeat_n(gate.id.as_str(), gate.assertions.len()))
            .zip(&report.assertions)
            .find(|(_, evaluation)| evaluation.truth == report.truth)
            .map(|(gate, evaluation)| {
                (
                    gate,
                    evaluation.expression.as_str(),
                    evaluation.reasons.join("; "),
                )
            });
        let decided = |truth: &str| match &decided {
            Some((gate, expression, reasons)) if reasons.is_empty() => {
                format!("gate {gate} {truth}: {expression}")
            }
            Some((gate, expression, reasons)) => format!(
                "gate {gate} {truth}: {expression} ({})",
                reasons.chars().take(300).collect::<String>()
            ),
            None => format!("no assertion is {truth}"),
        };
        let artifact = format!("gate-result-{seq}.bundle.json");
        let (result, summary) = match report.truth {
            canon_expr::Truth::True => {
                let gates: Vec<&str> = bundle
                    .document
                    .gates
                    .iter()
                    .map(|gate| gate.id.as_str())
                    .collect();
                (
                    Some("pass"),
                    format!(
                        "the gates document {document}: pass; every gate true: {}; retained as {artifact}",
                        gates.join(", ")
                    ),
                )
            }
            canon_expr::Truth::False => (
                Some("fail"),
                format!(
                    "the gates document {document}: fail; {}; retained as {artifact}",
                    decided("false")
                ),
            ),
            canon_expr::Truth::Unknown => (
                None,
                format!(
                    "the gates document {document}: unknown; {}; no record, the claim stays unknown; retained as {artifact}",
                    decided("unknown")
                ),
            ),
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("gate-result-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        let bytes = serde_json::to_vec_pretty(&bundle).context("serialize the evaluated bundle")?;
        Ok(Observation {
            summary,
            records,
            artifact: Some(Artifact {
                name: artifact,
                bytes,
            }),
        })
    }
}
