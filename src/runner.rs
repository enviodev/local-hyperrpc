use std::sync::Arc;

use crate::{args::Args, config::Config, eth_rpc::RpcHandler, http_server::HttpServer};
use anyhow::Context;

pub struct Runner;

impl Runner {
    pub async fn run(args: Args) -> Result<(), anyhow::Error> {
        let cfg = tokio::fs::read_to_string(&args.config_path)
            .await
            .context("read config file")?;

        let cfg: Config = toml::de::from_str(&cfg).context("parse config")?;

        let skar_client =
            skar_client::Client::new(cfg.hypersync).context("couldn't create skar client")?;

        let rpc_handler =
            RpcHandler::new(skar_client, cfg.eth_rpc).context("create rpc handler")?;

        let rpc_handler = Arc::new(rpc_handler);

        HttpServer::run(rpc_handler, cfg.http_server)
            .await
            .context("create http server")?;

        Ok(())
    }
}
