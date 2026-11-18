#!/bin/sh
# scripted "editor": applies $ITER_ED_SED to the buffer
if [ -n "$ITER_ED_SED" ]; then sed -i '' -e "$ITER_ED_SED" "$1"; fi
[ -n "$ITER_ED_DUMP" ] && cat "$1" >&2
exit 0
