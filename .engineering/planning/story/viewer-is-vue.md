---
format: aep.planning-md/3
id: story:viewer-is-vue
kind: story
status: draft
title: takt view's page is a Vue 3 application in TypeScript, built by Vite with pnpm
summary: viewer/app.js and the vendored Vue global build give way to a Vite build; the page stays one offline file
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: .github/workflows/check.yml
- confidence: inferred
  path: Taskfile.yml
- confidence: cited
  path: crates/takt-cli/tests/view_cli.rs
- confidence: cited
  path: crates/takt/src/view.rs
- confidence: inferred
  path: viewer
- confidence: cited
  path: viewer/app.js
- confidence: cited
  path: viewer/vendor/vue.global.prod.js
revision: 3
---
# Story: viewer-is-vue

## Outcome

The page `takt view` writes is built from a Vue 3 application in TypeScript, with single-file
components, built by Vite and managed by pnpm, as `uilab/widget/` is. The hand-written
`viewer/app.js` and the vendored `viewer/vendor/vue.global.prod.js` are gone. The page stays one
offline file: it fetches nothing and holds the runs as inline JSON, as today.

## Context

- The rule: code that runs in a browser is Vue 3 with TypeScript, built with Vite and pnpm; the
  server it talks to stays Rust, and types it shares with the server are generated from Rust and
  drift-checked.
- Today `crates/takt/src/view.rs:28-31` compiles four assets into the binary with `include_str!`:
  `viewer/index.html`, `viewer/style.css`, `viewer/app.js` (63 lines of plain JavaScript, Vue's
  global build used from `setup()`) and `viewer/vendor/vue.global.prod.js`. `takt view` puts
  them in place at the template's placeholders.
- The data the page reads (`takt-view-data`: runs, their ops, `trace_format`, `dir`) is written by
  `view.rs`; its TypeScript types are generated from the Rust types and drift-checked.
- `cargo build` must not need Node. Proposed, to be confirmed in scoping: the built page (one
  file, everything inlined) is committed under `viewer/dist/`, `view.rs` includes that file, and
  `task check` rebuilds it with pnpm and fails on drift, as the site's generated pages are
  checked.
- Not in this story: `docs/site/protocols/` is a Vue 3 application in JavaScript with npm
  (`docs/site/protocols/package.json`), so it also falls short of TypeScript and pnpm; a
  follow-up.

```expected_before
host: any checkout of this repository
command: git ls-files viewer/app.js viewer/vendor/vue.global.prod.js | grep -q . && echo viewer-hand-written-js
exit: 0
healthy_exit: 1
output: viewer-hand-written-js
source: .engineering/planning/approval-record/store-replan-2026-10-28.md
```

## Acceptance

1. The predicate above exits 1 and prints nothing at the story's merge.
2. `viewer/package.json` names `vue` 3, `typescript` and `vite`; `viewer/pnpm-lock.yaml` is
   committed; every component is a `.vue` file with `<script setup lang="ts">`; `vue-tsc
   --noEmit` passes in `task check`.
3. `crates/takt-cli/tests/view_cli.rs` passes unchanged, and a new case asserts that the written
   page holds no `http://` or `https://` URL in a `src` or `href` attribute and no `fetch(`.
4. `task check` fails when the committed build output differs from a fresh `pnpm build`, and
   when the generated TypeScript types differ from the Rust types they come from.

## Scope

- `viewer/`: the application, `package.json`, `pnpm-lock.yaml`, `vite.config.ts`, the built page (inferred)
- `viewer/app.js`, `viewer/vendor/vue.global.prod.js`: removed (cited)
- `crates/takt/src/view.rs`: the assets it includes (cited)
- `crates/takt-cli/tests/view_cli.rs` (cited)
- `Taskfile.yml`, `.github/workflows/check.yml`: the build and drift check (inferred)
