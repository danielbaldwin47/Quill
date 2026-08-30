#!/usr/bin/env bash
# Launch iA Writer for Windows under wine on whatever workspace the dispatch
# put it, with the ticket's test passage open.
# Wine's Z: is the filesystem root, so the fixture is reachable by its own
# absolute path with the separators turned round.
DOC=$(readlink -f "$(dirname "$0")/../jump.md")
export WINEDEBUG=-all
exec wine 'C:\Program Files\iA Writer\iAWriter.exe' "Z:${DOC//\//\\}"
