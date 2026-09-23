# Ours, against iA Writer for Mac

Four shots of the Editor after [ADR 0014](../../../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md)
took the bars off the selection's ends. Shot on the Gate's own stage by
`dev/shots/caret/ia/shoot.mjs`, so they are the same 3200×2000 headless output at scale 2 and the same
`grim -T` capture the judged states use.

| File | State | Measured |
|---|---|---|
| `ours-caret.png` | `caret`, verbatim | one bar `#00b5ff`, 6 × 72 px at x 1896–1901 — unmoved by this change |
| `ours-selection.png` | `selection`, verbatim | fill `#c2eafa`, x 948–1403, one band 72 px tall; **no accent pixel in the frame** |
| `ours-unfocused.png` | `unfocused`, verbatim | the ghost caret, no selection |
| `ours-idle-band.png` | `selection` with the window deactivated | idle band `#e4e4e4`; **no accent pixel** — the idle swap is the fill's alone now |

The "no accent pixel in the frame" reading is the one the owner's captures of iA Writer for Mac are
measured by (`dev/ref/ia/shots/owner-mac-03` and `-04`, catalogued in `dev/ref/ia/REFERENCE.md` § 1.1): a
caret bar is the brightest thing in any frame that holds one, so a frame with none holds neither a
bar nor a caret. Ours and iA's now answer it the same way.

`dev/shots/caret/ia/` is the other half of this: the Wine rig that drives iA Writer for Windows, and the
shots ADR 0013 was measured from. Those are left as 0013 shot them.
