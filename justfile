@_default:
  just --list

[arg('profile', pattern='debug|release')]
run data_dir="./data" port="8085" http_port="3000" profile="release":
    #!/usr/bin/env sh
    mkdir {{data_dir}}
    RUST_BACKTRACE=1 \
    RUST_LOG=automerge_repo=debug,info \
    DATA_DIR={{data_dir}} \
    PORT={{port}} \
    HTTP_PORT={{http_port}} \
    cargo run --{{profile}}