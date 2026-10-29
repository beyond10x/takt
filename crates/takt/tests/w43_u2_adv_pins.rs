//! Adversary pass on wave 43 unit U2, the dependency pins: the harness the container image ships
//! against the harness wire the lock holds and the pages that name it, and the session-request
//! document metaharness 0.23.0 publishes through loom-sdk against the acceptance items of the
//! stories its blocker held.

use std::path::{Path, PathBuf};

use loom_sdk::session::metaharness::check_spec;
use loom_sdk::session::metaharness::protocol::RunSpec;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// The value of `ARG <name>=` in the Dockerfile.
fn dockerfile_arg(name: &str) -> String {
    let prefix = format!("ARG {name}=");
    read("Dockerfile")
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
        .unwrap_or_else(|| panic!("the Dockerfile sets {name}"))
}

/// The version of the package `name` in `Cargo.lock`.
fn locked(name: &str) -> String {
    let lock = read("Cargo.lock");
    let marker = format!("name = \"{name}\"\n");
    let at = lock
        .find(&marker)
        .unwrap_or_else(|| panic!("Cargo.lock holds {name}"));
    lock[at + marker.len()..]
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_else(|| panic!("{name}'s version"))
        .to_owned()
}

/// The one line of `page` that `is_it` picks.
fn line_of(page: &str, is_it: impl Fn(&str) -> bool) -> String {
    let text = read(page);
    let found: Vec<&str> = text.lines().filter(|line| is_it(line)).collect();
    assert_eq!(found.len(), 1, "{page}: one line picked, found {found:?}");
    found[0].to_owned()
}

/// AGENTS.md: `HARNESS_VERSION` moves with the harness wire the `Cargo.lock` holds.
#[test]
fn the_image_ships_the_harness_the_lock_holds() {
    assert_eq!(
        dockerfile_arg("HARNESS_VERSION"),
        locked("b10x-harness-wire"),
        "the Dockerfile's HARNESS_VERSION is the locked b10x-harness-wire"
    );
}

/// The pages that say what the image carries name the harness the Dockerfile pins.
#[test]
fn the_image_pages_name_the_harness_the_dockerfile_pins() {
    let pinned = dockerfile_arg("HARNESS_VERSION");
    let lines = [
        (
            "website/docs/reference/container-image.md",
            line_of("website/docs/reference/container-image.md", |line| {
                line.starts_with("| [b10x-harness]")
            }),
        ),
        (
            "website/docs/guides/run-in-a-container.md",
            line_of("website/docs/guides/run-in-a-container.md", |line| {
                line.starts_with("The image carries takt")
            }),
        ),
    ];
    let stale: Vec<String> = lines
        .iter()
        .filter(|(_, line)| !line.contains(&pinned))
        .map(|(page, line)| format!("{page}: {line}"))
        .collect();
    assert!(
        stale.is_empty(),
        "the image ships b10x-harness {pinned}, and these lines name another:\n{}",
        stale.join("\n")
    );
}

/// The pages that say what `--harness harness` needs name the harness metaharness admits, the
/// harness wire the lock holds.
#[test]
fn the_requirement_pages_name_the_harness_the_lock_holds() {
    let admitted = locked("b10x-harness-wire");
    let lines = [
        (
            "website/docs/guides/install.md",
            line_of("website/docs/guides/install.md", |line| {
                line.starts_with("| `--harness harness` |")
            }),
        ),
        (
            "website/docs/concepts/architecture.md",
            line_of("website/docs/concepts/architecture.md", |line| {
                line.starts_with("| `harness` | b10x-harness")
            }),
        ),
    ];
    let stale: Vec<String> = lines
        .iter()
        .filter(|(_, line)| !line.contains(&admitted))
        .map(|(page, line)| format!("{page}: {line}"))
        .collect();
    assert!(
        stale.is_empty(),
        "metaharness admits b10x-harness {admitted}, and these lines name another:\n{}",
        stale.join("\n")
    );
}

/// The blocker's observed claims: an unknown key is refused by the document reader, and
/// `max-turns` on Codex by `check_spec`, both reached through loom-sdk.
#[test]
fn metaharness_refuses_an_unknown_key_and_a_codex_turn_ceiling() {
    assert!(
        RunSpec::from_document_str(r#"{"kind":"claude","no-such-key":1}"#).is_err(),
        "an unknown key is refused"
    );
    let codex = RunSpec::from_document_str(r#"{"kind":"codex","max-turns":3}"#)
        .expect("the document reads");
    assert!(check_spec(&codex).is_err(), "codex max-turns is refused");
}

/// story:harness-section-is-metaharness-session-request acceptance 4,
/// `harness-section/bad-value-refused-by-metaharness`: `harness: {kind: claude, max-turns: sixty}`
/// is refused with metaharness's own message, a value of the wrong type. An unknown `effort`
/// value is passed through to whoever serves the model, so the item no longer names one.
#[test]
fn metaharness_refuses_a_value_of_the_wrong_type() {
    assert!(
        RunSpec::from_document_str(r#"{"kind":"claude","max-turns":"sixty"}"#).is_err(),
        "metaharness 0.23.0 refuses a max-turns that is no number"
    );
    let effort = RunSpec::from_document_str(r#"{"kind":"claude","effort":"extreme"}"#)
        .expect("an unknown effort value reads");
    assert!(
        check_spec(&effort).is_ok(),
        "an unknown effort value is passed through, not refused"
    );
}
