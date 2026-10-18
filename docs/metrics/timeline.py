"""Draw docs/metrics/timeline.svg from the metrics series.

Run as `python3 -I docs/metrics/timeline.py` from the repository root, or from anywhere: paths are
resolved from this file. Standard library only. When docs/pitch/index.html holds the markers
<!-- timeline.svg begin --> and <!-- timeline.svg end -->, the SVG between them is replaced too, so
the deck stays one file and no number in it is copied by hand.
"""

import html
import json
import os
import statistics
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
RUNS = os.path.join(HERE, "scenario-runs.jsonl")
WAVES = os.path.join(HERE, "waves.jsonl")
TURNS = os.path.join(HERE, "improvement-turns.jsonl")
OUT = os.path.join(HERE, "timeline.svg")
DECK = os.path.join(HERE, os.pardir, "pitch", "index.html")
BEGIN, END = "<!-- timeline.svg begin -->", "<!-- timeline.svg end -->"

SCENARIO = "1"
W, H = 960, 620
L, R, T, B = 86, 70, 112, 168
PW, PH = W - L - R, H - T - B
GREY = "#8a9a95"
BASE_MODEL = "claude-opus-5-5"


def load(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def esc(s):
    return html.escape(str(s), quote=True)


def thousands(n):
    return f"{n:,}"


def nice_step(top, ticks=5):
    raw = top / ticks
    mag = 10 ** (len(str(int(raw))) - 1)
    for m in (1, 2, 2.5, 5, 10):
        if raw <= m * mag:
            return m * mag
    return 10 * mag


def build():
    runs = [r for r in load(RUNS) if r["scenario"] == SCENARIO]
    waves = load(WAVES)
    turns = load(TURNS)

    first = waves[0]["wave"] if waves else "?"
    def at(w):
        return w.get("measured_at") or w["merge_sha"]

    slots = ["pre-wave-" + first] + [at(w) for w in waves]
    slot_label = {slots[0]: f"before wave {first}"}
    for w in waves:
        slot_label[at(w)] = at(w)
    wave_slots = {at(w) for w in waves}

    def ancestor(a, b):
        # a revision measured before a wave merged goes before that wave's column, not after it
        p = subprocess.run(["git", "merge-base", "--is-ancestor", a, b], cwd=HERE,
                           capture_output=True)
        return p.returncode == 0

    candidate_of = {t["after"]["takt_revision"]: t for t in turns}
    takt = [r for r in runs if r["arm"] == "takt"]
    by_slot = {s: [] for s in slots}
    for r in sorted(takt, key=lambda r: r["date"]):
        s = r["takt_revision"] or slots[0]
        by_slot.setdefault(s, []).append(r)
        if s not in slots:
            before = candidate_of[s]["before"]["takt_revision"] if s in candidate_of else None
            if before in slots:
                slots.insert(slots.index(before) + 1, s)
            else:
                later = [i for i, sl in enumerate(slots) if sl in wave_slots and ancestor(s, sl)]
                if later:
                    slots.insert(later[0], s)
                else:
                    slots.append(s)
            slot_label[s] = s

    refs = []
    for arm, name in (("claude-code", "Claude Code alone, operator's full config"),
                      ("codex", "Codex alone, operator's full config"),
                      ("claude-code-isolated", "Claude Code isolated like takt's harness, with a shell")):
        # one line per model the arm ran on: a median across a model change would mix two things
        counted = [r for r in runs if r["arm"] == arm and r["tokens"]["sum"] is not None]
        models = sorted({r["model"] for r in counted}, key=lambda m: m or "")
        for model in models:
            vals = [r["tokens"]["sum"] for r in counted if r["model"] == model]
            label = name if len(models) == 1 or not model else f"{name}, {model}"
            refs.append((label, statistics.median(vals), len(vals)))

    tok_vals = [r["tokens"]["sum"] for r in takt if r["tokens"]["sum"] is not None]
    wall_vals = [r["wall_seconds"] for r in takt if r["wall_seconds"] is not None]
    tok_top = max(tok_vals + [v for _, v, _ in refs] + [1])
    tok_step = nice_step(tok_top * 1.08)
    tok_max = tok_step * (int(tok_top * 1.08 // tok_step) + 1)
    wall_top = max(wall_vals + [1])
    wall_step = nice_step(wall_top * 1.1)
    wall_max = wall_step * (int(wall_top * 1.1 // wall_step) + 1)

    slot_w = PW / len(slots)

    def x_of(i):
        return L + slot_w * (i + 0.5)

    def y_tok(v):
        return T + PH - PH * v / tok_max

    def y_wall(v):
        return T + PH - PH * v / wall_max

    o = []
    o.append(f'<svg class="takt-timeline" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" '
             f'role="img" aria-labelledby="takt-timeline-title" '
             f'font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="12">')
    o.append('<title id="takt-timeline-title">takt on scenario 1: tokens and wall time per takt revision, '
             'with waves, the improvement turn and the free-running medians</title>')
    o.append("<style>"
             ".takt-timeline{color:#26332f}"
             ".takt-timeline .acc{fill:#17684a;stroke:#17684a}"
             ".takt-timeline .acc-s{fill:none;stroke:#17684a}"
             ".takt-timeline .acc-t{fill:#17684a}"
             "@media (prefers-color-scheme: dark){"
             ".takt-timeline{color:#d5dfdb}"
             ".takt-timeline .acc{fill:#b6e5ce;stroke:#b6e5ce}"
             ".takt-timeline .acc-s{stroke:#b6e5ce}"
             ".takt-timeline .acc-t{fill:#b6e5ce}}"
             "@media print{.takt-timeline{color:#26332f}"
             ".takt-timeline .acc{fill:#17684a;stroke:#17684a}"
             ".takt-timeline .acc-s{stroke:#17684a}.takt-timeline .acc-t{fill:#17684a}}"
             "</style>")

    o.append(f'<text x="{L}" y="24" font-size="15" font-weight="600" fill="currentColor">'
             f'Scenario {SCENARIO}: takt runs to the merge approval, by takt revision</text>')

    for k in range(0, int(tok_max // tok_step) + 1):
        v = k * tok_step
        y = y_tok(v)
        o.append(f'<line x1="{L}" x2="{L + PW}" y1="{y:.1f}" y2="{y:.1f}" stroke="{GREY}" '
                 f'stroke-opacity="0.25"/>')
        o.append(f'<text x="{L - 8}" y="{y + 4:.1f}" text-anchor="end" fill="currentColor">'
                 f'{thousands(int(v // 1000)) + "k" if v else "0"}</text>')
    for k in range(0, int(wall_max // wall_step) + 1):
        v = k * wall_step
        y = y_wall(v)
        o.append(f'<text x="{L + PW + 8}" y="{y + 4:.1f}" fill="{GREY}">{int(v)} s</text>')
    o.append(f'<text transform="translate(20 {T + PH / 2:.0f}) rotate(-90)" text-anchor="middle" '
             f'fill="currentColor">tokens per run (input + output + cache read + cache write)</text>')
    o.append(f'<text transform="translate({W - 14} {T + PH / 2:.0f}) rotate(90)" text-anchor="middle" '
             f'fill="{GREY}">wall time per run</text>')
    o.append(f'<line x1="{L}" x2="{L + PW}" y1="{T + PH}" y2="{T + PH}" stroke="currentColor" '
             f'stroke-opacity="0.6"/>')

    last_left = None
    for name, v, n in sorted(refs, key=lambda ref: -ref[1]):
        y = y_tok(v)
        o.append(f'<line x1="{L}" x2="{L + PW}" y1="{y:.1f}" y2="{y:.1f}" stroke="currentColor" '
                 f'stroke-opacity="0.55" stroke-dasharray="6 4"/>')
        # a label within a line's height of the last left-hand label goes to the right-hand end
        right = last_left is not None and abs(y - last_left) < 14
        x, anchor = (L + PW - 6, "end") if right else (L + 6, "start")
        if not right:
            last_left = y
        o.append(f'<text x="{x}" y="{y - 5:.1f}" text-anchor="{anchor}" font-size="11" fill="currentColor">'
                 f'{esc(name)}, median {thousands(int(v))} (n={n})</text>')

    shown = [w for w in waves if at(w) in slots]
    rows = {T - 46: [], T - 20: [], T - 72: []}
    for w in shown:
        x = x_of(slots.index(at(w))) - slot_w / 2
        label = w.get("label") or ", ".join(w["stories"][:2])
        # about 6.2 px per character at 11 px; a label that would leave the chart is set left of its
        # line, and each label takes the first row where it overlaps no other
        wide = (len(label) + len(w["wave"]) + 7) * 6.2
        tx, anchor = (x - 5, "end") if x + 5 + wide > W - 4 else (x + 5, "start")
        span = (tx - wide, tx) if anchor == "end" else (tx, tx + wide)
        top = next((y for y, taken in rows.items() if all(span[1] < a or span[0] > b for a, b in taken)),
                   T - 46)
        rows[top].append(span)
        o.append(f'<line x1="{x:.1f}" x2="{x:.1f}" y1="{top - 10}" y2="{T + PH}" class="acc-s" '
                 f'stroke-width="1.5" stroke-dasharray="3 3"/>')
        o.append(f'<text x="{tx:.1f}" y="{top}" text-anchor="{anchor}" font-size="11" font-weight="600" '
                 f'class="acc-t">wave {esc(w["wave"])}: <tspan font-weight="400">{esc(label)}</tspan></text>')

    for t in turns:
        before = t["before"]["takt_revision"]
        if before not in slots:
            continue
        after = t["after"]["takt_revision"]
        if after in slots:
            x = x_of(slots.index(after)) - slot_w / 2
        else:
            x = x_of(slots.index(before)) + slot_w * 0.4
        o.append(f'<line x1="{x:.1f}" x2="{x:.1f}" y1="{T + 8}" y2="{T + PH}" class="acc-s" '
                 f'stroke-width="2.5"/>')
        o.append(f'<circle cx="{x:.1f}" cy="{T + 8}" r="4" class="acc"/>')
        lines = [f'{t["id"]}: {t["finding"].replace("story:", "")}',
                 f'{t["before"]["result"]} at {before}',
                 f'{t["after"]["result"]} at {after}']
        sc = t.get("scorecard")
        if t.get("scorecard_result") and sc:
            b, c = sc["baseline"]["tokens_median"], sc["candidate"]["tokens_median"]
            lines.append(f'scorecard: {t["scorecard_result"]}, median tokens {(c - b) / b * 100:+.0f}% '
                         f'(n={len(sc["candidate"]["runs"])})')
        lines.append(f'merged at {t["merged"]["sha"]}; '
                     + (f'promoted at {t["promoted_at"]}' if t.get("promoted_at") else 'not yet promoted'))
        if t.get("outcome"):
            lines.append(f'outcome: {t["outcome"]}')
        for k, line in enumerate(lines):
            o.append(f'<text x="{x + 7:.1f}" y="{T + 26 + 13 * k}" font-size="10" '
                     f'fill="currentColor">{esc(line)}</text>')

    for i, s in enumerate(slots):
        rs = by_slot.get(s, [])
        cx = x_of(i)
        n = len(rs)
        toks = [r["tokens"]["sum"] for r in rs if r["tokens"]["sum"] is not None]
        walls = [r["wall_seconds"] for r in rs if r["wall_seconds"] is not None]
        for k, r in enumerate(rs):
            x = cx + (k - (n - 1) / 2) * 16
            tok, wall = r["tokens"]["sum"], r["wall_seconds"]
            tip = (f'{r["id"]}: tokens {thousands(tok) if tok is not None else "not stated"}, '
                   f'wall {str(wall) + " s" if wall is not None else "not timed"}, '
                   f'resumes {r["resumes"]}, stop {r["stop"]}')
            if tok is not None:
                y = y_tok(tok)
                if s in candidate_of:
                    o.append(f'<path d="M{x:.1f},{y - 6:.1f}L{x + 6:.1f},{y:.1f}L{x:.1f},{y + 6:.1f}'
                             f'L{x - 6:.1f},{y:.1f}Z" class="acc" fill-opacity="0.25" stroke-width="1.8">'
                             f'<title>{esc(tip)}</title></path>')
                else:
                    o.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="5" class="acc"><title>{esc(tip)}'
                             f'</title></circle>')
                o.append(f'<text x="{x:.1f}" y="{y - 9:.1f}" text-anchor="middle" font-size="9.5" '
                         f'fill="currentColor">r{r["resumes"]}</text>')
            if wall is not None:
                y = y_wall(wall)
                if tok is None:
                    o.append(f'<path d="M{x - 4:.1f},{y - 4:.1f}L{x + 4:.1f},{y + 4:.1f}M{x - 4:.1f},'
                             f'{y + 4:.1f}L{x + 4:.1f},{y - 4:.1f}" stroke="{GREY}" stroke-width="1.8">'
                             f'<title>{esc(tip)}</title></path>')
                else:
                    o.append(f'<rect x="{x - 4:.1f}" y="{y - 4:.1f}" width="8" height="8" fill="none" '
                             f'stroke="{GREY}" stroke-width="1.6"><title>{esc(tip)}</title></rect>')
            if tok is None and wall is None:
                o.append(f'<circle cx="{x:.1f}" cy="{T + PH:.1f}" r="4" fill="none" stroke="{GREY}" '
                         f'stroke-width="1.5"><title>{esc(tip)}</title></circle>')
        if toks:
            m = statistics.median(toks)
            y = y_tok(m)
            half = max(14, (n - 1) * 8 + 12)
            o.append(f'<line x1="{cx - half:.1f}" x2="{cx + half:.1f}" y1="{y:.1f}" y2="{y:.1f}" '
                     f'class="acc-s" stroke-width="2" stroke-opacity="0.7"/>')
        o.append(f'<text x="{cx:.1f}" y="{T + PH + 18}" text-anchor="middle" font-weight="600" '
                 f'fill="currentColor">{esc(slot_label.get(s, s))}</text>')
        o.append(f'<text x="{cx:.1f}" y="{T + PH + 33}" text-anchor="middle" font-size="10.5" '
                 f'fill="{GREY}">runs {n}, timed {len(walls)}</text>')
        o.append(f'<text x="{cx:.1f}" y="{T + PH + 46}" text-anchor="middle" font-size="10.5" '
                 f'fill="{GREY}">with tokens {len(toks)}</text>')
        if s in candidate_of:
            o.append(f'<text x="{cx:.1f}" y="{T + PH + 60}" text-anchor="middle" font-size="10.5" '
                     f'class="acc-t">{esc(candidate_of[s]["id"])} candidate</text>')
        used = sorted({m for r in rs for m in (r["model"] or "").split(",") if m})
        if any(m != BASE_MODEL for m in used):
            o.append(f'<text x="{cx:.1f}" y="{T + PH + 60}" text-anchor="middle" font-size="10.5" '
                     f'fill="#b8430f">model{"s" if len(used) > 1 else ""}: {esc(", ".join(m.replace("claude-", "") for m in used))}</text>')

    ly = H - 80
    o.append(f'<circle cx="{L + 6}" cy="{ly}" r="5" class="acc"/>')
    o.append(f'<text x="{L + 16}" y="{ly + 4}" font-size="11" fill="currentColor">tokens, one run '
             f'(rN = resumes; bar = median)</text>')
    o.append(f'<rect x="{L + 262}" y="{ly - 4}" width="8" height="8" fill="none" stroke="{GREY}" '
             f'stroke-width="1.6"/>')
    o.append(f'<text x="{L + 276}" y="{ly + 4}" font-size="11" fill="currentColor">wall time, one run '
             f'(right axis)</text>')
    o.append(f'<path d="M{L + 438},{ly - 4}L{L + 446},{ly + 4}M{L + 438},{ly + 4}L{L + 446},{ly - 4}" '
             f'stroke="{GREY}" stroke-width="1.8"/>')
    o.append(f'<text x="{L + 452}" y="{ly + 4}" font-size="11" fill="currentColor">refused before the '
             f'first step, no tokens</text>')
    o.append(f'<circle cx="{L + 668}" cy="{ly}" r="4" fill="none" stroke="{GREY}" stroke-width="1.5"/>')
    o.append(f'<text x="{L + 678}" y="{ly + 4}" font-size="11" fill="currentColor">no figure stated</text>')
    if candidate_of:
        y2 = ly + 18
        o.append(f'<path d="M{L + 6},{y2 - 6}L{L + 12},{y2}L{L + 6},{y2 + 6}L{L},{y2}Z" class="acc" '
                 f'fill-opacity="0.25" stroke-width="1.8"/>')
        o.append(f'<text x="{L + 16}" y="{y2 + 4}" font-size="11" fill="currentColor">tokens, one run '
                 f'of a candidate revision of an improvement turn, scored against the revision before the turn marker</text>')

    dates = sorted({r["date"][:10] for r in runs})
    span = dates[0] if len(dates) == 1 else f"{dates[0]} to {dates[-1]}"
    o.append(f'<text x="{L}" y="{H - 32}" font-size="11" fill="{GREY}">Data: {span} (UTC), '
             f'docs/metrics/scenario-runs.jsonl, {len(runs)} runs. One scenario, n per revision under '
             f'its label: first results, not a benchmark.</text>')
    o.append(f'<text x="{L}" y="{H - 15}" font-size="11" fill="{GREY}">Untimed revisions have no wall '
             f'time; tokens are summed over the invocations that reached the merge approval, resumes '
             f'included.</text>')
    o.append("</svg>")
    return "\n".join(o) + "\n"


def main():
    svg = build()
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(svg)
    print("wrote", os.path.relpath(OUT))
    if os.path.exists(DECK):
        with open(DECK, encoding="utf-8") as f:
            deck = f.read()
        a, b = deck.find(BEGIN), deck.find(END)
        if a != -1 and b > a:
            deck = deck[: a + len(BEGIN)] + "\n" + svg + deck[b:]
            with open(DECK, "w", encoding="utf-8") as f:
                f.write(deck)
            print("updated", os.path.relpath(DECK))


if __name__ == "__main__":
    main()
