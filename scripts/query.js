// Example query for aggregating request responses

db = connect("127.0.0.1:27017/coordinator")

db.getCollection("Request").aggregate([
    {
        $lookup: {
            "from": "Response",
            "localField": "_id",
            "foreignField": "request_id",
            "as": "challenges"
        }
    },
    {
        $match: {
            "txid": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        }
    },
    {
        $unwind: "$challenges"
    },
    {
        $project: {
            "_id": 0,
            "response_bid_txids": "$challenges.bid_txids"
        }
    }
]).pretty();
