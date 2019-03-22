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
    'port=6666' \
    'initialfreecoins=2100000000000000' \
    'daemon=1' \
    'listen=1' \
    'txindex=1' \
    'initialfreecoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'freezelistcoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'burnlistcoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'whitelistcoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' \
    'challengecoinsdestination=76a914be70510653867b1c648b43cfb3b0edf8420f08d788ac' > ~/co-client-dir/ocean.conf

ocd
sleep 5

echo "Importing challenger key"
ocl importprivkey cScSHCQp9AEwzZoucRpX9bMRkLCJ4LoQWBNFTZuD6tPX9qwNMWfQ
sleep 1
