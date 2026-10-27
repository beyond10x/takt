//! `takt protocol show` through the binary (story:takt-lists-a-protocols-settings): the listing of
//! a built-in protocol's inputs with the run file's value and line, the same as JSON, and an
//! unknown protocol refused with exit 1 naming the ones takt knows.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn show(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["protocol", "show"])
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_file(name: &str, text: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("protocol-show-cli-{}-{name}", std::process::id()));
    fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("run.yaml");
    fs::write(&path, text).expect("a run file");
    path
}

const RUN_FILE: &str = "protocols:\n  software.change:\n    risk: elevated\n";

#[test]
fn lists_inputs_defaults_and_the_run_file_value() {
    let path = run_file("lists", RUN_FILE);
    let config = path.to_str().expect("utf-8");
    let (code, out, err) = show(&["software.change/1", "--config", config]);
    assert_eq!(code, 0, "{out} {err}");
    assert!(out.starts_with("protocol software.change/1\n"), "{out}");
    assert!(out.contains("input risk\n"), "{out}");
    assert!(
        out.contains("  values: trivial, standard, elevated, critical\n"),
        "{out}"
    );
    assert!(out.contains("  default: standard\n"), "{out}");
    assert!(
        out.contains(&format!("  run file: elevated ({config}:3)\n")),
        "{out}"
    );
    assert!(out.contains("input change_kind\n"), "{out}");
    assert!(out.contains("  run file: unset\n"), "{out}");
    let _ = fs::remove_dir_all(path.parent().expect("a directory"));
}

#[test]
fn json_matches_text() {
    let path = run_file("json", RUN_FILE);
    let config = path.to_str().expect("utf-8");
    let (code, out, err) = show(&["software.change/1", "--config", config, "--format", "json"]);
    assert_eq!(code, 0, "{out} {err}");
    let json: serde_json::Value = serde_json::from_str(&out).expect("one JSON document");
    assert_eq!(json["protocol"], "software.change/1");
    assert_eq!(json["inputs"][0]["name"], "risk");
    assert_eq!(json["inputs"][0]["default"], "standard");
    assert_eq!(json["inputs"][0]["run_file"]["value"], "elevated");
    assert_eq!(json["inputs"][0]["run_file"]["line"], 3);
    assert_eq!(json["inputs"][0]["run_file"]["path"], config);
    assert!(json["inputs"][1]["run_file"].is_null());
    let (_, text, _) = show(&["software.change/1", "--config", config]);
    for input in json["inputs"].as_array().expect("inputs") {
        assert!(
            text.contains(&format!(
                "input {}\n",
                input["name"].as_str().expect("a name")
            )),
            "{text}"
        );
    }
    let _ = fs::remove_dir_all(path.parent().expect("a directory"));
}

#[test]
fn an_unknown_protocol_names_the_known_ones() {
    let (code, out, err) = show(&["nothing.here/1"]);
    assert_eq!(code, 1, "{out} {err}");
    assert!(out.is_empty(), "{out}");
    assert!(err.contains("nothing.here/1"), "{err}");
    assert!(err.contains("software.change/1"), "{err}");
    assert!(err.contains("story.delivery/1"), "{err}");
}

#[test]
fn a_run_file_that_does_not_read_is_refused() {
    let path = run_file("bad", "protocols: [not, a, mapping]\n");
    let (code, out, err) = show(&[
        "software.change/1",
        "--config",
        path.to_str().expect("utf-8"),
    ]);
    assert_eq!(code, 1, "{out} {err}");
    assert!(err.contains("run.yaml"), "{err}");
    let _ = fs::remove_dir_all(path.parent().expect("a directory"));
}
