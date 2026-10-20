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
listed under it, open by default on a narrow screen where the graph is one tap away. New views go
in `src/` as components that read the same `protocols.json`.

The page is one of the documentation site's (`website/`, which builds it into
`/takt/protocols/`). `vite.config.js` reads the site's `docusaurus.config.ts` at build time for
what the two share: the address, the navbar's title, mark and links, and the localStorage key
under which the site keeps the reader's colour mode choice. The page follows that choice, and a
change made on a docs page in another tab, as the site's pages do (`src/theme.js`); its toggle
writes the same key. The palette, type and navbar are the site's; the graph's node colours are
custom properties in `src/style.css`, one set per colour mode, shared with the legend.
