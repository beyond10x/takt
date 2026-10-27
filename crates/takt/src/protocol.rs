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

/// An input a protocol declares under `case.inputs`: its name, its description and the values it
/// takes, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredInput {
    /// The input's name.
    pub name: String,
    /// What the protocol says the input is, on one line; empty when it says nothing.
    pub description: String,
    /// The values it takes; empty when the protocol lists none, which admits any.
    pub values: Vec<String>,
}

impl LoadedProtocol {
    /// The declared case inputs with their descriptions, in declaration order; the same inputs and
    /// values as [`LoadedProtocol::inputs`].
    #[must_use]
    pub fn declared_inputs(&self) -> Vec<DeclaredInput> {
        let Ok(doc) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&self.yaml) else {
            return Vec::new();
        };
        self.inputs
            .iter()
            .map(|(name, values)| DeclaredInput {
                name: name.clone(),
                description: doc["case"]["inputs"][name.as_str()]["description"]
                    .as_str()
                    .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
                    .unwrap_or_default(),
                values: values.clone(),
            })
            .collect()
    }
}

/// Where the value of a setting came from, highest precedence first: the command line, a story's
/// tag, the run file, takt's built-in default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `--input` or `--protocol`.
    Flag,
    /// A story tag `input:<name>=<value>` or `protocol:<name>/<major>`.
    Tag,
    /// The run file's `protocols` section or `protocol` key.
    RunFile,
    /// takt's built-in default.
    Default,
}

impl Source {
    /// The name the case record and the run's line write: `flag`, `tag`, `run-file`, `default`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Tag => "tag",
            Self::RunFile => "run-file",
            Self::Default => "default",
        }
    }
}

/// One case input with the source of its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// The declared input.
    pub name: String,
    /// Its value, a boolean as `true` or `false`.
    pub value: String,
    /// Where the value came from.
    pub source: Source,
}

/// takt's built-in defaults, `(protocol id, input, value)`: the values a case takes for a declared
/// input nobody sets, the least demanding a change can carry.
const BUILTIN_DEFAULTS: &[(&str, &str, &str)] = &[
    ("software.change/1", "risk", "standard"),
    ("software.change/1", "change_kind", "feature"),
    ("software.change/1", "affects_security_boundary", "false"),
];

/// takt's built-in default for `input` of the protocol `protocol`, an id, when it ships one.
#[must_use]
pub fn builtin_default(protocol: &str, input: &str) -> Option<&'static str> {
    BUILTIN_DEFAULTS
        .iter()
        .find(|(id, name, _)| *id == protocol && *name == input)
        .map(|(_, _, value)| *value)
}

/// What a case opens on: the protocol and each declared input that something sets, with sources.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The protocol the case opens on.
    pub protocol: LoadedProtocol,
    /// What `RunOptions::protocol` takes for it: the id, or the file `--protocol` named.
    pub spec: String,
    /// Where the protocol was chosen.
    pub protocol_source: Source,
    /// The declared inputs something set, in declaration order.
    pub inputs: Vec<Setting>,
}

impl Settings {
    /// The inputs as the case is opened with them, `(name, value)`.
    #[must_use]
    pub fn pairs(&self) -> Vec<(String, String)> {
        self.inputs
            .iter()
            .map(|setting| (setting.name.clone(), setting.value.clone()))
            .collect()
    }

    /// The sources as the case record keeps them, `(name, source)`.
    #[must_use]
    pub fn sources(&self) -> Vec<(String, String)> {
        self.inputs
            .iter()
            .map(|setting| (setting.name.clone(), setting.source.as_str().to_owned()))
            .collect()
    }

    /// One line naming the protocol, the inputs and each one's source:
    /// `settings: protocol story.delivery/1 (default); risk=elevated (run-file)`.
    #[must_use]
    pub fn line(&self) -> String {
        let mut line = format!(
            "settings: protocol {} ({})",
            self.protocol.id,
            self.protocol_source.as_str()
        );
        for setting in &self.inputs {
            line.push_str(&format!(
                "; {}={} ({})",
                setting.name,
                setting.value,
                setting.source.as_str()
            ));
        }
        line
    }
}

/// What a case's settings are resolved from.
#[derive(Debug, Clone, Copy)]
pub struct SettingsRequest<'a> {
    /// The run file's `protocol` and `protocols`.
    pub file: &'a crate::run_config::ProtocolSettings,
    /// `--protocol`: a built-in id or a `protocol/1` file.
    pub flag_protocol: Option<&'a str>,
    /// `--input`, `name=value` each.
    pub flag_inputs: &'a [(String, String)],
    /// The tags of the story the case opens on; none for an intent.
    pub tags: &'a [String],
    /// The built-in id the case opens on when nothing chooses.
    pub default_protocol: &'a str,
    /// Whether the run file's `protocol` key chooses: for a story of the planning store, not for
    /// an intent.
    pub file_protocol_applies: bool,
}

/// The ids of the built-in protocols, `<name>/<major>`, as the protocols write them
/// (`story.delivery/1`).
#[must_use]
pub fn builtin_ids() -> Vec<String> {
    LoadedProtocol::builtins()
        .into_iter()
        .map(|(name, major)| {
            let listed = format!("{name}/{major}");
            LoadedProtocol::builtin(&listed).map_or(listed, |protocol| protocol.id)
        })
        .collect()
}

/// A protocol's name as the registry spells it, `story-delivery`.
fn registry_name(name: &str) -> String {
    name.replace(['.', '_'], "-")
}

/// Whether the key `section` of a run file's `protocols`, a name or an id, names `protocol`.
fn names(section: &str, protocol: &LoadedProtocol) -> bool {
    let section = registry_name(section);
    section == registry_name(&protocol.name) || section == registry_name(&protocol.id)
}

/// The built-in protocol a `protocols` key names: the id's, or the name's newest revision.
fn builtin_named(section: &str) -> Option<LoadedProtocol> {
    if section.contains('/') {
        return LoadedProtocol::builtin(section).ok();
    }
    let wanted = registry_name(section);
    let major = LoadedProtocol::builtins()
        .into_iter()
        .filter(|(name, _)| registry_name(name) == wanted)
        .map(|(_, major)| major)
        .max()?;
    LoadedProtocol::builtin(&format!("{section}/{major}")).ok()
}

/// The names of `declared`, for a refusal.
fn declared_names(declared: &[DeclaredInput]) -> String {
    if declared.is_empty() {
        "none".to_owned()
    } else {
        declared
            .iter()
            .map(|input| input.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The values of `input` for a refusal.
fn declared_values(input: &DeclaredInput) -> String {
    input.values.join(", ")
}

/// The protocol a case opens on, with what it was given as and where it was chosen: the first of
/// `--protocol`, the story's tag `protocol:<name>/<major>`, the run file's `protocol` (for a story
/// of the planning store) and the default. A tag or a key names a built-in protocol only, never a
/// file: a tag is the store's text, and the file a key names is not beside the run file.
pub fn choose_protocol(request: &SettingsRequest<'_>) -> Result<(String, Source, LoadedProtocol)> {
    let file = request.file;
    let mut protocols = request
        .tags
        .iter()
        .filter_map(|tag| tag.strip_prefix("protocol:"));
    let tagged = protocols.next();
    if protocols.next().is_some() {
        bail!("the story carries more than one protocol: tag; it chooses one protocol");
    }
    let known = || builtin_ids().join(", ");
    let (spec, source) = if let Some(flag) = request.flag_protocol {
        (flag.to_owned(), Source::Flag)
    } else if let Some(tag) = tagged {
        (tag.to_owned(), Source::Tag)
    } else if let Some((key, _)) = file
        .protocol
        .as_ref()
        .filter(|_| request.file_protocol_applies)
    {
        (key.clone(), Source::RunFile)
    } else {
        (request.default_protocol.to_owned(), Source::Default)
    };
    let protocol = match source {
        Source::Flag | Source::Default => LoadedProtocol::resolve(&spec)?,
        Source::Tag => LoadedProtocol::builtin(&spec).map_err(|_| {
            anyhow!(
                "the story's tag protocol:{spec} names no protocol takt knows; it knows {}",
                known()
            )
        })?,
        Source::RunFile => LoadedProtocol::builtin(&spec).map_err(|_| {
            let line = file.protocol.as_ref().map_or(1, |(_, line)| *line);
            file.refusal(
                line,
                &format!(
                    "`protocol` names no protocol takt knows; it knows {}",
                    known()
                ),
            )
        })?,
    };
    if source == Source::Tag && protocol.id == crate::case::TURN_PROTOCOL {
        bail!(
            "the story's tag protocol:{spec} names {}, which opens on a finding and a candidate, never on a story",
            protocol.id
        );
    }
    Ok((spec, source, protocol))
}

/// Resolves what a case opens on, refusing before anything is opened what the run file, a tag or
/// the command line gets wrong.
///
/// The protocol is the first of `--protocol`, the story's tag `protocol:<name>/<major>`, the run
/// file's `protocol` (for a story of the planning store) and the default. Each declared input
/// takes the first of `--input`, the story's tag `input:<name>=<value>`, the run file's
/// `protocols` section for the protocol, and takt's built-in default; an input nobody sets is
/// left out, and opening the case refuses it, naming its values. Every entry of the run file's
/// `protocols` is checked, whichever protocol the case opens on: an unknown protocol, input or
/// value is refused naming the file and the line, and a tag's or a flag's input the protocol does
/// not declare is refused naming it. A tag or a key names a built-in protocol only, never a file.
pub fn resolve_settings(request: &SettingsRequest<'_>) -> Result<Settings> {
    let file = request.file;
    let known = || builtin_ids().join(", ");
    let (spec, source, protocol) = choose_protocol(request)?;
    let declared = protocol.declared_inputs();
    // Every entry of the run file's `protocols`, against the protocol it names.
    let mut from_file: Vec<(&str, &crate::run_config::InputSetting)> = Vec::new();
    for section in &file.sections {
        let target = if names(&section.protocol, &protocol) {
            protocol.clone()
        } else {
            builtin_named(&section.protocol).ok_or_else(|| {
                file.refusal(
                    section.line,
                    &format!(
                        "`protocols` names no protocol takt knows; it knows {}",
                        known()
                    ),
                )
            })?
        };
        let theirs = target.declared_inputs();
        for set in &section.inputs {
            let Some(input) = theirs.iter().find(|input| input.name == set.input) else {
                return Err(file.refusal(
                    set.line,
                    &format!(
                        "{} declares no input {}; it declares {}",
                        target.id,
                        set.input,
                        declared_names(&theirs)
                    ),
                ));
            };
            if !input.values.is_empty() && !input.values.contains(&set.value) {
                return Err(file.refusal(
                    set.line,
                    &format!(
                        "the input {} of {} takes one of {}",
                        set.input,
                        target.id,
                        declared_values(input)
                    ),
                ));
            }
            if names(&section.protocol, &protocol) {
                from_file.push((section.protocol.as_str(), set));
            }
        }
    }

    // The story's tags `input:<name>=<value>`.
    let mut from_tags: Vec<(&str, &str)> = Vec::new();
    for tag in request.tags {
        let Some(rest) = tag.strip_prefix("input:") else {
            continue;
        };
        let Some((name, value)) = rest.split_once('=') else {
            bail!("the story's tag {tag} is `input:<name>=<value>`");
        };
        let Some(input) = declared.iter().find(|input| input.name == name) else {
            bail!(
                "the story's tag {tag}: {} declares no input {name}; it declares {}",
                protocol.id,
                declared_names(&declared)
            );
        };
        if !input.values.is_empty() && !input.values.iter().any(|allowed| allowed == value) {
            bail!(
                "the story's tag {tag}: the input {name} of {} takes one of {}",
                protocol.id,
                declared_values(input)
            );
        }
        if from_tags.iter().any(|(given, _)| *given == name) {
            bail!("the story carries two tags input:{name}=; it sets one value");
        }
        from_tags.push((name, value));
    }

    // `--input`: one the protocol does not declare would be dropped without a word.
    for (name, _) in request.flag_inputs {
        if !declared.iter().any(|input| input.name == *name) {
            bail!(
                "--input {name}: {} declares no input {name}; it declares {}",
                protocol.id,
                declared_names(&declared)
            );
        }
    }

    let mut inputs = Vec::new();
    for input in &declared {
        let name = input.name.as_str();
        // An id's own section is more specific than a bare name's.
        let in_file = from_file
            .iter()
            .filter(|(_, set)| set.input == name)
            .max_by_key(|(section, _)| section.contains('/'))
            .map(|(_, set)| set.value.clone());
        let chosen = request
            .flag_inputs
            .iter()
            .rev()
            .find(|(given, _)| given == name)
            .map(|(_, value)| (value.clone(), Source::Flag))
            .or_else(|| {
                from_tags
                    .iter()
                    .find(|(given, _)| *given == name)
                    .map(|(_, value)| ((*value).to_owned(), Source::Tag))
            })
            .or_else(|| in_file.map(|value| (value, Source::RunFile)))
            .or_else(|| {
                builtin_default(&protocol.id, name).map(|value| (value.to_owned(), Source::Default))
            });
        if let Some((value, from)) = chosen {
            inputs.push(Setting {
                name: name.to_owned(),
                value,
                source: from,
            });
        }
    }
    Ok(Settings {
        protocol,
        spec,
        protocol_source: source,
        inputs,
    })
}

/// The value a run file's `protocols` section sets for an input, with where it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFileValue {
    /// The value written.
    pub value: String,
    /// The run file's path.
    pub path: Option<std::path::PathBuf>,
    /// The line the value is written on, 1-based.
    pub line: usize,
}

/// One declared input of a protocol as `takt protocol show` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputListing {
    /// The input's name.
    pub name: String,
    /// What the protocol says the input is, on one line; empty when it says nothing.
    pub description: String,
    /// The values it takes; empty when the protocol lists none, which admits any.
    pub values: Vec<String>,
    /// takt's built-in default, when it ships one.
    pub default: Option<String>,
    /// The value the run file sets, when it sets one.
    pub run_file: Option<RunFileValue>,
}

/// A protocol's declared inputs, each with its values, description, built-in default and the value
/// the run file sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolListing {
    /// The protocol's id, `<name>/<major>`.
    pub id: String,
    /// The declared inputs, in declaration order.
    pub inputs: Vec<InputListing>,
}

/// Lists the declared inputs of the built-in protocol `spec`, `<name>/<major>` in either spelling,
/// against `file`, a run file's protocol settings: an unknown protocol is refused naming the ones
/// takt knows. The run file is not validated here; a value written for the protocol is shown as
/// written.
///
/// # Errors
/// When `spec` names no built-in protocol.
pub fn list_settings(
    spec: &str,
    file: &crate::run_config::ProtocolSettings,
) -> Result<ProtocolListing> {
    let protocol = LoadedProtocol::builtin(spec).map_err(|_| {
        anyhow!(
            "no protocol {spec}; takt knows {}",
            builtin_ids().join(", ")
        )
    })?;
    let inputs = protocol
        .declared_inputs()
        .into_iter()
        .map(|input| {
            let run_file = file
                .sections
                .iter()
                .filter(|section| names(&section.protocol, &protocol))
                .flat_map(|section| {
                    section
                        .inputs
                        .iter()
                        .filter(|set| set.input == input.name)
                        .map(move |set| (section.protocol.contains('/'), set))
                })
                .max_by_key(|(specific, _)| *specific)
                .map(|(_, set)| RunFileValue {
                    value: set.value.clone(),
                    path: file.file.clone(),
                    line: set.line,
                });
            InputListing {
                default: builtin_default(&protocol.id, &input.name).map(str::to_owned),
                name: input.name,
                description: input.description,
                values: input.values,
                run_file,
            }
        })
        .collect();
    Ok(ProtocolListing {
        id: protocol.id,
        inputs,
    })
}

impl ProtocolListing {
    /// The listing as text: the protocol, then per input its values, description, default and run
    /// file value, `unset` for a run file that does not set it and `none` for a default takt does
    /// not ship.
    #[must_use]
    pub fn text(&self) -> String {
        let mut text = format!("protocol {}\n", self.id);
        if self.inputs.is_empty() {
            text.push_str("declares no inputs\n");
        }
        for input in &self.inputs {
            text.push_str(&format!("input {}\n", input.name));
            if input.values.is_empty() {
                text.push_str("  values: any\n");
            } else {
                text.push_str(&format!("  values: {}\n", input.values.join(", ")));
            }
            text.push_str(&format!("  description: {}\n", input.description));
            text.push_str(&format!(
                "  default: {}\n",
                input.default.as_deref().unwrap_or("none")
            ));
            match &input.run_file {
                Some(set) => text.push_str(&format!(
                    "  run file: {} ({}:{})\n",
                    set.value,
                    set.path.as_ref().map_or_else(
                        || "<run file>".to_owned(),
                        |path| path.display().to_string()
                    ),
                    set.line
                )),
                None => text.push_str("  run file: unset\n"),
            }
        }
        text
    }

    /// [`ProtocolListing::json`] as indented text.
    #[must_use]
    pub fn json_text(&self) -> String {
        serde_json::to_string_pretty(&self.json()).unwrap_or_default()
    }

    /// The listing as data: `{"protocol", "inputs": [{"name", "description", "values", "default",
    /// "run_file": {"value", "path", "line"}}]}`, `default` and `run_file` null when there is none.
    #[must_use]
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "protocol": self.id,
            "inputs": self.inputs.iter().map(|input| serde_json::json!({
                "name": input.name,
                "description": input.description,
                "values": input.values,
                "default": input.default,
                "run_file": input.run_file.as_ref().map(|set| serde_json::json!({
                    "value": set.value,
                    "path": set.path.as_ref().map(|path| path.display().to_string()),
                    "line": set.line,
                })),
            })).collect::<Vec<_>>(),
        })
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
        assert!(LoadedProtocol::builtin("story.delivery/2").is_ok());
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
