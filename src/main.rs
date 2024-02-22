use anyhow::Context;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
fn main() {
    env_logger::init();

    runtime
        .block_on(async move {
            match Subcommands::parse() {
                Subcommands::Run(args) => Runner::run(args).await.context("run local hyperRPC proxy")?,
                Subcommands::Check(args) => run_checker(args).context("check data integrity")?,
            }

            Ok::<_, anyhow::Error>(())
        })
        .expect("run local hyperRPC proxy");
}