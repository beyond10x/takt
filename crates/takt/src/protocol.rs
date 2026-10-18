//! A protocol as data: a built-in of the engineering protocols, one of this repository's
//! `protocol/1` files embedded in the crate, or a `protocol/1` file, validated and compiled by
//! Canon. Nothing here models a protocol in Rust.

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

/// The protocols this crate ships beside the engineering protocols' built-ins, `(name, major,
/// document)`: the `protocol/1` files of this repository's `protocols/`, embedded so that a
/// repository a story is delivered in need not carry them.
const EMBEDDED: &[(&str, u32, &str)] = &[(
    "story.delivery",
    1,
    include_str!("../../../protocols/story-delivery/1.yaml"),
)];

impl LoadedProtocol {
    /// A built-in, by `name/major` (`software.change/1`): one this crate embeds
    /// (`story.delivery/1`), else one of the engineering protocols.
    pub fn builtin(id: &str) -> Result<Self> {
        let (name, major) = split_id(id)?;
        // An embedded protocol answers to its own id, `story.delivery`, and to its directory's
        // name, `story-delivery`, as the registry's built-ins do.
        if let Some((_, _, yaml)) = EMBEDDED.iter().find(|(embedded, revision, _)| {
            *revision == major && (*embedded == name || embedded.replace('.', "-") == name)
        }) {
            return Self::from_yaml(yaml);
        }
        // The registry names a protocol by its directory, `software-change`; the protocol's own
        // id is `software.change`. Both spellings are accepted.
        let builtin = canon_engineering::registry::get(name, major).or_else(|first| {
            canon_engineering::registry::get(&name.replace('.', "-"), major).map_err(|_| first)
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
    /// artifacts: a grant is never bound to a guess.
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
            let (Some(subject), None) = (subjects.next(), subjects.next()) else {
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

/// Adds to `subjects` the subject of every evidence condition in `condition`, following each
/// claim it names into that claim's `true_when` in `claims`, each claim once.
fn subjects_of(
    claims: &serde_yaml_ng::Value,
    condition: &serde_yaml_ng::Value,
    seen: &mut std::collections::BTreeSet<String>,
    subjects: &mut std::collections::BTreeSet<String>,
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
                    (Some("subject"), serde_yaml_ng::Value::String(subject)) => {
                        subjects.insert(subject.clone());
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
    /// The built-in protocols, `(name, major)` each: the engineering protocols', then the ones
    /// this crate embeds.
    #[must_use]
    pub fn builtins() -> Vec<(String, u32)> {
        canon_engineering::registry::list()
            .into_iter()
            .map(|(name, major)| (name.to_owned(), major))
            .chain(
                EMBEDDED
                    .iter()
                    .map(|(name, major, _)| ((*name).to_owned(), *major)),
            )
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

    /// The story-delivery protocol ships embedded, byte for byte the repository's file, under
    /// both spellings, and is listed among the built-ins.
    #[test]
    fn the_story_delivery_builtin_is_embedded() {
        let file = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../protocols/story-delivery/1.yaml"
        ))
        .expect("the repository's file");
        for id in ["story.delivery/1", "story-delivery/1"] {
            let protocol = LoadedProtocol::builtin(id).expect("a built-in");
            assert_eq!(protocol.id, "story.delivery/1");
            assert_eq!(protocol.yaml, file);
            assert!(protocol.actions.contains(&"pr.open".to_owned()));
        }
        assert!(LoadedProtocol::builtin("story.delivery/2").is_err());
        assert!(
            LoadedProtocol::builtins()
                .iter()
                .any(|(name, major)| name == "story.delivery" && *major == 1)
        );
    }

    /// A protocol given as an id or as a file resolves to the same protocol.
    #[test]
    fn resolve_takes_a_file_or_a_builtin() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../protocols/story-delivery/1.yaml"
        );
        let file = LoadedProtocol::resolve(path).expect("the file");
        let builtin = LoadedProtocol::resolve("story.delivery/1").expect("the built-in");
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
