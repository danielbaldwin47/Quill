#!/usr/bin/env bash
# Every state this comparison needs, in one pass, so focus leaves the owner's
# panel once rather than once per shot.
set -euo pipefail
D="$(dirname "$0")"
for s in "$@"; do "$D/drive.sh" "$s" >/dev/null; echo "$s"; done
