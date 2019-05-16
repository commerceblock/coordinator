#!/bin/bash
shopt -s expand_aliases

alias ocd="oceand -datadir=$HOME/co-client-dir"
alias ocl="ocean-cli -datadir=$HOME/co-client-dir"

ocl stop
sleep 1

rm -r ~/co-client-dir
mkdir ~/co-client-dir

printf '%s\n' '#!/bin/sh' 'rpcuser=user1' \
    'rpcpassword=password1' \
    'rpcport=5555' \
    'rpcallowip=0.0.0.0/0'\
    'port=6666' \
    'initialfreecoins=2100000000000000' \
    'daemon=1' \
    'listen=1' \
    'txindex=1' \
    'initialfreecoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'freezelistcoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'burnlistcoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'whitelistcoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'permissioncoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'challengecoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' > ~/co-client-dir/ocean.conf

ocd
sleep 5

echo "Importing challenger key"
ocl importprivkey cScSHCQp9AEwzZoucRpX9bMRkLCJ4LoQWBNFTZuD6tPX9qwNMWfQ
sleep 2

# Issue asset for bid creation
echo "Issue asset for guardnodes"
asset=`ocl issueasset 500 0`
asset_hash=`echo $asset | jq -r ".asset"`

# Create request
echo "Create request"
pub=`ocl validateaddress $(ocl getnewaddress) | jq -r ".pubkey"`
unspent=`ocl listunspent 1 9999999 [] true "PERMISSION" | jq .[0]`
value=`echo $unspent | jq -r ".amount"`
genesis=`ocl getblockhash 0`

inputs="{\"txid\":$(echo $unspent | jq ".txid"),\"vout\":$(echo $unspent | jq -r ".vout")}"
outputs="{\"decayConst\":1000,\"endBlockHeight\":10,\"fee\":3,\"genesisBlockHash\":\"$genesis\",\
\"startBlockHeight\":5,\"tickets\":2,\"startPrice\":50,\"value\":$value,\"pubkey\":\"$pub\"}"

signedtx=`ocl signrawtransaction $(ocl createrawrequesttx $inputs $outputs)`
txid=`ocl sendrawtransaction $(echo $signedtx | jq -r ".hex")`

ocl generate 1
ocl getrequests

# Create bid
echo "Create request bid"
addr=`ocl getnewaddress`
pub=`ocl validateaddress $addr | jq -r ".pubkey"`
unspent=`ocl listunspent 1 9999999 [] true $asset_hash | jq .[0]`
value=`echo $unspent | jq -r ".amount"`

inputs="[{\"txid\":$(echo $unspent | jq ".txid"),\"vout\":$(echo $unspent | jq -r ".vout"),\"asset\":\"$asset_hash\"}]"
outputs="{\"endBlockHeight\":10,\"requestTxid\":\"$txid\",\"pubkey\":\"$pub\",\
\"feePubkey\":\"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3\",\
\"value\":50,\"change\":449.999,\"changeAddress\":\"$addr\",\"fee\":0.001}"

signedtx=`ocl signrawtransaction $(ocl createrawbidtx $inputs $outputs)`
txid=`ocl sendrawtransaction $(echo $signedtx | jq -r ".hex")`

ocl generate 1
ocl getrequestbids $(ocl getrequests | jq -r ".[].txid")
