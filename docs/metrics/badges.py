"""Rewrite the badge block of README.md from the records, so no figure in it is typed by hand.

Run as `python3 -I docs/metrics/badges.py` from anywhere in the repository. Standard library only.
It reads:

- `aep plan artifact list --format json`: stories implemented, of all stories not archived, in all
  and per epic;
- `docs/metrics/improvement-turns.jsonl`: improvement turns accepted and promoted;
- `docs/metrics/scenario-runs.jsonl`: scenario runs recorded, and the held-out check's pass rate
  over the runs that have a result;
- `python3 -I scripts/check-citations --require-digests`: the ESS citations that resolve.

It replaces what lies between <!-- badges begin --> and <!-- badges end --> in README.md with
shields.io static badges, each linked to the record it was computed from.
"""

import json
import os
import re
import subprocess
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, os.pardir, os.pardir))
README = os.path.join(ROOT, "README.md")
BEGIN, END = "<!-- badges begin -->", "<!-- badges end -->"


def load(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def colour(done, total):
    if total == 0:
        return "lightgrey"
    r = done / total
    return "brightgreen" if r >= 0.9 else "green" if r >= 0.6 else "yellowgreen" if r >= 0.4 else "orange"


def shield(label, message, col):
    def part(s):
        return urllib.parse.quote(s.replace("-", "--").replace("_", "__"), safe="")
    return f"https://img.shields.io/badge/{part(label)}-{part(message)}-{col}"


def badge(label, message, col, link):
    return f"[![{label}: {message}]({shield(label, message, col)})]({link})"


def stories():
    out = subprocess.run(["aep", "plan", "artifact", "list", "--format", "json"], cwd=ROOT, check=True,
                         capture_output=True, text=True).stdout
    arts = json.loads(out)
    total, done, per_epic = 0, 0, {}
    for a in arts:
        if a["kind"] != "story" or a["status"] == "archived":
            continue
        total += 1
        done += a["status"] == "implemented"
        for r in a.get("relations", []):
            if r["relation"] == "decomposes" and r["target"].startswith("epic:"):
                e = per_epic.setdefault(r["target"], [0, 0])
                e[0] += a["status"] == "implemented"
                e[1] += 1
    return done, total, per_epic


def citations():
    p = subprocess.run(["python3", "-I", "scripts/check-citations", "--require-digests"], cwd=ROOT,
                       capture_output=True, text=True)
    m = re.search(r"citations: (\d+) in-repository, (\d+) resolved", p.stdout + p.stderr)
    return (int(m.group(2)), int(m.group(1))) if m else None


def main():
    done, total, per_epic = stories()
    turns = load(os.path.join(HERE, "improvement-turns.jsonl"))
    accepted = sum(1 for t in turns if t.get("outcome") == "accepted")
    promoted = sum(1 for t in turns if t.get("promoted_at"))
    runs = load(os.path.join(HERE, "scenario-runs.jsonl"))
    judged = [r for r in runs if r.get("acceptance") in ("pass", "fail")]
    passed = sum(1 for r in judged if r["acceptance"] == "pass")
    cites = citations()

    store = ".engineering/planning"
    lines = [badge("stories implemented", f"{done} of {total}", colour(done, total), store)]
    for epic in sorted(per_epic, key=lambda e: (-per_epic[e][1], e)):
        d, t = per_epic[epic]
        lines.append(badge(epic.split(":", 1)[1].replace("-", " "), f"{d}/{t} stories", colour(d, t),
                           f"{store}/epic/{epic.split(':', 1)[1]}.md"))
    lines.append(badge("improvement turns", f"{accepted} accepted, {promoted} promoted",
                       "blue" if accepted else "lightgrey", "docs/metrics/improvement-turns.jsonl"))
    lines.append(badge("scenario runs recorded", str(len(runs)), "blue", "docs/metrics/scenario-runs.jsonl"))
    lines.append(badge("held-out check", f"{passed} of {len(judged)} pass", colour(passed, len(judged)),
                       "docs/metrics/scenario-runs.jsonl"))
    if cites:
        lines.append(badge("ESS citations resolved", f"{cites[0]} of {cites[1]}", colour(*cites),
                           "ess/domains"))
    block = BEGIN + "\n" + "\n".join(lines) + "\n" + END

    text = open(README, encoding="utf-8").read()
    a, b = text.find(BEGIN), text.find(END)
    if a == -1 or b < a:
        raise SystemExit(f"README.md holds no {BEGIN} ... {END} block")
    text = text[:a] + block + text[b + len(END):]
    with open(README, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"README.md: {len(lines)} badges; stories {done}/{total}, turns {accepted} accepted, "
          f"{promoted} promoted, runs {len(runs)}, held-out {passed}/{len(judged)}, citations "
          f"{'%d/%d' % cites if cites else 'not read'}")


if __name__ == "__main__":
    main()
