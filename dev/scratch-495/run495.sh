#!/bin/bash
# run495.sh <label> [bench args...]: wait for an idle machine, run the bench on the #495 branch, keep its log.
cd /home/diggle/repos/quill/.claude/worktrees/bench-launch-495 || exit 9
label=$1; shift
out=/home/diggle/.claude/jobs/1a4897df/tmp/runs495
mkdir -p "$out"
for i in $(seq 1 30); do
  python3 tools/idle-check.py 20 && break
  echo "busy, waiting ($i)" >&2
done
start=$(date +%s.%N)
tools/gate bench "$@" > "$out/$label.out" 2> "$out/$label.err"
code=$?
end=$(date +%s.%N)
cp target/gate/bench.log "$out/$label.log"
echo "exit $code, wall $(awk "BEGIN{print $end - $start}") s"
cat "$out/$label.out"
node /home/diggle/.claude/jobs/1a4897df/tmp/launch495.mjs "$out/$label.log"
