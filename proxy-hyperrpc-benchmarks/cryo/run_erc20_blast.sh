cryo erc20_transfers -b 18753440:18757440 --chunk-size 1000 --rpc "https://eth-mainnet.blastapi.io/9f18f305-3290-438a-a6ff-d6cc03490779" -o data --hex --inner-request-size 50
rm -rf data
