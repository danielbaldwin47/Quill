#!/bin/bash
# wait-done.sh <label>: waits up to 580 s for a detached run's done-file, then prints it and the summary.
out=/home/diggle/.claude/jobs/1a4897df/tmp/runs495
for s in $(seq 1 580); do
  [ -f "$out/$1.done" ] && break
  sleep 1
done
if [ -f "$out/$1.done" ]; then cat "$out/$1.done" "$out/$1.out"; else echo "still running"; fi
