//! How the message of a step commit is written: `semantic` (the default), `repository` or
//! `action`, a `trailer` takt writes itself from the case's story, and the `types` an action
//! commits as. The repository wins: a line of an instruction file at the base that names the
//! repository's commit message, style or convention makes `repository` the style in effect,
//! whatever the configuration says. Everything here is deterministic: the match is a test on the
//! base's bytes, and a harness's reply is checked for its shape, never trusted for it.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The most characters a subject holds.
pub const SUBJECT_MAX: usize = 72;
/// The most characters a line of a `semantic` body holds.
pub const BODY_LINE_MAX: usize = 120;
/// The most characters a `repository` body holds.
pub const BODY_MAX: usize = 4000;
/// The key of the trailer takt writes when none is configured.
pub const DEFAULT_TRAILER: &str = "Refs";
/// The type of an action no default and no configuration names.
pub const FALLBACK_TYPE: &str = "chore";
/// The tag of a story whose implementation commits as a fix.
pub const BUG_TAG: &str = "bug";

/// How a step commit's message is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// The harness's reply carries `commit: {subject, body}`: `<type>: <imperative subject>` and
    /// a body of 3 or 4 lines.
    #[default]
    Semantic,
    /// The harness writes the message as the repository's instruction files require.
    Repository,
    /// `<action>: <summary>`, as takt wrote it before styles.
    Action,
}

impl Style {
    /// The style's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Repository => "repository",
            Self::Action => "action",
        }
    }

    /// The style `name` names; none for any other word.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        [Self::Semantic, Self::Repository, Self::Action]
            .into_iter()
            .find(|style| style.name() == name)
    }
}

/// The `commit` section of a run configuration as written: each member optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    /// `style`.
    pub style: Option<Style>,
    /// `trailer`; empty writes none.
    pub trailer: Option<String>,
    /// `types`: an action to a commit type.
    pub types: Option<BTreeMap<String, String>>,
}

/// What a run is configured to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The style asked for; the repository's wins over it ([`Plan::new`]).
    pub style: Style,
    /// The key of the trailer written after the message; empty for none.
    pub trailer: String,
    /// The type each named action commits as, over the defaults ([`Plan::new`]).
    pub types: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            style: Style::Semantic,
            trailer: DEFAULT_TRAILER.to_owned(),
            types: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// The settings `section` gives over the defaults.
    #[must_use]
    pub fn of_section(section: &Section) -> Self {
        let mut settings = Self::default();
        settings.apply(section);
        settings
    }

    /// Writes each member `section` gives over these settings.
    pub fn apply(&mut self, section: &Section) {
        if let Some(style) = section.style {
            self.style = style;
        }
        if let Some(trailer) = &section.trailer {
            self.trailer.clone_from(trailer);
        }
        if let Some(types) = &section.types {
            self.types.extend(types.clone());
        }
    }

    /// Why the settings cannot be used, naming the member and never its value; none when they
    /// can: a trailer is empty or a git trailer key (a letter, then letters, digits and
    /// hyphens), and each type is a lowercase word other than `takt`, whose lines are takt's.
    ///
    /// # Errors
    /// The member that is refused, as text.
    pub fn check(&self) -> Result<(), String> {
        if !self.trailer.is_empty() && !is_key(&self.trailer) {
            return Err(
                "`commit.trailer` is empty or a git trailer key: a letter, then letters, digits and hyphens"
                    .to_owned(),
            );
        }
        for (action, kind) in &self.types {
            if action.is_empty() || action.chars().any(char::is_control) {
                return Err("`commit.types` names an action by its id".to_owned());
            }
            let word = kind
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
                && kind
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && kind.len() <= 20;
            if !word || kind == "takt" {
                return Err(
                    "`commit.types` gives each action a lowercase word other than `takt`"
                        .to_owned(),
                );
            }
        }
        Ok(())
    }
}

/// Whether `text` is a git trailer key: a letter, then letters, digits and hyphens.
fn is_key(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The line of an instruction file at the base that names the repository's commit style.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rule {
    /// The file's path, relative to the repository.
    pub file: String,
    /// The line's number, from 1.
    pub line: usize,
}

/// Whether `line` matches `(?i)commit (message|style|convention)`.
#[must_use]
pub fn names_a_commit_rule(line: &str) -> bool {
    let folded = line.to_lowercase().replace('\u{17f}', "s");
    ["commit message", "commit style", "commit convention"]
        .iter()
        .any(|needle| folded.contains(needle))
}

/// The first line, in the order of `paths`, of an instruction file `revision` of `tree` holds
/// that names the repository's commit style ([`names_a_commit_rule`]); none when no file does
/// or `revision` tracks none of them. The files are read as that revision holds them.
#[must_use]
pub fn rule_in(tree: &Path, revision: &str, paths: &[String]) -> Option<Rule> {
    let files = crate::harness::instruction_files(tree, revision, paths, None);
    files.given.iter().find_map(|file| {
        file.text()
            .lines()
            .position(names_a_commit_rule)
            .map(|at| Rule {
                file: file.path().to_owned(),
                line: at + 1,
            })
    })
}

/// What a run writes, decided when it opens: the style in effect and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The style the configuration gave, or the default.
    pub configured: Style,
    /// The style in effect: `repository` when the base's instructions name a commit rule,
    /// else the configured one.
    pub style: Style,
    /// The trailer's key; empty for none.
    pub trailer: String,
    /// The type each action commits as.
    pub types: BTreeMap<String, String>,
    /// The line that made `repository` the style in effect, when one did.
    pub rule: Option<Rule>,
}

impl Default for Plan {
    fn default() -> Self {
        Self::new(&Settings::default(), None, false)
    }
}

impl Plan {
    /// The plan for `settings`, `rule` the base's line naming a commit style and `bug` whether
    /// the story carries the tag [`BUG_TAG`]: `tests.write` commits as `test`, and
    /// `implementation.edit` as `fix` for a bug and `feat` otherwise, each unless the settings
    /// name a type for it.
    #[must_use]
    pub fn new(settings: &Settings, rule: Option<Rule>, bug: bool) -> Self {
        let mut types = BTreeMap::from([
            ("tests.write".to_owned(), "test".to_owned()),
            (
                "implementation.edit".to_owned(),
                if bug { "fix" } else { "feat" }.to_owned(),
            ),
        ]);
        types.extend(settings.types.clone());
        Self {
            configured: settings.style,
            style: if rule.is_some() {
                Style::Repository
            } else {
                settings.style
            },
            trailer: settings.trailer.clone(),
            types,
            rule,
        }
    }

    /// The type `action` commits as.
    #[must_use]
    pub fn type_of(&self, action: &str) -> &str {
        self.types.get(action).map_or(FALLBACK_TYPE, String::as_str)
    }

    /// What `opened` records: the style in effect and the one configured, the trailer, the
    /// types and the rule's file and line.
    #[must_use]
    pub fn record(&self) -> serde_json::Value {
        let mut record = serde_json::json!({
            "style": self.style.name(),
            "configured": self.configured.name(),
            "trailer": self.trailer,
            "types": self.types,
        });
        if let Some(rule) = &self.rule {
            record["rule"] = serde_json::json!({"file": rule.file, "line": rule.line});
        }
        record
    }

    /// What a step is asked to reply with, for the styles that ask; none for `action`.
    #[must_use]
    pub fn ask(&self) -> Option<Ask> {
        (self.style != Style::Action).then(|| Ask {
            style: self.style,
            types: self.types.clone(),
        })
    }
}

/// What a step's prompt asks of the harness's reply: a `commit` of the style, and the type of
/// each action it may choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// `Semantic` or `Repository`.
    pub style: Style,
    /// The type each action commits as; an action not named is [`FALLBACK_TYPE`].
    pub types: BTreeMap<String, String>,
}

impl Ask {
    /// The ask a run's `opened` record states in its `commit` member, as [`Plan::record`]
    /// writes it; none for a record of the `action` style or one that does not read.
    #[must_use]
    pub fn of_record(record: &serde_json::Value) -> Option<Self> {
        let style = Style::named(record["style"].as_str()?)?;
        if style == Style::Action {
            return None;
        }
        let types = record["types"]
            .as_object()?
            .iter()
            .filter_map(|(action, kind)| Some((action.clone(), kind.as_str()?.to_owned())))
            .collect();
        Some(Self { style, types })
    }

    /// The paragraph of the prompt that asks for the `commit` member, naming the type of each of
    /// the `offered` actions.
    #[must_use]
    pub fn paragraph(&self, offered: &[String]) -> String {
        let mut text = String::from(
            "Commit message: takt commits your change with the message you give in a `commit` \
             member of that JSON line, an object with a `subject` and a `body`, both strings. ",
        );
        match self.style {
            Style::Repository => text.push_str(
                "The repository's own instructions, given above, say how a commit message is \
                 written: write it as they require. The subject is one line of at most 72 \
                 characters. ",
            ),
            _ => {
                text.push_str(
                    "The subject is `<type>: <imperative subject>` on one line of at most 72 \
                     characters, the type being",
                );
                let mut named: Vec<String> = Vec::new();
                for action in offered {
                    let kind = self.types.get(action).map_or(FALLBACK_TYPE, String::as_str);
                    named.push(format!(" `{kind}` for `{action}`"));
                }
                text.push_str(&named.join(","));
                text.push_str(
                    ". The body is 3 or 4 lines, none of them blank and each at most 120 \
                     characters, saying the intent of the change. ",
                );
            }
        }
        text.push_str(
            "No line of the body may open with a word and a colon. Write no trailer: takt \
             writes it.\n\n",
        );
        text
    }
}

/// A message that passed its check: its subject and its body's lines.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reply {
    subject: String,
    body: Vec<String>,
}

/// The message a step commits, and why it is not the harness's when it is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// The whole message, ending in a line end, the trailer last when there is one.
    pub text: String,
    /// Why the harness's `commit` was not used, for the styles that ask for one; none when it
    /// was used or the style asks for none.
    pub fallback: Option<String>,
}

/// `text` on one line: every run of white space, line breaks among them, as one space.
#[must_use]
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether `line` opens with a word and a colon: the shape of a trailer, and of a line of
/// takt's own (`takt: take`).
fn trailer_shaped(line: &str) -> bool {
    line.trim_start().split_once(':').is_some_and(|(word, _)| {
        !word.is_empty()
            && word
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    })
}

/// The `reply`'s `commit` as a message of `style` and `kind` (the action's type), or why it is
/// not one. The reason names the check, never the reply's text.
fn checked(style: Style, kind: &str, reply: Option<&serde_json::Value>) -> Result<Reply, String> {
    let reply = reply.ok_or("the reply has no `commit`")?;
    let object = reply
        .as_object()
        .ok_or("the reply's `commit` is not an object")?;
    let subject = object
        .get("subject")
        .and_then(serde_json::Value::as_str)
        .ok_or("the commit's `subject` is not a string")?;
    let body = object
        .get("body")
        .and_then(serde_json::Value::as_str)
        .ok_or("the commit's `body` is not a string")?;
    if subject.trim().is_empty() || subject.contains('\n') || subject.chars().any(char::is_control)
    {
        return Err("the commit's subject is not one line of text".to_owned());
    }
    if subject.chars().count() > SUBJECT_MAX {
        return Err(format!(
            "the commit's subject is past {SUBJECT_MAX} characters"
        ));
    }
    if trailer_shaped(subject) && subject.trim_start().to_lowercase().starts_with("takt:") {
        return Err("the commit's subject opens as a line of takt's".to_owned());
    }
    if body
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err("the commit's body holds a control character".to_owned());
    }
    let lines: Vec<String> = body
        .trim_end()
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect();
    if lines.iter().any(|line| trailer_shaped(line)) {
        return Err(
            "a line of the commit's body opens with a word and a colon, the shape of a trailer or of a line of takt's"
                .to_owned(),
        );
    }
    if style == Style::Semantic {
        let rest = subject
            .strip_prefix(kind)
            .and_then(|rest| match rest.strip_prefix('(') {
                Some(scoped) => scoped
                    .split_once(')')
                    .filter(|(scope, _)| !scope.is_empty() && !scope.contains(char::is_whitespace))
                    .map(|(_, after)| after),
                None => Some(rest),
            });
        if !rest
            .and_then(|rest| rest.strip_prefix(": "))
            .is_some_and(|text| !text.trim().is_empty())
        {
            return Err(format!(
                "the commit's subject does not open with `{kind}: ` or `{kind}(scope): `"
            ));
        }
        if !(3..=4).contains(&lines.len()) {
            return Err("the commit's body is not 3 or 4 lines".to_owned());
        }
        if lines.iter().any(|line| line.trim().is_empty()) {
            return Err("the commit's body has a blank line".to_owned());
        }
        if lines
            .iter()
            .any(|line| line.chars().count() > BODY_LINE_MAX)
        {
            return Err(format!(
                "a line of the commit's body is past {BODY_LINE_MAX} characters"
            ));
        }
    } else if body.chars().count() > BODY_MAX {
        return Err(format!("the commit's body is past {BODY_MAX} characters"));
    }
    Ok(Reply {
        subject: subject.to_owned(),
        body: lines,
    })
}

/// The message of the step commit for `action`, its summary `summary` and the harness's `reply`
/// (the whole JSON line's `commit` member, when it had one), under `plan`:
/// - `action`: `<action>: <summary>`.
/// - `semantic` and `repository`: the reply's `commit` when it passes its check, else
///   `<type>: <title>` (at most 72 characters) with the summary on one line as the body, and the
///   reason it did not pass.
///
/// `title` is the story's title, the intent's first line for a case with no story; `story` the
/// story's id, from which takt writes the trailer, none for a case with no story. The trailer is
/// never read from `reply`.
#[must_use]
pub fn message(
    plan: &Plan,
    action: &str,
    summary: &str,
    title: &str,
    reply: Option<&serde_json::Value>,
    story: Option<&str>,
) -> Message {
    let summary = one_line(summary);
    let (mut text, fallback) = if plan.style == Style::Action {
        (format!("{action}: {summary}"), None)
    } else {
        let kind = plan.type_of(action);
        match checked(plan.style, kind, reply) {
            Ok(reply) => {
                let mut text = reply.subject;
                if !reply.body.is_empty() {
                    text.push_str("\n\n");
                    text.push_str(&reply.body.join("\n"));
                }
                (text, None)
            }
            Err(why) => {
                let title = one_line(title);
                let title = if title.is_empty() { "change" } else { &title };
                let subject: String = format!("{kind}: {title}")
                    .chars()
                    .take(SUBJECT_MAX)
                    .collect();
                let mut text = subject.trim_end().to_owned();
                if !summary.is_empty() {
                    text.push_str("\n\n");
                    if trailer_shaped(&summary) {
                        text.push_str("reported: ");
                    }
                    text.push_str(&summary);
                }
                (text, Some(why))
            }
        }
    };
    if let Some(story) = story.filter(|_| !plan.trailer.is_empty()) {
        text.push_str(&format!("\n\n{}: {story}", plan.trailer));
    }
    text.push('\n');
    Message { text, fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn semantic_reply(subject: &str, body: &str) -> serde_json::Value {
        serde_json::json!({"subject": subject, "body": body})
    }

    #[test]
    fn the_rule_is_the_pattern_of_the_story() {
        for line in [
            "Follow the commit message rules",
            "COMMIT STYLE: sentences",
            "our Commit Convention",
            "write a commit messages like so",
        ] {
            assert!(names_a_commit_rule(line), "{line}");
        }
        for line in [
            "commit  message",
            "commits message",
            "message of the commit",
            "",
        ] {
            assert!(!names_a_commit_rule(line), "{line}");
        }
    }

    #[test]
    fn a_bug_makes_the_implementation_a_fix_and_a_configured_type_wins() {
        let plan = Plan::new(&Settings::default(), None, true);
        assert_eq!(plan.type_of("implementation.edit"), "fix");
        assert_eq!(plan.type_of("tests.write"), "test");
        assert_eq!(plan.type_of("docs.write"), "chore");
        let settings = Settings {
            types: [("implementation.edit".to_owned(), "build".to_owned())].into(),
            ..Settings::default()
        };
        assert_eq!(
            Plan::new(&settings, None, true).type_of("implementation.edit"),
            "build"
        );
    }

    #[test]
    fn a_rule_makes_the_repository_the_style_in_effect() {
        let rule = Rule {
            file: "AGENTS.md".to_owned(),
            line: 3,
        };
        let plan = Plan::new(&Settings::default(), Some(rule), false);
        assert_eq!(plan.style, Style::Repository);
        assert_eq!(plan.configured, Style::Semantic);
        assert_eq!(plan.record()["rule"]["line"], 3);
        assert_eq!(
            Ask::of_record(&plan.record()).map(|ask| ask.style),
            Some(Style::Repository)
        );
    }

    #[test]
    fn a_well_formed_reply_is_the_message_and_the_trailer_is_takts() {
        let plan = Plan::default();
        let reply = semantic_reply("feat(add): sum the numbers", "One.\nTwo.\nThree.");
        let made = message(
            &plan,
            "implementation.edit",
            "ignored",
            "Title",
            Some(&reply),
            Some("story:x"),
        );
        assert_eq!(made.fallback, None);
        assert_eq!(
            made.text,
            "feat(add): sum the numbers\n\nOne.\nTwo.\nThree.\n\nRefs: story:x\n"
        );
    }

    #[test]
    fn each_check_of_the_shape_falls_back() {
        let plan = Plan::default();
        let long = "x".repeat(80);
        let cases = [
            serde_json::json!({"subject": format!("feat: {long}"), "body": "a\nb\nc"}),
            serde_json::json!({"subject": "fix: wrong type", "body": "a\nb\nc"}),
            serde_json::json!({"subject": "feat: ok", "body": "a\nb"}),
            serde_json::json!({"subject": "feat: ok", "body": "a\n\nb\nc"}),
            serde_json::json!({"subject": "feat: ok", "body": "a\nRefs: story:other\nc"}),
            serde_json::json!({"subject": "feat: ok", "body": "a\ntakt: take\nc"}),
            serde_json::json!({"subject": "feat: ok\ntakt: take", "body": "a\nb\nc"}),
            serde_json::json!({"subject": "feat: ok", "body": 3}),
            serde_json::json!("feat: ok"),
        ];
        for reply in &cases {
            let made = message(
                &plan,
                "implementation.edit",
                "did it",
                "Title",
                Some(reply),
                Some("story:x"),
            );
            assert!(made.fallback.is_some(), "{reply}");
            assert_eq!(
                made.text, "feat: Title\n\ndid it\n\nRefs: story:x\n",
                "{reply}"
            );
        }
        let none = message(&plan, "implementation.edit", "did it", "Title", None, None);
        assert_eq!(none.text, "feat: Title\n\ndid it\n");
        assert!(none.fallback.is_some());
    }

    #[test]
    fn the_action_style_is_todays_message() {
        let settings = Settings {
            style: Style::Action,
            ..Settings::default()
        };
        let plan = Plan::new(&settings, None, false);
        let made = message(
            &plan,
            "tests.write",
            "  wrote\n tests ",
            "Title",
            None,
            Some("story:x"),
        );
        assert_eq!(made.text, "tests.write: wrote tests\n\nRefs: story:x\n");
        assert_eq!(made.fallback, None);
        assert!(plan.ask().is_none());
    }

    #[test]
    fn a_summary_that_looks_like_a_trailer_is_not_one() {
        let plan = Plan::new(
            &Settings {
                trailer: String::new(),
                ..Settings::default()
            },
            None,
            false,
        );
        let made = message(
            &plan,
            "tests.write",
            "takt: take story:other",
            "Title",
            None,
            Some("story:x"),
        );
        assert_eq!(
            made.text,
            "test: Title\n\nreported: takt: take story:other\n"
        );
    }

    #[test]
    fn a_fallback_subject_is_cut_at_the_bound() {
        let plan = Plan::default();
        let title = "word ".repeat(30);
        let made = message(&plan, "tests.write", "", &title, None, None);
        let subject = made.text.lines().next().expect("a subject");
        assert!(subject.chars().count() <= SUBJECT_MAX, "{subject}");
        assert!(subject.starts_with("test: word word"));
    }

    #[test]
    fn the_settings_name_the_member_they_refuse() {
        assert!(Settings::default().check().is_ok());
        let trailer = Settings {
            trailer: "not a key".to_owned(),
            ..Settings::default()
        };
        assert!(
            trailer
                .check()
                .expect_err("refused")
                .contains("commit.trailer")
        );
        let kind = Settings {
            types: [("tests.write".to_owned(), "takt".to_owned())].into(),
            ..Settings::default()
        };
        let why = kind.check().expect_err("refused");
        assert!(why.contains("commit.types"), "{why}");
        assert!(
            Settings {
                trailer: String::new(),
                ..Settings::default()
            }
            .check()
            .is_ok()
        );
    }

    #[test]
    fn the_ask_names_each_offered_actions_type() {
        let ask = Plan::default().ask().expect("an ask");
        let text = ask.paragraph(&["tests.write".to_owned(), "implementation.edit".to_owned()]);
        assert!(text.contains("`test` for `tests.write`"), "{text}");
        assert!(text.contains("`feat` for `implementation.edit`"), "{text}");
        assert!(text.contains("3 or 4 lines"), "{text}");
    }
}
