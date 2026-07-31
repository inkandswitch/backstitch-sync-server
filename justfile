@_default:
  just --list

[arg('profile', pattern='debug|release')]
[arg('auth', pattern='none')]
run data_dir="./data" port="3000" profile="release" auth="none" webviewer="" :
    #!/usr/bin/env sh
    mkdir -p {{data_dir}}

    webviewer_arg=""
    if [ -n "{{webviewer}}" ]; then
        webviewer_arg="--webviewer \"{{webviewer}}\""
    fi

    RUST_BACKTRACE=full \
    RUST_LOG=automerge_repo=debug,info \
    cargo run --{{profile}} -- \
        --data-dir "{{data_dir}}" \
        --port "{{port}}" \
        --auth "{{auth}}" \
        $webviewer_arg