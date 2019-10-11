#!/bin/bash
shopt -s expand_aliases

# Check parameters are set
if [ -z $1 ] || [ -z $2 ] || [ -z $3 ] || [ -z $4 ] || [ -z $5 ] || [ -z $6 ] || [ -z $7 ]
then
    printf "%s\n" "buildRequest genesisHash startPrice endPrice requestDuration, auctionDuraction, numTickets feePercentage " \
    "Script builds, signs and sends a request transaction to service chain." \
    \
    "Arguments" \
    "..."
    exit
fi
# parameters:
# $1 Genesis hash
# $2 start price
# $3 end price
# $4 request duration
# $5 auction duration
# $6 number of tickets
# $7 fee percentage


# alias ocl="ocean-cli -rpcport=7043 -rpcuser=ocean -rpcpassword=oceanpass"
alias ocl="/$HOME/ocean/src/ocean-cli -datadir=$HOME/nodes/node1"

echo "Creating request in service chain"

# Address permission tokens will be locked in
pub=`ocl validateaddress $(ocl getnewaddress) | jq -r ".pubkey"`
# Get permission asset unspent
unspent=`ocl listunspent 1 9999999 [] true "PERMISSION" | jq .[0]`
value=`echo $unspent | jq -r ".amount"`
txid=`echo $unspent | jq ".txid"`
vout=`echo $unspent | jq -r ".vout"`

# TO UNLOCK A PREVIOUS REQUEST
# Provide the `txid` and `vout` for that transaction
# The output can be spent after the locktime is expired
# e.g.
# txid="\"1d91bae7353c0b1fb7178b92b642746ea4ace1d79e1c5d3c680526ef9f4589a7\""
# vout=0
# value=210000

# Client chain genesis block hash
genesis=$1
# Request start height = current height + auction duration
start=
# Request end height = request start height + request duration
end=
# Starting price
price=$2
# Decay constant = starting price (some algor) end price
decay=
# Number of tickets
tickets=$6
# Fee percentage paid
fee=$7

echo $0
echo $1
echo $2

# check for active request for given genesis hash
if [ `ocl getrequests | jq "if .[].genesisBlock == \"$genesis\" then 1 else empty end" ` ]
then
    echo "ERROR: Genesis hash already in active request."
    exit
fi


# Generate and sign request transaction
inputs="{\"txid\":$txid,\"vout\":$vout}"
outputs="{\"decayConst\":$decay,\"endBlockHeight\":$end,\"fee\":$fee,\"genesisBlockHash\":\"$genesis\",\
\"startBlockHeight\":$start,\"tickets\":$tickets,\"startPrice\":$price,\"value\":$value,\"pubkey\":\"$pub\"}"

signedtx=`ocl signrawtransaction $(ocl createrawrequesttx $inputs $outputs)`
txid=`ocl sendrawtransaction $(echo $signedtx | jq -r ".hex")`
echo "txid: $txid"
