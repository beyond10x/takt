use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("adversary-llm-red-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(["-c", "commit.gpgsign=false", "init", "-q", "-b", "main"])
            .current_dir(world.repo())
            .status()
            .expect("git runs");
        assert!(status.success(), "git init");
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(["-c", "commit.gpgsign=false", "add", "-A"])
            .current_dir(world.repo())
            .status()
            .expect("git runs");
        assert!(status.success(), "git add");
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(["-c", "commit.gpgsign=false", "commit", "-q", "-m", "start"])
            .current_dir(world.repo())
            .status()
            .expect("git runs");
        assert!(status.success(), "git commit");
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
        self.repo().join(".engineering/cases")
    }

    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a file beside the run file");
        path
    }

    fn run_fresh(&self, config: &Path) -> Output {
        if self.cases().exists() {
            fs::remove_dir_all(self.cases()).expect("an earlier case is removed");
        }
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(["run", "--repo"])
            .arg(self.repo())
            .args(["--intent", "probe", "--config"])
            .arg(config)
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("B10X_GATES_POLICY")
            .output()
            .expect("takt runs")
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

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

fn run_file(llm: &str) -> String {
    format!("iterations: 0\nharness: none\nforge: none\n{llm}")
}

#[test]
fn adversary_llm_item3_inline_yaml_unknown_key_names_field() {
    let world = World::new("inline-field");
    let mut doc = document();
    doc["token"] = serde_json::Value::String("MARKER-red-inline".to_owned());
    let config = world.file("run.yaml", &run_file(&format!("llm: {doc}\n")));
    let output = world.run_fresh(&config);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(
        all.contains("line") && all.contains("column"),
        "story:llm-section-is-an-llm-routes-document item 3: llm 0.16.0 names no field for a YAML document; change this assertion when llm does: {all}"
    );
    assert!(!all.contains("`token`"), "{all}");
    assert!(!all.contains("MARKER-red-inline"), "{all}");
    assert!(!world.cases().exists(), "no case opened: {all}");
}

#[test]
fn adversary_llm_item3_yaml_path_unknown_key_names_field() {
    let world = World::new("path-yaml-field");
    let mut doc = document();
    doc["token"] = serde_json::Value::String("MARKER-red-path".to_owned());
    world.file("llm.yaml", &doc.to_string());
    let config = world.file("run.yaml", &run_file("llm: {path: llm.yaml}\n"));
    let output = world.run_fresh(&config);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(
        all.contains("line") && all.contains("column"),
        "story:llm-section-is-an-llm-routes-document item 3: llm 0.16.0 names no field for a YAML document; change this assertion when llm does: {all}"
    );
    assert!(!all.contains("`token`"), "{all}");
    assert!(!all.contains("MARKER-red-path"), "{all}");
    assert!(!world.cases().exists(), "no case opened: {all}");
}

#[test]
fn adversary_llm_item5_empty_llm_section_refused() {
    let world = World::new("empty-section");
    let config = world.file("run.yaml", &run_file("llm:\n"));
    let output = world.run_fresh(&config);
    let all = text(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "an empty llm section is neither a document nor a path, and llm refuses an empty document: {all}"
    );
    assert!(!world.cases().exists(), "no case opened: {all}");
}

#[test]
fn adversary_llm_path_escape_refused() {
    let world = World::new("escape");
    fs::write(world.0.join("llm-outside.yaml"), document().to_string())
        .expect("a file outside the configuration directory");
    let config = world.file("run.yaml", &run_file("llm: {path: ../llm-outside.yaml}\n"));
    let output = world.run_fresh(&config);
    let all = text(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "acceptance Decision: the document is read beside the run file, so one outside its directory is refused: {all}"
    );
    assert!(!world.cases().exists(), "no case opened: {all}");
}
