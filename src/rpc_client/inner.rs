use anyhow::Context;
use tokio::time::sleep;

use super::LimitConfig;
use super::{endpoint::Endpoint, EndpointConfig, Error, Result, RpcRequest, RpcResponse};
use std::cmp;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

pub struct RpcClient {
    endpoints: Vec<Endpoint>,
}

impl RpcClient {
    pub fn new(name: String, url: String) -> anyhow::Result<Self> {
        let http_client = reqwest::Client::builder()
            .gzip(true)
            .http1_only()
            .timeout(Duration::from_secs(20))
            .tcp_keepalive(Duration::from_secs(7200))
            .build()
            .unwrap();

        let endpoints = vec![Endpoint::new(
            http_client,
            EndpointConfig {
                url: url.parse().context("parse url")?,
                bearer_token: None,
                status_refresh_interval_secs: NonZeroU64::new(1).unwrap(),
                limit: LimitConfig {
                    req_limit: NonZeroUsize::new(123123123).unwrap(),
                    req_limit_window_ms: NonZeroU64::new(1000).unwrap(),
                    batch_size_limit: NonZeroUsize::new(123123).unwrap(),
                },
                label: Some(name),
            },
        )];

        Ok(Self { endpoints })
    }

    pub async fn last_block(&self) -> u64 {
        let mut last_block = 0;

        for e in self.endpoints.iter() {
            if let Some(lb) = e.last_block().await {
                last_block = cmp::max(last_block, *lb);
            }
        }

        last_block
    }

    /// Executes the given rpc request without retries
    pub async fn send_once(&self, req: RpcRequest) -> Result<RpcResponse> {
        let req = Arc::new(req);
        let mut errs = Vec::new();
        for endpoint in self.endpoints.iter() {
            match endpoint.send(req.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    log::debug!(
                        "failed make request to endpoint {}.\nCaused by: {}",
                        endpoint.url(),
                        e
                    );
                    errs.push(e);
                }
            }
        }

        Err(Error::NoHealthyEndpoints(errs))
    }

    // Executes the given rpc request, retries using exponential backoff.
    pub async fn send(&self, req: RpcRequest) -> Result<RpcResponse> {
        let mut base = 1;

        for _ in 0..3 {
            match self.send_once(req.clone()).await {
                Ok(res) => return Ok(res),
                Err(e) => {
                    log::debug!("failed to execute request: {}", e);

                    // only retry if there is a limit error
                    let errs = match e {
                        Error::NoHealthyEndpoints(e) => e,
                        _ => return Err(e),
                    };

                    if errs
                        .iter()
                        .all(|e| !matches!(e, Error::EndpointLimitTooLow))
                    {
                        return Err(Error::NoHealthyEndpoints(errs));
                    }
                }
            }

            let secs = Duration::from_secs(base);
            let millis = Duration::from_millis(fastrange_rs::fastrange_64(rand::random(), 1000));

            sleep(secs + millis).await;

            base = cmp::min(base + 1, 5);
        }

        Err(Error::RetriesFailed)
    }
}
