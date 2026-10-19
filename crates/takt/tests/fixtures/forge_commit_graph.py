"""Rewrite the first parent of one commit in a commit-graph file, and fix the checksum.

usage: forge_graph.py <commit-graph path> <commit oid hex> <new parent oid hex>
Both commits must be in the graph.
"""
import hashlib
import struct
import sys


def main() -> None:
    path, target, parent = sys.argv[1], sys.argv[2], sys.argv[3]
    data = bytearray(open(path, "rb").read())
    assert data[:4] == b"CGPH", "not a commit-graph"
    hash_len = 20 if data[5] == 1 else 32
    chunks = data[6]
    table = {}
    for n in range(chunks + 1):
        at = 8 + n * 12
        cid = bytes(data[at:at + 4])
        off = struct.unpack(">Q", data[at + 4:at + 12])[0]
        table[cid] = off
    fanout = table[b"OIDF"]
    count = struct.unpack(">I", data[fanout + 255 * 4:fanout + 256 * 4])[0]
    lookup = table[b"OIDL"]
    oids = [bytes(data[lookup + i * hash_len:lookup + (i + 1) * hash_len]).hex() for i in range(count)]
    t = oids.index(target)
    p = oids.index(parent)
    cdat = table[b"CDAT"]
    rec = cdat + t * (hash_len + 16)
    data[rec + hash_len:rec + hash_len + 4] = struct.pack(">I", p)
    data[-hash_len:] = (hashlib.sha1 if hash_len == 20 else hashlib.sha256)(bytes(data[:-hash_len])).digest()
    import os; os.unlink(path); open(path, "wb").write(bytes(data))
    print(f"forged: {target[:8]} first parent -> {parent[:8]} ({count} commits)")


main()
