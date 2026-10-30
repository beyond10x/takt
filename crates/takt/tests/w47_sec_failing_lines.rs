//! Wave 47 security review, story:takt-carries-no-organisations-own-route (U1): with the
//! `gates` scan gone, a failed test's or gate's output reaches a judgement step's prompt through
//! [`takt::validator::failing_lines`] alone, and U1 states that "a line of a credential's shape is
//! still left out". Each case prints one line of a common credential shape among ordinary failing
//! output and asserts that line, and its secret part, is not among the lines kept.
//!
//! The values are made-up, of the documented shapes only; none is a live credential.

use takt::validator::failing_lines;

/// A budget large enough that nothing is left out for room.
const ROOM: usize = 64 << 10;

/// `printed` as a failed command's output around `line`, and asserts `secret` is in no kept line.
fn withheld(line: &str, secret: &str) {
    let printed = format!(
        "running 2 tests\nerror: assertion failed at src/lib.rs:10\n{line}\nerror: could not compile `x`\n"
    );
    let (kept, omitted) = failing_lines(&printed, ROOM);
    assert!(
        kept.iter().all(|kept| !kept.contains(secret)),
        "a line holding a credential reaches the prompt: kept {kept:?}, {omitted} left out"
    );
}

/// The shapes `credential_shape` already knows: these stay out (a control: green).
#[test]
fn known_shapes_stay_out_of_failing_lines() {
    let github = format!("ghp_{}", "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8");
    withheld(&format!("token was {github}"), &github);
    let gitlab = format!("glpat-{}", "a1B2c3D4e5F6g7H8i9J0");
    withheld(&format!("token was {gitlab}"), &gitlab);
    withheld(
        "-----BEGIN OPENSSH PRIVATE KEY-----",
        "BEGIN OPENSSH PRIVATE KEY",
    );
}

/// A PEM private key's body lines, between its `BEGIN` and `END` lines, stay out (a control).
#[test]
fn a_private_key_block_stays_out_of_failing_lines() {
    let body = format!(
        "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcw{}",
        "ggSjAgEAAoIBAQC7VJTUt9Us8cKj"
    );
    let dashes = "-".repeat(5);
    let printed = format!(
        "error: assertion failed\n{dashes}BEGIN RSA PRIVATE KEY{dashes}\n{body}\n{dashes}END RSA PRIVATE KEY{dashes}\nerror: could not compile `x`\n"
    );
    let (kept, _) = failing_lines(&printed, ROOM);
    assert!(
        kept.iter().all(|kept| !kept.contains(body.as_str())),
        "{kept:?}"
    );
}

/// An HTTP `Authorization: Bearer` header with an opaque token, as an HTTP client's debug output
/// or a failed request's dump prints it.
#[test]
fn a_bearer_authorization_header_stays_out_of_failing_lines() {
    let token = &format!("7f3c9a1e5b2d4f60{}", "a8c7e9b1d3f5a7c9e1b3d5f7");
    withheld(&format!("> Authorization: Bearer {token}"), token);
}

/// An AWS access key pair, as a failed test's environment dump or a config file shown by a
/// failing assertion prints it.
#[test]
fn an_aws_secret_access_key_stays_out_of_failing_lines() {
    let secret = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYzEXAMPLEKEY";
    withheld(&format!("aws_secret_access_key = {secret}"), secret);
}

/// An AWS access key id, `AKIA` and 16 upper-case letters or digits.
#[test]
fn an_aws_access_key_id_stays_out_of_failing_lines() {
    let id = &format!("AKIA{}", "Q3EGZ7XN2MBV4KPL");
    withheld(&format!("AWS_ACCESS_KEY_ID={id}"), id);
}

/// A URL with a password in its userinfo, as a failed fetch or a connection error prints it.
#[test]
fn a_url_with_a_password_stays_out_of_failing_lines() {
    let password = format!("s3cr3t{}", "Passw0rdZq9");
    withheld(
        &format!("error: could not connect to postgres://app:{password}@db.internal:5432/app"),
        &password,
    );
}
