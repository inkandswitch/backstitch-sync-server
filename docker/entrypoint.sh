#!/bin/sh
set -e

set -- \
    --data-dir "/data" \
    --port "3000" \
    --auth "${AUTH:-none}"

if [ -n "${WEBVIEWER:-}" ]; then
    set -- "$@" --webviewer "$WEBVIEWER"
fi

exec backstitch-sync-server "$@"