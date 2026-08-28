#!/bin/sh
set -e

set -- \
    --data-dir "/data" \
    --sync-port "8085" \
    --public-sync-port "${PUBLIC_SYNC_PORT:-8085}" \
    --http-port "3000" \
    --auth "${AUTH:-none}"

if [ -n "${WEBVIEWER:-}" ]; then
    set -- "$@" --webviewer "$WEBVIEWER"
fi

exec backstitch-sync-server "$@"