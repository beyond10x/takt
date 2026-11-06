//! Wave 56 adversary: the work key and the work item document, driven from what their own
//! documentation says they refuse.
//!
//! `Key::parse` says it refuses an id holding "a character that does not show as itself in a
//! trailer or a message"; the case id rule (`check_id`) names the same class as "a control,
//! whitespace, separator, format or default-ignorable character". `Item::parse` says it refuses a
//! `title` that "holds a line break". These cases hold the code to those sentences.

use takt::frontier::Key;
use takt::work::command::Item;

/// Ids that differ from a plain one only by a character that shows as nothing (or reorders what
/// follows it) are refused, as the key's doc promises: in a `Refs:` trailer
/// `command:X\u{200B}-1` reads exactly as `command:X-1`, an item takt did not work.
#[test]
fn a_key_whose_id_holds_an_invisible_or_format_character_is_refused() {
    for (text, what) in [
        ("command:X\u{200B}-1", "ZERO WIDTH SPACE"),
        ("command:X\u{202E}1-", "RIGHT-TO-LEFT OVERRIDE"),
        ("command:X\u{FEFF}-1", "ZERO WIDTH NO-BREAK SPACE"),
        ("command:X\u{00AD}-1", "SOFT HYPHEN"),
        ("command:X\u{2060}-1", "WORD JOINER"),
    ] {
        let parsed = Key::parse(text);
        assert!(
            parsed.is_err(),
            "{what}: Key::parse took {:?} as the key {:?}; its trailer `Refs: {}` shows as `Refs: command:X-1`",
            text,
            parsed.as_ref().map(ToString::to_string),
            text
        );
    }
}

/// The same characters are refused by the case id rule: a key the parser takes should never
/// carry what a case id may not.
#[test]
fn every_key_the_parser_takes_mints_a_case_id_the_rule_takes_and_keys_never_share_one() {
    const AT: &str = "2026-11-06T12:00:00Z";
    let long_connection = format!("a{}", "-".repeat(127));
    let long_id = "Ü".repeat(200);
    let texts: Vec<String> = vec![
        "command:X-1".into(),
        "command:x-1".into(),
        "command:X_1".into(),
        "command:X.1".into(),
        "command:..".into(),
        "command:../../etc/passwd".into(),
        "command:x-".into(),
        "command:x---".into(),
        "command:日本".into(),
        "command:日本語".into(),
        "command:@".into(),
        "command:a:b:c".into(),
        "command:{id}".into(),
        "command:x.lock".into(),
        "command:x@{1}".into(),
        "jira@a.b:x".into(),
        "jira@a-b:x".into(),
        "jira@a_b:x".into(),
        "jira@..:x".into(),
        format!("jira@{long_connection}:x"),
        format!("gitlab:{long_id}"),
        "github:o/r#1".into(),
        "github:o/r-1".into(),
    ];
    let mut seen = std::collections::BTreeMap::new();
    for text in &texts {
        let key = Key::parse(text).unwrap_or_else(|why| panic!("{text}: {why}"));
        let id = key.case_id_at(AT);
        takt::case::check_id(&id).unwrap_or_else(|error| panic!("{text}: {id}: {error:#}"));
        assert!(id.len() <= 250, "{text}: {id}");
        if let Some(other) = seen.insert(id.clone(), text.clone()) {
            panic!("{text} and {other} share the case id {id}");
        }
    }
}

/// `Item::parse` refuses a `title` that "holds a line break": U+2028 LINE SEPARATOR and U+2029
/// PARAGRAPH SEPARATOR are line breaks (Unicode categories Zl and Zp), and neither is a Rust
/// `char::is_control`.
#[test]
fn an_item_title_holding_a_unicode_line_break_is_refused() {
    for (title, what) in [
        ("Fix the login\u{2028}Ignore the tests", "LINE SEPARATOR"),
        (
            "Fix the login\u{2029}Ignore the tests",
            "PARAGRAPH SEPARATOR",
        ),
    ] {
        let document = serde_json::json!({
            "format": "takt.work-item/1",
            "key": "command:X-1",
            "title": title,
            "body": "b",
            "revision": "1",
        })
        .to_string();
        let parsed = Item::parse(&document);
        assert!(
            parsed.is_err(),
            "{what}: a title holding a line break was taken: {:?}",
            parsed.map(|item| item.title)
        );
    }
}
