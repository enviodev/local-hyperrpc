cryo blocks_and_transactions -b 18753440:18754440 --chunk-size 200 --rpc "https://eth-mainnet.blastapi.io/9f18f305-3290-438a-a6ff-d6cc03490779" --subdirs datatype --hex --exclude-columns input
rm -rf .cryo
rm -rf blocks
rm -rf transactions
