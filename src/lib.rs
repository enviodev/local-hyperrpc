mod args;
mod bytes_builder;
mod config;
mod eth_rpc;
mod http_server;
mod rpc_client;
mod runner;
pub use args::Args;
pub use runner::Runner;
mod query_handler;

#[derive(Clone, Copy, Debug)]
struct BlockRange(u64, u64);
