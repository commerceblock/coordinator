#!/bin/bash

echo "Getting all requests..."
RESP=$(curl -s -S -X POST -H "Content-Type: application/json"\
    -d "{\"jsonrpc\": \"2.0\", \"method\": \"getrequests\", \"params\" : {}, \"id\":1 }" -u $1 $2)
echo $RESP | jq -r '.result' | jq -r .

TXID=$(echo $RESP | jq -r ".result" | jq -r  ".requests[0].request.txid")
if [ ! -z $3 ]; then
    TXID=$3
fi

echo "Getting request $TXID..."
RESP=$(curl -s -S -X POST -H "Content-Type: application/json"\
    -d "{\"jsonrpc\": \"2.0\", \"method\": \"getrequestresponse\", \"params\" : {\"txid\": \"$TXID\"}, \"id\":2 }" -u $1 $2)

echo $RESP | jq -r '.'

RESP=$(curl -s -S -X POST -H "Content-Type: application/json"\
    -d "{\"jsonrpc\": \"2.0\", \"method\": \"getrequest\", \"params\" : {\"txid\": \"$TXID\"}, \"id\":3 }"  -u $1 $2)

echo $RESP | jq -r '.'
