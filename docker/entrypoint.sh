#!/bin/sh
set -e

set -- \
    --data-dir "/data" \
    --port "3000" \
    --auth "${AUTH:-none}"
    --oidc-redirect-port "${OIDC_REDIRECT_PORT:-58656}"
    --oidc-client-id "${OIDC_REDIRECT_PORT:-backstitch}"

if [ -n "${WEBVIEWER:-}" ]; then
    set -- "$@" --webviewer "$WEBVIEWER"
fi

if [ -n "${OIDC_ISSUER:-}" ]; then
    set -- "$@" --oidc-issuer "$OIDC_ISSUER"
fi

exec backstitch-sync-server "$@"