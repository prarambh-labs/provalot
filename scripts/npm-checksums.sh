#!/usr/bin/env bash
# Fill npm/checksums.json from a GitHub release's *.sha256 assets. Run before `npm publish`.
# Usage: scripts/npm-checksums.sh v0.1.0
set -euo pipefail
cd "$(dirname "$0")/.."
tag="${1:?usage: npm-checksums.sh vX.Y.Z}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
gh release download "$tag" --repo vaishach0523-P1/provalot --pattern '*.sha256' --dir "$work"
python3 - "$work" <<'PY'
import json, os, sys
d = sys.argv[1]
out = {}
for f in sorted(os.listdir(d)):
    digest, name = open(os.path.join(d, f)).read().split()[:2]
    out[name.lstrip('*')] = digest.lower()
json.dump(out, open("npm/checksums.json", "w"), indent=2)
open("npm/checksums.json", "a").write("\n")
print(f"wrote npm/checksums.json with {len(out)} digests")
PY
