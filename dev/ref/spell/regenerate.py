#!/usr/bin/env python3
"""Regenerate the Spell check fixture dictionary, dev/ref/spell/hunspell/en_US.dic.

The word list is every correctly spelled word of dev/ref/spell.md, of the prose
passage the bench regimes type (the PROSE constant in tools/regimes.mjs) and of
the document they type it into (dev/shots/latency/doc10k.md, tools/bench.mjs's DOC),
plus the corrections of the passage's eight misspellings, so a suggestion has a
word to offer. A word joined by an apostrophe or a hyphen goes in whole and as
its parts, since whether the dictionary splits it is its own word-character
rule. A token carrying a digit is left out, since the tokeniser never checks
one, and so is every misspelling. Run from anywhere: python3 dev/ref/spell/regenerate.py
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

MISSPELLINGS = {
    "definately": "definitely",
    "recieved": "received",
    "comittee": "committee",
    "Teh": "the",
    "DRAFFT": "draft",
    "seperate": "separate",
    "mispelled": "misspelled",
    "accomodate": "accommodate",
}

# Words the passages capitalise because they are names, not because a sentence
# starts with them; every other word goes into the list lower-cased, and
# hunspell accepts its capitalised and all-caps forms.
NAMES = {"Tuesday", "Marguerite"}


def prose() -> str:
    source = (ROOT / "tools/regimes.mjs").read_text(encoding="utf-8")
    found = re.search(r"const PROSE = `(.*?)`;", source, re.S)
    if found is None:
        raise SystemExit("tools/regimes.mjs has no PROSE passage")
    return found.group(1)


def words(text: str):
    return re.findall(r"[^\W_]+(?:['’-][^\W_]+)*", text)


def main() -> None:
    passage = (ROOT / "dev/ref/spell.md").read_text(encoding="utf-8")
    bench = (ROOT / "dev/shots/latency/doc10k.md").read_text(encoding="utf-8")
    held = set(MISSPELLINGS.values())
    for token in words(passage) + words(prose()) + words(bench):
        for word in {token, *re.split(r"['’-]", token)}:
            if not word or word in MISSPELLINGS or any(c.isdigit() for c in word):
                continue
            held.add(word if word in NAMES else word.lower())
    listing = sorted(held, key=lambda w: (w.lower(), w))
    out = ROOT / "dev/ref/spell/hunspell/en_US.dic"
    out.write_text(f"{len(listing)}\n" + "".join(f"{w}\n" for w in listing), encoding="utf-8")
    print(f"{out.relative_to(ROOT)}: {len(listing)} words")


if __name__ == "__main__":
    main()
