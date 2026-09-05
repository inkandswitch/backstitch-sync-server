#!/bin/sh
set -e

set -- \
    --data-dir "/data" \
    --port "3000" \
    --auth "${AUTH:-none}" \
    --oidc-redirect-port "${OIDC_REDIRECT_PORT:-58656}" \
    --oidc-client-id "${OIDC_CLIENT_ID:-backstitch}"

if [ -n "${WEBVIEWER:-}" ]; then
    set -- "$@" --webviewer "$WEBVIEWER"
fi

if [ -n "${WEBVIEWER_PATH:-}" ]; then
    set -- "$@" --webviewer-path "$WEBVIEWER_PATH"
fi

if [ -n "${SIGNING_KEY:-}" ]; then
    set -- "$@" --signing-key "$SIGNING_KEY"
fi

if [ -n "${OIDC_ISSUER:-}" ]; then
    set -- "$@" --oidc-issuer "$OIDC_ISSUER"
fi

if [ -n "${OIDC_RESOURCE:-}" ]; then
    set -- "$@" --oidc-resource "$OIDC_RESOURCE"
fi

if [ "${NO_WEBVIEWER_AUTH:-false}" = "true" ]; then
    set -- "$@" --no-webviewer-auth
fi

exec backstitch-sync-server "$@"