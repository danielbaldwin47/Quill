#!/usr/bin/env bash
# Send raw key events to whatever iA Writer window is on the stage.
#
#     keys.sh 29:1 44:1 44:0 29:0     # ctrl+z
set -euo pipefail
export YDOTOOL_SOCKET=/run/user/1000/.ydotool_socket
hyprctl repl 'return hl.dispatch(hl.dsp.focus{monitor=hl.get_monitor("HEADLESS-184")})' >/dev/null
sleep 0.4
ydotool key "$@"
sleep 0.3
hyprctl repl 'return hl.dispatch(hl.dsp.focus{monitor=hl.get_monitor("DP-3")})' >/dev/null
