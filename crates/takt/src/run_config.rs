//! A run's configuration file, `takt run --config <FILE>`: the loop's budget, the harness, its
//! model and endpoint file, the forge and the test command, read from one TOML file so a run
//! started in a container takes them from a mounted file. A flag given on the command line wins
//! over the file, and the file over the flag's default; the run applies them
//! (`crates/takt-cli/src/main.rs`).

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow};

/// The fields a run configuration file may set, each one a `takt run` flag's: `iterations`,
/// `hours`, `harness`, `model`, `endpoint_file`, `forge` and `test_cmd`. None is required.
#[derive(Debug, Default, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunConfig {
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
    /// `--forge`.
    pub forge: Option<String>,
    /// `--test-cmd`.
    pub test_cmd: Option<String>,
}

const SHAPE: &str = "a run configuration file holds `iterations` and `hours`, numbers, and `harness`, `model`, `endpoint_file`, `forge` and `test_cmd`, strings, each optional";

impl RunConfig {
    /// The run configuration file at `path`, TOML. Any other key, or a value of another type,
    /// is refused naming the path and, for a parse, the line, never a value written there. A
    /// relative `endpoint_file` is made relative to the file's directory.
    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("the run configuration file {}", path.display()))?;
        let mut config: Self = toml::from_str(&text).map_err(|error| {
            match error
                .span()
                .and_then(|span| text.get(..span.start))
                .map(|before| before.matches('\n').count() + 1)
            {
                Some(line) => anyhow!(
                    "the run configuration file {} is refused at line {line}: {SHAPE}",
                    path.display()
                ),
                None => anyhow!(
                    "the run configuration file {} is refused: {SHAPE}",
                    path.display()
                ),
            }
        })?;
        if let Some(file) = config.endpoint_file.take() {
            config.endpoint_file = Some(match path.parent() {
                Some(dir) if file.is_relative() => dir.join(file),
                _ => file,
            });
        }
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

    #[test]
    fn an_absolute_endpoint_file_stays() {
        let path = scratch("absolute", "endpoint_file = \"/config/endpoint.toml\"\n");
        assert_eq!(
            RunConfig::read(&path).expect("read").endpoint_file,
            Some(PathBuf::from("/config/endpoint.toml"))
        );
    }
}
