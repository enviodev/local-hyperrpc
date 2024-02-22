use anyhow::Context;

use local_hyperrpc::{Runner, Args};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = Args::parse();

    Runner::run(args)
        .await
        .unwrap();
}
        