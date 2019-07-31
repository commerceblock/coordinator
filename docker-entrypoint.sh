#!/bin/bash

if [ -f /run/secrets/mongo_pass ]; then
    export CO_STORAGE_PASS="$(cat /run/secrets/mongo_pass)"
fi

if [ -f /run/secrets/coordinator_pass ]; then
    export CO_API_PASS="$(cat /run/secrets/coordinator_pass)"
fi

case "$1" in
        coordinator)
            echo "Running coordinator"
            cargo run
            ;;
        shell)
            bash
            ;;
        *)
            echo $"Usage: $0 {coordinator|shell}"
            exit 1

esac
