//! The judgement actor: a harness performs one admissible action in the worktree and reports
//! which. The first adapter is Claude Code (`claude -p`), given file tools only: it reads and
//! edits; takt runs the tests and writes the evidence. It runs on the user's claude.ai login:
//! an `ANTHROPIC_API_KEY` in the environment is dropped, never spent. Each step's stream is
//! kept as the case's `artifacts/run-<r>/step-<n>.jsonl` ([`keep_transcript`]).

use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, Result, anyhow, bail};

use crate::decision::Decision;
use crate::protocol::LoadedProtocol;

/// What a step is given.
#[derive(Debug)]
pub struct StepContext<'a> {
    /// The worktree the action is performed in.
    pub tree: &'a Path,
    /// The intent the case was opened for.
    pub intent: &'a str,
    /// The actions the harness may choose among; nothing else is accepted.
    pub offered: &'a [String],
    /// The actions that need an approval: choosing one requests it and ends the run; nothing is
    /// performed for it.
    pub approvals: &'a [String],
    /// Of the offered actions, the ones that record a choice: taken with no file changed, takt
    /// records that the harness took it (`change.handover`).
    pub choices: &'a [String],
    /// The current decision: claims, obligations, the frontier.
    pub decision: &'a Decision,
    /// The protocol the case runs under.
    pub protocol: &'a LoadedProtocol,
    /// The step number.
    pub step: usize,
    /// What is left of the run's hour budget, read just before the harness is called; `None`
    /// for a run without one. A harness ends its step by then: a child it runs is held to the
    /// smaller of what is still left when it starts and its own timeout, and killed past it.
    pub time_left: Option<std::time::Duration>,
}

/// What a step reported.
#[derive(Debug, Clone)]
pub struct StepReport {
    /// The action the harness says it performed.
    pub action: String,
    /// One line on what it did.
    pub summary: String,
    /// Tokens the step used, when the harness reports them.
    pub tokens: Option<u64>,
}

/// What a harness reported about one call besides its tokens, each field absent when it did
/// not state it, never 0: the model, the cost in dollars, the call's duration and its time in
/// the API in milliseconds, the turns, and the instant the call started (stamped by takt, as
/// [`crate::events::instant`] writes one).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CallReport {
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub api_ms: Option<u64>,
    pub turns: Option<u64>,
    pub started_at: Option<String>,
}

/// A judgement actor.
pub trait Harness {
    /// The adapter's name.
    fn name(&self) -> &str;
    /// Performs one of the offered actions in the tree and reports it.
    fn step(&self, context: &StepContext<'_>) -> Result<StepReport>;
    /// The tokens of the last step by component, when the harness reported them; none by
    /// default.
    fn last_usage(&self) -> Option<crate::events::TokenUsage> {
        None
    }
    /// Whether the harness keeps what it learned in one step for the next. A harness that does
    /// not is never offered an action whose effect is `read` (`repository.inspect`) unless the
    /// action records a choice (`change.handover`): what it read would be gone at the next
    /// step, and a step that changes nothing and records nothing ends the run with
    /// `NoProgress`. Not by default.
    fn keeps_state(&self) -> bool {
        false
    }
    /// What the harness reported about the last step's call besides its tokens; none by
    /// default.
    fn last_call(&self) -> Option<CallReport> {
        None
    }
    /// The last step's transcript, the bytes takt keeps as the case's
    /// `artifacts/run-<r>/step-<n>.jsonl` ([`keep_transcript`]): the prompt and what the harness
    /// printed, whether the step succeeded, failed or was killed; none by default.
    fn last_transcript(&self) -> Option<Vec<u8>> {
        None
    }
}

/// The most bytes a step's transcript holds: past it, its head and its tail
/// ([`ClaudeCode::transcript`]).
pub const TRANSCRIPT_CAP: usize = 16 << 20;

/// Room kept under [`TRANSCRIPT_CAP`] for the line that marks a cut.
const CUT_ROOM: usize = 64;

/// Claude Code in print mode, with file tools only.
#[derive(Debug, Clone)]
pub struct ClaudeCode {
    /// The `claude` program.
    pub program: String,
    /// The model, when given.
    pub model: Option<String>,
    /// The most turns one step may take.
    pub max_turns: u32,
    /// What the child is held to.
    pub limits: crate::forge::Limits,
    /// The tokens by component the last step reported, set by each step; none before the
    /// first and after a step that reported none.
    pub reported: std::cell::Cell<Option<crate::events::TokenUsage>>,
    /// What the last step reported about its call besides its tokens, set by each step; none
    /// before the first and after a step that reported nothing of it.
    pub call: std::cell::RefCell<Option<CallReport>>,
    /// The last step's transcript ([`Harness::last_transcript`]), set by each step; none
    /// before the first.
    pub transcript: std::cell::RefCell<Option<Vec<u8>>>,
}

impl Default for ClaudeCode {
    fn default() -> Self {
        Self {
            program: "claude".to_owned(),
            model: None,
            max_turns: 40,
            limits: crate::forge::Limits {
                timeout: std::time::Duration::from_secs(1800),
                output: 8 << 20,
            },
            reported: std::cell::Cell::new(None),
            call: std::cell::RefCell::new(None),
            transcript: std::cell::RefCell::new(None),
        }
    }
}

/// The meaning of each action the prompt explains: the adapter's own words for the actions it
/// knows, else the description the protocol declares for the action, on one line.
fn describe(action: &str, protocol: &LoadedProtocol) -> String {
    let known = match action {
        "repository.inspect" => "read files to understand the code; change nothing",
        "tests.write" => {
            "add or change test files only (paths under tests/ or test/, or named *_test.*, test_*.*, *.test.*, *.spec.*)"
        }
        "implementation.edit" => "change the implementation so the intent holds",
        "plan.edit" => "write or change the plan",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_owned();
    }
    b10x_canon::model::parse(&protocol.yaml)
        .ok()
        .and_then(|model| {
            model
                .actions
                .iter()
                .find(|(id, _)| id.as_str() == action)
                .and_then(|(_, declared)| declared.description.clone())
        })
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "perform this protocol action".to_owned())
}

impl ClaudeCode {
    /// The tools a step is offered (`--tools`) and allowed (`--allowedTools`), the same list:
    /// the files of the tree, read and written, and nothing else.
    pub const TOOLS: &str = "Read,Edit,Write,Glob,Grep";

    /// The arguments that keep the child from loading anything the tree or the user's own
    /// configuration supplies: `--restricted` ignores the user, project and local settings
    /// files (their hooks, plugins and permissions with them), confines the file tools to the
    /// tree and leaves writes to settings and git files to a person; `--strict-mcp-config`
    /// starts no MCP server; the `--settings` given turns hooks off in whatever settings are
    /// still read, as far as their precedence lets it. Not `--bare`: it reads only an API key,
    /// and the step runs on the user's login.
    pub const ISOLATION: [&str; 4] = [
        "--restricted",
        "--strict-mcp-config",
        "--settings",
        r#"{"disableAllHooks":true}"#,
    ];

    fn prompt(context: &StepContext<'_>) -> String {
        let mut text = String::new();
        text.push_str("You are the judgement step of a governed engineering run (takt).\n\n");
        text.push_str(&format!("Intent: {}\n\n", context.intent));
        text.push_str(&format!(
            "Protocol: {}. Step {}.\n\n",
            context.protocol.id, context.step
        ));
        text.push_str("Claims right now:\n");
        for (claim, value) in &context.decision.claims {
            text.push_str(&format!("- {claim}: {value}\n"));
        }
        text.push_str("\nOpen obligations:\n");
        for (obligation, status) in &context.decision.obligations {
            if status == "open" {
                text.push_str(&format!("- {obligation}\n"));
            }
        }
        text.push_str("\nYou may perform exactly ONE of these actions now, nothing else:\n");
        for action in context.offered {
            text.push_str(&format!(
                "- {action}: {}\n",
                describe(action, context.protocol)
            ));
        }
        if !context.approvals.is_empty() {
            text.push_str(
                "\nThese need an approval: name one of them, changing no file, when the intent holds \
                 and no further edit is needed; that requests the approval and ends the run:\n",
            );
            for action in context.approvals {
                text.push_str(&format!("- {action}\n"));
            }
        }
        text.push_str(
            "\nRules: use only your file tools (read, edit, write) inside this directory. Do not run \
             commands; takt runs the tests and records the evidence. Do not commit. If the action \
             you pick is `repository.inspect`, change no file. If it is `tests.write`, change test \
             files only. Keep the change minimal and complete for the action.\n\n\
             When done, end your reply with exactly one line of JSON and nothing after it:\n\
             {\"action\": \"<one of the offered ids>\", \"summary\": \"<one line on what you did>\"}\n",
        );
        text
    }

    /// The action and summary of a reply, read from its last line that opens a JSON object naming
    /// `action`; no such line, or one that does not parse or names no string action, is an error.
    pub fn parse(result: &str) -> Result<(String, String)> {
        for line in result.lines().rev() {
            let line = line.trim();
            if line.starts_with('{') && line.contains("\"action\"") {
                let doc: serde_json::Value =
                    serde_json::from_str(line).context("read the step's JSON line")?;
                let action = doc["action"]
                    .as_str()
                    .ok_or_else(|| anyhow!("the step's JSON names no action"))?
                    .to_owned();
                let summary = doc["summary"].as_str().unwrap_or("").to_owned();
                return Ok((action, summary));
            }
        }
        bail!("the harness ended without the JSON line naming its action")
    }

    /// A step's transcript: `opening`, the line takt writes first, then what the harness
    /// printed on standard output (`printed`; none when it never ran), at most
    /// [`TRANSCRIPT_CAP`] bytes ([`bounded`]). Of output longer than the head and the tail
    /// [`crate::forge::run_child_keeping_head`] kept, the bytes between them are left out and
    /// the cut is marked.
    #[must_use]
    pub fn transcript(opening: &[u8], printed: Option<&crate::forge::Finished>) -> Vec<u8> {
        let empty: &[u8] = &[];
        let (head, omitted, tail) = match printed {
            None => (empty, 0, empty),
            Some(out) if !out.stdout_cut => (empty, 0, out.stdout.as_slice()),
            Some(out) => {
                let kept = out.stdout_head.len() as u64 + out.stdout.len() as u64;
                if kept >= out.stdout_len {
                    // The head and the tail meet: the whole stream, the overlap once.
                    let overlap = usize::try_from(kept - out.stdout_len).unwrap_or(usize::MAX);
                    let tail = out.stdout.get(overlap..).unwrap_or_default();
                    (out.stdout_head.as_slice(), 0, tail)
                } else {
                    (
                        out.stdout_head.as_slice(),
                        out.stdout_len - kept,
                        out.stdout.as_slice(),
                    )
                }
            }
        };
        let mut front = opening.to_vec();
        front.extend_from_slice(head);
        bounded(front, omitted, tail, TRANSCRIPT_CAP)
    }

    /// The line a reply is read from: the last line of standard output that is not blank, its
    /// end trimmed (the result event of `--output-format stream-json`, which reads as the one
    /// document `--output-format json` prints). None when the output was cut and no line end
    /// precedes it, so its start may be missing.
    fn last_line(stdout: &[u8], cut: bool) -> Option<&[u8]> {
        let end = stdout
            .iter()
            .rposition(|byte| !byte.is_ascii_whitespace())
            .map_or(0, |at| at + 1);
        let body = &stdout[..end];
        match body.iter().rposition(|byte| *byte == b'\n') {
            Some(at) => Some(&body[at + 1..]),
            None if cut => None,
            None => Some(body),
        }
    }
}

/// `front`, `omitted` bytes left out, then `back`, at most `cap` bytes. When nothing was left
/// out and the two fit, both whole; else as much of each as fits in what is left besides a
/// line that marks the cut (the front's first half at most when the back needs the rest), the
/// front cut back to its last line end and the back started past its first one (unless that
/// line end is its last byte), and between them one line,
/// `{"type":"takt.cut","omitted_bytes":<n>}`, counting every byte left out.
fn bounded(mut front: Vec<u8>, omitted: u64, back: &[u8], cap: usize) -> Vec<u8> {
    if omitted == 0 && front.len() + back.len() <= cap {
        front.extend_from_slice(back);
        return front;
    }
    let room = cap.saturating_sub(CUT_ROOM);
    let back_kept = back.len().min(room - front.len().min(room / 2));
    let front_kept = front.len().min(room - back_kept);
    let mut omitted = omitted + (front.len() - front_kept + back.len() - back_kept) as u64;
    front.truncate(front_kept);
    let line_end = front
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at + 1);
    omitted += (front.len() - line_end) as u64;
    front.truncate(line_end);
    let mut back = &back[back.len() - back_kept..];
    if let Some(at) = back.iter().position(|byte| *byte == b'\n')
        && at + 1 < back.len()
    {
        omitted += (at + 1) as u64;
        back = &back[at + 1..];
    }
    front.extend_from_slice(
        format!("{{\"type\":\"takt.cut\",\"omitted_bytes\":{omitted}}}\n").as_bytes(),
    );
    front.extend_from_slice(back);
    front
}

/// Writes `bytes` as `artifacts/run-<run>/step-<step>.jsonl` under `case_dir`, `run` the run's
/// number in the case (1 for the run that opened it), so a resumed case keeps every run's;
/// readable and writable by its owner only (`0600`, in a `0700` directory): a transcript can
/// carry what the model read. What is at that path is replaced, never written through: a link
/// there is removed, not followed. An `artifacts` or `run-<run>` that is there and is not a
/// directory (a link to one) is refused. No validator reads the file.
pub fn keep_transcript(
    case_dir: &Path,
    run: u64,
    step: u64,
    bytes: &[u8],
) -> Result<std::path::PathBuf> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let artifacts = case_dir.join("artifacts");
    plain_dir(&artifacts, None)?;
    let dir = artifacts.join(format!("run-{run}"));
    plain_dir(&dir, Some(0o700))?;
    let path = dir.join(format!("step-{step}.jsonl"));
    match std::fs::remove_file(&path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(error).with_context(|| format!("remove {}", path.display()));
        }
        _ => {}
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .and_then(|mut file| file.write_all(bytes))
        .with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// The number of the run about to open on a case, which names its transcripts' directory
/// ([`keep_transcript`]): one more than the `opened` lines the case's record at `record`
/// holds, 1 when there is none. A line that does not read as an event is skipped here, as the
/// run itself goes on past one; a record that cannot be opened (a link) is the error the run's
/// own first write would be.
pub(crate) fn run_number(record: &Path) -> Result<u64> {
    use std::io::Read as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut bytes = Vec::new();
    match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(record)
    {
        Ok(mut file) => {
            file.read_to_end(&mut bytes)
                .with_context(|| format!("read {}", record.display()))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(1),
        Err(error) => return Err(error).with_context(|| format!("read {}", record.display())),
    }
    let opened = bytes
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
        .filter(|line| line["event"] == "opened")
        .count();
    Ok(1 + opened as u64)
}

/// Makes `dir` a directory of its own, never through a link: one there is kept, given `mode`
/// when one is named; none there is created with it; anything else there (a link, a file) is
/// refused.
fn plain_dir(dir: &Path, mode: Option<u32>) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => {
            if let Some(mode) = mode
                && meta.permissions().mode() & 0o7777 != mode
            {
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode))
                    .with_context(|| format!("set the mode of {}", dir.display()))?;
            }
            Ok(())
        }
        Ok(_) => bail!(
            "{} is not a directory; no transcript is written through it",
            dir.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            if let Some(mode) = mode {
                builder.mode(mode);
            }
            builder
                .create(dir)
                .with_context(|| format!("create {}", dir.display()))
        }
        Err(error) => Err(error).with_context(|| format!("read {}", dir.display())),
    }
}

/// A harness step's transcript, kept ([`keep_transcript`]) when this is dropped: on every way
/// out of the step, after its change was kept or put back, so putting the cases directory back
/// never takes it along, and on none before. A write that fails is told on standard error and
/// changes nothing about how the step ends.
pub(crate) struct KeptOnDrop {
    case_dir: std::path::PathBuf,
    run: u64,
    step: u64,
    bytes: Option<Vec<u8>>,
}

impl KeptOnDrop {
    /// What `harness` reports as its last step's transcript, for the case at `case_dir`, its
    /// run `run`.
    pub(crate) fn new(harness: &dyn Harness, case_dir: &Path, run: u64, step: u64) -> Self {
        Self {
            case_dir: case_dir.to_path_buf(),
            run,
            step,
            bytes: harness.last_transcript(),
        }
    }
}

impl Drop for KeptOnDrop {
    fn drop(&mut self) {
        if let Some(bytes) = self.bytes.take()
            && let Err(error) = keep_transcript(&self.case_dir, self.run, self.step, &bytes)
        {
            eprintln!("step {}: its transcript was not kept: {error:#}", self.step);
        }
    }
}

impl Harness for ClaudeCode {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn last_usage(&self) -> Option<crate::events::TokenUsage> {
        self.reported.get()
    }

    /// Each step is a `claude -p` of its own with `--no-session-persistence`: nothing it read
    /// is there at the next step.
    fn keeps_state(&self) -> bool {
        false
    }

    fn last_call(&self) -> Option<CallReport> {
        self.call.borrow().clone()
    }

    fn last_transcript(&self) -> Option<Vec<u8>> {
        self.transcript.borrow().clone()
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport> {
        self.reported.set(None);
        // The instant the step must end by, taken as the call begins: the time spent below
        // before the child starts comes out of it.
        let deadline = context
            .time_left
            .and_then(|left| std::time::Instant::now().checked_add(left));
        *self.call.borrow_mut() = None;
        let started_at = crate::events::instant();
        let prompt = Self::prompt(context);
        // The transcript's first line, the prompt; what the harness prints follows it once it
        // has run, and it alone is the transcript of a step whose harness never started.
        let mut opening = serde_json::json!({
            "type": "takt.prompt",
            "step": context.step,
            "prompt": &prompt,
        })
        .to_string()
        .into_bytes();
        opening.push(b'\n');
        *self.transcript.borrow_mut() = Some(opening.clone());
        let mut command = Command::new(&self.program);
        // The stream, not the one result document: every message and tool call for the
        // transcript, then the result event as its last line. Print mode streams only with
        // `--verbose`.
        command
            .arg("-p")
            .arg(&prompt)
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--permission-mode")
            .arg("acceptEdits")
            // The tools the model is offered are the tools it may use: a tool offered and never
            // allowed costs its schema in every call's prompt. Claude Code 2.1.303 has no
            // MultiEdit, so it is neither offered nor allowed.
            .arg("--tools")
            .arg(Self::TOOLS)
            .arg("--allowedTools")
            .arg(Self::TOOLS)
            .arg("--disallowedTools")
            .arg("Bash,WebFetch,WebSearch,Agent,Task,NotebookEdit")
            .arg("--max-turns")
            .arg(self.max_turns.to_string())
            .arg("--no-session-persistence")
            .args(Self::ISOLATION)
            .current_dir(context.tree);
        if let Some(model) = &self.model {
            command.arg("--model").arg(model);
        }
        // The step runs on the user's claude.ai login: an API key in the environment would take
        // precedence and spend, and a parent Claude Code session's variables would make the child
        // a nested session. No other credential reaches it either: what the model does in the
        // tree needs none.
        command.env_remove("CLAUDECODE");
        command.env_remove("CLAUDE_PID");
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("CLAUDE_CODE_") {
                command.env_remove(name);
            }
        }
        crate::forge::without_credentials(&mut command, |_| false);
        let mut limits = self.limits;
        if let Some(deadline) = deadline {
            limits.timeout = limits
                .timeout
                .min(deadline.saturating_duration_since(std::time::Instant::now()));
        }
        // The tail is the reply read below; the head is kept for the transcript only.
        let ran = crate::forge::run_child_keeping_head(&mut command, limits, TRANSCRIPT_CAP / 2);
        let printed = match &ran {
            Ok(output) => Some(output),
            Err(error) => error
                .downcast_ref::<crate::forge::TimedOut>()
                .map(|timed_out| &timed_out.output),
        };
        *self.transcript.borrow_mut() = Some(Self::transcript(&opening, printed));
        let output = ran.with_context(|| format!("the harness {}", self.program))?;
        if !output.success() {
            bail!(
                "{} exited {}: {}",
                self.program,
                output
                    .code
                    .map_or("on a signal".to_owned(), |code| code.to_string()),
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(400)
                    .collect::<String>()
            );
        }
        let Some(last) = Self::last_line(&output.stdout, output.stdout_cut) else {
            bail!(
                "{} printed more than {} bytes; its reply was cut and is not read",
                self.program,
                self.limits.output
            );
        };
        let doc: serde_json::Value =
            serde_json::from_slice(last).context("read the harness's JSON output")?;
        let result = doc["result"].as_str().unwrap_or("");
        let usage = doc["usage"].as_object().map(|usage| {
            let component = |key: &str| {
                usage
                    .get(key)
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
            };
            crate::events::TokenUsage {
                input: component("input_tokens"),
                output: component("output_tokens"),
                cache_read: component("cache_read_input_tokens"),
                cache_creation: component("cache_creation_input_tokens"),
            }
        });
        let usage = usage.filter(|usage| usage.total() > 0);
        let tokens = usage.map(|usage| usage.total());
        let call = Self::call(&doc, started_at);
        let (action, summary) = Self::parse(result)?;
        self.reported.set(usage);
        *self.call.borrow_mut() = Some(call);
        Ok(StepReport {
            action,
            summary,
            tokens,
        })
    }
}

impl ClaudeCode {
    /// What the result event of `claude --output-format stream-json` (the document
    /// `--output-format json` prints) stated about the call, each field absent when the
    /// result does not state it: `modelUsage`'s keys as the model (several joined with `+`),
    /// `total_cost_usd`, `duration_ms`, `duration_api_ms` and `num_turns`; `started_at` is the
    /// instant takt started the child.
    fn call(doc: &serde_json::Value, started_at: String) -> CallReport {
        let model = doc["modelUsage"].as_object().and_then(|models| {
            let names: Vec<&str> = models.keys().map(String::as_str).collect();
            (!names.is_empty()).then(|| names.join("+"))
        });
        CallReport {
            model,
            cost_usd: doc["total_cost_usd"].as_f64(),
            duration_ms: doc["duration_ms"].as_u64(),
            api_ms: doc["duration_api_ms"].as_u64(),
            turns: doc["num_turns"].as_u64(),
            started_at: Some(started_at),
        }
    }
}

/// Whether `path` is a test path, by the same patterns the prompt names.
#[must_use]
pub fn is_test_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let Some(file) = parts.last() else {
        return false;
    };
    if parts[..parts.len() - 1]
        .iter()
        .any(|dir| *dir == "tests" || *dir == "test")
    {
        return true;
    }
    let stem = file.rsplit_once('.').map_or(*file, |(stem, _)| stem);
    stem.ends_with("_test")
        || stem.starts_with("test_")
        || stem.ends_with(".test")
        || stem.ends_with(".spec")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_follow_the_patterns() {
        for path in [
            "tests/a.rs",
            "crates/x/tests/b.rs",
            "test/c.py",
            "src/foo_test.go",
            "src/test_foo.py",
            "web/app.test.ts",
            "web/app.spec.ts",
            "tests/a b.rs",
            "tests/ü.rs",
        ] {
            assert!(is_test_path(path), "{path}");
        }
        // A path as git names it, whole: never with its first character cut off or quoted.
        for path in [
            "src/lib.rs",
            "Cargo.toml",
            "src/testing.rs",
            "tester/x.rs",
            "rc/lib.rs",
            "ests/a.rs",
            "\"tests/a b.rs\"",
            " src/lib.rs",
        ] {
            assert!(!is_test_path(path), "{path}");
        }
    }

    #[test]
    fn the_json_line_is_the_last_object_naming_an_action() {
        let text = "I edited the file.\n{\"action\": \"implementation.edit\", \"summary\": \"fixed add\"}\n";
        let (action, summary) = ClaudeCode::parse(text).expect("parses");
        assert_eq!(action, "implementation.edit");
        assert_eq!(summary, "fixed add");
        assert!(ClaudeCode::parse("no json here").is_err());
    }

    fn finished(head: &[u8], tail: &[u8], cut: bool, len: u64) -> crate::forge::Finished {
        crate::forge::Finished {
            code: Some(0),
            stdout: tail.to_vec(),
            stderr: Vec::new(),
            stdout_cut: cut,
            stdout_head: head.to_vec(),
            stdout_len: len,
        }
    }

    /// The ruling's cap: within it the transcript is the opening and the whole stream; past
    /// it, the head and the tail, each cut at a line end, and one line marking the cut that
    /// counts every byte left out, never more than the cap in all.
    #[test]
    fn a_transcript_past_its_cap_keeps_the_head_and_the_tail_marked_as_cut() {
        assert_eq!(
            bounded(b"p\n".to_vec(), 0, b"a\nb\n", 100),
            b"p\na\nb\n".to_vec()
        );
        let line = |c: char| format!("{}\n", c.to_string().repeat(9));
        let stream: String = ('a'..='z').map(line).collect();
        let cap = CUT_ROOM + 60;
        let kept = bounded(b"p\n".to_vec(), 0, stream.as_bytes(), cap);
        let text = String::from_utf8(kept.clone()).expect("text");
        assert!(kept.len() <= cap, "{} bytes: {text}", kept.len());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "p", "{text}");
        let marker = lines
            .iter()
            .position(|line| line.starts_with("{\"type\":\"takt.cut\""))
            .unwrap_or_else(|| panic!("a cut marked: {text}"));
        let doc: serde_json::Value = serde_json::from_str(lines[marker]).expect("a JSON line");
        let omitted = doc["omitted_bytes"].as_u64().expect("a count");
        let whole_lines = |part: &[&str]| part.iter().all(|line| line.len() == 9);
        assert!(whole_lines(&lines[1..marker]), "{text}");
        assert!(whole_lines(&lines[marker + 1..]), "{text}");
        assert_eq!(lines.last(), Some(&"zzzzzzzzz"), "{text}");
        assert_eq!(
            omitted as usize,
            2 + stream.len() - (text.len() - lines[marker].len() - 1),
            "every byte left out is counted: {text}"
        );
    }

    /// What the forge kept of a long stream: its head and its tail, overlapping when the stream
    /// is shorter than the two; the overlap is written once, and a gap is marked as cut.
    #[test]
    fn the_head_and_the_tail_the_forge_kept_make_the_stream_once() {
        let whole = ClaudeCode::transcript(b"p\n", Some(&finished(b"a\nb\n", b"b\nc\n", true, 6)));
        assert_eq!(whole, b"p\na\nb\nc\n".to_vec());
        let gap = ClaudeCode::transcript(b"p\n", Some(&finished(b"a\nb", b"y\nz\n", true, 12)));
        let text = String::from_utf8(gap).expect("text");
        assert_eq!(
            text, "p\na\n{\"type\":\"takt.cut\",\"omitted_bytes\":8}\nz\n",
            "the 5 bytes between, the head's cut line `b` and the tail's `y` line"
        );
        assert_eq!(ClaudeCode::transcript(b"p\n", None), b"p\n".to_vec());
        let short = ClaudeCode::transcript(b"p\n", Some(&finished(b"", b"r\n", false, 2)));
        assert_eq!(short, b"p\nr\n".to_vec());
    }

    /// The reply is read from the last line, the result event; a cut output whose last line
    /// has no line end before it is not read.
    #[test]
    fn the_reply_is_the_last_line_and_a_cut_one_is_not_read() {
        assert_eq!(
            ClaudeCode::last_line(b"{\"a\":1}\n{\"b\":2}\n\n", false),
            Some(&b"{\"b\":2}"[..])
        );
        assert_eq!(
            ClaudeCode::last_line(b"{\"b\":2}", false),
            Some(&b"{\"b\":2}"[..])
        );
        assert_eq!(
            ClaudeCode::last_line(b"t\":1}\n{\"b\":2}\n", true),
            Some(&b"{\"b\":2}"[..])
        );
        assert_eq!(ClaudeCode::last_line(b"\"b\":2}", true), None);
    }
}
