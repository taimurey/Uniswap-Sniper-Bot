use ethers_providers::Http;
use std::env;

pub fn wss_node_endpoint() -> Result<String, env::VarError> {
    env::var("WSS_LOCAL_NODE_ENDPOINT")
}

pub fn wss_alchemy_node_endpoint() -> Result<String, env::VarError> {
    env::var("WSS_NODE_ENDPOINT")
}
use ethers::prelude::Provider;
pub fn http_node_endpoint() -> eyre::Result<Provider<Http>> {
    let endpoint = env::var("HTTP_NODE_ENDPOINT")?;
    let provider = Provider::<Http>::try_from(endpoint)?;
    Ok(provider)
}
