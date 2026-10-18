//! Validators: programs that turn an observation into evidence: `tests.run`, `gate.run`,
//! `pr.open` and `ci.read`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{Context as _, Result, anyhow, bail};

use crate::case::{Branch, current};
use crate::evidence::Record;
use crate::forge::{Checks, Forge, PullRequest, PullRequestState};

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

/// The first eight characters of a revision, as the loop prints one.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// `pr.open`: pushes the case's branch through the forge and opens its one pull request, the
/// head at the implementation revision; writes one `pull_request` record with the state the
/// forge reports, `open` for a pull request just opened. At a later revision the branch is
/// pushed again and the pull request's state read: one pull request per case. The tree's
/// `HEAD` must be the revision, or the pull request's head would not be what the claim is
/// about. The title is the intent's first line, the body the intent and the claims that hold;
/// the forge passes both through `scan-text` before they leave the machine.
pub struct PrOpen {
    /// The forge the push and the pull request go through.
    pub forge: Rc<dyn Forge>,
}

impl PrOpen {
    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self { forge }
    }
}

impl std::fmt::Debug for PrOpen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrOpen")
            .field("forge", &self.forge.name())
            .finish()
    }
}

/// The pull request's title: the intent's first line.
fn title_of(intent: &str) -> String {
    intent
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("takt")
        .chars()
        .take(120)
        .collect()
}

/// The claims the case's latest decision holds, in id order; none before a decision.
fn claims_held(case_dir: &Path) -> Result<Vec<String>> {
    let decisions = case_dir.join("decisions");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&decisions)
        .with_context(|| format!("read {}", decisions.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let Some(latest) = paths.last() else {
        return Ok(Vec::new());
    };
    let json =
        std::fs::read_to_string(latest).with_context(|| format!("read {}", latest.display()))?;
    let decision = crate::Decision::from_json(&json)?;
    Ok(decision
        .claims
        .iter()
        .filter(|(_, value)| value.as_str() == "true")
        .map(|(id, _)| id.clone())
        .collect())
}

/// The pull request's body: the intent, then the case, the revision and the claims that hold.
fn body_of(case_dir: &Path, intent: &str, revision: &str) -> Result<String> {
    let case = case_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let claims = claims_held(case_dir)?;
    let claims = if claims.is_empty() {
        "none yet".to_owned()
    } else {
        claims.join(", ")
    };
    Ok(format!(
        "{}\n\n---\n\nOpened by takt for the case `{case}` at `{revision}`. Claims that hold: {claims}.\n",
        intent.trim()
    ))
}

impl Validator for PrOpen {
    fn name(&self) -> &str {
        "takt.pr-open"
    }

    fn action(&self) -> &str {
        "pr.open"
    }

    fn kind(&self) -> &str {
        "pull_request"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let head = crate::git::head(tree)?;
        if head != revision {
            bail!(
                "the tree's HEAD {head} is not the implementation revision {revision}; the pull request's head would not be the revision the claim is about"
            );
        }
        let dir = current(tree)?;
        let mut branch = Branch::read(&dir)?;
        let forge = self.forge.name();
        self.forge
            .push(tree, &branch.name)
            .with_context(|| format!("push {} through the forge {forge}", branch.name))?;
        branch.pushed = Some(revision.to_owned());
        let (state, summary) = match branch.pull_request {
            Some(number) => {
                let state = self.forge.pull_request(tree, number).with_context(|| {
                    format!("read pull request #{number} through the forge {forge}")
                })?;
                (
                    state,
                    format!(
                        "pushed {} at {} through {forge}; pull request #{number} is {state}",
                        branch.name,
                        short(revision)
                    ),
                )
            }
            None => {
                let base = branch.base.clone().ok_or_else(|| {
                    anyhow!(
                        "the case has no base branch: it was opened from a detached HEAD or over a case branch left behind; open it from the branch its pull request is against"
                    )
                })?;
                let intent = dir.join("intent.md");
                let intent = std::fs::read_to_string(&intent)
                    .with_context(|| format!("read {}", intent.display()))?;
                let request = PullRequest {
                    branch: branch.name.clone(),
                    base,
                    title: title_of(&intent),
                    body: body_of(&dir, &intent, revision)?,
                };
                let number = self
                    .forge
                    .open_pull_request(tree, &dir.join("artifacts"), &request)
                    .with_context(|| {
                        format!(
                            "open the pull request for {} through the forge {forge}",
                            branch.name
                        )
                    })?;
                branch.pull_request = Some(number);
                (
                    PullRequestState::Open,
                    format!(
                        "pushed {} at {} through {forge} and opened pull request #{number}: open",
                        branch.name,
                        short(revision)
                    ),
                )
            }
        };
        branch.write(&dir)?;
        let record = Record::new(
            format!("pull-request-{seq}"),
            self.kind(),
            Some(state.as_str().to_owned()),
            self.subject(),
            revision,
            self.name(),
        );
        Ok(Observation {
            summary,
            records: vec![record],
            artifact: None,
        })
    }
}

/// `ci.read`: reads the checks on the head of the case's pull request through the forge and
/// writes one `ci_result` record, `pass` or `fail`. Pending checks write no record, because a
/// claim that could not be observed stays unknown; so does a case with no pull request yet, or
/// one whose pull request head is not the revision. Nothing but the pull request head is read.
pub struct CiRead {
    /// The forge the checks are read through.
    pub forge: Rc<dyn Forge>,
}

impl CiRead {
    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self { forge }
    }
}

impl std::fmt::Debug for CiRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CiRead")
            .field("forge", &self.forge.name())
            .finish()
    }
}

impl Validator for CiRead {
    fn name(&self) -> &str {
        "takt.ci-read"
    }

    fn action(&self) -> &str {
        "ci.read"
    }

    fn kind(&self) -> &str {
        "ci_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let dir = current(tree)?;
        let branch = Branch::read(&dir)?;
        let nothing = |summary: String| Observation {
            summary,
            records: Vec::new(),
            artifact: None,
        };
        let Some(number) = branch.pull_request else {
            return Ok(nothing(
                "no pull request is open for the case; nothing to read, the claim stays unknown"
                    .to_owned(),
            ));
        };
        if branch.pushed.as_deref() != Some(revision) {
            return Ok(nothing(format!(
                "the head of pull request #{number} is {}, not the implementation revision {}; nothing to read, the claim stays unknown",
                branch.pushed.as_deref().map_or("not pushed", short),
                short(revision)
            )));
        }
        let forge = self.forge.name();
        let checks = self.forge.checks(tree, number).with_context(|| {
            format!("read the checks on pull request #{number} through the forge {forge}")
        })?;
        let result = match checks {
            Checks::Pass => Some("pass"),
            Checks::Fail => Some("fail"),
            Checks::Pending => None,
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("ci-result-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        let summary = match result {
            Some(result) => format!(
                "checks on pull request #{number} at {} through {forge}: {result}",
                short(revision)
            ),
            None => format!(
                "checks on pull request #{number} at {} through {forge}: pending; no record, the claim stays unknown",
                short(revision)
            ),
        };
        Ok(Observation {
            summary,
            records,
            artifact: None,
        })
    }
}
