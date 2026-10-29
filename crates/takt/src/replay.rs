//! Replays one judgement step of a recorded case (`takt replay`): the state the step started
//! from is rebuilt from the case's record, the step's judgement is run again in a scratch
//! worktree at the revision it started at, and each replay is recorded outside the case. The
//! case's files are only read.
//!
//! The state a step started from ([`Start`]): the revision, the last one the record names before
//! the step's line (a validator's observed revision, a step's committed effect, a store write's
//! commit), else the parent of the step's own commit; the decision, the last the step's run
//! evaluated before it (`decisions/<seq>.json`), with each approval-required action answered as
//! the run answered it, by the case's authority over the revisions that decision was computed at
//! where the record's line shows the run had that answer; the evidence records written before
//! it; the record up to it; the case's intent, protocol and inputs. In the scratch tree, at that
//! revision, the case directory is rebuilt as the step found it ([`rebuild_case`]), and the
//! harness is given the step's context ([`crate::StepContext`]) with the offer the run made at
//! that decision ([`crate::run::judgement_offer`]) and the change so far read as the run read it
//! ([`crate::run::change_so_far`]). A replay records `replay.json`
//! ([`FORMAT`]) and the step's transcript beside it, and the scratch tree is removed.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::case::Decision;
use crate::harness::{ChangeSoFar, ClaudeCode, Harness, StepContext};
use crate::protocol::LoadedProtocol;
use crate::scan::gates::ChangeScan;

/// The format of a replay's record, `replay.json`.
pub const FORMAT: &str = "takt-replay/1";

/// What a replay's record says of its prompt against the recorded step's: the same first line
/// of the transcript, byte for byte.
pub const AS_RECORDED: &str = "as recorded";
/// The replay's prompt line is not the recorded one.
pub const DIFFERS: &str = "differs";
/// No recorded transcript was found to compare with.
pub const UNRECORDED: &str = "unrecorded";

/// What of the case directory a replay does not rebuild in its scratch tree: the record does
/// not say what they held when the step started.
pub const NOT_REBUILT: [&str; 4] = ["authority.yaml", "artifacts/", "riders/", "run.lock"];

/// The files of a case directory a replay copies as they are now: written when the case opened
/// and changed by no step (`branch.yaml` gains the push and the pull request once they are
/// made, which a replay of an earlier step sees as they are now). The module names are among
/// them only when a Python answered at open; a file not there is not copied, and the replay's
/// tree reads the built-in lists.
const OPENING_FILES: [&str; 9] = [
    "intent.md",
    "protocol.yaml",
    "protocol.ir.json",
    crate::case::BRANCH_FILE,
    crate::case::STORY_FILE,
    crate::case::COMMIT_IDENTITY_RECORD,
    crate::case::FORGE_FILE,
    crate::case::INSTALL_ROOT_FILE,
    crate::run::MODULES_FILE,
];

/// The state one judgement step of a recorded case started from.
#[derive(Debug)]
pub struct Start {
    /// The case directory read.
    pub case_dir: PathBuf,
    /// The case id, the directory's name.
    pub case: String,
    /// The run of the case the step belongs to, from 1 (the run that opened it).
    pub run: u64,
    /// The step's number in its run, as the record numbers it.
    pub step: u64,
    /// The revision the step started at: a commit, or `<commit>^` for the parent of the
    /// step's own commit ([`Start::revision_from`]).
    pub revision: String,
    /// Which line of the record named the revision, for a person.
    pub revision_from: String,
    /// The decision file the step was offered its actions at, relative to the case directory.
    pub decision_file: String,
    /// The decision's sequence number.
    pub decision_seq: usize,
    /// The decision, with each action it finds approval-required that the case's authority
    /// answered for the run, over the revisions the decision was computed at, given that answer
    /// where the record's line for the evaluation shows it.
    pub decision: Decision,
    /// The case's intent, `intent.md`.
    pub intent: String,
    /// The case's protocol, its own `protocol.yaml`.
    pub protocol: LoadedProtocol,
    /// The case's inputs.
    pub inputs: Vec<(String, String)>,
    /// The evidence records written before the step, their names under `evidence/`.
    pub evidence: Vec<String>,
    /// The record, `events.jsonl`, up to the step's line, byte for byte.
    pub record: Vec<u8>,
    /// The step's transcript as its line names it, relative to the transcripts directory.
    pub transcript: Option<String>,
    /// The action the recorded step took.
    pub action: String,
}

/// The tree a case directory lies in, `<tree>/.takt/cases/<id>` or `<tree>/.engineering/cases/<id>`;
/// an error for a directory that does not lie so.
pub fn tree_of(case_dir: &Path) -> Result<PathBuf> {
    let dir = std::fs::canonicalize(case_dir)
        .with_context(|| format!("resolve the case directory {}", case_dir.display()))?;
    let cases = dir.parent().unwrap_or(&dir);
    if !crate::case::CASE_ROOTS
        .iter()
        .any(|root| cases.ends_with(root))
    {
        bail!(
            "the case directory {} does not lie under a tree's {}; name the repository with --repo",
            case_dir.display(),
            crate::case::CASE_ROOTS.join(" or ")
        );
    }
    Ok(cases
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default())
}

/// One line of the record, read as JSON, with the byte at which it starts.
struct Line {
    at: usize,
    run: u64,
    value: serde_json::Value,
}

/// The state step `step` of the case at `case_dir` started from, in the run `run` or, when none
/// is given, the one run whose record numbers a step `step`. Refused: a record that does not
/// read, a step the record does not hold or holds in several runs (name one with `run`), a step
/// whose first line is not a harness's step line (a validator's step, a refusal, an error), no
/// decision before it, no revision the record names for it.
pub fn starting_state(case_dir: &Path, step: u64, run: Option<u64>) -> Result<Start> {
    let path = case_dir.join(crate::events::EVENTS_FILE);
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let mut lines = Vec::new();
    let mut at = 0;
    let mut runs = 0;
    for raw in bytes.split_inclusive(|byte| *byte == b'\n') {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(raw) {
            if value["event"] == "opened" {
                runs += 1;
            }
            lines.push(Line {
                at,
                run: runs,
                value,
            });
        }
        at += raw.len();
    }
    let mut firsts: Vec<usize> = Vec::new();
    for r in 1..=runs {
        if run.is_some_and(|wanted| wanted != r) {
            continue;
        }
        if let Some(index) = lines
            .iter()
            .position(|line| line.run == r && line.value["step"].as_u64() == Some(step))
        {
            firsts.push(index);
        }
    }
    let index = match firsts.as_slice() {
        [] => match run {
            Some(r) => bail!("run {r} of the case records no step {step}"),
            None => bail!("the case records no step {step}"),
        },
        [one] => *one,
        several => bail!(
            "step {step} is in runs {} of the case; name one with --run",
            several
                .iter()
                .map(|index| lines[*index].run.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let first = &lines[index];
    let r = first.run;
    let value = &first.value;
    if value["event"] != "step" || value["by"] != "harness" {
        let what = if value["event"] == "step" {
            format!(
                "the {} {}'s ({})",
                value["by"].as_str().unwrap_or("?"),
                value["name"].as_str().unwrap_or("?"),
                value["action"].as_str().unwrap_or("?")
            )
        } else {
            format!(
                "a `{}` line with no harness step line",
                value["event"].as_str().unwrap_or("?")
            )
        };
        bail!(
            "step {step} of run {r} is {what}, not a judgement step that recorded its line; only such a step is replayed"
        );
    }
    let decision_line = lines[..index]
        .iter()
        .rev()
        .take_while(|line| line.run == r)
        .find(|line| line.value["event"] == "decision")
        .ok_or_else(|| anyhow!("run {r} records no decision before step {step}"))?;
    let decision_file = decision_line.value["file"]
        .as_str()
        .ok_or_else(|| anyhow!("the decision before step {step} names no file"))?
        .to_owned();
    let decision_seq = decision_line.value["seq"]
        .as_u64()
        .and_then(|seq| usize::try_from(seq).ok())
        .ok_or_else(|| anyhow!("the decision before step {step} has no sequence number"))?;
    if decision_file != format!("decisions/{decision_seq:04}.json") {
        bail!(
            "the decision before step {step} names the file {decision_file:?}, not one of the case's decisions"
        );
    }
    let decision_path = case_dir.join(&decision_file);
    let decision_bytes = match read_plain(&decision_path)? {
        Some(bytes) => bytes,
        // A `default` run keeps the latest decision alone: an earlier one it evaluated is gone,
        // and a replay needs it, so it is refused by name, not read as missing from the case. Any
        // run of the case opened under `default` may have dropped it, not only the step's own.
        None if any_run_default(&lines) => bail!(
            "the decision before step {step} of run {r}, {decision_file}, is not in the case: a run of this case kept its latest decision alone, as the default records profile does, and dropped the earlier ones; replay a step of a run made with `--records trial`, which keeps every decision"
        ),
        None => bail!("{} is not in the case", decision_path.display()),
    };
    let decision = Decision::from_json(
        &String::from_utf8(decision_bytes)
            .with_context(|| format!("read {}", decision_path.display()))?,
    )
    .with_context(|| format!("read {}", decision_path.display()))?;
    let (revision, revision_from) = start_revision(&lines, index, step)?;
    let evidence = lines[..index]
        .iter()
        .filter(|line| line.value["event"] == "evidence")
        .filter_map(|line| {
            let path = line.value["path"].as_str()?;
            let (_, name) = path.rsplit_once("/evidence/")?;
            (!name.is_empty() && !name.contains('/')).then(|| name.to_owned())
        })
        .collect();
    let intent_path = case_dir.join("intent.md");
    let intent = std::fs::read_to_string(&intent_path)
        .with_context(|| format!("read {}", intent_path.display()))?;
    let protocol = LoadedProtocol::from_file(&case_dir.join("protocol.yaml"))?;
    let inputs = crate::case::inputs_of(case_dir)?;
    let case = case_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| anyhow!("the case directory {} has no name", case_dir.display()))?;
    crate::case::check_id(&case)?;
    let decision = as_answered(
        case_dir,
        &case,
        &decision,
        decision_line.value["frontier"].as_str(),
    )?;
    Ok(Start {
        case_dir: case_dir.to_path_buf(),
        case,
        run: r,
        step,
        revision,
        revision_from,
        decision_file,
        decision_seq,
        decision,
        intent,
        protocol,
        inputs,
        evidence,
        record: bytes[..first.at].to_vec(),
        transcript: value["transcript"].as_str().map(str::to_owned),
        action: value["action"].as_str().unwrap_or_default().to_owned(),
    })
}

/// The decision `recorded`, the governor's document the step's run evaluated, with each action
/// it finds approval-required answered as the run answered it: by the case's authority over the
/// case and artifact revisions the document was computed at
/// ([`crate::case::CaseAuthority::answer_recorded`]), where the record's line for that
/// evaluation, its `frontier`, gives the run that same answer. The document holds no authority,
/// so a step offered an action through a grant is offered it again. A grant records the
/// revision of its subject and no case revision, and its instant only to the second, so a grant
/// on file now may have been given after the step, at the same revisions: the record says
/// whether the run had it. Neither grants alone: a status the record names that the authority
/// does not answer is not taken, and an answer the record does not show is not applied.
fn as_answered(
    case_dir: &Path,
    case: &str,
    recorded: &Decision,
    frontier: Option<&str>,
) -> Result<Decision> {
    let by_authority = crate::case::CaseAuthority::new(case_dir, case).answer_recorded(recorded)?;
    let run_had = frontier.map(frontier_statuses).unwrap_or_default();
    let mut answered = recorded.clone();
    for (action, status) in by_authority.actions {
        if recorded.actions.get(&action) != Some(&status) && run_had.get(&action) == Some(&status) {
            answered.actions.insert(action, status);
        }
    }
    Ok(answered)
}

/// Each action's status as a record's `frontier` line gives it (`frontier: <action> (<status>),
/// ...`, [`Decision::frontier_line`]); none from a line of another form.
fn frontier_statuses(line: &str) -> BTreeMap<String, String> {
    line.strip_prefix("frontier: ")
        .into_iter()
        .flat_map(|actions| actions.split(", "))
        .filter_map(|part| {
            let (action, status) = part.strip_suffix(')')?.rsplit_once(" (")?;
            Some((action.to_owned(), status.to_owned()))
        })
        .collect()
}

/// Whether any run of the record was opened under the `default` records profile: each run's
/// `opened` line names the profile it kept, and a line written before the profile names none,
/// which kept every decision. Such a run may have dropped a decision of any run of the case.
fn any_run_default(lines: &[Line]) -> bool {
    lines
        .iter()
        .any(|line| line.value["event"] == "opened" && line.value["records"] == "default")
}

/// The revision the step whose line is `lines[index]` started at: the last the record names
/// before it, a validator's observed revision, a step's committed effect or a store write's
/// commit, each the case branch's `HEAD` when it was written; else the parent of the step's
/// own commit, when its effect names one.
fn start_revision(lines: &[Line], index: usize, step: u64) -> Result<(String, String)> {
    for line in lines[..index].iter().rev() {
        let value = &line.value;
        let named = match value["event"].as_str() {
            Some("step" | "effect") => value["revision"].as_str(),
            Some("store_evidence" | "store_move") => value["commit"].as_str(),
            _ => None,
        };
        if let Some(revision) = named {
            return Ok((
                revision.to_owned(),
                format!(
                    "the {} line of step {} in run {}",
                    value["event"].as_str().unwrap_or("?"),
                    value["step"].as_u64().unwrap_or(0),
                    line.run
                ),
            ));
        }
    }
    let own = lines[index + 1..]
        .iter()
        .take_while(|line| line.value["step"].as_u64() == Some(step))
        .find(|line| line.value["event"] == "effect")
        .and_then(|line| {
            let committed = line.value["paths"]
                .as_array()
                .is_some_and(|paths| !paths.is_empty());
            committed.then(|| line.value["revision"].as_str()).flatten()
        });
    match own {
        Some(revision) => Ok((
            format!("{revision}^"),
            format!("the parent of step {step}'s own commit"),
        )),
        None => bail!(
            "the record names no revision before step {step}, and the step committed nothing: the revision it started at is not known"
        ),
    }
}

/// The first line of a step's transcript, without its end: the line takt writes first, the
/// prompt. Read at `<transcripts>/<path>` when the step's line names one, else where takt keeps
/// it, `<transcripts>/<repo-key>/<case>/run-<r>/step-<n>.jsonl`, `repo` the case's repository;
/// none when neither is a file.
pub fn recorded_prompt_line(
    start: &Start,
    transcripts: &Path,
    repo: &Path,
) -> Result<Option<(PathBuf, Vec<u8>)>> {
    let mut candidates = Vec::new();
    if let Some(named) = &start.transcript {
        candidates.push(transcripts.join(named));
    }
    if let Ok(key) = crate::harness::repo_key(repo) {
        candidates.push(
            transcripts
                .join(key)
                .join(&start.case)
                .join(format!("run-{}", start.run))
                .join(format!("step-{}.jsonl", start.step)),
        );
    }
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        return Ok(Some((path, first_line(&bytes).to_vec())));
    }
    Ok(None)
}

/// `bytes` up to its first line end, the end left out.
fn first_line(bytes: &[u8]) -> &[u8] {
    bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes, |end| &bytes[..end])
}

/// The first line of the transcript `claude` would write for the step `start` describes, the
/// prompt it builds, without running anything: the step is asked of a copy whose program
/// cannot start, and a step whose harness never started has that line alone as its transcript.
/// The change so far is read in `repo`, the repository that holds the case's commits, and so
/// are the scope files, at the revision the step started at.
pub fn rebuilt_prompt_line(
    claude: &ClaudeCode,
    start: &Start,
    gate: Option<&str>,
    repo: &Path,
) -> Result<Vec<u8>> {
    let probe = ClaudeCode {
        program: String::new(),
        reported: std::cell::Cell::new(None),
        call: std::cell::RefCell::new(None),
        transcript: std::cell::RefCell::new(None),
        ..claude.clone()
    };
    let offer = crate::run::judgement_offer(
        &start.protocol,
        &start.inputs,
        &start.decision,
        probe.keeps_state(),
        gate,
    )?;
    let change = change_of(start, &start.case_dir, repo, &start.revision)?;
    let (failed, last_gate) = failed_of(start);
    // The scope files as the revision the step started at holds them, in the repository.
    let ask = commit_ask(start);
    let context = context(
        start,
        repo,
        &offer,
        &change,
        &failed,
        last_gate.as_ref(),
        Some(&start.revision),
        None,
        ask.as_ref(),
    );
    let _ = probe.step(&context);
    let transcript = probe
        .last_transcript()
        .ok_or_else(|| anyhow!("the harness kept no prompt"))?;
    Ok(first_line(&transcript).to_vec())
}

/// The change so far the step `start` describes was shown, read by the code a run reads it with
/// ([`crate::run::change_so_far_for`], under the case's intent): the `base_commit` of the case's
/// `branch.yaml` in `case_dir`,
/// the paths the `effect` lines of its record up to the step name, the diff to `revision`, the
/// revision the step started at, in the repository of `tree`, under the scan the environment
/// names ([`ChangeScan::from_env`]), as a run under the same environment scans it. What a run
/// would print of a change it could not show is not printed: the prompt says it is not shown.
fn change_of(start: &Start, case_dir: &Path, tree: &Path, revision: &str) -> Result<ChangeSoFar> {
    let record = || {
        Ok(start
            .record
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice(line).ok())
            .collect())
    };
    crate::run::change_so_far_for(
        tree,
        case_dir,
        record,
        (Some(revision), Some(&start.intent)),
        ChangeScan::from_env().as_ref(),
        &mut std::io::sink(),
    )
}

/// The context the step `start` describes was given, in `tree`, with `offer` and the change so
/// far ([`change_of`]), its scope files read at `scope_revision` when one is given, else in
/// `tree` as it stands ([`crate::harness::scope_files`]).
#[allow(clippy::too_many_arguments)]
fn context<'a>(
    start: &'a Start,
    tree: &'a Path,
    offer: &'a crate::run::Offer,
    change: &'a ChangeSoFar,
    failed: &'a [crate::validator::FailedCheck],
    last_gate: Option<&'a crate::run::GateFailure>,
    scope_revision: Option<&'a str>,
    base_revision: Option<&'a str>,
    commit: Option<&'a crate::commit_style::Ask>,
) -> StepContext<'a> {
    StepContext {
        scope_revision,
        base_revision,
        commit,
        tree,
        intent: &start.intent,
        offered: &offer.offered,
        approvals: &offer.approvals,
        choices: &offer.choices,
        // A replay gives the recorded harness its prompt, which shows no grant, and the offer a
        // run records names none: a replayed step is given none.
        granted: &[],
        decision: &start.decision,
        protocol: &start.protocol,
        step: usize::try_from(start.step).unwrap_or(usize::MAX),
        time_left: None,
        change,
        failed,
        last_gate,
        obligations: &[],
    }
}

/// What the step `start` describes was asked of the reply's `commit`: the `commit` member of the
/// last `opened` line of its record ([`crate::commit_style::Ask::of_record`]); none for a record
/// written before the member, whose prompt asked for none.
fn commit_ask(start: &Start) -> Option<crate::commit_style::Ask> {
    start
        .record
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
        .rfind(|line| line["event"] == "opened")
        .and_then(|line| crate::commit_style::Ask::of_record(&line["commit"]))
}

/// What the step `start` describes was shown of the checks that failed before it and of the gate
/// failure its record ended on, read from its record up to the step as a run reads them
/// ([`crate::run::failed_checks`], [`crate::run::last_gate_failure`]).
fn failed_of(
    start: &Start,
) -> (
    Vec<crate::validator::FailedCheck>,
    Option<crate::run::GateFailure>,
) {
    let record: Vec<serde_json::Value> = start
        .record
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice(line).ok())
        .collect();
    (
        crate::run::failed_checks(&record),
        crate::run::last_gate_failure(&record),
    )
}

/// Writes the case directory as the step `start` describes found it under `tree`
/// (`<tree>/.engineering/cases/<id>`): the files the case opened with, its `case.yaml` with the
/// `implementation` artifact at the step's revision ([`crate::case::snapshot_at`], `revision`
/// resolved), the evidence records written before the step, its decisions up to the step's,
/// and its record up to the step's line. What the record cannot date is not written
/// ([`NOT_REBUILT`]).
///
/// Nothing is read or written through a link: a link on the way to the case directory in the
/// tree (`.engineering`, the cases directory, the case's own) or at its `evidence` or
/// `decisions` is refused, each file is written without following a link at it (`O_NOFOLLOW`),
/// and a file of the case that is a link is refused, never copied ([`read_plain`]). The
/// revision is the record's to name, and its tree could put a link anywhere.
pub fn rebuild_case(start: &Start, tree: &Path, revision: &str) -> Result<PathBuf> {
    crate::case::refuse_link_on_the_way(tree, &start.case, "rebuilt for a replay")?;
    let dir = crate::case::CaseDir::locate(tree, &start.case);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    plain_dir(&dir.join("evidence"))?;
    plain_dir(&dir.join("decisions"))?;
    for name in OPENING_FILES {
        if let Some(bytes) = read_plain(&start.case_dir.join(name))? {
            write_plain(&dir.join(name), &bytes)?;
        }
    }
    write_plain(
        &dir.join("case.yaml"),
        crate::case::snapshot_at(&start.case_dir, revision)?.as_bytes(),
    )?;
    for name in &start.evidence {
        let from = start.case_dir.join("evidence").join(name);
        let bytes = read_plain(&from)?.ok_or_else(|| {
            anyhow!(
                "the evidence record {} the record names before step {} is not in the case",
                from.display(),
                start.step
            )
        })?;
        write_plain(&dir.join("evidence").join(name), &bytes)?;
    }
    for seq in 1..=start.decision_seq {
        let name = format!("{seq:04}.json");
        if let Some(bytes) = read_plain(&start.case_dir.join("decisions").join(&name))? {
            write_plain(&dir.join("decisions").join(&name), &bytes)?;
        }
    }
    write_plain(&dir.join(crate::events::EVENTS_FILE), &start.record)?;
    Ok(dir)
}

/// The bytes of the regular file at `path`, read without following a link at it
/// (`O_NOFOLLOW`); none when nothing is there. A link there, or anything but a regular file, is
/// refused: a case is never read through a link. The case's `authority.yaml` is read so too
/// ([`crate::case::Authority::read`]).
pub(crate) fn read_plain(path: &Path) -> Result<Option<Vec<u8>>> {
    use std::io::Read as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        Ok(meta) if meta.file_type().is_symlink() => bail!(
            "{} is a link; a case is never read or written through a link",
            path.display()
        ),
        Ok(meta) if !meta.is_file() => bail!("{} is not a file", path.display()),
        Ok(_) => {}
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("read {}", path.display()))?;
    Ok(Some(bytes))
}

/// Writes `bytes` to the file at `path`, made or emptied first, without following a link at
/// it (`O_NOFOLLOW`): a link there is refused.
fn write_plain(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| {
            if error.raw_os_error() == Some(libc::ELOOP) {
                anyhow!(
                    "{} is a link in the revision's tree; a case is never read or written through a link",
                    path.display()
                )
            } else {
                anyhow::Error::new(error).context(format!("create {}", path.display()))
            }
        })?;
    file.write_all(bytes)
        .with_context(|| format!("write {}", path.display()))
}

/// The directory `path`, made when nothing is there; a link there, or anything but a
/// directory, is refused.
fn plain_dir(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(meta) if meta.file_type().is_symlink() => bail!(
            "{} is a link in the revision's tree; a case is never read or written through a link",
            path.display()
        ),
        Ok(_) => bail!("{} is not a directory", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(path).with_context(|| format!("create {}", path.display()))
        }
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

/// What one replay did.
#[derive(Debug)]
pub struct Replayed {
    /// Where it is recorded: `replay.json` and `transcript.jsonl`.
    pub dir: PathBuf,
    /// The action the harness reported, or why the step failed.
    pub result: std::result::Result<String, String>,
    /// The tokens the harness reported.
    pub tokens: Option<u64>,
    /// The paths the step changed in its scratch tree, the case directory left out.
    pub changed: Vec<String>,
    /// [`AS_RECORDED`], [`DIFFERS`] or [`UNRECORDED`].
    pub prompt: &'static str,
}

/// A worktree `git worktree add` made for a replay, removed when dropped.
struct ScratchTree {
    repo: PathBuf,
    tree: PathBuf,
}

impl Drop for ScratchTree {
    fn drop(&mut self) {
        let tree = self.tree.display().to_string();
        if crate::git::run(&self.repo, &["worktree", "remove", "--force", &tree]).is_err() {
            let _ = std::fs::remove_dir_all(&self.tree);
        }
        let _ = crate::git::run(&self.repo, &["worktree", "prune"]);
    }
}

/// Replays the step `start` describes once, recorded in `dir`, which must not exist: a scratch
/// worktree of `repo` at the step's revision, detached, made at `<dir>/tree`, the case
/// directory rebuilt in it ([`rebuild_case`]); `harness` asked for the step with the offer the
/// run made at the step's decision under the approval gate `gate`; its transcript kept as
/// `<dir>/transcript.jsonl` and compared with `recorded`, the recorded step's first line; and
/// `<dir>/replay.json` written ([`FORMAT`]). The scratch tree is removed whatever the step did.
/// A step that failed is recorded with its error, not answered as one.
pub fn replay_once(
    start: &Start,
    repo: &Path,
    harness: &dyn Harness,
    gate: Option<&str>,
    dir: &Path,
    recorded: Option<&[u8]>,
) -> Result<Replayed> {
    use std::os::unix::fs::DirBuilderExt as _;
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("create {}", dir.display()))?;
    let revision = crate::git::run(
        repo,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", start.revision),
        ],
    )
    .with_context(|| format!("the revision step {} started at", start.step))?;
    let tree = dir.join("tree");
    let tree_arg = tree.display().to_string();
    crate::git::run(
        repo,
        &[
            "worktree", "add", "--quiet", "--detach", &tree_arg, &revision,
        ],
    )?;
    let scratch = ScratchTree {
        repo: repo.to_path_buf(),
        tree: tree.clone(),
    };
    let case_dir = rebuild_case(start, &tree, &revision)?;
    let offer = crate::run::judgement_offer(
        &start.protocol,
        &start.inputs,
        &start.decision,
        harness.keeps_state(),
        gate,
    )?;
    let started_at = crate::events::instant();
    let change = change_of(start, &case_dir, &tree, &revision)?;
    let (failed, last_gate) = failed_of(start);
    // The instruction files are read at the base the case pinned, as the run read them.
    let base = crate::case::Branch::read(&start.case_dir)
        .ok()
        .and_then(|branch| branch.base_commit);
    let ask = commit_ask(start);
    let stepped = harness.step(&context(
        start,
        &tree,
        &offer,
        &change,
        &failed,
        last_gate.as_ref(),
        None,
        base.as_deref(),
        ask.as_ref(),
    ));
    let transcript = harness.last_transcript();
    if let Some(bytes) = &transcript {
        write_private(&dir.join("transcript.jsonl"), bytes)?;
    }
    let prompt = match (recorded, &transcript) {
        (None, _) => UNRECORDED,
        (Some(line), Some(bytes)) if first_line(bytes) == line => AS_RECORDED,
        (Some(_), _) => DIFFERS,
    };
    let changed: Vec<String> = crate::git::changed_paths(&tree)
        .unwrap_or_default()
        .into_iter()
        .filter(|path| !crate::case::is_under_cases(path))
        .collect();
    drop(scratch);
    let usage = harness.last_usage();
    let call = harness.last_call();
    let tokens = stepped.as_ref().ok().and_then(|report| report.tokens);
    let mut record = serde_json::json!({
        "format": FORMAT,
        "case": start.case,
        "run": start.run,
        "step": start.step,
        "replay": dir.file_name().map(|name| name.to_string_lossy().into_owned()),
        "revision": revision,
        "revision_from": start.revision_from,
        "decision": start.decision_file,
        "offered": offer.offered,
        "approvals": offer.approvals,
        "evidence": start.evidence,
        "not_rebuilt": NOT_REBUILT,
        "recorded_action": start.action,
        "recorded_transcript": start.transcript,
        "harness": harness.name(),
        "started_at": started_at,
        "prompt": prompt,
        "changed": changed,
    });
    match &stepped {
        Ok(report) => {
            record["action"] = report.action.clone().into();
            record["summary"] = report.summary.clone().into();
        }
        Err(error) => record["error"] = format!("{error:#}").into(),
    }
    if let Some(tokens) = tokens {
        record["tokens"] = tokens.into();
    }
    if let Some(usage) = usage {
        record["usage"] = serde_json::to_value(usage)?;
    }
    // What the harness stated about its call, each field only when it stated it.
    if let Some(call) = call {
        if let Some(model) = call.model {
            record["model"] = model.into();
        }
        if let Some(cost) = call.cost_usd {
            record["cost_usd"] = cost.into();
        }
        if let Some(duration) = call.duration_ms {
            record["duration_ms"] = duration.into();
        }
        if let Some(turns) = call.turns {
            record["turns"] = turns.into();
        }
    }
    let mut text = serde_json::to_string_pretty(&record)?;
    text.push('\n');
    write_private(&dir.join("replay.json"), text.as_bytes())?;
    Ok(Replayed {
        dir: dir.to_path_buf(),
        result: stepped
            .map(|report| report.action)
            .map_err(|error| format!("{error:#}")),
        tokens,
        changed,
        prompt,
    })
}

/// Writes `bytes` to a new file at `path`, readable by its owner only (`0600`): a transcript
/// carries what the model read. A file already there is refused.
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .with_context(|| format!("create {}", path.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("write {}", path.display()))
}

/// Refuses `out` as the directory replays are recorded in when it is, or would be once made,
/// inside the case directory, the tree the case directory lies in ([`tree_of`], when it lies in
/// one) or the repository's tree (through any link on the way): a replay is recorded beside the
/// case, never in it, and leaves both trees as they were.
pub fn check_out_dir(out: &Path, case_dir: &Path, repo: &Path) -> Result<()> {
    let absolute = if out.is_absolute() {
        out.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read the current directory")?
            .join(out)
    };
    let mut rest = PathBuf::new();
    let mut base = absolute.as_path();
    while std::fs::symlink_metadata(base).is_err() {
        match (base.parent(), base.file_name()) {
            (Some(parent), Some(name)) => {
                rest = Path::new(name).join(rest);
                base = parent;
            }
            _ => break,
        }
    }
    let resolved = std::fs::canonicalize(base)
        .with_context(|| format!("resolve {}", base.display()))?
        .join(rest);
    let case_tree = tree_of(case_dir)
        .ok()
        .filter(|tree| !tree.as_os_str().is_empty());
    let mut insides = vec![("case directory", case_dir), ("repository's tree", repo)];
    if let Some(tree) = &case_tree {
        insides.push(("case's tree", tree.as_path()));
    }
    for (what, inside) in insides {
        let inside = std::fs::canonicalize(inside).unwrap_or_else(|_| inside.to_path_buf());
        if resolved.starts_with(&inside) {
            bail!(
                "the replays directory {} is inside the {what} {}: a replay is recorded beside the case, never in it",
                out.display(),
                inside.display()
            );
        }
    }
    Ok(())
}
