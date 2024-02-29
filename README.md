# Local RPC proxy that optionally utilizes HyperSync and HyperRPC

![Architecture](architecture.png?raw=true "Architecture")

## How to use it

### Requirements
- Rust toolchain, can be found here: https://www.rust-lang.org/learn/get-started. We only test the repo with latest stable release of rust.
- Need to have the `capnp` tool installed. More info on this below.

#### Installing `capnp` tool
- On mac: `brew install capnp`
- Linux: on ubuntu/debian: `apt-get install -y capnproto libcapnp-dev`. If you are using something other than ubuntu/debian, can check the package names for your distro online.
- On windows with chocolatey: `choco install capnproto`. This is not tested so might require more steps, please open an issue if you have trouble with this.

### Configuration
Configure the hyperrpc.toml to your liking. Example config for arbitrum:
```toml
[eth_rpc]
rpc_chain_id = 42161
hyperrpc_url = "https://arbitrum.rpc.hypersync.xyz"
fallback_url = "https://rpc.ankr.com/arbitrum"

[http_server]
addr = "127.0.0.1:3113"

[hypersync]
url = "https://arbitrum.hypersync.xyz"
```

Here we specify:
- `rpc_chain_id` which can be found on chainlist by searching for network name.
- `hyperrpc_url` and `hypersync.url` can be found on envio docs or can just replace eth with your network name (polygon, bsc, optimism etc.)
- `fallback_url` is an rpc endpoint you provide. Can put a garbage url here if you are not using methods that are not provided by the proxy. We implement most read methods so not supported methods can be trace calls or write methods. Can also put a garbage url and test if it works out.
- `addr` is the http socket address the proxy will listen to. When proxy is running you can make regular RPC requests to this address in your machine and the proxy will handle them.

### Start the proxy
Execute `make run` in the project root.
Can also run `RUST_LOG=info cargo run --release` if make is not available.

The proxy will be available on the addr configured in the toml file.

Alternatively if you build the binary and want to specify a path to the config file, you can run it like this:
`RUST_LOG=info local-hyperrpc --config-path /path/to/my/config`
