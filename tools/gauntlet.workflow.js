export const meta = {
  name: 'quill-gauntlet',
  description: 'Per-piece builder → blind critic → recorder loop until the critic picks ours blind',
  phases: [
    { title: 'Build', detail: 'one builder per open piece, fed the last critic gap' },
    { title: 'Judge', detail: 'fresh critic, unlabeled A/B at the same viewport' },
    { title: 'Record', detail: 'reveal + write progress/rounds/<piece>-r<N>.json', model: 'haiku' },
  ],
}
const ROOT = '/home/diggle/Work/wisprflowcopy'
const MAX_ROUNDS = (args && args.maxRounds) || 4
const START_ROUND = (args && args.startRound) || 1
const PIECES = [
  { id: 'page', title: 'The page', judge: 'the page itself: paper colour, margins, the measure (line length), vertical placement of text in the window, scrolling feel implied by spacing, how the empty space frames the text; overall calm', build: 'paper, margins, measure (iA: 64 chars default, adapts to window), vertical rhythm of the page, the empty state / placeholder, scrollbars, what the eye lands on when a document opens', states: 'light editor with the ALICE-FALL passage (appstore-mac-01 crop) and the RABBIT-HOLE full window (ianet-mac-light-window-focus-syntax-italic.webp)' },
  { id: 'type', title: 'The type', judge: 'the typography only: typeface rendering, size, line height, rhythm, weight and colour of the ink, the spacing of blank lines, how italic/bold sit; would a writer rather read and type in this type', build: 'font faces (Duo default; Quattro/Mono switchable), the default size, line-height (iA measured ≈1.71×), letterforms rendering crisp, blank-line pitch, hanging punctuation/markers if metric-safe, size steps for Mod+/-', states: 'light ALICE-FALL (appstore-mac-01 crop) and dark ALICE-FALL (appstore-mac-08 or 03 crop, plain text without syntax colours)' },
  { id: 'caret', title: 'Cursor & caret', judge: 'the caret and selection only: caret colour, width, height relative to the line, how it sits against the glyphs, blink/motion implied, selection colour; does the caret feel like a precise, alive instrument', build: 'caret colour (iA #00b5ff), width ≈0.15 em, height = full line pitch, smooth movement between positions, blink that pauses while typing, selection highlight colour and shape, caret when unfocused', states: 'light ALICE-FALL with caret after "earth" (appstore-mac-01 crop) and dark ALICE-FALL with "so-called" selected (appstore-mac-04 crop)' },
  { id: 'focus', title: 'Focus & typewriter', judge: 'focus mode and typewriter scrolling: how the non-focused text is dimmed (colour, amount), sentence vs paragraph scope, where the active line rests in the window, whether the eye is drawn to exactly the right place', build: 'sentence and paragraph focus (iA dims to #cccccc light / #707070 dark), typewriter line position, smoothness, transitions when the focus moves, combined with dark mode; keyboard shortcut Mod+D', states: 'light sentence focus ALICE-FALL (appstore-mac-06 crop) and dark paragraph/sentence focus + typewriter (ianet-mac-dark-focus-sentence-support.webp / ianet-mac-dark-typewriter-support.webp)' },
  { id: 'theme', title: 'Dark & light', judge: 'the two palettes: paper and ink colours, contrast, how dimmed and marker greys read, how the chrome sits on each ground, whether dark mode feels designed rather than inverted', build: 'light (iA measured bg #f9f9f9 / ink #1c1c1c) and dark (#1a1a1a / #cccccc) palettes, marker/dim greys, selection and caret on each, system-follow, instant switch (Ctrl+Mod+N style shortcut), no flash on load', states: 'light ALICE-FALL (appstore-mac-01 crop) and dark ALICE-FALL (appstore-mac-08 crop)' },
  { id: 'markup', title: 'Markup rendering', judge: 'how Markdown is shown while writing: heading markers, emphasis, lists, blockquotes, code, links — are markers quiet, does the text stay readable, do headings and structure read at a glance without the text jumping', build: 'headings (bold, same size, # markers hanging into the margin if metric-safe: iA hangs "# " 2 cells left), emphasis markers greyed, lists with hanging bullets, blockquote marks, code spans/blocks, links with grey URLs, task lists, horizontal rules; also an OPTIONAL syntax-highlight (parts of speech) is NOT required', states: 'light RABBIT-HOLE window with headings/italic (ianet-mac-light-window-focus-syntax-italic.webp or ianet-mac-light-syntax-highlight.png) and dark heading shot (appstore-mac-08 crop)' },
  { id: 'chrome', title: 'Chrome & menus', judge: 'the chrome: what UI is visible around the text, the stats/word-count bar, title, menus/palette, how it recedes while writing; is there exactly as much interface as a writer needs and no more', build: 'top bar (title, minimal controls), bottom stats bar (words, characters, reading time like iA), a Focus/typewriter menu, command palette polish, hide-while-typing behaviour, hover reveal; compare against ianet-mac-light-stats-bar.webp, ianet-mac-light-focus-dropdown-menu.webp, the full window shot', states: 'light full window with stats bar (ianet-mac-light-stats-bar.webp) and a menu/palette open (ianet-mac-light-focus-dropdown-menu.webp)' },
  { id: 'files', title: 'File handling', judge: 'file handling UI: the library/sidebar, file list, how documents are named and found, open/save affordances, how it looks next to the editor; would a writer trust it with their manuscripts', build: 'a Library sidebar (folder picker via File System Access API, recent files, search, new file), autosave indicator, open/save/save-as, drag & drop, .md/.txt, title from first heading; compare against appstore-mac-05-light-library.png and ianet-mac-light-library-organizer-filelist.webp', states: 'light editor with library sidebar open (appstore-mac-05 crop) and a second view with the file list' },
  { id: 'latency', title: 'Latency', judge: 'measured numbers and methodology: keystroke-to-paint p50/p99, startup to editor-ready, with a large document; is the measurement honest and does it beat the bar (Sublime 32.5 ms keyboard-to-photon; ≤5 ms internal input→paint; iA publishes no numbers)', build: 'harden tools/latency.mjs (count every keystroke, FCP in headless, real 10k-word document, p50/p95/p99, also measure via CDP tracing frame presentation), a bin/quill launcher (chromium --app) with cold-start measurement, profile and remove any per-keystroke cost in core.js/render, write progress/latency.json with keys {keystroke_p50, keystroke_p99, startup_ready, startup_note, bar_keystroke, bar_note, method, at} and progress/latency-report.md', states: 'not a screenshot piece — the critic reads progress/latency.json and progress/latency-report.md' },
]
const BUILD_SCHEMA = { type: 'object', required: ['oursShot', 'theirsShot', 'note'], properties: { oursShot: { type: 'string', description: 'path to our screenshot used for pairing (repo-relative)' }, theirsShot: { type: 'string', description: 'path to the cropped iA reference used (repo-relative)' }, refSource: { type: 'string' }, note: { type: 'string', description: '1-3 sentences: what changed this round' }, paired: { type: 'boolean' } } }
const JUDGE_SCHEMA = { type: 'object', required: ['pick', 'margin', 'gapA', 'gapB', 'verdict'], properties: { pick: { type: 'string', enum: ['A', 'B'] }, margin: { type: 'string', enum: ['clear', 'slight'] }, gapA: { type: 'string', description: 'single biggest gap of A relative to B, concrete and measurable' }, gapB: { type: 'string', description: 'single biggest gap of B relative to A, concrete and measurable' }, verdict: { type: 'string', description: '2-4 harsh sentences a writer would recognise' }, sameViewport: { type: 'boolean' }, secondary: { type: 'array', items: { type: 'string' } } } }
const LAT_JUDGE_SCHEMA = { type: 'object', required: ['pick', 'gapOurs', 'verdict'], properties: { pick: { type: 'string', enum: ['ours', 'theirs'] }, gapOurs: { type: 'string' }, verdict: { type: 'string' }, methodologyHoles: { type: 'array', items: { type: 'string' } } } }
const RECORD_SCHEMA = { type: 'object', required: ['winner', 'file'], properties: { winner: { type: 'string', enum: ['ours', 'theirs', 'tie'] }, file: { type: 'string' }, gapOurs: { type: 'string' } } }

function builderPrompt(p, round, feedback) {
  return `You are the BUILDER for the piece "${p.title}" (id: ${p.id}) of Quill, a long-form writing environment that must beat iA Writer. Round ${round}. Work in ${ROOT} (git repo; the static server is already running at http://localhost:4173/ — if it is not, start it with \`node tools/serve.mjs 4173 &\`).

FIRST read, in this order: BRIEF.md, NOTES.md, app/js/core.js, then your own files, then ref/ia/REFERENCE.md (the spec sheet, quotes and the screenshot table with transcribed passages). Look at the relevant iA reference screenshots in ref/ia/shots/ with the Read tool — you must compare against the real thing, not a description.

Your piece covers: ${p.build}.
The critic will judge: ${p.judge}.
Reference states to reproduce for pairing: ${p.states}.

${feedback ? `PRIOR CRITIC VERDICT (blind; ours lost). Fix the biggest gap FIRST, then the rest:\n${feedback}\n` : 'This is the first round: bring this piece from "works" to "the best writing tool that exists". Go beyond copying iA — match its measured values where they are right, and improve where a writer would feel it.'}

Rules:
- Edit only the files you own (BRIEF.md table). Minimal, noted changes elsewhere go in NOTES.md under "## ${p.id}". Never break glyph alignment between #input and #mirror; never change glyph advance widths per token; never add per-keystroke heavy work. Keep \`node --check\` passing on any JS you touch, and check both themes and all three fonts still render (a quick shoot in dark + light).
- Other builders are editing other files concurrently; the served app is shared. If a screenshot looks broken in an area outside your piece, retry once, and don't "fix" others' files.
- Verify visually: \`node tools/shoot.mjs\` (see BRIEF.md flags) at EXACTLY the pixel size of the reference crop, with the same transcribed passage (write it to a file under shots/${p.id}/ e.g. shots/${p.id}/passage.md), same theme/mode/caret. View both images with the Read tool and compare like a typographer. Iterate until you honestly believe a demanding writer would pick ours blind. Be ruthless with yourself; the critic is.
- Then produce the pair for the critic: crop the reference to its app region with tools/crop.mjs (record the crop box), render ours to shots/${p.id}/r${round}-ours.png, save the crop to shots/${p.id}/r${round}-theirs.png, and run \`node tools/blind.mjs pair ${p.id} shots/${p.id}/r${round}-ours.png shots/${p.id}/r${round}-theirs.png\`. NEVER run \`blind.mjs reveal\`.
- Finally commit your changes: \`git add app tools shots/${p.id} NOTES.md && git commit -m "${p.id}: round ${round} — <what changed>"\` (if the commit races with another builder's, retry once; if the index is locked, wait 2s and retry).
${p.id === 'latency' ? `\nLATENCY SPECIFICS: this piece is judged on numbers, not screenshots. Produce progress/latency.json and progress/latency-report.md (methods, environment: headless Chromium 151 on Linux, CPU, document size, N, how frames were detected, caveats). Also try a headed measurement through the bin/quill launcher if a display is available (check $WAYLAND_DISPLAY / $DISPLAY; if none, say so). For the pair, set oursShot to progress/latency.json and theirsShot to ref/ia/REFERENCE.md and skip blind pairing (paired:false). Do not delete previous shots.` : ''}
Return JSON per the schema: oursShot, theirsShot (repo-relative paths), refSource (which ref image + crop box), note (what changed), paired (true if blind.mjs pair ran).`
}
function criticPrompt(p, round) {
  return `You are a HARSH CRITIC: a professional novelist and essayist who has written three books in iA Writer, and a typographer who notices half-pixel misalignments. You are judging ONE piece of a writing environment: "${p.title}" — ${p.judge}.

Two screenshots of writing apps, unlabeled, at the same viewport: ${ROOT}/shots/blind/${p.id}/A.png and ${ROOT}/shots/blind/${p.id}/B.png. Look at BOTH with the Read tool (view each at least twice; zoom into details by cropping with \`node tools/crop.mjs <img> /home/diggle/.claude/jobs/e9d93e91/tmp/crop-${p.id}-r${round}-<n>.png x y w h 2\` and viewing the crop, e.g. the caret, a heading marker, a line of body text). Do NOT read any other files or images, do NOT run \`blind.mjs reveal\`, do NOT try to work out which app is which — judge only what you see. Ignore differences in the passage's wording if any; judge the design and the writing experience it implies for this piece only. If the two images are not the same pixel size, set sameViewport=false and still judge, but say so.

Answer: which one would a writer rather write in, for this piece? Praise is useless and forbidden. For EACH image name the single biggest gap relative to the other — concrete and measurable (px, ratio, hex/greyness, missing behaviour, "caret 1px vs 3px", "line-height ~1.5 vs ~1.7", "markers same colour as text") — and a 2–4 sentence verdict a writer would recognise. Be decisive: no ties. margin = 'clear' or 'slight'.`
}
function latencyCriticPrompt(p) {
  return `You are a HARSH performance reviewer for a writing app. Read ${ROOT}/progress/latency.json and ${ROOT}/progress/latency-report.md, and the "Performance" section of ${ROOT}/ref/ia/REFERENCE.md (the bar: iA Writer publishes no numbers; best native editors ≈32.5 ms keyboard-to-photon (Sublime, macOS), ≤5 ms internal input→paint; Dan Luu's 2 ms perception threshold). Also read tools/latency.mjs and skim app/js/core.js for per-keystroke cost. Decide: do OUR measured numbers beat that bar with HONEST methodology (large document, every keystroke counted, paint actually detected, p99 not just p50, startup measured to editor-ready and ideally cold launch of the real launcher)? Poke holes: what is unmeasured, what is optimistic, what would a sceptic say? pick='ours' only if the numbers beat the bar AND the methodology holds up; otherwise 'theirs'. gapOurs = the single biggest hole or shortfall to fix next. verdict = 2–4 blunt sentences.`
}
function recorderPrompt(p, round, build, judge) {
  return `Mechanical bookkeeping in ${ROOT}. Piece "${p.id}", round ${round}.
1. ${p.id === 'latency' ? 'This piece is not blind: winner = ' + (judge.pick === 'ours' ? 'ours' : 'theirs') + '.' : 'Run `node tools/blind.mjs reveal ' + p.id + '` — it prints JSON with "ours": "A" or "B". The critic picked ' + judge.pick + ' (margin ' + judge.margin + '). winner = "ours" if the critic\'s pick equals the revealed letter, else "theirs".'}
2. Write ${ROOT}/progress/rounds/${p.id}-r${round}.json with EXACTLY these keys: piece="${p.id}", round=${round}, winner, margin=${JSON.stringify(judge.margin || 'clear')}, gap=<the critic's gap for OUR image (gapA if ours is A, gapB if ours is B${p.id === 'latency' ? '; for latency use gapOurs' : ''})>, gapTheirs=<the other one>, verdict=${JSON.stringify(judge.verdict)}, oursShot=${JSON.stringify(build.oursShot)}, theirsShot=${JSON.stringify(build.theirsShot)}, refSource=${JSON.stringify(build.refSource || '')}, builderNote=${JSON.stringify(build.note)}, secondary=${JSON.stringify(judge.secondary || judge.methodologyHoles || [])}, at=<current ISO timestamp from \`date -Iseconds\`>.
The critic's gaps: gapA=${JSON.stringify(judge.gapA || '')} gapB=${JSON.stringify(judge.gapB || '')} gapOurs=${JSON.stringify(judge.gapOurs || '')}.
Use a heredoc or python to write valid JSON. Then \`git add progress/rounds && git commit -m "${p.id}: round ${round} verdict"\` (retry once on lock). Return {winner, file, gapOurs}.`
}

const won = new Set((args && args.alreadyWon) || [])
const feedback = Object.assign({}, (args && args.feedback) || {})
const history = []
for (let round = START_ROUND; round < START_ROUND + MAX_ROUNDS; round++) {
  const open = PIECES.filter(p => !won.has(p.id))
  if (!open.length) { log('All pieces picked blind.'); break }
  log(`Round ${round}: ${open.length} open piece(s): ${open.map(p => p.id).join(', ')}`)
  const results = await pipeline(open,
    p => agent(builderPrompt(p, round, feedback[p.id]), { label: `build:${p.id} r${round}`, phase: 'Build', schema: BUILD_SCHEMA, model: 'opus' }),
    async (build, p) => {
      if (!build) return null
      const judge = p.id === 'latency'
        ? await agent(latencyCriticPrompt(p), { label: `judge:${p.id} r${round}`, phase: 'Judge', schema: LAT_JUDGE_SCHEMA, effort: 'high', model: 'opus' })
        : await agent(criticPrompt(p, round), { label: `judge:${p.id} r${round}`, phase: 'Judge', schema: JUDGE_SCHEMA, effort: 'high', model: 'opus' })
      return judge ? { build, judge } : null
    },
    async (r, p) => {
      if (!r) return null
      const rec = await agent(recorderPrompt(p, round, r.build, r.judge), { label: `record:${p.id} r${round}`, phase: 'Record', schema: RECORD_SCHEMA, model: 'haiku', effort: 'low' })
      return rec ? { piece: p.id, round, ...rec, judge: r.judge, build: r.build } : null
    })
  for (const r of results.filter(Boolean)) {
    history.push({ piece: r.piece, round: r.round, winner: r.winner, gap: r.gapOurs })
    if (r.winner === 'ours') { won.add(r.piece); delete feedback[r.piece] }
    else feedback[r.piece] = `Round ${r.round} verdict: ${r.judge.verdict}\nBiggest gap of OURS: ${r.gapOurs || r.judge.gapOurs || ''}\nSecondary: ${(r.judge.secondary || r.judge.methodologyHoles || []).join('; ')}`
  }
  log(`Round ${round} done — picked blind so far: ${[...won].join(', ') || 'none'}`)
}
return { won: [...won], feedback, history }