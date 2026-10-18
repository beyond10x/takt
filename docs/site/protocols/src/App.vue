<script setup>
import { computed, onMounted, ref, watch, nextTick } from "vue";
import mermaid from "mermaid";
import protocols from "./protocols.json";
import { describe } from "./predicate.js";
import { flowchart } from "./graph.js";

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

mermaid.initialize({ startOnLoad: false, theme: dark ? "dark" : "neutral", securityLevel: "strict" });

async function draw() {
  await nextTick();
  if (!graph.value) return;
  const { svg } = await mermaid.render(`g${current.value}${Date.now()}`, flowchart(doc.value));
  graph.value.innerHTML = svg;
}

function pick(i) {
  current.value = i;
  location.hash = encodeURIComponent(key(protocols[i]));
}

onMounted(() => {
  window.addEventListener("hashchange", () => (current.value = fromHash()));
  draw();
});
watch(current, draw);

const entries = (o) => Object.entries(o || {});
const requires = (a) => (a.requires || []).map((r) => r.capability || r.decision || JSON.stringify(r)).join(", ");
const produces = (a) => (a.may_produce || []).map((x) => x.evidence).join(", ");
</script>

<template>
  <header>
    <h1>takt protocols</h1>
    <p class="lede">
      Every protocol takt runs under, read from the <code>protocol/1</code> files in the repository at build time.
      The files are the source of truth; this page shows what they say.
    </p>
  </header>
  <div class="layout">
    <nav>
      <button v-for="(q, i) in protocols" :key="q.source" :class="{ on: i === current }" @click="pick(i)">
        <strong>{{ q.doc.protocol.id }}/{{ q.doc.protocol.revision }}</strong>
        <span>{{ q.source }}</span>
      </button>
    </nav>
    <main>
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
        <div ref="graph" class="graph"></div>
      </section>

      <section v-if="doc.case?.inputs">
        <h3>Inputs</h3>
        <table><tr><th>input</th><th>values</th><th>meaning</th></tr>
          <tr v-for="[n, i] in entries(doc.case.inputs)" :key="n"><td><code>{{ n }}</code></td><td>{{ (i.values || []).join(", ") }}</td><td>{{ i.description }}</td></tr>
        </table>
      </section>

      <section>
        <h3>Artifacts</h3>
        <table><tr><th>artifact</th><th>meaning</th></tr>
          <tr v-for="[n, a] in entries(doc.artifacts)" :key="n"><td><code>{{ n }}</code></td><td>{{ a.description }}</td></tr>
        </table>
      </section>

      <section>
        <h3>Actions</h3>
        <table><tr><th>action</th><th>effect</th><th>precondition</th><th>needs a grant of</th><th>may produce</th></tr>
          <tr v-for="[n, a] in entries(doc.actions)" :key="n">
            <td><code>{{ n }}</code><div class="sub">{{ a.description }}</div></td>
            <td>{{ a.effect }}</td><td>{{ a.precondition ? describe(a.precondition) : "" }}</td>
            <td>{{ requires(a) }}</td><td>{{ produces(a) }}</td>
          </tr>
        </table>
      </section>

      <section>
        <h3>Claims</h3>
        <table><tr><th>claim</th><th>true when</th></tr>
          <tr v-for="[n, c] in entries(doc.claims)" :key="n"><td><code>{{ n }}</code><div class="sub">{{ c.description }}</div></td><td>{{ describe(c.true_when) }}</td></tr>
        </table>
      </section>

      <section>
        <h3>Evidence kinds</h3>
        <table><tr><th>kind</th><th>meaning</th></tr>
          <tr v-for="[n, e] in entries(doc.evidence_kinds)" :key="n"><td><code>{{ n }}</code></td><td>{{ e.description }}</td></tr>
        </table>
      </section>

      <section v-if="doc.obligations">
        <h3>Obligations</h3>
        <table><tr><th>obligation</th><th>discharged when</th></tr>
          <tr v-for="[n, o] in entries(doc.obligations)" :key="n"><td><code>{{ n }}</code><div class="sub">{{ o.description }}</div></td><td>{{ describe(o.discharged_when) }}</td></tr>
        </table>
      </section>

      <section>
        <h3>Outcomes</h3>
        <table><tr><th>outcome</th><th>legitimate when</th></tr>
          <tr v-for="[n, o] in entries(doc.outcomes)" :key="n"><td><code>{{ n }}</code><div class="sub">{{ o.description }}</div></td><td>{{ describe(o.requires) }}</td></tr>
        </table>
      </section>
    </main>
  </div>
  <footer><a href="../">takt</a> · <a href="https://github.com/beyond10x/takt">github.com/beyond10x/takt</a></footer>
</template>
