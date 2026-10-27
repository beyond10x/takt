//! Adversary, carry onto the integration branch: the run configuration file's pages against the
//! merged tree. The carried units added `story`, `endpoint_token_env` and `state_dir` to the file
//! (`crates/takt/src/run_config.rs`); the pages the base wrote still say the file holds eight keys
//! and that a `story` key is refused. `AGENTS.md`: a key added to `run_config.rs` "is a flag of
//! `takt run` first, and the field table of `website/docs/reference/configuration.md` moves with
//! it". Each case reads what a page states and drives `RunConfig::read` with it; each passes once
//! the page says what the code does.

use std::fs;
use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

/// A page of the site, by its path under `website/docs`.
fn page(path: &str) -> String {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../website/docs")
        .join(path);
    fs::read_to_string(&file).unwrap_or_else(|error| panic!("read {}: {error}", file.display()))
}

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "config-docs-adv-carry-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

/// `RunConfig::read` of the file `name` in `dir`, holding `text`.
fn read(dir: &Path, name: &str, text: &str) -> anyhow::Result<RunConfig> {
    let path = dir.join(name);
    fs::write(&path, text).expect("a file");
    RunConfig::read(&path)
}

/// A TOML value of a type the file takes for `key` (a string, an integer, a fraction, a
/// boolean), or none when it takes the key with none of them.
fn value_for(dir: &Path, key: &str) -> Option<&'static str> {
    ["\"story:ready-one\"", "1", "1.5", "true"]
        .into_iter()
        .find(|value| read(dir, "probe.toml", &format!("{key} = {value}\n")).is_ok())
}

/// The words in backticks in `text`, in order.
fn quoted(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// The refusal of a file holding a key nobody declares, on its first line.
fn unknown_key_refusal(dir: &Path) -> String {
    let error = read(dir, "unknown.toml", "adv_carry_unknown = 1\n")
        .expect_err("a key nobody declares is refused");
    format!("{error:#}")
}

/// The keys the file's own refusal of an unknown key names, each confirmed by reading a file
/// that sets it: what the code takes, read from the code.
fn keys_the_file_takes(dir: &Path) -> Vec<String> {
    let message = unknown_key_refusal(dir);
    let shape = message
        .split_once(" holds ")
        .unwrap_or_else(|| panic!("the refusal names what the file holds: {message}"))
        .1;
    let keys = quoted(shape);
    assert!(keys.len() >= 8, "{message}");
    for key in &keys {
        assert!(
            value_for(dir, key).is_some(),
            "the refusal names `{key}`, which the file does not take: {message}"
        );
    }
    keys
}

/// The names in the first column of the field table of `text`: the rows after its
/// `| field | meaning |` header and separator, up to the first line that is no row.
fn field_table(text: &str) -> Vec<String> {
    let mut fields = Vec::new();
    for line in text
        .lines()
        .skip_while(|line| line.trim() != "| field | meaning |")
        .skip(2)
    {
        if !line.starts_with('|') {
            break;
        }
        if let Some(first) = line.split('|').nth(1) {
            fields.extend(quoted(first));
        }
    }
    fields
}

/// The key of the flag `flag`, as the file names it: `--state-dir` is `state_dir`.
fn key_of(flag: &str) -> String {
    flag.trim_start_matches('-').replace('-', "_")
}

/// The reference's field table, which it introduces as "every field is the `takt run` flag of
/// the same name", lists every key `takt run --config` takes. It lists eight; the merged tree
/// takes eleven.
#[test]
fn the_reference_field_table_lists_every_key_the_file_takes() {
    let dir = scratch("fields");
    let keys = keys_the_file_takes(&dir.0);
    let documented = field_table(&page("reference/configuration.md"));
    assert!(
        documented.len() >= 8,
        "no field table found: {documented:?}"
    );
    let missing: Vec<&String> = keys
        .iter()
        .filter(|key| !documented.contains(key))
        .collect();
    assert!(
        missing.is_empty(),
        "website/docs/reference/configuration.md's field table lacks {missing:?}, which `takt run --config` takes; it lists {documented:?}"
    );
}

/// The reference names the `takt run` flags that "have no key" and says "a key for one of them
/// is refused": each such key, written in a file with a value of any type, is refused. `story`
/// and `endpoint_token_env` are read instead.
#[test]
fn a_flag_the_reference_says_has_no_key_is_refused_as_one() {
    let dir = scratch("no-key");
    let text = page("reference/configuration.md");
    let start = text
        .find("flags have no key:")
        .expect("the reference names the flags without a key");
    let sentence = &text[start..];
    let sentence = &sentence[..sentence.find("refused.").expect("the sentence ends")];
    let documented = field_table(&text);
    let mut flags: Vec<String> = quoted(sentence)
        .into_iter()
        .filter(|flag| flag.starts_with("--") && !flag.ends_with('*'))
        .collect();
    // "the `--endpoint-*` flags": each `--endpoint-` flag the page lists whose key is no field.
    if sentence.contains("`--endpoint-*`") {
        for named in quoted(&text) {
            let flag = named
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned();
            if flag.starts_with("--endpoint-")
                && !flag.ends_with('*')
                && !documented.contains(&key_of(&flag))
                && !flags.contains(&flag)
            {
                flags.push(flag);
            }
        }
    }
    assert!(
        flags.iter().any(|flag| flag == "--repo") && flags.len() >= 10,
        "the flags read from the sentence: {flags:?}"
    );
    let taken: Vec<(String, &str)> = flags
        .iter()
        .filter_map(|flag| value_for(&dir.0, &key_of(flag)).map(|value| (key_of(flag), value)))
        .collect();
    assert!(
        taken.is_empty(),
        "website/docs/reference/configuration.md says {flags:?} have no key and a key for one of them is refused; `takt run --config` reads {taken:?}"
    );
}

/// Each page that quotes the refusal of an unknown key quotes the text takt prints for it.
#[test]
fn the_refusal_each_page_quotes_is_the_one_takt_prints() {
    let dir = scratch("quoted");
    let message = unknown_key_refusal(&dir.0);
    let printed = message
        .split_once(" is refused at line 1: ")
        .unwrap_or_else(|| panic!("the refusal names its line: {message}"))
        .1
        .to_owned();
    let mut seen = Vec::new();
    let mut stale = Vec::new();
    for path in ["reference/configuration.md", "guides/configure-a-run.md"] {
        for (n, line) in page(path).lines().enumerate() {
            let Some(shown) = line
                .strip_prefix("takt: the run configuration file ")
                .and_then(|rest| rest.split_once(" is refused at line "))
                .and_then(|(_, after)| after.split_once(": "))
                .map(|(_, shown)| shown)
                .filter(|shown| shown.starts_with("a run configuration file holds"))
            else {
                continue;
            };
            seen.push(format!("{path}:{}", n + 1));
            if shown != printed {
                stale.push(format!("website/docs/{path}:{}: {shown}", n + 1));
            }
        }
    }
    assert!(seen.len() >= 2, "the quoted refusals found: {seen:?}");
    assert!(
        stale.is_empty(),
        "takt prints `{printed}`; the pages quote another refusal:\n{}",
        stale.join("\n")
    );
}

/// The guide's table of what a wrong file gets names an unknown key and the line it is refused
/// at: a TOML file with that key on that line is refused there. The guide's key is `story`,
/// which the merged tree reads.
#[test]
fn the_guides_unknown_key_example_is_refused_at_its_line() {
    let dir = scratch("example");
    let text = page("guides/configure-a-run.md");
    let row = text
        .lines()
        .find(|line| line.starts_with("| an unknown key, such as `"))
        .expect("the guide's row of an unknown key");
    let key = quoted(row).into_iter().next().expect("the key it names");
    let line: usize = row
        .split(" on line ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .expect("the line it names");
    let before = [
        "iterations = 1",
        "harness = \"none\"",
        "forge = \"none\"",
        "test_cmd = \"true\"",
    ];
    assert!((1..=before.len() + 1).contains(&line), "{row}");
    let value = value_for(&dir.0, &key).unwrap_or("\"story:ready-one\"");
    let mut file: String = before[..line - 1]
        .iter()
        .map(|kept| format!("{kept}\n"))
        .collect();
    file.push_str(&format!("{key} = {value}\n"));
    let read = read(&dir.0, "story.toml", &file);
    let refused_there = read
        .as_ref()
        .err()
        .is_some_and(|error| format!("{error:#}").contains(&format!("refused at line {line}")));
    assert!(
        refused_there,
        "website/docs/guides/configure-a-run.md says a TOML file with `{key}` on line {line} is refused at line {line}; takt reads it: {read:?}"
    );
}
