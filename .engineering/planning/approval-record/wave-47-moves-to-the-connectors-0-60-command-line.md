---
format: aep.planning-md/3
id: approval-record:wave-47-moves-to-the-connectors-0-60-command-line
kind: approval-record
status: approved
title: Wave 47 moves the connectors forge to the Connectors 0.60.0 command line
summary: connectors invoke <connector>/<instance> <operation>; no reliance on serve
relations:
- decides: story:connectors-forge-speaks-github-at-an-endpoint
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-30T06:23:12Z", actor: "human:timo", revision: 3}
---
# Approval: wave 47 moves the connectors forge to the Connectors 0.60.0 command line

## Decision

The orchestrator, under the owner's grant (2026-10-30): Connectors 0.60.0 removes
`connectors operations invoke`, `invoke --endpoint`, `describe --endpoint` and `serve`; each old
form is a parse refusal, exit status 2. In wave 47, takt's connectors forge moves every call to
the new form, `connectors invoke <connector>[/<instance>] <operation> --input-json <json>`, and
the served-wire path of story:connectors-forge-speaks-github-at-an-endpoint does not rely on
`serve`: a running instance is reached over the served wire, in a pod through Loom's boundary.
The docs, the specification and the example that name the old form follow. Only the connectors
forge's test targets run for it.

## Covered

| item | what |
|---|---|
| forge | `crates/takt/src/forge/connectors.rs`: the command line of each operation, in U3 |
| text | `takt run --help`, the run's opening line, the configuration reference, the ESS citation, the cluster-delivery example |
