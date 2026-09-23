#!/bin/bash
# probe-run.sh <label> [env assignments...]: an idle-gated bench of the four regimes with the probe on.
cd /home/diggle/repos/quill/.claude/worktrees/bench-489 || exit 9
label=$1; shift
dir=/home/diggle/.claude/jobs/1a4897df/tmp/probe/$label
rm -rf "$dir"; mkdir -p "$dir"
for i in $(seq 1 20); do
  python3 tools/idle-check.py 20 && break
  echo "busy, waiting ($i)" >&2
done
start=$(date +%s.%N)
env "$@" BENCH_PROBE_DIR="$dir" tools/gate bench --regimes ${REGIMES:-prose_end_of_draft,bursts_and_pauses,preview,annotators} > "$dir/out" 2> "$dir/err"
code=$?
end=$(date +%s.%N)
cp target/gate/bench.log "$dir/bench.log"
echo "exit $code, wall $(awk "BEGIN{print $end - $start}") s"
cat "$dir/out"
