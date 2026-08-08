@_default:
  just --list

help:
    #!/usr/bin/env sh
    set -euo pipefail

    RUST_BACKTRACE=full \
    RUST_LOG=automerge_repo=debug,info \
    cargo run --release -- --help

run *args:
    #!/usr/bin/env sh
    set -euo pipefail

    RUST_BACKTRACE=full \
    RUST_LOG=automerge_repo=debug,info \
    cargo run --release -- \
        {{args}}

[arg('profile', pattern='debug|release')]
dev profile="release" *args:
    #!/usr/bin/env sh
    set -euo pipefail

    mkdir -p "./data"

    RUST_BACKTRACE=full \
    RUST_LOG=automerge_repo=debug,info \
    cargo run --{{profile}} -- \
        --data-dir "./data" \
        --port 3000 \
        {{args}}

[arg('profile', pattern='debug|release')]
dev-oidc profile="release" *args:
    #!/usr/bin/env sh

    mkdir -p "./data"

    cleanup() {
        docker compose -f tools/rauthy/docker-compose.yml down
        exit 1
    }

    trap cleanup EXIT INT TERM

    docker compose -f tools/rauthy/docker-compose.yml up -d

    echo "Waiting for Rauthy..."
    until curl -kfsS https://localhost:8443/auth/v1/health >/dev/null 2>&1; do
        echo "Checking Rauthy's health..."
        sleep 1
    done

    RUST_BACKTRACE=full \
    RUST_LOG=automerge_repo=debug,info \
    cargo run --{{profile}} -- \
        --data-dir "./data" \
        --port 3001 \
        --auth "oidc" \
        --oidc-issuer "https://localhost:8443/auth/v1/" \
        {{args}}