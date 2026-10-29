//! story:local-forge-merges-into-a-target-branch: `--forge local` makes a bare repository on the
//! machine, the hub the tree's `origin` names, the forge. Each case drives the `Forge` seam of
//! `takt::forge::local::Local` directly over real git repositories under this crate's build
//! directory: two clones of a hub delivering into its `integration` in order, the second on the
//! first's merge; a conflict refused naming the path, the hub where it was; a base moved between
//! the read of the target's tip and the merge refused by the compare-and-swap; a stale head and
//! a merged request merging nothing; a record that says merged believed only of a merge the hub
//! holds; the checks run as the test command is, in a repository of their own that names no
//! remote, a pass kept per head and a fail or a timeout run again; and an `origin` that is no
//! hub, or a tree that shares the hub's git directory, refused before anything is written.
//! Nothing here reaches a host.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::forge::local::{Local, LocalRefused, refused_for_a_moved_base};
use takt::forge::{Checks, Forge, Limits, MergeMessage, PullRequest, PullRequestState};
use takt::git::CommitIdentity;

/// Runs `git <args>` in `dir` with no configuration of the user's or the system's, no signing
/// and no hook; its trimmed standard output.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("local-forge-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }

    /// The hub, `<scratch>/hub.git`: a bare repository whose `integration` holds one commit, a
    /// `README.md` of three lines.
    fn hub(&self) -> PathBuf {
        let seed = self.0.join("seed");
        fs::create_dir_all(&seed).expect("a directory");
        git(&seed, &["init", "-q", "-b", "integration"]);
        fs::write(seed.join("README.md"), "one\ntwo\nthree\n").expect("a file");
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "-q", "-m", "start"]);
        git(&self.0, &["clone", "-q", "--bare", "seed", "hub.git"]);
        fs::remove_dir_all(&seed).expect("the seed is removed");
        self.0.join("hub.git")
    }

    /// A clone of the hub at `<scratch>/<name>`, on `integration`.
    fn clone(&self, name: &str) -> PathBuf {
        git(&self.0, &["clone", "-q", "--no-local", "hub.git", name]);
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity() -> CommitIdentity {
    CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity")
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// The local forge over `tree`, its checks `checks`, its state under the scratch directory.
fn local(scratch: &Scratch, tree: &Path, checks: &[&str]) -> Local {
    Local::for_tree(tree, argv(checks), scratch.0.join("state"), identity())
        .expect("the hub is a local bare repository outside the tree")
}

/// Commits `text` as `file` in `tree` on a new branch `branch`; the commit.
fn change(tree: &Path, branch: &str, file: &str, text: &str) -> String {
    git(tree, &["checkout", "-q", "-b", branch]);
    fs::write(tree.join(file), text).expect("a file");
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", &format!("change {file}")]);
    git(tree, &["rev-parse", "HEAD"])
}

/// Pushes `branch` of `tree` to the hub and opens its request into `integration`; its number.
fn deliver(forge: &Local, tree: &Path, artifacts: &Path, branch: &str) -> u64 {
    forge.push(tree, branch).expect("pushed to the hub");
    forge
        .open_pull_request(
            tree,
            artifacts,
            &PullRequest {
                branch: branch.to_owned(),
                base: "integration".to_owned(),
                title: format!("deliver {branch}"),
                body: "the change".to_owned(),
            },
        )
        .expect("a request is opened")
}

fn message(title: &str) -> MergeMessage {
    MergeMessage {
        title: title.to_owned(),
        body: "merged by takt".to_owned(),
    }
}

/// The hub's `integration`.
fn integration(hub: &Path) -> String {
    git(hub, &["rev-parse", "refs/heads/integration"])
}

/// Acceptance 2: two clones each deliver a change; both merge into `integration` in order, the
/// second on the first's merge, each a merge commit under the run's identity carrying takt's
/// message, and each request reads `merged` from then on.
#[test]
fn two_clones_merge_in_order_into_integration() {
    let scratch = Scratch::new("in-order");
    let hub = scratch.hub();
    let start = integration(&hub);
    let a = scratch.clone("a");
    let b = scratch.clone("b");
    let a_head = change(&a, "takt/a", "a.txt", "a\n");
    let b_head = change(&b, "takt/b", "b.txt", "b\n");
    let forge_a = local(&scratch, &a, &["true"]);
    let forge_b = local(&scratch, &b, &["true"]);
    let artifacts = scratch.0.join("artifacts");

    let first = deliver(&forge_a, &a, &artifacts, "takt/a");
    let second = deliver(&forge_b, &b, &artifacts, "takt/b");
    assert_eq!((first, second), (1, 2));
    assert!(hub.join("takt/requests/1.json").is_file());

    let view = forge_a.pull_request(&a, first).expect("read");
    assert_eq!(view.state, PullRequestState::Open);
    assert_eq!(view.head.branch, "takt/a");
    assert_eq!(view.head.sha, a_head);
    assert_eq!(view.base, "integration");
    assert_eq!(forge_a.checks(&a, first).expect("checks"), Checks::Pass);

    assert_eq!(forge_a.target_tip(&a, "integration").expect("a tip"), start);
    git(&a, &["cat-file", "-e", &format!("{start}^{{commit}}")]);
    forge_a
        .merge_with(&a, &artifacts, first, &a_head, &message("merge a"))
        .expect("merged");
    let merged_a = integration(&hub);
    assert_eq!(
        git(&hub, &["rev-list", "--parents", "-n", "1", &merged_a]),
        format!("{merged_a} {start} {a_head}")
    );
    assert_eq!(
        git(
            &hub,
            &["log", "-1", "--format=%an <%ae>|%cn <%ce>|%s|%b", &merged_a]
        ),
        "Test Bot <bot@example.invalid>|Test Bot <bot@example.invalid>|merge a|merged by takt"
    );
    assert_eq!(
        forge_a.pull_request(&a, first).expect("read").state,
        PullRequestState::Merged
    );

    assert_eq!(
        forge_b.target_tip(&b, "integration").expect("a tip"),
        merged_a
    );
    git(&b, &["cat-file", "-e", &format!("{merged_a}^{{commit}}")]);
    forge_b
        .merge_with(&b, &artifacts, second, &b_head, &message("merge b"))
        .expect("merged");
    let merged_b = integration(&hub);
    assert_eq!(
        git(&hub, &["rev-list", "--parents", "-n", "1", &merged_b]),
        format!("{merged_b} {merged_a} {b_head}")
    );
    assert_eq!(
        git(&hub, &["show", &format!("{merged_b}:a.txt")]),
        "a",
        "the second merge holds the first's change"
    );
    assert_eq!(
        forge_b.pull_request(&b, second).expect("read").state,
        PullRequestState::Merged
    );
}

/// Acceptance 3: two changes to the same lines; the second merge is refused naming the path, the
/// hub's `integration` stays at the first merge and the second request stays open.
#[test]
fn a_conflict_is_refused_and_the_hub_stays() {
    let scratch = Scratch::new("conflict");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    let b = scratch.clone("b");
    let a_head = change(&a, "takt/a", "README.md", "one\nA\nthree\n");
    let b_head = change(&b, "takt/b", "README.md", "one\nB\nthree\n");
    let forge_a = local(&scratch, &a, &["true"]);
    let forge_b = local(&scratch, &b, &["true"]);
    let artifacts = scratch.0.join("artifacts");
    let first = deliver(&forge_a, &a, &artifacts, "takt/a");
    let second = deliver(&forge_b, &b, &artifacts, "takt/b");
    forge_a.target_tip(&a, "integration").expect("a tip");
    forge_a
        .merge_with(&a, &artifacts, first, &a_head, &message("merge a"))
        .expect("merged");
    let merged_a = integration(&hub);

    forge_b.target_tip(&b, "integration").expect("a tip");
    let error = forge_b
        .merge_with(&b, &artifacts, second, &b_head, &message("merge b"))
        .expect_err("a conflict is refused");
    assert!(
        matches!(
            error.downcast_ref::<LocalRefused>(),
            Some(LocalRefused::Conflict { paths, .. }) if paths == &["README.md".to_owned()]
        ),
        "{error:#}"
    );
    assert!(format!("{error:#}").contains("README.md"), "{error:#}");
    assert!(
        !refused_for_a_moved_base(&error),
        "a conflict stays a refusal"
    );
    assert_eq!(integration(&hub), merged_a);
    assert_eq!(
        forge_b.pull_request(&b, second).expect("read").state,
        PullRequestState::Open
    );
}

/// Acceptance 4: the base moved between the read of the target's tip and the merge is refused by
/// the compare-and-swap, the base where the other writer left it; a later read of the tip merges
/// the request on the moved base.
#[test]
fn a_moved_base_is_refused_by_the_compare_and_swap() {
    let scratch = Scratch::new("moved");
    let hub = scratch.hub();
    let start = integration(&hub);
    let a = scratch.clone("a");
    let c = scratch.clone("c");
    let a_head = change(&a, "takt/a", "a.txt", "a\n");
    let forge = local(&scratch, &a, &["true"]);
    let artifacts = scratch.0.join("artifacts");
    let number = deliver(&forge, &a, &artifacts, "takt/a");
    assert_eq!(forge.target_tip(&a, "integration").expect("a tip"), start);

    fs::write(c.join("c.txt"), "c\n").expect("a file");
    git(&c, &["add", "-A"]);
    git(&c, &["commit", "-q", "-m", "another writer"]);
    git(&c, &["push", "-q", "origin", "HEAD:refs/heads/integration"]);
    let moved = integration(&hub);
    assert_ne!(moved, start);

    let error = forge
        .merge_with(&a, &artifacts, number, &a_head, &message("merge a"))
        .expect_err("a moved base is refused");
    assert!(
        matches!(
            error.downcast_ref::<LocalRefused>(),
            Some(LocalRefused::BaseMoved { expected, found, .. })
                if expected == &start && found.as_deref() == Some(moved.as_str())
        ),
        "{error:#}"
    );
    assert!(
        refused_for_a_moved_base(&error.context("merge pull request #1 through the forge local")),
        "a moved base is nothing merged, judged again on the base where it is"
    );
    assert_eq!(integration(&hub), moved, "the other writer's base stands");
    assert_eq!(
        forge.pull_request(&a, number).expect("read").state,
        PullRequestState::Open
    );

    assert_eq!(forge.target_tip(&a, "integration").expect("a tip"), moved);
    forge
        .merge_with(&a, &artifacts, number, &a_head, &message("merge a"))
        .expect("merged on the moved base");
    let merged = integration(&hub);
    assert_eq!(
        git(&hub, &["rev-list", "--parents", "-n", "1", &merged]),
        format!("{merged} {moved} {a_head}")
    );
}

/// The checks run the run file's argument list in a fresh clone of the hub at the head, once per
/// head: a later read answers the recorded verdict without running them again. Any exit but 0
/// fails.
#[test]
fn checks_run_once_per_head_in_a_clone_at_the_head() {
    let scratch = Scratch::new("checks");
    let _hub = scratch.hub();
    let a = scratch.clone("a");
    change(&a, "takt/a", "a.txt", "a\n");
    let ran = scratch.0.join("ran");
    let line = format!("test -f a.txt && echo ran >> '{}'", ran.display());
    let forge = local(&scratch, &a, &["sh", "-c", &line]);
    let artifacts = scratch.0.join("artifacts");
    let number = deliver(&forge, &a, &artifacts, "takt/a");
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Pass);
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Pass);
    assert_eq!(
        fs::read_to_string(&ran).expect("the checks ran"),
        "ran\n",
        "the checks ran once for the head"
    );
    assert!(
        !scratch.0.join("state").join("checks").exists()
            || fs::read_dir(scratch.0.join("state").join("checks"))
                .expect("a directory")
                .all(|entry| !entry.expect("an entry").path().is_dir()),
        "the clone the checks ran in is removed"
    );

    git(&a, &["commit", "-q", "--allow-empty", "-m", "again"]);
    forge.push(&a, "takt/a").expect("pushed");
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Pass);
    assert_eq!(
        fs::read_to_string(&ran).expect("the checks ran"),
        "ran\nran\n",
        "a new head runs the checks again"
    );

    let failing = local(&scratch, &a, &["false"]);
    git(&a, &["checkout", "-q", "-b", "takt/f"]);
    git(&a, &["commit", "-q", "--allow-empty", "-m", "f"]);
    let number = deliver(&failing, &a, &artifacts, "takt/f");
    assert_eq!(failing.checks(&a, number).expect("checks"), Checks::Fail);
}

/// While another live process runs the checks on the head, they read `pending` and are not run
/// again; once it is gone, the next read runs them.
#[test]
fn checks_another_process_runs_are_pending() {
    let scratch = Scratch::new("pending");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    let head = change(&a, "takt/a", "a.txt", "a\n");
    let ran = scratch.0.join("ran");
    let line = format!("echo ran >> '{}'", ran.display());
    let argv = ["sh", "-c", &line];
    let forge = local(&scratch, &a, &argv);
    let number = deliver(&forge, &a, &scratch.0.join("artifacts"), "takt/a");
    // A verdict is kept under the head and the digest of the argument list as a JSON array.
    let key = {
        use sha2::Digest as _;
        let digest = sha2::Sha256::digest(serde_json::to_vec(&argv).expect("json"));
        format!("{head} sha256:{digest:x}")
    };
    let mut other = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("another process");
    let record = hub.join(format!("takt/requests/{number}.json"));
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).expect("the record")).expect("json");
    document["checks"][&key] = serde_json::json!({"result": "pending", "pid": other.id()});
    fs::write(&record, serde_json::to_vec(&document).expect("json")).expect("written");
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Pending);
    assert!(!ran.exists(), "the checks were not run again");
    other.kill().expect("killed");
    other.wait().expect("reaped");
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Pass);
    assert_eq!(fs::read_to_string(&ran).expect("ran"), "ran\n");
}

/// A request's number is taken with an exclusive create: a number some other run holds, by an
/// empty file or another run's record, is passed over, and the record holding it is not written.
#[test]
fn a_number_held_is_passed_over() {
    let scratch = Scratch::new("numbers");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    change(&a, "takt/a", "a.txt", "a\n");
    fs::create_dir_all(hub.join("takt/requests")).expect("a directory");
    fs::write(hub.join("takt/requests/1.json"), "").expect("a number held");
    let forge = local(&scratch, &a, &["true"]);
    let artifacts = scratch.0.join("artifacts");
    let number = deliver(&forge, &a, &artifacts, "takt/a");
    assert_eq!(number, 2);
    let held = fs::read(hub.join("takt/requests/2.json")).expect("the record");

    let b = scratch.clone("b");
    change(&b, "takt/b", "b.txt", "b\n");
    let other = local(&scratch, &b, &["true"]);
    assert_eq!(deliver(&other, &b, &artifacts, "takt/b"), 3);
    assert_eq!(
        fs::read(hub.join("takt/requests/2.json")).expect("the record"),
        held,
        "another run's record is not written over"
    );
    assert_eq!(
        fs::read(hub.join("takt/requests/1.json")).expect("the file"),
        b""
    );
}

/// The merge names the head: a head the branch has moved past is refused, a request already
/// merged is not merged again, and either leaves the base where it was. A second merge with no
/// read of the target's tip between goes onto the base where the first left it.
#[test]
fn a_stale_head_and_a_merged_request_merge_nothing() {
    let scratch = Scratch::new("stale");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    let stale = change(&a, "takt/a", "a.txt", "a\n");
    let forge = local(&scratch, &a, &["true"]);
    let artifacts = scratch.0.join("artifacts");
    let first = deliver(&forge, &a, &artifacts, "takt/a");
    fs::write(a.join("a.txt"), "a again\n").expect("a file");
    git(&a, &["commit", "-q", "-am", "again"]);
    let head = git(&a, &["rev-parse", "HEAD"]);
    forge.push(&a, "takt/a").expect("pushed");
    let start = integration(&hub);
    forge.target_tip(&a, "integration").expect("a tip");

    let error = forge
        .merge_with(&a, &artifacts, first, &stale, &message("merge a"))
        .expect_err("a stale head is refused");
    assert!(format!("{error:#}").contains(&head), "{error:#}");
    assert_eq!(integration(&hub), start, "nothing merged");

    forge
        .merge_with(&a, &artifacts, first, &head, &message("merge a"))
        .expect("merged");
    let merged = integration(&hub);
    let error = forge
        .merge_with(&a, &artifacts, first, &head, &message("merge a"))
        .expect_err("a merged request is not merged again");
    assert!(format!("{error:#}").contains("not open"), "{error:#}");
    assert_eq!(integration(&hub), merged, "the base stays at the merge");

    git(&a, &["checkout", "-q", "-b", "takt/b", &start]);
    fs::write(a.join("b.txt"), "b\n").expect("a file");
    git(&a, &["add", "-A"]);
    git(&a, &["commit", "-q", "-m", "b"]);
    let b_head = git(&a, &["rev-parse", "HEAD"]);
    let second = deliver(&forge, &a, &artifacts, "takt/b");
    forge
        .merge_with(&a, &artifacts, second, &b_head, &message("merge b"))
        .expect("merged onto the base where the first merge left it");
    let after = integration(&hub);
    assert_eq!(
        git(&hub, &["rev-list", "--parents", "-n", "1", &after]),
        format!("{after} {merged} {b_head}")
    );
}

/// A record that says merged is believed only when it names the head and a commit on the hub's
/// base that holds it, the head or a commit with it in its history: one naming no merge or no
/// head, a merge of the head off the base, a commit on the base that does not hold the head, or
/// a merge the branch has moved past is an error, never `merged`. Once the branch is gone, the
/// head is the record's.
#[test]
fn a_merged_record_is_believed_only_of_a_merge_the_hub_holds() {
    let scratch = Scratch::new("believed");
    let hub = scratch.hub();
    let start = integration(&hub);
    let a = scratch.clone("a");
    let head = change(&a, "takt/a", "a.txt", "a\n");
    let forge = local(&scratch, &a, &["true"]);
    let artifacts = scratch.0.join("artifacts");
    let number = deliver(&forge, &a, &artifacts, "takt/a");
    git(&a, &["checkout", "-q", "-b", "off", &start]);
    fs::write(a.join("off.txt"), "off\n").expect("a file");
    git(&a, &["add", "-A"]);
    git(&a, &["commit", "-q", "-m", "off"]);
    git(&a, &["merge", "-q", "--no-ff", "--no-edit", "takt/a"]);
    let off = git(&a, &["rev-parse", "HEAD"]);
    git(&a, &["push", "-q", "origin", "off"]);
    git(&a, &["checkout", "-q", "takt/a"]);
    let record = hub.join(format!("takt/requests/{number}.json"));
    let open = fs::read(&record).expect("the record");
    let write = |merge: Option<&str>, merged: Option<&str>| {
        let mut document: serde_json::Value = serde_json::from_slice(&open).expect("json");
        document["state"] = serde_json::json!("merged");
        if let Some(merge) = merge {
            document["merge"] = serde_json::json!(merge);
        }
        if let Some(merged) = merged {
            document["head"] = serde_json::json!(merged);
        }
        fs::write(&record, serde_json::to_vec(&document).expect("json")).expect("written");
    };
    for (merge, merged, why) in [
        (None, Some(head.as_str()), "names no merge commit"),
        (Some(start.as_str()), None, "names no head"),
        (
            Some(off.as_str()),
            Some(head.as_str()),
            "is not on integration",
        ),
        (
            Some(start.as_str()),
            Some(head.as_str()),
            "does not hold the head",
        ),
    ] {
        write(merge, merged);
        let error = forge
            .pull_request(&a, number)
            .expect_err("a merge the hub does not hold");
        let text = format!("{error:#}");
        assert!(
            text.contains("not believed") && text.contains(why),
            "{merge:?} {merged:?}: {text}"
        );
    }
    fs::write(&record, &open).expect("restored");

    forge.target_tip(&a, "integration").expect("a tip");
    forge
        .merge_with(&a, &artifacts, number, &head, &message("merge a"))
        .expect("merged");
    let view = forge.pull_request(&a, number).expect("read");
    assert_eq!(
        (view.state, view.head.sha),
        (PullRequestState::Merged, head.clone())
    );
    fs::write(a.join("a.txt"), "after\n").expect("a file");
    git(&a, &["commit", "-q", "-am", "after the merge"]);
    forge.push(&a, "takt/a").expect("pushed");
    let error = forge
        .pull_request(&a, number)
        .expect_err("a branch moved past the merge");
    assert!(format!("{error:#}").contains("not believed"), "{error:#}");
    git(&hub, &["update-ref", "-d", "refs/heads/takt/a"]);
    let view = forge
        .pull_request(&a, number)
        .expect("read with the branch gone");
    assert_eq!(
        (view.state, view.head.sha),
        (PullRequestState::Merged, head)
    );
}

/// A base that already holds the head merges nothing more: after a merge made by hand with a
/// commit on the base after it, the request is recorded merged by the hand merge, the base
/// where it was; after a base fast-forwarded to the head, by the head itself. Each reads back
/// merged at the head.
#[test]
fn a_base_holding_the_head_is_recorded_merged_by_the_commit_that_holds_it() {
    let scratch = Scratch::new("holder");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    let head = change(&a, "takt/a", "a.txt", "a\n");
    let forge = local(&scratch, &a, &["true"]);
    let artifacts = scratch.0.join("artifacts");
    let number = deliver(&forge, &a, &artifacts, "takt/a");
    let other = scratch.clone("other");
    git(&other, &["fetch", "-q", "origin", "takt/a"]);
    git(
        &other,
        &["merge", "-q", "--no-ff", "--no-edit", "FETCH_HEAD"],
    );
    let by_hand = git(&other, &["rev-parse", "HEAD"]);
    git(&other, &["commit", "-q", "--allow-empty", "-m", "later"]);
    git(
        &other,
        &["push", "-q", "origin", "HEAD:refs/heads/integration"],
    );
    let tip = integration(&hub);

    forge.target_tip(&a, "integration").expect("a tip");
    forge
        .merge_with(&a, &artifacts, number, &head, &message("merge a"))
        .expect("recorded merged");
    assert_eq!(integration(&hub), tip, "no ref moved");
    let record: serde_json::Value = serde_json::from_slice(
        &fs::read(hub.join(format!("takt/requests/{number}.json"))).expect("the record"),
    )
    .expect("json");
    assert_eq!(record["merge"], by_hand.as_str());
    assert_eq!(record["head"], head.as_str());
    let view = forge.pull_request(&a, number).expect("read");
    assert_eq!(
        (view.state, view.head.sha),
        (PullRequestState::Merged, head)
    );

    git(&a, &["checkout", "-q", "-b", "takt/f", &tip]);
    git(&a, &["commit", "-q", "--allow-empty", "-m", "f"]);
    let f_head = git(&a, &["rev-parse", "HEAD"]);
    let second = deliver(&forge, &a, &artifacts, "takt/f");
    git(&other, &["fetch", "-q", "origin", "takt/f"]);
    git(&other, &["merge", "-q", "--ff-only", "FETCH_HEAD"]);
    git(
        &other,
        &["push", "-q", "origin", "HEAD:refs/heads/integration"],
    );
    forge.target_tip(&a, "integration").expect("a tip");
    forge
        .merge_with(&a, &artifacts, second, &f_head, &message("merge f"))
        .expect("recorded merged");
    assert_eq!(integration(&hub), f_head, "no ref moved");
    let record: serde_json::Value = serde_json::from_slice(
        &fs::read(hub.join(format!("takt/requests/{second}.json"))).expect("the record"),
    )
    .expect("json");
    assert_eq!(record["merge"], f_head.as_str());
    let view = forge.pull_request(&a, second).expect("read");
    assert_eq!(
        (view.state, view.head.sha),
        (PullRequestState::Merged, f_head)
    );
}

/// The checks are not run while the state directory's file system has less free than the run's
/// floor: the read is an error naming the floor, nothing run and nothing recorded.
#[test]
fn checks_are_not_run_under_the_free_space_floor() {
    let scratch = Scratch::new("floor");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    change(&a, "takt/a", "a.txt", "a\n");
    let ran = scratch.0.join("ran");
    let line = format!("echo ran >> '{}'", ran.display());
    let forge = local(&scratch, &a, &["sh", "-c", &line]).with_min_free(u64::MAX);
    let number = deliver(&forge, &a, &scratch.0.join("artifacts"), "takt/a");
    let error = forge.checks(&a, number).expect_err("under the floor");
    assert!(format!("{error:#}").contains("--min-free-gib"), "{error:#}");
    assert!(!ran.exists(), "the checks were not run");
    let record: serde_json::Value = serde_json::from_slice(
        &fs::read(hub.join(format!("takt/requests/{number}.json"))).expect("the record"),
    )
    .expect("json");
    assert_eq!(record["checks"], serde_json::json!({}));
}

/// The checks run in a repository of their own under the state directory: it fetched the head
/// from the hub, names no remote, shares no object file with the hub, and the checks see a
/// scratch home, not this process's.
#[test]
fn checks_run_in_a_repository_of_their_own() {
    let scratch = Scratch::new("own-repository");
    let _hub = scratch.hub();
    let a = scratch.clone("a");
    change(&a, "takt/a", "a.txt", "a\n");
    let seen = scratch.0.join("seen");
    let script = format!(
        "{{ test -f a.txt || echo no-head; git remote; find .git/objects -type f -links +1; \
         test -e .git/objects/info/alternates && echo alternates; \
         test \"$HOME\" = \"$1\" && echo home; true; }} > '{}' 2>&1",
        seen.display()
    );
    let home = std::env::var("HOME").unwrap_or_default();
    let forge = local(&scratch, &a, &["sh", "-c", &script, "sh", &home]);
    let number = deliver(&forge, &a, &scratch.0.join("artifacts"), "takt/a");
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Pass);
    assert_eq!(
        fs::read_to_string(&seen).expect("the checks ran"),
        "",
        "the checks saw the head, no remote, no shared object and a home of their own"
    );
}

/// A fail is not recorded for good: the next read runs the checks again, and a run past the
/// timeout fails and is run again too, so neither a flaky check nor a slow one holds the head.
#[test]
fn a_failed_or_timed_out_check_is_run_again() {
    let scratch = Scratch::new("again");
    let _hub = scratch.hub();
    let a = scratch.clone("a");
    change(&a, "takt/a", "a.txt", "a\n");
    let count = scratch.0.join("count");
    let line = format!(
        "n=$(cat '{0}' 2>/dev/null || echo 0); n=$((n + 1)); echo $n > '{0}'; \
         case $n in 1) exit 1 ;; 2) exec sleep 30 ;; esac",
        count.display()
    );
    let forge = local(&scratch, &a, &["sh", "-c", &line]).with_limits(Limits {
        timeout: std::time::Duration::from_secs(2),
        output: 1 << 16,
    });
    let number = deliver(&forge, &a, &scratch.0.join("artifacts"), "takt/a");
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Fail);
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Fail);
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Pass);
    assert_eq!(forge.checks(&a, number).expect("checks"), Checks::Pass);
    assert_eq!(
        fs::read_to_string(&count).expect("the count"),
        "3\n",
        "a fail and a timeout ran again, a pass did not"
    );
}

/// A tree that shares the hub's git directory is refused: a worktree of the hub, outside it, and
/// a repository inside the hub, each for its own reason.
#[test]
fn a_tree_sharing_the_hub_is_refused() {
    let scratch = Scratch::new("shared");
    let hub = scratch.hub();
    let state = scratch.0.join("state");
    let hub_path = hub.to_str().expect("utf-8");
    let linked = scratch.0.join("linked");
    git(
        &scratch.0,
        &[
            "--git-dir",
            hub_path,
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            linked.to_str().expect("utf-8"),
            "integration",
        ],
    );
    git(&linked, &["remote", "set-url", "origin", hub_path]);
    let error = Local::for_tree(&linked, argv(&["true"]), state.clone(), identity())
        .expect_err("a worktree of the hub");
    assert!(
        format!("{error:#}").contains("shares its git directory"),
        "{error:#}"
    );

    git(&hub, &["clone", "-q", hub_path, "inner"]);
    let error = Local::for_tree(
        &hub.join("inner"),
        argv(&["true"]),
        state.clone(),
        identity(),
    )
    .expect_err("a repository inside the hub");
    assert!(
        format!("{error:#}").contains("inside the tree or holds it"),
        "{error:#}"
    );
    assert!(!hub.join("takt").exists());
    assert!(!state.exists());
}

/// Acceptance 5, in the library: an `origin` that is no local path, a path inside the tree, a
/// repository that is not bare, or none, is refused before anything is written to the hub or
/// the state directory.
#[test]
fn an_origin_that_is_no_hub_is_refused() {
    let scratch = Scratch::new("refused");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    let state = scratch.0.join("state");
    let build = |tree: &Path| Local::for_tree(tree, argv(&["true"]), state.clone(), identity());
    let inside = a.join("inner.git");
    git(&a, &["init", "-q", "--bare", "inner.git"]);
    let worktree = scratch.0.join("plain");
    fs::create_dir_all(&worktree).expect("a directory");
    git(&worktree, &["init", "-q"]);
    for (url, words) in [
        ("https://forge.example/group/project.git", "local path"),
        ("git@forge.example:group/project.git", "local path"),
        ("file://forge.example/srv/hub.git", "local path"),
        (inside.to_str().expect("utf-8"), "inside the tree"),
        (worktree.to_str().expect("utf-8"), "bare"),
        (
            scratch.0.join("missing.git").to_str().expect("utf-8"),
            "missing.git",
        ),
    ] {
        git(&a, &["remote", "set-url", "origin", url]);
        let error = build(&a).expect_err(url);
        assert!(format!("{error:#}").contains(words), "{url}: {error:#}");
    }
    git(&a, &["remote", "remove", "origin"]);
    let error = build(&a).expect_err("no origin");
    assert!(format!("{error:#}").contains("origin"), "{error:#}");
    git(
        &a,
        &[
            "remote",
            "add",
            "origin",
            &format!("file://{}", hub.display()),
        ],
    );
    build(&a).expect("a file URL of a local bare repository is a hub");
    assert!(!hub.join("takt").exists());
    assert!(!state.exists());
}
