// Builds a Mermaid flowchart of one protocol: actions produce evidence, evidence makes claims
// true, claims gate actions and outcomes, capabilities gate actions, obligations wait on claims.
import { reads } from "./predicate.js";

const id = (kind, name) => `${kind}_${String(name).replace(/[^A-Za-z0-9_]/g, "_")}`;
const label = (text) => `"${String(text).replace(/"/g, "'")}"`;

export const KINDS = ["action", "evidence", "claim", "capability", "outcome", "obligation", "decision"];

// The same relations as a list, for a screen too narrow for the graph: [from, relation, to], each
// end a [kind, name] pair.
export function relations(doc) {
  return flowchart(doc, null, true);
}

// palette: per kind of KINDS, its { fill, line, ink } colours for the current colour mode.
export function flowchart(doc, palette, asList = false) {
  const lines = ["flowchart LR"];
  const list = [];
  const rel = (from, verb, to) => list.push([from, verb, to]);
  const node = (kind, name, shape) => {
    const n = id(kind, name);
    const [open, close] = shape;
    lines.push(`  ${n}${open}${label(name)}${close}`);
    lines.push(`  class ${n} ${kind}`);
    return n;
  };
  const actions = doc.actions || {};
  const claims = doc.claims || {};
  const outcomes = doc.outcomes || {};
  const obligations = doc.obligations || {};
  const capabilities = new Set();

  for (const name of Object.keys(doc.evidence_kinds || {})) node("evidence", name, ["[/", "/]"]);
  for (const name of Object.keys(claims)) node("claim", name, ["(", ")"]);
  for (const [name, a] of Object.entries(actions)) {
    node("action", name, ["[", "]"]);
    for (const p of a.may_produce || []) {
      lines.push(`  ${id("action", name)} --> ${id("evidence", p.evidence)}`);
      rel(["action", name], "produces", ["evidence", p.evidence]);
    }
    const r = reads(a.precondition);
    for (const c of r.claims) {
      lines.push(`  ${id("claim", c)} -.-> ${id("action", name)}`);
      rel(["claim", c], "gates", ["action", name]);
    }
    for (const req of a.requires || []) {
      if (req.capability) {
        if (!capabilities.has(req.capability)) {
          node("capability", req.capability, ["{{", "}}"]);
          capabilities.add(req.capability);
        }
        lines.push(`  ${id("capability", req.capability)} ==> ${id("action", name)}`);
        rel(["capability", req.capability], "needs a grant before", ["action", name]);
      }
    }
  }
  for (const [name, c] of Object.entries(claims)) {
    for (const e of reads(c.true_when).evidence) {
      lines.push(`  ${id("evidence", e)} --> ${id("claim", name)}`);
      rel(["evidence", e], "makes true", ["claim", name]);
    }
    for (const d of reads(c.true_when).claims) {
      lines.push(`  ${id("claim", d)} --> ${id("claim", name)}`);
      rel(["claim", d], "makes true", ["claim", name]);
    }
  }
  for (const [name, o] of Object.entries(obligations)) {
    node("obligation", name, ["[[", "]]"]);
    for (const c of reads(o.discharged_when).claims) {
      lines.push(`  ${id("claim", c)} --> ${id("obligation", name)}`);
      rel(["claim", c], "discharges", ["obligation", name]);
    }
  }
  for (const [name, o] of Object.entries(outcomes)) {
    node("outcome", name, ["([", "])"]);
    const r = reads(o.requires);
    for (const c of r.claims) {
      lines.push(`  ${id("claim", c)} --> ${id("outcome", name)}`);
      rel(["claim", c], "is required by", ["outcome", name]);
    }
    for (const e of r.evidence) {
      lines.push(`  ${id("evidence", e)} --> ${id("outcome", name)}`);
      rel(["evidence", e], "is required by", ["outcome", name]);
    }
    for (const d of r.decisions) {
      node("decision", d, ["[(", ")]"]);
      lines.push(`  ${id("decision", d)} --> ${id("outcome", name)}`);
      rel(["decision", d], "is required by", ["outcome", name]);
    }
  }
  if (asList) return list;
  for (const kind of KINDS) {
    const { fill, line, ink } = palette[kind];
    lines.push(`  classDef ${kind} fill:${fill},stroke:${line},color:${ink}`);
  }
  return lines.join("\n");
}
