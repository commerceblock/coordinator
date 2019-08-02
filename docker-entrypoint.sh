#!/bin/bash

if [ -f /run/secrets/mongo_pass ]; then
    export CO_STORAGE_PASS="$(cat /run/secrets/mongo_pass)"
fi

if [ -f /run/secrets/coordinator_pass ]; then
    export CO_API_PASS="$(cat /run/secrets/coordinator_pass)"
fi

if [ -f /run/secrets/ocean_pass ]; then
    export CO_CLIENTCHAIN_PASS="$(cat /run/secrets/ocean_pass)"
fi

if [ -f /run/secrets/service_pass ]; then
    export CO_SERVICE_PASS="$(cat /run/secrets/oceanservice_pass)"
fi

case "$1" in
        coordinator)
            echo "Running coordinator"
            cargo run
            ;;
        *)
            "$@"

esac
