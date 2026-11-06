//! The `command` work source (`takt.work.WorkCommands`): a team's own command line, for a
//! tracker no connector serves. The run file's `work_commands` section names an argument list
//! for each entry, run without a shell through the forge [`Runner`] as the command forge's are
//! ([`crate::forge::command`]), each element substituted on its own and never split.
//!
//! | entry | runs | prints |
//! |---|---|---|
//! | `read` | for `--work command:<id>`: `{id}`, the key's id | the item, one `takt.work-item/1` document |
//! | `select` | for `command` in the run file's `work` list | the next ready item, or nothing when none is ready |
//!
//! Exit 0 is ok; an exit in `refused_exit` is a refusal, any other exit a failure, as for the
//! command forge. Output that is no `takt.work-item/1` document ([`Item::parse`]), or an item
//! of another key than the one asked for, is refused naming the command and repeating nothing
//! it printed. Standard error of every command is kept in a file under the state directory,
//! never in the case.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::forge::command::{named_in, substitute};
use crate::forge::{Exit, Runner};
use crate::frontier::{Key, Source, SourceName};

/// The format a work item document names (`takt.work.Item` `format`).
pub const ITEM_FORMAT: &str = "takt.work-item/1";

/// An entry of `work_commands`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// `read`: one item by its id.
    Read,
    /// `select`: the next ready item.
    Select,
}

impl Entry {
    /// Every entry, in the order the run file documents them.
    pub const ALL: [Self; 2] = [Self::Read, Self::Select];

    /// The entry's key in the section.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Select => "select",
        }
    }

    /// The placeholders the entry's argument list may hold.
    #[must_use]
    pub fn placeholders(self) -> &'static [&'static str] {
        match self {
            Self::Read => &["id"],
            Self::Select => &[],
        }
    }
}

impl fmt::Display for Entry {
    /// `work_commands.<entry>`, as a refusal names it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "work_commands.{}", self.name())
    }
}

/// The `work_commands` section of a run file: one argument list per entry, never a shell line.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkCommands {
    /// Prints the item of one id, `{id}`.
    pub read: Option<Vec<String>>,
    /// Prints the next ready item, or nothing when none is ready.
    pub select: Option<Vec<String>>,
    /// The exit codes that are a refusal and not a failure.
    pub refused_exit: Option<Vec<i32>>,
}

impl WorkCommands {
    /// The argument list of `entry`, when the section names one.
    #[must_use]
    pub fn argv(&self, entry: Entry) -> Option<&[String]> {
        match entry {
            Entry::Read => &self.read,
            Entry::Select => &self.select,
        }
        .as_deref()
    }

    /// Checks the section's shape as [`crate::forge::command::ForgeCommands::check`] does: each
    /// entry a non-empty argument list naming a program that is no placeholder, holding no
    /// placeholder the entry does not take, `refused_exit` codes above 0 and below 256. The
    /// refusal names the key and never repeats a value.
    pub fn check(&self) -> Result<()> {
        for entry in Entry::ALL {
            let Some(argv) = self.argv(entry) else {
                continue;
            };
            if argv.first().is_none_or(|program| program.trim().is_empty()) {
                bail!(
                    "`{entry}` is an argument list whose first element is the program, and it names none"
                );
            }
            for element in argv {
                if let Some(unknown) = named_in(element)
                    .into_iter()
                    .find(|name| !entry.placeholders().contains(name))
                {
                    let takes: Vec<String> = entry
                        .placeholders()
                        .iter()
                        .map(|name| format!("{{{name}}}"))
                        .collect();
                    bail!(
                        "`{entry}` holds the placeholder {{{unknown}}}, which that entry does not take: it takes {}",
                        if takes.is_empty() {
                            "none".to_owned()
                        } else {
                            takes.join(", ")
                        }
                    );
                }
            }
            if !named_in(&argv[0]).is_empty() {
                bail!(
                    "`{entry}` names its program with a placeholder: the first element is a program of the file's own, never a substituted value"
                );
            }
        }
        if let Some(codes) = &self.refused_exit
            && codes.iter().any(|code| !(1..=255).contains(code))
        {
            bail!("`work_commands.refused_exit` holds exit codes from 1 to 255; 0 is success");
        }
        Ok(())
    }
}

/// The document as written, every member the format has and no other.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: String,
    key: String,
    title: String,
    body: String,
    #[serde(default)]
    url: Option<String>,
    revision: String,
    #[serde(default)]
    labels: Option<Vec<String>>,
}

/// A work item as its source hands it over (`takt.work.Item`), read from a `takt.work-item/1`
/// document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Its key.
    pub key: Key,
    /// Its title, one line.
    pub title: String,
    /// Its body.
    pub body: String,
    /// The URL a person opens it at, when the source gives one.
    pub url: Option<String>,
    /// Its revision as the source reports it: an etag or an update time for a tracker.
    pub revision: String,
    /// Its labels; empty when it has none.
    pub labels: Vec<String>,
}

impl Item {
    /// The item `text` writes as a `takt.work-item/1` JSON document; why it is none, naming the
    /// member and repeating nothing of `text`: it is no JSON object of the format's members
    /// alone, each of its type; its `format` is another; its `key` is no key
    /// ([`Key::parse`]); its `title` is empty or holds a line break or a control character, so
    /// it would not stand as the first line of the case's intent; or its `revision` is empty.
    pub fn parse(text: &str) -> std::result::Result<Self, String> {
        let document: Document = serde_json::from_str(text).map_err(|_| {
            "it does not read as one: a JSON object of `format`, `key`, `title`, `body` and `revision`, strings, and optionally `url`, a string, and `labels`, a list of strings, and no other member".to_owned()
        })?;
        if document.format != ITEM_FORMAT {
            return Err(format!("its `format` is not `{ITEM_FORMAT}`"));
        }
        let key = Key::parse(&document.key)
            .map_err(|_| "its `key` is no key `<source>[@<connection>]:<id>`".to_owned())?;
        if document.title.trim().is_empty() || document.title.chars().any(char::is_control) {
            return Err(
                "its `title` is empty or holds a line break or a control character".to_owned(),
            );
        }
        if document.revision.trim().is_empty() {
            return Err("its `revision` is empty".to_owned());
        }
        Ok(Self {
            key,
            title: document.title,
            body: document.body,
            url: document.url,
            revision: document.revision,
            labels: document.labels.unwrap_or_default(),
        })
    }

    /// The case's `intent.md`: the title, a blank line and the body, as a story's is
    /// ([`crate::frontier::Story::intent`]).
    #[must_use]
    pub fn intent(&self) -> String {
        format!("{}\n\n{}", self.title, self.body)
    }
}

/// Why a work command gave no item: a refusal, a failure, or output that is no item or not the
/// item asked for. Each names the command as `work_commands.<entry>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkCommandError {
    /// It exited with a code in `refused_exit`.
    Refused {
        /// The entry that ran.
        entry: Entry,
        /// Its exit code.
        exit: i32,
        /// The file its standard error was kept in.
        stderr: PathBuf,
    },
    /// It exited otherwise than 0 or a refusal, or ended on a signal.
    Failed {
        /// The entry that ran.
        entry: Entry,
        /// Its exit code; `None` when a signal ended it.
        exit: Option<i32>,
        /// The file its standard error was kept in.
        stderr: PathBuf,
    },
    /// What it printed is no `takt.work-item/1` document.
    NoItem {
        /// The entry that ran.
        entry: Entry,
        /// Why, from [`Item::parse`].
        why: String,
    },
    /// It printed an item of another key than `read` was asked for.
    AnotherKey {
        /// The key asked for.
        asked: String,
    },
    /// `select` printed an item of another source than the one it serves.
    AnotherSource {
        /// The source `select` serves.
        source: String,
    },
}

impl fmt::Display for WorkCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused {
                entry,
                exit,
                stderr,
            } => write!(
                f,
                "the work command `{entry}` was refused with exit {exit}; its standard error is in {}",
                stderr.display()
            ),
            Self::Failed {
                entry,
                exit,
                stderr,
            } => write!(
                f,
                "the work command `{entry}` failed, {}; its standard error is in {}",
                exit.map_or("it ended on a signal".to_owned(), |code| format!(
                    "exit {code}"
                )),
                stderr.display()
            ),
            Self::NoItem { entry, why } => write!(
                f,
                "the output of the work command `{entry}` is not a {ITEM_FORMAT} document: {why}"
            ),
            Self::AnotherKey { asked } => write!(
                f,
                "the work command `{}` printed an item that names another key than {asked}",
                Entry::Read
            ),
            Self::AnotherSource { source } => write!(
                f,
                "the work command `{}` printed an item whose key names no item of the source {source}",
                Entry::Select
            ),
        }
    }
}

impl std::error::Error for WorkCommandError {}

/// The `command` source over the commands of `work_commands`: `read` and `select`, each run
/// through `runner` in the tree it is given, its standard error kept under `state`.
pub struct CommandSource {
    commands: WorkCommands,
    state: PathBuf,
    runner: Box<dyn Runner>,
}

impl fmt::Debug for CommandSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandSource")
            .field("commands", &self.commands)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl CommandSource {
    /// Over `commands`, refused when the section is misshapen ([`WorkCommands::check`]);
    /// `state` is the directory each command's standard error is kept in. Every line runs
    /// through `runner`, which for a run is [`crate::forge::Process`]: no shell, no
    /// credential, nothing that waits on a person.
    pub fn new(
        commands: WorkCommands,
        state: impl Into<PathBuf>,
        runner: Box<dyn Runner>,
    ) -> Result<Self> {
        commands.check()?;
        Ok(Self {
            commands,
            state: state.into(),
            runner,
        })
    }

    /// The source `select` serves: `command`, which takes no connection.
    #[must_use]
    pub fn source() -> Source {
        Source {
            name: SourceName::Command,
            connection: None,
        }
    }

    /// The item of `key` as `read` prints it, run in `tree` with `{id}` the key's id. Refused
    /// when the section has no `read`, the command refuses or fails, its output is no item, or
    /// the item names another key than `key`.
    pub fn read(&self, tree: &Path, key: &Key) -> Result<Item> {
        let exit = self.run(Entry::Read, tree, &[("id", key.id())])?;
        let item = Item::parse(&exit.stdout).map_err(|why| WorkCommandError::NoItem {
            entry: Entry::Read,
            why,
        })?;
        if item.key != *key {
            return Err(WorkCommandError::AnotherKey {
                asked: key.to_string(),
            }
            .into());
        }
        Ok(item)
    }

    /// The next ready item as `select` prints it, run in `tree`; `None` when it prints nothing
    /// but white space. Refused when the section has no `select`, the command refuses or
    /// fails, its output is no item, or the item is of another source than `command`.
    pub fn select(&self, tree: &Path) -> Result<Option<Item>> {
        let exit = self.run(Entry::Select, tree, &[])?;
        if exit.stdout.trim().is_empty() {
            return Ok(None);
        }
        let item = Item::parse(&exit.stdout).map_err(|why| WorkCommandError::NoItem {
            entry: Entry::Select,
            why,
        })?;
        if *item.key.source() != Self::source() {
            return Err(WorkCommandError::AnotherSource {
                source: Self::source().to_string(),
            }
            .into());
        }
        Ok(Some(item))
    }

    /// Runs `entry` with `values` substituted, in `tree`: the argument list of the section,
    /// each element substituted on its own and never split. Standard error goes to the state
    /// directory.
    fn run(&self, entry: Entry, tree: &Path, values: &[(&str, &str)]) -> Result<Exit> {
        let Some(template) = self.commands.argv(entry) else {
            bail!(
                "the run file's `work_commands` has no `{}` entry",
                entry.name()
            );
        };
        let argv: Vec<String> = template
            .iter()
            .map(|element| substitute(element, values))
            .collect();
        std::fs::create_dir_all(&self.state)
            .with_context(|| format!("create {}", self.state.display()))?;
        let exit = self
            .runner
            .run(tree, &argv)
            .with_context(|| format!("the work command `{entry}` could not be run"))?;
        let stderr = self.state.join(format!("{}.stderr", entry.name()));
        std::fs::write(&stderr, &exit.stderr)
            .with_context(|| format!("write {}", stderr.display()))?;
        match exit.code {
            Some(0) => Ok(exit),
            Some(code)
                if self
                    .commands
                    .refused_exit
                    .as_deref()
                    .is_some_and(|codes| codes.contains(&code)) =>
            {
                Err(WorkCommandError::Refused {
                    entry,
                    exit: code,
                    stderr,
                }
                .into())
            }
            code => Err(WorkCommandError::Failed {
                entry,
                exit: code,
                stderr,
            }
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    const ITEM: &str = r#"{"format": "takt.work-item/1", "key": "command:X-1", "title": "Fix it", "body": "The body.", "revision": "7"}"#;

    /// A runner that records every argument list and answers one exit; it starts no process.
    struct Fake {
        calls: Rc<RefCell<Vec<Vec<String>>>>,
        answer: Exit,
    }

    impl Runner for Fake {
        fn run(&self, _tree: &Path, argv: &[String]) -> Result<Exit> {
            self.calls.borrow_mut().push(argv.to_vec());
            Ok(self.answer.clone())
        }

        fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> Result<Exit> {
            self.run(tree, argv)
        }
    }

    fn over(
        name: &str,
        commands: WorkCommands,
        code: i32,
        stdout: &str,
    ) -> (CommandSource, Rc<RefCell<Vec<Vec<String>>>>, PathBuf) {
        let state =
            std::env::temp_dir().join(format!("takt-work-command-{}-{name}", std::process::id()));
        let calls = Rc::new(RefCell::new(Vec::new()));
        let fake = Fake {
            calls: Rc::clone(&calls),
            answer: Exit {
                code: Some(code),
                stdout: stdout.to_owned(),
                stderr: "said on stderr".to_owned(),
            },
        };
        let source = CommandSource::new(commands, &state, Box::new(fake)).expect("a source");
        (source, calls, state)
    }

    fn commands() -> WorkCommands {
        WorkCommands {
            read: Some(vec![
                "tracker".to_owned(),
                "show".to_owned(),
                "--id={id}".to_owned(),
            ]),
            select: Some(vec!["tracker".to_owned(), "next".to_owned()]),
            refused_exit: Some(vec![3]),
        }
    }

    fn key(text: &str) -> Key {
        Key::parse(text).expect("a key")
    }

    /// story:the-command-work-source: `read` runs its argument list with `{id}` the key's id,
    /// substituted inside its element and never split, and hands the item over.
    #[test]
    fn read_runs_its_argv_with_the_id_and_reads_the_item() {
        let (source, calls, state) = over("read", commands(), 0, ITEM);
        let item = source
            .read(Path::new("."), &key("command:X-1"))
            .expect("the item");
        assert_eq!(
            calls.borrow()[0],
            ["tracker", "show", "--id=X-1"].map(str::to_owned)
        );
        assert_eq!(item.key, key("command:X-1"));
        assert_eq!(item.title, "Fix it");
        assert_eq!(item.revision, "7");
        assert_eq!(item.url, None);
        assert!(item.labels.is_empty());
        assert_eq!(item.intent(), "Fix it\n\nThe body.");
        assert_eq!(
            std::fs::read_to_string(state.join("read.stderr")).expect("stderr kept"),
            "said on stderr"
        );
        let _ = std::fs::remove_dir_all(&state);
        // An id holding shell syntax is one argument, as written.
        let (source, calls, state) = over("read-shell", commands(), 0, ITEM);
        let _ = source.read(Path::new("."), &key("command:X-1;rm"));
        assert_eq!(calls.borrow()[0][2], "--id=X-1;rm");
        let _ = std::fs::remove_dir_all(&state);
    }

    /// An item naming another key than the one asked for is refused, naming `read`; output that
    /// is no item is refused, naming `read` and repeating nothing it printed.
    #[test]
    fn read_refuses_another_key_and_output_that_is_no_item() {
        let (source, _, state) = over("another", commands(), 0, ITEM);
        let error = format!(
            "{:#}",
            source
                .read(Path::new("."), &key("command:X-2"))
                .expect_err("another key")
        );
        assert!(
            error.contains(
                "`work_commands.read` printed an item that names another key than command:X-2"
            ),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&state);
        for (text, why) in [
            ("SECRETISH", "it does not read as one"),
            ("", "it does not read as one"),
            (
                &ITEM.replace("takt.work-item/1", "SECRETISH/2"),
                "its `format` is not `takt.work-item/1`",
            ),
            (
                &ITEM.replace("command:X-1", "SECRETISH"),
                "its `key` is no key",
            ),
            (
                &ITEM.replace("\"Fix it\"", "\"Fix\\nSECRETISH\""),
                "its `title` is empty or holds a line break",
            ),
            (
                &ITEM.replace("\"Fix it\"", "\"  \""),
                "its `title` is empty",
            ),
            (&ITEM.replace("\"7\"", "\"\""), "its `revision` is empty"),
            (&ITEM.replace("\"7\"", "7"), "it does not read as one"),
            (
                &ITEM.replace("\"body\"", "\"SECRETISH\""),
                "it does not read as one",
            ),
        ] {
            let (source, _, state) = over("no-item", commands(), 0, text);
            let error = format!(
                "{:#}",
                source
                    .read(Path::new("."), &key("command:X-1"))
                    .expect_err(text)
            );
            assert!(
                error.contains(
                    "the output of the work command `work_commands.read` is not a takt.work-item/1 document"
                ) && error.contains(why),
                "{text}: ({why}) {error}"
            );
            assert!(!error.contains("SECRETISH"), "{error}");
            let _ = std::fs::remove_dir_all(&state);
        }
    }

    /// An exit in `refused_exit` is a refusal, any other non-zero exit a failure, each naming
    /// the command and the file its standard error is in.
    #[test]
    fn a_refused_or_failed_command_is_named() {
        for (code, said) in [
            (
                3,
                "the work command `work_commands.select` was refused with exit 3",
            ),
            (4, "the work command `work_commands.select` failed, exit 4"),
        ] {
            let (source, _, state) = over("exit", commands(), code, ITEM);
            let error = format!("{:#}", source.select(Path::new(".")).expect_err("no item"));
            assert!(error.contains(said), "{error}");
            assert!(
                error.contains(&state.join("select.stderr").display().to_string()),
                "{error}"
            );
            let _ = std::fs::remove_dir_all(&state);
        }
    }

    /// `select` printing nothing but white space is no ready item; an item of another source is
    /// refused naming `select`; one of `command` is handed over.
    #[test]
    fn select_reads_nothing_as_no_ready_item() {
        for nothing in ["", "\n", "  \n\t"] {
            let (source, calls, state) = over("none", commands(), 0, nothing);
            assert_eq!(source.select(Path::new(".")).expect("no item"), None);
            assert_eq!(calls.borrow()[0], ["tracker", "next"].map(str::to_owned));
            let _ = std::fs::remove_dir_all(&state);
        }
        let (source, _, state) = over("one", commands(), 0, ITEM);
        assert_eq!(
            source
                .select(Path::new("."))
                .expect("an item")
                .map(|i| i.key),
            Some(key("command:X-1"))
        );
        let _ = std::fs::remove_dir_all(&state);
        let (source, _, state) = over(
            "other",
            commands(),
            0,
            &ITEM.replace("command:X-1", "jira:X-1"),
        );
        let error = format!("{:#}", source.select(Path::new(".")).expect_err("other"));
        assert!(
            error.contains("`work_commands.select` printed an item whose key names no item of the source command"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&state);
    }

    /// The section's shape: an entry with no program, a placeholder the entry does not take, a
    /// program named by a placeholder, and an exit code outside 1 to 255 are refused, naming
    /// the key.
    #[test]
    fn the_section_shape_is_checked() {
        assert!(commands().check().is_ok());
        assert!(WorkCommands::default().check().is_ok());
        let argv = |words: &[&str]| Some(words.iter().map(|w| (*w).to_owned()).collect());
        for (commands, why) in [
            (
                WorkCommands {
                    read: argv(&[]),
                    ..WorkCommands::default()
                },
                "`work_commands.read` is an argument list whose first element is the program",
            ),
            (
                WorkCommands {
                    select: argv(&["tracker", "{id}"]),
                    ..WorkCommands::default()
                },
                "`work_commands.select` holds the placeholder {id}, which that entry does not take: it takes none",
            ),
            (
                WorkCommands {
                    read: argv(&["tracker", "{number}"]),
                    ..WorkCommands::default()
                },
                "it takes {id}",
            ),
            (
                WorkCommands {
                    read: argv(&["{id}"]),
                    ..WorkCommands::default()
                },
                "names its program with a placeholder",
            ),
            (
                WorkCommands {
                    refused_exit: Some(vec![0]),
                    ..WorkCommands::default()
                },
                "`work_commands.refused_exit` holds exit codes from 1 to 255",
            ),
        ] {
            let error = format!("{:#}", commands.check().expect_err(why));
            assert!(error.contains(why), "({why}) {error}");
        }
    }
}
