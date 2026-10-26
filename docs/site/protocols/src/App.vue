<script setup>
import { computed, onMounted, ref, watch, nextTick } from "vue";
import mermaid from "mermaid";
import protocols from "./protocols.json";
import { describe } from "./predicate.js";
import { KINDS, flowchart, relations } from "./graph.js";
import { choice, dark, follow, next } from "./theme.js";

// The site's navbar, read from its config at build time (vite.config.js).
const site = __SITE__;
const left = site.nav.filter((i) => i.position === "left");
const right = site.nav.filter((i) => i.position === "right");
const mode = ref(choice());
const MODE_NAME = { system: "system mode", light: "light mode", dark: "dark mode" };
// The site's icons (Docusaurus theme-classic): an external link, and the three colour modes.
const EXTERNAL = "M21 13v10h-21v-19h12v2h-10v15h17v-8h2zm3-12h-10.988l4.035 4-6.977 7.07 2.828 2.828 6.977-7.07 4.125 4.172v-11z";
const MODES = [
  { name: "light", d: "M12,9c1.65,0,3,1.35,3,3s-1.35,3-3,3s-3-1.35-3-3S10.35,9,12,9 M12,7c-2.76,0-5,2.24-5,5s2.24,5,5,5s5-2.24,5-5 S14.76,7,12,7L12,7z M2,13l2,0c0.55,0,1-0.45,1-1s-0.45-1-1-1l-2,0c-0.55,0-1,0.45-1,1S1.45,13,2,13z M20,13l2,0c0.55,0,1-0.45,1-1 s-0.45-1-1-1l-2,0c-0.55,0-1,0.45-1,1S19.45,13,20,13z M11,2v2c0,0.55,0.45,1,1,1s1-0.45,1-1V2c0-0.55-0.45-1-1-1S11,1.45,11,2z M11,20v2c0,0.55,0.45,1,1,1s1-0.45,1-1v-2c0-0.55-0.45-1-1-1C11.45,19,11,19.45,11,20z M5.99,4.58c-0.39-0.39-1.03-0.39-1.41,0 c-0.39,0.39-0.39,1.03,0,1.41l1.06,1.06c0.39,0.39,1.03,0.39,1.41,0s0.39-1.03,0-1.41L5.99,4.58z M18.36,16.95 c-0.39-0.39-1.03-0.39-1.41,0c-0.39,0.39-0.39,1.03,0,1.41l1.06,1.06c0.39,0.39,1.03,0.39,1.41,0c0.39-0.39,0.39-1.03,0-1.41 L18.36,16.95z M19.42,5.99c0.39-0.39,0.39-1.03,0-1.41c-0.39-0.39-1.03-0.39-1.41,0l-1.06,1.06c-0.39,0.39-0.39,1.03,0,1.41 s1.03,0.39,1.41,0L19.42,5.99z M7.05,18.36c0.39-0.39,0.39-1.03,0-1.41c-0.39-0.39-1.03-0.39-1.41,0l-1.06,1.06 c-0.39,0.39-0.39,1.03,0,1.41s1.03,0.39,1.41,0L7.05,18.36z" },
  { name: "dark", d: "M9.37,5.51C9.19,6.15,9.1,6.82,9.1,7.5c0,4.08,3.32,7.4,7.4,7.4c0.68,0,1.35-0.09,1.99-0.27C17.45,17.19,14.93,19,12,19 c-3.86,0-7-3.14-7-7C5,9.07,6.81,6.55,9.37,5.51z M12,3c-4.97,0-9,4.03-9,9s4.03,9,9,9s9-4.03,9-9c0-0.46-0.04-0.92-0.1-1.36 c-0.98,1.37-2.58,2.26-4.4,2.26c-2.98,0-5.4-2.42-5.4-5.4c0-1.81,0.89-3.42,2.26-4.4C12.92,3.04,12.46,3,12,3L12,3z" },
  { name: "system", d: "m12 21c4.971 0 9-4.029 9-9s-4.029-9-9-9-9 4.029-9 9 4.029 9 9 9zm4.95-13.95c1.313 1.313 2.05 3.093 2.05 4.95s-0.738 3.637-2.05 4.95c-1.313 1.313-3.093 2.05-4.95 2.05v-14c1.857 0 3.637 0.737 4.95 2.05z" },
];

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
// On a narrow screen the graph would be too small to read: the list stands in for it, and the
// graph is one tap away.
const narrow = window.matchMedia("(max-width: 40rem)").matches;
const showGraph = ref(!narrow);
const zoom = ref(1);
let natural = null;

// The graph is drawn in the page's colour mode, and drawn again when it changes.
function setUp() {
  mermaid.initialize({
    startOnLoad: false,
    theme: dark() ? "dark" : "neutral",
    securityLevel: "strict",
    fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif',
    themeVariables: { fontSize: "15px" },
    flowchart: { useMaxWidth: false, padding: 10, nodeSpacing: 36, rankSpacing: 48 },
  });
}

// Each node kind's colours, from the stylesheet's custom properties for the current mode.
function palette() {
  const style = getComputedStyle(document.documentElement);
  const read = (name) => {
    const value = style.getPropertyValue(name).trim();
    if (!value) throw new Error(`style.css defines no ${name}`);
    return value;
  };
  return Object.fromEntries(KINDS.map((k) => [k, { fill: read(`--${k}-fill`), line: read(`--${k}-line`), ink: read(`--${k}-ink`) }]));
}

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
  setUp();
  const { svg } = await mermaid.render(`g${current.value}${Date.now()}`, flowchart(doc.value, palette()));
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
  follow();
  let drawnDark = dark();
  new MutationObserver(() => {
    mode.value = choice();
    if (dark() === drawnDark) return;
    drawnDark = dark();
    if (showGraph.value) draw();
  }).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "data-theme-choice"] });
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
  <nav class="navbar" aria-label="Main" :style="{ '--mark': JSON.stringify(site.mark) }">
    <div class="navbar__inner">
      <div class="navbar__items">
        <a class="navbar__brand" :href="site.baseUrl"><b class="navbar__title">{{ site.title }}</b></a>
        <a v-for="i in left" :key="i.label" :href="i.href" :class="['navbar__item', 'navbar__link', { 'navbar__link--active': i.current }]"
          :aria-current="i.current ? 'page' : undefined" v-bind="i.external ? { target: '_blank', rel: 'noopener noreferrer' } : {}"
          >{{ i.label }}<svg v-if="i.external" width="13.5" height="13.5" viewBox="0 0 24 24" aria-label="(opens in new tab)"><path fill="currentColor" :d="EXTERNAL" /></svg></a>
        <details class="navbar__menu">
          <summary aria-label="Toggle navigation bar"><svg viewBox="0 0 30 30" aria-hidden="true"><path stroke="currentColor" stroke-linecap="round" stroke-miterlimit="10" stroke-width="2" d="M4 7h22M4 15h22M4 23h22" /></svg></summary>
          <div class="navbar__sheet">
            <a v-for="i in site.nav" :key="i.label" :href="i.href" :aria-current="i.current ? 'page' : undefined"
              v-bind="i.external ? { target: '_blank', rel: 'noopener noreferrer' } : {}"
              >{{ i.label }}<svg v-if="i.external" width="13.5" height="13.5" viewBox="0 0 24 24" aria-label="(opens in new tab)"><path fill="currentColor" :d="EXTERNAL" /></svg></a>
            <div class="toggle"><button class="clean-btn" type="button" @click="next" :title="MODE_NAME[mode]" :aria-label="`Switch between dark and light mode (currently ${MODE_NAME[mode]})`">
              <svg v-for="m in MODES" :key="m.name" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" :class="m.name"><path fill="currentColor" :d="m.d" /></svg>
            </button></div>
          </div>
        </details>
      </div>
      <div class="navbar__items navbar__items--right">
        <a v-for="i in right" :key="i.label" :href="i.href" class="navbar__item navbar__link" :aria-current="i.current ? 'page' : undefined"
          v-bind="i.external ? { target: '_blank', rel: 'noopener noreferrer' } : {}"
          >{{ i.label }}<svg v-if="i.external" width="13.5" height="13.5" viewBox="0 0 24 24" aria-label="(opens in new tab)"><path fill="currentColor" :d="EXTERNAL" /></svg></a>
        <div class="toggle"><button class="clean-btn" type="button" @click="next" :title="MODE_NAME[mode]" :aria-label="`Switch between dark and light mode (currently ${MODE_NAME[mode]})`">
          <svg v-for="m in MODES" :key="m.name" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" :class="m.name"><path fill="currentColor" :d="m.d" /></svg>
        </button></div>
      </div>
    </div>
  </nav>
  <main>
    <h1>Protocols</h1>
    <p class="lede">
      Every protocol takt runs under, read from the engineering protocols' <code>protocol/1</code> documents takt pins, at build time.
      The documents are the source of truth; this page shows what they say.
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

    <footer><a :href="site.baseUrl">takt</a> · <a href="https://github.com/beyond10x/takt">github.com/beyond10x/takt</a></footer>
  </main>
</template>
