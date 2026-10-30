//! Adversarial cases of wave 40: story:a-duplicate-yaml-key-is-refused-at-its-line (U4, the
//! duplicate found from libyaml's events in the mapping serde_yaml_ng's error points at) and
//! story:a-turn-pins-the-repository-of-its-forge (U3, `authority.yaml`'s `mainline_tip` and
//! `mainline_repository`, and the `forge` file).

use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

/// A scratch directory under this crate's build directory, one per case.
fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w40-adv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// Writes `text` as `<dir(name)>/<file>` and returns its path.
fn file(name: &str, file: &str, text: &str) -> PathBuf {
    let path = dir(name).join(file);
    std::fs::write(&path, text).expect("a configuration file");
    path
}

/// `RunConfig::read`'s refusal rendered as the CLI prints it.
fn refusal(path: &Path) -> String {
    format!("{:#}", RunConfig::read(path).expect_err("refused"))
}

/// Each `(name, text, want)`: the refusal holds `want`; the failures, one line each.
fn wrong(cases: &[(&str, &str, &str)]) -> Vec<String> {
    let mut wrong = Vec::new();
    for (name, text, want) in cases {
        let y = refusal(&file(name, "run.yaml", text));
        if !y.contains(want) {
            wrong.push(format!("{name}: {text:?}: want {want:?}: {y}"));
        }
    }
    wrong
}

/// Probes the fix should and does hold: a byte order mark, multi-byte text before the mapping,
/// a duplicate deep in nested mappings, a declared name duplicated in a nested mapping.
#[test]
fn w40_adv_duplicates_found_at_their_line_bom_unicode_depth() {
    let deep = {
        let mut text = String::new();
        for depth in 0..60 {
            text.push_str(&format!("{}k{depth}:\n", "  ".repeat(depth)));
        }
        let indent = "  ".repeat(60);
        text.push_str(&format!("{indent}iterations: 1\n{indent}iterations: 2\n"));
        text
    };
    let cases: Vec<(&str, &str, &str)> = vec![
        (
            "bom",
            "\u{feff}iterations: 1\nhours: 1.0\niterations: 2\n",
            "line 3: it holds the key `iterations` twice",
        ),
        (
            "unicode",
            "test_cmd: \"\u{fc}n\u{ef}c\u{f8}d\u{e9} \u{2713}\u{1f600}\"\niterations: 1\niterations: 2\n",
            "line 3: it holds the key `iterations` twice",
        ),
        (
            "deep",
            &deep,
            "line 62: it holds the key `iterations` twice",
        ),
        (
            "nested-declared",
            "commit:\n  iterations: 1\n  iterations: 2\n",
            "line 3: it holds the key `iterations` twice",
        ),
        (
            "crlf-flow",
            "{iterations: 1,\r\n hours: 1.0,\r\n iterations: 2}\r\n",
            "line 3: it holds the key `iterations` twice",
        ),
        (
            "anchored-mapping",
            "protocols: &p\n  a: {}\n  a: {}\nx: *p\n",
            "line 3: it holds a key twice",
        ),
    ];
    let wrong = wrong(&cases);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The duplicate written as an alias of the first key: serde_yaml_ng resolves `*k` to
/// `iterations` and refuses the mapping as holding it twice; the refusal is at the mapping's
/// first line and says nothing of a key held twice, where the story asks for the duplicate's
/// own line.
#[test]
fn w40_adv_a_duplicate_written_as_an_alias_is_refused_at_its_line() {
    let wrong = wrong(&[(
        "alias-key",
        "&k iterations: 1\nhours: 1.0\n*k : 2\n",
        "line 3: it holds the key `iterations` twice",
    )]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A tagged key before the duplicate (`!!str model`): the event reader stops at the first tag,
/// so the refusal falls back to the mapping's first line and says nothing of a key held twice.
#[test]
fn w40_adv_a_tagged_key_before_the_duplicate_hides_it() {
    let wrong = wrong(&[(
        "tag-before",
        "!!str model: x\niterations: 1\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    )]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A plain key that ends in a colon (`iterations::` is the key `iterations:`): its text read back
/// as `k: iterations:` is not one scalar, so the reader gives up on the mapping, and the refusal
/// says nothing of a key held twice.
#[test]
fn w40_adv_a_plain_key_ending_in_a_colon_is_found_at_its_line() {
    let wrong = wrong(&[(
        "colon-key",
        "x:: 1\nhours: 1.0\nx:: 2\n",
        "line 3: it holds a key twice",
    )]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// `authority.yaml` keeps a tip YAML would read as a number, written by takt and read back,
/// and as a person writes it unquoted; a file without `mainline_repository` reads as no
/// repository; a repository with no tip is no tip.
#[test]
fn w40_adv_authority_keeps_number_like_tips_and_old_files_read() {
    let tips = [
        "0123456789012345678901234567890123456789",
        "1234567890123456789012345678901234567890",
        "1e23456789012345678901234567890123456789",
        "0b01010101010101010101010101010101010101",
        "0e00000000000000000000000000000000000000",
    ];
    for (index, tip) in tips.into_iter().enumerate() {
        let dir = dir(&format!("authority-{index}"));
        let decision = takt::case::AuthorityDecision {
            capability: "repository.merge.judge_surface".to_owned(),
            decision: takt::case::Grant::Granted,
            approver: "human:owner".to_owned(),
            granted_at: "2026-10-28T00:00:00Z".to_owned(),
            revision: Some("a".repeat(40)),
            mainline_tip: Some(takt::case::MainlineTip {
                tip: tip.to_owned(),
                repository: Some("acme/one".to_owned()),
            }),
        };
        takt::case::Authority {
            decisions: vec![decision.clone()],
        }
        .write(&dir)
        .expect("written");
        let read = takt::case::Authority::read(&dir)
            .expect("reads")
            .expect("held");
        assert_eq!(read.decisions, vec![decision.clone()], "{tip}");
        std::fs::write(
            dir.join("authority.yaml"),
            format!(
                "- capability: repository.merge.judge_surface\n  decision: granted\n  approver: human:owner\n  granted_at: '2026-10-28T00:00:00Z'\n  revision: {}\n  mainline_tip: {tip}\n  mainline_repository: acme/one\n",
                "a".repeat(40)
            ),
        )
        .expect("written");
        let read = takt::case::Authority::read(&dir)
            .expect("reads")
            .expect("held");
        assert_eq!(read.decisions, vec![decision], "{tip} unquoted");
    }
    let dir = dir("authority-old");
    std::fs::write(
        dir.join("authority.yaml"),
        "- capability: repository.merge.judge_surface\n  decision: granted\n  approver: human:owner\n  granted_at: '2026-10-28T00:00:00Z'\n  revision: r\n  mainline_tip: abc\n",
    )
    .expect("written");
    let read = takt::case::Authority::read(&dir)
        .expect("reads")
        .expect("held");
    let tip = read.decisions[0].mainline_tip.clone().expect("a tip");
    assert_eq!((tip.tip.as_str(), tip.repository), ("abc", None));
    std::fs::write(
        dir.join("authority.yaml"),
        "- capability: repository.merge.judge_surface\n  decision: granted\n  approver: human:owner\n  granted_at: '2026-10-28T00:00:00Z'\n  mainline_repository: acme/one\n",
    )
    .expect("written");
    let read = takt::case::Authority::read(&dir)
        .expect("reads")
        .expect("held");
    assert_eq!(read.decisions[0].mainline_tip, None);
}
