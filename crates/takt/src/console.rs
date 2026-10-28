//! What a run prints for the person watching it, when its caller turns the console on
//! ([`crate::run::Run::console`]): a header before the first step that names takt's version and
//! the unit, the unit's title and a summary of its intent, and the frontier said as what the run
//! may do, `may:` once and then `may now: +x -y` only when the admissible set changes. Pure
//! functions over what the run already holds: the case's `intent.md` and each decision's
//! admissible actions. The record (`events.jsonl`) is not this module's; it is unchanged.

/// The version of takt this library is, as the header names it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The columns a summary line's text is wrapped at, its two-space indent left out, so that the
/// line is at most 100 columns.
pub const SUMMARY_WIDTH: usize = 98;

/// The most lines a summary takes; a longer paragraph ends its last line with `…`.
pub const SUMMARY_LINES: usize = 3;

/// How the console says the frontier: as changes only, or the full `frontier:` line at each
/// decision as well (`takt run --verbose`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Console {
    /// Whether every decision's full `frontier: <action> (<status>), …` line is printed too.
    pub verbose: bool,
}

/// The title of an intent: its first line that is not blank, a heading's `#` marks and the
/// spaces around it taken off. A story's `intent.md` starts with the story's title.
#[must_use]
pub fn title(intent: &str) -> String {
    intent
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches('#').trim().to_owned())
        .unwrap_or_default()
}

/// The summary of an intent, at most [`SUMMARY_LINES`] lines wrapped at [`SUMMARY_WIDTH`]: the
/// first paragraph under its `## Outcome` heading, or, when it has none, its first paragraph
/// after the title. A paragraph is a run of lines that are neither blank nor a heading; none is
/// no line.
#[must_use]
pub fn summary(intent: &str) -> Vec<String> {
    let lines: Vec<&str> = intent.lines().map(str::trim).collect();
    let after_title = lines
        .iter()
        .position(|line| !line.is_empty())
        .map_or(lines.len(), |at| at + 1);
    let outcome = lines.iter().position(|line| is_outcome_heading(line));
    let from = outcome.map_or(after_title, |at| at + 1);
    let paragraph: Vec<&str> = lines[from.min(lines.len())..]
        .iter()
        .skip_while(|line| line.is_empty() || (outcome.is_none() && is_heading(line)))
        .take_while(|line| !line.is_empty() && !is_heading(line))
        .copied()
        .collect();
    wrap(&paragraph.join(" "), SUMMARY_WIDTH, SUMMARY_LINES)
}

/// Whether `line` is a markdown heading.
fn is_heading(line: &str) -> bool {
    line.starts_with('#')
}

/// Whether `line` is the heading `## Outcome`, at any level from two, in any case.
fn is_outcome_heading(line: &str) -> bool {
    line.starts_with("##")
        && line
            .trim_start_matches('#')
            .trim()
            .eq_ignore_ascii_case("outcome")
}

/// `text`'s words in lines of at most `width` columns (a longer word alone on its line), at most
/// `most` lines, the last ending with `…` when words are left over.
#[must_use]
pub fn wrap(text: &str, width: usize, most: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut words = text.split_whitespace().peekable();
    while let Some(word) = words.next() {
        let fits =
            current.is_empty() || current.chars().count() + 1 + word.chars().count() <= width;
        if !fits {
            if lines.len() + 1 == most {
                current.push_str(" …");
                lines.push(current);
                return lines;
            }
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
        if words.peek().is_none() {
            lines.push(std::mem::take(&mut current));
        }
    }
    lines
}

/// The header's lines: `takt <version> · <story id>` and the intent's [`title`] under it, or, for
/// a run of an intent, `takt <version> · <title>` alone; then the intent's [`summary`], each line
/// under the first indented two spaces.
#[must_use]
pub fn header(version: &str, story: Option<&str>, intent: &str) -> Vec<String> {
    let title = title(intent);
    let mut lines = match story {
        Some(story) if title.is_empty() => vec![format!("takt {version} · {story}")],
        Some(story) => vec![format!("takt {version} · {story}"), format!("  {title}")],
        None => vec![format!("takt {version} · {title}")],
    };
    lines.extend(summary(intent).into_iter().map(|line| format!("  {line}")));
    lines
}

/// The header's harness line: `  harness <name>[ <version>] · confinement <name>`.
#[must_use]
pub fn harness_line(harness: &str, version: Option<&str>, confinement: &str) -> String {
    match version {
        Some(version) => format!("  harness {harness} {version} · confinement {confinement}"),
        None => format!("  harness {harness} · confinement {confinement}"),
    }
}

/// What the run may do at its first decision: `  may: <action> …`, `nothing` when no action is
/// admissible.
#[must_use]
pub fn may(admissible: &[String]) -> String {
    if admissible.is_empty() {
        "  may: nothing".to_owned()
    } else {
        format!("  may: {}", admissible.join(" "))
    }
}

/// What changed in the admissible set from `before` to `now`: `  may now: +<added> -<removed>`,
/// the added in `now`'s order, then the removed in `before`'s; none when the set is the same.
#[must_use]
pub fn may_now(before: &[String], now: &[String]) -> Option<String> {
    let added = now
        .iter()
        .filter(|a| !before.contains(a))
        .map(|a| format!("+{a}"));
    let removed = before
        .iter()
        .filter(|a| !now.contains(a))
        .map(|a| format!("-{a}"));
    let changes: Vec<String> = added.chain(removed).collect();
    (!changes.is_empty()).then(|| format!("  may now: {}", changes.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORY: &str = "Make the check pass\n\n# Story: ready-one\n\n## Outcome\n\nThe check in \
                         `check.txt` passes for whoever runs `./check.sh`.\n\n## Acceptance\n\n\
                         `./check.sh` exits 0.\n";

    fn owned(actions: &[&str]) -> Vec<String> {
        actions.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn the_title_is_the_first_line_without_its_heading_marks() {
        assert_eq!(title(STORY), "Make the check pass");
        assert_eq!(title("\n\n# A heading\n\nbody"), "A heading");
        assert_eq!(title(""), "");
    }

    #[test]
    fn the_summary_is_the_outcome_paragraph() {
        assert_eq!(
            summary(STORY),
            vec!["The check in `check.txt` passes for whoever runs `./check.sh`.".to_owned()]
        );
    }

    #[test]
    fn without_an_outcome_the_summary_is_the_first_paragraph_after_the_title() {
        let intent = "Fix it\n\n# Story: x\n\nThe first\nparagraph.\n\nThe second.\n";
        assert_eq!(summary(intent), vec!["The first paragraph.".to_owned()]);
        assert!(summary("deliver the story").is_empty());
    }

    #[test]
    fn a_long_summary_wraps_to_three_lines_and_ends_with_an_ellipsis() {
        let words = "word ".repeat(100);
        let intent = format!("Title\n\n## Outcome\n\n{words}\n");
        let lines = summary(&intent);
        assert_eq!(lines.len(), SUMMARY_LINES, "{lines:?}");
        assert!(
            lines[..2]
                .iter()
                .all(|l| l.chars().count() <= SUMMARY_WIDTH)
        );
        assert!(lines[2].ends_with(" …"), "{lines:?}");
        assert_eq!(wrap("a b c", 3, 3), vec!["a b".to_owned(), "c".to_owned()]);
        assert_eq!(wrap("a b c", 1, 2), vec!["a".to_owned(), "b …".to_owned()]);
        assert!(wrap("", 10, 3).is_empty());
    }

    #[test]
    fn the_header_names_the_version_the_story_its_title_and_summary() {
        assert_eq!(
            header("0.18.0", Some("story:ready-one"), STORY),
            vec![
                "takt 0.18.0 · story:ready-one".to_owned(),
                "  Make the check pass".to_owned(),
                "  The check in `check.txt` passes for whoever runs `./check.sh`.".to_owned(),
            ]
        );
        assert_eq!(
            header("0.18.0", None, "deliver the story"),
            vec!["takt 0.18.0 · deliver the story".to_owned()]
        );
        assert_eq!(
            header("0.18.0", None, "# Fix it\n\nThe first paragraph.\n"),
            vec![
                "takt 0.18.0 · Fix it".to_owned(),
                "  The first paragraph.".to_owned()
            ]
        );
    }

    #[test]
    fn the_harness_line_names_the_version_when_known() {
        assert_eq!(
            harness_line("claude-code", Some("2.1.309"), "none"),
            "  harness claude-code 2.1.309 · confinement none"
        );
        assert_eq!(
            harness_line("none", None, "substrate"),
            "  harness none · confinement substrate"
        );
    }

    #[test]
    fn may_lists_the_admissible_actions() {
        assert_eq!(
            may(&owned(&["tests.run", "tests.write"])),
            "  may: tests.run tests.write"
        );
        assert_eq!(may(&[]), "  may: nothing");
    }

    #[test]
    fn may_now_says_only_a_change() {
        let before = owned(&["tests.run", "tests.write"]);
        assert_eq!(may_now(&before, &before), None);
        assert_eq!(
            may_now(&before, &owned(&["implementation.edit", "tests.run"])),
            Some("  may now: +implementation.edit -tests.write".to_owned())
        );
        assert_eq!(
            may_now(&before, &[]),
            Some("  may now: -tests.run -tests.write".to_owned())
        );
    }
}
