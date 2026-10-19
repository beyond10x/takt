//! The harness's reply parser over any text: it names an action or refuses, never panics, and
//! the action it names is read from a line that names one.

#![no_main]

use libfuzzer_sys::fuzz_target;
use takt::harness::ClaudeCode;

fuzz_target!(|data: &[u8]| {
    let Ok(reply) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok((action, _)) = ClaudeCode::parse(reply) {
        let named = reply.lines().rev().find(|line| {
            let line = line.trim();
            line.starts_with('{') && line.contains("\"action\"")
        });
        let line = named.expect("an action comes from an action line");
        let doc: serde_json::Value = serde_json::from_str(line.trim()).expect("the line reads");
        assert_eq!(doc["action"].as_str(), Some(action.as_str()));
    }
});
