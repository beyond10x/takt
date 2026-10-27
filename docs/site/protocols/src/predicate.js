// Turns a protocol/1 predicate (claim, evidence, first, before, decision, input, all, any, not)
// into text, and lists the claims, evidence kinds and decisions it reads, for the tables and the graph.

export function describe(p) {
  if (p == null) return "always";
  if (typeof p !== "object") return String(p);
  if (p.claim) return `claim ${p.claim}`;
  if (p.decision) return `decision ${p.decision}`;
  if (p.input) {
    if (p.equals !== undefined) return `input ${p.input} = ${p.equals}`;
    if (Array.isArray(p.in)) return `input ${p.input} in ${p.in.join(", ")}`;
    return `input ${p.input}`;
  }
  if (p.evidence) return evidence("evidence", p.evidence);
  if (p.first) return evidence("first evidence", p.first);
  if (p.before) {
    const b = p.before;
    return `first ${b.first} before first ${b.then}${b.subject ? ` on ${b.subject}` : ""}`;
  }
  if (p.all) return p.all.map(describe).join(" and ");
  if (p.any) return "(" + p.any.map(describe).join(" or ") + ")";
  if (p.not) return `not (${describe(p.not)})`;
  return JSON.stringify(p);
}

function evidence(label, e) {
  const parts = [`${label} ${e.kind}`];
  if (e.result !== undefined) parts.push(`= ${e.result}`);
  if (e.subject) parts.push(`on ${e.subject}`);
  return parts.join(" ");
}

export function reads(p, out = { claims: new Set(), evidence: new Set(), decisions: new Set(), inputs: new Set() }) {
  if (p == null || typeof p !== "object") return out;
  if (p.claim) out.claims.add(p.claim);
  if (p.decision) out.decisions.add(p.decision);
  if (p.input) out.inputs.add(p.input);
  if (p.evidence) out.evidence.add(p.evidence.kind);
  if (p.first) out.evidence.add(p.first.kind);
  if (p.before) [p.before.first, p.before.then].forEach((kind) => kind && out.evidence.add(kind));
  for (const key of ["all", "any"]) if (Array.isArray(p[key])) p[key].forEach((q) => reads(q, out));
  if (p.not) reads(p.not, out);
  return out;
}
