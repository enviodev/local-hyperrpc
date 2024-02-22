use super::{
    EndpointConfig, Error, GetBlockNumber, LimitConfig, Result, RpcRequest, RpcRequestImpl,
    RpcResponse,
};
use reqwest::Method;
use skar_format::BlockNumber;
use std::{
    cmp,
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use url::Url;

pub struct Endpoint {
    url: Arc<Url>,
    last_block: Arc<RwLock<Option<BlockNumber>>>,
    job_tx: mpsc::Sender<Job>,
    label: String,
}

impl Endpoint {
    pub fn new(http_client: reqwest::Client, config: EndpointConfig) -> Self {
        let last_block = Arc::new(RwLock::new(None));
        let url = Arc::new(config.url);
        // Make label default to the url if not specified
        let label = config.label.unwrap_or_else(|| url.to_string());
        let bearer_token = config.bearer_token.map(Arc::new);

        tokio::spawn(
            WatchHealth {
                http_client: http_client.clone(),
                last_block: last_block.clone(),
                status_refresh_interval_secs: config.status_refresh_interval_secs,
                url: url.clone(),
                label: label.clone(),
                bearer_token: bearer_token.clone(),
            }
            .watch(),
        );

        let (job_tx, job_rx) = mpsc::channel(1);

        tokio::spawn(
            Listen {
                http_client,
                job_rx,
                limit_config: config.limit,
                window_num_reqs: 0,
                last_limit_refresh: Instant::now(),
                url: url.clone(),
                bearer_token,
            }
            .listen(),
        );

        Self {
            url,
            last_block,
            job_tx,
            label,
        }
    }

    pub async fn last_block(&self) -> Option<BlockNumber> {
        *self.last_block.read().await
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub async fn send(&self, req: Arc<RpcRequest>) -> Result<RpcResponse> {
        self.send_impl(req).await
    }

    pub async fn send_impl(&self, req: Arc<RpcRequest>) -> Result<RpcResponse> {
        if let Some(requirement) = Self::calculate_required_last_block(&req) {
            match *self.last_block.read().await {
                Some(last_block) => {
                    if requirement > last_block {
                        return Err(Error::EndpointTooBehind);
                    }
                }
                None => return Err(Error::EndpointUnavailable),
            }
        }

        let (res_tx, mut res_rx) = mpsc::channel(1);

        self.job_tx.send(Job { res_tx, req }).await.ok().unwrap();

        res_rx.recv().await.unwrap()
    }

    fn calculate_required_last_block(req: &RpcRequest) -> Option<BlockNumber> {
        match req {
            RpcRequest::Single(req) => Self::calculate_required_last_block_impl(req),
            RpcRequest::Batch(reqs) => reqs.iter().fold(None, |acc, req| {
                match (acc, Self::calculate_required_last_block_impl(req)) {
                    (Some(a), Some(b)) => Some(cmp::max(a, b)),
                    (None, v) => v,
                    (v, None) => v,
                }
            }),
        }
    }

    fn calculate_required_last_block_impl(req: &RpcRequestImpl) -> Option<BlockNumber> {
        match req {
            RpcRequestImpl::GetBlockNumber => None,
            RpcRequestImpl::GetBlockByNumber(block_number) => Some(*block_number),
            RpcRequestImpl::GetTransactionReceipt(block_number, _) => Some(*block_number),
            RpcRequestImpl::GetBlockReceipts(block_number) => Some(*block_number),
            RpcRequestImpl::TraceBlock(block_number) => Some(*block_number),
        }
    }
}

struct WatchHealth {
    url: Arc<Url>,
    label: String,
    bearer_token: Option<Arc<String>>,
    http_client: reqwest::Client,
    last_block: Arc<RwLock<Option<BlockNumber>>>,
    status_refresh_interval_secs: NonZeroU64,
}

impl WatchHealth {
    async fn watch(self) {
        loop {
            let (res_tx, mut res_rx) = mpsc::channel(1);

            let req = Arc::new(GetBlockNumber.into());

            tokio::spawn(
                SendRpcRequest {
                    url: self.url.clone(),
                    bearer_token: self.bearer_token.clone(),
                    http_client: self.http_client.clone(),
                    job: Job { res_tx, req },
                }
                .send(),
            );

            match res_rx.recv().await.unwrap() {
                Ok(resp) => {
                    let height = resp.try_into_single().unwrap();
                    *self.last_block.write().await = Some(height);
                }
                Err(e) => {
                    *self.last_block.write().await = None;
                    log::warn!(
                        "Failed to get last block for {}. Caused By: {}",
                        self.url,
                        e
                    );
                }
            }

            tokio::time::sleep(Duration::from_secs(self.status_refresh_interval_secs.get())).await;
        }
    }
}

struct Job {
    req: Arc<RpcRequest>,
    res_tx: mpsc::Sender<Result<RpcResponse>>,
}

struct Listen {
    url: Arc<Url>,
    bearer_token: Option<Arc<String>>,
    http_client: reqwest::Client,
    job_rx: mpsc::Receiver<Job>,
    limit_config: LimitConfig,
    window_num_reqs: usize,
    last_limit_refresh: Instant,
}

impl Listen {
    async fn listen(mut self) {
        while let Some(job) = self.job_rx.recv().await {
            if let Err(e) = self.update_limit(&job.req) {
                tokio::spawn(async move {
                    job.res_tx.send(Err(e)).await.ok();
                });
                continue;
            }

            tokio::spawn(
                SendRpcRequest {
                    http_client: self.http_client.clone(),
                    job,
                    url: self.url.clone(),
                    bearer_token: self.bearer_token.clone(),
                }
                .send(),
            );
        }
    }

    fn update_limit(&mut self, req: &RpcRequest) -> Result<()> {
        let needed_reqs = self.calculate_needed_reqs(req);

        if self.last_limit_refresh.elapsed().as_millis()
            >= u128::from(self.limit_config.req_limit_window_ms.get())
        {
            self.last_limit_refresh = Instant::now();
            self.window_num_reqs = 0;
        }

        if self.window_num_reqs + needed_reqs.get() < self.limit_config.req_limit.get() {
            self.window_num_reqs += needed_reqs.get();
            Ok(())
        } else {
            Err(Error::EndpointLimitTooLow)
        }
    }

    fn calculate_needed_reqs(&self, req: &RpcRequest) -> NonZeroUsize {
        match req {
            RpcRequest::Single(req) => self.calculate_needed_reqs_impl(req),
            RpcRequest::Batch(reqs) => {
                let needed_reqs_for_batch = |batch: &[RpcRequestImpl]| {
                    // start folding from 1 and add any extra required requests
                    batch.iter().fold(1, |acc, req| {
                        acc + self.calculate_needed_reqs_impl(req).get() - 1
                    })
                };

                let needed_reqs = reqs
                    .chunks(self.limit_config.batch_size_limit.get())
                    .map(needed_reqs_for_batch)
                    .sum();

                NonZeroUsize::new(needed_reqs).unwrap()
            }
        }
    }

    fn calculate_needed_reqs_impl(&self, _req: &RpcRequestImpl) -> NonZeroUsize {
        NonZeroUsize::new(1).unwrap()
    }
}

struct SendRpcRequest {
    url: Arc<Url>,
    bearer_token: Option<Arc<String>>,
    http_client: reqwest::Client,
    job: Job,
}

impl SendRpcRequest {
    async fn send(self) {
        let res_tx = self.job.res_tx.clone();
        let res = self.send_impl().await;

        if let Err(e) = res.as_ref() {
            let req: serde_json::Value = self.job.req.as_ref().into();
            let req_str = serde_json::to_string(&req)
                .unwrap_or_else(|_| "Failed to serialize request".to_string());
            log::warn!(
                "rpc request failed: {:?} . The request body was: {} . The url was: {}",
                e,
                req_str,
                self.url.as_str()
            );
        }

        res_tx.send(res).await.ok();
    }

    async fn send_impl(&self) -> Result<RpcResponse> {
        let json: serde_json::Value = self.job.req.as_ref().into();

        let mut req = self
            .http_client
            .request(Method::POST, Url::clone(&self.url));

        if let Some(bearer_token) = &self.bearer_token {
            req = req.bearer_auth(bearer_token);
        }

        let res = req
            .json(&json)
            .send()
            .await
            .map_err(Error::HttpRequest)?
            .text()
            .await
            .map_err(Error::HttpRequest)?;

        let json = tokio::task::block_in_place(|| self.job.req.resp_from_json(res.clone()));

        match json {
            Ok(json) => Ok(json),
            Err(e) => {
                log::warn!("failed to parse rpc response: {res}");
                Err(Error::InvalidRPCResponse(e))
            }
        }
    }
}

fn method_name(req: &RpcRequest) -> Option<String> {
    match req {
        RpcRequest::Batch(reqs) => reqs.first().map(|req| method_name_single(req).to_owned()),
        RpcRequest::Single(req) => Some(method_name_single(req).to_owned()),
    }
}

fn method_name_single(req: &RpcRequestImpl) -> &'static str {
    match req {
        RpcRequestImpl::GetBlockNumber => "eth_getBlockNumber",
        RpcRequestImpl::GetBlockByNumber(_) => "eth_getBlockByNumber",
        RpcRequestImpl::GetTransactionReceipt(_, _) => "eth_getTransactionReceipt",
        RpcRequestImpl::GetBlockReceipts(_) => "eth_getBlockReceipts",
        RpcRequestImpl::TraceBlock(_) => "trace_block",
    }
}
