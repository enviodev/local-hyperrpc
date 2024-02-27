cryo erc20_transfers -b 18753440:18757440 --chunk-size 1000 --rpc "http://localhost:3113" -o data --hex --inner-request-size 50
rm -rf data
