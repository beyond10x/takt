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
L, R, T, PH = 100, 56, 112, 340
PW = W - L - R
MIN_SLOT = 64
FS = 12
ROW = 18
GREY = "#8a9a95"
BASE_MODEL = "claude-opus-5-5"


def text_w(s, size=FS, bold=False):
    """An estimate of a label's width in Inter: wide enough that two estimated boxes that keep
    apart do not touch when drawn."""
    wide = sum(0.66 if c.isupper() or c in "mwMW%" else 0.3 if c in " .,:;il1()'|" else 0.58 for c in s)
    return wide * size * (1.06 if bold else 1.0)


def wrap(text, width, size=FS):
    lines, line = [], ""
    for word in text.split(" "):
        cand = f"{line} {word}" if line else word
        if line and text_w(cand, size) > width:
            lines.append(line)
            line = word
        else:
            line = cand
    return lines + [line] if line else lines


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

    # Layout. The chart is drawn at its own size (960 px wide while the revisions fit) and every
    # text is 12 px or more, so a page shows it at 1:1 and scrolls it sideways rather than shrinking
    # it. Every label is placed where its estimated box meets no other label's box.
    global W, H, T, PW
    W = max(960, L + R + MIN_SLOT * len(slots))
    PW = W - L - R
    slot_w = PW / len(slots)

    def x_of(i):
        return L + slot_w * (i + 0.5)

    # header rows above the plot: one label per wave, and one per improvement turn
    heads = []
    for w in waves:
        if at(w) in slots:
            heads.append(dict(x=x_of(slots.index(at(w))) - slot_w / 2, kind="wave", wave=w["wave"],
                              label=w.get("label") or ", ".join(w["stories"][:2])))
    turn_x = {}
    for t in turns:
        before, after = t["before"]["takt_revision"], t["after"]["takt_revision"]
        if before not in slots:
            continue
        x = x_of(slots.index(after)) - slot_w / 2 if after in slots else x_of(slots.index(before)) + slot_w * 0.4
        turn_x[t["id"]] = x
        heads.append(dict(x=x, kind="turn", label=t["id"]))
    heads.sort(key=lambda h: h["x"])
    placed_heads = []
    for h in heads:
        text = f'wave {h["wave"]}: {h["label"]}' if h["kind"] == "wave" else h["label"]
        wide = text_w(text, FS, bold=h["kind"] == "turn") + (text_w(f'wave {h["wave"]}: ', FS, True) -
                                                             text_w(f'wave {h["wave"]}: ', FS) if h["kind"] == "wave" else 0)
        x = h["x"]
        tx, anchor = (x - 5, "end") if x + 5 + wide > W - 4 else (x + 5, "start")
        span = (tx - wide, tx) if anchor == "end" else (tx, tx + wide)
        def fits(row, strict):
            # the row is free where the label goes; no line from a higher row runs through the
            # label; and, strictly, the label's own line runs through no label below it
            ok = all(not (span[0] < b + 10 and span[1] > a - 10) for (a, b), r, _ in placed_heads if r == row)
            ok = ok and all(not (span[0] - 2 <= lx <= span[1] + 2) for _, r, lx in placed_heads if r > row)
            return ok and (not strict or all(not (a - 2 <= x <= b + 2) for (a, b), r, _ in placed_heads
                                             if r < row))
        rows_now = max([r for _, r, _ in placed_heads] + [0]) + 2
        row = next((r for r in range(rows_now) if fits(r, True)), None)
        if row is None:
            row = next(r for r in range(rows_now + 1) if fits(r, False))
        placed_heads.append((span, row, x))
        h.update(tx=tx, anchor=anchor, row=row)
    n_rows = max([h["row"] for h in heads] + [0]) + 1
    T = 44 + ROW * n_rows + 12

    tok_y = lambda v: T + PH - PH * v / tok_max  # noqa: E731
    wall_y = lambda v: T + PH - PH * v / wall_max  # noqa: E731

    o = []
    boxes = []

    def free(box):
        return all(box[2] <= b[0] or box[0] >= b[2] or box[3] <= b[1] or box[1] >= b[3] for b in boxes)

    def label_box(x, y, text, anchor="start", size=FS, bold=False):
        w_ = text_w(text, size, bold)
        x0 = x - w_ if anchor == "end" else x - w_ / 2 if anchor == "middle" else x
        return (x0 - 1, y - size + 1, x0 + w_ + 1, y + 4)

    css = (".takt-timeline{color:#26332f}"
           ".takt-timeline .acc{fill:#17684a;stroke:#17684a}"
           ".takt-timeline .acc-s{fill:none;stroke:#17684a}"
           ".takt-timeline .acc-t{fill:#17684a}"
           ".takt-timeline .mut{fill:#4f625d}"
           ".takt-timeline .warn{fill:#a5410f}")
    dark = (".takt-timeline{color:#d5dfdb}"
            ".takt-timeline .acc{fill:#b6e5ce;stroke:#b6e5ce}"
            ".takt-timeline .acc-s{stroke:#b6e5ce}"
            ".takt-timeline .acc-t{fill:#b6e5ce}"
            ".takt-timeline .mut{fill:#a8b8b3}"
            ".takt-timeline .warn{fill:#f0a06a}")
    o.append(f"<style>{css}@media (prefers-color-scheme: dark){{{dark}}}@media print{{{css}}}</style>")
    o.append(f'<text x="{L}" y="24" font-size="15" font-weight="600" fill="currentColor">'
             f'Scenario {SCENARIO}: takt runs to the merge approval, by takt revision</text>')

    for h in heads:
        top = T - 12 - ROW * h["row"]
        if h["kind"] == "wave":
            o.append(f'<line x1="{h["x"]:.1f}" x2="{h["x"]:.1f}" y1="{top - 10}" y2="{T + PH}" class="acc-s" '
                     f'stroke-width="1.5" stroke-dasharray="3 3"/>')
            o.append(f'<text x="{h["tx"]:.1f}" y="{top}" text-anchor="{h["anchor"]}" font-weight="600" '
                     f'class="acc-t">wave {esc(h["wave"])}: <tspan font-weight="400">{esc(h["label"])}</tspan></text>')
        else:
            o.append(f'<line x1="{h["x"]:.1f}" x2="{h["x"]:.1f}" y1="{top - 10}" y2="{T + PH}" class="acc-s" '
                     f'stroke-width="2.5"/>')
            o.append(f'<circle cx="{h["x"]:.1f}" cy="{top - 10}" r="4" class="acc"/>')
            o.append(f'<text x="{h["tx"]:.1f}" y="{top}" text-anchor="{h["anchor"]}" font-weight="700" '
                     f'class="acc-t">{esc(h["label"])}</text>')

    for k in range(0, int(tok_max // tok_step) + 1):
        v = k * tok_step
        y = tok_y(v)
        o.append(f'<line x1="{L}" x2="{L + PW}" y1="{y:.1f}" y2="{y:.1f}" stroke="{GREY}" '
                 f'stroke-opacity="0.25"/>')
        o.append(f'<text x="{L - 8}" y="{y + 4:.1f}" text-anchor="end" fill="currentColor">'
                 f'{thousands(int(v // 1000)) + "k" if v else "0"}</text>')
    for k in range(0, int(wall_max // wall_step) + 1):
        v = k * wall_step
        y = wall_y(v)
        o.append(f'<text x="{L + PW + 8}" y="{y + 4:.1f}" class="mut">{int(v)} s</text>')
    o.append(f'<text transform="translate(18 {T + PH / 2:.0f}) rotate(-90)" text-anchor="middle" '
             f'fill="currentColor">tokens per run (input + output + cache read + cache write)</text>')
    o.append(f'<text transform="translate({W - 12} {T + PH / 2:.0f}) rotate(90)" text-anchor="middle" '
             f'class="mut">wall time per run</text>')
    o.append(f'<line x1="{L}" x2="{L + PW}" y1="{T + PH}" y2="{T + PH}" stroke="currentColor" '
             f'stroke-opacity="0.6"/>')

    # the runs: a dot per run's tokens, a square per run's wall time, the median as a bar
    for i, s in enumerate(slots):
        rs = by_slot.get(s, [])
        cx = x_of(i)
        n = len(rs)
        sp = min(16.0, (slot_w - 14) / max(n - 1, 1))
        toks = [r["tokens"]["sum"] for r in rs if r["tokens"]["sum"] is not None]
        for k, r in enumerate(rs):
            x = cx + (k - (n - 1) / 2) * sp
            tok, wall = r["tokens"]["sum"], r["wall_seconds"]
            tip = (f'{r["id"]}: tokens {thousands(tok) if tok is not None else "not stated"}, '
                   f'wall {str(wall) + " s" if wall is not None else "not timed"}, '
                   f'resumes {r["resumes"]}, stop {r["stop"]}')
            if tok is not None:
                y = tok_y(tok)
                if s in candidate_of:
                    o.append(f'<path d="M{x:.1f},{y - 6:.1f}L{x + 6:.1f},{y:.1f}L{x:.1f},{y + 6:.1f}'
                             f'L{x - 6:.1f},{y:.1f}Z" class="acc" fill-opacity="0.25" stroke-width="1.8">'
                             f'<title>{esc(tip)}</title></path>')
                else:
                    o.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="5" class="acc"><title>{esc(tip)}'
                             f'</title></circle>')
                boxes.append((x - 6, y - 6, x + 6, y + 6))
            if wall is not None:
                y = wall_y(wall)
                if tok is None:
                    o.append(f'<path d="M{x - 4:.1f},{y - 4:.1f}L{x + 4:.1f},{y + 4:.1f}M{x - 4:.1f},'
                             f'{y + 4:.1f}L{x + 4:.1f},{y - 4:.1f}" stroke="{GREY}" stroke-width="1.8">'
                             f'<title>{esc(tip)}</title></path>')
                else:
                    o.append(f'<rect x="{x - 4:.1f}" y="{y - 4:.1f}" width="8" height="8" fill="none" '
                             f'stroke="{GREY}" stroke-width="1.6"><title>{esc(tip)}</title></rect>')
                boxes.append((x - 5, y - 5, x + 5, y + 5))
            if tok is None and wall is None:
                o.append(f'<circle cx="{x:.1f}" cy="{T + PH:.1f}" r="4" fill="none" stroke="{GREY}" '
                         f'stroke-width="1.5"><title>{esc(tip)}</title></circle>')
        if toks:
            y = tok_y(statistics.median(toks))
            half = max(14, (n - 1) * sp / 2 + 7)
            o.append(f'<line x1="{cx - half:.1f}" x2="{cx + half:.1f}" y1="{y:.1f}" y2="{y:.1f}" '
                     f'class="acc-s" stroke-width="2" stroke-opacity="0.7"/>')
        # resumes: only a run that needed one is labelled (rN); an unlabelled dot is r0
        for k, r in enumerate(rs):
            if r["tokens"]["sum"] is None or not r["resumes"]:
                continue
            x = cx + (k - (n - 1) / 2) * sp
            y = tok_y(r["tokens"]["sum"])
            text = f'r{r["resumes"]}'
            for ly in (y - 10, y + 20, y - 24, y + 34):
                b = label_box(x, ly, text, "middle")
                if free(b):
                    break
            boxes.append(b)
            o.append(f'<text x="{x:.1f}" y="{ly:.1f}" text-anchor="middle" fill="currentColor">{text}</text>')

    # the free-running medians: a dashed line each, its label where it meets no other label
    for name, v, n in sorted(refs, key=lambda ref: -ref[1]):
        y = tok_y(v)
        o.append(f'<line x1="{L}" x2="{L + PW}" y1="{y:.1f}" y2="{y:.1f}" stroke="currentColor" '
                 f'stroke-opacity="0.55" stroke-dasharray="6 4"/>')
        text = f'{name}, median {thousands(int(v))} (n={n})'
        tries = [(L + 6, y - 5, "start"), (L + PW - 6, y - 5, "end"), (L + 6, y + 15, "start"),
                 (L + PW - 6, y + 15, "end"), (L + 6, y - 19, "start"), (L + PW - 6, y + 29, "end")]
        for tx, ty, anchor in tries:
            b = label_box(tx, ty, text, anchor)
            if free(b):
                break
        boxes.append(b)
        o.append(f'<text x="{tx}" y="{ty:.1f}" text-anchor="{anchor}" fill="currentColor">{esc(text)}</text>')

    # under the axis: the revision, then one row per count, each row named in the left margin
    y = T + PH + 20
    for i, s in enumerate(slots):
        lines = slot_label.get(s, s).split(" ", 1) if s == slots[0] and " " in slot_label.get(s, s) else \
            [slot_label.get(s, s)]
        for k, line in enumerate(lines):
            o.append(f'<text x="{x_of(i):.1f}" y="{y + 15 * k}" text-anchor="middle" font-weight="600" '
                     f'fill="currentColor">{esc(line)}</text>')
    y += 15 * max(len(slot_label.get(slots[0], "").split(" ", 1)), 1) + 6
    rows = [("runs", lambda rs: str(len(rs)), "mut"),
            ("timed", lambda rs: str(sum(1 for r in rs if r["wall_seconds"] is not None)), "mut"),
            ("with tokens", lambda rs: str(sum(1 for r in rs if r["tokens"]["sum"] is not None)), "mut")]
    if any(s in candidate_of for s in slots):
        rows.append(("candidate of", None, "acc-t"))
    if any(m != BASE_MODEL for s in slots for r in by_slot.get(s, []) for m in (r["model"] or "").split(",") if m):
        rows.append(("other model", None, "warn"))
    for name, fn, cls in rows:
        o.append(f'<text x="{L - 8}" y="{y}" text-anchor="end" class="{cls}">{name}</text>')
        tall = 1
        for i, s in enumerate(slots):
            rs = by_slot.get(s, [])
            if fn:
                vals = [fn(rs)]
            elif name == "candidate of":
                vals = [candidate_of[s]["id"]] if s in candidate_of else []
            else:
                used = sorted({m for r in rs for m in (r["model"] or "").split(",") if m})
                vals = [m.replace("claude-", "") for m in used] if any(m != BASE_MODEL for m in used) else []
            tall = max(tall, len(vals))
            for k, v in enumerate(vals):
                o.append(f'<text x="{x_of(i):.1f}" y="{y + 15 * k}" text-anchor="middle" class="{cls}">'
                         f'{esc(v)}</text>')
        y += 15 * tall + 2

    # legend, flowing left to right and wrapping at the chart's width
    y += 16
    items = [("dot", "tokens, one run (rN: it needed N resumes; unlabelled: none; bar: median)"),
             ("square", "wall time, one run (right axis)"),
             ("cross", "refused before the first step, no tokens"),
             ("ring", "no figure stated")]
    if candidate_of:
        items.append(("diamond", "tokens, one run of a candidate revision of an improvement turn"))
    x = L
    for mark, text in items:
        wide = 18 + text_w(text, FS) + 22
        if x + wide > W - 8 and x > L:
            x, y = L, y + 20
        cx_, cy_ = x + 6, y - 4
        if mark == "dot":
            o.append(f'<circle cx="{cx_}" cy="{cy_}" r="5" class="acc"/>')
        elif mark == "square":
            o.append(f'<rect x="{cx_ - 4}" y="{cy_ - 4}" width="8" height="8" fill="none" stroke="{GREY}" '
                     f'stroke-width="1.6"/>')
        elif mark == "cross":
            o.append(f'<path d="M{cx_ - 4},{cy_ - 4}L{cx_ + 4},{cy_ + 4}M{cx_ - 4},{cy_ + 4}L{cx_ + 4},{cy_ - 4}" '
                     f'stroke="{GREY}" stroke-width="1.8"/>')
        elif mark == "ring":
            o.append(f'<circle cx="{cx_}" cy="{cy_}" r="4" fill="none" stroke="{GREY}" stroke-width="1.5"/>')
        else:
            o.append(f'<path d="M{cx_},{cy_ - 6}L{cx_ + 6},{cy_}L{cx_},{cy_ + 6}L{cx_ - 6},{cy_}Z" class="acc" '
                     f'fill-opacity="0.25" stroke-width="1.8"/>')
        o.append(f'<text x="{x + 18}" y="{y}" fill="currentColor">{esc(text)}</text>')
        x += wide

    # the improvement turns, spelled out under the legend instead of over the plot
    for t in turns:
        if t["id"] not in turn_x:
            continue
        parts = [f'{t["id"]} (solid line): {t["finding"].replace("story:", "")}',
                 f'{t["before"]["result"]} at {t["before"]["takt_revision"]}',
                 f'{t["after"]["result"]} at {t["after"]["takt_revision"]}']
        sc = t.get("scorecard")
        if t.get("scorecard_result") and sc:
            b, c = sc["baseline"]["tokens_median"], sc["candidate"]["tokens_median"]
            parts.append(f'scorecard: {t["scorecard_result"]}, median tokens {(c - b) / b * 100:+.0f}% '
                         f'(n={len(sc["candidate"]["runs"])})')
        parts.append(f'merged at {t["merged"]["sha"]}')
        parts.append(f'promoted at {t["promoted_at"]}' if t.get("promoted_at") else 'not yet promoted')
        if t.get("outcome"):
            parts.append(f'outcome: {t["outcome"]}')
        y += 22
        for k, line in enumerate(wrap("; ".join(parts), W - L - 8, FS)):
            o.append(f'<text x="{L}" y="{y}" fill="currentColor">'
                     + (f'<tspan font-weight="700" class="acc-t">{esc(t["id"])}</tspan>{esc(line[len(t["id"]):])}'
                        if k == 0 else esc(line)) + '</text>')
            y += 16
        y -= 16

    dates = sorted({r["date"][:10] for r in runs})
    span = dates[0] if len(dates) == 1 else f"{dates[0]} to {dates[-1]}"
    note = (f'Data: {span} (UTC), docs/metrics/scenario-runs.jsonl, {len(runs)} runs. One scenario, n per '
            f'revision in the rows under the axis: first results, not a benchmark. Untimed revisions have no '
            f'wall time; tokens are summed over the invocations that reached the merge approval, resumes '
            f'included.')
    y += 26
    for line in wrap(note, W - L - 8, FS):
        o.append(f'<text x="{L}" y="{y}" class="mut">{esc(line)}</text>')
        y += 16
    H = int(y + 6)

    head = (f'<svg class="takt-timeline" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" '
            f'width="{W}" height="{H}" role="img" aria-labelledby="takt-timeline-title" '
            f'font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="{FS}">\n'
            '<title id="takt-timeline-title">takt on scenario 1: tokens and wall time per takt revision, '
            'with waves, the improvement turn and the free-running medians</title>')
    return "\n".join([head] + o + ["</svg>"]) + "\n"


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
