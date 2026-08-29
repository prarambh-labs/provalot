#!/usr/bin/env bash
# Release-build latency check: p95 of 100 Stop hooks over a 500-line ledger must be under 50 ms.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -q
BIN="$PWD/target/release/provalot"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/.provalot/sessions"
python3 - "$WORK" <<'PY'
import json, sys, os
root = sys.argv[1]
with open(os.path.join(root, ".provalot/sessions/sess-1.jsonl"), "w") as f:
    for i in range(500):
        f.write(json.dumps({"kind":"run","ts":i,"agent_id":None,"tool":"Bash","command":"pytest -q","runner":"pytest","outcome":"pass","stdout_hash":"x","stderr_hash":"y","is_error":False,"interrupted":False}) + "\n")
PY
PAYLOAD="$(sed "s#\"__CWD__\"#\"$WORK\"#g; s#__CWD__#$WORK#g" fixtures/hooks/claude/stop-tests-only.json)"
python3 - "$BIN" "$WORK" "$PAYLOAD" <<'PY'
import subprocess, sys, time, statistics
bin_, cwd, payload = sys.argv[1], sys.argv[2], sys.argv[3]
times = []
for _ in range(100):
    t = time.perf_counter()
    subprocess.run([bin_, "hook", "claude"], input=payload.encode(), cwd=cwd, capture_output=True, check=True)
    times.append((time.perf_counter() - t) * 1000)
times.sort()
p50, p95 = statistics.median(times), times[int(len(times) * 0.95) - 1]
print(f"p50 {p50:.1f} ms, p95 {p95:.1f} ms, max {times[-1]:.1f} ms")
sys.exit(0 if p95 < 50 else 1)
PY
