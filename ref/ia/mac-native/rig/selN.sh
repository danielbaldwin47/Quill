#!/bin/bash
# selN.sh <n> <out.png> [downs]  — caret to doc top, down <downs> lines, home, select n cells, shoot.
set -u
D="$(dirname "$0")/drv.sh"
N="$1"; OUT="$2"; DOWNS="${3:-2}"
$D act >/dev/null
osascript -e 'tell application "System Events" to key code 126 using {command down}' >/dev/null
sleep 0.4
for i in $(seq 1 "$DOWNS"); do osascript -e 'tell application "System Events" to key code 125' >/dev/null; done
sleep 0.3
osascript -e 'tell application "System Events" to key code 123 using {command down}' >/dev/null
sleep 0.4
osascript -e "tell application \"System Events\" to repeat $N times
  key code 124 using {shift down}
end repeat" >/dev/null
sleep 0.7
screencapture -x -R 0,48,1512,380 "$OUT"
