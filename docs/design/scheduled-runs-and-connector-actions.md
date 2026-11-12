# Takt: scheduled runs and connector actions

Status: design of 2026-11-12; nothing in it is built yet; the waves that build it are under `docs/waves/`. It is carried by `epic:scheduled-runs-and-connector-actions`; § 7 lists its stories and their waves. § 5 records the owner's decision on unattended writes.

## 1. The target

A team runs a reconcile: every 30 minutes inside calendar windows (for example Monday and Tuesday 07:00 to 22:30 and Wednesday 07:30, in a named zone), read a chat thread and a forge (merge request lists, tags), and keep one status message in that thread current. Post it once, edit it in place when its text changes, send nothing when nothing changed; stop when the work is done (a final tag exists and a sign-off is recorded).

Today such a team writes this as scripts that hold tokens, under a CI schedule. takt 0.34.0 lacks two things for it:

- **F1**, a schedule: a run that starts a pass at the slots a calendar names, and stops on a condition.
- **F2**, an action that invokes a Connectors operation: a protocol step that reads or writes through a served Connectors instance, with no provider credential in takt.

Later the same reconcile hands judgement units to a takt case through the `command` work source, which exists in 0.34.0.

The example names in this document are placeholders: `chat/team`, `gitlab/work`, `group/chart`, `#releases`.

## 2. What exists

| what | state | where |
|---|---|---|
| a run | one pass: `Commands::Run` resumes or opens one case and calls `finish()`; no watch, loop or schedule concept exists | `crates/takt-cli/src/main.rs:1857`, `main.rs:3829` |
| exit codes | consts: 0 done, 1 refused, 2 usage, 3 stopped, 4 approval, 5 error, 6 waiting, 7 unperformable, 8 disk low, 9 declined. `design:continuous-orchestration` § 5 is the registry of exit codes from 6 up and proposes 9 Paused and 10 FrontierExhausted; code already uses 9 for Declined. This design adds no exit code to `takt run` | `main.rs:1523-1561` |
| nothing ready | `frontier: nothing ready`, exit 0. Stdout is the only place that says so; there is no run outcome file. A case's `events.jsonl` ends with a `stop` event, but a run that opens no case writes none | `main.rs:2473-2475`, `crates/takt/src/events.rs:313` |
| a run-file key | a flag first (`unset`; prose at `main.rs:819-820`); `RunConfig` is the run file's type, its key sentence is `SHAPE` | `main.rs:850`, `crates/takt/src/run_config.rs:198`, `run_config.rs:1200` |
| time | UTC clock helpers only; `jiff` 0.2.38 and `jiff-tzdb` 0.1.9 are in `Cargo.lock` (through gix-date); no cron crate | `crates/takt/src/lib.rs:49`, `events.rs:30-33`, `Cargo.lock` |
| protocol actions | bound to validators by name only (`Validator::action()`); the built-ins are a closed table. An action no validator owns is offered to the harness | `crates/takt/src/validator.rs:62-72`, `validator.rs:8191-8273` |
| evidence from a command | none: no run-file command produces evidence. The evidence port takes a record only from a validator of the run or the choice producer. `takt.md` § 3 lists "step deterministic: action, argv" as M3, not built | `crates/takt/src/evidence.rs:147-153`, `evidence.rs:313-319` |
| an evidence record's payload | `canon-evidence/1` has no payload field; a validator's one retained artifact goes to `<case>/artifacts/<name>` | `evidence.rs:23-62`, `validator.rs:33-50`, `crates/takt/src/run.rs:12521-12532` |
| a grant | binds to the case revision and the revision of the grant's subject (`CaseAuthority::verdict_over`, `applies_at`); a new commit makes the action approval-required again. Only the run's one gate action is approval-gated | `crates/takt/src/case.rs:2062-2115`, `case.rs:748-759`, `run.rs:13119-13123` |
| the connectors forge's served route | `exchange` (reqwest, no Authorization header, 16 MiB cap); describe `preflight_served`; invoke `served_answer`; `idempotency_key` (`pub`); the refusal record `pull-request.refusals.json`. The client parts are private and take `&'static str` operations on a forge-shaped struct. `story:work-from-jira-github-and-gitlab` needs the same client | `crates/takt/src/forge/connectors.rs:2082-2129`, `connectors.rs:1062-1121`, `connectors.rs:1400-1445`, `connectors.rs:182-194`, `connectors.rs:849`, `connectors.rs:378-418` |
| credentials | `forge::without_credentials` clears credentials for forge and work commands | `crates/takt/src/forge.rs:842-848` |

## 3. F1: a run on a schedule

Shape, flags first, the run file's `schedule` mapping second:

```
takt run … --every 30m --time-zone Europe/Berlin \
          --window "Mon-Tue 07:00-22:30" --window "Wed 07:30" \
          --until outcome:released --until deadline:2026-11-19T00:00:00+01:00
```

```toml
[schedule]
every = "30m"
time_zone = "Europe/Berlin"
windows = ["Mon-Tue 07:00-22:30", "Wed 07:30"]
until = ["nothing-ready", "deadline:2026-11-19T00:00:00+01:00"]
```

A run-file key is a flag first (§ 2), so each key of `schedule` is a flag of `takt run`.

- `--every <duration>` turns `takt run` resident. Without it nothing changes: one pass, as today.
- **Slots.** Inside each window, the window's start and every `every` after it up to and including its end; a window with no end (`Wed 07:30`) is one slot. Days are `Mon` to `Sun`, a range `Mon-Tue` or a list `Mon,Wed`. With no window, a slot falls every `every` from the start.
- **Time.** Times are wall-clock in `time_zone`, an IANA name resolved by the bundled tz database, so the container needs no tzdata. A slot in a DST gap moves forward to the first valid instant; one in a DST fold is taken once, the earlier.
- **A tick** is one child `takt run` process: the same argument list with the schedule flags removed and `--once` added (`--once` also ignores the run file's `schedule`), and `--outcome-file <tick dir>/outcome.json`. One tick runs at a time. A tick still running at its next slot skips that slot, which is counted and never queued. After a suspend the missed slots run once, not one each. Crash isolation and the per-tick exit code come from the child process.
- `--outcome-file <path>`, also usable without a schedule, writes `takt.run-outcome/1`: `exit`, `stop` (the Stop's kind or null), `case` (id or null), `outcome` (the legitimate outcome the case reached, or null), `frontier` (`nothing_ready` or null), `started`, `ended`. This is the file "nothing ready" is read from; stdout stays as it is.

### Stop conditions

`until` holds any of:

| condition | holds when |
|---|---|
| `nothing-ready` | a tick's outcome says `nothing_ready` |
| `exit:<n>` | a tick exited n |
| `outcome:<name>` | a tick's case reached that outcome of its protocol |
| `deadline:<RFC 3339 instant>` | no slot falls at or after it |

Also the schedule ends when no slot is left. The windows are exhausted only with a deadline; without one the windows repeat weekly. A refusal or usage error of a tick (exit 1 or 2) ends the schedule with that exit; other tick exits do not end it unless listed.

### Files and exit

Under `<state dir>/schedule/<schedule id>/`, the schedule id being a digest of the schedule and the argument list:

| file | holds |
|---|---|
| `schedule.json` | the resolved schedule, the argument list, pid, started |
| `ticks.jsonl` | one `takt.schedule-tick/1` line per tick: slot, started, ended, exit, outcome path, skipped slots before it |
| `ticks/<slot>/stdout`, `stderr`, `outcome.json` | what the tick wrote |
| `end.json` | when the schedule ends: reason, exit |

A later orchestrator reads files and the exit code only. The resident process exits:

| exit | when |
|---|---|
| 0 | an `until` condition holds or no slot is left, with `schedule: done (<reason>)` |
| 1 or 2 | a tick was refused |
| 5 | its own error |
| 3 | SIGTERM or SIGINT: no further tick starts, the running tick ends on its own, the schedule writes `end.json` with reason `signal` and exits 3 (stopped, as a budget stop). Nothing is forwarded to the tick, as `design:continuous-orchestration` § 5 reads a pause at a boundary; a second signal is forwarded |

A second start of the same schedule while `schedule.json` names a live pid is refused, exit 1, naming the pid. After a dead pid or an `end.json`, it starts again, rebuilt from the files: it runs no slot that `ticks.jsonl` already holds.

### Fit with the orchestration design

A schedule decides when a pass starts, never what it works on. Nothing is kept in memory that is not in the files (`design:continuous-orchestration` § 9, crash). It adds no exit code (§ 5 there). The dispatcher of § 9 stays the place where a tree of cases is driven. A schedule is not a case: it is a caller of `takt run`, as the later orchestrator `AGENTS.md` names is, and each tick's `takt run` is the one run that § 6 there makes a root case under `run/1` once `run/1` lands. Nothing in the schedule changes when it does.

Spec first: `takt.work.Schedule` (every, time_zone, windows, until), `takt.work.ScheduleTick`, `takt.work.RunOutcome`, and the field `schedule` on `takt.work.RunConfiguration`, in `ess/domains/takt.yaml`.

## 4. F2: connector actions

Protocols stay data. A `protocol/1` file declares the actions and claims; the run file binds an action id to a Connectors operation, as `forge_commands` binds forge steps to argv, and one generic validator performs every binding. No action is modelled in Rust.

```yaml
connectors:
  endpoint: http://127.0.0.1:8402          # a served instance; https, or http to loopback
  instance: chat/team                      # <connector>[/<instance>], checked against GET /v1/describe
connector_actions:
  thread.read:
    operation: conversations.replies
    input: {channel: C0000000, ts: "1700000000.000100"}
    evidence: thread.read                  # the kind the protocol's may_produce names
  status.publish:
    operations: [chat.postMessage, chat.update]
    input_from: status/publish.json        # an artifact a command action wrote: {operation, input}
    evidence: status.published
command_actions:
  status.render:
    argv: [scripts/render-status, "{artifacts}"]
    writes: status/publish.json
    evidence: status.rendered
```

### Read action

`effect: read` in the protocol. One `POST /v1alpha2/invoke` after one describe. The answer is kept as the observation's artifact `connector/<action>.json`. The record is `canon-evidence/1` of the bound kind, result `pass` on `status: success` and `fail` on a refusal, bound to the case revision as every validator's record is. The answer is evidence's artifact, never the record: the record has no payload field (§ 2).

### Command action

argv without a shell, in the tree, with no credential (`without_credentials`). `{artifacts}` is the case's artifacts directory; its one output file is its declared `writes`, under `artifacts/`. Exit 0 is result `pass`; a declared `unchanged_exit` is result `unchanged`; a declared `done_exit` is result `done`; any other non-zero exit is `fail`. This is the smallest piece of `takt.md`'s "deterministic step" (M3), bound by name as validators are.

### Write action

`effect: write`, with `requires: [{capability: connector.write}]` in the protocol. Its precondition reads the render's record about one subject, so the grant has a subject (`crates/takt/src/protocol.rs:188-228`).

- It is performed only under a grant at the case revision and the subject's revision: a declaration in the repository's `.engineering/delegations.yaml` that covers it (§ 5), else `takt approve <case> connector.write`.
- The operation is the one `input_from` names, refused unless listed in `operations`.
- The call carries `idempotency_key` from the shared key function (case, operation, descriptor revision, input), so a resumed case never posts twice. Each tick is a new case, so across ticks the key does not deduplicate: the render does, by comparing its text with the message it found in the thread read.
- The refusal record is per action: `artifacts/connector/<action>.refusals.json`.
- An `unchanged` render makes the write inadmissible by the protocol's precondition: nothing is sent and the case reaches its outcome.

### No credential in takt

takt sends none, as the forge does. The served instance holds the provider's credential; the Connectors standing grant scopes the bot to one channel and the two chat operations, with `max_per_hour`. takt's grant and the Connectors standing grant are two independent locks.

### One client

The describe, invoke, key and refusal bookkeeping move out of `forge/connectors.rs` into a module that the forge, the connector actions and `story:work-from-jira-github-and-gitlab` share, generic over the operation name.

Spec first: `takt.work.ConnectorAction`, `takt.work.CommandAction`, `takt.work.ConnectorsEndpoint`, and the fields on `RunConfiguration`.

### 4.1 Connectors facts

From a reading of `beyond10x/connectors` main at release v0.71.0, 2026-11-12.

| fact | detail |
|---|---|
| connector `gitlab` | reads `merge_requests.list` (state, target_branch, updated_after), `tags.list`, `tag.get`, `merge_request.get`, `commits.list`, `pipelines.list`; a self-hosted GitLab through the instance's `endpoint` |
| connector `slack`, a bot instance | reads `conversations.history`, `conversations.replies`, `users.list`, `auth.test`; writes `chat.postMessage` (with `thread_ts`) and `chat.update` (own messages only). `ts` is a string |
| local CLI writes | need a signed per-input approval proof (`--approval-file`); `mounted_secret` custody refuses approval signing. Unattended writes therefore go through the served host, never the CLI |
| the served host | `connectors-catalog-provider --config <file>`, one instance per host: `GET /v1/describe`, `POST /v1alpha2/invoke` with `{version, request_id, operation, revision, input[, idempotency_key]}`, `GET /healthz` |
| admission | a static bearer or a Kubernetes TokenReview |
| writes | only under `service.standing_grants` (`connectors.standing-grants/1`: `id`, `instance`, `operation`, `target: [{pointer, value}]`, `not_after_unix_ms`, `max_per_hour`). Every write carries an `idempotency_key`; the same key replays and never resends. An uncovered write is `approval_required` (403) |
| the image | `ghcr.io/beyond10x/connectors:<version>` from 0.70.0; credentials come from a mounted Secret |

## 5. The owner's decision

takt's grant binds to a revision, so an unattended reconcile that must edit the message whenever it changed would need a person's `takt approve` on every changed tick.

The owner decided on 2026-11-12: "in our specific case, there should be no approval, ideally this behavior/permissions can be configured via config file" (`decision-blocker:connector-writes-unattended`, cleared).

- **The declaration.** The repository declares the writes it allows in its `.engineering/delegations.yaml`, a judge surface, so only a change the owner grants can widen it. One entry per allowed write: the capability `connector.write`, the role takt grants as, the connector and its instance, the operations (for a chat, `chat.postMessage` and `chat.update`), the target as JSON pointers and values into the input (for a chat, `/channel` and one channel id), an optional `max_per_hour`, an optional `not_after`, and `revoked_at` once revoked.
- **The grant.** takt reads the declaration at the revision the case opened on. When a write's operation and target fall inside an entry that is not revoked, not expired and not over its rate, takt records the grant in the case's `authority.yaml` as `agent:<role>`, naming the entry, and the write goes with no `takt approve`. Any other write is refused and nothing is sent. The governor and `CaseAuthority` decide as for any grant; the validator holds no gate of its own.
- **The rule.** `AGENTS.md` § Rules names this as the second path by which takt grants itself, beside the merge's delegation, with the owner's words and date.
- **Per-case grants stay.** A write with no declaration covering it still waits for `takt approve` (`story:a-connector-write-under-a-grant`).
- **Two locks.** The Connectors standing grant on the served instance (one channel, two operations, `max_per_hour`) stays a second, independent limit.

## 6. Composition and Kubernetes

The reconcile, end to end. `takt run --intent "reconcile" --protocol <file> --once` opens a fresh case each tick: evidence is bound to the case revision, so reads must be fresh per case. The reads run; the render compares the new status text with the bot's own message found in the thread read (stateless: no state between ticks); the write posts or edits it, or sends nothing. The schedule wraps it: `--every 30m --window … --until outcome:released`.

A reconcile done (final tag and sign-off) is a second outcome of the reconcile protocol, `released`, which the render's record (result `done`, a declared `done_exit` of the command action) makes legitimate. `until outcome:released` stops the schedule. Judgement units go later to a separate run through the `command` work source.

Kubernetes. A resident `takt run --every` is a long-running process; Loom's LoomAgent Command variant renders a run-once Job (`backoffLimit 0`). Two routes:

- a Job per tick from a CronJob (Loom `spec.schedule`) with `--once`, where the schedule's windows live in the CronJob and the `until` conditions are lost;
- one long-lived Job running `takt run --every`.

Not decided here; out of scope for the first slices. Loom 0.57.2 gives a Command LoomAgent without an embedded agent document DNS-only egress and refuses `spec.boundary` beside `spec.secretRef`. The served Connectors instance as a separate in-cluster service reached over http is the shape that fits both.

## 7. Slices and waves

Stories under `epic:scheduled-runs-and-connector-actions`:

| wave | story | holds |
|---|---|---|
| 57 | `story:a-run-on-a-schedule` | F1: flags, run-file key, ticks, outcome file, `until` |
| 57 | `story:one-served-connectors-client` | F2 groundwork: the client extracted, the forge on it, no behaviour change |
| 58 | `story:a-command-action` | the command action (§ 4) and the registration of run-file-bound validators |
| 59 | `story:a-connector-read-action` | the read action (§ 4), on that registration |
| 60 | `story:a-connector-write-under-a-grant` | the write action under a per-case grant |
| 61 | `story:connector-writes-under-a-delegation` | the write granted by a declaration in `.engineering/delegations.yaml`, no `takt approve` (§ 5) |
| 62 | `story:a-reconcile-example` | `examples/reconcile/`: a `protocol/1` file, a run file with a schedule and a declaration, three ticks against a fake served instance that post once, edit once and send nothing, and a tick that ends the schedule at `released` |

Wave 57's two stories each add one `pub mod` line to `crates/takt/src/lib.rs`; the schedule's unit is committed first. Waves 58 to 62 are one story each, because each lands on `run_config.rs`, `ess/domains/takt.yaml` and `configuration.md`, or reads what the one before produces.

Each tick is a fresh case, so a grant given by hand to one case is not used by the next; a scheduled reconcile writes under the repository's declaration (§ 5, `story:connector-writes-under-a-delegation`).

## 8. Not in this design

- Cron expressions: windows plus `every` cover the case; a cron field can come later as another way to name slots.
- A daemon or service manager.
- Schedules across repositories, which are conductor's.
- Loom's CronJob route (§ 6).
- A Slack or GitLab model in takt: the operations are Connectors'.
- Local CLI writes, which need a per-input approval proof (§ 4.1).

## 9. Open items

- **Authorization and TokenReview.** takt sends no Authorization header (`website/docs/reference/configuration.md`, the connectors forge). A served instance that admits by bearer or TokenReview cannot be reached by takt today. A pod's projected service-account token as the caller's identity is the likely route (TokenReview); it is a credential of the pod, not a provider credential, but it is a bearer in takt's process. The decision is deferred to the story that first needs it; the loopback instance with admission off is the tested route.
- **Untested on the Connectors side.** A served Slack `chat.postMessage` then `chat.update`, and the git relay against a live GitLab, are not tested in the Connectors repository yet.
- **The Loom route.** One CronJob per tick or one long-lived Job (§ 6) is not decided.
