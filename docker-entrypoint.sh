#!/bin/bash

if [ -f /run/secrets/co_storage_pass ]; then
    export CO_STORAGE_PASS="$(cat /run/secrets/co_storage_pass)"
fi

if [ -f /run/secrets/co_api_pass ]; then
    export CO_API_PASS="$(cat /run/secrets/co_api_pass)"
fi

if [ -f /run/secrets/co_clientchain_pass ]; then
    export CO_CLIENTCHAIN_PASS="$(cat /run/secrets/co_clientchain_pass)"
fi

if [ -f /run/secrets/co_service_pass ]; then
    export CO_SERVICE_PASS="$(cat /run/secrets/co_service_pass)"
fi

if [ -f /run/secrets/co_clientchain_asset_key ]; then
    export CO_CLIENTCHAIN_ASSET_KEY="$(cat /run/secrets/co_clientchain_asset_key)"
fi

case "$1" in
        coordinator)
            echo "Running coordinator"
            cargo run
            ;;
        *)
            "$@"

esac
