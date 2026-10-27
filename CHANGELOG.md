# Changelog

This file follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and takt follows
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). Bare tags such as `0.7.0` are
takt's releases.

## [Unreleased]

### Added

- A run lists each gate of the repository's gates document with its status in the prompt, the last line of the run and `takt report` (`--format json`); the gate runner is told base, subject and story.
- `forge: command` with a `forge_commands` section in the `takt run --config` file: the forge actions `open`, `read`, `checks` and `merge` are the argument lists the file names, run without a shell.
- Placeholders of `forge_commands`: `{branch}`, `{base}`, `{title}`, `{body_file}`, `{number}` and `{head}`; `merge` receives the head the checks were read on. The scanned push stays takt's own.
- `transport: api` with a `credential` (`{env: NAME}` or `{file: PATH}`, a mode 600 file outside the tree): the `gitlab` forge calls the GitLab REST API over HTTPS. `glab` stays the default transport.
- The `api` token is held in memory only, replaced where an answer of the host is quoted, and given to the push through a credential helper that reads it from that one command's environment.
- The run file takes `protocol`, the protocol a store story opens on, and `protocols`, the values of that protocol's declared inputs. A value the protocol does not list is refused at its line.
- Each protocol input takes the first of `--input`, the story's tag `input:<name>=<value>`, the run file and takt's built-in default; the tag `protocol:<name>/<major>` chooses a story's protocol.
- `case.yaml` records each protocol input value with its source.
- `takt protocol show <name/rev>` prints each case input a built-in protocol declares: values, description, built-in default and the run file's value with its path and line.
- `takt protocol show --format json` prints the same as data; an unknown protocol exits 1 and names the known ones.
- `examples/loom-agent/`: placeholder manifests that run takt as a Loom 0.44.0 `LoomAgent` on a kind cluster, read by a test through takt's own configuration reader.
- Site guides: install, configure a run, approve a capability, read a case, run in a container, deploy on Kubernetes.
- Site pages: what takt is built on, capabilities and approvals, how a run ends, the configuration file, the case files, the image, known limitations.
- A failed `gate.run` shows the next judgement step what each failed gate printed, at most 4 KiB per gate and 8 KiB in total.
- That output has terminal escape codes stripped and credentials withheld, and passes the secret scan when a policy is set.
- `takt run --config` takes `story`, `endpoint_token_env` and `state_dir`; `--state-dir` puts transcripts under `<state_dir>/transcripts`.
- `takt scenario run` takes `--config`.
- `scan` in the `takt run --config` file, the run's scan adapter: `gates`, the default, scans proposals with `b10x-gates` under `B10X_GATES_POLICY`; `none` scans nothing.
- `scan = "none"` is accepted only in that file, never as a flag; the run says so before its first step, and the case's `opened` event records the adapter as `scan`.

### Changed

- Every harness kind is given the repository's `AGENTS.md` and `CLAUDE.md` before the prompt, each only when the case's base commit tracks it.
- They are read as the base commit holds them, never from the step's copy; Claude Code and b10x-harness steps were given none, and a Codex step read the tree.
- The step's opening record lists them in `instructions` with path, sha256 and revision; they count against the scope files' bound of 8 files and 64 KiB.
- A tracked instruction file that cannot be given (past the bound, not text, of a credential's shape) stops the run with `StepRefused`, naming it, before the step starts.
- **Breaking:** `--harness codex` or `--harness harness` with neither `B10X_GATES_POLICY` nor `scan: none` is refused before the case opens, exit 1, naming the scan adapter; a resumed case too.
- `scan: none` with `B10X_GATES_POLICY` set is refused before the case opens: a run has one scan adapter.
- `takt scenario run --config` refuses a file that declares `scan: none`, naming the file, before anything is cut or opened.
- `--forge gitlab` scans what it publishes with the run's scan adapter, and under `scan: none` pushes unscanned; `--forge bot` always scans with `b10x-gates`.
- A step commit is a semantic message by default: a `<type>: <subject>` of at most 72 characters and a 3-4 line body from the harness, `Refs: <story id>` written by takt, never from the reply.
- `--commit-style semantic|repository|action`, `--commit-trailer`, `--commit-types` and a `commit` run-file section; a commit rule in the repository's instruction files wins over the style.
- A reply with a missing or ill-shaped `commit` falls back to `<type>: <story title>` and a `commit.fallback` event; `opened` records the style in effect as `commit`.
- `b10x-loom-sdk` moves from loom 0.43.0 to 0.45.0 with metaharness 0.20.0: a Codex step on a codex off metaharness's pin proposes its change, H9 advisory (story:codex-off-pin-proposes).
- A case keeps `python-modules.json` only when a Python answered at case open; with none, no file is written and the step guard uses the built-in lists.
- `takt run` keeps only the latest `decisions/NNNN.json` of a case by default (`--records default`, or `records` in the run configuration).
- `--records trial` keeps every decision; scenario runs and improvement cases always keep every one.
- New cases open under `.takt/cases`, with a `.takt/.gitignore` of `*` when absent; a case already under `.engineering/cases` resumes where it is, and every guard covers both roots.
- A new `.takt/` also gets `.takt/.worktreekeep` (`cases/`), so `worktree` 0.19.0 cleanup keeps every case and its archive holds them; an existing file is left alone.
- The store's `test_result` record names its case record by path, sha256, result, subject revision and producer.
- The pull request body summarises the case's evidence, steps and tokens, bounded under 60,000 bytes.
- A resume keeps its case's records profile; `default` drops only its own run's decisions, the reference's parts are percent-encoded, and the body lists the newest evidence that fits.
- The state directory holds discarded steps' set-aside files too; a `loom.agent/1` document's `runtime.state_dir` is the default when neither `--state-dir` nor `takt.state_dir` names one.
- `--harness none` takes a granted action, changing no file, not `stopped: error:` (exit 5); with `--forge none` the run then stops `NoProgress` (exit 3) (story:harness-none-takes-a-granted-action).

### Fixed

- A base branch under takt's reserved prefix `takt/` that is no case's is refused before a case opens, exit 1, naming the prefix and a rename; it was refused as a case's branch with no base recorded.
- A gate's command finds a bare program name on `/usr/local/bin:/usr/bin:/bin`, or on the `PATH` its `env` declares, never on the user's `PATH`; a name found nowhere refuses the gate.
- A gate whose script holds the shell's `:` builtin is run and judged on its own exit; a path a gate names that starts with `:` is looked up literally.
- The first `tests.run` in a fresh `HOME` is no longer refused for the cache cargo makes in `~/.cargo`; a program in `~/.cargo/bin` still stops the run.
- A `tests.run` that writes an untracked `Cargo.lock` is refused with the remedy named: commit it, ignore it (`.gitignore`), or give `--test-cmd` a command that does not write it.
- The endpoint token staging tests pass when the temporary directory is writable by others without the sticky bit.
- A judgement step whose session ends without its action line is tried again, 3 times at most; each try counts against `--iterations`.
- Each such try is charged the tokens its session reported; the run ends exit 5 after the third, or exit 3 if a try spent the budget.
- The site's protocol viewer shows all eight protocols `takt protocols` lists, `software.change/1` among them, and writes no predicate as raw JSON.
- The site says what runs under Seatbelt on macOS since 0.10.0, which scan and forge adapters a run can have, and that `bot` is beyond10x's github.com route.

### Security

- A proposal is refused when a path it names holds text of the shape of a credential, as its content and summary already were, before any refusal that would name the path.

## [0.14.0] - 2026-10-26

A run whose tests are red stops instead of spending model calls: at a red base before the first
model call, and after two test runs in a row at new revisions that fail the same tests.

### Added

- `takt run` and `takt scenario run` stop `RedBase` (exit 3) when a case's first test run fails before any step changed its base, with no model call, naming the failed tests and the rerun hint.
- They stop `RepeatedTestFailure` (exit 3) when two test runs in a row, each at a new revision, fail the same tests; the stop names the tests.
- A resume after either stop runs the tests again before any model call and stops again while they fail the same way.
- `--continue-past-red-base` (`takt run`, `takt scenario run`; `continue_past_red_base` in `takt run --config`) goes on past a red base.

### Changed

- **Breaking:** a run whose base fails its tests stops `RedBase` by default; `--continue-past-red-base` goes on as before.
- A failing `tests.run` summary names up to 64 failed tests (cargo, Python unittest, pytest) and cargo's rerun hint; its printed `effect:` line stays bounded.

## [0.13.1] - 2026-10-26

0.13.0's release run failed on macOS, so 0.13.0 has no GitHub Release; 0.13.1 is the first release that ships its
changes, with the macOS fix below.

### Fixed

- `--endpoint-token-env` stages its key on macOS: links in a candidate's ancestors are resolved first, so the temporary directory under `/var`, a link, passes; a candidate that is a link is refused.
- Three `--endpoint-token-env` tests that read `/proc` or write outside the tree from the test command run on Linux only.

## [0.13.0] - 2026-10-26

takt runs as a Loom agent: `--config` reads the `loom.agent/1` document, `--endpoint-token-env` takes the key from the
environment, `--story` names the story, and the image tests and gates what it builds; loom 0.43.0.

### Added

- `takt run --story <ID>` resumes or takes the named story instead of the frontier's next; one not listed or not ready is refused before any case opens, exit 1, naming why.
- `takt run --config` reads YAML (`.yaml`, `.yml`) beside TOML, and a `loom.agent/1` document by its `takt` member: the file Loom mounts for a `LoomAgent` at `/config/agent.yaml`.
- `--endpoint-token-env <NAME>` (`[credential] env`, `openai-responses` only) stages a variable's key as a file, removed at exit or on SIGTERM, SIGINT, SIGHUP; no child sees it (substrate: README).
- The image ships `b10x-gates` 0.1.23, `pkg-config` and the OpenSSL headers, so a run in it scans its edits and tests a repository that builds against OpenSSL.

### Changed

- **Breaking:** the image's entrypoint reads `/config/agent.yaml`, not `/config/run.toml`; `--config` refuses a file whose extension is not `.toml`, `.yaml` or `.yml`.
- The image is built on Debian trixie with the Rust toolchain, unwritable by the run, linked into `/usr/local/bin` where a gate finds it; no `CARGO_HOME`, so cargo caches under `$HOME/.cargo`.
- The image runs takt under `tini`, so `docker stop` or a pod's stop ends it on SIGTERM rather than SIGKILL.
- **Breaking:** `takt protocols` lists the three protocols as engineering-protocols 0.10.0, whose built-ins they now are, names them: `story-delivery/1`; either spelling is read.
- **Breaking:** `b10x-loom-sdk` moves from loom 0.41.0 to 0.43.0 with metaharness 0.19.0, so `--harness harness` needs b10x-harness 0.20.0 on the `PATH`; the image ships it.

## [0.12.0] - 2026-10-26

`takt run --config` takes the run's configuration from a TOML file, and a `Dockerfile` builds a container image
that runs takt on a mounted repository and configuration; loom 0.41.0.

### Added

- `takt run --config <FILE>` reads `iterations`, `hours`, `harness`, `model`, `endpoint_file`, `forge` and `test_cmd` from TOML; a flag wins over the file, the file over the default.
- A relative `endpoint_file` in a run configuration is read beside the file; any other key is refused before the case opens, naming the file and the line, never a value.
- `Dockerfile` builds an x86_64 image with `aep`, `b10x-harness` and `ess` pinned by SHA256, running `takt run --repo /workspace --config /config/run.toml` as uid 10001.

### Changed

- `b10x-loom-sdk` moves from loom 0.40.0 to 0.41.0; metaharness stays at 0.15.0. loom 0.41.0 marks nothing breaking.
- The `ess` that `task check` runs in CI (`check.yml`, `darwin.yml`) moves from 0.69.0 to 0.69.2, and the specification requires ess 0.69.

## [0.11.1] - 2026-10-24

`--harness harness` reads an action line that b10x-harness streams in several text-delta events as one line,
through loom 0.40.0; the `ess` the checks run moves to 0.69.0.

### Changed

- `b10x-loom-sdk` moves from loom 0.39.1 to 0.40.0; metaharness stays at 0.15.0. loom 0.40.0 marks `model::run::SelectActionOutcome` breaking, and takt names it nowhere, so no source changes.
- The `ess` that `task check` runs in CI (`check.yml`, `darwin.yml`) moves from 0.68.0 to 0.69.0, the release loom 0.40.0 requires.

### Fixed

- `--harness harness`: an action line streamed in several `text-delta` events is read as one line; on 0.11.0 the step ended with "the session ended without the JSON line naming its action".

## [0.11.0] - 2026-10-24

`--forge gitlab` delivers through `glab` on a GitLab host, and `--harness harness` takes its endpoint from flags or
a file, with a token file or a secret's name handed to b10x-harness; loom 0.39.1 with metaharness 0.15.0.

### Added

- `--harness harness` takes its endpoint from `--endpoint-base-url`, `--endpoint-wire`, `--endpoint-model`, `--endpoint-token-file` and `--endpoint-token-pointer`.
- `--endpoint-file` gives the same as a TOML file (`base_url`, `wire`, `model`, a `[credential]` table); a flag wins over it, and with neither the endpoint is the one before.
- An `--endpoint-*` flag given with a harness other than `harness` is refused before the case opens.
- An endpoint's credential is a subscription token file with an optional JSON pointer, or a secret's name (`--endpoint-secret <namespace>/<name>`); takt reads no secret.
- A missing token file, or a pointer that resolves to no token, exits 1 before the case opens, naming the path and the pointer.
- A secret's name is handed to b10x-harness by name, as the session executor's `api_key_secret`, and the harness resolves it; takt reads no secret and opens no store.
- A `--harness harness` run records its endpoint on the `opened` line by reference: base URL, wire, model, the token file's path (`~/…` under HOME) and pointer or the secret's name; never a token.
- `--endpoint-base-url` takes `https://`, or `http://` to a loopback host only (`localhost`, `127.0.0.0/8`, `::1`).
- A base URL with a userinfo, a query or a fragment is refused before the case opens, naming the part and repeating none of it.
- A base URL other than the default needs a credential by flag or file: the default claude.ai token goes to `https://api.anthropic.com/v1` only.
- A relative `--endpoint-token-file` is read against the current directory; the harness gets it, and the case records it, as an absolute path.
- `--forge gitlab` delivers through `glab` on a GitLab host: it pushes over HTTPS under `glab auth git-credential`, and opens, reads and merges the merge request through `glab api`.
- `--forge gitlab` without `glab` on the program path, or without `glab auth status --hostname <host>` exiting 0, is refused before the case opens, naming `glab` and the host.
- `.gitlab-ci.yml` and everything under `.gitlab/` at the repository's root are on the judge surface, as `.github/**` is; a file an `include: local:` names outside both is not.
- Through `--forge gitlab`, `ci.read` waits on the merge request's head pipeline as on GitHub's checks, under the same poll and `--hours`.
- A head pipeline that is not the pushed head's, or that names no commit, is pending; one that ends writes `ci_result` `pass` (`success`, `skipped`) or `fail` (`failed`, `canceled`).
- Through `--forge gitlab`, a `glab api` call that exits non-zero is an error naming the call, GitLab's message and the HTTP status; only a 404 on the merge target's branch means no such branch.
- An HTTPS `origin` on a port of its own (`https://gitlab.example:8443/group/repo.git`) keeps the port for the push and for every `glab --hostname`.
- `takt scenario run --forge gitlab` is refused before anything is cut or opened: a scenario's trial base is created only through the bot route.
- A proposal holding a GitLab token (each prefix GitLab's documentation lists, from `glpat-` to `glffct-`) or glab's `token:` line is refused before the edit effect, as a GitHub token is.

### Changed

- `b10x-loom-sdk` moves from loom 0.39.0 to 0.39.1, whose session executor carries an endpoint's `api_key_secret` to b10x-harness; metaharness stays at 0.15.0.
- Without `--forge`, an `origin` on a host other than github.com gets the forge `gitlab` when `glab` is logged in to it, and otherwise `forge: none` with what `glab` said in one line.
- Without `--forge`, `takt scenario run` on an `origin` that is not on github.com runs with no forge and says so in one line; `glab` is not asked.
- The test command run under `--confinement none` no longer inherits `GLAB_CONFIG_DIR`, as it no longer inherits `GH_CONFIG_DIR`; a harness child never did, its environment being built from nothing.

## [0.10.1] - 2026-10-23

`--harness claude` proposes on Claude Code 2.1.309 and `--harness codex` runs on Codex 0.160.0, through loom 0.39.0
and metaharness 0.15.0.

### Changed

- `b10x-loom-sdk` moves from loom 0.37.0 to 0.39.0, which runs harness sessions on metaharness 0.15.0; takt names none of the types either release marks breaking.

### Fixed

- `--harness claude` proposes on Claude Code 2.1.309 as well as 2.1.304 and 2.1.305; 0.10.0 held back every proposal on 2.1.309.
- `--harness codex` runs on Codex 0.160.0 as well as 0.153.4.

## [0.10.0] - 2026-10-23

The minor version moves because takt runs on macOS again: 0.10.0 ships `aarch64-apple-darwin`, which 0.7.0 to 0.9.0
did not, and on macOS each step takt confines runs under Seatbelt, with the differences from Linux listed as Breaking.

### Added

- The release builds `takt` for `aarch64-apple-darwin` again, on `macos-15`, beside the two Linux targets.
- On macOS the test command and a promotion's build run under `/usr/bin/sandbox-exec`; with no `sandbox-exec` both are refused by name, never run unconfined.
- On macOS a process a child of takt left running is killed by its `TAKT_CHILD` mark.
- On macOS what a harness session's program left running in takt's process group is killed when the session ends.
- On macOS what a promotion's build or `--version` run left running is killed by its `TAKT_PROMOTION` mark.
- On macOS `scripts/check-finding` builds and runs the predicate under `sandbox-exec`: no network, the homes hidden, writes only to its own directories.
- On macOS with no `sandbox-exec`, `scripts/check-finding` builds nothing and gives no result.
- On macOS no confined step starts a program outside its sandbox: no launchd job, no LaunchServices open, no Apple event.
- On macOS the test command connects to no launchd socket under `/private/tmp`, the ssh agent's among them.
- On macOS the test command writes no hook nor configuration of its git directories, nor the files that name them.
- The release publishes only after the macOS tests pass on the tagged commit.

### Changed

- **Breaking:** On macOS a promotion's build sees the clone, its target and the fresh root at their host paths, not at fixed paths inside a sandbox as under bubblewrap.
- **Breaking:** On macOS a promotion's build reads every file outside the user's home, not only the clone, toolchain, registry cache and system directories.
- **Breaking:** On macOS the test command's `TMPDIR` is a directory in its fresh home, and neither it nor a promotion's build may write the user's temporary and cache directories under `/var/folders`.
- **Breaking:** On macOS the test command is denied a registry login, `Library/Keychains`, the runtime directory and a bus socket by path only.
- **Breaking:** On macOS a hard link to such a file elsewhere, and the keychain daemons reached over Mach, are not denied.
- **Breaking:** On macOS a process that clears `TAKT_CHILD` from its environment and leaves the process group outlives the run; on Linux takt adopts and kills it.
- **Breaking:** On macOS a harness session's program runs in takt's process group, not a session of its own: a process it leaves running that leaves that group or session outlives the run.
- **Breaking:** On macOS `scripts/check-finding` runs the predicate in a process group, not a PID namespace: a process that leaves the group outlives the check.
- **Breaking:** On macOS the predicate of `scripts/check-finding` reaches the host's System V and POSIX IPC and its Mach services but those that start a program; there is no IPC namespace.
- **Breaking:** On macOS the predicate of `scripts/check-finding` has no `/tmp` of its own: the host's `/tmp` is out of its sight, and its `TMPDIR` is a directory in its home.
- **Breaking:** On macOS the protocol sources in the predicate's home are a link to the caller's, which Seatbelt keeps read-only, not a read-only bind.
- **Breaking:** On macOS a write the predicate of `scripts/check-finding` is refused fails with "Operation not permitted", not "Read-only file system".
- `b10x-loom-sdk` moves from loom 0.36.0 to 0.37.0; takt uses none of the confinement types its CHANGELOG marks breaking.

## [0.9.0] - 2026-10-23

The minor version moves because harness sessions run through Loom's session executor and takt no longer runs Canon's
evaluation itself, which changes the error a refused or unreadable case ends with.

### Changed

- **Breaking:** No macOS binary in 0.9.0 either: loom-sdk 0.36.0 builds on macOS, but takt's tests do not yet pass there.
- ESS moves from 0.67 to 0.68: `ess/ess-inputs.yaml` requires ess 0.68.
- `b10x-loom-sdk` moves from loom 0.34.0 to 0.36.0, which builds on macOS.
- Harness sessions run through Loom's session executor at 0.36.0 with metaharness 0.13.0; takt's own crate `b10x-session-executor` is gone.
- Under `--confinement substrate` the test command runs on Substrate 0.10.0, which loom-sdk 0.36.0 links; 0.8.0 linked Substrate 0.7.18.
- After each harness step, the case's `observations.json` holds the session's `AgentSession` record: kind, action, vendor session id and each file's path and sha256.
- `--harness claude` proposes only on Claude Code 2.1.304 or 2.1.305: on any other version metaharness 0.13.0's memory-canary floor (H11) holds the proposal back.
- A session that writes a file the tree already holds outside its copy, such as an ignored one, now ends the run with an error ("the workspace changed under the session"), not a `StepRefused` stop.
- takt no longer runs Canon's evaluation itself: a case Loom's governor refuses to decide ends the run with its refusal ("the governor of Loom does not decide the case <id>: …"), not Canon's.
- An evidence record under a case that does not read ends the run with "evidence is not a canon-evidence/1 document", naming the record's file.

## [0.8.0] - 2026-10-22

The minor version moves because takt's cases are decided by Loom's governor, the loop runs on loom-sdk's
`run_until_blocked`, and two stops now come before a spent budget: exit 4 and exit 7 where 0.7.0 gave exit 3.

### Added

- `--confinement none|substrate` (default `none`): every child process of a run starts through one adapter.
- Under `--confinement substrate` the test command runs in loom-sdk's `SubstrateRunner`; a test command its profile does not serve, and a gate's commands, are refused by name, never run unconfined.
- A Codex judgement step is given `AGENTS.md` and its story's scope files as metaharness context files, named in one prompt line, within 8 files, 64 KiB and the 131,072-byte launch argument.
- A Codex step's `takt.prompt` record lists each context file it was given: path, sha256, bytes and channel.

### Changed

- **Breaking:** No macOS binary in 0.8.0 either: loom-sdk 0.34.0 does not compile on macOS, so macOS does not return in 0.7.1 as 0.7.0 said.
- `b10x-loom-sdk` moves from loom 0.28.0 to 0.34.0, which pins substrate 0.7.18; `metaharness` moves from 0.10.1 to 0.11.0.
- Every evaluation of a case goes through Loom's governor over the case's files; each `decisions/NNNN.json` is the `canon-decision/1` it rendered.
- A case gains a revision and `observations.json`, which the Commission runtime records.
- Each validator performs as a Commission effect, and every evidence record reaches the case through its evidence port; a record from anything but the run's validators is refused.
- `takt run` takes every step through one call of loom-sdk's `run_until_blocked`, and its stop comes from how that call ended.
- **Breaking:** A case that only a grant moves stops `ApprovalRequired` (exit 4) before any budget or low-disk stop; 0.7.0 stopped on a spent budget (exit 3) first.
- **Breaking:** A case whose open claim has no performer stops `Unperformable` (exit 7) before any budget or low-disk stop; 0.7.0 stopped on a spent budget (exit 3) first.
- A turn's judge is checked through the path the platform reports for the running binary off Linux; Linux reads `/proc/self/exe` as before.
- ESS 0.67 is required.

### Fixed

- `takt replay` offers a step taken after a grant the granted action again, so it no longer refuses on a prompt mismatch.
- A denial in `authority.yaml` wins over a grant of the same capability, and `authority.yaml` is never read or written through a link.
- A process sweep no longer kills a gate command another thread is waiting on.

## [0.7.0] - 2026-10-21

The minor version moves because every harness step runs through a metaharness session, `--harness` takes `codex` and `harness`
beside `claude`, and the gate's reader fails closed on lines 0.6.0 read.

### Changed

- **Breaking:** No macOS binary in 0.7.0; macOS returns in 0.7.1.
- `--harness claude` runs Claude Code through a metaharness session (`HermeticMode::Strict`) in a scratch copy of the case tree, offered only the file tools.
- **Breaking:** A Claude step needs `$HOME/.claude/.credentials.json`; without it the session gets no login (not from `CLAUDE_CONFIG_DIR`, probably not from the macOS Keychain) and probably fails.
- The session's change comes back as a proposal that takt writes to the case tree only after the step guard admits it; a refused one ends the run `StepRefused`.
- A gap the hermetic floor reports holds the proposal back; H9 and H1a are reported by row id in the step summary until metaharness declares Claude Code 2.1.304.
- `takt replay` keeps the print-mode harness in its scratch worktree.
- Each judgement step's prompt carries the change the case has made so far: the diff of its accepted steps from the base commit, about 8 KiB at most, fenced as data.
- `--harness` takes `claude`, `codex` (codex-cli 0.153.4) or `harness` (b10x-harness), each through one metaharness session seam.
- Codex runs hermetic `On` with H9 gating, its sandbox writing only to the copy with no network, its home and XDG directories in the session's scratch.
- **Breaking:** A proposal is refused whole, `StepRefused`, on a credential shape in its files or summary, a `b10x-gates scan-text` refusal or a scan that cannot run; 0.6.0 completed such steps.
- With no gates policy a `codex` or `harness` proposal is refused unscanned; a `claude` one is checked for credential shapes only.
- The session's scratch copy reaches the case tree's git state through a `.git` file naming a git directory outside it; the tree's `.git` is never written.
- On the Claude route a judgement step's prompt names its story's `## Scope` files as file mentions (`@todo.py`), at most 8 files and 64 KiB.
- `b10x-loom-sdk` 0.27.0 → 0.28.0 is a dependency of `b10x-takt`; `metaharness` 0.10.1 comes through the new crate `b10x-session-executor`.
- `task site-check` fails a page on text under 12 px, console errors, failed requests and an ignored stored theme.
- The metaharness session starts Claude Code with `--restricted`, as 0.6.0's print mode did, so no auto-memory: a scenario 1 request is 754-757 tokens smaller than without it in three live pairs.
- CI runs `task check` as three parallel jobs (`task check-static`, `test-cli`, `test-library`); test builds compile `sha2` at opt-level 3.
- CHANGELOG.md follows Keep a Changelog 1.1.0; `scripts/release-notes <version>` writes a release's GitHub notes, and `task check` runs its `--check`.
- takt's specification requires ess 0.66 (`ess/ess-inputs.yaml`), and CI installs ess 0.66.0.

### Fixed

- `takt scorecard --against` prints a `median <metric>` line only for a metric with a per-case figure, no longer wall-time medians as `step_tokens`.
- The site fixes the defects of its 2026-10-20 screen but the AEP card's family styling.
- On the runs page no chart line runs through a label, and on a phone each chart's title and legend stay in view while only the plot scrolls.
- A wrapped entry of a CLI help list continues under its description, and code blocks have the same space above and below as at the sides.

### Security

- **Breaking:** The gate's reader fails a line closed when a tree file handing off to a known program sets `PYTHON*` or an environment, changes directory or writes `sys.path` (a comment counts).
- **Breaking:** The reader fails it closed, too, if that file hides such a name (`getattr(`, `import sys as`), takes a word before the program, is not Python or shell, or is unreadable or over 1 MiB.

## [0.6.0] - 2026-10-20

The minor version moves because a run whose harness step recorded no tokens has unknown tokens, a run records the layers it ran
on, and the gate's reader fails closed on lines 0.5.0 read.

### Added

- A run records its layers (ADR 0101): the `opened` line's `layers` names `takt`'s revision, each harness step's line the harness `version` and the `model`.
- `takt report` attributes tokens, cost, wall time and failures by layer, per run and in total, each figure naming its line (`events.jsonl:<n>`).
- In `takt report` a figure no layer's record states prints `not reported`, never 0; trace operations carry their record line and the trace header the layers.

### Changed

- **Breaking:** `takt scorecard` prints the layers of the runs it scores and refuses runs whose layers differ, naming the layer, unless `--across-layers`.
- **Breaking:** A run whose harness step recorded no tokens, or 0, has unknown tokens (0.5.0's known limit); `takt scorecard --against` and `takt promote` refuse such a set, exit 1.
- **Breaking:** The metrics `tokens` and `step_tokens` move to definition version 2: a baseline of 0.5.0 or earlier is compared only after `takt promote --rebaseline`.
- `b10x-loom-sdk` 0.27.0 is a dev-dependency; Loom's governor decides takt's 30 recorded case decisions as takt did but where a grant is missing.

### Security

- `gate.run` compares what a gate command's Python files import from every import directory, a `PYTHONPATH` set anywhere on the line included.
- `gate.run` reads `cd`, `env -C`, a redirection-only `exec`, `taskset`, `chrt`, `unbuffer`, `script -c`, `uv run`, `env -S` and `pytest FILE`: 0.5.0's known limit of 11 forms and wrappers closes.
- **Breaking:** The gate's import walk reads at most 64 Python files a command; the cap, `importlib`, `__import__(`, `exec(`, `eval(` or `xargs` input fails closed.
- **Breaking:** The step guard reads these forms and fails closed in five more places (`doas python3 …`); while a test command line is unreadable, it refuses steps changing Python, shell or programs.

## [0.5.0] - 2026-10-20

The minor version moves because a run can stop `ActiveTime`, a stored baseline is judged again only after `takt promote --rebaseline`,
a commit identity the forge's scan refuses stops a run at open, and a gate line the reader cannot read fails closed.

### Added

- `takt run` and `takt scenario run` take `--active-seconds <n>`, a budget on wall time less forge waits; over it the run stops `ActiveTime`, exit 3, the case open.
- `takt scorecard --against <saved> --steps <action>[,...]` judges the steps a candidate targets: better on them but worse on the run exits 3, not better exits 4.
- `takt replay --case <dir> --step <n> [--times <k>]` runs a recorded judgement step again in a scratch worktree and records each replay as `takt-replay/1`.
- `takt replay` exits 3, running nothing, when the rebuilt prompt differs from the recorded transcript, and 5 when the harness failed; it follows no link.

### Changed

- **Breaking:** `takt run` and `takt scenario run` through the bot route scan the commit identity at open; an identity the forge's scan refuses exits 1 before step 1.
- **Breaking:** The scorecard gains the metric `step_tokens` (definition version 1); a baseline stored before it is refused until `takt promote --rebaseline`.
- takt's own gates run through `scripts/gate-exec`, now a Python program (`/usr/bin/python3 -I`, standard library only) that the gate compares whole at the base.
- **Breaking:** `scripts/check-finding` runs one check at a time per cache directory and refuses, exit 1, under `CHECK_FINDING_MIN_FREE_GIB` (20) GiB free.
- The trial agent's profile adds a pre-flight, a token and an active-time cap per run, and an early stop once a set's verdict is decided.
- Known limit: run-level tokens still read a harness step that recorded no tokens as 0, so a missing record reads as a saving; no promotion is made on 0.5.0.
- Known limit: 11 shell forms still let a change decide the gate, 6 of them `PYTHONPATH` set on the line, by `export` or in a sourced file, and `cd dir && python3` with `-m x`, `-c …` or `- < x.py`.
- Known limit: the other 5 are `env -C dir python3 -m x`, a script on `xargs`'s input, `exec 0< FILE; sh`, `exec < FILE; sh` and `exec < FILE; bash` (story:the-gate-reads-every-shell-form).
- Known limit: wrapper programs the reader does not know (`taskset`, `chrt`, `unbuffer`, `script -qec`, `uv run`, `env -S`, `pytest`) and `env PYTHONPATH=…` slip too.
- Known limit: 0.4.0's known limits on build-time code, squash merges, the forge's repository, tipless grants, candidate refs and `takt view` still hold.

### Fixed

- `takt scorecard --steps` reads a harness step that recorded no tokens as unknown, not 0; a targeted step unknown on either side is refused, exit 1.
- `takt run --help`: `--forge none` names the `Unperformable` stop, exit 7, when only a forge action is left.
- A step that writes `.engineering/commit-identity.conf` is refused naming the commit identity file, no longer as a write into the planning store (0.4.0's known limit).

### Security

- **Breaking:** `gate.run` fails closed on a line its reader cannot read with certainty (a glob, an unknown option or redirection, a second script input), naming why.
- `gate.run` compares a Python script run through `sh -c`, a shell script, `exec` or a shell reading a tree file, and what it imports, as it compares one run directly (0.4.0's known limit).

## [0.4.0] - 2026-10-20

The minor version moves because a `DiskLow` stop exits 8, a 0.3.x baseline is judged again only after `takt promote --rebaseline`,
and a run that pushes is refused without a commit identity.

### Added

- A `--forge none` delivery that stops `Unperformable` at `pr.opened` hands its change over as a candidate, `refs/takt/candidates/<case-id>`, printing its id.
- `takt view <dir> [--output <file>]` writes one offline HTML page of the `takt-trace/1` runs under a directory, one lane per run on a shared time axis.
- `takt view`'s page derives from the MIT-licensed Context Lens dashboard and embeds Vue (MIT); the notices are in `viewer/`.

### Changed

- **Breaking:** A run that stops `DiskLow` exits 8, not 3, for `takt run` and `takt scenario run`; its case stays open.
- **Breaking:** takt's commits carry `--commit-identity`, else `.engineering/commit-identity.conf`, else `takt <takt@localhost>`, never the operator's git identity.
- **Breaking:** A run through the bot route is refused at open, exit 1, without a commit identity, and so is the resume of a case opened by 0.3.x.
- **Breaking:** The scorecard's outcome metric is at definition version 3; a version 2 baseline (any 0.3.x one) meets a candidate only after `takt promote --rebaseline`.
- **Breaking:** A self-improvement turn is refused at open, exit 1, when the mainline's tip already holds its candidate or the tip cannot be read.
- **Breaking:** A turn resumed through another forge than the one it recorded is refused, exit 1, naming `--forge <recorded>` as the way past.
- **Breaking:** `candidate.merge_judge_surface` records `merged` only for a candidate that reached the mainline after the grant; a 0.3.x turn's such merge is refused.
- Known limit: build-time code (a build script, a procedural macro) is not closed by comparing files, so the merge of such a change stays the owner's to grant.
- Known limit: a candidate squashed or rebased onto `main` before the grant can still be recorded `merged` (story:a-squash-merge-before-the-grant-is-refused).
- Known limit: a turn records its forge by name only, so a grant's tip and the merge can be read from two repositories (story:a-turn-pins-the-repository-of-its-forge).
- Known limit: a judge-surface merge refused because its grant records no tip names no way past (story:a-tipless-judge-grant-names-its-way-past).
- Known limit: nothing keeps the commit an open turn was opened on once a resume deletes its candidate ref (story:a-candidate-stays-reachable-while-a-turn-names-it).
- Known limit: `takt view` joins a transcript by path suffix and writes through a linked `--output` (story:view-joins-own-transcripts-and-writes-a-private-page).
- Known limit: a step refused for writing `.engineering/commit-identity.conf` is told it wrote into the planning store (story:commit-identity-refusal-names-the-identity-file).
- Known limit: a gate running Python through a shell (`sh -c "python3 …"`, a shell script) has neither the script nor its imports compared (story:gate-compares-python-run-through-a-shell).
- Known limit: nor is a module that `python3 -c` imports from the directory the command runs in.
- takt's repository configures its commit identity in `.engineering/commit-identity.conf` and lists `scripts/**` on its judge surface.
- The agent profiles describe one dispatcher session that runs the waves and one trial agent that runs the trials.

### Fixed

- `takt run` and `takt scenario run` on an empty home no longer have their first harness step refused for `~/.local`: the transcripts directory is made at open.
- No step is refused for the transcripts takt writes when `PYTHONUSERBASE`, `PYTHONPATH` or `XDG_STATE_HOME` places them inside a Python import directory.
- A file or a link where the transcripts directory goes is named as such, and the directories made before that failure no longer get the next step refused.

### Security

- `gate.run` with a project file compares at the case's base every file a gate command takes as an argument, the tool configuration, `[lints]` and the judge surface.
- `gate.run` compares at the base what Python loads for a gate's commands: the modules, `*.pth` files and packages of a script's directory, of `-m` and of `PYTHONPATH`.

## [0.3.0] - 2026-10-20

The minor version moves because `takt promote` now refuses scorecards that 0.2.x accepted.

### Added

- The `Unperformable` stop: no judgement step is taken, the case stays open, and a scorecard ranks it with the budget stops.
- A run's stop line in `events.jsonl` records `completed`; a scorecard takes that word for a stop it does not know, so an unknown stop never counts as completed.
- **Breaking:** `takt run` and `takt scenario run` stop `DiskLow`, exit 3, under `--min-free-gib` (default 10, `0` off) free on the tree's or `CARGO_TARGET_DIR`'s disk.
- Each harness step's transcript is kept under `$XDG_STATE_HOME/takt/transcripts/`, mode `0600`, capped at 16 MiB; no validator reads it.
- takt's repository has a gates document, `.engineering/gates.yaml`, with four gates run through `scripts/gate-exec`, so a delivery on takt reaches `pr.open`.

### Changed

- **Breaking:** `takt promote` judges a candidate against the latest record and against the start of its series; a scenario worse than either is refused, exit 3.
- **Breaking:** `takt promote`, `scorecard --against` and `scorecard.compare` cover every scenario of either side; none, or one only the candidate has, is refused.
- **Breaking:** A new scenario joins the corpus only through `takt promote --rebaseline`, and `--baseline-only` refuses a scorecard with no scenario case.
- **Breaking:** A run whose open obligation only an action nobody performs can decide stops `Unperformable`, exit 7, not `NothingAdmissible`, exit 3.
- A wait on something local (a saved scorecard, a promoted binary, a merge into the local `main`) stops `Waiting`, exit 6, right after the step that observed it.
- `takt run --install-root` on a resumed turn is accepted when it names the root the turn was opened with.
- Claude runs with `--output-format stream-json --verbose`.
- Known limit: `Unperformable` ranks with the budget stops, so a `--forge none` run that stopped `NoProgress` under 0.2.x ranks one higher against an older baseline.
- Known limit: the `lint` gate runs `cargo clippy`, which compiles and runs build code a step wrote, so the merge of such a change stays guarded by the owner.

### Fixed

- `takt scenario run` without the bot route numbers a trial with 22 digits (the second and a random nonce), so fresh clones no longer all cut `trial/0`.
- `takt scorecard --help` names the held-out check, not `events.jsonl` alone, as where a scenario run's outcome is read from.
- The wall-clock tests that failed only on a loaded host hold under load.

## [0.2.3] - 2026-10-19

### Changed

- A Claude step gets `--tools Read,Edit,Write,Glob,Grep` beside `--allowedTools`; a step's first request fell from 12,554 to 5,550 prompt tokens on scenario 1.

### Fixed

- `MultiEdit` is no longer in `--allowedTools`: Claude Code 2.1.303 has no such tool.

## [0.2.2] - 2026-10-19

### Added

- A `self.improvement/1` turn reaches `accepted` on validators' records: the candidate merged, no scorecard regression, the candidate promoted.
- `self.improvement/1` is built in: `--protocol self.improvement/1`, listed by `takt protocols`.
- The merge validators record `merged` when the case holds the grant and the candidate is an ancestor of the mainline; they observe the merge, never make it.
- `binary.promote` records `promoted` only from a promotion record of the candidate whose `binary_sha256` is that of `<install root>/bin/takt`.
- `takt run --install-root <dir>` names where a turn reads the promoted binary, else `CARGO_INSTALL_ROOT`, then `CARGO_HOME`, then `~/.cargo`.
- `takt promote` keeps the binary it replaces at `$XDG_STATE_HOME/takt/judges/<sha256>` and names it, so a turn is finished with that binary.

### Changed

- **Breaking:** A turn opens and resumes only on the embedded `self.improvement/1`; another document with its id is refused.
- **Breaking:** The case input `surface` is computed at open from the baseline's `.engineering/judge-allow.yaml`; a given `surface` that differs is refused.
- **Breaking:** A turn records the sha256 of the binary that opens it, and every resume by another binary is refused before any step.
- **Breaking:** `--gate` defaults to the protocol's first action that requires a capability and that the case's inputs do not block for good.
- A `candidate.merge_judge_surface` grant counts as `repository.merge` does for a judge surface: the owner's, or a role's an unrevoked delegation names.
- A turn opened without `--intent` takes its finding's title as its intent, read on the mainline; a turn writes nothing to the planning store.
- Known limit: a turn's judge check is the running binary checking itself, so a binary built without it would resume the turn and judge it.
- Known limit: under `--forge none` the operator's local `main`, and the `.engineering/delegations.yaml` it holds, decide what counts as merged.

### Removed

- **Breaking:** The action `candidate.deliver`: a turn's candidate is given at open.

### Fixed

- A step's line in `events.jsonl`, its tokens included, is written before the revision update that can end the run.
- The loop-case test's forged-commit precondition no longer depends on the host's commit-graph or gc settings.

### Security

- `scripts/check-finding` runs git with `GIT_NO_REPLACE_OBJECTS=1`, so a replace ref in a scenario checkout cannot stand in for a seed.

## [0.2.1] - 2026-10-19

The first published release of the 0.2 line; its changes are those listed under 0.2.0, which was tagged but not published.

### Fixed

- `finding_scripts.rs` accepts dash's "Directory nonexistent" beside bash's "No such file or directory", which failed 0.2.0's release gate on CI.

## [0.2.0] - 2026-10-19

### Added

- `takt run --protocol protocols/self-improvement/1.yaml --input finding=<story id> --input candidate=<commit>` opens a turn on the promoted baseline.
- A turn is refused at open unless the running binary is the promoted one and the candidate leaves the finding's story and both finding scripts unchanged.
- Validators `finding.file`, `scenario.reproduce`, `scenario.verify` and `scorecard.compare` end a turn `unreproducible` or `declined`; none reaches `accepted` yet.
- `--scenarios <dir>` names the scenario checkout a finding's check runs in.
- `loop.improvement/1` cases open with `--input loop_baseline=<commit>`; `loop.measure`, `loop.definition.check` and `loop.compare` write their records.
- A predicate may name `healthy_exit`, 0 when absent.
- `takt promote --rejudge` judges a refused revision again on a scorecard of new cases; a second refusal is recorded beside the first.
- `takt promote --rebaseline <file>` re-scores the baseline's runs under a newer metric definition and records the replaced scorecard's sha256.
- `promotions.head` anchors the promotion chain's latest record: a deleted or replaced latest record is refused; a store with no head is read on trust.
- `task check` runs 12 property tests with a fixed seed; `task fuzz` runs four cargo-fuzz targets for 60 s each on nightly.

### Changed

- **Breaking:** A merge that changes a judge surface (every path but `docs/**`, `README.md` and `CHANGELOG.md`) needs the owner's grant or a delegated role's.
- `.engineering/judge-allow.yaml` and `.engineering/delegations.yaml` are read at the merge target's tip, and a change to either merges under a `human:` grant alone.
- **Breaking:** The judge guard needs git 2.38 or later (`git merge-tree --write-tree`) and refuses a conflict, a base off the target or a pull request on another base.
- **Breaking:** `takt approve --by` takes only `human:<id>` or `agent:<role>`.
- **Breaking:** `scripts/check-finding` says `not_reproduced` only for the predicate's healthy exit, and `error` for a status the predicate does not name (0.1.0's known limit).
- **Breaking:** `scripts/file-finding` refuses a predicate whose command never runs `$TAKT`, and an `output` shorter than 8 characters.
- **Breaking:** `takt scorecard --save` takes `--revision` only as the full 40-hex commit id and refuses a run whose `opened` line names another or `-dirty`.
- **Breaking:** A scenario case's outcome comes from `held-out.txt` beside its record, `failed` unless its first line is `PASS`; a case without one is refused (0.1.0's known limit).
- **Breaking:** `takt scorecard --against` gives no verdict, exit 1, on fewer cases a side than the rule's trials, and `takt promote` refuses such a scenario.
- **Breaking:** `takt promote` refuses a revision that is no ancestor of `origin/main` as last fetched, and a scorecard holding a run made by another binary.
- `takt promote` no longer refuses a second promotion on the same UTC day; promotions still run one at a time under the store's lock.
- Known limit: `story.delivery/1` does not require a failing test before the implementation (story:story-delivery-tests-first).
- Known limit: `tests.run` passes on exit 0 alone, so code under test that exits 0 before the runner reports earns a pass (story:test-pass-needs-the-runners-report).
- Known limit: a relative or in-tree `B10X_GATES_POLICY` resolves inside the case's tree, where a step can rewrite it (story:scan-policy-outside-the-tree).
- Known limit: a deleted `promotions.head` is read on trust again, and a process running as the same user can rewrite a record and the head together.
- Known limit: a predicate's command still sees a host socket outside `/run`, `/tmp` and the hidden homes, and one that needs `~/.cargo` or `~/.rustup` fails.

### Security

- **Breaking:** `pr.open` scans every commit, path and blob the push would send and the pull request's title and body; over 32 MiB is refused, never cut.
- **Breaking:** Pushes go over HTTPS only (`GIT_ALLOW_PROTOCOL=https`), push the scanned commit by id, and are refused when the remote's base moved.
- A step cannot replace what the test command's script loads: its modules, `sitecustomize.py`, `usercustomize.py`, `*.pth` files and the tree's `PATH` executables.
- **Breaking:** `tests.run` and `gate.run` run with a fresh empty `HOME` outside the tree, `PYTHONNOUSERSITE=1` and `PYTHONDONTWRITEBYTECODE=1`.
- A step that changes a writable directory on their `PATH`, or one Python imports from outside the tree, stops the run naming the path.
- **Breaking:** `tests.run` starts without the variables that locate a login, in its own user and mount namespace on Linux; without user namespaces it does not run (0.1.0's known limit).
- A harness step that changes a file in an ignored directory that existed before it is refused and undone, a rewritten file moved to `$XDG_STATE_HOME/takt/set-aside/` (0.1.0's known limit).
- A step or a test command that replaces `.engineering/cases` with a link or a non-directory is undone, and a case is never opened or resumed through a link (0.1.0's known limit).
- **Breaking:** `scripts/check-finding` builds the revision under check in bubblewrap with a cleared environment and a scratch home; without `bwrap` nothing is built (0.1.0's known limit).
- A predicate's command runs in bubblewrap: host paths read-only but the clone and its home, no network, its own PID and IPC namespaces, `/tmp` and `/run`.

## [0.1.0] - 2026-10-19

### Added

- `takt run --repo <path>` opens or resumes a case and takes one validated step at a time until a stop, bounded by `--iterations`, `--hours` and `--tokens`.
- `--hours` also ends a harness step, the test command or a gate command that runs past it.
- `--harness claude` (default) or `--harness none`, which runs only validators.
- A run takes no model step when only a validator or a grant can advance the case.
- Without `--intent` or `--case`, a run resumes the open case of a story being worked, or takes the next ready story from the planning store.
- A story opens on the embedded `story.delivery/1`, an `--intent` on `software.change/1`; `--protocol` takes a built-in id or a `protocol/1` file.
- `--input name=value` sets case inputs, a resumed case keeps its own protocol and inputs, and `takt protocols` lists the built-ins.
- Exit codes: 0 completed or nothing ready, 1 refused before any step, 2 bad arguments, 3 a budget or no way on, 4 approval, 5 an error, 6 a forge wait.
- A run whose pull request still has checks running waits for them inside its budget, else stops `Waiting`; the next run resumes the case.
- `takt scenario run --repo <path> <id>` replays a scenario of `.engineering/scenarios.conf` on the next free `trial/<n>` cut from its seed commit.
- `story.delivery/1` makes `tests.pass`, `gate.green`, `pr.opened`, `ci.green` and `pr.merged` claims; `accepted` holds only once the pull request merged.
- `gate.green` comes only from the repository's `engineering-gates/1` document (`--gates`, else `.engineering/gates.yaml`) at the implementation revision.
- With `--forge bot` (the default for an `origin` on github.com) a run pushes the case's branch and opens one pull request through the bot route.
- `pr.opened` and `ci.green` come only from what the forge reports; `pr.open` scans the body under the policy in `B10X_GATES_POLICY`.
- A run merges only under a recorded grant of `repository.merge`, else stops `ApprovalRequired`; `takt approve --repo <path> <case> <capability> --by <who>` records it.
- A grant authorises only the revision of the artifact its action is about, and a case is held by one run at a time.
- After the merge, the base's planning store holds the story `implemented` with its `test_result` and approval records.
- A case refuses to open on a dirty tree; a step commits only the paths it changed; a refused step is undone, index included.
- Evidence binds only to a revision whose content it observed; gate and project definitions are read from the base revision.
- Child processes inherit the environment without credential variables, with a timeout and bounded output.
- A step that edits a validator's inputs (the test script, `.cargo/**`, `build.rs`, `rust-toolchain*`, the CI workflow) earns no `tests.pass` or `ci.green` from it.
- Each case keeps `events.jsonl`, one line per evaluation, step, evidence record and stop; a step's line carries what the harness reported.
- A line of `events.jsonl` that is not valid UTF-8 is skipped and named; the rest still reads.
- `takt --version` prints the revision the binary was built from, and every run records it.
- `takt scorecard --repo <path>` prints per run outcome, steps, tokens, refusals, `NoProgress`, errors and wall time from `events.jsonl`.
- `takt scorecard --save <dir> --revision <sha>` saves it; `--against <file>` compares per scenario (5 trials, margin of 1 case) and exits 3 when worse.
- `takt promote --repo <path> --scorecard <file>` (Linux, bubblewrap) installs a candidate when no scenario is worse than the baseline, one a UTC day.
- `takt promote --dry-run` writes nothing; `--baseline-only` records the first baseline.
- `takt trace --case <dir>` writes a run as one `takt-trace/1` JSON Lines file; token and cost figures come from the harness's raw usage.
- `takt report --case <dir>` or `--trace <file>` prints wall time per actor, steps, model calls, tokens, cost, defects and what the sources did not record.
- `docs/metrics/` keeps scenario runs, improvement turns and waves as JSON Lines series, drawn by `timeline.py` and `journey.py`.
- `scripts/file-finding` files a finding once per key with an `expected_before` predicate; `scripts/check-finding <id>` prints `reproduced` or `not_reproduced`.
- `ess/` declares takt's nouns and relations, and `task check` fails on a stale `read from:` citation there.
- <https://beyond10x.github.io/takt/> describes takt, its results with n and caveats, and its roadmap, and draws every protocol from its file.
- A version tag publishes `takt` for Linux (x86_64, aarch64) and macOS (aarch64) as `.tar.gz` with a `SHA256SUMS` file.

### Changed

- Known limit: `story.delivery/1` does not require a failing test before the implementation (story:story-delivery-tests-first).
- Known limit: a relative or in-tree `B10X_GATES_POLICY` resolves inside the case's tree, where a step can rewrite it (story:scan-policy-outside-the-tree).
- Known limit: the scorecard takes a run's outcome from the run's own record, not from a held-out acceptance check (story:scorecard-outcome-from-held-out-check).
- Known limit: the test command can reach the operator's forge login through the keyring and session bus (story:test-command-reaches-no-forge-login).
- Known limit: a step's writes inside a git-ignored directory that existed before it are not detected (story:ignored-directories-are-guarded).
- Known limit: a step that replaces `.engineering/cases` or a case directory with a symbolic link is not undone (story:cases-root-link-is-undone).
- Known limit: under load, a process a test command started with `setsid` can write into the tree after it returns (story:run-child-sweep-survives-setsid).
- Known limit: `scripts/check-finding` builds the revision under check with the operator's HOME and environment (story:check-finding-builds-confined).
- Known limit: `scripts/check-finding` can print `not_reproduced` when the predicate's command crashed or never reached the scenario (story:check-finding-positive-control).

[Unreleased]: https://github.com/beyond10x/takt/compare/0.14.0...HEAD
[0.14.0]: https://github.com/beyond10x/takt/compare/0.13.1...0.14.0
[0.13.1]: https://github.com/beyond10x/takt/compare/0.13.0...0.13.1
[0.13.0]: https://github.com/beyond10x/takt/compare/0.12.0...0.13.0
[0.12.0]: https://github.com/beyond10x/takt/compare/0.11.1...0.12.0
[0.11.1]: https://github.com/beyond10x/takt/compare/0.11.0...0.11.1
[0.11.0]: https://github.com/beyond10x/takt/compare/0.10.1...0.11.0
[0.10.1]: https://github.com/beyond10x/takt/compare/0.10.0...0.10.1
[0.10.0]: https://github.com/beyond10x/takt/compare/0.9.0...0.10.0
[0.9.0]: https://github.com/beyond10x/takt/compare/0.8.0...0.9.0
[0.8.0]: https://github.com/beyond10x/takt/compare/0.7.0...0.8.0
[0.7.0]: https://github.com/beyond10x/takt/compare/0.6.0...0.7.0
[0.6.0]: https://github.com/beyond10x/takt/compare/0.5.0...0.6.0
[0.5.0]: https://github.com/beyond10x/takt/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/beyond10x/takt/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/beyond10x/takt/compare/0.2.3...0.3.0
[0.2.3]: https://github.com/beyond10x/takt/compare/0.2.2...0.2.3
[0.2.2]: https://github.com/beyond10x/takt/compare/0.2.1...0.2.2
[0.2.1]: https://github.com/beyond10x/takt/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/beyond10x/takt/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/beyond10x/takt/releases/tag/0.1.0
