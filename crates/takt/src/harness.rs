//! The judgement actor: a harness performs one admissible action in the worktree and reports
//! which. The first adapter is Claude Code (`claude -p`), given file tools only: it reads and
//! edits; takt runs the tests and writes the evidence. It runs on the user's claude.ai login:
//! an `ANTHROPIC_API_KEY` in the environment is dropped, never spent.

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
}

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
        let mut command = Command::new(&self.program);
        command
            .arg("-p")
            .arg(&prompt)
            .arg("--output-format")
            .arg("json")
            .arg("--permission-mode")
            .arg("acceptEdits")
            .arg("--allowedTools")
            .arg("Read,Edit,Write,Glob,Grep,MultiEdit")
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
        let output = crate::forge::run_child(&mut command, limits)
            .with_context(|| format!("the harness {}", self.program))?;
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
        if output.stdout_cut {
            bail!(
                "{} printed more than {} bytes; its reply was cut and is not read",
                self.program,
                self.limits.output
            );
        }
        let doc: serde_json::Value =
            serde_json::from_slice(&output.stdout).context("read the harness's JSON output")?;
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
    /// What `claude --output-format json` stated about the call, each field absent when the
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
}
