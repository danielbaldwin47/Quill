#!/usr/bin/env python3
"""Build Quill's six Faces from the iA Writer variable fonts.

    python3 tools/fontbuild.py

Reads the six `*V*.ttf` variable files from `dev/ref/ia/fonts` and writes `fonts/`
at the repo root: six renamed TTFs and the `OFL.txt` that carries their
attribution. Needs `python-fonttools`, which is a tool-time dependency only —
the fonts are committed, so neither the workspace build nor the package runs
this.

Why a rename at all: the OFL FAQ counts any table edit as a Modified Version,
and OFL 1.1 section 3 forbids one from carrying the Reserved Font Name
"iA Writer", which lives in the file's `name` table. So the Quill names are
written into the files rather than pinned on at match time
(`docs/adr/0007-quill-faces-renamed-and-private.md`).

Each Italic becomes a family of its own — Quill Duo Italic, not the Italic
style of Quill Duo. That is what the fonts already are: every Italic file
declares subfamily "Regular" and leaves the OS/2 italic bit clear, so
fontconfig reads it as roman whatever we call it, and the Editor asks for the
Face it wants by family name.

Output is a pure function of the input: running this twice produces identical
bytes. It is not a passthrough of the tables it leaves alone, though — fontTools
recompiles every table this script loads, so a Face with nothing to pin still
comes out a little smaller than iA compiled it, `gvar` packed tighter. Nothing
checks the committed `fonts/` against a fresh build; a change here is rebuilt by
hand and the diff read.
"""

import sys
from pathlib import Path

from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "dev" / "ref" / "ia" / "fonts"
FONTS = ROOT / "fonts"

# The prefix ADR 0007 keeps in one place: renaming every Face is an edit here
# and a rebuild.
PREFIX = "Quill"

# (directory under dev/ref/ia/fonts, source file, name of the Face, italic?)
FACES = [
    ("Duo", "iAWriterDuoV.ttf", "Duo", False),
    ("Duo", "iAWriterDuoV-Italic.ttf", "Duo", True),
    ("Quattro", "iAWriterQuattroV.ttf", "Quattro", False),
    ("Quattro", "iAWriterQuattroV-Italic.ttf", "Quattro", True),
    ("Mono", "iAWriterMonoV.ttf", "Mono", False),
    ("Mono", "iAWriterMonoV-Italic.ttf", "Mono", True),
]

# The four named instances every file carries, in `fvar` order (wght 400, 450,
# 650, 700). The Italics name theirs "Italic", "Text Italic" and so on; inside
# a family that is entirely italic those styles say nothing, so all six files
# end up with the same four style names.
STYLES = ["Regular", "Text", "Semibold", "Bold"]

# The `name` IDs this script owns. Everything else — the licence text, the
# designers, the vendor URLs, the character-variant names — is iA's and stays.
COPYRIGHT, FAMILY, SUBFAMILY, UNIQUE_ID, FULL, POSTSCRIPT = 0, 1, 2, 3, 4, 6
# Typographic family and subfamily. A family whose only style is Regular has
# nothing to say in them that IDs 1 and 2 do not, and leaving them would give
# fontconfig a second family name to match on.
TYPOGRAPHIC = [16, 17]

# Where the new records for the instance style names start. Above every ID the
# source files use — their instance PostScript names run to 503 — rather than
# next to them, because the `STAT` tables in iA's Italics already point at IDs
# that do not exist (281 to 284), and a new record landing on one of those would
# hang an instance's style name off a `STAT` axis value.
INSTANCE_NAMES_FROM = 600

# Both source copyrights, kept, and one line saying what was changed. No date:
# the output has to be reproducible.
NOTICE = (
    "Copyright 2017 IBM Corp. All rights reserved.\n"
    "Copyright 2018 Information Architects Inc. All rights reserved.\n"
    "Modified for Quill under SIL OFL 1.1 section 3: renamed, the Quattro "
    "Italic word space regularised, and every advance pinned across the weight "
    "axis. See OFL.txt."
)

# The Quattro Italic word space is 600 units where its Roman's is 450, so a run
# of italic text would sit a third of a space wider than the same words upright.
# `tools/fontgrid.py` made this patch for the web app, where it kept the mirror
# under the textarea; here it keeps an italic word the width the writer sees
# before and after they style it. The width is read from the Roman rather than
# written down, because matching the Roman is the whole point.
SPACE_GLYPH = "space"

# Five of the six Faces vary some advance on `wght`, so that a run of text
# changes width the moment it is set bold: Quattro Italic moves `t` and `f`,
# which is every emphasis inside a heading, and the rest move only marks and
# punctuation the judged passage happens not to use. A `gvar` tuple's deltas
# run point by point and end with four phantom points — left side bearing,
# advance, top side bearing, advance height — so the advance is the second
# from the end of the four, and the third from the end of the tuple.
ADVANCE_PHANTOM_INDEX = -3

# `fonts/OFL.txt` is this preamble followed by iA's LICENSE.md verbatim.
LICENCE_PREAMBLE = """\
Quill Duo, Quill Quattro and Quill Mono
=======================================

Quill's Faces are Modified Versions of the iA Writer typefaces, built from the
variable fonts under `dev/ref/ia/fonts` by `tools/fontbuild.py`. As section 3 of
the licence below requires, no Reserved Font Name appears in them.

What was modified:

  * family, subfamily, full, PostScript and named-instance names rewritten with
    the `Quill` prefix, each Italic a family of its own;
  * the Quattro Italic word space narrowed from 600 to 450 units to match its
    Roman, with the glyph's `gvar` entry dropped so it holds at every weight;
  * every glyph's advance frozen across the weight axis, by zeroing the
    advance delta of each `gvar` tuple that varies on `wght`, so that a run of
    text keeps its width when it is set bold;
  * the digital signature (`DSIG`) dropped, no longer being valid;
  * the typographic family and subfamily names (`name` IDs 16 and 17) dropped.

Outlines, kerning, hinting and the variation axes are iA's, unchanged apart from
the two advance edits named above. The originals are in this repository under
`dev/ref/ia/fonts`.

The licence below is iA's own, copied verbatim from `dev/ref/ia/fonts/*/LICENSE.md`.


"""


def rename(font, family, postscript_family):
    """Writes the Quill names into `font`'s `name` table and `fvar` instances.

    Every record of a given ID is rewritten, on every platform the file carries
    it, so no mac-encoded leftover keeps the old name.
    """
    name = font["name"]
    for record in name.names:
        if record.nameID == COPYRIGHT:
            record.string = NOTICE
        elif record.nameID in (FAMILY, FULL):
            record.string = family
        elif record.nameID == SUBFAMILY:
            record.string = STYLES[0]
        elif record.nameID == POSTSCRIPT:
            record.string = f"{postscript_family}-{STYLES[0]}"
        elif record.nameID == UNIQUE_ID:
            # `<version>;<vendor>;<PostScript name>`; only the last field names
            # the font, so the version and vendor the file was built with stay.
            fields = record.toUnicode().split(";")
            fields[-1] = f"{postscript_family}-{STYLES[0]}"
            record.string = ";".join(fields)
    for nameID in TYPOGRAPHIC:
        name.removeNames(nameID)

    instances = font["fvar"].instances
    if len(instances) != len(STYLES):
        raise SystemExit(
            f"{family}: expected {len(STYLES)} named instances, found {len(instances)}"
        )
    for style, instance in zip(STYLES, instances):
        # A style name can be shared: in the Italics, the record the first
        # instance points at is also the name three of `STAT`'s axis values
        # carry. Rewriting it in place would relabel those too — an italic file
        # whose `ital` axis value reads "Regular". So the instance gets a record
        # of its own and `STAT` keeps the one it was reading.
        instance.subfamilyNameID = name.addName(style, minNameID=INSTANCE_NAMES_FROM)
        # The instance's PostScript name is `fvar`'s alone, and it is the one
        # that carries the Reserved Font Name, so it is rewritten where it lies.
        set_all(name, instance.postscriptNameID, f"{postscript_family}-{style}")


def set_all(name, nameID, string):
    """Sets every record of `nameID`, and complains if there are none."""
    records = [record for record in name.names if record.nameID == nameID]
    if not records:
        raise SystemExit(f"no `name` record {nameID} to set to {string!r}")
    for record in records:
        record.string = string


def regularise_space(font, roman):
    """Gives `font` the word space of `roman`, and freezes it across the axes.

    The space has no contours, so its `gvar` entry carries nothing but the
    phantom points that vary the advance; dropping it leaves the advance at the
    default instance's value at every weight and spacing.
    """
    with TTFont(roman, lazy=True) as upright:
        advance = upright["hmtx"][SPACE_GLYPH][0]
    font["hmtx"][SPACE_GLYPH] = (advance, font["hmtx"][SPACE_GLYPH][1])
    font["gvar"].variations[SPACE_GLYPH] = []
    return advance


def pin_advance(font):
    """Freezes every advance in `font` across `wght`, and counts what moved.

    A `gvar` tuple ends in the four phantom points, of which the second is the
    advance; zeroing it on every tuple that varies on `wght` leaves the outline
    deltas alone and holds the advance at the default instance's value, which
    is what `hmtx` already carries. The other axis, `SPCG`, is the one that is
    meant to vary an advance, and keeps its deltas — which holds for these six
    files, where every tuple that varies an advance varies on `wght` alone. A
    tuple varying on both would lose its `SPCG` advance too; none does.
    """
    pinned = 0
    for variations in font["gvar"].variations.values():
        for variation in variations:
            if "wght" not in variation.axes:
                continue
            if variation.coordinates[ADVANCE_PHANTOM_INDEX] in (None, (0, 0)):
                continue
            variation.coordinates[ADVANCE_PHANTOM_INDEX] = (0, 0)
            pinned += 1
    return pinned


# What the strikeout metric is pinned to, in the Faces' 1000-unit em.
#
# iA's own files ask for a 60-unit rule 309 above the baseline, and neither
# number is what the Design oracle draws: it rules **2 device px centred on the
# x-height** at the default size (#354, `dev/ref/ia/mac-native/VERDICTS.md` § The
# Style Check mark; `docs/design.md` § Rows, Style check mark). macOS ignores
# the metric and draws its own rule; Pango obeys the metric and has no API to
# override it, so the oracle's rule is written into the Faces here — the one
# place a strikethrough's geometry can be set at all.
#
# These two numbers are measured off a judged shot, not converted from the
# oracle's own em fraction. The oracle's 2 px is 0.047 em in the face and at
# the size it was measured in; 30 per 1000 units is 0.030 em, and what it is
# calibrated against is our own rasterisation at the judged text size. Pango
# draws the rule downward from `yStrikeoutPosition` and thickens it by
# `yStrikeoutSize`, and iA's 60 at 309 came out 4 device px with its top 2 px
# above the x-height centre, so the size is halved to the oracle's 2 px and the
# position drops 47 units — 2 device px at the judged size — to centre what is
# left.
#
# The position is the sounder of the two: 262 is the x-height centre (516 / 2 =
# 258) to within four units, which is #354's own reading of what a second
# capture would leave standing. The thickness has no such rule behind it —
# #354 leaves "whether the thickness is a fixed device value or a rounded
# fraction of the em" for a second step to separate — so it is right at the one
# text step the `style` Piece shoots and may part from the oracle at others.
STRIKEOUT_SIZE = 30
STRIKEOUT_POSITION = 262


def pin_strikeout(font):
    """Sets `font`'s strikeout rule to the one the Design oracle draws.

    Only strikethroughs read these two fields, so this moves Style check's
    mark and Markdown's `~~` and nothing else on the page.
    """
    os2 = font["OS/2"]
    os2.yStrikeoutSize = STRIKEOUT_SIZE
    os2.yStrikeoutPosition = STRIKEOUT_POSITION
    return f"  strikeout {STRIKEOUT_SIZE}/{STRIKEOUT_POSITION}"


def roman(face):
    """The upright source file of `face`, which its Italic is measured against."""
    for directory, filename, name, italic in FACES:
        if name == face and not italic:
            return SOURCE / directory / filename
    raise SystemExit(f"no upright source for {face}")


def licence():
    """iA's licence text, which all three families ship identically."""
    texts = {(SOURCE / directory / "LICENSE.md").read_text(encoding="utf-8") for directory, *_ in FACES}
    if len(texts) != 1:
        raise SystemExit("the three iA LICENSE.md files differ; pick one deliberately")
    return texts.pop()


def build():
    FONTS.mkdir(exist_ok=True)
    for directory, filename, face, italic in FACES:
        family = f"{PREFIX} {face} Italic" if italic else f"{PREFIX} {face}"
        postscript_family = family.replace(" ", "")
        source = SOURCE / directory / filename
        target = FONTS / f"{postscript_family}.ttf"

        # `recalcTimestamp` off, or every build would stamp `head.modified` with
        # the time it ran and no two runs would agree. `lazy` leaves the tables
        # this script never reads exactly as iA compiled them.
        font = TTFont(source, recalcTimestamp=False, recalcBBoxes=False, lazy=True)
        rename(font, family, postscript_family)
        note = ""
        if face == "Quattro" and italic:
            note = f"  word space {regularise_space(font, roman(face))}"
        note += f"  {pin_advance(font)} advances pinned"
        note += pin_strikeout(font)
        # A signature over bytes we have just changed is worse than none.
        if "DSIG" in font:
            del font["DSIG"]
        font.save(target)
        font.close()
        print(f"{source.relative_to(ROOT)} -> {target.relative_to(ROOT)}  {family}{note}")

    text = LICENCE_PREAMBLE + licence()
    (FONTS / "OFL.txt").write_text(text, encoding="utf-8")
    print(f"fonts/OFL.txt  {len(text)} bytes")


if __name__ == "__main__":
    if len(sys.argv) > 1:
        raise SystemExit(__doc__)
    build()
