#!/usr/bin/env bash
# What iA Writer's selection and caret do once the window is no longer focused.
# The state is set with focus on the stage, then focus goes back to the owner's
# panel and the shot is taken after the app has had time to repaint.
set -euo pipefail
D="$(dirname "$0")"
OUT="${IA_OUT:-${TMPDIR:-/tmp}/ia-shots}"
"$D/drive.sh" back-10 >/dev/null
sleep 1.0
grim -o "${IA_STAGE:-HEADLESS-184}" "$OUT/unfocused.png"
echo "$OUT/unfocused.png"
