//! `takt protocol show <name/rev>` lists a protocol's declared inputs
//! (story:takt-lists-a-protocols-settings), library side: each input with its values,
//! description, takt's built-in default and the value the run file sets, with the file's path and
//! line; the same as data; an unknown protocol refused naming the known ones. `software.change/1`
//! declares `risk`, `change_kind` and `affects_security_boundary`; the `story.delivery/2` example
//! of the story waits for `story:takt-ships-default-protocol-settings`.

use std::fs;
use std::path::{Path, PathBuf};

use takt::protocol::{ProtocolListing, list_settings};
use takt::run_config::{ProtocolSettings, RunConfig};

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The settings of a run file holding `text`, written as `name` in a scratch directory.
fn file_settings(name: &str, text: &str) -> (Scratch, PathBuf, ProtocolSettings) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("protocol-show-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("run.yaml");
    fs::write(&path, text).expect("a run file");
    let settings = RunConfig::read(&path)
        .expect("a readable run file")
        .protocol_settings;
    (Scratch(dir), path, settings)
}

const RUN_FILE: &str = "protocols:\n  software.change:\n    risk: elevated\n";

#[test]
fn lists_inputs_defaults_and_the_run_file_value() {
    let (_dir, path, settings) = file_settings("lists", RUN_FILE);
    let listing = list_settings("software.change/1", &settings).expect("a listing");
    let text = listing.text();

    assert_eq!(listing.id, "software.change/1");
    let names: Vec<&str> = listing
        .inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["risk", "change_kind", "affects_security_boundary"],
        "{text}"
    );

    let risk = &listing.inputs[0];
    assert_eq!(risk.values, ["trivial", "standard", "elevated", "critical"]);
    assert!(!risk.description.is_empty(), "the protocol describes risk");
    assert_eq!(risk.default.as_deref(), Some("standard"));
    let set = risk.run_file.as_ref().expect("the run file sets risk");
    assert_eq!((set.value.as_str(), set.line), ("elevated", 3));
    assert_eq!(set.path.as_deref(), Some(path.as_path()));
    assert!(
        text.contains("  values: trivial, standard, elevated, critical\n"),
        "{text}"
    );
    assert!(text.contains("  default: standard\n"), "{text}");
    assert!(
        text.contains(&format!("  run file: elevated ({}:3)\n", path.display())),
        "{text}"
    );

    let kind = &listing.inputs[1];
    assert_eq!(kind.default.as_deref(), Some("feature"));
    assert_eq!(kind.run_file, None, "the run file sets no change_kind");
    assert!(text.contains("  run file: unset\n"), "{text}");
}

#[test]
fn a_run_file_that_names_the_revision_wins_over_one_that_names_the_protocol() {
    let (_dir, _path, settings) = file_settings(
        "specific",
        "protocols:\n  software.change:\n    risk: trivial\n  software.change/1:\n    risk: critical\n",
    );
    let listing = list_settings("software.change/1", &settings).expect("a listing");
    let set = listing.inputs[0].run_file.as_ref().expect("a value");
    assert_eq!((set.value.as_str(), set.line), ("critical", 5));
}

/// What the text says about each input, read back as data.
fn parsed(text: &str) -> serde_json::Value {
    let mut lines = text.lines();
    let protocol = lines
        .next()
        .and_then(|line| line.strip_prefix("protocol "))
        .expect("a protocol line");
    let mut inputs: Vec<serde_json::Value> = Vec::new();
    for line in lines {
        if line == "declares no inputs" {
            continue;
        }
        if let Some(name) = line.strip_prefix("input ") {
            inputs.push(serde_json::json!({ "name": name }));
            continue;
        }
        let input = inputs.last_mut().expect("an input before its fields");
        let field = line.trim_start();
        if let Some(values) = field.strip_prefix("values: ") {
            let values: Vec<&str> = if values == "any" {
                Vec::new()
            } else {
                values.split(", ").collect()
            };
            input["values"] = serde_json::json!(values);
        } else if let Some(description) = field.strip_prefix("description: ") {
            input["description"] = serde_json::json!(description);
        } else if let Some(default) = field.strip_prefix("default: ") {
            input["default"] = if default == "none" {
                serde_json::Value::Null
            } else {
                serde_json::json!(default)
            };
        } else if let Some(set) = field.strip_prefix("run file: ") {
            input["run_file"] = if set == "unset" {
                serde_json::Value::Null
            } else {
                let (value, place) = set.split_once(" (").expect("a value and its place");
                let place = place.strip_suffix(')').expect("a closing parenthesis");
                let (path, line) = place.rsplit_once(':').expect("a path and a line");
                serde_json::json!({
                    "value": value,
                    "path": path,
                    "line": line.parse::<usize>().expect("a line number"),
                })
            };
        }
    }
    serde_json::json!({ "protocol": protocol, "inputs": inputs })
}

fn assert_matches(listing: &ProtocolListing) {
    assert_eq!(
        parsed(&listing.text()),
        listing.json(),
        "{}",
        listing.text()
    );
}

#[test]
fn json_matches_text() {
    let (_dir, _path, settings) = file_settings("json", RUN_FILE);
    let listing = list_settings("software.change/1", &settings).expect("a listing");
    assert_matches(&listing);
    let json = listing.json();
    assert_eq!(json["protocol"], "software.change/1");
    assert_eq!(json["inputs"][0]["name"], "risk");
    assert_eq!(json["inputs"][0]["default"], "standard");
    assert_eq!(json["inputs"][0]["run_file"]["value"], "elevated");
    assert_eq!(json["inputs"][0]["run_file"]["line"], 3);
    assert!(json["inputs"][1]["run_file"].is_null());

    let none = list_settings("software.change/1", &ProtocolSettings::default())
        .expect("a listing without a run file");
    assert_matches(&none);
    assert!(none.inputs.iter().all(|input| input.run_file.is_none()));
}

#[test]
fn a_protocol_without_inputs_lists_none() {
    let listing =
        list_settings("story.delivery/1", &ProtocolSettings::default()).expect("a listing");
    assert!(listing.inputs.is_empty());
    assert!(listing.text().contains("declares no inputs"));
    assert_matches(&listing);
}

#[test]
fn an_unknown_protocol_names_the_known_ones() {
    let error = list_settings("nothing.here/1", &ProtocolSettings::default())
        .expect_err("an unknown protocol is refused");
    let said = format!("{error:#}");
    assert!(said.contains("nothing.here/1"), "{said}");
    for known in takt::protocol::builtin_ids() {
        assert!(said.contains(&known), "{said} lacks {known}");
    }
    assert!(said.contains("software.change/1"), "{said}");
}
