#!/usr/bin/env bash
# The jump, three ways, on one sheet.
#
# The same crop window on all three — cell 10 of `jump.md`, whose boundary is at
# x=912 — at 3x nearest-neighbour, so a device pixel is a block and the bar's
# column can be counted rather than judged. Top row is the free caret at offset
# 10, bottom row a selection opening at that same offset: the distance between
# the two rows is the jump.
#
#     sheet.sh <before-jump-caret.png> <before-jump-select.png> \
#              <oracle-jump-caret.png> <oracle-jump-select.png>
set -euo pipefail
cd "$(dirname "$0")/../../.."
OUT=dev/shots/caret/ia
W=160x88+866+138
Z=300%

panel() { magick "$1" -crop "$W" +repage -filter point -resize "$Z" -bordercolor '#cccccc' -border 1 "$2"; }

panel "$3" "$OUT/.o-c.png"; panel "$4" "$OUT/.o-s.png"
panel "$1" "$OUT/.b-c.png"; panel "$2" "$OUT/.b-s.png"
panel "$OUT/ours-jump-caret.png" "$OUT/.a-c.png"
panel "$OUT/ours-jump-select.png" "$OUT/.a-s.png"

magick montage \
  -label 'Parity oracle' "$OUT/.o-c.png" \
  -label 'ours before' "$OUT/.b-c.png" \
  -label 'ours after' "$OUT/.a-c.png" \
  -label ' ' "$OUT/.o-s.png" \
  -label ' ' "$OUT/.b-s.png" \
  -label ' ' "$OUT/.a-s.png" \
  -tile 3x2 -geometry +8+8 -background white -pointsize 15 "$OUT/sheet-jump.png"
rm -f "$OUT"/.[oba]-[cs].png
echo "$OUT/sheet-jump.png"
