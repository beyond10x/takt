// Builds a Mermaid flowchart of one protocol: actions produce evidence, evidence makes claims
// true, claims gate actions and outcomes, capabilities gate actions, obligations wait on claims.
import { reads } from "./predicate.js";

const id = (kind, name) => `${kind}_${String(name).replace(/[^A-Za-z0-9_]/g, "_")}`;
const label = (text) => `"${String(text).replace(/"/g, "'")}"`;

export function flowchart(doc) {
  const lines = ["flowchart LR"];
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
    for (const p of a.may_produce || []) lines.push(`  ${id("action", name)} --> ${id("evidence", p.evidence)}`);
    const r = reads(a.precondition);
    for (const c of r.claims) lines.push(`  ${id("claim", c)} -.-> ${id("action", name)}`);
    for (const req of a.requires || []) {
      if (req.capability) {
        if (!capabilities.has(req.capability)) {
          node("capability", req.capability, ["{{", "}}"]);
          capabilities.add(req.capability);
        }
        lines.push(`  ${id("capability", req.capability)} ==> ${id("action", name)}`);
      }
    }
  }
  for (const [name, c] of Object.entries(claims)) {
    for (const e of reads(c.true_when).evidence) lines.push(`  ${id("evidence", e)} --> ${id("claim", name)}`);
    for (const d of reads(c.true_when).claims) lines.push(`  ${id("claim", d)} --> ${id("claim", name)}`);
  }
  for (const [name, o] of Object.entries(obligations)) {
    node("obligation", name, ["[[", "]]"]);
    for (const c of reads(o.discharged_when).claims) lines.push(`  ${id("claim", c)} --> ${id("obligation", name)}`);
  }
  for (const [name, o] of Object.entries(outcomes)) {
    node("outcome", name, ["([", "])"]);
    const r = reads(o.requires);
    for (const c of r.claims) lines.push(`  ${id("claim", c)} --> ${id("outcome", name)}`);
    for (const e of r.evidence) lines.push(`  ${id("evidence", e)} --> ${id("outcome", name)}`);
    for (const d of r.decisions) {
      node("decision", d, ["[(", ")]"]);
      lines.push(`  ${id("decision", d)} --> ${id("outcome", name)}`);
    }
  }
  lines.push(
    "  classDef action fill:#e8f2ec,stroke:#17684a,color:#0f1b1c",
    "  classDef evidence fill:#eef1f6,stroke:#5b6b86,color:#0f1b1c",
    "  classDef claim fill:#ffffff,stroke:#17684a,color:#0f1b1c",
    "  classDef capability fill:#fff4e0,stroke:#b07a12,color:#0f1b1c",
    "  classDef outcome fill:#17684a,stroke:#17684a,color:#ffffff",
    "  classDef obligation fill:#f6eef6,stroke:#86568a,color:#0f1b1c",
    "  classDef decision fill:#f2f2f2,stroke:#666,color:#0f1b1c",
  );
  return lines.join("\n");
}
