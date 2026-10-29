//! Adversarial cases for story:console-shows-reads-and-diffs (wave 44, unit 5): the listing of
//! a harness step's tool calls and the diff of its commit, attacked on escaping, bounds, colour
//! and transcript parsing. Each case cites the line of the story it rests on.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::console::{
    ColorMode, Console, DIFF_SHOWN, LINE_WIDTH, Paint, READS_SHOWN, color_on, diff, reads,
};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, TestsRun, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = "story.delivery/1";

/// A transcript as takt keeps it: its own first line, then `lines`, one JSON value each.
fn transcript(lines: &[serde_json::Value]) -> Vec<u8> {
    let mut bytes = b"{\"context\":[],\"prompt\":\"p\",\"step\":1,\"type\":\"takt\"}\n".to_vec();
    for line in lines {
        bytes.extend_from_slice(line.to_string().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

fn bash(command: &str) -> serde_json::Value {
    serde_json::json!({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": "t", "name": "Bash", "input": {"command": command}},
    ]}})
}

fn codex_script(input: &str) -> serde_json::Value {
    serde_json::json!({"type": "response_item", "payload": {
        "type": "custom_tool_call", "name": "exec", "input": input,
    }})
}

fn owned(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

/// Unicode's Bidi_Control property (PropList.txt): every character that changes how a
/// bidi-aware terminal orders the rest of a line.
fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

/// The columns a terminal gives `text`: two for an East Asian wide or fullwidth character (the
/// common blocks), one for any other.
fn columns(text: &str) -> usize {
    text.chars()
        .map(|c| match u32::from(c) {
            0x1100..=0x115f
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe30..=0xfe4f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
            | 0x1f300..=0x1faff
            | 0x20000..=0x3fffd => 2,
            _ => 1,
        })
        .sum()
}

/// `line` with each SGR sequence the paint writes (`ESC [ <digits> m`) taken out.
fn unpainted(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find("\u{1b}[") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        if after[digits..].starts_with('m') {
            rest = &after[digits + 1..];
        } else {
            out.push_str(&rest[at..at + 2]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------------------------
// Escaping
// ---------------------------------------------------------------------------------------------

/// Story, Outcome 1: each line "is escaped as every other console line is (`crate::trace::escape`:
/// no control or bidi character, no terminal escape from the transcript reaches the terminal)".
/// U+200E, U+200F and U+061C are bidi controls (Unicode's Bidi_Control property) as U+202E is.
#[test]
fn adv_w44_u5_bidi_marks_in_a_command_print_escaped() {
    let command = "cat \u{200f}cod.exe \u{200e}x \u{061c}y";
    let lines = reads(&transcript(&[bash(command)]), Paint::OFF);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let leaked: Vec<String> = lines[0]
        .chars()
        .filter(|c| is_bidi_control(*c))
        .map(|c| format!("U+{:04X}", u32::from(c)))
        .collect();
    assert!(
        leaked.is_empty(),
        "bidi controls reach the terminal unescaped: {leaked:?} in {:?}",
        lines[0]
    );
    let patched = diff(&format!("+{command}\n"), "abcdef12", Paint::OFF);
    assert!(
        !patched[0].chars().any(is_bidi_control),
        "bidi controls in a diff line reach the terminal: {:?}",
        patched[0]
    );
}

/// Story, Outcome 2: the diff is "escaped the same way"; Outcome 3: colour is the paint's only.
/// OSC, an 8-bit CSI (U+009B), NUL, DEL, a lone carriage return and an isolate in a diff's lines
/// and a command reach the terminal escaped; in colour the only sequences are the paint's.
#[test]
fn adv_w44_u5_terminal_sequences_in_a_diff_and_a_command_print_escaped() {
    let hostile = "a\u{1b}]0;owned\u{7} b\u{9b}2J c\u{0} d\u{7f} e\rOVERWRITE f\u{2067}g";
    let text = format!(
        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@ {hostile}\n-{hostile}\n+{hostile}\n {hostile}\n"
    );
    for paint in [Paint::OFF, Paint { on: true }] {
        let lines = diff(&text, "abcdef12", paint)
            .into_iter()
            .chain(reads(&transcript(&[bash(hostile)]), paint))
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 8, "{lines:?}");
        for (at, line) in lines.iter().enumerate() {
            let bare = unpainted(line);
            assert!(
                !bare.chars().any(|c| c.is_control() || is_bidi_control(c)),
                "a control reaches the terminal: {line:?}"
            );
            if at >= 3 {
                assert!(bare.contains("OVERWRITE"), "{line:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------------------------

/// Story, Outcome 3: "diff lines added green, removed red". A removed line whose text begins
/// `-- ` (an SQL, Lua or Haskell comment) shows in the diff as `--- …`, and an added line whose
/// text begins `++ ` as `+++ …`; inside a hunk they are a removed and an added line, not file
/// headers.
#[test]
fn adv_w44_u5_hunk_lines_that_look_like_file_headers_keep_their_colour() {
    let text = "diff --git a/q.sql b/q.sql\n--- a/q.sql\n+++ b/q.sql\n@@ -1,2 +1,2 @@\n\
                --- drop the old table\n+++ counter\n select 1;\n";
    let lines = diff(text, "abcdef12", Paint { on: true });
    assert_eq!(lines.len(), 7, "{lines:?}");
    assert_eq!(
        lines[4], "    \u{1b}[31m--- drop the old table\u{1b}[0m",
        "a removed line inside a hunk is not red"
    );
    assert_eq!(
        lines[5], "    \u{1b}[32m+++ counter\u{1b}[0m",
        "an added line inside a hunk is not green"
    );
}

/// Story, Outcome 3: "`auto`: on when standard output is a terminal and `NO_COLOR` is unset or
/// empty, as <https://no-color.org> says" (any non-empty value, `0` among them, turns it off);
/// `never` is off whatever the rest; `always` is on piped.
#[test]
fn adv_w44_u5_no_color_of_any_non_empty_value_turns_auto_off() {
    let os = |value: &str| Some(std::ffi::OsString::from(value));
    for value in ["0", "false", " ", "no"] {
        assert!(
            !color_on(ColorMode::Auto, true, os(value).as_deref()),
            "NO_COLOR={value:?} left colour on"
        );
    }
    assert!(!color_on(ColorMode::Auto, false, os("").as_deref()));
    assert!(color_on(ColorMode::Auto, true, os("").as_deref()));
    assert!(!color_on(ColorMode::Never, true, os("").as_deref()));
    assert!(color_on(ColorMode::Always, false, None));
    let off = Console {
        color: false,
        ..Console::default()
    };
    assert_eq!(off.paint(), Paint::OFF);
}

// ---------------------------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------------------------

/// Story, Outcome 1: each line is "cut at the console's width with ` …`" (fixed at 100
/// columns). A command of wide characters takes two columns a character.
#[test]
fn adv_w44_u5_wide_characters_are_cut_at_the_console_width() {
    let wide = "漢".repeat(300);
    let lines = reads(&transcript(&[bash(&format!("echo {wide}"))]), Paint::OFF);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        columns(&lines[0]) <= LINE_WIDTH,
        "the listed line takes {} columns, past the console's {LINE_WIDTH}: {:?}",
        columns(&lines[0]),
        lines[0]
    );
    let patched = diff(&format!("+{wide}\n"), "abcdef12", Paint::OFF);
    assert!(
        columns(&patched[0]) <= LINE_WIDTH,
        "the diff line takes {} columns, past the console's {LINE_WIDTH}",
        columns(&patched[0])
    );
}

/// Story, Outcome 1: "at most 20 lines per step, then `… <n> more`": twenty calls are twenty
/// lines, twenty-one are twenty and `… 1 more`.
#[test]
fn adv_w44_u5_twenty_calls_list_whole_and_twenty_one_say_one_more() {
    let calls = |n: usize| -> Vec<serde_json::Value> {
        (0..n).map(|i| bash(&format!("echo {i}"))).collect()
    };
    let twenty = reads(&transcript(&calls(READS_SHOWN)), Paint::OFF);
    assert_eq!(twenty.len(), READS_SHOWN, "{twenty:?}");
    assert_eq!(twenty[READS_SHOWN - 1], "  $ echo 19");
    let more = reads(&transcript(&calls(READS_SHOWN + 1)), Paint::OFF);
    assert_eq!(more.len(), READS_SHOWN + 1, "{more:?}");
    assert_eq!(more[READS_SHOWN], "  … 1 more");
}

/// Story, Outcome 2: "at most 120 lines per step, then `… <n> more lines (git show <sha>)`".
#[test]
fn adv_w44_u5_a_diff_of_121_lines_names_one_more_and_its_commit() {
    let text: String = (0..=DIFF_SHOWN).map(|n| format!("+l{n}\n")).collect();
    let lines = diff(&text, "0123abcd", Paint { on: true });
    assert_eq!(lines.len(), DIFF_SHOWN + 1, "{lines:?}");
    assert_eq!(lines[DIFF_SHOWN], "    … 1 more lines (git show 0123abcd)");
}

// ---------------------------------------------------------------------------------------------
// Transcript parsing
// ---------------------------------------------------------------------------------------------

/// Story, Outcome 1: "One line per tool call in the step's transcript, in order". A Codex
/// script whose comment holds an apostrophe still made the call after it.
#[test]
fn adv_w44_u5_an_apostrophe_in_a_script_comment_hides_no_call() {
    let script = "// don't print the whole file\ntext(await tools.exec_command({cmd:\"head -5 src/lib.rs\"}));\n";
    let lines = reads(&transcript(&[codex_script(script)]), Paint::OFF);
    assert_eq!(
        lines,
        owned(&["  $ head -5 src/lib.rs"]),
        "the call after a comment's apostrophe is not listed"
    );
}

/// Story, Outcome 1: "One line per tool call in the step's transcript". A Codex
/// `custom_tool_call` whose freeform input is no `tools.` script is still one call.
#[test]
fn adv_w44_u5_a_freeform_custom_tool_call_is_still_one_line() {
    let call = serde_json::json!({"type": "response_item", "payload": {
        "type": "custom_tool_call", "name": "web_search", "input": "serde flatten enum",
    }});
    let lines = reads(&transcript(&[call]), Paint::OFF);
    assert_eq!(
        lines.len(),
        1,
        "a tool call is listed as no line: {lines:?}"
    );
}

/// Story, Outcome 1: a shell command's line is "the command line". Escaped quotes in a Codex
/// script's `cmd`, several calls in one script, a `function_call`'s JSON-escaped command and a
/// `bash -lc` vector all read as the command run.
#[test]
fn adv_w44_u5_escaped_quotes_and_several_calls_read_as_the_commands() {
    let script = "const a = await tools.exec_command({cmd:\"echo \\\"a b\\\" 'c'\"});\n\
                  const b = await tools.exec_command({ 'cmd' : 'grep -n \\'x\\' f', yield_time_ms: 1 });\n\
                  text(a + b);\n";
    let function = serde_json::json!({"type": "response_item", "payload": {
        "type": "function_call", "name": "exec_command",
        "arguments": serde_json::json!({"cmd": "printf \"%s\\n\" \"q\""}).to_string(),
    }});
    let shell = serde_json::json!({"type": "response_item", "payload": {
        "type": "local_shell_call", "action": {"command": ["bash", "-lc", "ls -a \"d\""]},
    }});
    let lines = reads(
        &transcript(&[codex_script(script), function, shell]),
        Paint::OFF,
    );
    assert_eq!(
        lines,
        owned(&[
            "  $ echo \"a b\" 'c'",
            "  $ grep -n 'x' f",
            "  $ printf \"%s\\n\" \"q\"",
            "  $ ls -a \"d\"",
        ])
    );
}

/// The brief's priority 4: malformed JSON lines, invalid UTF-8 and a cut line do not stop the
/// listing (story, Outcome 1: one line per tool call, in order).
#[test]
fn adv_w44_u5_malformed_lines_and_invalid_utf8_do_not_stop_the_listing() {
    let mut bytes = transcript(&[bash("first")]);
    bytes.extend_from_slice(
        b"{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\"\n",
    );
    bytes.extend_from_slice(b"\xff\xfe\x00garbage\x1b[2J\n");
    bytes.extend_from_slice(b"[1,2,3]\n42\nnull\n\"type\"\n{\"type\":[]}\n");
    bytes.extend_from_slice(b"{\"type\":\"assistant\",\"message\":{\"content\":\"text\"}}\n");
    bytes.extend_from_slice(b"{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"arguments\":\"{not json\"}}\n");
    bytes.extend_from_slice(bash("second").to_string().as_bytes());
    bytes.extend_from_slice(b"\r\n");
    bytes.extend_from_slice(bash("third").to_string().as_bytes());
    let lines = reads(&bytes, Paint::OFF);
    assert_eq!(
        lines,
        owned(&["  $ first", "  $ ", "  $ second", "  $ third"]),
        "{lines:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Through a run
// ---------------------------------------------------------------------------------------------

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed scratch repository holding `src/lib.rs` and `src/old.rs`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adv-w44-u5-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join("src")).expect("scratch dir");
    fs::write(dir.join("src/lib.rs"), "pub fn add() {}\n").expect("a file");
    fs::write(
        dir.join("src/old.rs"),
        "pub fn one() {}\npub fn two() {}\npub fn three() {}\n",
    )
    .expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// On its first call runs `edit` on the tree under `implementation.edit` (or `tests.write`);
/// later calls change nothing. Reports `transcript` as each step's, none when `None`.
struct Editing<'a> {
    transcript: Option<Vec<u8>>,
    edit: &'a dyn Fn(&Path) -> std::io::Result<()>,
    calls: Cell<usize>,
}

impl Harness for Editing<'_> {
    fn name(&self) -> &str {
        "editing"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        let action = ["implementation.edit", "tests.write"]
            .into_iter()
            .find(|action| context.offered.iter().any(|offered| offered == action))
            .unwrap_or("implementation.edit");
        if self.calls.get() == 1 {
            (self.edit)(context.tree)?;
        }
        Ok(StepReport {
            action: action.to_owned(),
            summary: "edited".to_owned(),
            tokens: Some(10),
        })
    }

    fn last_transcript(&self) -> Option<Vec<u8>> {
        self.transcript.clone()
    }
}

/// What a run printed under `console`, its tree kept for reading after.
fn ran(
    tree: &Path,
    name: &str,
    transcript: Option<Vec<u8>>,
    edit: &dyn Fn(&Path) -> std::io::Result<()>,
    console: Console,
) -> String {
    let harness = Editing {
        transcript,
        edit,
        calls: Cell::new(0),
    };
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
    ];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "edit".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(tree, options, &harness, &validators).expect("the case opens");
    run.console(console);
    let mut out = Vec::new();
    run.run(&mut out).expect("the run stops without an error");
    String::from_utf8(out).expect("utf-8")
}

/// The lines of the first harness step that committed: from its `said:` line to its `effect:`
/// line, both included.
fn committed_step(out: &str) -> Vec<&str> {
    let lines: Vec<&str> = out.lines().collect();
    let effect = lines
        .iter()
        .position(|line| line.contains("effect:") && line.contains(" committed "))
        .unwrap_or_else(|| panic!("no step committed:\n{out}"));
    let said = lines[..effect]
        .iter()
        .rposition(|line| line.contains("said:"))
        .unwrap_or_else(|| panic!("no said: line before the effect:\n{out}"));
    lines[said..=effect].to_vec()
}

/// Story, Outcome 2: the diff is "`git diff <before>..<after>`, outside `.engineering/` and
/// `.takt/`"; "A step that committed nothing prints no diff." A harness step that writes under
/// `.engineering/` is refused and its change discarded (the step guard), so the console prints
/// no diff of it, neither of the source file nor of the planning file; the case's own commits
/// under `.takt/` are never printed.
#[test]
fn adv_w44_u5_engineering_paths_stay_out_of_the_printed_diff() {
    let tree = scratch("engineering");
    let edit = |tree: &Path| -> std::io::Result<()> {
        fs::write(
            tree.join("src/lib.rs"),
            "pub fn add() {}\npub fn sub() {}\n",
        )?;
        fs::create_dir_all(tree.join(".engineering/notes"))?;
        fs::write(
            tree.join(".engineering/notes/secret-note.md"),
            "PLANNING-ONLY\n",
        )
    };
    let out = ran(&tree, "engineering", None, &edit, Console::default());
    assert!(out.contains("refused: implementation.edit"), "{out}");
    assert!(
        !out.contains("diff --git") && !out.contains("PLANNING-ONLY"),
        "a refused step's change is printed as a diff:\n{out}"
    );
    assert!(!out.contains("+++ b/.takt/"), "{out}");
}

/// Story, Outcome 2 and the brief's bound: a diff git gives past 4 MiB is one line that names
/// the commit, and the run goes on to its `effect:` line.
#[test]
fn adv_w44_u5_a_diff_past_four_mib_is_one_line_naming_its_commit() {
    let tree = scratch("big");
    let edit = |tree: &Path| -> std::io::Result<()> {
        let line = format!("{}\n", "z".repeat(99));
        fs::write(tree.join("src/big.txt"), line.repeat(50_000))
    };
    let out = ran(&tree, "big", None, &edit, Console::default());
    let step = committed_step(&out);
    let head = git(&tree, &["log", "-1", "--format=%H", "--", "src/big.txt"]);
    let short = &head.trim()[..8];
    assert_eq!(
        &step[1..step.len() - 1],
        &[format!("    … the diff passes 4 MiB (git show {short})").as_str()],
        "{}",
        step.iter().take(5).copied().collect::<Vec<_>>().join("\n")
    );
}

/// Story, Outcome 1 and 2: a step with no transcript lists no call; a binary file's bytes never
/// reach the terminal, and a rename is printed as git says it.
#[test]
fn adv_w44_u5_a_binary_file_and_a_rename_with_no_transcript() {
    let tree = scratch("binary");
    let edit = |tree: &Path| -> std::io::Result<()> {
        fs::write(
            tree.join("src/blob.bin"),
            b"\x00\x1b]0;owned\x07\x1b[2J\xff\xfe\x9b2J",
        )?;
        fs::rename(tree.join("src/old.rs"), tree.join("src/new.rs"))
    };
    let out = ran(&tree, "binary", None, &edit, Console::default());
    let step = committed_step(&out);
    assert!(
        step[1].starts_with("    diff --git "),
        "a line other than the diff follows said: with no transcript:\n{}",
        step.join("\n")
    );
    let joined = step.join("\n");
    assert!(
        !joined
            .chars()
            .any(|c| (c.is_control() && c != '\n') || is_bidi_control(c)),
        "a raw control reaches the terminal:\n{joined:?}"
    );
    assert!(joined.contains("Binary files"), "{joined}");
    assert!(joined.contains("src/new.rs"), "{joined}");
}

/// Story, Outcome 3: "Colour is never written to the case's records (`events.jsonl`,
/// evidence); only the console carries it", and the last paragraph: "The record a run writes is
/// unchanged by this story". With colour on and a transcript holding sequences, no file of the
/// case and no commit message holds an SGR sequence.
#[test]
fn adv_w44_u5_colour_and_listing_reach_no_record_and_no_commit() {
    let tree = scratch("records");
    let edit = |tree: &Path| -> std::io::Result<()> {
        fs::write(
            tree.join("src/lib.rs"),
            "pub fn add() {}\npub fn sub() {}\n",
        )
    };
    let bytes = transcript(&[bash("cat MARKER-CMD")]);
    let out = ran(
        &tree,
        "records",
        Some(bytes),
        &edit,
        Console {
            color: true,
            ..Console::default()
        },
    );
    assert!(out.contains("\u{1b}[32m+pub fn sub() {}\u{1b}[0m"), "{out}");
    assert!(out.contains("MARKER-CMD"), "{out}");
    let mut stack = vec![tree.join(".takt")];
    let mut read = 0;
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("a case directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = String::from_utf8_lossy(&fs::read(&path).expect("a record")).into_owned();
            read += 1;
            for needle in ["\u{1b}", "\\u001b", "\\u{1b}", "[32m", "[2m", "[1m"] {
                assert!(
                    !text.contains(needle),
                    "{} holds {needle:?}",
                    path.display()
                );
            }
        }
    }
    assert!(read > 0, "no record read");
    let log = git(&tree, &["log", "--format=%B"]);
    assert!(!log.contains('\u{1b}') && !log.contains("[32m"), "{log}");
}
