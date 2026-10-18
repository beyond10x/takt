#!/usr/bin/env bash
# Rebuilds the protocol viewer from protocols/*/*.yaml and publishes it at /takt/protocols/ on the
# gh-pages branch. Run from anywhere inside the repository after a protocol changes:
#   docs/site/publish-protocols.sh            build, commit to gh-pages and push gh-pages only
#   docs/site/publish-protocols.sh --no-push  build and commit, push nothing
# Needs node and npm, the worktree CLI, and B10X_GATES_POLICY for the bot route.
set -euo pipefail
push=1
[ "${1:-}" = "--no-push" ] && push=0
repo=$(git rev-parse --show-toplevel)
app="$repo/docs/site/protocols"
rev=$(git -C "$repo" rev-parse --short HEAD)

(cd "$app" && npm ci --no-audit --no-fund && npm run build)

id="site-protocols-$rev"
tree=$(cd "$repo" && worktree create --purpose "publish protocol viewer at $rev" --id "$id" --base gh-pages | tail -1)
rm -rf "$tree/protocols"
cp -r "$app/dist" "$tree/protocols"
cd "$tree"
b10x-gates scan-text protocols/index.html
git add protocols
if git diff --cached --quiet; then
  echo "protocol viewer unchanged at $rev"
else
  msg=$(mktemp)
  printf '%s\n' "Protocol viewer rebuilt from protocols/ at $rev" '' \
    "- Built by docs/site/publish-protocols.sh from the protocol/1 files at $rev." > "$msg"
  b10x-gates bot -- commit -F "$msg"
  rm -f "$msg"
  [ "$push" = 1 ] && b10x-gates bot -- push origin HEAD:refs/heads/gh-pages
fi
cd "$repo" && worktree finish --discard-cache --archive "$id"
