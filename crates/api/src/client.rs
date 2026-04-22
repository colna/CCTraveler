use crate::types::{ApiRequest, ApiResponse};
use anyhow::Result;

/// API client trait — abstracts over LLM providers.
/// Full implementation comes in P1 when we add the Agent conversation loop.
pub trait ApiClient {
    fn send(&self, request: &ApiRequest) -> Result<ApiResponse>;
}
