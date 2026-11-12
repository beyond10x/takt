//! Command actions (`takt.work.CommandAction`): a protocol action the run file's
//! `command_actions` binds to an argument list, performed by one generic validator,
//! [`CommandAction`]. The argv runs without a shell, in the tree, with no credential
//! ([`crate::forge::without_credentials`], through [`crate::forge::Process`]), each `{artifacts}`
//! in it the case's artifacts directory; its one output file is the declared `writes`, kept under
//! `artifacts/`, and its exit gives the one record's result
//! (`takt.work.CommandActionResult`): `pass` on 0, `unchanged` on `unchanged_exit`, `done` on
//! `done_exit`, `fail` on any other.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, Result};

use crate::evidence::Record;
use crate::forge::{Limits, Process, Runner as _};
use crate::validator::{Observation, Validator};

/// The placeholder an argv word holds for the case's artifacts directory.
pub const ARTIFACTS: &str = "{artifacts}";

/// The member of the run file that binds command actions.
pub const MEMBER: &str = "command_actions";

/// One binding of the run file's `command_actions`: the action and what performs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The protocol action's id, the binding's key.
    pub action: String,
    /// The argument list, run without a shell.
    pub argv: Vec<String>,
    /// The one file it writes, relative to the case's `artifacts/`.
    pub writes: PathBuf,
    /// The evidence kind of its record.
    pub evidence: String,
    /// The exit that gives `unchanged`.
    pub unchanged_exit: Option<i32>,
    /// The exit that gives `done`.
    pub done_exit: Option<i32>,
    /// The line the binding's key is written on, from 1; set by the reader of the file.
    pub line: usize,
}

/// The run file's `command_actions` as read: the file and its bindings, in the file's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandActions {
    /// The run file.
    pub file: PathBuf,
    /// The bindings.
    pub bindings: Vec<Binding>,
}

impl CommandActions {
    /// One validator per binding, each bound by the file at its line
    /// ([`crate::validator::RunFileBound`]).
    #[must_use]
    pub fn validators(&self) -> Vec<crate::validator::RunFileBound> {
        self.bindings
            .iter()
            .map(|binding| {
                crate::validator::RunFileBound::new(
                    Box::new(CommandAction::new(binding.clone())),
                    crate::validator::RunFileBinding {
                        file: self.file.clone(),
                        member: MEMBER,
                        line: binding.line,
                    },
                )
            })
            .collect()
    }
}

/// Why a `command_actions` member does not read: the keys from the top of the file to what is
/// refused, so the reader can name its line, and the rule it breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The keys from `command_actions` down to what is refused.
    pub keys: Vec<String>,
    /// The rule it breaks, worded after the member's name.
    pub rule: &'static str,
}

/// The rule of the member's shape.
const SHAPE_RULE: &str = "is a mapping of a protocol action's id to `argv`, an argument list, `writes`, a relative path under artifacts/, `evidence`, an evidence kind, and optional `unchanged_exit` and `done_exit`, exit codes from 1 to 255, not the same one";

/// The rule a binding of a built-in validator's action breaks.
const BUILT_IN_RULE: &str = "binds an action takt's own validators perform";

/// The keys a binding may hold.
const KEYS: [&str; 5] = ["argv", "writes", "evidence", "unchanged_exit", "done_exit"];

/// The bindings `member`, the run file's `command_actions` as written, holds, in the file's
/// order, each `line` 0 for the reader to set; or what is refused in it.
///
/// # Errors
///
/// A [`Refusal`] naming the keys down to what does not read.
pub fn read(member: &serde_yaml_ng::Value) -> std::result::Result<Vec<Binding>, Refusal> {
    let refused = |keys: &[&str], rule| Refusal {
        keys: std::iter::once(MEMBER)
            .chain(keys.iter().copied())
            .map(str::to_owned)
            .collect(),
        rule,
    };
    let Some(mapping) = member.as_mapping() else {
        return Err(refused(&[], SHAPE_RULE));
    };
    let mut bindings = Vec::new();
    for (action, entry) in mapping {
        let Some(action) = action.as_str().filter(|action| !action.trim().is_empty()) else {
            return Err(refused(&[], SHAPE_RULE));
        };
        let at = |key: &str| refused(&[action, key], SHAPE_RULE);
        let Some(entry) = entry.as_mapping() else {
            return Err(refused(&[action], SHAPE_RULE));
        };
        if crate::validator::is_built_in_action(action) {
            return Err(refused(&[action], BUILT_IN_RULE));
        }
        for key in entry.keys() {
            match key.as_str() {
                Some(key) if KEYS.contains(&key) => {}
                Some(key) => return Err(at(key)),
                None => return Err(refused(&[action], SHAPE_RULE)),
            }
        }
        let argv: Vec<String> = match entry.get("argv").and_then(|argv| argv.as_sequence()) {
            Some(words) => words
                .iter()
                .map(|word| word.as_str().map(str::to_owned))
                .collect::<Option<_>>()
                .ok_or_else(|| at("argv"))?,
            None => return Err(at("argv")),
        };
        if argv.first().is_none_or(|program| program.trim().is_empty()) {
            return Err(at("argv"));
        }
        let writes = entry
            .get("writes")
            .and_then(|writes| writes.as_str())
            .map(PathBuf::from)
            .filter(|writes| relative(writes))
            .ok_or_else(|| at("writes"))?;
        let evidence = entry
            .get("evidence")
            .and_then(|evidence| evidence.as_str())
            .filter(|evidence| !evidence.trim().is_empty())
            .ok_or_else(|| at("evidence"))?
            .to_owned();
        let exit = |key: &str| -> std::result::Result<Option<i32>, Refusal> {
            match entry.get(key) {
                None => Ok(None),
                Some(value) => value
                    .as_u64()
                    .filter(|code| (1..=255).contains(code))
                    .and_then(|code| i32::try_from(code).ok())
                    .map(Some)
                    .ok_or_else(|| at(key)),
            }
        };
        let unchanged_exit = exit("unchanged_exit")?;
        let done_exit = exit("done_exit")?;
        if unchanged_exit.is_some() && unchanged_exit == done_exit {
            return Err(at("done_exit"));
        }
        bindings.push(Binding {
            action: action.to_owned(),
            argv,
            writes,
            evidence,
            unchanged_exit,
            done_exit,
            line: 0,
        });
    }
    Ok(bindings)
}

/// Whether `path` is relative with no empty, `.` or `..` part: a file under `artifacts/`.
fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.to_string_lossy().ends_with('/')
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && !path.to_string_lossy().split('/').any(str::is_empty)
}

/// The generic validator of a command action: performs its [`Binding`].
#[derive(Debug, Clone)]
pub struct CommandAction {
    binding: Binding,
    limits: Limits,
}

impl CommandAction {
    /// The producer of its records.
    pub const NAME: &str = "takt.command-action";

    /// The validator performing `binding`, held to a forge program's limits
    /// ([`Process::LIMITS`]).
    #[must_use]
    pub fn new(binding: Binding) -> Self {
        Self {
            binding,
            limits: Process::LIMITS,
        }
    }

    /// The result of an exit `code` (`None` for a signal) under the binding.
    #[must_use]
    pub fn result_of(&self, code: Option<i32>) -> &'static str {
        match code {
            Some(0) => "pass",
            Some(code) if Some(code) == self.binding.unchanged_exit => "unchanged",
            Some(code) if Some(code) == self.binding.done_exit => "done",
            _ => "fail",
        }
    }

    /// The argv with each `{artifacts}` replaced by `artifacts`.
    fn argv_in(&self, artifacts: &Path) -> Vec<String> {
        let artifacts = artifacts.display().to_string();
        self.binding
            .argv
            .iter()
            .map(|word| word.replace(ARTIFACTS, &artifacts))
            .collect()
    }
}

impl Validator for CommandAction {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn action(&self) -> &str {
        &self.binding.action
    }

    fn kind(&self) -> &str {
        &self.binding.evidence
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let case = crate::case::current(tree)?;
        let artifacts = case.join("artifacts");
        let file = artifacts.join(&self.binding.writes);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let argv = self.argv_in(&artifacts);
        let exit = Process {
            limits: self.limits,
        }
        .run(tree, &argv)?;
        let result = self.result_of(exit.code);
        let code = exit
            .code
            .map_or_else(|| "a signal".to_owned(), |code| format!("exit {code}"));
        let record = Record::new(
            format!("command-action-{seq}"),
            self.kind(),
            Some(result.to_owned()),
            self.subject(),
            revision,
            self.name(),
        );
        Ok(Observation {
            summary: format!("{}: {} ended {code}: {result}", self.action(), argv[0]),
            records: vec![record],
            artifact: None,
        })
    }

    fn writes(&self) -> Vec<PathBuf> {
        vec![Path::new("artifacts").join(&self.binding.writes)]
    }

    fn runs(&self) -> Vec<String> {
        self.binding.argv.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(text: &str) -> serde_yaml_ng::Value {
        serde_yaml_ng::from_str(text).expect("YAML")
    }

    #[test]
    fn a_binding_reads_with_its_exits() {
        let bindings = read(&member(
            "status.render:\n  argv: [render, \"{artifacts}\"]\n  writes: status/publish.json\n  evidence: status.rendered\n  unchanged_exit: 3\n  done_exit: 4\n",
        ))
        .expect("reads");
        assert_eq!(bindings.len(), 1);
        let binding = &bindings[0];
        assert_eq!(binding.action, "status.render");
        assert_eq!(binding.writes, PathBuf::from("status/publish.json"));
        assert_eq!(binding.unchanged_exit, Some(3));
        let action = CommandAction::new(binding.clone());
        assert_eq!(action.result_of(Some(0)), "pass");
        assert_eq!(action.result_of(Some(3)), "unchanged");
        assert_eq!(action.result_of(Some(4)), "done");
        assert_eq!(action.result_of(Some(1)), "fail");
        assert_eq!(action.result_of(None), "fail");
        assert_eq!(
            action.argv_in(Path::new("/c/artifacts")),
            ["render", "/c/artifacts"]
        );
        assert_eq!(
            action.writes(),
            [PathBuf::from("artifacts/status/publish.json")]
        );
    }

    #[test]
    fn a_binding_that_does_not_read_names_its_keys() {
        for (text, keys, rule) in [
            ("[a]", vec!["command_actions"], SHAPE_RULE),
            ("a: [x]", vec!["command_actions", "a"], SHAPE_RULE),
            (
                "a: {argv: [], writes: f, evidence: k}",
                vec!["command_actions", "a", "argv"],
                SHAPE_RULE,
            ),
            (
                "a: {argv: [x], writes: ../f, evidence: k}",
                vec!["command_actions", "a", "writes"],
                SHAPE_RULE,
            ),
            (
                "a: {argv: [x], writes: /f, evidence: k}",
                vec!["command_actions", "a", "writes"],
                SHAPE_RULE,
            ),
            (
                "a: {argv: [x], writes: f, evidence: k, shell: true}",
                vec!["command_actions", "a", "shell"],
                SHAPE_RULE,
            ),
            (
                "a: {argv: [x], writes: f, evidence: k, done_exit: 0}",
                vec!["command_actions", "a", "done_exit"],
                SHAPE_RULE,
            ),
            (
                "a: {argv: [x], writes: f, evidence: k, unchanged_exit: 3, done_exit: 3}",
                vec!["command_actions", "a", "done_exit"],
                SHAPE_RULE,
            ),
            (
                "tests.run: {argv: [x], writes: f, evidence: k}",
                vec!["command_actions", "tests.run"],
                BUILT_IN_RULE,
            ),
        ] {
            let refusal = read(&member(text)).expect_err(text);
            assert_eq!(refusal.keys, keys, "{text}");
            assert_eq!(refusal.rule, rule, "{text}");
        }
    }
}
