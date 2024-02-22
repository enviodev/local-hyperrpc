use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::types::RpcResponse;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RpcErrorCode {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum RpcError {
    ParseError(String),
    MethodNotFound(String),
    InternalError(Arc<anyhow::Error>),
    InvalidParams(String),
    JsonRpcVersionNotSupported(String),
    LimitExceeded(String),
}

impl PartialEq for RpcError {
    fn eq(&self, other: &Self) -> bool {
        use RpcError::*;

        match (self, other) {
            (ParseError(a), ParseError(b)) => a == b,
            (MethodNotFound(a), MethodNotFound(b)) => a == b,
            (InternalError(a), InternalError(b)) => a.to_string() == b.to_string(),
            (InvalidParams(a), InvalidParams(b)) => a == b,
            (JsonRpcVersionNotSupported(a), JsonRpcVersionNotSupported(b)) => a == b,
            (LimitExceeded(a), LimitExceeded(b)) => a == b,
            _ => false,
        }
    }
}

impl RpcError {
    pub fn to_response(&self, req_id: &i64) -> RpcResponse {
        RpcResponse {
            id: *req_id,
            jsonrpc: "2.0".into(),
            result: Err(self.code()),
        }
    }

    pub fn code(&self) -> RpcErrorCode {
        match self {
            RpcError::ParseError(msg) => RpcErrorCode {
                code: -32700,
                message: format!("Invalid JSON: {}", msg),
            },
            RpcError::MethodNotFound(msg) => RpcErrorCode {
                code: -32601,
                message: format!("Method not found: {}", msg),
            },
            RpcError::InvalidParams(msg) => RpcErrorCode {
                code: -32602,
                message: format!("Invalid params: {}", msg),
            },
            RpcError::InternalError(msg) => RpcErrorCode {
                code: -32603,
                message: format!("Internal error: {:?}", msg),
            },
            RpcError::JsonRpcVersionNotSupported(msg) => RpcErrorCode {
                code: -32006,
                message: format!("JSON-RPC version not supported: {}", msg),
            },
            RpcError::LimitExceeded(msg) => RpcErrorCode {
                code: -32005,
                message: format!("Limit exceeded: {}", msg),
            },
        }
    }
}
