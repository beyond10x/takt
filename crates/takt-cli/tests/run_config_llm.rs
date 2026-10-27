//! story:llm-section-is-an-llm-routes-document: `takt run --config <FILE>` reads an `llm` member
//! that is an `llm.routes/1` document, inline or by path, and llm validates it. A refused section
//! opens no case, and an accepted one records the digest llm gives the document on the `opened`
//! line. These runs reach no network: the routes name no secret but a literal one that llm must
//! refuse, and HOME is a scratch directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use takt::run_config::Routes;

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch tree under this crate's build directory, removed on drop: a repository with one
/// commit, an empty HOME and a `config` directory holding the run files.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("run-config-llm-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        world
    }

    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }

    fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    fn config_dir(&self) -> PathBuf {
        self.0.join("config")
    }

    fn cases(&self) -> PathBuf {
        self.repo().join(takt::case::CASES_DIR)
    }

    /// Writes `text` as `config/<name>` and returns its path.
    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a file beside the run file");
        path
    }

    /// `takt run` on the repository with an intent and `--config <config>`, HOME the scratch one.
    fn run(&self, config: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args([
                "run",
                "--repo",
                self.repo().to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--config",
                config.to_str().expect("utf-8"),
            ])
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("B10X_GATES_POLICY")
            .output()
            .expect("takt runs")
    }

    /// Runs `config` on a case of its own: the case directory of any earlier run is removed first,
    /// so the one record read back is this run's.
    fn run_fresh(&self, config: &Path) -> Output {
        if self.cases().exists() {
            fs::remove_dir_all(self.cases()).expect("an earlier case is removed");
        }
        self.run(config)
    }

    /// The `opened` lines of the one case record.
    fn opened(&self) -> Vec<serde_json::Value> {
        let events: Vec<PathBuf> = files_under(&self.cases())
            .into_iter()
            .filter(|path| path.file_name().is_some_and(|name| name == "events.jsonl"))
            .collect();
        assert_eq!(events.len(), 1, "one case record: {events:?}");
        fs::read_to_string(&events[0])
            .expect("the run record")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON line"))
            .filter(|line| line["event"] == "opened")
            .collect()
    }
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            found.extend(files_under(&path));
        } else if meta.is_file() {
            found.push(path);
        }
    }
    found
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The routes document, as JSON, which is YAML: one HTTP endpoint with no credential and one route.
fn document() -> serde_json::Value {
    serde_json::json!({
        "format": "llm.routes/1",
        "endpoints": {
            "lab": {
                "base_url": "http://127.0.0.1:8000/v1",
                "protocol": "chat-completions",
                "auth": {"kind": "anonymous"}
            }
        },
        "routes": {"small": "lab/small"}
    })
}

/// The same routes as [`document`], written in TOML.
const TOML_DOCUMENT: &str = r#"format = "llm.routes/1"

[endpoints.lab]
base_url = "http://127.0.0.1:8000/v1"
protocol = "chat-completions"
auth = { kind = "anonymous" }

[routes]
small = "lab/small"
"#;

/// The digest llm gives [`document`], read by llm itself.
fn expected_digest() -> String {
    Routes::parse(&document().to_string())
        .expect("llm accepts the document")
        .digest()
        .to_owned()
}

/// A run file of the loop's budget and no harness or forge, holding `llm` as the given YAML.
fn run_file(llm: &str) -> String {
    format!("iterations: 0\nharness: none\nforge: none\n{llm}")
}

fn inline() -> String {
    run_file(&format!("llm: {}\n", document()))
}

/// The inline document is accepted, and the case's `opened` line names llm's digest of it.
#[test]
fn llm_inline_accepted() {
    let world = World::new("inline");
    let config = world.file("run.yaml", &inline());
    let output = world.run(&config);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    let opened = world.opened();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(opened[0]["llm_digest"], expected_digest(), "{opened:?}");
}

/// `llm: {path: llm.toml}` and `llm: {path: llm.yaml}`, the same routes, each name the digest the
/// inline form names.
#[test]
fn llm_path_accepted() {
    let toml = TOML_DOCUMENT.to_owned();
    let yaml = document().to_string();
    for (name, file, routes) in [
        ("path-toml", "llm.toml", toml),
        ("path-yaml", "llm.yaml", yaml),
    ] {
        let world = World::new(name);
        world.file(file, &routes);
        let config = world.file("run.yaml", &run_file(&format!("llm: {{path: {file}}}\n")));
        let output = world.run(&config);
        assert_eq!(output.status.code(), Some(3), "{}", text(&output));
        let opened = world.opened();
        assert_eq!(opened.len(), 1, "{opened:?}");
        assert_eq!(
            opened[0]["llm_digest"],
            expected_digest(),
            "{file}: {opened:?}"
        );
    }
}

/// An unknown key in the document is refused before any case opens, exit 1, with llm's message
/// and no value of the file. In TOML llm names the field; in YAML it names the line and column.
#[test]
fn llm_unknown_key_refused() {
    let world = World::new("unknown");
    let marker = "MARKER-llm-unknown-value";
    let toml = TOML_DOCUMENT.replacen(
        "format = \"llm.routes/1\"\n",
        &format!("format = \"llm.routes/1\"\ntoken = \"{marker}\"\n"),
        1,
    );
    world.file("llm.toml", &toml);
    let by_path = world.file("run-path.yaml", &run_file("llm: {path: llm.toml}\n"));
    let output = world.run_fresh(&by_path);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains("unknown field `token`"), "{all}");
    assert!(!all.contains(marker), "{all}");
    assert!(!world.cases().exists(), "no case was opened");

    let mut yaml = document();
    yaml["token"] = serde_json::Value::String(marker.to_owned());
    let inline = world.file("run-inline.yaml", &run_file(&format!("llm: {yaml}\n")));
    let output = world.run_fresh(&inline);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains("invalid llm.routes/1 YAML document"), "{all}");
    assert!(!all.contains(marker), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// Whether `all` holds `line` followed by a number, the way llm locates a refusal in a document.
fn names_a_line(all: &str) -> bool {
    all.match_indices("line ")
        .any(|(at, _)| all[at + "line ".len()..].starts_with(|c: char| c.is_ascii_digit()))
}

/// An inline `llm` section is refused without a line or a column: they would locate takt's
/// re-serialised copy of the section, not the run file. The refusal names the `llm` section and
/// shows no value of the file, for an unknown key and for a literal secret alike.
#[test]
fn an_inline_llm_refusal_cites_no_line_of_the_copy() {
    let world = World::new("inline-position");
    let marker = "MARKER-inline-position";
    let mut unknown = document();
    unknown["token"] = serde_json::Value::String(marker.to_owned());
    let config = world.file("run-unknown.yaml", &run_file(&format!("llm: {unknown}\n")));
    let output = world.run_fresh(&config);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains("`llm` section"), "{all}");
    assert!(!names_a_line(&all), "{all}");
    assert!(!all.contains(marker), "{all}");
    assert!(!world.cases().exists(), "no case was opened");

    let mut literal = document();
    literal["endpoints"]["lab"] = serde_json::json!({
        "base_url": "https://models.example/v1",
        "protocol": "messages",
        "auth": {"kind": "api-key", "header": "x-api-key", "secret": marker}
    });
    let config = world.file("run-literal.yaml", &run_file(&format!("llm: {literal}\n")));
    let output = world.run_fresh(&config);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains("auth.secret"), "{all}");
    assert!(!names_a_line(&all), "{all}");
    assert!(!all.contains(marker), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// An endpoint whose `auth.secret` is a literal is refused by llm before any case opens, exit 1.
/// The literal appears in no output and in no file takt wrote: every file under the scratch tree
/// but the run file the test itself wrote is searched.
#[test]
fn llm_literal_secret_refused() {
    let world = World::new("literal");
    let literal = "MARKER-llm-literal-secret";
    let mut document = document();
    document["endpoints"]["lab"] = serde_json::json!({
        "base_url": "https://models.example/v1",
        "protocol": "messages",
        "auth": {"kind": "api-key", "header": "x-api-key", "secret": literal}
    });
    let config = world.file("run.yaml", &run_file(&format!("llm: {document}\n")));
    let output = world.run_fresh(&config);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains("auth.secret"), "{all}");
    assert!(!all.contains(literal), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
    for file in files_under(&world.0) {
        if file == config {
            continue;
        }
        let written = fs::read(&file).expect("a file takt wrote");
        assert!(
            !written
                .windows(literal.len())
                .any(|window| window == literal.as_bytes()),
            "{} holds the literal",
            file.display()
        );
    }
}

/// A run file without an `llm` member opens a case whose `opened` line has no `llm_digest`.
#[test]
fn no_llm_section_no_digest() {
    let world = World::new("none");
    let config = world.file("run.yaml", &run_file(""));
    let output = world.run(&config);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    let opened = world.opened();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(opened[0].get("llm_digest").is_none(), "{opened:?}");
}

/// A TOML run file holding the routes inline as an `[llm]` table, the same routes as
/// [`TOML_DOCUMENT`] nested under it.
const TOML_RUN_FILE: &str = r#"iterations = 0
harness = "none"
forge = "none"

[llm]
format = "llm.routes/1"

[llm.endpoints.lab]
base_url = "http://127.0.0.1:8000/v1"
protocol = "chat-completions"
auth = { kind = "anonymous" }

[llm.routes]
small = "lab/small"
"#;

/// A TOML run file with the routes inline as an `[llm]` table is accepted, and so is one that
/// names the same routes by path; each `opened` line names the digest the inline form names.
#[test]
fn llm_toml_run_file_accepted() {
    let world = World::new("toml-inline");
    let config = world.file("run.toml", TOML_RUN_FILE);
    let output = world.run(&config);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    let opened = world.opened();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(opened[0]["llm_digest"], expected_digest(), "{opened:?}");

    let world = World::new("toml-path");
    world.file("llm.toml", TOML_DOCUMENT);
    let config = world.file(
        "run.toml",
        "iterations = 0\nharness = \"none\"\nforge = \"none\"\n\n[llm]\npath = \"llm.toml\"\n",
    );
    let output = world.run(&config);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    let opened = world.opened();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(opened[0]["llm_digest"], expected_digest(), "{opened:?}");
}
