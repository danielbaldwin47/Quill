#!/usr/bin/env bash
# Put one caret or selection state on screen in iA Writer for Windows and shoot it.
#
# Keys go through ydotool rather than wtype: the app is Wine under XWayland and
# only saw the kernel-level injection. The window lives on a created headless
# output, so focus has to move off the owner's panel for as long as the
# keystrokes take and go straight back. Every state starts from Home, so none of
# them depends on where the last one left the caret, and the shot is taken just
# after the last keystroke because a caret that blinks is on at the moment it
# moves.
#
#     drive.sh <state>
set -euo pipefail
export YDOTOOL_SOCKET=/run/user/1000/.ydotool_socket

STATE="$1"
# Shots go beside the rig unless told otherwise; they are working frames, not
# judging evidence, and none of them belongs in the repository.
DIR="${IA_OUT:-${TMPDIR:-/tmp}/ia-shots}"
mkdir -p "$DIR"
STAGE="${IA_STAGE:-HEADLESS-184}"
HOME_MON="${IA_PANEL:-DP-3}"
PASSAGE="This is a test of the caret's column."

HOME_K=102 END_K=107 LEFT=105 RIGHT=106 SHIFT=42 CTRL=29 A=30

focus() { hyprctl repl "return hl.dispatch(hl.dsp.focus{monitor=hl.get_monitor(\"$1\")})" >/dev/null; }
tap()   { local n=$1 k=$2; for _ in $(seq "$n"); do ydotool key "$k:1" "$k:0"; done; }
shifted() { local n=$1 k=$2; ydotool key "$SHIFT:1"; tap "$n" "$k"; ydotool key "$SHIFT:0"; }

focus "$STAGE"
sleep 0.4

if [ "$STATE" = reset ]; then
  ydotool key "$CTRL:1" "$A:1" "$A:0" "$CTRL:0"
  sleep 0.2
  ydotool type "$PASSAGE"
  sleep 0.6
  focus "$HOME_MON"
  echo reset
  exit 0
fi

# Home twice, and a wide settle after it: the first key sent to a freshly
# focused Wine window is swallowed, which showed up as shots landing one cell
# short of the offset they were asked for.
ydotool key "$HOME_K:1" "$HOME_K:0"
sleep 0.2
ydotool key "$HOME_K:1" "$HOME_K:0"
sleep 0.4

case "$STATE" in
  # Caret alone at one offset. Shot at several consecutive offsets, these give
  # the cell pitch outright, so where in its cell the bar sits is measured
  # rather than eyeballed.
  caret-*)      tap "${STATE#caret-}" "$RIGHT" ;;
  # Caret at the end of the line: the only case iA's own stills ever show.
  end)          ydotool key "$END_K:1" "$END_K:0" ;;
  # Four cells selected from <offset>, caret at the selection's trailing edge.
  fwd-*)        tap "${STATE#fwd-}" "$RIGHT"; shifted 4 "$RIGHT" ;;
  # The same four cells selected, caret at the selection's *leading* edge — the
  # same offset as `caret-<N>`, which is the pair the ticket is about.
  back-*)       tap $(( ${STATE#back-} + 4 )) "$RIGHT"; shifted 4 "$LEFT" ;;
  *) echo "unknown state: $STATE" >&2; exit 2 ;;
esac

# The caret blinks, so one shot can catch it off. Take a burst across more than
# a blink period and keep the frame with the most accent in it; a state that
# never draws a bar keeps its first frame and reads as having none.
for i in 0 1 2 3 4 5 6 7; do
  grim -o "$STAGE" "$DIR/.$STATE.$i.png"
  sleep 0.09
done
focus "$HOME_MON"
python3 "$(dirname "$0")/pickbright.py" "$DIR/$STATE.png" "$DIR/.$STATE".*.png
rm -f "$DIR/.$STATE".*.png
echo "$DIR/$STATE.png"
