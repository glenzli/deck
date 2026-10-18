#!/bin/sh
set -eu
DECK_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DECK_DEBUG_ROOT="${DECK_DEBUG_ROOT:-$(dirname -- "$DECK_ROOT")/.deck-local-build}"
DECK_APP=""
DECK_EVIDENCE=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --app) DECK_APP="$2"; shift 2 ;;
        --evidence) DECK_EVIDENCE="$2"; shift 2 ;;
        *) echo "Usage: $0 [--app VALIDATED.app --evidence PASSED_SMOKE_DIRECTORY]" >&2; exit 2 ;;
    esac
done
if [ -z "$DECK_APP" ]; then
    DECK_BUILD_DIRECTORY="$DECK_DEBUG_ROOT/desktop-dev"
    cmake --preset desktop-dev -S "$DECK_ROOT" -B "$DECK_BUILD_DIRECTORY"
    cmake --build "$DECK_BUILD_DIRECTORY" --parallel "${DECK_BUILD_JOBS:-2}"
    DECK_EVIDENCE="$DECK_BUILD_DIRECTORY/validation-$(date -u +%Y%m%dT%H%M%S)-$$"
    DECK_SMOKE_EVIDENCE="$DECK_EVIDENCE" ctest --test-dir "$DECK_BUILD_DIRECTORY" --output-on-failure
    DECK_APP="$DECK_BUILD_DIRECTORY/apps/desktop/deck-desktop.app"
fi
if [ -z "$DECK_EVIDENCE" ]; then echo "A passed smoke evidence directory is required." >&2; exit 2; fi
python3 "$DECK_ROOT/scripts/check_translations.py"
exec python3 "$DECK_ROOT/scripts/promote_debug.py" --app "$DECK_APP" --evidence "$DECK_EVIDENCE" --root "$DECK_DEBUG_ROOT"
