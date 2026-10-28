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

/// The most failed checks a validator step's line names; more are counted, not named.
pub const FAILED_NAMED: usize = 5;

/// The line before a harness call: `step N: <harness> working (may: <offered> …)`, `nothing`
/// when none is offered, and `; may ask: <action> …` when actions that need an approval are
/// offered beside them.
#[must_use]
pub fn working(step: u64, harness: &str, offered: &[String], approvals: &[String]) -> String {
    let may = if offered.is_empty() {
        "nothing".to_owned()
    } else {
        offered.join(" ")
    };
    let ask = if approvals.is_empty() {
        String::new()
    } else {
        format!("; may ask: {}", approvals.join(" "))
    };
    format!("step {step}: {harness} working (may: {may}{ask})")
}

/// A duration as the console says it: `4.3s` under a minute, `7m24s` under an hour, `1h05m`
/// from there.
#[must_use]
pub fn duration(ms: u64) -> String {
    if ms < 60_000 {
        // Tenths of a second, rounded half up, in integers: no float is shown.
        let tenths = (ms + 50) / 100;
        return format!("{}.{}s", tenths / 10, tenths % 10);
    }
    let seconds = ms / 1000;
    if seconds < 3600 {
        format!("{}m{:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h{:02}m", seconds / 3600, seconds % 3600 / 60)
    }
}

/// Tokens as the console says them: `850 tok`, `1.2k tok`, `3.4M tok`.
#[must_use]
pub fn tokens(n: u64) -> String {
    if n < 1000 {
        format!("{n} tok")
    } else if n < 1_000_000 {
        let tenths = (n + 50) / 100;
        format!("{}.{}k tok", tenths / 10, tenths % 10)
    } else {
        let tenths = (n + 50_000) / 100_000;
        format!("{}.{}M tok", tenths / 10, tenths % 10)
    }
}

/// A cost in dollars to the cent: `$0.16`.
#[must_use]
pub fn cost(usd: f64) -> String {
    format!("${usd:.2}")
}

/// What a harness step's line appends after `step N: <action> (<harness>)`: ` · <duration> ·
/// <tokens> · <cost>`, each only when the call reported it; nothing when it reported none.
#[must_use]
pub fn harness_figures(
    duration_ms: Option<u64>,
    tokens_used: Option<u64>,
    cost_usd: Option<f64>,
) -> String {
    let parts = [
        duration_ms.map(duration),
        tokens_used.map(tokens),
        cost_usd.map(cost),
    ];
    parts
        .into_iter()
        .flatten()
        .map(|part| format!(" · {part}"))
        .collect()
}

/// What a validator step's line appends after `step N: <action> {}`: ` · pass`, ` · fail` and
/// the failed checks named (at most [`FAILED_NAMED`], then how many more), or nothing for a
/// result that is neither; then ` · <duration>`. Each name is escaped, so no name shapes a line
/// of its own ([`crate::trace::escape`]).
#[must_use]
pub fn validator_figures(result: Option<&str>, failed: &[String], duration_ms: u64) -> String {
    let said = match result {
        Some("pass") => " · pass".to_owned(),
        Some("fail") if failed.is_empty() => " · fail".to_owned(),
        Some("fail") => {
            let mut named: Vec<String> = failed
                .iter()
                .take(FAILED_NAMED)
                .map(|name| crate::trace::escape(name))
                .collect();
            if failed.len() > FAILED_NAMED {
                named.push(format!("{} more", failed.len() - FAILED_NAMED));
            }
            format!(" · fail: {}", named.join(", "))
        }
        _ => String::new(),
    };
    format!("{said} · {}", duration(duration_ms))
}

/// What the `stopped:` line names between its tokens and its `, case <dir>`: `, wall
/// <duration>`, `, cost <cost>` when any step reported one, and `, commits N` with ` (P
/// path[s])` when it made any.
#[must_use]
pub fn stop_figures(wall_ms: u64, cost_usd: Option<f64>, commits: u64, paths: usize) -> String {
    let mut line = format!(", wall {}", duration(wall_ms));
    if let Some(usd) = cost_usd {
        line.push_str(&format!(", cost {}", cost(usd)));
    }
    line.push_str(&format!(", commits {commits}"));
    if commits > 0 {
        let plural = if paths == 1 { "" } else { "s" };
        line.push_str(&format!(" ({paths} path{plural})"));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_working_line_names_the_harness_and_what_it_may_do() {
        assert_eq!(
            working(
                2,
                "claude-code",
                &owned(&["tests.write", "change.handover"]),
                &[]
            ),
            "step 2: claude-code working (may: tests.write change.handover)"
        );
        assert_eq!(
            working(3, "none", &[], &owned(&["repository.merge"])),
            "step 3: none working (may: nothing; may ask: repository.merge)"
        );
    }

    #[test]
    fn durations_tokens_and_cost_are_short() {
        assert_eq!(duration(0), "0.0s");
        assert_eq!(duration(4321), "4.3s");
        assert_eq!(duration(59_949), "59.9s");
        assert_eq!(duration(60_000), "1m00s");
        assert_eq!(duration(444_000), "7m24s");
        assert_eq!(duration(3_900_000), "1h05m");
        assert_eq!(tokens(850), "850 tok");
        assert_eq!(tokens(1234), "1.2k tok");
        assert_eq!(tokens(65_700), "65.7k tok");
        assert_eq!(tokens(3_400_000), "3.4M tok");
        assert_eq!(cost(0.0123), "$0.01");
        assert_eq!(cost(0.155), "$0.15");
    }

    #[test]
    fn a_harness_step_appends_only_what_its_call_reported() {
        assert_eq!(
            harness_figures(Some(26_100), Some(65_700), Some(0.16)),
            " · 26.1s · 65.7k tok · $0.16"
        );
        assert_eq!(harness_figures(None, Some(12), None), " · 12 tok");
        assert_eq!(harness_figures(None, None, None), "");
    }

    #[test]
    fn a_validator_step_says_its_result_and_names_what_failed() {
        assert_eq!(
            validator_figures(Some("pass"), &[], 12_600),
            " · pass · 12.6s"
        );
        assert_eq!(
            validator_figures(Some("fail"), &owned(&["specification", "format"]), 900),
            " · fail: specification, format · 0.9s"
        );
        let many: Vec<String> = (0..7).map(|n| format!("t{n}")).collect();
        assert_eq!(
            validator_figures(Some("fail"), &many, 100),
            " · fail: t0, t1, t2, t3, t4, 2 more · 0.1s"
        );
        assert_eq!(
            validator_figures(Some("fail"), &owned(&["a\nstopped: x"]), 100),
            " · fail: a\\u{a}stopped: x · 0.1s"
        );
        assert_eq!(validator_figures(None, &[], 100), " · 0.1s");
    }

    #[test]
    fn the_stop_figures_name_wall_time_cost_and_commits() {
        assert_eq!(
            stop_figures(444_000, Some(0.89), 2, 2),
            ", wall 7m24s, cost $0.89, commits 2 (2 paths)"
        );
        assert_eq!(
            stop_figures(1200, None, 1, 1),
            ", wall 1.2s, commits 1 (1 path)"
        );
        assert_eq!(stop_figures(1200, None, 0, 0), ", wall 1.2s, commits 0");
    }

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
