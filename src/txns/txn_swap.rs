use ethers_flashbots::{BundleRequest, PendingBundleError};
use rlp;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::env_setup::provider::http_node_endpoint;
use crate::txns::contracts::{
    deadline_timestamp, load_client_middleware, load_uniswap_v2_mempool, WETH_ADDRESS,
};

use ethers::prelude::*;
use ethers::types::{H256, U256};
use ethers_core::types::Eip1559TransactionRequest;
use ethers_signers::Signer;
use rand::Rng;
use serde_json::json;
use tokio::join;

pub const MAX_ATTEMPTS: usize = 5;
pub const INITIAL_DELAY: Duration = Duration::from_secs(5);
pub const MAX_DELAY: Duration = Duration::from_secs(60);

pub async fn uniswap_v2_transaction(
    value: U256,
    token_address: &str,
    slippage_percentage: f64,
    buy_extra_gas: U256,
    miner_tip: U256,
    maxbuy_amount: U256,
    wallet: &LocalWallet,
    provider: Arc<Provider<Http>>,
    provider_ws: Arc<Provider<Ws>>,
) -> eyre::Result<()> {
    let (nonce_result, gas_details_result, uniswap_v2_contract_result, client_result) = join!(
        provider.get_transaction_count(wallet.address(), None),
        provider.estimate_eip1559_fees(None),
        load_uniswap_v2_mempool(wallet),
        load_client_middleware(&wallet, &wallet, provider_ws.clone()),
    );

    let nonce = nonce_result.map_err(|e| eyre::eyre!("Failed to get transaction count: {}", e))?;

    let (max_fee_per_gas, max_priority_fee_per_gas) =
        gas_details_result.map_err(|_| eyre::eyre!("Failed to estimate EIP-1559 fees"))?;

    let uniswap_v2_contract = uniswap_v2_contract_result.or_else(|e| {
        Err(eyre::eyre!(
            "Failed to load uniswap v2 mempool contract: {}",
            e
        ))
    })?;

    let client =
        client_result.or_else(|e| Err(eyre::eyre!("Failed to load client middleware: {}", e)))?;

    // Run the asynchronous operations in parallel

    let tokenaddress = H160::from_str(token_address)?;
    let path = vec![*WETH_ADDRESS, tokenaddress];

    let get_input_ether_method = uniswap_v2_contract
        .method::<_, Vec<U256>>("getAmountsIn", (maxbuy_amount, path.clone()))
        .or_else(|_| Err(eyre::eyre!("Uniswap V2 Router contract method not found")))?;

    let get_input_ether_result = get_input_ether_method.call().await;

    let get_input_ether = get_input_ether_result
        .map_err(|e| eyre::eyre!("Failed to get amount of Intokens: {}", e))?;

    println!("get_input_ether: {:?}", get_input_ether);

    let value_to_use = if value > get_input_ether[0] {
        get_input_ether[0]
    } else {
        value
    };

    println!("value: {:?}", value);

    let get_output_tokens_method = uniswap_v2_contract
        .method::<_, Vec<U256>>("getAmountsOut", (value_to_use, path.clone()))
        .or_else(|_| Err(eyre::eyre!("Uniswap V2 Router contract method not found")))?;

    let get_output_tokens_result = get_output_tokens_method.call().await;

    let get_output_tokens = get_output_tokens_result
        .map_err(|e| eyre::eyre!("Failed to get amount of Outtokens: {}", e))?;

    // Ensure get_output_tokens is not empty
    let last_token_value = match get_output_tokens.last() {
        Some(value) => *value,
        None => return Err(eyre::eyre!("get_output_tokens is empty")),
    };

    println!("Slippage percentage: {}", slippage_percentage);

    // Calculate the slippage adjustment
    let slippage_multiplier = U256::from((slippage_percentage * 1e18) as u128);
    let slippage_adjustment = last_token_value
        .checked_mul(slippage_multiplier)
        .and_then(|result| result.checked_div(U256::from(1_000_000_000_000_000_000u128)))
        .ok_or_else(|| eyre::eyre!("Overflow occurred during slippage adjustment calculation"))?;

    let amount_out_tokens = last_token_value
        .checked_sub(slippage_adjustment)
        .ok_or_else(|| eyre::eyre!("Overflow occurred during amount out tokens calculation"))?;

    let to = wallet.address();

    let call_data = uniswap_v2_contract
        .method::<_, H160>(
            "swapExactETHForTokensSupportingFeeOnTransferTokens",
            (amount_out_tokens, path.clone(), to, deadline_timestamp()),
        )
        .or_else(|_| Err(eyre::eyre!("Uniswap V2 Router contract method not found")))?;

    let transaction_data = call_data
        .calldata()
        .ok_or_else(|| eyre::eyre!("Failed to get calldata"))?;

    println!("Transaction data: {:?}", transaction_data);

    let estimated_gas = U256::from(313252);
    // // // Constructing the EIP1559 transaction
    let txn_request = Eip1559TransactionRequest::new()
        .from(wallet.address())
        .to(uniswap_v2_contract.address())
        .value(value_to_use)
        .gas(estimated_gas)
        .max_priority_fee_per_gas(max_priority_fee_per_gas + miner_tip)
        .max_fee_per_gas(max_fee_per_gas + buy_extra_gas)
        .data(transaction_data)
        .nonce(nonce);

    // Send the transaction
    let tx_hash = client
        .send_transaction(txn_request, None)
        .await
        .map_err(|e| eyre::eyre!("Failed to send transaction: {}", e))?;

    // Extract the transaction hash from the PendingTransaction
    let pending_tx_hash = tx_hash.tx_hash();

    // Check if the swap transaction was successful
    let mut delay = INITIAL_DELAY;

    let receipt = loop {
        let r = provider.get_transaction_receipt(pending_tx_hash).await;

        match &r {
            Ok(Some(_)) => break r,
            Ok(None) if delay <= MAX_DELAY => {
                println!("No receipt found, retrying in {:?}", delay);
                sleep(delay).await;
                delay = std::cmp::min(delay * 2, MAX_DELAY); // double the delay, but don't exceed MAX_DELAY
                continue;
            }
            Ok(None) => {
                eprintln!("Exceeded max attempts or max delay without finding a receipt.");
                break r;
            }
            Err(_) => {
                eprintln!("Error fetching receipt: {:?}", r);
                break r;
            }
        }
    };

    let receipt = receipt.unwrap();

    match receipt {
        Some(receipt) => match receipt.status {
            Some(status) if status == ethers::types::U64([1]) => {
                println!("Transaction succeeded {:?}", receipt.transaction_hash);
            }
            Some(status) => {
                return Err(eyre::eyre!(
                    "Transaction reverted with status: {:?}",
                    status
                ))
            }
            None => return Err(eyre::eyre!("No transaction status found")),
        },
        None => return Err(eyre::eyre!("No transaction receipt found")),
    }

    Ok(())
}
