//! A protocol as data: a built-in of the engineering protocols or a `protocol/1` file,
//! validated and compiled by Canon. Nothing here models a protocol in Rust.

use std::path::Path;

use anyhow::{Context as _, Result, anyhow, bail};

/// A protocol Canon validated, with its compiled IR as canonical JSON.
#[derive(Debug, Clone)]
pub struct LoadedProtocol {
    /// `<name>/<major>`, for example `software.change/1`.
    pub id: String,
    /// The protocol name, `software.change`.
    pub name: String,
    /// The major revision.
    pub revision: u64,
    /// The `protocol/1` document, byte for byte.
    pub yaml: String,
    /// The compiled `canon-ir/1`, canonical JSON.
    pub ir_json: String,
    /// The declared artifact ids, in declaration order.
    pub artifacts: Vec<String>,
    /// The declared case inputs, in declaration order, each with its allowed values.
    pub inputs: Vec<(String, Vec<String>)>,
    /// The declared action ids, in declaration order.
    pub actions: Vec<String>,
}

impl LoadedProtocol {
    /// A built-in of the engineering protocols, by `name/major`: `software.change/1`,
    /// `story.delivery/1`, `self.improvement/1`, `loop.improvement/1` and the rest of the
    /// registry.
    pub fn builtin(id: &str) -> Result<Self> {
        let (name, major) = split_id(id)?;
        // The registry names a protocol by its directory, `software-change` or
        // `eval-gated-change`; the protocol's own id is `software.change` or
        // `eval.gated_change`. Both spellings are accepted. A name the registry knows under its
        // own spelling reports that spelling's refusal, so an unknown major is called one.
        let listed = name.replace(['.', '_'], "-");
        let builtin = canon_engineering::registry::get(name, major).or_else(|first| {
            if listed == name {
                Err(first)
            } else {
                canon_engineering::registry::get(&listed, major)
            }
        });
        let builtin = builtin.map_err(|error| anyhow!("protocol {id}: {error:?}"))?;
        Self::from_yaml(builtin.yaml)
    }

    /// What `--protocol` names: a `protocol/1` file when `spec` is the path of one, else a
    /// built-in by id.
    pub fn resolve(spec: &str) -> Result<Self> {
        let path = Path::new(spec);
        if path.is_file() {
            Self::from_file(path)
        } else {
            Self::builtin(spec)
        }
    }

    /// A `protocol/1` file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let yaml = std::fs::read_to_string(path)
            .with_context(|| format!("read the protocol {}", path.display()))?;
        Self::from_yaml(&yaml)
    }

    /// A `protocol/1` document, validated and compiled by Canon.
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let model = b10x_canon::model::parse(yaml).map_err(|error| anyhow!("{error}"))?;
        b10x_canon::validate::validate(&model)
            .map_err(|problems| anyhow!("the protocol is invalid: {problems:?}"))?;
        let ir = b10x_canon::ir::compile(&model)
            .map_err(|problems| anyhow!("the protocol does not compile: {problems:?}"))?;
        let doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(yaml).context("read the protocol document")?;
        let name = doc["protocol"]["id"]
            .as_str()
            .ok_or_else(|| anyhow!("the protocol document names no id"))?
            .to_owned();
        let revision = doc["protocol"]["revision"]
            .as_u64()
            .ok_or_else(|| anyhow!("the protocol document names no revision"))?;
        let keys = |section: &serde_yaml_ng::Value| -> Vec<String> {
            section
                .as_mapping()
                .map(|map| {
                    map.keys()
                        .filter_map(|key| key.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let inputs = doc["case"]["inputs"]
            .as_mapping()
            .map(|map| {
                map.iter()
                    .filter_map(|(key, value)| {
                        let values = value["values"]
                            .as_sequence()
                            .map(|seq| {
                                seq.iter()
                                    .map(|v| match v {
                                        serde_yaml_ng::Value::Bool(b) => b.to_string(),
                                        other => other.as_str().unwrap_or("").to_owned(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        key.as_str().map(|k| (k.to_owned(), values))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            id: format!("{name}/{revision}"),
            name,
            revision,
            yaml: yaml.to_owned(),
            ir_json: ir.canonical_json(),
            artifacts: keys(&doc["artifacts"]),
            inputs,
            actions: keys(&doc["actions"]),
        })
    }
}

impl LoadedProtocol {
    /// The artifact a grant of `capability` binds to: the subject of the evidence the
    /// precondition of each action requiring it names, directly or through the claims it names.
    /// Refused, naming the action, when an action's precondition names evidence about no
    /// artifact or about more than one, or when two actions requiring it are about different
    /// artifacts: a grant is never bound to a guess. A match (`evidence`, `first` or `before`)
    /// that names no subject reads records about any artifact, so it is about no single one. An
    /// action with no precondition is about no artifact, so a capability only it requires is
    /// refused even where a grant used to bind to `implementation` (`software.change/1`'s
    /// `release.rollback`): never binding to a guess outranks granting as before.
    pub fn grant_subject(&self, capability: &str) -> Result<String> {
        let doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&self.yaml).context("read the protocol document")?;
        let mut bound: Option<(String, String)> = None;
        for (name, action) in doc["actions"].as_mapping().into_iter().flatten() {
            let Some(name) = name.as_str() else { continue };
            let requires = action["requires"].as_sequence().is_some_and(|requires| {
                requires
                    .iter()
                    .any(|requirement| requirement["capability"].as_str() == Some(capability))
            });
            if !requires {
                continue;
            }
            let mut subjects = std::collections::BTreeSet::new();
            subjects_of(
                &doc["claims"],
                &action["precondition"],
                &mut std::collections::BTreeSet::new(),
                &mut subjects,
            );
            let mut subjects = subjects.into_iter();
            let (Some(Some(subject)), None) = (subjects.next(), subjects.next()) else {
                bail!(
                    "the action {name} of {} requires {capability}, and its precondition names evidence about no single artifact; a grant of {capability} binds to none",
                    self.id
                );
            };
            if let Some((other, theirs)) = &bound
                && *theirs != subject
            {
                bail!(
                    "the action {name} of {} is about {subject} and the action {other} about {theirs}, and both require {capability}; a grant of {capability} binds to neither",
                    self.id
                );
            }
            bound = Some((name.to_owned(), subject));
        }
        bound
            .map(|(_, subject)| subject)
            .ok_or_else(|| anyhow!("no action of {} requires {capability}", self.id))
    }
}

/// Adds to `subjects` the subject of every evidence match (`evidence`, `first`, `before`) in
/// `condition`, `None` for one that names none and so reads records about any artifact,
/// following each claim it names into that claim's `true_when` in `claims`, each claim once.
fn subjects_of(
    claims: &serde_yaml_ng::Value,
    condition: &serde_yaml_ng::Value,
    seen: &mut std::collections::BTreeSet<String>,
    subjects: &mut std::collections::BTreeSet<Option<String>>,
) {
    match condition {
        serde_yaml_ng::Value::Mapping(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    (Some("claim"), serde_yaml_ng::Value::String(claim)) => {
                        if seen.insert(claim.clone()) {
                            subjects_of(
                                claims,
                                &claims[claim.as_str()]["true_when"],
                                seen,
                                subjects,
                            );
                        }
                    }
                    (Some("evidence" | "first" | "before"), serde_yaml_ng::Value::Mapping(_)) => {
                        subjects.insert(value["subject"].as_str().map(str::to_owned));
                    }
                    _ => subjects_of(claims, value, seen, subjects),
                }
            }
        }
        serde_yaml_ng::Value::Sequence(items) => {
            for item in items {
                subjects_of(claims, item, seen, subjects);
            }
        }
        _ => {}
    }
}

/// Splits `name/major`.
fn split_id(id: &str) -> Result<(&str, u32)> {
    let (name, major) = id
        .rsplit_once('/')
        .ok_or_else(|| anyhow!("a protocol id is `<name>/<major>`, not `{id}`"))?;
    let major = major
        .parse()
        .with_context(|| format!("the major of `{id}` is not a number"))?;
    Ok((name, major))
}

impl LoadedProtocol {
    /// The approval gate a run on this protocol takes when it is given none: the first action,
    /// in declaration order, that requires a capability and whose precondition the case's
    /// `inputs` do not make false for good, an `input` test (alone or a term of an `all`) whose
    /// value is not among those it names. `repository.merge` for a delivery or a software
    /// change; `candidate.merge` or `candidate.merge_judge_surface` for an improvement turn, as
    /// its `surface` says. None when no action requires a capability.
    #[must_use]
    pub fn default_gate(&self, inputs: &[(String, String)]) -> Option<String> {
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&self.yaml).ok()?;
        let blocked = |test: &serde_yaml_ng::Value| -> bool {
            let (Some(input), Some(values)) = (test["input"].as_str(), test["in"].as_sequence())
            else {
                return false;
            };
            inputs
                .iter()
                .find(|(name, _)| name == input)
                .is_some_and(|(_, value)| {
                    !values.iter().any(|allowed| match allowed {
                        serde_yaml_ng::Value::Bool(b) => b.to_string() == *value,
                        other => other.as_str() == Some(value.as_str()),
                    })
                })
        };
        doc["actions"]
            .as_mapping()?
            .iter()
            .find(|(_, action)| {
                let requires = action["requires"].as_sequence().is_some_and(|requires| {
                    requires
                        .iter()
                        .any(|requirement| requirement["capability"].as_str().is_some())
                });
                let precondition = &action["precondition"];
                let never = blocked(precondition)
                    || precondition["all"]
                        .as_sequence()
                        .is_some_and(|terms| terms.iter().any(blocked));
                requires && !never
            })
            .and_then(|(name, _)| name.as_str().map(str::to_owned))
    }

    /// The built-in protocols, `(name, major)` each, as the engineering protocols' registry
    /// names them (`story-delivery`).
    #[must_use]
    pub fn builtins() -> Vec<(String, u32)> {
        canon_engineering::registry::list()
            .into_iter()
            .map(|(name, major)| (name.to_owned(), major))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_software_change_builtin_loads_and_compiles() {
        let protocol = LoadedProtocol::builtin("software.change/1").expect("a built-in");
        assert_eq!(protocol.name, "software.change");
        assert_eq!(protocol.revision, 1);
        assert!(protocol.artifacts.contains(&"implementation".to_owned()));
        assert!(protocol.actions.contains(&"tests.run".to_owned()));
        assert!(protocol.inputs.iter().any(|(k, _)| k == "risk"));
        assert!(protocol.ir_json.contains("canon-ir/1"));
    }

    /// Story delivery, self-improvement and loop improvement are the engineering protocols'
    /// built-ins: each answers to its own id and to the registry's name, byte for byte the
    /// registry's document, and is listed among the built-ins.
    #[test]
    fn the_three_protocols_are_the_registrys_builtins() {
        for (id, name, action) in [
            ("story.delivery/1", "story-delivery", "pr.open"),
            ("self.improvement/1", "self-improvement", "finding.file"),
            ("loop.improvement/1", "loop-improvement", "loop.measure"),
        ] {
            let registry = canon_engineering::registry::get(name, 1).expect("a registry built-in");
            for given in [id.to_owned(), format!("{name}/1")] {
                let protocol = LoadedProtocol::builtin(&given).expect("a built-in");
                assert_eq!(protocol.id, id);
                assert_eq!(protocol.yaml, registry.yaml);
                assert!(protocol.actions.contains(&action.to_owned()), "{id}");
            }
            assert!(
                LoadedProtocol::builtins()
                    .iter()
                    .any(|(listed, major)| listed == name && *major == 1),
                "{id}"
            );
        }
        assert!(LoadedProtocol::builtin("story.delivery/2").is_err());
    }

    /// A protocol given as an id or as a file resolves to the same protocol.
    #[test]
    fn resolve_takes_a_file_or_a_builtin() {
        let dir = std::env::temp_dir().join(format!("takt-resolve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a directory");
        let path = dir.join("story-delivery.yaml");
        std::fs::write(
            &path,
            canon_engineering::registry::get("story-delivery", 1)
                .expect("a registry built-in")
                .yaml,
        )
        .expect("the file");
        let file = LoadedProtocol::resolve(path.to_str().expect("a UTF-8 path")).expect("the file");
        let builtin = LoadedProtocol::resolve("story.delivery/1").expect("the built-in");
        std::fs::remove_dir_all(&dir).expect("removed");
        assert_eq!(file.id, builtin.id);
        assert_eq!(file.ir_json, builtin.ir_json);
        assert!(LoadedProtocol::resolve("no.such/1").is_err());
    }

    #[test]
    fn an_unknown_builtin_is_refused() {
        assert!(LoadedProtocol::builtin("no.such/1").is_err());
        assert!(LoadedProtocol::builtin("software.change").is_err());
    }
}
