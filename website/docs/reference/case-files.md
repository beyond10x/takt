---
title: Case files
sidebar_position: 4
description: The layout of .takt/cases/<case>/, which file takt or a command writes, and which are never edited by hand.
lede: A case is a directory of files in the repository, and every file in it is written by takt, by one of its validators or by takt approve.
source: crates/takt/src/case.rs (CaseDir, Branch, Authority, Records, the file constants), crates/takt/src/events.rs, crates/takt/src/evidence.rs, crates/takt/src/validator.rs, crates/takt/src/run.rs; a case from a takt 0.14.0 run on a scratch repository and one from a container run, read, not copied
---

# Case files

A run on a repository keeps its record in `.takt/cases/<case>/`, where `<case>` is the
case id (`--case-id`, or `takt-<UTC time>` when absent, and for a story `story-<name>-<UTC time>`).
The directory is the case: a later run resumes it from these files, `takt approve` writes into it,
and `takt trace`, `takt scorecard` and `takt replay` read it. How to read one after a run:
[Read a case](../guides/read-a-case.md). What a case is:
[Protocol and case](../concepts/protocol-and-case.md).

### Where cases live, and git

When takt creates `.takt/` and finds no `.takt/.gitignore`, it writes one holding `*`, so
`git status` shows none of a run's records. A `.takt/.gitignore` already there is left alone: a
repository that keeps its cases in version control commits its own, for example one holding
`!cases/`, and takt never overwrites it. takt itself never commits a case.

Earlier takt versions opened a case under `.engineering/cases/<case>/`. A case already there is
still resumed, approved, traced, scored and replayed where it is, with `--case <case>` or as a
story's open case, and takt does not move it; only a case that opens now goes under `.takt/cases`.
Both roots are guarded alike: a step may write under neither, and a link at either is refused.

takt also puts the repository on a branch `takt/<case id>` when the case opens, and
`branch.yaml` records where it came from.

## A case after one run

`takt run --repo ~/demo --intent "add a README line" --harness none --forge none --iterations 1
--test-cmd true`, then `takt approve --repo ~/demo --by human:demo <case> repository.merge`, on a
scratch repository:

```console
$ find .takt/cases -type f | sort
.takt/cases/<case>/authority.yaml
.takt/cases/<case>/branch.yaml
.takt/cases/<case>/case.yaml
.takt/cases/<case>/decisions/0002.json
.takt/cases/<case>/events.jsonl
.takt/cases/<case>/evidence/0001-test-result-1.json
.takt/cases/<case>/intent.md
.takt/cases/<case>/observations.json
.takt/cases/<case>/protocol.ir.json
.takt/cases/<case>/protocol.yaml
```

The run kept its latest decision alone, as the default records profile does for each run; `--records trial`
would list `decisions/0001.json` as well. A case that works a story, goes through a forge or a
harness has more files. All of them:

| file | what it holds | written by |
|---|---|---|
| `case.yaml` | `canon-case/1`: the case id, the protocol, each artifact's revision and the inputs | takt, when the case opens; the revisions move as evidence arrives |
| `protocol.yaml` | the protocol document the case runs on, copied at open, so a protocol that changes later does not change the case | takt, at open |
| `protocol.ir.json` | the same protocol compiled, as the governor reads it | takt, at open |
| `intent.md` | what the run was meant to achieve; for a story, its title, a blank line and its body | takt, at open |
| `story.yaml` | `id:` of the story the case works; absent for an intent | takt, at open |
| `branch.yaml` | the branch `takt/<case>`, its `base`, the `base_commit` it forked from, the branch the tree was `left` on, and once pushed, `pushed` and `pull_request` | takt; the `pr.open` validator adds the last two |
| `commit-identity` | the identity of every commit of the case, `<name> <email>` on one line; absent when none was configured | takt, at open ([identity](./configuration.md#commit-identity)) |
| `observations.json` | the observations reported to the case, in order, as Loom's file case store keeps them | takt, appended during the run |
| `python-modules.json` | the Python module names a test command can import from outside the repository, as the Python the case opened with answered, or built-in lists | takt, at open, only when a Python answered or could not be established; absent otherwise, when the built-in lists stand in |
| `decisions/NNNN.json` | one file per evaluation: every action's status (`admissible`, `approval-required`, `blocked`), the claims and why; under the default records profile only each run's latest is kept, numbering goes on from it, and `--records trial` keeps every one ([Records profiles](./configuration.md#records-profiles)) | the governor of Loom, through takt, one per decision |
| `evidence/NNNN-<kind>-<n>.json` | one `canon-evidence/1` record: `kind`, `result`, `subject`, `subject_revision`, `observed_at`, `producer` | a validator only |
| `artifacts/` | what a validator retained beside its record, such as a gate's bundle | the validator, through takt |
| `authority.yaml` | the grants: `capability`, `decision`, `approver`, `granted_at`, `revision` | `takt approve` |
| `events.jsonl` | the run record, one JSON object per line, appended | takt; see [events.jsonl](./events-jsonl.md) |
| `run.lock` | the instant a run, a resume or a grant took hold of the case; removed when it ends | takt |
| `riders/<revision>/` | copies of what the run had the store write on the story (`test_result.json`, `implemented.md`), for takt to compare with what a step committed | takt |

An improvement turn (`self.improvement/1`) adds `judge.sha256` (the binary that judges it),
`install-root` and `forge`, each written when the turn opens.

## The records

An evidence record, the one the run above wrote:

```json
{
  "format": "canon-evidence/1",
  "id": "test-result-1",
  "kind": "test_result",
  "result": "pass",
  "subject": "implementation",
  "subject_revision": "3f0ec13bd4ca1e4723ae96c0690f402a803768bc",
  "observed_at": "2026-10-26T23:10:54Z",
  "producer": "takt.tests-run"
}
```

`subject_revision` is the commit the validator saw. The file name is the record's position in the
case (`0001`), its kind with `-` for `_`, and a counter. `producer` names who wrote it:

| `kind` | `producer` | written when |
|---|---|---|
| `test_result` | `takt.tests-run` | the test command ran |
| `gate_result` | `takt.gate-run` | the gates document ran; its bundle is in `artifacts/gate-result-<n>.bundle.json` |
| `change_record` | `takt.change-read` | takt read what the change adds against its base |
| `handover` | `takt.choice` | the harness chose to hand a change over; it names a choice, not a claim about the work |
| `pull_request`, `ci_result`, `merge_observation` | `takt.pr-open`, `takt.ci-read`, `takt.pr-merged` | the forge was asked |

A step's own claim, in the model's words, is never evidence. Only a validator writes into
`evidence/`, and Canon reads the records, not the transcript:
[Evidence and the judge](../concepts/evidence-and-the-judge.md).

The grant `takt approve` wrote:

```yaml
- capability: repository.merge
  decision: granted
  approver: human:demo
  granted_at: 2026-10-26T23:11:48Z
  revision: 3f0ec13bd4ca1e4723ae96c0690f402a803768bc
```

A grant authorises the merge of that `revision` only
([Capabilities and approvals](../concepts/capabilities-and-approvals.md)).

`artifacts/` holds a `gate-result-<n>.bundle.json` (`engineering-gate-evidence/1`: the gates
document and what each assertion saw) for each gate run, and with `--forge bot` the request and
response of the pull request, the text scanned before it was published and the commit messages and
files scanned before each push.

## Outside the case directory

| what | where | why it is not in the case |
|---|---|---|
| a harness step's transcript | `$XDG_STATE_HOME/takt/transcripts/<repo>/<case>/run-<r>/step-<n>.jsonl` (`~/.local/state/takt/transcripts/...` without it), mode 0600; under `--state-dir`, `<state_dir>/transcripts/...` | it carries what the model read, and no step may reach it; `events.jsonl` names it relative to that directory |
| files a discarded step rewrote inside an ignored directory | `$XDG_STATE_HOME/takt/set-aside`; under `--state-dir`, `<state_dir>/set-aside` | they may be the operator's own |
| a replay | `$XDG_STATE_HOME/takt/replays` | `takt replay` only reads the case |
| a promotion | `$XDG_STATE_HOME/takt/promotions` | it belongs to the binary, not the case |

## Never edited by hand

Nothing in a case directory is edited by hand. The one thing an operator does to it is
`takt approve`, which writes `authority.yaml`. A grant is bound to a revision, a record that
changes after the fact changes what Canon reads, and takt checks it:

- A step that proposes a path under `.takt/cases` (or `.engineering/cases`) is refused, and one
  that changed the directory has it put back.
- takt refuses a link at `.takt/cases` (or `.engineering/cases`) or at `authority.yaml`, and does
  not read or write through one.
- `events.jsonl` is appended and never rewritten; a line cut short is skipped on read and named
  by a `skipped` line.
- Only one run, resume or grant holds a case at a time (`run.lock`); a second is refused.

To change the outcome, run again or `takt approve`. To see what happened, read the files, or ask
takt: [`takt trace`](./takt-trace.md), `takt report`, `takt scorecard`.
