---
format: aep.planning-md/3
id: story:connector-action-binds-an-action-not-declared-read
kind: story
status: draft
title: 'A connector action binds only an action its protocol declares effect: read'
tags:
- finding
refs:
- provider: finding
  reference: connector-action-binds-an-action-not-declared-read
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
revision: 1
---
# Story: connector-action-binds-an-action-not-declared-read

## Outcome

`connector_actions` binds only an action the case's protocol declares `effect: read`. A binding of
an action with no `effect`, or with any effect but `read`, is refused at its line when the case
opens, exit 2, the case discarded, and nothing is invoked. Once the Connectors describe says which
of its operations write, a binding to an operation the instance calls a write is refused too.

## Context

Found by an independent review of pull request 67 (wave 59, docs/waves/wave-59.md) at b47913d3,
should-fix. The read-only rule of the connector read action rests on the run's own protocol file
alone. `declared_write` (`crates/takt/src/connector_action.rs:318-330`) is true only when the
pinned protocol says `effect: write`, and both checks read it: the one at case open
(`connector_action.rs:531-541`) and the one before each invoke (`connector_action.rs:378-383`). An
action with no `effect`, or with an effect that is not `write`, may be bound to any operation the
instance describes, a chat post or update among them, and takt POSTs it with no
`idempotency_key`. The describe is read for operation ids only
(`crates/takt/src/connectors_client.rs:546-551`).

The specification (`ess/domains/takt.yaml`, the `ConnectorAction` comment near line 404) and
`website/docs/reference/configuration.md` (`connector_actions`) say the binding is for an action
"with `effect: read`"; the code accepts any effect but `write`, none included.

Dependency: part 2 below needs a change in the Connectors repository
(https://github.com/beyond10x/connectors): `GET /v1/describe` carries a read or write effect per
operation. Its descriptor has none today (`Operation` in `crates/connectors-core/src/lib.rs`:
`id`, `description`, `contract`, `profile`, `input_schema`, `output_schema`). Part 1 needs nothing
outside takt.

The predicate writes, in its own home, a protocol whose action `thread.read` declares no `effect`,
a run file binding it in `connector_actions` to `chat.postMessage` of an instance on 127.0.0.1, and
a python3 stand-in for that instance which describes `chat.postMessage`, answers every invoke with
success and prints what it was invoked with. It then runs `takt run` on a fresh repository. Exit 0,
with the stand-in's line, means the binding was accepted and the operation called. Exit 2 means
the binding was refused. Nothing is implemented with this filing.

## Acceptance

1. A binding in `connector_actions` of an action whose protocol declares no `effect`, or an
   `effect` other than `read`, is refused at the binding's line when the case opens, exit 2, the
   case discarded, nothing invoked; a resumed case with such a binding resumes nothing, exit 2.
   Tests through the binary, one with no `effect` and one with an unknown effect.
2. Each observation checks again, against the case's pinned `protocol.yaml`, that the action is
   declared `effect: read`, and calls nothing when it is not. A test.
3. Once the Connectors describe carries an effect per operation: a binding to an operation the
   describe calls a write is refused before any case opens, naming the action and the operation,
   as an operation the describe does not list is (exit 1, no case opened). A test with a fake
   instance. Blocked on the Connectors change until it lands.
4. `ess/domains/takt.yaml` and `website/docs/reference/configuration.md` state rule 1 (and rule
   3 once it holds) in their own words, specification first.
5. At the candidate, `scripts/check-finding story:connector-action-binds-an-action-not-declared-read
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
repository: https://github.com/beyond10x/takt
seed: 32d0263206eefbf7170d0a548845001f10453d99
command: cd "$HOME" && printf '%s\n' 'format: protocol/1' 'protocol:' '  id: connector.probe' '  revision: 1' '  description: reads a thread through a connector action' 'artifacts:' '  implementation:' '    description: a revision of the repository' 'evidence_kinds:' '  thread_read:' '    description: what reading the thread came to' 'claims:' '  thread.found:' '    description: the thread was read' '    true_when:' '      evidence:' '        kind: thread_read' '        result: pass' '        subject: implementation' 'actions:' '  thread.read:' '    description: read the thread, no effect declared' '    may_produce:' '      - evidence: thread_read' 'outcomes:' '  found:' '    description: the thread was read' '    requires:' '      claim: thread.found' > p.yaml && printf '%s\n' 'connectors:' '  endpoint: http://127.0.0.1:8402' '  instance: chat/team' 'connector_actions:' '  thread.read:' '    operation: chat.postMessage' '    input: {channel: C0000000, text: hello}' '    evidence: thread_read' > r.yaml && printf '%s\n' 'import http.server, json, sys' 'class H(http.server.BaseHTTPRequestHandler):' '    def answer(s, body):' '        b = json.dumps(body).encode()' '        s.send_response(200)' '        s.send_header("content-type", "application/json")' '        s.send_header("content-length", str(len(b)))' '        s.end_headers()' '        s.wfile.write(b)' '    def do_GET(s):' '        s.answer({"instance": "team", "revision": "r1", "operations": [{"id": "chat.postMessage"}]})' '    def do_POST(s):' '        q = json.loads(s.rfile.read(int(s.headers["content-length"])))' '        sys.stderr.write("the instance was invoked: " + q["operation"] + ", idempotency_key " + str(q.get("idempotency_key")) + "\n")' '        s.answer({"version": "v1alpha2", "request_id": q["request_id"], "status": "success", "result": {"ok": True}})' '    def log_message(s, *a):' '        pass' 'v = http.server.HTTPServer(("127.0.0.1", 8402), H)' 'open("up", "w").close()' 'v.serve_forever()' > s.py && mkdir t && echo a > t/README.md && git -C t init -q -b main && git -C t -c user.name=t -c user.email=t@example.invalid add -A && git -C t -c user.name=t -c user.email=t@example.invalid commit -q -m start || exit 70; python3 s.py & P=$!; i=0; while [ ! -e up ] && [ $i -lt 100 ]; do sleep 0.1; i=$((i+1)); done; "$TAKT" run --repo t --harness none --forge none --iterations 3 --test-cmd true --intent "read the thread" --protocol p.yaml --config r.yaml --state-dir s --min-free-gib 0; e=$?; kill $P; exit $e
exit: 0
healthy_exit: 2
output: the instance was invoked: chat.postMessage, idempotency_key None
source: docs/waves/wave-59.md
```
