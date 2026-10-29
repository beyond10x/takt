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

/// The columns a title is cut at: one line of at most this many columns of its words, then ` …`
/// when words are left over; a first word longer than that is cut to this many columns and ends
/// with ` …` too. So a title line is at most `TITLE_WIDTH + 2` columns after its prefix.
pub const TITLE_WIDTH: usize = 98;

/// How the console says the frontier: as changes only, or the full `frontier:` line at each
/// decision as well (`takt run --verbose`); whether it lists what each harness step looked at
/// ([`reads`]) and the diff it committed ([`diff`]); and whether it is in colour ([`Paint`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Console {
    /// Whether every decision's full `frontier: <action> (<status>), …` line is printed too.
    pub verbose: bool,
    /// Whether a harness step's tool calls are listed under its `said:` line ([`reads`]).
    pub reads: bool,
    /// Whether the diff a harness step committed is printed before its `effect:` line
    /// ([`diff`]).
    pub diff: bool,
    /// Whether the console is in colour ([`color_on`]).
    pub color: bool,
}

impl Default for Console {
    /// Frontier changes only, both listings, no colour.
    fn default() -> Self {
        Self {
            verbose: false,
            reads: true,
            diff: true,
            color: false,
        }
    }
}

impl Console {
    /// The paint this console's lines are written in: colour when [`Console::color`] is on.
    #[must_use]
    pub fn paint(self) -> Paint {
        Paint { on: self.color }
    }
}

/// When the console is in colour: `takt run --color auto|always|never`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// When standard output is a terminal and `NO_COLOR` is unset or empty.
    #[default]
    Auto,
    /// Always: for a run whose output is piped through `tee` into a terminal.
    Always,
    /// Never.
    Never,
}

/// Whether the console is in colour under `mode`, standard output being a terminal or not
/// (`terminal`) and `NO_COLOR` holding `no_color`: under `auto`, on for a terminal when
/// `NO_COLOR` is unset or empty, as <https://no-color.org> says.
#[must_use]
pub fn color_on(mode: ColorMode, terminal: bool, no_color: Option<&std::ffi::OsStr>) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => terminal && no_color.is_none_or(std::ffi::OsStr::is_empty),
    }
}

/// The columns a listed line is cut at, its indent included: a longer one is cut to two fewer
/// and ends with ` …`.
pub const LINE_WIDTH: usize = 100;

/// The most tool calls a step's listing names ([`reads`]); more are counted, `… <n> more`.
pub const READS_SHOWN: usize = 20;

/// The most lines of a step's diff the console prints ([`diff`]); more are counted, `… <n> more
/// lines (git show <sha>)`.
pub const DIFF_SHOWN: usize = 120;

/// The colours the console writes, as ANSI SGR sequences around a text, or the text alone when
/// off. Text is painted after it is escaped and cut, so no sequence comes from what is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Paint {
    /// Whether any sequence is written.
    pub on: bool,
}

impl Paint {
    /// No colour.
    pub const OFF: Self = Self { on: false };

    fn sgr(self, code: &str, text: &str) -> String {
        if self.on {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_owned()
        }
    }

    /// Green: a pass, an added line.
    #[must_use]
    pub fn green(self, text: &str) -> String {
        self.sgr("32", text)
    }

    /// Red: a fail, a removed line.
    #[must_use]
    pub fn red(self, text: &str) -> String {
        self.sgr("31", text)
    }

    /// Cyan: a hunk header.
    #[must_use]
    pub fn cyan(self, text: &str) -> String {
        self.sgr("36", text)
    }

    /// Dim: the `said:` and `effect:` labels, a tool call.
    #[must_use]
    pub fn dim(self, text: &str) -> String {
        self.sgr("2", text)
    }

    /// Bold: the step line's action, a diff's file header.
    #[must_use]
    pub fn bold(self, text: &str) -> String {
        self.sgr("1", text)
    }
}

/// What a harness step looked at, one line per tool call of its transcript in order, each
/// `  $ <command>` for a shell command (Codex's `exec_command`, Claude's `Bash`) or `  <tool>
/// <path or pattern>` for a file tool (Claude's `Read`, `Grep`, `Glob`, `Edit`, `Write`,
/// Codex's `apply_patch`), a path under the session's working directory relative to it; escaped
/// and cut at [`LINE_WIDTH`] terminal columns, a tab as spaces ([`shown`]); at most
/// [`READS_SHOWN`] and then `  … <n> more`, each dim. A line of the transcript that is no JSON,
/// or no tool call, says nothing.
#[must_use]
pub fn reads(transcript: &[u8], paint: Paint) -> Vec<String> {
    let calls = tool_calls(transcript);
    let mut lines: Vec<String> = calls
        .iter()
        .take(READS_SHOWN)
        .map(|call| format!("  {}", paint.dim(&shown(call, 2))))
        .collect();
    if calls.len() > READS_SHOWN {
        let more = format!("… {} more", calls.len() - READS_SHOWN);
        lines.push(format!("  {}", paint.dim(&more)));
    }
    lines
}

/// `text` as a listed line prints it after an indent of `indent` columns: each tab as spaces to
/// the next multiple of four columns of the printed line, every other part escaped
/// ([`crate::trace::escape`]), then cut to fit [`LINE_WIDTH`] ([`fit`]).
fn shown(text: &str, indent: usize) -> String {
    let mut line = String::new();
    let mut column = indent;
    for (at, part) in text.split('\t').enumerate() {
        if at > 0 {
            let spaces = 4 - column % 4;
            line.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        }
        let part = crate::trace::escape(part);
        column += columns(&part);
        line.push_str(&part);
    }
    fit(&line, indent)
}

/// `text` as it fits after an indent of `indent` columns in [`LINE_WIDTH`] terminal columns
/// ([`columns`]): whole, or cut to at most two columns fewer and ended with ` …`.
fn fit(text: &str, indent: usize) -> String {
    let room = LINE_WIDTH.saturating_sub(indent);
    if columns(text) <= room {
        return text.to_owned();
    }
    let mut cut = String::new();
    let mut used = 0;
    for c in text.chars() {
        used += width(c);
        if used > room.saturating_sub(2) {
            break;
        }
        cut.push(c);
    }
    cut.push_str(" …");
    cut
}

/// The terminal columns `text` takes, a wide character two ([`width`]).
fn columns(text: &str) -> usize {
    text.chars().map(width).sum()
}

/// The terminal columns `c` takes: two for an East Asian wide or fullwidth character or an
/// emoji shown wide (the blocks of Unicode's East Asian Width `W` and `F`, a block counted
/// whole where it is mostly wide, so a cut is at worst early), one for any other.
fn width(c: char) -> usize {
    match u32::from(c) {
        0x1100..=0x115f
        | 0x231a..=0x231b
        | 0x2329..=0x232a
        | 0x23e9..=0x23ec
        | 0x23f0
        | 0x23f3
        | 0x25fd..=0x25fe
        | 0x2614..=0x2615
        | 0x2648..=0x2653
        | 0x267f
        | 0x2693
        | 0x26a1
        | 0x26aa..=0x26ab
        | 0x26bd..=0x26be
        | 0x26c4..=0x26c5
        | 0x26ce
        | 0x26d4
        | 0x26ea
        | 0x26f2..=0x26f3
        | 0x26f5
        | 0x26fa
        | 0x26fd
        | 0x2705
        | 0x270a..=0x270b
        | 0x2728
        | 0x274c
        | 0x274e
        | 0x2753..=0x2755
        | 0x2757
        | 0x2795..=0x2797
        | 0x27b0
        | 0x27bf
        | 0x2b1b..=0x2b1c
        | 0x2b50
        | 0x2b55
        | 0x2e80..=0xa4cf
        | 0xa960..=0xa97f
        | 0xac00..=0xd7a3
        | 0xf900..=0xfaff
        | 0xfe10..=0xfe19
        | 0xfe30..=0xfe6f
        | 0xff00..=0xff60
        | 0xffe0..=0xffe6
        | 0x16fe0..=0x16fe4
        | 0x17000..=0x18cff
        | 0x1b000..=0x1b2ff
        | 0x1f004
        | 0x1f0cf
        | 0x1f18e
        | 0x1f191..=0x1f19a
        | 0x1f200..=0x1f251
        | 0x1f300..=0x1faff
        | 0x20000..=0x3fffd => 2,
        _ => 1,
    }
}

/// Each tool call of a transcript, in order, unescaped, as [`reads`] names it: from Claude's
/// stream-json `assistant` messages (`tool_use` blocks) and Codex's `response_item` events
/// (`custom_tool_call`, `function_call`, `local_shell_call`). The working directory a session
/// states (Claude's `system` event's `cwd`, Codex's `session_meta` or `turn_context` payload's)
/// is taken off a path under it. Nothing else of the transcript is read: not takt's own first
/// line with the prompt, nor a tool's output.
fn tool_calls(transcript: &[u8]) -> Vec<String> {
    let mut cwd: Option<String> = None;
    let mut calls = Vec::new();
    for line in transcript.split(|byte| *byte == b'\n') {
        let Ok(event) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        let text = |value: &serde_json::Value, key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        match event.get("type").and_then(serde_json::Value::as_str) {
            Some("system") => {
                if let Some(dir) = text(&event, "cwd") {
                    cwd = Some(dir);
                }
            }
            Some("session_meta" | "turn_context") => {
                if let Some(dir) = event
                    .get("payload")
                    .and_then(|payload| text(payload, "cwd"))
                {
                    cwd = Some(dir);
                }
            }
            Some("assistant") => {
                let blocks = event
                    .get("message")
                    .and_then(|message| message.get("content"))
                    .and_then(serde_json::Value::as_array);
                for block in blocks.into_iter().flatten() {
                    if block.get("type").and_then(serde_json::Value::as_str) == Some("tool_use") {
                        calls.push(claude_call(block, cwd.as_deref()));
                    }
                }
            }
            Some("response_item") => {
                if let Some(payload) = event.get("payload") {
                    codex_calls(payload, cwd.as_deref(), &mut calls);
                }
            }
            _ => {}
        }
    }
    calls
}

/// `path` relative to `cwd` when it lies under it, else as it is.
fn relative(path: &str, cwd: Option<&str>) -> String {
    let Some(cwd) = cwd
        .map(|cwd| cwd.trim_end_matches('/'))
        .filter(|cwd| !cwd.is_empty())
    else {
        return path.to_owned();
    };
    match path.strip_prefix(cwd) {
        Some("") => ".".to_owned(),
        Some(rest) if rest.starts_with('/') => rest.trim_start_matches('/').to_owned(),
        _ => path.to_owned(),
    }
}

/// One Claude `tool_use` block as [`reads`] names it.
fn claude_call(block: &serde_json::Value, cwd: Option<&str>) -> String {
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool");
    let input = block.get("input");
    let field = |key: &str| {
        input
            .and_then(|input| input.get(key))
            .and_then(serde_json::Value::as_str)
    };
    let path = |key: &str| field(key).map(|path| relative(path, cwd));
    let named = |parts: Vec<Option<String>>| {
        let mut line = name.to_owned();
        for part in parts.into_iter().flatten() {
            line.push(' ');
            line.push_str(&part);
        }
        line
    };
    match name {
        "Bash" => format!("$ {}", field("command").unwrap_or_default()),
        "Read" | "Edit" | "Write" | "MultiEdit" => named(vec![path("file_path")]),
        "NotebookEdit" | "NotebookRead" => named(vec![path("notebook_path")]),
        "Grep" | "Glob" => named(vec![
            field("pattern").map(str::to_owned),
            path("path").map(|path| format!("in {path}")),
        ]),
        "WebFetch" => named(vec![field("url").map(str::to_owned)]),
        "WebSearch" => named(vec![field("query").map(str::to_owned)]),
        _ => name.to_owned(),
    }
}

/// The calls of one Codex `response_item` payload, as [`reads`] names them, onto `calls`: a
/// `custom_tool_call`'s script (`tools.<name>(…)`, each call of it), a `function_call`'s
/// `exec_command` or `shell`, a `local_shell_call`'s command; a patch names each file it
/// touches.
fn codex_calls(payload: &serde_json::Value, cwd: Option<&str>, calls: &mut Vec<String>) {
    let text = |key: &str| payload.get(key).and_then(serde_json::Value::as_str);
    let name = text("name").unwrap_or("tool");
    match text("type") {
        Some("custom_tool_call") => {
            let script = text("input")
                .or_else(|| text("arguments"))
                .unwrap_or_default();
            if name == "apply_patch" {
                patched(script, cwd, calls);
            } else {
                // A freeform input that holds no `tools.` call is still one call, by its name.
                let before = calls.len();
                script_calls(script, cwd, calls);
                if calls.len() == before {
                    calls.push(name.to_owned());
                }
            }
        }
        Some("function_call") => {
            let arguments: serde_json::Value = text("arguments")
                .and_then(|arguments| serde_json::from_str(arguments).ok())
                .unwrap_or_default();
            match name {
                "exec_command" | "shell" | "container.exec" => {
                    let command = arguments.get("cmd").or_else(|| arguments.get("command"));
                    calls.push(format!("$ {}", command.map(shell_line).unwrap_or_default()));
                }
                "apply_patch" => patched(
                    arguments
                        .get("input")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                    cwd,
                    calls,
                ),
                _ => calls.push(name.to_owned()),
            }
        }
        Some("local_shell_call") => {
            let command = payload
                .get("action")
                .and_then(|action| action.get("command"));
            calls.push(format!("$ {}", command.map(shell_line).unwrap_or_default()));
        }
        _ => {}
    }
}

/// A command as a string, or as an argument vector: `bash -lc <script>` is its script, any
/// other vector its words joined by a space.
fn shell_line(command: &serde_json::Value) -> String {
    if let Some(line) = command.as_str() {
        return line.to_owned();
    }
    let words: Vec<&str> = command
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .collect();
    match words.as_slice() {
        [shell, flag, script] if shell.ends_with("sh") && flag.starts_with('-') => {
            (*script).to_owned()
        }
        _ => words.join(" "),
    }
}

/// Each file a patch in Codex's `apply_patch` form touches, as `apply_patch <path>`, onto
/// `calls`; a patch that names none is `apply_patch`.
fn patched(patch: &str, cwd: Option<&str>, calls: &mut Vec<String>) {
    let before = calls.len();
    for line in patch.lines() {
        let path = ["*** Update File: ", "*** Add File: ", "*** Delete File: "]
            .iter()
            .find_map(|marker| line.strip_prefix(marker));
        if let Some(path) = path {
            calls.push(format!("apply_patch {}", relative(path.trim(), cwd)));
        }
    }
    if calls.len() == before {
        calls.push("apply_patch".to_owned());
    }
}

/// The calls of a Codex script, `tools.<name>(…)` each, in order, onto `calls`: an
/// `exec_command` by its `cmd`, an `apply_patch` by the files it touches, another by its name.
/// A string literal of the script is skipped whole, so text inside a command never reads as a
/// call; so is a `//` or `/* */` comment, so a quote inside one opens no string.
fn script_calls(script: &str, cwd: Option<&str>, calls: &mut Vec<String>) {
    let chars: Vec<char> = script.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        if let Some(end) = comment_end(&chars, at) {
            at = end;
            continue;
        }
        if matches!(chars[at], '"' | '\'' | '`') {
            at = literal(&chars, at).map_or(chars.len(), |(_, end)| end);
            continue;
        }
        let prefix: Vec<char> = "tools.".chars().collect();
        let starts_word = at == 0 || !is_word(chars[at - 1]);
        if !(starts_word && chars[at..].starts_with(&prefix)) {
            at += 1;
            continue;
        }
        let from = at + prefix.len();
        let mut end = from;
        while end < chars.len() && is_word(chars[end]) {
            end += 1;
        }
        let name: String = chars[from..end].iter().collect();
        let open = skip_space(&chars, end);
        if name.is_empty() || chars.get(open) != Some(&'(') {
            at = end.max(at + 1);
            continue;
        }
        let argument = skip_space(&chars, open + 1);
        match name.as_str() {
            "exec_command" => match command_of(&chars, argument) {
                Some(command) => calls.push(format!("$ {command}")),
                None => calls.push(name),
            },
            "apply_patch" => match literal(&chars, argument) {
                Some((patch, _)) => patched(&patch, cwd, calls),
                None => calls.push(name),
            },
            _ => calls.push(name),
        }
        at = open + 1;
    }
}

/// Whether `c` may be part of a script's name.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// The first index from `at` that is neither white space nor a comment ([`comment_end`]).
fn skip_space(chars: &[char], mut at: usize) -> usize {
    loop {
        if chars.get(at).is_some_and(|c| c.is_whitespace()) {
            at += 1;
        } else if let Some(end) = comment_end(chars, at) {
            at = end;
        } else {
            return at;
        }
    }
}

/// Where the comment of a script that starts at `at` ends: after the line of a `//` comment
/// (its line break left), after the `*/` of a `/* */` one, or at the script's end when that
/// never comes; none when no comment starts at `at`.
fn comment_end(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&'/') {
        return None;
    }
    let rest = at + 2;
    match chars.get(at + 1)? {
        '/' => Some(
            chars[rest..]
                .iter()
                .position(|c| *c == '\n')
                .map_or(chars.len(), |n| rest + n),
        ),
        '*' => Some(
            chars[rest..]
                .windows(2)
                .position(|pair| pair == ['*', '/'])
                .map_or(chars.len(), |n| rest + n + 2),
        ),
        _ => None,
    }
}

/// The `cmd` of the object literal at `at` (`{cmd:"…", …}`, its keys bare or quoted, in any
/// order): none when it is no object, or holds no `cmd` that is a string literal.
fn command_of(chars: &[char], at: usize) -> Option<String> {
    if chars.get(at) != Some(&'{') {
        return None;
    }
    let mut at = at + 1;
    loop {
        at = skip_space(chars, at);
        let key = match chars.get(at)? {
            '}' => return None,
            '"' | '\'' => {
                let (key, end) = literal(chars, at)?;
                at = end;
                key
            }
            _ => {
                let start = at;
                while chars.get(at).is_some_and(|c| is_word(*c)) {
                    at += 1;
                }
                if at == start {
                    return None;
                }
                chars[start..at].iter().collect()
            }
        };
        at = skip_space(chars, at);
        if chars.get(at) != Some(&':') {
            return None;
        }
        at = skip_space(chars, at + 1);
        if key == "cmd" {
            return literal(chars, at).map(|(command, _)| command);
        }
        // Another key's value: skipped to the comma or the brace that closes the object, its
        // literals and nested brackets whole.
        let mut depth = 0_usize;
        loop {
            if let Some(end) = comment_end(chars, at) {
                at = end;
                continue;
            }
            match chars.get(at)? {
                '"' | '\'' | '`' => {
                    at = literal(chars, at)?.1;
                    continue;
                }
                '{' | '[' | '(' => depth += 1,
                '}' | ']' | ')' if depth > 0 => depth -= 1,
                '}' => return None,
                ',' if depth == 0 => {
                    at += 1;
                    break;
                }
                _ => {}
            }
            at += 1;
        }
    }
}

/// The string literal of a script at `at` (quoted `"`, `'` or `` ` ``), its escapes decoded,
/// and the index after its closing quote; none when no literal starts there or it does not
/// close.
fn literal(chars: &[char], at: usize) -> Option<(String, usize)> {
    let quote = *chars.get(at)?;
    if !matches!(quote, '"' | '\'' | '`') {
        return None;
    }
    let mut text = String::new();
    let mut at = at + 1;
    loop {
        let c = *chars.get(at)?;
        at += 1;
        if c == quote {
            return Some((text, at));
        }
        if c != '\\' {
            text.push(c);
            continue;
        }
        let escaped = *chars.get(at)?;
        at += 1;
        let hex = |from: usize, to: usize| -> Option<char> {
            let digits: String = chars.get(from..to)?.iter().collect();
            char::from_u32(u32::from_str_radix(&digits, 16).ok()?)
        };
        match escaped {
            'n' => text.push('\n'),
            't' => text.push('\t'),
            'r' => text.push('\r'),
            'b' => text.push('\u{8}'),
            'f' => text.push('\u{c}'),
            'v' => text.push('\u{b}'),
            '0' => text.push('\0'),
            '\n' => {}
            'x' => {
                text.push(hex(at, at + 2).unwrap_or('\u{fffd}'));
                at += 2;
            }
            'u' if chars.get(at) == Some(&'{') => {
                let close = chars[at..].iter().position(|c| *c == '}')? + at;
                text.push(hex(at + 1, close).unwrap_or('\u{fffd}'));
                at = close + 1;
            }
            'u' => {
                text.push(hex(at, at + 4).unwrap_or('\u{fffd}'));
                at += 4;
            }
            other => text.push(other),
        }
    }
}

/// The diff a step committed as the console prints it: each line of `diff` (`git diff`'s
/// unified output) indented four spaces, escaped ([`crate::trace::escape`]) and cut at
/// [`LINE_WIDTH`] terminal columns, a tab as spaces ([`shown`]); an added line green, a removed
/// one red, a hunk header cyan, and a file header bold: a `diff --git` line, and a `--- ` or
/// `+++ ` line only between it and its first `@@`, since inside a hunk such a line is a removed
/// or an added one. At most [`DIFF_SHOWN`], then `    … <n> more lines (git show <sha>)`. An
/// empty diff is no line.
#[must_use]
pub fn diff(diff: &str, sha: &str, paint: Paint) -> Vec<String> {
    let all: Vec<&str> = diff.lines().collect();
    let mut in_header = false;
    let mut lines: Vec<String> = all
        .iter()
        .take(DIFF_SHOWN)
        .map(|line| {
            let text = shown(line, 4);
            let painted = if line.starts_with("diff --git ") {
                in_header = true;
                paint.bold(&text)
            } else if in_header && (line.starts_with("--- ") || line.starts_with("+++ ")) {
                paint.bold(&text)
            } else if line.starts_with("@@") {
                in_header = false;
                paint.cyan(&text)
            } else if line.starts_with('+') {
                paint.green(&text)
            } else if line.starts_with('-') {
                paint.red(&text)
            } else {
                text
            };
            format!("    {painted}")
        })
        .collect();
    if all.len() > DIFF_SHOWN {
        lines.push(format!(
            "    … {} more lines (git show {})",
            all.len() - DIFF_SHOWN,
            crate::trace::escape(sha)
        ));
    }
    lines
}

/// The title of an intent as the header prints it: its first line that is not blank, a heading's
/// `#` marks and the spaces around it taken off, each word escaped ([`crate::trace::escape`], so
/// no control or bidi character reaches the console) and the words bounded at [`TITLE_WIDTH`]. A
/// story's `intent.md` starts with the story's title.
#[must_use]
pub fn title(intent: &str) -> String {
    let line = intent
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches('#').trim())
        .unwrap_or_default();
    let Some(first) = wrap(&escaped_words(line), TITLE_WIDTH, 1)
        .into_iter()
        .next()
    else {
        return String::new();
    };
    if first.chars().count() <= TITLE_WIDTH + 2 {
        return first;
    }
    let mut cut: String = first.chars().take(TITLE_WIDTH).collect();
    cut.push_str(" …");
    cut
}

/// `text`'s words, each escaped ([`crate::trace::escape`]), joined by one space. A word holds no
/// whitespace and its escape adds none, so wrapping the result measures what is printed.
fn escaped_words(text: &str) -> String {
    text.split_whitespace()
        .map(crate::trace::escape)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The summary of an intent, at most [`SUMMARY_LINES`] lines wrapped at [`SUMMARY_WIDTH`]: the
/// first paragraph under its `## Outcome` heading, or, when it has none, its first paragraph
/// after the title. A paragraph is a run of lines that are neither blank nor a heading; none is
/// no line. Each word is escaped before the wrap ([`crate::trace::escape`]), so no control or
/// bidi character reaches the console and the width is that of the printed text.
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
    wrap(
        &escaped_words(&paragraph.join(" ")),
        SUMMARY_WIDTH,
        SUMMARY_LINES,
    )
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
/// under the first indented two spaces. The title and the summary come escaped and bounded; the
/// story id is escaped here.
#[must_use]
pub fn header(version: &str, story: Option<&str>, intent: &str) -> Vec<String> {
    let title = title(intent);
    let story = story.map(crate::trace::escape);
    let mut lines = match story.as_deref() {
        Some(story) if title.is_empty() => vec![format!("takt {version} · {story}")],
        Some(story) => vec![format!("takt {version} · {story}"), format!("  {title}")],
        None => vec![format!("takt {version} · {title}")],
    };
    lines.extend(summary(intent).into_iter().map(|line| format!("  {line}")));
    lines
}

/// The header's harness line: `  harness <name>[ <version>] · confinement <name>`, each part
/// escaped ([`crate::trace::escape`]): the version is what a harness program printed.
#[must_use]
pub fn harness_line(harness: &str, version: Option<&str>, confinement: &str) -> String {
    let harness = crate::trace::escape(harness);
    let version = version.map(crate::trace::escape);
    let confinement = crate::trace::escape(confinement);
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
/// <tokens> · <cost>`, each only when the call reported it; nothing when it reported none. The
/// tokens name how many of them were input read from the provider's cache (`cached`), when
/// any were: a session re-reads its context on every request, so most of a long step's tokens
/// are cached input, which costs a fraction of the rest.
#[must_use]
pub fn harness_figures(
    duration_ms: Option<u64>,
    tokens_used: Option<u64>,
    cached: Option<u64>,
    cost_usd: Option<f64>,
) -> String {
    let parts = [
        duration_ms.map(duration),
        tokens_used.map(|used| match cached.filter(|cached| *cached > 0) {
            Some(cached) => format!(
                "{} ({} cached)",
                tokens(used),
                tokens(cached).trim_end_matches(" tok")
            ),
            None => tokens(used),
        }),
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

/// [`validator_figures`] in `paint`: a pass green, a fail and what failed red.
#[must_use]
pub fn validator_figures_in(
    result: Option<&str>,
    failed: &[String],
    duration_ms: u64,
    paint: Paint,
) -> String {
    let plain = validator_figures(result, failed, duration_ms);
    let Some(said) = plain.strip_prefix(" · ") else {
        return plain;
    };
    let took = format!(" · {}", duration(duration_ms));
    match (result, said.strip_suffix(took.as_str())) {
        (Some("pass"), Some(said)) => format!(" · {}{took}", paint.green(said)),
        (Some("fail"), Some(said)) => format!(" · {}{took}", paint.red(said)),
        _ => plain,
    }
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

    /// A transcript as takt keeps it: its own first line (whose prompt names a call no tool
    /// made), then `lines`, one JSON value each.
    fn transcript(lines: &[serde_json::Value]) -> Vec<u8> {
        let mut bytes = serde_json::json!({
            "context": [],
            "prompt": "text(await tools.exec_command({cmd:\"rm -rf /\"}));",
            "step": 3,
            "type": "takt",
        })
        .to_string()
        .into_bytes();
        bytes.push(b'\n');
        for line in lines {
            bytes.extend_from_slice(line.to_string().as_bytes());
            bytes.push(b'\n');
        }
        bytes.extend_from_slice(b"not json at all\n");
        bytes
    }

    /// One Codex `custom_tool_call` whose script is `input`.
    fn codex_call(input: &str) -> serde_json::Value {
        serde_json::json!({
            "timestamp": "2026-10-29T08:00:00Z",
            "type": "response_item",
            "payload": {"type": "custom_tool_call", "name": "exec", "input": input},
        })
    }

    /// One Claude stream-json `assistant` message holding `uses`, each `(name, input)`.
    fn claude_uses(uses: &[(&str, serde_json::Value)]) -> serde_json::Value {
        let content: Vec<serde_json::Value> = uses
            .iter()
            .map(|(name, input)| {
                serde_json::json!({"type": "tool_use", "id": "t", "name": name, "input": input})
            })
            .collect();
        serde_json::json!({"type": "assistant", "message": {"content": content}})
    }

    /// A Codex step's transcript holding two shell commands, a tool's output between them.
    fn codex_two_commands() -> Vec<u8> {
        transcript(&[
            serde_json::json!({"type": "session_meta", "payload": {"cwd": "/w/copy"}}),
            codex_call(
                "text(await tools.exec_command({cmd:\"cat src/lib.rs\",\"max_output_tokens\":1000}));\n",
            ),
            serde_json::json!({"type": "response_item", "payload": {
                "type": "custom_tool_call_output",
                "output": "tools.exec_command({cmd:\"not a call\"})",
            }}),
            codex_call("text(await tools.exec_command({cmd:\"git status --short\"}));\n"),
        ])
    }

    /// A Claude step's transcript holding two shell commands.
    fn claude_two_commands() -> Vec<u8> {
        transcript(&[
            serde_json::json!({"type": "system", "subtype": "init", "cwd": "/w/copy"}),
            claude_uses(&[("Bash", serde_json::json!({"command": "cat src/lib.rs"}))]),
            serde_json::json!({"type": "user", "message": {"content": [
                {"type": "tool_result", "content": "pub fn add() {}"},
            ]}}),
            claude_uses(&[("Bash", serde_json::json!({"command": "git status --short"}))]),
        ])
    }

    const ONE_FILE_DIFF: &str = "diff --git a/src/lib.rs b/src/lib.rs\nindex 1111111..2222222 \
                                 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1,2 @@\n \
                                 pub fn add() {}\n+pub fn sub() {}\n";

    /// Whether `line` holds a character a terminal acts on: a C0 or C1 control (ESC among
    /// them), or a bidi control.
    fn has_control(line: &str) -> bool {
        line.chars().any(|c| {
            c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
    }

    /// Acceptance 1, Codex: both shell commands, in order, and nothing from the prompt or a
    /// tool's output.
    #[test]
    fn a_codex_steps_two_shell_commands_are_listed_in_order() {
        assert_eq!(
            reads(&codex_two_commands(), Paint::OFF),
            owned(&["  $ cat src/lib.rs", "  $ git status --short"])
        );
    }

    /// Acceptance 1, Claude: both `Bash` commands, in order.
    #[test]
    fn a_claude_steps_two_shell_commands_are_listed_in_order() {
        assert_eq!(
            reads(&claude_two_commands(), Paint::OFF),
            owned(&["  $ cat src/lib.rs", "  $ git status --short"])
        );
    }

    /// Acceptance 1: the diff of the one file the step committed, its added line among it.
    #[test]
    fn a_one_file_diff_is_printed_indented_and_whole() {
        let lines = diff(ONE_FILE_DIFF, "abcdef12", Paint::OFF);
        assert_eq!(
            lines,
            owned(&[
                "    diff --git a/src/lib.rs b/src/lib.rs",
                "    index 1111111..2222222 100644",
                "    --- a/src/lib.rs",
                "    +++ b/src/lib.rs",
                "    @@ -1 +1,2 @@",
                "     pub fn add() {}",
                "    +pub fn sub() {}",
            ])
        );
        assert!(diff("", "abcdef12", Paint::OFF).is_empty());
    }

    /// Several calls in one Codex script are each listed; a patch names the files it touches,
    /// relative to the session's working directory; another tool is named.
    #[test]
    fn a_codex_script_lists_each_call_and_a_patch_its_files() {
        let script = "text(await tools.exec_command({cmd:\"cat src/lib.rs Cargo.toml\"}));\n\
                      text(await tools.exec_command({\"max_output_tokens\":10,cmd:'ls -a'}));\n\
                      text(await tools.apply_patch(\"*** Begin Patch\\n*** Update File: \
                      /w/copy/src/lib.rs\\n@@\\n-a\\n+b\\n*** Add File: docs/x.md\\n+x\\n*** End \
                      Patch\\n\"));\ntext(await tools.mcp__logs__get_profile({}));\n";
        let bytes = transcript(&[
            serde_json::json!({"type": "turn_context", "payload": {"cwd": "/w/copy"}}),
            codex_call(script),
        ]);
        assert_eq!(
            reads(&bytes, Paint::OFF),
            owned(&[
                "  $ cat src/lib.rs Cargo.toml",
                "  $ ls -a",
                "  apply_patch src/lib.rs",
                "  apply_patch docs/x.md",
                "  mcp__logs__get_profile",
            ])
        );
    }

    /// Claude's file tools are named with their path or pattern, relative to the session's
    /// working directory; another tool by its name.
    #[test]
    fn claudes_file_tools_name_their_path_or_pattern() {
        let bytes = transcript(&[
            serde_json::json!({"type": "system", "subtype": "init", "cwd": "/w/copy"}),
            claude_uses(&[
                (
                    "Read",
                    serde_json::json!({"file_path": "/w/copy/src/lib.rs"}),
                ),
                (
                    "Grep",
                    serde_json::json!({"pattern": "fn sub", "path": "/w/copy/src"}),
                ),
                ("Glob", serde_json::json!({"pattern": "**/*.rs"})),
                (
                    "Edit",
                    serde_json::json!({"file_path": "/w/copy/src/lib.rs", "old_string": "a"}),
                ),
                ("Write", serde_json::json!({"file_path": "/elsewhere/x"})),
                ("TodoWrite", serde_json::json!({"todos": []})),
            ]),
        ]);
        assert_eq!(
            reads(&bytes, Paint::OFF),
            owned(&[
                "  Read src/lib.rs",
                "  Grep fn sub in src",
                "  Glob **/*.rs",
                "  Edit src/lib.rs",
                "  Write /elsewhere/x",
                "  TodoWrite",
            ])
        );
    }

    /// Acceptance 2: a command holding `ESC [2J` or U+202E, raw in the transcript or written as
    /// a script's escape, prints both escaped, and no line carries a control character.
    #[test]
    fn a_commands_terminal_escape_and_bidi_override_print_escaped() {
        let raw = "printf '\u{1b}[2J' # \u{202e}txt.exe";
        let codex = transcript(&[codex_call(
            "text(await tools.exec_command({cmd:\"printf '\\u001b[2J' # \\u202etxt.exe\"}));",
        )]);
        let claude = transcript(&[claude_uses(&[(
            "Bash",
            serde_json::json!({"command": raw}),
        )])]);
        for bytes in [codex, claude] {
            let lines = reads(&bytes, Paint { on: true });
            assert_eq!(lines.len(), 1, "{lines:?}");
            let plain = reads(&bytes, Paint::OFF);
            assert_eq!(
                plain,
                owned(&["  $ printf '\\u{1b}[2J' # \\u{202e}txt.exe"]),
                "{plain:?}"
            );
            assert!(!has_control(&plain[0]), "{plain:?}");
            // In colour, the only escape is the paint's own, around the escaped text.
            assert_eq!(lines[0], format!("  \u{1b}[2m{}\u{1b}[0m", &plain[0][2..]));
        }
        let patched = diff("+say \u{1b}[2J \u{202e}x\n", "abcdef12", Paint::OFF);
        assert_eq!(patched, owned(&["    +say \\u{1b}[2J \\u{202e}x"]));
    }

    /// At most twenty calls, then how many more; a long command cut at the line width.
    #[test]
    fn the_listing_is_bounded_in_lines_and_width() {
        let uses: Vec<(&str, serde_json::Value)> = (0..25)
            .map(|n| ("Bash", serde_json::json!({"command": format!("echo {n}")})))
            .collect();
        let lines = reads(&transcript(&[claude_uses(&uses)]), Paint::OFF);
        assert_eq!(lines.len(), READS_SHOWN + 1, "{lines:?}");
        assert_eq!(lines[READS_SHOWN - 1], "  $ echo 19");
        assert_eq!(lines[READS_SHOWN], "  … 5 more");
        let long = "x".repeat(300);
        let cut = reads(
            &transcript(&[claude_uses(&[(
                "Bash",
                serde_json::json!({"command": long}),
            )])]),
            Paint::OFF,
        );
        assert_eq!(cut, vec![format!("  $ {} …", "x".repeat(LINE_WIDTH - 6))]);
        assert_eq!(cut[0].chars().count(), LINE_WIDTH);
    }

    /// Decision (wave 44, review): a tab in a listed command or a diff line prints as spaces to
    /// the next multiple of four columns of the printed line, never as `\u{9}`; every other
    /// control character stays escaped.
    #[test]
    fn a_tab_prints_as_spaces_to_the_next_multiple_of_four_columns() {
        let listed = reads(
            &transcript(&[claude_uses(&[(
                "Bash",
                serde_json::json!({"command": "a\tb\tcd\u{7}\te"}),
            )])]),
            Paint::OFF,
        );
        // `  $ a` ends at column 5: three spaces to 8; `b` to 9: three to 12; `cd\u{7}`, seven
        // columns escaped, to 19: one to 20.
        assert_eq!(listed, owned(&["  $ a   b   cd\\u{7} e"]));
        let at_start = reads(
            &transcript(&[claude_uses(&[(
                "Bash",
                serde_json::json!({"command": "\tx"}),
            )])]),
            Paint::OFF,
        );
        assert_eq!(at_start, owned(&["  $     x"]));
        let patched = diff("+\tfn x() {}\n \t\tkeep\n", "abcdef12", Paint::OFF);
        assert_eq!(patched, owned(&["    +   fn x() {}", "            keep"]));
        assert!(patched.iter().all(|line| !line.contains('\t')));
    }

    /// Decision (wave 44, review): a line is cut at [`LINE_WIDTH`] terminal columns, a wide
    /// character counting two; ` …` ends a cut line.
    #[test]
    fn a_wide_character_counts_two_columns_in_the_cut() {
        assert_eq!(columns("a漢🎉"), 5);
        let wide = "漢".repeat(60);
        let lines = reads(
            &transcript(&[claude_uses(&[(
                "Bash",
                serde_json::json!({"command": wide}),
            )])]),
            Paint::OFF,
        );
        // `  $ ` is four columns; 47 characters of two make 98 with ` …` after them.
        assert_eq!(lines, vec![format!("  $ {} …", "漢".repeat(47))]);
        assert_eq!(columns(&lines[0]), LINE_WIDTH);
        let fits = "漢".repeat(48);
        assert_eq!(
            reads(
                &transcript(&[claude_uses(&[(
                    "Bash",
                    serde_json::json!({"command": fits})
                )])]),
                Paint::OFF,
            ),
            vec![format!("  $ {fits}")]
        );
    }

    /// Acceptance 4: a diff over 120 lines ends with how many more and where to read them.
    #[test]
    fn a_diff_over_its_bound_ends_with_how_many_more_lines() {
        let long: String = (0..150).map(|n| format!("+line {n}\n")).collect();
        let lines = diff(&long, "abcdef12", Paint::OFF);
        assert_eq!(lines.len(), DIFF_SHOWN + 1, "{lines:?}");
        assert_eq!(lines[DIFF_SHOWN - 1], "    +line 119");
        assert_eq!(lines[DIFF_SHOWN], "    … 30 more lines (git show abcdef12)");
        let exact: String = (0..DIFF_SHOWN).map(|n| format!("+line {n}\n")).collect();
        assert_eq!(diff(&exact, "abcdef12", Paint::OFF).len(), DIFF_SHOWN);
        let wide = format!("+{}\n", "y".repeat(300));
        let cut = diff(&wide, "abcdef12", Paint::OFF);
        assert_eq!(cut[0].chars().count(), LINE_WIDTH, "{cut:?}");
        assert!(cut[0].ends_with(" …"), "{cut:?}");
    }

    /// Acceptance 3: `always` paints an added line green, a removed one red, a hunk header
    /// cyan; without colour no line holds an escape sequence.
    #[test]
    fn colour_paints_the_diff_and_none_writes_no_sequence() {
        let on = Paint { on: true };
        let painted = diff(
            &format!("{ONE_FILE_DIFF}-pub fn gone() {{}}\n"),
            "abcdef12",
            on,
        );
        assert!(
            painted.contains(&"    \u{1b}[32m+pub fn sub() {}\u{1b}[0m".to_owned()),
            "{painted:?}"
        );
        assert!(
            painted.contains(&"    \u{1b}[31m-pub fn gone() {}\u{1b}[0m".to_owned()),
            "{painted:?}"
        );
        assert!(
            painted.contains(&"    \u{1b}[36m@@ -1 +1,2 @@\u{1b}[0m".to_owned()),
            "{painted:?}"
        );
        assert!(
            painted.contains(&"    \u{1b}[1m+++ b/src/lib.rs\u{1b}[0m".to_owned()),
            "{painted:?}"
        );
        assert_eq!(on.green("pass"), "\u{1b}[32mpass\u{1b}[0m");
        assert_eq!(on.dim("said:"), "\u{1b}[2msaid:\u{1b}[0m");
        let off = Paint::OFF;
        let plain: Vec<String> = diff(ONE_FILE_DIFF, "abcdef12", off)
            .into_iter()
            .chain(reads(&codex_two_commands(), off))
            .chain(reads(&claude_two_commands(), off))
            .chain([
                off.green("pass"),
                off.red("fail"),
                off.dim("said:"),
                off.bold("x"),
            ])
            .collect();
        assert!(
            plain.iter().all(|line| !line.contains('\u{1b}')),
            "{plain:?}"
        );
        assert_eq!(
            validator_figures_in(Some("pass"), &[], 100, on),
            " · \u{1b}[32mpass\u{1b}[0m · 0.1s"
        );
        assert_eq!(
            validator_figures_in(Some("fail"), &owned(&["fmt"]), 100, on),
            " · \u{1b}[31mfail: fmt\u{1b}[0m · 0.1s"
        );
        assert_eq!(
            validator_figures_in(Some("fail"), &owned(&["fmt"]), 100, off),
            validator_figures(Some("fail"), &owned(&["fmt"]), 100)
        );
    }

    /// Acceptance 3: `never` is never colour; `auto` is colour on a terminal unless `NO_COLOR`
    /// is set and not empty; `always` is colour, piped or not.
    #[test]
    fn colour_follows_the_mode_the_terminal_and_no_color() {
        let set = Some(std::ffi::OsStr::new("1"));
        let empty = Some(std::ffi::OsStr::new(""));
        assert!(!color_on(ColorMode::Never, true, None));
        assert!(!color_on(ColorMode::Auto, true, set));
        assert!(!color_on(ColorMode::Auto, false, None));
        assert!(color_on(ColorMode::Auto, true, None));
        assert!(color_on(ColorMode::Auto, true, empty));
        assert!(color_on(ColorMode::Always, false, set));
        let console = Console::default();
        assert!(console.reads && console.diff && !console.color && !console.verbose);
        assert_eq!(console.paint(), Paint::OFF);
    }

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
            harness_figures(Some(26_100), Some(65_700), None, Some(0.16)),
            " · 26.1s · 65.7k tok · $0.16"
        );
        assert_eq!(harness_figures(None, Some(12), Some(0), None), " · 12 tok");
        assert_eq!(
            harness_figures(None, Some(630_200), Some(611_000), None),
            " · 630.2k tok (611.0k cached)"
        );
        assert_eq!(harness_figures(None, None, Some(5), None), "");
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
    fn a_long_title_is_one_line_bounded_at_the_title_width() {
        let words = "word ".repeat(100);
        let bounded = title(&words);
        assert!(bounded.ends_with(" …"), "{bounded}");
        assert!(bounded.chars().count() <= TITLE_WIDTH + 2, "{bounded}");
        let one = "x".repeat(500);
        let cut = title(&format!("# {one}"));
        assert_eq!(cut, format!("{} …", "x".repeat(TITLE_WIDTH)));
        assert_eq!(title("Fix \u{1b}[2Jit"), "Fix \\u{1b}[2Jit");
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
