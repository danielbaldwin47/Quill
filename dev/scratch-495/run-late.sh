#!/bin/bash
# run-late.sh <label> <regimes> [env assignments...]: an idle-gated bench in the late-495 scratch worktree.
cd /home/diggle/repos/quill/.claude/worktrees/late-495 || exit 9
label=$1; regimes=$2; shift 2
out=/home/diggle/.claude/jobs/1a4897df/tmp/runs495
mkdir -p "$out"
for i in $(seq 1 30); do
  python3 tools/idle-check.py 20 && break
  echo "busy, waiting ($i)" >&2
done
start=$(date +%s.%N)
env "$@" tools/gate bench --regimes "$regimes" > "$out/$label.out" 2> "$out/$label.err"
code=$?
end=$(date +%s.%N)
cp target/gate/bench.log "$out/$label.log"
echo "exit $code, wall $(awk "BEGIN{print $end - $start}") s"
cat "$out/$label.out"
grep -h "refus\|launch\|top-up" "$out/$label.log" | tail -6
node -e '
const fs = require("fs"), path = require("path");
for (const m of fs.readFileSync(process.argv[1], "utf8").matchAll(/wrote (dev\/shots\/latency\/bench-\S+?-\d{8}T\d{6}\.json)/g)) {
  const r = JSON.parse(fs.readFileSync(path.join("/home/diggle/repos/quill/.claude/worktrees/late-495", m[1]), "utf8"));
  const top = r.samples_ms.map((v, i) => `${v}@${i}`).sort((a, b) => parseFloat(b) - parseFloat(a)).slice(0, 12);
  console.log(r.regime, JSON.stringify(r.launch), "worst", top.join(" "));
}' "$out/$label.log"
