# A Template starts from iA's look and is then Quill's own

A built-in Template takes every value the Design oracle has measured for the iA template of the
same name, once, in one **baseline pass**; after that pass a Template is Quill's own typography,
and a later iA measurement informs it rather than binding it. Preview and Templates stay outside
[ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md)'s reach. Decided 2026-09-10 at
[#333](https://github.com/danielbaldwin47/Quill/issues/333), the grilling that gathered the seven
decisions the 2026-09-09 Mac captures could not take.

## Context

The five Templates are what the Preview and Export render; the Editor never wears one
([ADR 0005](0005-native-templates.md)). Modern and Classic were written from the Mac captures and
measure as ports of iA's. The three Manuscripts were written from the Editor's own ladder and their
files say so ("the Editor's own typography with the Markup removed"). Nothing said which of the two
a Template *is*, and the captures under #261 found the two families disagreeing with iA in opposite
directions: the Manuscript paragraph gap short of iA's by half a pitch, the Web preview's scale
fixed where iA's follows the editor's text size. Each was argued one ticket at a time, and one of
them (the fixed scale) had been given a reason that the capture then showed to be false.

ADR 0015 binds the writing surface to the oracle and stops there: "Chrome, the Library, Preview,
file handling and every setting's *switch* stay Quill's own". A rule for Templates has to say how
far iA's measurements reach into a thing that ADR left to Quill.

## Decision

**Baseline first.** Every value the oracle has measured for a Template at the time of the pass is
taken as it was measured. What the 2026-09-09 and 2026-09-10 captures settle for the Manuscript
family:

- the paragraph gap is one full pitch of air, the Editor's own blank line — `paragraph_spacing`,
  `space_before_heading` and `space_after_heading` equal to `line_height`, in ems as before;
- the paper is the shared Web paper Modern and Classic already render on, `#101010` dark and
  `#fcfcfc` light, in place of the Editor's ground.

Classic's own baseline (its paragraph gap, and whether iA's Classic indents a first line) waits on a
capture, and so does the em that would say which of `base` and `line_height` carries the 2 % pitch
difference between Classic and Manuscript. The pass lands in two tickets under the Templates parent
rather than one held on the owner's Mac.

**Then Quill's own.** After the pass a Template answers to its own file: a value stands with the
reason its comment gives, and a later iA measurement is evidence for a ticket, never a verdict.
The reach of ADR 0015 does not widen.

**The Manuscript family derives from the Editor ladder**, not from iA's Manuscript template: base,
pitch and measure are the Editor's default step converted once, so its rhythm is the Editor's and
the one-pitch gap is that ladder's blank line. That the gap also matches iA's two pitches is why
no capture is needed for it.

**The Web preview scales by `[preview] zoom` alone.** iA's follows the editor's text size; Quill's
Preview is a rendering of what Export produces, so its sizes are the Template's absolute points, and
the pane already has one zoom over its Web and PDF modes. A second multiplier would give the pane
two size controls, and a preview that changed size with the editor could not show what prints.

## Considered options

**A Template is a port, and a difference is a defect until closed.** Widens ADR 0015 to Preview,
which #263 put outside it for a reason: iA's preview is that app's design, not the port's, and the
Gate judges Preview by assertion, not against an opponent (ADR 0017).

**A Template is Quill's own from the start, no baseline.** What the files said before this ADR, and
what let the Manuscript rhythm and the Preview zoom drift from iA in opposite directions with no
rule to hold either. The owner wants iA's feel as the floor, then a design of Quill's own on top.

**Keep the Manuscript paper as the Editor's ground**, on the principle that a Manuscript is the
page being typed on. The principle was never a departure from anything, because there was no
baseline to depart from; it may return as one, with its reason in the file.

**A `pitches` unit for the spacing fields.** Spares the value `1.711` being written five times,
at the cost of an engine change for a number that moves only when the ladder does. The files
already derive every value from the ladder in comments; one more comment does it.

## Consequences

- The three Manuscript files change their spacing and paper, and the `//!` block in
  `quill-engine/src/template.rs` cites this ADR for the fixed-scale rule instead of arguing it.
  The judged Preview states are asserted on Modern, so no oracle goes stale.
- Two capture tickets under #154: Classic's indent and an em read from ink (for Classic and
  Manuscript both), which feed the Classic half of the pass.
- The narrow-window question (#333 decision 6, where iA's Editor type shrinks with the window and
  Quill's measure narrows) is the writing surface's, inside ADR 0015, and goes to a capture on the
  original rig rather than a decision here.
- A future spec wanting a Template to follow a new iA measurement files a ticket under the
  Templates parent and argues it there.
