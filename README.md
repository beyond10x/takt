# Takt

Composable agentic work. Point `takt` at a repository and say "work on this": it takes an intent or
pulls the next unit from the planning store, opens a case on an engineering protocol, lets a
harness such as Claude Code do one admissible step at a time, and accepts nothing a validator has
not turned into evidence. Canon decides what is permitted, what is owed and whether completion was
earned; the model never does.

Status: milestone M1, one step validated, done on 2026-10-18 (`docs/runs/2026-10-18-m1-scratch.md`). The design is `docs/design/takt.md`.

```
takt run --repo <path> --intent "make the failing test pass" --iterations 1
```

## Build

```
cargo install --locked --path crates/takt-cli
takt --version
```

Built on [Canon](https://github.com/beyond10x/canon) and
[Engineering protocols](https://github.com/beyond10x/engineering-protocols). Beside their
built-ins, `protocols/story-delivery/1.yaml` is this repository's own `protocol/1` for delivering
a planning-store story (`--protocol protocols/story-delivery/1.yaml`, a path read from the directory you run `takt` in, so give it in full from elsewhere): tests, gate, pull request,
CI and merge as claims about the implementation revision, `accepted` once the pull request is
merged. Licence: Apache-2.0.
