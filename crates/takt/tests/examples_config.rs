//! The manifests under `examples/loom-agent/`, which the guides "Run in a container" and "Deploy
//! on Kubernetes" show, read back through takt's own readers, so an example cannot drift from
//! what takt accepts: `agent.yaml` through [`RunConfig::read`], each `endpoint.toml` through
//! [`EndpointFields::read`], the `takt.yaml` and `llm.yaml` of the `LoomAgent`'s
//! `spec.configFiles` through [`RunConfig::read`] (the one run file and its llm document beside
//! it), the `spec.agent` through loom's own decoder, and the names the `LoomAgent` uses held to
//! the chart values that admit them.

use std::path::{Path, PathBuf};

use serde_yaml_ng::Value;
use takt::harness::EndpointFields;
use takt::run_config::{Routes, RunConfig};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/loom-agent")
}

fn yaml(name: &str) -> Value {
    let text = std::fs::read_to_string(examples().join(name)).expect(name);
    serde_yaml_ng::from_str(&text).expect(name)
}

fn get<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |value, key| {
        value
            .get(*key)
            .unwrap_or_else(|| panic!("no `{key}` in {path:?}"))
    })
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_sequence()
        .expect("a list")
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

/// The run configuration both examples give: the guides show these values.
fn assert_run(config: &RunConfig) {
    assert_eq!(config.iterations, None);
    assert_eq!(config.hours, Some(1.0));
    assert_eq!(
        config
            .harness
            .as_ref()
            .map(takt::run_config::HarnessSetting::harness),
        Some("harness")
    );
    assert_eq!(config.model, None);
    assert_eq!(config.forge.as_deref(), Some("none"));
    assert_eq!(config.scan, Some(takt::scan::ScanName::None));
    assert_eq!(
        config.test_cmd.as_deref(),
        Some("cargo test --quiet --locked")
    );
    assert_eq!(config.continue_past_red_base, None);
    assert_eq!(
        config
            .endpoint_file
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str()),
        Some("endpoint.toml")
    );
}

/// The endpoint both examples give: a placeholder host and model, the key by variable name.
fn assert_endpoint(endpoint: &EndpointFields) {
    assert_eq!(
        endpoint,
        &EndpointFields {
            base_url: Some("https://llm.example.com/v1".to_owned()),
            wire: Some("openai-responses".to_owned()),
            model: Some("example-model".to_owned()),
            env: Some("LLM_API_KEY".to_owned()),
            ..EndpointFields::default()
        }
    );
}

#[test]
fn the_container_examples_agent_yaml_and_endpoint_toml_are_read_as_the_guide_says() {
    let config = RunConfig::read(&examples().join("agent.yaml")).expect("agent.yaml reads");
    assert_run(&config);
    let endpoint_file = config.endpoint_file.expect("an endpoint file");
    assert_eq!(endpoint_file, examples().join("endpoint.toml"));
    assert_endpoint(&EndpointFields::read(&endpoint_file).expect("endpoint.toml reads"));
}

/// The run configuration the `LoomAgent`'s `takt.yaml` gives: the story, the budget, the harness,
/// the route `agent` of its llm document and the forge, with no endpoint file of its own.
fn assert_loom_run(config: &RunConfig) {
    assert_eq!(config.story.as_deref(), Some("story:add-sub"));
    assert_eq!(config.iterations, None);
    assert_eq!(config.hours, Some(1.0));
    assert_eq!(
        config
            .harness
            .as_ref()
            .map(takt::run_config::HarnessSetting::harness),
        Some("harness")
    );
    assert_eq!(config.model.as_deref(), Some("agent"));
    assert_eq!(config.forge.as_deref(), Some("none"));
    assert_eq!(config.scan, Some(takt::scan::ScanName::None));
    assert_eq!(
        config.test_cmd.as_deref(),
        Some("cargo test --quiet --locked")
    );
    assert_eq!(config.endpoint_file, None);
    assert!(config.llm_digest.is_some(), "the llm section is read");
}

/// Writes the `LoomAgent`'s `spec.configFiles` into `dir` verbatim and returns their names.
fn write_config_files(spec: &Value, dir: &Path) -> Vec<String> {
    std::fs::create_dir_all(dir).expect("a scratch directory");
    let mut names = Vec::new();
    for file in get(spec, &["configFiles"]).as_sequence().expect("a list") {
        let name = get(file, &["name"]).as_str().expect("a name");
        let content = get(file, &["content"]).as_str().expect("a content");
        std::fs::write(dir.join(name), content).expect("a config file");
        names.push(name.to_owned());
    }
    names
}

#[test]
fn the_loom_agents_config_files_are_read_as_the_guide_says() {
    let agent = yaml("takt-agent.yaml");
    let spec = get(&agent, &["spec"]);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("examples-config");
    let names = write_config_files(spec, &dir);
    assert_eq!(names, ["takt.yaml", "llm.yaml"]);
    let config = RunConfig::read(&dir.join("takt.yaml")).expect("takt.yaml reads");
    assert_loom_run(&config);
    let llm = std::fs::read_to_string(dir.join("llm.yaml")).expect("llm.yaml");
    assert_eq!(
        config.llm_digest.as_deref(),
        Some(Routes::parse(&llm).expect("llm parses").digest()),
        "the run file's llm digest is the mounted llm.yaml's"
    );
    let llm_value: Value = serde_yaml_ng::from_str(&llm).expect("llm.yaml is YAML");
    assert_eq!(
        get(&llm_value, &["endpoints", "llm", "auth", "secret"]).as_str(),
        Some("env:LLM_API_KEY"),
        "the key is named by its variable, never written"
    );

    let entrypoint = strings(get(spec, &["command", "entrypoint"])).join(" ");
    assert!(
        entrypoint.contains("exec takt run --repo /workspace \"$@\""),
        "{entrypoint}"
    );
    assert_eq!(
        strings(get(spec, &["command", "args"])),
        ["--config", "/config/takt.yaml"]
    );
    assert!(
        get(spec, &["agent"]).get("takt").is_none(),
        "Loom 0.44.0 refuses a `takt` member in spec.agent"
    );
    assert_eq!(
        get(spec, &["image", "tag"]).as_str(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

/// Acceptance 2 of story:one-run-file-for-host-and-loom-agent, the part of Loom's admission takt can
/// reach: the `spec.agent` of the example decodes under `loom_sdk::agent::parse` with its `model`
/// and no `takt` member. A document without `model`, or with one, is refused by the same decoder.
#[test]
fn the_loom_agents_spec_agent_is_admitted_by_loom_with_a_model_and_no_takt_member() {
    let agent = yaml("takt-agent.yaml");
    let spec_agent = get(get(&agent, &["spec"]), &["agent"]).clone();
    let text = serde_yaml_ng::to_string(&spec_agent).expect("spec.agent");
    loom_sdk::agent::parse(&text).expect("loom admits spec.agent");

    let mut without_model = spec_agent.clone();
    without_model
        .as_mapping_mut()
        .expect("a mapping")
        .remove("model");
    let text = serde_yaml_ng::to_string(&without_model).expect("spec.agent");
    assert!(loom_sdk::agent::parse(&text).is_err());

    let mut with_takt = spec_agent;
    with_takt
        .as_mapping_mut()
        .expect("a mapping")
        .insert("takt".into(), Value::Mapping(Default::default()));
    let text = serde_yaml_ng::to_string(&with_takt).expect("spec.agent");
    assert!(loom_sdk::agent::parse(&text).is_err());
}

/// The image's ENTRYPOINT runs the run file's `--config` argument and its CMD names `agent.yaml`,
/// so an image started without arguments still reads `/config/agent.yaml`.
#[test]
fn the_image_reads_agent_yaml_when_started_without_arguments() {
    let dockerfile =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Dockerfile"))
            .expect("the Dockerfile");
    let lines: Vec<&str> = dockerfile.lines().collect();
    assert!(
        lines.contains(&r#"ENTRYPOINT ["tini", "--", "takt", "run", "--repo", "/workspace"]"#),
        "the ENTRYPOINT"
    );
    assert!(
        lines.contains(&r#"CMD ["--config", "/config/agent.yaml"]"#),
        "the CMD"
    );
}

#[test]
fn the_loom_agent_uses_only_names_the_chart_values_admit() {
    let values = yaml("values.yaml");
    let agent = yaml("takt-agent.yaml");
    let spec = get(&agent, &["spec"]);
    let secrets = strings(get(&values, &["allowedSecretNames"]));
    // `secretFiles` is optional: the example mounts none.
    for path in [["secretRef", "name"], ["secretFiles", "name"]] {
        if path[0] == "secretFiles" && spec.get("secretFiles").is_none() {
            continue;
        }
        let name = get(spec, &path).as_str().expect("a name");
        assert!(
            secrets.iter().any(|s| s == name),
            "{name} not in {secrets:?}"
        );
    }
    let claims = strings(get(&values, &["allowedClaimNames"]));
    let claim = get(spec, &["workspace", "claimName"])
        .as_str()
        .expect("a claim");
    assert!(
        claims.iter().any(|c| c == claim),
        "{claim} not in {claims:?}"
    );
    let hosts = strings(get(&values, &["allowedEgressHosts"]));
    let egress = get(spec, &["agent", "deploy", "kubernetes", "egress"]);
    for entry in egress.as_sequence().expect("a list") {
        let host = get(entry, &["host"]).as_str().expect("a host");
        assert!(hosts.iter().any(|h| h == host), "{host} not in {hosts:?}");
    }
}

#[test]
fn every_yaml_example_parses() {
    let mut read = 0;
    for entry in std::fs::read_dir(examples()).expect("examples/loom-agent") {
        let path = entry.expect("an entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a file");
        for document in serde_yaml_ng::Deserializer::from_str(&text) {
            let value = <Value as serde::Deserialize>::deserialize(document)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(value.is_mapping(), "{}", path.display());
        }
        read += 1;
    }
    assert_eq!(
        read, 5,
        "agent, kind-cluster, takt-agent, values, workspace"
    );
}
