# Protocol viewer

A small Vue 3 app that shows every `protocol/1` document under `protocols/`: its actions,
evidence kinds, claims, obligations and outcomes as tables, and as one graph of how they connect.

The protocol files are the source of truth. `scripts/collect.mjs` reads them at build time into
`src/protocols.json` (generated, not committed), so a rebuild after a protocol change shows the
change, and nothing in this directory restates a protocol.

```
npm ci && npm run dev                  # local, with hot reload
npm run build                          # dist/
../publish-protocols.sh                # rebuild and publish at /takt/protocols/ (gh-pages only)
```

The graph: an action produces evidence; evidence makes a claim true; claims gate actions (dotted)
and outcomes; a capability (hexagon) needs a grant before its action runs; obligations wait on
claims. The graph is drawn at its natural size with pan (drag) and zoom; the same relations are
listed under it, open by default on a narrow screen where the graph is one tap away. The page uses
the home page's header, palette, type and column. New views go in `src/` as components that read the same `protocols.json`.
