//! The `b10x-gates` policy stand-in the test doubles never read, which the bot route still needs
//! named (story:forge-preflight): `fixtures/gates-policy-stand-in.json`, set to mode `0600`
//! before it is named. The preflight checks the policy as `b10x-gates` loads one, a file of this
//! user's that nobody else can read or write, and git keeps no mode below the execute bit, so a
//! checkout leaves the fixture `0644`. Included by path from both crates' tests; either crate's
//! manifest directory reaches it through `../takt`.
#![cfg(unix)]

/// The stand-in's path.
const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/gates-policy-stand-in.json"
);

/// The stand-in's path, the file at mode `0600`.
pub fn path() -> &'static str {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(PATH, std::fs::Permissions::from_mode(0o600))
        .expect("the stand-in policy at 0600");
    PATH
}
