#!/bin/bash
shopt -s expand_aliases
alias ocl="$HOME/jsonrpc-cli/jsonrpc-cli --user=$RPC_USER --pass=$RPC_PASS --format=jsonpretty --resultonly=on --highlight=off  http://$RPC_CONNECT:$RPC_PORT/"
# parameters:
# $1 Genesis hash
# $2 start price
# $3 end price
# $4 auction duration
# $5 request duration
# $6 number of tickets
# $7 fee percentage
# $8 Permission asset private key
# OPTIONAL
# $9 prevtxid
# $10 prevvout

# Check parameters are set
if [ -z $1 ] || [ -z $2 ] || [ -z $3 ] || [ -z $4 ] || [ -z $5 ] || [ -z $6 ] || [ -z $7 ]
then
    printf "%s\n" "createRequest genesisHash startPrice endPrice auctionDuration requestDuration numTickets feePercentage privKey ( txid ) ( vout )" \ \
    "Script builds, signs and sends a request transaction to service chain." \
    "Set shell enviroment variables RPC_CONNECT, RPC_PORT, RPC_USER, RPC_PASS with network connection information." \
    "By deflault a TX_LOCKED_MULTISIG transaction or standard permission asset unspent output is spent to fund the request. If a specific permission asset transaction should be used then set parameters 9 and 10 accordingly." \
    \ \
    "Arguments:" \
    "1. \"Genesis hash\"        (Hex string, Required) Hash of client chain genesis block" \
    "2. \"StartPrice\"          (Amount, Required) Starting auction price of tickets" \
    "3. \"endPrice\"            (Amount, Required) Ending auction price of tickets" \
    "4. \"auctionDuration\"     (Integer, Required) Number of blocks auction to last for" \
    "5. \"requestDuration\"     (Integer, Required) Number of blocks service period to last for" \
    "6. \"numTickets\"          (Integer, Required) Number of tickets to be sold" \
    "7. \"feePercentage\"       (Integer, Required) Percentage of fee to go towards rewarding guardnodes" \
    "8. \"privKey\"             (String (hex), Optional) Hex encoded private key of address with permission asset" \
    "9. \"txid\"                (String (hex), Optional) Specified previous request transaction ID to fund new request" \
    "10. \"vout\"                (Integer, Optional) Specified previous request vout to fund new request"
    \ \
    "Result: " \
    "\"txid\"                    (hex string) Transaction ID of request transaction"
    exit
fi

# Client chain genesis block hash
genesis=$1
# check for currently active request for given genesis hash
if [[ `ocl getrequests | jq "if .[].genesisBlock == \"$genesis\" then 1 else 0 end"` == *"1"* ]]
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

unspent=`ocl listunspent '[1, 9999999, [], true, "PERMISSION"]' | jq -c '.[]'`
# Import private key. Check if list unspent is empty first to avoid unnessesary re-scanning every time script runs
if [ ! -z $8 ] && [[ -z $unspent ]]
then
        echo "Importing private key..."
        ocl importprivkey $8 > /dev/null
        unspent=`ocl listunspent '[1, 9999999, [], true, "PERMISSION"]' | jq -c '.[]'`
fi

checkLockTime () {
    if [[ $currentblockheight -gt `echo $1 | jq -r '.locktime'` ]]
    then
        return 0
    fi
    return 1
}
# Check for specified previous request transaction info and set txid, vout variables accordingly
if [ -n "$9" ] || [ -n "${10}" ]
then
    if [ -z $9 ] || [ -z ${10} ]
    then
        printf "Input parameter error: txid and vout must be provided for previous request transaction.\n"
        exit
    fi
    txid=$9
    vout=$10
    tx=`ocl decoderawtransaction $(ocl getrawtransaction $txid)`
    if checkLockTime "$tx";
    then
        value=`echo $tx | jq -r '.vout[0].value'`
    else
        printf "Input parameter error: Previous request transaction nlocktime not met.\n"
        exit
    fi
else
    # Get previously locked TX_LOCKED_MULTISIG unspent output
    for i in $unspent;
    do
        if [ `echo $i | jq ".solvable"` = "false" ]
        then
            txid=`echo $i | jq -r ".txid"`
            tx=`ocl decoderawtransaction $(ocl getrawtransaction $txid | jq -r '.')`
            if checkLockTime "$tx";
            then
                value=`echo $tx | jq -r '.vout[0].value'` # TX_LOCKED_MULTISIG permission
                vout=0                                    # asset always vout=0
                break
            fi
        fi
    done
    # If value not set yet then get standard permission asset unspent output
    if [ -z $value ]
    then
        for i in $unspent;
        do
            txid=`echo $i | jq -r ".txid"`
            tx=`ocl decoderawtransaction $(ocl getrawtransaction $txid | jq -r '.')`
            if checkLockTime "$tx";
            then
                value=`echo $i | jq -r ".amount"`
                vout=`echo $i | jq ".vout"`
                break
            fi
        done
    fi
    if [[ ${#unspent[0]} = 4 || -z $txid ]] # unspent or txid is null
    then
        printf "Error: No unspent TX_LOCKED_MULTISIG or permission asset transaction outputs available in wallet.\n"
        exit
    fi
fi
# Address permission tokens will be locked in
pub=`ocl validateaddress $(ocl getnewaddress | jq -r '.') | jq -r ".pubkey"`

# Generate and sign request transaction
inputs="{\"txid\":\"$txid\",\"vout\":$vout}"
outputs="{\"decayConst\":$decay,\"endBlockHeight\":$end,\"fee\":$fee,\"genesisBlockHash\":\"$genesis\",\
\"startBlockHeight\":$start,\"tickets\":$tickets,\"startPrice\":$price,\"value\":$value,\"pubkey\":\"$pub\"}"

rawtx=`ocl createrawrequesttx '['$(echo $inputs)','$(echo $outputs)']' | jq -r '.'`
signedrawtx=`ocl signrawtransaction $rawtx`

# Catch signing error
if [ `echo $signedrawtx | jq ".complete"` = "false" ]
then
    echo "Signing error: Script cannot be signed. Is the input transaction information correct and is it unlockable now?"
fi

txid=`ocl sendrawtransaction $(echo $signedrawtx | jq -r ".hex") | jq -r '.'`
echo "Request txid: $txid"

# Import spending address to allow script to automatically update request
address=`ocl decoderawtransaction $(echo $signedrawtx | jq -r '.hex') | jq -r '.vout[0].scriptPubKey.hex'`
ocl importaddress $address > /dev/null
