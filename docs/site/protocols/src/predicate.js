// Turns a protocol/1 predicate (claim, evidence, decision, input, all, any, not) into text, and
// lists the claims and decisions it reads, for the tables and the graph.

export function describe(p) {
  if (p == null) return "always";
  if (typeof p !== "object") return String(p);
  if (p.claim) return `claim ${p.claim}`;
  if (p.decision) return `decision ${p.decision}`;
  if (p.input) return `input ${p.input}${p.equals !== undefined ? ` = ${p.equals}` : ""}`;
  if (p.evidence) {
    const e = p.evidence;
    const parts = [`evidence ${e.kind}`];
    if (e.result !== undefined) parts.push(`= ${e.result}`);
    if (e.subject) parts.push(`on ${e.subject}`);
    return parts.join(" ");
  }
  if (p.all) return p.all.map(describe).join(" and ");
  if (p.any) return "(" + p.any.map(describe).join(" or ") + ")";
  if (p.not) return `not (${describe(p.not)})`;
  return JSON.stringify(p);
}

export function reads(p, out = { claims: new Set(), evidence: new Set(), decisions: new Set(), inputs: new Set() }) {
  if (p == null || typeof p !== "object") return out;
  if (p.claim) out.claims.add(p.claim);
  if (p.decision) out.decisions.add(p.decision);
  if (p.input) out.inputs.add(p.input);
  if (p.evidence) out.evidence.add(p.evidence.kind);
  for (const key of ["all", "any"]) if (Array.isArray(p[key])) p[key].forEach((q) => reads(q, out));
  if (p.not) reads(p.not, out);
  return out;
}
