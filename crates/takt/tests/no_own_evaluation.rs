//! `crates/takt/src` holds none of the evaluation code loom-sdk replaced (story
//! takt-evaluation-code-retired): no `b10x_canon::eval` path, no `decision.rs` and no
//! `evaluate` method of `CaseDir`.
//!
//! One exception, held by approval-record:loop-pre-checks-ahead-of-loom until Loom's documented
//! contract changes: the loop's two checks ahead of each Loom step, `Shape::only_behind` and
//! `Shape::unperformable`, read the case's decision, and `CaseDir::evaluate` is what hands it to
//! them. It is allowed while, and only while, `run.rs` runs both checks over the decision and
//! the loop's one call is the only one in the library.

use std::path::{Path, PathBuf};

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.expect("an entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

/// The source with every comment, string and char literal blanked to spaces, lines kept, so
/// that what is left is code.
fn code(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let blank = |out: &mut Vec<u8>, byte: u8| out.push(if byte == b'\n' { b'\n' } else { b' ' });
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        if rest.starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                blank(&mut out, bytes[i]);
                i += 1;
            }
        } else if rest.starts_with(b"/*") {
            let mut depth = 0usize;
            while i < bytes.len() {
                if bytes[i..].starts_with(b"/*") {
                    depth += 1;
                    out.extend_from_slice(b"  ");
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    depth -= 1;
                    out.extend_from_slice(b"  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    blank(&mut out, bytes[i]);
                    i += 1;
                }
            }
        } else if (rest.starts_with(b"r\"") || rest.starts_with(b"r#"))
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
        {
            let hashes = rest[1..].iter().take_while(|&&b| b == b'#').count();
            if rest.get(1 + hashes) != Some(&b'"') {
                out.push(bytes[i]);
                i += 1;
                continue;
            }
            let mut close = vec![b'"'];
            close.extend(std::iter::repeat_n(b'#', hashes));
            out.extend(std::iter::repeat_n(b' ', 2 + hashes));
            i += 2 + hashes;
            while i < bytes.len() && !bytes[i..].starts_with(&close) {
                blank(&mut out, bytes[i]);
                i += 1;
            }
            out.extend(std::iter::repeat_n(b' ', close.len().min(bytes.len() - i)));
            i += close.len();
        } else if bytes[i] == b'"' {
            out.push(b' ');
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    blank(&mut out, bytes[i]);
                    i += 1;
                }
                if i < bytes.len() {
                    blank(&mut out, bytes[i]);
                    i += 1;
                }
            }
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'\'' {
            // A char literal is blanked; a lifetime is kept.
            let end = if rest.get(1) == Some(&b'\\') {
                rest.iter().skip(2).position(|&b| b == b'\'').map(|n| n + 2)
            } else {
                let width = text[i + 1..].chars().next().map_or(1, char::len_utf8);
                (rest.get(1 + width) == Some(&b'\'')).then_some(1 + width)
            };
            match end {
                Some(end) => {
                    out.extend(std::iter::repeat_n(b' ', end + 1));
                    i += end + 1;
                }
                None => {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_at(code: &str, at: usize, word: &str) -> bool {
    !code[..at].chars().next_back().is_some_and(ident)
        && !code[at + word.len()..].chars().next().is_some_and(ident)
}

fn line_of(code: &str, at: usize) -> usize {
    code[..at].matches('\n').count() + 1
}

/// Each `b10x_canon::eval` path: written out, or `eval` inside a `b10x_canon::{…}` group.
fn canon_eval(code: &str) -> Vec<usize> {
    let mut found = Vec::new();
    for (at, _) in code.match_indices("b10x_canon") {
        if !word_at(code, at, "b10x_canon") {
            continue;
        }
        let rest: String = code[at + "b10x_canon".len()..]
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(4096)
            .collect();
        let Some(path) = rest.strip_prefix("::") else {
            continue;
        };
        if path
            .strip_prefix("eval")
            .is_some_and(|after| !after.starts_with(ident))
        {
            found.push(line_of(code, at));
        } else if let Some(group) = path.strip_prefix('{') {
            let mut depth = 1;
            let end = group
                .char_indices()
                .find(|&(_, c)| {
                    match c {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                })
                .map_or(group.len(), |(n, _)| n);
            if group[..end]
                .split(|c: char| !ident(c))
                .any(|word| word == "eval")
            {
                found.push(line_of(code, at));
            }
        }
    }
    found
}

/// The lines of each method named `evaluate` in an `impl` block of `CaseDir`.
fn case_dir_evaluate(code: &str) -> Vec<usize> {
    let mut found = Vec::new();
    for (at, _) in code.match_indices("impl") {
        if !word_at(code, at, "impl") {
            continue;
        }
        let Some(open) = code[at..].find('{').map(|n| at + n) else {
            continue;
        };
        let header = &code[at..open];
        let header = &header[..header.find(" where ").unwrap_or(header.len())];
        let target = header.rsplit(" for ").next().unwrap_or(header);
        if !target
            .split(|c: char| !ident(c))
            .any(|word| word == "CaseDir")
        {
            continue;
        }
        let mut depth = 0usize;
        let mut close = code.len();
        for (n, c) in code[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + n;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &code[open..close];
        for (n, _) in body.match_indices("fn") {
            if !word_at(body, n, "fn") {
                continue;
            }
            let name = body[n + 2..].trim_start();
            if name
                .strip_prefix("evaluate")
                .is_some_and(|after| !after.starts_with(ident))
            {
                found.push(line_of(code, open + n));
            }
        }
    }
    found
}

/// The calls of `CaseDir`'s `evaluate`: each method call `.evaluate`, and each path
/// `CaseDir::evaluate` or `Self::evaluate`. A free function of another crate named `evaluate`
/// (`canon_engineering_assertions::evaluate`, an assertions bundle's check) is not one.
fn evaluate_calls(code: &str) -> usize {
    code.match_indices("evaluate")
        .filter(|&(at, _)| word_at(code, at, "evaluate"))
        .filter(|&(at, _)| {
            let before = code[..at].trim_end();
            if before.ends_with('.') {
                return true;
            }
            let Some(path) = before.strip_suffix("::") else {
                return false;
            };
            let owner: String = path
                .chars()
                .rev()
                .take_while(|&c| ident(c))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            owner == "CaseDir" || owner == "Self"
        })
        .count()
}

/// Whether the approved exception holds: `run.rs` runs both pre-checks over the decision the
/// loop evaluates, and the library calls `evaluate` once, from the loop.
fn pre_checks_read_the_evaluation(files: &[(PathBuf, String)]) -> Result<(), String> {
    let run = files
        .iter()
        .find(|(path, _)| path.ends_with("src/run.rs"))
        .map(|(_, code)| code.split_whitespace().collect::<String>())
        .ok_or("no src/run.rs")?;
    for check in [
        "letdecision=self.case.evaluate(seq)?;",
        "shape.only_behind(&decision,",
        "shape.unperformable(&decision,",
    ] {
        if !run.contains(check) {
            return Err(format!("src/run.rs does not hold `{check}`"));
        }
    }
    let calls: usize = files.iter().map(|(_, code)| evaluate_calls(code)).sum();
    if calls == 1 {
        Ok(())
    } else {
        Err(format!(
            "the library calls `evaluate` {calls} times; the exception allows the loop's one call"
        ))
    }
}

#[test]
fn the_library_holds_no_evaluation_of_its_own() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    sources(&src, &mut paths);
    assert!(!paths.is_empty(), "no sources under {}", src.display());
    let files: Vec<(PathBuf, String)> = paths
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            (path, code(&text))
        })
        .collect();
    let mut found = Vec::new();
    let mut evaluate = Vec::new();
    for (path, code) in &files {
        let shown = path
            .strip_prefix(&src)
            .unwrap_or(path)
            .display()
            .to_string();
        if path.file_name().is_some_and(|name| name == "decision.rs") {
            found.push(format!("src/{shown}: a decision.rs"));
        }
        for line in canon_eval(code) {
            found.push(format!("src/{shown}:{line}: a b10x_canon::eval path"));
        }
        for line in case_dir_evaluate(code) {
            evaluate.push(format!("src/{shown}:{line}: an evaluate method of CaseDir"));
        }
    }
    if !evaluate.is_empty()
        && let Err(why) = pre_checks_read_the_evaluation(&files)
    {
        for at in evaluate {
            found.push(format!(
                "{at}, and the pre-checks' exception does not hold: {why}"
            ));
        }
    }
    assert!(
        found.is_empty(),
        "evaluation code left:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_scan_finds_each_kind_it_refuses() {
    let probe = code(concat!(
        "use b10x_canon::eval;\n",
        "use b10x_canon::{model, eval::read_case};\n",
        "// use b10x_canon::eval; in a comment\n",
        "const S: &str = \"b10x_canon::eval impl CaseDir {\";\n",
        "impl<'a> CaseDir {\n",
        "    fn open(&self) -> char { '{' }\n",
        "    pub fn evaluate(&self, seq: usize) {}\n",
        "    fn evaluated(&self) {}\n",
        "}\n",
        "impl Other { fn evaluate(&self) {} }\n",
        "use b10x_canon::model::EvidenceRecord;\n",
        "fn f(case: &CaseDir) { case.evaluate(1); CaseDir::evaluate(case, 2); }\n",
        "fn g() { assertions::evaluate(); case.evaluated(); }\n",
    ));
    assert_eq!(canon_eval(&probe), vec![1, 2]);
    assert_eq!(case_dir_evaluate(&probe), vec![7]);
    assert_eq!(evaluate_calls(&probe), 2);
}
