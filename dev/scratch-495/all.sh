#!/bin/bash
# all.sh <label>: `tools/gate bench --all` on the #495 branch, detached so a run past the tool's 600 s
# cap is not killed, timed by itself; this call then waits on it up to 590 s.
cd /home/diggle/repos/quill/.claude/worktrees/bench-launch-495 || exit 9
label=$1
out=/home/diggle/.claude/jobs/1a4897df/tmp/runs495
mkdir -p "$out"
for i in $(seq 1 30); do
  python3 tools/idle-check.py 20 && break
  echo "busy, waiting ($i)" >&2
done
cat > "$out/$label.run" <<EOF
#!/bin/bash
cd /home/diggle/repos/quill/.claude/worktrees/bench-launch-495
start=\$(date +%s.%N)
tools/gate bench --all > "$out/$label.out" 2> "$out/$label.err"
code=\$?
end=\$(date +%s.%N)
cp target/gate/bench.log "$out/$label.log"
echo "exit \$code, wall \$(awk "BEGIN{print \$end - \$start}") s" > "$out/$label.done"
EOF
setsid nohup bash "$out/$label.run" > /dev/null 2>&1 &
for s in $(seq 1 590); do
  [ -f "$out/$label.done" ] && break
  sleep 1
done
if [ -f "$out/$label.done" ]; then cat "$out/$label.done" "$out/$label.out"; else echo "still running after 590 s"; fi
