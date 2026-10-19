<script setup>
import { computed, onMounted, ref, watch, nextTick } from "vue";
import mermaid from "mermaid";
import protocols from "./protocols.json";
import { describe } from "./predicate.js";
import { flowchart, relations } from "./graph.js";

const fromHash = () => {
  const want = decodeURIComponent(location.hash.slice(1));
  const i = protocols.findIndex((p) => key(p) === want);
  return i < 0 ? 0 : i;
};
const key = (p) => `${p.doc.protocol.id}/${p.doc.protocol.revision}`;
const current = ref(fromHash());
const p = computed(() => protocols[current.value]);
const doc = computed(() => p.value.doc);
const graph = ref(null);
const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
// On a narrow screen the graph would be too small to read: the list stands in for it, and the
// graph is one tap away.
const narrow = window.matchMedia("(max-width: 40rem)").matches;
const showGraph = ref(!narrow);
const zoom = ref(1);
let natural = null;

mermaid.initialize({
  startOnLoad: false,
  theme: dark ? "dark" : "neutral",
  securityLevel: "strict",
  fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif',
  themeVariables: { fontSize: "15px" },
  flowchart: { useMaxWidth: false, padding: 10, nodeSpacing: 36, rankSpacing: 48 },
});

function applyZoom() {
  const svg = graph.value?.querySelector("svg");
  if (!svg || !natural) return;
  svg.style.width = `${natural.w * zoom.value}px`;
  svg.style.height = `${natural.h * zoom.value}px`;
  svg.style.maxWidth = "none";
}

async function draw() {
  await nextTick();
  if (!graph.value) return;
  const { svg } = await mermaid.render(`g${current.value}${Date.now()}`, flowchart(doc.value));
  graph.value.innerHTML = svg;
  const el = graph.value.querySelector("svg");
  const vb = el.viewBox.baseVal;
  natural = { w: vb.width, h: vb.height };
  zoom.value = 1;
  applyZoom();
}

function setZoom(z) {
  const box = graph.value;
  if (!box) return;
  // keep the point at the centre of the view where it is
  const cx = (box.scrollLeft + box.clientWidth / 2) / zoom.value;
  const cy = (box.scrollTop + box.clientHeight / 2) / zoom.value;
  zoom.value = Math.min(3, Math.max(0.3, z));
  applyZoom();
  box.scrollLeft = cx * zoom.value - box.clientWidth / 2;
  box.scrollTop = cy * zoom.value - box.clientHeight / 2;
}
const fit = () => natural && setZoom((graph.value.clientWidth - 24) / natural.w);

// drag to pan
let drag = null;
function down(e) {
  if (e.button !== 0) return;
  drag = { x: e.clientX, y: e.clientY, l: graph.value.scrollLeft, t: graph.value.scrollTop };
  graph.value.setPointerCapture(e.pointerId);
}
function move(e) {
  if (!drag) return;
  graph.value.scrollLeft = drag.l - (e.clientX - drag.x);
  graph.value.scrollTop = drag.t - (e.clientY - drag.y);
}
const up = () => (drag = null);

function pick(i) {
  current.value = i;
  location.hash = encodeURIComponent(key(protocols[i]));
}

onMounted(() => {
  window.addEventListener("hashchange", () => (current.value = fromHash()));
  if (showGraph.value) draw();
});
watch(current, () => showGraph.value && draw());
watch(showGraph, (on) => on && draw());

const entries = (o) => Object.entries(o || {});
const requires = (a) => (a.requires || []).map((r) => r.capability || r.decision || JSON.stringify(r)).join(", ");
const produces = (a) => (a.may_produce || []).map((x) => x.evidence).join(", ");
const VERBS = ["produces", "makes true", "gates", "needs a grant before", "discharges", "is required by"];
const HEADS = {
  produces: "Actions produce evidence",
  "makes true": "Evidence and claims make claims true",
  gates: "Claims gate actions",
  "needs a grant before": "Capabilities need a grant before an action",
  discharges: "Claims discharge obligations",
  "is required by": "Outcomes require",
};
const groups = computed(() => {
  const rel = relations(doc.value);
  return VERBS.map((v) => ({ verb: v, head: HEADS[v], rows: rel.filter((r) => r[1] === v) })).filter((g) => g.rows.length);
});
</script>

<template>
  <header class="site">
    <div><a class="brand" href="../">takt</a><a href="../docs">Docs</a><a href="../docs/self-improvement/runs">Measurements</a><a href="./" aria-current="page">Protocols</a><a href="https://github.com/beyond10x/takt">GitHub</a></div>
  </header>
  <main>
    <h1>Protocols</h1>
    <p class="lede">
      Every protocol takt runs under, read from the <code>protocol/1</code> files in the repository at build time.
      The files are the source of truth; this page shows what they say.
    </p>
    <nav class="tabs" aria-label="Protocols">
      <button v-for="(q, i) in protocols" :key="q.source" :class="{ on: i === current }" :aria-pressed="i === current" @click="pick(i)">
        <strong>{{ q.doc.protocol.id }}/{{ q.doc.protocol.revision }}</strong>
        <span>{{ q.source }}</span>
      </button>
    </nav>

    <h2>{{ doc.protocol.id }}/{{ doc.protocol.revision }}</h2>
    <p class="source">source: <code>{{ p.source }}</code></p>
    <p>{{ doc.protocol.description }}</p>

    <section>
      <h3>Graph</h3>
      <p class="legend">
        <span class="k action">action</span> produces <span class="k evidence">evidence</span>, which makes a
        <span class="k claim">claim</span> true; claims gate actions (dotted) and <span class="k outcome">outcomes</span>;
        a <span class="k capability">capability</span> needs a grant; <span class="k obligation">obligations</span> wait on claims.
      </p>
      <template v-if="showGraph">
        <div class="tools">
          <button @click="setZoom(zoom / 1.25)" aria-label="Zoom out">&minus;</button>
          <button @click="setZoom(1)">100%</button>
          <button @click="setZoom(zoom * 1.25)" aria-label="Zoom in">+</button>
          <button @click="fit">Fit</button>
          <span>Drag to pan; the graph scrolls both ways.</span>
        </div>
        <div ref="graph" class="graph" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="up"></div>
      </template>
      <button v-else class="show" @click="showGraph = true">Show the graph (wide: pan and zoom)</button>
      <details class="relations" :open="narrow">
        <summary>The graph as a list</summary>
        <div v-for="g in groups" :key="g.verb">
          <h4>{{ g.head }}</h4>
          <ul>
            <li v-for="(r, i) in g.rows" :key="i">
              <code>{{ r[0][1] }}</code> <span class="kind">{{ r[0][0] }}</span> {{ r[1] }}
              <code>{{ r[2][1] }}</code> <span class="kind">{{ r[2][0] }}</span>
            </li>
          </ul>
        </div>
      </details>
    </section>

    <section v-if="doc.case?.inputs">
      <h3>Inputs</h3>
      <table class="stack"><tr><th>input</th><th>values</th><th>meaning</th></tr>
        <tr v-for="[n, i] in entries(doc.case.inputs)" :key="n"><td><code>{{ n }}</code></td><td data-label="values">{{ (i.values || []).join(", ") }}</td><td data-label="meaning">{{ i.description }}</td></tr>
      </table>
    </section>

    <section>
      <h3>Artifacts</h3>
      <table class="stack"><tr><th>artifact</th><th>meaning</th></tr>
        <tr v-for="[n, a] in entries(doc.artifacts)" :key="n"><td><code>{{ n }}</code></td><td data-label="meaning">{{ a.description }}</td></tr>
      </table>
    </section>

    <section>
      <h3>Actions</h3>
      <table class="stack"><tr><th>action</th><th>effect</th><th>precondition</th><th>needs a grant of</th><th>may produce</th></tr>
        <tr v-for="[n, a] in entries(doc.actions)" :key="n">
          <td><code>{{ n }}</code><div class="sub">{{ a.description }}</div></td>
          <td data-label="effect">{{ a.effect }}</td><td data-label="precondition">{{ a.precondition ? describe(a.precondition) : "" }}</td>
          <td data-label="needs a grant of">{{ requires(a) }}</td><td data-label="may produce">{{ produces(a) }}</td>
        </tr>
      </table>
    </section>

    <section>
      <h3>Claims</h3>
      <table class="stack"><tr><th>claim</th><th>true when</th></tr>
        <tr v-for="[n, c] in entries(doc.claims)" :key="n"><td><code>{{ n }}</code><div class="sub">{{ c.description }}</div></td><td data-label="true when">{{ describe(c.true_when) }}</td></tr>
      </table>
    </section>

    <section>
      <h3>Evidence kinds</h3>
      <table class="stack"><tr><th>kind</th><th>meaning</th></tr>
        <tr v-for="[n, e] in entries(doc.evidence_kinds)" :key="n"><td><code>{{ n }}</code></td><td data-label="meaning">{{ e.description }}</td></tr>
      </table>
    </section>

    <section v-if="doc.obligations">
      <h3>Obligations</h3>
      <table class="stack"><tr><th>obligation</th><th>discharged when</th></tr>
        <tr v-for="[n, o] in entries(doc.obligations)" :key="n"><td><code>{{ n }}</code><div class="sub">{{ o.description }}</div></td><td data-label="discharged when">{{ describe(o.discharged_when) }}</td></tr>
      </table>
    </section>

    <section>
      <h3>Outcomes</h3>
      <table class="stack"><tr><th>outcome</th><th>legitimate when</th></tr>
        <tr v-for="[n, o] in entries(doc.outcomes)" :key="n"><td><code>{{ n }}</code><div class="sub">{{ o.description }}</div></td><td data-label="legitimate when">{{ describe(o.requires) }}</td></tr>
      </table>
    </section>

    <footer><a href="../">takt</a> · <a href="https://github.com/beyond10x/takt">github.com/beyond10x/takt</a></footer>
  </main>
</template>
