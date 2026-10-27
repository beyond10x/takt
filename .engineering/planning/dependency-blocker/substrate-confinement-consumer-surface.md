---
format: aep.planning-md/3
id: dependency-blocker:substrate-confinement-consumer-surface
kind: dependency-blocker
status: cleared
title: Substrate's confinement consumer surface is in no substrate release yet
summary: substrate story:confinement-consumer-surface, in the first substrate release after 0.11.0 that ships it
tags:
- adr-0109
relations:
- blocks: story:confinement-through-substrate-on-linux-and-macos
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-10-27T08:56:28Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"verification":1}}}
---
# Dependency blocker: substrate-confinement-consumer-surface

story:confinement-through-substrate-on-linux-and-macos moves takt's confinement onto Substrate's
confinement contract (ADR 0109, decision 6, hop T1). The surface takt calls does not exist in a
substrate release yet.

- Substrate's latest release is 0.11.0 (tag `0.11.0`); its `HostDriver::open` refuses on macOS with
  `host.platform-unserved` and no driver uses its Seatbelt primitive (ADR 0109 § Context).
- The surface is substrate's `story:confinement-consumer-surface` (epic `confinement-contract-family`),
  active on substrate's integration branch for ADR 0109: a consumer-written test that imports only
  `b10x_substrate_host::confinement`, covering takt's promotion build, and a committed list of the
  module's public items. It depends on the Linux and `local` contract, the `macos` adapter, the
  Seatbelt policy that runs `cargo test`, and the `systemd`, `oci` and `pod` adapters.
- Substrate ships that epic as one pull request and one release through its own process; the
  version is not named in its plan yet.

Cleared when a substrate release after 0.11.0 ships story:confinement-consumer-surface, cited by
its tag and the CHANGELOG line, and its `macos` adapter runs `cargo test` on the `macos-15` runner.
takt builds no substitute before then.
