//! A run's configuration file, `takt run --config <FILE>` and `takt scenario run --config
//! <FILE>`: the story, the loop's budget, the harness, its model, endpoint file and token
//! variable, the forge, the test command, the state directory, whether the run goes on past
//! a red base and its scan adapter (`scan`, `gates` or `none`, set nowhere else), read from
//! one file so a run started in a container takes them from a mounted file. The file is TOML
//! (`.toml`) or YAML (`.yaml`, `.yml`), the same fields in either; a YAML file whose `format`
//! is `loom.agent/1` is the agent document Loom mounts for a `LoomAgent` at
//! `/config/agent.yaml`, and its `takt` member is the run configuration. A flag given on the
//! command line wins over the file, and the file over the flag's default; the run applies
//! them (`crates/takt-cli/src/main.rs`).

use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, Result, anyhow};

/// llm's parsed `llm.routes/1` document, the type a run file's `llm` member is validated as; its
/// [`Routes::digest`] is the `llm_digest` a run records.
pub use llm_models::routes::Routes;

/// The `format` of the agent document Loom writes to `/config/agent.yaml`.
pub const LOOM_AGENT_FORMAT: &str = "loom.agent/1";

/// A `loom.agent/1` document as takt reads it for the lines of its refusals: its `takt` member
/// alone, the members checked against [`LOOM_AGENT_MEMBERS`] beforehand.
#[derive(serde::Deserialize)]
struct LoomAgentDocument {
    takt: Option<RunConfig>,
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
/// `iterations`, `hours`, `harness`, `model`, `endpoint_file`, `endpoint_token_env`, `forge`,
/// `test_cmd`, `state_dir`, `continue_past_red_base` and `records`; and `scan` and `llm`, which
/// no flag gives. None is required.
#[derive(Debug, Default, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunConfig {
    /// `--story`, a story's id.
    pub story: Option<String>,
    /// `--iterations`.
    pub iterations: Option<u64>,
    /// `--hours`.
    pub hours: Option<f64>,
    /// `--harness`.
    pub harness: Option<String>,
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
    /// The run's scan adapter, `gates` or `none` ([`crate::scan::Scan`]): the one field no
    /// flag sets, so a run is unscanned only where its file says so. Not set, it is `gates`.
    pub scan: Option<crate::scan::ScanName>,
    /// The model routes: an `llm.routes/1` document written as a mapping, or `{path: <file>}`
    /// read beside the file. Kept as written; [`RunConfig::read`] has llm validate it.
    #[serde(default, deserialize_with = "present")]
    pub llm: Option<serde_yaml_ng::Value>,
    /// The digest of the document `llm` holds, as [`Routes::digest`] gives it. Set by
    /// [`RunConfig::read`], never read from the file.
    #[serde(skip)]
    pub llm_digest: Option<String>,
}

/// A present `llm` member, null included: serde reads a null as absent, so a bare `llm:` would
/// otherwise open a case with no section. It is kept as a value and refused by `llm_digest`.
fn present<'de, D>(deserializer: D) -> std::result::Result<Option<serde_yaml_ng::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <serde_yaml_ng::Value as serde::Deserialize>::deserialize(deserializer).map(Some)
}

const SHAPE: &str = "a run configuration file holds `iterations` and `hours`, numbers, `story`, `harness`, `model`, `endpoint_file`, `endpoint_token_env`, `forge`, `test_cmd`, `state_dir` and `records`, strings, `continue_past_red_base`, a boolean, `scan`, `gates` or `none`, and `llm`, an `llm.routes/1` document or `{path: <file>}`, each optional; `records` is `default` or `trial`";

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

/// The digest of the `llm.routes/1` document `llm` holds, validated by llm. A mapping whose only
/// key is `path` names a file beside the run file, parsed by `Routes::parse_toml` when it ends in
/// `.toml` and by `Routes::parse` when it ends in `.yaml` or `.yml`; any other mapping is the
/// whole document, re-serialised to YAML text and parsed by `Routes::parse`. llm's refusal is
/// relayed as it stands, and nothing here repeats a value written in the file.
fn llm_digest(path: &Path, llm: &serde_yaml_ng::Value) -> Result<String> {
    let shape = || {
        anyhow!(
            "the run configuration file {} is refused: its `llm` member is neither an \
             `llm.routes/1` document nor `{{path: <file>}}`",
            path.display()
        )
    };
    let routes = match llm
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
            let file = path.parent().unwrap_or(Path::new(".")).join(name);
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("the llm document {}", file.display()))?;
            let parsed = match file.extension().and_then(|extension| extension.to_str()) {
                Some("toml") => Routes::parse_toml(&text),
                Some("yaml" | "yml") => Routes::parse(&text),
                _ => {
                    return Err(anyhow!(
                        "the llm document {} is refused: its name ends in neither .toml, .yaml \
                         nor .yml",
                        file.display()
                    ));
                }
            };
            parsed.with_context(|| format!("the llm document {}", file.display()))?
        }
        None if llm.as_mapping().is_some_and(|mapping| !mapping.is_empty()) => {
            let text = serde_yaml_ng::to_string(llm).map_err(|_| shape())?;
            Routes::parse(&text).with_context(|| {
                format!(
                    "the run configuration file {}: its `llm` document",
                    path.display()
                )
            })?
        }
        None => return Err(shape()),
    };
    Ok(routes.digest().to_owned())
}

/// The run configuration in `text`, TOML.
fn from_toml(path: &Path, text: &str) -> Result<RunConfig> {
    toml::from_str(text).map_err(|error| {
        refused(
            path,
            error
                .span()
                .and_then(|span| text.get(..span.start))
                .map(|before| before.matches('\n').count() + 1),
        )
    })
}

/// The run configuration in `text`, YAML: a `loom.agent/1` document's `takt` member, nothing
/// when it has none, or else the whole document. A document with a `format` other than
/// `loom.agent/1`, a member of one that is neither Loom's nor `takt`, or a YAML tag anywhere is
/// refused without repeating what is written.
fn from_yaml(path: &Path, text: &str) -> Result<RunConfig> {
    let line = |error: &serde_yaml_ng::Error| error.location().map(|at| at.line());
    let document: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(text).map_err(|error| refused(path, line(&error)))?;
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
            match (agent.takt, document.get("takt")) {
                (Some(_), Some(takt)) => strict(path, takt),
                _ => Ok(RunConfig::default()),
            }
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
    /// `takt` member, or nothing without one. Any other key, or a value of another type, is
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
        if let Some(records) = &config.records
            && crate::case::Records::named(records).is_none()
        {
            return Err(anyhow!(
                "the run configuration file {} is refused: `records` is `default` or `trial`, no other value",
                path.display()
            ));
        }
        if let Some(llm) = &config.llm {
            config.llm_digest = Some(llm_digest(path, llm)?);
        }
        let beside = |file: PathBuf| match path.parent() {
            Some(dir) if file.is_relative() => dir.join(file),
            _ => file,
        };
        config.endpoint_file = config.endpoint_file.take().map(beside);
        config.state_dir = config.state_dir.take().map(beside);
        Ok(config)
    }
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

    #[test]
    fn every_field_is_read_and_a_relative_endpoint_file_is_beside_the_file() {
        let path = scratch(
            "all",
            "iterations = 2\nhours = 1.5\nharness = \"harness\"\nmodel = \"m\"\nendpoint_file = \"endpoint.toml\"\nforge = \"none\"\ntest_cmd = \"cargo test\"\n",
        );
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.iterations, Some(2));
        assert_eq!(config.hours, Some(1.5));
        assert_eq!(config.harness.as_deref(), Some("harness"));
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
        assert_eq!(config.harness.as_deref(), Some("harness"));
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

    /// story:scan-and-forge-are-adapters: `scan` names the run's scan adapter, `gates` or
    /// `none`, in either format; any other value is refused naming the file and the line, never
    /// the value.
    #[test]
    fn scan_names_gates_or_none_and_any_other_value_is_refused_at_its_line() {
        use crate::scan::ScanName;
        let path = scratch("scan-none", "iterations = 1\nscan = \"none\"\n");
        assert_eq!(
            RunConfig::read(&path).expect("read").scan,
            Some(ScanName::None)
        );
        let path = scratch_named(
            "scan-loom",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  scan: gates\n",
        );
        assert_eq!(
            RunConfig::read(&path).expect("read").scan,
            Some(ScanName::Gates)
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
        ] {
            let path = scratch_named(name, file, text);
            let error = format!("{:#}", RunConfig::read(&path).expect_err(name));
            assert!(error.contains(&path.display().to_string()), "{error}");
            assert!(error.contains(&format!("line {line}")), "{error}");
            assert!(error.contains("`scan`, `gates` or `none`"), "{error}");
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
}
