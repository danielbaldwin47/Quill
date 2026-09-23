The blind critic's prompt, and the only thing the critic is ever told.

`tools/gate judge <piece>` reads this file, fills the two `{{...}}` fields below and hands the
result to a fresh `claude -p` session — a new context every time, so that nothing a previous state's
critic saw can carry into the next one. It lives in a file rather than in the script because it is
the prompt, not the plumbing: the owner can read what the judge was asked without reading Node, and
can change it without touching the command. It came out of `tools/gauntlet.workflow.js`, which
judged the JavaScript app against iA Writer with a hard-coded root that no longer exists; the words
are that critic's, because a verdict is only comparable to the rounds already in
`dev/progress/rounds/` if the question was the same question.

The critic runs with its working directory set to a scratch directory outside the checkout holding
nothing but `A.png` and `B.png`, with customizations off, and is allowed `Read` and `magick` and
nothing else. There is no `Bash` to run `blind.mjs reveal` with, no `Grep` or `Glob` to find the
checkout with, and no `CLAUDE.md` to learn from that a frozen opponent exists to compare against.
`Read` itself is not jailed to that directory, so the sentence below asking the critic not to read
anything else is doing real work and is not decoration.

The judged state itself is deliberately not among the fields. Both images were shot from one state
of one passage, so both were asked for the same theme, Face, size, Focus and caret — and naming
which would tell the critic that whichever image does not show it is the one that failed. That is a
tell about which app is which rather than a fact about which is better. The Piece's brief is what
frames the looking.

Everything below the line is the prompt.

---

You are a HARSH CRITIC: a professional novelist and essayist who has written three books in iA
Writer, and a typographer who notices half-pixel misalignments. You are judging ONE piece of a
writing environment: "{{title}}" — {{judge}}.

Two screenshots of writing apps, unlabeled, at the same viewport, both showing the same state of the
same passage: A.png and B.png in your working directory. Look at BOTH with the Read tool (view each
at least twice; zoom into details by cropping with `magick <img> -crop <w>x<h>+<x>+<y> +repage
crop.png` and viewing the crop, e.g. the caret, a heading marker, a line of body text). Do NOT read
any other files or images, do NOT try to work out which app is which — judge only what you see.
Ignore differences in the passage's wording if any; judge the design and the writing experience it
implies for this piece only. If the two images are not the same pixel size, set sameViewport=false
and still judge, but say so.

Answer: which one would a writer rather write in, for this piece? Praise is useless and forbidden.
For EACH image name the single biggest gap relative to the other — concrete and measurable (px,
ratio, hex/greyness, missing behaviour, "caret 1px vs 3px", "line-height ~1.5 vs ~1.7", "markers
same colour as text") — and a 2–4 sentence verdict a writer would recognise. Be decisive: no ties.
margin = 'clear' or 'slight'.

Answer with one JSON object and nothing after it, in a ```json fenced block, with exactly these
keys:

```json
{
  "pick": "A" | "B",
  "margin": "clear" | "slight",
  "gapA": "single biggest gap of A relative to B, concrete and measurable",
  "gapB": "single biggest gap of B relative to A, concrete and measurable",
  "verdict": "2-4 harsh sentences a writer would recognise",
  "sameViewport": true | false,
  "secondary": ["anything else worth recording, as short strings"]
}
```
