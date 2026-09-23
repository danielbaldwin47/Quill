#!/bin/bash
# leave-test.sh: does a scripted cursor crossing make an idle launch print "pointer left"?
(
  sleep 3
  hyprctl monitors -j | node -e 'for (const m of JSON.parse(require("fs").readFileSync(0))) process.stdout.write(`${m.name} ${m.x},${m.y} ${m.width}x${m.height} ws ${m.activeWorkspace.id}\n`)'
  geo=$(hyprctl clients -j | node -e '
    const c = JSON.parse(require("fs").readFileSync(0)).find((w) => w.class === "io.github.danielbaldwin47.Quill");
    if (c) process.stdout.write([c.at[0], c.at[1], c.size[0], c.size[1], c.workspace.id].join(" ") + "\n");')
  read -r x y w h ws <<< "$geo"
  echo "window at $x,$y size ${w}x$h on workspace $ws"
  for d in 0 10 20 30 40; do
    hyprctl repl "hl.dispatch(hl.dsp.cursor.move($((x + w / 2 + d)), $((y + h / 2 + d)))) return \"in $d\"" > /dev/null
    sleep 0.05
  done
  for d in 0 10 20 30; do
    hyprctl repl "hl.dispatch(hl.dsp.cursor.move($((x + w + 20 + d)), $((y + h / 2)))) return \"out $d\"" > /dev/null
    sleep 0.05
  done
  hyprctl cursorpos
) &
node /home/diggle/.claude/jobs/1a4897df/tmp/idle-launch-495.mjs /dev/null 6 2>&1 | grep -v "^pid"
wait
