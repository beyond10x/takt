"""Overlay every recorded scenario run on one timeline, in one vocabulary.

Run as `python3 -I docs/metrics/journey.py`. Standard library only. For each record in
scenario-runs.jsonl it reads the raw files the record names (and the run's steps.jsonl and
events.jsonl beside them, where present), writes one line per operation to journeys.jsonl, and
draws journey.svg: one lane per run, grouped by arm and then by takt revision. When
docs/pitch/index.html holds the markers <!-- journey.svg begin --> and <!-- journey.svg end -->,
the SVG between them is replaced too.

The SVG is whole on its own. As in timeline.svg, the plot is one group between <!-- plot begin -->
and <!-- plot end --> whose data-plot attribute is its box, and <metadata id="takt-journey-key">
holds the title, the introduction, the legend and the note as JSON (takt-chart-key/1), so a page
can show the plot alone in a frame that scrolls and lay out the rest as its own text.

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
    fmt = "%Y-%m-%dT%H:%M:%S.%fZ" if "." in s else "%Y-%m-%dT%H:%M:%SZ"
    return datetime.datetime.strptime(s, fmt).replace(tzinfo=datetime.timezone.utc)


def from_ms(ms):
    return datetime.datetime.fromtimestamp(ms / 1000, tz=datetime.timezone.utc)


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


STEP_RE = re.compile(r"^step \d+: (\S+) (\{.*\}|\((.+)\))(?: · .*)?\s*$")
STOP_RE = re.compile(r"^stopped: (.*?); steps \d+, tokens \d+(?:, wall .*?)?, case ")
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

    def call_span(c):
        """A harness step's model call: the shim's started_at and recorded_at, or else
        recorded_at minus duration_ms."""
        if t0 is None or "recorded_at" not in c:
            return None
        end = rel(c["recorded_at"])
        if c.get("started_at"):
            return rel(c["started_at"]), end
        if c.get("duration_ms") is not None:
            return round((iso(c["recorded_at"]) - t0).total_seconds() - c["duration_ms"] / 1000, 1), end
        return None

    out = []
    # events.jsonl gives every step its own time: use it as the order and the clock. Where the
    # shim's steps.jsonl holds one line per harness step, its finer times replace the harness steps'.
    if timed_steps is not None:
        n_harness = sum(1 for e in timed_steps if e["event"] == "step" and e.get("by") == "harness")
        use_calls = len(calls) == n_harness and any(c.get("started_at") for c in calls)
        call_i = 0
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
            ev = dict(kind=kind, by=by, effect=eff, action=e["action"], t_start=prev,
                      t_end=rel(e["at"]), source=tilde(ev_src))
            if by == "harness":
                span = call_span(calls[call_i]) if use_calls else None
                if span:
                    ev["t_start"], ev["t_end"] = span
                    ev["source"] = tilde(steps_src)
                    ev["num_turns"] = calls[call_i].get("num_turns")
                call_i += 1
            out.append(ev)
            prev = rel(e["at"])
        return out, t0, "timed"

    call_i = 0
    note_i = 0
    for o in ops:
        if o["type"] == "step":
            kind, eff = classify_step(o["action"], o["by"], o["effect"])
            ev = dict(kind=kind, by=o["by"], effect=eff, action=o["action"], t_start=None, t_end=None,
                      source=src_log[0] if len(src_log) == 1 else ", ".join(src_log))
            if o["by"] == "harness" and call_i < len(calls):
                span = call_span(calls[call_i])
                if span:
                    ev["t_start"], ev["t_end"] = span
                    ev["source"] = tilde(steps_src)
                    ev["num_turns"] = calls[call_i].get("num_turns")
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
    """The class and kind of one shell command, from Codex or from Claude Code's Bash tool."""
    c = cmd
    if "b10x-gates api" in c and "/pulls" in c and "POST" in c:
        return "b10x-gates", "pr_open"
    if "gh pr create" in c:
        return "gh", "pr_open"
    if "push" in c and ("b10x-gates bot" in c or "git push" in c):
        return "b10x-gates" if "b10x-gates" in c else "git", "push"
    if "gh pr checks" in c or "gh run watch" in c:
        return "gh", "ci_wait"
    if re.search(r"(^|[\s'&;])\./check\.sh|unittest|pytest", c):
        return "test", "test"
    if re.search(r"\bgh\b", c):
        return "gh", "other"
    if "b10x-gates" in c:
        return "b10x-gates", "other"
    if re.search(r"\bgit (commit|add|checkout|switch|reset|rebase|merge)\b|\bgit branch(?!\s+(-a|-r|-v|-vv|--list|--show-current)\b)", c):
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


AGENT_EXIT_RE = re.compile(r"^(\S+Z) agent exit=\d+$")
CLAUDE_TOOL_KIND = {"Read": "read", "Glob": "read", "Grep": "read", "LS": "read", "Edit": "edit",
                    "Write": "edit", "MultiEdit": "edit", "NotebookEdit": "edit"}


def stream_lines(path):
    """(epoch ms, event) pairs of an agent.stream.tsv: the runner's arrival stamp, a tab, the JSON."""
    out = []
    for line in open(path, encoding="utf-8", errors="replace"):
        ms, _, js = line.rstrip("\n").partition("\t")
        try:
            out.append((int(ms), json.loads(js)))
        except ValueError:
            continue
    return out


def stream_t0(rec, run_log):
    """The run's start, as for takt: the runner's agent exit note minus wall_seconds."""
    if rec["wall_seconds"] is None or not os.path.exists(run_log):
        return None
    end = None
    for line in open(run_log, encoding="utf-8", errors="replace"):
        m = AGENT_EXIT_RE.match(line.strip())
        if m:
            end = m.group(1)
    return iso(end) - datetime.timedelta(seconds=rec["wall_seconds"]) if end else None


def claude_stream_events(rec, tsv, t0):
    """Claude Code's stream-json, each line stamped on arrival. A model call runs from the
    previous event to the last line of its assistant message; a tool runs from its tool_use to
    its tool_result. Events inside a sub-agent (parent_tool_use_id set) fall inside the Task
    tool's span and are not drawn on their own."""
    src = tilde(tsv)

    def rel(ms):
        return round((from_ms(ms) - t0).total_seconds(), 1)

    out, calls, tools = [], {}, {}
    prev = None
    for ms, e in stream_lines(tsv):
        t = e.get("type")
        if e.get("parent_tool_use_id"):
            continue
        if t == "system" and e.get("subtype") == "init":
            prev = rel(ms)
        elif t == "assistant":
            mid = e.get("message", {}).get("id")
            if mid not in calls:
                calls[mid] = dict(kind="model_call", by="harness", effect=None, t_start=prev, t_end=rel(ms),
                                  source=src)
                out.append(calls[mid])
            calls[mid]["t_end"] = rel(ms)
            for b in e.get("message", {}).get("content", []):
                if not isinstance(b, dict) or b.get("type") != "tool_use":
                    continue
                name = b.get("name") or ""
                if name == "Bash":
                    cls, kind = codex_class((b.get("input") or {}).get("command") or "")
                else:
                    cls, kind = name, CLAUDE_TOOL_KIND.get(name, "other")
                ev = dict(kind=kind, by="harness", effect="changed" if kind == "edit" else None,
                          tool_class=cls, t_start=rel(ms), t_end=None, source=src)
                tools[b.get("id")] = ev
                out.append(ev)
            prev = rel(ms)
        elif t == "user":
            for b in e.get("message", {}).get("content", []):
                if isinstance(b, dict) and b.get("type") == "tool_result" and b.get("tool_use_id") in tools:
                    ev = tools[b["tool_use_id"]]
                    ev["t_end"] = rel(ms)
                    if b.get("is_error") and ev["kind"] == "test":
                        ev["effect"] = "fail"
            prev = rel(ms)
        elif t == "result":
            out.append(dict(kind="other", by=None, effect=e.get("subtype") or "result", t_start=rel(ms),
                            t_end=rel(ms), source=src))
    for ev in out:
        if ev["t_start"] is None:
            ev["t_start"] = ev["t_end"]
    return out


def codex_stream_events(rec, tsv, t0):
    """Codex's --json events, each line stamped on arrival. A command or file change runs from
    its item.started to its item.completed. A model call runs from the previous event to the
    next thing the model emitted: an agent message, or the start of a command or file change."""
    src = tilde(tsv)

    def rel(ms):
        return round((from_ms(ms) - t0).total_seconds(), 1)

    out, items = [], {}
    prev = None
    for ms, e in stream_lines(tsv):
        t = e.get("type") or ""
        it = e.get("item")
        if t in ("thread.started", "turn.started"):
            prev = rel(ms) if prev is None else prev
            continue
        if t == "turn.completed":
            out.append(dict(kind="other", by=None, effect="turn.completed", t_start=rel(ms), t_end=rel(ms),
                            source=src))
            continue
        if not it or not t.startswith("item."):
            continue
        ty = it.get("type")
        if ty in ("agent_message", "reasoning"):
            if t == "item.completed":
                out.append(dict(kind="model_call", by="harness", effect=ty, t_start=prev, t_end=rel(ms),
                                source=src))
                prev = rel(ms)
            continue
        if ty not in ("command_execution", "file_change"):
            continue
        ev = items.get(it.get("id"))
        if ev is None:
            out.append(dict(kind="model_call", by="harness", effect=None, t_start=prev, t_end=rel(ms),
                            source=src))
            if ty == "command_execution":
                cls, kind = codex_class(it.get("command") or "")
            else:
                cls, kind = "edit", "edit"
            ev = dict(kind=kind, by="harness", effect="changed" if ty == "file_change" else None,
                      tool_class=cls, t_start=rel(ms), t_end=rel(ms), source=src)
            items[it.get("id")] = ev
            out.append(ev)
        if t == "item.completed":
            ev["t_end"] = rel(ms)
            if ev["kind"] == "test" and it.get("exit_code") not in (0, None):
                ev["effect"] = "fail"
        prev = rel(ms)
    for ev in out:
        if ev["t_start"] is None:
            ev["t_start"] = ev["t_end"]
    return out


def stream_journey(rec, tsv, run_log):
    t0 = stream_t0(rec, run_log)
    if t0 is None:
        lines = stream_lines(tsv)
        if not lines:
            return [], None, "order only"
        t0 = from_ms(lines[0][0]).replace(microsecond=0)
    ev = codex_stream_events(rec, tsv, t0) if rec["arm"] == "codex" else claude_stream_events(rec, tsv, t0)
    return ev, t0, "timed"


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
    stream = [s for s in srcs if s.endswith("agent.stream.tsv")]
    if stream and os.path.exists(stream[0]):
        return stream_journey(rec, stream[0], os.path.join(os.path.dirname(stream[0]), "run.log"))
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
        if b is not None and b in order:
            order.insert(order.index(b) + 1, r)
        else:
            order.append(r)
    return order


# --- drawing ---------------------------------------------------------------------------------
#
# The chart is drawn at its own size, 960 px wide, with every text at 12 px or more: a page shows it
# at 1:1 and scrolls it sideways on a narrow screen rather than shrinking it. The patterns
# mark_patterns finds are drawn as marks in the lane's own headroom, named in the legend, so no
# label reaches into the lane above.

W = 960
X0, X1 = 236, 936
LANE = 24
FS = 12
MARKS = {"seed pr.open": "seed", "2nd CI": "ci2", "inspect, then NoProgress": "inspect"}
MARK_TEXT = {"seed": "seed pr.open: a pr.open step before any file changed",
             "ci2": "2nd CI: the run's second passing CI read",
             "inspect": "inspect, then NoProgress: a harness step that only read, then the NoProgress stop"}


def text_w(s, size=FS, bold=False):
    """An estimate of a label's width in Inter, on the wide side."""
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


TITLE = "Scenario 1: every recorded run, one lane each"


def swatch(classes, x, y, width=12):
    """An operation's swatch in the legend, its foot at y."""
    return f'<rect x="{x}" y="{y - 10}" width="{width}" height="10" rx="1.5" class="{classes}"/>'


def mark(kind, x, y):
    """A pattern mark centred at x, its foot at y."""
    if kind == "seed":
        return f'<path d="M{x - 4:.1f},{y - 5:.1f}L{x + 4:.1f},{y - 5:.1f}L{x:.1f},{y:.1f}Z" class="mark"/>'
    if kind == "ci2":
        return f'<circle cx="{x:.1f}" cy="{y - 2.8:.1f}" r="2.8" class="mark"/>'
    return (f'<path d="M{x - 3:.1f},{y - 6:.1f}L{x + 3:.1f},{y:.1f}M{x - 3:.1f},{y:.1f}L{x + 3:.1f},{y - 6:.1f}" '
            f'class="mark-s" stroke-width="1.6"/>')


def draw(lanes, legend_kinds, data_dates):
    timed = [l for l in lanes if l["mode"] != "order only"]
    tmax = max([e["t_end"] or e["t_start"] or 0 for l in timed for e in l["events"]] + [10])
    tick = 30 if tmax <= 360 else 60
    tmax = tick * (int(tmax // tick) + 1)
    order_max = max([len(l["events"]) for l in lanes if l["mode"] == "order only"] + [1])
    step_w = min(16.0, (X1 - X0) / order_max)

    def xs(t):
        return X0 + (X1 - X0) * t / tmax

    intro = ('Timed lanes share the axis: seconds from the run\'s start. Lanes marked "order only" have no '
             'times in their records and show their operations as equal steps in order.')
    intro_lines = wrap(intro, W - 40)
    top = 44 + 16 * len(intro_lines) + 26

    rows = []
    y = top
    last_arm = last_rev = object()
    for l in lanes:
        if l["arm"] != last_arm:
            y += 10
            rows.append(("arm", y, ARM_NAME.get(l["arm"], l["arm"])))
            y += 22
            last_arm, last_rev = l["arm"], object()
        if l["arm"] == "takt" and l["revision"] != last_rev:
            rows.append(("rev", y, l["revision_label"]))
            y += 18
            last_rev = l["revision"]
        rows.append(("lane", y, l))
        y += LANE
    plot_bottom = y

    css = [".takt-journey{color:#26332f}", ".takt-journey .grid{stroke:#8a9a95;stroke-opacity:.22}",
           ".takt-journey .mut{fill:#4f625d}", ".takt-journey .mark{fill:currentColor}",
           ".takt-journey .mark-s{stroke:currentColor;fill:none}"]
    dark = [".takt-journey{color:#d5dfdb}", ".takt-journey .mut{fill:#a8b8b3}"]
    for k, (light, darkc) in COLOURS.items():
        css.append(f".takt-journey .k-{k}{{fill:{light};stroke:{light}}}")
        dark.append(f".takt-journey .k-{k}{{fill:{darkc};stroke:{darkc}}}")
    css.append(".takt-journey .noop{fill-opacity:0;stroke-width:1.3}")
    css.append(".takt-journey .placed{fill-opacity:.35;stroke-dasharray:2 1.5;stroke-width:1}")
    css.append(".takt-journey .call{stroke:currentColor;stroke-width:1.4}")
    style = "".join(css) + "@media (prefers-color-scheme: dark){" + "".join(dark) + "}" + \
        "@media print{" + "".join(css) + "}"

    # head: the title and the introduction; o: the plot; foot: the legend and the note
    head = [f'<text x="20" y="24" font-size="15" font-weight="600" fill="currentColor">{esc(TITLE)}</text>']
    for k, line in enumerate(intro_lines):
        head.append(f'<text x="20" y="{44 + 16 * k}" class="mut">{esc(line)}</text>')
    o = [f"<style>{style}</style>"]
    foot = []
    for k in range(0, int(tmax // tick) + 1):
        x = xs(k * tick)
        o.append(f'<text x="{x:.1f}" y="{top - 8}" text-anchor="middle" class="mut">{k * tick} s</text>')
    used_marks = []
    for kind_row, y, val in rows:
        if kind_row == "arm":
            o.append(f'<text x="20" y="{y + 13}" font-size="13" font-weight="600" fill="currentColor">'
                     f'{esc(val)}</text>')
            o.append(f'<line x1="20" x2="{X1}" y1="{y + 18}" y2="{y + 18}" class="grid"/>')
            continue
        if kind_row == "rev":
            o.append(f'<text x="30" y="{y + 12}" font-style="italic" class="mut">{esc(val)}</text>')
            continue
        l = val
        cy = y + LANE / 2
        tag = {"order only": "order only", "partly timed": "model calls timed", "timed": ""}[l["mode"]]
        o.append(f'<text x="40" y="{cy + 4:.1f}" fill="currentColor">{esc(l["run"])}'
                 f'{" <tspan class=\"mut\">" + esc(tag) + "</tspan>" if tag else ""}</text>')
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
        for e, (xa, xb, placed) in zip(evs, pos):
            cls = [f"k-{e['kind']}"]
            if e.get("effect") in ("nothing", "unchanged"):
                cls.append("noop")
            if placed:
                cls.append("placed")
            if l["arm"] == "takt" and e.get("by") == "harness":
                cls.append("call")
            tip = f'{l["run"]} op {e["seq"]}: {e["kind"]}'
            if e.get("action"):
                tip += f' ({e["action"]})'
            if e.get("effect"):
                tip += f', {e["effect"]}'
            if e.get("label"):
                tip += f'; {e["label"]}'
            if e["t_end"] is not None or e["t_start"] is not None:
                tip += f', {e["t_start"]}-{e["t_end"]} s'
            if e["kind"] == "resume":
                o.append(f'<line x1="{xa:.1f}" x2="{xa:.1f}" y1="{y + 2}" y2="{y + LANE - 2}" '
                         f'class="{" ".join(cls)}" stroke-width="2"><title>{esc(tip)}</title></line>')
                if xb - xa > 4 and not placed:
                    o.append(f'<line x1="{xa:.1f}" x2="{xb:.1f}" y1="{cy + 1:.1f}" y2="{cy + 1:.1f}" '
                             f'class="k-resume" stroke-width="1" stroke-dasharray="2 2"/>')
                continue
            o.append(f'<rect x="{xa:.1f}" y="{y + 7}" width="{max(xb - xa, 1.5):.1f}" height="{LANE - 12}" '
                     f'rx="1.5" class="{" ".join(cls)}"><title>{esc(tip)}</title></rect>')
            if e.get("label") in MARKS:
                k = MARKS[e["label"]]
                if k not in used_marks:
                    used_marks.append(k)
                o.append(mark(k, (xa + xb) / 2, y + 6))
        if l["mode"] == "order only" and l.get("wall_seconds") is not None:
            xe = pos[-1][1] if pos else X0
            o.append(f'<text x="{xe + 6:.1f}" y="{cy + 5:.1f}" class="mut">{len(evs)} operations; '
                     f'{l["wall_seconds"]} s in all</text>')

    # the plot's box: from the time axis's labels to under the last lane
    plot_box = (0, top - 24, W, plot_bottom + 6 - (top - 24))

    # legend: the operation kinds, flowing and wrapping at the chart's width, then the drawing
    # conventions and the pattern marks one to a line
    kinds = [(swatch(f"k-{k}", 1, 12), k.replace("_", " ")) for k in legend_kinds]
    ways = [(swatch("k-pr_open noop", 1, 12), "hollow: the step changed nothing"),
            (swatch("k-edit call", 1, 12), "outlined: a takt step taken by the harness (one model call)"),
            (swatch("k-test placed", 4, 12, 6), "dashed: no time recorded, placed in order between timed operations")]
    ways += [(mark(k, 7, 12), MARK_TEXT[k]) for k in used_marks]
    y = plot_bottom + 30
    x = 20
    for k in legend_kinds:
        text = k.replace("_", " ")
        wide = 17 + text_w(text) + 24
        if x + wide > W - 10:
            x, y = 20, y + 20
        foot.append(swatch(f"k-{k}", x, y))
        foot.append(f'<text x="{x + 17}" y="{y}" fill="currentColor">{text}</text>')
        x += wide
    y += 26
    foot.append(swatch("k-pr_open noop", 20, y))
    foot.append(f'<text x="37" y="{y}" fill="currentColor">{ways[0][1]}</text>')
    foot.append(swatch("k-edit call", 300, y))
    foot.append(f'<text x="317" y="{y}" fill="currentColor">{ways[1][1]}</text>')
    y += 20
    foot.append(swatch("k-test placed", 23, y, 6))
    foot.append(f'<text x="37" y="{y}" fill="currentColor">{ways[2][1]}</text>')
    for k in used_marks:
        y += 20
        foot.append(mark(k, 26, y - 2))
        foot.append(f'<text x="37" y="{y}" fill="currentColor">{esc(MARK_TEXT[k])}</text>')
    span = data_dates[0] if len(set(data_dates)) == 1 else f"{min(data_dates)} to {max(data_dates)}"
    note = (f'Data: {span} (UTC), docs/metrics/journeys.jsonl from the raw records each run in '
            f'docs/metrics/scenario-runs.jsonl names. No token or cost figure is read from the journeys. '
            f'Free and isolated runs without a stream record only a turn count (Claude Code) or their items '
            f'in order (Codex); runs with a stream are timed. First results, not a benchmark.')
    y += 30
    for line in wrap(note, W - 40):
        foot.append(f'<text x="20" y="{y}" class="mut">{esc(line)}</text>')
        y += 16
    H = int(y + 4)
    # the key a page lays out as its own text, each mark in a 14 px box
    key = {"format": "takt-chart-key/1", "class": "takt-journey", "title": TITLE, "intro": intro,
           "legend": [[{"mark": m, "text": t} for m, t in kinds], [{"mark": m, "text": t} for m, t in ways]],
           "notes": [{"text": note, "muted": True}]}
    svg = (f'<svg class="takt-journey" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" '
           f'width="{W}" height="{H}" role="img" aria-labelledby="takt-journey-title" '
           f'font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="{FS}">\n'
           '<title id="takt-journey-title">Every recorded scenario 1 run, one lane each, in one vocabulary of '
           'operations</title>\n'
           f'<metadata id="takt-journey-key">{html.escape(json.dumps(key, ensure_ascii=False), quote=False)}'
           '</metadata>')
    return "\n".join([svg] + head + ["<!-- plot begin -->", f'<g data-plot="{" ".join(str(v) for v in plot_box)}">']
                     + o + ["</g>", "<!-- plot end -->"] + foot + ["</svg>"]) + "\n"


def main():
    runs = [r for r in load(RUNS) if r["scenario"] == "1"]
    waves = load(WAVES)
    turns = load(TURNS)
    cand = {t["after"]["takt_revision"]: t["id"] for t in turns}
    order = revision_order(waves, turns, list(dict.fromkeys(r["takt_revision"] for r in runs if r["arm"] == "takt")))
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
