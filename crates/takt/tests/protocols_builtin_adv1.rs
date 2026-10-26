//! Adversary pass 1 on story:protocols-are-engineering-protocols-built-ins: takt's three
//! protocols are the engineering protocols' built-ins, byte for byte what takt 0.12.0 carried in
//! `protocols/<name>/1.yaml`, under both spellings, and every built-in `takt protocols` lists
//! resolves to a protocol whose own id is the listed name.

use sha2::{Digest as _, Sha256};
use takt::LoadedProtocol;

/// The sha256 of `git show 0.12.0:protocols/<name>/1.yaml`, read from the tag on 2026-10-26.
const AT_0_12_0: [(&str, &str, &str); 3] = [
    (
        "story.delivery/1",
        "story-delivery/1",
        "34c7d2d742f7002f68207e48dbfefbcf23d464112cca85e5e2510e7cd97a269c",
    ),
    (
        "self.improvement/1",
        "self-improvement/1",
        "7947a8ebb461b56b608c1d15821b126ed353d12a20eaab8a30ebc2f5f71d9033",
    ),
    (
        "loop.improvement/1",
        "loop-improvement/1",
        "04ba56c8fe24cc29f9deaa209697ec6a33f1c34c4e8fcaa8c4d0d3bbd198389f",
    ),
];

/// A case opened by takt 0.12.0 holds that release's document in its `protocol.yaml`, and a
/// turn resumes only when that copy equals the built-in byte for byte
/// (`takt::case::turn_protocol_refusal`). The unit's own tests compare the built-in with the
/// registry it reads, which holds whatever the registry says; this one pins the release.
#[test]
fn adv_each_builtin_is_byte_for_byte_takt_0_12_0s_file() {
    for (id, listed, digest) in AT_0_12_0 {
        for given in [id, listed] {
            let protocol = LoadedProtocol::builtin(given).expect("a built-in");
            assert_eq!(protocol.id, id, "{given}");
            assert_eq!(
                format!("{:x}", Sha256::digest(protocol.yaml.as_bytes())),
                digest,
                "{given} is not takt 0.12.0's document"
            );
        }
    }
}

/// Every built-in `takt protocols` prints, `<registry name>/<major>`, is accepted back by
/// `--protocol` and names a protocol whose own id is that name with `-` read as `.` or `_`
/// (dispatcher: the registry lists `eval.gated_change` as `eval-gated-change`), so the
/// listed spelling and the recorded one (`case.yaml`, `events.jsonl`) are one protocol.
#[test]
fn adv_every_listed_builtin_resolves_to_its_own_id() {
    let listed = LoadedProtocol::builtins();
    assert!(listed.len() >= 4, "{listed:?}");
    for (name, major) in listed {
        let spec = format!("{name}/{major}");
        let protocol = LoadedProtocol::resolve(&spec).expect("a listed built-in resolves");
        assert_eq!(protocol.id.replace(['.', '_'], "-"), spec, "{spec}");
        let dotted = LoadedProtocol::builtin(&protocol.id).expect("its own id resolves too");
        assert_eq!(dotted.yaml, protocol.yaml, "{spec}");
    }
}

/// A major the registry does not hold for a protocol it does hold is refused, and the refusal
/// says that the major is unknown, not that the protocol is: `story-delivery` is a built-in, so
/// `story.delivery/2` names a known protocol at an unknown major.
#[test]
fn adv_an_unknown_major_of_a_known_builtin_is_not_called_an_unknown_name() {
    for spec in ["story.delivery/2", "self.improvement/2", "loop.improvement/2"] {
        let error = format!(
            "{:#}",
            LoadedProtocol::builtin(spec).expect_err("no such major")
        );
        assert!(
            !error.contains("UnknownName"),
            "{spec}: the refusal calls a known protocol unknown: {error}"
        );
    }
}

/// `LoadedProtocol::builtin` says a built-in "answers to its own id and to the registry's name";
/// the run prints and records the own id (`case <id> on eval.gated_change/1`), so that id given
/// back as `--protocol` names the same built-in.
#[test]
fn adv_a_builtins_own_id_with_an_underscore_resolves() {
    let listed = LoadedProtocol::builtin("eval-gated-change/1").expect("the listed name");
    assert_eq!(listed.id, "eval.gated_change/1");
    let own = LoadedProtocol::builtin(&listed.id);
    assert!(
        own.is_ok(),
        "its own id is refused: {:#}",
        own.expect_err("refused")
    );
}
