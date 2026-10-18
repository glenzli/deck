#!/bin/sh
set -eu
DECK_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DECK_DEBUG_ROOT="${DECK_DEBUG_ROOT:-$(dirname -- "$DECK_ROOT")/.deck-local-build}"
DECK_EXECUTABLE="$DECK_DEBUG_ROOT/current-debug/Deck.app/Contents/MacOS/deck-desktop"
if [ ! -x "$DECK_EXECUTABLE" ]; then
    echo "Deck debug app is not available. Run scripts/build_and_promote_debug.sh first." >&2
    exit 1
fi
exec "$DECK_EXECUTABLE" "$@"
