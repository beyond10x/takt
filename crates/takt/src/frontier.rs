//! The `Frontier` seam: the planning store behind a trait, the ready rule as a pure function
//! over the artifacts the store lists, and one adapter, [`Aep`], which reads and writes the
//! store only through the `aep` command line. No model takes part in the choice of the unit.
//!
//! A story is ready when it is `proposed`, nothing blocks it and every artifact it depends on
//! is done (`implemented` or `archived`); among the ready ones the first in id order is the
//! unit. Taking a story moves it `proposed` → `active`, so a story with an open case is never
//! taken twice. A store may refuse the move; then the story is not taken, the refusal is printed
//! verbatim and the next ready story is tried.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, Result, anyhow};
use serde::Deserialize;

/// The status a story is ready at: refined and agreed enough to pick up.
pub const READY: &str = "proposed";
/// The status a taken story is moved to: being worked on now.
pub const TAKEN: &str = "active";
/// The statuses an artifact a story depends on is done at.
pub const DONE: [&str; 2] = ["implemented", "archived"];

/// A story taken from the planning store: the unit of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    /// The artifact id, `story:<name>`.
    pub id: String,
    /// The title.
    pub title: String,
    /// The markdown body, without the frontmatter.
    pub body: String,
}

impl Story {
    /// The case's `intent.md`: the title, a blank line, the body.
    #[must_use]
    pub fn intent(&self) -> String {
        format!("{}\n\n{}", self.title, self.body)
    }

    /// A case id from the story id and `instant`: `story-<name>-<instant>`, the instant as
    /// [`crate::now`] writes it with its separators dropped.
    #[must_use]
    pub fn case_id(&self, instant: &str) -> String {
        format!(
            "{}-{}",
            self.id.replace(':', "-"),
            instant.replace([':', '-'], "")
        )
    }
}

/// What a run works on: an intent given on the command line, or a story the frontier pulled
/// from the planning store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unit {
    /// An intent, as given.
    Intent(String),
    /// A story the frontier took.
    Story(Story),
}

/// The planning store behind a trait: asked for the next ready story, it hands one over and
/// records in the store that the story is being worked.
pub trait Frontier {
    /// The adapter's name.
    fn name(&self) -> &str;
    /// The next ready story of the store in `repo`, taken; `None` when nothing is ready. A story
    /// the store refuses to hand over is reported on `out`, verbatim, and skipped.
    fn next(&self, repo: &Path, out: &mut dyn Write) -> Result<Option<Story>>;
}

/// An artifact as `aep plan artifact list --format json` lists it: the fields the rule reads.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Listed {
    /// The artifact id, `<kind>:<name>`.
    pub id: String,
    /// The kind, `story` for a unit.
    pub kind: String,
    /// The status on the kind's ladder.
    pub status: String,
    /// The title.
    #[serde(default)]
    pub title: String,
    /// The edges leaving the artifact.
    #[serde(default)]
    pub relations: Vec<Relation>,
    /// What the store says blocks the artifact.
    #[serde(default)]
    pub blocked_by: Vec<Blocker>,
}

/// An edge from the listed artifact to `target`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Relation {
    /// The relation, `depends_on` among others.
    pub relation: String,
    /// The artifact the edge points at.
    pub target: String,
}

/// Something the store says blocks the listed artifact.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Blocker {
    /// The blocking artifact's id.
    pub blocker: String,
}

/// The ready rule: `story` is a story at [`READY`], nothing blocks it, and every artifact it
/// `depends_on` is listed at one of [`DONE`]. A dependency the store does not list is not done.
#[must_use]
pub fn is_ready(story: &Listed, all: &[Listed]) -> bool {
    story.kind == "story"
        && story.status == READY
        && story.blocked_by.is_empty()
        && story
            .relations
            .iter()
            .filter(|r| r.relation == "depends_on")
            .all(|r| {
                all.iter()
                    .any(|a| a.id == r.target && DONE.contains(&a.status.as_str()))
            })
}

/// The ready stories among `all`, in id order; the first is the unit.
#[must_use]
pub fn ready(all: &[Listed]) -> Vec<&Listed> {
    let mut ready: Vec<&Listed> = all.iter().filter(|a| is_ready(a, all)).collect();
    ready.sort_by(|a, b| a.id.cmp(&b.id));
    ready
}

/// What the store answered a take with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Take {
    /// The story was moved to [`TAKEN`].
    Taken,
    /// The store refused the move; what it said, verbatim.
    Refused(String),
}

/// The adapter over the `aep` command line, run in the repository: `list` and `show` read the
/// store, `move` is the one write, and the lifecycle the store enforces is the store's own.
#[derive(Debug, Clone)]
pub struct Aep {
    /// The command, `aep` on the path by default.
    pub command: String,
}

impl Default for Aep {
    fn default() -> Self {
        Self {
            command: "aep".to_owned(),
        }
    }
}

impl Aep {
    /// Runs `aep plan artifact <args>` in `repo`: what it printed on success, or what it
    /// printed on a non-zero exit, trimmed, as the inner error.
    fn plan(&self, repo: &Path, args: &[&str]) -> Result<std::result::Result<String, String>> {
        let output = Command::new(&self.command)
            .args(["plan", "artifact"])
            .args(args)
            .current_dir(repo)
            .output()
            .with_context(|| format!("run {} plan artifact {}", self.command, args.join(" ")))?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if output.status.success() {
            return Ok(Ok(stdout));
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(Err(format!("{stdout}{stderr}").trim().to_owned()))
    }

    /// Refuses a repository without a planning store: `aep` would otherwise walk up from it
    /// and find a store above it.
    fn store(repo: &Path) -> Result<()> {
        if repo.join(".engineering/project.yaml").is_file() {
            Ok(())
        } else {
            Err(anyhow!(
                "the repository {} has no planning store: no .engineering/project.yaml",
                repo.display()
            ))
        }
    }

    /// Every artifact the store lists.
    pub fn list(&self, repo: &Path) -> Result<Vec<Listed>> {
        Self::store(repo)?;
        let json = self
            .plan(repo, &["list", "--format", "json"])?
            .map_err(|refusal| anyhow!("aep plan artifact list: {refusal}"))?;
        serde_json::from_str(&json).context("read the artifact list")
    }

    /// The story `id`, with its body.
    pub fn show(&self, repo: &Path, id: &str) -> Result<Story> {
        #[derive(Deserialize)]
        struct Shown {
            id: String,
            title: String,
            #[serde(default)]
            body: String,
        }
        let json = self
            .plan(repo, &["show", id, "--format", "json"])?
            .map_err(|refusal| anyhow!("aep plan artifact show {id}: {refusal}"))?;
        let shown: Shown = serde_json::from_str(&json).with_context(|| format!("read {id}"))?;
        Ok(Story {
            id: shown.id,
            title: shown.title,
            body: shown.body,
        })
    }

    /// Moves `id` to [`TAKEN`], the store judging the move; its refusal, verbatim, when it
    /// would not.
    pub fn take(&self, repo: &Path, id: &str) -> Result<Take> {
        Ok(
            match self.plan(
                repo,
                &["move", id, "--to", TAKEN, "--executor", "agent:takt"],
            )? {
                Ok(_) => Take::Taken,
                Err(refusal) => Take::Refused(refusal),
            },
        )
    }
}

impl Frontier for Aep {
    fn name(&self) -> &str {
        "aep"
    }

    fn next(&self, repo: &Path, out: &mut dyn Write) -> Result<Option<Story>> {
        let listed = self.list(repo)?;
        for candidate in ready(&listed) {
            let story = self.show(repo, &candidate.id)?;
            match self.take(repo, &candidate.id)? {
                Take::Taken => return Ok(Some(story)),
                Take::Refused(refusal) => {
                    writeln!(out, "frontier: {} not taken: {refusal}", candidate.id)?;
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(id: &str, status: &str) -> Listed {
        Listed {
            id: id.to_owned(),
            kind: id.split(':').next().expect("a kind").to_owned(),
            status: status.to_owned(),
            title: String::new(),
            relations: vec![],
            blocked_by: vec![],
        }
    }

    fn depends(mut artifact: Listed, target: &str) -> Listed {
        artifact.relations.push(Relation {
            relation: "depends_on".to_owned(),
            target: target.to_owned(),
        });
        artifact
    }

    fn blocked(mut artifact: Listed, by: &str) -> Listed {
        artifact.blocked_by.push(Blocker {
            blocker: by.to_owned(),
        });
        artifact
    }

    fn ids(all: &[Listed]) -> Vec<&str> {
        ready(all).into_iter().map(|a| a.id.as_str()).collect()
    }

    #[test]
    fn a_proposed_story_nothing_blocks_or_precedes_is_ready() {
        let all = [listed("story:a", "proposed")];
        assert_eq!(ids(&all), ["story:a"]);
    }

    #[test]
    fn only_proposed_is_ready() {
        let all = [
            listed("story:a", "draft"),
            listed("story:b", "active"),
            listed("story:c", "implemented"),
            listed("story:d", "archived"),
            listed("story:e", "rejected"),
        ];
        assert!(ids(&all).is_empty());
    }

    #[test]
    fn a_blocked_story_is_not_ready() {
        let all = [
            blocked(listed("story:a", "proposed"), "decision-blocker:q"),
            listed("decision-blocker:q", "open"),
        ];
        assert!(ids(&all).is_empty());
    }

    #[test]
    fn a_dependency_is_met_only_once_it_is_done() {
        for (status, met) in [
            ("draft", false),
            ("proposed", false),
            ("active", false),
            ("implemented", true),
            ("archived", true),
        ] {
            let all = [
                depends(listed("story:a", "proposed"), "story:dep"),
                listed("story:dep", status),
            ];
            assert_eq!(
                ids(&all).contains(&"story:a"),
                met,
                "a dependency at {status}"
            );
        }
        let unlisted = [depends(listed("story:a", "proposed"), "story:gone")];
        assert!(
            ids(&unlisted).is_empty(),
            "an unlisted dependency is not done"
        );
        let two = [
            depends(
                depends(listed("story:a", "proposed"), "story:done"),
                "story:open",
            ),
            listed("story:done", "implemented"),
            listed("story:open", "proposed"),
        ];
        assert!(
            !ids(&two).contains(&"story:a"),
            "every dependency has to be done"
        );
    }

    #[test]
    fn the_first_in_id_order_is_the_unit() {
        let all = [
            listed("story:c", "proposed"),
            listed("story:a", "proposed"),
            blocked(listed("story:0", "proposed"), "decision-blocker:q"),
            listed("story:b", "proposed"),
        ];
        assert_eq!(ids(&all), ["story:a", "story:b", "story:c"]);
    }

    #[test]
    fn only_a_story_is_a_unit() {
        let all = [listed("epic:e", "proposed"), listed("task:t", "proposed")];
        assert!(ids(&all).is_empty());
    }

    #[test]
    fn the_intent_is_the_title_then_the_body() {
        let story = Story {
            id: "story:a".to_owned(),
            title: "Make it pass".to_owned(),
            body: "# Story: a\n\n## Outcome\n\nIt passes.\n".to_owned(),
        };
        assert_eq!(
            story.intent(),
            "Make it pass\n\n# Story: a\n\n## Outcome\n\nIt passes.\n"
        );
    }

    #[test]
    fn the_case_id_is_the_story_and_the_instant() {
        let story = Story {
            id: "story:ready-one".to_owned(),
            title: String::new(),
            body: String::new(),
        };
        assert_eq!(
            story.case_id("2026-10-18T12:00:00Z"),
            "story-ready-one-20261018T120000Z"
        );
    }

    #[test]
    fn the_listing_reads_as_aep_prints_it() {
        let json = r#"[{"id":"story:a","kind":"story","status":"proposed","title":"A",
            "path":"story/a.md","relations":[{"relation":"depends_on","target":"story:b"}],
            "refs":[],"blocked_by":[{"blocker":"decision-blocker:q","type":"decision"}]}]"#;
        let listed: Vec<Listed> = serde_json::from_str(json).expect("reads");
        assert_eq!(listed[0].relations[0].target, "story:b");
        assert_eq!(listed[0].blocked_by[0].blocker, "decision-blocker:q");
    }
}
