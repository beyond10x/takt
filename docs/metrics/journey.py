"""Overlay every recorded scenario run on one timeline, in one vocabulary.

Run as `python3 -I docs/metrics/journey.py`. Standard library only. For each record in
scenario-runs.jsonl it reads the raw files the record names (and the run's steps.jsonl and
events.jsonl beside them, where present), writes one line per operation to journeys.jsonl, and
draws journey.svg: one lane per run, grouped by arm and then by takt revision. When
docs/pitch/index.html holds the markers <!-- journey.svg begin --> and <!-- journey.svg end -->,
the SVG between them is replaced too.

No token or cost figure is taken from the journeys: those come only from scenario-runs.jsonl.
"""

import datetime
import glob
import html
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
RUNS = os.path.join(HERE, "scenario-runs.jsonl")
WAVES = os.path.join(HERE, "waves.jsonl")
TURNS = os.path.join(HERE, "improvement-turns.jsonl")
OUT_JSONL = os.path.join(HERE, "journeys.jsonl")
OUT_SVG = os.path.join(HERE, "journey.svg")
DECK = os.path.join(HERE, os.pardir, "pitch", "index.html")
BEGIN, END = "<!-- journey.svg begin -->", "<!-- journey.svg end -->"
HOME = os.path.expanduser("~")

KINDS = ["model_call", "read", "edit", "test", "gate", "push", "pr_open", "ci_wait", "ci_pass",
         "approval_wait", "refusal", "no_progress", "resume", "other"]
COLOURS = {
    "model_call": ("#17684a", "#6fcf9f"), "read": ("#8a9a95", "#8a9a95"),
    "edit": ("#2f6fb0", "#6aa7e8"), "test": ("#7b5cc4", "#a98ee6"), "gate": ("#a07a1a", "#d4ad4a"),
    "push": ("#e07b39", "#f0a06a"), "pr_open": ("#b8430f", "#e87a46"), "ci_wait": ("#d9a42b", "#e8c25e"),
    "ci_pass": ("#23877a", "#53c2b2"), "approval_wait": ("#c43b8f", "#e57ab9"),
    "refusal": ("#c0392b", "#ef6f61"), "no_progress": ("#7d4a3a", "#c08a77"),
    "resume": ("#26332f", "#d5dfdb"), "other": ("#b5c0bc", "#5d6f6a"),
}
ARM_ORDER = ["takt", "claude-code-isolated", "claude-code", "codex"]
ARM_NAME = {"takt": "takt", "claude-code-isolated": "Claude Code, isolated like takt's harness",
            "claude-code": "Claude Code, full configuration", "codex": "Codex, full configuration"}


def load(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def expand(p):
    return HOME + p[1:] if p.startswith("~/") else p


def tilde(p):
    return "~" + p[len(HOME):] if p.startswith(HOME + os.sep) else p


def iso(s):
    return datetime.datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc)


def esc(s):
    return html.escape(str(s), quote=True)


# --- takt: classify one step from its action, who took it and its effect text ---------------

def classify_step(action, by, effect):
    e = effect or ""
    if by == "harness":
        if "needs an approval" in e or action == "repository.merge":
            return "model_call", "approval"
        if action == "repository.inspect" or "no file changed" in e:
            return "read", "unchanged"
        if "committed" in e:
            return "edit", "changed"
        return "edit", "unchanged" if "no file" in e else "changed"
    if action == "tests.run":
        return "test", "pass" if "exited 0: pass" in e else "fail"
    if action == "gate.run":
        return "gate", "pass" if ": pass" in e else "fail"
    if action == "pr.open":
        if "did not pass" in e or "refused" in e:
            return "refusal", "refused"
        if "opened pull request" in e:
            return "pr_open", "opened"
        if e.startswith("pushed"):
            return "push", "pushed"
        return "pr_open", "nothing"
    if action == "ci.read":
        if "pending" in e:
            return "ci_wait", "pending"
        if ": pass" in e:
            return "ci_pass", "pass"
        return "other", "nothing"
    if action == "repository.merge":
        return "other", "merged" if "merged pull request" in e else "nothing"
    return "other", None


def stop_kind(stop):
    if stop.startswith("NoProgress"):
        return "no_progress", "NoProgress"
    if stop.startswith("ApprovalRequired"):
        return "approval_wait", "approval"
    if stop.startswith("Completed"):
        return "other", "completed"
    return "refusal", "refused"


STEP_RE = re.compile(r"^step \d+: (\S+) (\{.*\}|\((.+)\))\s*$")
STOP_RE = re.compile(r"^stopped: (.*?); steps \d+, tokens \d+, case ")
NOTE_EXIT_RE = re.compile(r"^(\S+Z) (takt scenario run|resume \d+) exit=(\d+)$")
NOTE_DONE_RE = re.compile(r"^(\S+Z) done: ")


def parse_takt_log(path):
    """The operations of one takt log in order, with invocation boundaries and exit-note times."""
    ops = []
    pending = None
    for line in open(path, encoding="utf-8", errors="replace"):
        line = line.rstrip("\n")
        m = STEP_RE.match(line)
        if m:
            if pending:
                ops.append(pending)
            by = "harness" if m.group(3) else "deterministic"
            pending = {"type": "step", "action": m.group(1), "by": by, "effect": None}
            continue
        if line.startswith("  effect: ") and pending:
            pending["effect"] = line[len("  effect: "):]
            ops.append(pending)
            pending = None
            continue
        if pending and not line.startswith("  "):
            ops.append(pending)
            pending = None
        m = STOP_RE.match(line)
        if m:
            ops.append({"type": "stop", "stop": m.group(1)})
            continue
        if line.startswith("takt: ") or line.startswith("Error: "):
            ops.append({"type": "refusal"})
            continue
        if line.startswith("--- "):
            label = line[4:].split(":")[0].strip()
            ops.append({"type": "grant"} if label == "approve" else {"type": "boundary", "label": label})
            continue
        m = NOTE_EXIT_RE.match(line)
        if m:
            ops.append({"type": "exit_note", "at": m.group(1), "resume": m.group(2) != "takt scenario run"})
            continue
        m = NOTE_DONE_RE.match(line)
        if m:
            ops.append({"type": "done", "at": m.group(1)})
    if pending:
        ops.append(pending)
    return ops


def takt_events(rec, run_dir, logs):
    """Journey events of a takt run. Times only where a record states them."""
    src_log = [tilde(p) for p in logs]
    ops = []
    for p in logs:
        ops.extend(parse_takt_log(p))
    notes = [o for o in ops if o["type"] == "exit_note"]
    t0 = None
    if notes and rec["wall_seconds"] is not None:
        t0 = iso(notes[-1]["at"]) - datetime.timedelta(seconds=rec["wall_seconds"])
    if t0 is None:
        done = [o for o in ops if o["type"] == "done"]
        if done and rec["wall_seconds"] is not None:
            t0 = iso(done[-1]["at"]) - datetime.timedelta(seconds=rec["wall_seconds"])

    def rel(at):
        return None if t0 is None else round((iso(at) - t0).total_seconds(), 1)

    calls, steps_src = [], None
    if run_dir and os.path.exists(os.path.join(run_dir, "steps.jsonl")):
        steps_src = os.path.join(run_dir, "steps.jsonl")
        calls = [json.loads(l) for l in open(steps_src) if l.strip()]
    timed_steps, ev_src = None, None
    if run_dir:
        found = sorted(glob.glob(os.path.join(run_dir, "tree", ".engineering", "cases", "*", "events.jsonl")))
        if found:
            ev_src = found[0]
            timed_steps = [e for e in load(ev_src) if e["event"] in ("step", "stop", "opened")]

    out = []
    # events.jsonl gives every step its own time: use it as the order and the clock
    if timed_steps is not None:
        prev = None
        n_open = 0
        for e in timed_steps:
            if e["event"] == "opened":
                n_open += 1
                if n_open > 1:
                    out.append(dict(kind="resume", by=None, effect=None, t_start=prev, t_end=rel(e["at"]),
                                    source=tilde(ev_src)))
                prev = rel(e["at"])
                continue
            if e["event"] == "stop":
                kind, eff = stop_kind(e["stop"])
                out.append(dict(kind=kind, by=None, effect=eff, t_start=rel(e["at"]), t_end=rel(e["at"]),
                                source=tilde(ev_src)))
                prev = rel(e["at"])
                continue
            by = "harness" if e.get("by") == "harness" else "deterministic"
            kind, eff = classify_step(e["action"], by, e.get("summary"))
            out.append(dict(kind=kind, by=by, effect=eff, action=e["action"], t_start=prev,
                            t_end=rel(e["at"]), source=tilde(ev_src)))
            prev = rel(e["at"])
        return out, t0, "timed"

    call_i = 0
    note_i = 0
    for o in ops:
        if o["type"] == "step":
            kind, eff = classify_step(o["action"], o["by"], o["effect"])
            ev = dict(kind=kind, by=o["by"], effect=eff, action=o["action"], t_start=None, t_end=None,
                      source=src_log[0] if len(src_log) == 1 else ", ".join(src_log))
            if o["by"] == "harness" and call_i < len(calls) and t0 is not None:
                c = calls[call_i]
                end = iso(c["recorded_at"])
                ev["t_end"] = rel(c["recorded_at"])
                ev["t_start"] = round((end - t0).total_seconds() - c["duration_ms"] / 1000, 1)
                ev["source"] = tilde(steps_src)
                ev["num_turns"] = c.get("num_turns")
            if o["by"] == "harness":
                call_i += 1
            out.append(ev)
        elif o["type"] == "stop":
            kind, eff = stop_kind(o["stop"])
            t = rel(notes[note_i]["at"]) if note_i < len(notes) else None
            out.append(dict(kind=kind, by=None, effect=eff, t_start=t, t_end=t, source=src_log[0]))
        elif o["type"] == "refusal":
            t = rel(notes[note_i]["at"]) if note_i < len(notes) else None
            out.append(dict(kind="refusal", by=None, effect="refused", t_start=0.0 if t is not None else None,
                            t_end=t, source=src_log[0]))
        elif o["type"] == "exit_note":
            note_i += 1
            if o.get("resume") is False:
                pass
        elif o["type"] == "boundary":
            out.append(dict(kind="resume", by=None, effect=o["label"], t_start=None, t_end=None,
                            source=src_log[0]))
        elif o["type"] == "grant":
            out.append(dict(kind="other", by=None, effect="granted", t_start=None, t_end=None,
                            source=src_log[0]))
    # baseline resumes: the runner's notes mark them; put one between the two invocations
    if any(n["resume"] for n in notes):
        merged = []
        stops_seen = 0
        for ev in out:
            merged.append(ev)
            if ev["kind"] in ("no_progress", "approval_wait", "refusal") and ev["t_end"] is not None:
                stops_seen += 1
                if stops_seen < len(notes):
                    merged.append(dict(kind="resume", by=None, effect="resume", t_start=ev["t_end"],
                                       t_end=None, source=src_log[0]))
        out = merged
    timed = any(e["t_end"] is not None for e in out)
    return out, t0, "partly timed" if timed else "order only"


def claude_events(rec, agent_json):
    a = json.load(open(agent_json))
    turns = a.get("num_turns")
    out = [dict(kind="model_call", by="harness", effect=None, t_start=None, t_end=None,
                source=tilde(agent_json)) for _ in range(turns or 0)]
    return out, "order only"


def codex_class(cmd):
    c = cmd
    if "b10x-gates api" in c and "/pulls" in c and "POST" in c:
        return "b10x-gates", "pr_open"
    if "push" in c and ("b10x-gates bot" in c or "git push" in c):
        return "b10x-gates" if "b10x-gates" in c else "git", "push"
    if "gh pr checks" in c:
        return "gh", "ci_wait"
    if re.search(r"(^|[\s'&;])\./check\.sh|unittest|pytest", c):
        return "test", "test"
    if re.search(r"\bgh\b", c):
        return "gh", "other"
    if "b10x-gates" in c:
        return "b10x-gates", "other"
    if re.search(r"\bgit (commit|add|checkout|switch|branch|reset|rebase|merge)\b", c):
        return "git", "other"
    if re.search(r"sed -i|apply_patch|>\s*\S+\.(py|md)", c):
        return "edit", "edit"
    if re.search(r"\b(cat|rg|ls|sed -n|head|tail|grep|find|wc)\b|\bgit (status|diff|log|show|remote)\b", c):
        return "read", "read"
    return "other", "other"


def codex_events(rec, agent_jsonl):
    items = {}
    order = []
    for line in open(agent_jsonl, encoding="utf-8"):
        j = json.loads(line)
        it = j.get("item")
        if not it:
            continue
        if j["type"] == "item.started" and it["id"] not in items:
            order.append(it["id"])
        if it["id"] not in items:
            items[it["id"]] = it
        if j["type"] == "item.completed":
            if it["id"] not in order:
                order.append(it["id"])
            items[it["id"]] = it
    out = []
    for i in order:
        it = items[i]
        t = it.get("type")
        if t == "command_execution":
            cls, kind = codex_class(it.get("command") or "")
            eff = "fail" if kind == "test" and it.get("exit_code") not in (0, None) else None
            out.append(dict(kind=kind, by="harness", effect=eff, tool_class=cls, t_start=None, t_end=None,
                            source=tilde(agent_jsonl)))
        elif t == "file_change":
            out.append(dict(kind="edit", by="harness", effect="changed", tool_class="edit", t_start=None,
                            t_end=None, source=tilde(agent_jsonl)))
        elif t in ("agent_message", "reasoning"):
            out.append(dict(kind="model_call", by="harness", effect=t, t_start=None, t_end=None,
                            source=tilde(agent_jsonl)))
    return out, "order only"


def refusal_only(rec, log):
    ops = parse_takt_log(log)
    done = [o for o in ops if o["type"] == "done"]
    end = None
    for line in open(log, encoding="utf-8", errors="replace"):
        m = re.match(r"^(\S+Z) (agent|takt scenario run) exit=\d+$", line.strip())
        if m:
            end = m.group(1)
    end = end or (done[-1]["at"] if done else None)
    if end is None or rec["wall_seconds"] is None:
        return [dict(kind="refusal", by=None, effect="refused", t_start=None, t_end=None, source=tilde(log))], \
            None, "order only"
    t0 = iso(end) - datetime.timedelta(seconds=rec["wall_seconds"])
    return [dict(kind="refusal", by=None, effect="refused", t_start=0.0,
                 t_end=float(rec["wall_seconds"]), source=tilde(log))], t0, "timed"


def journey(rec):
    srcs = [expand(s) for s in rec["source"]]
    run_dir = os.path.dirname(srcs[0]) if "takt-baseline" in srcs[0] else None
    if rec["arm"] == "takt":
        logs = [s for s in srcs if s.endswith(".log")]
        if rec["tokens"]["sum"] is None and rec["steps"] is None and run_dir:
            return refusal_only(rec, logs[0])
        return takt_events(rec, run_dir, logs)
    agent_json = [s for s in srcs if s.endswith("agent.json")]
    agent_jsonl = [s for s in srcs if s.endswith("agent.jsonl")]
    if agent_jsonl:
        ev, mode = codex_events(rec, agent_jsonl[0])
        return ev, None, mode
    if agent_json:
        ev, mode = claude_events(rec, agent_json[0])
        return ev, None, mode
    log = [s for s in srcs if s.endswith("run.log")]
    return refusal_only(rec, log[0])


def mark_patterns(evs):
    """Small labels: the seed-time pr.open, the second CI, an inspect step before NoProgress."""
    edited = False
    seed_done = False
    ci_passes = 0
    for k, e in enumerate(evs):
        if e["kind"] == "edit" and e.get("effect") == "changed":
            edited = True
        if e.get("action") == "pr.open" and e.get("effect") == "nothing" and not edited and not seed_done:
            e["label"] = "seed pr.open"
            seed_done = True
        if e["kind"] == "ci_pass":
            ci_passes += 1
            if ci_passes == 2:
                e["label"] = "2nd CI"
        if e["kind"] == "no_progress" and k > 0 and evs[k - 1]["kind"] == "read" and evs[k - 1].get("by") == "harness":
            e["label"] = "inspect, then NoProgress"


def revision_order(waves, turns, revs):
    order = [None] + [w.get("measured_at") or w["merge_sha"] for w in waves]
    cand = {t["after"]["takt_revision"]: t["before"]["takt_revision"] for t in turns}
    for r in revs:
        if r in order:
            continue
        b = cand.get(r)
        if b in order:
            order.insert(order.index(b) + 1, r)
        else:
            order.append(r)
    return order


# --- drawing ---------------------------------------------------------------------------------

W = 1100
X0, X1 = 260, 1070
LANE = 21
TOP = 86


def draw(lanes, legend_kinds, data_dates):
    timed = [l for l in lanes if l["mode"] != "order only"]
    tmax = max([e["t_end"] or e["t_start"] or 0 for l in timed for e in l["events"]] + [10])
    tick = 30 if tmax <= 360 else 60
    tmax = tick * (int(tmax // tick) + 1)
    order_max = max([len(l["events"]) for l in lanes if l["mode"] == "order only"] + [1])
    step_w = min(16.0, (X1 - X0) / order_max)

    def xs(t):
        return X0 + (X1 - X0) * t / tmax

    rows = []
    y = TOP
    last_arm = last_rev = object()
    for l in lanes:
        if l["arm"] != last_arm:
            y += 8
            rows.append(("arm", y, ARM_NAME.get(l["arm"], l["arm"])))
            y += 18
            last_arm, last_rev = l["arm"], object()
        if l["arm"] == "takt" and l["revision"] != last_rev:
            rows.append(("rev", y, l["revision_label"]))
            y += 15
            last_rev = l["revision"]
        rows.append(("lane", y, l))
        y += LANE
    plot_bottom = y
    H = plot_bottom + 150

    css = [".takt-journey{color:#26332f}", ".takt-journey .grid{stroke:#8a9a95;stroke-opacity:.22}"]
    dark = [".takt-journey{color:#d5dfdb}"]
    for k, (light, darkc) in COLOURS.items():
        css.append(f".takt-journey .k-{k}{{fill:{light};stroke:{light}}}")
        dark.append(f".takt-journey .k-{k}{{fill:{darkc};stroke:{darkc}}}")
    css.append(".takt-journey .noop{fill-opacity:0;stroke-width:1.3}")
    css.append(".takt-journey .placed{fill-opacity:.35;stroke-dasharray:2 1.5;stroke-width:1}")
    css.append(".takt-journey .call{stroke:currentColor;stroke-width:1.4}")
    style = "".join(css) + "@media (prefers-color-scheme: dark){" + "".join(dark) + "}" + \
        "@media print{" + "".join(css) + "}"

    o = [f'<svg class="takt-journey" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" '
         f'aria-labelledby="takt-journey-title" font-family="Inter, ui-sans-serif, system-ui, sans-serif" '
         f'font-size="11">',
         '<title id="takt-journey-title">Every recorded scenario 1 run, one lane each, in one vocabulary of '
         'operations</title>', f"<style>{style}</style>",
         f'<text x="20" y="24" font-size="15" font-weight="600" fill="currentColor">Scenario 1: every '
         f'recorded run, one lane each</text>',
         f'<text x="20" y="42" font-size="11" fill="currentColor" fill-opacity=".75">Timed lanes share the '
         f'axis: seconds from the run\'s start. Lanes marked "order only" have no times in their records and '
         f'show their operations as equal steps in order.</text>']
    for k in range(0, int(tmax // tick) + 1):
        x = xs(k * tick)
        o.append(f'<text x="{x:.1f}" y="{TOP - 8}" text-anchor="middle" font-size="10" fill="currentColor" '
                 f'fill-opacity=".7">{k * tick} s</text>')
    for kind_row, y, val in rows:
        if kind_row == "arm":
            o.append(f'<text x="20" y="{y + 12}" font-size="12" font-weight="600" fill="currentColor">'
                     f'{esc(val)}</text>')
            o.append(f'<line x1="20" x2="{X1}" y1="{y + 16}" y2="{y + 16}" class="grid"/>')
            continue
        if kind_row == "rev":
            o.append(f'<text x="30" y="{y + 10}" font-size="10.5" font-style="italic" fill="currentColor" '
                     f'fill-opacity=".8">{esc(val)}</text>')
            continue
        l = val
        cy = y + LANE / 2
        tag = {"order only": "order only", "partly timed": "model calls timed", "timed": ""}[l["mode"]]
        o.append(f'<text x="40" y="{cy + 4:.1f}" fill="currentColor">{esc(l["run"])}'
                 f'{" <tspan font-size=\"9.5\" fill-opacity=\".7\">" + esc(tag) + "</tspan>" if tag else ""}'
                 f'</text>')
        if l["mode"] != "order only":
            for k in range(0, int(tmax // tick) + 1):
                x = xs(k * tick)
                o.append(f'<line x1="{x:.1f}" x2="{x:.1f}" y1="{y + 2}" y2="{y + LANE - 2}" class="grid"/>')
        evs = l["events"]
        pos = []
        if l["mode"] == "order only":
            for i, e in enumerate(evs):
                pos.append((X0 + i * step_w, X0 + (i + 1) * step_w - 1.5, False))
        else:
            known = [(i, e["t_start"] if e["t_start"] is not None else e["t_end"],
                      e["t_end"] if e["t_end"] is not None else e["t_start"])
                     for i, e in enumerate(evs) if e["t_end"] is not None or e["t_start"] is not None]
            kmap = {i: (a, b) for i, a, b in known}
            for i, e in enumerate(evs):
                if i in kmap:
                    a, b = kmap[i]
                    xa, xb = xs(a), xs(b)
                    if xb - xa < 2.5:
                        xa, xb = xa - 1.5, xa + 1.5
                    pos.append((xa, xb, False))
                else:
                    # no time recorded: spread the untimed run of operations evenly between the
                    # timed operations either side of it, and draw them dashed
                    prev_known = max([j for j in kmap if j < i] or [-1])
                    next_known = min([j for j in kmap if j > i] or [len(evs)])
                    lo = kmap[prev_known][1] if prev_known >= 0 else 0.0
                    hi = kmap[next_known][0] if next_known < len(evs) else lo + 4
                    gap = list(range(prev_known + 1, next_known))
                    t = lo + (hi - lo) * (gap.index(i) + 1) / (len(gap) + 1)
                    pos.append((xs(t) - 2.5, xs(t) + 2.5, True))
        label_end = -1e9
        for e, (xa, xb, placed) in zip(evs, pos):
            cls = [f"k-{e['kind']}"]
            if e.get("effect") in ("nothing", "unchanged"):
                cls.append("noop")
            if placed:
                cls.append("placed")
            if l["arm"] == "takt" and e.get("by") == "harness":
                cls.append("call")
            tip = f'{l["run"]} #{e["seq"]}: {e["kind"]}'
            if e.get("action"):
                tip += f' ({e["action"]})'
            if e.get("effect"):
                tip += f', {e["effect"]}'
            if e["t_end"] is not None or e["t_start"] is not None:
                tip += f', {e["t_start"]}-{e["t_end"]} s'
            if e["kind"] == "resume":
                o.append(f'<line x1="{xa:.1f}" x2="{xa:.1f}" y1="{y + 1}" y2="{y + LANE - 1}" '
                         f'class="{" ".join(cls)}" stroke-width="2"><title>{esc(tip)}</title></line>')
                if xb - xa > 4 and not placed:
                    o.append(f'<line x1="{xa:.1f}" x2="{xb:.1f}" y1="{cy:.1f}" y2="{cy:.1f}" class="k-resume" '
                             f'stroke-width="1" stroke-dasharray="2 2"/>')
                continue
            o.append(f'<rect x="{xa:.1f}" y="{y + 4.5}" width="{max(xb - xa, 1.5):.1f}" height="{LANE - 9}" '
                     f'rx="1.5" class="{" ".join(cls)}"><title>{esc(tip)}</title></rect>')
            if e.get("label"):
                lx = max(xa, label_end + 6)
                if lx > xa + 1:
                    o.append(f'<line x1="{xa + 1:.1f}" x2="{lx:.1f}" y1="{y + 4}" y2="{y + 1}" '
                             f'stroke="currentColor" stroke-opacity=".5" stroke-width=".7"/>')
                o.append(f'<text x="{lx:.1f}" y="{y + 3}" font-size="8.5" fill="currentColor">'
                         f'{esc(e["label"])}</text>')
                label_end = lx + 4.6 * len(e["label"])
        if l["mode"] == "order only" and l.get("wall_seconds") is not None:
            xe = pos[-1][1] if pos else X0
            o.append(f'<text x="{xe + 6:.1f}" y="{cy + 3.5:.1f}" font-size="9.5" fill="currentColor" '
                     f'fill-opacity=".7">{len(evs)} operations; {l["wall_seconds"]} s in all</text>')

    ly = plot_bottom + 22
    for i, k in enumerate(legend_kinds):
        x = 20 + (i % 7) * 152
        yy = ly + (i // 7) * 18
        o.append(f'<rect x="{x}" y="{yy - 8}" width="12" height="9" rx="1.5" class="k-{k}"/>')
        o.append(f'<text x="{x + 17}" y="{yy}" fill="currentColor">{k.replace("_", " ")}</text>')
    yy = ly + 2 * 18 + 6
    o.append(f'<rect x="20" y="{yy - 8}" width="12" height="9" rx="1.5" class="k-pr_open noop"/>')
    o.append(f'<text x="37" y="{yy}" fill="currentColor">hollow: the step changed nothing</text>')
    o.append(f'<rect x="268" y="{yy - 8}" width="12" height="9" rx="1.5" class="k-edit call"/>')
    o.append(f'<text x="285" y="{yy}" fill="currentColor">outlined: a takt step taken by the harness '
             f'(one model call)</text>')
    o.append(f'<rect x="620" y="{yy - 8}" width="6" height="9" rx="1.5" class="k-test placed"/>')
    o.append(f'<text x="632" y="{yy}" fill="currentColor">dashed: no time recorded, placed in order '
             f'between timed operations</text>')
    span = data_dates[0] if len(set(data_dates)) == 1 else f"{min(data_dates)} to {max(data_dates)}"
    o.append(f'<text x="20" y="{yy + 26}" font-size="10.5" fill="currentColor" fill-opacity=".7">Data: '
             f'{span} (UTC), docs/metrics/journeys.jsonl from the raw records each run in '
             f'docs/metrics/scenario-runs.jsonl names. No token or cost figure is read from the journeys.'
             f'</text>')
    o.append(f'<text x="20" y="{yy + 42}" font-size="10.5" fill="currentColor" fill-opacity=".7">Free '
             f'Claude Code runs record only a turn count; Codex records its items in order without times. '
             f'First results, not a benchmark.</text>')
    o.append("</svg>")
    return "\n".join(o) + "\n"


def main():
    runs = [r for r in load(RUNS) if r["scenario"] == "1"]
    waves = load(WAVES)
    turns = load(TURNS)
    cand = {t["after"]["takt_revision"]: t["id"] for t in turns}
    order = revision_order(waves, turns, {r["takt_revision"] for r in runs if r["arm"] == "takt"})
    lanes = []
    lines = []
    for r in runs:
        evs, t0, mode = journey(r)
        for i, e in enumerate(evs, 1):
            e["seq"] = i
        mark_patterns(evs)
        rev = r["takt_revision"]
        if r["arm"] == "takt":
            label = rev or "before wave 06 (revision not recorded)"
            if rev in cand:
                label += f" ({cand[rev]} candidate)"
        else:
            label = None
        lanes.append({"run": r["id"], "arm": r["arm"], "revision": rev, "revision_label": label,
                      "mode": mode, "events": evs, "wall_seconds": r["wall_seconds"], "date": r["date"]})
        for e in evs:
            line = {"run": r["id"], "arm": r["arm"], "revision": rev, "seq": e["seq"],
                    "t_start": e["t_start"], "t_end": e["t_end"], "kind": e["kind"]}
            for k in ("by", "action", "effect", "tool_class", "label"):
                if e.get(k) is not None:
                    line[k] = e[k]
            line["t0"] = t0.strftime("%Y-%m-%dT%H:%M:%SZ") if t0 else None
            line["source"] = e["source"]
            lines.append(line)

    def key(l):
        a = ARM_ORDER.index(l["arm"]) if l["arm"] in ARM_ORDER else len(ARM_ORDER)
        rv = order.index(l["revision"]) if l["arm"] == "takt" and l["revision"] in order else 0
        return (a, rv, l["date"])

    lanes.sort(key=key)
    with open(OUT_JSONL, "w", encoding="utf-8") as f:
        for line in lines:
            f.write(json.dumps(line, ensure_ascii=False) + "\n")
    used = [k for k in KINDS if any(e["kind"] == k for l in lanes for e in l["events"])]
    svg = draw(lanes, used, sorted(r["date"][:10] for r in runs))
    with open(OUT_SVG, "w", encoding="utf-8") as f:
        f.write(svg)
    modes = {}
    for l in lanes:
        modes[l["mode"]] = modes.get(l["mode"], 0) + 1
    print(f"wrote {os.path.relpath(OUT_JSONL)} ({len(lines)} events) and {os.path.relpath(OUT_SVG)}: "
          f"{len(lanes)} lanes, " + ", ".join(f"{v} {k}" for k, v in sorted(modes.items())))
    if os.path.exists(DECK):
        deck = open(DECK, encoding="utf-8").read()
        a, b = deck.find(BEGIN), deck.find(END)
        if a != -1 and b > a:
            deck = deck[: a + len(BEGIN)] + "\n" + svg + deck[b:]
            with open(DECK, "w", encoding="utf-8") as f:
                f.write(deck)
            print("updated", os.path.relpath(DECK))


if __name__ == "__main__":
    main()
