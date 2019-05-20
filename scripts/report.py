#!/usr/bin/env python3
import requests
import json

txid = "f78038d112a567f0a2efc4a4d6ecafb9c815227846b97c014bfaa172dd944506"
url = 'http://userApi:passwordApi@localhost:3333'

payload = '{{"jsonrpc": "2.0", "method": "getrequest", "params": {{"txid": "{}"}}, "id": 1}}'.format(txid)
headers = {'content-type': 'application/json', 'Accept-Charset': 'UTF-8'}
r = requests.post(url, data=payload, headers=headers)

result = json.loads(json.loads(r.content)['result'])
request = result["request"]

print("Request txid: {}".format(txid))
print("Request details:\n{}".format(request))
print("")

print("Bids")
bids = {}
for bid in result["bids"]:
    bids[bid['txid']] = bid['pubkey']
    print(bid)
print("")

payload = '{{"jsonrpc": "2.0", "method": "getrequestresponses", "params": {{"txid": "{}"}}, "id": 1}}'.format(txid)
headers = {'content-type': 'application/json', 'Accept-Charset': 'UTF-8'}
r = requests.post(url, data=payload, headers=headers)

result = json.loads(json.loads(r.content)['result'])
challenge_resps = result["responses"]
num_of_challenges = len(challenge_resps)
print("Number of challenges: {}".format(num_of_challenges))
resps = {}
for challenge_resp in challenge_resps:
    for bid_resp in challenge_resp:
        if bid_resp in resps:
            resps[bid_resp] += (1/num_of_challenges)
        else:
            resps[bid_resp] = (1/num_of_challenges)

print("Results")
for bid, key in bids.items():
    performance = 0.0
    if bid in resps:
        performance = resps[bid]
    print("bid: {0}\tpubkey: {1}\t performance: {2:.2f}%".format(bid, key, 100*performance))
