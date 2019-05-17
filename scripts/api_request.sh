#!/bin/bash

NUM=1   # set $1 for load testing
if [ ! -z "$1" ]; then
    NUM=$1
    echo "a"
fi

for ((i=1;i<=$NUM;i++)); do
    RESP=$(curl -s -S -X POST -H "Content-Type: application/json"\
        -d "{\"jsonrpc\": \"2.0\", \"method\": \"getrequests\", \"params\" : {}, \"id\":1 }"\
        userApi:passwordApi@localhost:3333)
    echo $i
    if [ $i == 1 ]; then
        echo $RESP | jq -r '.result' | jq -r .
    fi
done

TXID=$(echo $RESP | jq -r ".result" | jq -r  ".requests[0].request.txid")

RESP=$(curl -s -S -X POST -H "Content-Type: application/json"\
    -d "{\"jsonrpc\": \"2.0\", \"method\": \"getrequestresponses\", \"params\" : {\"txid\": \"$TXID\"}, \"id\":2 }"\
    userApi:passwordApi@localhost:3333)

echo $RESP | jq -r '.'

RESP=$(curl -s -S -X POST -H "Content-Type: application/json"\
    -d "{\"jsonrpc\": \"2.0\", \"method\": \"getrequest\", \"params\" : {\"txid\": \"$TXID\"}, \"id\":3 }"\
    userApi:passwordApi@localhost:3333)

echo $RESP | jq -r '.'
