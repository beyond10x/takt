//! Validators: programs that turn an observation into evidence: `tests.run`, `gate.run`,
//! `pr.open`, `ci.read` and `repository.merge`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{Context as _, Result, anyhow, bail};

use crate::case::{BRANCH_FILE, Branch, current};
use crate::evidence::Record;
use crate::forge::{BotRoute, Checks, Forge, Limits, PullRequest, PullRequestState};

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
    /// The files under the case's directory the validator itself writes while it observes,
    /// relative to it. Nothing else under the cases directory may change during an
    /// observation: the loop puts back whatever else changed and refuses the observation,
    /// because what a validator runs (a test command, a gate's commands) may be code a model
    /// wrote. None by default.
    fn writes(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// `tests.run`: runs the test command without a shell; exit 0 is `pass`, anything else `fail`.
/// The command is code from the tree: it gets no credential, no standard input, a timeout past
/// which it is killed with what it started, and only a bounded tail of its output is kept.
#[derive(Debug, Clone)]
pub struct TestsRun {
    /// The command and its arguments.
    pub command: Vec<String>,
    /// What the command is held to.
    pub limits: Limits,
}

impl TestsRun {
    /// Splits `command` at white space.
    pub fn new(command: &str) -> Result<Self> {
        let command: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        if command.is_empty() {
            bail!("the test command is empty");
        }
        Ok(Self {
            command,
            limits: Self::LIMITS,
        })
    }

    /// The limits a test command is held to by default: thirty minutes, 64 KiB of output.
    pub const LIMITS: Limits = Limits {
        timeout: std::time::Duration::from_secs(1800),
        output: 64 << 10,
    };

    /// Held to `limits`.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
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
        let mut command = Command::new(&self.command[0]);
        command.args(&self.command[1..]).current_dir(tree);
        crate::forge::without_credentials(&mut command, |_| false);
        let output = crate::forge::run_child(&mut command, self.limits)
            .with_context(|| format!("run {}", self.command.join(" ")))?;
        let result = if output.success() { "pass" } else { "fail" };
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
                output.code.map_or("signal".to_owned(), |c| c.to_string()),
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
///
/// The gates document and the project file are read from the case's base revision, the tip of
/// the branch in `branch.yaml`, never from the tree: the tree is the case's branch, which a
/// model edits, and a gate it rewrote would decide its own verdict. What the gates observe is
/// the tree. The two files are copied for the evaluation into the case's directory, which the
/// tree's source identity leaves out, and removed again before the observation ends.
#[derive(Debug, Clone)]
pub struct GateRun {
    /// The gates document, relative to the tree.
    pub gates: PathBuf,
    /// The `engineering-assertions-project/1` file that admits the gates' commands and
    /// providers, relative to the tree; `None` takes [`DEFAULT_PROJECT`] when the base
    /// revision has it.
    pub project: Option<PathBuf>,
}

/// The directory under the case's that holds the base revision's gate inputs while the gate
/// validator evaluates them.
const GATE_INPUTS: &str = "gate-inputs";

/// The case's base revision: the commit pinned in `branch.yaml` when the case opened, shown
/// with the base branch's name, and the commit. `None` for a case that pins none (one opened
/// before the pin was recorded): no branch name stands in for it, because a test command can
/// move any branch, even onto `HEAD`, without touching the tree.
fn base_revision(tree: &Path, case_dir: &Path) -> Result<Option<(String, String)>> {
    let branch = Branch::read(case_dir)?;
    let shown = branch.base.as_deref().unwrap_or("a detached HEAD");
    let Some(pinned) = &branch.base_commit else {
        return Ok(None);
    };
    let at = crate::git::run(
        tree,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("{pinned}^{{commit}}"),
        ],
    )
    .with_context(|| format!("resolve the case's base commit {pinned}"))?;
    Ok(Some((
        format!("{shown} at {}, pinned at open", short(&at)),
        at,
    )))
}

/// The files a project file names that decide what a gate runs: the executables of the
/// commands and providers it admits, and the catalogs it extends the functions with; those
/// relative to the tree, which a model can edit, as they are named.
fn admitted_files(project: &str) -> Result<Vec<String>> {
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(project).context("read the project file")?;
    let mut paths = Vec::new();
    let named = |value: &serde_yaml_ng::Value| value.as_str().map(str::to_owned);
    if let Some(commands) = doc["commands"].as_mapping() {
        paths.extend(commands.values().filter_map(|c| named(&c["executable"])));
    }
    if let Some(providers) = doc["providers"].as_mapping() {
        paths.extend(
            providers
                .values()
                .filter_map(|p| named(&p["command"]["executable"])),
        );
    }
    if let Some(catalogs) = doc["catalogs"].as_sequence() {
        paths.extend(catalogs.iter().filter_map(named));
    }
    // Relative to the tree and normalised; one that leads out of the tree is not the tree's.
    let mut inside: Vec<String> = paths
        .iter()
        .filter(|path| Path::new(path).is_relative())
        .filter_map(|path| {
            let mut parts: Vec<&str> = Vec::new();
            for part in path.split('/') {
                match part {
                    "" | "." => {}
                    ".." => {
                        parts.pop()?;
                    }
                    part => parts.push(part),
                }
            }
            (!parts.is_empty()).then(|| parts.join("/"))
        })
        .collect();
    inside.sort();
    inside.dedup();
    Ok(inside)
}

/// What git stores at `path` in the commit `at`: its mode and object, or `None` when the commit
/// has no such path.
fn object_at(tree: &Path, at: &str, path: &str) -> Result<Option<String>> {
    let listed = crate::git::run(tree, &["ls-tree", "--full-tree", at, "--", path])?;
    Ok(listed
        .split('\t')
        .next()
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned))
}

/// The text of `path`, relative to the tree, at the commit `at`; `None` when it has no such
/// file.
fn file_at(tree: &Path, at: &str, path: &Path) -> Result<Option<String>> {
    let relative = path
        .to_str()
        .filter(|path| path.split('/').all(|part| part != ".."))
        .filter(|_| path.is_relative())
        .ok_or_else(|| anyhow!("{} is not a path inside the tree", path.display()))?;
    let spec = format!("{at}:./{relative}");
    if crate::git::run(tree, &["cat-file", "-e", &spec]).is_err() {
        return Ok(None);
    }
    crate::git::run(tree, &["show", &spec]).map(|text| Some(format!("{text}\n")))
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
        let dir = current(tree)?;
        let Some((base, at)) = base_revision(tree, &dir)? else {
            return Ok(Observation {
                summary: format!(
                    "the gates document {}: not run; the case pins no base commit ({BRANCH_FILE} has no base_commit; it was opened before takt recorded one), and the gate is read from nothing else; no record, the claim stays unknown; abandon the case and open it again",
                    self.gates.display()
                ),
                records: Vec::new(),
                artifact: None,
            });
        };
        let document = format!("{} from {base}", self.gates.display());
        let gates = file_at(tree, &at, &self.gates)?
            .ok_or_else(|| anyhow!("the gates document {document} is not in the base revision"))?;
        let project = match &self.project {
            Some(path) => Some((
                path.clone(),
                file_at(tree, &at, path)?.ok_or_else(|| {
                    anyhow!(
                        "the project file {} is not in the base revision, {base}",
                        path.display()
                    )
                })?,
            )),
            None => file_at(tree, &at, Path::new(DEFAULT_PROJECT))?
                .map(|text| (PathBuf::from(DEFAULT_PROJECT), text)),
        };
        // What the project file admits runs from the tree: an executable or a catalog the case's
        // branch changed would let a model's edit decide the gate the base revision defines.
        // Nothing is run and no record written while one differs from the base revision.
        if let Some((path, text)) = &project {
            let mut changed = Vec::new();
            for file in admitted_files(text)
                .with_context(|| format!("read the project file {}", path.display()))?
            {
                if object_at(tree, &at, &file)? != object_at(tree, revision, &file)? {
                    changed.push(file);
                }
            }
            if !changed.is_empty() {
                return Ok(Observation {
                    summary: format!(
                        "the gates document {document}: not run; what the project file {} admits differs from the base revision at {}: {}; no record, the claim stays unknown",
                        path.display(),
                        short(revision),
                        changed.join(", ")
                    ),
                    records: Vec::new(),
                    artifact: None,
                });
            }
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        // The case directory, which the loop writes every pass, stays out of the bundle's
        // source identity: that identity is the repository tree's. The base revision's two
        // files are evaluated from there, the paths relative to the tree as the library takes
        // them, and are gone again before the observation ends.
        let cases = Path::new(crate::case::CASES_DIR);
        let inputs = dir.join(GATE_INPUTS);
        let relative = inputs
            .strip_prefix(tree)
            .with_context(|| format!("{} is not under the tree", inputs.display()))?
            .to_path_buf();
        let copy = |kind: &str, path: &Path, text: &str| -> Result<PathBuf> {
            let name = path
                .file_name()
                .ok_or_else(|| anyhow!("{} names no file", path.display()))?;
            let at = inputs.join(kind);
            std::fs::create_dir_all(&at).with_context(|| format!("create {}", at.display()))?;
            std::fs::write(at.join(name), text)
                .with_context(|| format!("write {}", at.join(name).display()))?;
            Ok(relative.join(kind).join(name))
        };
        let clear = || match std::fs::remove_dir_all(&inputs) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(error).with_context(|| format!("remove {}", inputs.display()))
            }
            _ => Ok(()),
        };
        clear()?;
        let collected = (|| -> Result<_> {
            let gates = copy("gates", &self.gates, &gates)?;
            let copied = project
                .as_ref()
                .map(|(path, text)| copy("project", path, text))
                .transpose()?;
            let prepared = canon_engineering_assertions::prepare(
                tree,
                &gates,
                copied.as_deref(),
                Some(cases),
            )
            .with_context(|| match &project {
                Some((project, _)) => format!(
                    "prepare the gates document {document} with the project file {} from {base}",
                    project.display()
                ),
                None => format!("prepare the gates document {document}"),
            })?;
            canon_engineering_assertions::collect(prepared, now)
                .with_context(|| format!("collect the observations of {document}"))
        })();
        let cleared = clear();
        let bundle = collected?;
        cleared?;
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

/// Whether the pull request head `head` carries the implementation revision `revision` of the
/// case at `case_dir`: it is the revision, or it reaches the revision only through the
/// approval's own commits ([`crate::case::approvals_only`]), which change nothing but the
/// planning store and are how an approval's record reaches the base with the pull request. A
/// head that is not a commit of the repository carries nothing.
fn carries(tree: &Path, case_dir: &Path, revision: &str, head: &str) -> Result<bool> {
    if head == revision {
        return Ok(true);
    }
    if head.len() < 7
        || !head.bytes().all(|byte| byte.is_ascii_hexdigit())
        || crate::git::run(tree, &["cat-file", "-e", &format!("{head}^{{commit}}")]).is_err()
    {
        return Ok(false);
    }
    crate::case::approvals_only(
        tree,
        revision,
        head,
        crate::case::story_of(case_dir)?.as_deref(),
    )
}

/// How a summary names a head that carries the approval's commits above the revision; nothing
/// for a head that is the revision.
fn above(revision: &str, head: &str) -> String {
    if head == revision {
        String::new()
    } else {
        format!(
            "; its head {} carries the approval's record above it",
            short(head)
        )
    }
}

/// `pr.open`: pushes the case's branch through the forge and opens its one pull request, the
/// head at the implementation revision; writes one `pull_request` record with the state the
/// forge reports, `open` for a pull request just opened. At a later revision the branch is
/// pushed again and the pull request's state read: one pull request per case. The tree's
/// `HEAD` must be the revision, or the revision with only the approval's own commits above it
/// ([`crate::case::approvals_only`]), which are pushed with it so the record of the grant
/// reaches the base with the pull request; anything else would not be what the claim is
/// about. A number the case directory carries is trusted only once the forge reports the pull
/// request's head as the case's branch at the head pushed, else no record is written. A case
/// with no base is refused before anything is pushed. The title is the intent's first line,
/// the body the intent and the claims that hold; the forge passes both through `scan-text`
/// before they leave the machine.
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
    Ok(crate::case::latest_decision(case_dir)?
        .map(|decision| {
            decision
                .claims
                .iter()
                .filter(|(_, value)| value.as_str() == "true")
                .map(|(id, _)| id.clone())
                .collect()
        })
        .unwrap_or_default())
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

/// What `pr.open` does after the push: opens the case's pull request, or reads the one it has.
enum Next {
    Open(PullRequest),
    Read(u64),
}

/// The case's branch, from the case directory's name: what the validators push and open for,
/// never the `name` a file under the case directory carries, which a harness can write.
fn branch_of(case_dir: &Path) -> Result<String> {
    let id = case_dir
        .file_name()
        .ok_or_else(|| anyhow!("the case directory {} has no name", case_dir.display()))?;
    Ok(Branch::name_of(&id.to_string_lossy()))
}

/// The pull request a case opens: `name` against `base`, the intent's first line as the
/// title, the intent and the claims that hold as the body. A case with no base is refused.
fn request_of(
    case_dir: &Path,
    name: &str,
    base: Option<&str>,
    revision: &str,
) -> Result<PullRequest> {
    let base = base.ok_or_else(|| {
        anyhow!(
            "the case has no base branch: it was opened from a detached HEAD; open it from the branch its pull request is against"
        )
    })?;
    let intent = case_dir.join("intent.md");
    let intent =
        std::fs::read_to_string(&intent).with_context(|| format!("read {}", intent.display()))?;
    Ok(PullRequest {
        branch: name.to_owned(),
        base: base.to_owned(),
        title: title_of(&intent),
        body: body_of(case_dir, &intent, revision)?,
    })
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

    /// The case's `branch.yaml`, and what the bot route keeps of the request it sent and the
    /// response it read.
    fn writes(&self) -> Vec<PathBuf> {
        let artifacts = Path::new("artifacts");
        vec![
            PathBuf::from(BRANCH_FILE),
            artifacts.join(BotRoute::REQUEST),
            artifacts.join(BotRoute::RESPONSE),
        ]
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let dir = current(tree)?;
        // `HEAD` is the implementation revision, or the approval's own commits on top of it
        // (`approvals_only`); `HEAD` is what is pushed, so the pull request carries the
        // approval's record to the base, and the record written is about the revision.
        let head = crate::git::head(tree)?;
        if head != revision
            && !crate::case::approvals_only(
                tree,
                revision,
                &head,
                crate::case::story_of(&dir)?.as_deref(),
            )?
        {
            bail!(
                "the tree's HEAD {head} is not the implementation revision {revision}; the pull request's head would not be the revision the claim is about"
            );
        }
        let mut branch = Branch::read(&dir)?;
        let forge = self.forge.name();
        // The branch pushed and opened for is the case's, from the case id: the `name` in
        // `branch.yaml` is in a file a harness can write, and a refspec there must never reach
        // the forge. A `name` that differs is set right, and the summary says so.
        let name = branch_of(&dir)?;
        let repaired = if branch.name == name {
            String::new()
        } else {
            let was = std::mem::replace(&mut branch.name, name.clone());
            format!("{BRANCH_FILE} named {was}, not the case's branch; set right; ")
        };
        // What follows the push is settled before it: a case with no pull request yet needs its
        // base and its request, and one without a base is refused with nothing pushed.
        let next = match branch.pull_request {
            Some(number) => Next::Read(number),
            None => Next::Open(request_of(&dir, &name, branch.base.as_deref(), revision)?),
        };
        // A pull request needs a commit on the case's branch beyond its base: at the base itself
        // the forge has nothing to compare, and opening one would be refused there. The take
        // committed when the case opened (`takt: take <story>`) is not such a commit: it moves
        // the story's status, not the work. Nothing is pushed and no record written; a commit
        // moves the revision and makes `pr.open` due again.
        if let Next::Open(request) = &next {
            let ahead = crate::git::run(
                tree,
                &[
                    "rev-list",
                    "--count",
                    "--invert-grep",
                    "--extended-regexp",
                    "--grep=^takt: take [^ ]+$",
                    &format!("{}..{revision}", request.base),
                ],
            )?;
            if ahead.trim() == "0" {
                return Ok(Observation {
                    summary: format!(
                        "{name} at {} has no commit beyond {}; nothing pushed, no pull request; the claim stays unknown",
                        short(revision),
                        request.base
                    ),
                    records: Vec::new(),
                    artifact: None,
                });
            }
        }
        self.forge
            .push_revision(tree, &name, &head)
            .with_context(|| format!("push {name} through the forge {forge}"))?;
        branch.pushed = Some(head.clone());
        let carried = above(revision, &head);
        let (state, summary) = match next {
            Next::Read(number) => {
                let view = self.forge.pull_request(tree, number).with_context(|| {
                    format!("read pull request #{number} through the forge {forge}")
                })?;
                if view.head.branch == name && view.head.sha == head {
                    (
                        Some(view.state),
                        format!(
                            "{repaired}pushed {name} at {} through {forge}{carried}; pull request #{number} is {}",
                            short(revision),
                            view.state
                        ),
                    )
                } else {
                    (
                        None,
                        format!(
                            "{repaired}pushed {name} at {} through {forge}; pull request #{number} has the head {} at {}, not the case's branch at the revision; no record, the claim stays unknown",
                            short(revision),
                            view.head.branch,
                            short(&view.head.sha)
                        ),
                    )
                }
            }
            Next::Open(request) => {
                let number = self
                    .forge
                    .open_pull_request(tree, &dir.join("artifacts"), &request)
                    .with_context(|| {
                        format!("open the pull request for {name} through the forge {forge}")
                    })?;
                branch.pull_request = Some(number);
                (
                    Some(PullRequestState::Open),
                    format!(
                        "{repaired}pushed {name} at {} through {forge}{carried} and opened pull request #{number}: open",
                        short(revision)
                    ),
                )
            }
        };
        branch.write(&dir)?;
        let records = state
            .map(|state| {
                vec![Record::new(
                    format!("pull-request-{seq}"),
                    self.kind(),
                    Some(state.as_str().to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        Ok(Observation {
            summary,
            records,
            artifact: None,
        })
    }
}

/// `ci.read`: reads the checks on the head of the case's pull request through the forge and
/// writes one `ci_result` record, `pass` or `fail`. Pending checks write no record, because a
/// claim that could not be observed stays unknown; so does a case with no pull request yet, or
/// one whose pull request the forge does not report as the case's branch at the head pushed,
/// the revision or the revision with only the approval's own commits above it: a number the
/// case directory carries is not trusted on its own. Nothing but the pull request head is read.
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
        // The case's branch, from the case id: not the `name` of a file a harness can write.
        let name = branch_of(&dir)?;
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
        let pushed = match branch.pushed {
            Some(pushed) if carries(tree, &dir, revision, &pushed)? => pushed,
            pushed => {
                return Ok(nothing(format!(
                    "the head of pull request #{number} is {}, not the implementation revision {}; nothing to read, the claim stays unknown",
                    pushed.as_deref().map_or("not pushed", short),
                    short(revision)
                )));
            }
        };
        let forge = self.forge.name();
        let view = self
            .forge
            .pull_request(tree, number)
            .with_context(|| format!("read pull request #{number} through the forge {forge}"))?;
        if view.head.branch != name || view.head.sha != pushed {
            return Ok(nothing(format!(
                "pull request #{number} has the head {} at {}, not {name} at {}; nothing to read, the claim stays unknown",
                view.head.branch,
                short(&view.head.sha),
                short(revision)
            )));
        }
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

/// `repository.merge`: merges the case's pull request through the forge, the one deterministic
/// step with a write effect, and writes one `merge_observation` record with the state the forge
/// reports afterwards: `merged` once it is, `closed` for a pull request closed unmerged. The
/// loop takes the step only when Canon admits the action, and Canon admits it only under a
/// recorded grant of `repository.merge` read from the case's `authority.yaml`: a connection to
/// the forge is never authority. The grant authorises the implementation revision it was given
/// at and no other, and the step checks that again itself: an open pull request at a revision
/// the case holds no grant for is not merged. Nothing is merged unless the forge reports the
/// pull request's head as the case's branch at the head last pushed, and that head carries the
/// revision: it is the revision, or the revision with only the approval's own commits above it
/// ([`crate::case::approvals_only`]), the record of the grant in the planning store, which so
/// reaches the base with the merge. Any other head is not merged and not replaced. When the
/// pull request's head is the revision and the case's branch holds the approval's commits
/// above it, the branch is pushed first, so the head merged carries the record. The merge names
/// the head, so a head that moved meanwhile is refused by the forge rather than merged. The
/// record is about the revision the grant authorises; the head merged is retained beside it as
/// `artifacts/merged-head-<seq>.json`. A pull request already merged is recorded without a
/// second merge; one the forge still reports open after the merge writes no record, because a
/// claim that could not be observed stays unknown.
pub struct PrMerged {
    /// The forge the merge goes through and the state is read from.
    pub forge: Rc<dyn Forge>,
}

impl PrMerged {
    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self { forge }
    }
}

impl std::fmt::Debug for PrMerged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrMerged")
            .field("forge", &self.forge.name())
            .finish()
    }
}

impl Validator for PrMerged {
    fn name(&self) -> &str {
        "takt.pr-merged"
    }

    fn action(&self) -> &str {
        "repository.merge"
    }

    fn kind(&self) -> &str {
        "merge_observation"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    /// The case's `branch.yaml`, when the approval's commits are pushed before the merge, and
    /// what the bot route keeps of the merge request it sent and the response it read.
    fn writes(&self) -> Vec<PathBuf> {
        let artifacts = Path::new("artifacts");
        vec![
            PathBuf::from(BRANCH_FILE),
            artifacts.join(BotRoute::MERGE_REQUEST),
            artifacts.join(BotRoute::MERGE_RESPONSE),
        ]
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let dir = current(tree)?;
        let mut branch = Branch::read(&dir)?;
        // The case's branch, from the case id: not the `name` of a file a harness can write.
        let name = branch_of(&dir)?;
        let forge = self.forge.name();
        let nothing = |summary: String| Observation {
            summary,
            records: Vec::new(),
            artifact: None,
        };
        let Some(number) = branch.pull_request else {
            return Ok(nothing(
                "no pull request is open for the case; nothing to merge, the claim stays unknown"
                    .to_owned(),
            ));
        };
        let pushed = match branch.pushed.clone() {
            Some(pushed) if carries(tree, &dir, revision, &pushed)? => pushed,
            pushed => {
                return Ok(nothing(format!(
                    "the head of pull request #{number} is {}, not the implementation revision {}; nothing to merge, the claim stays unknown",
                    pushed.as_deref().map_or("not pushed", short),
                    short(revision)
                )));
            }
        };
        let mut view = self
            .forge
            .pull_request(tree, number)
            .with_context(|| format!("read pull request #{number} through the forge {forge}"))?;
        if view.head.branch != name || view.head.sha != pushed {
            return Ok(nothing(format!(
                "pull request #{number} has the head {} at {}, not {name} at {}; nothing to merge, the claim stays unknown",
                view.head.branch,
                short(&view.head.sha),
                short(&pushed)
            )));
        }
        let (state, merged_now) = match view.state {
            PullRequestState::Open => {
                // The grant authorises the revision the case reached the gate at and no other:
                // checked here as well as in the evaluation that admitted the step.
                let authority = crate::case::Authority::read(&dir)?.unwrap_or_default();
                if !authority.grants_at(self.action(), revision) {
                    let granted = authority
                        .decision(self.action())
                        .filter(|decision| decision.decision == crate::case::Grant::Granted)
                        .map_or_else(
                            || "the case holds no grant of it".to_owned(),
                            |decision| {
                                format!(
                                    "the case's grant is for {}",
                                    decision.revision.as_deref().map_or("no revision", short)
                                )
                            },
                        );
                    return Ok(nothing(format!(
                        "pull request #{number} at {} is open, and {} is not granted at that revision ({granted}); nothing merged, the claim stays unknown",
                        short(revision),
                        self.action()
                    )));
                }
                // The approval's own commits above the revision go with the pull request, so the
                // record of the grant reaches the base with the merge: a head still at the
                // revision is brought up to the branch's `HEAD` when that carries the revision.
                let local = crate::git::head(tree)?;
                if view.head.sha == revision
                    && local != revision
                    && carries(tree, &dir, revision, &local)?
                {
                    self.forge
                        .push_revision(tree, &name, &local)
                        .with_context(|| format!("push {name} through the forge {forge}"))?;
                    branch.pushed = Some(local.clone());
                    branch.write(&dir)?;
                    view = self.forge.pull_request(tree, number).with_context(|| {
                        format!("read pull request #{number} through the forge {forge}")
                    })?;
                    if view.head.branch != name || view.head.sha != local {
                        return Ok(nothing(format!(
                            "pushed {name} at {} through {forge}, and pull request #{number} has the head {} at {}; nothing merged, the claim stays unknown",
                            short(&local),
                            view.head.branch,
                            short(&view.head.sha)
                        )));
                    }
                }
                self.forge
                    .merge(tree, &dir.join("artifacts"), number, &view.head.sha)
                    .with_context(|| {
                        format!("merge pull request #{number} through the forge {forge}")
                    })?;
                let after = self.forge.pull_request(tree, number).with_context(|| {
                    format!("read pull request #{number} through the forge {forge} after the merge")
                })?;
                (after.state, true)
            }
            state => (state, false),
        };
        let head = view.head.sha.clone();
        let carried = above(revision, &head);
        let result = match state {
            PullRequestState::Merged => Some("merged"),
            PullRequestState::Closed => Some("closed"),
            PullRequestState::Open => None,
        };
        let summary = match (state, merged_now) {
            (PullRequestState::Merged, true) => format!(
                "merged pull request #{number} at {} through {forge}: merged{carried}",
                short(revision)
            ),
            (PullRequestState::Merged, false) => format!(
                "pull request #{number} at {} is already merged, read through {forge}: merged{carried}",
                short(revision)
            ),
            (PullRequestState::Closed, _) => format!(
                "pull request #{number} at {} is closed unmerged, read through {forge}: closed",
                short(revision)
            ),
            (PullRequestState::Open, _) => format!(
                "merged pull request #{number} at {} through {forge}, which still reports it open; no record, the claim stays unknown",
                short(revision)
            ),
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("merge-observation-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        // The record is about the revision the grant authorises; which head was merged, the
        // revision or the revision with the approval's commits above it, is kept beside it.
        let artifact = if state == PullRequestState::Merged {
            let kept = serde_json::json!({
                "pull_request": number,
                "granted": revision,
                "merged_head": head,
            });
            Some(Artifact {
                name: format!("merged-head-{seq}.json"),
                bytes: serde_json::to_vec_pretty(&kept).context("serialize the merged head")?,
            })
        } else {
            None
        };
        Ok(Observation {
            summary,
            records,
            artifact,
        })
    }
}
