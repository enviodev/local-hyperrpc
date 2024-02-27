# Raw latency script

In this script we run a very simple vanilla javascript (mostly gpt generated) to run 30 iterations of each natively support local proxy rpc method to

The raw results from the run are saved into the folder `data/raw` and are saved in a file named by the timestamp it was created, similarily a summary of the results is saved to a file in the `data` folder named results-(timestamp).json, where timestamp is the timestamp generated when the script was run. The latest results are also save to a file `results.txt` in the root directory as the results from the latest run.

The following rpc methods are tested;

`eth_getLogs`

Each method is benchmarked on the following rpc endpoints (the actual endpoints are stored in the .env file) here is the .env.example file

```txt
FREE_RPC=https://eth.llamarpc.com
OUR_NODE=http://91.216.245.118:11890/<api-key>
HYPERRPC=https://eth.rpc.hypersync.xyz
LOCAL_PROXY=http://127.0.0.1:3113
BLAST=https://eth-mainnet.blastapi.io/<api-key>
```

The raw data files stored in results-(timestamp).json as an object per rpc method which has an entry for each rpc endpoint and the values are is an array of the time taken in milliseconds of each iteration. We run 30 iterations for each rpc endpoint.

The results.txt file displays this as a table for each rpc method with the average time in milliseconds for each rpc endpoint.

The script uses the native node-fetch library and makes the following request (eth_getLogs of USDT transfers)

```json
{
  "id": 1,
  "method": "eth_getLogs",
  "jsonrpc": "2.0",
  "params": [
    {
      "address": "0xdAC17F958D2ee523a2206206994597C13D831ec7",
      "fromBlock": "0x989610",
      "toBlock": "0x989684",
      "topics": [
        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
      ]
    }
  ]
}
```

Except where the fromBlock value and toBlock value are randomized with the same block range of 100 blocks to prevent cached rpc requests from influencing the results. The block range is in hex values.

To run the benchmarks run `yarn start`
