"""Overwrite (or plant) the loose object file for <oid> in <git dir> with <content file>'s bytes
as a blob, keeping the oid. usage: plant_loose.py <git dir> <oid> <content file>"""
import os
import sys
import zlib

git_dir, oid, content_file = sys.argv[1:4]
data = open(content_file, "rb").read()
raw = b"blob %d\x00" % len(data) + data
path = os.path.join(git_dir, "objects", oid[:2], oid[2:])
os.makedirs(os.path.dirname(path), exist_ok=True)
if os.path.exists(path):
    os.chmod(path, 0o644)
    os.remove(path)
with open(path, "wb") as f:
    f.write(zlib.compress(raw))
print("planted", path)
