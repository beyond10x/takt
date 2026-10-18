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
[Engineering protocols](https://github.com/beyond10x/engineering-protocols). Licence: Apache-2.0.
