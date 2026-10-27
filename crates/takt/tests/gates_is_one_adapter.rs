//! story:scan-and-forge-are-adapters, Acceptance 6: `b10x-gates` is one adapter, the `gates`
//! scan and the `bot` forge, not a tool every run needs. In the library's and the binary's code
//! the program's name and its policy variable appear only in the two files that are those
//! adapters, `crates/takt/src/forge/bot.rs` and `crates/takt/src/scan/gates.rs`; the binary's
//! `///` help text may name them where it describes those two adapters. Every `.rs` file under
//! `crates/takt/src` and `crates/takt-cli/src` is read, a test module's lines included.

use std::fs;
use std::path::{Path, PathBuf};

/// What names the gates adapter: its program and its policy variable's prefix.
const NEEDLES: [&str; 2] = ["b10x-gates", "B10X_GATES"];

/// The two files that are the gates adapters, relative to the workspace's root.
const ADAPTERS: [&str; 2] = [
    "crates/takt/src/forge/bot.rs",
    "crates/takt/src/scan/gates.rs",
];

/// The binary's sources, where a `///` line (clap's help text) may name the adapters.
const CLI_SOURCES: &str = "crates/takt-cli/src/";

/// The workspace's root, two levels above this crate's manifest.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace's root")
}

/// Every `.rs` file under `dir`, a link never followed.
fn sources(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display())) {
        let path = entry.expect("an entry").path();
        let meta = fs::symlink_metadata(&path).expect("metadata");
        if meta.is_dir() {
            found.extend(sources(&path));
        } else if meta.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// Whether the line `text` of the file `relative` (to the workspace's root) names the gates
/// adapter where it may not.
fn offends(relative: &str, text: &str) -> bool {
    if ADAPTERS.contains(&relative) || !NEEDLES.iter().any(|needle| text.contains(needle)) {
        return false;
    }
    !(relative.starts_with(CLI_SOURCES) && text.trim_start().starts_with("///"))
}

/// Every `<file>:<line>: <text>` under `crates/takt/src` and `crates/takt-cli/src` that names
/// the gates adapter outside its two files and the binary's help text.
fn offending() -> Vec<String> {
    let root = root();
    let mut found = Vec::new();
    for dir in ["crates/takt/src", "crates/takt-cli/src"] {
        for path in sources(&root.join(dir)) {
            let relative = path
                .strip_prefix(&root)
                .expect("under the root")
                .to_string_lossy()
                .into_owned();
            let text = fs::read_to_string(&path).expect("a source file");
            for (at, line) in text.lines().enumerate() {
                if offends(&relative, line) {
                    found.push(format!("{relative}:{}: {}", at + 1, line.trim()));
                }
            }
        }
    }
    found
}

#[test]
fn gates_is_named_only_by_its_two_adapters_and_the_binarys_help() {
    let found = offending();
    assert!(
        found.is_empty(),
        "b10x-gates or its policy variable named outside forge/bot.rs and scan/gates.rs:\n{}",
        found.join("\n")
    );
}

/// The rule reads what it is meant to: a literal in a library file, in a comment of the
/// binary's code, in a test module's line, is found; the same in either adapter, and a `///`
/// line of the binary, is not. Both adapters exist and name the program, so the rule is not
/// met by their absence.
#[test]
fn the_rule_finds_a_literal_outside_the_adapters() {
    for (relative, text) in [
        (
            "crates/takt/src/run.rs",
            "    let program = \"b10x-gates\";",
        ),
        (
            "crates/takt/src/forge.rs",
            "/// pushes through b10x-gates bot",
        ),
        (
            "crates/takt/src/forge/gitlab.rs",
            "        .env(\"B10X_GATES_POLICY\", p)",
        ),
        (
            "crates/takt-cli/src/main.rs",
            "    // b10x-gates pushes the branch",
        ),
        (
            "crates/takt-cli/src/main.rs",
            "        assert!(line.starts_with(\"b10x-gates\"));",
        ),
    ] {
        assert!(offends(relative, text), "{relative}: {text}");
    }
    for (relative, text) in [
        ("crates/takt/src/forge/bot.rs", "    \"b10x-gates\","),
        (
            "crates/takt/src/scan/gates.rs",
            "pub const POLICY_VAR: &str = \"B10X_GATES_POLICY\";",
        ),
        (
            "crates/takt-cli/src/main.rs",
            "    /// The forge: `bot` (`b10x-gates` pushes the branch)",
        ),
        (
            "crates/takt/src/run.rs",
            "    let program = gates::PROGRAM;",
        ),
    ] {
        assert!(!offends(relative, text), "{relative}: {text}");
    }
    let root = root();
    for adapter in ADAPTERS {
        let text = fs::read_to_string(root.join(adapter))
            .unwrap_or_else(|error| panic!("{adapter}: {error}"));
        assert!(
            NEEDLES.iter().any(|needle| text.contains(needle)),
            "{adapter} names the program"
        );
    }
}
