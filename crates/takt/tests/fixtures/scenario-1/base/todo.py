#!/usr/bin/env python3
"""A todo list kept in a JSON file: add an item, list the items, mark one done."""

import argparse
import json
import os
import sys
from pathlib import Path

DEFAULT_FILE = "todo.json"


def load(path):
    path = Path(path)
    if not path.exists():
        return []
    return json.loads(path.read_text(encoding="utf-8"))


def save(path, items):
    Path(path).write_text(json.dumps(items, indent=2) + "\n", encoding="utf-8")


def add(items, text):
    text = text.strip()
    if not text:
        raise ValueError("a todo needs some text")
    items.append({"text": text, "done": False})
    return items


def done(items, number):
    if number < 1 or number > len(items):
        raise ValueError(f"there is no todo {number}")
    items[number - 1]["done"] = True
    return items


def render(items):
    return [
        f"{number}. [{'x' if item['done'] else ' '}] {item['text']}"
        for number, item in enumerate(items, start=1)
    ]


def main(argv=None):
    parser = argparse.ArgumentParser(prog="todo")
    parser.add_argument("--file", default=os.environ.get("TODO_FILE", DEFAULT_FILE))
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("list")
    add_command = commands.add_parser("add")
    add_command.add_argument("text")
    done_command = commands.add_parser("done")
    done_command.add_argument("number", type=int)
    args = parser.parse_args(argv)

    items = load(args.file)
    try:
        if args.command == "add":
            save(args.file, add(items, args.text))
        elif args.command == "done":
            save(args.file, done(items, args.number))
        else:
            for line in render(items):
                print(line)
    except ValueError as error:
        print(f"todo: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
