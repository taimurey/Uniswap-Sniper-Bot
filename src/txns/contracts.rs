use ethers::{
    contract::Contract,
    prelude::{NonceManagerMiddleware, SignerMiddleware},
    providers::{Http, Provider, Ws},
    signers::{LocalWallet, Signer},
    types::H160,
};
use ethers_flashbots::{BroadcasterMiddleware, FlashbotsMiddleware};
use regex::Regex;
use std::fs;
use std::result::Result;
use std::str::FromStr;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use url::Url;
pub fn deadline_timestamp() -> u64 {
    let deadline = SystemTime::now() + Duration::from_secs(60 * 1); // 3 minutes from now
    deadline
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
pub type ConfigContractmempool = Contract<SignerMiddleware<Provider<Ws>, LocalWallet>>;
#[derive(Debug)]
pub struct CustomError(pub String);

impl std::fmt::Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CustomError {}
pub type StandardMiddlewareProvider =
    SignerMiddleware<Arc<NonceManagerMiddleware<Arc<Provider<Ws>>>>, LocalWallet>;

pub type BroadcasterMiddlewareProvider = SignerMiddleware<
    BroadcasterMiddleware<Arc<ethers_providers::Provider<ethers_providers::Http>>, LocalWallet>,
    LocalWallet,
>;

pub type FlashbotsMiddlewareProvider = SignerMiddleware<
    FlashbotsMiddleware<Arc<ethers_providers::Provider<ethers_providers::Http>>, LocalWallet>,
    LocalWallet,
>;

pub const _ZERO_ADDRESS: [u8; 20] = [0u8; 20];

lazy_static::lazy_static! {
    pub static ref UNISWAP_V2_ROUTER: H160 = H160::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").expect("Failed to create v2 router address from string");
    pub static ref WETH_ADDRESS: H160 = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").expect("Failed to create weth address from string");


    pub static ref UNISWAP_V2_ROUTER_02: String = fs::read_to_string("./abi/uniswapV2Router02_ABI.json")
        .expect("Unable to read Uniswap V2 ABI file");



        pub static ref ETH_ADDRESS_REGEX: Regex = Regex::new(r"^0x[a-fA-F0-9]{40}$").unwrap();


        pub static ref UNISWAP_V2_PAIR: String = fs::read_to_string("./abi/IUniswapV2PairABI.json")
        .expect("Unable to read Uniswap V2 ABI file");


        pub static ref ERC20: String = fs::read_to_string("./abi/ERC20_ABI.json")
        .expect("Unable to read ERC20 ABI file");

        pub static ref ERC20_MINIMAL: String = fs::read_to_string("./abi/ERC20_MINIMAL_ABI.json")
        .expect("Unable to read ERC20 ABI file");

}
pub async fn load_uniswap_v2_mempool(
    wallet: &LocalWallet,
) -> Result<ConfigContractmempool, Box<dyn std::error::Error + Send>> {
    let v2_router_contract_abi =
        ethabi::Contract::load(UNISWAP_V2_ROUTER_02.as_bytes()).map_err(|e| {
            Box::new(CustomError(format!(
                "Failed to load v2 router contract ABI: {}",
                e
            ))) as Box<dyn std::error::Error + Send>
        })?;

    let v2_router_address =
        H160::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").map_err(|e| {
            Box::new(CustomError(format!(
                "Failed to create v2 router address from string: {}",
                e
            ))) as Box<dyn std::error::Error + Send>
        })?;

    uniswap_v2_router_mempool(wallet, v2_router_address, v2_router_contract_abi).await
}

async fn uniswap_v2_router_mempool(
    wallet: &LocalWallet,
    v2_router_address: H160,
    v2_router_contract_abi: ethabi::Contract,
) -> Result<ConfigContractmempool, Box<dyn std::error::Error + Send>> {
    let endpoint = match crate::env_setup::provider::wss_node_endpoint() {
        Ok(ep) => ep,
        Err(e) => {
            return Err(
                Box::new(CustomError(format!("Failed to get node endpoint: {}", e)))
                    as Box<dyn std::error::Error + Send>,
            )
        }
    };

    // Handle the possible error here
    let provider = Provider::<Ws>::connect(&endpoint).await.map_err(|e| {
        Box::new(CustomError(format!("Provider connection error: {}", e)))
            as Box<dyn std::error::Error + Send>
    })?;

    // Uniswap V2 Router for regular transactions through the mempool
    let uniswap_v2_router_mempool = Contract::new(
        v2_router_address,
        v2_router_contract_abi,
        SignerMiddleware::new(provider, wallet.clone()).into(),
    );

    Ok(uniswap_v2_router_mempool)
}

pub async fn load_client_middleware(
    _bundle_signer: &LocalWallet,
    wallet: &LocalWallet,
    provider: Arc<Provider<Ws>>,
) -> Result<StandardMiddlewareProvider, Box<dyn std::error::Error + Send>> {
    create_client_middleware(wallet, provider).await
}

pub async fn load_flashbots_client_middleware(
    bundle_signer: &LocalWallet,
    wallet: &LocalWallet,
    provider: Arc<Provider<Http>>,
) -> Result<
    (BroadcasterMiddlewareProvider, FlashbotsMiddlewareProvider),
    Box<dyn std::error::Error + Send>,
> {
    create_flashbots_client_middleware(bundle_signer, wallet, provider).await
}

async fn create_client_middleware(
    wallet: &LocalWallet,
    provider: Arc<Provider<Ws>>,
) -> Result<StandardMiddlewareProvider, Box<dyn std::error::Error + Send>> {
    let client = NonceManagerMiddleware::new(provider, wallet.address());

    let client_middleware = SignerMiddleware::new(Arc::new(client), wallet.clone());

    Ok(client_middleware)
}

async fn create_flashbots_client_middleware(
    bundle_signer: &LocalWallet,
    wallet: &LocalWallet,
    provider: Arc<Provider<Http>>,
) -> Result<
    (BroadcasterMiddlewareProvider, FlashbotsMiddlewareProvider),
    Box<dyn std::error::Error + Send>,
> {
    let builders = vec![
        parse_url("https://builder0x69.io")?,
        parse_url("https://rpc.beaverbuild.org")?,
        parse_url("https://relay.flashbots.net")?,
        parse_url("https://rsync-builder.xyz")?,
        parse_url("https://api.blocknative.com/v1/auction")?,
        parse_url("https://builder.gmbit.co/rpc")?,
        parse_url("https://eth-builder.com")?,
        parse_url("https://rpc.titanbuilder.xyz")?,
        parse_url("https://buildai.net")?,
        parse_url("https://rpc.payload.de")?,
        parse_url("https://mev.api.blxrbdn.com")?,
        parse_url("https://rpc.lightspeedbuilder.info")?,
        parse_url("https://rpc.nfactorial.xyz")?,
        parse_url("https://boba-builder.com/searcher")?,
        parse_url("https://rpc.f1b.io")?,
    ];

    let relay_url = Url::parse("https://relay.flashbots.net").map_err(|e| {
        Box::new(CustomError(format!("Failed to parse URL: {}", e)))
            as Box<dyn std::error::Error + Send>
    })?;

    let _client = Arc::new(NonceManagerMiddleware::new(
        provider.clone(),
        wallet.address(),
    ));

    let flashbots_middleware = SignerMiddleware::new(
        FlashbotsMiddleware::new(provider.clone(), relay_url.clone(), bundle_signer.clone()),
        wallet.clone(),
    );

    let broadcaster =
        BroadcasterMiddleware::new(provider, builders, relay_url.clone(), bundle_signer.clone());

    // Combine with SignerMiddleware
    let client_middleware = SignerMiddleware::new(broadcaster, wallet.clone());

    Ok((client_middleware, flashbots_middleware))
}

fn parse_url(url_str: &str) -> Result<Url, Box<dyn std::error::Error + Send>> {
    Url::parse(url_str).map_err(|e| {
        Box::new(CustomError(format!("Failed to parse URL: {}", e)))
            as Box<dyn std::error::Error + Send>
    })
}
