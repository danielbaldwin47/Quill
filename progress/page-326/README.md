# #326: narrow Page loss, reproduced on PR #325

The Page Gate remains red. Its narrow writing surface matches the approved
geometry, but the Parity critique prefers geometry that the design overruled.
The existing Mac captures cannot supply a matched default-size narrow opponent.
[#328](https://github.com/danielbaldwin47/Quill/issues/328), a capture child of #154,
blocks #326. No application geometry, judged state, opponent, critic or Gate rule
changes in this investigation.

## Reproduction

On 2026-09-08 UTC, branch `ticket-326` was created from PR #325 head
`d295ed941633d835a03e47da18ccdb305f1718ce`. The release binary's SHA-256 was
`53c63f81628a97e79aff929f31cd5add7b74f90f1a8d74c1a8863ae8b470ad37`.

`tools/gate shoot page narrow` initially refused because no active caret appeared
in three settled captures. That refusal supplied no judging evidence. A subsequent
`tools/gate shoot page` captured all three states successfully:

```text
gate shoot page: light -> target/gate/shoot/page/light-ours.png: the pixels round 8 judged (ours, clear); a judge would carry that verdict
gate shoot page: narrow -> target/gate/shoot/page/narrow-ours.png: the pixels round 8 judged (theirs, slight); a judge would carry that verdict
gate shoot page: short -> target/gate/shoot/page/short-ours.png: the pixels round 7 judged (ours, slight); a judge would carry that verdict
gate shoot page: shot 3 states (3 unchanged since their last round)
```

The narrow PNG SHA-256 is
`31eb76cf27bbc89b31746062e26fa49c52646feef90300d178edd3263a1031e8`;
its decoded RGB SHA-256 is
`254b9d42c09faceafd2adf9810aa18f31912cdc6e8c73a615453d8b86e109b03`.
Both equal round 8's committed failure. This independently reproduces PR #325;
the original report's identical capture from main `ffa30d5` was not repeated here.

## What changed between the winning and losing rounds

Run from the repository root:

```sh
node progress/page-326/measure.mjs
```

The output is committed as [measurements.json](measurements.json). The script
reads decoded RGB pixels; its line bands use dark glyph cores below 90 per channel,
so bounds describe ink rather than container edges. All numbers below are device
pixels at scale 2.

| Comparison or measurement | Result |
|---|---|
| Round 7 versus round 8, whole 1920 × 1800 image | 316,050 changed pixels |
| Round 8 versus fresh round 9, whole image | Zero changed pixels |
| Round 7 rectangle `[0,100,1920,1536]` versus round 8 `[0,164,1920,1536]` | Zero changed pixels |
| First heading ink top, round 7 → 8 | 168 → 232: +64 |
| First body line ink top, round 7 → 8 | 316 → 380: +64 |
| First body line ink left, both rounds | 181 |
| Opening paragraph row tops, round 8 | 380, 454, 528, 602, 676, 750: 74-pixel pitch |
| Same paragraph row tops, Parity | 391, 467, 543, 619, 695, 771: 76-pixel pitch |

The aligned comparison covers the full width, including both gutters, and all
shared visible prose through y=1699 in the current shot. It excludes the title
and stats bars and the old bottom content clipped by the new stats bar. Chrome
translated the visible writing surface by 64 pixels; the matching region shows
no changed wrapping, horizontal placement, glyphs or inter-row spacing.

## Geometry and ownership

`shots/oracle/states.json` gives `page/narrow` width 960 logical pixels, step 5,
Duo, scale 2 and chrome on. `typography::column` in
`quill-engine/src/typography.rs` holds seven-cell gutters when the 78-cell container
exceeds the window. Here it returns logical `left=0`, `right=960`, `side=90`,
`width=780`: a 180-device-pixel margin and 1560-pixel measure. The first body ink
at x=181 is consistent with that margin; the round 8 critic's approximately
1558-pixel ink extent is not itself the measure's boundary.

The design rows **Text container**, **What hangs**, **Text sizes**, **Line pitch**
and **Block spacing** own these choices. Native pitch 74 is within the row's
one-device-pixel tolerance of the measured Mac default 73; Parity's 76 is not
the target. Headings deliberately hang. **Page top** stays two pitches pending
#231; this investigation provides no new measurement that would change it.

The narrow critique prefers Parity's wider measure, 76-pixel pitch and heading
markers inside the column. Its preference therefore conflicts with approved
writing-surface decisions. This explains why adjusting Quill toward that critique
would be unjustified; it does not establish every unmeasured narrow Mac dimension.

## Missing evidence and continuation

`ref/ia/mac-native/NOTES.md` §§ The rig and State 11 establish a 1512-point-wide Mac
window at every recorded default-step state. Selection, markup and default-size
crops can establish individual wide-window facts; cropping one cannot reproduce
960-point reflow. The size sweep becomes window-limited at steps 8–13, and those
images are scrolled with unrecorded origins. ADR 0016 § Consequences explicitly
says which dimension gives under that constraint was not measured.

`docs/design.md` § Adding or changing a row requires a capture ticket and retaining
the Parity pair until it lands. Missing evidence for a state the Mac can display
does not meet ADR 0017's requirement that neither oracle holds the subject.

#328 requests a 960-point-wide, default-step Mono Mac capture, selection-derived
container and cell measurements, all six heading hangs, and a 1040-point control
above the full container width. It records full window and Editor bounds so a
later writing-surface crop can retain both horizontal edges and narrow reflow.
Mac chrome remains outside the comparison's design ownership. Absolute page-top
portability remains #231.

After #328 is measured, resume #326 with the supported crop and any measured
design decision, preserve narrow coverage, and run both Commit and Page Gates.
#326 closes only when every page state is ours.

## Gate evidence

Round 9 was recorded with the existing carry mechanism, without `--fresh` or a
new critic. Its build is the PR #325 head above. The full output is
[judge.log](judge.log); the round is [page-r9.json](../rounds/page-r9.json).

```text
gate judge page: light: ours (clear, carried from round 8)
gate judge page: narrow: theirs (slight, carried from round 8)
gate judge page: short: ours (slight, carried from round 7)
gate judge page: this Piece was won in round 7 against the Parity oracle and is lost now (docs/agents/gate.md: a Piece once won is never lost)
gate judge page: theirs, round 9
```

The judge exits 2. Historical rounds remain intact. The Commit Gate passes with
`RUST_TEST_THREADS=1 tools/gate check`, matching PR #325's serial test setting:

```text
gate check: pass
```
