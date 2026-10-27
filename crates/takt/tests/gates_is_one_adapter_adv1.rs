//! Adversary pass 1 on story:scan-and-forge-are-adapters, Acceptance 6, read past one line and
//! one literal. `gates_is_one_adapter.rs` looks for `b10x-gates` and `B10X_GATES` written whole
//! on one line, so the program's name built by `concat!("b10x", "-gates")`, or a string
//! continued over a line (`"b10x-\` then `gates"`), stays green outside the two adapter files.
//! This rule reads each logical line (a line ending in `\` joined to the next) and finds the two
//! words `b10x` and `gates` together on it, whatever stands between them.

use std::fs;
use std::path::{Path, PathBuf};

/// The two files that are the gates adapters, relative to the workspace's root.
const ADAPTERS: [&str; 2] = [
    "crates/takt/src/forge/bot.rs",
    "crates/takt/src/scan/gates.rs",
];

/// The binary's sources, where a `///` line (clap's help text) may name the adapters.
const CLI_SOURCES: &str = "crates/takt-cli/src/";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace's root")
}

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

/// The logical lines of `text`, numbered from 1: a line ending in `\` (a string continued) is
/// joined to the next with its leading white space dropped, as the compiler reads the literal.
fn logical_lines(text: &str) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    let mut pending: Option<(usize, String)> = None;
    for (at, line) in text.lines().enumerate() {
        let (start, mut joined) = match pending.take() {
            Some((start, before)) => (start, before + line.trim_start()),
            None => (at + 1, line.to_owned()),
        };
        if joined.ends_with('\\') {
            joined.pop();
            pending = Some((start, joined));
        } else {
            lines.push((start, joined));
        }
    }
    lines.extend(pending);
    lines
}

/// Whether `line` names the gates program or its variable in any spelling: the words `b10x`
/// and `gates` both on it, compared without case.
fn names_gates(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("b10x") && lower.contains("gates")
}

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
            if ADAPTERS.contains(&relative.as_str()) {
                continue;
            }
            let text = fs::read_to_string(&path).expect("a source file");
            for (at, line) in logical_lines(&text) {
                let help =
                    relative.starts_with(CLI_SOURCES) && line.trim_start().starts_with("///");
                if names_gates(&line) && !help {
                    found.push(format!("{relative}:{at}: {}", line.trim()));
                }
            }
        }
    }
    found
}

#[test]
fn gates_is_named_by_no_other_file_in_any_spelling() {
    let found = offending();
    assert!(
        found.is_empty(),
        "the gates program or its variable named outside forge/bot.rs and scan/gates.rs:\n{}",
        found.join("\n")
    );
}

/// The rule finds what the line-literal rule misses, and keeps what it keeps.
#[test]
fn the_rule_reads_a_built_or_continued_literal() {
    for text in [
        "const P: &str = concat!(\"b10x\", \"-gates\");",
        "const V: &str = concat!(\"B10X_\", \"GATES_POLICY\");",
        "let p = format!(\"{}-gates\", \"b10x\");",
    ] {
        assert!(names_gates(text), "{text}");
    }
    let continued = logical_lines("const P: &str = \"b10x-\\\n    gates\";\n");
    assert_eq!(
        continued,
        [(1, "const P: &str = \"b10x-gates\";".to_owned())]
    );
    assert!(names_gates(&continued[0].1));
    for text in [
        "let program = gates::PROGRAM;",
        "use b10x_canon::model;",
        "/// The name, `b10x-bot[bot]`.",
    ] {
        assert!(!names_gates(text), "{text}");
    }
}
