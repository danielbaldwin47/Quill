# Sources for Quill's Style check lists

`fillers.txt`, `redundancies.txt` and `cliches.txt` are Quill data. Every entry
came from one of the sources below, or was written for Quill. Each list file
groups its entries under a `# --- <source> ---` comment, so which entries came
from which source is readable in the file itself.

Only licences the FSF lists as compatible with the GNU GPL were taken from, and
only ones compatible with **GPL-3.0-or-later** specifically ([ADR
0003](../../docs/adr/0003-gpl-3-license.md)): the Expat/MIT licence, the
Modified BSD licence, and public-domain dedications. Nothing under CC BY-SA or
CC BY-NC was used, so no Wikipedia or Wiktionary list is in here — CC BY-SA 4.0
is one-way compatible with GPL **version 3 only**, which would cost Quill its
"or later".

The full findings, including the sources that were rejected and why, are in
[`docs/research/style-lists.md`](../../docs/research/style-lists.md).

## Sources adopted

| Source | Version / commit | Licence | Licence text here | Entries taken |
|---|---|---|---|---|
| npm `fillers` | 2.0.1 | MIT | `LICENSE-MIT-wooorm.txt` | 63 fillers |
| npm `hedges` | 2.0.1 | MIT | `LICENSE-MIT-wooorm.txt` | 41 fillers |
| npm `weasels` | 2.0.1 | MIT | `LICENSE-MIT-wooorm.txt` | 8 fillers |
| npm `too-wordy` | 0.3.6 | MIT | `LICENSE-MIT-duereg.txt` | 50 fillers |
| npm `no-cliches` | 0.3.6 | MIT | `LICENSE-MIT-duereg.txt` | 697 clichés |
| proselint | `dbed789`, 2026-06-22 | BSD-3-Clause | `LICENSE-BSD-3-Clause-proselint.txt` | 20 redundancies (RAS syndrome, Nordquist, Wallace) |
| proselint, `redundancy/after-the-deadline` | `dbed789`, 2026-06-22 | BSD-3-Clause | `LICENSE-BSD-3-Clause-proselint.txt` | 500 redundancies |
| plainlanguage.gov | `fd76947`, 2025-09-23 | Public domain / CC0 1.0 | `LICENSE-CC0-plainlanguage.txt` | 9 redundancies |
| Quill | — | GPL-3.0-or-later | root `LICENSE` | 97 fillers, 1 redundancy, 7 clichés |

## Attribution

The MIT and BSD-3-Clause licences require the copyright notice and the licence
text to travel with the data. Both are reproduced in this directory, and these
are the notices:

- **npm `fillers`, `hedges`, `weasels`** — `Copyright (c) 2014 Titus Wormer
  <tituswormer@gmail.com>`, MIT. <https://github.com/words>
- **npm `too-wordy`, `no-cliches`** — `Copyright (c) 2021 Matt Blair`, MIT.
  <https://github.com/duereg/too-wordy>, <https://github.com/duereg/no-cliches>
- **proselint** — `Copyright © 2014–2015, Jordan Suchow, Michael Pacer, and Lara
  A. Ross. All rights reserved.`, BSD-3-Clause. Its third clause forbids using
  the authors' names to endorse a derived product, which Quill does not do.
  <https://github.com/amperser/proselint>
- **plainlanguage.gov** — a work of the United States Government, in the public
  domain within the United States, and dedicated worldwide under CC0 1.0 by the
  Plain Language Action and Information Network and GSA. No attribution is
  required; it is given anyway. <https://www.plainlanguage.gov>
- **Quill's own entries** — written for Quill, GPL-3.0-or-later with the rest of
  the repository. The three phrases per list that iA Writer publishes as
  examples of its own Style check (`basically`, `pretty much`, `sort of`;
  `basic fundamentals`, `combine together`, `fall down`; `against all odds`,
  `brass tacks`, `long and short of it`) are quoted from
  <https://ia.net/writer/support/editor/style-check> as the floor the spec's
  fixture passage sets, not copied as a list. Two more — `get down to brass
  tacks` and `past history` — are what the running app struck in that passage
  where the published examples did not say it would (#354), and
  `basic fundamentals` is marked `[basic] fundamentals` for the same reason:
  the page bolds the noun and the app strikes the adjective.

## The one provenance caveat

`redundancies.txt`'s largest block — 500 of its 530 entries — is proselint's
`checks/redundancy/after-the-deadline`, a corpus proselint ships and licenses
under BSD-3-Clause but which originates in Automattic's **After the Deadline**,
whose server is distributed under the GNU GPL **version 2** with no "or later"
statement in the repository. Quill takes the data under proselint's BSD-3-Clause
grant, and re-curates it (bracketed form, subset filter, inflections, its own
ordering) rather than copying a compilation. The block is delimited by its own
`# ---` comment so it can be removed in one edit if the owner would rather not
carry the question; `docs/research/style-lists.md` § Provenance caveats sets out
the reasoning and what removing it would cost.
