#!/bin/bash

TXID=$1

RESP=$(curl -s -X POST -H "Content-Type: application/json"\
    -d "{\"jsonrpc\": \"2.0\", \"method\": \"get_challenge_responses\", \"params\" : {\"txid\": \"$TXID\"}, \"id\":1 }"\
    userApi:passwordApi@localhost:3333)

echo $RESP | jq -r '.'