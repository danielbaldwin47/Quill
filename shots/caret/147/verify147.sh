#!/usr/bin/env bash
# THROWAWAY (#147): the rig proof, in differing pixels.
#
# Acceptance asks that shape 1's shots be the ones the Piece is judged on
# today. `compare -metric AE` counts the pixels that differ, so this says it as
# a number: shape 1 against round 6 (must be 0 — the same build, the same
# stage), shape 1 against the frozen oracle (the margin the Piece already
# wins at), and every other shape against shape 1 (what that shape moves).
set -u
D=shots/caret/147
ae() { printf '%-52s %s\n' "$1" "$(compare -metric AE "$2" "$3" null: 2>&1 | cut -d' ' -f1)"; }

echo "-- shape 1 against the round the Piece stands on (expect 0) --"
for s in caret selection unfocused; do
  ae "s1-$s vs r6-$s-ours" "$D/s1-$s.png" "shots/caret/r6-$s-ours.png"
done

echo
echo "-- shape 1 against the unpatched binary (expect 0: the switch is inert) --"
for s in caret selection unfocused jump-caret jump-select; do
  ae "s1-$s vs s1base-$s" "$D/s1-$s.png" "$D/s1base-$s.png"
done

echo
echo "-- ours against the frozen Parity oracle --"
for s in caret selection unfocused; do
  ae "s1-$s vs oracle/$s" "$D/s1-$s.png" "shots/oracle/caret/$s.png"
done

echo
echo "-- each shape against shape 1, per judged state --"
for n in 2 3 4 5; do
  for s in caret selection unfocused; do
    ae "s$n-$s vs s1-$s" "$D/s$n-$s.png" "$D/s1-$s.png"
  done
done
