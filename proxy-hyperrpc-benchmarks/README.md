# Proxy HyperRPC Benchmarks

This repository is for benchmarking the performance of the proxy hyperRPC service

Our thesis is that the service will significantly perform better over the Proxy HyperRPC service due to the fact that the communication is happening over HyperSync which is significantly faster. 

> Note: The Proxy HyperRPC service, can't natively support every RPC request, for example all write operations, in order to make the proxy RPC service feature rich, non supported methods are supported by porting them to a vanilla RPC service. 

## Benchmarks

### Scenarios

We benchmark and present the results from the following scenarios

1. HyperIndex - Rocket Pool ERC-20 token on mainnet (via RPC)
1. Ponder - Rocket Pool ERC-20 token on mainnet
1. Flood
1. TheGraph node

### Test RPC's

We select a few varying RPC endpoints for comparison

todo
- Llama free RPC
- Blast paid RPC
- Our own running node
- Current HyperRPC 
- Proxy RPC service

### Setup

todo
The results are from running the scenarios under xyz

### HyperIndex
todo
We run the envio HyperIndex on...


## Furtherwork

- Test against more paid RPC services
- Increase number of iterations
- Benchmark under a different setup (high/slow network speed, beefy CPU, chonky RAM)
