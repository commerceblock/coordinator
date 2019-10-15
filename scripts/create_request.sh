#!/bin/bash
shopt -s expand_aliases

alias ocl="ocean-cli -rpcport=7043 -rpcuser=ocean -rpcpassword=oceanpass"

# parameters:
# $1 Genesis hash
# $2 start price
# $3 end price
# $4 auction duration
# $5 request duration
# $6 number of tickets
# $7 fee percentage
# OPTIONAL
# $8 prevtxid
# $9 prevvout

# Check parameters are set
if [ -z $1 ] || [ -z $2 ] || [ -z $3 ] || [ -z $4 ] || [ -z $5 ] || [ -z $6 ] || [ -z $7 ]
then
    printf "%s\n" "createRequest genesisHash startPrice endPrice, auctionDuration, requestDuration, numTickets feePercentage ( txid ) ( vout )"
    \ \
    "Script builds, signs and sends a request transaction to service chain. By deflault a previously TX_LOCKED_MULTISIG transaction is spent to fund the request. If a specific permission asset transaction should be used then set parameters 8 and 9 accordingly." \
    ""
    \ \
    "Arguments:" \
    "1. \"Genesis hash\"        (Hex string, Required) Hash of client chain genesis block" \
    "2. \"StartPrice\"          (Amount, Required) Starting auction price of tickets" \
    "3. \"endPrice\"            (Amount, Required) Ending auction price of tickets" \
    "4. \"auctionDuration\"     (Integer, Required) Number of blocks auction to last for" \
    "5. \"requestDuration\"     (Integer, Required) Number of blocks service period to last for" \
    "6. \"numTickets\"          (Integer, Required) Number of tickets to be sold" \
    "7. \"feePercentage\"       (Integer, Required) Percentage of fee to go towards rewarding guardnodes" \
    "8. \"txid\"                (Hex string, Optional) Specified previous request transaction ID to fund new request" \
    "9. \"vout\"                (Integer, Optional) Specified previous request vout to fund new request"
    \ \
    "Result: " \
    "\"txid\"                    (hex string) Transaction ID of request transaction"

    exit
fi

# Client chain genesis block hash
genesis=$1
# check for currently active request for given genesis hash
if [ `ocl getrequests | jq "if .[].genesisBlock == \"$genesis\" then 1 else empty end"` ]
then
    printf "Input parameter error: Genesis hash already in active request list. Relevant request info below.\n\n"
    echo "Current block height: " `ocl getblockcount`
    ocl getrequests $1 | jq '.[]'
    exit
fi

# Request start height = current height + auction duration
currentblockheight=`ocl getblockchaininfo | jq ".blocks"`
let start=$currentblockheight+$5
# Request end height = request start height + request duration
let end=start+$4

# Starting price
price=$2
# Decay constant formula
decay=$(echo "$4^3/((1+$4)*(($2/$3)-1))" | bc)
# Number of tickets
tickets=$6
# Fee percentage paid
fee=$7

# Check for specified previous request transaction info and set txid, vout variables accordingly
if [ -n "$8" ] || [ -n "$9" ]
then
    if [ -z $8 ] || [ -z $9 ]
    then
        printf "Input parameter error: txid and vout must be provided for previous request transaction.\n"
        exit
    fi
    txid=$8
    vout=$9
    tx=`ocl decoderawtransaction $(ocl getrawtransaction $txid)`
else
    # Get previously locked TX_LOCKED_MULTISIG unspent output
    unspent=`ocl listunspent 1 9999999 [] true "PERMISSION" | jq -c '.[]'`
    for i in $unspent;
    do
        if [ `echo $i | jq ".solvable"` = "false" ]
        then
            txid=`echo $i | jq -r ".txid"`
            tx=`ocl decoderawtransaction $(ocl getrawtransaction $txid)`
            value=`echo $tx | jq -r '.vout[0].value'`
            vout=0
            break
        fi
    done
    if [ -z $txid ] # unspent is null
    then
        printf "Error: No available TX_LOCKED_MULTISIG unspent transaction outputs.\n"
        exit
    fi
fi
# Check lock time
if [[ `echo $tx | jq -r '.locktime'` -lt $currentblockheight ]]
then
    value=`echo $tx | jq -r '.vout[0].value'`
else
    printf "Input parameter error: Previous request transaction nlocktime not met.\n"
    exit
fi

# Address permission tokens will be locked in
pub=`ocl validateaddress $(ocl getnewaddress) | jq -r ".pubkey"`

# Generate and sign request transaction
inputs="{\"txid\":\"$txid\",\"vout\":$vout}"
outputs="{\"decayConst\":$decay,\"endBlockHeight\":$end,\"fee\":$fee,\"genesisBlockHash\":\"$genesis\",\
\"startBlockHeight\":$start,\"tickets\":$tickets,\"startPrice\":$price,\"value\":$value,\"pubkey\":\"$pub\"}"

signedtx=`ocl signrawtransaction $(ocl createrawrequesttx $inputs $outputs)`
# Catch signing error
if [ `echo $signedtx | jq ".complete"` = "false" ]
then
    echo "Signing error: Script cannot be signed. Is the input transaction information correct and is it unlockable now?"
fi

txid=`ocl sendrawtransaction $(echo $signedtx | jq -r ".hex")`
echo "txid: $txid"

# import spending address to allow script to automatically update request
address=`ocl decoderawtransaction $(echo $signedtx | jq -r '.hex') | jq -r '.vout[0].scriptPubKey.hex'`
ocl importaddress $address
