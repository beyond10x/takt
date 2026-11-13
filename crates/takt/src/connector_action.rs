//! Connector actions (`takt.work.ConnectorAction`): a protocol action with `effect: read` the run
//! file's `connector_actions` binds to one operation of the served Connectors instance its
//! `connectors` names (`takt.work.ConnectorsEndpoint`), performed by one generic validator,
//! [`ConnectorAction`], through the one client ([`crate::connectors_client::Client`]). Before any
//! case opens, one describe must describe the instance and list every bound operation
//! ([`ConnectorActions::preflight`]); each observation is one invoke at the described revision.
//! The answer is kept compact as the observation's artifact `artifacts/connector/<action>.json`,
//! never in the record, which is `canon-evidence/1` of the bound kind: `pass` when the instance
//! answered success, `fail` only when the instance refused the read ([`refuses_the_read`]);
//! any other answer is an error of the observation, no record. The call is held to what is left
//! of the run's hours. No credential goes with a call.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::Value;

use crate::connectors_client::{Client, ServedRefused};
use crate::evidence::Record;
use crate::validator::{Artifact, Observation, Validator};

/// The member of the run file that binds connector actions.
pub const MEMBER: &str = "connector_actions";

/// The member of the run file that names the served instance.
pub const CONNECTORS: &str = "connectors";

/// The directory under the case's `artifacts/` the answers are kept in.
pub const ANSWERS: &str = "connector";

/// The served instance the run file's `connectors` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connectors {
    /// The URL of the served instance, checked by [`crate::connectors_client::check_endpoint`].
    pub endpoint: String,
    /// `<connector>[/<instance>]`.
    pub instance: String,
}

impl Connectors {
    /// The instance's own name, which the describe must report: what follows `/`, `None` when
    /// `instance` names the connector alone.
    #[must_use]
    pub fn described_instance(&self) -> Option<&str> {
        self.instance.split_once('/').map(|(_, instance)| instance)
    }
}

/// One binding of the run file's `connector_actions`: the action and what performs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// The protocol action's id, the binding's key.
    pub action: String,
    /// The operation it invokes.
    pub operation: String,
    /// The operation's input.
    pub input: Value,
    /// The evidence kind of its record.
    pub evidence: String,
    /// The line the binding's key is written on, from 1; set by the reader of the file.
    pub line: usize,
}

/// The run file's `connectors` and `connector_actions` as read: the file, the instance and the
/// bindings, in the file's order.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorActions {
    /// The run file.
    pub file: PathBuf,
    /// The served instance.
    pub connectors: Connectors,
    /// The bindings.
    pub bindings: Vec<Binding>,
}

impl ConnectorActions {
    /// The one `GET /v1/describe` of the run, before any case opens: the instance must be the
    /// one `connectors.instance` names and list every bound operation, else an error naming each
    /// one it does not list. The descriptor revision every invocation names.
    pub fn preflight(&self) -> Result<String> {
        let client = Client::at(&self.connectors.endpoint)
            .map_err(|rule| anyhow!("`connectors.endpoint` {rule}; no case was opened"))?;
        let description = client.describe(
            self.connectors.described_instance(),
            "`connectors.instance`",
            "no case was opened",
        )?;
        let missing: Vec<String> = self
            .bindings
            .iter()
            .filter(|binding| !description.operations.contains(&binding.operation))
            .map(|binding| {
                format!(
                    "{} (`{MEMBER}` {}, line {})",
                    binding.operation, binding.action, binding.line
                )
            })
            .collect();
        if !missing.is_empty() {
            bail!(
                "the instance served at {} does not describe {}, which the run configuration file {} binds; no case was opened",
                client.endpoint(),
                missing.join(", "),
                self.file.display()
            );
        }
        Ok(description.revision)
    }

    /// One validator per binding, each bound by the file at its line
    /// ([`crate::validator::RunFileBound`]), each invoking at `revision`, the one
    /// [`Self::preflight`] found, or describing once itself when it is `None`.
    #[must_use]
    pub fn validators(&self, revision: Option<&str>) -> Vec<crate::validator::RunFileBound> {
        self.bindings
            .iter()
            .map(|binding| {
                let bound = crate::validator::RunFileBinding {
                    file: self.file.clone(),
                    member: MEMBER,
                    line: binding.line,
                };
                crate::validator::RunFileBound::new(
                    Box::new(ConnectorAction {
                        binding: binding.clone(),
                        connectors: self.connectors.clone(),
                        bound: bound.clone(),
                        revision: std::cell::RefCell::new(revision.map(str::to_owned)),
                    }),
                    bound,
                )
            })
            .collect()
    }
}

/// Why a `connectors` or `connector_actions` member does not read: the keys from the top of the
/// file to what is refused, so the reader can name its line, the member named and the rule it
/// breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The keys from the member down to what is refused.
    pub keys: Vec<String>,
    /// The member as the refusal names it.
    pub member: &'static str,
    /// The rule it breaks, worded after the member's name.
    pub rule: &'static str,
}

/// The rule of `connectors`' shape.
const CONNECTORS_RULE: &str = "is a mapping of `endpoint`, the URL of a served Connectors instance, and `instance`, `<connector>[/<instance>]`";

/// The rule of `connector_actions`' shape.
const SHAPE_RULE: &str = "is a mapping of a protocol action's id, a plain file name, to `operation`, the operation it invokes, `input`, a mapping, and `evidence`, an evidence kind";

/// The code of the served host's answer to an invocation at a revision it no longer describes:
/// no refusal of the read, the descriptor moved.
const STALE_DESCRIPTION: &str = "stale_description";

/// Whether `refusal` is the instance's own refusal of the read: a v1alpha2 error envelope
/// ([`crate::connectors_client::is_error_envelope`]) whose code is not [`STALE_DESCRIPTION`].
/// A gateway's page, a redirect, or any other answer that is no envelope is not.
fn refuses_the_read(refusal: &ServedRefused) -> bool {
    refusal.envelope && refusal.code.as_deref() != Some(STALE_DESCRIPTION)
}

/// The rule an action `command_actions` binds too breaks, at the `connector_actions` binding.
pub const BOUND_BY_COMMAND_RULE: &str =
    "binds an action `command_actions` binds too; an action is bound in one member only";

/// The rule an action `connector_actions` binds too breaks, at the `command_actions` binding.
pub const BOUND_BY_CONNECTOR_RULE: &str =
    "binds an action `connector_actions` binds too; an action is bound in one member only";

/// The rule a binding of a built-in validator's action breaks.
const BUILT_IN_RULE: &str = "binds an action takt's own validators perform";

/// The rule a binding with no `connectors` breaks.
const NO_INSTANCE_RULE: &str =
    "binds an operation, and the run file's `connectors` names no instance to invoke it on";

/// The rule a binding of an action the case's protocol declares `effect: write` breaks.
pub const WRITE_RULE: &str = "binds an action the case's protocol declares `effect: write`, and a connector action performs a read only";

/// The keys a binding may hold.
const KEYS: [&str; 3] = ["operation", "input", "evidence"];

/// Whether `word` is a word of `connectors.instance`: letters, digits, `.`, `_` and `-`, not
/// starting with `-`.
fn instance_word(word: &str) -> bool {
    crate::forge::connectors::is_instance_name(word)
}

/// The instance `member`, the run file's `connectors` as written, names; or what is refused in
/// it, the endpoint refused with the rule [`crate::connectors_client::check_endpoint`] names.
///
/// # Errors
///
/// A [`Refusal`] naming the keys down to what does not read.
pub fn read_connectors(member: &serde_yaml_ng::Value) -> std::result::Result<Connectors, Refusal> {
    let refused = |keys: &[&str], member, rule| Refusal {
        keys: std::iter::once(CONNECTORS)
            .chain(keys.iter().copied())
            .map(str::to_owned)
            .collect(),
        member,
        rule,
    };
    let Some(mapping) = member.as_mapping() else {
        return Err(refused(&[], CONNECTORS, CONNECTORS_RULE));
    };
    for key in mapping.keys() {
        match key.as_str() {
            Some("endpoint" | "instance") => {}
            Some(key) => return Err(refused(&[key], CONNECTORS, CONNECTORS_RULE)),
            None => return Err(refused(&[], CONNECTORS, CONNECTORS_RULE)),
        }
    }
    let Some(endpoint) = mapping.get("endpoint").and_then(|value| value.as_str()) else {
        return Err(refused(&["endpoint"], CONNECTORS, CONNECTORS_RULE));
    };
    if let Err(rule) = crate::connectors_client::check_endpoint(endpoint) {
        return Err(refused(&["endpoint"], "connectors.endpoint", rule));
    }
    let instance = mapping
        .get("instance")
        .and_then(|value| value.as_str())
        .filter(|instance| {
            let (connector, name) = instance
                .split_once('/')
                .map_or((*instance, None), |(connector, name)| {
                    (connector, Some(name))
                });
            instance_word(connector) && name.is_none_or(instance_word)
        })
        .ok_or_else(|| refused(&["instance"], CONNECTORS, CONNECTORS_RULE))?;
    Ok(Connectors {
        endpoint: endpoint.to_owned(),
        instance: instance.to_owned(),
    })
}

/// The bindings `member`, the run file's `connector_actions` as written, holds, in the file's
/// order, each `line` 0 for the reader to set; or what is refused in it. A binding with no
/// `connectors` (`has_instance` false) is refused.
///
/// # Errors
///
/// A [`Refusal`] naming the keys down to what does not read.
pub fn read(
    member: &serde_yaml_ng::Value,
    has_instance: bool,
) -> std::result::Result<Vec<Binding>, Refusal> {
    let refused = |keys: &[&str], rule| Refusal {
        keys: std::iter::once(MEMBER)
            .chain(keys.iter().copied())
            .map(str::to_owned)
            .collect(),
        member: MEMBER,
        rule,
    };
    let Some(mapping) = member.as_mapping() else {
        return Err(refused(&[], SHAPE_RULE));
    };
    let mut bindings = Vec::new();
    for (action, entry) in mapping {
        let Some(action) = action
            .as_str()
            .filter(|action| !action.trim().is_empty())
            .filter(|action| !action.starts_with('.') && !action.contains(['/', '\0']))
        else {
            return Err(refused(&[], SHAPE_RULE));
        };
        let at = |key: &str| refused(&[action, key], SHAPE_RULE);
        let Some(entry) = entry.as_mapping() else {
            return Err(refused(&[action], SHAPE_RULE));
        };
        if crate::validator::is_built_in_action(action) {
            return Err(refused(&[action], BUILT_IN_RULE));
        }
        if !has_instance {
            return Err(refused(&[action], NO_INSTANCE_RULE));
        }
        for key in entry.keys() {
            match key.as_str() {
                Some(key) if KEYS.contains(&key) => {}
                Some(key) => return Err(at(key)),
                None => return Err(refused(&[action], SHAPE_RULE)),
            }
        }
        let text = |key: &str| {
            entry
                .get(key)
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| at(key))
        };
        let operation = text("operation")?;
        let evidence = text("evidence")?;
        let input = entry
            .get("input")
            .filter(|input| input.is_mapping())
            .and_then(|input| serde_json::to_value(input).ok())
            .filter(Value::is_object)
            .ok_or_else(|| at("input"))?;
        bindings.push(Binding {
            action: action.to_owned(),
            operation,
            input,
            evidence,
            line: 0,
        });
    }
    Ok(bindings)
}

/// Whether the protocol the case at `case_dir` pinned declares `action` `effect: write`. A case
/// with no pinned protocol file declares nothing.
fn declared_write(case_dir: &Path, action: &str) -> Result<bool> {
    let path = case_dir.join("protocol.yaml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    };
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    Ok(doc["actions"][action]["effect"].as_str() == Some("write"))
}

/// The generic validator of a connector action: performs its [`Binding`].
#[derive(Debug)]
pub struct ConnectorAction {
    binding: Binding,
    connectors: Connectors,
    bound: crate::validator::RunFileBinding,
    /// The descriptor revision, once described.
    revision: std::cell::RefCell<Option<String>>,
}

impl ConnectorAction {
    /// The producer of its records.
    pub const NAME: &str = "takt.connector-action";

    /// The file name of the answer under the case's `artifacts/`.
    fn answer_name(&self) -> String {
        format!("{ANSWERS}/{}.json", self.binding.action)
    }

    /// Performs the binding, refusing to call when `deadline` has passed and holding the call to
    /// what is left before it.
    fn observe_by(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<Observation> {
        let case = crate::case::current(tree)?;
        // Whoever calls it, a connector action records no kind its action may not produce in
        // the protocol the case pinned, and calls nothing for an action declared a write.
        let produced = crate::validator::produces_kind(&case, self.action(), self.kind());
        if !matches!(produced, Ok(true)) {
            let why = produced.map_or_else(
                |error| format!("{error:#}"),
                |_| {
                    "the case's protocol does not list it among what the action may produce"
                        .to_owned()
                },
            );
            bail!(
                "{}: no `{}` record is made, nothing is invoked: {why}",
                self.action(),
                self.kind()
            );
        }
        if declared_write(&case, self.action())? {
            bail!(
                "{}: nothing is invoked: the case's protocol declares it `effect: write`",
                self.action()
            );
        }
        let artifacts = case.join("artifacts");
        let answers = artifacts.join(ANSWERS);
        for dir in [&artifacts, &answers] {
            match std::fs::symlink_metadata(dir) {
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => bail!(
                    "{}: {} is no directory of the case's own; nothing is invoked",
                    self.action(),
                    dir.display()
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(dir)
                        .with_context(|| format!("create {}", dir.display()))?;
                }
                Err(error) => {
                    return Err(error).with_context(|| format!("read {}", dir.display()));
                }
            }
        }
        if std::fs::symlink_metadata(artifacts.join(self.answer_name()))
            .is_ok_and(|meta| !meta.is_file())
        {
            bail!(
                "{}: artifacts/{} is no regular file of the case's own; nothing is invoked",
                self.action(),
                self.answer_name()
            );
        }
        let client = Client::at(&self.connectors.endpoint)
            .map_err(|rule| anyhow!("`connectors.endpoint` {rule}; nothing is invoked"))?;
        let described = self.revision.borrow().clone();
        let described = match described {
            Some(described) => described,
            None => {
                let description = client.describe(
                    self.connectors.described_instance(),
                    "`connectors.instance`",
                    "nothing is invoked",
                )?;
                if !description.operations.contains(&self.binding.operation) {
                    bail!(
                        "{}: the instance served at {} does not describe {}; nothing is invoked",
                        self.action(),
                        client.endpoint(),
                        self.binding.operation
                    );
                }
                *self.revision.borrow_mut() = Some(description.revision.clone());
                description.revision
            }
        };
        if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
            bail!(
                "{}: the run's end came first; nothing is invoked",
                self.action()
            );
        }
        let within = deadline.map_or(std::time::Duration::MAX, |deadline| {
            deadline.saturating_duration_since(std::time::Instant::now())
        });
        let (result, kept, said) = match client.answer_within(
            &described,
            &self.binding.operation,
            &self.binding.input,
            None,
            within,
        ) {
            Ok(answer) => ("pass", answer, "answered".to_owned()),
            Err(error) => match error.downcast_ref::<ServedRefused>() {
                Some(refusal) if refuses_the_read(refusal) => (
                    "fail",
                    serde_json::json!({
                        "status": "error",
                        "http_status": refusal.status,
                        "code": refusal.code,
                        "message": refusal.message,
                    }),
                    format!("refused: {refusal}"),
                ),
                _ => {
                    return Err(error.context(format!(
                        "{}: no refusal of the read by the instance; no record is made, and the next run describes again",
                        self.action()
                    )));
                }
            },
        };
        let record = Record::new(
            format!("connector-action-{seq}"),
            self.kind(),
            Some(result.to_owned()),
            self.subject(),
            revision,
            self.name(),
        );
        Ok(Observation {
            summary: format!(
                "{}: {} {said}: {result}",
                self.action(),
                client.named(&self.binding.operation)
            ),
            records: vec![record],
            artifact: Some(Artifact {
                name: self.answer_name(),
                bytes: serde_json::to_vec(&kept)?,
            }),
        })
    }
}

impl Validator for ConnectorAction {
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
        self.observe_by(tree, revision, seq, None)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = self.observe_by(tree, revision, seq, deadline);
        Ok((
            crate::validator::ended_by(deadline, self.action(), observed)?,
            false,
        ))
    }

    /// A binding of an action the case's protocol declares `effect: write` is refused at its
    /// line, the case discarded.
    fn check_at_open(&self, _tree: &Path, case_dir: &Path) -> Result<()> {
        if declared_write(case_dir, self.action())? {
            return Err(anyhow::Error::new(crate::run_config::MemberRefused {
                path: self.bound.file.clone(),
                line: self.bound.line,
                member: self.bound.member,
                rule: WRITE_RULE,
            }));
        }
        Ok(())
    }

    fn check_at_resume(&self, tree: &Path, case_dir: &Path) -> Result<()> {
        self.check_at_open(tree, case_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(text: &str) -> serde_yaml_ng::Value {
        serde_yaml_ng::from_str(text).expect("YAML")
    }

    #[test]
    fn a_binding_reads() {
        let bindings = read(
            &member(
                "thread.read:\n  operation: conversations.replies\n  input: {channel: C0000000, ts: \"1700000000.000100\"}\n  evidence: thread_read\n",
            ),
            true,
        )
        .expect("reads");
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].operation, "conversations.replies");
        assert_eq!(bindings[0].input["ts"], "1700000000.000100");
        let connectors = read_connectors(&member(
            "endpoint: http://127.0.0.1:8402\ninstance: chat/team\n",
        ))
        .expect("reads");
        assert_eq!(connectors.described_instance(), Some("team"));
    }

    #[test]
    fn a_member_that_does_not_read_names_its_keys() {
        for (text, keys, rule) in [
            ("[a]", vec![MEMBER], SHAPE_RULE),
            (
                "a/b: {operation: o, input: {}, evidence: k}",
                vec![MEMBER],
                SHAPE_RULE,
            ),
            (
                "a: {operation: o, input: [x], evidence: k}",
                vec![MEMBER, "a", "input"],
                SHAPE_RULE,
            ),
            (
                "a: {operation: o, input: {}, evidence: k, x: 1}",
                vec![MEMBER, "a", "x"],
                SHAPE_RULE,
            ),
            (
                "tests.run: {operation: o, input: {}, evidence: k}",
                vec![MEMBER, "tests.run"],
                BUILT_IN_RULE,
            ),
        ] {
            let refusal = read(&member(text), true).expect_err(text);
            assert_eq!(refusal.keys, keys, "{text}");
            assert_eq!(refusal.rule, rule, "{text}");
        }
        let refusal = read(&member("a: {operation: o, input: {}, evidence: k}"), false)
            .expect_err("no instance");
        assert_eq!(refusal.rule, NO_INSTANCE_RULE);
        let refusal = read_connectors(&member("endpoint: http://example.com\ninstance: chat\n"))
            .expect_err("plain http to another host");
        assert_eq!(refusal.member, "connectors.endpoint");
        assert_eq!(refusal.keys, [CONNECTORS, "endpoint"]);
        let refusal = read_connectors(&member("endpoint: https://c.example\ninstance: a/b/c\n"))
            .expect_err("no instance");
        assert_eq!(refusal.keys, [CONNECTORS, "instance"]);
    }
}
