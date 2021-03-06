# Script to generate report of responses from guardnodes using
# coordinator RPC API interface.
#
# First connection to a local Ocean node is made, the request data for a
# txid is found via getrequest RPC and fee size is calculated.
# Then getrequestresponse RPC is called to get challenge responses which can be
# used to determine rewards for guardnodes.

#!/usr/bin/env python3
import requests
import json

############# PORTED FROM BITCOIN TEST-FRAMEWORK ##################
import authproxy
import hashlib
from binascii import hexlify, unhexlify

def connect(user, password, host, port):
    return authproxy.AuthServiceProxy("http://%s:%s@%s:%s"%
        (user, password, host, port))

def sha256(s):
    return hashlib.new('sha256', s).digest()

def hash160(s):
    return hashlib.new('ripemd160', sha256(s)).digest()

def hash256(s):
    return sha256(sha256(s))

def bytes_to_hex_str(byte_str):
    return hexlify(byte_str).decode('ascii')

def hex_str_to_bytes(hex_str):
    return unhexlify(hex_str.encode('ascii'))

chars = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

def byte_to_base58(b, version):
    result = ''
    str = bytes_to_hex_str(b)
    str = bytes_to_hex_str(chr(version).encode('latin-1')) + str
    checksum = bytes_to_hex_str(hash256(hex_str_to_bytes(str)))
    str += checksum[:8]
    value = int('0x'+str,0)
    while value > 0:
        result = chars[value % 58] + result
        value //= 58
    while (str[:2] == '00'):
        result = chars[0] + result
        str = str[2:]
    return result

def check_key(key):
    if (type(key) is str):
        key = hex_str_to_bytes(key) # Assuming this is hex string
    if (type(key) is bytes and (len(key) == 33 or len(key) == 65)):
        return key
    assert(False)

def key_to_p2pkh(key, version):
    key = check_key(key)
    return byte_to_base58(hash160(key), version)

###################################################################

# Calculate fees from starting to ending height or
# print an error if connectivity to ocean client fails
def calculate_fees(rpc, start_height, end_height):
    fee = 0
    try:
        for i in range(start_height, end_height + 1):
            block = rpc.getblock(rpc.getblockhash(i))
            coinbase_tx = rpc.getrawtransaction(block['tx'][0], True)
            for out in coinbase_tx['vout']:
                fee += out['value']
    except Exception as e:
        print("ERROR with rpc connectivity: {0}".format(e))
    return fee

addr_prefix = 28
txid = "de365af1df965518fff05fc7e82716107074553535217daaa5b886b2d63b1d46"
url = 'http://userApi:passwordApi@localhost:3333'
rpc = connect("user1", "password1", "localhost", "5555")

payload = '{{"jsonrpc": "2.0", "method": "getrequest", "params": {{"txid": "{}"}}, "id": 1}}'.format(txid)
headers = {'content-type': 'application/json', 'Accept-Charset': 'UTF-8'}
r = requests.post(url, data=payload, headers=headers)

result = json.loads(json.loads(r.content)['result'])
request = result["request"]

print("Request txid: {}".format(txid))
print("Request details:\n{}".format(request))
print("")

print("Calculating total fees...")
fee_start_height = request['start_blockheight_clientchain']
fee_end_height = request['end_blockheight_clientchain']
fee = calculate_fees(rpc, fee_start_height, fee_end_height)
fee_percentage = request['fee_percentage']
fee_out = fee*fee_percentage/100
print("Fee: {0}".format(fee))
print("Paying out ({0}%): {1}".format(fee_percentage, fee_out))
print("")

print("Bids")
bids = {}
for bid in result["bids"]:
    bids[bid['txid']] = bid['pubkey']
    print(bid)
fee_per_guard = 0.0
if len(bids) > 0:
    fee_per_guard = float(fee_out/len(bids))
print("")

payload = '{{"jsonrpc": "2.0", "method": "getrequestresponse", "params": {{"txid": "{}"}}, "id": 1}}'.format(txid)
headers = {'content-type': 'application/json', 'Accept-Charset': 'UTF-8'}
r = requests.post(url, data=payload, headers=headers)

result = json.loads(json.loads(r.content)['result'])
challenge_resps = result["response"]
num_of_challenges = challenge_resps["num_challenges"]
print("Number of challenges: {}".format(num_of_challenges))
resps = {}
for (bid, resp_num) in challenge_resps["bid_responses"].items():
    resps[bid] = resp_num / num_of_challenges

print("Results")
for bid, key in bids.items():
    performance = 0.0
    if bid in resps:
        performance = resps[bid]
    print("Bid {0}\npubkey: {1}\naddress: {2}\nperformance: {3:.2f}%\nreward: {4}\n".\
        format(bid, key, key_to_p2pkh(key, addr_prefix), 100*performance, fee_per_guard*performance))
print("End")
