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

// Function to make RPC request
async function makeRPCRequest(endpoint) {
  let seedBlock = 0x989610;
  let startBlock = getRandomBlock(seedBlock);
  let endBlock = startBlock + 100;
  let startBlockHex = "0x" + startBlock.toString(16);
  let endBlockHex = "0x" + endBlock.toString(16);

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

  const startTime = performance.now();
  try {
    await fetch(endpoint, {
      method: "POST",
      body: JSON.stringify(requestBody),
      headers: { "Content-Type": "application/json" },
    });
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
  const rawResults = {}; // Object to store raw results
  const summaryResults = {}; // Object to store summary results

  // Iterate over RPC endpoints
  for (const [endpointName, endpoint] of Object.entries(endpoints)) {
    console.log(`Benchmarking ${endpointName}...`);
    const requestTimes = [];

    // Make 30 iterations of RPC requests
    for (let i = 0; i < 30; i++) {
      const requestTime = await makeRPCRequest(endpoint);
      if (requestTime !== null) {
        requestTimes.push(requestTime);
      }
    }

    // Store raw results
    rawResults[endpointName] = requestTimes;

    // Calculate average request time
    const averageRequestTime =
      requestTimes.reduce((acc, curr) => acc + curr, 0) / requestTimes.length;
    summaryResults[endpointName] = averageRequestTime;

    console.log(
      `Average request time for ${endpointName}: ${averageRequestTime.toFixed(
        2
      )} ms`
    );
  }

  // Save raw results to file
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
  const latestResultsContent = Object.entries(summaryResults)
    .map(
      ([endpoint, averageTime]) => `${endpoint}: ${averageTime.toFixed(2)} ms`
    )
    .join("\n");
  fs.writeFileSync(latestResultsFilePath, latestResultsContent);
  console.log(`Latest results saved to ${latestResultsFilePath}`);
  console.log("Summary results");
  console.log("----------");
  console.log(latestResultsContent);
}

// Run benchmarks
runBenchmarks();
