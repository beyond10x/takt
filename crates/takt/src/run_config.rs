//! A run's configuration file, `takt run --config <FILE>` and `takt scenario run --config
//! <FILE>`: the story, the loop's budget, the harness, its model, endpoint file and token
//! variable, the forge, the test command, the state directory, whether the run goes on past
//! a red base and its scan adapter (`scan`, `none`, set nowhere else), read from
//! one file so a run started in a container takes them from a mounted file. The file is TOML
//! (`.toml`) or YAML (`.yaml`, `.yml`), the same fields in either; a YAML file whose `format`
//! is `loom.agent/1` is the agent document Loom mounts for a `LoomAgent` at
//! `/config/agent.yaml`, and its `takt` member is the run configuration, its `runtime.state_dir`
//! the state directory when `takt` names none. A flag given on the
//! command line wins over the file, and the file over the flag's default; the run applies
//! them (`crates/takt-cli/src/main.rs`).

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow};

use crate::forge::command::ForgeCommands;
use crate::forge::credential::CredentialSource;
use crate::forge::gitlab::Transport;

/// llm's parsed `llm.routes/1` document, the type a run file's `llm` member is validated as; its
/// [`Routes::digest`] is the `llm_digest` a run records.
pub use llm_models::routes::Routes;

/// The `llm.routes/1` document a run file's `llm` member holds, parsed by llm and shared, so a run
/// resolves its `model` against it. Two are equal when their digests are.
#[derive(Debug, Clone)]
pub struct LlmRoutes {
    routes: Arc<Routes>,
    variables: Vec<String>,
}

impl LlmRoutes {
    fn new(routes: Routes, variables: Vec<String>) -> Self {
        Self {
            routes: Arc::new(routes),
            variables,
        }
    }

    /// The parsed document.
    #[must_use]
    pub fn routes(&self) -> &Routes {
        &self.routes
    }

    /// The variables the document's `env:` secret references name, in any endpoint, sorted and
    /// without repeats: a child of takt must not inherit them.
    #[must_use]
    pub fn variables(&self) -> &[String] {
        &self.variables
    }
}

impl PartialEq for LlmRoutes {
    fn eq(&self, other: &Self) -> bool {
        self.routes.digest() == other.routes.digest()
    }
}

/// The variables the `env:` references in `document` name, in any place of it. llm keeps the
/// names of the references it reads to itself, so takt reads them off the parsed document; only
/// names that a variable may carry are kept.
fn secret_variables(document: &serde_json::Value) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![document];
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::String(text) => {
                if let Some(name) = text.strip_prefix("env:")
                    && !name.is_empty()
                    && !name.contains(['=', '\0'])
                {
                    names.push(name.to_owned());
                }
            }
            serde_json::Value::Array(items) => pending.extend(items),
            serde_json::Value::Object(map) => pending.extend(map.values()),
            _ => {}
        }
    }
    names.sort();
    names.dedup();
    names
}

/// The `format` of the agent document Loom writes to `/config/agent.yaml`.
pub const LOOM_AGENT_FORMAT: &str = "loom.agent/1";

/// A `loom.agent/1` document as takt reads it for the lines of its refusals: its `takt` member
/// and its `runtime` member as a value, which [`loom_runtime_state_dir`] reads.
#[derive(serde::Deserialize)]
struct LoomAgentDocument {
    takt: Option<RunConfig>,
    runtime: Option<serde_yaml_ng::Value>,
}

/// The `runtime` member of a `loom.agent/1` document, the members Loom declares for it
/// (`loom-sdk` `agent.rs`): `state_dir`, the default state directory takt reads, and
/// `connectors` and `confinement`, which takt does not read. A member Loom does not declare is
/// refused.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LoomRuntime {
    state_dir: Option<serde_yaml_ng::Value>,
    #[allow(dead_code)]
    connectors: Option<serde_yaml_ng::Value>,
    #[allow(dead_code)]
    confinement: Option<serde_yaml_ng::Value>,
}

/// The refusal of a `loom.agent/1` document's `runtime` member, naming `runtime.state_dir` when
/// it is that member, never a value written in the file.
fn runtime_refused(path: &Path, what: &str) -> anyhow::Error {
    anyhow!(
        "the run configuration file {} is refused: {what}",
        path.display()
    )
}

/// The `runtime.state_dir` of a `loom.agent/1` document: nothing when it has none, else a
/// non-blank string. A number, a boolean, a mapping, an empty or blank string is refused; so is a
/// `runtime` that is not a mapping of Loom's declared members.
fn loom_runtime_state_dir(path: &Path, runtime: serde_yaml_ng::Value) -> Result<Option<PathBuf>> {
    let runtime: LoomRuntime = serde_yaml_ng::from_value(runtime).map_err(|_| {
        runtime_refused(
            path,
            "the `runtime` member of a loom.agent/1 document is not a mapping of only the members \
             Loom declares, `state_dir`, `connectors` and `confinement`",
        )
    })?;
    match runtime.state_dir {
        None => Ok(None),
        Some(serde_yaml_ng::Value::String(dir)) if !dir.trim().is_empty() => {
            Ok(Some(PathBuf::from(dir)))
        }
        Some(_) => Err(runtime_refused(
            path,
            "`runtime.state_dir` is a non-blank string naming a directory, and no other value",
        )),
    }
}

/// The members of a `loom.agent/1` document Loom declares (Loom 0.43.0, `loom-sdk` `agent.rs`
/// `spec`). takt reads none of them; a member that is neither one of these nor `takt` is refused,
/// so a run field written beside `format`, or `takt` misspelt, is not dropped.
pub const LOOM_AGENT_MEMBERS: [&str; 13] = [
    "format", "agent", "model", "session", "loops", "states", "bindings", "channel", "writes",
    "policy", "expose", "runtime", "deploy",
];

/// Whether every number in `value` is finite.
fn finite(value: &serde_yaml_ng::Value) -> bool {
    match value {
        serde_yaml_ng::Value::Number(number) => number.as_f64().is_none_or(f64::is_finite),
        serde_yaml_ng::Value::Sequence(items) => items.iter().all(finite),
        serde_yaml_ng::Value::Mapping(mapping) => mapping
            .iter()
            .all(|(key, item)| finite(key) && finite(item)),
        _ => true,
    }
}

/// Whether `value` or anything in it carries a YAML tag, which neither takt nor Loom declares.
fn tagged(value: &serde_yaml_ng::Value) -> bool {
    match value {
        serde_yaml_ng::Value::Tagged(_) => true,
        serde_yaml_ng::Value::Sequence(items) => items.iter().any(tagged),
        serde_yaml_ng::Value::Mapping(mapping) => mapping
            .iter()
            .any(|(key, item)| tagged(key) || tagged(item)),
        _ => false,
    }
}

/// The run configuration `value` holds, its types as strict as TOML's: a number or a boolean
/// where a string field is, or a fraction where `iterations` is, is refused. YAML reads any
/// scalar as a string when a string is asked for; JSON's reader does not. A number that is not
/// finite (`.inf`, `.nan`) is refused: JSON has none, and the round trip would read it as unset.
fn strict(path: &Path, value: &serde_yaml_ng::Value) -> Result<RunConfig> {
    if !finite(value) {
        return Err(refused(path, None));
    }
    serde_json::to_value(value)
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(|| refused(path, None))
}

/// The fields a run configuration file may set, each one a `takt run` flag's: `story`,
/// `iterations`, `hours`, `tokens`, `harness`, `model`, `endpoint_file`, `endpoint_token_env`,
/// `forge`, `test_cmd`, `state_dir`, `continue_past_red_base` and `records`; and `scan` and
/// `llm`, which no flag gives. None is required.
#[derive(Debug, Default, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunConfig {
    /// `--story`, a story's id.
    pub story: Option<String>,
    /// `--iterations`.
    pub iterations: Option<u64>,
    /// `--hours`.
    pub hours: Option<f64>,
    /// `--tokens`, a whole number above zero. Set by [`RunConfig::read`] from the member, never
    /// read from the file.
    #[serde(skip)]
    pub tokens: Option<u64>,
    /// The `tokens` member as written, which [`RunConfig::read`] checks into
    /// [`RunConfig::tokens`] or refuses at its line ([`TokensRefused`]). A `tokens` written with
    /// no value (YAML's null) is a value written, refused as any other ([`written`]).
    #[serde(rename = "tokens", default, deserialize_with = "written")]
    tokens_member: Option<serde_yaml_ng::Value>,
    /// `--harness`, or the `harness` mapping: metaharness's session request. Set by
    /// [`RunConfig::read`] from the member, never read from the file.
    #[serde(skip)]
    pub harness: Option<HarnessSetting>,
    /// The `harness` member as written, a name or a mapping, which [`RunConfig::read`] parses
    /// into [`RunConfig::harness`].
    #[serde(rename = "harness")]
    harness_member: Option<serde_yaml_ng::Value>,
    /// `--model`.
    pub model: Option<String>,
    /// `--endpoint-file`; a relative path is read beside the configuration file.
    pub endpoint_file: Option<PathBuf>,
    /// `--endpoint-token-env`, the name of the variable holding the endpoint's API key, never
    /// its value.
    pub endpoint_token_env: Option<String>,
    /// `--forge`.
    pub forge: Option<String>,
    /// `--test-cmd`.
    pub test_cmd: Option<String>,
    /// `--state-dir`; a relative path is read beside the configuration file.
    pub state_dir: Option<PathBuf>,
    /// `--continue-past-red-base`.
    pub continue_past_red_base: Option<bool>,
    /// `--records`: `default` or `trial`, checked when the file is read.
    pub records: Option<String>,
    /// The `commit` section: `style`, `trailer` and `types`, each a `--commit-*` flag's, checked
    /// when the file is read ([`crate::commit_style::Settings::check`]).
    pub commit: Option<crate::commit_style::Section>,
    /// The run's scan adapter, `none` ([`crate::scan::Scan`]): the one field no flag sets, so
    /// a run is unscanned only where its file says so. Not set, it is `unset`.
    pub scan: Option<crate::scan::ScanName>,
    /// The model routes: an `llm.routes/1` document written as a mapping, or `{path: <file>}`
    /// read beside the file. Kept as written; [`RunConfig::read`] has llm validate it.
    #[serde(default, deserialize_with = "present")]
    pub llm: Option<serde_yaml_ng::Value>,
    /// The digest of the document `llm` holds, as [`Routes::digest`] gives it. Set by
    /// [`RunConfig::read`], never read from the file.
    #[serde(skip)]
    pub llm_digest: Option<String>,
    /// The document `llm` holds, parsed by llm. Set by [`RunConfig::read`], never read from the file.
    #[serde(skip)]
    pub llm_routes: Option<LlmRoutes>,
    /// The protocol a store story opens on, `<name>/<major>` or a `protocol/1` file: `--protocol`'s.
    /// [`RunConfig::read`] checks its shape and keeps it with its line in
    /// [`RunConfig::protocol_settings`].
    pub protocol: Option<String>,
    /// The `protocols` member as written, a mapping of protocol to a mapping of its declared
    /// inputs, which [`RunConfig::read`] parses into [`RunConfig::protocol_settings`].
    #[serde(rename = "protocols")]
    protocols_member: Option<serde_yaml_ng::Value>,
    /// What the file's `protocol` and `protocols` members set, each with its line. Set by
    /// [`RunConfig::read`], never read from the file.
    #[serde(skip)]
    pub protocol_settings: ProtocolSettings,
    /// The connector `forge: connectors` calls and, after one `/`, its instance:
    /// `<connector>[/<instance>]`, the connector `gitlab` or `github`, the instance a plain word
    /// of letters, digits, `.`, `_` and `-`; checked when the file is read
    /// ([`crate::forge::connectors::target_of`]).
    pub forge_instance: Option<String>,
    /// The URL of the served Connectors instance `forge: connectors` calls instead of the
    /// `connectors` program: `https://`, or `http://` to a loopback host alone; checked when
    /// the file is read ([`crate::forge::connectors::check_endpoint`]), a refusal exits 2.
    pub forge_endpoint: Option<String>,
    /// The `forge_commands` section, the argument lists `forge: command` runs.
    pub forge_commands: Option<ForgeCommands>,
    /// The argument list `forge: local` runs as the checks on a request's head, without a shell,
    /// as the test command is, in a fresh repository at the head; checked when the file is read.
    pub local_checks: Option<Vec<String>>,
    /// How `forge: gitlab` reaches the host: `glab` (the default) or `api`.
    pub transport: Option<Transport>,
    /// Where the `api` transport's credential comes from, by reference; a relative `file` is
    /// read beside the run file.
    pub credential: Option<CredentialSource>,
    /// The `setup` list: commands run once at open, before the red-base check ([`SetupCommand`]).
    pub setup: Option<Vec<SetupCommand>>,
}

/// What a run file says about the forge beyond its name: the commands, the transport and the
/// credential's source.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ForgeSettings {
    /// `forge_commands`.
    pub commands: Option<ForgeCommands>,
    /// `local_checks`.
    pub local_checks: Option<Vec<String>>,
    /// `forge_instance`.
    pub instance: Option<String>,
    /// `forge_endpoint`.
    pub endpoint: Option<String>,
    /// `transport`.
    pub transport: Option<Transport>,
    /// `credential`.
    pub credential: Option<CredentialSource>,
}

impl ForgeSettings {
    /// Refuses what does not fit the forge `forge` names, before any case opens: `forge_commands`
    /// belongs to `forge: command` and `forge: command` needs it, as `local_checks` and
    /// `forge: local` go together; `transport` and `credential`
    /// belong to `forge: gitlab`, and `transport: api` needs a `credential` which only it takes.
    /// The refusal names keys, never a value.
    pub fn check_for(&self, forge: Option<&str>) -> Result<()> {
        if self.instance.is_some() && forge != Some("connectors") {
            return Err(anyhow!(
                "`forge_instance` is set, and it belongs to `forge: connectors`, which this run does not use; nothing was opened"
            ));
        }
        if self.endpoint.is_some() && forge != Some("connectors") {
            return Err(anyhow!(
                "`forge_endpoint` is set, and it belongs to `forge: connectors`, which this run does not use; nothing was opened"
            ));
        }
        if forge == Some("connectors") && self.instance.is_none() {
            return Err(anyhow!(
                "`forge: connectors` calls the connector `forge_instance` names in the run file, and there is none; nothing was opened"
            ));
        }
        if self.commands.is_some() && forge != Some("command") {
            return Err(anyhow!(
                "`forge_commands` is set, and it belongs to `forge: command`, which this run does not use; nothing was opened"
            ));
        }
        if forge == Some("command") && self.commands.is_none() {
            return Err(anyhow!(
                "`forge: command` runs the commands of a `forge_commands` section in the run file, and there is none; nothing was opened"
            ));
        }
        if self.local_checks.is_some() && forge != Some("local") {
            return Err(anyhow!(
                "`local_checks` is set, and it belongs to `forge: local`, which this run does not use; nothing was opened"
            ));
        }
        if forge == Some("local") && self.local_checks.is_none() {
            return Err(anyhow!(
                "`forge: local` runs the checks a `local_checks` argument list in the run file names, and there is none: no check is never a pass; nothing was opened"
            ));
        }
        if (self.transport.is_some() || self.credential.is_some()) && forge != Some("gitlab") {
            return Err(anyhow!(
                "`transport` or `credential` is set, and they belong to `forge: gitlab`, which this run does not use; nothing was opened"
            ));
        }
        match (self.transport, &self.credential) {
            (Some(Transport::Api), None) => Err(anyhow!(
                "`transport: api` needs a `credential`, `{{env: NAME}}` or `{{file: PATH}}`; nothing was opened"
            )),
            (Some(Transport::Glab) | None, Some(_)) => Err(anyhow!(
                "`credential` belongs to `transport: api`; the default transport, glab, runs under its own login; nothing was opened"
            )),
            _ => Ok(()),
        }
    }
}

impl RunConfig {
    /// The forge settings the file holds.
    #[must_use]
    pub fn forge_settings(&self) -> ForgeSettings {
        ForgeSettings {
            commands: self.forge_commands.clone(),
            local_checks: self.local_checks.clone(),
            instance: self.forge_instance.clone(),
            endpoint: self.forge_endpoint.clone(),
            transport: self.transport,
            credential: self.credential.clone(),
        }
    }
}

/// One command of a run file's `setup` list (`takt.work.SetupCommand`): a `name` and the `argv`
/// that is run as a process, never as a shell string.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupCommand {
    /// One plain word of letters, digits, `.`, `_` and `-`, unique in the list; it names the
    /// command's output file.
    pub name: String,
    /// The program and its arguments, none empty.
    pub argv: Vec<String>,
}

impl SetupCommand {
    /// Why `commands` cannot be run, or `None`: a name that is not one plain word (`.` and `..`
    /// included) or is given twice, an `argv` that is empty or names an empty program, and an
    /// argument holding a NUL byte. The reason repeats no argument.
    #[must_use]
    pub fn refusal(commands: &[Self]) -> Option<String> {
        let mut seen = std::collections::BTreeSet::new();
        for (index, command) in commands.iter().enumerate() {
            let at = index + 1;
            let plain = !command.name.is_empty()
                && command.name != "."
                && command.name != ".."
                && command
                    .name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
            if !plain {
                return Some(format!(
                    "the name of `setup` command {at} is one plain word of letters, digits, `.`, `_` and `-`"
                ));
            }
            if !seen.insert(command.name.as_str()) {
                return Some(format!(
                    "`setup` command {at} repeats the name `{}`, which is given once",
                    command.name
                ));
            }
            if command.argv.first().is_none_or(String::is_empty) {
                return Some(format!(
                    "the `argv` of `setup` command `{}` names a program, a non-empty first word",
                    command.name
                ));
            }
            if command.argv.iter().any(|word| word.contains('\0')) {
                return Some(format!(
                    "the `argv` of `setup` command `{}` holds a NUL byte, which no argument can",
                    command.name
                ));
            }
        }
        None
    }
}

/// A present `llm` member, null included: serde reads a null as absent, so a bare `llm:` would
/// otherwise open a case with no section. It is kept as a value and refused by `llm_digest`.
fn present<'de, D>(deserializer: D) -> std::result::Result<Option<serde_yaml_ng::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <serde_yaml_ng::Value as serde::Deserialize>::deserialize(deserializer).map(Some)
}

/// The `harness` member of a run file: `--harness`'s name as a string, or a mapping, metaharness's
/// session request, whose `kind` names the harness.
#[derive(Debug, Clone, PartialEq)]
pub enum HarnessSetting {
    /// The name `--harness` takes.
    Name(String),
    /// The session request a mapping gives.
    Session(SessionRequest),
}

impl HarnessSetting {
    /// The harness the setting names: the name, or the mapping's `kind` as takt names it.
    #[must_use]
    pub fn harness(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Session(request) => request.harness(),
        }
    }
}

/// metaharness's session request as a `harness` mapping gives it: its admitted keys as written,
/// under metaharness's names, `kind` one of its three and `max-turns` a whole number.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRequest {
    harness: &'static str,
    settings: serde_json::Map<String, serde_json::Value>,
}

impl SessionRequest {
    /// The harness the request runs: `claude`, `codex`, or `harness` for b10x-harness.
    #[must_use]
    pub fn harness(&self) -> &str {
        self.harness
    }

    /// The most turns the session may take, when the mapping gives `max-turns`.
    #[must_use]
    pub fn max_turns(&self) -> Option<u32> {
        self.settings
            .get("max-turns")
            .and_then(serde_json::Value::as_u64)
            .and_then(|turns| u32::try_from(turns).ok())
    }
}

impl SessionRequest {
    /// The request a run takes: `base`, the file's mapping, with each flag's setting written over
    /// its key, under `harness`, the harness the run takes and its `kind`. `None` when neither the
    /// file nor a flag gives a setting, and when `harness` is `none`, which starts no session. A
    /// setting refused under `harness` is an error, named as the file's refusal names it; under
    /// `none` a setting the file or a flag gives is refused, since it would be dropped.
    pub fn with_flags(
        base: Option<&Self>,
        harness: &str,
        flags: SessionFlags<'_>,
    ) -> Result<Option<Self>> {
        if flags.max_turns == Some(0) {
            return Err(anyhow!(
                "the command line's --max-turns is a whole number above zero, no other value; nothing was started"
            ));
        }
        let mut settings = base
            .map(|request| request.settings.clone())
            .unwrap_or_default();
        for (key, value) in flags.settings() {
            settings.insert(key.to_owned(), value);
        }
        if let Some(detail) = refused_setting(&settings, harness, flags.max_turns.is_some()) {
            return Err(anyhow!(detail));
        }
        let (harness, kind) = match harness {
            "claude" => ("claude", "claude"),
            "codex" => ("codex", "codex"),
            "harness" => ("harness", "b10x"),
            _ => {
                let given: Vec<&str> = settings
                    .keys()
                    .map(String::as_str)
                    .filter(|key| *key != "kind")
                    .collect();
                if given.is_empty() {
                    return Ok(None);
                }
                return Err(anyhow!(
                    "the session settings {} are given with --harness {harness}, which starts no session, so they would be dropped; nothing was started",
                    given.join(", ")
                ));
            }
        };
        if base.is_none() && settings.is_empty() {
            return Ok(None);
        }
        settings.insert("kind".to_owned(), serde_json::Value::from(kind));
        Ok(Some(Self { harness, settings }))
    }
}

/// The session settings a command line's flags give, under the names metaharness's `run` takes
/// them by. A flag not given is `None`, `false` or empty.
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionFlags<'a> {
    /// `--effort`.
    pub effort: Option<&'a str>,
    /// `--max-turns`.
    pub max_turns: Option<u32>,
    /// `--max-budget-usd`.
    pub max_budget_usd: Option<&'a str>,
    /// `--strict-version`.
    pub strict_version: bool,
    /// `--context`, each file given.
    pub context: &'a [PathBuf],
}

impl SessionFlags<'_> {
    /// Each setting given, under its key.
    fn settings(&self) -> Vec<(&'static str, serde_json::Value)> {
        let mut settings = Vec::new();
        if let Some(effort) = self.effort {
            settings.push(("effort", serde_json::Value::from(effort)));
        }
        if let Some(turns) = self.max_turns {
            settings.push(("max-turns", serde_json::Value::from(turns)));
        }
        if let Some(usd) = self.max_budget_usd {
            settings.push(("max-budget-usd", serde_json::Value::from(usd)));
        }
        if self.strict_version {
            settings.push(("strict-version", serde_json::Value::from(true)));
        }
        if !self.context.is_empty() {
            let files: Vec<serde_json::Value> = self
                .context
                .iter()
                .map(|path| serde_json::Value::from(path.to_string_lossy().into_owned()))
                .collect();
            settings.push(("context", serde_json::Value::Array(files)));
        }
        settings
    }
}

/// The refusal of a key the mapping admits but takt's launch does not carry yet, naming it, or
/// `None` for a key it carries. A file's key and a flag's are refused alike.
fn not_carried_yet(key: &str) -> Option<String> {
    HARNESS_KEYS_NOT_CARRIED.contains(&key).then(|| {
        format!(
            "`{key}` in its `harness` mapping is admitted but takt's launch does not carry it yet, so the run is refused rather than run without it"
        )
    })
}

/// The refusal of `max-turns` in a file's `harness` mapping under `kind: codex`.
const CODEX_MAX_TURNS_REFUSED: &str = "`max-turns` under `kind: codex` in its `harness` mapping is refused: takt's Codex launch takes no turn ceiling, so the value would be dropped";

/// The refusal of `--max-turns` on the command line under the codex harness, which names no mapping.
const CODEX_FLAG_MAX_TURNS_REFUSED: &str = "the command line's --max-turns is refused under the codex harness: takt's Codex launch takes no turn ceiling, so the value would be dropped";

/// The refusal of the first setting `settings` holds that `harness` does not take, or `None`;
/// `turns_from_flag` says the `max-turns` it holds is the command line's.
fn refused_setting(
    settings: &serde_json::Map<String, serde_json::Value>,
    harness: &str,
    turns_from_flag: bool,
) -> Option<String> {
    HARNESS_KEYS_NOT_CARRIED
        .iter()
        .filter(|key| settings.contains_key(**key))
        .find_map(|key| not_carried_yet(key))
        .or_else(|| {
            (harness == "codex" && settings.contains_key("max-turns")).then(|| {
                if turns_from_flag {
                    CODEX_FLAG_MAX_TURNS_REFUSED
                } else {
                    CODEX_MAX_TURNS_REFUSED
                }
                .to_owned()
            })
        })
}

/// The keys a `harness` mapping admits: metaharness's `run` settings, under their own names.
const HARNESS_KEYS: &[&str] = &[
    "kind",
    "effort",
    "max-turns",
    "max-budget-usd",
    "strict-version",
    "context",
];

/// The keys a `harness` mapping admits but that takt's launch cannot carry yet: loom's partial
/// RunSpec and metaharness's session request are not released with them. Refused, not dropped.
const HARNESS_KEYS_NOT_CARRIED: &[&str] =
    &["effort", "max-budget-usd", "strict-version", "context"];

/// takt's own keys, its confinement, which a `harness` mapping refuses as takt's.
const TAKT_KEYS: &[&str] = &[
    "cwd",
    "scratch-cwd",
    "frame",
    "decisions",
    "hermetic",
    "driver",
    "hooks",
    "tool-surface",
    "catalogue",
    "substrate",
    "substrate-embedded",
    "cgroup-root",
    "write-scope",
    "process-write-subtree",
    "allow-program",
    "plugin",
    "plugin-dir",
    "spec",
];

/// The endpoint keys, which a `harness` mapping refuses as the run's `model` and `llm` section's;
/// every `subscription-token-` key is one too.
const ENDPOINT_KEYS: &[&str] = &[
    "model",
    "model-endpoint",
    "model-wire",
    "credentials",
    "api-key-secret",
];

/// Whose a refused `harness` key is (`takt.work.HarnessSettingOwner`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HarnessSettingOwner {
    Takt,
    Run,
}

/// A `harness` mapping key refused before any case opens: the key and whose it is
/// (`takt.work.HarnessSettingRefused`). A key the mapping does not admit is takt's to refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HarnessSettingRefused {
    key: String,
    owner: HarnessSettingOwner,
}

impl HarnessSettingRefused {
    fn of(key: &str) -> Self {
        let owner = if ENDPOINT_KEYS.contains(&key) || key.starts_with("subscription-token-") {
            HarnessSettingOwner::Run
        } else {
            HarnessSettingOwner::Takt
        };
        Self {
            key: key.to_owned(),
            owner,
        }
    }

    fn message(&self) -> String {
        let key = &self.key;
        match self.owner {
            HarnessSettingOwner::Run => format!(
                "`{key}` belongs to the run's model and llm section, not to the `harness` mapping"
            ),
            HarnessSettingOwner::Takt if TAKT_KEYS.contains(&key.as_str()) => format!(
                "`{key}` is takt's own, its confinement, which the `harness` mapping does not take"
            ),
            HarnessSettingOwner::Takt => format!(
                "`{key}` is not admitted in the `harness` mapping, which admits {}",
                HARNESS_KEYS.join(", ")
            ),
        }
    }
}

/// The refusal of the `harness` member of the file at `path`, detailed as `detail` says; it
/// repeats no value written in the file.
fn harness_refused(path: &Path, detail: &str) -> anyhow::Error {
    anyhow!(
        "the run configuration file {} is refused: {detail}",
        path.display()
    )
}

/// The `harness` member `member` holds: a string, the name `--harness` takes, or a mapping, the
/// session request; any other value is refused.
fn harness_setting(path: &Path, member: &serde_yaml_ng::Value) -> Result<HarnessSetting> {
    match member {
        serde_yaml_ng::Value::String(name)
            if matches!(name.as_str(), "claude" | "codex" | "harness" | "none") =>
        {
            Ok(HarnessSetting::Name(name.clone()))
        }
        serde_yaml_ng::Value::String(_) => Err(harness_refused(
            path,
            "its `harness` names no harness: claude, codex, harness or none, or it is a mapping of metaharness's session request",
        )),
        serde_yaml_ng::Value::Mapping(mapping) => {
            session_request(path, mapping).map(HarnessSetting::Session)
        }
        _ => Err(harness_refused(
            path,
            "its `harness` is a name, a string, or a mapping of metaharness's session request",
        )),
    }
}

/// The session request `mapping` holds: each key admitted or refused by name, in order, then
/// `kind` and `max-turns` checked. No value is repeated.
fn session_request(path: &Path, mapping: &serde_yaml_ng::Mapping) -> Result<SessionRequest> {
    let mut settings = serde_json::Map::new();
    for (key, value) in mapping {
        let Some(key) = key.as_str() else {
            return Err(harness_refused(
                path,
                "its `harness` mapping holds a key that is not a name",
            ));
        };
        if !HARNESS_KEYS.contains(&key) {
            return Err(harness_refused(
                path,
                &HarnessSettingRefused::of(key).message(),
            ));
        }
        let value = serde_json::to_value(value).map_err(|_| {
            harness_refused(
                path,
                "its `harness` mapping holds a value that has no JSON form",
            )
        })?;
        settings.insert(key.to_owned(), value);
    }
    let harness = match settings.get("kind") {
        None => {
            return Err(harness_refused(
                path,
                "its `harness` mapping names no `kind`: claude, codex or b10x",
            ));
        }
        Some(kind) => match kind.as_str() {
            Some("claude") => "claude",
            Some("codex") => "codex",
            Some("b10x") => "harness",
            _ => {
                return Err(harness_refused(
                    path,
                    "the `kind` of its `harness` mapping is claude, codex or b10x, no other value",
                ));
            }
        },
    };
    if let Some(detail) = refused_setting(&settings, harness, false) {
        return Err(harness_refused(path, &detail));
    }
    let request = SessionRequest { harness, settings };
    if request.settings.contains_key("max-turns")
        && request.max_turns().is_none_or(|turns| turns == 0)
    {
        return Err(harness_refused(
            path,
            "`max-turns` in its `harness` mapping is a whole number above zero, no other value",
        ));
    }
    Ok(request)
}

const SHAPE: &str = "a run configuration file holds `iterations` and `hours`, numbers, `tokens`, a whole number above zero, `story`, `harness`, `model`, `endpoint_file`, `endpoint_token_env`, `forge`, `forge_instance`, `forge_endpoint`, `test_cmd`, `state_dir`, `records` and `protocol`, strings, `continue_past_red_base`, a boolean, `scan`, `none`, `llm`, an `llm.routes/1` document or `{path: <file>}`, `transport`, glab or api, `credential`, a mapping naming an env variable or a file, `forge_commands`, a mapping of argument lists, `local_checks`, an argument list, `setup`, a list of commands each with a name and an argv, and `protocols`, a mapping of a protocol to its declared inputs, each optional; `records` is `default` or `trial`";

/// The largest file the `llm` member may name by path, in bytes; a larger one is refused before it is read.
const LLM_DOCUMENT_BOUND: u64 = 2 * 1024 * 1024;

/// Why an llm document is not read: its file is not a regular file, or it holds more than the bound.
enum Unread {
    NotRegular,
    Larger,
}

/// The text of the llm document at `path`, read through one open handle: the handle is checked to
/// be a regular file of at most the bound, and no more than the bound plus one byte is read, so a
/// file that grows after its metadata is read is refused too. Opened without blocking, so a named
/// pipe is refused rather than waited on.
fn read_llm_document(path: &Path) -> std::io::Result<std::result::Result<String, Unread>> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Ok(Err(Unread::NotRegular));
    }
    if metadata.len() > LLM_DOCUMENT_BOUND {
        return Ok(Err(Unread::Larger));
    }
    Ok(read_bounded(file)?.ok_or(Unread::Larger))
}

/// The text `reader` yields when it holds at most [`LLM_DOCUMENT_BOUND`] bytes, or `None` once more
/// has arrived: the reader is read through `take`, so no more than the bound plus one byte is ever
/// read from it.
fn read_bounded(reader: impl std::io::Read) -> std::io::Result<Option<String>> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    reader
        .take(LLM_DOCUMENT_BOUND + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LLM_DOCUMENT_BOUND {
        return Ok(None);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

/// The refusal of the file at `path`, at `line` when it is known, repeating nothing written in it.
fn refused(path: &Path, line: Option<usize>) -> anyhow::Error {
    match line {
        Some(line) => anyhow!(
            "the run configuration file {} is refused at line {line}: {SHAPE}",
            path.display()
        ),
        None => anyhow!(
            "the run configuration file {} is refused: {SHAPE}",
            path.display()
        ),
    }
}

/// A member as written, null included: `Some` whenever the key is there, so a key written
/// with no value is told from one not written (`#[serde(default)]` gives `None` for that).
fn written<'de, D>(deserializer: D) -> std::result::Result<Option<serde_yaml_ng::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <serde_yaml_ng::Value as serde::Deserialize>::deserialize(deserializer).map(Some)
}

/// The refusal of a run file member whose value does not read as an argument, at the line it is
/// written on, repeating nothing written there: a `tokens` that is no whole number above zero,
/// as `--tokens x` is, and a `forge_endpoint` that is no URL the forge calls. `takt run` exits 2
/// on it, before any case opens.
#[derive(Debug)]
pub struct MemberRefused {
    /// The run file.
    pub path: PathBuf,
    /// The line the member is written on, from 1.
    pub line: usize,
    /// The member.
    pub member: &'static str,
    /// The rule its value breaks, worded after the member's name.
    pub rule: &'static str,
}

/// [`MemberRefused`] by the name `takt run` matches it under.
pub type TokensRefused = MemberRefused;

/// The rule a `tokens` that is no whole number above zero breaks.
const TOKENS_RULE: &str = "is a whole number above zero, the tokens the run stops at";

impl std::fmt::Display for MemberRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the run configuration file {} is refused at line {}: `{}` {}",
            self.path.display(),
            self.line,
            self.member,
            self.rule
        )
    }
}

impl std::error::Error for MemberRefused {}

/// Whether `key` is a name takt or Loom declares: one the shape text quotes, a member of a
/// `loom.agent/1` document, or `takt`. Only such a key is named in a refusal; the comparison is
/// whole names, so a key that quotes a stretch of the shape text is not one.
fn declared(key: &str) -> bool {
    SHAPE.split('`').skip(1).step_by(2).any(|name| name == key)
        || LOOM_AGENT_MEMBERS.contains(&key)
        || key == "takt"
}

/// The refusal of the file at `path` for a key written twice in one `container` (a YAML mapping,
/// a TOML table), at the second one's `line`, naming the key when it is one takt or Loom
/// declares and repeating nothing else written in it.
fn refused_duplicate(
    path: &Path,
    line: usize,
    key: Option<&str>,
    container: &str,
) -> anyhow::Error {
    match key.filter(|key| declared(key)) {
        Some(key) => anyhow!(
            "the run configuration file {} is refused at line {line}: it holds the key `{key}` \
             twice in one {container}",
            path.display()
        ),
        None => anyhow!(
            "the run configuration file {} is refused at line {line}: it holds a key twice in one \
             {container}",
            path.display()
        ),
    }
}

/// The key a TOML key is when it is `written` bare, or quoted with no escape in it; none when it
/// is written otherwise.
fn toml_key(written: &str) -> Option<&str> {
    let written = written.trim();
    let bare = !written.is_empty()
        && written
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if bare {
        return Some(written);
    }
    let quoted = |mark: char| {
        written
            .strip_prefix(mark)
            .and_then(|inner| inner.strip_suffix(mark))
            .filter(|inner| !inner.contains(['\\', '"', '\'']))
    };
    quoted('"').or_else(|| quoted('\''))
}

/// Where serde_yaml_ng's `error` found a key twice in the YAML `text`: the line of the second
/// one, from one, and the key when it is a string. serde_yaml_ng locates that error at the start
/// of the mapping and names the key in its message (`duplicate entry with key "a"`); the
/// parser's events are read again ([`repeated_key`]), and the key they find is taken only when
/// the message names it. None for every other error, and where the events do not settle it.
fn duplicate_yaml_key(text: &str, error: &serde_yaml_ng::Error) -> Option<(usize, Option<String>)> {
    let at = error.location()?;
    let message = error.to_string();
    let message = message
        .strip_suffix(&format!(" at line {} column {}", at.line(), at.column()))
        .unwrap_or(&message);
    let (line, key) = repeated_key(text, at.index())?;
    let named = match &key {
        serde_yaml_ng::Value::Null => "with null key".to_owned(),
        serde_yaml_ng::Value::Bool(boolean) => format!("with key `{boolean}`"),
        serde_yaml_ng::Value::Number(number) => format!("with key {number}"),
        serde_yaml_ng::Value::String(string) => format!("with key {string:?}"),
        _ => return None,
    };
    message
        .ends_with(&format!("duplicate entry {named}"))
        .then(|| (line, key.as_str().map(str::to_owned)))
}

/// A plain scalar's `text` as serde_yaml_ng resolves it (null, a boolean, a number or a string),
/// read as a mapping's value; none when that reading is not one scalar or does not give the
/// text back as the string it is.
fn plain_scalar(text: &str) -> Option<serde_yaml_ng::Value> {
    let read: serde_yaml_ng::Mapping = match serde_yaml_ng::from_str(&format!("k: {text}")) {
        Ok(read) => read,
        Err(_) => {
            let extended = format!("{text}a");
            let read: serde_yaml_ng::Mapping =
                serde_yaml_ng::from_str(&format!("k: {extended}")).ok()?;
            return match read.into_iter().next()?.1 {
                serde_yaml_ng::Value::String(value) if value == extended => {
                    Some(serde_yaml_ng::Value::String(text.to_owned()))
                }
                _ => None,
            };
        }
    };
    let (_, value) = read.into_iter().next()?;
    match &value {
        serde_yaml_ng::Value::String(string) if string != text => None,
        serde_yaml_ng::Value::Null
        | serde_yaml_ng::Value::Bool(_)
        | serde_yaml_ng::Value::Number(_)
        | serde_yaml_ng::Value::String(_) => Some(value),
        _ => None,
    }
}

/// The line, from one, of the first key that repeats an earlier key of the mapping starting at
/// byte `mapping` of the YAML `text`, and the key, read from the parser's own events: a scalar
/// or a comment is never read as a key, and a key is compared as serde_yaml_ng compares it, as
/// the value it resolves to. None when the mapping holds no such key, when one of its keys
/// before it cannot be resolved, or when the parser cannot be set up.
fn repeated_key(text: &str, mapping: usize) -> Option<(usize, serde_yaml_ng::Value)> {
    use unsafe_libyaml as unsafe_yaml;
    /// An open sequence or mapping; a mapping's next node is a key or a value, and the one
    /// starting at `mapping` holds the keys read so far.
    struct Open {
        mapping: bool,
        key_next: bool,
        keys: Option<std::collections::HashSet<serde_yaml_ng::Value>>,
    }
    let size = u64::try_from(text.len()).ok()?;
    let mapping = u64::try_from(mapping).ok()?;
    let mut parser = Box::<unsafe_yaml::yaml_parser_t>::new_uninit();
    let parser = parser.as_mut_ptr();
    let mut open: Vec<Open> = Vec::new();
    let mut anchors = std::collections::HashMap::new();
    let mut found = None;
    // Marks count UTF-8 bytes. Index their file lines once so looking up every event
    // stays linear even for large mappings, including CR-only and CRLF line endings.
    let mut lines = vec![1usize];
    let mut line = 1usize;
    let mut after_cr = false;
    for ch in text.bytes() {
        if ch == b'\r' || (ch == b'\n' && !after_cr) {
            line = line.saturating_add(1);
        }
        after_cr = ch == b'\r';
        lines.push(line);
    }
    // SAFETY: as `anchor_line`'s (`loop_scorecard`): the parser lives in its own allocation,
    // never moved, initialized before any other call and deleted after the last; the input
    // outlives it. Each event is filled by `yaml_parser_parse` before it is read, its union read
    // only under the variant its type names, and deleted before the next.
    unsafe {
        if unsafe_yaml::yaml_parser_initialize(parser).fail {
            return None;
        }
        unsafe_yaml::yaml_parser_set_encoding(parser, unsafe_yaml::YAML_UTF8_ENCODING);
        unsafe_yaml::yaml_parser_set_input_string(parser, text.as_ptr(), size);
        let mut event = std::mem::MaybeUninit::<unsafe_yaml::yaml_event_t>::uninit();
        loop {
            let event = event.as_mut_ptr();
            if unsafe_yaml::yaml_parser_parse(parser, event).fail {
                break;
            }
            let kind = (*event).type_;
            // libyaml counts Unicode line separators as breaks, but file line numbers count
            // CR, LF and CRLF. Its index counts UTF-8 bytes.
            let index = usize::try_from((*event).start_mark.index).unwrap_or(usize::MAX);
            let line = lines.get(index).copied().unwrap_or(line);

            let counted = open
                .last()
                .is_some_and(|parent| parent.mapping && parent.key_next && parent.keys.is_some());
            // Some(Some(key)): a key of the mapping, resolved; Some(None): one not resolved.
            let key = match kind {
                unsafe_yaml::YAML_SCALAR_EVENT => {
                    let scalar = (*event).data.scalar;
                    let bytes = std::slice::from_raw_parts(
                        scalar.value,
                        usize::try_from(scalar.length).unwrap_or(0),
                    );
                    let resolved = match std::str::from_utf8(bytes) {
                        Ok(text) if !scalar.tag.is_null() => {
                            let tag = std::ffi::CStr::from_ptr(scalar.tag.cast()).to_bytes();
                            if tag == b"tag:yaml.org,2002:str" {
                                Some(serde_yaml_ng::Value::String(text.to_owned()))
                            } else if matches!(
                                tag,
                                b"tag:yaml.org,2002:null"
                                    | b"tag:yaml.org,2002:bool"
                                    | b"tag:yaml.org,2002:int"
                                    | b"tag:yaml.org,2002:float"
                            ) {
                                let tag = std::str::from_utf8(tag).ok();
                                tag.and_then(|tag| {
                                    let quoted = serde_yaml_ng::to_string(text).ok()?;
                                    serde_yaml_ng::from_str(&format!("!<{tag}> {quoted}")).ok()
                                })
                            } else if tag.starts_with(b"!") {
                                let value = if scalar.style == unsafe_yaml::YAML_PLAIN_SCALAR_STYLE
                                {
                                    plain_scalar(text)
                                } else {
                                    Some(serde_yaml_ng::Value::String(text.to_owned()))
                                };
                                std::str::from_utf8(tag).ok().and_then(|tag| {
                                    value.map(|value| {
                                        serde_yaml_ng::Value::Tagged(Box::new(
                                            serde_yaml_ng::value::TaggedValue {
                                                tag: serde_yaml_ng::value::Tag::new(tag),
                                                value,
                                            },
                                        ))
                                    })
                                })
                            } else {
                                Some(serde_yaml_ng::Value::String(text.to_owned()))
                            }
                        }
                        Ok(text) if scalar.style == unsafe_yaml::YAML_PLAIN_SCALAR_STYLE => {
                            plain_scalar(text)
                        }
                        Ok(text) => Some(serde_yaml_ng::Value::String(text.to_owned())),
                        Err(_) => None,
                    };
                    if !scalar.anchor.is_null() {
                        let anchor = std::ffi::CStr::from_ptr(scalar.anchor.cast())
                            .to_bytes()
                            .to_vec();
                        anchors.insert(anchor, Some(resolved.clone()));
                    }
                    counted.then_some(resolved)
                }
                unsafe_yaml::YAML_ALIAS_EVENT if counted => {
                    let anchor = std::ffi::CStr::from_ptr((*event).data.alias.anchor.cast());
                    match anchors.get(anchor.to_bytes()) {
                        Some(Some(Some(value))) => Some(Some(value.clone())),
                        Some(Some(None)) => Some(None),
                        // A collection cannot equal a scalar key. Keep looking for a
                        // scalar duplicate; the deserializer's error corroborates it.
                        Some(None) => None,
                        None => Some(None),
                    }
                }
                _ => None,
            };
            let complete = match kind {
                unsafe_yaml::YAML_SCALAR_EVENT | unsafe_yaml::YAML_ALIAS_EVENT => true,
                unsafe_yaml::YAML_SEQUENCE_START_EVENT | unsafe_yaml::YAML_MAPPING_START_EVENT => {
                    let is_mapping = kind == unsafe_yaml::YAML_MAPPING_START_EVENT;
                    let anchor = if is_mapping {
                        (*event).data.mapping_start.anchor
                    } else {
                        (*event).data.sequence_start.anchor
                    };
                    if !anchor.is_null() {
                        anchors.insert(
                            std::ffi::CStr::from_ptr(anchor.cast()).to_bytes().to_vec(),
                            None,
                        );
                    }
                    open.push(Open {
                        mapping: is_mapping,
                        key_next: true,
                        keys: (is_mapping && (*event).start_mark.index == mapping)
                            .then(std::collections::HashSet::new),
                    });
                    false
                }
                unsafe_yaml::YAML_SEQUENCE_END_EVENT | unsafe_yaml::YAML_MAPPING_END_EVENT => {
                    open.pop();
                    true
                }
                _ => false,
            };
            let end = kind == unsafe_yaml::YAML_STREAM_END_EVENT;
            unsafe_yaml::yaml_event_delete(event);
            match key {
                Some(None) => break,
                Some(Some(key)) => {
                    let keys = open.last_mut().and_then(|parent| parent.keys.as_mut());
                    if let Some(keys) = keys
                        && !keys.insert(key.clone())
                    {
                        found = Some((line, key));
                        break;
                    }
                }
                None => {}
            }
            if complete && let Some(parent) = open.last_mut().filter(|parent| parent.mapping) {
                parent.key_next = !parent.key_next;
            }
            if end {
                break;
            }
        }
        unsafe_yaml::yaml_parser_delete(parser);
    }
    found
}

/// `message` without the line and column llm appends to a YAML document's refusal: they locate the
/// copy of an inline section that takt re-serialises, not the run file.
fn without_position(message: &str) -> &str {
    message
        .split_once(" at line ")
        .map_or(message, |(head, _)| head)
}

/// The `llm.routes/1` document `llm` holds, validated by llm. A mapping whose only
/// key is `path` names a file beside the run file, parsed by `Routes::parse_toml` when it ends in
/// `.toml` and by `Routes::parse` when it ends in `.yaml` or `.yml`; any other mapping is the
/// whole document, re-serialised to YAML text and parsed by `Routes::parse`. llm's refusal is
/// relayed as it stands, and nothing here repeats a value written in the file.
fn llm_document(path: &Path, llm: &serde_yaml_ng::Value) -> Result<(Routes, Vec<String>)> {
    let shape = || {
        anyhow!(
            "the run configuration file {} is refused: its `llm` member is neither an \
             `llm.routes/1` document nor `{{path: <file>}}`",
            path.display()
        )
    };
    let (routes, variables) = match llm
        .as_mapping()
        .filter(|mapping| mapping.len() == 1)
        .and_then(|mapping| mapping.get("path"))
    {
        Some(file) => {
            let file = file.as_str().ok_or_else(shape)?;
            let name = Path::new(file);
            if name.is_absolute() || name.components().any(|part| part == Component::ParentDir) {
                return Err(anyhow!(
                    "the run configuration file {} is refused: its `llm` member names a file that is absolute or has a `..` part, and a file is read beside the run file",
                    path.display()
                ));
            }
            let directory = match path.parent() {
                Some(directory) if !directory.as_os_str().is_empty() => directory,
                _ => Path::new("."),
            };
            let file = directory.join(name);
            let toml = match name.extension().and_then(|extension| extension.to_str()) {
                Some("toml") => true,
                Some("yaml" | "yml") => false,
                _ => {
                    return Err(anyhow!(
                        "the llm document {} is refused: its name ends in neither .toml, .yaml \
                         nor .yml",
                        file.display()
                    ));
                }
            };
            let real = std::fs::canonicalize(&file)
                .with_context(|| format!("the llm document {}", file.display()))?;
            let real_directory = std::fs::canonicalize(directory)
                .with_context(|| format!("the llm document {}", file.display()))?;
            if !real.starts_with(&real_directory) {
                return Err(anyhow!(
                    "the run configuration file {} is refused: its `llm` member names a file that resolves outside the run file's directory",
                    path.display()
                ));
            }
            let text = match read_llm_document(&real)
                .with_context(|| format!("the llm document {}", file.display()))?
            {
                Ok(text) => text,
                Err(Unread::NotRegular) => {
                    return Err(anyhow!(
                        "the run configuration file {} is refused: its `llm` member names a file that is not a regular file",
                        path.display()
                    ));
                }
                Err(Unread::Larger) => {
                    return Err(anyhow!(
                        "the run configuration file {} is refused: its `llm` member names a file larger than the 2 MiB bound (2097152 bytes)",
                        path.display()
                    ));
                }
            };
            let document: serde_json::Value = if toml {
                toml::from_str(&text)
                    .with_context(|| format!("the llm document {}", file.display()))?
            } else {
                serde_yaml_ng::from_str(&text)
                    .with_context(|| format!("the llm document {}", file.display()))?
            };
            let parsed = if toml {
                Routes::parse_toml(&text)
            } else {
                Routes::parse(&text)
            };
            let routes = parsed.with_context(|| format!("the llm document {}", file.display()))?;
            (routes, secret_variables(&document))
        }
        None if llm.as_mapping().is_some_and(|mapping| !mapping.is_empty()) => {
            let text = serde_yaml_ng::to_string(llm).map_err(|_| shape())?;
            let routes = Routes::parse(&text).map_err(|error| {
                anyhow!(
                    "the run configuration file {} is refused: its `llm` section is not valid: {:?}: {}",
                    path.display(),
                    error.code,
                    without_position(&error.message)
                )
            })?;
            let document = serde_json::to_value(llm).map_err(|_| shape())?;
            (routes, secret_variables(&document))
        }
        None => return Err(shape()),
    };
    Ok((routes, variables))
}

/// The run configuration in `text`, TOML. A key written twice is refused at the second one's
/// line, where the toml parser points, named when takt or Loom declares it.
fn from_toml(path: &Path, text: &str) -> Result<RunConfig> {
    toml::from_str(text).map_err(|error| {
        let line = error
            .span()
            .and_then(|span| text.get(..span.start))
            .map(|before| before.matches('\n').count() + 1);
        match line {
            Some(line) if error.message() == "duplicate key" => {
                let key = error.span().and_then(|span| text.get(span));
                refused_duplicate(path, line, key.and_then(toml_key), "table")
            }
            _ => refused(path, line),
        }
    })
}

/// The run configuration in `text`, YAML: a `loom.agent/1` document's `takt` member, nothing
/// when it has none, or else the whole document. A document with a `format` other than
/// `loom.agent/1`, a member of one that is neither Loom's nor `takt`, or a YAML tag anywhere is
/// refused without repeating what is written. A key written twice is refused at the second one's
/// line, as TOML refuses it ([`duplicate_yaml_key`]).
fn from_yaml(path: &Path, text: &str) -> Result<RunConfig> {
    let line = |error: &serde_yaml_ng::Error| error.location().map(|at| at.line());
    let document: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(text).map_err(|error| match duplicate_yaml_key(text, &error) {
            Some((at, key)) => refused_duplicate(path, at, key.as_deref(), "mapping"),
            None => refused(path, line(&error)),
        })?;
    if tagged(&document) {
        return Err(anyhow!(
            "the run configuration file {} is refused: it carries a YAML tag, which no member \
             declares",
            path.display()
        ));
    }
    let format = document
        .as_mapping()
        .and_then(|mapping| mapping.get("format"));
    match format {
        None if document.is_null() => Ok(RunConfig::default()),
        None => {
            let _: RunConfig =
                serde_yaml_ng::from_str(text).map_err(|error| refused(path, line(&error)))?;
            strict(path, &document)
        }
        Some(format) if format.as_str() == Some(LOOM_AGENT_FORMAT) => {
            let mut keys = document
                .as_mapping()
                .into_iter()
                .flatten()
                .map(|(key, _)| key);
            if keys.any(|key| {
                key.as_str()
                    .is_none_or(|key| key != "takt" && !LOOM_AGENT_MEMBERS.contains(&key))
            }) {
                return Err(anyhow!(
                    "the run configuration file {} is refused: a {LOOM_AGENT_FORMAT} document \
                     holds a member that is neither one of Loom's ({}) nor `takt`",
                    path.display(),
                    LOOM_AGENT_MEMBERS.join(", ")
                ));
            }
            let agent: LoomAgentDocument =
                serde_yaml_ng::from_str(text).map_err(|error| refused(path, line(&error)))?;
            let mut config = match (agent.takt, document.get("takt")) {
                (Some(_), Some(takt)) => strict(path, takt)?,
                _ => RunConfig::default(),
            };
            // The document's `runtime.state_dir` is the default: `takt.state_dir` names one first.
            let runtime = match agent.runtime {
                Some(runtime) => loom_runtime_state_dir(path, runtime)?,
                None => None,
            };
            config.state_dir = config.state_dir.or(runtime);
            Ok(config)
        }
        Some(_) => Err(anyhow!(
            "the run configuration file {} is refused: its `format` is not {LOOM_AGENT_FORMAT}, \
             and a run configuration file of its own has no `format`",
            path.display()
        )),
    }
}

impl RunConfig {
    /// The run configuration file at `path`: TOML when its name ends in `.toml`, YAML when it
    /// ends in `.yaml` or `.yml`, any other refused. A YAML `loom.agent/1` document gives its
    /// `takt` member, or nothing without one, and a `state_dir` its `runtime.state_dir` when its
    /// `takt` member names none. Any other key, or a value of another type, is
    /// refused naming the path and, for a parse, the line, never a value written there. A
    /// relative `endpoint_file` or `state_dir` is made relative to the file's directory.
    pub fn read(path: &Path) -> Result<Self> {
        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("toml" | "yaml" | "yml")) {
            return Err(anyhow!(
                "the run configuration file {} is refused: its name ends in neither .toml, .yaml \
                 nor .yml",
                path.display()
            ));
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("the run configuration file {}", path.display()))?;
        // A byte order mark opens no member: it is read past in either form.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let mut config = if extension == Some("toml") {
            from_toml(path, text)?
        } else {
            from_yaml(path, text)?
        };
        if let Some(member) = config.tokens_member.take() {
            match member.as_u64().filter(|tokens| *tokens > 0) {
                Some(tokens) => config.tokens = Some(tokens),
                None => {
                    return Err(anyhow::Error::new(MemberRefused {
                        path: path.to_owned(),
                        line: locate(text, extension == Some("toml"), &["tokens"]).unwrap_or(1),
                        member: "tokens",
                        rule: TOKENS_RULE,
                    }));
                }
            }
        }
        if let Some(endpoint) = &config.forge_endpoint
            && let Err(rule) = crate::forge::connectors::check_endpoint(endpoint)
        {
            let toml = extension == Some("toml");
            return Err(anyhow::Error::new(MemberRefused {
                path: path.to_owned(),
                line: locate(text, toml, &["forge_endpoint"])
                    .or_else(|| locate(text, toml, &["takt", "forge_endpoint"]))
                    .unwrap_or(1),
                member: "forge_endpoint",
                rule,
            }));
        }
        if let Some(forge) = &config.forge
            && !crate::forge::FORGES.contains(&forge.as_str())
        {
            let toml = extension == Some("toml");
            let line = locate(text, toml, &["forge"])
                .or_else(|| locate(text, toml, &["takt", "forge"]))
                .unwrap_or(1);
            return Err(anyhow!(
                "the run configuration file {} is refused at line {line}: `forge` is {}, not `{}`",
                path.display(),
                forge_names(),
                forge.escape_debug()
            ));
        }
        if let Some(records) = &config.records
            && crate::case::Records::named(records).is_none()
        {
            return Err(anyhow!(
                "the run configuration file {} is refused: `records` is `default` or `trial`, no other value",
                path.display()
            ));
        }
        if let Some(section) = &config.commit
            && let Err(why) = crate::commit_style::Settings::of_section(section).check()
        {
            return Err(anyhow!(
                "the run configuration file {} is refused: {why}",
                path.display()
            ));
        }
        if let Some(member) = config.harness_member.take() {
            config.harness = Some(harness_setting(path, &member)?);
        }
        config.protocol_settings = ProtocolSettings::read(
            path,
            text,
            extension == Some("toml"),
            config.protocol.as_deref(),
            config.protocols_member.take().as_ref(),
        )?;
        if let Some(llm) = &config.llm {
            let (routes, variables) = llm_document(path, llm)?;
            config.llm_digest = Some(routes.digest().to_owned());
            config.llm_routes = Some(LlmRoutes::new(routes, variables));
        }
        let beside = |file: PathBuf| match path.parent() {
            Some(dir) if file.is_relative() => dir.join(file),
            _ => file,
        };
        config.endpoint_file = config.endpoint_file.take().map(beside);
        config.state_dir = config.state_dir.take().map(beside);
        config.credential = match config.credential.take() {
            Some(CredentialSource::File(file)) => Some(CredentialSource::File(beside(file))),
            other => other,
        };
        if let Some(why) = config.setup.as_deref().and_then(SetupCommand::refusal) {
            return Err(anyhow!(
                "the run configuration file {} is refused: {why}",
                path.display()
            ));
        }
        if let Some(instance) = &config.forge_instance
            && crate::forge::connectors::target_of(instance).is_none()
        {
            let toml = extension == Some("toml");
            let line = locate(text, toml, &["forge_instance"])
                .or_else(|| locate(text, toml, &["takt", "forge_instance"]))
                .unwrap_or(1);
            return Err(anyhow!(
                "the run configuration file {} is refused at line {line}: `forge_instance` is `<connector>` or `<connector>/<instance>`, the connector gitlab or github, the instance a word of letters, digits, `.`, `_` and `-`, not starting with `-`",
                path.display()
            ));
        }
        if config.local_checks.as_deref().is_some_and(|checks| {
            checks
                .first()
                .is_none_or(|program| program.trim().is_empty())
        }) {
            return Err(anyhow!(
                "the run configuration file {} is refused: `local_checks` is an argument list whose first element is the program, and it names none",
                path.display()
            ));
        }
        if let Some(commands) = &config.forge_commands {
            commands.check().map_err(|error| {
                anyhow!(
                    "the run configuration file {} is refused: {error}",
                    path.display()
                )
            })?;
        }
        Ok(config)
    }

    /// Sets the document `--llm-config` names, a `.toml`, `.yaml` or `.yml` file of at most the bound
    /// a run file's `llm` path takes, parsed by llm: it replaces the document the run file's `llm`
    /// member holds, and its digest is the run's `llm_digest`. The file is read as it stands, beside
    /// nothing: the flag names it from the working directory.
    pub fn set_llm_file(&mut self, path: &Path) -> Result<()> {
        let toml = match path.extension().and_then(|extension| extension.to_str()) {
            Some("toml") => true,
            Some("yaml" | "yml") => false,
            _ => {
                return Err(anyhow!(
                    "the llm document {} is refused: its name ends in neither .toml, .yaml nor .yml",
                    path.display()
                ));
            }
        };
        let text = match read_llm_document(path)
            .with_context(|| format!("the llm document {}", path.display()))?
        {
            Ok(text) => text,
            Err(Unread::NotRegular) => {
                return Err(anyhow!(
                    "the llm document {} is refused: it is not a regular file",
                    path.display()
                ));
            }
            Err(Unread::Larger) => {
                return Err(anyhow!(
                    "the llm document {} is refused: it is larger than the 2 MiB bound (2097152 bytes)",
                    path.display()
                ));
            }
        };
        let document: serde_json::Value = if toml {
            toml::from_str(&text).with_context(|| format!("the llm document {}", path.display()))?
        } else {
            serde_yaml_ng::from_str(&text)
                .with_context(|| format!("the llm document {}", path.display()))?
        };
        let parsed = if toml {
            Routes::parse_toml(&text)
        } else {
            Routes::parse(&text)
        };
        let routes = parsed.with_context(|| format!("the llm document {}", path.display()))?;
        self.llm_digest = Some(routes.digest().to_owned());
        self.llm_routes = Some(LlmRoutes::new(routes, secret_variables(&document)));
        Ok(())
    }
}

/// One input a run file's `protocols` section sets, with the line it is written on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSetting {
    /// The declared input's name.
    pub input: String,
    /// The value as text, a boolean as `true` or `false`.
    pub value: String,
    /// The line the input is written on, 1-based; the line of its section when it cannot be told.
    pub line: usize,
}

/// One protocol's entry of a run file's `protocols` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSection {
    /// The protocol as the key writes it: `<name>` for every revision, or `<name>/<major>`.
    pub protocol: String,
    /// The line the key is written on, 1-based.
    pub line: usize,
    /// The inputs it sets, in the file's order.
    pub inputs: Vec<InputSetting>,
}

/// The protocol a run file names and the inputs it sets, as [`RunConfig::read`] keeps them, with the
/// file's path and each entry's line, so that a refusal made later, once the protocol the case
/// opens on is known, names the file and the line.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProtocolSettings {
    /// The file they were read from; none when no file was.
    pub file: Option<PathBuf>,
    /// The `protocol` key and its line.
    pub protocol: Option<(String, usize)>,
    /// The `protocols` section's entries.
    pub sections: Vec<SettingsSection>,
}

impl ProtocolSettings {
    /// The refusal of the entry written on `line`, naming the file and the line and repeating no
    /// value written there.
    #[must_use]
    pub fn refusal(&self, line: usize, detail: &str) -> anyhow::Error {
        match &self.file {
            Some(file) => anyhow!(
                "the run configuration file {} is refused at line {line}: {detail}",
                file.display()
            ),
            None => anyhow!("the run configuration is refused: {detail}"),
        }
    }

    /// The settings the members hold, `protocol` and `protocols` as [`RunConfig`] read them from
    /// the file at `path`, whose text is `text`, TOML when `toml` says so. The shape is checked
    /// here: the protocol is `<name>/<major>`, and `protocols` is a mapping of a protocol to a
    /// mapping of inputs to a string or a boolean. A value is checked against the protocol's
    /// declared values later, by [`crate::protocol::resolve_settings`].
    fn read(
        path: &Path,
        text: &str,
        toml: bool,
        protocol: Option<&str>,
        protocols: Option<&serde_yaml_ng::Value>,
    ) -> Result<Self> {
        if protocol.is_none() && protocols.is_none() {
            return Ok(Self::default());
        }
        let mut settings = Self {
            file: Some(path.to_owned()),
            ..Self::default()
        };
        let at = |keys: &[&str]| locate(text, toml, keys).unwrap_or(1);
        if let Some(protocol) = protocol {
            let line = at(&["protocol"]);
            let well_formed = protocol.rsplit_once('/').is_some_and(|(name, major)| {
                !name.is_empty() && !major.is_empty() && major.chars().all(|c| c.is_ascii_digit())
            });
            if !well_formed {
                return Err(settings.refusal(
                    line,
                    "`protocol` names a protocol by its id, `<name>/<major>`, as `takt protocols` lists them",
                ));
            }
            settings.protocol = Some((protocol.to_owned(), line));
        }
        let Some(protocols) = protocols else {
            return Ok(settings);
        };
        let shape = "`protocols` is a mapping of a protocol to a mapping of its declared inputs, each set to a string or a boolean";
        let Some(mapping) = protocols.as_mapping() else {
            return Err(settings.refusal(at(&["protocols"]), shape));
        };
        for (name, inputs) in mapping {
            let Some(name) = name.as_str() else {
                return Err(settings.refusal(at(&["protocols"]), shape));
            };
            let line = at(&["protocols", name]);
            let Some(inputs) = inputs.as_mapping() else {
                return Err(settings.refusal(line, shape));
            };
            let mut section = SettingsSection {
                protocol: name.to_owned(),
                line,
                inputs: Vec::new(),
            };
            for (input, value) in inputs {
                let Some(input) = input.as_str() else {
                    return Err(settings.refusal(line, shape));
                };
                let line = at(&["protocols", name, input]);
                let value = match value {
                    serde_yaml_ng::Value::String(text) => text.clone(),
                    serde_yaml_ng::Value::Bool(flag) => flag.to_string(),
                    _ => return Err(settings.refusal(line, shape)),
                };
                if section.inputs.iter().any(|set| set.input == input) {
                    return Err(settings.refusal(line, "an input is set twice in one protocol"));
                }
                section.inputs.push(InputSetting {
                    input: input.to_owned(),
                    value,
                    line,
                });
            }
            settings.sections.push(section);
        }
        Ok(settings)
    }
}

/// The forges a run may name ([`crate::forge::FORGES`]) as a refusal lists them:
/// `none, local, gitlab, command or connectors`.
fn forge_names() -> String {
    let (last, rest) = crate::forge::FORGES
        .split_last()
        .expect("takt has at least one forge");
    format!("{} or {last}", rest.join(", "))
}

/// The 1-based line a member of the file is written on, `keys` the path from the top of the
/// document to it: the line of the last key found, or of the deepest one when a flow mapping
/// holds the rest on its line. `None` when not even the first key is found.
fn locate(text: &str, toml: bool, keys: &[&str]) -> Option<usize> {
    if toml {
        locate_toml(text, keys)
    } else {
        locate_yaml(text, keys)
    }
}

/// Whether `trimmed`, a line without its indentation, starts with the key `key` followed by
/// `separator`, the key bare or quoted.
fn starts_with_key(trimmed: &str, key: &str, separators: &[char]) -> bool {
    [key.to_owned(), format!("\"{key}\""), format!("'{key}'")]
        .iter()
        .any(|candidate| {
            trimmed
                .strip_prefix(candidate.as_str())
                .is_some_and(|rest| {
                    rest.trim_start()
                        .chars()
                        .next()
                        .is_some_and(|next| separators.contains(&next))
                })
        })
}

/// [`locate`] in a YAML document: each key is looked for below the line of the one before, in the
/// lines indented deeper than it.
fn locate_yaml(text: &str, keys: &[&str]) -> Option<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = None;
    let mut from = 0;
    let mut parent: Option<usize> = None;
    for key in keys {
        let mut hit = None;
        for (number, line) in lines.iter().enumerate().skip(from) {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let indent = line.len() - trimmed.len();
            if parent.is_some_and(|parent| indent <= parent) {
                break;
            }
            if starts_with_key(trimmed, key, &[':']) {
                hit = Some((number, indent, trimmed));
                break;
            }
        }
        let (number, indent, trimmed) = hit?;
        found = Some(number + 1);
        let after_colon = trimmed.split_once(':').map_or("", |(_, rest)| rest.trim());
        if !after_colon.is_empty() && !after_colon.starts_with('#') {
            return found;
        }
        from = number + 1;
        parent = Some(indent);
    }
    found
}

/// [`locate`] in a TOML document: a `[protocols.<name>]` table's own lines, a `[protocols]` table's,
/// or the top level's dotted and inline forms.
fn locate_toml(text: &str, keys: &[&str]) -> Option<usize> {
    let wanted: String = keys.join(".");
    let mut header = String::new();
    let mut best: Option<usize> = None;
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(table) = trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.trim_end().strip_suffix(']'))
        {
            header = table
                .trim_matches(['[', ']'])
                .chars()
                .filter(|c| !matches!(c, '"' | '\'' | ' '))
                .collect();
            let depth = keys.len();
            if depth > 0 && header == keys[..depth].join(".") {
                return Some(number + 1);
            }
            if keys.len() > 1 && header == keys[..keys.len() - 1].join(".") && best.is_none() {
                best = Some(number + 1);
            }
            continue;
        }
        let flat: String = trimmed
            .chars()
            .filter(|c| !matches!(c, '"' | '\'' | ' '))
            .collect();
        let qualified = if header.is_empty() {
            flat.clone()
        } else {
            format!("{header}.{flat}")
        };
        if qualified.starts_with(&wanted)
            && qualified[wanted.len()..]
                .chars()
                .next()
                .is_some_and(|next| matches!(next, '=' | '.'))
        {
            return Some(number + 1);
        }
        let (table_key, member) = (keys.first().copied().unwrap_or(""), keys.last().copied());
        if header == table_key
            && keys.len() > 1
            && starts_with_key(trimmed, keys[1], &['=', '.'])
            && (keys.len() == 2 || member.is_some_and(|member| trimmed.contains(member)))
        {
            return Some(number + 1);
        }
        if header.is_empty() && starts_with_key(trimmed, table_key, &['=', '.']) && best.is_none() {
            best = Some(number + 1);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str, text: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("takt-run-config-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let path = dir.join("run.toml");
        std::fs::write(&path, text).expect("a configuration file");
        path
    }

    /// A `harness` string that names no harness is refused at read, without repeating it: the
    /// string is a value written in the file.
    #[test]
    fn a_harness_name_that_is_no_harness_is_refused_without_repeating_it() {
        let path = scratch("harness-name", "harness = \"MARKER-harness-name\"\n");
        let error = RunConfig::read(&path).expect_err("refused");
        let message = format!("{error:#}");
        assert!(!message.contains("MARKER-harness-name"), "{message}");
        assert!(
            message.contains("claude, codex, harness or none"),
            "{message}"
        );
    }

    /// The bounded read takes at most the bound and no more: an endless reader, which holds more
    /// than the bound, is refused, and a reader of exactly the bound is read whole.
    #[test]
    fn a_reader_past_the_bound_is_refused_without_being_read_to_its_end() {
        assert_eq!(read_bounded(std::io::repeat(b'a')).expect("read"), None);
        let exact = std::io::Read::take(std::io::repeat(b'a'), LLM_DOCUMENT_BOUND);
        let text = read_bounded(exact)
            .expect("read")
            .expect("within the bound");
        assert_eq!(text.len() as u64, LLM_DOCUMENT_BOUND);
        assert_eq!(
            read_bounded(&b"llm"[..]).expect("read"),
            Some("llm".to_owned())
        );
    }

    #[test]
    fn every_field_is_read_and_a_relative_endpoint_file_is_beside_the_file() {
        let path = scratch(
            "all",
            "iterations = 2\nhours = 1.5\nharness = \"harness\"\nmodel = \"m\"\nendpoint_file = \"endpoint.toml\"\nforge = \"none\"\ntest_cmd = \"cargo test\"\n",
        );
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.iterations, Some(2));
        assert_eq!(config.hours, Some(1.5));
        assert_eq!(
            config.harness.as_ref().map(HarnessSetting::harness),
            Some("harness")
        );
        assert_eq!(config.model.as_deref(), Some("m"));
        assert_eq!(
            config.endpoint_file,
            Some(path.parent().expect("a directory").join("endpoint.toml"))
        );
        assert_eq!(config.forge.as_deref(), Some("none"));
        assert_eq!(config.test_cmd.as_deref(), Some("cargo test"));
    }

    #[test]
    fn story_token_variable_and_a_relative_state_dir_beside_the_file_are_read() {
        let path = scratch(
            "story-env-state",
            "story = \"story:s\"\nendpoint_token_env = \"KEY\"\nstate_dir = \"state\"\n",
        );
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.story.as_deref(), Some("story:s"));
        assert_eq!(config.endpoint_token_env.as_deref(), Some("KEY"));
        assert_eq!(
            config.state_dir,
            Some(path.parent().expect("a directory").join("state"))
        );
        let path = scratch_named(
            "loom-state",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  story: story:s\n  endpoint_token_env: KEY\n  state_dir: /data/takt\n",
        );
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.story.as_deref(), Some("story:s"));
        assert_eq!(config.endpoint_token_env.as_deref(), Some("KEY"));
        assert_eq!(config.state_dir, Some(PathBuf::from("/data/takt")));
    }

    #[test]
    fn continue_past_red_base_is_a_boolean_in_either_format() {
        let path = scratch("red-base", "continue_past_red_base = true\n");
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.continue_past_red_base, Some(true));
        let path = scratch_named(
            "red-base-loom",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  continue_past_red_base: false\n",
        );
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.continue_past_red_base, Some(false));
        let path = scratch("red-base-string", "continue_past_red_base = \"yes\"\n");
        let error = format!("{:#}", RunConfig::read(&path).expect_err("refused"));
        assert!(
            error.contains("`continue_past_red_base`, a boolean"),
            "{error}"
        );
    }

    #[test]
    fn the_commit_section_is_read_in_either_format_and_refused_by_member() {
        use crate::commit_style::{Settings, Style};
        let path = scratch(
            "commit-toml",
            "[commit]\nstyle = \"action\"\ntrailer = \"Story\"\n[commit.types]\n\"implementation.edit\" = \"build\"\n",
        );
        let section = RunConfig::read(&path)
            .expect("read")
            .commit
            .expect("a section");
        let settings = Settings::of_section(&section);
        assert_eq!(settings.style, Style::Action);
        assert_eq!(settings.trailer, "Story");
        assert_eq!(settings.types["implementation.edit"], "build");
        let path = scratch_named(
            "commit-loom",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  commit:\n    trailer: ''\n",
        );
        let section = RunConfig::read(&path)
            .expect("read")
            .commit
            .expect("a section");
        assert_eq!(Settings::of_section(&section).trailer, "");
        assert_eq!(Settings::of_section(&section).style, Style::Semantic);
        for (name, text, member) in [
            (
                "commit-key",
                "[commit]\ntrailer = \"not a key\"\n",
                "commit.trailer",
            ),
            (
                "commit-type",
                "[commit.types]\n\"tests.write\" = \"takt\"\n",
                "commit.types",
            ),
        ] {
            let error = format!(
                "{:#}",
                RunConfig::read(&scratch(name, text)).expect_err("refused")
            );
            assert!(error.contains(member), "{error}");
            assert!(!error.contains("not a key"), "{error}");
        }
        let error = format!(
            "{:#}",
            RunConfig::read(&scratch(
                "commit-style",
                "[commit]\nstyle = \"MARKER-style\"\n"
            ))
            .expect_err("refused")
        );
        assert!(error.contains("line"), "{error}");
        assert!(!error.contains("MARKER-style"), "{error}");
    }

    #[test]
    fn records_is_default_or_trial_in_either_format() {
        let path = scratch("records", "records = \"trial\"\n");
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.records.as_deref(), Some("trial"));
        let path = scratch_named(
            "records-loom",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  records: default\n",
        );
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.records.as_deref(), Some("default"));
        let path = scratch("records-value", "records = \"sometimes\"\n");
        let error = format!("{:#}", RunConfig::read(&path).expect_err("refused"));
        assert!(error.contains("`records`"), "{error}");
        assert!(!error.contains("sometimes"), "{error}");
        let path = scratch_named(
            "records-loom-value",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  records: MARKER-records\n",
        );
        let error = format!("{:#}", RunConfig::read(&path).expect_err("refused"));
        assert!(error.contains("`records`"), "{error}");
        assert!(!error.contains("MARKER-records"), "{error}");
        let path = scratch("records-type", "records = true\n");
        let error = format!("{:#}", RunConfig::read(&path).expect_err("refused"));
        assert!(error.contains("`records`"), "{error}");
        assert!(error.contains("line 1"), "{error}");
    }

    #[test]
    fn an_empty_file_sets_nothing() {
        let path = scratch("empty", "");
        assert_eq!(RunConfig::read(&path).expect("read"), RunConfig::default());
    }

    #[test]
    fn an_unknown_key_is_refused_naming_path_and_line_but_no_value() {
        let path = scratch("unknown", "iterations = 1\ntoken = \"MARKER-run-config\"\n");
        let error = format!("{:#}", RunConfig::read(&path).expect_err("refused"));
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(error.contains("line 2"), "{error}");
        assert!(!error.contains("MARKER-run-config"), "{error}");
    }

    fn scratch_named(name: &str, file: &str, text: &str) -> PathBuf {
        let path = scratch(name, "").with_file_name(file);
        std::fs::write(&path, text).expect("a configuration file");
        path
    }

    #[test]
    fn yaml_reads_every_field_and_a_relative_endpoint_file_beside_it() {
        let path = scratch_named(
            "yaml-all",
            "run.yml",
            "iterations: 2\nhours: 1.5\nharness: harness\nmodel: m\nendpoint_file: endpoint.toml\nforge: none\ntest_cmd: cargo test\n",
        );
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.iterations, Some(2));
        assert_eq!(config.hours, Some(1.5));
        assert_eq!(
            config.harness.as_ref().map(HarnessSetting::harness),
            Some("harness")
        );
        assert_eq!(config.model.as_deref(), Some("m"));
        assert_eq!(
            config.endpoint_file,
            Some(path.parent().expect("a directory").join("endpoint.toml"))
        );
        assert_eq!(config.forge.as_deref(), Some("none"));
        assert_eq!(config.test_cmd.as_deref(), Some("cargo test"));
    }

    #[test]
    fn an_empty_yaml_file_and_a_loom_agent_document_without_takt_set_nothing() {
        let empty = scratch_named("yaml-empty", "run.yaml", "");
        assert_eq!(RunConfig::read(&empty).expect("read"), RunConfig::default());
        let bare = scratch_named(
            "loom-bare",
            "agent.yaml",
            "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml}\n",
        );
        assert_eq!(RunConfig::read(&bare).expect("read"), RunConfig::default());
        let null = scratch_named(
            "loom-null",
            "agent.yaml",
            "format: loom.agent/1\ntakt: null\n",
        );
        assert_eq!(RunConfig::read(&null).expect("read"), RunConfig::default());
    }

    #[test]
    fn a_loom_agent_documents_takt_member_is_the_configuration() {
        let path = scratch_named(
            "loom-takt",
            "agent.yaml",
            "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\ntakt:\n  iterations: 3\n  endpoint_file: /config/endpoint.toml\n",
        );
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.iterations, Some(3));
        assert_eq!(
            config.endpoint_file,
            Some(PathBuf::from("/config/endpoint.toml"))
        );
    }

    #[test]
    fn a_yaml_list_a_wrong_type_and_another_extension_are_refused_without_values() {
        for (name, file, text) in [
            ("yaml-list", "run.yaml", "- MARKER-list\n"),
            ("yaml-type", "run.yaml", "iterations: MARKER-type\n"),
            (
                "loom-type",
                "agent.yaml",
                "format: loom.agent/1\ntakt: MARKER-scalar\n",
            ),
            ("ext", "run.conf", "iterations = 1\n"),
            ("no-ext", "run", "iterations = 1\n"),
        ] {
            let path = scratch_named(name, file, text);
            let error = format!("{:#}", RunConfig::read(&path).expect_err(name));
            assert!(error.contains(&path.display().to_string()), "{error}");
            assert!(!error.contains("MARKER"), "{error}");
        }
    }

    /// story:scan-and-forge-are-adapters: `scan` names the run's scan adapter, `none`, in either
    /// format; any other value, `gates` too, is refused naming the file and the line, never the
    /// value.
    #[test]
    fn scan_names_none_and_any_other_value_is_refused_at_its_line() {
        use crate::scan::ScanName;
        let path = scratch("scan-none", "iterations = 1\nscan = \"none\"\n");
        assert_eq!(
            RunConfig::read(&path).expect("read").scan,
            Some(ScanName::None)
        );
        let path = scratch_named(
            "scan-loom",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  scan: none\n",
        );
        assert_eq!(
            RunConfig::read(&path).expect("read").scan,
            Some(ScanName::None)
        );
        for (name, file, text, line) in [
            (
                "scan-toml",
                "run.toml",
                "iterations = 1\nscan = \"MARKER-scan\"\n",
                2,
            ),
            (
                "scan-yaml",
                "run.yaml",
                "iterations: 1\nhours: 1\nscan: MARKER-scan\n",
                3,
            ),
            ("scan-gates", "run.yaml", "iterations: 1\nscan: gates\n", 2),
        ] {
            let path = scratch_named(name, file, text);
            let error = format!("{:#}", RunConfig::read(&path).expect_err(name));
            assert!(error.contains(&path.display().to_string()), "{error}");
            assert!(error.contains(&format!("line {line}")), "{error}");
            assert!(error.contains("`scan`, `none`"), "{error}");
            assert!(!error.contains("MARKER"), "{error}");
        }
    }

    #[test]
    fn an_absolute_endpoint_file_stays() {
        let path = scratch("absolute", "endpoint_file = \"/config/endpoint.toml\"\n");
        assert_eq!(
            RunConfig::read(&path).expect("read").endpoint_file,
            Some(PathBuf::from("/config/endpoint.toml"))
        );
    }

    /// story:the-state-directory-holds-every-host-record: a `loom.agent/1` document's
    /// `runtime.state_dir` is the state directory when its `takt` member names none, a relative
    /// one read beside the file; `takt.state_dir` wins over it; the other `runtime` members are
    /// Loom's and are not refused.
    #[test]
    fn a_loom_runtime_state_dir_is_the_default_and_takt_state_dir_wins() {
        let path = scratch_named(
            "runtime-state",
            "agent.yaml",
            "format: loom.agent/1\nruntime:\n  state_dir: state\n  connectors: []\n",
        );
        assert_eq!(
            RunConfig::read(&path).expect("read").state_dir,
            Some(path.parent().expect("a directory").join("state"))
        );
        let path = scratch_named(
            "runtime-takt-state",
            "agent.yaml",
            "format: loom.agent/1\nruntime:\n  state_dir: /data/runtime\ntakt:\n  state_dir: /data/takt\n",
        );
        assert_eq!(
            RunConfig::read(&path).expect("read").state_dir,
            Some(PathBuf::from("/data/takt"))
        );
    }

    /// story:the-state-directory-holds-every-host-record: a `runtime.state_dir` that is not a
    /// string is refused naming the file and never its value.
    #[test]
    fn a_loom_runtime_state_dir_that_is_not_a_string_is_refused_without_its_value() {
        let path = scratch_named(
            "runtime-type",
            "agent.yaml",
            "format: loom.agent/1\nruntime:\n  state_dir: [MARKER-runtime]\n",
        );
        let error = format!("{:#}", RunConfig::read(&path).expect_err("refused"));
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(!error.contains("MARKER"), "{error}");
    }
}
