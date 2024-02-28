const fetch = require("node-fetch");
const fs = require("fs");
const { performance } = require("perf_hooks");

// Load environment variables
require("dotenv").config();

// RPC endpoints
const endpoints = {
  FREE_RPC: process.env.FREE_RPC,
  OUR_NODE: process.env.OUR_NODE,
  HYPERRPC: process.env.HYPERRPC,
  LOCAL_PROXY: process.env.LOCAL_PROXY,
  BLAST: process.env.BLAST,
};

const seedBlock = 18118338;
const seedBlockHex = "0x" + seedBlock.toString(16);

const ethBlockNumberRPCRequest = (_) => {
  return {
    id: 1,
    jsonrpc: "2.0",
    method: "eth_blockNumber",
    params: [],
  };
};

const ethGetLogsRPCRequest = (randomBlockNumber) => {
  const startBlock = randomBlockNumber;
  const blockRange = process.env.ETH_GETLOGS_BLOCKRANGE || 10_000;
  const endBlock = startBlock + blockRange;
  const startBlockHex = "0x" + startBlock.toString(16);
  const endBlockHex = "0x" + endBlock.toString(16);

  const requestBody = {
    id: 1,
    method: "eth_getLogs",
    jsonrpc: "2.0",
    params: [
      {
        address: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
        fromBlock: startBlockHex,
        toBlock: endBlockHex,
        topics: [
          "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
        ],
      },
    ],
  };

  return requestBody;
};

const ethGetBlockReceiptsRPCRequest = (randomBlockNumber) => {
  let blockNumberHex = "0x" + randomBlockNumber.toString(16);

  return {
    id: 1,
    jsonrpc: "2.0",
    method: "eth_getBlockReceipts",
    // params: ["latest"],
    params: [blockNumberHex],
  };
};

// Function to make RPC request
async function makeRPCRequest(endpoint, request) {
  const startTime = performance.now();
  try {
    let resp = await fetch(endpoint, {
      method: "POST",
      body: JSON.stringify(request),
      headers: { "Content-Type": "application/json" },
    });
    if (!resp.ok) {
      console.warn("Request failed", resp);
      resp.data;
    }
    const endTime = performance.now();
    return endTime - startTime; // Return time taken in milliseconds
  } catch (error) {
    console.error(`Error making request to ${endpoint}:`, error);
    return null;
  }
}

// Function to get random block number to shift the range of blocks to query to prevent caching impacting results
function getRandomBlock(seedBlock) {
  const seedBlockNumber = parseInt(seedBlock, 16);
  const entropy = 100_000;
  const randomNumberWithinEntropy = Math.floor(Math.random() * entropy);
  const randomBlock = randomNumberWithinEntropy + seedBlockNumber;
  return randomBlock;
}

// Main function to run benchmarks
async function runBenchmarks() {
  let methods = [
    {
      name: "eth_blockNumber",
      method: (randomBlockNumber) =>
        ethBlockNumberRPCRequest(randomBlockNumber),
    },
    {
      name: "eth_getLogs",
      method: (randomBlockNumber) => ethGetLogsRPCRequest(randomBlockNumber),
    },
    {
      name: "eth_getBlockReceipts",
      method: (randomBlockNumber) =>
        ethGetBlockReceiptsRPCRequest(randomBlockNumber),
    },
  ];

  // these data structures could be improved
  const rawResults = {
    eth_blockNumber: {},
    eth_getLogs: {},
    eth_getBlockReceipts: {},
  }; // Object to store raw results
  const summaryResults = {
    eth_blockNumber: {},
    eth_getLogs: {},
    eth_getBlockReceipts: {},
  }; // Object to store summary results

  for (let method of methods) {
    console.log(`\n`);
    console.log(`Benchmarking ${method.name}`);
    console.log(`--------`);

    // Take the array of endpoints and remove the ones that are ignored in the array environment variable IGNORE_ENDPOINTS
    const ignoreEndpoints = process.env.IGNORE_ENDPOINTS
      ? process.env.IGNORE_ENDPOINTS.split(",")
      : [];

    const filteredEndpoints = Object.fromEntries(
      Object.entries(endpoints).filter(
        ([key, _]) => !ignoreEndpoints.includes(key)
      )
    );

    // Iterate over RPC endpoints
    for (const [endpointName, endpoint] of Object.entries(filteredEndpoints)) {
      console.log(`Benchmarking ${endpointName}...`);
      const requestTimes = [];

      // Make 30 iterations of RPC requests by default unless overridden by ITERATIONS environment variable
      const iterations = process.env.ITERATIONS || 30;

      for (let i = 0; i < iterations; i++) {
        // Make eth_getLogs RPC request
        let blockNumber = getRandomBlock(seedBlockHex);
        const requestTime = await makeRPCRequest(
          endpoint,
          method.method(blockNumber)
        );
        if (requestTime !== null) {
          requestTimes.push(requestTime);
          if (process.env.VERBOSE == "true") {
            console.log(`${i + 1}/${iterations}: ${requestTime.toFixed(2)} ms`);
          }
        }
      }

      // Store raw results
      rawResults[method.name][endpointName] = requestTimes;

      // Calculate average request time
      const averageRequestTime =
        requestTimes.reduce((acc, curr) => acc + curr, 0) / requestTimes.length;

      summaryResults[method.name][endpointName] = averageRequestTime;

      console.log(
        `Average request time for ${endpointName}: ${averageRequestTime.toFixed(
          2
        )} ms`
      );
    }
  }

  //   Save raw results to file
  const timestamp = new Date().toISOString();
  const rawResultsFilePath = `data/raw/results-${timestamp}.json`;
  fs.writeFileSync(rawResultsFilePath, JSON.stringify(rawResults, null, 2));
  console.log(`Raw results saved to ${rawResultsFilePath}`);

  // Save summary results to file
  const summaryResultsFilePath = `data/results-${timestamp}.json`;
  fs.writeFileSync(
    summaryResultsFilePath,
    JSON.stringify(summaryResults, null, 2)
  );
  console.log(`Summary results saved to ${summaryResultsFilePath}`);

  // Save latest results to results.txt
  const latestResultsFilePath = "results.txt";

  let latestResultsContent = "";

  Object.entries(summaryResults).map(([methodName, method]) => {
    latestResultsContent += methodName + "\n";
    const methodResultsContent = Object.entries(method)
      .map(([endpoint, averageTime]) => {
        return `${endpoint}: ${averageTime.toFixed(2)} ms`;
      })
      .join("\n");
    latestResultsContent += methodResultsContent + "\n\n";
  });

  fs.writeFileSync(latestResultsFilePath, latestResultsContent);
  console.log(`Latest results saved to ${latestResultsFilePath}`);
  console.log("\n");
  console.log("Summary results");
  console.log("----------");
  console.log(latestResultsContent);
}

// Run benchmarks
runBenchmarks();
