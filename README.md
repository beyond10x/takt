# Takt

Secure, autonomous, reliable, deterministic and efficient agentic coding, in one open-source
binary. Point `takt` at a repository and say "work on this": it takes an intent or pulls the next
unit from the planning store, opens a case on a protocol, lets a harness such as Claude Code do one
admissible step at a time, and accepts nothing a validator has not turned into evidence. Canon
decides what is permitted, what is owed and whether completion was earned; the model never does.

Agents choose how to do the work. The protocol defines what must be true before the work counts
and which actions wait for a person's grant (`takt approve`); a merge through the bot route always
does.

## Status

| milestone | state |
|---|---|
| M1 one step validated | done 2026-10-18 (`docs/runs/2026-10-18-m1-scratch.md`) |
| M2 a story end to end | built: the planning-store frontier, the story protocol, the gate validator, the pull request and CI through the bot route, the merge under a recorded grant; not yet proven: no recorded run has taken a story from ready to merged with the grant as the only manual step |
| M3 to M5 | workflows as `workflow/1` files, a second harness and the confinement seam, a week of daily runs: not built |
| continuous runs | a run that plans its next stories and works a wave with parallel workers: designed, not built |

The design is `docs/design/takt.md`; its § 4 lists what each milestone has.

## Use

```
takt run --repo <path> --intent "make the failing test pass" --iterations 1
takt run --repo <path> --hours 1
takt approve --repo <path> <case> repository.merge --by <who>
takt scenario run --repo <path> <id> --hours 1
takt protocols
```

The first form opens a case on `software.change/1`. The second resumes the open case of a story
being worked, or else takes the next ready story from the repository's planning store and opens
its case on `story.delivery/1`, which is embedded in the binary: tests, gate, pull request, CI and
merge as claims about the implementation revision, `accepted` once the pull request is merged. A
case opens only on a clean tree. `--protocol` takes a built-in id or a `protocol/1` file, read
from the directory you run `takt` in. Bounds: `--iterations`, `--hours`, `--tokens`. The tests are
`--test-cmd` (default `cargo test --quiet`); `.engineering/gates.yaml`, when the repository has
one, adds the gate. `takt run --help` lists every default and the exit codes:

| exit code | the run |
|---|---|
| 0 | completed, or nothing was ready |
| 1 | refused before any step; the reason is on stderr |
| 2 | the arguments do not parse |
| 3 | stopped on a budget, with nothing admissible, on a refused step, or with no progress |
| 4 | waits for an approval (`takt approve`); the next run resumes the case |
| 5 | stopped on an error once its case was open; the case stays open |

`takt scenario run` replays a declared scenario: `.engineering/scenarios.conf` on `main` names
its story and seed commit. The run fetches the remote's `trial/*` branches, cuts the next free
`trial/<n>` from the seed, pushes it (it must not exist yet), and runs the story on it, so its pull
request targets `trial/<n>`. A trial branch is deleted afterwards with `b10x-gates bot
--repository <owner>/<repo> -- delete-branch trial/<n>`, which the bot route admits only while the
branch's tip is merged into the default branch.

## Build

```
cargo install --locked --path crates/takt-cli
takt --version
```

## Requirements

- a harness: Claude Code (`claude` on the `PATH`), or `--harness none` to run validators only
- for story runs, `aep` on the `PATH` and its planning store in the repository
  (`.engineering/project.yaml`) with ready stories
- for pull requests, an `origin` on github.com and `b10x-gates` and `gh` on the `PATH`; another
  host gets no forge, and `--forge none` runs without one; the forge's gates policy comes from
  `B10X_GATES_POLICY`

Built on [Canon](https://github.com/beyond10x/canon) and
[Engineering protocols](https://github.com/beyond10x/engineering-protocols). Licence: Apache-2.0.
