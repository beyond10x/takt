//! `Events::read` over any bytes as a case's run record: it reads or refuses, never panics, and
//! a record that reads holds one value a line at most.

#![no_main]

use std::path::PathBuf;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use takt::events::{EVENTS_FILE, Events};

/// This process's case directory, under the fuzz crate's own target directory.
fn dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/scratch")
            .join(format!("events-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create the case directory");
        dir
    })
}

fuzz_target!(|data: &[u8]| {
    let dir = dir();
    std::fs::write(dir.join(EVENTS_FILE), data).expect("write the record");
    if let Ok(read) = Events::new(dir).read() {
        let lines = data.split(|byte| *byte == b'\n').count();
        assert!(read.len() <= lines);
    }
});
