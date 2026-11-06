#!/usr/bin/env bash
# Live in a terminal; --once for scripts, or --watch [seconds] to force refresh.
exec python3 "$(dirname -- "$(readlink -f -- "$0")")/status.py" "$@"
