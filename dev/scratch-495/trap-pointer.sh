#!/bin/bash
# trap-pointer.sh <label>: #327's trap on the #495 branch — the cursor crosses the Quill window mid-run,
# and the bench must refuse.
cd /home/diggle/repos/quill/.claude/worktrees/bench-launch-495 || exit 9
label=$1
out=/home/diggle/.claude/jobs/1a4897df/tmp/runs495
mkdir -p "$out"
for i in $(seq 1 30); do
  python3 tools/idle-check.py 20 && break
  echo "busy, waiting ($i)" >&2
done
(
  sleep 12
  geo=$(hyprctl clients -j | node -e '
    const c = JSON.parse(require("fs").readFileSync(0)).find((w) => w.class === "io.github.danielbaldwin47.Quill");
    if (c) process.stdout.write([Math.round(c.at[0] + c.size[0] / 2), Math.round(c.at[1] + c.size[1] / 2), c.at[0] + c.size[0] + 40, c.at[1] + c.size[1] + 40].join(" ") + "\n");')
  read -r cx cy ox oy <<< "$geo"
  echo "cursor into the window at $cx,$cy then out to $ox,$oy" > "$out/$label.trap"
  hyprctl repl "hl.dispatch(hl.dsp.cursor.move($cx, $cy)) return \"in\"" >> "$out/$label.trap"
  sleep 0.4
  hyprctl repl "hl.dispatch(hl.dsp.cursor.move($ox, $oy)) return \"out\"" >> "$out/$label.trap"
) &
start=$(date +%s.%N)
tools/gate bench --regimes prose_end_of_draft,bursts_and_pauses > "$out/$label.out" 2> "$out/$label.err"
code=$?
end=$(date +%s.%N)
wait
cp target/gate/bench.log "$out/$label.log"
echo "exit $code, wall $(awk "BEGIN{print $end - $start}") s"
cat "$out/$label.trap" "$out/$label.out" "$out/$label.err" | tail -8
