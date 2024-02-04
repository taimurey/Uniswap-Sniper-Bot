use ethers::{
    prelude::k256::SecretKey,
    providers::{Http, Provider, Ws},
    signers::LocalWallet,
    types::U256,
};
use hex::decode;
use log::info;
use serde::Deserialize;
use spinners::{Spinner, Spinners};
use std::{collections::HashMap, fs::File, io::Read, sync::Arc, time::Duration};
use tokio::time::sleep;

use crate::core::{
    contracts::CustomError, private_txn::uniswap_v2_bundler, public_txn::uniswap_v2_transaction,
};

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Settings {
    pub wallets: HashMap<String, String>,
    pub tokenToBuy: String,
    pub slippage: f64,
    pub autoSlippage: bool,
    pub amountOfETHToBuy: HashMap<String, f64>,
    pub BuyExtraGas: f64,
    pub MinerTip: f64,
    pub delayBetweenEachWalletBuy: u64,
    pub numberOfRounds: u32,
    pub PrivateTransaction: bool,
    pub rpc: RpcSettings,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct RpcSettings {
    pub Url_Https: String,
    pub Url_Wss: String,
}

pub async fn app() -> eyre::Result<(Settings, HashMap<String, LocalWallet>)> {
    let mut file = File::open("settings.json")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let settings: Settings = serde_json::from_str(&contents)?;

    let mut wallet_secret_keys = HashMap::new();

    for (wallet, address) in settings.wallets.iter() {
        let wallet_private_key_bytes = match decode(address) {
            Ok(bytes) => bytes,
            Err(_) => {
                return Err(eyre::eyre!(
                    "Failed to decode WALLET_PRIVATE_KEY for {}",
                    wallet
                ))
            }
        };

        let wallet_secret_key = SecretKey::from_slice(&wallet_private_key_bytes)
            .map_err(|e| {
                Box::new(CustomError(format!(
                    "Failed to create SecretKey from WALLET_PRIVATE_KEY: {}",
                    e
                ))) as Box<dyn std::error::Error + Send>
            })
            .unwrap();

        wallet_secret_keys.insert(wallet.clone(), LocalWallet::from(wallet_secret_key));
    }

    Ok((settings, wallet_secret_keys))
}

pub async fn run_app_and_swap() -> eyre::Result<()> {
    info!("Fetching JSON settings...");
    let (settings, wallet_secret_keys) = match app().await {
        Ok((settings, wallet_secret_keys)) => (settings, wallet_secret_keys),
        Err(e) => {
            println!("Error: {}", e);
            return Err(e);
        }
    };

    let provider = Arc::new(Provider::<Http>::try_from(&settings.rpc.Url_Https)?);

    for _ in 0..settings.numberOfRounds {
        for (wallet, secret_key) in wallet_secret_keys.iter() {
            let value = U256::from((settings.amountOfETHToBuy[wallet] * 1e18 as f64) as u128);
            let token_address = &settings.tokenToBuy;
            let slippage_percentage = if settings.autoSlippage {
                0.5
            } else {
                settings.slippage
            };
            let buy_extra_gas = U256::from((settings.BuyExtraGas * 1e9 as f64) as u128);
            let miner_tip = U256::from((settings.MinerTip * 1e9 as f64) as u128);
            let maxbuy_amount = value;

            if settings.PrivateTransaction {
                uniswap_v2_bundler(
                    value,
                    token_address,
                    slippage_percentage,
                    buy_extra_gas,
                    miner_tip,
                    maxbuy_amount,
                    secret_key,
                    Arc::clone(&provider),
                )
                .await?;
            } else {
                uniswap_v2_transaction(
                    value,
                    token_address,
                    slippage_percentage,
                    buy_extra_gas,
                    miner_tip,
                    maxbuy_amount,
                    secret_key,
                    Arc::clone(&provider),
                )
                .await?;
            }
        }

        // Delay between each round.
        sleep(Duration::from_secs(settings.delayBetweenEachWalletBuy)).await;
    }

    Ok(())
}
